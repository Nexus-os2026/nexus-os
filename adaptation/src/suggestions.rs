use crate::preferences::{ApprovalRecord, Weekday};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    pub suggestion_id: Uuid,
    pub user_id: String,
    pub message: String,
    pub requires_opt_in: bool,
    pub opted_in: bool,
}

#[derive(Debug, Default)]
pub struct ProactiveSuggestionEngine {
    opted_in: HashMap<String, Vec<Uuid>>,
}

impl ProactiveSuggestionEngine {
    pub fn new() -> Self {
        Self {
            opted_in: HashMap::new(),
        }
    }

    pub fn generate_suggestions(
        &self,
        user_id: &str,
        approvals: &[ApprovalRecord],
    ) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        if has_monday_streak(approvals, 4) {
            suggestions.push(Suggestion {
                suggestion_id: deterministic_suggestion_id(user_id, "monday-auto-schedule"),
                user_id: user_id.to_string(),
                message: "You approve social media posts every Monday. Want to auto-schedule?"
                    .to_string(),
                requires_opt_in: true,
                opted_in: false,
            });
        }

        suggestions
    }

    pub fn opt_in(&mut self, user_id: &str, suggestion_id: Uuid) {
        let accepted = self.opted_in.entry(user_id.to_string()).or_default();
        if !accepted.contains(&suggestion_id) {
            accepted.push(suggestion_id);
            accepted.sort();
        }
    }

    pub fn is_opted_in(&self, user_id: &str, suggestion_id: Uuid) -> bool {
        self.opted_in
            .get(user_id)
            .map(|ids| ids.contains(&suggestion_id))
            .unwrap_or(false)
    }
}

fn has_monday_streak(approvals: &[ApprovalRecord], streak_len: usize) -> bool {
    if approvals.len() < streak_len {
        return false;
    }

    let mut ordered = approvals.to_vec();
    ordered.sort_by_key(|approval| approval.approved_at);

    let monday_streak = ordered
        .iter()
        .rev()
        .take_while(|approval| approval.weekday == Weekday::Monday)
        .count();

    monday_streak >= streak_len
}

fn deterministic_suggestion_id(user_id: &str, key: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(user_id.as_bytes());
    hasher.update(b":");
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    Uuid::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::ProactiveSuggestionEngine;
    use crate::preferences::{ApprovalRecord, Weekday};

    #[test]
    fn test_proactive_suggestion() {
        let engine = ProactiveSuggestionEngine::new();

        let approvals = vec![
            ApprovalRecord {
                approved_at: 1,
                weekday: Weekday::Monday,
                content_style: "tutorial".to_string(),
                posting_time: "9am".to_string(),
                platform: "x".to_string(),
                tone: "professional".to_string(),
                topic: "rust".to_string(),
            },
            ApprovalRecord {
                approved_at: 2,
                weekday: Weekday::Monday,
                content_style: "tutorial".to_string(),
                posting_time: "9am".to_string(),
                platform: "x".to_string(),
                tone: "professional".to_string(),
                topic: "rust".to_string(),
            },
            ApprovalRecord {
                approved_at: 3,
                weekday: Weekday::Monday,
                content_style: "tutorial".to_string(),
                posting_time: "9am".to_string(),
                platform: "x".to_string(),
                tone: "professional".to_string(),
                topic: "rust".to_string(),
            },
            ApprovalRecord {
                approved_at: 4,
                weekday: Weekday::Monday,
                content_style: "tutorial".to_string(),
                posting_time: "9am".to_string(),
                platform: "x".to_string(),
                tone: "professional".to_string(),
                topic: "rust".to_string(),
            },
        ];

        let suggestions = engine.generate_suggestions("user-1", approvals.as_slice());
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].message.contains("auto-schedule"));
        assert!(suggestions[0].requires_opt_in);
        assert!(!suggestions[0].opted_in);
    }
}
