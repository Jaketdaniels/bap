use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_bap");

fn hello_path() -> String {
    format!("{}/../../examples/hello.js", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn run_hello_prints_stdout() {
    let out = Command::new(BIN)
        .args(["run", &hello_path()])
        .output()
        .expect("spawn bap");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("hello from bap"));
    assert!(stdout.contains("sum: 3"));
}

#[test]
fn metrics_json_reports_console_ops() {
    let out = Command::new(BIN)
        .args(["--metrics", "json", "run", &hello_path()])
        .output()
        .expect("spawn bap");
    assert!(out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    let marker = stderr.find("\"cold_start_ms\"").expect("cold_start in metrics json");
    let start = stderr[..marker].rfind('{').expect("metrics json opening brace");
    let snap: serde_json::Value =
        serde_json::from_str(&stderr[start..]).expect("metrics json on stderr");
    assert!(snap["cold_start_ms"].as_f64().is_some());
    assert!(snap["ops"]["console.log"]["calls"].as_u64().unwrap() >= 2);
}
