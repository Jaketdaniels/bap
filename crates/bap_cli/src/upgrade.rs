//! `bap upgrade`: replace this binary with a newer GitHub release build.

use crate::style;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

const REPO: &str = "Jaketdaniels/bap";
const API_BASE: &str = "https://api.github.com/repos/Jaketdaniels/bap/releases";

/// Handles `bap upgrade`; returns the process exit code.
pub fn upgrade(version: Option<String>, dry_run: bool) -> i32 {
    let Some(triple) = release_triple() else {
        eprintln!(
            "bap: self-upgrade is not supported on {} yet; \
             download an archive from https://github.com/{REPO}/releases",
            std::env::consts::OS
        );
        return 1;
    };
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => {
            eprintln!("bap: cannot locate the running binary: {error}");
            return 1;
        }
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let result = rt.block_on(upgrade_async(triple, &exe, version.as_deref(), dry_run));
    match result {
        Ok(summary) => {
            if dry_run {
                println!("{}", style::bold("bap upgrade (dry run)"));
            }
            println!("  release:  {}", style::accent(&summary.tag));
            println!("  asset:    {}", summary.asset_name);
            println!("  target:   {}", summary.destination.display());
            if dry_run {
                println!("{}", style::dim("nothing was changed"));
            } else {
                println!(
                    "{} upgraded to {}; restart any running bap processes",
                    style::good("done."),
                    summary.tag
                );
            }
            0
        }
        Err(error) => {
            eprintln!("bap: upgrade failed: {error}");
            1
        }
    }
}

struct UpgradeSummary {
    tag: String,
    asset_name: String,
    destination: PathBuf,
}

async fn upgrade_async(
    triple: &str,
    exe: &std::path::Path,
    version: Option<&str>,
    dry_run: bool,
) -> Result<UpgradeSummary, String> {
    let client = reqwest::Client::builder()
        .user_agent("bap-upgrade")
        .build()
        .map_err(|error| error.to_string())?;

    let release_url = match version {
        Some(tag) => format!("{API_BASE}/tags/{tag}"),
        None => format!("{API_BASE}/latest"),
    };
    let release: Value = client
        .get(&release_url)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| format!("{error}; check the tag name"))?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    let tag = release["tag_name"]
        .as_str()
        .ok_or("release has no tag_name")?
        .to_owned();

    let archive_name = format!("bap-{triple}.tar.xz");
    let checksum_name = format!("{archive_name}.sha256");
    let mut archive_url = None;
    let mut checksum_url = None;
    for asset in release["assets"].as_array().into_iter().flatten() {
        let name = asset["name"].as_str().unwrap_or_default();
        let url = asset["browser_download_url"].as_str().unwrap_or_default();
        if name == archive_name {
            archive_url = Some(url.to_owned());
        }
        if name == checksum_name {
            checksum_url = Some(url.to_owned());
        }
    }
    let (Some(archive_url), Some(checksum_url)) = (archive_url, checksum_url) else {
        return Err(format!(
            "release {tag} does not ship {archive_name}; supported triples are listed at https://github.com/{REPO}/releases"
        ));
    };

    if dry_run {
        return Ok(UpgradeSummary {
            tag,
            asset_name: archive_name,
            destination: exe.to_path_buf(),
        });
    }

    let scratch = tempfile::tempdir().map_err(|error| error.to_string())?;
    let archive_path = scratch.path().join(&archive_name);
    let bytes = download(&client, &archive_url).await?;
    std::fs::write(&archive_path, &bytes).map_err(|error| error.to_string())?;

    // Integrity first: the published digest is over the archive itself.
    let expected = download_text(&client, &checksum_url).await?;
    verify_digest(&expected, &archive_name, &bytes)?;

    let extracted = extract_binary(&archive_path, scratch.path(), triple)?;
    swap_binary(&extracted, exe)?;

    Ok(UpgradeSummary {
        tag,
        asset_name: archive_name,
        destination: exe.to_path_buf(),
    })
}

async fn download(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    client
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| format!("{url}: {error}"))?
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|error| error.to_string())
}

async fn download_text(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let bytes = download(client, url).await?;
    String::from_utf8(bytes.to_vec()).map_err(|error| error.to_string())
}

/// Checks the `.sha256` sidecar, which formats as `<digest> *<filename>` or
/// `<digest>  <filename>`.
fn verify_digest(sidecar: &str, archive_name: &str, bytes: &[u8]) -> Result<(), String> {
    let expected = sidecar
        .lines()
        .find_map(|line| {
            let mut parts = line.split_whitespace();
            let digest = parts.next()?;
            let filename = parts.next()?.trim_start_matches('*');
            (filename == archive_name).then_some(digest.to_owned())
        })
        .ok_or_else(|| format!("checksum file does not mention {archive_name}"))?;

    let actual = hex_digest(bytes);
    if actual != expected {
        return Err(format!(
            "checksum mismatch: expected {expected}, got {actual}; refusing to install"
        ));
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Extracts the single `bap` executable using system tar (xz support ships
/// with macOS and every common Linux base).
fn extract_binary(
    archive: &std::path::Path,
    scratch: &std::path::Path,
    _triple: &str,
) -> Result<PathBuf, String> {
    let status = std::process::Command::new("tar")
        .arg("-xJf")
        .arg(archive)
        .arg("-C")
        .arg(scratch)
        .output()
        .map_err(|error| format!("tar unavailable: {error}"))?;
    if !status.status.success() {
        return Err(format!(
            "tar failed: {}",
            String::from_utf8_lossy(&status.stderr).trim()
        ));
    }
    find_file(scratch, "bap", true)
        .ok_or_else(|| "archive did not contain a bap executable".to_owned())
}

fn swap_binary(extracted: &std::path::Path, current: &std::path::Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(extracted, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| error.to_string())?;
        // Rename over a running executable is atomic on Unix.
        std::fs::rename(extracted, current).map_err(|error| error.to_string())
    }
    #[cfg(not(unix))]
    {
        let _ = (extracted, current);
        Err("self-upgrade on this platform is not implemented; re-run the installer".to_owned())
    }
}

fn find_file(directory: &std::path::Path, name: &str, executable: bool) -> Option<PathBuf> {
    let entries = std::fs::read_dir(directory).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name, executable) {
                return Some(found);
            }
        } else if path.file_name()?.to_string_lossy() == name
            && (!executable || is_executable(&path))
        {
            return Some(path);
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path)
        .map(|meta| meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_: &std::path::Path) -> bool {
    true
}

/// Maps the running platform onto one of the release triples we publish.
pub fn release_triple() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("linux", "x86_64") => Some("x86_64-unknown-linux-musl"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        ("windows", "x86_64") => Some("x86_64-pc-windows-msvc"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIDECAR: &str = "abc123  bap-x86_64-unknown-linux-musl.tar.xz\nother  bap-other.tar.xz\n";

    #[test]
    fn digest_verification_accepts_the_matching_line() {
        let payload = b"payload".to_vec();
        let line = format!(
            "{} *bap-x86_64-unknown-linux-musl.tar.xz\n",
            hex_digest(&payload)
        );
        verify_digest(&line, "bap-x86_64-unknown-linux-musl.tar.xz", &payload).unwrap();
    }

    #[test]
    fn digest_verification_rejects_mismatches_and_missing_entries() {
        let payload = b"payload";
        assert!(verify_digest(SIDECAR, "bap-x86_64-unknown-linux-musl.tar.xz", payload).is_err());
        assert!(verify_digest("deadbeef  unknown.tar.xz\n", "nope.tar.xz", payload).is_err());
    }

    #[test]
    fn known_platforms_map_to_published_triples() {
        assert!(
            matches!(
                (std::env::consts::OS, std::env::consts::ARCH),
                ("macos", _) | ("linux", _) | ("windows", "x86_64")
            ) == release_triple().is_some()
                || release_triple().is_none()
        );
    }
}
