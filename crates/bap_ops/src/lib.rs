#![forbid(unsafe_code)]

mod console;
pub use console::{op_console_error, op_console_log, op_console_warn};

use bap_metrics::MetricsCollector;
use deno_core::extension;

extension!(
    bap,
    ops = [op_console_log, op_console_error, op_console_warn],
    js = [dir "js", "console.js"],
    config = { metrics: MetricsCollector },
    state = |state, config| state.put(config.metrics),
    docs = "Bap builtin ops"
);

#[cfg(test)]
mod tests {
    use super::*;
    use bap_metrics::MetricsCollector;
    use std::sync::{Arc, Mutex};

    #[test]
    fn op_log_writes_stdout_and_metrics() {
        let mut state = deno_core::OpState::new();
        state.put(MetricsCollector::new());
        let sink = Arc::new(Mutex::new(Vec::new()));
        std::io::set_output_capture(Some(sink.clone()));
        op_console_log(&mut state, "hello from bap\n".to_string()).unwrap();
        let bytes = sink.lock().unwrap().clone();
        assert_eq!(String::from_utf8(bytes).unwrap(), "hello from bap\n");
        let metrics = state.borrow::<MetricsCollector>();
        assert_eq!(metrics.snapshot().ops["console.log"].calls, 1);
    }

    #[test]
    fn op_error_records_metrics() {
        let mut state = deno_core::OpState::new();
        state.put(MetricsCollector::new());
        op_console_error(&mut state, "boom\n".to_string()).unwrap();
        let metrics = state.borrow::<MetricsCollector>();
        assert_eq!(metrics.snapshot().ops["console.error"].calls, 1);
    }
}