use crate::cache::{Cache, CacheInput, CachedArtifact};
use crate::cjs::{ModuleKind, PackageTypeCache, synthesize_wrapper};
use crate::proof::backend::ProofBackend;
use crate::proof::manifest::{ProofAssessment, assess};
use crate::proof::{ProofMode, is_proof_candidate, specialize};
use crate::shims::{SHIM_SCHEME, find as find_shim, module_source as shim_module_source};
use crate::{ProofBuild, transpile};
use base64::Engine;
use deno_core::{
    FsModuleLoader, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader,
    ModuleResolveResponse, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType,
    RequestedModuleType, ResolutionKind,
};
use deno_error::JsErrorBox;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const NO_BACKEND_MESSAGE: &str =
    "no Node proof tool found; set BAP_TS_PROOF_TOOL to tools/ts-proof/proof.mjs";

/// Loads local module graphs, transpiles TypeScript modules, and wraps CommonJS.
pub struct TsModuleLoader {
    cache_root: Option<PathBuf>,
    files: FsModuleLoader,
    proof_mode: ProofMode,
    backend: Option<ProofBackend>,
    package_types: PackageTypeCache,
    aliases: BTreeMap<String, PathBuf>,
    format_override: Option<ModuleKind>,
}

impl TsModuleLoader {
    pub fn new(cache_root: impl AsRef<Path>) -> Result<Self, deno_core::anyhow::Error> {
        Ok(Self {
            cache_root: Some(cache_root.as_ref().to_owned()),
            files: FsModuleLoader,
            proof_mode: ProofMode::Normal,
            backend: ProofBackend::discover(),
            package_types: PackageTypeCache::default(),
            aliases: BTreeMap::new(),
            format_override: None,
        })
    }

    /// Maps bare specifiers onto local files; values resolve from `base`.
    pub fn with_aliases(mut self, base: &Path, aliases: &BTreeMap<String, String>) -> Self {
        for (name, target) in aliases {
            let path = if Path::new(target).is_absolute() {
                PathBuf::from(target)
            } else {
                base.join(target)
            };
            self.aliases.insert(name.clone(), path);
        }
        self
    }

    /// Forces `.js` classification regardless of markers.
    pub fn with_format_override(mut self, kind: ModuleKind) -> Self {
        self.format_override = Some(kind);
        self
    }

    /// Uses an override or the current user's platform cache directory.
    pub fn for_user() -> Self {
        let cache_root = std::env::var_os("BAP_TS_CACHE_DIR")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                directories::ProjectDirs::from("", "", "bap")
                    .map(|dirs| dirs.cache_dir().join("typescript").join("v2"))
            });
        Self {
            cache_root,
            files: FsModuleLoader,
            proof_mode: ProofMode::Normal,
            backend: ProofBackend::discover(),
            package_types: PackageTypeCache::default(),
            aliases: BTreeMap::new(),
            format_override: None,
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
        if let Some(name) = specifier
            .strip_prefix("node:")
            .or_else(|| specifier.strip_prefix("bap:"))
        {
            return match find_shim(name) {
                Some(_) => ModuleSpecifier::parse(&format!("{SHIM_SCHEME}://{name}"))
                    .map_err(|error| JsErrorBox::generic(error.to_string())),
                None => Err(JsErrorBox::generic(format!(
                    "bap: node:{name} is not implemented on Bap"
                ))),
            };
        }
        if let Some(path) = self.alias_target(specifier) {
            let url = deno_core::ModuleSpecifier::from_file_path(path).map_err(|_| {
                JsErrorBox::generic(format!("bap: alias '{specifier}' is not a valid path"))
            })?;
            return Ok(url);
        }
        if kind != ResolutionKind::MainModule && is_bare_specifier(specifier) {
            // Installed packages: walk ancestor node_modules directories
            // from the importing file.
            if let Some(path) = self.node_modules_target(referrer, specifier) {
                return deno_core::ModuleSpecifier::from_file_path(path).map_err(|_| {
                    JsErrorBox::generic(format!(
                        "bap: package '{specifier}' resolved to an invalid path"
                    ))
                });
            }
            return Err(JsErrorBox::generic(format!(
                "bap: cannot find module '{specifier}' (is it installed? run `bap add {specifier}`)"
            )));
        }
        self.files.resolve(specifier, referrer, kind)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        maybe_referrer: Option<&ModuleLoadReferrer>,
        options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
        if module_specifier.scheme() == SHIM_SCHEME {
            let name = module_specifier.host_str().unwrap_or_default();
            let code = shim_module_source(name);
            return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(code.into()),
                module_specifier,
                None,
            )));
        }
        if options.requested_module_type != RequestedModuleType::None {
            return self.files.load(module_specifier, maybe_referrer, options);
        }
        if is_typescript(module_specifier) {
            return self.load_typescript(module_specifier);
        }
        self.load_plain(module_specifier)
    }
}

impl TsModuleLoader {
    fn alias_target(&self, specifier: &str) -> Option<PathBuf> {
        let path = self.aliases.get(specifier)?;
        std::fs::canonicalize(path).ok()
    }

    /// Walks ancestor `node_modules` directories starting at the referrer.
    /// `specifier` may carry a subpath (`pkg/sub/file.js`); directories fall
    /// back to their package.json `main`, then `index.js`.
    fn node_modules_target(&self, referrer: &str, specifier: &str) -> Option<PathBuf> {
        let referrer_path = ModuleSpecifier::parse(referrer)
            .ok()
            .and_then(|url| url.to_file_path().ok())?;
        let mut directory = referrer_path.parent()?.to_path_buf();
        loop {
            let base = directory.join("node_modules").join(specifier);
            for candidate in [
                base.clone(),
                with_extension(&base, "js"),
                with_extension(&base, "cjs"),
            ] {
                if candidate.is_file() {
                    return std::fs::canonicalize(candidate).ok();
                }
            }
            if base.is_dir() {
                if let Some(main) = package_main(&base).filter(|main| main.is_file()) {
                    return std::fs::canonicalize(main).ok();
                }
                let index = base.join("index.js");
                if index.is_file() {
                    return std::fs::canonicalize(index).ok();
                }
            }
            directory = directory.parent()?.to_path_buf();
        }
    }
}

fn with_extension(path: &Path, extension: &str) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(".");
    os.push(extension);
    PathBuf::from(os)
}

fn package_main(package_directory: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(package_directory.join("package.json")).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&text).ok()?;
    let main = manifest.get("main")?.as_str()?;
    Some(package_directory.join(main))
}

fn is_bare_specifier(specifier: &str) -> bool {
    !specifier.starts_with("./")
        && !specifier.starts_with("../")
        && !specifier.starts_with('/')
        && ModuleSpecifier::parse(specifier).is_err()
}

impl TsModuleLoader {
    /// Serves `.js`/`.cjs` sources, wrapping CommonJS into synthetic ESM.
    fn load_plain(&self, module_specifier: &ModuleSpecifier) -> ModuleLoadResponse {
        let result = (|| {
            let path = module_specifier
                .to_file_path()
                .map_err(|_| JsErrorBox::type_error("modules must use file URLs"))?;
            let source = std::fs::read_to_string(&path).map_err(|error| {
                JsErrorBox::generic(format!("failed to read {}: {error}", path.display()))
            })?;
            let code = match path.extension().and_then(|extension| extension.to_str()) {
                Some("js" | "cjs")
                    if self
                        .package_types
                        .classify_cached(&path, &source, self.format_override)
                        == ModuleKind::CommonJs =>
                {
                    synthesize_wrapper(&path, &source)
                }
                _ => source,
            };
            Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(code.into()),
                module_specifier,
                None,
            ))
        })();
        ModuleLoadResponse::Sync(result)
    }

    /// Runs the TypeScript pipeline: proof specialization plus the artifact cache.
    fn load_typescript(&self, module_specifier: &ModuleSpecifier) -> ModuleLoadResponse {
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
