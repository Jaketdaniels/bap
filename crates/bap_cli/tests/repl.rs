use std::io::Write;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_bap");

#[test]
fn repl_evaluates_expression_from_pipe() {
    let mut child = Command::new(BIN)
        .arg("repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn bap repl");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"1 + 1\n.exit\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), "2");
}

#[test]
fn repl_piped_exit_code_reflects_eval_errors() {
    let mut child = Command::new(BIN)
        .arg("repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn bap repl");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"1 + 1\nthrow new Error('boom')\n.exit\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(!out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), "2");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("eval:"));
}
