//! Consent: the Owner's yes to one exact contract hash, from the broker.
//!
//! A [`Consent`] cannot be built, cloned, copied or deserialized outside
//! this crate. The coordinator makes one live only from the broker's
//! [`MissionApproval`]: the Owner's yes at the broker's native mission
//! dialog (`ControlConfirmer::confirm_mission`), recorded in the broker's
//! evidence store, for the exact contract hash and request the coordinator
//! asked about, while the contract has not changed. On restart, a verified
//! journal's record of that consent is replayed. No string, journal text or
//! lane (model) output is ever read as consent.
//!
//! ```compile_fail
//! // A consent cannot be made from outside the crate.
//! let _ = nexus_mother::consent::Consent { hash: todo!() };
//! ```
//!
//! ```compile_fail
//! // Nor copied.
//! fn copy(c: &nexus_mother::consent::Consent) -> nexus_mother::consent::Consent { c.clone() }
//! ```

use crate::contract::{ContractHash, EffectKind, MissionContract};
use crate::journal::MissionId;
use nexus_governed_control::authority::approval::{MissionApproval, MissionConfirmation};
use nexus_governed_control::authority::evidence::quoted;
use std::collections::BTreeSet;

/// What the Owner is shown about a mission, built only by the coordinator
/// from the mission's current contract. The broker's dialog shows it as
/// plain lines ([`MissionSummary::lines`]) beside the exact hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionSummary {
    pub mission: MissionId,
    pub hash: ContractHash,
    pub owner_words: String,
    pub restatement: String,
    pub requested_folder: Option<String>,
    pub max_duration_ms: u64,
    pub max_model_calls: u32,
    pub max_money_minor: u64,
    pub forbidden_effects: BTreeSet<EffectKind>,
    /// Each question with its answer.
    pub answers: Vec<(String, String)>,
    pub pass_criteria: Vec<String>,
}

impl MissionSummary {
    pub fn title(&self) -> &'static str {
        "Start this mission?"
    }

    pub(crate) fn of(mission: MissionId, contract: &MissionContract) -> Self {
        Self {
            mission,
            hash: contract.hash(),
            owner_words: contract.owner_words.clone(),
            restatement: contract.restatement.clone(),
            requested_folder: contract.requested_folder.clone(),
            max_duration_ms: contract.budget.max_duration_ms,
            max_model_calls: contract.budget.max_model_calls,
            max_money_minor: contract.budget.max_money_minor,
            forbidden_effects: contract.forbidden_effects.clone(),
            answers: contract
                .questions
                .iter()
                .map(|q| (q.text.clone(), q.answer.clone().unwrap_or_default()))
                .collect(),
            pass_criteria: contract
                .pass_criteria
                .iter()
                .map(|c| c.text.clone())
                .collect(),
        }
    }

    /// The summary as the broker's dialog shows it: plain lines, every
    /// text the Owner or a lane wrote quoted (each of its lines marked
    /// `│ `, hidden characters escaped, nothing cut), so none of it can
    /// pass for the dialog's own text.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![format!("Mission: {}", self.mission)];
        lines.extend(quoted("Your words, exactly", &self.owner_words));
        lines.extend(quoted("What I understood", &self.restatement));
        match &self.requested_folder {
            Some(folder) => lines.extend(quoted(
                "Folder you asked for (a request to the broker, not a permission)",
                folder,
            )),
            None => lines.push("Folder: none requested".into()),
        }
        lines.push(format!(
            "Budget: {} ms, {} lane calls, money {} (minor units)",
            self.max_duration_ms, self.max_model_calls, self.max_money_minor
        ));
        let forbidden: Vec<&str> = self.forbidden_effects.iter().map(|k| k.as_str()).collect();
        lines.push(format!(
            "Never allowed: {}",
            if forbidden.is_empty() {
                "nothing".to_string()
            } else {
                forbidden.join(", ")
            }
        ));
        for (question, answer) in &self.answers {
            lines.extend(quoted("I asked", question));
            lines.extend(quoted("You said", answer));
        }
        for criterion in &self.pass_criteria {
            lines.extend(quoted("Pass criterion (fixed now)", criterion));
        }
        lines
    }

    /// The broker's mission confirmation for this summary, under the
    /// coordinator's `request` label.
    pub(crate) fn confirmation(&self, request: String) -> MissionConfirmation {
        MissionConfirmation {
            contract_hash: self.hash.to_hex(),
            request,
            summary: self.lines(),
        }
    }
}

/// The Owner's consent to exactly one contract hash.
#[derive(Debug, PartialEq, Eq)]
pub struct Consent {
    hash: ContractHash,
}

impl Consent {
    /// Only when a verified journal recorded the consent (replay).
    pub(crate) fn minted(hash: ContractHash) -> Self {
        Self { hash }
    }

    /// The live consent: from the broker's approval of exactly `asked`
    /// (its hash and its request), consuming the approval. `None` for an
    /// approval of anything else.
    pub(crate) fn from_broker(
        approval: MissionApproval,
        asked: &MissionConfirmation,
    ) -> Option<Self> {
        if approval.contract_hash() != asked.contract_hash || approval.request() != asked.request {
            return None;
        }
        let hash = ContractHash::from_hex(approval.contract_hash())?;
        Some(Self { hash })
    }

    pub fn hash(&self) -> ContractHash {
        self.hash
    }

    /// Whether this consent covers `contract` exactly.
    pub fn covers(&self, contract: &MissionContract) -> bool {
        self.hash == contract.hash()
    }
}
