use bap_ts::TsModuleLoader;
use base64::Engine;
use deno_core::{
    JsRuntime, ModuleLoadOptions, ModuleLoadResponse, ModuleLoader, RequestedModuleType,
    RuntimeOptions,
};
use std::path::Path;
use std::rc::Rc;

async fn run(entry: &Path, cache: &Path) -> Result<(), deno_core::anyhow::Error> {
    let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(Rc::new(TsModuleLoader::new(cache)?)),
        ..Default::default()
    });
    let specifier = deno_core::resolve_path(entry.to_str().unwrap(), &std::env::current_dir()?)?;
    let module = runtime.load_main_es_module(&specifier).await?;
    let result = runtime.mod_evaluate(module);
    runtime.run_event_loop(Default::default()).await?;
    result.await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn executes_ts_mts_and_mixed_graphs() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let dep = dir.path().join("dep.ts");
    let ts = dir.path().join("entry.ts");
    let mts = dir.path().join("entry.mts");
    let js = dir.path().join("mixed.js");
    std::fs::write(&dep, "export const answer: number = 42;\n").unwrap();
    std::fs::write(
        &ts,
        "import { answer } from './dep.ts'; answer satisfies number;\n",
    )
    .unwrap();
    std::fs::write(&mts, "const answer: number = 42; export { answer };\n").unwrap();
    std::fs::write(&js, "import { answer } from './dep.ts'; answer;\n").unwrap();

    run(&ts, &cache).await.unwrap();
    run(&mts, &cache).await.unwrap();
    run(&js, &cache).await.unwrap();
}

#[test]
fn appends_an_inline_source_map() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("entry.ts");
    std::fs::write(&source, "export const answer: number = 42;\n").unwrap();
    let specifier = deno_core::ModuleSpecifier::from_file_path(&source).unwrap();
    let loader = TsModuleLoader::new(dir.path().join("cache")).unwrap();
    let response = loader.load(
        &specifier,
        None,
        ModuleLoadOptions {
            is_dynamic_import: false,
            is_synchronous: true,
            requested_module_type: RequestedModuleType::None,
        },
    );
    let ModuleLoadResponse::Sync(Ok(module)) = response else {
        panic!("expected synchronous TypeScript module");
    };
    let code = String::from_utf8_lossy(module.code.as_bytes());
    let encoded = code
        .split("//# sourceMappingURL=data:application/json;base64,")
        .nth(1)
        .expect("inline source map");
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .unwrap();
    let map: serde_json::Value = serde_json::from_slice(&decoded).unwrap();
    assert_eq!(
        map["sourcesContent"][0],
        "export const answer: number = 42;\n"
    );
    assert!(map["sources"][0].as_str().unwrap().ends_with("entry.ts"));
}

#[test]
fn delegates_requested_text_modules_without_transpiling() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("entry.ts");
    let original = "export const answer: number = 42;\n";
    std::fs::write(&source, original).unwrap();
    let specifier = deno_core::ModuleSpecifier::from_file_path(&source).unwrap();
    let loader = TsModuleLoader::new(dir.path().join("cache")).unwrap();
    let response = loader.load(
        &specifier,
        None,
        ModuleLoadOptions {
            is_dynamic_import: false,
            is_synchronous: true,
            requested_module_type: RequestedModuleType::Text,
        },
    );
    let ModuleLoadResponse::Async(future) = response else {
        panic!("expected delegated filesystem load");
    };
    let module = futures::executor::block_on(future).unwrap();
    assert_eq!(module.module_type, deno_core::ModuleType::Text);
    assert_eq!(module.code.as_bytes(), original.as_bytes());
}

#[tokio::test(flavor = "current_thread")]
async fn javascript_does_not_require_a_usable_cache_root() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("occupied");
    let entry = dir.path().join("entry.js");
    std::fs::write(&cache, "not a directory").unwrap();
    std::fs::write(&entry, "export const answer = 42;\n").unwrap();

    run(&entry, &cache).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn runtime_error_uses_original_typescript_location() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let entry = dir.path().join("mapped.ts");
    std::fs::write(
        &entry,
        "interface Hidden {}\nconst value: number = 1;\nthrow new Error(`mapped ${value}`);\n",
    )
    .unwrap();

    let error = run(&entry, &cache).await.unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("mapped.ts:3:"), "error was: {message}");
}
