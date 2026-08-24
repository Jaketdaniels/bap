//! Extracts npm tarballs into `node_modules` safely.

use anyhow::{Context, Result, anyhow};
use flate2::read::GzDecoder;
use std::io::Read;
use std::path::Path;
use tar::Archive;

/// Extracts a `.tgz` into `destination`, stripping the leading `package/`
/// component every npm tarball uses.
///
/// Safety rules: absolute paths, `..` segments, and symlinks are rejected —
/// a malicious tarball must not write outside its package directory.
pub fn extract_package(tgz: &[u8], destination: &Path) -> Result<usize> {
    let decoder = GzDecoder::new(tgz);
    let mut archive = Archive::new(decoder);
    std::fs::create_dir_all(destination)
        .with_context(|| format!("create {}", destination.display()))?;

    let mut files = 0usize;
    for entry in archive.entries().context("read tarball entries")? {
        let mut entry = entry.context("tarball entry")?;
        let raw_path = entry
            .path()
            .context("tarball entry path")?
            .to_string_lossy()
            .into_owned();

        let Some(relative) = raw_path.strip_prefix("package/") else {
            // npm tarballs contain exactly one root: package/. Anything else
            // is malformed or hostile; refuse rather than skip silently.
            return Err(anyhow!("tarball entry outside package root: {raw_path}"));
        };
        if relative.is_empty() {
            continue;
        }
        ensure_safe(relative)?;

        let target = destination.join(relative);
        let header_type = entry.header().entry_type();
        match header_type {
            tar::EntryType::Directory => {
                std::fs::create_dir_all(&target)
                    .with_context(|| format!("create {}", target.display()))?;
            }
            tar::EntryType::Regular => {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("create {}", parent.display()))?;
                }
                let mut contents = Vec::new();
                entry
                    .read_to_end(&mut contents)
                    .with_context(|| format!("read {raw_path}"))?;
                std::fs::write(&target, contents)
                    .with_context(|| format!("write {}", target.display()))?;
                files += 1;
            }
            _ => return Err(anyhow!("tarball contains non-file entry {raw_path}")),
        }
    }
    Ok(files)
}

fn ensure_safe(relative: &str) -> Result<()> {
    if relative.starts_with('/') {
        return Err(anyhow!("tarball entry uses an absolute path: {relative}"));
    }
    for segment in Path::new(relative).components() {
        if matches!(segment, std::path::Component::ParentDir) {
            return Err(anyhow!("tarball entry escapes its root: {relative}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_tgz(files: &[(&str, &str)]) -> Vec<u8> {
        use tar::Builder;
        let buffer = std::io::Cursor::new(Vec::new());
        let mut builder = Builder::new(buffer);
        for (name, contents) in files {
            let path = format!("package/{name}");
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, path, contents.as_bytes())
                .unwrap();
        }
        builder.into_inner().expect("tar").into_inner()
    }

    fn gzip(bytes: Vec<u8>) -> Vec<u8> {
        use flate2::write::GzEncoder;
        use std::io::Write as _;
        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn extracts_stripping_the_package_prefix() {
        let tgz = gzip(build_tgz(&[
            ("index.js", "module.exports = 1;"),
            ("lib/util.js", "x"),
        ]));
        let dir = tempfile::tempdir().unwrap();
        let count = extract_package(&tgz, dir.path()).unwrap();
        assert_eq!(count, 2);
        assert!(dir.path().join("index.js").is_file());
        assert!(dir.path().join("lib/util.js").is_file());
    }

    #[test]
    fn rejects_entries_that_escape_the_root() {
        // Hand-rolled header: the tar crate refuses to *write* `..` names.
        let mut header = [0_u8; 512];
        header[..10].copy_from_slice(b"../evil.js");
        header[100..108].copy_from_slice(b"0000644\0");
        header[108..116].copy_from_slice(b"0000000\0");
        header[116..124].copy_from_slice(b"0000000\0");
        header[124..136].copy_from_slice(b"00000000004\0"); // 4 bytes
        header[136..148].copy_from_slice(b"00000000000\0");
        header[156] = b'0'; // regular file
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");
        let checksum: u32 = header.iter().map(|byte| u32::from(*byte)).sum();
        header[148..156].copy_from_slice(format!("{checksum:06}\0 ").as_bytes());

        let mut raw = Vec::new();
        raw.extend_from_slice(&header);
        raw.extend_from_slice(b"nope\0\0\0\0");
        raw.extend(std::iter::repeat_n(0_u8, 512 - 8));
        raw.extend_from_slice(&[0_u8; 1024]); // two zero blocks end the archive

        let dir = tempfile::tempdir().unwrap();
        let result = extract_package(&gzip(raw), dir.path());
        assert!(result.is_err(), "escape entry must be rejected");
        assert!(dir.path().read_dir().unwrap().next().is_none());
    }
}
