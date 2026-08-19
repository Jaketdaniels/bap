use tracing_subscriber::{EnvFilter, fmt};

pub fn init(verbosity: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(verbosity));
    let _ = fmt().with_env_filter(filter).try_init();
}
