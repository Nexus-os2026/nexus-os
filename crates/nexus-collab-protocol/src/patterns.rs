use serde::{Deserialize, Serialize};

use crate::roles::CollaborationRole;

/// Pre-defined collaboration patterns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaborationPattern {
    PeerReview,
    Debate,
    Brainstorm,
    ExpertPanel,
    Pipeline,
    RedTeam,
    Custom { name: String },
}

impl CollaborationPattern {
    pub fn recommended_roles(&self) -> Vec<(String, CollaborationRole)> {
        match self {
            Self::PeerReview => vec![
                ("proposer".into(), CollaborationRole::Lead),
                ("reviewer_1".into(), CollaborationRole::Reviewer),
                ("reviewer_2".into(), CollaborationRole::Reviewer),
            ],
            Self::Debate => vec![
                ("moderator".into(), CollaborationRole::Lead),
                (
                    "proponent".into(),
                    CollaborationRole::Expert {
                        domain: "for".into(),
                    },
                ),
                (
                    "opponent".into(),
                    CollaborationRole::Expert {
                        domain: "against".into(),
                    },
                ),
            ],
            Self::Brainstorm => vec![
                ("facilitator".into(), CollaborationRole::Lead),
                ("contributor_1".into(), CollaborationRole::Contributor),
                ("contributor_2".into(), CollaborationRole::Contributor),
                ("contributor_3".into(), CollaborationRole::Contributor),
            ],
            Self::ExpertPanel => vec![
                ("moderator".into(), CollaborationRole::Lead),
                (
                    "expert_1".into(),
                    CollaborationRole::Expert {
                        domain: "domain_1".into(),
                    },
                ),
                (
                    "expert_2".into(),
                    CollaborationRole::Expert {
                        domain: "domain_2".into(),
                    },
                ),
                (
                    "expert_3".into(),
                    CollaborationRole::Expert {
                        domain: "domain_3".into(),
                    },
                ),
            ],
            Self::Pipeline => vec![
                ("coordinator".into(), CollaborationRole::Lead),
                (
                    "stage_1".into(),
                    CollaborationRole::Expert {
                        domain: "stage_1".into(),
                    },
                ),
                (
                    "stage_2".into(),
                    CollaborationRole::Expert {
                        domain: "stage_2".into(),
                    },
                ),
            ],
            Self::RedTeam => vec![
                ("lead".into(), CollaborationRole::Lead),
                (
                    "builder".into(),
                    CollaborationRole::Expert {
                        domain: "building".into(),
                    },
                ),
                (
                    "attacker".into(),
                    CollaborationRole::Expert {
                        domain: "security".into(),
                    },
                ),
            ],
            Self::Custom { .. } => vec![
                ("lead".into(), CollaborationRole::Lead),
                ("member".into(), CollaborationRole::Contributor),
            ],
        }
    }

    pub fn recommended_majority(&self) -> f64 {
        match self {
            Self::RedTeam => 1.0,
            Self::ExpertPanel => 0.67,
            _ => 0.5,
        }
    }

    pub fn max_participants(&self) -> usize {
        match self {
            Self::PeerReview => 5,
            Self::Debate => 4,
            Self::Brainstorm => 8,
            Self::ExpertPanel => 6,
            Self::Pipeline => 10,
            Self::RedTeam => 6,
            Self::Custom { .. } => 10,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_recommended_roles() {
        let roles = CollaborationPattern::PeerReview.recommended_roles();
        assert_eq!(roles.len(), 3);
        assert_eq!(roles[0].1, CollaborationRole::Lead);

        let debate_roles = CollaborationPattern::Debate.recommended_roles();
        assert_eq!(debate_roles.len(), 3);

        let brainstorm = CollaborationPattern::Brainstorm.recommended_roles();
        assert_eq!(brainstorm.len(), 4);
    }

    #[test]
    fn test_pattern_max_participants() {
        assert_eq!(CollaborationPattern::PeerReview.max_participants(), 5);
        assert_eq!(CollaborationPattern::Brainstorm.max_participants(), 8);
        assert_eq!(CollaborationPattern::Pipeline.max_participants(), 10);
        assert!(CollaborationPattern::Debate.max_participants() >= 3);
    }
}
