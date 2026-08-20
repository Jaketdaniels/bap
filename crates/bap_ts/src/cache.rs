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
        prepare_root(root.as_ref())?;
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
        let _ = self.write_atomic(&path, &encoded);

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
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&temporary)
                .context("failed to create temporary TypeScript artifact")?;
            file.write_all(bytes)
                .context("failed to write TypeScript artifact")?;
            file.sync_all()
                .context("failed to sync TypeScript artifact")?;
            fs::rename(&temporary, destination).context("failed to publish TypeScript artifact")?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        write_result
    }
}

fn prepare_root(root: &Path) -> Result<()> {
    match fs::symlink_metadata(root) {
        Ok(metadata) => validate_root(root, &metadata)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => create_secure_dirs(root)?,
        Err(error) => return Err(error).context("failed to inspect TypeScript cache"),
    }
    let metadata = fs::symlink_metadata(root).context("failed to inspect TypeScript cache")?;
    validate_root(root, &metadata)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))
            .context("failed to secure TypeScript cache")?;
    }
    Ok(())
}

fn validate_root(root: &Path, metadata: &fs::Metadata) -> Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!(
            "TypeScript cache root is not a directory: {}",
            root.display()
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != rustix::process::geteuid().as_raw() {
            bail!(
                "TypeScript cache root has a different owner: {}",
                root.display()
            );
        }
    }
    Ok(())
}

#[cfg(unix)]
fn create_secure_dirs(root: &Path) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    let mut missing = Vec::new();
    let mut current = root;
    while !current.exists() {
        missing.push(current.to_owned());
        current = current
            .parent()
            .context("TypeScript cache has no existing parent")?;
    }
    for directory in missing.iter().rev() {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(directory)
            .with_context(|| {
                format!(
                    "failed to create TypeScript cache directory {}",
                    directory.display()
                )
            })?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn create_secure_dirs(root: &Path) -> Result<()> {
    fs::create_dir_all(root).context("failed to create TypeScript cache")
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

    #[test]
    fn publish_failure_does_not_discard_transpiled_code() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path()).unwrap();
        let first = cache.get_or_transpile(&fixture()).unwrap();
        std::fs::remove_file(&first.path).unwrap();
        std::fs::create_dir(&first.path).unwrap();

        let second = cache.get_or_transpile(&fixture()).unwrap();
        assert_eq!(second.code, first.code);
        assert!(!second.cache_hit);
    }

    #[cfg(unix)]
    #[test]
    fn cache_root_and_artifacts_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("nested").join("cache");
        let cache = Cache::new(&root).unwrap();
        let artifact = cache.get_or_transpile(&fixture()).unwrap();
        assert_eq!(
            std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(artifact.path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink_cache_root() {
        let parent = tempfile::tempdir().unwrap();
        let actual = parent.path().join("actual");
        let root = parent.path().join("cache");
        std::fs::create_dir(&actual).unwrap();
        std::os::unix::fs::symlink(actual, &root).unwrap();
        assert!(Cache::new(root).is_err());
    }
}
