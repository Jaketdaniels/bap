//! Lockfile (`bap-lock.json`) and `package.json` dependency editing.

use anyhow::Context as _;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One resolved package as recorded in the lockfile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedPackage {
    pub version: String,
    pub integrity: String,
    pub tarball: String,
    /// Ranges that required this package, sorted for determinism.
    pub dependents: BTreeMap<String, String>,
}

/// Deterministic install state; sorted keys make diffs stable.
pub type Lockfile = BTreeMap<String, LockedPackage>;

pub fn lock_path(project: &Path) -> PathBuf {
    project.join("bap-lock.json")
}

pub fn node_modules_root(project: &Path) -> PathBuf {
    project.join("node_modules")
}

/// Reads the lockfile; a missing file is an empty lock.
pub fn read_lock(project: &Path) -> Result<Lockfile> {
    let path = lock_path(project);
    if !path.is_file() {
        return Ok(BTreeMap::new());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

pub fn write_lock(project: &Path, lock: &Lockfile) -> Result<()> {
    let mut rendered = serde_json::to_vec_pretty(lock).context("encode lockfile")?;
    rendered.push(b'\n');
    std::fs::write(lock_path(project), rendered).context("write lockfile")
}

/// Reads `dependencies` from the project's package.json (absent = empty).
pub fn read_dependencies(project: &Path) -> Result<BTreeMap<String, String>> {
    let manifest = project_manifest(project)?;
    let Some(Value::Object(map)) = manifest.get("dependencies") else {
        return Ok(BTreeMap::new());
    };
    Ok(map
        .iter()
        .filter_map(|(name, range)| range.as_str().map(|range| (name.clone(), range.to_owned())))
        .collect())
}

/// Adds or updates entries in `dependencies`, preserving all other content.
pub fn write_dependency(project: &Path, name: &str, range: Option<&str>) -> Result<()> {
    let path = project.join("package.json");
    if !path.is_file() {
        // No package.json yet: create a minimal one so installs persist.
        let seed = serde_json::json!({
            "name": project.file_name().and_then(|n| n.to_str()).unwrap_or("app"),
            "version": "0.0.0",
            "dependencies": {},
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&seed)?)
            .with_context(|| format!("create {}", path.display()))?;
    }
    let text = std::fs::read_to_string(&path)?;
    let mut manifest: Value = serde_json::from_str(&text).context("parse package.json")?;

    let object = manifest
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("package.json must contain an object"))?;
    let dependencies = object
        .entry("dependencies")
        .or_insert_with(|| Value::Object(Default::default()));
    let map = dependencies
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("package.json dependencies must be an object"))?;
    match range {
        Some(range) => {
            map.insert(name.to_owned(), Value::String(range.to_owned()));
        }
        None => {
            map.remove(name);
            if map.is_empty() {
                object.remove("dependencies");
            }
        }
    }

    let mut rendered = serde_json::to_vec_pretty(&manifest).context("encode package.json")?;
    rendered.push(b'\n');
    std::fs::write(&path, rendered).context("write package.json")
}

fn project_manifest(project: &Path) -> Result<Value> {
    let path = project.join("package.json");
    if !path.is_file() {
        return Ok(Value::Object(Default::default()));
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
}
