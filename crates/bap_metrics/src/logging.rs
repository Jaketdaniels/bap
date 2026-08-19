//! Tracing subscriber setup for the bap CLI.

use tracing_subscriber::{EnvFilter, fmt};

/// Initializes the global tracing subscriber using `verbosity` as the fallback filter.
/// A `RUST_LOG` environment variable overrides the filter when present.
pub fn init(verbosity: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(verbosity));
    let _ = fmt().with_env_filter(filter).try_init();
}
