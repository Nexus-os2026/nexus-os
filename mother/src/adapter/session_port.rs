//! Mandatory File-only session port. Legacy doors remain a separate variant;
//! no session variant can adopt or invoke another effect domain.
use super::*;
use nexus_governed_control::host_session::{SessionContext, SessionDoor};

pub(super) enum PortDoor {
    Legacy(AgentDoor),
    Session(SessionDoor),
}
impl PortDoor {
    pub(super) fn context(&self) -> Option<SessionContext> {
        match self {
            Self::Legacy(_) => None,
            Self::Session(door) => Some(door.context()),
        }
    }
    pub(super) fn file_port(&self) -> Result<FilePort, FileSessionError> {
        match self {
            Self::Legacy(door) => door.file_port(),
            Self::Session(door) => door.file_port(),
        }
    }
    pub(super) fn claim_file_session(
        &self,
        attachment: &FileAttachment,
    ) -> Result<HaltCell, FileSessionError> {
        match self {
            Self::Legacy(door) => door.claim_file_session(attachment),
            Self::Session(door) => door.claim_file_session(attachment),
        }
    }
    pub(super) fn open_file_run(
        &self,
        attachment: &FileAttachment,
    ) -> Result<RunId, AuthorityError> {
        match self {
            Self::Legacy(door) => door.open_file_run(attachment),
            Self::Session(door) => door.open_file_run(attachment).map_err(Into::into),
        }
    }
    pub(super) fn open_run(&self) -> Result<RunId, AuthorityError> {
        match self {
            Self::Legacy(door) => door.open_run(),
            Self::Session(_) => Err(AuthorityError::Halted),
        }
    }
    pub(super) fn confirm_mission(
        &self,
        request: &MissionConfirmation,
    ) -> Result<MissionApproval, AuthorityError> {
        match self {
            Self::Legacy(door) => door.confirm_mission(request),
            Self::Session(door) => door.confirm_mission(request).map_err(Into::into),
        }
    }
    pub(super) fn confirm_held_mission(
        &self,
        request: &MissionResumeConfirmation,
    ) -> Result<MissionResumeApproval, AuthorityError> {
        match self {
            Self::Legacy(door) => door.confirm_held_mission(request),
            Self::Session(_) => Err(AuthorityError::Unavailable(
                "held resume is not a session route",
            )),
        }
    }
    pub(super) fn admit_files(
        &self,
        run: RunId,
        hash: &str,
        request: &str,
    ) -> Result<(), AuthorityError> {
        match self {
            Self::Legacy(door) => door.admit_files(run, hash, request),
            Self::Session(door) => door.admit_files(run, hash, request).map_err(Into::into),
        }
    }
    pub(super) fn agent_action(
        &self,
        run: RunId,
        intent: &Intent,
        effect: Option<u64>,
    ) -> Result<AgentOutcome, AuthorityError> {
        match self {
            Self::Legacy(door) => door.agent_action(run, intent),
            Self::Session(door) => match intent {
                Intent::File(file) => match effect {
                    Some(number) => door
                        .create_file_effect(run, file, number)
                        .map_err(Into::into),
                    None => Err(AuthorityError::NotAdmitted),
                },
                _ => Err(AuthorityError::Unavailable(
                    "only File creation is a session effect",
                )),
            },
        }
    }
    pub(super) fn cancel_run(&self, run: RunId) -> Result<RecordBatch, AuthorityError> {
        match self {
            Self::Legacy(door) => door.cancel_run(run),
            Self::Session(door) => door.cancel_run(run).map_err(Into::into),
        }
    }
    pub(super) fn finish_when_settled(&self, run: RunId) -> Result<(), AuthorityError> {
        match self {
            Self::Legacy(door) => door.finish_when_settled(run),
            Self::Session(door) => door.finish_when_settled(run).map_err(Into::into),
        }
    }
    pub(super) fn halt(&self, reason: HaltReason) -> RecordBatch {
        match self {
            Self::Legacy(door) => door.halt(reason),
            Self::Session(door) => door.halt(reason),
        }
    }
    pub(super) fn clock(&self) -> Arc<dyn nexus_governed_control::authority::clock::Clock> {
        match self {
            Self::Legacy(door) => door.clock(),
            Self::Session(door) => door.clock(),
        }
    }
    pub(super) fn emergency_stopped(&self) -> bool {
        match self {
            Self::Legacy(door) => door.emergency_stopped(),
            Self::Session(door) => door.emergency_stopped(),
        }
    }
    pub(super) fn commitment(&self, id: CommitmentId) -> Lookup<CommitmentView> {
        match self {
            Self::Legacy(door) => door.commitment(id),
            Self::Session(door) => door.commitment(id),
        }
    }
    pub(super) fn file_observation(
        &self,
        id: CommitmentId,
    ) -> Result<Lookup<(CommitmentView, PublicationClass)>, AuthorityError> {
        match self {
            Self::Legacy(door) => door.file_observation(id),
            Self::Session(door) => door.file_observation(id).map_err(Into::into),
        }
    }
    pub(super) fn adopt(
        &self,
        run: RunId,
        id: CommitmentId,
    ) -> Result<CommitmentView, AuthorityError> {
        match self {
            Self::Legacy(door) => door.adopt(run, id),
            Self::Session(_) => Err(AuthorityError::NotMember),
        }
    }
    pub(super) fn ack_commitment(&self, id: CommitmentId) -> Result<(), AuthorityError> {
        match self {
            Self::Legacy(door) => door.ack_commitment(id),
            Self::Session(door) => door.ack_commitment(id).map_err(Into::into),
        }
    }
}

/// Hands receives this closed port; its inner legacy adapter never escapes.
pub struct SessionPort {
    inner: GovernedPort,
}
impl SessionPort {
    pub fn new(door: SessionDoor, workspace: String) -> Self {
        Self {
            inner: GovernedPort::from_door(PortDoor::Session(door)).with_workspace(workspace),
        }
    }
    pub fn context(&self) -> SessionContext {
        match &self.inner.door {
            PortDoor::Session(door) => door.context(),
            PortDoor::Legacy(_) => unreachable!("SessionPort construction is mandatory"),
        }
    }
}
impl EffectPort for SessionPort {
    fn session_context(&self) -> Option<SessionContext> {
        Some(self.context())
    }
    fn file_workspace(&self) -> Option<&str> {
        self.inner.file_workspace()
    }
    fn file_attach(&self) -> Result<Option<Arc<FileAttachment>>, FileSessionError> {
        self.inner.file_attach()
    }
    fn file_claim(&self, attachment: Arc<FileAttachment>) -> Result<HaltCell, FileSessionError> {
        self.inner.file_claim(attachment)
    }
    fn file_admit(&self, mission: MissionId, admission: &Admission) -> Result<(), EffectRefusal> {
        self.inner.file_admit(mission, admission)
    }
    fn perform_file(
        &self,
        mission: MissionId,
        number: u64,
        effect: &EffectRequest,
    ) -> Result<EffectOutcome, EffectRefusal> {
        self.inner.perform_file(mission, number, effect)
    }
    fn file_publication_for(
        &self,
        mission: MissionId,
        effect: u64,
    ) -> Result<Option<FileSnapshot>, EffectRefusal> {
        self.inner.file_publication_for(mission, effect)
    }
    fn file_publication(&self, mission: MissionId) -> Result<Option<FileSnapshot>, EffectRefusal> {
        self.inner.file_publication(mission)
    }
    fn file_deadline_elapsed(&self, pending: &PendingAction) -> bool {
        self.inner.file_deadline_elapsed(pending)
    }
    fn file_ack(&self, pending: &PendingAction) -> Result<(), EffectRefusal> {
        self.inner.file_ack(pending)
    }
    fn perform(&self, _: MissionId, _: &EffectRequest) -> Result<EffectOutcome, EffectRefusal> {
        Err(refused(BrokerClass::NoBrokerRoute))
    }
    fn cancel(&self, mission: MissionId) {
        self.inner.cancel(mission)
    }
    fn finish(&self, mission: MissionId) {
        self.inner.finish(mission)
    }
    fn emergency_stopped(&self) -> bool {
        self.inner.emergency_stopped()
    }
    fn confirm_mission(
        &self,
        request: &MissionConfirmation,
    ) -> Result<MissionApproval, AuthorityError> {
        self.inner.confirm_mission(request)
    }
    fn confirm_held_mission(
        &self,
        _: &MissionResumeConfirmation,
    ) -> Result<MissionResumeApproval, AuthorityError> {
        Err(AuthorityError::Unavailable(
            "held resume is not a session route",
        ))
    }
    fn approval(&self, mission: MissionId) -> ApprovalStatus {
        self.inner.approval(mission)
    }
    fn pending(&self, mission: MissionId) -> Option<PendingAction> {
        self.inner.pending(mission)
    }
    fn adopt(&self, _: MissionId, _: &PendingAction) {
        self.context().fail()
    }
    fn settled(&self, pending: &PendingAction) -> bool {
        self.inner.settled(pending)
    }
    fn withdraw(&self, mission: MissionId) -> Result<(), EffectRefusal> {
        self.inner.withdraw(mission)
    }
    fn fence(&self) -> Owed {
        self.inner.fence()
    }
    fn hand(&self, mission: MissionId, pending: &PendingAction) {
        self.inner.hand(mission, pending)
    }
}
