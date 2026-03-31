//! # Analyzer
//!
//! Stage 2 of the self-improvement pipeline. Groups related signals,
//! classifies opportunities, estimates severity and blast radius.

use crate::types::{
    BlastRadius, ImprovementDomain, ImprovementOpportunity, ImprovementSignal, OpportunityClass,
    Severity, SignalSource,
};
use uuid::Uuid;

/// Configuration for the Analyzer.
#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    /// Minimum confidence (0.0–1.0) to emit an opportunity.
    pub min_confidence: f64,
    /// Maximum age (seconds) for signals to be grouped together.
    pub grouping_window_secs: u64,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.3,
            grouping_window_secs: 300,
        }
    }
}

/// The Analyzer classifies, prioritizes, and scopes improvement opportunities.
pub struct Analyzer {
    config: AnalyzerConfig,
}

impl Analyzer {
    pub fn new(config: AnalyzerConfig) -> Self {
        Self { config }
    }

    /// Analyze signals and produce prioritized opportunities.
    pub fn analyze(&self, signals: &[ImprovementSignal]) -> Vec<ImprovementOpportunity> {
        let groups = self.group_signals(signals);

        groups
            .into_iter()
            .filter_map(|group| {
                let confidence = self.calculate_confidence(&group);
                if confidence < self.config.min_confidence {
                    return None;
                }

                Some(ImprovementOpportunity {
                    id: Uuid::new_v4(),
                    signal_ids: group.iter().map(|s| s.id).collect(),
                    domain: self.classify_domain(&group),
                    classification: self.classify_type(&group),
                    severity: self.assess_severity(&group),
                    blast_radius: self.assess_blast_radius(&group),
                    confidence,
                    estimated_impact: self.estimate_impact(&group),
                })
            })
            .collect()
    }

    /// Group related signals by domain and time window.
    fn group_signals<'a>(
        &self,
        signals: &'a [ImprovementSignal],
    ) -> Vec<Vec<&'a ImprovementSignal>> {
        // Group by domain, then by time proximity
        let mut groups: Vec<Vec<&ImprovementSignal>> = Vec::new();

        for signal in signals {
            let mut added = false;
            for group in &mut groups {
                if let Some(first) = group.first() {
                    if first.domain == signal.domain
                        && signal.timestamp.abs_diff(first.timestamp)
                            <= self.config.grouping_window_secs
                    {
                        group.push(signal);
                        added = true;
                        break;
                    }
                }
            }
            if !added {
                groups.push(vec![signal]);
            }
        }

        groups
    }

    fn classify_domain(&self, group: &[&ImprovementSignal]) -> ImprovementDomain {
        // Use majority domain from the group
        group
            .first()
            .map(|s| s.domain)
            .unwrap_or(ImprovementDomain::ConfigTuning)
    }

    fn classify_type(&self, group: &[&ImprovementSignal]) -> OpportunityClass {
        // Classify based on signal sources and metric names
        for signal in group {
            match signal.source {
                SignalSource::AnomalyMonitor => return OpportunityClass::Security,
                SignalSource::TestSuite => return OpportunityClass::Reliability,
                SignalSource::CapabilityMeasurement => return OpportunityClass::Quality,
                _ => {}
            }
            if signal.metric_name.contains("latency") || signal.metric_name.contains("throughput") {
                return OpportunityClass::Performance;
            }
        }
        OpportunityClass::Performance
    }

    fn assess_severity(&self, group: &[&ImprovementSignal]) -> Severity {
        let max_sigma = group
            .iter()
            .map(|s| s.deviation_sigma.abs())
            .fold(0.0_f64, f64::max);

        if max_sigma > 5.0 {
            Severity::Critical
        } else if max_sigma > 3.0 {
            Severity::High
        } else if max_sigma > 2.0 {
            Severity::Medium
        } else {
            Severity::Low
        }
    }

    fn assess_blast_radius(&self, group: &[&ImprovementSignal]) -> BlastRadius {
        // Multiple distinct metric names suggest broader impact
        let unique_metrics: std::collections::HashSet<&str> =
            group.iter().map(|s| s.metric_name.as_str()).collect();

        if unique_metrics.len() >= 5 {
            BlastRadius::Platform
        } else if unique_metrics.len() >= 2 {
            BlastRadius::Subsystem
        } else {
            BlastRadius::Agent
        }
    }

    fn calculate_confidence(&self, group: &[&ImprovementSignal]) -> f64 {
        if group.is_empty() {
            return 0.0;
        }

        // More signals = higher confidence; higher sigma = higher confidence
        let count_factor = (group.len() as f64).min(5.0) / 5.0;
        let sigma_factor = group
            .iter()
            .map(|s| (s.deviation_sigma.abs() / 5.0).min(1.0))
            .sum::<f64>()
            / group.len() as f64;

        // Multiple sources = higher confidence
        let unique_sources: std::collections::HashSet<_> = group.iter().map(|s| s.source).collect();
        let source_factor = (unique_sources.len() as f64).min(3.0) / 3.0;

        (count_factor * 0.3 + sigma_factor * 0.4 + source_factor * 0.3).min(1.0)
    }

    fn estimate_impact(&self, group: &[&ImprovementSignal]) -> f64 {
        // Average absolute deviation as a proxy for impact
        if group.is_empty() {
            return 0.0;
        }
        group.iter().map(|s| s.deviation_sigma.abs()).sum::<f64>() / group.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EvidenceItem;

    fn make_signal(
        domain: ImprovementDomain,
        sigma: f64,
        source: SignalSource,
    ) -> ImprovementSignal {
        ImprovementSignal {
            id: Uuid::new_v4(),
            timestamp: 1000,
            domain,
            source,
            metric_name: "test_metric".into(),
            current_value: 200.0,
            baseline_value: 100.0,
            deviation_sigma: sigma,
            evidence: vec![EvidenceItem {
                timestamp: 1000,
                description: "test".into(),
                data: serde_json::json!({}),
            }],
        }
    }

    #[test]
    fn test_analyzer_classifies_opportunity() {
        let analyzer = Analyzer::new(AnalyzerConfig::default());
        let signals = vec![make_signal(
            ImprovementDomain::ConfigTuning,
            3.0,
            SignalSource::PerformanceProfiler,
        )];
        let opps = analyzer.analyze(&signals);
        assert_eq!(opps.len(), 1);
        assert_eq!(opps[0].domain, ImprovementDomain::ConfigTuning);
    }

    #[test]
    fn test_analyzer_severity_assessment() {
        let analyzer = Analyzer::new(AnalyzerConfig::default());

        let mild = vec![make_signal(
            ImprovementDomain::ConfigTuning,
            2.5,
            SignalSource::PerformanceProfiler,
        )];
        let opps = analyzer.analyze(&mild);
        assert_eq!(opps[0].severity, Severity::Medium);

        let severe = vec![make_signal(
            ImprovementDomain::ConfigTuning,
            6.0,
            SignalSource::PerformanceProfiler,
        )];
        let opps = analyzer.analyze(&severe);
        assert_eq!(opps[0].severity, Severity::Critical);
    }

    #[test]
    fn test_analyzer_blast_radius_estimation() {
        let analyzer = Analyzer::new(AnalyzerConfig::default());
        // Single metric = Agent scope
        let signals = vec![make_signal(
            ImprovementDomain::ConfigTuning,
            3.0,
            SignalSource::PerformanceProfiler,
        )];
        let opps = analyzer.analyze(&signals);
        assert_eq!(opps[0].blast_radius, BlastRadius::Agent);
    }

    #[test]
    fn test_analyzer_confidence_calculation() {
        let analyzer = Analyzer::new(AnalyzerConfig {
            min_confidence: 0.0,
            ..Default::default()
        });

        // High sigma + single signal = moderate confidence
        let signals = vec![make_signal(
            ImprovementDomain::ConfigTuning,
            5.0,
            SignalSource::PerformanceProfiler,
        )];
        let opps = analyzer.analyze(&signals);
        assert!(opps[0].confidence > 0.2);
        assert!(opps[0].confidence < 1.0);
    }

    #[test]
    fn test_analyzer_filters_low_confidence() {
        let analyzer = Analyzer::new(AnalyzerConfig {
            min_confidence: 0.99,
            ..Default::default()
        });
        // Single weak signal won't meet 0.99 confidence
        let signals = vec![make_signal(
            ImprovementDomain::ConfigTuning,
            1.0,
            SignalSource::PerformanceProfiler,
        )];
        let opps = analyzer.analyze(&signals);
        assert!(opps.is_empty());
    }
}
