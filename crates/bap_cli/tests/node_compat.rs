use std::ops::{Deref, DerefMut};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_bap");

fn fixture(path: &str) -> String {
    format!("{}/tests/fixtures/cjs/{path}", env!("CARGO_MANIFEST_DIR"))
}

struct TestCommand {
    command: Command,
    _cache: tempfile::TempDir,
}

impl Deref for TestCommand {
    type Target = Command;

    fn deref(&self) -> &Self::Target {
        &self.command
    }
}

impl DerefMut for TestCommand {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.command
    }
}

fn command() -> TestCommand {
    let cache = tempfile::tempdir().unwrap();
    let mut command = Command::new(BIN);
    command.env("BAP_TS_CACHE_DIR", cache.path());
    TestCommand {
        command,
        _cache: cache,
    }
}

#[test]
fn esm_imports_cjs_named_and_default_exports() {
    let out = command()
        .args(["run", &fixture("consumer.ts")])
        .output()
        .expect("spawn bap");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "cjs interop 42 bap-cjs\n"
    );
}

#[test]
fn require_cycle_values_are_consistent() {
    // Evaluate through an ESM driver so values can be printed.
    let dir = std::env::temp_dir().join(format!("bap_cycle_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("entry.ts");
    std::fs::write(
        &entry,
        "import a from \"./cycle-a.cjs\";\nimport b from \"./cycle-b.cjs\";\nconsole.log(a.startedWith, a.bValue, b.sawAStarted);\n",
    )
    .unwrap();
    for name in ["cycle-a.cjs", "cycle-b.cjs"] {
        std::fs::copy(fixture(name), dir.join(name)).unwrap();
    }
    let out = command()
        .arg("run")
        .arg(entry.as_os_str())
        .output()
        .expect("spawn bap");
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // b saw a's exports after `startedWith` was set, before `bValue` existed.
    assert_eq!(stdout.trim(), "a b false", "stdout was: {stdout}");
}

#[test]
fn cjs_requires_json_files() {
    let out = command()
        .args(["run", &fixture("require-json.cjs")])
        .output()
        .expect("spawn bap");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "json required\n");
}

#[test]
fn missing_npm_package_reports_clear_error() {
    let dir = std::env::temp_dir().join(format!("bap_npm_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("needs-lodash.js");
    std::fs::write(&file, "const _ = require('lodash');\n").unwrap();
    let out = command()
        .args(["run", file.to_str().unwrap()])
        .output()
        .expect("spawn bap");
    std::fs::remove_dir_all(&dir).ok();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("npm packages are not supported"),
        "stderr was: {stderr}"
    );
}
