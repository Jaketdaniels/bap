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
    assert!(stdout.contains("2"));
}
