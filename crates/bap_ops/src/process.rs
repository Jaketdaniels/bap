//! Process-surface ops backing the node:process shim.

use super::metrics::record_op;
use deno_core::OpState;
use deno_core::op2;
use deno_error::JsErrorBox;
use serde::Serialize;
use std::time::Instant;

/// Static platform facts reported once at startup.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub platform: &'static str,
    pub arch: &'static str,
}

/// Exposes the inherited environment as `{key: value}`.
#[op2]
#[serde]
pub fn op_process_env(state: &mut OpState) -> Result<Vec<(String, String)>, JsErrorBox> {
    let started = Instant::now();
    let pairs: Vec<(String, String)> = std::env::vars().collect();
    record_op(state, "process.env", started);
    Ok(pairs)
}

/// Reports the current working directory.
#[op2]
#[string]
pub fn op_process_cwd(state: &mut OpState) -> Result<String, JsErrorBox> {
    let started = Instant::now();
    let cwd = std::env::current_dir().map_err(|e| JsErrorBox::new("Error", format!("cwd: {e}")))?;
    record_op(state, "process.cwd", started);
    Ok(cwd.to_string_lossy().into_owned())
}

/// Terminates the runtime with the given exit code.
#[op2(fast)]
pub fn op_process_exit(code: i32) {
    std::process::exit(code);
}

/// Reports compile-time platform and architecture.
#[op2]
#[serde]
pub fn op_process_info(state: &mut OpState) -> ProcessInfo {
    let started = Instant::now();
    let platform = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "win32"
    } else {
        "unknown"
    };
    record_op(state, "process.info", started);
    ProcessInfo {
        platform,
        arch: std::env::consts::ARCH,
    }
}
