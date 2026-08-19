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
