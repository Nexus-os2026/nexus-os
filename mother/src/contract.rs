//! The mission contract: what the Owner consents to, exactly.
//!
//! A contract is data. It names the goal in plain text and every step Mother
//! would take, each a broker [`Intent`] with the cost Mother declares for it.
//! Its effect categories are derived from each intent's domain, never from
//! anything a model says about it. Its digest binds every field.

use nexus_governed_control::governed::Intent;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fmt;

/// The most steps one contract may hold.
pub const MAX_STEPS: usize = 32;
/// The longest goal, in characters.
pub const MAX_GOAL_CHARS: usize = 512;
/// The longest consent lifetime: 8 hours.
pub const MAX_CONSENT_TTL_MS: u64 = 8 * 3600 * 1000;

/// What kind of effect a step may have: what the Owner consents to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectCategory {
    File,
    Network,
    ModelTool,
    Spend,
}

impl EffectCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            EffectCategory::File => "file",
            EffectCategory::Network => "network",
            EffectCategory::ModelTool => "model_tool",
            EffectCategory::Spend => "spend",
        }
    }
}

/// One step: a broker intent and the cost Mother declares for it, in minor
/// units (a declaration only ever adds the spend category; M1 meters
/// nothing).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Step {
    pub intent: Intent,
    pub declared_cost_minor: u64,
}

/// A mission contract.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MissionContract {
    /// The goal, in plain text.
    pub goal: String,
    /// The steps, in the order they run.
    pub steps: Vec<Step>,
    /// How long the Owner's consent lasts once given.
    pub consent_ttl_ms: u64,
}

/// The SHA-256 digest of a contract's canonical form.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContractDigest([u8; 32]);

impl ContractDigest {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ContractDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sha256:{}", hex::encode(self.0))
    }
}

impl fmt::Debug for ContractDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Why a contract cannot be drafted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractError {
    EmptyGoal,
    GoalTooLong,
    /// The goal holds a control, bidirectional, zero-width or non-ASCII
    /// space character.
    GoalNotPlainText,
    NoSteps,
    TooManySteps,
    /// A step's domain is not one M1 routes (perception, input, synthetic).
    UnsupportedDomain {
        step: usize,
        domain: &'static str,
    },
    ConsentTtlZero,
    ConsentTtlTooLong,
    /// The contract could not be put in canonical form.
    Unencodable,
}

/// The domain name of an intent, as the broker serialises it.
pub fn domain_of(intent: &Intent) -> &'static str {
    let _ = intent;
    todo!("M1 implementation follows the oracle commit")
}

/// The effect categories of an intent, from its domain alone; `Err` names
/// a domain M1 does not route.
pub fn categories_of(intent: &Intent) -> Result<BTreeSet<EffectCategory>, &'static str> {
    let _ = intent;
    todo!("M1 implementation follows the oracle commit")
}

impl Step {
    /// The step's categories: its intent's, plus spend if it declares a
    /// cost.
    pub fn categories(&self) -> Result<BTreeSet<EffectCategory>, &'static str> {
        todo!("M1 implementation follows the oracle commit")
    }
}

impl MissionContract {
    /// Check every bound; nothing is drafted that fails one.
    pub fn validate(&self) -> Result<(), ContractError> {
        todo!("M1 implementation follows the oracle commit")
    }

    /// Every category any step has.
    pub fn categories(&self) -> Result<BTreeSet<EffectCategory>, ContractError> {
        todo!("M1 implementation follows the oracle commit")
    }

    /// The sum of the declared costs (saturating).
    pub fn total_declared_cost_minor(&self) -> u64 {
        todo!("M1 implementation follows the oracle commit")
    }

    /// The digest of the canonical form, binding every field.
    pub fn digest(&self) -> Result<ContractDigest, ContractError> {
        todo!("M1 implementation follows the oracle commit")
    }
}
