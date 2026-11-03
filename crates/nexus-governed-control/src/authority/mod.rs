//! The Phase Three authority core: platform-neutral.
//!
//! Nothing here performs a real-world effect. It decides whether one may
//! happen, records that decision, and gives the actuator a one-shot guard.

pub mod approval;
pub mod clock;
pub mod commitment;
pub mod effect;
pub mod evidence;
pub mod halt;
pub mod ids;
pub mod policy;
pub mod run;

#[cfg(test)]
mod privacy_tests;
#[cfg(test)]
pub(crate) mod scripted;
#[cfg(test)]
mod tests;

use self::approval::{
    AcknowledgementConfirmation, ControlConfirmer, MissionApproval, MissionConfirmation,
    MissionResumeApproval, MissionResumeConfirmation, ResumeConfirmation, MAX_MISSION_REQUEST,
    MAX_MISSION_SUMMARY_LINES,
};
use self::clock::Clock;
use self::commitment::CommitmentRegistry;
use self::evidence::{is_plain, EndStatus, EvidencePhase, EvidenceRecord, EvidenceSink, Privacy};
pub(crate) use self::evidence::{write_end, write_record};
use self::halt::{HaltCounters, RecordBatch};
use self::ids::{AgentId, RunId};
use self::policy::{GrantStore, PolicyGeneration};
use self::run::{GroupId, Lift, Refusal, RunClass, RunOrigin, RunRegistry};
use std::fmt;
use std::sync::Arc;

/// The I6 registry of `reason` codes: every name [`AuthorityError::class`]
/// gives, a door halt's reasons ([`halt::HaltReason`]) and the stop's
/// `recovery_required` (a test keeps them in step).
pub(crate) const REASONS: &[&str] = &[
    "unknown_commitment",
    "unknown_run",
    "unknown_grant",
    "unknown_lease",
    "wrong_agent",
    "wrong_run",
    "not_pending",
    "not_authorized",
    "expired",
    "stale_policy_generation",
    "grant_not_live",
    "no_covering_grant",
    "run_cancelled",
    "run_not_active",
    "emergency_stopped",
    "resume_superseded",
    "approval_required",
    "approval_not_applicable",
    "approval_mismatch",
    "declined",
    "target_changed",
    "parameters_changed",
    "evidence_unavailable",
    "capacity",
    "not_member",
    "halted",
    "closed",
    "invalid_action",
    "unavailable",
    "journal_failed",
    "recovery_required",
    "not_admitted",
    "no_consent",
    "already_admitted",
    "file_exists",
    "change_too_large_to_show",
    "unsupported_mount_topology",
    "model_not_registered",
    "model_identity_mismatch",
    "model_not_local",
    "model_invalid_address",
    "model_invalid_model_name",
    "model_cloud_name",
    "model_cloud_enabled",
    "model_not_installed",
    "model_digest_mismatch",
    "model_runtime_mismatch",
    "model_remote",
    "model_config_mismatch",
    "model_unavailable",
    "model_timeout",
    "model_request_too_large",
    "model_response_too_large",
    "model_malformed",
    "model_truncated",
    "model_tool_calls",
    "model_thinking_leaked",
    "model_empty_content",
    "model_over_budget",
    "model_stopped",
    "model_cancelled",
    "model_late",
];

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
    /// Mother Nexus M2c: the run is not in the calling door's group.
    NotMember,
    /// Mother Nexus M2c PR-B: the calling door, or the run's door group, is
    /// halted (final).
    Halted,
    /// M2d-B1: a Files intent on a run that holds no Files admission.
    NotAdmitted,
    /// M2d-B1: the door holds no usable cached yes of the Owner's to this
    /// contract hash and request (never given, given at another door, or
    /// invalidated by a Stop or a halt since).
    NoConsent,
    /// M2d-B1: the run is already admitted, to another consent.
    AlreadyAdmitted,
    /// M2d-B1: the file to create already exists (create-only).
    FileExists,
    /// M2d-B1: the change cannot be shown to the Owner whole.
    ChangeTooLargeToShow,
    /// M2d-B1: the workspace, or a protected or reserved location, is
    /// reached through a mount that is not a whole filesystem (a subtree
    /// bind mount), or the mount topology cannot be read or matched.
    UnsupportedMountTopology,
    /// M2e-B: no local model is registered in this process.
    ModelNotRegistered,
    /// M2e-B: the model identity a caller expects is not the registered
    /// one.
    ModelIdentityMismatch,
    /// M2e-B: the kernel's local model transport refused or ended a model
    /// call or a registration (its fixed code only).
    ModelCall(nexus_kernel::inference::ModelError),
    /// M2d-B2: a Files operation reached the Owner's workspace and its
    /// outcome is not established (a sync, a record or a cleanup failed):
    /// recovery is required, and nothing is called done.
    RecoveryRequired,
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
            AuthorityError::NotMember => "not_member",
            AuthorityError::Halted => "halted",
            AuthorityError::NotAdmitted => "not_admitted",
            AuthorityError::NoConsent => "no_consent",
            AuthorityError::AlreadyAdmitted => "already_admitted",
            AuthorityError::FileExists => "file_exists",
            AuthorityError::ChangeTooLargeToShow => "change_too_large_to_show",
            AuthorityError::UnsupportedMountTopology => "unsupported_mount_topology",
            AuthorityError::RecoveryRequired => "recovery_required",
            AuthorityError::ModelNotRegistered => "model_not_registered",
            AuthorityError::ModelIdentityMismatch => "model_identity_mismatch",
            AuthorityError::ModelCall(error) => crate::inference::model_class(*error),
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
    /// M2c PR-B: every door halt's records, accounted for.
    halt_counters: Arc<HaltCounters>,
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
            halt_counters: Arc::new(HaltCounters::default()),
        }
    }

    /// M2c PR-B: an empty record batch, counted from now on.
    pub(crate) fn batch(&self) -> RecordBatch {
        RecordBatch::new(self.evidence.clone(), self.halt_counters.clone())
    }

    /// M2c PR-B: the halt counters.
    pub(crate) fn halt_records(&self) -> halt::HaltRecords {
        self.halt_counters.snapshot()
    }

    /// The broker's test hooks (tests).
    #[cfg(test)]
    pub(crate) fn hooks(&self) -> &Arc<crate::test_hooks::Hooks> {
        self.commitments.hooks()
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
        write_record(&*self.evidence, record)
            .map(|_| ())
            .map_err(|_| AuthorityError::EvidenceUnavailable)
    }

    /// An end record: what it records has already taken effect (EL-1 §6).
    fn record_end(&self, record: &EvidenceRecord) -> EndStatus {
        write_end(&*self.evidence, record)
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
            Privacy::Owner,
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
    /// request; it grants nothing. (The owner's own front door: its labels
    /// are recorded as given.)
    pub fn confirm_mission(
        &self,
        request: &MissionConfirmation,
        confirmer: &dyn ControlConfirmer,
    ) -> Result<MissionApproval, AuthorityError> {
        self.confirm_mission_as(request, confirmer, Privacy::Owner, || Ok(()))
            .map(|(approval, _, ())| approval)
    }

    /// [`Self::confirm_mission`], its record of class `privacy`. An agent's
    /// door asks as `Restricted`: its holder supplies every label, so only
    /// Mother's controlled request label is admitted ([`controlled`]),
    /// refused before any dialog or record otherwise, and the record is
    /// sealed as Restricted. M2d-B1: `before_ask` runs once every check
    /// before the dialog has passed, immediately before the Owner is asked
    /// (a door reserves its consent slot there); refused, nothing is asked
    /// or recorded. What it returns is held across the dialog and returned
    /// with the approval and the record's own time.
    pub(crate) fn confirm_mission_as<T>(
        &self,
        request: &MissionConfirmation,
        confirmer: &dyn ControlConfirmer,
        privacy: Privacy,
        before_ask: impl FnOnce() -> Result<T, AuthorityError>,
    ) -> Result<(MissionApproval, u64, T), AuthorityError> {
        mission_text(&request.contract_hash, &request.request, &request.summary)?;
        if privacy == Privacy::Restricted {
            controlled(&request.request, None)?;
        }
        if self
            .evidence
            .primary()
            .is_some_and(|primary| primary.health().latched)
        {
            return Err(AuthorityError::EvidenceUnavailable);
        }
        self.mission_admissible()?;
        let reserved = before_ask()?;
        let yes = confirmer.confirm_mission(request);
        let phase = if yes {
            EvidencePhase::MissionConsented
        } else {
            EvidencePhase::MissionDeclined
        };
        let at_wall_ms = self.clock.wall_ms();
        let mut record = EvidenceRecord::new(phase, at_wall_ms, self.generation.current(), privacy);
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
        let seq = write_record(&*self.evidence, &record)
            .map_err(|_| AuthorityError::EvidenceUnavailable)?;
        Ok((
            MissionApproval::confirmed(request.contract_hash.clone(), request.request.clone(), seq),
            at_wall_ms,
            reserved,
        ))
    }

    /// The owner's yes to resuming one held mission of an agent (Mother
    /// Nexus M2c), only through the native confirmation of exactly
    /// `request`. Refused before the owner is asked when the request is
    /// not a SHA-256 hash with plain labels and a plain summary, when the
    /// store is latched, under an emergency stop or while quitting. A no is
    /// recorded (an end record: it permits nothing). A yes counts only once
    /// its record is written (evidence first) and not if an emergency stop
    /// or quitting came while the owner was asked; after the record it is
    /// checked again, and then the record stands with no approval. The
    /// approval records the owner's yes to the hash and the request: it
    /// resumes nothing and grants nothing. (A door calls this with the
    /// confirmer its host bound, through [`Self::confirm_held_mission_as`].)
    pub fn confirm_held_mission(
        &self,
        request: &MissionResumeConfirmation,
        confirmer: &dyn ControlConfirmer,
    ) -> Result<MissionResumeApproval, AuthorityError> {
        self.confirm_held_mission_as(request, confirmer, Privacy::Owner, || Ok(()))
            .map(|(approval, _, ())| approval)
    }

    /// [`Self::confirm_held_mission`], its record of class `privacy`. An
    /// agent's door asks as `Restricted`: only Mother's controlled mission
    /// id, request label and held reasons are admitted ([`controlled`]),
    /// refused before any dialog or record otherwise, and the record is
    /// sealed as Restricted. `before_ask` as for
    /// [`Self::confirm_mission_as`].
    pub(crate) fn confirm_held_mission_as<T>(
        &self,
        request: &MissionResumeConfirmation,
        confirmer: &dyn ControlConfirmer,
        privacy: Privacy,
        before_ask: impl FnOnce() -> Result<T, AuthorityError>,
    ) -> Result<(MissionResumeApproval, u64, T), AuthorityError> {
        mission_text(&request.contract_hash, &request.request, &request.summary)?;
        for label in [&request.mission, &request.held_reason] {
            if label.is_empty() || label.chars().count() > MAX_MISSION_REQUEST || !is_plain(label) {
                return Err(AuthorityError::InvalidAction(
                    "the held mission is not named by plain labels",
                ));
            }
        }
        if privacy == Privacy::Restricted {
            controlled(
                &request.request,
                Some((&request.mission, &request.held_reason)),
            )?;
        }
        if self
            .evidence
            .primary()
            .is_some_and(|primary| primary.health().latched)
        {
            return Err(AuthorityError::EvidenceUnavailable);
        }
        self.mission_admissible()?;
        let reserved = before_ask()?;
        let yes = confirmer.confirm_held_mission(request);
        let phase = if yes {
            EvidencePhase::MissionResumeConfirmed
        } else {
            EvidencePhase::MissionResumeDeclined
        };
        let at_wall_ms = self.clock.wall_ms();
        let mut record = EvidenceRecord::new(phase, at_wall_ms, self.generation.current(), privacy);
        record
            .detail
            .push(("mission".into(), request.mission.clone()));
        record
            .detail
            .push(("contract_hash".into(), request.contract_hash.clone()));
        record
            .detail
            .push(("request".into(), request.request.clone()));
        record
            .detail
            .push(("held_reason".into(), request.held_reason.clone()));
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
        // Evidence first: an unrecorded yes is no yes.
        let seq = write_record(&*self.evidence, &record)
            .map_err(|_| AuthorityError::EvidenceUnavailable)?;
        // After the record (M2c §2.6 step 6): the record says the owner said
        // yes; no approval exists if a stop or quitting came meanwhile.
        self.mission_admissible()?;
        Ok((
            MissionResumeApproval::confirmed(
                request.contract_hash.clone(),
                request.request.clone(),
                seq,
            ),
            at_wall_ms,
            reserved,
        ))
    }

    /// Whether a door may be issued now (Mother Nexus M2c): the store not
    /// latched, no recovery required, no emergency stop, not quitting.
    pub(crate) fn door_admissible(&self) -> Result<(), AuthorityError> {
        if let Some(primary) = self.evidence.primary() {
            let health = primary.health();
            if health.latched {
                return Err(AuthorityError::EvidenceUnavailable);
            }
            if health.recovery_required {
                return Err(AuthorityError::Unavailable(
                    "actions whose outcome was not recorded need acknowledgement",
                ));
            }
        }
        self.mission_admissible()
    }

    /// The record of the owner's answer to a door: a yes is a gating record
    /// (no door exists unless it is written); a no is an end record.
    pub(crate) fn record_door(
        &self,
        yes: bool,
        agent: &AgentId,
        door: &str,
    ) -> Result<(), AuthorityError> {
        let phase = if yes {
            EvidencePhase::AgentDoorOpened
        } else {
            EvidencePhase::AgentDoorDeclined
        };
        let mut record = EvidenceRecord::new(
            phase,
            self.clock.wall_ms(),
            self.generation.current(),
            Privacy::Restricted,
        );
        record.agent = Some(agent.to_string());
        record.detail.push(("door".into(), door.to_string()));
        if yes {
            self.record(&record)
        } else {
            self.record_end(&record);
            Ok(())
        }
    }

    /// Unclean sessions the evidence store reports (advisory only).
    pub(crate) fn unclean_sessions(&self) -> usize {
        self.evidence
            .primary()
            .map_or(0, |primary| primary.health().unclean_sessions)
    }

    /// Whether a mission may be consented to now: no emergency stop, not
    /// quitting.
    pub(crate) fn mission_admissible(&self) -> Result<(), AuthorityError> {
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
        let mut record = EvidenceRecord::new(
            phase,
            self.clock.wall_ms(),
            self.generation.current(),
            Privacy::Owner,
        );
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
        self.open_classed(agent, origin, RunClass::Agent, None)
    }

    /// Open an agent's run that joins a door's group at its insertion, in
    /// the same hold of the runs lock (Mother Nexus M2c).
    pub(crate) fn open_run_in(
        &self,
        agent: AgentId,
        group: GroupId,
    ) -> Result<RunId, AuthorityError> {
        self.open_classed(agent, RunOrigin::AgentGoal, RunClass::Agent, Some(group))
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
            None,
        )?;
        Ok((agent, run))
    }

    fn open_classed(
        &self,
        agent: AgentId,
        origin: RunOrigin,
        class: RunClass,
        group: Option<GroupId>,
    ) -> Result<RunId, AuthorityError> {
        if self.runs.is_stopped() {
            return Err(AuthorityError::EmergencyStopped);
        }
        let mut record = EvidenceRecord::new(
            EvidencePhase::RunOpened,
            self.clock.wall_ms(),
            self.generation.current(),
            class.privacy(),
        );
        record.agent = Some(agent.to_string());
        if class == RunClass::Owner {
            record.detail.push(("class".into(), "owner".into()));
        }
        let run = self
            .runs
            .open_in(agent, origin, class, self.clock.wall_ms(), group)?;
        #[cfg(test)]
        self.hooks().fire(crate::test_hooks::at::OPEN_INSERTED);
        record.run = Some(run.to_string());
        if self.record(&record).is_err() {
            self.runs.cancel(run);
            return Err(AuthorityError::EvidenceUnavailable);
        }
        // M2c PR-B, after the record: a door's run its halt cancelled meanwhile
        // is refused (an earlier opening may still return one, cancelled for
        // good: every call on it refuses).
        if group.is_some() && self.runs.view(run).is_some_and(|view| view.cancelled) {
            return Err(AuthorityError::Halted);
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
            self.runs.privacy(run),
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
            Privacy::Owner,
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
            Privacy::Owner,
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

/// The labels an agent's door may give the owner's mission and
/// held-mission dialogs (P0-C): only Mother's controlled values, exactly as
/// Mother writes them. A request label `{mission}/{seq}/{number}`; for a
/// held mission, also its mission id and one of the held reasons Mother
/// names. Anything else is refused before any dialog or record. (The
/// contract hash and the plain summary are checked by [`mission_text`];
/// the summary is recorded only as its digest.)
fn controlled(request: &str, held: Option<(&str, &str)>) -> Result<(), AuthorityError> {
    if !evidence::is_mission_request(request) {
        return Err(AuthorityError::InvalidAction(
            "the mission request is not a controlled label",
        ));
    }
    if let Some((mission, reason)) = held {
        if !evidence::is_mission_id(mission) || !evidence::is_held_reason(reason) {
            return Err(AuthorityError::InvalidAction(
                "the held mission is not named by controlled labels",
            ));
        }
    }
    Ok(())
}

/// The text of a mission or held-mission request: a SHA-256 hex digest, a
/// plain request label, and a plain summary the owner reads whole.
fn mission_text(hash: &str, label: &str, summary: &[String]) -> Result<(), AuthorityError> {
    if hash.len() != 64 || !hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(AuthorityError::InvalidAction(
            "the contract hash is not a sha-256 hex digest",
        ));
    }
    if label.is_empty() || label.chars().count() > MAX_MISSION_REQUEST || !is_plain(label) {
        return Err(AuthorityError::InvalidAction(
            "the mission request is not a plain label",
        ));
    }
    // The owner must read exactly what is consented to.
    if summary.is_empty()
        || summary.len() > MAX_MISSION_SUMMARY_LINES
        || !summary.iter().all(|line| is_plain(line))
    {
        return Err(AuthorityError::InvalidAction("mission text is not plain"));
    }
    Ok(())
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
