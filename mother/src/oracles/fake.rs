//! A recording broker for the coordinator's oracles. It records every call
//! in order and can hold a run's opening or an action until the test lets
//! it go (or, for an action, until its run is cancelled), so that races are
//! driven by readiness, never by sleeping.

use crate::broker::{Acted, Broker, Refusal};
use nexus_governed_control::authority::commitment::CommitmentState;
use nexus_governed_control::authority::ids::{AgentId, CommitmentId, RunId};
use nexus_governed_control::governed::Intent;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The longest a held call waits before the fake gives up (a safety bound:
/// a test asserts it was released by its event, never by this).
const HOLD_BOUND: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    OpenRun { agent: String },
    Act { run: RunId, intent: String },
    CommitmentState(CommitmentId),
    Cancel(RunId),
    Finish(RunId),
}

impl Call {
    /// Whether the call could start an effect.
    pub fn is_effect(&self) -> bool {
        matches!(self, Call::OpenRun { .. } | Call::Act { .. })
    }
}

/// What the next action answers.
pub enum Reply {
    Done(Option<String>),
    Await(CommitmentId),
    Refuse(&'static str),
}

/// How a held call ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Released {
    ByEvent,
    ByBound,
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    replies: VecDeque<Reply>,
    open_refusal: Option<&'static str>,
    emergency: bool,
    minted: u128,
    cancelled: BTreeSet<RunId>,
    commitments: HashMap<CommitmentId, CommitmentState>,
    hold_open: bool,
    hold_act: bool,
    entered_open: bool,
    entered_act: bool,
    open_released: bool,
    released: Vec<Released>,
}

#[derive(Default)]
pub struct FakeBroker {
    state: Mutex<State>,
    changed: Condvar,
}

pub fn run_id(n: u128) -> RunId {
    RunId::parse(&format!("run-{n:032x}")).expect("run id")
}

pub fn commitment_id(n: u128) -> CommitmentId {
    CommitmentId::parse(&format!("cmt-{n:032x}")).expect("commitment id")
}

impl FakeBroker {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("fake broker")
    }

    pub fn calls(&self) -> Vec<Call> {
        self.lock().calls.clone()
    }

    pub fn effect_calls(&self) -> Vec<Call> {
        self.calls().into_iter().filter(Call::is_effect).collect()
    }

    pub fn acts(&self) -> usize {
        self.calls()
            .iter()
            .filter(|call| matches!(call, Call::Act { .. }))
            .count()
    }

    pub fn reply(&self, reply: Reply) {
        self.lock().replies.push_back(reply);
    }

    pub fn refuse_open(&self, class: &'static str) {
        self.lock().open_refusal = Some(class);
    }

    pub fn set_emergency(&self, on: bool) {
        self.lock().emergency = on;
    }

    pub fn set_commitment(&self, id: CommitmentId, state: CommitmentState) {
        self.lock().commitments.insert(id, state);
    }

    /// The next action blocks until its run is cancelled.
    pub fn hold_act(&self) {
        self.lock().hold_act = true;
    }

    /// The next run opening blocks until [`Self::release_open`].
    pub fn hold_open(&self) {
        self.lock().hold_open = true;
    }

    pub fn release_open(&self) {
        self.lock().open_released = true;
        self.changed.notify_all();
    }

    fn wait_for(&self, ready: impl Fn(&State) -> bool) -> bool {
        let deadline = Instant::now() + HOLD_BOUND;
        let mut state = self.lock();
        while !ready(&state) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            state = self.changed.wait_timeout(state, left).expect("fake").0;
        }
        true
    }

    /// Wait until a held action has been entered.
    pub fn wait_entered_act(&self) -> bool {
        self.wait_for(|state| state.entered_act)
    }

    /// Wait until a held run opening has been entered.
    pub fn wait_entered_open(&self) -> bool {
        self.wait_for(|state| state.entered_open)
    }

    /// How every held call ended.
    pub fn released(&self) -> Vec<Released> {
        self.lock().released.clone()
    }

    pub fn is_cancelled(&self, run: RunId) -> bool {
        self.lock().cancelled.contains(&run)
    }
}

impl Broker for FakeBroker {
    fn open_run(&self, agent: &AgentId) -> Result<RunId, Refusal> {
        let mut state = self.lock();
        state.calls.push(Call::OpenRun {
            agent: agent.to_string(),
        });
        if state.emergency {
            return Err(Refusal {
                class: "emergency_stopped",
            });
        }
        if let Some(class) = state.open_refusal {
            return Err(Refusal { class });
        }
        if state.hold_open {
            state.hold_open = false;
            state.entered_open = true;
            self.changed.notify_all();
            drop(state);
            let by_event = self.wait_for(|state| state.open_released);
            state = self.lock();
            state.released.push(if by_event {
                Released::ByEvent
            } else {
                Released::ByBound
            });
        }
        state.minted += 1;
        Ok(run_id(state.minted))
    }

    fn act(&self, _agent: &AgentId, run: RunId, intent: &Intent) -> Result<Acted, Refusal> {
        let mut state = self.lock();
        state.calls.push(Call::Act {
            run,
            intent: serde_json::to_string(intent).expect("intent"),
        });
        if state.cancelled.contains(&run) {
            return Err(Refusal {
                class: "run_cancelled",
            });
        }
        if state.emergency {
            return Err(Refusal {
                class: "emergency_stopped",
            });
        }
        if state.hold_act {
            // An effect in flight: it ends at its next step once its run
            // is cancelled.
            state.hold_act = false;
            state.entered_act = true;
            self.changed.notify_all();
            drop(state);
            let by_event = self.wait_for(|state| state.cancelled.contains(&run));
            self.lock().released.push(if by_event {
                Released::ByEvent
            } else {
                Released::ByBound
            });
            return Err(Refusal {
                class: "run_cancelled",
            });
        }
        match state.replies.pop_front().unwrap_or(Reply::Done(None)) {
            Reply::Done(text) => Ok(Acted::Done { text }),
            Reply::Await(commitment) => Ok(Acted::AwaitingApproval { commitment }),
            Reply::Refuse(class) => Err(Refusal { class }),
        }
    }

    fn commitment_state(&self, id: CommitmentId) -> Option<CommitmentState> {
        let mut state = self.lock();
        state.calls.push(Call::CommitmentState(id));
        state.commitments.get(&id).copied()
    }

    fn cancel_run(&self, run: RunId) -> Result<(), Refusal> {
        let mut state = self.lock();
        state.calls.push(Call::Cancel(run));
        state.cancelled.insert(run);
        self.changed.notify_all();
        Ok(())
    }

    fn finish_when_settled(&self, run: RunId) {
        self.lock().calls.push(Call::Finish(run));
    }

    fn emergency_stopped(&self) -> bool {
        self.lock().emergency
    }
}
