use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeClass {
    StrategyEdit,
    AuthorityChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrategyChange {
    PostingTimeUpdate { from: String, to: String },
    ContentStyleUpdate { from: String, to: String },
    HashtagUpdate { added: Vec<String> },
    AddPlatform { platform: String },
    IncreaseBudget { from: u64, to: u64 },
    AddCapability { capability: String },
    FuelOverride { from: u64, to: u64 },
    AuditBypassRequested,
}

pub fn classify_change(change: &StrategyChange) -> ChangeClass {
    match change {
        StrategyChange::PostingTimeUpdate { .. }
        | StrategyChange::ContentStyleUpdate { .. }
        | StrategyChange::HashtagUpdate { .. } => ChangeClass::StrategyEdit,
        StrategyChange::AddPlatform { .. }
        | StrategyChange::IncreaseBudget { .. }
        | StrategyChange::AddCapability { .. }
        | StrategyChange::FuelOverride { .. }
        | StrategyChange::AuditBypassRequested => ChangeClass::AuthorityChange,
    }
}

pub fn is_never_allowed(change: &StrategyChange) -> bool {
    matches!(
        change,
        StrategyChange::AddCapability { .. }
            | StrategyChange::FuelOverride { .. }
            | StrategyChange::AuditBypassRequested
    )
}

#[cfg(test)]
mod tests {
    use super::{classify_change, is_never_allowed, ChangeClass, StrategyChange};

    #[test]
    fn test_classify_change() {
        let edit = StrategyChange::PostingTimeUpdate {
            from: "2pm".to_string(),
            to: "9am".to_string(),
        };
        let authority = StrategyChange::AddPlatform {
            platform: "instagram".to_string(),
        };

        assert_eq!(classify_change(&edit), ChangeClass::StrategyEdit);
        assert_eq!(classify_change(&authority), ChangeClass::AuthorityChange);
        assert!(is_never_allowed(&StrategyChange::AddCapability {
            capability: "social.instagram.post".to_string()
        }));
    }
}
