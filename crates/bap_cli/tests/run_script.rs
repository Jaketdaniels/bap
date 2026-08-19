use std::fs;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_bap");

fn hello_path() -> String {
    format!("{}/../../examples/hello.js", env!("CARGO_MANIFEST_DIR"))
}

fn fixture(path: &str) -> String {
    format!("{}/tests/fixtures/{path}", env!("CARGO_MANIFEST_DIR"))
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
    let marker = stderr
        .find("\"cold_start_ms\"")
        .expect("cold_start in metrics json");
    let start = stderr[..marker]
        .rfind('{')
        .expect("metrics json opening brace");
    let snap: serde_json::Value =
        serde_json::from_str(&stderr[start..]).expect("metrics json on stderr");
    assert!(snap["cold_start_ms"].as_f64().is_some());
    assert!(snap["ops"]["console.log"]["calls"].as_u64().unwrap() >= 2);
}

#[test]
fn missing_file_exits_nonzero_with_bap_prefix() {
    let out = Command::new(BIN)
        .args(["run", "no-such-file.js"])
        .output()
        .expect("spawn bap");
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("bap: load: "), "stderr was: {stderr}");
}

#[test]
fn stdout_and_stderr_stay_separate() {
    let dir = std::env::temp_dir().join(format!("bap_streams_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("streams.js");
    fs::write(
        &file,
        "console.log('out-only');\nconsole.error('err-only');\n",
    )
    .unwrap();
    let out = Command::new(BIN)
        .args(["run", file.to_str().unwrap()])
        .output()
        .expect("spawn bap");
    fs::remove_dir_all(&dir).ok();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert_eq!(stdout.trim(), "out-only");
    assert!(stdout.contains("out-only") && !stdout.contains("err-only"));
    assert!(stderr.contains("err-only") && !stderr.contains("out-only"));
}

#[test]
fn script_args_reach_bap_global() {
    let dir = std::env::temp_dir().join(format!("bap_args_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("args.js");
    fs::write(&file, "console.log(JSON.stringify(Bap.args));\n").unwrap();
    let out = Command::new(BIN)
        .args(["run", file.to_str().unwrap(), "alpha", "beta"])
        .output()
        .expect("spawn bap");
    fs::remove_dir_all(&dir).ok();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), "[\"alpha\",\"beta\"]");
}

#[test]
fn metrics_flag_after_subcommand_is_accepted() {
    let out = Command::new(BIN)
        .args(["run", "--metrics", "json", &hello_path()])
        .output()
        .expect("spawn bap");
    assert!(out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("\"cold_start_ms\""));
}

#[test]
fn runs_typescript_entry() {
    let out = Command::new(BIN)
        .args(["run", &fixture("ts/entry.ts")])
        .output()
        .expect("spawn bap");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "typed 42\n");
}

#[test]
fn js_entry_imports_typescript_dependency() {
    let out = Command::new(BIN)
        .args(["run", &fixture("ts/mixed.js")])
        .output()
        .expect("spawn bap");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "typed 42\n");
}
