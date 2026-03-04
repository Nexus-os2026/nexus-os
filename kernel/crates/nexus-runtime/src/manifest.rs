use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentManifest {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<Capability>,
    pub fuel_budget: u64,
    pub autonomy_level: Option<u8>,
    pub schedule: String,
    pub llm_model: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    FsRead,
    FsWrite,
    NetOutbound,
    LlmInvoke,
    AuditWrite,
}

impl Capability {
    pub const ALL: [Capability; 5] = [
        Capability::FsRead,
        Capability::FsWrite,
        Capability::NetOutbound,
        Capability::LlmInvoke,
        Capability::AuditWrite,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Capability::FsRead => "fs.read",
            Capability::FsWrite => "fs.write",
            Capability::NetOutbound => "net.outbound",
            Capability::LlmInvoke => "llm.invoke",
            Capability::AuditWrite => "audit.write",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "fs.read" => Some(Capability::FsRead),
            "fs.write" => Some(Capability::FsWrite),
            "net.outbound" => Some(Capability::NetOutbound),
            "llm.invoke" => Some(Capability::LlmInvoke),
            "audit.write" => Some(Capability::AuditWrite),
            _ => None,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("manifest TOML syntax error: {reason}")]
    TomlSyntax { reason: String },

    #[error("manifest missing required field '{field}'")]
    MissingField { field: &'static str },

    #[error("manifest parse error in field '{field}': {reason}")]
    InvalidField {
        field: &'static str,
        reason: String,
    },
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    name: Option<String>,
    version: Option<String>,
    #[serde(default)]
    capabilities: Vec<String>,
    fuel_budget: Option<u64>,
    autonomy_level: Option<u8>,
    schedule: Option<String>,
    llm_model: Option<String>,
}

pub fn parse_manifest(input: &str) -> Result<AgentManifest, ManifestError> {
    let raw: RawManifest = toml::from_str(input).map_err(|err| ManifestError::TomlSyntax {
        reason: err.to_string(),
    })?;

    AgentManifest::try_from(raw)
}

impl TryFrom<RawManifest> for AgentManifest {
    type Error = ManifestError;

    fn try_from(raw: RawManifest) -> Result<Self, Self::Error> {
        let name = require_non_empty(raw.name, "name")?;
        validate_agent_name(&name)?;

        let version = require_non_empty(raw.version, "version")?;
        validate_version(&version)?;

        let capabilities = validate_capabilities(raw.capabilities)?;

        let fuel_budget = match raw.fuel_budget {
            Some(0) => {
                return Err(ManifestError::InvalidField {
                    field: "fuel_budget",
                    reason: "must be greater than 0".to_string(),
                });
            }
            Some(value) => value,
            None => return Err(ManifestError::MissingField { field: "fuel_budget" }),
        };
        let autonomy_level = validate_autonomy_level(raw.autonomy_level)?;

        let schedule = require_non_empty(raw.schedule, "schedule")?;
        let llm_model = require_non_empty(raw.llm_model, "llm_model")?;

        Ok(AgentManifest {
            name,
            version,
            capabilities,
            fuel_budget,
            autonomy_level,
            schedule,
            llm_model,
        })
    }
}

fn require_non_empty(
    value: Option<String>,
    field: &'static str,
) -> Result<String, ManifestError> {
    let value = match value {
        Some(value) => value.trim().to_string(),
        None => return Err(ManifestError::MissingField { field }),
    };

    if value.is_empty() {
        return Err(ManifestError::InvalidField {
            field,
            reason: "cannot be empty".to_string(),
        });
    }

    Ok(value)
}

fn validate_agent_name(name: &str) -> Result<(), ManifestError> {
    let valid = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');
    if !valid {
        return Err(ManifestError::InvalidField {
            field: "name",
            reason: "must only contain ASCII letters, digits, '-', '_' or '.'".to_string(),
        });
    }

    Ok(())
}

fn validate_version(version: &str) -> Result<(), ManifestError> {
    let mut parts = version.split('.');
    let major = parts.next();
    let minor = parts.next();
    let patch = parts.next();
    let extra = parts.next();

    let valid = major.is_some_and(is_unsigned_number)
        && minor.is_some_and(is_unsigned_number)
        && patch.is_some_and(is_unsigned_number)
        && extra.is_none();

    if !valid {
        return Err(ManifestError::InvalidField {
            field: "version",
            reason: "must follow semantic version core format 'MAJOR.MINOR.PATCH'".to_string(),
        });
    }

    Ok(())
}

fn is_unsigned_number(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_digit())
}

fn validate_capabilities(values: Vec<String>) -> Result<Vec<Capability>, ManifestError> {
    if values.is_empty() {
        return Ok(Vec::new());
    }

    let mut parsed = Vec::with_capacity(values.len());
    let mut seen = BTreeSet::new();
    let mut unknown = Vec::new();

    for raw in values {
        let capability = raw.trim();
        if capability.is_empty() {
            return Err(ManifestError::InvalidField {
                field: "capabilities",
                reason: "capability entries cannot be empty strings".to_string(),
            });
        }

        match Capability::from_str(capability) {
            Some(parsed_capability) => {
                if !seen.insert(parsed_capability) {
                    return Err(ManifestError::InvalidField {
                        field: "capabilities",
                        reason: format!("duplicate capability '{}'", capability),
                    });
                }
                parsed.push(parsed_capability);
            }
            None => unknown.push(capability.to_string()),
        }
    }

    if !unknown.is_empty() {
        unknown.sort();
        let allowed_values = Capability::ALL
            .iter()
            .map(|capability| capability.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        return Err(ManifestError::InvalidField {
            field: "capabilities",
            reason: format!(
                "unknown capability value(s): [{}]; allowed values: [{}]",
                unknown.join(", "),
                allowed_values
            ),
        });
    }

    parsed.sort();
    Ok(parsed)
}

fn validate_autonomy_level(value: Option<u8>) -> Result<Option<u8>, ManifestError> {
    let Some(level) = value else {
        return Ok(None);
    };
    if level > 5 {
        return Err(ManifestError::InvalidField {
            field: "autonomy_level",
            reason: "must be one of 0, 1, 2, 3, 4, 5".to_string(),
        });
    }
    Ok(Some(level))
}

#[cfg(test)]
mod tests {
    use super::{parse_manifest, Capability, ManifestError};

    #[test]
    fn parses_valid_manifest_with_capabilities() {
        let input = r#"
name = "agent.alpha"
version = "0.1.0"
capabilities = ["fs.read", "llm.invoke"]
fuel_budget = 500
schedule = "*/5 * * * *"
llm_model = "gpt-5-mini"
"#;

        let result = parse_manifest(input);
        match result {
            Ok(manifest) => {
                assert_eq!(manifest.name, "agent.alpha");
                assert_eq!(manifest.version, "0.1.0");
                assert_eq!(manifest.fuel_budget, 500);
                assert_eq!(manifest.autonomy_level, None);
                assert_eq!(manifest.schedule, "*/5 * * * *");
                assert_eq!(manifest.llm_model, "gpt-5-mini");
                assert_eq!(
                    manifest.capabilities,
                    vec![Capability::FsRead, Capability::LlmInvoke]
                );
            }
            Err(err) => panic!("expected manifest to parse successfully, got error: {err}"),
        }
    }

    #[test]
    fn allows_empty_capability_list() {
        let input = r#"
name = "agent.beta"
version = "1.2.3"
capabilities = []
fuel_budget = 1
schedule = "0 * * * *"
llm_model = "gpt-5"
"#;

        let result = parse_manifest(input);
        match result {
            Ok(manifest) => assert!(manifest.capabilities.is_empty()),
            Err(err) => panic!("expected empty capability list to be allowed, got: {err}"),
        }
    }

    #[test]
    fn rejects_unknown_capability_with_precise_error() {
        let input = r#"
name = "agent.gamma"
version = "0.2.0"
capabilities = ["fs.read", "db.admin"]
fuel_budget = 10
schedule = "@hourly"
llm_model = "gpt-5-nano"
"#;

        let result = parse_manifest(input);
        match result {
            Err(ManifestError::InvalidField { field, reason }) => {
                assert_eq!(field, "capabilities");
                assert!(reason.contains("unknown capability value(s): [db.admin]"));
                assert!(reason.contains("allowed values"));
            }
            Ok(_) => panic!("expected unknown capability validation to fail"),
            Err(other) => panic!("expected capability validation error, got: {other}"),
        }
    }

    #[test]
    fn rejects_duplicate_capabilities() {
        let input = r#"
name = "agent.delta"
version = "0.2.1"
capabilities = ["fs.read", "fs.read"]
fuel_budget = 10
schedule = "@daily"
llm_model = "gpt-5-mini"
"#;

        let result = parse_manifest(input);
        match result {
            Err(ManifestError::InvalidField { field, reason }) => {
                assert_eq!(field, "capabilities");
                assert!(reason.contains("duplicate capability"));
            }
            Ok(_) => panic!("expected duplicate capability validation to fail"),
            Err(other) => panic!("expected duplicate capability error, got: {other}"),
        }
    }

    #[test]
    fn rejects_missing_required_field() {
        let input = r#"
name = "agent.epsilon"
version = "0.9.0"
capabilities = []
fuel_budget = 2
schedule = "0 0 * * *"
"#;

        let result = parse_manifest(input);
        match result {
            Err(ManifestError::MissingField { field }) => assert_eq!(field, "llm_model"),
            Ok(_) => panic!("expected missing field validation to fail"),
            Err(other) => panic!("expected missing field error, got: {other}"),
        }
    }

    #[test]
    fn rejects_invalid_name() {
        let input = r#"
name = "agent bad"
version = "0.1.0"
capabilities = []
fuel_budget = 2
schedule = "0 0 * * *"
llm_model = "gpt-5"
"#;

        let result = parse_manifest(input);
        match result {
            Err(ManifestError::InvalidField { field, reason }) => {
                assert_eq!(field, "name");
                assert!(reason.contains("must only contain ASCII letters"));
            }
            Ok(_) => panic!("expected invalid name validation to fail"),
            Err(other) => panic!("expected invalid name error, got: {other}"),
        }
    }

    #[test]
    fn rejects_invalid_fuel_budget() {
        let input = r#"
name = "agent.zeta"
version = "0.1.0"
capabilities = []
fuel_budget = 0
schedule = "@daily"
llm_model = "gpt-5"
"#;

        let result = parse_manifest(input);
        match result {
            Err(ManifestError::InvalidField { field, reason }) => {
                assert_eq!(field, "fuel_budget");
                assert!(reason.contains("greater than 0"));
            }
            Ok(_) => panic!("expected fuel budget validation to fail"),
            Err(other) => panic!("expected fuel budget error, got: {other}"),
        }
    }

    #[test]
    fn parses_autonomy_level_when_in_range() {
        let input = r#"
name = "agent.theta"
version = "1.0.0"
capabilities = []
fuel_budget = 100
autonomy_level = 3
schedule = "@daily"
llm_model = "gpt-5"
"#;

        let result = parse_manifest(input).expect("manifest with autonomy_level should parse");
        assert_eq!(result.autonomy_level, Some(3));
    }

    #[test]
    fn rejects_out_of_range_autonomy_level() {
        let input = r#"
name = "agent.iota"
version = "1.0.0"
capabilities = []
fuel_budget = 100
autonomy_level = 9
schedule = "@daily"
llm_model = "gpt-5"
"#;

        let result = parse_manifest(input);
        match result {
            Err(ManifestError::InvalidField { field, reason }) => {
                assert_eq!(field, "autonomy_level");
                assert!(reason.contains("must be one of 0, 1, 2, 3, 4, 5"));
            }
            Ok(_) => panic!("expected autonomy_level validation to fail"),
            Err(other) => panic!("expected autonomy_level validation error, got: {other}"),
        }
    }

    #[test]
    fn surfaces_toml_syntax_errors() {
        let input = r#"
name = "agent.eta"
version = "0.1.0"
capabilities = ["fs.read"
fuel_budget = 100
schedule = "@hourly"
llm_model = "gpt-5"
"#;

        let result = parse_manifest(input);
        match result {
            Err(ManifestError::TomlSyntax { reason }) => {
                assert!(reason.to_lowercase().contains("toml"));
            }
            Ok(_) => panic!("expected TOML syntax parsing to fail"),
            Err(other) => panic!("expected TOML syntax error, got: {other}"),
        }
    }
}
