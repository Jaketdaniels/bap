//! Lane-based benchmark orchestration over `BapRuntime`.

pub mod profile;
pub mod ssr;

use bap_core::BapRuntime;
use bap_metrics::MetricsCollector;
use bap_ts::TsModuleLoader;
use profile::{LatencyHistogram, ProfileScope, RequestSample, is_cold};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

/// What and how to benchmark.
#[derive(Clone, Debug)]
pub struct BenchOptions {
    /// Script executed once per request.
    pub script: PathBuf,
    /// Unmeasured executions per lane before the measured phase.
    pub warmup: u32,
    /// Measured requests distributed round-robin across lanes.
    pub iterations: u32,
    /// Long-lived isolates; each owns a thread and a runtime.
    pub lanes: u32,
    /// Script arguments exposed as `Bap.args`.
    pub args: Vec<String>,
    /// Extra environment for the workload (e.g. the fixture port).
    pub env: BTreeMap<String, String>,
}

/// Aggregate result of one benchmark run.
#[derive(Debug)]
pub struct BenchReport {
    pub measured_samples: Vec<RequestSample>,
    pub warmup_samples: Vec<RequestSample>,
    pub histogram: LatencyHistogram,
    /// Per-lane heap totals after the final request on that lane.
    pub lane_heaps: BTreeMap<u32, (usize, usize)>,
    pub wall_ns: u64,
}

impl BenchReport {
    /// Chrome-trace JSON events for the measured samples.
    pub fn trace_events(&self) -> Vec<serde_json::Value> {
        let mut events = Vec::new();
        let mut cursor_us = 0_u64;
        for sample in &self.measured_samples {
            let duration_us = sample.duration_ns / 1_000;
            events.push(serde_json::json!({
                "name": "ssr.route",
                "cat": "bap,ssr",
                "ph": "X",
                "ts": cursor_us,
                "dur": duration_us,
                "pid": 0,
                "tid": sample.lane_id,
                "args": {
                    "lane": sample.lane_id,
                    "index": sample.index,
                    "heapUsedAfter": sample.heap_after.0,
                },
            }));
            cursor_us += duration_us.max(1);
        }
        events
    }

    /// Full chrome-trace document.
    pub fn trace_document(&self) -> serde_json::Value {
        serde_json::json!({ "traceEvents": self.trace_events() })
    }
}

/// Executes the benchmark across lanes; each lane owns its thread and runtime.
pub fn run(options: BenchOptions) -> Result<BenchReport, String> {
    let lanes = options.lanes.max(1);
    // Workload environment applies once before lanes exist; `set_var` is
    // unsafe in edition 2024 because it is process-global, which is why this
    // runs single-threaded here rather than per lane.
    for (key, value) in &options.env {
        // SAFETY: no other thread exists yet; values are bench-owned.
        unsafe {
            std::env::set_var(key, value);
        }
    }
    let (sender, receiver) = mpsc::channel::<Result<RequestSample, String>>();
    let started = Instant::now();

    let handles: Vec<_> = (0..lanes)
        .map(|lane_id| {
            let options = options.clone();
            let sender = sender.clone();
            std::thread::spawn(move || {
                lane_main(lane_id, &options, sender);
            })
        })
        .collect();
    drop(sender);

    let mut measured_samples = Vec::new();
    let mut warmup_samples = Vec::new();
    let mut failures = Vec::new();
    for message in receiver {
        match message {
            Ok(sample) if sample.measured => measured_samples.push(sample),
            Ok(sample) => warmup_samples.push(sample),
            Err(error) => failures.push(error),
        }
    }
    for handle in handles {
        handle
            .join()
            .map_err(|_| "bench lane panicked".to_owned())?;
    }

    if !failures.is_empty() {
        return Err(failures.into_iter().next().expect("non-empty"));
    }

    let mut histogram = LatencyHistogram::new();
    for sample in &measured_samples {
        histogram.record(sample.duration_ns);
    }
    let lane_heaps = measured_samples
        .iter()
        .chain(warmup_samples.iter().rev())
        .map(|sample| (sample.lane_id, sample.heap_after))
        .collect();

    Ok(BenchReport {
        measured_samples,
        warmup_samples,
        histogram,
        lane_heaps,
        wall_ns: u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX),
    })
}

fn lane_main(
    lane_id: u32,
    options: &BenchOptions,
    sender: mpsc::Sender<Result<RequestSample, String>>,
) {
    let outcome = std::panic::catch_unwind(|| lane_run(lane_id, options, sender.clone()));
    if outcome.is_err() {
        let _ = sender.send(Err("lane thread failed".to_owned()));
    }
}

fn lane_run(
    lane_id: u32,
    options: &BenchOptions,
    sender: mpsc::Sender<Result<RequestSample, String>>,
) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime per lane");
    rt.block_on(async move {
        let metrics = MetricsCollector::new();
        let loader = TsModuleLoader::for_user();
        let mut runtime = BapRuntime::with_args_and_loader(
            metrics.clone(),
            options.args.clone(),
            std::rc::Rc::new(loader),
        );

        // TypeScript sources transpile once; every iteration evaluates a
        // fresh side module under a unique query so per-request work repeats
        // while relative imports keep resolving against the real directory.
        let raw = match std::fs::read_to_string(&options.script) {
            Ok(source) => source,
            Err(error) => {
                let _ = sender.send(Err(format!("lane {lane_id}: {error}")));
                return;
            }
        };
        let extension = options
            .script
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("js");
        let source = match extension {
            "ts" | "mts" => match bap_ts::transpile(
                &format!("file:///bench/{}", options.script.display()),
                &raw,
            ) {
                Ok(output) => output.code,
                Err(error) => {
                    let _ = sender.send(Err(format!("lane {lane_id}: {error:#}")));
                    return;
                }
            },
            _ => raw,
        };
        let base_specifier = deno_core::ModuleSpecifier::from_file_path(
            std::fs::canonicalize(&options.script).unwrap_or_else(|_| options.script.clone()),
        )
        .map(|url| url.to_string())
        .unwrap_or_else(|_| format!("file:///{}", options.script.display()));

        let total_executions = options.warmup + iterations_for_lane(lane_id, options);
        let mut measured_index = 0_u32;
        for step in 0..total_executions {
            let is_warmup_phase = step < options.warmup;
            let cold = is_cold(step);
            let mut scope = ProfileScope::begin(
                lane_id,
                if is_warmup_phase {
                    step
                } else {
                    measured_index
                },
                cold,
                !is_warmup_phase,
            );
            let heap_before = runtime.sample_heap();
            scope.observe_heap(heap_before.0, heap_before.1);
            let specifier = format!("{base_specifier}?__bap_step={step}");
            let result = runtime.run_script_source(&specifier, source.clone()).await;
            let heap_after = runtime.sample_heap();
            let sample = scope.finish(heap_after);
            let send_result = match result {
                Ok(()) => sender.send(Ok(sample)),
                Err(error) => sender.send(Err(format!("lane {lane_id}: {error}"))),
            };
            if send_result.is_err() {
                return;
            }
            if !is_warmup_phase {
                measured_index += 1;
                if measured_index >= iterations_for_lane(lane_id, options) {
                    break;
                }
            }
        }
    });
}

fn iterations_for_lane(lane_id: u32, options: &BenchOptions) -> u32 {
    if lane_id >= options.iterations {
        return 0;
    }
    (options.iterations - lane_id - 1) / options.lanes + 1
}
