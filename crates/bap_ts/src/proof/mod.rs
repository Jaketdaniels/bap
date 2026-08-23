//! Proof-carrying numerics: trusted TypeScript evidence, independent Rust validation,
//! width-preserving specialization, and the bounded Node backend.

pub mod backend;
pub mod manifest;
pub mod specialize;

/// Wire protocol understood by this build.
pub const PROTOCOL_VERSION: u32 = 1;
/// Exact TypeScript compiler version trusted to emit manifests.
pub const TYPESCRIPT_VERSION: &str = "7.0.2";
/// Ambient declaration bundled with the proof tool.
pub const AMBIENT_SPECIFIER: &str = "bap:units.d.ts";
/// Same bytes the proof tool checks against, kept in sync by CI.
pub const AMBIENT_SOURCE: &str = include_str!("../../../../tools/ts-proof/bap-units.d.ts");
/// Maximum evidence-tree depth, mirroring the producer limit.
pub const MAX_EVIDENCE_DEPTH: usize = 64;
/// Maximum evidence nodes per expression, mirroring the producer limit.
pub const MAX_EVIDENCE_NODES: usize = 256;

/// Source marker that qualifies a module for the proof pipeline.
const CANDIDATE_MARKER: &str = "Bap.numeric.";

/// How eagerly `bap run` consults the proof backend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProofMode {
    /// Start the backend only for proof candidates on cache misses.
    #[default]
    Normal,
    /// Never start the backend; modules run unspecialized.
    Disabled,
    /// Recognized candidates must prove or the run fails.
    Required,
}

/// Reports whether a source mentions the trusted helper vocabulary.
pub fn is_proof_candidate(source: &str) -> bool {
    source.contains(CANDIDATE_MARKER)
}

/// Canonicalizes a filesystem path into the file URL form manifests use.
pub fn file_url_specifier(path: &std::path::Path) -> Option<String> {
    let absolute = std::fs::canonicalize(path).ok()?;
    deno_core::url::Url::from_file_path(absolute)
        .ok()
        .map(|url| url.to_string())
}
