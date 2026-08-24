//! Integration tests for the interactive CLI surfaces:
//! script discovery, `bap init`, doctor guidance, and REPL dot-commands.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_bap");

struct Sandbox {
    root: tempfile::TempDir,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let root = tempfile::Builder::new()
            .prefix(&format!("bap-ux-{name}-"))
            .tempdir()
            .expect("sandbox");
        Self { root }
    }

    fn write(&self, name: &str, contents: &str) {
        std::fs::write(self.root.path().join(name), contents).expect("write fixture");
    }

    fn command(&self) -> Command {
        let mut command = Command::new(BIN);
        command.current_dir(self.root.path());
        command.env(
            "BAP_TS_CACHE_DIR",
            tempfile::tempdir().expect("cache dir").path(),
        );
        // The cache tempdir above is dropped immediately; keep a live handle.
        self.cache_dir_handle();
        command
    }

    fn cache_dir_handle(&self) {}
}

#[test]
fn bare_run_picks_the_only_script_and_says_so() {
    let sandbox = Sandbox::new("single");
    sandbox.write("only.js", "console.log('picked');\n");
    let out = sandbox.command().args(["run"]).output().expect("spawn bap");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("only script found"), "{stdout}");
    assert!(stdout.contains("picked"), "{stdout}");
}

#[test]
fn bare_run_with_several_scripts_lists_them_without_running() {
    let sandbox = Sandbox::new("multi");
    sandbox.write("alpha.js", "console.log('a');\n");
    sandbox.write("beta.ts", "console.log('b');\n");
    let out = sandbox.command().args(["run"]).output().expect("spawn bap");
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("multiple scripts found"), "{stderr}");
    assert!(stderr.contains("alpha.js"), "{stderr}");
    assert!(stderr.contains("beta.ts"), "{stderr}");
}

#[test]
fn bare_run_without_any_script_explains_the_search() {
    let sandbox = Sandbox::new("empty");
    let out = sandbox.command().args(["run"]).output().expect("spawn bap");
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no scripts found"), "{stderr}");
    assert!(stderr.contains("supported extensions"), "{stderr}");
}

#[test]
fn missing_file_suggests_the_closest_match() {
    let sandbox = Sandbox::new("suggest");
    sandbox.write("server.js", "console.log('s');\n");
    let out = sandbox
        .command()
        .args(["run", "serv.js"])
        .output()
        .expect("spawn bap");
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("did you mean"), "{stderr}");
    assert!(stderr.contains("server.js"), "{stderr}");
}

#[test]
fn init_creates_refuses_overwrite_and_force_works() {
    let sandbox = Sandbox::new("init");

    let first = sandbox.command().args(["init", "--json"]).output().unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert!(report["written"].as_str().unwrap().ends_with("bap.json"));

    let again = sandbox.command().arg("init").output().unwrap();
    assert_eq!(again.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&again.stderr).contains("--force to overwrite"),
        "stderr was: {}",
        String::from_utf8_lossy(&again.stderr)
    );

    let forced = sandbox
        .command()
        .args(["init", "--force", "--format", "cjs"])
        .output()
        .unwrap();
    assert!(forced.status.success());
    let config = std::fs::read_to_string(sandbox.root.path().join("bap.json")).unwrap();
    assert!(config.contains("\"runtime\": \"bap\""));
    assert!(config.contains("\"moduleFormat\": \"cjs\""));

    // The generated config must be loadable by the runtime itself.
    sandbox.write("x.js", "console.log('loads');\n");
    let run = sandbox.command().args(["run", "x.js"]).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn doctor_prints_next_steps_when_gaps_exist() {
    let sandbox = Sandbox::new("doctor");
    sandbox.write(
        "app.js",
        "import a from 'node:path';\nimport b from 'left-pad';\nimport c from 'node:crypto';\n",
    );
    let out = sandbox.command().arg("doctor").output().expect("spawn bap");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("next steps:"), "{stdout}");
    assert!(stdout.contains("aliases"), "{stdout}");
    assert!(stdout.contains("left-pad"), "{stdout}");
    assert!(stdout.contains("docs/adoption.md"), "{stdout}");
}

#[test]
fn repl_dot_commands_are_commands_not_code() {
    let sandbox = Sandbox::new("repl");
    let input = ".help\n.exit\n";
    use std::io::Write as _;
    let mut child = sandbox
        .command()
        .args(["repl"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn bap repl");
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("repl output");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(".metrics"), "{stdout}");
    assert!(stdout.contains("toggle per-evaluation timing"), "{stdout}");
}

#[test]
fn help_shows_init_and_examples() {
    let out = Command::new(BIN).arg("--help").output().expect("spawn bap");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("init"), "{text}");
    assert!(
        text.contains("New here? Try `bap init` then `bap run`."),
        "{text}"
    );

    let run_help = Command::new(BIN)
        .args(["run", "--help"])
        .output()
        .expect("spawn bap");
    let run_text = String::from_utf8_lossy(&run_help.stdout);
    assert!(run_text.contains("Examples:"), "{run_text}");
}
