//! V8 ops backing the console and Bap globals.

use bap_metrics::MetricsCollector;
use deno_core::OpState;
use deno_core::op2;
use deno_error::JsErrorBox;
use std::time::Instant;

use std::io::Write;

/// Writes `msg` to `stream`, records op timing, and maps write errors to `JsErrorBox`.
fn emit(
    state: &mut OpState,
    stream: &mut dyn Write,
    name: &str,
    msg: &str,
) -> Result<(), JsErrorBox> {
    let started = Instant::now();
    let metrics = state.borrow::<MetricsCollector>();
    let result = stream
        .write_all(msg.as_bytes())
        .and_then(|_| stream.flush());
    metrics.record_op(name, started.elapsed());
    tracing::debug!(target: "bap::ops", op = name, "console write");
    result.map_err(|err| JsErrorBox::new("Error", format!("{name}: {err}")))
}

/// Writes a message to stdout, recording metrics under `console.log`.
#[op2(fast)]
pub fn op_console_log(state: &mut OpState, #[string] msg: String) -> Result<(), JsErrorBox> {
    emit(state, &mut std::io::stdout(), "console.log", &msg)
}

/// Writes a message to stderr, recording metrics under `console.error`.
#[op2(fast)]
pub fn op_console_error(state: &mut OpState, #[string] msg: String) -> Result<(), JsErrorBox> {
    emit(state, &mut std::io::stderr(), "console.error", &msg)
}

/// Writes a message to stderr, recording metrics under `console.warn`.
#[op2(fast)]
pub fn op_console_warn(state: &mut OpState, #[string] msg: String) -> Result<(), JsErrorBox> {
    emit(state, &mut std::io::stderr(), "console.warn", &msg)
}

/// Returns the script arguments stored in `OpState`.
#[op2]
pub fn op_bap_args(state: &mut OpState) -> Vec<String> {
    state.borrow::<Vec<String>>().clone()
}
