use bap_core::BapRuntime;
use bap_metrics::MetricsCollector;

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[tokio::test]
async fn timer_resolves_under_event_loop() {
    let metrics = MetricsCollector::new();
    let mut runtime = BapRuntime::new(metrics.clone());
    let script = fixture("timer_fires.js");
    runtime.run_script(&script).await.unwrap();
    let snap = metrics.snapshot();
    assert_eq!(snap.ops["timers.create"].calls, 1);
    assert_eq!(snap.ops["timers.setTimeout"].calls, 1);
}

#[tokio::test]
async fn shorter_timer_fires_first() {
    let mut runtime = BapRuntime::new(MetricsCollector::new());
    let script = fixture("timer_order.js");
    runtime.run_script(&script).await.unwrap();
}

#[tokio::test]
async fn cleared_timer_does_not_fire() {
    let mut runtime = BapRuntime::new(MetricsCollector::new());
    let script = fixture("timer_cleared.js");
    runtime.run_script(&script).await.unwrap();
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
    let script = fixture("fs_roundtrip.js");
    runtime.run_script(&script).await.unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "hello bap\n");
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn fs_read_dir_lists_fixtures() {
    let dir = format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"));
    let mut runtime = BapRuntime::with_args(MetricsCollector::new(), vec![dir]);
    let script = fixture("fs_readdir.js");
    runtime.run_script(&script).await.unwrap();
}

#[tokio::test]
async fn fs_read_file_reports_missing_path() {
    let missing = format!("{}/no-such-bap-file.txt", env!("CARGO_MANIFEST_DIR"));
    let mut runtime = BapRuntime::with_args(MetricsCollector::new(), vec![missing]);
    let script = fixture("fs_missing.js");
    runtime.run_script(&script).await.unwrap();
}

#[tokio::test]
async fn tcp_echo_round_trip() {
    let metrics = MetricsCollector::new();
    let mut runtime = BapRuntime::new(metrics.clone());
    let script = fixture("net_echo.js");
    let future = runtime.run_script(&script);
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), future)
        .await
        .expect("net echo timed out");
    result.unwrap();
    let snap = metrics.snapshot();
    assert_eq!(snap.ops["net.listen"].calls, 1);
    assert_eq!(snap.ops["net.accept"].calls, 1);
    assert_eq!(snap.ops["net.connect"].calls, 1);
}

#[tokio::test]
async fn tcp_connect_reports_refused() {
    let mut runtime = BapRuntime::new(MetricsCollector::new());
    let script = fixture("net_refused.js");
    let future = runtime.run_script(&script);
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), future)
        .await
        .expect("net refused timed out");
    result.unwrap();
}
