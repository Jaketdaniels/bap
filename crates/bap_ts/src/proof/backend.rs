//! Bounded Node backend that drives `tools/ts-proof/proof.mjs`.

use crate::proof::manifest::ProofManifest;
use deno_core::anyhow::{Context, Result, anyhow, bail};
use serde_json::json;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const PROOF_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const MAX_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;

/// A discovered proof backend; discovery never starts a process.
pub struct ProofBackend {
    node_program: PathBuf,
    tool_script: PathBuf,
}

impl ProofBackend {
    /// Locates the tool via `BAP_TS_PROOF_TOOL`, else `tools/ts-proof/proof.mjs`
    /// under the current directory. `BAP_TS_PROOF_NODE` overrides the program.
    pub fn discover() -> Option<Self> {
        let node_program = std::env::var_os("BAP_TS_PROOF_NODE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("node"));
        let tool_script = std::env::var_os("BAP_TS_PROOF_TOOL")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|directory| directory.join("tools/ts-proof/proof.mjs"))
            })
            .filter(|script| script.is_file())?;
        Some(Self {
            node_program,
            tool_script,
        })
    }

    /// Runs one prove request and parses the strict manifest response.
    pub fn prove(&self, source: &str, specifier: &str) -> Result<ProofManifest> {
        let workspace = tempfile::tempdir().context("failed to create proof request directory")?;
        let request_path = workspace.path().join("request.json");
        let response_path = workspace.path().join("response.json");
        std::fs::write(
            &request_path,
            serde_json::to_vec(&json!({ "source": source, "specifier": specifier }))?,
        )
        .context("failed to write proof request")?;

        let mut child = Command::new(&self.node_program)
            .arg(&self.tool_script)
            .arg(&request_path)
            .arg(&response_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| {
                format!(
                    "failed to start {} against {}",
                    self.node_program.display(),
                    self.tool_script.display()
                )
            })?;

        let status = self.wait_bounded(&mut child)?;
        if !status.success() {
            bail!("proof tool exited with {status}");
        }
        let metadata =
            std::fs::metadata(&response_path).context("proof tool wrote no response file")?;
        if metadata.len() > MAX_RESPONSE_BYTES {
            bail!("proof response exceeds 16 MiB");
        }
        let response =
            std::fs::read_to_string(&response_path).context("failed to read proof response")?;
        serde_json::from_str(&response).map_err(|error| anyhow!("invalid proof response: {error}"))
    }

    fn wait_bounded(&self, child: &mut std::process::Child) -> Result<std::process::ExitStatus> {
        let deadline = Instant::now() + PROOF_TIMEOUT;
        loop {
            match child.try_wait()? {
                Some(status) => return Ok(status),
                None if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    bail!("proof tool timed out after {}s", PROOF_TIMEOUT.as_secs());
                }
                None => std::thread::sleep(POLL_INTERVAL),
            }
        }
    }
}
