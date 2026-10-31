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
//!
//! M2a, an effect that waits for the Owner's native approval:
//!   Investigating | Building | Verifying ─await approval─► AwaitingApproval
//!   AwaitingApproval ─action approved─► the phase it paused in
//!                    ─action denied─► Denied
//!                    ─approval lapsed─► Blocked
//! ```

use crate::contract::{ContractHash, MissionContract};
use crate::lane::LaneRole;
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
    /// M2a: an effect waits for the Owner's native approval at the broker.
    /// No lane runs and nothing is sent until the broker answers.
    AwaitingApproval,
}

impl MissionState {
    pub const ALL: [MissionState; 13] = [
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
        MissionState::AwaitingApproval,
    ];

    /// A terminal mission never moves again and dispatches nothing.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            MissionState::Done
                | MissionState::Stopped
                | MissionState::Denied
                | MissionState::Failed
                | MissionState::Blocked
        )
    }

    /// The states in which lanes run and effects may be dispatched.
    pub fn is_working(self) -> bool {
        matches!(
            self,
            MissionState::Investigating
                | MissionState::Building
                | MissionState::Verifying
                | MissionState::Explaining
        )
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
            MissionState::AwaitingApproval => "awaiting_approval",
        }
    }

    /// The phases in which an effect may wait for the Owner's approval:
    /// those whose lanes may ask for effects.
    pub fn may_wait(self) -> bool {
        matches!(
            self,
            MissionState::Investigating | MissionState::Building | MissionState::Verifying
        )
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
    /// M2a: effect number `effect` waits for the Owner's approval. What the
    /// phase had still to do when it paused: the rest of the asking lane's
    /// effects (as data) and the lanes still to run.
    AwaitApproval {
        effect: u64,
        rest: Vec<serde_json::Value>,
        roles_left: Vec<LaneRole>,
    },
    /// The Owner approved the waited effect and it ran at the broker: the
    /// mission returns to `phase`, the phase it paused in.
    ActionApproved {
        effect: u64,
        phase: MissionState,
    },
    /// The Owner denied the waited effect.
    ActionDenied {
        effect: u64,
    },
    /// The waited effect will not run: its approval expired, it was
    /// revoked or failed, or Mother's own budget ran out.
    ApprovalLapsed {
        effect: u64,
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
            Trigger::AwaitApproval { .. } => "await_approval",
            Trigger::ActionApproved { .. } => "action_approved",
            Trigger::ActionDenied { .. } => "action_denied",
            Trigger::ApprovalLapsed { .. } => "approval_lapsed",
        }
    }
}

/// The one transition table.
pub fn transition(
    from: MissionState,
    trigger: &Trigger,
) -> Result<MissionState, IllegalTransition> {
    use MissionState::*;
    let to = match (trigger, from) {
        (Trigger::Revise { .. }, Draft | AwaitingConsent | Approved) => Some(Draft),
        (Trigger::Submit { .. }, Draft) => Some(AwaitingConsent),
        (Trigger::Consent { .. }, AwaitingConsent) => Some(Approved),
        (Trigger::Deny { .. }, AwaitingConsent) => Some(Denied),
        (Trigger::Start, Approved) => Some(Investigating),
        (Trigger::PhaseDone, Investigating) => Some(Building),
        (Trigger::PhaseDone, Building) => Some(Verifying),
        (Trigger::PhaseDone, Explaining) => Some(Done),
        (Trigger::VerifyPass, Verifying) => Some(Explaining),
        (Trigger::VerifyFail { .. }, Verifying) => Some(Failed),
        (Trigger::Block { .. }, Approved) => Some(Blocked),
        (Trigger::Block { .. }, s) if s.is_working() => Some(Blocked),
        (Trigger::Fail { .. }, s) if s.is_working() => Some(Failed),
        (Trigger::Stop { .. }, s) if !s.is_terminal() => Some(Stopped),
        (Trigger::AwaitApproval { .. }, s) if s.may_wait() => Some(AwaitingApproval),
        (Trigger::ActionApproved { phase, .. }, AwaitingApproval) if phase.may_wait() => {
            Some(*phase)
        }
        (Trigger::ActionDenied { .. }, AwaitingApproval) => Some(Denied),
        (Trigger::ApprovalLapsed { .. }, AwaitingApproval) => Some(Blocked),
        _ => None,
    };
    to.ok_or(IllegalTransition {
        from,
        trigger: trigger.name(),
    })
}
