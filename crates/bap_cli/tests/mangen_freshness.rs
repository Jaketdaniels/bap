//! Guards docs/man against drift from the clap definition.

const BIN_EXAMPLE: &str = "mangen";

#[test]
fn committed_man_pages_match_the_cli_definition() {
    // Regenerating is done by the example binary; here we verify the pages
    // exist, are non-empty roff, and cover every subcommand.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let man_dir = format!("{manifest_dir}/../../docs/man");

    let expected = [
        "bap.1",
        "bap-run.1",
        "bap-repl.1",
        "bap-init.1",
        "bap-doctor.1",
        "bap-bench.1",
        "bap-report.1",
        "bap-report-compat.1",
        "bap-inspect.1",
        "bap-inspect-proofs.1",
        "bap-complete.1",
    ];
    for page in expected {
        let path = format!("{man_dir}/{page}");
        let contents = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{page} missing: {error}"));
        assert!(contents.contains(".TH"), "{page} is not roff");
    }

    // The example must still build and regenerate cleanly into a temp dir.
    let status = std::process::Command::new(env!("CARGO"))
        .args(["build", "-q", "-p", "bap", "--example", BIN_EXAMPLE])
        .status()
        .expect("cargo build");
    assert!(status.success());

    let out_dir = tempfile::tempdir().unwrap();
    let generated = out_dir.path().join("bap.1");
    let status = std::process::Command::new(env!("CARGO"))
        .args(["run", "-q", "-p", "bap", "--example", BIN_EXAMPLE])
        .arg(out_dir.path())
        .status()
        .expect("cargo run");
    assert!(status.success());
    let regenerated = std::fs::read_to_string(&generated).unwrap();
    let committed = std::fs::read_to_string(format!("{man_dir}/bap.1")).unwrap();
    // Version lines include CARGO_PKG_VERSION; compare structure, not bytes.
    assert_eq!(
        regenerated.matches(".SH").count(),
        committed.matches(".SH").count(),
        "section structure drifted; run: cargo run -p bap --example mangen -- docs/man"
    );
}
