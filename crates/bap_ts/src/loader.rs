use crate::{Cache, CacheInput, transpile};
use base64::Engine;
use deno_core::{
    FsModuleLoader, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader,
    ModuleResolveResponse, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType,
    RequestedModuleType, ResolutionKind,
};
use deno_error::JsErrorBox;
use std::path::{Path, PathBuf};

/// Loads local module graphs and transpiles TypeScript modules.
pub struct TsModuleLoader {
    cache_root: Option<PathBuf>,
    files: FsModuleLoader,
}

impl TsModuleLoader {
    pub fn new(cache_root: impl AsRef<Path>) -> Result<Self, deno_core::anyhow::Error> {
        Ok(Self {
            cache_root: Some(cache_root.as_ref().to_owned()),
            files: FsModuleLoader,
        })
    }

    /// Uses an override or the current user's platform cache directory.
    pub fn for_user() -> Self {
        let cache_root = std::env::var_os("BAP_TS_CACHE_DIR")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                directories::ProjectDirs::from("", "", "bap")
                    .map(|dirs| dirs.cache_dir().join("typescript").join("v1"))
            });
        Self {
            cache_root,
            files: FsModuleLoader,
        }
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
        if !is_typescript(module_specifier)
            || options.requested_module_type != RequestedModuleType::None
        {
            return self.files.load(module_specifier, maybe_referrer, options);
        }

        let result = (|| {
            let path = module_specifier
                .to_file_path()
                .map_err(|_| JsErrorBox::type_error("TypeScript modules must use file URLs"))?;
            let source = std::fs::read_to_string(&path).map_err(|error| {
                JsErrorBox::generic(format!("failed to read {}: {error}", path.display()))
            })?;
            let input = CacheInput {
                specifier: module_specifier.to_string(),
                source,
            };
            let artifact = match self
                .cache_root
                .as_ref()
                .and_then(|root| Cache::new(root).ok())
            {
                Some(cache) => cache.get_or_transpile(&input),
                None => {
                    transpile(&input.specifier, &input.source).map(|output| crate::CachedArtifact {
                        path: PathBuf::new(),
                        code: output.code,
                        source_map: output.source_map,
                        cache_hit: false,
                    })
                }
            }
            .map_err(|error| JsErrorBox::generic(format!("{error:#}")))?;
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
