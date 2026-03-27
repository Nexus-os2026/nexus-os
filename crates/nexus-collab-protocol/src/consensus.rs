use serde::{Deserialize, Serialize};

use crate::message::{CollaborationMessage, MessageType};

/// Analyzes messages to detect natural consensus.
pub struct ConsensusDetector;

impl ConsensusDetector {
    pub fn detect_consensus(messages: &[CollaborationMessage]) -> ConsensusState {
        if messages.is_empty() {
            return ConsensusState::NoMessages;
        }

        let agrees = messages
            .iter()
            .filter(|m| m.message_type == MessageType::Agree)
            .count();
        let disagrees = messages
            .iter()
            .filter(|m| m.message_type == MessageType::Disagree)
            .count();
        let proposals = messages
            .iter()
            .filter(|m| m.message_type == MessageType::Propose)
            .count();
        let risks = messages
            .iter()
            .filter(|m| m.message_type == MessageType::RaiseRisk)
            .count();

        let latest_proposal = messages
            .iter()
            .rev()
            .find(|m| m.message_type == MessageType::Propose);

        if proposals == 0 {
            return ConsensusState::NoProposalYet;
        }

        let recent_window = messages.len().min(5);
        let recent = &messages[messages.len() - recent_window..];
        let recent_agrees = recent
            .iter()
            .filter(|m| m.message_type == MessageType::Agree)
            .count();
        let recent_disagrees = recent
            .iter()
            .filter(|m| m.message_type == MessageType::Disagree)
            .count();

        if let Some(proposal) = latest_proposal {
            if recent_agrees >= 2 && recent_disagrees == 0 {
                return ConsensusState::NaturalConsensus {
                    proposal: proposal.content.text.clone(),
                    agreement_count: agrees,
                    confidence: recent
                        .iter()
                        .filter(|m| m.message_type == MessageType::Agree)
                        .map(|m| m.content.confidence)
                        .sum::<f64>()
                        / recent_agrees.max(1) as f64,
                };
            }
        }

        if disagrees > agrees && risks > 0 {
            return ConsensusState::Deadlocked {
                proposals_count: proposals,
                for_count: agrees,
                against_count: disagrees,
                unresolved_risks: risks,
            };
        }

        ConsensusState::InProgress {
            proposals_count: proposals,
            for_count: agrees,
            against_count: disagrees,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusState {
    NoMessages,
    NoProposalYet,
    InProgress {
        proposals_count: usize,
        for_count: usize,
        against_count: usize,
    },
    NaturalConsensus {
        proposal: String,
        agreement_count: usize,
        confidence: f64,
    },
    Deadlocked {
        proposals_count: usize,
        for_count: usize,
        against_count: usize,
        unresolved_risks: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(mtype: MessageType, text: &str, confidence: f64) -> CollaborationMessage {
        CollaborationMessage::new("s1", "agent-1", None, mtype, text, confidence)
    }

    #[test]
    fn test_consensus_detector_natural() {
        let messages = vec![
            msg(MessageType::Propose, "Use REST API", 0.9),
            msg(MessageType::Agree, "Good idea", 0.85),
            msg(MessageType::Agree, "I concur", 0.9),
            msg(MessageType::Agree, "Agreed", 0.8),
        ];
        match ConsensusDetector::detect_consensus(&messages) {
            ConsensusState::NaturalConsensus {
                agreement_count, ..
            } => {
                assert_eq!(agreement_count, 3);
            }
            other => panic!("Expected NaturalConsensus, got {:?}", other),
        }
    }

    #[test]
    fn test_consensus_detector_deadlock() {
        let messages = vec![
            msg(MessageType::Propose, "Plan A", 0.7),
            msg(MessageType::Disagree, "Too complex", 0.8),
            msg(MessageType::Disagree, "Too expensive", 0.6),
            msg(MessageType::RaiseRisk, "Security concern", 0.9),
        ];
        match ConsensusDetector::detect_consensus(&messages) {
            ConsensusState::Deadlocked { against_count, .. } => {
                assert_eq!(against_count, 2);
            }
            other => panic!("Expected Deadlocked, got {:?}", other),
        }
    }
}
