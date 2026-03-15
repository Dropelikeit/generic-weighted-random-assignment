//! Logging and tracing setup.

use tracing_subscriber::{fmt, EnvFilter};

/// Initializes the tracing/logging subsystem.
///
/// Reads the `RUST_LOG` environment variable for filter configuration.
/// Defaults to `info` level if not set.
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt().with_env_filter(filter).with_target(true).init();
}
