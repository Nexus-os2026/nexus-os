use crate::package::SignedPackageBundle;
use crate::trust::{CapabilityRisk, TrustSystem};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyLintReport {
    pub risk: CapabilityRisk,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticAnalysisReport {
    pub suspicious_patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BehavioralSandboxReport {
    pub passed: bool,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafetyScanReport {
    pub policy: PolicyLintReport,
    pub static_analysis: StaticAnalysisReport,
    pub sandbox: BehavioralSandboxReport,
}

pub fn scan_package(bundle: &SignedPackageBundle) -> SafetyScanReport {
    SafetyScanReport {
        policy: policy_lint(bundle.metadata.capabilities.as_slice()),
        static_analysis: static_analysis(bundle.agent_code.as_str()),
        sandbox: behavioral_sandbox(bundle.agent_code.as_str()),
    }
}

pub fn policy_lint(capabilities: &[String]) -> PolicyLintReport {
    let normalized = capabilities
        .iter()
        .map(|capability| capability.trim().to_lowercase())
        .collect::<BTreeSet<_>>();

    if normalized.contains("screen.capture")
        && normalized.contains("input.keyboard")
        && normalized.contains("fs.write")
    {
        return PolicyLintReport {
            risk: CapabilityRisk::High,
            findings: vec![
                "excessive interactive device control with write access".to_string(),
                "requires manual security review before publish".to_string(),
            ],
        };
    }

    let base_risk = TrustSystem::classify_capability_set(capabilities);
    let mut findings = Vec::new();

    if capabilities.len() > 5 {
        findings.push(
            "package requests many capabilities; principle-of-least-privilege warning".to_string(),
        );
    }
    if base_risk == CapabilityRisk::High {
        findings.push("contains high-risk capability".to_string());
    }

    PolicyLintReport {
        risk: base_risk,
        findings,
    }
}

pub fn static_analysis(agent_code: &str) -> StaticAnalysisReport {
    let patterns = [
        "std::process::Command",
        "unsafe",
        "curl http",
        "wget http",
        "rm -rf",
        "shell.exec",
    ];

    let suspicious_patterns = patterns
        .iter()
        .filter(|pattern| agent_code.contains(**pattern))
        .map(|pattern| (*pattern).to_string())
        .collect::<Vec<_>>();

    StaticAnalysisReport {
        suspicious_patterns,
    }
}

pub fn behavioral_sandbox(agent_code: &str) -> BehavioralSandboxReport {
    let mut findings = Vec::new();

    if agent_code.contains("while true") || agent_code.contains("loop {") {
        findings.push("possible non-terminating behavior detected".to_string());
    }
    if agent_code.contains("thread::spawn(") && agent_code.contains("unbounded_channel") {
        findings.push("possible unbounded concurrency pattern".to_string());
    }

    BehavioralSandboxReport {
        passed: findings.is_empty(),
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::policy_lint;
    use crate::trust::CapabilityRisk;

    #[test]
    fn test_policy_lint_excessive_capabilities() {
        let capabilities = vec![
            "screen.capture".to_string(),
            "input.keyboard".to_string(),
            "fs.write".to_string(),
        ];

        let lint = policy_lint(capabilities.as_slice());
        assert_eq!(lint.risk, CapabilityRisk::High);
        assert!(!lint.findings.is_empty());
    }
}
