//! The page verdict (design v2 §6). SKELETON: every candidate gets an
//! unbound refusal.

use super::facts::{ApprovedFacts, FactCode};
use super::policy::POLICY_V1;
use super::RefusalCode;

/// Which approved facts a verdict was evaluated with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FactsMarker {
    NoFacts,
    Facts([u8; 32]),
}

impl FactsMarker {
    pub fn of(_facts: Option<&ApprovedFacts>) -> Self {
        FactsMarker::NoFacts
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PageOutcome {
    Passed,
    Refused(RefusalCode),
    FactMissing(FactCode),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageVerdict {
    pub candidate_sha256: [u8; 32],
    pub policy_hash: [u8; 32],
    pub facts: FactsMarker,
    pub outcome: PageOutcome,
    hours_lines: u8,
}

impl PageVerdict {
    pub fn evaluate(_candidate: &[u8], _facts: Option<&ApprovedFacts>) -> Self {
        PageVerdict {
            candidate_sha256: [0; 32],
            policy_hash: [0; 32],
            facts: FactsMarker::NoFacts,
            outcome: PageOutcome::Refused(RefusalCode::NotCanonical),
            hours_lines: 0,
        }
    }

    pub fn passed(&self) -> bool {
        false
    }

    pub fn binding_hash(&self) -> [u8; 32] {
        [self.hours_lines; 32]
    }

    pub fn is_for(&self, _candidate: &[u8], _policy: &[u8; 32], _facts: &FactsMarker) -> bool {
        false
    }

    pub fn label(&self) -> &'static str {
        POLICY_V1.label_refused_prefix
    }

    pub fn facts_line(&self) -> Option<String> {
        None
    }
}
