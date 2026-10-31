//! Lanes: the specialists Mother coordinates.
//!
//! A lane is called with a [`LaneInput`] and answers a [`LaneOutput`]. Its
//! answer is data: text, the effects it asks for (each checked and sent
//! through the EffectPort by the coordinator, never by the lane) and a
//! verdict. Nothing a lane says is consent or authority. Every call is
//! journaled with the lane's identity, an invocation id and the hashes of
//! its input and output.
//!
//! M1 has only deterministic lanes ([`ScriptedLane`]): no model is called.

use crate::contract::{sha256, ContractHash, PassCriterion};
use crate::effect::EffectRequest;
use crate::journal::MissionId;
use serde::{Deserialize, Serialize};
use std::fmt;

const INPUT_DOMAIN: &[u8] = b"nexus-mother-lane-input-v1\n";
const OUTPUT_DOMAIN: &[u8] = b"nexus-mother-lane-output-v1\n";

/// A lane's role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneRole {
    ResearcherA,
    ResearcherB,
    Skeptic,
    Builder,
    Verifier,
    Sentinel,
}

impl LaneRole {
    pub const ALL: [LaneRole; 6] = [
        LaneRole::ResearcherA,
        LaneRole::ResearcherB,
        LaneRole::Skeptic,
        LaneRole::Builder,
        LaneRole::Verifier,
        LaneRole::Sentinel,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            LaneRole::ResearcherA => "researcher_a",
            LaneRole::ResearcherB => "researcher_b",
            LaneRole::Skeptic => "skeptic",
            LaneRole::Builder => "builder",
            LaneRole::Verifier => "verifier",
            LaneRole::Sentinel => "sentinel",
        }
    }
}

/// Who a lane is, as journaled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaneIdentity {
    pub role: LaneRole,
    pub name: String,
    pub version: String,
}

/// One call of a lane: `inv-<mission>-<journal sequence>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InvocationId {
    pub mission: MissionId,
    pub seq: u64,
}

impl fmt::Display for InvocationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "inv-{}-{}", self.mission.0, self.seq)
    }
}

/// What a lane is called with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LaneInput {
    pub invocation: InvocationId,
    pub role: LaneRole,
    pub contract_hash: ContractHash,
    pub owner_words: String,
    pub restatement: String,
    pub pass_criteria: Vec<PassCriterion>,
    /// The output hashes of the lanes called before, in order.
    pub prior_outputs: Vec<String>,
}

/// A lane's judgement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    Continue,
    /// The Verifier: every pass criterion holds.
    Pass,
    /// The Verifier: a criterion does not hold.
    Fail {
        reason: String,
    },
    /// Stop the work and hand it back to the Owner (the Sentinel's call).
    Halt {
        reason: String,
    },
}

/// What a lane answers.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LaneOutput {
    pub text: String,
    pub effects: Vec<EffectRequest>,
    pub verdict: Verdict,
}

/// A lane.
pub trait Lane: Send + Sync {
    fn identity(&self) -> LaneIdentity;
    fn invoke(&self, input: &LaneInput) -> LaneOutput;
}

/// The hex SHA-256 of a lane input's canonical form.
pub fn input_hash(input: &LaneInput) -> String {
    let _ = (input, INPUT_DOMAIN);
    todo!("M1 implementation follows the oracle commit")
}

/// The hex SHA-256 of a lane output's canonical form.
pub fn output_hash(output: &LaneOutput) -> String {
    let _ = (output, OUTPUT_DOMAIN);
    todo!("M1 implementation follows the oracle commit")
}

#[allow(dead_code)]
fn hash_hex(domain: &[u8], json: &[u8]) -> String {
    hex::encode(sha256(domain, &[json]))
}

/// A deterministic lane: the same answer to every call. M1's only lanes.
#[derive(Clone, Debug)]
pub struct ScriptedLane {
    pub identity: LaneIdentity,
    pub output: LaneOutput,
}

impl ScriptedLane {
    /// A lane of `role` that answers `text` with `verdict` and asks for no
    /// effect.
    pub fn new(role: LaneRole, text: &str, verdict: Verdict) -> Self {
        Self {
            identity: LaneIdentity {
                role,
                name: format!("scripted-{}", role.as_str()),
                version: "m1".into(),
            },
            output: LaneOutput {
                text: text.into(),
                effects: vec![],
                verdict,
            },
        }
    }

    pub fn asking(mut self, effects: Vec<EffectRequest>) -> Self {
        self.output.effects = effects;
        self
    }
}

impl Lane for ScriptedLane {
    fn identity(&self) -> LaneIdentity {
        self.identity.clone()
    }

    fn invoke(&self, _: &LaneInput) -> LaneOutput {
        self.output.clone()
    }
}
