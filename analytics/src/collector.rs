use nexus_connectors_social::facebook::FacebookPageMetrics;
use nexus_connectors_social::instagram::InstagramMetrics;
use nexus_connectors_web::twitter::EngagementMetrics;
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Platform {
    X,
    Facebook,
    Instagram,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawMetric {
    pub content_id: String,
    pub like_count: u64,
    pub retweet_count: u64,
    pub reply_count: u64,
    pub comment_count: u64,
    pub follower_growth: i64,
    pub content_type: String,
    pub time_slot: String,
}

impl RawMetric {
    pub fn from_twitter(
        content_id: &str,
        metrics: EngagementMetrics,
        follower_growth: i64,
    ) -> Self {
        Self {
            content_id: content_id.to_string(),
            like_count: metrics.likes,
            retweet_count: metrics.retweets,
            reply_count: metrics.replies,
            comment_count: metrics.replies,
            follower_growth,
            content_type: "post".to_string(),
            time_slot: "unknown".to_string(),
        }
    }

    pub fn from_instagram(
        content_id: &str,
        metrics: InstagramMetrics,
        follower_growth: i64,
    ) -> Self {
        Self {
            content_id: content_id.to_string(),
            like_count: metrics.likes,
            retweet_count: 0,
            reply_count: metrics.comments,
            comment_count: metrics.comments,
            follower_growth,
            content_type: "post".to_string(),
            time_slot: "unknown".to_string(),
        }
    }

    pub fn from_facebook(
        page_id: &str,
        metrics: FacebookPageMetrics,
        follower_growth: i64,
    ) -> Self {
        Self {
            content_id: page_id.to_string(),
            like_count: metrics.engagement,
            retweet_count: 0,
            reply_count: metrics.engagement / 5,
            comment_count: metrics.engagement / 4,
            follower_growth,
            content_type: "post".to_string(),
            time_slot: "unknown".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricRecord {
    pub platform: Platform,
    pub content_id: String,
    pub like_count: u64,
    pub retweet_count: u64,
    pub reply_count: u64,
    pub comment_count: u64,
    pub follower_growth: i64,
    pub content_type: String,
    pub time_slot: String,
    pub collected_at: u64,
}

pub trait PlatformMetricsProvider {
    fn id(&self) -> &str;
    fn platform(&self) -> Platform;
    fn min_poll_interval_secs(&self) -> u64;
    fn poll_metrics(&mut self) -> Result<Vec<RawMetric>, AgentError>;
}

struct ProviderRegistration {
    provider: Box<dyn PlatformMetricsProvider>,
    last_poll_timestamp: Option<u64>,
}

pub struct MetricsCollector {
    providers: HashMap<String, ProviderRegistration>,
    storage: Vec<MetricRecord>,
    clock: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            storage: Vec::new(),
            clock: Arc::new(current_unix_timestamp),
        }
    }

    pub fn with_clock(clock: Arc<dyn Fn() -> u64 + Send + Sync>) -> Self {
        Self {
            providers: HashMap::new(),
            storage: Vec::new(),
            clock,
        }
    }

    pub fn register_provider(
        &mut self,
        provider: Box<dyn PlatformMetricsProvider>,
    ) -> Result<(), AgentError> {
        let id = provider.id().to_string();
        if self.providers.contains_key(&id) {
            return Err(AgentError::SupervisorError(format!(
                "metrics provider '{id}' is already registered"
            )));
        }

        self.providers.insert(
            id,
            ProviderRegistration {
                provider,
                last_poll_timestamp: None,
            },
        );
        Ok(())
    }

    pub fn collect_scheduled(&mut self) -> Result<Vec<MetricRecord>, AgentError> {
        let now = (self.clock)();
        let mut provider_ids = self.providers.keys().cloned().collect::<Vec<_>>();
        provider_ids.sort();

        let mut collected = Vec::new();
        for provider_id in provider_ids {
            let Some(registration) = self.providers.get_mut(provider_id.as_str()) else {
                continue;
            };

            let min_interval = registration.provider.min_poll_interval_secs();
            let should_poll = match registration.last_poll_timestamp {
                Some(last_poll) => now.saturating_sub(last_poll) >= min_interval,
                None => true,
            };

            if !should_poll {
                continue;
            }

            let platform = registration.provider.platform();
            let raw_metrics = registration.provider.poll_metrics()?;
            registration.last_poll_timestamp = Some(now);

            for metric in raw_metrics {
                let record = MetricRecord {
                    platform: platform.clone(),
                    content_id: metric.content_id,
                    like_count: metric.like_count,
                    retweet_count: metric.retweet_count,
                    reply_count: metric.reply_count,
                    comment_count: metric.comment_count,
                    follower_growth: metric.follower_growth,
                    content_type: metric.content_type,
                    time_slot: metric.time_slot,
                    collected_at: now,
                };
                self.storage.push(record.clone());
                collected.push(record);
            }
        }

        Ok(collected)
    }

    pub fn stored_metrics(&self) -> &[MetricRecord] {
        &self.storage
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

fn current_unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{MetricRecord, MetricsCollector, Platform, PlatformMetricsProvider, RawMetric};
    use nexus_connectors_web::twitter::EngagementMetrics;
    use nexus_kernel::errors::AgentError;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    struct MockTwitterProvider;

    impl PlatformMetricsProvider for MockTwitterProvider {
        fn id(&self) -> &str {
            "x-metrics"
        }

        fn platform(&self) -> Platform {
            Platform::X
        }

        fn min_poll_interval_secs(&self) -> u64 {
            60
        }

        fn poll_metrics(&mut self) -> Result<Vec<RawMetric>, AgentError> {
            let mut rows = Vec::new();
            for idx in 0..5 {
                rows.push(RawMetric::from_twitter(
                    format!("tweet-{idx}").as_str(),
                    EngagementMetrics {
                        likes: 100 + idx,
                        retweets: 50 + idx,
                        replies: 25 + idx,
                    },
                    5,
                ));
            }
            Ok(rows)
        }
    }

    #[test]
    fn test_metrics_collection() {
        let now = Arc::new(AtomicU64::new(1_000));
        let now_for_clock = Arc::clone(&now);
        let mut collector =
            MetricsCollector::with_clock(Arc::new(move || now_for_clock.load(Ordering::SeqCst)));

        let register_result = collector.register_provider(Box::new(MockTwitterProvider));
        assert!(register_result.is_ok());

        let first_collect = collector.collect_scheduled();
        assert!(first_collect.is_ok());

        if let Ok(metrics) = first_collect {
            assert_eq!(metrics.len(), 5);
            for row in &metrics {
                assert!(row.like_count >= 100);
                assert!(row.retweet_count >= 50);
                assert!(row.reply_count >= 25);
            }
        }

        let second_collect = collector.collect_scheduled();
        assert!(second_collect.is_ok());
        if let Ok(metrics) = second_collect {
            assert_eq!(metrics.len(), 0);
        }

        now.store(1_061, Ordering::SeqCst);
        let third_collect = collector.collect_scheduled();
        assert!(third_collect.is_ok());
        if let Ok(metrics) = third_collect {
            assert_eq!(metrics.len(), 5);
        }

        let stored = collector.stored_metrics();
        assert_eq!(stored.len(), 10);
        assert!(stored
            .iter()
            .all(|MetricRecord { platform, .. }| *platform == Platform::X));
    }
}
