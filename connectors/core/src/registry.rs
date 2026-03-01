use crate::connector::{Connector, ConnectorMetadata, HealthStatus};
use nexus_kernel::errors::AgentError;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default)]
pub struct ConnectorRegistry {
    connectors: HashMap<String, Arc<dyn Connector>>,
}

impl ConnectorRegistry {
    pub fn new() -> Self {
        Self {
            connectors: HashMap::new(),
        }
    }

    pub fn register(&mut self, connector: Arc<dyn Connector>) -> Result<(), AgentError> {
        let connector_id = connector.id().to_string();
        if self.connectors.contains_key(&connector_id) {
            return Err(AgentError::SupervisorError(format!(
                "connector '{connector_id}' is already registered"
            )));
        }

        self.connectors.insert(connector_id, connector);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn Connector>> {
        self.connectors.get(id).cloned()
    }

    pub fn health_check_all(&self) -> HashMap<String, HealthStatus> {
        let mut health = HashMap::new();

        for (id, connector) in &self.connectors {
            let status = match connector.health_check() {
                Ok(status) => status,
                Err(error) => HealthStatus::Unhealthy(error.to_string()),
            };
            health.insert(id.clone(), status);
        }

        health
    }

    pub fn list(&self) -> Vec<ConnectorMetadata> {
        let mut metadata = self
            .connectors
            .values()
            .map(|connector| ConnectorMetadata {
                id: connector.id().to_string(),
                name: connector.name().to_string(),
                required_capabilities: connector.required_capabilities(),
                retry_policy: connector.retry_policy(),
                degrade_gracefully: connector.degrade_gracefully(),
            })
            .collect::<Vec<_>>();

        metadata.sort_by(|left, right| left.id.cmp(&right.id));
        metadata
    }
}

#[cfg(test)]
mod tests {
    use super::ConnectorRegistry;
    use crate::connector::{Connector, HealthStatus, RetryPolicy};
    use nexus_kernel::errors::AgentError;
    use std::sync::Arc;

    struct MockHttpConnector;

    impl Connector for MockHttpConnector {
        fn id(&self) -> &str {
            "http"
        }

        fn name(&self) -> &str {
            "HTTP Connector"
        }

        fn required_capabilities(&self) -> Vec<String> {
            vec!["net.outbound".to_string()]
        }

        fn health_check(&self) -> Result<HealthStatus, AgentError> {
            Ok(HealthStatus::Healthy)
        }

        fn retry_policy(&self) -> RetryPolicy {
            RetryPolicy {
                max_retries: 3,
                backoff_ms: 100,
                backoff_multiplier: 2.0,
            }
        }

        fn degrade_gracefully(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_register_and_retrieve() {
        let mut registry = ConnectorRegistry::new();
        let connector = Arc::new(MockHttpConnector);

        let register_result = registry.register(connector);
        assert!(register_result.is_ok());

        let retrieved = registry.get("http");
        assert!(retrieved.is_some());
        if let Some(connector) = retrieved {
            assert_eq!(connector.id(), "http");
            assert_eq!(connector.name(), "HTTP Connector");
        }
    }

    #[test]
    fn test_health_check_all() {
        let mut registry = ConnectorRegistry::new();
        let register_result = registry.register(Arc::new(MockHttpConnector));
        assert!(register_result.is_ok());

        let status_map = registry.health_check_all();
        let status = status_map.get("http");
        assert!(status.is_some());
        assert_eq!(status, Some(&HealthStatus::Healthy));
    }
}
