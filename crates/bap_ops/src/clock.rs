//! Monotonic clock op backing `performance.now`.

use super::metrics::record_op;
use deno_core::OpState;
use deno_core::op2;
use std::sync::OnceLock;
use std::time::Instant;

fn anchor() -> Instant {
    static ANCHOR: OnceLock<Instant> = OnceLock::new();
    *ANCHOR.get_or_init(Instant::now)
}

/// Nanoseconds on a process-relative monotonic clock.
#[op2(fast)]
#[number]
pub fn op_performance_now_ns(state: &mut OpState) -> u64 {
    let started = Instant::now();
    let elapsed = anchor().elapsed().as_nanos() as u64;
    record_op(state, "performance.now", started);
    elapsed
}
