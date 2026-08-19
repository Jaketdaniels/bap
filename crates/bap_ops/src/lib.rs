//! The bap V8 extension: console ops, script args, and their JS bootstrap.

#![forbid(unsafe_code)]

mod console;
mod timers;
pub use console::{op_bap_args, op_console_error, op_console_log, op_console_warn};
pub use timers::{op_timer_clear, op_timer_create, op_timer_set_timeout};

use bap_metrics::MetricsCollector;
use deno_core::extension;

extension!(
    bap,
    ops = [
        op_console_log,
        op_console_error,
        op_console_warn,
        op_bap_args,
        op_timer_create,
        op_timer_clear,
        op_timer_set_timeout
    ],
    js = [dir "src/js", "console.js", "runtime.js"],
    options = { metrics: MetricsCollector, args: Vec<String> },
    state = |state, config| {
        state.put(config.metrics);
        state.put(config.args);
        state.put(timers::TimerState::default());
    },
    docs = "Bap builtin ops"
);

#[cfg(test)]
mod tests {
    use super::*;
    use bap_metrics::MetricsCollector;
    use deno_core::{JsRuntime, RuntimeOptions};

    fn eval_with_ext(
        metrics: &MetricsCollector,
        args: Vec<String>,
        code: &str,
    ) -> Result<(), deno_core::error::AnyError> {
        let extensions = vec![bap::init(metrics.clone(), args)];
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
            Vec::new(),
            "Deno.core.ops.op_console_log('hello from bap\\n')",
        )
        .expect("eval console.log");
        assert_eq!(metrics.snapshot().ops["console.log"].calls, 1);
    }

    #[test]
    fn op_error_records_metrics() {
        let metrics = MetricsCollector::new();
        eval_with_ext(
            &metrics,
            Vec::new(),
            "Deno.core.ops.op_console_error('boom\\n')",
        )
        .expect("eval console.error");
        assert_eq!(metrics.snapshot().ops["console.error"].calls, 1);
    }

    #[test]
    fn op_bap_args_returns_config_args() {
        let metrics = MetricsCollector::new();
        eval_with_ext(
            &metrics,
            vec!["alpha".into(), "beta".into()],
            "if (JSON.stringify(Deno.core.ops.op_bap_args()) !== '[\"alpha\",\"beta\"]') throw new Error('args mismatch')",
        )
        .expect("eval op_bap_args");
    }
}
