//! TypeScript transpilation, artifact caching, and proof-carrying numerics support.

#![forbid(unsafe_code)]

mod cache;
mod loader;
pub mod proof;
mod transpile;

pub use cache::{Cache, CacheInput, CachedArtifact, ProofBuild};
pub use loader::TsModuleLoader;
pub use proof::backend::ProofBackend;
pub use proof::manifest::{ProofAssessment, ProofManifest, ProofRejection};
pub use proof::{ProofMode, is_proof_candidate};
pub use transpile::{TranspileOutput, transpile};
