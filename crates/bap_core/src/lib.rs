#![forbid(unsafe_code)]

mod error;

use bap_metrics::MetricsCollector;
use deno_core::{FsModuleLoader, JsRuntime, RuntimeOptions};
use serde_json::Value;

pub use error::{Error, ErrorKind};

pub struct BapRuntime {
    runtime: JsRuntime,
}

impl BapRuntime {
    pub fn new(metrics: MetricsCollector) -> Self {
        Self::with_args(metrics, Vec::new())
    }

    pub fn with_args(metrics: MetricsCollector, args: Vec<String>) -> Self {
        let extensions = vec![bap_ops::bap::init(metrics.clone(), args)];
        let started = std::time::Instant::now();
        let runtime = JsRuntime::new(RuntimeOptions {
            module_loader: Some(std::rc::Rc::new(FsModuleLoader)),
            extensions,
            ..Default::default()
        });
        metrics.record_cold_start(started.elapsed());
        Self { runtime }
    }

    pub async fn run_script(&mut self, path: &str) -> Result<(), Error> {
        let cwd = std::env::current_dir().map_err(Error::io)?;
        let main_module = deno_core::resolve_path(path, &cwd).map_err(Error::load)?;
        let mod_id = self
            .runtime
            .load_main_es_module(&main_module)
            .await
            .map_err(Error::load)?;
        let result = self.runtime.mod_evaluate(mod_id);
        self.runtime
            .run_event_loop(Default::default())
            .await
            .map_err(Error::eval)?;
        result.await.map_err(Error::eval)?;
        Ok(())
    }

    pub fn eval(&mut self, code: &str) -> Result<Option<Value>, Error> {
        let global = self
            .runtime
            .execute_script("<anon>", code.to_owned())
            .map_err(Error::eval)?;
        deno_core::scope!(scope, self.runtime);
        let local = deno_core::v8::Local::new(scope, global);
        if local.is_undefined() {
            return Ok(None);
        }
        Ok(Some(
            serde_v8::from_v8::<Value>(scope, local).map_err(Error::eval)?,
        ))
    }

    pub fn sample_heap(&mut self) -> (usize, usize) {
        let isolate = self.runtime.v8_isolate();
        let stats = isolate.get_heap_statistics();
        (stats.used_heap_size(), stats.total_heap_size())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_returns_json_value() {
        let mut runtime = BapRuntime::new(MetricsCollector::new());
        let value = runtime.eval("1 + 1").unwrap();
        assert_eq!(value, Some(serde_json::json!(2)));
    }

    #[test]
    fn console_log_records_metrics() {
        let metrics = MetricsCollector::new();
        let mut runtime = BapRuntime::new(metrics.clone());
        runtime.eval("console.log('hello', 42)").unwrap();
        assert_eq!(metrics.snapshot().ops["console.log"].calls, 1);
    }

    #[test]
    fn args_exposed_on_bap_global() {
        let mut runtime =
            BapRuntime::with_args(MetricsCollector::new(), vec!["a".into(), "b".into()]);
        let value = runtime.eval("JSON.stringify(Bap.args)").unwrap();
        assert_eq!(value, Some(serde_json::json!("[\"a\",\"b\"]")));
    }

    #[tokio::test]
    async fn missing_file_reports_load_error() {
        let mut runtime = BapRuntime::new(MetricsCollector::new());
        let err = runtime.run_script("no-such-file.js").await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Load);
    }

    #[test]
    fn eval_error_reports_eval_kind() {
        let mut runtime = BapRuntime::new(MetricsCollector::new());
        let err = runtime.eval("throw new Error('boom')").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Eval);
    }

    #[test]
    fn sample_heap_reports_nonzero() {
        let mut runtime = BapRuntime::new(MetricsCollector::new());
        let (used, total) = runtime.sample_heap();
        assert!(used > 0);
        assert!(total >= used);
    }
}
