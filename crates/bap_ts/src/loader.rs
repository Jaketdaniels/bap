use crate::{Cache, CacheInput};
use base64::Engine;
use deno_core::{
    FsModuleLoader, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader,
    ModuleResolveResponse, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType,
    ResolutionKind,
};
use deno_error::JsErrorBox;
use std::path::Path;

/// Loads local module graphs and transpiles TypeScript modules.
pub struct TsModuleLoader {
    cache: Cache,
    files: FsModuleLoader,
}

impl TsModuleLoader {
    pub fn new(cache_root: impl AsRef<Path>) -> Result<Self, deno_core::anyhow::Error> {
        Ok(Self {
            cache: Cache::new(cache_root)?,
            files: FsModuleLoader,
        })
    }
}

impl ModuleLoader for TsModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        kind: ResolutionKind,
    ) -> ModuleResolveResponse {
        self.files.resolve(specifier, referrer, kind)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        maybe_referrer: Option<&ModuleLoadReferrer>,
        options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
        if !is_typescript(module_specifier) {
            return self.files.load(module_specifier, maybe_referrer, options);
        }

        let result = (|| {
            let path = module_specifier
                .to_file_path()
                .map_err(|_| JsErrorBox::type_error("TypeScript modules must use file URLs"))?;
            let source = std::fs::read_to_string(&path).map_err(|error| {
                JsErrorBox::generic(format!("failed to read {}: {error}", path.display()))
            })?;
            let artifact = self
                .cache
                .get_or_transpile(&CacheInput {
                    specifier: module_specifier.to_string(),
                    source,
                })
                .map_err(|error| JsErrorBox::generic(error.to_string()))?;
            let mut code = artifact.code;
            if !artifact.source_map.is_empty() {
                if !code.ends_with('\n') {
                    code.push('\n');
                }
                let map = base64::engine::general_purpose::STANDARD
                    .encode(artifact.source_map.as_bytes());
                code.push_str("//# sourceMappingURL=data:application/json;base64,");
                code.push_str(&map);
            }
            Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(code.into()),
                module_specifier,
                None,
            ))
        })();
        ModuleLoadResponse::Sync(result)
    }
}

fn is_typescript(specifier: &ModuleSpecifier) -> bool {
    matches!(
        Path::new(specifier.path())
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("ts" | "mts")
    )
}
