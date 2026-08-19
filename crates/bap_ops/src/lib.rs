#![forbid(unsafe_code)]

mod console;
pub use console::{op_console_error, op_console_log, op_console_warn};

use bap_metrics::MetricsCollector;
use deno_core::extension;

extension!(
    bap,
    ops = [op_console_log, op_console_error, op_console_warn],
    js = [dir "src/js", "console.js"],
    options = { metrics: MetricsCollector },
    state = |state, config| state.put(config.metrics),
    docs = "Bap builtin ops"
);

#[cfg(test)]
mod tests {
    use super::*;
    use bap_metrics::MetricsCollector;
    use deno_core::{JsRuntime, RuntimeOptions};

    fn eval_with_ext(
        metrics: &MetricsCollector,
        code: &str,
    ) -> Result<(), deno_core::error::AnyError> {
        let extensions = vec![bap::init(metrics.clone())];
        let mut runtime = JsRuntime::new(RuntimeOptions {
            extensions,
            ..Default::default()
        });
        runtime.execute_script("<anon>", code.to_owned())?;
        Ok(())
    }

    #[test]
    fn op_log_writes_stdout_and_metrics() {
        let metrics = MetricsCollector::new();
        eval_with_ext(
            &metrics,
            "Deno.core.ops.op_console_log('hello from bap\\n')",
        )
        .expect("eval console.log");
        assert_eq!(metrics.snapshot().ops["console.log"].calls, 1);
    }

    #[test]
    fn op_error_records_metrics() {
        let metrics = MetricsCollector::new();
        eval_with_ext(&metrics, "Deno.core.ops.op_console_error('boom\\n')")
            .expect("eval console.error");
        assert_eq!(metrics.snapshot().ops["console.error"].calls, 1);
    }
}
