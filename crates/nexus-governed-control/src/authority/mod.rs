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
mod tests;

use self::approval::{ControlConfirmer, ResumeConfirmation};
use self::clock::Clock;
use self::commitment::CommitmentRegistry;
use self::evidence::{EvidencePhase, EvidenceRecord, EvidenceSink};
use self::ids::{AgentId, RunId};
use self::policy::{GrantStore, PolicyGeneration};
use self::run::{RunOrigin, RunRegistry};
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

    fn record(&self, record: &EvidenceRecord) -> Result<(), AuthorityError> {
        self.evidence
            .record(record)
            .map_err(|_| AuthorityError::EvidenceUnavailable)
    }

    /// Open a run for `agent`. Refused while an emergency stop is in force.
    pub fn open_run(&self, agent: AgentId, origin: RunOrigin) -> Result<RunId, AuthorityError> {
        if self.runs.is_stopped() {
            return Err(AuthorityError::EmergencyStopped);
        }
        let mut record = EvidenceRecord::new(
            EvidencePhase::RunOpened,
            self.clock.wall_ms(),
            self.generation.current(),
        );
        record.agent = Some(agent.to_string());
        let run = self.runs.open(agent, origin, self.clock.wall_ms())?;
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
        self.record(&record)
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
    /// owner resumes.
    pub fn emergency_stop(&self) -> usize {
        let cancelled = self.runs.stop_all();
        self.commitments.revoke_all();
        self.generation.bump();
        let mut record = EvidenceRecord::new(
            EvidencePhase::EmergencyStop,
            self.clock.wall_ms(),
            self.generation.current(),
        );
        record.cancelled = true;
        record
            .detail
            .push(("runs_cancelled".into(), cancelled.to_string()));
        let _ = self.record(&record);
        cancelled
    }

    /// Lift an emergency stop: only after the owner's native confirmation.
    pub fn resume(&self, confirmer: &dyn ControlConfirmer) -> Result<(), AuthorityError> {
        if !self.runs.is_stopped() {
            return Ok(());
        }
        let request = ResumeConfirmation {
            runs_cancelled_by_the_stop: self.runs.cancelled_by_stop(),
        };
        if !confirmer.confirm_resume(&request) {
            return Err(AuthorityError::Declined);
        }
        let mut record = EvidenceRecord::new(
            EvidencePhase::Resumed,
            self.clock.wall_ms(),
            self.generation.current(),
        );
        record.detail.push((
            "runs_cancelled_by_the_stop".into(),
            request.runs_cancelled_by_the_stop.to_string(),
        ));
        // Evidence first: an unrecorded resume does not happen.
        self.record(&record)?;
        self.runs.resume();
        Ok(())
    }
}
