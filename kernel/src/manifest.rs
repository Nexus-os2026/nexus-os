use crate::autonomy::AutonomyLevel;
use crate::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const MIN_NAME_LEN: usize = 3;
const MAX_NAME_LEN: usize = 64;
const MAX_FUEL_BUDGET: u64 = 1_000_000;
const CAPABILITY_REGISTRY: [&str; 11] = [
    "web.search",
    "web.read",
    "llm.query",
    "fs.read",
    "fs.write",
    "process.exec",
    "social.post",
    "social.x.post",
    "social.x.read",
    "messaging.send",
    "audit.read",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentManifest {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub fuel_budget: u64,
    pub autonomy_level: Option<u8>,
    pub consent_policy_path: Option<String>,
    pub requester_id: Option<String>,
    pub schedule: Option<String>,
    pub llm_model: Option<String>,
    pub fuel_period_id: Option<String>,
    pub monthly_fuel_cap: Option<u64>,
    /// URL prefixes this agent is allowed to call. `None` = default deny all.
    #[serde(default)]
    pub allowed_endpoints: Option<Vec<String>>,
    /// Domain tags for EU AI Act risk classification (e.g. "biometric", "critical-infrastructure").
    #[serde(default)]
    pub domain_tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    name: Option<String>,
    version: Option<String>,
    capabilities: Option<Vec<String>>,
    fuel_budget: Option<u64>,
    autonomy_level: Option<u8>,
    consent_policy_path: Option<String>,
    requester_id: Option<String>,
    schedule: Option<String>,
    llm_model: Option<String>,
    fuel_period_id: Option<String>,
    monthly_fuel_cap: Option<u64>,
    allowed_endpoints: Option<Vec<String>>,
    domain_tags: Option<Vec<String>>,
}

pub fn parse_manifest(input: &str) -> Result<AgentManifest, AgentError> {
    let raw: RawManifest =
        toml::from_str(input).map_err(|e| AgentError::ManifestError(e.to_string()))?;

    let name = raw
        .name
        .ok_or_else(|| AgentError::ManifestError("missing required field: name".to_string()))?;
    validate_name(&name)?;

    let version = raw
        .version
        .ok_or_else(|| AgentError::ManifestError("missing required field: version".to_string()))?;
    if version.trim().is_empty() {
        return Err(AgentError::ManifestError(
            "version cannot be empty".to_string(),
        ));
    }

    let capabilities = raw.capabilities.ok_or_else(|| {
        AgentError::ManifestError("missing required field: capabilities".to_string())
    })?;
    validate_capabilities(&capabilities)?;

    let fuel_budget = raw.fuel_budget.ok_or_else(|| {
        AgentError::ManifestError("missing required field: fuel_budget".to_string())
    })?;
    validate_fuel_budget(fuel_budget)?;

    let autonomy_level = parse_autonomy_level(raw.autonomy_level)?;
    let consent_policy_path = parse_optional_non_empty(raw.consent_policy_path);
    let requester_id = parse_optional_non_empty(raw.requester_id);
    validate_fuel_period_id(raw.fuel_period_id.as_deref())?;
    validate_monthly_fuel_cap(raw.monthly_fuel_cap)?;

    Ok(AgentManifest {
        name,
        version,
        capabilities,
        fuel_budget,
        autonomy_level,
        consent_policy_path,
        requester_id,
        schedule: raw.schedule,
        llm_model: raw.llm_model,
        fuel_period_id: raw.fuel_period_id,
        monthly_fuel_cap: raw.monthly_fuel_cap,
        allowed_endpoints: raw.allowed_endpoints,
        domain_tags: raw.domain_tags.unwrap_or_default(),
    })
}

fn validate_name(name: &str) -> Result<(), AgentError> {
    let len = name.chars().count();
    if !(MIN_NAME_LEN..=MAX_NAME_LEN).contains(&len) {
        return Err(AgentError::ManifestError(format!(
            "name must be {}-{} characters",
            MIN_NAME_LEN, MAX_NAME_LEN
        )));
    }

    let valid = name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-');
    if !valid {
        return Err(AgentError::ManifestError(
            "name must be alphanumeric plus hyphens only".to_string(),
        ));
    }

    Ok(())
}

fn validate_capabilities(capabilities: &[String]) -> Result<(), AgentError> {
    if capabilities.is_empty() {
        return Err(AgentError::ManifestError(
            "capabilities cannot be empty".to_string(),
        ));
    }

    let known: BTreeSet<&str> = CAPABILITY_REGISTRY.iter().copied().collect();
    for capability in capabilities {
        if !known.contains(capability.as_str()) {
            return Err(AgentError::CapabilityDenied(capability.clone()));
        }
    }

    Ok(())
}

fn validate_fuel_budget(fuel_budget: u64) -> Result<(), AgentError> {
    if fuel_budget == 0 {
        return Err(AgentError::ManifestError(
            "fuel_budget must be greater than 0".to_string(),
        ));
    }
    if fuel_budget > MAX_FUEL_BUDGET {
        return Err(AgentError::ManifestError(format!(
            "fuel_budget must be <= {}",
            MAX_FUEL_BUDGET
        )));
    }
    Ok(())
}

fn parse_autonomy_level(value: Option<u8>) -> Result<Option<u8>, AgentError> {
    let Some(value) = value else {
        return Ok(None);
    };

    let _ = AutonomyLevel::from_numeric(value).ok_or_else(|| {
        AgentError::ManifestError("autonomy_level must be one of 0, 1, 2, 3, 4, 5".to_string())
    })?;
    Ok(Some(value))
}

fn parse_optional_non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn validate_fuel_period_id(period_id: Option<&str>) -> Result<(), AgentError> {
    if let Some(period_id) = period_id {
        if period_id.trim().is_empty() {
            return Err(AgentError::ManifestError(
                "fuel_period_id cannot be empty".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_monthly_fuel_cap(monthly_fuel_cap: Option<u64>) -> Result<(), AgentError> {
    if monthly_fuel_cap == Some(0) {
        return Err(AgentError::ManifestError(
            "monthly_fuel_cap must be greater than 0".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_manifest, AgentManifest};
    use crate::errors::AgentError;

    #[test]
    fn test_parse_valid_manifest() {
        let toml = r#"
name = "my-social-poster"
version = "0.1.0"
capabilities = ["web.search", "llm.query", "fs.read"]
fuel_budget = 10000
schedule = "*/10 * * * *"
llm_model = "claude-sonnet-4-5"
"#;

        let parsed = parse_manifest(toml);
        assert!(parsed.is_ok());

        let manifest = parsed.expect("valid manifest should parse");
        let expected = AgentManifest {
            name: "my-social-poster".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![
                "web.search".to_string(),
                "llm.query".to_string(),
                "fs.read".to_string(),
            ],
            fuel_budget: 10_000,
            autonomy_level: None,
            consent_policy_path: None,
            requester_id: None,
            schedule: Some("*/10 * * * *".to_string()),
            llm_model: Some("claude-sonnet-4-5".to_string()),
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
        };
        assert_eq!(manifest, expected);
    }

    #[test]
    fn test_reject_invalid_manifest() {
        let missing_name = r#"
version = "0.1.0"
capabilities = ["web.search"]
fuel_budget = 100
"#;
        let empty_capabilities = r#"
name = "valid-name"
version = "0.1.0"
capabilities = []
fuel_budget = 100
"#;
        let zero_fuel = r#"
name = "valid-name"
version = "0.1.0"
capabilities = ["web.search"]
fuel_budget = 0
"#;

        let missing_name_error = parse_manifest(missing_name);
        assert!(matches!(
            missing_name_error,
            Err(AgentError::ManifestError(_))
        ));

        let empty_capabilities_error = parse_manifest(empty_capabilities);
        assert!(matches!(
            empty_capabilities_error,
            Err(AgentError::ManifestError(_))
        ));

        let zero_fuel_error = parse_manifest(zero_fuel);
        assert!(matches!(zero_fuel_error, Err(AgentError::ManifestError(_))));
    }

    #[test]
    fn test_parse_autonomy_level() {
        let toml = r#"
name = "agent-with-autonomy"
version = "0.1.0"
capabilities = ["web.search"]
fuel_budget = 100
autonomy_level = 2
consent_policy_path = "/tmp/consent.toml"
requester_id = "agent.alpha"
"#;

        let parsed = parse_manifest(toml).expect("manifest with autonomy level should parse");
        assert_eq!(parsed.autonomy_level, Some(2));
        assert_eq!(
            parsed.consent_policy_path,
            Some("/tmp/consent.toml".to_string())
        );
        assert_eq!(parsed.requester_id, Some("agent.alpha".to_string()));
    }

    #[test]
    fn test_reject_invalid_autonomy_level() {
        let invalid_autonomy = r#"
name = "valid-name"
version = "0.1.0"
capabilities = ["web.search"]
fuel_budget = 100
autonomy_level = 9
"#;

        let parsed = parse_manifest(invalid_autonomy);
        assert!(matches!(parsed, Err(AgentError::ManifestError(_))));
    }
}
