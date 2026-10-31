//! The coordinator: turns a contract into a governed mission.
//!
//! Every change is journaled before it takes effect. Nothing reaches the
//! EffectPort before the Owner consented to the mission's exact contract
//! hash, and nothing reaches it after Stop. No coordinator lock is held
//! while the Owner is asked, while a lane runs or while an effect runs: a
//! call is reserved under the lock, made with the lock released, and its
//! result is kept only if the mission did not move meanwhile. Stop never
//! waits for a lane or an effect.

use crate::consent::{Consent, MissionConfirmation, MissionConfirmer};
use crate::contract::{ContractError, ContractHash, EffectKind, MissionContract};
use crate::effect::EffectPort;
use crate::journal::{Entry, JournalError, JournalStore, MissionId};
use crate::lane::{InvocationId, Lane, LaneIdentity, LaneRole, Verdict};
use crate::state::{IllegalTransition, MissionState};
use nexus_governed_control::authority::clock::Clock;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// The most missions one coordinator keeps.
pub const MAX_MISSIONS: usize = 1024;
/// The most effects one lane may ask for in one call.
pub const MAX_EFFECTS_PER_CALL: usize = 16;

/// One lane per role.
pub struct Lanes {
    by_role: BTreeMap<LaneRole, Arc<dyn Lane>>,
}

impl Lanes {
    /// Exactly one lane for every role, each naming its own role.
    pub fn new(lanes: Vec<Arc<dyn Lane>>) -> Result<Self, CoordinatorError> {
        let _ = lanes;
        todo!("M1 implementation follows the oracle commit")
    }

    #[allow(dead_code)]
    fn get(&self, role: LaneRole) -> &Arc<dyn Lane> {
        &self.by_role[&role]
    }
}

/// Why the coordinator refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoordinatorError {
    UnknownMission,
    Contract(ContractError),
    Journal(JournalError),
    Illegal(IllegalTransition),
    /// The lanes are not exactly one per role.
    Lanes(String),
    Capacity,
    /// Work was asked of a mission without a consent covering its contract.
    NoConsent,
    /// The Owner's answer came for a contract or state that had moved on.
    ConsentStale,
    Denied,
    Stopped,
    /// Another call is already working on this mission.
    Busy,
}

/// One lane call, as reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaneReport {
    pub invocation: InvocationId,
    pub identity: LaneIdentity,
    pub text: String,
    pub verdict: Verdict,
    pub input_hash: String,
    pub output_hash: String,
}

/// One effect, as reported: dispatched to the port, or refused by Mother.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectReport {
    pub kind: EffectKind,
    pub dispatched: bool,
    pub outcome: String,
}

/// What one [`Coordinator::advance`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Advance {
    pub from: MissionState,
    pub to: MissionState,
    pub lanes: Vec<LaneReport>,
    pub effects: Vec<EffectReport>,
    /// Set when the mission reaches Done: what Mother tells the Owner.
    pub explanation: Option<String>,
    /// Set when the mission ends Blocked or Failed.
    pub reason: Option<String>,
}

#[allow(dead_code)]
struct Mission {
    contract: MissionContract,
    state: MissionState,
    consent: Option<Consent>,
    /// Bumped by every transition: a reservation holds while it is equal.
    generation: u64,
    busy: bool,
    approved_wall_ms: Option<u64>,
    approved_mono_ms: Option<u64>,
    model_calls: u32,
    next_effect: u64,
    unsettled: Option<u64>,
    prior_outputs: Vec<String>,
    reason: Option<String>,
}

#[allow(dead_code)]
struct Inner {
    store: Box<dyn JournalStore>,
    head: String,
    next_seq: u64,
    missions: BTreeMap<MissionId, Mission>,
    next_mission: u64,
}

/// The coordinator.
pub struct Coordinator<P: EffectPort> {
    #[allow(dead_code)]
    port: Arc<P>,
    #[allow(dead_code)]
    lanes: Lanes,
    #[allow(dead_code)]
    clock: Arc<dyn Clock>,
    #[allow(dead_code)]
    inner: Mutex<Inner>,
}

impl<P: EffectPort> Coordinator<P> {
    /// Rebuild a coordinator from its journal (empty for a new home). A
    /// journal that does not verify opens nothing. A mission whose last
    /// effect's outcome was not recorded is blocked: it may have happened.
    pub fn open(
        store: impl JournalStore + 'static,
        port: Arc<P>,
        lanes: Lanes,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, CoordinatorError> {
        let _ = (store, port, lanes, clock);
        todo!("M1 implementation follows the oracle commit")
    }

    /// Draft a mission. Nothing reaches the port.
    pub fn draft(&self, contract: MissionContract) -> Result<MissionId, CoordinatorError> {
        let _ = contract;
        todo!("M1 implementation follows the oracle commit")
    }

    /// Change a mission's contract before work starts: it returns to Draft
    /// and any consent is void.
    pub fn revise(
        &self,
        id: MissionId,
        contract: MissionContract,
    ) -> Result<MissionState, CoordinatorError> {
        let _ = (id, contract);
        todo!("M1 implementation follows the oracle commit")
    }

    /// Put a complete contract (every question answered) to the Owner.
    pub fn submit(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    /// What the Owner would be shown.
    pub fn confirmation(&self, id: MissionId) -> Result<MissionConfirmation, CoordinatorError> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    /// Ask the Owner. No lock is held while the Owner answers; a yes counts
    /// only if the mission is still awaiting consent to the same hash.
    pub fn request_consent(
        &self,
        id: MissionId,
        confirmer: &dyn MissionConfirmer,
    ) -> Result<MissionState, CoordinatorError> {
        let _ = (id, confirmer);
        todo!("M1 implementation follows the oracle commit")
    }

    /// Move a consented mission one state forward, running that state's
    /// lanes and dispatching the effects they ask for that pass every
    /// check.
    pub fn advance(&self, id: MissionId) -> Result<Advance, CoordinatorError> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    /// Stop a mission from whatever state it is in; a terminal mission
    /// stays as it ended. Never waits for a lane or an effect.
    pub fn stop(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    /// Stop every mission that has not ended. Returns how many.
    pub fn stop_all(&self) -> usize {
        todo!("M1 implementation follows the oracle commit")
    }

    pub fn state(&self, id: MissionId) -> Option<MissionState> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    pub fn contract(&self, id: MissionId) -> Option<MissionContract> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    /// The hash the mission's consent covers, if it holds one.
    pub fn consented_hash(&self, id: MissionId) -> Option<ContractHash> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    /// Why a mission ended Blocked or Failed.
    pub fn reason(&self, id: MissionId) -> Option<String> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    pub fn missions(&self) -> Vec<MissionId> {
        todo!("M1 implementation follows the oracle commit")
    }

    /// The verified journal.
    pub fn journal(&self) -> Result<Vec<Entry>, CoordinatorError> {
        todo!("M1 implementation follows the oracle commit")
    }
}
