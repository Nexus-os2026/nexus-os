//! KPI monitoring dashboard backend for governance health scoring.
//!
//! Computes governance KPIs across four categories (Safety, Reliability,
//! Governance, Economic) and produces weighted health scores and per-agent
//! scorecards with risk ratings.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Types ───────────────────────────────────────────────────────────────

/// Risk rating derived from health score thresholds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskRating {
    /// Score >= 0.7
    Healthy,
    /// Score >= 0.5 and < 0.7
    Warning,
    /// Score < 0.5
    Critical,
}

/// A single KPI measurement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KpiMetric {
    pub name: String,
    pub category: KpiCategory,
    pub score: f64,
    pub description: String,
    pub measured_at: u64,
}

/// KPI category with associated weight in health score calculation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KpiCategory {
    /// Weight: 35%
    Safety,
    /// Weight: 25%
    Reliability,
    /// Weight: 25%
    Governance,
    /// Weight: 15%
    Economic,
}

/// Per-agent scorecard with all KPIs and aggregate health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentScorecard {
    pub scorecard_id: String,
    pub agent_id: String,
    pub metrics: Vec<KpiMetric>,
    pub health_score: f64,
    pub risk_rating: RiskRating,
    pub generated_at: u64,
}

/// Raw metrics fed into the KPI engine for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetrics {
    // Safety
    pub total_pii_detected: u64,
    pub pii_redacted: u64,
    pub total_firewall_checks: u64,
    pub firewall_blocks: u64,
    // Reliability
    pub uptime_seconds: u64,
    pub total_seconds: u64,
    pub total_requests: u64,
    pub error_count: u64,
    // Governance
    pub audit_chain_valid: bool,
    pub total_consent_checks: u64,
    pub consent_compliant: u64,
    // Economic
    pub fuel_allocated: u64,
    pub fuel_consumed: u64,
    pub budget_limit: f64,
    pub actual_spend: f64,
}

impl Default for AgentMetrics {
    fn default() -> Self {
        Self {
            total_pii_detected: 0,
            pii_redacted: 0,
            total_firewall_checks: 0,
            firewall_blocks: 0,
            uptime_seconds: 0,
            total_seconds: 0,
            total_requests: 0,
            error_count: 0,
            audit_chain_valid: true,
            total_consent_checks: 0,
            consent_compliant: 0,
            fuel_allocated: 0,
            fuel_consumed: 0,
            budget_limit: 0.0,
            actual_spend: 0.0,
        }
    }
}

// ── Engine ──────────────────────────────────────────────────────────────

/// KPI computation engine that tracks agent metrics and produces scorecards.
pub struct KpiEngine {
    agent_metrics: HashMap<String, AgentMetrics>,
    scorecards: Vec<AgentScorecard>,
}

impl Default for KpiEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl KpiEngine {
    pub fn new() -> Self {
        Self {
            agent_metrics: HashMap::new(),
            scorecards: Vec::new(),
        }
    }

    /// Update raw metrics for an agent.
    pub fn update_metrics(&mut self, agent_id: &str, metrics: AgentMetrics) {
        self.agent_metrics.insert(agent_id.to_string(), metrics);
    }

    /// Get current metrics for an agent.
    pub fn get_metrics(&self, agent_id: &str) -> Option<&AgentMetrics> {
        self.agent_metrics.get(agent_id)
    }

    /// Compute the PII redaction rate (Safety). Returns 1.0 if no PII detected.
    pub fn pii_redaction_rate(metrics: &AgentMetrics) -> f64 {
        if metrics.total_pii_detected == 0 {
            return 1.0;
        }
        (metrics.pii_redacted as f64 / metrics.total_pii_detected as f64).clamp(0.0, 1.0)
    }

    /// Compute the firewall block rate (Safety). Returns 1.0 if no checks performed.
    pub fn firewall_block_rate(metrics: &AgentMetrics) -> f64 {
        if metrics.total_firewall_checks == 0 {
            return 1.0;
        }
        (metrics.firewall_blocks as f64 / metrics.total_firewall_checks as f64).clamp(0.0, 1.0)
    }

    /// Compute uptime ratio (Reliability). Returns 1.0 if no time tracked.
    pub fn uptime_ratio(metrics: &AgentMetrics) -> f64 {
        if metrics.total_seconds == 0 {
            return 1.0;
        }
        (metrics.uptime_seconds as f64 / metrics.total_seconds as f64).clamp(0.0, 1.0)
    }

    /// Compute error rate inverted as a score (Reliability). Returns 1.0 if no requests.
    pub fn error_rate_score(metrics: &AgentMetrics) -> f64 {
        if metrics.total_requests == 0 {
            return 1.0;
        }
        let error_rate = metrics.error_count as f64 / metrics.total_requests as f64;
        (1.0 - error_rate).clamp(0.0, 1.0)
    }

    /// Compute audit chain integrity score (Governance). Binary: 1.0 or 0.0.
    pub fn audit_chain_score(metrics: &AgentMetrics) -> f64 {
        if metrics.audit_chain_valid {
            1.0
        } else {
            0.0
        }
    }

    /// Compute consent compliance rate (Governance). Returns 1.0 if no checks.
    pub fn consent_compliance_rate(metrics: &AgentMetrics) -> f64 {
        if metrics.total_consent_checks == 0 {
            return 1.0;
        }
        (metrics.consent_compliant as f64 / metrics.total_consent_checks as f64).clamp(0.0, 1.0)
    }

    /// Compute fuel efficiency (Economic). Returns 1.0 if no fuel allocated.
    pub fn fuel_efficiency(metrics: &AgentMetrics) -> f64 {
        if metrics.fuel_allocated == 0 {
            return 1.0;
        }
        // Higher is better: unused fuel ratio, capped at 1.0
        let ratio = metrics.fuel_consumed as f64 / metrics.fuel_allocated as f64;
        (1.0 - (ratio - 1.0).max(0.0)).clamp(0.0, 1.0)
    }

    /// Compute budget adherence (Economic). Returns 1.0 if no budget.
    pub fn budget_adherence(metrics: &AgentMetrics) -> f64 {
        if metrics.budget_limit <= 0.0 {
            return 1.0;
        }
        if metrics.actual_spend <= metrics.budget_limit {
            1.0
        } else {
            (metrics.budget_limit / metrics.actual_spend).clamp(0.0, 1.0)
        }
    }

    /// Compute all KPI metrics for an agent.
    pub fn compute_kpis(&self, agent_id: &str) -> Vec<KpiMetric> {
        let metrics = match self.agent_metrics.get(agent_id) {
            Some(m) => m,
            None => return Vec::new(),
        };
        let now = now_secs();

        vec![
            KpiMetric {
                name: "PII Redaction Rate".to_string(),
                category: KpiCategory::Safety,
                score: Self::pii_redaction_rate(metrics),
                description: "Fraction of detected PII that was redacted".to_string(),
                measured_at: now,
            },
            KpiMetric {
                name: "Firewall Block Rate".to_string(),
                category: KpiCategory::Safety,
                score: Self::firewall_block_rate(metrics),
                description: "Fraction of firewall checks that blocked threats".to_string(),
                measured_at: now,
            },
            KpiMetric {
                name: "Uptime Ratio".to_string(),
                category: KpiCategory::Reliability,
                score: Self::uptime_ratio(metrics),
                description: "System uptime as fraction of total time".to_string(),
                measured_at: now,
            },
            KpiMetric {
                name: "Error Rate Score".to_string(),
                category: KpiCategory::Reliability,
                score: Self::error_rate_score(metrics),
                description: "Inverse of error rate (1.0 = no errors)".to_string(),
                measured_at: now,
            },
            KpiMetric {
                name: "Audit Chain Integrity".to_string(),
                category: KpiCategory::Governance,
                score: Self::audit_chain_score(metrics),
                description: "Whether the audit hash-chain is intact".to_string(),
                measured_at: now,
            },
            KpiMetric {
                name: "Consent Compliance Rate".to_string(),
                category: KpiCategory::Governance,
                score: Self::consent_compliance_rate(metrics),
                description: "Fraction of actions with proper consent".to_string(),
                measured_at: now,
            },
            KpiMetric {
                name: "Fuel Efficiency".to_string(),
                category: KpiCategory::Economic,
                score: Self::fuel_efficiency(metrics),
                description: "Fuel usage within allocated budget".to_string(),
                measured_at: now,
            },
            KpiMetric {
                name: "Budget Adherence".to_string(),
                category: KpiCategory::Economic,
                score: Self::budget_adherence(metrics),
                description: "Spending within budget limits".to_string(),
                measured_at: now,
            },
        ]
    }

    /// Compute the weighted aggregate health score for an agent.
    ///
    /// Weights: Safety 35%, Reliability 25%, Governance 25%, Economic 15%.
    pub fn compute_health_score(&self, agent_id: &str) -> f64 {
        let kpis = self.compute_kpis(agent_id);
        if kpis.is_empty() {
            return 0.0;
        }

        let category_avg = |cat: &KpiCategory| -> f64 {
            let scores: Vec<f64> = kpis
                .iter()
                .filter(|k| &k.category == cat)
                .map(|k| k.score)
                .collect();
            if scores.is_empty() {
                return 0.0;
            }
            scores.iter().sum::<f64>() / scores.len() as f64
        };

        let safety = category_avg(&KpiCategory::Safety);
        let reliability = category_avg(&KpiCategory::Reliability);
        let governance = category_avg(&KpiCategory::Governance);
        let economic = category_avg(&KpiCategory::Economic);

        (0.35 * safety + 0.25 * reliability + 0.25 * governance + 0.15 * economic).clamp(0.0, 1.0)
    }

    /// Derive a risk rating from a health score.
    pub fn risk_rating(score: f64) -> RiskRating {
        if score < 0.5 {
            RiskRating::Critical
        } else if score < 0.7 {
            RiskRating::Warning
        } else {
            RiskRating::Healthy
        }
    }

    /// Generate a full scorecard for an agent.
    pub fn generate_scorecard(&mut self, agent_id: &str) -> AgentScorecard {
        let metrics = self.compute_kpis(agent_id);
        let health_score = self.compute_health_score(agent_id);
        let rating = Self::risk_rating(health_score);

        let scorecard = AgentScorecard {
            scorecard_id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            metrics,
            health_score,
            risk_rating: rating,
            generated_at: now_secs(),
        };

        self.scorecards.push(scorecard.clone());
        scorecard
    }

    /// Get historical scorecards for an agent.
    pub fn get_scorecards(&self, agent_id: &str) -> Vec<&AgentScorecard> {
        self.scorecards
            .iter()
            .filter(|s| s.agent_id == agent_id)
            .collect()
    }

    /// Total number of tracked agents.
    pub fn agent_count(&self) -> usize {
        self.agent_metrics.len()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn perfect_metrics() -> AgentMetrics {
        AgentMetrics {
            total_pii_detected: 100,
            pii_redacted: 100,
            total_firewall_checks: 50,
            firewall_blocks: 50,
            uptime_seconds: 3600,
            total_seconds: 3600,
            total_requests: 1000,
            error_count: 0,
            audit_chain_valid: true,
            total_consent_checks: 200,
            consent_compliant: 200,
            fuel_allocated: 1000,
            fuel_consumed: 500,
            budget_limit: 100.0,
            actual_spend: 80.0,
        }
    }

    fn poor_metrics() -> AgentMetrics {
        AgentMetrics {
            total_pii_detected: 100,
            pii_redacted: 20,
            total_firewall_checks: 50,
            firewall_blocks: 10,
            uptime_seconds: 1800,
            total_seconds: 3600,
            total_requests: 1000,
            error_count: 500,
            audit_chain_valid: false,
            total_consent_checks: 200,
            consent_compliant: 50,
            fuel_allocated: 1000,
            fuel_consumed: 1500,
            budget_limit: 100.0,
            actual_spend: 200.0,
        }
    }

    #[test]
    fn test_pii_redaction_rate() {
        let m = perfect_metrics();
        assert!((KpiEngine::pii_redaction_rate(&m) - 1.0).abs() < f64::EPSILON);

        let m2 = poor_metrics();
        assert!((KpiEngine::pii_redaction_rate(&m2) - 0.2).abs() < f64::EPSILON);

        let empty = AgentMetrics::default();
        assert!((KpiEngine::pii_redaction_rate(&empty) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_error_rate_score() {
        let m = perfect_metrics();
        assert!((KpiEngine::error_rate_score(&m) - 1.0).abs() < f64::EPSILON);

        let m2 = poor_metrics();
        assert!((KpiEngine::error_rate_score(&m2) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_audit_chain_score() {
        let valid = AgentMetrics {
            audit_chain_valid: true,
            ..Default::default()
        };
        assert!((KpiEngine::audit_chain_score(&valid) - 1.0).abs() < f64::EPSILON);

        let invalid = AgentMetrics {
            audit_chain_valid: false,
            ..Default::default()
        };
        assert!(KpiEngine::audit_chain_score(&invalid).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fuel_efficiency() {
        // Consumed less than allocated → 1.0
        let m = perfect_metrics();
        assert!((KpiEngine::fuel_efficiency(&m) - 1.0).abs() < f64::EPSILON);

        // Over-consumed
        let m2 = poor_metrics();
        assert!(KpiEngine::fuel_efficiency(&m2) < 1.0);
    }

    #[test]
    fn test_budget_adherence() {
        let m = perfect_metrics();
        assert!((KpiEngine::budget_adherence(&m) - 1.0).abs() < f64::EPSILON);

        let m2 = poor_metrics();
        assert!((KpiEngine::budget_adherence(&m2) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_health_score_perfect() {
        let mut engine = KpiEngine::new();
        engine.update_metrics("agent-1", perfect_metrics());
        let score = engine.compute_health_score("agent-1");
        assert!(
            (score - 1.0).abs() < f64::EPSILON,
            "perfect metrics should yield 1.0, got {score}"
        );
    }

    #[test]
    fn test_health_score_poor() {
        let mut engine = KpiEngine::new();
        engine.update_metrics("agent-bad", poor_metrics());
        let score = engine.compute_health_score("agent-bad");
        assert!(score < 0.5, "poor metrics should yield < 0.5, got {score}");
    }

    #[test]
    fn test_health_score_unknown_agent() {
        let engine = KpiEngine::new();
        assert!(engine.compute_health_score("ghost").abs() < f64::EPSILON);
    }

    #[test]
    fn test_risk_rating_thresholds() {
        assert_eq!(KpiEngine::risk_rating(0.9), RiskRating::Healthy);
        assert_eq!(KpiEngine::risk_rating(0.7), RiskRating::Healthy);
        assert_eq!(KpiEngine::risk_rating(0.69), RiskRating::Warning);
        assert_eq!(KpiEngine::risk_rating(0.5), RiskRating::Warning);
        assert_eq!(KpiEngine::risk_rating(0.49), RiskRating::Critical);
        assert_eq!(KpiEngine::risk_rating(0.0), RiskRating::Critical);
    }

    #[test]
    fn test_generate_scorecard() {
        let mut engine = KpiEngine::new();
        engine.update_metrics("agent-1", perfect_metrics());
        let card = engine.generate_scorecard("agent-1");
        assert_eq!(card.agent_id, "agent-1");
        assert_eq!(card.risk_rating, RiskRating::Healthy);
        assert_eq!(card.metrics.len(), 8);
        assert!(!card.scorecard_id.is_empty());
    }

    #[test]
    fn test_scorecard_history() {
        let mut engine = KpiEngine::new();
        engine.update_metrics("agent-1", perfect_metrics());
        engine.generate_scorecard("agent-1");
        engine.generate_scorecard("agent-1");
        assert_eq!(engine.get_scorecards("agent-1").len(), 2);
        assert_eq!(engine.get_scorecards("other").len(), 0);
    }

    #[test]
    fn test_compute_kpis_returns_all_categories() {
        let mut engine = KpiEngine::new();
        engine.update_metrics("a1", perfect_metrics());
        let kpis = engine.compute_kpis("a1");
        let categories: Vec<&KpiCategory> = kpis.iter().map(|k| &k.category).collect();
        assert!(categories.contains(&&KpiCategory::Safety));
        assert!(categories.contains(&&KpiCategory::Reliability));
        assert!(categories.contains(&&KpiCategory::Governance));
        assert!(categories.contains(&&KpiCategory::Economic));
    }

    #[test]
    fn test_agent_count() {
        let mut engine = KpiEngine::new();
        assert_eq!(engine.agent_count(), 0);
        engine.update_metrics("a1", AgentMetrics::default());
        engine.update_metrics("a2", AgentMetrics::default());
        assert_eq!(engine.agent_count(), 2);
    }
}
