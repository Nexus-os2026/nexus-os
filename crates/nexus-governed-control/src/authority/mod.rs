//! The Phase Three authority core: platform-neutral.
//!
//! Nothing here performs a real-world effect. It decides whether one may
//! happen, records that decision, and gives the actuator a one-shot guard.

pub mod approval;
pub mod clock;
pub mod commitment;
pub mod effect;
pub mod evidence;
pub mod ids;
pub mod policy;
pub mod run;

#[cfg(test)]
pub(crate) mod scripted;
#[cfg(test)]
mod tests;

use self::approval::{AcknowledgementConfirmation, ControlConfirmer, ResumeConfirmation};
use self::clock::Clock;
use self::commitment::CommitmentRegistry;
use self::evidence::{EndStatus, EvidencePhase, EvidenceRecord, EvidenceSink};
use self::ids::{AgentId, RunId};
use self::policy::{GrantStore, PolicyGeneration};
use self::run::{Lift, Refusal, RunClass, RunOrigin, RunRegistry};
use std::fmt;
use std::sync::Arc;

/// Why the authority refused. The message is bounded and names no input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorityError {
    UnknownCommitment,
    UnknownRun,
    UnknownGrant,
    UnknownLease,
    WrongAgent,
    WrongRun,
    NotPending,
    NotAuthorized,
    Expired,
    Stale,
    GrantNotLive,
    NoCoveringGrant,
    RunCancelled,
    RunNotActive,
    EmergencyStopped,
    /// A newer emergency stop came after the stop the owner confirmed
    /// lifting: the resume lifted nothing.
    ResumeSuperseded,
    ApprovalRequired,
    ApprovalNotApplicable,
    ApprovalMismatch,
    Declined,
    TargetChanged,
    ParametersChanged,
    EvidenceUnavailable,
    Capacity,
    Closed(&'static str),
    InvalidAction(&'static str),
    Unavailable(&'static str),
    /// An effect that started ended with `cause` (it failed or was
    /// cancelled); `evidence` says what became of its records (EL-1 §8).
    Ended {
        cause: Box<AuthorityError>,
        evidence: evidence::ExecutionEvidence,
    },
}

impl AuthorityError {
    /// A stable class name (evidence and tests).
    pub fn class(&self) -> &'static str {
        match self {
            AuthorityError::UnknownCommitment => "unknown_commitment",
            AuthorityError::UnknownRun => "unknown_run",
            AuthorityError::UnknownGrant => "unknown_grant",
            AuthorityError::UnknownLease => "unknown_lease",
            AuthorityError::WrongAgent => "wrong_agent",
            AuthorityError::WrongRun => "wrong_run",
            AuthorityError::NotPending => "not_pending",
            AuthorityError::NotAuthorized => "not_authorized",
            AuthorityError::Expired => "expired",
            AuthorityError::Stale => "stale_policy_generation",
            AuthorityError::GrantNotLive => "grant_not_live",
            AuthorityError::NoCoveringGrant => "no_covering_grant",
            AuthorityError::RunCancelled => "run_cancelled",
            AuthorityError::RunNotActive => "run_not_active",
            AuthorityError::EmergencyStopped => "emergency_stopped",
            AuthorityError::ResumeSuperseded => "resume_superseded",
            AuthorityError::ApprovalRequired => "approval_required",
            AuthorityError::ApprovalNotApplicable => "approval_not_applicable",
            AuthorityError::ApprovalMismatch => "approval_mismatch",
            AuthorityError::Declined => "declined",
            AuthorityError::TargetChanged => "target_changed",
            AuthorityError::ParametersChanged => "parameters_changed",
            AuthorityError::EvidenceUnavailable => "evidence_unavailable",
            AuthorityError::Capacity => "capacity",
            AuthorityError::Closed(_) => "closed",
            AuthorityError::InvalidAction(_) => "invalid_action",
            AuthorityError::Unavailable(_) => "unavailable",
            AuthorityError::Ended { cause, .. } => cause.class(),
        }
    }

    /// What refused or ended it: for an effect that started, the cause its
    /// evidence is reported beside.
    pub fn cause(&self) -> &AuthorityError {
        match self {
            AuthorityError::Ended { cause, .. } => cause.cause(),
            other => other,
        }
    }
}

impl fmt::Display for AuthorityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthorityError::Closed(why) => write!(f, "governed control: closed: {why}"),
            AuthorityError::InvalidAction(why) => {
                write!(f, "governed control: invalid action: {why}")
            }
            AuthorityError::Unavailable(why) => write!(f, "governed control: unavailable: {why}"),
            AuthorityError::Ended { cause, evidence } => write!(f, "{cause} ({evidence})"),
            other => write!(f, "governed control: refused: {}", other.class()),
        }
    }
}

impl std::error::Error for AuthorityError {}

/// The authority core: the policy generation, the owner's grants, the runs
/// and the commitments, sharing one evidence sink and one clock.
pub struct Authority {
    generation: Arc<PolicyGeneration>,
    grants: Arc<GrantStore>,
    runs: Arc<RunRegistry>,
    commitments: CommitmentRegistry,
    evidence: Arc<dyn EvidenceSink>,
    clock: Arc<dyn Clock>,
}

impl Authority {
    pub fn new(evidence: Arc<dyn EvidenceSink>, clock: Arc<dyn Clock>) -> Self {
        let generation = Arc::new(PolicyGeneration::default());
        let grants = Arc::new(GrantStore::new(
            generation.clone(),
            evidence.clone(),
            clock.clone(),
        ));
        let runs = Arc::new(RunRegistry::default());
        let commitments = CommitmentRegistry::new(
            generation.clone(),
            grants.clone(),
            runs.clone(),
            evidence.clone(),
            clock.clone(),
        );
        Self {
            generation,
            grants,
            runs,
            commitments,
            evidence,
            clock,
        }
    }

    pub fn grants(&self) -> &GrantStore {
        &self.grants
    }

    pub fn commitments(&self) -> &CommitmentRegistry {
        &self.commitments
    }

    pub fn runs(&self) -> &Arc<RunRegistry> {
        &self.runs
    }

    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    pub fn policy_generation(&self) -> u64 {
        self.generation.current()
    }

    pub(crate) fn evidence(&self) -> &Arc<dyn EvidenceSink> {
        &self.evidence
    }

    pub(crate) fn generation(&self) -> &Arc<PolicyGeneration> {
        &self.generation
    }

    /// Take and release every lock of the authority once (tests: it blocks
    /// while another thread holds one).
    #[cfg(test)]
    pub(crate) fn probe_locks(&self) {
        self.commitments.probe_locks();
        self.grants.probe_locks();
        self.runs.probe_locks();
    }

    /// A gating record: what it permits happens only once it is written.
    fn record(&self, record: &EvidenceRecord) -> Result<(), AuthorityError> {
        self.evidence
            .record(record)
            .map(|_| ())
            .map_err(|_| AuthorityError::EvidenceUnavailable)
    }

    /// An end record: what it records has already taken effect (EL-1 §6).
    fn record_end(&self, record: &EvidenceRecord) -> EndStatus {
        self.evidence.record_end(record)
    }

    /// Record that the agent display is starting (it is a process Phase
    /// Three owns; its start needs a grant and no emergency stop): evidence
    /// first.
    pub(crate) fn record_display(
        &self,
        phase: EvidencePhase,
        detail: Vec<(String, String)>,
    ) -> Result<(), AuthorityError> {
        self.record(&self.display_record(phase, detail))
    }

    /// Record that the agent display stopped (it already has).
    pub(crate) fn record_display_end(
        &self,
        phase: EvidencePhase,
        detail: Vec<(String, String)>,
    ) -> EndStatus {
        self.record_end(&self.display_record(phase, detail))
    }

    /// Whether the evidence store requires the owner's acknowledgement of
    /// unsettled actions before anything is admitted (EL-1 §7).
    pub fn recovery_required(&self) -> bool {
        self.evidence
            .primary()
            .is_some_and(|primary| primary.health().recovery_required)
    }

    /// While recovery is required (EL-1 §7): the owner's acknowledgement,
    /// only through the native confirmation, of exactly the statements read
    /// before the dialog. Declined, failed or absent, nothing changes. It
    /// grants nothing, clears no stop and changes no outcome.
    fn acknowledge_recovery(&self, confirmer: &dyn ControlConfirmer) -> Result<(), AuthorityError> {
        let Some(primary) = self.evidence.primary() else {
            return Ok(());
        };
        let Some(recovery) = primary.recovery() else {
            return Ok(());
        };
        let request = AcknowledgementConfirmation {
            unsettled: recovery.unsettled,
            unclean_sessions: recovery.unclean_sessions,
            covers_through: recovery.covers_through,
        };
        if !confirmer.confirm_acknowledgement(&request) {
            return Err(AuthorityError::Declined);
        }
        primary
            .acknowledge(request.covers_through)
            .map_err(|_| AuthorityError::EvidenceUnavailable)?;
        // A statement newer than what was shown still holds it.
        if primary.health().recovery_required {
            return Err(AuthorityError::Unavailable(
                "a newer unsettled action needs acknowledgement",
            ));
        }
        Ok(())
    }

    fn display_record(
        &self,
        phase: EvidencePhase,
        detail: Vec<(String, String)>,
    ) -> EvidenceRecord {
        let mut record =
            EvidenceRecord::new(phase, self.clock.wall_ms(), self.generation.current());
        record.detail = detail;
        record
    }

    /// Open a run for `agent`: an agent's run, whatever the agent is
    /// called. Refused while an emergency stop is in force.
    pub(crate) fn open_run(
        &self,
        agent: AgentId,
        origin: RunOrigin,
    ) -> Result<RunId, AuthorityError> {
        self.open_classed(agent, origin, RunClass::Agent)
    }

    /// Open the run of one of the owner's own commands (the unified front
    /// door): the only way to an owner run. Its agent identity is a label
    /// for display and evidence; what makes it the owner's is the class the
    /// run keeps.
    pub(crate) fn open_owner_run(
        &self,
        modalities: Vec<String>,
    ) -> Result<(AgentId, RunId), AuthorityError> {
        let agent = AgentId::owner_session();
        let run = self.open_classed(
            agent.clone(),
            RunOrigin::Command { modalities },
            RunClass::Owner,
        )?;
        Ok((agent, run))
    }

    fn open_classed(
        &self,
        agent: AgentId,
        origin: RunOrigin,
        class: RunClass,
    ) -> Result<RunId, AuthorityError> {
        if self.runs.is_stopped() {
            return Err(AuthorityError::EmergencyStopped);
        }
        let mut record = EvidenceRecord::new(
            EvidencePhase::RunOpened,
            self.clock.wall_ms(),
            self.generation.current(),
        );
        record.agent = Some(agent.to_string());
        if class == RunClass::Owner {
            record.detail.push(("class".into(), "owner".into()));
        }
        let run = self.runs.open(agent, origin, class, self.clock.wall_ms())?;
        record.run = Some(run.to_string());
        if self.record(&record).is_err() {
            self.runs.cancel(run);
            return Err(AuthorityError::EvidenceUnavailable);
        }
        Ok(run)
    }

    /// Cancel one run: no further commitment of it can be authorized or
    /// started, the executing ones see their token set, and what it owns is
    /// released.
    pub fn cancel_run(&self, run: RunId) -> Result<(), AuthorityError> {
        let view = self.runs.view(run).ok_or(AuthorityError::UnknownRun)?;
        self.runs.cancel(run);
        self.commitments
            .revoke_run(run, &AuthorityError::RunCancelled);
        let mut record = EvidenceRecord::new(
            EvidencePhase::RunCancelled,
            self.clock.wall_ms(),
            self.generation.current(),
        );
        record.run = Some(run.to_string());
        record.agent = Some(view.agent.to_string());
        record.cancelled = true;
        // The cancellation has taken effect: its record may only be owed.
        self.record_end(&record);
        Ok(())
    }

    /// A run is done: nothing more can be prepared for it, what it left
    /// unconsumed ends, and what it still owns is released.
    pub(crate) fn finish_run(&self, run: RunId) {
        self.runs.finish(run);
        self.commitments
            .revoke_run(run, &AuthorityError::RunNotActive);
    }

    /// The owner's emergency stop: cancel every run, end every unconsumed
    /// commitment, move the policy generation, and refuse new runs until the
    /// owner resumes. Its record carries its stop number, whether that
    /// number is exhausted, and whether the stop before it was lifted.
    /// Everything takes effect first, unconditionally; only then are the
    /// records written, and they may only become owed (EL-1 §6).
    pub fn emergency_stop(&self) -> usize {
        let stopped = self.stop_effects(None);
        let cancelled = stopped.cancelled;
        self.write_stop_records(stopped);
        cancelled
    }

    /// The effects of an emergency stop, with no record written: the runs
    /// cancelled, every unconsumed commitment ended (its leases too), the
    /// generation moved. `reason` is recorded beside it.
    pub(crate) fn stop_effects(&self, reason: Option<&'static str>) -> Stopped {
        let stop = self.runs.stop_all();
        let revoked = self.commitments.revoke_all();
        self.generation.bump();
        let mut record = EvidenceRecord::new(
            EvidencePhase::EmergencyStop,
            self.clock.wall_ms(),
            self.generation.current(),
        );
        record.cancelled = true;
        record
            .detail
            .push(("runs_cancelled".into(), stop.cancelled.to_string()));
        record
            .detail
            .push(("stop_number".into(), stop.number.to_string()));
        if stop.exhausted {
            record.detail.push(("exhausted".into(), "yes".into()));
        }
        record
            .detail
            .push(("previous_stop_lifted".into(), stop.previous.as_str().into()));
        if let Some(reason) = reason {
            record.detail.push(("reason".into(), reason.into()));
        }
        Stopped {
            cancelled: stop.cancelled,
            revoked,
            record,
        }
    }

    /// The records of a stop that has taken effect: the commitments it
    /// ended, then the stop.
    pub(crate) fn write_stop_records(&self, stopped: Stopped) {
        self.commitments.write_ends(stopped.revoked);
        self.record_end(&stopped.record);
    }

    /// Lift an emergency stop: only after the owner's native confirmation,
    /// and only the stop the owner confirmed. The stop's number and count
    /// are read once under the runs lock and shown; after the answer a
    /// read-only check refuses, before any record, a resume that a newer
    /// stop, an exhausted stop number or quitting has overtaken; the record
    /// is written; and the registry lifts the stop only if it is still that
    /// stop. A Resumed record is therefore not proof that the stop was
    /// lifted.
    pub(crate) fn resume(&self, confirmer: &dyn ControlConfirmer) -> Result<(), AuthorityError> {
        // Recovery first: only after the owner's acknowledgement is the
        // resume itself asked (EL-1 §7).
        self.acknowledge_recovery(confirmer)?;
        let Some(stop) = self.runs.stop_to_lift().map_err(refused)? else {
            // Nothing is stopped: nothing to resume.
            return Ok(());
        };
        let request = ResumeConfirmation {
            stop_number: stop.number,
            runs_cancelled_by_the_stop: stop.cancelled,
        };
        if !confirmer.confirm_resume(&request) {
            return Err(AuthorityError::Declined);
        }
        if self.runs.check_lift(request.stop_number).map_err(refused)? == Lift::AlreadyLifted {
            // Another resume lifted this stop, and recorded it.
            return Ok(());
        }
        let mut record = EvidenceRecord::new(
            EvidencePhase::Resumed,
            self.clock.wall_ms(),
            self.generation.current(),
        );
        record
            .detail
            .push(("stop_number".into(), request.stop_number.to_string()));
        record.detail.push((
            "runs_cancelled_by_the_stop".into(),
            request.runs_cancelled_by_the_stop.to_string(),
        ));
        // Evidence first: an unrecorded resume does not happen.
        self.record(&record)?;
        self.runs.lift(request.stop_number).map_err(refused)?;
        Ok(())
    }
}

/// An emergency stop that has taken effect, its records still to write.
pub(crate) struct Stopped {
    pub(crate) cancelled: usize,
    revoked: commitment::Unwritten,
    record: EvidenceRecord,
}

/// What a resume the run registry refused returns.
fn refused(refusal: Refusal) -> AuthorityError {
    match refusal {
        Refusal::Superseded => AuthorityError::ResumeSuperseded,
        Refusal::Exhausted => AuthorityError::EmergencyStopped,
        Refusal::TooLate => AuthorityError::Closed("the desktop is quitting"),
        Refusal::Unavailable => AuthorityError::Unavailable("the run registry is poisoned"),
    }
}
