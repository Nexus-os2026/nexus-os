//! The mission contract: what the Owner consents to, exactly.
//!
//! A contract is data. It keeps the Owner's words verbatim beside Mother's
//! restatement, the folder the Owner asked for (a request to the broker,
//! never path authority), the budget, the forbidden effects, the questions
//! Mother needed answered and the pass criteria, all fixed before any work.
//! Its hash binds every field.

use nexus_governed_control::authority::evidence::{is_plain, shown};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;

/// The longest Owner's words, in characters.
pub const MAX_OWNER_WORDS_CHARS: usize = 4096;
/// The longest restatement, in characters.
pub const MAX_RESTATEMENT_CHARS: usize = 2048;
/// The most questions or pass criteria one contract may hold.
pub const MAX_ITEMS: usize = 32;
/// The longest mission time budget: 24 hours.
pub const MAX_DURATION_MS: u64 = 24 * 3600 * 1000;
/// The most model calls one mission may make.
pub const MAX_MODEL_CALLS: u32 = 1000;

const HASH_DOMAIN: &[u8] = b"nexus-mother-contract-v1\n";

/// A kind of effect on the world. Every one goes through the
/// [`crate::effect::EffectPort`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    File,
    Network,
    Process,
    ModelTool,
    Spend,
}

impl EffectKind {
    pub const ALL: [EffectKind; 5] = [
        EffectKind::File,
        EffectKind::Network,
        EffectKind::Process,
        EffectKind::ModelTool,
        EffectKind::Spend,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            EffectKind::File => "file",
            EffectKind::Network => "network",
            EffectKind::Process => "process",
            EffectKind::ModelTool => "model_tool",
            EffectKind::Spend => "spend",
        }
    }
}

/// What a mission may spend. Money is always zero in M1.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    /// Wall time from consent, in milliseconds.
    pub max_duration_ms: u64,
    /// Lane invocations (each counts as one model call).
    pub max_model_calls: u32,
    /// Money, in minor units. M1 refuses any value but zero.
    pub max_money_minor: u64,
}

/// A question Mother needed answered before the mission could be planned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    pub id: String,
    pub text: String,
    pub answer: Option<String>,
}

/// A pass criterion, fixed before any work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PassCriterion {
    pub id: String,
    pub text: String,
}

/// A mission contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MissionContract {
    /// The Owner's words, verbatim: never trimmed, normalised or rewritten.
    pub owner_words: String,
    /// Mother's restatement of the goal, in plain text.
    pub restatement: String,
    /// The folder the Owner asked for: a request to the broker, never path
    /// authority. Mother never opens, resolves or creates it.
    pub requested_folder: Option<String>,
    pub budget: Budget,
    /// Effects the mission must never have, whatever a lane asks for.
    pub forbidden_effects: BTreeSet<EffectKind>,
    /// The questions Mother needed answered; every one must be answered
    /// before the contract goes to the Owner.
    pub questions: Vec<Question>,
    /// What the Verifier judges the result against, fixed before work.
    pub pass_criteria: Vec<PassCriterion>,
}

/// The SHA-256 hash of a contract's canonical form.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContractHash([u8; 32]);

impl ContractHash {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(text: &str) -> Option<Self> {
        let bytes = hex::decode(text).ok()?;
        Some(Self(bytes.try_into().ok()?))
    }

    /// The first eight hex characters, as the Owner sees them.
    pub fn short(&self) -> String {
        self.to_hex()[..8].to_string()
    }
}

impl fmt::Display for ContractHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sha256:{}", self.to_hex())
    }
}

impl fmt::Debug for ContractHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Serialize for ContractHash {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ContractHash {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::from_hex(&text).ok_or_else(|| serde::de::Error::custom("contract hash"))
    }
}

/// Why a contract cannot be drafted or submitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractError {
    EmptyOwnerWords,
    OwnerWordsTooLong,
    EmptyRestatement,
    RestatementTooLong,
    /// The restatement holds a control, bidirectional, zero-width or
    /// non-ASCII space character.
    RestatementNotPlain,
    /// The requested folder is empty or not plain text.
    FolderNotPlain,
    /// A question or criterion id or text is empty or not plain text.
    ItemNotPlain,
    DuplicateId,
    TooManyItems,
    NoPassCriteria,
    DurationOutOfRange,
    ModelCallsOutOfRange,
    /// M1 spends no money: the money budget must be zero.
    MoneyNotZero,
    /// A question has no answer: the contract cannot go to the Owner.
    UnansweredQuestion {
        id: String,
    },
}

impl MissionContract {
    /// Check every bound (not the answers). Nothing is drafted that fails
    /// one.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.owner_words.trim().is_empty() {
            return Err(ContractError::EmptyOwnerWords);
        }
        if self.owner_words.chars().count() > MAX_OWNER_WORDS_CHARS {
            return Err(ContractError::OwnerWordsTooLong);
        }
        if self.restatement.trim().is_empty() {
            return Err(ContractError::EmptyRestatement);
        }
        if self.restatement.chars().count() > MAX_RESTATEMENT_CHARS {
            return Err(ContractError::RestatementTooLong);
        }
        if !self.restatement.chars().all(shown) {
            return Err(ContractError::RestatementNotPlain);
        }
        if let Some(folder) = &self.requested_folder {
            if !plain(folder) {
                return Err(ContractError::FolderNotPlain);
            }
        }
        if self.questions.len() > MAX_ITEMS || self.pass_criteria.len() > MAX_ITEMS {
            return Err(ContractError::TooManyItems);
        }
        if self.pass_criteria.is_empty() {
            return Err(ContractError::NoPassCriteria);
        }
        for question in &self.questions {
            let answer_plain = question.answer.as_deref().is_none_or(plain);
            if !plain(&question.id) || !plain(&question.text) || !answer_plain {
                return Err(ContractError::ItemNotPlain);
            }
        }
        for criterion in &self.pass_criteria {
            if !plain(&criterion.id) || !plain(&criterion.text) {
                return Err(ContractError::ItemNotPlain);
            }
        }
        let unique = |ids: Vec<&String>| ids.iter().collect::<BTreeSet<_>>().len() == ids.len();
        if !unique(self.questions.iter().map(|q| &q.id).collect())
            || !unique(self.pass_criteria.iter().map(|c| &c.id).collect())
        {
            return Err(ContractError::DuplicateId);
        }
        if !(1..=MAX_DURATION_MS).contains(&self.budget.max_duration_ms) {
            return Err(ContractError::DurationOutOfRange);
        }
        if !(1..=MAX_MODEL_CALLS).contains(&self.budget.max_model_calls) {
            return Err(ContractError::ModelCallsOutOfRange);
        }
        if self.budget.max_money_minor != 0 {
            return Err(ContractError::MoneyNotZero);
        }
        Ok(())
    }

    /// [`Self::validate`], and every question answered: what the Owner may
    /// be asked to consent to.
    pub fn validate_for_consent(&self) -> Result<(), ContractError> {
        self.validate()?;
        match self.questions.iter().find(|q| q.answer.is_none()) {
            Some(question) => Err(ContractError::UnansweredQuestion {
                id: question.id.clone(),
            }),
            None => Ok(()),
        }
    }

    /// The hash of the canonical form, binding every field.
    pub fn hash(&self) -> ContractHash {
        // Struct fields serialise in declaration order and the effect set
        // in its sorted order, so the JSON is canonical.
        let json = serde_json::to_vec(self).expect("a contract serialises");
        ContractHash(sha256(HASH_DOMAIN, &[&json]))
    }
}

pub(crate) fn sha256(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize().into()
}

/// Non-blank, and every character shown as itself (at most the broker's
/// field length).
fn plain(text: &str) -> bool {
    !text.trim().is_empty() && is_plain(text)
}
