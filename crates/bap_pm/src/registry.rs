//! npm registry protocol: packuments, version picking, tarball downloads.

use anyhow::{Result, anyhow};
use semver::{Version, VersionReq};
use serde::Deserialize;
use sha2::{Digest, Sha512};
use std::collections::BTreeMap;

/// One concrete version entry from a packument.
#[derive(Debug, Clone)]
pub struct PackageVersion {
    pub version: Version,
    pub tarball_url: String,
    /// SRI string from `dist.integrity` (`sha512-<base64>`), when present.
    pub integrity: Option<String>,
    pub dependencies: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct Packument {
    #[serde(default)]
    versions: BTreeMap<String, PackumentVersion>,
}

#[derive(Deserialize)]
struct PackumentVersion {
    version: String,
    dist: Dist,
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct Dist {
    tarball: String,
    #[serde(default)]
    integrity: Option<String>,
}

/// Client bound to a registry base (override with `BAP_REGISTRY`).
pub struct Registry {
    client: reqwest::Client,
    base: String,
}

impl Registry {
    pub fn new() -> Self {
        let base = std::env::var("BAP_REGISTRY")
            .unwrap_or_else(|_| "https://registry.npmjs.org".to_owned());
        Self {
            client: reqwest::Client::builder()
                .user_agent(concat!("bap/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("registry http client"),
            base: base.trim_end_matches('/').to_owned(),
        }
    }

    fn encoded(name: &str) -> String {
        // Scoped names use @scope/pkg; the registry expects the slash escaped.
        name.replace('/', "%2f")
    }

    /// Fetches every published version of `name`.
    pub async fn packument(&self, name: &str) -> Result<BTreeMap<Version, PackageVersion>> {
        let url = format!("{}/{}", self.base, Self::encoded(name));
        let packument: Packument = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|error| anyhow!("registry request for {name} failed: {error}"))?
            .error_for_status()
            .map_err(|error| {
                if error.status() == Some(reqwest::StatusCode::NOT_FOUND) {
                    anyhow!("package '{name}' was not found on the registry")
                } else {
                    anyhow!("registry request for {name} failed: {error}")
                }
            })?
            .json()
            .await
            .map_err(|error| anyhow!("invalid packument for {name}: {error}"))?;

        let mut out = BTreeMap::new();
        for (text, entry) in packument.versions {
            let Ok(key) = Version::parse(&text) else {
                continue;
            };
            let version = Version::parse(&entry.version).unwrap_or_else(|_| key.clone());
            out.insert(
                key,
                PackageVersion {
                    version,
                    tarball_url: entry.dist.tarball,
                    integrity: entry.dist.integrity,
                    dependencies: entry.dependencies,
                },
            );
        }
        Ok(out)
    }

    /// Highest stable version satisfying `range`; prereleases only match
    /// ranges that ask for them.
    pub fn pick(
        versions: &BTreeMap<Version, PackageVersion>,
        range: &VersionReq,
        name: &str,
    ) -> Result<PackageVersion> {
        versions
            .values()
            .rev()
            .find(|entry| range.matches(&entry.version))
            .cloned()
            .ok_or_else(|| {
                anyhow!(
                    "no version of '{name}' satisfies '{range}' ({} published)",
                    versions.len()
                )
            })
    }

    /// Downloads a tarball and verifies its digest against the recorded one.
    pub async fn download_verified(&self, name: &str, entry: &PackageVersion) -> Result<Vec<u8>> {
        let bytes = self
            .client
            .get(&entry.tarball_url)
            .send()
            .await
            .map_err(|error| anyhow!("tarball download for {name} failed: {error}"))?
            .error_for_status()
            .map_err(|error| anyhow!("tarball download for {name} failed: {error}"))?
            .bytes()
            .await
            .map_err(|error| anyhow!("tarball read for {name} failed: {error}"))?
            .to_vec();

        verify_integrity(name, &bytes, entry)?;
        Ok(bytes)
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

fn verify_integrity(name: &str, bytes: &[u8], entry: &PackageVersion) -> Result<()> {
    let integrity = entry.integrity.as_deref().ok_or_else(|| {
        anyhow!("registry published no integrity for {name}; refusing to install")
    })?;
    let expected = integrity
        .strip_prefix("sha512-")
        .ok_or_else(|| anyhow!("unsupported integrity algorithm for {name}: {integrity}"))?;
    let digest = Sha512::digest(bytes);
    if !sri_matches(expected, &digest) {
        return Err(anyhow!(
            "integrity mismatch for {name}; refusing to install"
        ));
    }
    Ok(())
}

/// Constant-time-ish base64 comparison against an SRI digest.
fn sri_matches(expected_base64: &str, digest: &[u8]) -> bool {
    match base64_decode(expected_base64.trim()) {
        Some(expected) => {
            expected.len() == digest.len()
                && expected
                    .iter()
                    .zip(digest.iter())
                    .all(|(want, got)| want == got)
        }
        None => false,
    }
}

/// Minimal standard base64 decoder (no padding tolerance beyond '=' tail).
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let bytes: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .take_while(|byte| *byte != b'=')
        .collect();
    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 {
            return None;
        }
        let values: Option<Vec<usize>> = chunk
            .iter()
            .map(|b| TABLE.iter().position(|t| t == b))
            .collect();
        let values = values?;
        out.push(((values[0] << 2) | (values[1] >> 4)) as u8);
        if values.len() > 2 {
            out.push((((values[1] & 0x0f) << 4) | (values[2] >> 2)) as u8);
        }
        if values.len() > 3 {
            out.push((((values[2] & 0x03) << 6) | values[3]) as u8);
        }
    }
    Some(out)
}
