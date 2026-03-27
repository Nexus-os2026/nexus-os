use crate::governance::PerceptionPolicy;
use crate::vision::PerceptionTask;

pub struct PerceptionEconomy;

impl PerceptionEconomy {
    pub fn calculate_burn(task: &PerceptionTask, policy: &PerceptionPolicy) -> u64 {
        let base = policy.cost_per_perception;
        let multiplier = match task {
            PerceptionTask::Describe => 1.0,
            PerceptionTask::ExtractText => 1.5,
            PerceptionTask::VisualQuestion { .. } => 1.0,
            PerceptionTask::IdentifyUIElements => 1.5,
            PerceptionTask::ExtractStructuredData { .. } => 2.0,
            PerceptionTask::Compare { .. } => 2.5,
            PerceptionTask::ReadErrorMessage => 0.5,
            PerceptionTask::AnalyzeChart => 1.5,
            PerceptionTask::ReadDocument => 2.0,
        };
        (base as f64 * multiplier) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_economy_base_cost() {
        let policy = PerceptionPolicy::default();
        let cost = PerceptionEconomy::calculate_burn(&PerceptionTask::Describe, &policy);
        assert_eq!(cost, policy.cost_per_perception);
    }

    #[test]
    fn test_economy_expensive_tasks() {
        let policy = PerceptionPolicy::default();
        let base = policy.cost_per_perception;
        let cost = PerceptionEconomy::calculate_burn(
            &PerceptionTask::ExtractStructuredData { schema: None },
            &policy,
        );
        assert_eq!(cost, (base as f64 * 2.0) as u64);
    }

    #[test]
    fn test_economy_cheap_tasks() {
        let policy = PerceptionPolicy::default();
        let base = policy.cost_per_perception;
        let cost = PerceptionEconomy::calculate_burn(&PerceptionTask::ReadErrorMessage, &policy);
        assert_eq!(cost, (base as f64 * 0.5) as u64);
    }
}
