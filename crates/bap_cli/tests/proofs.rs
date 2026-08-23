use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_bap");
fn counting_node() -> String {
    format!(
        "{}/tests/fixtures/proofs/counting-node.sh",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn workspace_tool() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/ts-proof/proof.mjs")
        .canonicalize()
        .expect("proof tool exists in the workspace")
}

fn fixture(path: &str) -> String {
    format!(
        "{}/tests/fixtures/proofs/{path}",
        env!("CARGO_MANIFEST_DIR")
    )
}

struct TestCommand {
    command: Command,
    cache: tempfile::TempDir,
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
    command.env("BAP_TS_PROOF_TOOL", workspace_tool());
    TestCommand { command, cache }
}

fn proof_launches(log: &Path) -> usize {
    std::fs::read_to_string(log)
        .map(|contents| contents.lines().count())
        .unwrap_or(0)
}

#[test]
fn proven_specialization_prints_the_folded_constant() {
    let out = command()
        .args(["run", &fixture("unit_fold.ts")])
        .output()
        .expect("spawn bap");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "15\n");
}

#[test]
fn cache_hits_do_not_relaunch_node() {
    let workspace = tempfile::tempdir().unwrap();
    let log = workspace.path().join("launches.log");
    let mut test = command();
    test.env("BAP_TS_PROOF_NODE", counting_node());
    test.env("BAP_PROOF_NODE_LOG", &log);
    test.args(["run", &fixture("unit_fold.ts")]);
    let first = test.output().expect("spawn bap");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&first.stdout),
        "15\n",
        "first run must specialize"
    );

    let second = test.output().expect("spawn bap again");
    assert!(
        second.status.success(),
        "second stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&second.stdout),
        "15\n",
        "second stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(proof_launches(&log), 1, "cache hit must not relaunch Node");
}

#[test]
fn source_edits_trigger_a_new_proof() {
    let workspace = tempfile::tempdir().unwrap();
    let script = workspace.path().join("edited.ts");
    std::fs::write(&script, std::fs::read(fixture("unit_fold.ts")).unwrap()).unwrap();

    let log = workspace.path().join("launches.log");
    let mut test = command();
    test.env("BAP_TS_PROOF_NODE", counting_node());
    test.env("BAP_PROOF_NODE_LOG", &log);
    test.arg("run");

    let first = test.arg(script.as_os_str()).output().unwrap();
    assert!(first.status.success());

    std::fs::write(
        &script,
        "const total = Bap.numeric.unitAdd(Bap.numeric.unit(\"metre\", 10), Bap.numeric.unit(\"metre\", 5));\nconsole.log(total);\n",
    )
    .unwrap();
    // The previous Command consumed `script`; rebuild with the same path.
    let mut rerun = command();
    rerun.env("BAP_TS_PROOF_NODE", counting_node());
    rerun.env("BAP_PROOF_NODE_LOG", &log);
    let second = rerun
        .args(["run", script.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&second.stdout), "15\n");
    assert_eq!(
        proof_launches(&log),
        2,
        "an edit must produce a fresh proof"
    );
}

#[test]
fn corrupted_cache_rebuilds_and_reproves() {
    let workspace = tempfile::tempdir().unwrap();
    let log = workspace.path().join("launches.log");
    let mut test = command();
    test.env("BAP_TS_PROOF_NODE", counting_node());
    test.env("BAP_PROOF_NODE_LOG", &log);
    test.args(["run", &fixture("unit_fold.ts")]);
    let first = test.output().unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(proof_launches(&log), 1);

    let artifact = std::fs::read_dir(test.cache.path())
        .unwrap()
        .next()
        .expect("artifact stored")
        .unwrap()
        .path();
    std::fs::write(&artifact, b"garbage").unwrap();

    let second = test.output().unwrap();
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&second.stdout), "15\n");
    assert_eq!(
        proof_launches(&log),
        2,
        "corruption must trigger a re-proof"
    );
}

#[test]
fn mismatch_fails_under_require_proofs() {
    let out = command()
        .args(["run", "--require-proofs", &fixture("unit_mismatch.ts")])
        .output()
        .expect("spawn bap");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("TS2345"), "stderr was: {stderr}");
}

#[test]
fn mismatch_runs_unspecialized_in_normal_mode() {
    let out = command()
        .args(["run", &fixture("unit_mismatch.ts")])
        .output()
        .expect("spawn bap");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("rejected:"), "stdout was: {stdout}");
}

#[test]
fn no_proofs_never_consults_the_backend() {
    let out = command()
        .args(["run", "--no-proofs", &fixture("unit_fold.ts")])
        .env_remove("PATH")
        .output()
        .expect("spawn bap");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("\"lo\""), "stdout was: {stdout}");
}

#[test]
fn require_proofs_without_a_backend_fails() {
    let out = command()
        .args(["run", "--require-proofs", &fixture("unit_fold.ts")])
        .env_remove("BAP_TS_PROOF_TOOL")
        .current_dir(std::env::temp_dir())
        .output()
        .expect("spawn bap");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--require-proofs needs a proof backend"),
        "stderr was: {stderr}"
    );
}

#[test]
fn inspect_reports_unit_add_proofs() {
    let out = command()
        .args(["inspect", "proofs", "--json", &fixture("unit_fold.ts")])
        .output()
        .expect("spawn bap");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("inspect prints json");
    assert_eq!(report["status"], "proven");
    assert_eq!(report["proofCount"], 1);
    assert_eq!(report["kinds"][0], "unit.add");
    assert_eq!(report["units"][0], "metre");
}

#[test]
fn inspect_exits_nonzero_on_mismatch() {
    let out = command()
        .args(["inspect", "proofs", &fixture("unit_mismatch.ts")])
        .output()
        .expect("spawn bap");
    assert!(!out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("TS2345"), "stdout was: {stdout}");
}
