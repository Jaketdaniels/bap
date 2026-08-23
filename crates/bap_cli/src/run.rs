//! `bap run` execution and metrics reporting.

use crate::cli::MetricsFormat;
use bap_core::BapRuntime;
use bap_metrics::{MetricsCollector, MetricsSnapshot};
use bap_ts::{ModuleKind, TsModuleLoader};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Runs the file at `path` and returns the process exit code.
pub fn run_file(
    path: &str,
    script_args: Vec<String>,
    loader: TsModuleLoader,
    require_aliases: BTreeMap<String, String>,
    format_override: Option<ModuleKind>,
    format: MetricsFormat,
    metrics: MetricsCollector,
) -> i32 {
    let _ = format_override; // classification happens inside the loader
    execute(path, script_args, loader, require_aliases, format, metrics)
}

fn execute(
    path: &str,
    script_args: Vec<String>,
    loader: TsModuleLoader,
    require_aliases: BTreeMap<String, String>,
    format: MetricsFormat,
    metrics: MetricsCollector,
) -> i32 {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");
    let result = rt.block_on(async {
        let mut runtime = BapRuntime::with_args_loader_aliases(
            metrics.clone(),
            script_args,
            Rc::new(loader),
            require_aliases,
        );
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
