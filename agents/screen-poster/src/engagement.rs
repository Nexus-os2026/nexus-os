use crate::navigator::SocialPlatform;
use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngagementSnapshot {
    pub offset_hours: u64,
    pub likes: u64,
    pub comments: u64,
    pub shares: u64,
    pub views: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngagementReport {
    pub platform: SocialPlatform,
    pub post_url: String,
    pub snapshots: Vec<EngagementSnapshot>,
}

pub trait EngagementVision {
    fn read_metric_text(&mut self, metric: &str) -> Result<String, AgentError>;
}

pub struct EngagementTracker<V: EngagementVision> {
    vision: V,
}

impl<V: EngagementVision> EngagementTracker<V> {
    pub fn new(vision: V) -> Self {
        Self { vision }
    }

    pub fn track_over_time(
        &mut self,
        post_url: &str,
        platform: SocialPlatform,
        checkpoints_hours: &[u64],
    ) -> Result<EngagementReport, AgentError> {
        let mut snapshots = Vec::new();
        for checkpoint in checkpoints_hours {
            let likes = parse_metric(self.vision.read_metric_text("likes")?.as_str());
            let comments = parse_metric(self.vision.read_metric_text("comments")?.as_str());
            let shares = parse_metric(self.vision.read_metric_text("shares")?.as_str());
            let views = parse_metric(self.vision.read_metric_text("views")?.as_str());
            snapshots.push(EngagementSnapshot {
                offset_hours: *checkpoint,
                likes,
                comments,
                shares,
                views,
            });
        }

        Ok(EngagementReport {
            platform,
            post_url: post_url.to_string(),
            snapshots,
        })
    }
}

fn parse_metric(raw: &str) -> u64 {
    let trimmed = raw.trim().to_ascii_uppercase();
    if trimmed.is_empty() {
        return 0;
    }

    let multiplier = if let Some(number) = trimmed.strip_suffix('K') {
        return parse_decimal(number, 1_000.0);
    } else if let Some(number) = trimmed.strip_suffix('M') {
        return parse_decimal(number, 1_000_000.0);
    } else {
        1.0
    };

    parse_decimal(trimmed.as_str(), multiplier)
}

fn parse_decimal(raw: &str, multiplier: f64) -> u64 {
    let digits = raw
        .chars()
        .filter(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect::<String>();
    if digits.is_empty() {
        return 0;
    }
    let value = digits.parse::<f64>().unwrap_or(0.0);
    (value * multiplier).round() as u64
}
