//! Fluent manifest builder for agent developers.

use nexus_kernel::errors::AgentError;
use nexus_kernel::manifest::AgentManifest;

pub struct ManifestBuilder {
    name: String,
    version: String,
    capabilities: Vec<String>,
    fuel_budget: u64,
    autonomy_level: Option<u8>,
}

impl ManifestBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            capabilities: Vec::new(),
            fuel_budget: 0,
            autonomy_level: None,
        }
    }

    pub fn version(mut self, version: &str) -> Self {
        self.version = version.to_string();
        self
    }

    pub fn capability(mut self, capability: &str) -> Self {
        self.capabilities.push(capability.to_string());
        self
    }

    pub fn fuel_budget(mut self, budget: u64) -> Self {
        self.fuel_budget = budget;
        self
    }

    pub fn autonomy_level(mut self, level: u8) -> Self {
        self.autonomy_level = Some(level);
        self
    }

    pub fn build(self) -> Result<AgentManifest, AgentError> {
        if self.name.is_empty() {
            return Err(AgentError::ManifestError(
                "name cannot be empty".to_string(),
            ));
        }

        if self.fuel_budget == 0 {
            return Err(AgentError::ManifestError(
                "fuel_budget must be greater than 0".to_string(),
            ));
        }

        if let Some(level) = self.autonomy_level {
            if level > 5 {
                return Err(AgentError::ManifestError(
                    "autonomy_level must be 0-5".to_string(),
                ));
            }
        }

        Ok(AgentManifest {
            name: self.name,
            version: self.version,
            capabilities: self.capabilities,
            fuel_budget: self.fuel_budget,
            autonomy_level: self.autonomy_level,
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            llm_model: None,
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_valid_manifest() {
        let manifest = ManifestBuilder::new("my-agent")
            .version("1.0.0")
            .capability("llm.query")
            .capability("fs.read")
            .fuel_budget(5000)
            .autonomy_level(2)
            .build();

        assert!(manifest.is_ok());
        let m = manifest.unwrap();
        assert_eq!(m.name, "my-agent");
        assert_eq!(m.version, "1.0.0");
        assert_eq!(m.capabilities.len(), 2);
        assert_eq!(m.fuel_budget, 5000);
        assert_eq!(m.autonomy_level, Some(2));
    }

    #[test]
    fn empty_name_rejected() {
        let result = ManifestBuilder::new("")
            .capability("llm.query")
            .fuel_budget(100)
            .build();
        assert!(matches!(result, Err(AgentError::ManifestError(_))));
    }

    #[test]
    fn zero_fuel_rejected() {
        let result = ManifestBuilder::new("my-agent")
            .capability("llm.query")
            .build();
        assert!(matches!(result, Err(AgentError::ManifestError(_))));
    }

    #[test]
    fn invalid_autonomy_level_rejected() {
        let result = ManifestBuilder::new("my-agent")
            .capability("llm.query")
            .fuel_budget(100)
            .autonomy_level(9)
            .build();
        assert!(matches!(result, Err(AgentError::ManifestError(_))));
    }

    #[test]
    fn default_version_applied() {
        let manifest = ManifestBuilder::new("my-agent")
            .capability("llm.query")
            .fuel_budget(100)
            .build()
            .unwrap();
        assert_eq!(manifest.version, "0.1.0");
    }
}
