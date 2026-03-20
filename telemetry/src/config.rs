//! Telemetry configuration.

use serde::{Deserialize, Serialize};

/// Log output format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogFormat {
    /// JSON structured logs (server mode).
    Json,
    /// Human-readable pretty-printed logs (desktop mode).
    Pretty,
}

/// Telemetry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// Enable or disable telemetry collection globally.
    pub enabled: bool,
    /// OTLP gRPC endpoint for trace/metric export (e.g., `http://otel-collector:4317`).
    pub otlp_endpoint: String,
    /// Service name reported to backends.
    pub service_name: String,
    /// Trace sampling rate (0.0 = none, 1.0 = all).
    pub sample_rate: f64,
    /// Prometheus metrics export/render interval in seconds.
    pub metrics_export_interval_secs: u64,
    /// Log output format.
    pub log_format: LogFormat,
    /// Minimum log level (e.g., "info", "debug", "warn").
    pub log_level: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            otlp_endpoint: "http://localhost:4317".to_string(),
            service_name: "nexus-os".to_string(),
            sample_rate: 1.0,
            metrics_export_interval_secs: 15,
            log_format: LogFormat::Pretty,
            log_level: "info".to_string(),
        }
    }
}

impl TelemetryConfig {
    /// Create a server-mode config (JSON logs, full sampling).
    pub fn server() -> Self {
        Self {
            log_format: LogFormat::Json,
            ..Default::default()
        }
    }

    /// Create a desktop-mode config (pretty logs, reduced sampling).
    pub fn desktop() -> Self {
        Self {
            sample_rate: 0.1,
            log_format: LogFormat::Pretty,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = TelemetryConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.service_name, "nexus-os");
        assert!((cfg.sample_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn server_config_uses_json() {
        let cfg = TelemetryConfig::server();
        assert_eq!(cfg.log_format, LogFormat::Json);
    }

    #[test]
    fn desktop_config_uses_pretty() {
        let cfg = TelemetryConfig::desktop();
        assert_eq!(cfg.log_format, LogFormat::Pretty);
        assert!(cfg.sample_rate < 1.0);
    }

    #[test]
    fn serde_roundtrip() {
        let cfg = TelemetryConfig::server();
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: TelemetryConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.log_format, LogFormat::Json);
        assert_eq!(parsed.service_name, "nexus-os");
    }
}
