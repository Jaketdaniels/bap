use bap_core::BapRuntime;
use bap_metrics::MetricsCollector;

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[tokio::test]
async fn timer_resolves_under_event_loop() {
    let metrics = MetricsCollector::new();
    let mut runtime = BapRuntime::new(metrics.clone());
    runtime
        .run_script(&fixture("timer_fires.js"))
        .await
        .unwrap();
    let snap = metrics.snapshot();
    assert_eq!(snap.ops["timers.create"].calls, 1);
    assert_eq!(snap.ops["timers.setTimeout"].calls, 1);
}

#[tokio::test]
async fn shorter_timer_fires_first() {
    let mut runtime = BapRuntime::new(MetricsCollector::new());
    runtime
        .run_script(&fixture("timer_order.js"))
        .await
        .unwrap();
}

#[tokio::test]
async fn cleared_timer_does_not_fire() {
    let mut runtime = BapRuntime::new(MetricsCollector::new());
    runtime
        .run_script(&fixture("timer_cleared.js"))
        .await
        .unwrap();
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("bap-fs-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn fs_write_read_stat_round_trip() {
    let dir = temp_dir("roundtrip");
    let file = dir.join("roundtrip.txt");
    let file = file.to_str().unwrap().to_string();
    let mut runtime = BapRuntime::with_args(MetricsCollector::new(), vec![file.clone()]);
    runtime
        .run_script(&fixture("fs_roundtrip.js"))
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "hello bap\n");
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn fs_read_dir_lists_fixtures() {
    let dir = format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"));
    let mut runtime = BapRuntime::with_args(MetricsCollector::new(), vec![dir]);
    runtime.run_script(&fixture("fs_readdir.js")).await.unwrap();
}

#[tokio::test]
async fn fs_read_file_reports_missing_path() {
    let missing = format!("{}/no-such-bap-file.txt", env!("CARGO_MANIFEST_DIR"));
    let mut runtime = BapRuntime::with_args(MetricsCollector::new(), vec![missing]);
    runtime.run_script(&fixture("fs_missing.js")).await.unwrap();
}
