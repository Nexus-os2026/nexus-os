use serde::{Deserialize, Serialize};

pub const FACTORY_CAPABILITY: &str = "software_factory";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactoryPolicy {
    pub min_autonomy_level: u8,
    pub max_concurrent_projects: usize,
    pub max_team_size: usize,
    pub require_review_stage: bool,
    pub require_simulation_before_deploy: bool,
    pub auto_advance_on_gate_pass: bool,
}

impl Default for FactoryPolicy {
    fn default() -> Self {
        Self {
            min_autonomy_level: 3,
            max_concurrent_projects: 3,
            max_team_size: 8,
            require_review_stage: true,
            require_simulation_before_deploy: true,
            auto_advance_on_gate_pass: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_defaults() {
        let p = FactoryPolicy::default();
        assert_eq!(p.min_autonomy_level, 3);
        assert_eq!(p.max_concurrent_projects, 3);
        assert!(p.require_review_stage);
        assert!(p.auto_advance_on_gate_pass);
    }
}
