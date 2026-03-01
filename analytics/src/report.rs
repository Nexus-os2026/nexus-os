use crate::collector::{MetricRecord, Platform};
use crate::evaluator::{PerformanceEvaluation, ScoredPost};
use nexus_kernel::errors::AgentError;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportWindow {
    Weekly,
    Monthly,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalyticsReport {
    pub window: ReportWindow,
    pub generated_at: u64,
    pub top_posts: Vec<ScoredPost>,
    pub worst_posts: Vec<ScoredPost>,
    pub growth_trends: Vec<String>,
    pub recommendations: Vec<String>,
    pub llm_summary: String,
}

pub struct ReportGenerator {
    clock: Box<dyn Fn() -> u64 + Send + Sync>,
}

impl ReportGenerator {
    pub fn new(clock: Box<dyn Fn() -> u64 + Send + Sync>) -> Self {
        Self { clock }
    }

    pub fn generate(
        &self,
        window: ReportWindow,
        metrics: &[MetricRecord],
        evaluation: &PerformanceEvaluation,
    ) -> AnalyticsReport {
        AnalyticsReport {
            window,
            generated_at: (self.clock)(),
            top_posts: evaluation.top_posts.clone(),
            worst_posts: evaluation.worst_posts.clone(),
            growth_trends: compute_growth_trends(metrics),
            recommendations: evaluation.recommendations.clone(),
            llm_summary: evaluation.llm_summary.clone(),
        }
    }

    pub fn render_dashboard(&self, report: &AnalyticsReport) -> Result<String, AgentError> {
        let payload = serde_json::to_string_pretty(report)
            .map_err(|error| AgentError::SupervisorError(format!("failed to serialize report: {error}")))?;
        Ok(redact_sensitive(payload.as_str()))
    }
}

impl Default for ReportGenerator {
    fn default() -> Self {
        Self {
            clock: Box::new(current_unix_timestamp),
        }
    }
}

fn compute_growth_trends(metrics: &[MetricRecord]) -> Vec<String> {
    let mut grouped = HashMap::<Platform, i64>::new();
    for metric in metrics {
        let entry = grouped.entry(metric.platform.clone()).or_insert(0);
        *entry += metric.follower_growth;
    }

    let mut rows = grouped
        .into_iter()
        .map(|(platform, growth)| {
            format!(
                "{} follower_growth={} over {} observations",
                platform_label(&platform),
                growth,
                metrics.iter().filter(|row| row.platform == platform).count()
            )
        })
        .collect::<Vec<_>>();

    rows.sort();
    rows
}

fn platform_label(platform: &Platform) -> &'static str {
    match platform {
        Platform::X => "x",
        Platform::Facebook => "facebook",
        Platform::Instagram => "instagram",
    }
}

fn redact_sensitive(input: &str) -> String {
    let mut output = input.to_string();

    let patterns = [
        (r#"(?i)(api[_-]?key\s*[:=]\s*)([^\s",]+)"#, "$1[REDACTED]"),
        (r#"(?i)(token\s*[:=]\s*)([^\s",]+)"#, "$1[REDACTED]"),
        (r#"(?i)(secret\s*[:=]\s*)([^\s",]+)"#, "$1[REDACTED]"),
        (r#"(?i)(bearer\s+)([A-Za-z0-9._\-]+)"#, "$1[REDACTED]"),
        (r#"\bghp_[A-Za-z0-9]+\b"#, "[REDACTED]"),
        (r#"\bsk-[A-Za-z0-9\-_]+\b"#, "[REDACTED]"),
        (r#"\bxoxb-[A-Za-z0-9\-]+\b"#, "[REDACTED]"),
    ];

    for (pattern, replacement) in patterns {
        if let Ok(regex) = Regex::new(pattern) {
            output = regex.replace_all(output.as_str(), replacement).to_string();
        }
    }

    output
}

fn current_unix_timestamp() -> u64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{AnalyticsReport, ReportGenerator, ReportWindow};
    use crate::collector::{MetricRecord, Platform};
    use crate::evaluator::{DimensionPerformance, PerformanceEvaluation, ScoredPost};

    fn sample_metrics() -> Vec<MetricRecord> {
        vec![
            MetricRecord {
                platform: Platform::X,
                content_id: "tweet-1".to_string(),
                like_count: 120,
                retweet_count: 45,
                reply_count: 30,
                comment_count: 30,
                follower_growth: 8,
                content_type: "tutorial".to_string(),
                time_slot: "morning".to_string(),
                collected_at: 100,
            },
            MetricRecord {
                platform: Platform::Instagram,
                content_id: "ig-1".to_string(),
                like_count: 90,
                retweet_count: 0,
                reply_count: 20,
                comment_count: 20,
                follower_growth: 12,
                content_type: "carousel".to_string(),
                time_slot: "evening".to_string(),
                collected_at: 100,
            },
        ]
    }

    fn sample_evaluation() -> PerformanceEvaluation {
        PerformanceEvaluation {
            top_posts: vec![ScoredPost {
                platform: Platform::X,
                content_id: "tweet-1".to_string(),
                score: 300,
                follower_growth: 8,
            }],
            worst_posts: vec![ScoredPost {
                platform: Platform::Instagram,
                content_id: "ig-1".to_string(),
                score: 130,
                follower_growth: 12,
            }],
            platform_performance: vec![DimensionPerformance {
                dimension: "x".to_string(),
                average_score: 300.0,
                sample_size: 1,
            }],
            time_slot_performance: vec![DimensionPerformance {
                dimension: "morning".to_string(),
                average_score: 300.0,
                sample_size: 1,
            }],
            content_type_performance: vec![DimensionPerformance {
                dimension: "tutorial".to_string(),
                average_score: 300.0,
                sample_size: 1,
            }],
            recommendations: vec![
                "Post more tutorials".to_string(),
                "api_key=sk-live-12345 should never appear".to_string(),
            ],
            llm_summary: "token=ghp_abc123 insights".to_string(),
        }
    }

    #[test]
    fn test_report_generation() {
        let generator = ReportGenerator::new(Box::new(|| 123_456));
        let report = generator.generate(
            ReportWindow::Weekly,
            sample_metrics().as_slice(),
            &sample_evaluation(),
        );

        let rendered = generator.render_dashboard(&report);
        assert!(rendered.is_ok());

        if let Ok(text) = rendered {
            assert!(text.contains("\"top_posts\""));
            assert!(text.contains("\"recommendations\""));
        }
    }

    #[test]
    fn test_dashboard_redaction() {
        let generator = ReportGenerator::new(Box::new(|| 999));
        let report = generator.generate(
            ReportWindow::Monthly,
            sample_metrics().as_slice(),
            &sample_evaluation(),
        );

        let rendered = generator.render_dashboard(&report);
        assert!(rendered.is_ok());

        if let Ok(text) = rendered {
            assert!(!text.contains("sk-live-12345"));
            assert!(!text.contains("ghp_abc123"));
            assert!(text.contains("[REDACTED]"));
        }
    }

    #[test]
    fn test_report_roundtrip_serialization() {
        let report = AnalyticsReport {
            window: ReportWindow::Weekly,
            generated_at: 1,
            top_posts: Vec::new(),
            worst_posts: Vec::new(),
            growth_trends: vec!["x follower_growth=2".to_string()],
            recommendations: vec!["Ship weekly report".to_string()],
            llm_summary: "stable".to_string(),
        };

        let json = serde_json::to_string(&report);
        assert!(json.is_ok());

        if let Ok(serialized) = json {
            let parsed = serde_json::from_str::<AnalyticsReport>(serialized.as_str());
            assert!(parsed.is_ok());
            if let Ok(parsed) = parsed {
                assert_eq!(parsed.window, ReportWindow::Weekly);
            }
        }
    }
}
