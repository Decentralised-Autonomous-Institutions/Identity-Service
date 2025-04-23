use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    EnvFilter,
};

/// Initializes the tracing subscriber for logging.
///
/// Sets up a basic subscriber that logs to stdout.
/// The log level can be configured via the `RUST_LOG` environment variable.
/// Example: `RUST_LOG=identity_model=debug,warn`
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        // Fallback level if RUST_LOG is not set
        .unwrap_or_else(|_| EnvFilter::new("info"));

    fmt::Subscriber::builder()
        .with_env_filter(filter)
        // Include span events (useful for async contexts)
        .with_span_events(FmtSpan::CLOSE)
        // Use the default formatting
        .init();
}
