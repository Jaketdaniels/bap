//! End-to-end package manager tests against a local fixture registry.
//!
//! The fixture serves packument JSON and a real .tgz over raw HTTP, then
//! `bap add`/`bap install`/`bap remove` run against it via `BAP_REGISTRY`.

use std::io::Write as _;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_bap");

use sha2::Digest as _;

struct Sandbox {
    root: tempfile::TempDir,
    registry_port: u16,
}

impl Sandbox {
    fn write(&self, name: &str, contents: &str) {
        std::fs::write(self.root.path().join(name), contents).expect("write fixture");
    }
}

fn tgz_bytes(files: &[(&str, &str)]) -> Vec<u8> {
    use flate2::write::GzEncoder;
    let inner = std::io::Cursor::new(Vec::new());
    let mut tar = tar::Builder::new(inner);
    for (name, contents) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, format!("package/{name}"), contents.as_bytes())
            .expect("tar entry");
    }
    let tar_buffer = tar.into_inner().expect("tar").into_inner();
    let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&tar_buffer).expect("gzip");
    encoder.finish().expect("gzip")
}

fn sri(bytes: &[u8]) -> String {
    let digest = sha2::Sha512::digest(bytes);
    let b64 = {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in digest.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            out.push(TABLE[(b[0] >> 2) as usize] as char);
            out.push(TABLE[(((b[0] & 0x03) << 4) | (b[1] >> 4)) as usize] as char);
            out.push(if chunk.len() > 1 {
                TABLE[((b[1] & 0x0f) << 2 | (b[2] >> 6)) as usize] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                TABLE[(b[2] & 0x3f) as usize] as char
            } else {
                '='
            });
        }
        out
    };
    format!("sha512-{b64}")
}

impl Sandbox {
    /// Serves one unscoped package with two versions on an ephemeral port.
    fn new(name: &str) -> Self {
        let v1 = tgz_bytes(&[
            ("index.js", "module.exports = { version: () => '1.0.0' };"),
            ("package.json", "{}"),
        ]);
        let v2 = tgz_bytes(&[
            ("index.js", "module.exports = { version: () => '1.1.0' };"),
            ("package.json", "{}"),
        ]);
        let (sri1, sri2) = (sri(&v1), sri(&v2));
        let port = 0;

        let listener = std::net::TcpListener::bind(("127.0.0.1", port)).expect("bind");
        let bound = listener.local_addr().unwrap().port();

        let packument = serde_json::json!({
            "name": name,
            "versions": {
                "1.0.0": {
                    "version": "1.0.0",
                    "dist": { "tarball": format!("http://127.0.0.1:{bound}/{name}-1.0.0.tgz"), "integrity": sri1 },
                    "dependencies": {}
                },
                "1.1.0": {
                    "version": "1.1.0",
                    "dist": { "tarball": format!("http://127.0.0.1:{bound}/{name}-1.1.0.tgz"), "integrity": sri2 },
                    "dependencies": {}
                }
            }
        })
        .to_string();

        let routes: Vec<(String, Vec<u8>, &'static str)> = vec![
            (
                format!("/{name}"),
                packument.into_bytes(),
                "application/json",
            ),
            (format!("/{name}-1.0.0.tgz"), v1, "application/octet-stream"),
            (format!("/{name}-1.1.0.tgz"), v2, "application/octet-stream"),
        ];

        // Serve every connection by exact request-path lookup.
        let routes = std::sync::Arc::new(routes);
        let listener = std::sync::Arc::new(listener);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut socket) = stream else { return };
                let routes = std::sync::Arc::clone(&routes);
                std::thread::spawn(move || {
                    let mut buffer = [0_u8; 4096];
                    let read = std::io::Read::read(&mut socket, &mut buffer).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
                    let requested_path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .unwrap_or_default()
                        .to_owned();
                    let hit = routes.iter().find(|(path, _, _)| *path == requested_path);
                    match hit {
                        Some((_, body, content_type)) => {
                            let head = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            );
                            let _ = socket.write_all(head.as_bytes());
                            let _ = socket.write_all(body);
                        }
                        None => {
                            let _ = socket.write_all(
                                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            );
                        }
                    }
                    let _ = socket.flush();
                });
            }
        });

        Self {
            root: tempfile::Builder::new()
                .prefix("bap-pm-")
                .tempdir()
                .expect("sandbox"),
            registry_port: bound,
        }
    }

    fn command(&self) -> Command {
        let cache = tempfile::tempdir().expect("cache dir");
        let mut command = Command::new(BIN);
        command.current_dir(self.root.path());
        command.env("BAP_TS_CACHE_DIR", cache.path());
        command.env(
            "BAP_REGISTRY",
            format!("http://127.0.0.1:{}", self.registry_port),
        );
        command
    }
}

#[test]
fn add_installs_and_require_resolves() {
    let sandbox = Sandbox::new("tiny");

    let added = sandbox.command().args(["add", "tiny"]).output().unwrap();
    assert!(
        added.status.success(),
        "add stderr: {}",
        String::from_utf8_lossy(&added.stderr)
    );

    let manifest = std::fs::read_to_string(sandbox.root.path().join("package.json")).unwrap();
    assert!(manifest.contains("\"tiny\""), "{manifest}");
    let lock = std::fs::read_to_string(sandbox.root.path().join("bap-lock.json")).unwrap();
    assert!(lock.contains("1.1.0"), "{lock}");
    assert!(
        sandbox
            .root
            .path()
            .join("node_modules/tiny/index.js")
            .is_file()
    );

    sandbox.write(
        "app.cjs",
        "const tiny = require('tiny');\nif (tiny.version() !== '1.1.0') throw new Error('wrong version');\nconsole.log('required ok');\n",
    );
    let run = sandbox.command().args(["run", "app.cjs"]).output().unwrap();
    assert!(
        run.status.success(),
        "run stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(String::from_utf8_lossy(&run.stdout).contains("required ok"));
}

#[test]
fn install_without_specs_uses_the_manifest_range() {
    let sandbox = Sandbox::new("tiny");
    sandbox.write(
        "package.json",
        r#"{ "name": "probe", "version": "0.0.0", "dependencies": { "tiny": "^1.0.0" } }"#,
    );

    let installed = sandbox.command().arg("install").output().unwrap();
    assert!(
        installed.status.success(),
        "{}",
        String::from_utf8_lossy(&installed.stderr)
    );
    assert!(
        sandbox
            .root
            .path()
            .join("node_modules/tiny/index.js")
            .is_file()
    );
}

#[test]
fn remove_drops_manifest_lock_and_files() {
    let sandbox = Sandbox::new("tiny");
    sandbox.command().args(["add", "tiny"]).output().unwrap();

    let removed = sandbox.command().args(["remove", "tiny"]).output().unwrap();
    assert!(removed.status.success());
    assert!(!sandbox.root.path().join("node_modules/tiny").exists());
    let lock = std::fs::read_to_string(sandbox.root.path().join("bap-lock.json")).unwrap();
    assert!(!lock.contains("\"tiny\""));
}

#[test]
fn unknown_packages_fail_with_guidance() {
    let sandbox = Sandbox::new("unknown");
    let out = sandbox
        .command()
        .args(["add", "definitely-not-here"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("was not found"), "{stderr}");
}
