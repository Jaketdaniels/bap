//! V8-backed runtime embedding the bap extension.
//! Loads ES modules and evaluates expressions to JSON values.

#![forbid(unsafe_code)]

mod error;

use bap_metrics::MetricsCollector;
use deno_core::{FsModuleLoader, JsRuntime, ModuleLoader, RuntimeOptions};
use serde_json::Value;

pub use error::{Error, ErrorKind};

/// Owns a V8 isolate and the bap extension.
/// `eval` targets a single isolate with no separate worker.
pub struct BapRuntime {
    runtime: JsRuntime,
}

impl BapRuntime {
    /// Creates a runtime with no script arguments.
    pub fn new(metrics: MetricsCollector) -> Self {
        Self::with_args(metrics, Vec::new())
    }

    /// Creates a runtime whose `Bap.args` exposes `args`.
    /// Measures and records cold start from the isolate construction.
    pub fn with_args(metrics: MetricsCollector, args: Vec<String>) -> Self {
        Self::with_args_and_loader(metrics, args, std::rc::Rc::new(FsModuleLoader))
    }

    /// Creates a runtime with script arguments and a caller-provided module loader.
    pub fn with_args_and_loader(
        metrics: MetricsCollector,
        args: Vec<String>,
        module_loader: std::rc::Rc<dyn ModuleLoader>,
    ) -> Self {
        Self::with_args_loader_aliases(metrics, args, module_loader, Default::default())
    }

    /// Creates a runtime that also exposes `aliases` to the require surface.
    pub fn with_args_loader_aliases(
        metrics: MetricsCollector,
        args: Vec<String>,
        module_loader: std::rc::Rc<dyn ModuleLoader>,
        aliases: std::collections::BTreeMap<String, String>,
    ) -> Self {
        let extensions = vec![bap_ops::bap::init(metrics.clone(), args, aliases)];
        let started = std::time::Instant::now();
        let runtime = JsRuntime::new(RuntimeOptions {
            module_loader: Some(module_loader),
            extensions,
            ..Default::default()
        });
        metrics.record_cold_start(started.elapsed());
        Self { runtime }
    }

    /// Loads and evaluates the ES module at `path`, driving the event loop to completion.
    /// Returns an `Error` classified by the failing stage.
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

    /// Evaluates `source` as a fresh side ES module under `specifier`.
    /// Repeated calls with distinct specifiers never hit module caches, so
    /// benchmarks can treat each call as one full request.
    pub async fn run_script_source(
        &mut self,
        specifier: &str,
        source: String,
    ) -> Result<(), Error> {
        let module = deno_core::ModuleSpecifier::parse(specifier).map_err(Error::load)?;
        // Side modules allow many evaluations per isolate; a unique
        // specifier per call keeps each request independent.
        let mod_id = self
            .runtime
            .load_side_es_module_from_code(&module, source)
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

    /// Executes `code` and converts the result to a JSON value.
    /// Returns `None` when the result is `undefined`.
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

    /// Returns `(used, total)` heap bytes reported by the isolate.
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
