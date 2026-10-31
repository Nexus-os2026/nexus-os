//! The mission state machine: deterministic, one table, used both live and
//! when a journal is replayed.
//!
//! ```text
//! Draft ─submit─► AwaitingConsent ─consent─► Approved ─start─► Investigating
//!   ▲   ◄─revise──────┘   │ deny                 │              │ phase done
//!   └────────revise───────┼──────────────────────┘              ▼
//!                         ▼                                   Building
//!                       Denied                                  │ phase done
//!                                                               ▼
//!   Done ◄─phase done─ Explaining ◄─verify pass─ Verifying ─verify fail─► Failed
//!
//! Stop: every non-terminal state ─► Stopped.
//! Block: Approved … Explaining ─► Blocked.   Fail: Investigating … Explaining ─► Failed.
//! ```

use crate::contract::{ContractHash, MissionContract};
use serde::{Deserialize, Serialize};

/// Where a mission is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionState {
    Draft,
    AwaitingConsent,
    Approved,
    Investigating,
    Building,
    Verifying,
    Explaining,
    Done,
    Stopped,
    Denied,
    Failed,
    Blocked,
}

impl MissionState {
    pub const ALL: [MissionState; 12] = [
        MissionState::Draft,
        MissionState::AwaitingConsent,
        MissionState::Approved,
        MissionState::Investigating,
        MissionState::Building,
        MissionState::Verifying,
        MissionState::Explaining,
        MissionState::Done,
        MissionState::Stopped,
        MissionState::Denied,
        MissionState::Failed,
        MissionState::Blocked,
    ];

    /// A terminal mission never moves again and dispatches nothing.
    pub fn is_terminal(self) -> bool {
        todo!("M1 implementation follows the oracle commit")
    }

    /// The states in which lanes run and effects may be dispatched.
    pub fn is_working(self) -> bool {
        todo!("M1 implementation follows the oracle commit")
    }

    pub fn as_str(self) -> &'static str {
        match self {
            MissionState::Draft => "draft",
            MissionState::AwaitingConsent => "awaiting_consent",
            MissionState::Approved => "approved",
            MissionState::Investigating => "investigating",
            MissionState::Building => "building",
            MissionState::Verifying => "verifying",
            MissionState::Explaining => "explaining",
            MissionState::Done => "done",
            MissionState::Stopped => "stopped",
            MissionState::Denied => "denied",
            MissionState::Failed => "failed",
            MissionState::Blocked => "blocked",
        }
    }
}

/// What moves a mission. Every trigger is journaled before it takes
/// effect, and replay feeds the same triggers through [`transition`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "trigger", rename_all = "snake_case", deny_unknown_fields)]
pub enum Trigger {
    /// The contract changed; any consent is void.
    Revise {
        contract: MissionContract,
    },
    Submit {
        hash: ContractHash,
    },
    Consent {
        hash: ContractHash,
        at_wall_ms: u64,
    },
    Deny {
        hash: ContractHash,
    },
    Start,
    PhaseDone,
    VerifyPass,
    VerifyFail {
        reason: String,
    },
    Block {
        reason: String,
    },
    Fail {
        reason: String,
    },
    Stop {
        reason: String,
    },
}

/// A trigger that the state does not accept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IllegalTransition {
    pub from: MissionState,
    pub trigger: &'static str,
}

impl Trigger {
    pub fn name(&self) -> &'static str {
        match self {
            Trigger::Revise { .. } => "revise",
            Trigger::Submit { .. } => "submit",
            Trigger::Consent { .. } => "consent",
            Trigger::Deny { .. } => "deny",
            Trigger::Start => "start",
            Trigger::PhaseDone => "phase_done",
            Trigger::VerifyPass => "verify_pass",
            Trigger::VerifyFail { .. } => "verify_fail",
            Trigger::Block { .. } => "block",
            Trigger::Fail { .. } => "fail",
            Trigger::Stop { .. } => "stop",
        }
    }
}

/// The one transition table.
pub fn transition(
    from: MissionState,
    trigger: &Trigger,
) -> Result<MissionState, IllegalTransition> {
    let _ = (from, trigger);
    todo!("M1 implementation follows the oracle commit")
}
