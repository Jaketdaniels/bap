//! `bap bench`: lane-based latency measurement for a script.

use crate::cli::BenchFormat;
use crate::style;
use bap_bench::{self, BenchOptions};
use std::path::PathBuf;

/// Handles `bap bench <file>`; returns the process exit code.
pub fn bench(
    file: String,
    warmup: u32,
    iterations: u32,
    lanes: u32,
    script_args: Vec<String>,
    trace: Option<String>,
    format: BenchFormat,
) -> i32 {
    let script_path = PathBuf::from(&file);
    if !script_path.is_file() {
        eprintln!("bap: bench target {} does not exist or is not a file", file);
        return 1;
    }
    let options = BenchOptions {
        script: script_path,
        warmup,
        iterations,
        lanes,
        args: script_args,
        env: Default::default(),
    };
    let expected = u64::from(iterations);
    let report = match bap_bench::run_with_progress(options, &|completed: u64| {
        eprint!("\r  {} {completed}/{expected}", style::dim("running"));
        if completed == expected {
            eprintln!();
        }
    }) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("bap: {error}");
            return 1;
        }
    };

    if let Some(path) = trace {
        let document = report.trace_document();
        if let Err(error) = std::fs::write(
            &path,
            serde_json::to_vec_pretty(&document).expect("trace json"),
        ) {
            eprintln!("bap: failed to write trace {path}: {error}");
            return 1;
        }
    }

    match format {
        BenchFormat::Pretty => {
            println!("bap bench {}", file);
            println!(
                "  lanes={} warmup={} iterations={}",
                lanes, warmup, iterations
            );
            for (lane_id, heap) in &report.lane_heaps {
                println!("  lane {lane_id}: heap used {} KiB", heap.0 / 1024);
            }
            println!();
            println!(
                "  {:>10} {:>10} {:>10} {:>10} {:>8}",
                "count", "p50 ms", "p99 ms", "max ms", "cold"
            );
            println!(
                "  {:>10} {:>10.3} {:>10.3} {:>10.3} {:>8}",
                report.histogram.count(),
                report.histogram.p50() as f64 / 1_000_000.0,
                report.histogram.p99() as f64 / 1_000_000.0,
                report.histogram.max() as f64 / 1_000_000.0,
                report.warmup_samples.len(),
            );
            println!("  wall: {:.1} ms", report.wall_ns as f64 / 1_000_000.0);
        }
        BenchFormat::Json => {
            let payload = serde_json::json!({
                "file": file,
                "lanes": lanes,
                "warmup": warmup,
                "iterations": iterations,
                "count": report.histogram.count(),
                "p50Ns": report.histogram.p50(),
                "p99Ns": report.histogram.p99(),
                "maxNs": report.histogram.max(),
                "wallNs": report.wall_ns,
                "laneHeaps": report.lane_heaps.iter()
                    .map(|(lane, heap)| serde_json::json!({
                        "lane": lane, "usedBytes": heap.0, "totalBytes": heap.1,
                    }))
                    .collect::<Vec<_>>(),
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&payload).expect("bench json")
            );
        }
    }
    0
}
