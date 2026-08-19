use crate::transpile;
use deno_core::anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA: &str = "bap-ts-artifact-v1";
const COMPILER_VERSION: &str = "deno_ast-0.53.3";
const OPTIONS_FINGERPRINT: &str =
    "module=esm;sourcemap=separate;inline_sources=true;transpile_options=default";
const MAX_ARTIFACT_SIZE: u64 = 64 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Source material used to build a cache artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheInput {
    pub specifier: String,
    pub source: String,
}

/// Validated transpiler output and its cache location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedArtifact {
    pub path: PathBuf,
    pub code: String,
    pub source_map: String,
    pub cache_hit: bool,
}

/// Content-addressed TypeScript artifact cache.
#[derive(Clone)]
pub struct Cache {
    root: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
struct ArtifactV1 {
    schema: String,
    key: String,
    specifier: String,
    source_hash: String,
    compiler_version: String,
    options_fingerprint: String,
    code: String,
    code_hash: String,
    source_map: String,
    source_map_hash: String,
}

impl Cache {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        fs::create_dir_all(root.as_ref()).context("failed to create TypeScript cache")?;
        Ok(Self {
            root: root.as_ref().to_owned(),
        })
    }

    pub fn get_or_transpile(&self, input: &CacheInput) -> Result<CachedArtifact> {
        let specifier = canonical_specifier(&input.specifier)?;
        let key = content_key(&specifier, input.source.as_bytes());
        let path = self.root.join(&key);
        if let Some(hit) = self.read_valid(&path, &key, &specifier, input.source.as_bytes()) {
            return Ok(hit);
        }

        let output = transpile(&specifier, &input.source)?;
        let artifact = ArtifactV1 {
            schema: SCHEMA.to_owned(),
            key,
            specifier,
            source_hash: digest(input.source.as_bytes()),
            compiler_version: COMPILER_VERSION.to_owned(),
            options_fingerprint: OPTIONS_FINGERPRINT.to_owned(),
            code_hash: digest(output.code.as_bytes()),
            source_map_hash: digest(output.source_map.as_bytes()),
            code: output.code,
            source_map: output.source_map,
        };
        let encoded =
            serde_json::to_vec(&artifact).context("failed to encode TypeScript artifact")?;
        if encoded.len() as u64 > MAX_ARTIFACT_SIZE {
            bail!("TypeScript artifact exceeds 64 MiB");
        }
        self.write_atomic(&path, &encoded)?;

        Ok(CachedArtifact {
            path,
            code: artifact.code,
            source_map: artifact.source_map,
            cache_hit: false,
        })
    }

    fn read_valid(
        &self,
        path: &Path,
        expected_key: &str,
        expected_specifier: &str,
        source: &[u8],
    ) -> Option<CachedArtifact> {
        let metadata = fs::metadata(path).ok()?;
        if !metadata.is_file() || metadata.len() > MAX_ARTIFACT_SIZE {
            return None;
        }
        let bytes = fs::read(path).ok()?;
        if bytes.len() as u64 != metadata.len() {
            return None;
        }
        let artifact: ArtifactV1 = serde_json::from_slice(&bytes).ok()?;
        if artifact.schema != SCHEMA
            || artifact.key != expected_key
            || artifact.specifier != expected_specifier
            || artifact.source_hash != digest(source)
            || artifact.compiler_version != COMPILER_VERSION
            || artifact.options_fingerprint != OPTIONS_FINGERPRINT
            || artifact.code_hash != digest(artifact.code.as_bytes())
            || artifact.source_map_hash != digest(artifact.source_map.as_bytes())
            || artifact.code.len() as u64 + artifact.source_map.len() as u64 > MAX_ARTIFACT_SIZE
            || (!artifact.source_map.is_empty()
                && !matches!(
                    serde_json::from_str::<serde_json::Value>(&artifact.source_map),
                    Ok(serde_json::Value::Object(_))
                ))
        {
            return None;
        }
        Some(CachedArtifact {
            path: path.to_owned(),
            code: artifact.code,
            source_map: artifact.source_map,
            cache_hit: true,
        })
    }

    fn write_atomic(&self, destination: &Path, bytes: &[u8]) -> Result<()> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            ^ u128::from(TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed));
        let temporary = self.root.join(format!(
            "{}{:032x}{:08x}",
            destination
                .file_name()
                .and_then(|name| name.to_str())
                .context("invalid artifact filename")?,
            nonce,
            std::process::id()
        ));
        let write_result = (|| -> Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .context("failed to create temporary TypeScript artifact")?;
            file.write_all(bytes)
                .context("failed to write TypeScript artifact")?;
            file.sync_all()
                .context("failed to sync TypeScript artifact")?;
            fs::rename(&temporary, destination).context("failed to publish TypeScript artifact")?;
            fs::File::open(&self.root)
                .and_then(|directory| directory.sync_all())
                .context("failed to sync TypeScript cache directory")?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        write_result
    }
}

fn canonical_specifier(specifier: &str) -> Result<String> {
    Ok(deno_core::url::Url::parse(specifier)
        .context("invalid module specifier")?
        .to_string())
}

fn content_key(specifier: &str, source: &[u8]) -> String {
    let mut hasher = Sha256::new();
    for field in [
        SCHEMA.as_bytes(),
        specifier.as_bytes(),
        source,
        COMPILER_VERSION.as_bytes(),
        OPTIONS_FINGERPRINT.as_bytes(),
    ] {
        hasher.update((field.len() as u64).to_be_bytes());
        hasher.update(field);
    }
    format!("{:x}", hasher.finalize())
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> CacheInput {
        CacheInput {
            specifier: "file:///entry.ts".to_owned(),
            source: "export const answer: number = 42;\n".to_owned(),
        }
    }

    #[test]
    fn corrupt_artifact_is_rebuilt() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path()).unwrap();
        let first = cache.get_or_transpile(&fixture()).unwrap();
        std::fs::write(&first.path, b"not json").unwrap();
        let second = cache.get_or_transpile(&fixture()).unwrap();
        assert_eq!(second.code, first.code);
        assert!(!second.cache_hit);
    }

    #[test]
    fn valid_artifact_is_a_cache_hit() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path()).unwrap();
        let first = cache.get_or_transpile(&fixture()).unwrap();
        let second = cache.get_or_transpile(&fixture()).unwrap();
        assert_eq!(second.path, first.path);
        assert_eq!(second.code, first.code);
        assert!(second.cache_hit);
    }

    #[test]
    fn changed_source_uses_a_new_content_key() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path()).unwrap();
        let first = cache.get_or_transpile(&fixture()).unwrap();
        let mut changed = fixture();
        changed.source = "export const answer: number = 43;\n".to_owned();
        let second = cache.get_or_transpile(&changed).unwrap();
        assert_ne!(second.path, first.path);
        assert!(!second.cache_hit);
    }
}
