//! Provider health snapshot. Surfaced by `Provider::health_check` and
//! consumed by `nexus-swarm::events::SwarmEvent::ProviderHealthUpdate`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderHealthStatus {
    Ok,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderHealth {
    pub provider_id: String,
    pub status: ProviderHealthStatus,
    /// Observed latency in milliseconds; `None` when the probe failed
    /// before a response was received.
    pub latency_ms: Option<u64>,
    pub models: Vec<String>,
    /// Free-form notes (e.g. `"api_key not in keyring"`,
    /// `"spend: $0.42 / $2.00"`).
    pub notes: String,
    /// Unix timestamp (seconds) when the probe completed.
    pub checked_at_secs: i64,
}
