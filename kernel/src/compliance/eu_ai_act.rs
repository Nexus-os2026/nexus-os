//! EU AI Act (Regulation 2024/1689) risk classification for agents.
//!
//! Implements Article 6 risk tiers and maps agent manifests to the appropriate
//! tier based on autonomy level, declared capabilities, and domain tags.

use crate::autonomy::AutonomyLevel;
use crate::manifest::AgentManifest;
use serde::{Deserialize, Serialize};

/// EU AI Act risk tiers per Article 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EuAiActRiskTier {
    /// No significant risk — read-only, audit-only agents.
    Minimal,
    /// Limited transparency obligations — LLM query, web search.
    Limited,
    /// Significant risk — requires conformity assessment, logging, human oversight.
    High,
    /// Prohibited practices — biometric categorisation, social scoring, etc.
    Unacceptable,
}

impl EuAiActRiskTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Limited => "limited",
            Self::High => "high",
            Self::Unacceptable => "unacceptable",
        }
    }
}

/// Domain tags that trigger Unacceptable classification (Article 5 prohibitions).
const PROHIBITED_DOMAINS: &[&str] = &[
    "biometric",
    "biometric-categorisation",
    "social-scoring",
    "law-enforcement",
    "emotion-recognition",
    "predictive-policing",
];

/// Domain tags that trigger High-risk classification (Annex III).
const HIGH_RISK_DOMAINS: &[&str] = &[
    "critical-infrastructure",
    "education",
    "employment",
    "essential-services",
    "migration",
    "justice",
];

/// Capabilities that together signal High-risk (write + network access).
const HIGH_RISK_WRITE_CAPS: &[&str] = &["fs.write", "process.exec"];
const HIGH_RISK_NETWORK_CAPS: &[&str] = &["web.search", "web.read"];

/// Capabilities that alone signal Limited risk (AI interaction).
const LIMITED_CAPS: &[&str] = &["llm.query"];

/// Result of classifying an agent under the EU AI Act.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRiskProfile {
    /// The assigned risk tier.
    pub tier: EuAiActRiskTier,
    /// Human-readable justification for the classification.
    pub justification: String,
    /// Applicable EU AI Act articles.
    pub applicable_articles: Vec<String>,
    /// Controls required for this tier.
    pub required_controls: Vec<String>,
}

/// Classifies agents according to the EU AI Act risk framework.
#[derive(Debug, Clone, Default)]
pub struct RiskClassifier;

impl RiskClassifier {
    pub fn new() -> Self {
        Self
    }

    /// Classify an agent manifest into an EU AI Act risk tier.
    ///
    /// Classification priority (highest wins):
    /// 1. Prohibited domain tags → Unacceptable
    /// 2. High-risk domain tags → High
    /// 3. Autonomy L4-L5 → High
    /// 4. Write + network capabilities → High
    /// 5. LLM/AI capabilities → Limited
    /// 6. Everything else → Minimal
    pub fn classify_agent(&self, manifest: &AgentManifest) -> AgentRiskProfile {
        // 1. Check for prohibited domains (Article 5).
        for tag in &manifest.domain_tags {
            let normalised = tag.to_lowercase();
            if PROHIBITED_DOMAINS.iter().any(|p| normalised == *p) {
                return AgentRiskProfile {
                    tier: EuAiActRiskTier::Unacceptable,
                    justification: format!(
                        "Domain tag '{}' falls under EU AI Act Article 5 prohibited practices",
                        tag
                    ),
                    applicable_articles: vec!["Article 5 - Prohibited AI practices".to_string()],
                    required_controls: vec![
                        "Deployment prohibited".to_string(),
                        "Agent must not be started".to_string(),
                    ],
                };
            }
        }

        // 2. Check for high-risk domains (Annex III).
        for tag in &manifest.domain_tags {
            let normalised = tag.to_lowercase();
            if HIGH_RISK_DOMAINS.iter().any(|d| normalised == *d) {
                return self.high_risk_profile(format!(
                    "Domain tag '{}' listed in EU AI Act Annex III high-risk categories",
                    tag
                ));
            }
        }

        let autonomy = AutonomyLevel::from_manifest(manifest.autonomy_level);

        // 3. Autonomy L4-L5 → High (limited human oversight).
        if autonomy >= AutonomyLevel::L4 {
            return self.high_risk_profile(format!(
                "Autonomy level {} exceeds act-with-approval threshold; \
                 limited human oversight triggers Article 14 requirements",
                autonomy.as_str()
            ));
        }

        // 4. Write + network capabilities → High.
        let has_write = manifest
            .capabilities
            .iter()
            .any(|c| HIGH_RISK_WRITE_CAPS.contains(&c.as_str()));
        let has_network = manifest
            .capabilities
            .iter()
            .any(|c| HIGH_RISK_NETWORK_CAPS.contains(&c.as_str()));
        if has_write && has_network {
            return self.high_risk_profile(
                "Agent combines write/exec capabilities with network access, \
                 creating significant risk of external impact"
                    .to_string(),
            );
        }

        // 5. LLM/AI capabilities → Limited.
        let has_limited = manifest
            .capabilities
            .iter()
            .any(|c| LIMITED_CAPS.contains(&c.as_str()));
        if has_limited {
            return AgentRiskProfile {
                tier: EuAiActRiskTier::Limited,
                justification: "Agent uses AI model interaction (llm.query); \
                     Article 52 transparency obligations apply"
                    .to_string(),
                applicable_articles: vec!["Article 52 - Transparency obligations".to_string()],
                required_controls: vec![
                    "Disclose AI-generated content to users".to_string(),
                    "Log all LLM interactions in audit trail".to_string(),
                ],
            };
        }

        // 6. Minimal — no significant AI risk.
        AgentRiskProfile {
            tier: EuAiActRiskTier::Minimal,
            justification: "Agent capabilities limited to read-only or audit operations; \
                            no significant AI risk identified"
                .to_string(),
            applicable_articles: vec![],
            required_controls: vec![],
        }
    }

    /// Check whether an agent may be deployed. Unacceptable agents are rejected.
    pub fn may_deploy(
        &self,
        manifest: &AgentManifest,
    ) -> Result<AgentRiskProfile, AgentRiskProfile> {
        let profile = self.classify_agent(manifest);
        if profile.tier == EuAiActRiskTier::Unacceptable {
            Err(profile)
        } else {
            Ok(profile)
        }
    }

    /// For High-risk agents, return the minimum required autonomy level.
    /// Article 14 mandates human oversight — we enforce act-with-approval (L2).
    pub fn minimum_autonomy_for_tier(tier: EuAiActRiskTier) -> Option<AutonomyLevel> {
        match tier {
            EuAiActRiskTier::High => Some(AutonomyLevel::L2),
            _ => None,
        }
    }

    /// Check if an agent's autonomy level complies with its risk tier.
    /// High-risk agents must not exceed L2 (act-with-approval) to ensure human oversight.
    pub fn autonomy_compliant(manifest: &AgentManifest, tier: EuAiActRiskTier) -> bool {
        if tier != EuAiActRiskTier::High {
            return true;
        }
        let autonomy = AutonomyLevel::from_manifest(manifest.autonomy_level);
        // High-risk agents require human-in-the-loop: autonomy must be at most L2
        autonomy <= AutonomyLevel::L2
    }

    fn high_risk_profile(&self, justification: String) -> AgentRiskProfile {
        AgentRiskProfile {
            tier: EuAiActRiskTier::High,
            justification,
            applicable_articles: vec![
                "Article 6 - Classification rules for high-risk AI".to_string(),
                "Article 9 - Risk management system".to_string(),
                "Article 11 - Technical documentation".to_string(),
                "Article 12 - Record-keeping (logging)".to_string(),
                "Article 13 - Transparency and information".to_string(),
                "Article 14 - Human oversight".to_string(),
            ],
            required_controls: vec![
                "Conformity assessment before deployment".to_string(),
                "Continuous risk monitoring".to_string(),
                "Human oversight (act-with-approval minimum)".to_string(),
                "Complete audit trail with hash-chain integrity".to_string(),
                "Technical documentation maintained".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::AgentManifest;

    fn base_manifest(name: &str, caps: Vec<&str>) -> AgentManifest {
        AgentManifest {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            capabilities: caps.into_iter().map(String::from).collect(),
            fuel_budget: 1000,
            autonomy_level: None,
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            llm_model: None,
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
        }
    }

    #[test]
    fn minimal_read_only_agent() {
        let manifest = base_manifest("audit-reader", vec!["audit.read"]);
        let classifier = RiskClassifier::new();
        let profile = classifier.classify_agent(&manifest);

        assert_eq!(profile.tier, EuAiActRiskTier::Minimal);
        assert!(profile.required_controls.is_empty());
        assert!(profile.applicable_articles.is_empty());
    }

    #[test]
    fn limited_llm_agent() {
        let manifest = base_manifest("llm-agent", vec!["llm.query"]);
        let classifier = RiskClassifier::new();
        let profile = classifier.classify_agent(&manifest);

        assert_eq!(profile.tier, EuAiActRiskTier::Limited);
        assert!(profile.justification.contains("Article 52"));
        assert!(!profile.required_controls.is_empty());
    }

    #[test]
    fn high_risk_write_plus_network() {
        let manifest = base_manifest("risky-agent", vec!["fs.write", "web.search", "llm.query"]);
        let classifier = RiskClassifier::new();
        let profile = classifier.classify_agent(&manifest);

        assert_eq!(profile.tier, EuAiActRiskTier::High);
        assert!(profile.justification.contains("write/exec"));
        assert!(profile.applicable_articles.len() >= 5);
    }

    #[test]
    fn high_risk_autonomy_l4() {
        let mut manifest = base_manifest("autonomous-agent", vec!["llm.query"]);
        manifest.autonomy_level = Some(4);
        let classifier = RiskClassifier::new();
        let profile = classifier.classify_agent(&manifest);

        assert_eq!(profile.tier, EuAiActRiskTier::High);
        assert!(profile.justification.contains("L4"));
    }

    #[test]
    fn high_risk_domain_tag() {
        let mut manifest = base_manifest("infra-agent", vec!["audit.read"]);
        manifest.domain_tags = vec!["critical-infrastructure".to_string()];
        let classifier = RiskClassifier::new();
        let profile = classifier.classify_agent(&manifest);

        assert_eq!(profile.tier, EuAiActRiskTier::High);
        assert!(profile.justification.contains("Annex III"));
    }

    #[test]
    fn unacceptable_biometric_agent() {
        let mut manifest = base_manifest("face-scanner", vec!["fs.read"]);
        manifest.domain_tags = vec!["biometric".to_string()];
        let classifier = RiskClassifier::new();
        let profile = classifier.classify_agent(&manifest);

        assert_eq!(profile.tier, EuAiActRiskTier::Unacceptable);
        assert!(profile.justification.contains("Article 5"));
    }

    #[test]
    fn unacceptable_agent_rejected_by_may_deploy() {
        let mut manifest = base_manifest("social-scorer", vec!["llm.query"]);
        manifest.domain_tags = vec!["social-scoring".to_string()];
        let classifier = RiskClassifier::new();
        let result = classifier.may_deploy(&manifest);

        assert!(result.is_err());
        let profile = result.unwrap_err();
        assert_eq!(profile.tier, EuAiActRiskTier::Unacceptable);
    }

    #[test]
    fn high_risk_requires_l2_minimum() {
        let minimum = RiskClassifier::minimum_autonomy_for_tier(EuAiActRiskTier::High);
        assert_eq!(minimum, Some(AutonomyLevel::L2));
    }

    #[test]
    fn minimal_has_no_autonomy_requirement() {
        let minimum = RiskClassifier::minimum_autonomy_for_tier(EuAiActRiskTier::Minimal);
        assert_eq!(minimum, None);
    }

    #[test]
    fn autonomy_compliance_check() {
        let mut manifest = base_manifest("agent", vec!["fs.write", "web.read"]);

        // L2 is compliant for High
        manifest.autonomy_level = Some(2);
        assert!(RiskClassifier::autonomy_compliant(
            &manifest,
            EuAiActRiskTier::High
        ));

        // L3 exceeds allowed autonomy for High-risk
        manifest.autonomy_level = Some(3);
        assert!(!RiskClassifier::autonomy_compliant(
            &manifest,
            EuAiActRiskTier::High
        ));

        // Any autonomy is fine for Minimal
        manifest.autonomy_level = Some(5);
        assert!(RiskClassifier::autonomy_compliant(
            &manifest,
            EuAiActRiskTier::Minimal
        ));
    }

    #[test]
    fn law_enforcement_is_unacceptable() {
        let mut manifest = base_manifest("police-bot", vec!["llm.query"]);
        manifest.domain_tags = vec!["law-enforcement".to_string()];
        let classifier = RiskClassifier::new();
        let profile = classifier.classify_agent(&manifest);

        assert_eq!(profile.tier, EuAiActRiskTier::Unacceptable);
    }

    #[test]
    fn prohibited_domain_takes_priority_over_capabilities() {
        let mut manifest = base_manifest("benign-name", vec!["audit.read"]);
        manifest.domain_tags = vec!["biometric".to_string()];
        manifest.autonomy_level = Some(0);
        let classifier = RiskClassifier::new();
        let profile = classifier.classify_agent(&manifest);

        // Even with minimal capabilities and L0, biometric tag = Unacceptable
        assert_eq!(profile.tier, EuAiActRiskTier::Unacceptable);
    }

    #[test]
    fn process_exec_plus_network_is_high() {
        let manifest = base_manifest("exec-agent", vec!["process.exec", "web.read"]);
        let classifier = RiskClassifier::new();
        let profile = classifier.classify_agent(&manifest);

        assert_eq!(profile.tier, EuAiActRiskTier::High);
    }

    #[test]
    fn fs_read_only_is_minimal() {
        let manifest = base_manifest("reader", vec!["fs.read"]);
        let classifier = RiskClassifier::new();
        let profile = classifier.classify_agent(&manifest);

        assert_eq!(profile.tier, EuAiActRiskTier::Minimal);
    }
}
