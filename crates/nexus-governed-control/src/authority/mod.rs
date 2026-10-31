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

use self::approval::{
    AcknowledgementConfirmation, ControlConfirmer, MissionApproval, MissionConfirmation,
    ResumeConfirmation, MAX_MISSION_REQUEST, MAX_MISSION_SUMMARY_LINES,
};
use self::clock::Clock;
use self::commitment::CommitmentRegistry;
use self::evidence::{is_plain, EndStatus, EvidencePhase, EvidenceRecord, EvidenceSink};
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

    /// Record kernel work the owner's emergency stop ended (M0-FIX2 F10):
    /// an end record (the halt has taken effect) with the work's area, the
    /// stop's number, the outcome and, for a failed cleanup, its reason;
    /// never the work's command, prompt, response or credentials.
    pub(crate) fn record_kernel_halt(
        &self,
        area: &'static str,
        stop_number: u64,
        outcome: &'static str,
        cleanup: Option<&str>,
    ) -> EndStatus {
        let mut record = EvidenceRecord::new(
            EvidencePhase::KernelHalt,
            self.clock.wall_ms(),
            self.generation.current(),
        );
        record.outcome = Some(outcome);
        record.detail.push(("area".into(), area.into()));
        record
            .detail
            .push(("stop_number".into(), stop_number.to_string()));
        if let Some(cleanup) = cleanup {
            record.detail.push(("cleanup".into(), cleanup.into()));
        }
        self.record_end(&record)
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

    /// The owner's consent to an agent's mission (Mother Nexus M2a), only
    /// through the native confirmation of exactly `request`. Refused before
    /// the owner is asked when the request is not a SHA-256 hash with a
    /// plain label and plain summary, when the store is latched, under an
    /// emergency stop or while quitting. A no is recorded (an end record:
    /// it permits nothing). A yes counts only once its record is written
    /// (evidence first), and not if an emergency stop or quitting came
    /// while the owner was asked. The approval names the hash and the
    /// request; it grants nothing.
    pub fn confirm_mission(
        &self,
        request: &MissionConfirmation,
        confirmer: &dyn ControlConfirmer,
    ) -> Result<MissionApproval, AuthorityError> {
        let hash = &request.contract_hash;
        if hash.len() != 64 || !hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            return Err(AuthorityError::InvalidAction(
                "the contract hash is not a sha-256 hex digest",
            ));
        }
        let label = &request.request;
        if label.is_empty() || label.chars().count() > MAX_MISSION_REQUEST || !is_plain(label) {
            return Err(AuthorityError::InvalidAction(
                "the mission request is not a plain label",
            ));
        }
        // The owner must read exactly what is consented to.
        if request.summary.is_empty()
            || request.summary.len() > MAX_MISSION_SUMMARY_LINES
            || !request.summary.iter().all(|line| is_plain(line))
        {
            return Err(AuthorityError::InvalidAction("mission text is not plain"));
        }
        if self
            .evidence
            .primary()
            .is_some_and(|primary| primary.health().latched)
        {
            return Err(AuthorityError::EvidenceUnavailable);
        }
        self.mission_admissible()?;
        let yes = confirmer.confirm_mission(request);
        let phase = if yes {
            EvidencePhase::MissionConsented
        } else {
            EvidencePhase::MissionDeclined
        };
        let mut record =
            EvidenceRecord::new(phase, self.clock.wall_ms(), self.generation.current());
        record
            .detail
            .push(("contract_hash".into(), request.contract_hash.clone()));
        record
            .detail
            .push(("request".into(), request.request.clone()));
        record
            .detail
            .push(("summary_digest".into(), summary_digest(&request.summary)));
        if !yes {
            // The owner's decline has taken effect and permits nothing.
            self.record_end(&record);
            return Err(AuthorityError::Declined);
        }
        // A stop pressed, or quitting begun, while the owner was asked wins.
        self.mission_admissible()?;
        // Evidence first: an unrecorded consent is no consent.
        let seq = self
            .evidence
            .record(&record)
            .map_err(|_| AuthorityError::EvidenceUnavailable)?;
        Ok(MissionApproval::confirmed(
            request.contract_hash.clone(),
            request.request.clone(),
            seq,
        ))
    }

    /// Whether a mission may be consented to now: no emergency stop, not
    /// quitting.
    fn mission_admissible(&self) -> Result<(), AuthorityError> {
        if self.runs.is_stopped() {
            return Err(AuthorityError::EmergencyStopped);
        }
        if self.runs.is_closed() {
            return Err(AuthorityError::Closed("the desktop is quitting"));
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
    /// Tests only (M0-FIX F5): the owner's Stop goes through
    /// `GovernedControl::emergency_stop`, which also engages the kernel's
    /// stop; this authority-level stop alone would not, so production code
    /// cannot call it.
    #[cfg(test)]
    pub(crate) fn emergency_stop(&self) -> usize {
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
        // A latched store can record nothing in this process: refused before
        // any dialog, and nothing changes (C-6).
        if self
            .evidence
            .primary()
            .is_some_and(|primary| primary.health().latched)
        {
            return Err(AuthorityError::EvidenceUnavailable);
        }
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

/// The digest of a mission summary as shown: SHA-256 over a domain and each
/// line followed by a newline (no line holds one).
fn summary_digest(summary: &[String]) -> String {
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"nexus-p3-mission-summary-v1\n");
    for line in summary {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
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
