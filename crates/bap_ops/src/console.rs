use bap_metrics::MetricsCollector;
use deno_core::OpState;
use deno_core::op2;
use deno_error::JsErrorBox;
use std::time::Instant;

use std::io::Write;

fn emit(state: &mut OpState, stream: &mut dyn Write, name: &str, msg: &str) {
    let started = Instant::now();
    let metrics = state.borrow::<MetricsCollector>();
    let _ = stream.write_all(msg.as_bytes());
    let _ = stream.flush();
    metrics.record_op(name, started.elapsed());
    tracing::debug!(target: "bap::ops", op = name, "console write");
}

#[op2(fast)]
pub fn op_console_log(state: &mut OpState, #[string] msg: String) -> Result<(), JsErrorBox> {
    emit(state, &mut std::io::stdout(), "console.log", &msg);
    Ok(())
}

#[op2(fast)]
pub fn op_console_error(state: &mut OpState, #[string] msg: String) -> Result<(), JsErrorBox> {
    emit(state, &mut std::io::stderr(), "console.error", &msg);
    Ok(())
}

#[op2(fast)]
pub fn op_console_warn(state: &mut OpState, #[string] msg: String) -> Result<(), JsErrorBox> {
    emit(state, &mut std::io::stderr(), "console.warn", &msg);
    Ok(())
}
