use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchLangError {
    ParseError(String),
    UnsupportedOperation(String),
    VerifierBoundaryViolation,
    CapabilityEscalationAttempt,
    AuditBypassAttempt,
    EndpointFormatInvalid(String),
    ValueOutOfRange(String),
}

impl std::fmt::Display for PatchLangError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchLangError::ParseError(reason) => write!(f, "patch parse error: {reason}"),
            PatchLangError::UnsupportedOperation(op) => {
                write!(f, "unsupported patch operation: {op}")
            }
            PatchLangError::VerifierBoundaryViolation => {
                write!(f, "verifier boundary violation")
            }
            PatchLangError::CapabilityEscalationAttempt => {
                write!(f, "capability escalation attempt blocked")
            }
            PatchLangError::AuditBypassAttempt => write!(f, "audit bypass attempt blocked"),
            PatchLangError::EndpointFormatInvalid(endpoint) => {
                write!(f, "endpoint must start with https://, got '{endpoint}'")
            }
            PatchLangError::ValueOutOfRange(value) => {
                write!(f, "parameter value out of range: {value}")
            }
        }
    }
}

impl std::error::Error for PatchLangError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PatchOperation {
    SetConfigValue { key: String, value: String },
    UpdateApiEndpoint { service: String, url: String },
    AdjustParameter { name: String, value: f64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchProgram {
    pub operations: Vec<PatchOperation>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RuntimePatchState {
    pub config: BTreeMap<String, String>,
    pub endpoints: BTreeMap<String, String>,
    pub parameters: BTreeMap<String, f64>,
}

pub fn parse_patch(source: &str) -> Result<PatchProgram, PatchLangError> {
    let mut operations = Vec::new();

    for (line_number, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let (lhs, rhs) = trimmed.split_once('=').ok_or_else(|| {
            PatchLangError::ParseError(format!("line {} missing '='", line_number + 1))
        })?;
        let target = lhs.trim();
        let value_raw = rhs.trim();

        if let Some(key) = target.strip_prefix("config.") {
            let value = parse_string_or_raw(value_raw);
            operations.push(PatchOperation::SetConfigValue {
                key: key.trim().to_string(),
                value,
            });
            continue;
        }

        if let Some(service) = target.strip_prefix("endpoint.") {
            let url = parse_string_or_raw(value_raw);
            operations.push(PatchOperation::UpdateApiEndpoint {
                service: service.trim().to_string(),
                url,
            });
            continue;
        }

        if let Some(name) = target.strip_prefix("param.") {
            let value = parse_numeric(value_raw).map_err(|_| {
                PatchLangError::ParseError(format!(
                    "line {} expected numeric parameter value",
                    line_number + 1
                ))
            })?;
            operations.push(PatchOperation::AdjustParameter {
                name: name.trim().to_string(),
                value,
            });
            continue;
        }

        return Err(PatchLangError::UnsupportedOperation(target.to_string()));
    }

    if operations.is_empty() {
        return Err(PatchLangError::ParseError(
            "patch contains no operations".to_string(),
        ));
    }

    Ok(PatchProgram {
        operations,
        source: source.to_string(),
    })
}

pub fn validate_patch(program: &PatchProgram) -> Result<(), PatchLangError> {
    for operation in &program.operations {
        match operation {
            PatchOperation::SetConfigValue { key, value } => {
                guard_verifier_boundary(key.as_str(), value.as_str())?;
                guard_capability_escalation(key.as_str(), value.as_str())?;
                guard_audit_bypass(key.as_str(), value.as_str())?;
            }
            PatchOperation::UpdateApiEndpoint { service, url } => {
                guard_verifier_boundary(service.as_str(), url.as_str())?;
                guard_capability_escalation(service.as_str(), url.as_str())?;
                guard_audit_bypass(service.as_str(), url.as_str())?;
                if !url.starts_with("https://") {
                    return Err(PatchLangError::EndpointFormatInvalid(url.clone()));
                }
            }
            PatchOperation::AdjustParameter { name, value } => {
                guard_verifier_boundary(name.as_str(), value.to_string().as_str())?;
                guard_capability_escalation(name.as_str(), value.to_string().as_str())?;
                guard_audit_bypass(name.as_str(), value.to_string().as_str())?;
                if !(-1_000_000.0..=1_000_000.0).contains(value) {
                    return Err(PatchLangError::ValueOutOfRange(value.to_string()));
                }
            }
        }
    }
    Ok(())
}

pub fn apply_patch(
    program: &PatchProgram,
    state: &mut RuntimePatchState,
) -> Result<(), PatchLangError> {
    validate_patch(program)?;

    for operation in &program.operations {
        match operation {
            PatchOperation::SetConfigValue { key, value } => {
                state.config.insert(key.clone(), value.clone());
            }
            PatchOperation::UpdateApiEndpoint { service, url } => {
                state.endpoints.insert(service.clone(), url.clone());
            }
            PatchOperation::AdjustParameter { name, value } => {
                state.parameters.insert(name.clone(), *value);
            }
        }
    }
    Ok(())
}

fn parse_string_or_raw(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_numeric(raw: &str) -> Result<f64, std::num::ParseFloatError> {
    let value = parse_string_or_raw(raw);
    value.parse::<f64>()
}

fn guard_verifier_boundary(target: &str, value: &str) -> Result<(), PatchLangError> {
    const VERIFIER_BLOCKLIST: [&str; 7] = [
        "verifier",
        "verification_kernel",
        "kernel.verify",
        "tuf.root",
        "signature_verifier",
        "root_of_trust",
        "attestation_verifier",
    ];
    let haystack = format!("{} {}", target.to_lowercase(), value.to_lowercase());
    if VERIFIER_BLOCKLIST
        .iter()
        .any(|token| haystack.contains(token))
    {
        return Err(PatchLangError::VerifierBoundaryViolation);
    }
    Ok(())
}

fn guard_capability_escalation(target: &str, value: &str) -> Result<(), PatchLangError> {
    const CAPABILITY_BLOCKLIST: [&str; 4] =
        ["capability", "capabilities", "allow_all_caps", "privileged_mode"];
    let haystack = format!("{} {}", target.to_lowercase(), value.to_lowercase());
    if CAPABILITY_BLOCKLIST
        .iter()
        .any(|token| haystack.contains(token))
    {
        return Err(PatchLangError::CapabilityEscalationAttempt);
    }
    Ok(())
}

fn guard_audit_bypass(target: &str, value: &str) -> Result<(), PatchLangError> {
    let target_lower = target.to_lowercase();
    let value_lower = value.to_lowercase();
    let bypass = (target_lower.contains("audit")
        && (value_lower.contains("disable")
            || value_lower == "off"
            || value_lower == "false"))
        || value_lower.contains("bypass_audit")
        || value_lower.contains("skip_audit");
    if bypass {
        return Err(PatchLangError::AuditBypassAttempt);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_patch, validate_patch, PatchLangError};

    #[test]
    fn test_patch_cannot_modify_verifier() {
        let patch = parse_patch(r#"config.verification_kernel.strict_mode = "off""#)
            .expect("patch should parse");
        let validation = validate_patch(&patch);
        assert_eq!(validation, Err(PatchLangError::VerifierBoundaryViolation));
    }
}
