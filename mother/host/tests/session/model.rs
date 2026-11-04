//! Frozen independent rules: session spec §§1–3, 6 and amendment B.
//! No production imports, result strings, predicates, counters or classifiers.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    Bootstrap,
    Normal,
    Emergency,
    Failed,
    ClosingNormal,
    ClosingRestricted,
    SealedFinalizing,
    ClosedClean,
    ClosedUnclean,
}

pub const LIFECYCLES: [Lifecycle; 9] = [
    Lifecycle::Bootstrap,
    Lifecycle::Normal,
    Lifecycle::Emergency,
    Lifecycle::Failed,
    Lifecycle::ClosingNormal,
    Lifecycle::ClosingRestricted,
    Lifecycle::SealedFinalizing,
    Lifecycle::ClosedClean,
    Lifecycle::ClosedUnclean,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mission {
    Draft,
    AwaitingConsent,
    Approved,
    Investigating,
    Building,
    AwaitingApproval,
    Verifying,
    Explaining,
    Done,
    Stopped,
    Denied,
    Failed,
    Blocked,
}

pub const MISSIONS: [Mission; 13] = [
    Mission::Draft,
    Mission::AwaitingConsent,
    Mission::Approved,
    Mission::Investigating,
    Mission::Building,
    Mission::AwaitingApproval,
    Mission::Verifying,
    Mission::Explaining,
    Mission::Done,
    Mission::Stopped,
    Mission::Denied,
    Mission::Failed,
    Mission::Blocked,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    OwnerStop,
    Emergency,
    Budget,
    DoorFence,
    Denial,
    LaneFailure,
    VerifierFailure,
    MissingVerdict,
}
pub const CAUSES: [Cause; 8] = [
    Cause::OwnerStop,
    Cause::Emergency,
    Cause::Budget,
    Cause::DoorFence,
    Cause::Denial,
    Cause::LaneFailure,
    Cause::VerifierFailure,
    Cause::MissingVerdict,
];

// Literal specification-derived table. Columns are CAUSES in the order above.
// None is an inapplicable transition, never permission or a default Block.
pub const TERMINALS: [[Option<Mission>; 8]; 13] = {
    use Mission::*;
    const S: Option<Mission> = Some(Stopped);
    const B: Option<Mission> = Some(Blocked);
    const F: Option<Mission> = Some(Failed);
    const D: Option<Mission> = Some(Denied);
    [
        [S, S, S, S, None, None, None, None], // Draft
        [S, S, S, S, D, None, None, None],    // AwaitingConsent
        [S, S, B, S, None, None, None, None], // Approved
        [S, S, B, S, None, F, None, None],    // Investigating
        [S, S, B, S, None, F, None, None],    // Building
        [S, S, B, S, D, None, None, None],    // AwaitingApproval, conclusive nonpublication only
        [S, S, B, S, None, F, F, F],          // Verifying
        [S, S, B, S, None, F, None, None],    // Explaining
        [Some(Done); 8],                      // Done: records, ack and resources can still be owed
        [S; 8],
        [D; 8],
        [F; 8],
        [B; 8],
    ]
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    NotAsked,
    Awaiting,
    Declined,
    Affirmed,
    Invalidated,
    ProtocolFailed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Publication {
    NotDispatched,
    Pending,
    Absent,
    Published,
    Uncertain,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Record {
    Owed,
    Recorded,
    Failed,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceState {
    Reserved,
    Live,
    Borrowed,
    Retained,
    Released,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resource {
    pub state: ResourceState,
    pub observed_absent: bool,
    pub removal_durable: bool,
    pub authority_closed: bool,
    pub records: [Record; 3],
    pub intentional_retention: bool,
}
impl Resource {
    pub fn reserved() -> Self {
        Self {
            state: ResourceState::Reserved,
            observed_absent: false,
            removal_durable: false,
            authority_closed: false,
            records: [Record::Owed; 3],
            intentional_retention: false,
        }
    }
    pub fn discharged(&self) -> bool {
        self.state == ResourceState::Released
            && self.authority_closed
            && self.records == [Record::Recorded; 3]
            && (self.intentional_retention || (self.observed_absent && self.removal_durable))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub id: u16,
    pub generation: u64,
    pub target: Option<(u16, u64)>,
    pub answer: Answer,
    pub handed_off: BTreeSet<u16>,
}

#[derive(Clone, Debug)]
pub struct Model {
    pub lifecycle: Lifecycle,
    pub generation: u64,
    pub emergency: bool,
    pub fatal: bool,
    pub first: Option<Cause>,
    pub resources: BTreeMap<u16, Resource>,
    pub created: BTreeSet<u16>,
    pub ingress: BTreeSet<u64>,
    pub recorded_ingress: BTreeSet<u64>,
    pub workers: BTreeSet<u16>,
    pub quiescent: bool,
    pub publication: Publication,
    pub interruption: BTreeSet<u16>,
    pub recovery: bool,
    pub terminal_record: Record,
    pub ack_owed: bool,
    pub ack_receipt: Option<[u16; 6]>,
    pub close_generation: Option<u64>,
    pub close_resources: BTreeSet<u16>,
    pub end_records: [Record; 2],
    pub delivered: bool,
}
impl Model {
    pub fn normal() -> Self {
        Self {
            lifecycle: Lifecycle::Normal,
            generation: 0,
            emergency: false,
            fatal: false,
            first: None,
            resources: BTreeMap::new(),
            created: BTreeSet::new(),
            ingress: BTreeSet::new(),
            recorded_ingress: BTreeSet::new(),
            workers: BTreeSet::new(),
            quiescent: false,
            publication: Publication::NotDispatched,
            interruption: BTreeSet::new(),
            recovery: false,
            terminal_record: Record::Recorded,
            ack_owed: false,
            ack_receipt: None,
            close_generation: None,
            close_resources: BTreeSet::new(),
            end_records: [Record::Owed; 2],
            delivered: false,
        }
    }
    pub fn accept(&self, id: u16, target: Option<(u16, u64)>) -> Request {
        Request {
            id,
            generation: self.generation,
            target,
            answer: Answer::NotAsked,
            handed_off: BTreeSet::new(),
        }
    }
    pub fn stop(&mut self) -> bool {
        if matches!(
            self.lifecycle,
            Lifecycle::SealedFinalizing | Lifecycle::ClosedClean | Lifecycle::ClosedUnclean
        ) {
            return false;
        }
        let Some(next) = self.generation.checked_add(1) else {
            self.fatal = true;
            self.lifecycle = Lifecycle::Failed;
            return false;
        };
        self.generation = next;
        self.emergency = true;
        self.first.get_or_insert(Cause::Emergency);
        self.ingress.insert(next);
        self.lifecycle = match self.lifecycle {
            Lifecycle::ClosingNormal | Lifecycle::ClosingRestricted => Lifecycle::ClosingRestricted,
            Lifecycle::Failed => Lifecycle::Failed,
            _ => Lifecycle::Emergency,
        };
        true
    }
    pub fn admit(&self, request: &mut Request, step: u16) -> bool {
        if self.lifecycle != Lifecycle::Normal
            || self.emergency
            || self.fatal
            || request.generation != self.generation
        {
            return false;
        }
        request.handed_off.insert(step)
    }
    pub fn reserve(&mut self, id: u16) -> bool {
        if self.created.len() == 256 || self.created.contains(&id) {
            return false;
        }
        self.created.insert(id);
        self.resources.insert(id, Resource::reserved());
        true
    }
    pub fn abandon(&mut self, id: u16) {
        self.resources.get_mut(&id).unwrap().state = ResourceState::Retained;
    }
    pub fn quit(&mut self) {
        if self.close_generation.is_some() {
            return;
        }
        self.close_generation = Some(self.generation);
        self.close_resources = self.created.clone();
        self.lifecycle = if self.emergency || self.fatal {
            Lifecycle::ClosingRestricted
        } else {
            Lifecycle::ClosingNormal
        };
    }
    pub fn close_cleanup(&self, id: u16) -> bool {
        self.lifecycle == Lifecycle::ClosingNormal
            && !self.emergency
            && !self.fatal
            && self.close_generation == Some(self.generation)
            && self.close_resources.contains(&id)
            && self
                .resources
                .get(&id)
                .is_some_and(|r| r.state == ResourceState::Retained)
    }
    pub fn ack(
        &mut self,
        exact: [u16; 6],
        retained: bool,
        conclusive: bool,
        durable: bool,
    ) -> bool {
        if let Some(receipt) = self.ack_receipt {
            return receipt == exact;
        }
        if !retained || !conclusive || !durable {
            return false;
        }
        self.ack_receipt = Some(exact);
        self.ack_owed = false;
        true
    }
    pub fn seal(&mut self) -> bool {
        if !matches!(
            self.lifecycle,
            Lifecycle::ClosingNormal | Lifecycle::ClosingRestricted
        ) || !self.quiescent
            || !self.workers.is_empty()
            || self.fatal
            || self.ingress != self.recorded_ingress
            || self.ack_owed
            || self.terminal_record != Record::Recorded
            || matches!(
                self.publication,
                Publication::Pending | Publication::Uncertain
            )
            || self.resources.values().any(|r| !r.discharged())
        {
            return false;
        }
        self.lifecycle = Lifecycle::SealedFinalizing;
        true
    }
    pub fn finish(&mut self, within_deadline: bool) -> bool {
        let clean = self.lifecycle == Lifecycle::SealedFinalizing
            && !self.fatal
            && within_deadline
            && self.end_records == [Record::Recorded; 2]
            && self.delivered;
        self.lifecycle = if clean {
            Lifecycle::ClosedClean
        } else {
            Lifecycle::ClosedUnclean
        };
        clean
    }
    pub fn conserved(&self) -> bool {
        self.created == self.resources.keys().copied().collect()
    }
}

/// Observations come from fixture scheduling/files/verified records. Never
/// feed a production PublicationClass, success bool or inventory count here.
#[derive(Clone, Copy, Debug)]
pub struct Physical {
    pub invocation_scheduled: bool,
    pub final_session_observation_precedes_stop: bool,
    pub target_created: bool,
    pub compensation_observed: bool,
    pub conclusive_absence_record: bool,
    pub recovery_record: bool,
}
pub fn physical_outcome(observed: Physical) -> (Publication, bool, bool) {
    let published_history = observed.target_created;
    if !observed.invocation_scheduled {
        return (Publication::NotDispatched, false, false);
    }
    if observed.target_created && !observed.compensation_observed {
        assert!(
            observed.final_session_observation_precedes_stop,
            "publication after earlier session observation denial"
        );
        return (Publication::Published, observed.recovery_record, true);
    }
    if observed.conclusive_absence_record {
        (
            Publication::Absent,
            observed.recovery_record,
            published_history,
        )
    } else {
        (
            Publication::Uncertain,
            observed.recovery_record,
            published_history,
        )
    }
}
