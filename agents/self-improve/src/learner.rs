use crate::tracker::{OutcomeResult, PerformanceTracker, TaskType};
use nexus_connectors_llm::gateway::{
    select_provider, AgentRuntimeContext, GovernedLlmGateway, ProviderSelectionConfig,
};
use nexus_connectors_llm::providers::LlmProvider;
use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyInsights {
    pub agent_id: String,
    pub task_type: TaskType,
    pub summary: String,
    pub recommendations: Vec<String>,
    pub signals: BTreeMap<String, f64>,
}

pub struct StrategyLearner {
    gateway: GovernedLlmGateway<Box<dyn LlmProvider>>,
    runtime: AgentRuntimeContext,
}

impl Default for StrategyLearner {
    fn default() -> Self {
        Self::new()
    }
}

impl StrategyLearner {
    pub fn new() -> Self {
        let config = ProviderSelectionConfig::from_env();
        let provider: Box<dyn LlmProvider> = select_provider(&config).unwrap_or_else(|_| {
            Box::new(nexus_connectors_llm::providers::OllamaProvider::from_env())
        });
        let gateway = GovernedLlmGateway::new(provider);
        let capabilities = ["llm.query".to_string()]
            .into_iter()
            .collect::<HashSet<_>>();
        let runtime = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities,
            fuel_remaining: 6_000,
        };
        Self { gateway, runtime }
    }

    pub fn analyze_history(
        &mut self,
        tracker: &PerformanceTracker,
        agent_id: &str,
        task_type: TaskType,
    ) -> Result<StrategyInsights, AgentError> {
        let history = tracker.outcomes_for(agent_id, task_type);
        let summary_prompt = format!(
            "Analyze {} outcomes for task type {:?} and produce actionable patterns.",
            history.len(),
            task_type
        );
        let llm = self
            .gateway
            .query(&mut self.runtime, summary_prompt.as_str(), 220, "mock-1")?;

        let mut recommendations = Vec::new();
        let mut signals = BTreeMap::new();

        match task_type {
            TaskType::Posting => {
                let mut morning = Vec::<f64>::new();
                let mut afternoon = Vec::<f64>::new();
                for outcome in &history {
                    let Some(engagement) = outcome.metrics.engagement_rate else {
                        continue;
                    };
                    if let Some(hour) = parse_hour(outcome.task.as_str()) {
                        if hour < 10 {
                            morning.push(engagement);
                        } else if hour >= 12 {
                            afternoon.push(engagement);
                        }
                    }
                }

                let morning_avg = average(morning.as_slice());
                let afternoon_avg = average(afternoon.as_slice());
                signals.insert("morning_engagement".to_string(), morning_avg);
                signals.insert("afternoon_engagement".to_string(), afternoon_avg);

                if morning_avg > afternoon_avg * 1.5 && morning_avg > 0.0 {
                    recommendations.push(
                        "Post before 10am for higher engagement based on recent outcomes."
                            .to_string(),
                    );
                }

                if recommendations.is_empty() {
                    recommendations.push(
                        "Test posting times with a morning and afternoon A/B split for 2 weeks."
                            .to_string(),
                    );
                }
            }
            TaskType::Coding => {
                let failure_count = history
                    .iter()
                    .filter(|outcome| outcome.result == OutcomeResult::Failure)
                    .count() as f64;
                let average_iterations = average(
                    history
                        .iter()
                        .filter_map(|outcome| outcome.metrics.fix_iterations)
                        .collect::<Vec<_>>()
                        .as_slice(),
                );
                signals.insert("failures".to_string(), failure_count);
                signals.insert("avg_fix_iterations".to_string(), average_iterations);

                if average_iterations > 2.0 {
                    recommendations.push(
                        "Always add explicit error handling and regression tests when modifying Rust code."
                            .to_string(),
                    );
                }
                if failure_count > 0.0 {
                    recommendations.push(
                        "Run fast targeted tests before full workspace runs to catch failures earlier."
                            .to_string(),
                    );
                }
            }
            TaskType::Website => {
                let build_success_rate = average(
                    history
                        .iter()
                        .filter_map(|outcome| outcome.metrics.build_success)
                        .collect::<Vec<_>>()
                        .as_slice(),
                );
                let satisfaction = average(
                    history
                        .iter()
                        .filter_map(|outcome| outcome.metrics.user_satisfaction)
                        .collect::<Vec<_>>()
                        .as_slice(),
                );
                signals.insert("build_success_rate".to_string(), build_success_rate);
                signals.insert("user_satisfaction".to_string(), satisfaction);

                if satisfaction >= 0.75 {
                    recommendations.push(
                        "Reuse high-performing layouts and visual motifs for similar website briefs."
                            .to_string(),
                    );
                } else {
                    recommendations.push(
                        "Simplify layout density and improve first-contentful paint for better satisfaction."
                            .to_string(),
                    );
                }
            }
            TaskType::Other => {
                recommendations.push(
                    "Collect at least 20 outcomes to establish statistically useful strategy trends."
                        .to_string(),
                );
            }
        }

        Ok(StrategyInsights {
            agent_id: agent_id.to_string(),
            task_type,
            summary: llm.output_text,
            recommendations,
            signals,
        })
    }
}

pub fn analyze_history(
    tracker: &PerformanceTracker,
    agent_id: &str,
    task_type: TaskType,
) -> Result<StrategyInsights, AgentError> {
    let mut learner = StrategyLearner::new();
    learner.analyze_history(tracker, agent_id, task_type)
}

fn parse_hour(task: &str) -> Option<u8> {
    let lower = task.to_ascii_lowercase();
    if lower.contains("9am") || lower.contains("09:00") {
        return Some(9);
    }
    if lower.contains("3pm") || lower.contains("15:00") {
        return Some(15);
    }

    for token in lower.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != ':') {
        if token.len() == 5 && token.chars().nth(2) == Some(':') {
            let hour = token[..2].parse::<u8>().ok()?;
            if hour < 24 {
                return Some(hour);
            }
        }
    }
    None
}

fn average(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}
