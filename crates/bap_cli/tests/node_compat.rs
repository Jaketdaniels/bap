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

#[test]
fn node_style_example_runs() {
    let out = command()
        .arg("run")
        .arg(format!(
            "{}/../../examples/node-style.js",
            env!("CARGO_MANIFEST_DIR")
        ))
        .env("BAP_DEMO_TOKEN", "abc123")
        .args(["--", "demo"])
        .output()
        .expect("spawn bap");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("hello from events"), "{stdout}");
    assert!(
        stdout.contains("path.join -> /tmp/bap-demo/file.txt"),
        "{stdout}"
    );
    assert!(
        stdout.contains("fs roundtrip -> written by bap"),
        "{stdout}"
    );
    assert!(stdout.contains("process.platform="), "{stdout}");
}

#[test]
fn ts_imports_node_builtins() {
    let path = format!(
        "{}/tests/fixtures/node/builtin-imports.ts",
        env!("CARGO_MANIFEST_DIR")
    );
    let out = command().args(["run", &path]).output().expect("spawn bap");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "answer=42\n");
}

#[test]
fn fs_shims_roundtrip() {
    let path = format!(
        "{}/tests/fixtures/node/fs-shims.cjs",
        env!("CARGO_MANIFEST_DIR")
    );
    let out = command().args(["run", &path]).output().expect("spawn bap");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("roundtrip"), "{stdout}");
    assert!(stdout.contains("promises ok (9)"), "{stdout}");
}

#[test]
fn buffer_encodings_work() {
    let path = format!(
        "{}/tests/fixtures/node/buffer-encodings.cjs",
        env!("CARGO_MANIFEST_DIR")
    );
    let out = command().args(["run", &path]).output().expect("spawn bap");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn process_surface_reports_env_and_exits_with_code() {
    let dir = std::env::temp_dir().join(format!("bap_process_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let surface = dir.join("surface.cjs");
    std::fs::write(
        &surface,
        std::fs::read(format!(
            "{}/tests/fixtures/node/process-surface.cjs",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    let ok = command()
        .args(["run", surface.to_str().unwrap()])
        .env("BAP_PROCESS_PROBE", "visible")
        .output()
        .expect("spawn bap");
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );

    let exiting = dir.join("exiting.cjs");
    std::fs::write(&exiting, "console.log('bye'); process.exit(7);\n").unwrap();
    let code = command()
        .args(["run", exiting.to_str().unwrap()])
        .output()
        .expect("spawn bap");
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(code.status.code(), Some(7));
    assert!(String::from_utf8_lossy(&code.stdout).contains("bye"));
}

#[test]
fn unimplemented_builtins_report_clearly() {
    let dir = std::env::temp_dir().join(format!("bap_crypto_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("wants-crypto.js");
    std::fs::write(&file, "const crypto = require('node:crypto');\n").unwrap();
    let out = command()
        .args(["run", file.to_str().unwrap()])
        .output()
        .expect("spawn bap");
    std::fs::remove_dir_all(&dir).ok();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("node:crypto is not implemented on Bap"),
        "stderr was: {stderr}"
    );
}

fn workspace_bin() -> &'static str {
    BIN
}

#[test]
fn doctor_reports_supported_missing_and_gaps() {
    let dir = std::env::temp_dir().join(format!("bap_doctor_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("bap.json"),
        r#"{ "runtime": "bap", "aliases": { "mylib": "./src/mylib.js" } }"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("src/app.js"),
        "import { join } from 'node:path';\nimport helper from 'mylib';\nconsole.log(typeof join);\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/legacy.cjs"),
        "require('fs');\nrequire('node:crypto');\nrequire('lodash');\n",
    )
    .unwrap();
    std::fs::write(dir.join("src/mylib.js"), "export default {};\n").unwrap();

    let out = Command::new(workspace_bin())
        .arg("doctor")
        .current_dir(&dir)
        .output()
        .expect("spawn bap");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(stdout.contains("supported        node:path"), "{stdout}");
    assert!(stdout.contains("supported        node:fs"), "{stdout}");
    assert!(stdout.contains("NOT IMPLEMENTED  node:crypto"), "{stdout}");
    assert!(stdout.contains("missing          lodash"), "{stdout}");

    let json_out = Command::new(BIN)
        .args(["doctor", "--json"])
        .current_dir(&dir)
        .output()
        .expect("spawn bap");
    std::fs::remove_dir_all(&dir).ok();
    assert!(json_out.status.success());
    let report: serde_json::Value = serde_json::from_slice(&json_out.stdout).unwrap();
    assert_eq!(report["dependencies"], 5);
    assert_eq!(report["supported"], 3);
}

#[test]
fn bap_json_alias_resolves_for_import_and_require() {
    let dir = std::env::temp_dir().join(format!("bap_alias_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("bap.json"),
        r#"{ "runtime": "bap", "aliases": { "vendor-util": "./vendor/util.cjs" } }"#,
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("vendor")).unwrap();
    std::fs::write(
        dir.join("vendor/util.cjs"),
        "exports.tag = () => 'vendored';\n",
    )
    .unwrap();

    let entry = dir.join("entry.mjs");
    std::fs::write(
        &entry,
        "import { tag } from 'vendor-util';\nif (tag() !== 'vendored') throw new Error('bad tag');\nconsole.log(tag());\n",
    )
    .unwrap();

    let driver = dir.join("driver.cjs");
    std::fs::write(
        &driver,
        "const util = require('vendor-util');\nconsole.log(util.tag());\n",
    )
    .unwrap();

    let cache = tempfile::tempdir().unwrap();
    let esm = Command::new(BIN)
        .args(["run", entry.to_str().unwrap()])
        .env("BAP_TS_CACHE_DIR", cache.path())
        .current_dir(&dir)
        .output()
        .expect("spawn bap");
    let cjs = Command::new(BIN)
        .args(["run", "driver.cjs"])
        .env("BAP_TS_CACHE_DIR", cache.path())
        .current_dir(&dir)
        .output()
        .expect("spawn bap");
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        esm.status.success(),
        "{}",
        String::from_utf8_lossy(&esm.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&esm.stdout).trim(), "vendored");
    assert!(
        cjs.status.success(),
        "{}",
        String::from_utf8_lossy(&cjs.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&cjs.stdout).trim(), "vendored");
}
