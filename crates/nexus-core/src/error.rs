use thiserror::Error;

#[derive(Debug, Error)]
pub enum NexusCoreError {
    #[error("Capability denied: agent '{agent_id}' requested '{capability}' which is not in its manifest")]
    CapabilityDenied {
        agent_id: String,
        capability: String,
    },

    #[error("Fuel exhausted: agent '{agent_id}' exceeded {limit_type} limit of {limit_value}")]
    FuelExhausted {
        agent_id: String,
        limit_type: String,
        limit_value: u64,
    },

    #[error("Manifest parse error in '{path}': {source}")]
    ManifestParseError {
        path: String,
        #[source]
        source: toml::de::Error,
    },

    #[error("I/O proxy rejected request: {reason}")]
    IoProxyRejected { reason: String },

    #[error("I/O error: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
    },
}
