//! `bap run` execution and metrics reporting.

use crate::cli::MetricsFormat;
use bap_core::BapRuntime;
use bap_metrics::{MetricsCollector, MetricsSnapshot};
use bap_ts::TsModuleLoader;
use std::rc::Rc;

/// Runs the file at `path` with `script_args` and returns the process exit code.
/// Reports the snapshot to stderr on success and `bap: {error}` on failure.
pub fn run_file(
    path: &str,
    script_args: Vec<String>,
    format: MetricsFormat,
    metrics: MetricsCollector,
) -> i32 {
    let loader = match TsModuleLoader::new(std::env::temp_dir().join("bap-ts-cache-v1")) {
        Ok(loader) => loader,
        Err(error) => {
            eprintln!("bap: load: failed to initialize TypeScript cache: {error}");
            return 1;
        }
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");
    let result = rt.block_on(async {
        let mut runtime =
            BapRuntime::with_args_and_loader(metrics.clone(), script_args, Rc::new(loader));
        let result = runtime.run_script(path).await;
        let (used, total) = runtime.sample_heap();
        metrics.record_heap(used, total);
        result
    });
    match result {
        Ok(()) => {
            report(format, &metrics.snapshot());
            0
        }
        Err(err) => {
            eprintln!("bap: {err}");
            1
        }
    }
}

/// Prints the snapshot to stderr in `format`.
pub fn report(format: MetricsFormat, snap: &MetricsSnapshot) {
    match format {
        MetricsFormat::Pretty => {
            eprintln!("--- bap metrics ---");
            eprintln!("cold start: {:.3} ms", snap.cold_start_ms);
            eprintln!("heap samples: {}", snap.heap_samples.len());
            for (name, stat) in &snap.ops {
                let tail = match (stat.p50_ns, stat.p99_ns) {
                    (Some(p50), Some(p99)) => {
                        format!(", {} us p50, {} us p99", p50 / 1_000, p99 / 1_000)
                    }
                    _ => String::new(),
                };
                eprintln!(
                    "op {name}: {} calls, {} us total, {} us max{tail}",
                    stat.calls,
                    stat.total_ns / 1_000,
                    stat.max_ns / 1_000
                );
            }
        }
        MetricsFormat::Json => {
            eprintln!(
                "{}",
                serde_json::to_string_pretty(snap).expect("metrics serialize to json")
            );
        }
    }
}
