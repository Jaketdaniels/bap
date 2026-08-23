//! The bap V8 extension: console ops, script args, numeric helpers, and their JS bootstrap.

#![forbid(unsafe_code)]

mod clock;
mod console;
mod fetch;
mod fs;
mod metrics;
mod net;
mod numeric;
mod process;
mod timers;
pub use clock::op_performance_now_ns;
pub use console::{op_bap_aliases, op_bap_args, op_console_error, op_console_log, op_console_warn};
pub use fetch::{FetchState, op_fetch, op_fetch_body};
pub use fs::{
    op_fs_exists_sync, op_fs_make_dir_sync, op_fs_read_dir, op_fs_read_dir_sync, op_fs_read_file,
    op_fs_read_file_sync, op_fs_stat, op_fs_stat_sync, op_fs_write_file, op_fs_write_file_sync,
};
pub use net::{
    op_net_accept, op_net_close, op_net_close_listener, op_net_connect, op_net_listen, op_net_read,
    op_net_write,
};
pub use numeric::{
    op_numeric_fixed_binop, op_numeric_fixed_parse, op_numeric_i32_binop, op_numeric_i32_neg,
    op_numeric_i64_add_batch, op_numeric_i64_binop, op_numeric_i64_neg, op_numeric_interval_add,
    op_numeric_interval_add_batch, op_numeric_interval_div, op_numeric_interval_div_batch,
    op_numeric_interval_mul, op_numeric_interval_mul_batch, op_numeric_interval_new,
    op_numeric_interval_sub, op_numeric_interval_sub_batch,
};
pub use process::{op_process_cwd, op_process_env, op_process_exit, op_process_info};
pub use timers::{op_timer_clear, op_timer_create, op_timer_set_timeout};

use bap_metrics::MetricsCollector;
use deno_core::extension;
use std::collections::BTreeMap;

extension!(
    bap,
    ops = [
        op_console_log,
        op_console_error,
        op_console_warn,
        op_bap_args,
        op_timer_create,
        op_timer_clear,
        op_timer_set_timeout,
        op_fs_read_file,
        op_fs_write_file,
        op_fs_read_dir,
        op_fs_stat,
        op_fs_read_file_sync,
        op_fs_write_file_sync,
        op_fs_exists_sync,
        op_fs_read_dir_sync,
        op_fs_stat_sync,
        op_fs_make_dir_sync,
        op_bap_aliases,
        op_performance_now_ns,
        op_process_env,
        op_process_cwd,
        op_process_exit,
        op_process_info,
        op_net_listen,
        op_net_accept,
        op_net_connect,
        op_net_read,
        op_net_write,
        op_net_close,
        op_net_close_listener,
        op_fetch,
        op_fetch_body,
        op_numeric_interval_new,
        op_numeric_interval_add,
        op_numeric_interval_sub,
        op_numeric_interval_mul,
        op_numeric_interval_div,
        op_numeric_interval_add_batch,
        op_numeric_interval_sub_batch,
        op_numeric_interval_mul_batch,
        op_numeric_interval_div_batch,
        op_numeric_i32_binop,
        op_numeric_i32_neg,
        op_numeric_i64_binop,
        op_numeric_i64_neg,
        op_numeric_i64_add_batch,
        op_numeric_fixed_parse,
        op_numeric_fixed_binop
    ],
    js = [
        dir "src/js",
        "console.js",
        "encoding.js",
        "numeric.js",
        "cjs.js",
        "node/path.js",
        "node/fs.js",
        "node/assert.js",
        "node/util.js",
        "node/events.js",
        "node/buffer.js",
        "node/process.js",
        "runtime.js"
    ],
    options = { metrics: MetricsCollector, args: Vec<String>, aliases: BTreeMap<String, String> },
    state = |state, config| {
        state.put(config.metrics);
        state.put(config.args);
        state.put(config.aliases);
        state.put(timers::TimerState::default());
        state.put(net::NetState::default());
        state.put(fetch::FetchState::default());
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
        let extensions = vec![bap::init(metrics.clone(), args, Default::default())];
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
