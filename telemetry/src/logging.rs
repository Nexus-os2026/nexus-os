//! Structured logging initialization with JSON or pretty output.
//!
//! Configures a `tracing-subscriber` with environment-based filtering
//! and format selection (JSON for server mode, pretty for desktop mode).

use crate::config::{LogFormat, TelemetryConfig};

/// Initialize the global tracing subscriber based on the telemetry config.
///
/// This should be called once at application startup. If called more than once,
/// subsequent calls are no-ops (the global subscriber is already set).
///
/// Returns `true` if the subscriber was successfully installed, `false` if
/// a subscriber was already installed.
pub fn init_logging(config: &TelemetryConfig) -> bool {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.log_level));

    let result = match config.log_format {
        LogFormat::Json => tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .json()
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .try_init(),
        LogFormat::Pretty => tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_thread_ids(false)
            .try_init(),
    };

    result.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_logging_does_not_panic() {
        // Just verify the function doesn't panic, even if a subscriber
        // is already installed (it returns false in that case).
        let config = TelemetryConfig::desktop();
        let _ = init_logging(&config);
    }
}
