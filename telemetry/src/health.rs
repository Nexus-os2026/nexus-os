//! Health and readiness check data structures.
//!
//! These types are used by the HTTP gateway to serve `/health` and `/ready`
//! endpoints in a standardized format.

use serde::{Deserialize, Serialize};

/// Overall system health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// All subsystems operational.
    Healthy,
    /// Some subsystems degraded but functional.
    Degraded,
    /// System is not operational.
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded => write!(f, "degraded"),
            Self::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

/// Readiness status of individual subsystems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemStatus {
    pub name: String,
    pub ready: bool,
    pub message: Option<String>,
}

/// Health check response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub version: String,
    pub uptime_seconds: f64,
    pub agents_active: u64,
    pub audit_chain_valid: bool,
}

/// Readiness check response (more detailed than health).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessResponse {
    pub ready: bool,
    pub subsystems: Vec<SubsystemStatus>,
}

impl ReadinessResponse {
    /// Build a readiness response from subsystem checks.
    pub fn from_checks(subsystems: Vec<SubsystemStatus>) -> Self {
        let ready = subsystems.iter().all(|s| s.ready);
        Self { ready, subsystems }
    }
}

/// Helper to create a subsystem status entry.
pub fn subsystem(name: &str, ready: bool, message: Option<&str>) -> SubsystemStatus {
    SubsystemStatus {
        name: name.to_string(),
        ready,
        message: message.map(|s| s.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_status_display() {
        assert_eq!(format!("{}", HealthStatus::Healthy), "healthy");
        assert_eq!(format!("{}", HealthStatus::Degraded), "degraded");
        assert_eq!(format!("{}", HealthStatus::Unhealthy), "unhealthy");
    }

    #[test]
    fn readiness_all_ready() {
        let resp = ReadinessResponse::from_checks(vec![
            subsystem("kernel", true, None),
            subsystem("audit", true, None),
            subsystem("auth", true, None),
        ]);
        assert!(resp.ready);
        assert_eq!(resp.subsystems.len(), 3);
    }

    #[test]
    fn readiness_one_not_ready() {
        let resp = ReadinessResponse::from_checks(vec![
            subsystem("kernel", true, None),
            subsystem("database", false, Some("connection refused")),
        ]);
        assert!(!resp.ready);
    }

    #[test]
    fn health_response_serde() {
        let health = HealthResponse {
            status: HealthStatus::Healthy,
            version: "9.0.0".to_string(),
            uptime_seconds: 3600.0,
            agents_active: 5,
            audit_chain_valid: true,
        };
        let json = serde_json::to_string(&health).unwrap();
        let parsed: HealthResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.status, HealthStatus::Healthy);
        assert!(parsed.audit_chain_valid);
    }

    #[test]
    fn readiness_response_serde() {
        let resp = ReadinessResponse::from_checks(vec![
            subsystem("kernel", true, None),
            subsystem("llm", false, Some("no API key configured")),
        ]);
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ReadinessResponse = serde_json::from_str(&json).unwrap();
        assert!(!parsed.ready);
        assert_eq!(
            parsed.subsystems[1].message.as_deref(),
            Some("no API key configured")
        );
    }
}
