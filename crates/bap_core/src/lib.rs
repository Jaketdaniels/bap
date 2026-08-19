#![forbid(unsafe_code)]

use bap_metrics::MetricsCollector;
use deno_core::{FsModuleLoader, JsRuntime, RuntimeOptions};
use serde_json::Value;

pub struct BapRuntime {
    runtime: JsRuntime,
    metrics: MetricsCollector,
}

impl BapRuntime {
    pub fn new(metrics: MetricsCollector) -> Self {
        let extensions = vec![bap_ops::bap::init(metrics.clone())];
        let runtime = JsRuntime::new(RuntimeOptions {
            module_loader: Some(std::rc::Rc::new(FsModuleLoader)),
            extensions,
            ..Default::default()
        });
        Self { runtime, metrics }
    }

    pub async fn run_script(&mut self, path: &str) -> Result<(), deno_core::error::AnyError> {
        let cwd = std::env::current_dir()?;
        let main_module = deno_core::resolve_path(path, &cwd)?;
        let mod_id = self.runtime.load_main_es_module(&main_module).await?;
        let result = self.runtime.mod_evaluate(mod_id);
        self.runtime.run_event_loop(Default::default()).await?;
        result.await?;
        Ok(())
    }

    pub fn eval(&mut self, code: &str) -> Result<Value, deno_core::error::AnyError> {
        let global = self.runtime.execute_script("<anon>", code)?;
        deno_core::scope!(scope, self.runtime);
        let local = deno_core::v8::Local::new(scope, global);
        Ok(serde_v8::from_v8::<Value>(scope, local)?)
    }

    pub fn sample_heap(&mut self) -> (usize, usize) {
        let isolate = self.runtime.v8_isolate();
        let mut stats = deno_core::v8::HeapStatistics::default();
        isolate.get_heap_statistics(&mut stats);
        (stats.used_heap_size(), stats.total_heap_size())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn eval_returns_json_value() {
        let mut runtime = BapRuntime::new(MetricsCollector::new());
        let value = runtime.eval("1 + 1").unwrap();
        assert_eq!(value, serde_json::json!(2));
    }

    #[test]
    fn console_log_flows_to_stdout_and_metrics() {
        let metrics = MetricsCollector::new();
        let mut runtime = BapRuntime::new(metrics.clone());
        let sink = Arc::new(Mutex::new(Vec::new()));
        std::io::set_output_capture(Some(sink.clone()));
        runtime.eval("console.log('hello', 42)").unwrap();
        let bytes = sink.lock().unwrap().clone();
        assert_eq!(String::from_utf8(bytes).unwrap(), "hello 42\n");
        assert_eq!(metrics.snapshot().ops["console.log"].calls, 1);
    }

    #[test]
    fn sample_heap_reports_nonzero() {
        let mut runtime = BapRuntime::new(MetricsCollector::new());
        let (used, total) = runtime.sample_heap();
        assert!(used > 0);
        assert!(total >= used);
    }
}