use crate::collector::{MetricRecord, Platform};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredPost {
    pub platform: Platform,
    pub content_id: String,
    pub score: u64,
    pub follower_growth: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionPerformance {
    pub dimension: String,
    pub average_score: f64,
    pub sample_size: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceEvaluation {
    pub top_posts: Vec<ScoredPost>,
    pub worst_posts: Vec<ScoredPost>,
    pub platform_performance: Vec<DimensionPerformance>,
    pub time_slot_performance: Vec<DimensionPerformance>,
    pub content_type_performance: Vec<DimensionPerformance>,
    pub recommendations: Vec<String>,
    pub llm_summary: String,
}

pub trait NarrativeAnalyzer {
    fn summarize(&self, prompt: &str) -> Result<String, AgentError>;
}

#[derive(Debug, Default)]
pub struct TemplateAnalyzer;

impl NarrativeAnalyzer for TemplateAnalyzer {
    fn summarize(&self, prompt: &str) -> Result<String, AgentError> {
        Ok(prompt.to_string())
    }
}

pub struct PerformanceEvaluator<A: NarrativeAnalyzer> {
    analyzer: A,
}

impl<A: NarrativeAnalyzer> PerformanceEvaluator<A> {
    pub fn new(analyzer: A) -> Self {
        Self { analyzer }
    }

    pub fn evaluate(&self, metrics: &[MetricRecord]) -> Result<PerformanceEvaluation, AgentError> {
        if metrics.is_empty() {
            return Err(AgentError::SupervisorError(
                "cannot evaluate performance with no metrics".to_string(),
            ));
        }

        let mut scored = metrics
            .iter()
            .map(|row| ScoredPost {
                platform: row.platform.clone(),
                content_id: row.content_id.clone(),
                score: engagement_score(row),
                follower_growth: row.follower_growth,
            })
            .collect::<Vec<_>>();

        scored.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.content_id.cmp(&right.content_id))
        });

        let top_posts = scored.iter().take(3).cloned().collect::<Vec<_>>();
        let mut worst_posts = scored.iter().rev().take(3).cloned().collect::<Vec<_>>();
        worst_posts.reverse();

        let platform_performance = aggregate_by(metrics, |row| platform_name(&row.platform));
        let time_slot_performance = aggregate_by(metrics, |row| row.time_slot.clone());
        let content_type_performance = aggregate_by(metrics, |row| row.content_type.clone());

        let recommendations = generate_recommendations(
            top_posts.as_slice(),
            platform_performance.as_slice(),
            time_slot_performance.as_slice(),
            content_type_performance.as_slice(),
        );

        let summary_prompt = format!(
            "Top post score: {}. Best platform: {}. Recommendations: {}",
            top_posts.first().map(|post| post.score).unwrap_or(0),
            platform_performance
                .first()
                .map(|row| row.dimension.as_str())
                .unwrap_or("unknown"),
            recommendations.join(" | ")
        );
        let llm_summary = self.analyzer.summarize(summary_prompt.as_str())?;

        Ok(PerformanceEvaluation {
            top_posts,
            worst_posts,
            platform_performance,
            time_slot_performance,
            content_type_performance,
            recommendations,
            llm_summary,
        })
    }
}

impl Default for PerformanceEvaluator<TemplateAnalyzer> {
    fn default() -> Self {
        Self {
            analyzer: TemplateAnalyzer,
        }
    }
}

fn engagement_score(metric: &MetricRecord) -> u64 {
    metric
        .like_count
        .saturating_add(metric.retweet_count.saturating_mul(2))
        .saturating_add(metric.reply_count)
        .saturating_add(metric.comment_count)
}

fn aggregate_by<F>(metrics: &[MetricRecord], key_fn: F) -> Vec<DimensionPerformance>
where
    F: Fn(&MetricRecord) -> String,
{
    let mut grouped = HashMap::<String, (u64, usize)>::new();
    for row in metrics {
        let key = key_fn(row);
        let entry = grouped.entry(key).or_insert((0, 0));
        entry.0 = entry.0.saturating_add(engagement_score(row));
        entry.1 = entry.1.saturating_add(1);
    }

    let mut rows = grouped
        .into_iter()
        .map(|(dimension, (sum, count))| DimensionPerformance {
            dimension,
            average_score: if count == 0 {
                0.0
            } else {
                (sum as f64) / (count as f64)
            },
            sample_size: count,
        })
        .collect::<Vec<_>>();

    rows.sort_by(|left, right| {
        right
            .average_score
            .total_cmp(&left.average_score)
            .then_with(|| left.dimension.cmp(&right.dimension))
    });
    rows
}

fn generate_recommendations(
    top_posts: &[ScoredPost],
    platform_performance: &[DimensionPerformance],
    time_slot_performance: &[DimensionPerformance],
    content_type_performance: &[DimensionPerformance],
) -> Vec<String> {
    let mut recommendations = Vec::new();

    if let Some(best_platform) = platform_performance.first() {
        recommendations.push(format!(
            "Increase publishing cadence on {} where average engagement is {:.1}",
            best_platform.dimension, best_platform.average_score
        ));
    }

    if let Some(best_time_slot) = time_slot_performance.first() {
        recommendations.push(format!(
            "Prioritize {} time slots which currently outperform alternatives",
            best_time_slot.dimension
        ));
    }

    if let Some(best_content_type) = content_type_performance.first() {
        recommendations.push(format!(
            "Double down on '{}' content style",
            best_content_type.dimension
        ));
    }

    if top_posts.iter().all(|post| post.follower_growth <= 0) {
        recommendations.push(
            "Engagement is not translating to follower growth; add explicit follow CTAs"
                .to_string(),
        );
    }

    recommendations
}

fn platform_name(platform: &Platform) -> String {
    match platform {
        Platform::X => "x".to_string(),
        Platform::Facebook => "facebook".to_string(),
        Platform::Instagram => "instagram".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{PerformanceEvaluator, TemplateAnalyzer};
    use crate::collector::{MetricRecord, Platform};

    #[test]
    fn test_evaluation_builds_recommendations() {
        let metrics = vec![
            MetricRecord {
                platform: Platform::X,
                content_id: "tweet-1".to_string(),
                like_count: 100,
                retweet_count: 40,
                reply_count: 20,
                comment_count: 20,
                follower_growth: 10,
                content_type: "tutorial".to_string(),
                time_slot: "morning".to_string(),
                collected_at: 10,
            },
            MetricRecord {
                platform: Platform::Instagram,
                content_id: "ig-1".to_string(),
                like_count: 50,
                retweet_count: 0,
                reply_count: 10,
                comment_count: 10,
                follower_growth: 3,
                content_type: "carousel".to_string(),
                time_slot: "evening".to_string(),
                collected_at: 10,
            },
        ];

        let evaluator = PerformanceEvaluator::new(TemplateAnalyzer);
        let result = evaluator.evaluate(metrics.as_slice());
        assert!(result.is_ok());

        if let Ok(report) = result {
            assert!(!report.top_posts.is_empty());
            assert!(!report.recommendations.is_empty());
        }
    }
}
