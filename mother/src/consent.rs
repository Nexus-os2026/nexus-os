//! Consent: the Owner's yes to one exact contract hash.
//!
//! A [`Consent`] cannot be built, cloned, copied or deserialized outside
//! this crate. The coordinator mints one only when a [`MissionConfirmer`]
//! answered yes to a [`MissionConfirmation`] the coordinator built from the
//! mission's contract, and only if that contract has not changed since.
//! No string, journal text or lane (model) output is ever read as consent.
//!
//! M1 non-claim: the broker's native confirmer (`ControlConfirmer`) has no
//! mission dialog, so the confirmer is Mother's own trait, with no
//! implementation in Mother's library. M2 moves mission consent into the
//! broker (`confirm_mission` in `ControlConfirmer`) when the desktop
//! confirmer is rebuilt; that is a security-core change that needs Owner
//! approval and a fresh review.
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
use std::collections::BTreeSet;

/// What the Owner is shown and asked to consent to. Built only by the
/// coordinator, from the mission's current contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionConfirmation {
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

impl MissionConfirmation {
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
}

/// The Owner's native answer to a mission confirmation: `true` only if the
/// Owner consented. Mother's library has no implementation; the host
/// provides it (M2: the desktop's native dialog).
pub trait MissionConfirmer {
    fn confirm_mission(&self, request: &MissionConfirmation) -> bool;
}

/// The Owner's consent to exactly one contract hash.
#[derive(Debug, PartialEq, Eq)]
pub struct Consent {
    hash: ContractHash,
}

impl Consent {
    /// Only after the Owner's confirmer said yes to this hash, or when a
    /// verified journal recorded that it did.
    pub(crate) fn minted(hash: ContractHash) -> Self {
        Self { hash }
    }

    pub fn hash(&self) -> ContractHash {
        self.hash
    }

    /// Whether this consent covers `contract` exactly.
    pub fn covers(&self, contract: &MissionContract) -> bool {
        self.hash == contract.hash()
    }
}
