use nexus_kernel::errors::AgentError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff_ms: u64,
    pub backoff_multiplier: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorMetadata {
    pub id: String,
    pub name: String,
    pub required_capabilities: Vec<String>,
    pub retry_policy: RetryPolicy,
    pub degrade_gracefully: bool,
}

pub trait Connector: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn required_capabilities(&self) -> Vec<String>;
    fn health_check(&self) -> Result<HealthStatus, AgentError>;
    fn retry_policy(&self) -> RetryPolicy;
    fn degrade_gracefully(&self) -> bool;
}
