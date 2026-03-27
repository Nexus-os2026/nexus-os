use crate::governance::CollaborationPolicy;

pub struct CollaborationEconomy;

impl CollaborationEconomy {
    pub fn session_cost(policy: &CollaborationPolicy) -> u64 {
        policy.session_creation_cost
    }

    pub fn message_cost(policy: &CollaborationPolicy) -> u64 {
        policy.message_cost
    }

    pub fn vote_cost(policy: &CollaborationPolicy) -> u64 {
        policy.vote_cost
    }

    pub fn estimate_total(
        policy: &CollaborationPolicy,
        participants: usize,
        estimated_messages: usize,
    ) -> u64 {
        policy.session_creation_cost
            + (policy.message_cost * estimated_messages as u64)
            + (policy.vote_cost * participants as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_economy_cost_estimation() {
        let policy = CollaborationPolicy::default();
        let total = CollaborationEconomy::estimate_total(&policy, 4, 20);
        let expected =
            policy.session_creation_cost + (policy.message_cost * 20) + (policy.vote_cost * 4);
        assert_eq!(total, expected);
    }
}
