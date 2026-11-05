//! The Hands session's original identity and lock-free input boundary.
//!
//! A seed is issued before input or Home opening. It carries no raw broker,
//! door or kernel object. Acceptance binds a request; it does not admit work.
use crate::authority::file_session::HaltCell;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

pub mod approval;
pub mod close;
pub mod outcome;
mod receipt;
pub mod recovery;
pub mod stop;

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
    facts: outcome::RequestFacts,
    restore: outcome::RestoreFacts,
    // Once a native File run is admitted, Mother sinks retain that original
    // prerequisite. Installing it only narrows the original request.
    mother_run: OnceLock<(
        std::sync::Weak<crate::governed::GovernedControl>,
        crate::authority::ids::RunId,
    )>,
}
impl RequestScope {
    pub(crate) fn fail(&self) {
        if let Some(core) = self.core.upgrade() {
            SessionIngress { core }.fail();
        }
    }

    pub(crate) fn recovery(&self, close_allowed: bool) -> bool {
        matches!(self.kind, ScopeKind::Restoration)
            || (close_allowed && matches!(self.kind, ScopeKind::CloseCleanup))
    }
    pub(crate) fn live(&self, word: u64) -> bool {
        let same = if matches!(self.kind, ScopeKind::CloseCleanup) {
            word & EPOCH_MASK == self.snapshot & EPOCH_MASK
                && word & (CLOSING | FAILED | SEALED | EMERGENCY) == CLOSING
        } else {
            word == self.snapshot
        };
        same && self.eligible()
    }
    fn eligible(&self) -> bool {
        #[cfg(test)]
        if self
            .core
            .upgrade()
            .is_some_and(|core| core.expired_request.load(Ordering::SeqCst) == self.request)
        {
            return false;
        }
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
        let expected = if matches!(self.kind, ScopeKind::CloseCleanup) {
            let current = cell.session_word();
            if !self.live(current) {
                return Err(AuthorityError::Halted);
            }
            current
        } else {
            self.snapshot
        };
        cell.session_compare(expected, expected)
            .map_err(|_| AuthorityError::Halted)?;
        // Observation only; acceptance and failed CAS attempts never count.
        // Bounded by next_step, whose exhaustion permanently fails closed.
        self.facts.admitted.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub(crate) fn reserve_resource(
        &self,
    ) -> Result<ResourceReservation, crate::authority::AuthorityError> {
        if !matches!(self.kind, ScopeKind::Work | ScopeKind::Restoration) {
            return Err(crate::authority::AuthorityError::Halted);
        }
        let core = self
            .core
            .upgrade()
            .ok_or(crate::authority::AuthorityError::Halted)?;
        let mut resources = core.resources.lock().map_err(|_| {
            self.fail();
            crate::authority::AuthorityError::Halted
        })?;
        if resources.len() == 256 {
            return Err(crate::authority::AuthorityError::Capacity);
        }
        #[cfg(test)]
        crate::authority::file_session::probe(Some(&core.cell), "registry.resource_reserve.before");
        self.admit(&core.cell)?;
        let resource = Arc::new(ResourceState {
            request: self.request,
            released: std::sync::atomic::AtomicBool::new(false),
            uncertain: std::sync::atomic::AtomicBool::new(false),
            private: std::sync::Mutex::new(None),
        });
        resources.push(resource.clone());
        #[cfg(test)]
        crate::authority::file_session::probe(Some(&core.cell), "registry.resource_reserve.after");
        Ok(ResourceReservation(resource))
    }
}

struct ResourceState {
    request: u64,
    released: std::sync::atomic::AtomicBool,
    uncertain: std::sync::atomic::AtomicBool,
    private: std::sync::Mutex<Option<PrivateResourceIdentity>>,
}

struct PrivateResourceIdentity {
    // Observation only. These names never authorize opening or removing
    // anything; the private sink owns its original paths and descriptors.
    temporary: std::path::PathBuf,
    retained: Option<std::path::PathBuf>,
    observed: Option<std::fs::Metadata>,
}

/// One pre-reserved Mother private-store obligation. Its metadata survives
/// failed writes, unwind and final store destruction in the original session.
pub struct PrivateObligation {
    reservation: ResourceReservation,
    context: SessionContext,
}
impl PrivateObligation {
    pub fn observe_created(&self, metadata: std::fs::Metadata) -> Result<(), SessionError> {
        let mut identity = self.reservation.0.private.lock().map_err(|_| {
            self.context.fail();
            SessionError::Restricted
        })?;
        let identity = identity.as_mut().ok_or(SessionError::Restricted)?;
        identity.observed = Some(metadata);
        Ok(())
    }
    /// Called by the private store only after its required file and parent
    /// synchronizations, or conclusive non-creation/removal and parent sync.
    /// This is accounting, not permission for a later filesystem call.
    pub fn complete(self) {
        self.reservation.release();
    }
}
impl Drop for PrivateObligation {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.reservation.uncertain();
            self.context.fail();
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ResourceObligations {
    pub reserved: usize,
    pub discharged: usize,
    pub unresolved: usize,
    pub uncertain: usize,
    pub private: usize,
    pub private_unresolved: usize,
}

/// Fixed metadata reserved before creation. Dropping it never discharges an
/// obligation; the session retains the same slot until finalization.
#[derive(Clone)]
pub(crate) struct ResourceReservation(Arc<ResourceState>);
impl ResourceReservation {
    pub(crate) fn release(&self) {
        self.0.released.store(true, Ordering::SeqCst);
    }
    pub(crate) fn uncertain(&self) {
        self.0.uncertain.store(true, Ordering::SeqCst);
    }
    pub(crate) fn request(&self) -> u64 {
        self.0.request
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
    #[cfg(test)]
    expired_request: AtomicU64,
    identity: SessionIdentity,
    stops: stop::Stops,
    cell: HaltCell,
    sequence: AtomicU64,
    pending_context: AtomicU64,
    undo_context: AtomicU64,
    next_target: AtomicU64,
    pending: std::sync::Mutex<Option<PendingContext>>,
    actions: std::sync::Mutex<Vec<receipt::Action>>,
    resources: std::sync::Mutex<Vec<Arc<ResourceState>>>,
    history: OnceLock<receipt::History>,
    close: OnceLock<CloseIdentity>,
    producers: std::sync::Mutex<close::Producers>,
    finals: OnceLock<close::Finals>,
    authority_closed: std::sync::atomic::AtomicBool,
    created: Instant,
}

struct PendingContext {
    mission: Option<u64>,
    effect: Option<u64>,
    generation: Option<u64>,
    claimed: Option<Arc<RequestScope>>,
    approved: Option<Arc<RequestScope>>,
    continuation_taken: bool,
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
    run: Option<(
        std::sync::Weak<crate::governed::GovernedControl>,
        crate::authority::ids::RunId,
    )>,
}
impl SessionContext {
    pub fn reserve_private(
        &self,
        temporary: &std::path::Path,
        retained: Option<&std::path::Path>,
    ) -> Result<PrivateObligation, SessionError> {
        if !temporary.is_absolute()
            || temporary.as_os_str().len() > 4096
            || retained.is_some_and(|path| !path.is_absolute() || path.as_os_str().len() > 4096)
        {
            return Err(SessionError::Restricted);
        }
        let identity = PrivateResourceIdentity {
            temporary: temporary.to_owned(),
            retained: retained.map(std::path::Path::to_owned),
            observed: None,
        };
        let reservation = self
            .scope
            .reserve_resource()
            .map_err(|_| SessionError::Exhausted)?;
        *reservation
            .0
            .private
            .lock()
            .map_err(|_| SessionError::Restricted)? = Some(identity);
        Ok(PrivateObligation {
            reservation,
            context: self.clone(),
        })
    }
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
        match self.run.as_ref().or_else(|| self.scope.mother_run.get()) {
            Some((control, run)) => control
                .upgrade()
                .ok_or(SessionError::Restricted)?
                .authority()
                .runs()
                .admit_file_request(*run, Some(&self.cell)),
            None => self.cell.admit(),
        }
        .map_err(|_| SessionError::Restricted)
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
    evidence: Arc<crate::evidence_store::PrimaryEvidence>,
    report: OnceLock<recovery::StartupReport>,
}

/// Mother's closed File door. Its raw legacy representation is private and
/// cannot be extracted, adopted, resumed, or used for another effect domain.
pub struct SessionDoor {
    door: Arc<crate::governed::AgentDoor>,
    origin_request: u64,
    context: SessionContext,
    core: Arc<Core>,
}

impl HostSession {
    #[cfg(test)]
    pub(crate) fn expire_request_for_test(&self, request: u64) {
        self.core.expired_request.store(request, Ordering::SeqCst);
    }
    pub fn open(
        seed: SessionSeed,
        root: &std::path::Path,
        evidence: Arc<crate::evidence_store::PrimaryEvidence>,
        clock: Arc<dyn crate::authority::clock::Clock>,
        confirmer: Arc<dyn crate::authority::approval::ControlConfirmer + Send + Sync>,
    ) -> Result<Self, SessionError> {
        Self::open_with_history(seed, root, root, evidence, clock, confirmer)
    }

    /// Bind both stores at construction. The history root is an owned Home
    /// directory, never a path obtained from an action or a journal record.
    pub fn open_with_history(
        seed: SessionSeed,
        root: &std::path::Path,
        history_root: &std::path::Path,
        evidence: Arc<crate::evidence_store::PrimaryEvidence>,
        clock: Arc<dyn crate::authority::clock::Clock>,
        confirmer: Arc<dyn crate::authority::approval::ControlConfirmer + Send + Sync>,
    ) -> Result<Self, SessionError> {
        let history = receipt::History::bind(history_root)
            .map_err(|_| SessionError::Backend("mother_history"))?;
        let canonical = root.canonicalize().map_err(|_| SessionError::Unavailable)?;
        let parent = history_root
            .canonicalize()
            .map_err(|_| SessionError::Unavailable)?;
        if canonical != parent && canonical.parent() != Some(parent.as_path()) {
            return Err(SessionError::Restricted);
        }
        seed.core
            .history
            .set(history)
            .map_err(|_| SessionError::Unavailable)?;
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
            facts: outcome::RequestFacts::default(),
            restore: outcome::RestoreFacts::default(),
            mother_run: OnceLock::new(),
        });
        let bootstrap = SessionContext {
            run: None,
            cell: seed.core.cell.scoped(scope.clone()),
            scope,
        };
        let control = crate::governed::GovernedControl::with_file_sessions(
            root,
            crate::broker::Vault::Disabled.source(),
            evidence.clone(),
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
            evidence,
            report: OnceLock::new(),
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
        if !Arc::ptr_eq(&self.core, &request.core) || request.kind != CommandKind::Run {
            return Err(SessionError::Restricted);
        }
        Ok(SessionContext {
            run: None,
            cell: self.core.cell.scoped(request.scope.clone()),
            scope: request.scope.clone(),
        })
    }
    pub fn recovery_report(&self) -> Result<Vec<crate::files::RecoveryEntry>, SessionError> {
        self.control
            .files_recovery_report()
            .map_err(|error| SessionError::Backend(error.class()))
    }
    pub fn startup_report(&self) -> recovery::StartupReport {
        self.report
            .get_or_init(|| {
                use recovery::SourceFailure;
                let report = recovery::StartupReport {
                    kernel: self
                        .control
                        .files_recovery_report()
                        .map_err(|_| SourceFailure::KernelLedger),
                    resources: self
                        .control
                        .files()
                        .recorded_resources()
                        .map_err(|_| SourceFailure::KernelLedger),
                    mother: self
                        .core
                        .history
                        .get()
                        .ok_or(SourceFailure::MotherHistory)
                        .and_then(recovery::history),
                    primary: self.evidence.health(),
                };
                if !report.ready() {
                    self.ingress().fail();
                }
                report
            })
            .clone()
    }
    pub fn bootstrap_hold(&self, hold: Duration) -> Result<(), SessionError> {
        self.startup_report().require_ready()?;
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
        self.startup_report().require_ready()?;
        self.control
            .register_workspace_with_halt(path, reserved, Some(&self.bootstrap.cell))
            .map_err(|error| SessionError::Backend(error.class()))
    }

    /// Last bootstrap transition, invoked by the trusted host only after its
    /// three-source report has succeeded. A late result never installs ready.
    pub fn finish_bootstrap(&self) -> Result<(), SessionError> {
        self.startup_report().require_ready()?;
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

    pub fn run_count(&self) -> usize {
        self.control.authority().runs().views().len()
    }

    pub fn stop_snapshot(&self) -> crate::governed::StopSnapshot {
        self.control.stop_snapshot()
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
        self.grant_files(request, grant_lifetime, &*self.confirmer)?;
        self.activate_file_door(request, self.confirmer.clone())
    }

    fn grant_files(
        &self,
        request: &SessionRequest,
        grant_lifetime: Duration,
        confirmer: &dyn crate::authority::approval::ControlConfirmer,
    ) -> Result<crate::authority::ids::GrantId, SessionError> {
        let context = self.context(request)?;
        let workspace = self.control.workspace().ok_or(SessionError::Unavailable)?;
        let grant = self
            .control
            .request_grant_with_halt(
                &crate::governed::GrantRequest::Files {
                    workspace: workspace.id,
                },
                grant_lifetime,
                confirmer,
                Some(&context.cell),
            )
            .map_err(|error| SessionError::Backend(error.class()))?;
        Ok(grant)
    }

    fn activate_file_door(
        &self,
        request: &SessionRequest,
        confirmer: Arc<dyn crate::authority::approval::ControlConfirmer + Send + Sync>,
    ) -> Result<SessionDoor, SessionError> {
        let context = self.context(request)?;
        let door = self
            .control
            .issue_agent_door_with_halt(
                crate::authority::ids::AgentId::new("mother").ok_or(SessionError::Unavailable)?,
                confirmer,
                Some(context.cell.clone()),
            )
            .map_err(|error| SessionError::Backend(error.class()))?;
        Ok(SessionDoor {
            origin_request: context.request(),
            door: Arc::new(door),
            context,
            core: self.core.clone(),
        })
    }

    /// The trusted host supplies an immutable presentation for this command.
    /// Native consent and the original request still govern every sink.
    pub fn open_confirmed_file_door(
        &self,
        request: &SessionRequest,
        confirmer: Arc<dyn crate::authority::approval::ControlConfirmer + Send + Sync>,
    ) -> Result<SessionDoor, SessionError> {
        if request.kind != CommandKind::Run {
            return Err(SessionError::Restricted);
        }
        self.grant_files(
            request,
            request.deadline.saturating_duration_since(Instant::now()),
            &*confirmer,
        )?;
        self.activate_file_door(request, confirmer)
    }

    pub fn cleanup(
        &self,
        request: &SessionRequest,
    ) -> Result<crate::files::CleanupRetry, SessionFailure> {
        if !Arc::ptr_eq(&self.core, &request.core) || request.kind != CommandKind::Cleanup {
            return Err(crate::authority::AuthorityError::Halted.into());
        }
        let cell = self.core.cell.scoped(request.scope.clone());
        self.control
            .files_retry_cleanup_with_halt(Some(&cell))
            .map_err(Into::into)
    }

    pub fn inventory(&self) -> Result<crate::files::SessionInventory, SessionFailure> {
        self.control.files().session_inventory().map_err(|error| {
            self.ingress().fail();
            error.into()
        })
    }
    pub fn resource_obligations(&self) -> Result<ResourceObligations, SessionError> {
        let resources = self.core.resources.lock().map_err(|_| {
            self.ingress().fail();
            SessionError::Restricted
        })?;
        let mut result = ResourceObligations {
            reserved: resources.len(),
            ..ResourceObligations::default()
        };
        for resource in resources.iter() {
            let discharged = resource.released.load(Ordering::SeqCst);
            result.discharged += usize::from(discharged);
            result.unresolved += usize::from(!discharged);
            result.uncertain += usize::from(resource.uncertain.load(Ordering::SeqCst));
            let private = resource.private.lock().map_err(|_| {
                self.ingress().fail();
                SessionError::Restricted
            })?;
            if let Some(identity) = private.as_ref() {
                // Keep the exact expected and observed metadata bounded and
                // owned, even when its creator did not return. No path lookup.
                let _owned = (&identity.temporary, &identity.retained, &identity.observed);
                result.private += 1;
                result.private_unresolved += usize::from(!discharged);
            }
        }
        Ok(result)
    }

    /// Resource settlement is one part of the close proof. This neither seals
    /// the session nor emits either end record.
    pub fn settle_close_resources(
        &self,
        request: &SessionRequest,
    ) -> Result<crate::files::SessionInventory, SessionFailure> {
        if !Arc::ptr_eq(&self.core, &request.core) || request.kind != CommandKind::Quit {
            return Err(crate::authority::AuthorityError::Halted.into());
        }
        let cell = self.core.cell.scoped(request.scope.clone());
        let normal = self.state() == SessionState::ClosingNormal;
        self.control
            .files()
            .close_resources(normal.then_some(&cell))
            .map_err(|error| {
                self.ingress().fail();
                error.into()
            })
    }

    pub fn undo(
        &self,
        request: &SessionRequest,
    ) -> Result<crate::files::UndoReport, SessionFailure> {
        self.undo_confirmed(request, &*self.confirmer)
    }
    pub fn undo_confirmed(
        &self,
        request: &SessionRequest,
        confirmer: &dyn crate::authority::approval::ControlConfirmer,
    ) -> Result<crate::files::UndoReport, SessionFailure> {
        if !Arc::ptr_eq(&self.core, &request.core)
            || request.kind != CommandKind::Undo
            || request.target == 0
            || request.target != self.core.undo_context.load(Ordering::SeqCst)
        {
            return Err(crate::authority::AuthorityError::Halted.into());
        }
        let cell = self.core.cell.scoped(request.scope.clone());
        cell.admit()?;
        let claim = outcome::UndoClaim::take(&self.core, request.target)?;
        let result = self
            .control
            .files_undo_with_halt(confirmer, Some(&cell))
            .map_err(Into::into);
        if self.control.files().session_undo_available()? && cell.admit().is_ok() {
            let version = self
                .core
                .next_target
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_add(1))
                .map_err(|_| crate::authority::AuthorityError::Capacity)?
                + 1;
            // A distinct returned version cannot be adopted by a queued command.
            // A separately published target is never overwritten by this return.
            let _published = self.core.undo_context.compare_exchange(
                0,
                version,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
        }
        claim.returned();
        result
    }

    pub fn approve_file(
        &self,
        request: &SessionRequest,
    ) -> Result<crate::control::EffectOutput, SessionFailure> {
        self.approve_file_confirmed(request, &*self.confirmer)
    }
    pub fn approve_file_confirmed(
        &self,
        request: &SessionRequest,
        confirmer: &dyn crate::authority::approval::ControlConfirmer,
    ) -> Result<crate::control::EffectOutput, SessionFailure> {
        use crate::authority::AuthorityError;
        if !Arc::ptr_eq(&self.core, &request.core) || request.kind != CommandKind::Approve {
            return Err(AuthorityError::Halted.into());
        }
        let cell = self.core.cell.scoped(request.scope.clone());
        let (run, commitment, identity) = {
            let mut target = self.core.pending.lock().map_err(|_| {
                self.ingress().fail();
                SessionFailure::from(AuthorityError::Halted)
            })?;
            let target = target
                .as_mut()
                .filter(|t| {
                    t.identity == request.target
                        && t.claimed.is_none()
                        && self.core.pending_context.load(Ordering::SeqCst) == t.identity
                })
                .ok_or(AuthorityError::UnknownCommitment)?;
            self.control
                .authority()
                .runs()
                .admit_file_request(target.run, Some(&cell))?;
            target.claimed = Some(request.scope.clone());
            self.core.pending_context.store(0, Ordering::SeqCst);
            (target.run, target.commitment, target.identity)
        };
        let agent =
            crate::authority::ids::AgentId::new("mother").ok_or(AuthorityError::WrongAgent)?;
        self.control
            .authority()
            .commitments()
            .bind_session_request(commitment, &agent, run, &cell)?;
        self.control.authorize(commitment, &agent, run, confirmer)?;
        cell.admit()?;
        let result = self.control.execute(commitment, &agent, run);
        let _observed = request
            .scope
            .facts
            .primary
            .set(result.as_ref().ok().and_then(|output| output.evidence));
        if result.as_ref().is_ok_and(|output| {
            output.evidence.is_some_and(|evidence| {
                matches!(
                    evidence.end,
                    crate::authority::evidence::EndStatus::Recorded { .. }
                )
            })
        }) {
            let mut target = self.core.pending.lock().map_err(|_| {
                self.ingress().fail();
                SessionFailure::from(AuthorityError::Halted)
            })?;
            let target = target
                .as_mut()
                .filter(|t| {
                    t.identity == identity
                        && t.claimed
                            .as_ref()
                            .is_some_and(|scope| Arc::ptr_eq(scope, &request.scope))
                })
                .ok_or(AuthorityError::UnknownCommitment)?;
            if cell.admit().is_ok() {
                target.approved = Some(request.scope.clone());
                self.core.undo_context.store(identity, Ordering::SeqCst);
            }
        }
        result.map_err(Into::into)
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
        if self
            .context
            .scope
            .mother_run
            .get()
            .is_some_and(|(_, known)| *known != run)
        {
            return Err(crate::authority::AuthorityError::NotMember.into());
        }
        self.door.admit_files(run, contract_hash, request)?;
        if self
            .context
            .scope
            .mother_run
            .set((self.door.session_backend(), run))
            .is_err()
            && self
                .context
                .scope
                .mother_run
                .get()
                .is_none_or(|(_, known)| *known != run)
        {
            self.context.fail();
            return Err(crate::authority::AuthorityError::Halted.into());
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn create_file(
        &self,
        run: crate::authority::ids::RunId,
        file: &crate::files::FileCreate,
    ) -> Result<crate::governed::AgentOutcome, SessionFailure> {
        self.create_file_numbered(run, file, None)
    }

    /// A Mother effect's number is only correlation data: the original
    /// consent, run, door and terminal journal must independently match it.
    pub fn create_file_effect(
        &self,
        run: crate::authority::ids::RunId,
        file: &crate::files::FileCreate,
        effect: u64,
    ) -> Result<crate::governed::AgentOutcome, SessionFailure> {
        self.create_file_numbered(run, file, Some(effect))
    }

    fn create_file_numbered(
        &self,
        run: crate::authority::ids::RunId,
        file: &crate::files::FileCreate,
        effect: Option<u64>,
    ) -> Result<crate::governed::AgentOutcome, SessionFailure> {
        if self
            .context
            .run
            .as_ref()
            .or_else(|| self.context.scope.mother_run.get())
            .is_some_and(|(_, known)| *known != run)
        {
            return Err(crate::authority::AuthorityError::WrongRun.into());
        }
        let identity = self
            .core
            .next_target
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| old.checked_add(1))
            .map_err(|_| {
                self.context.fail();
                SessionFailure::from(crate::authority::AuthorityError::Capacity)
            })?
            + 1;
        // Retain the outcome/ack fields before any native allocation. A
        // failed preparation consumes its reserved entry; it cannot erase
        // uncertainty or evict a previous successful receipt.
        let action = receipt::Action::reserved(
            self.core.identity,
            self.context.request(),
            self.origin_request,
            identity,
            self.door.origin(),
            run,
            self.door.mission_request(run)?,
            effect,
        );
        let slot = {
            let mut actions = self.core.actions.lock().map_err(|_| {
                self.context.fail();
                SessionFailure::from(crate::authority::AuthorityError::Halted)
            })?;
            if actions.len() == 256 {
                return Err(crate::authority::AuthorityError::Capacity.into());
            }
            let slot = actions.len();
            #[cfg(test)]
            crate::authority::file_session::probe(
                Some(&self.context.cell),
                "registry.action_reserve.before",
            );
            self.context.cell.admit()?;
            actions.push(action);
            #[cfg(test)]
            crate::authority::file_session::probe(
                Some(&self.context.cell),
                "registry.action_reserve.after",
            );
            slot
        };
        let outcome = self
            .door
            .session_file_action(run, file, &self.context.cell)
            .map_err(SessionFailure::from)?;
        if let crate::governed::AgentOutcome::AwaitingApproval(view) = &outcome {
            self.core.actions.lock().map_err(|_| {
                self.context.fail();
                SessionFailure::from(crate::authority::AuthorityError::Halted)
            })?[slot]
                .commitment = Some(view.id);
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
                    mission: self.door.mission_request(run).ok().and_then(|request| {
                        request
                            .split('/')
                            .next()?
                            .strip_prefix("msn-")?
                            .parse()
                            .ok()
                    }),
                    effect,
                    generation: None,
                    claimed: None,
                    approved: None,
                    continuation_taken: false,
                });
                // Native unit fixtures have no Mother journal. Production
                // effects publish only after the exact durable wait proof.
                #[cfg(test)]
                if effect.is_none() {
                    self.core.pending_context.store(identity, Ordering::SeqCst);
                }
            }
        }
        Ok(outcome)
    }
    pub fn cancel_run(
        &self,
        run: crate::authority::ids::RunId,
    ) -> Result<crate::governed::RecordBatch, SessionFailure> {
        let call = close::settlement(&self.core)?;
        {
            let pending = self.core.pending.lock().map_err(|_| {
                self.context.fail();
                crate::authority::AuthorityError::Halted
            })?;
            if pending.as_ref().is_some_and(|pending| pending.run == run) {
                self.core.pending_context.store(0, Ordering::SeqCst);
            }
        }
        let result = self.door.cancel_run(run).map_err(Into::into);
        call.joined();
        result
    }
    pub fn finish_when_settled(
        &self,
        run: crate::authority::ids::RunId,
    ) -> Result<(), SessionFailure> {
        let call = close::settlement(&self.core)?;
        let result = self.door.finish_when_settled(run).map_err(Into::into);
        call.joined();
        result
    }
    pub fn halt(
        &self,
        reason: crate::governed::HaltReason,
    ) -> Result<crate::governed::RecordBatch, SessionFailure> {
        let call = close::settlement(&self.core)?;
        let result = self.door.halt(reason);
        call.joined();
        Ok(result)
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
        id: crate::authority::ids::CommitmentId,
    ) -> Result<(), SessionFailure> {
        let call = close::settlement(&self.core)?;
        let result = receipt::ack(self, id).map_err(Into::into);
        call.joined();
        result
    }
}

#[cfg(test)]
impl HostSession {
    pub(crate) fn pending_for_test(&self) -> Option<crate::authority::ids::CommitmentId> {
        self.core
            .pending
            .lock()
            .unwrap()
            .as_ref()
            .map(|target| target.commitment)
    }
    pub(crate) fn grant_for_test(
        &self,
        request: &SessionRequest,
    ) -> Result<crate::authority::ids::GrantId, SessionError> {
        self.grant_files(request, Duration::from_secs(60), &*self.confirmer)
    }
    pub(crate) fn activate_for_test(
        &self,
        request: &SessionRequest,
    ) -> Result<SessionDoor, SessionError> {
        self.activate_file_door(request, self.confirmer.clone())
    }
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
    session: SessionIdentity,
    index: usize,
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
                #[cfg(test)]
                expired_request: AtomicU64::new(u64::MAX),
                identity: SessionIdentity(identity),
                stops: stop::Stops::default(),
                cell: HaltCell::new_host(),
                sequence: AtomicU64::new(0),
                pending_context: AtomicU64::new(0),
                undo_context: AtomicU64::new(0),
                next_target: AtomicU64::new(0),
                pending: std::sync::Mutex::new(None),
                actions: std::sync::Mutex::new(Vec::with_capacity(256)),
                resources: std::sync::Mutex::new(Vec::with_capacity(256)),
                history: OnceLock::new(),
                close: OnceLock::new(),
                producers: std::sync::Mutex::default(),
                finals: OnceLock::new(),
                authority_closed: std::sync::atomic::AtomicBool::new(false),
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
                return self.core.stops.register(&self.core, next / EPOCH_STEP);
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
                    facts: outcome::RequestFacts::default(),
                    restore: outcome::RestoreFacts::default(),
                    mother_run: OnceLock::new(),
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
                facts: outcome::RequestFacts::default(),
                restore: outcome::RestoreFacts::default(),
                mother_run: OnceLock::new(),
            }),
        })
    }
}
