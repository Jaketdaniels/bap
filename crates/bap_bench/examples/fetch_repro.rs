//! Bisects the CLI fetch hang: TsModuleLoader vs default FsModuleLoader.
use bap_core::BapRuntime;
use std::rc::Rc;

fn main() {
    let which = std::env::args().nth(1).unwrap_or_default();
    let script = std::env::args().nth(2).expect("script path");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async move {
        let mut runtime = match which.as_str() {
            "ts" => BapRuntime::with_args_and_loader(
                bap_metrics::MetricsCollector::new(),
                Vec::new(),
                Rc::new(bap_ts::TsModuleLoader::for_user()),
            ),
            _ => BapRuntime::new(bap_metrics::MetricsCollector::new()),
        };
        match runtime.run_script(&script).await {
            Ok(()) => println!("done"),
            Err(error) => println!("error: {error}"),
        }
    });
}
