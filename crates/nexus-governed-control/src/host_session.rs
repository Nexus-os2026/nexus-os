//! The Hands session's original identity and lock-free input boundary.
//!
//! A seed is issued before input or Home opening. It carries no raw broker,
//! door or kernel object. Acceptance binds a request; it does not admit work.
use crate::authority::file_session::HaltCell;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub(crate) enum ScopeKind {
    Bootstrap,
    Work,
    Restoration,
    CloseCleanup,
}

/// Immutable request facts shared by its single-use child admissions. This
/// contains no cell: binding it cannot create an ownership cycle or a new
/// authority source. The cell remains the one issued before input.
#[derive(Debug)]
pub(crate) struct RequestScope {
    core: std::sync::Weak<Core>,
    session: SessionIdentity,
    request: u64,
    snapshot: u64,
    deadline: Instant,
    kind: ScopeKind,
    next_step: AtomicU64,
}
impl RequestScope {
    pub(crate) fn live(&self, word: u64) -> bool {
        word == self.snapshot && self.eligible()
    }
    fn eligible(&self) -> bool {
        let invalid = self.core.strong_count() == 0
            || Instant::now() >= self.deadline
            || self.snapshot & (FAILED | SEALED | EMERGENCY) != 0
            || match self.kind {
                ScopeKind::Bootstrap => self.snapshot & (READY | CLOSING | 7) != 0,
                ScopeKind::Work => self.snapshot & (READY | CLOSING | 7) != READY,
                ScopeKind::Restoration => self.snapshot & (READY | CLOSING) != READY,
                ScopeKind::CloseCleanup => self.snapshot & (READY | CLOSING) != (READY | CLOSING),
            };
        !invalid
    }
    pub(crate) fn admit(&self, cell: &HaltCell) -> Result<(), crate::authority::AuthorityError> {
        use crate::authority::AuthorityError;
        if !self.eligible() {
            return Err(AuthorityError::Halted);
        }
        self.next_step
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| old.checked_add(1))
            .map_err(|_| {
                if let Some(core) = self.core.upgrade() {
                    SessionIngress { core }.fail();
                }
                AuthorityError::Capacity
            })?;
        // No refresh on CAS failure. Every child carries this original word.
        cell.session_compare(self.snapshot, self.snapshot)
            .map_err(|_| AuthorityError::Halted)
    }
}

pub(crate) const CLOSING: u64 = 0x08;
pub(crate) const FAILED: u64 = 0x10;
pub(crate) const SEALED: u64 = 0x20;
pub(crate) const READY: u64 = 0x40;
pub(crate) const EMERGENCY: u64 = 0x80;
pub(crate) const EPOCH_STEP: u64 = 1 << 24;
pub(crate) const EPOCH_MASK: u64 = !(EPOCH_STEP - 1);

/// Opaque process-local identity, never reconstructed from persistent data.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SessionIdentity([u8; 16]);
impl fmt::Debug for SessionIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionIdentity(opaque)")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionState {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionError {
    Unavailable,
    Restricted,
    SessionEnded,
    NoTarget,
    Sequence,
    Exhausted,
    Backend(&'static str),
}
impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "session refused: {self:?}")
    }
}
impl std::error::Error for SessionError {}

/// Typed native failure data. This wraps no broker or execution capability.
#[derive(Debug)]
pub struct SessionFailure {
    error: crate::authority::AuthorityError,
}
impl SessionFailure {
    pub fn class(&self) -> &'static str {
        self.error.class()
    }
}
impl From<SessionFailure> for crate::authority::AuthorityError {
    fn from(error: SessionFailure) -> Self {
        error.error
    }
}
impl From<crate::authority::AuthorityError> for SessionFailure {
    fn from(error: crate::authority::AuthorityError) -> Self {
        Self { error }
    }
}

/// Closed command vocabulary; unsupported routes cannot become generic intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandKind {
    Run,
    Approve,
    Undo,
    Cleanup,
    Detail,
    Status,
    Evidence,
    Quit,
    Lift,
}

struct CloseIdentity {
    sequence: u64,
    generation: u64,
    deadline: Instant,
}

/// All authority restrictions and the original first cause reside in cell's
/// single atomic word. The other atomics publish non-authoritative indices.
struct Core {
    identity: SessionIdentity,
    cell: HaltCell,
    sequence: AtomicU64,
    pending_context: AtomicU64,
    undo_context: AtomicU64,
    next_target: AtomicU64,
    pending: std::sync::Mutex<Option<PendingContext>>,
    close: OnceLock<CloseIdentity>,
    created: Instant,
}

struct PendingContext {
    identity: u64,
    run: crate::authority::ids::RunId,
    commitment: crate::authority::ids::CommitmentId,
}

/// One bootstrap owner. Its ingress may be cloned, but its identity cannot be
/// adopted by another session or replaced after a Stop.
pub struct SessionSeed {
    core: Arc<Core>,
}

/// Input producer's memory-only interface. It never acquires broker,
/// coordinator, resource or output locks and never performs recording.
#[derive(Clone)]
pub struct SessionIngress {
    core: Arc<Core>,
}

/// Immutable command envelope. A child must carry this same snapshot; a fresh
/// code, dequeue or callback does not create a fresh envelope.
pub struct SessionRequest {
    core: Arc<Core>,
    kind: CommandKind,
    sequence: u64,
    snapshot: u64,
    target: u64,
    deadline: Instant,
    close: Option<u64>,
    scope: Arc<RequestScope>,
}
impl SessionRequest {
    pub fn session(&self) -> SessionIdentity {
        self.core.identity
    }
    pub fn kind(&self) -> CommandKind {
        self.kind
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn generation(&self) -> u64 {
        self.snapshot / EPOCH_STEP
    }
    pub fn context(&self) -> Option<u64> {
        (self.target != 0).then_some(self.target)
    }
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
    /// This is a correlation identity, not a cleanup capability.
    pub fn close_identity(&self) -> Option<u64> {
        self.close
    }
    pub fn close_generation(&self) -> Option<u64> {
        self.close
            .and_then(|_| self.core.close.get().map(|close| close.generation))
    }
}

/// The Mother's operational sinks. An admission consumes one child identity;
/// it carries no raw owner, door, kernel object or callback authority.
#[derive(Clone, Copy, Debug)]
pub enum MotherStep {
    Transition,
    ConsentReservation,
    PhaseReservation,
    LaneInvocation,
    PortProjection,
    PrivateDirectory,
    PrivateTemporary,
    PrivateWrite,
    PrivateSync,
    PrivateRename,
    PrivateParentSync,
    PrivateErrorRemoval,
}

/// Original request context for Mother's private sinks. Creation is restricted
/// to the broker facade; cloning never refreshes a deadline or snapshot.
#[derive(Clone)]
pub struct SessionContext {
    cell: HaltCell,
    scope: Arc<RequestScope>,
}
impl SessionContext {
    pub fn session(&self) -> SessionIdentity {
        self.scope.session
    }
    pub fn request(&self) -> u64 {
        self.scope.request
    }
    pub fn deadline(&self) -> Instant {
        self.scope.deadline
    }
    /// This returns no capability. The owning private sink must do its one
    /// mutation or invocation immediately, without further validation or I/O.
    pub fn admit(&self, _step: MotherStep) -> Result<(), SessionError> {
        self.cell.admit().map_err(|_| SessionError::Restricted)
    }
    pub fn fail(&self) {
        if let Some(core) = self.scope.core.upgrade() {
            SessionIngress { core }.fail();
        }
    }
}

/// The Hands owner surface. Its backend is constructed with the original
/// pre-input cell. There is no raw-handle getter or conversion to a legacy
/// broker. Native consent remains required by every supported work route.
pub struct HostSession {
    core: Arc<Core>,
    control: Arc<crate::governed::GovernedControl>,
    confirmer: Arc<dyn crate::authority::approval::ControlConfirmer + Send + Sync>,
    bootstrap: SessionContext,
}

/// Mother's closed File door. Its raw legacy representation is private and
/// cannot be extracted, adopted, resumed, or used for another effect domain.
pub struct SessionDoor {
    door: crate::governed::AgentDoor,
    context: SessionContext,
    core: Arc<Core>,
}

impl HostSession {
    pub fn open(
        seed: SessionSeed,
        root: &std::path::Path,
        evidence: Arc<crate::evidence_store::PrimaryEvidence>,
        clock: Arc<dyn crate::authority::clock::Clock>,
        confirmer: Arc<dyn crate::authority::approval::ControlConfirmer + Send + Sync>,
    ) -> Result<Self, SessionError> {
        let sessions = Arc::new(crate::authority::file_session::FileSessions::host(
            seed.core.cell.clone(),
        ));
        let scope = Arc::new(RequestScope {
            core: Arc::downgrade(&seed.core),
            session: seed.core.identity,
            request: 0,
            snapshot: 0,
            deadline: seed.core.created + Duration::from_secs(300),
            kind: ScopeKind::Bootstrap,
            next_step: AtomicU64::new(0),
        });
        let bootstrap = SessionContext {
            cell: seed.core.cell.scoped(scope.clone()),
            scope,
        };
        let control = crate::governed::GovernedControl::with_file_sessions(
            root,
            crate::broker::Vault::Disabled.source(),
            evidence,
            clock,
            sessions,
        )
        .map_err(|error| {
            seed.ingress().fail();
            SessionError::Backend(error.class())
        })?;
        Ok(Self {
            core: seed.core,
            control: Arc::new(control),
            confirmer,
            bootstrap,
        })
    }

    pub fn identity(&self) -> SessionIdentity {
        self.core.identity
    }
    pub fn state(&self) -> SessionState {
        state(self.core.cell.session_word())
    }
    pub fn ingress(&self) -> SessionIngress {
        SessionIngress {
            core: self.core.clone(),
        }
    }
    pub fn context(&self, request: &SessionRequest) -> Result<SessionContext, SessionError> {
        if !Arc::ptr_eq(&self.core, &request.core)
            || !matches!(request.kind, CommandKind::Run | CommandKind::Approve)
        {
            return Err(SessionError::Restricted);
        }
        Ok(SessionContext {
            cell: self.core.cell.scoped(request.scope.clone()),
            scope: request.scope.clone(),
        })
    }
    pub fn recovery_report(&self) -> Result<Vec<crate::files::RecoveryEntry>, SessionError> {
        self.control
            .files_recovery_report()
            .map_err(|error| SessionError::Backend(error.class()))
    }
    pub fn bootstrap_hold(&self, hold: Duration) -> Result<(), SessionError> {
        self.control
            .files()
            .set_hold_with_halt(hold, Some(&self.bootstrap.cell))
            .map_err(|error| SessionError::Backend(error.class()))
    }
    pub fn bootstrap_workspace(
        &self,
        path: &std::path::Path,
        reserved: &[&std::path::Path],
    ) -> Result<crate::files::WorkspaceInfo, SessionError> {
        self.control
            .register_workspace_with_halt(path, reserved, Some(&self.bootstrap.cell))
            .map_err(|error| SessionError::Backend(error.class()))
    }

    /// Last bootstrap transition, invoked by the trusted host only after its
    /// three-source report has succeeded. A late result never installs ready.
    pub fn finish_bootstrap(&self) -> Result<(), SessionError> {
        if self.control.authority().recovery_required()
            || !self.recovery_report()?.is_empty()
            || self.control.workspace().is_none()
        {
            return Err(SessionError::Restricted);
        }
        self.core
            .cell
            .session_compare(0, READY)
            .map_err(|_| SessionError::Restricted)
    }

    pub fn workspace(&self) -> Option<crate::files::WorkspaceInfo> {
        self.control.workspace()
    }

    pub fn open_file_door(&self, request: &SessionRequest) -> Result<SessionDoor, SessionError> {
        self.open_door_with_grant(
            request,
            request.deadline.saturating_duration_since(Instant::now()),
        )
    }

    fn open_door_with_grant(
        &self,
        request: &SessionRequest,
        grant_lifetime: Duration,
    ) -> Result<SessionDoor, SessionError> {
        if request.kind != CommandKind::Run {
            return Err(SessionError::Restricted);
        }
        let context = self.context(request)?;
        let workspace = self.control.workspace().ok_or(SessionError::Unavailable)?;
        self.control
            .request_grant_with_halt(
                &crate::governed::GrantRequest::Files {
                    workspace: workspace.id,
                },
                grant_lifetime,
                &*self.confirmer,
                Some(&context.cell),
            )
            .map_err(|error| SessionError::Backend(error.class()))?;
        let door = self
            .control
            .issue_agent_door_with_halt(
                crate::authority::ids::AgentId::new("mother").ok_or(SessionError::Unavailable)?,
                self.confirmer.clone(),
                Some(context.cell.clone()),
            )
            .map_err(|error| SessionError::Backend(error.class()))?;
        Ok(SessionDoor {
            door,
            context,
            core: self.core.clone(),
        })
    }

    pub fn approve_file(
        &self,
        request: &SessionRequest,
    ) -> Result<crate::control::EffectOutput, SessionFailure> {
        use crate::authority::AuthorityError;
        if !Arc::ptr_eq(&self.core, &request.core) || request.kind != CommandKind::Approve {
            return Err(AuthorityError::Halted.into());
        }
        let target = self.core.pending.lock().map_err(|_| {
            self.ingress().fail();
            SessionFailure::from(AuthorityError::Halted)
        })?;
        let target = target
            .as_ref()
            .filter(|t| t.identity == request.target)
            .ok_or(AuthorityError::UnknownCommitment)?;
        let cell = self.core.cell.scoped(request.scope.clone());
        cell.admit()?;
        let agent =
            crate::authority::ids::AgentId::new("mother").ok_or(AuthorityError::WrongAgent)?;
        self.control
            .authorize(target.commitment, &agent, target.run, &*self.confirmer)?;
        cell.admit()?;
        self.control
            .execute(target.commitment, &agent, target.run)
            .map_err(Into::into)
    }
}

impl Drop for HostSession {
    fn drop(&mut self) {
        // Owner destruction closes authority in memory. It never performs
        // cleanup, recording, a join, or a poison-sensitive metadata lookup.
        self.ingress().fail();
    }
}

impl SessionDoor {
    pub fn agent(&self) -> &crate::authority::ids::AgentId {
        self.door.agent()
    }
    pub fn context(&self) -> SessionContext {
        self.context.clone()
    }
    pub fn file_port(
        &self,
    ) -> Result<
        crate::authority::file_session::FilePort,
        crate::authority::file_session::FileSessionError,
    > {
        self.door.file_port()
    }
    pub fn claim_file_session(
        &self,
        attachment: &crate::authority::file_session::FileAttachment,
    ) -> Result<HaltCell, crate::authority::file_session::FileSessionError> {
        self.door.claim_file_session(attachment)
    }
    pub fn open_file_run(
        &self,
        attachment: &crate::authority::file_session::FileAttachment,
    ) -> Result<crate::authority::ids::RunId, SessionFailure> {
        self.door.open_file_run(attachment).map_err(Into::into)
    }
    pub fn confirm_mission(
        &self,
        request: &crate::authority::approval::MissionConfirmation,
    ) -> Result<crate::authority::approval::MissionApproval, SessionFailure> {
        self.door.confirm_mission(request).map_err(Into::into)
    }
    pub fn admit_files(
        &self,
        run: crate::authority::ids::RunId,
        contract_hash: &str,
        request: &str,
    ) -> Result<(), SessionFailure> {
        self.door
            .admit_files(run, contract_hash, request)
            .map_err(Into::into)
    }
    pub fn create_file(
        &self,
        run: crate::authority::ids::RunId,
        file: &crate::files::FileCreate,
    ) -> Result<crate::governed::AgentOutcome, SessionFailure> {
        let identity = self
            .core
            .next_target
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| old.checked_add(1))
            .map_err(|_| {
                self.context.fail();
                SessionFailure::from(crate::authority::AuthorityError::Capacity)
            })?
            + 1;
        let outcome = self
            .door
            .agent_action(run, &crate::governed::Intent::File(file.clone()))
            .map_err(SessionFailure::from)?;
        if let crate::governed::AgentOutcome::AwaitingApproval(view) = &outcome {
            let mut pending = self.core.pending.lock().map_err(|_| {
                self.context.fail();
                SessionFailure::from(crate::authority::AuthorityError::Halted)
            })?;
            // A late result remains observable, but cannot install a target
            // for a new approval after the original request was invalidated.
            if self.context.cell.admit().is_ok() {
                *pending = Some(PendingContext {
                    identity,
                    run,
                    commitment: view.id,
                });
                self.core.pending_context.store(identity, Ordering::SeqCst);
            }
        }
        Ok(outcome)
    }
    pub fn cancel_run(
        &self,
        run: crate::authority::ids::RunId,
    ) -> Result<crate::governed::RecordBatch, SessionFailure> {
        self.door.cancel_run(run).map_err(Into::into)
    }
    pub fn finish_when_settled(
        &self,
        run: crate::authority::ids::RunId,
    ) -> Result<(), SessionFailure> {
        self.door.finish_when_settled(run).map_err(Into::into)
    }
    pub fn halt(&self, reason: crate::governed::HaltReason) -> crate::governed::RecordBatch {
        self.door.halt(reason)
    }
    pub fn clock(&self) -> Arc<dyn crate::authority::clock::Clock> {
        self.door.clock()
    }
    pub fn emergency_stopped(&self) -> bool {
        self.door.emergency_stopped()
    }
    pub fn commitment(
        &self,
        id: crate::authority::ids::CommitmentId,
    ) -> crate::governed::Lookup<crate::authority::commitment::CommitmentView> {
        self.door.commitment(id)
    }
    pub fn file_observation(
        &self,
        id: crate::authority::ids::CommitmentId,
    ) -> Result<
        crate::governed::Lookup<(
            crate::authority::commitment::CommitmentView,
            crate::authority::commitment::PublicationClass,
        )>,
        SessionFailure,
    > {
        self.door.file_observation(id).map_err(Into::into)
    }
    pub fn file_receipt(
        &self,
        id: crate::authority::ids::CommitmentId,
    ) -> Option<crate::files::FileReceipt> {
        self.door.file_receipt(id)
    }
    pub fn ack_commitment(
        &self,
        _id: crate::authority::ids::CommitmentId,
    ) -> Result<(), SessionFailure> {
        // A legacy boolean ack cannot establish the mandatory durable Mother
        // reference. The exact receipt path supplies that proof separately.
        Err(crate::authority::AuthorityError::EvidenceUnavailable.into())
    }
}

#[cfg(test)]
impl HostSession {
    pub(crate) fn control_for_test(&self) -> Arc<crate::governed::GovernedControl> {
        self.control.clone()
    }
    pub(crate) fn door_for_test(
        &self,
        request: &SessionRequest,
        grant_lifetime: Duration,
    ) -> Result<SessionDoor, SessionError> {
        self.open_door_with_grant(request, grant_lifetime)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IngressEvent {
    generation: u64,
}
impl IngressEvent {
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

impl SessionSeed {
    pub fn new() -> Result<Self, SessionError> {
        let mut identity = [0; 16];
        getrandom::getrandom(&mut identity).map_err(|_| SessionError::Unavailable)?;
        Ok(Self {
            core: Arc::new(Core {
                identity: SessionIdentity(identity),
                cell: HaltCell::new_host(),
                sequence: AtomicU64::new(0),
                pending_context: AtomicU64::new(0),
                undo_context: AtomicU64::new(0),
                next_target: AtomicU64::new(0),
                pending: std::sync::Mutex::new(None),
                close: OnceLock::new(),
                created: Instant::now(),
            }),
        })
    }
    pub fn ingress(&self) -> SessionIngress {
        SessionIngress {
            core: self.core.clone(),
        }
    }
    pub fn identity(&self) -> SessionIdentity {
        self.core.identity
    }
    pub fn state(&self) -> SessionState {
        state(self.core.cell.session_word())
    }
    pub fn failed(&self) -> bool {
        self.core.cell.session_word() & FAILED != 0
    }
    pub fn emergency_seen(&self) -> bool {
        self.core.cell.session_word() & EMERGENCY != 0
    }
    pub fn close_deadline(&self) -> Option<Instant> {
        self.core.close.get().map(|close| close.deadline)
    }
}

fn state(word: u64) -> SessionState {
    if word & SEALED != 0 {
        return if word & READY == 0 {
            SessionState::SealedFinalizing
        } else if word & FAILED != 0 {
            SessionState::ClosedUnclean
        } else {
            SessionState::ClosedClean
        };
    }
    if word & CLOSING != 0 {
        return if word & (EMERGENCY | FAILED) == 0 {
            SessionState::ClosingNormal
        } else {
            SessionState::ClosingRestricted
        };
    }
    if word & FAILED != 0 {
        SessionState::Failed
    } else if word & EMERGENCY != 0 {
        SessionState::Emergency
    } else if word & READY != 0 {
        SessionState::Normal
    } else {
        SessionState::Bootstrap
    }
}

impl SessionIngress {
    pub fn emergency(&self) -> Result<IngressEvent, SessionError> {
        loop {
            let old = self.core.cell.session_word();
            if old & SEALED != 0 {
                return Err(SessionError::SessionEnded);
            }
            if old & EPOCH_MASK == EPOCH_MASK {
                self.fail();
                return Err(SessionError::Exhausted);
            }
            // The original immutable emergency cause/tag shares this CAS.
            let cause = if old & 7 == 0 {
                2 | (u64::from(u16::MAX - 1) << 8)
            } else {
                0
            };
            let next = (old + EPOCH_STEP) | EMERGENCY | cause;
            if self.core.cell.session_compare(old, next).is_ok() {
                return Ok(IngressEvent {
                    generation: next / EPOCH_STEP,
                });
            }
        }
    }

    pub fn fail(&self) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let old = self.core.cell.session_word();
            if old & SEALED != 0 || old & FAILED != 0 {
                return;
            }
            if self.core.cell.session_compare(old, old | FAILED).is_ok() {
                self.core.close.get_or_init(|| CloseIdentity {
                    sequence: self.core.sequence.load(Ordering::SeqCst),
                    generation: old / EPOCH_STEP,
                    deadline,
                });
                return;
            }
        }
    }

    pub fn accept(&self, kind: CommandKind, sequence: u64) -> Result<SessionRequest, SessionError> {
        let accepted = Instant::now();
        let snapshot = self.core.cell.session_word();
        if snapshot & SEALED != 0 {
            return Err(SessionError::SessionEnded);
        }
        if sequence == u64::MAX {
            self.fail();
            return Err(SessionError::Exhausted);
        }
        self.core
            .sequence
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| {
                (sequence > old).then_some(sequence)
            })
            .map_err(|_| SessionError::Sequence)?;
        if kind == CommandKind::Lift {
            return Err(SessionError::Unavailable);
        }
        let target = match kind {
            CommandKind::Approve => self.core.pending_context.load(Ordering::SeqCst),
            CommandKind::Undo => self.core.undo_context.load(Ordering::SeqCst),
            _ => 0,
        };
        if kind == CommandKind::Quit {
            // Publish the first shutdown identity exactly once. Input has one
            // producer; concurrent fatal shutdown can only narrow this close.
            let close = self.core.close.get_or_init(|| CloseIdentity {
                sequence,
                generation: snapshot / EPOCH_STEP,
                deadline: accepted + Duration::from_secs(3),
            });
            let mut current = snapshot;
            loop {
                if current & SEALED != 0 {
                    return Err(SessionError::SessionEnded);
                }
                if current & CLOSING != 0 {
                    break;
                }
                match self.core.cell.session_compare(current, current | CLOSING) {
                    Ok(()) => break,
                    Err(now) => current = now,
                }
            }
            // Close generation is preserved; repeated Quit cannot refresh it.
            return Ok(SessionRequest {
                core: self.core.clone(),
                kind,
                sequence,
                snapshot,
                target: 0,
                deadline: close.deadline,
                close: Some(close.sequence),
                scope: Arc::new(RequestScope {
                    core: Arc::downgrade(&self.core),
                    session: self.core.identity,
                    request: sequence,
                    snapshot: snapshot | CLOSING,
                    deadline: close.deadline,
                    kind: ScopeKind::CloseCleanup,
                    next_step: AtomicU64::new(0),
                }),
            });
        }
        if !matches!(kind, CommandKind::Status | CommandKind::Evidence)
            && snapshot & (CLOSING | FAILED | EMERGENCY) != 0
        {
            return Err(SessionError::Restricted);
        }
        if matches!(kind, CommandKind::Approve | CommandKind::Undo) && target == 0 {
            return Err(SessionError::NoTarget);
        }
        let seconds = if matches!(kind, CommandKind::Run | CommandKind::Approve) {
            300
        } else {
            120
        };
        Ok(SessionRequest {
            core: self.core.clone(),
            kind,
            sequence,
            snapshot,
            target,
            deadline: accepted + Duration::from_secs(seconds),
            close: None,
            scope: Arc::new(RequestScope {
                core: Arc::downgrade(&self.core),
                session: self.core.identity,
                request: sequence,
                snapshot,
                deadline: accepted + Duration::from_secs(seconds),
                kind: if matches!(kind, CommandKind::Undo | CommandKind::Cleanup) {
                    ScopeKind::Restoration
                } else {
                    ScopeKind::Work
                },
                next_step: AtomicU64::new(0),
            }),
        })
    }
}
