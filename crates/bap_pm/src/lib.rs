//! `bap` package manager: install npm packages into a project's node_modules.
//!
//! Resolution is flat and strict: every name resolves to exactly one version
//! that must satisfy all ranges in the graph. Conflicting ranges fail loudly
//! with the requirers listed instead of silently nesting copies.

pub mod project;
pub mod registry;
pub mod tarball;

use anyhow::{Context, Result, anyhow};
use project::LockedPackage;
use registry::{PackageVersion, Registry};
use semver::VersionReq;
use std::collections::BTreeMap;
use std::path::Path;

/// One dependency edge: who needs what range. The root package is `"."`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Requirement {
    dependent: String,
    name: String,
    range: VersionReq,
    raw_range: String,
}

/// Resolves the full dependency graph for `dependencies`.
///
/// Returns one chosen entry per package name; any two packages needing
/// incompatible versions of the same dependency aborts resolution.
pub async fn resolve(
    registry: &Registry,
    dependencies: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, PackageVersion>> {
    let mut requirements = Vec::new();
    let mut queue: Vec<(String, BTreeMap<String, String>)> =
        vec![(".".to_owned(), dependencies.clone())];
    let mut seen_versions: BTreeMap<String, Vec<String>> = BTreeMap::new();

    while let Some((dependent, deps)) = queue.pop() {
        for (name, raw_range) in deps {
            if raw_range.split_whitespace().next().is_none() {
                return Err(anyhow!("empty version range for '{name}'"));
            }
            let range = VersionReq::parse(&raw_range)
                .with_context(|| format!("invalid range '{raw_range}' for '{name}'"))?;
            requirements.push(Requirement {
                dependent: dependent.clone(),
                name: name.clone(),
                range,
                raw_range: raw_range.clone(),
            });

            // Visit each distinct package once per range set.
            seen_versions.entry(name.clone()).or_default();
            if !seen_versions[&name]
                .iter()
                .any(|seen| seen.as_str() == raw_range.as_str())
            {
                seen_versions
                    .get_mut(&name)
                    .unwrap()
                    .push(raw_range.clone());
                let versions = registry.packument(&name).await?;
                let chosen = Registry::pick(&versions, &VersionReq::parse(&raw_range)?, &name)?;
                if !chosen.dependencies.is_empty() {
                    queue.push((name, chosen.dependencies.clone()));
                }
            }
        }
    }

    let mut by_name: BTreeMap<String, Vec<&Requirement>> = BTreeMap::new();
    for requirement in &requirements {
        by_name
            .entry(requirement.name.clone())
            .or_default()
            .push(requirement);
    }

    let mut resolved = BTreeMap::new();
    for (name, requirements) in by_name {
        let packument = registry.packument(&name).await?;
        // Highest published version satisfying every range at once.
        let satisfying: Vec<VersionReq> = requirements
            .iter()
            .map(|requirement| requirement.range.clone())
            .collect();
        let chosen = packument
            .values()
            .rev()
            .find(|entry| satisfying.iter().all(|range| range.matches(&entry.version)))
            .ok_or_else(|| {
                let detail = requirements
                    .iter()
                    .map(|requirement| {
                        format!(
                            "  {} needs '{}'",
                            requirement.dependent, requirement.raw_range
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                anyhow!(
                    "conflicting requirements for '{name}'; no single version satisfies:\n{detail}"
                )
            })?
            .clone();
        resolved.insert(name, chosen);
    }
    Ok(resolved)
}

/// Installs everything in `dependencies` (transitively), refreshing the
/// lockfile and node_modules as needed. Skips network work when an existing
/// lockfile already matches and node_modules carries the exact versions.
pub async fn install(
    project: &Path,
    dependencies: &BTreeMap<String, String>,
) -> Result<Vec<String>> {
    if dependencies.is_empty() {
        return Ok(Vec::new());
    }
    let registry = Registry::new();
    let resolved = resolve(&registry, dependencies).await?;

    let mut lock = project::read_lock(project)?;
    let root = project::node_modules_root(project);
    let mut installed = Vec::new();

    for (name, entry) in &resolved {
        let already = lock.get(name).is_some_and(|locked| {
            locked.version == entry.version.to_string()
                && project::node_modules_root(project)
                    .join(name)
                    .join("package.json")
                    .is_file()
        });
        if already {
            continue;
        }

        let bytes = registry.download_verified(name, entry).await?;
        let destination = root.join(name);
        if destination.exists() {
            std::fs::remove_dir_all(&destination)
                .with_context(|| format!("replace {}", destination.display()))?;
        }
        let files = tarball::extract_package(&bytes, &destination)
            .with_context(|| format!("extract {name}"))?;

        lock.insert(
            name.clone(),
            LockedPackage {
                version: entry.version.to_string(),
                integrity: entry.integrity.clone().unwrap_or_default(),
                tarball: entry.tarball_url.clone(),
                dependents: BTreeMap::new(),
            },
        );
        installed.push(format!("{name}@{} ({files} files)", entry.version));
    }

    project::write_lock(project, &lock)?;
    Ok(installed)
}

/// Adds specs (`name[@range]`) to package.json, then installs.
pub async fn add(project: &Path, specs: &[String]) -> Result<Vec<String>> {
    if specs.is_empty() {
        return Err(anyhow!("bap add needs at least one package name"));
    }
    let registry = Registry::new();
    let mut dependencies = project::read_dependencies(project)?;
    for spec in specs {
        let (name, range) = split_spec(spec)?;
        let latest = latest_version(&registry, &name, &range).await?;
        dependencies.insert(name.clone(), format!("^{latest}"));
        project::write_dependency(project, &name, Some(&format!("^{latest}")))?;
    }
    install(project, &dependencies).await
}

/// Removes names from package.json, the lockfile, and node_modules.
pub fn remove(project: &Path, names: &[String]) -> Result<Vec<String>> {
    let mut removed = Vec::new();
    for name in names {
        project::write_dependency(project, name, None)?;
        lock_remove(project, name)?;
        let directory = project::node_modules_root(project).join(name);
        if directory.exists() {
            std::fs::remove_dir_all(&directory)
                .with_context(|| format!("remove {}", directory.display()))?;
        }
        removed.push(name.clone());
    }
    Ok(removed)
}

/// Installs from package.json alone (`bap install`).
pub async fn install_from_manifest(project: &Path) -> Result<Vec<String>> {
    let dependencies = project::read_dependencies(project)?;
    install(project, &dependencies).await
}

async fn latest_version(registry: &Registry, name: &str, requested: &str) -> Result<String> {
    let versions = registry.packument(name).await?;
    match requested {
        "" | "latest" => versions
            .values()
            .next_back()
            .map(|entry| entry.version.to_string())
            .ok_or_else(|| anyhow!("no versions found for '{name}'")),
        range => {
            let request = VersionReq::parse(range)
                .with_context(|| format!("invalid range '{range}' for '{name}'"))?;
            Registry::pick(&versions, &request, name).map(|entry| entry.version.to_string())
        }
    }
}

/// Splits `name`, `name@range`, or `@scope/name[@range]`.
fn split_spec(spec: &str) -> Result<(String, String)> {
    if spec.starts_with('@') {
        let (name, rest) = spec
            .split_once('/')
            .ok_or_else(|| anyhow!("scoped spec '{spec}' needs a '/' after the scope"))?;
        match rest.split_once('@') {
            Some((pkg, range)) => Ok((format!("{name}/{pkg}"), range.to_owned())),
            None => Ok((spec.to_owned(), "latest".to_owned())),
        }
    } else {
        match spec.split_once('@') {
            Some((name, range)) => Ok((name.to_owned(), range.to_owned())),
            None => Ok((spec.to_owned(), "latest".to_owned())),
        }
    }
}

fn lock_remove(project: &Path, name: &str) -> Result<()> {
    let mut lock = project::read_lock(project)?;
    lock.remove(name);
    project::write_lock(project, &lock)
}
