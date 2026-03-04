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
    pub schedule: Option<String>,
    pub llm_model: Option<String>,
    pub autonomy_level: Option<u8>,
    pub fuel_period_id: Option<String>,
    pub monthly_fuel_cap: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    name: Option<String>,
    version: Option<String>,
    capabilities: Option<Vec<String>>,
    fuel_budget: Option<u64>,
    schedule: Option<String>,
    llm_model: Option<String>,
    autonomy_level: Option<u8>,
    fuel_period_id: Option<String>,
    monthly_fuel_cap: Option<u64>,
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
    validate_autonomy_level(raw.autonomy_level)?;
    validate_fuel_period_id(raw.fuel_period_id.as_deref())?;
    validate_monthly_fuel_cap(raw.monthly_fuel_cap)?;

    Ok(AgentManifest {
        name,
        version,
        capabilities,
        fuel_budget,
        schedule: raw.schedule,
        llm_model: raw.llm_model,
        autonomy_level: raw.autonomy_level,
        fuel_period_id: raw.fuel_period_id,
        monthly_fuel_cap: raw.monthly_fuel_cap,
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

fn validate_autonomy_level(level: Option<u8>) -> Result<(), AgentError> {
    match level {
        Some(value) if value > 5 => Err(AgentError::ManifestError(
            "autonomy_level must be in range 0..=5".to_string(),
        )),
        _ => Ok(()),
    }
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
            schedule: Some("*/10 * * * *".to_string()),
            llm_model: Some("claude-sonnet-4-5".to_string()),
            autonomy_level: None,
            fuel_period_id: None,
            monthly_fuel_cap: None,
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
