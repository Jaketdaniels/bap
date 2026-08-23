//! `bap report compat`: print the measured npm-compatibility table.

use serde_json::Value;
use std::path::PathBuf;

const DEFAULT_RESULTS: &str = "bench/compat/results.json";

/// Handles `bap report ...`; returns the process exit code (report-only: 0).
pub fn report(input: Option<String>) -> i32 {
    let path = PathBuf::from(input.unwrap_or_else(|| DEFAULT_RESULTS.to_owned()));
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!(
            "bap: no results at {}; run bench/compat/run.sh first",
            path.display()
        );
        return 1;
    };
    let Ok(report) = serde_json::from_str::<Value>(&text) else {
        eprintln!("bap: {} is not valid JSON", path.display());
        return 1;
    };

    println!("bap npm-compatibility report ({})", path.display());
    if let Some(generated) = report["generatedAt"].as_str() {
        println!("  generated {generated}");
    }
    let results = match report["results"].as_array() {
        Some(results) => results,
        None => {
            eprintln!("bap: report has no results array");
            return 1;
        }
    };

    println!();
    println!("  {:<24} {:>12} {:>10}", "package", "status", "time");
    for entry in results {
        let name = entry["name"].as_str().unwrap_or("?");
        let status = entry["status"].as_str().unwrap_or("?");
        let duration = entry["durationMs"].as_u64().unwrap_or(0);
        println!("  {name:<24} {status:>12} {duration:>8} ms");
    }

    let total = report["total"].as_u64().unwrap_or(0);
    let passed = report["passed"].as_u64().unwrap_or(0);
    let ratio = report["ratio"].as_str().unwrap_or("?");
    println!();
    println!("  aggregate: {passed}/{total} passed ({ratio})");
    println!(
        "  scope: pinned local corpus over the node: shim surface; remote npm\n  entries activate with the package-manager milestone. The spec's >=99%\n  gate applies once that corpus runs end to end."
    );
    0
}
