use crate::cache::{Cache, CacheInput, CachedArtifact};
use crate::proof::backend::ProofBackend;
use crate::proof::manifest::{ProofAssessment, assess};
use crate::proof::{ProofMode, is_proof_candidate, specialize};
use crate::{ProofBuild, transpile};
use base64::Engine;
use deno_core::{
    FsModuleLoader, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader,
    ModuleResolveResponse, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType,
    RequestedModuleType, ResolutionKind,
};
use deno_error::JsErrorBox;
use std::path::{Path, PathBuf};

const NO_BACKEND_MESSAGE: &str =
    "no Node proof tool found; set BAP_TS_PROOF_TOOL to tools/ts-proof/proof.mjs";

/// Loads local module graphs and transpiles TypeScript modules.
pub struct TsModuleLoader {
    cache_root: Option<PathBuf>,
    files: FsModuleLoader,
    proof_mode: ProofMode,
    backend: Option<ProofBackend>,
}

impl TsModuleLoader {
    pub fn new(cache_root: impl AsRef<Path>) -> Result<Self, deno_core::anyhow::Error> {
        Ok(Self {
            cache_root: Some(cache_root.as_ref().to_owned()),
            files: FsModuleLoader,
            proof_mode: ProofMode::Normal,
            backend: ProofBackend::discover(),
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
            proof_mode: ProofMode::Normal,
            backend: ProofBackend::discover(),
        }
    }

    /// Sets how eagerly loads consult the proof backend.
    pub fn with_proof_mode(mut self, mode: ProofMode) -> Self {
        self.proof_mode = mode;
        self
    }

    fn build_artifact(&self, input: &CacheInput) -> Result<CachedArtifact, JsErrorBox> {
        let cache = self
            .cache_root
            .as_ref()
            .and_then(|root| Cache::new(root).ok());
        if let Some(hit) = cache.as_ref().and_then(|cache| cache.lookup(input)) {
            return Ok(hit);
        }
        if self.proof_mode == ProofMode::Disabled || !is_proof_candidate(&input.source) {
            return self.store_or_transpile(&cache, input, None);
        }

        let backend = match (&self.backend, self.proof_mode) {
            (Some(backend), _) => backend,
            (None, ProofMode::Required) => {
                return Err(JsErrorBox::generic(format!(
                    "bap: --require-proofs needs a proof backend; {NO_BACKEND_MESSAGE}"
                )));
            }
            (None, _) => return self.store_or_transpile(&cache, input, None),
        };

        match self.run_proof(backend, input) {
            Ok(proof) => self.store_or_transpile(&cache, input, proof),
            Err(failure) => {
                if self.proof_mode == ProofMode::Required {
                    return Err(JsErrorBox::generic(format!(
                        "bap: --require-proofs failed; {failure}"
                    )));
                }
                eprintln!("bap: continuing without proofs ({failure})");
                self.store_or_transpile(&cache, input, None)
            }
        }
    }

    fn run_proof(
        &self,
        backend: &ProofBackend,
        input: &CacheInput,
    ) -> Result<Option<ProofBuild>, String> {
        let manifest = backend
            .prove(&input.source, &input.specifier)
            .map_err(|error| format!("proof backend failed: {error:#}"))?;
        let hard_failure = |detail: String| detail;

        match assess(&manifest, &input.specifier, &input.source) {
            Ok(ProofAssessment::Proven(entries)) => {
                let edits = specialize::plan_edits(&entries, &input.source)
                    .map_err(|rejection| format!("specialization refused ({rejection})"))?;
                let working_source = specialize::apply_edits(&input.source, &edits);
                let stored = serde_json::to_value(&manifest)
                    .map_err(|error| format!("proof manifest encode failed: {error}"))?;
                Ok(Some(ProofBuild {
                    working_source,
                    manifest: stored,
                }))
            }
            Ok(ProofAssessment::Failed {
                diagnostic,
                message,
            }) => Err(hard_failure(format!(
                "TypeScript rejected the file ({diagnostic}): {message}"
            ))),
            Ok(ProofAssessment::Unproven { reason }) => {
                Err(hard_failure(format!("candidate unproven: {reason}")))
            }
            Err(rejection) => Err(hard_failure(format!(
                "manifest rejected ({}): {}",
                rejection.code(),
                rejection.message()
            ))),
        }
    }

    fn store_or_transpile(
        &self,
        cache: &Option<Cache>,
        input: &CacheInput,
        proof: Option<ProofBuild>,
    ) -> Result<CachedArtifact, JsErrorBox> {
        if let Some(cache) = cache {
            return cache
                .store(input, proof.as_ref())
                .map_err(|error| JsErrorBox::generic(format!("{error:#}")));
        }
        let working_source = match &proof {
            Some(build) => build.working_source.as_str(),
            None => input.source.as_str(),
        };
        let output = transpile(&input.specifier, working_source)
            .map_err(|error| JsErrorBox::generic(format!("{error:#}")))?;
        Ok(CachedArtifact {
            path: PathBuf::new(),
            code: output.code,
            source_map: output.source_map,
            cache_hit: false,
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
            let artifact = self.build_artifact(&input)?;
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
