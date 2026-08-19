//! Shared op-timing helpers against the collector in `OpState`.

use bap_metrics::MetricsCollector;
use deno_core::OpState;
use std::time::Instant;

/// Records a sync op's elapsed time.
pub fn record_op(state: &OpState, name: &str, started: Instant) {
    state
        .borrow::<MetricsCollector>()
        .record_op(name, started.elapsed());
}

/// Records an async op's elapsed time, feeding the latency histogram.
pub fn record_op_async(state: &OpState, name: &str, started: Instant) {
    state
        .borrow::<MetricsCollector>()
        .record_op_async(name, started.elapsed());
}
