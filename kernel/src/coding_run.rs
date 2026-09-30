//! Governed coding run primitive (Phase One, P1A-002). Backend only.
//!
//! A coding run takes an existing backend-issued project grant, freezes three
//! scopes, stages a content snapshot of the read scope into a
//! backend-allocated staging directory, applies candidate edits to staging
//! only, and verifies the candidate structurally. Every step is recorded in
//! the durable, fail-closed coding-run ledger.
//!
//! Authority never comes from a string. A [`RunId`] is only a name: parsing
//! one yields nothing that can act, and there is no API that looks a run up by
//! id. Project authority is the live [`WorkspaceAuthorityRegistry`] grant for
//! the run's [`WorkspaceBinding`] plus the directory identity retained when
//! the run was granted; both are re-checked before every sensitive step. This
//! checkpoint issues no `UserSelected` grant, exposes no IPC, runs no process
//! and never writes to the project.
//!
//! Phase One adds a governed worker ([`run_worker`]): a local model pinned
//! into the run ([`CodingRun::pin_model`]) proposes typed edits that reach
//! staging only through [`CodingRun::edit`]. The only model access is the
//! loopback-only [`LocalOllama`]; no cloud provider exists on this path.
//!
//! ```compile_fail
//! use nexus_kernel::coding_run::{CodingRun, RunId};
//! let id = RunId::parse("7f3a2c8e-0000-4000-8000-000000000000").unwrap();
//! let _run = CodingRun::open(id); // no lookup by id exists
//! ```
//!
//! ```compile_fail
//! use nexus_kernel::coding_run::RunId;
//! let _id = RunId(uuid::Uuid::new_v4()); // the field is private
//! ```

mod fsops;
mod ledger;
mod local_model;
mod manifest;
mod project;
mod review;
mod scope;
mod structural;
mod worker;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use thiserror::Error;
use uuid::Uuid;

use crate::manifest::FsPermissionLevel;
use crate::workspace_authority::{
    WorkspaceAuthorityError, WorkspaceAuthorityRegistry, WorkspaceAuthoritySource,
    WorkspaceBinding, WorkspaceGrant, WorkspaceGrantId,
};
use fsops::{DirHandle, EntryKind, NodeIdentity};

pub use ledger::{analyze as analyze_ledger, LedgerFailure, LedgerRecovery, LedgerStore};
pub use local_model::{
    loopback_endpoint, LocalModel, LocalOllama, ModelError, ModelMessage, ModelPin, ModelRole,
    LOCAL_PROVIDER,
};
pub use manifest::{Manifest, ManifestEntry, ManifestHash};
pub use project::{
    FolderPicker, ProjectError, ProjectGrant, ProjectId, ProjectInfo, ProjectRegistry,
    APPLY_GRANT_LIFETIME, RUN_GRANT_LIFETIME,
};
pub use review::{
    ChangeKind, DiffOmitted, FileChange, Review, ReviewBinding, TextDiff, MAX_DIFF_BYTES_PER_FILE,
    MAX_DIFF_BYTES_TOTAL, MAX_DIFF_FILE_BYTES,
};
pub use scope::{RelPath, RunScopes, ScopeEntry, ScopeError, ScopeSet};
pub use structural::{
    StructuralOutcome, StructuralProfile, StructuralVerification, StructuralViolation,
};
pub use worker::{
    run_worker, EditProposal, ProposalOp, ProposalRejection, WorkerError, WorkerLimits,
    WorkerReport, WORKER_LIMITS,
};

use ledger::{record, EventKind};
use scope::is_git_metadata;

/// Opaque run name. It authorizes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunId(Uuid);

impl RunId {
    fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    /// Parse a run name (for display or ledger queries). Grants nothing.
    pub fn parse(text: &str) -> Option<Self> {
        Uuid::parse_str(text)
            .ok()
            .filter(|uuid| !uuid.is_nil())
            .map(Self)
    }

    /// The UUID under which the run's ledger entries are stored.
    pub fn ledger_key(&self) -> Uuid {
        self.0
    }
}

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevocationReason {
    /// The backend revoked the run.
    Explicit,
    /// The project or staging grant (or an ancestor) was revoked.
    GrantRevoked,
    /// The project or staging grant (or an ancestor) expired.
    GrantExpired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureReason {
    AuthorityDenied,
    IdentityChanged,
    SnapshotRejected,
    StagingRedirect,
    StagingIo,
    StructuralRejected,
    /// The pinned local model could not answer.
    ModelUnavailable,
    /// The worker reached its turn limit or deadline.
    WorkerLimitExceeded,
    /// A required worker record could not be written.
    AuditUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryReason {
    /// A staging mutation happened but its outcome could not be recorded.
    OutcomeNotRecorded,
    /// The ledger shows a prepared operation without an outcome.
    UnmatchedPrepared,
    /// The ledger failed verification.
    LedgerIntegrity,
    /// A terminal transition could not be recorded.
    TerminalNotRecorded,
    /// Staging was allocated but `run.granted` could not be recorded.
    GrantedNotRecorded,
    /// A rejected edit could not be recorded.
    RejectionNotRecorded,
    /// The staging grant could not be revoked when the run closed.
    StagingRevocationFailed,
}

/// Run state. Terminal outcomes are kept as they happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Created,
    Granted,
    Staged,
    Candidate,
    StructurallyVerified,
    Cancelled,
    Revoked(RevocationReason),
    Failed(FailureReason),
    RecoveryRequired(RecoveryReason),
}

impl RunState {
    /// No further run transition is possible (cleanup may still happen).
    pub fn is_terminal(self) -> bool {
        !matches!(
            self,
            Self::Created | Self::Granted | Self::Staged | Self::Candidate
        )
    }
}

/// Staging cleanup, tracked separately from the run outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupStatus {
    NotStarted,
    Discarded,
    DiscardFailed,
}

/// Why a snapshot was refused. Paths are project-relative, never host paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotRejection {
    Symlink(String),
    SpecialFile(String),
    HardLink(String),
    OtherDevice(String),
    InvalidName(String),
    NotADirectory(String),
    FileTooLarge(String),
    FileCountCap,
    TotalBytesCap,
    Io(String),
}

/// Why a candidate edit was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditRejection {
    OutsideWriteScope,
    ProtectedInput,
    NotText,
    TooLarge,
    NotStaged,
    AlreadyExists,
    /// A read outside the frozen read scope.
    OutsideReadScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RunError {
    #[error("operation {operation} is not allowed in state {state:?}")]
    InvalidState {
        operation: &'static str,
        state: RunState,
    },
    #[error("coding-run ledger: {0}")]
    Ledger(LedgerFailure),
    #[error("scope: {0}")]
    Scope(ScopeError),
    #[error("run authority revoked: {0:?}")]
    Revoked(RevocationReason),
    #[error("run authority denied")]
    AuthorityDenied,
    #[error("project or staging directory identity changed")]
    IdentityChanged,
    #[error("snapshot rejected: {0:?}")]
    Snapshot(SnapshotRejection),
    #[error("edit rejected: {0:?}")]
    EditRejected(EditRejection),
    #[error("staging entry redirected")]
    StagingRedirect,
    #[error("staging I/O failed")]
    StagingIo,
    #[error("recovery required: {0:?}")]
    RecoveryRequired(RecoveryReason),
    #[error("staging location unavailable")]
    StagingUnavailable,
    #[error("the staged candidate no longer matches its verification")]
    CandidateChanged,
    #[error("the verified candidate is no longer available")]
    CandidateUnavailable,
}

/// A candidate edit. Content is data; the path is checked against the frozen
/// scopes before anything is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateEdit {
    /// Replace an existing staged file.
    Replace { path: RelPath, content: Vec<u8> },
    /// Create a file that does not exist (a scoped creation).
    Create { path: RelPath, content: Vec<u8> },
}

impl CandidateEdit {
    fn path(&self) -> &RelPath {
        match self {
            Self::Replace { path, .. } | Self::Create { path, .. } => path,
        }
    }
    fn content(&self) -> &[u8] {
        match self {
            Self::Replace { content, .. } | Self::Create { content, .. } => content,
        }
    }
    fn kind(&self) -> &'static str {
        match self {
            Self::Replace { .. } => "replace",
            Self::Create { .. } => "create",
        }
    }
}

/// The backend-owned directory under which staging areas are allocated.
#[derive(Debug)]
pub struct StagingParent {
    handle: DirHandle,
    path: PathBuf,
}

impl StagingParent {
    /// `<identity home>/.nexus/coding-runs`, derived by construction from
    /// retained directory identities (see [`Self::derive`]).
    pub fn from_identity_home() -> Result<Self, RunError> {
        let home =
            crate::identity_home::identity_home().map_err(|_| RunError::StagingUnavailable)?;
        Self::derive(&home)
    }

    /// Derive the staging parent under an identity home without trusting any
    /// pathname below it. The identity home must already be a real directory
    /// whose path is its exact canonical spelling (no symlink anywhere in it);
    /// it is then opened and retained, and `.nexus` and `coding-runs` are
    /// opened, or created owner-only, one at a time as direct children of the
    /// retained handles. An existing symlink, file or special entry at either
    /// name makes coding runs unavailable; nothing is followed or created
    /// through it. Finally the pathname used for grant issuance must name the
    /// retained `coding-runs` directory.
    fn derive(home: &Path) -> Result<Self, RunError> {
        let unavailable = |_| RunError::StagingUnavailable;
        let canonical = std::fs::canonicalize(home).map_err(unavailable)?;
        if canonical != home {
            return Err(RunError::StagingUnavailable);
        }
        let home_handle = DirHandle::open_absolute(home).map_err(unavailable)?;
        let nexus = home_handle
            .open_or_create_subdir(".nexus")
            .map_err(unavailable)?;
        let runs = nexus
            .open_or_create_subdir("coding-runs")
            .map_err(unavailable)?;
        let path = home.join(".nexus").join("coding-runs");
        let named = DirHandle::open_absolute(&path).map_err(unavailable)?;
        if named.identity() != runs.identity() {
            return Err(RunError::StagingUnavailable);
        }
        Ok(Self { handle: runs, path })
    }

    #[cfg(test)]
    pub(crate) fn derive_for_test(home: &Path) -> Result<Self, RunError> {
        Self::derive(home)
    }

    #[cfg(test)]
    fn open(path: PathBuf) -> Result<Self, RunError> {
        let handle = DirHandle::open_absolute(&path).map_err(|_| RunError::StagingUnavailable)?;
        Ok(Self { handle, path })
    }

    #[cfg(test)]
    pub(crate) fn for_test(path: PathBuf) -> Self {
        Self::open(path).expect("test staging parent")
    }
}

/// Owner of one run's staging authority: the run directory (created through
/// a duplicate of the staging parent's retained handle), its retained handle
/// and the backend-issued staging grant.
///
/// A lease exists before the directory is created, so from the moment the
/// directory or the grant exists exactly one object is responsible for
/// removing and revoking it. [`StagingLease::close`] revokes the grant
/// (idempotently) and removes the directory through retained handles; it
/// never depends on the ledger. `Drop` closes an unreleased lease without
/// panicking. The project grant is never held or revoked here.
#[derive(Debug)]
struct StagingLease {
    registry: Arc<WorkspaceAuthorityRegistry>,
    binding: WorkspaceBinding,
    parent: DirHandle,
    parent_path: PathBuf,
    name: String,
    created: bool,
    handle: Option<DirHandle>,
    grant: Option<WorkspaceGrantId>,
    grant_revoked: bool,
    released: bool,
}

impl StagingLease {
    /// Take ownership responsibility before anything is created.
    fn begin(
        registry: Arc<WorkspaceAuthorityRegistry>,
        binding: WorkspaceBinding,
        parent: &StagingParent,
        name: String,
    ) -> Result<Self, RunError> {
        let retained = parent.handle.try_clone().map_err(|_| RunError::StagingIo)?;
        Ok(Self {
            registry,
            binding,
            parent: retained,
            parent_path: parent.path.clone(),
            name,
            created: false,
            handle: None,
            grant: None,
            grant_revoked: false,
            released: false,
        })
    }

    /// Create the run directory under the retained parent, confirm the
    /// parent pathname still names that parent, and issue the staging grant.
    /// Every step's result stays owned by the lease.
    fn allocate(&mut self, expires_at: Option<std::time::SystemTime>) -> Result<(), RunError> {
        self.parent
            .make_subdir(&self.name)
            .map_err(|_| RunError::StagingIo)?;
        self.created = true;
        self.handle = Some(
            self.parent
                .open_subdir(&self.name)
                .map_err(|_| RunError::StagingIo)?,
        );
        let named =
            DirHandle::open_absolute(&self.parent_path).map_err(|_| RunError::IdentityChanged)?;
        if named.identity() != self.parent.identity() {
            return Err(RunError::IdentityChanged);
        }
        let grant = self
            .registry
            .issue_trusted_root(
                &self.parent_path.join(&self.name),
                self.binding,
                WorkspaceAuthoritySource::BackendAllocated,
                FsPermissionLevel::ReadWrite,
                expires_at,
            )
            .map_err(|_| RunError::StagingIo)?;
        self.grant = Some(grant);
        Ok(())
    }

    fn handle(&self) -> Result<&DirHandle, RunError> {
        self.handle.as_ref().ok_or(RunError::AuthorityDenied)
    }

    /// Revoke the staging grant, idempotently. Returns whether no live grant
    /// remains.
    fn revoke_grant(&mut self) -> bool {
        match self.grant {
            None => true,
            Some(_) if self.grant_revoked => true,
            Some(grant) => {
                self.grant_revoked = self.registry.revoke(grant, self.binding).is_ok();
                self.grant_revoked
            }
        }
    }

    /// Whether no live staging grant remains.
    fn authority_closed(&self) -> bool {
        self.grant.is_none() || self.grant_revoked
    }

    /// Revoke the grant and remove the run directory through retained
    /// handles. The directory entry is removed only if it still names the
    /// retained directory; a renamed original or a replacement at the old
    /// name is never deleted, and then cleanup fails (the handle is kept for
    /// a retry). Idempotent; never touches the ledger.
    fn close(&mut self) -> CleanupStatus {
        if self.released {
            return CleanupStatus::Discarded;
        }
        let revoked = self.revoke_grant();
        let removed = if !self.created {
            true
        } else {
            match self.handle.as_ref() {
                Some(handle) => {
                    handle.remove_all_entries().is_ok()
                        && self
                            .parent
                            .remove_subdir_if_identity(&self.name, handle.identity())
                            .is_ok()
                }
                // Created but never opened: its identity is unknown, so it
                // cannot be proven safe to remove.
                None => false,
            }
        };
        if removed {
            self.handle = None;
        }
        if revoked && removed {
            self.released = true;
            CleanupStatus::Discarded
        } else {
            CleanupStatus::DiscardFailed
        }
    }

    #[cfg(test)]
    fn path(&self) -> PathBuf {
        self.parent_path.join(&self.name)
    }
}

impl Drop for StagingLease {
    fn drop(&mut self) {
        if !self.released {
            let _ = self.close();
        }
    }
}

/// One governed coding run.
pub struct CodingRun {
    id: RunId,
    state: RunState,
    cleanup: CleanupStatus,
    scopes: RunScopes,
    profile: StructuralProfile,
    ledger: Arc<dyn LedgerStore>,
    registry: Arc<WorkspaceAuthorityRegistry>,
    project_grant: WorkspaceGrantId,
    binding: WorkspaceBinding,
    project: Option<DirHandle>,
    staging: Option<StagingLease>,
    base: Option<Manifest>,
    generation_valid: bool,
    next_op: u64,
    model_pin: Option<ModelPin>,
    created: BTreeSet<RelPath>,
    verification: Option<StructuralVerification>,
    /// The project and its directory identity at selection, for runs
    /// created from an owner-selected project.
    project_id: Option<ProjectId>,
    expected_root: Option<NodeIdentity>,
    /// Base bytes of each base file, captured (and hash-checked) from
    /// staging before its first edit; the old side of the review.
    base_contents: BTreeMap<RelPath, Vec<u8>>,
}

impl std::fmt::Debug for CodingRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodingRun")
            .field("id", &self.id)
            .field("state", &self.state)
            .field("cleanup", &self.cleanup)
            .finish_non_exhaustive()
    }
}

impl CodingRun {
    /// Create a run over an existing backend-issued project grant. Nothing is
    /// resolved or opened yet; the creation is recorded first, and a run whose
    /// creation cannot be recorded does not exist.
    pub fn create(
        ledger: Arc<dyn LedgerStore>,
        registry: Arc<WorkspaceAuthorityRegistry>,
        project_grant: WorkspaceGrantId,
        binding: WorkspaceBinding,
        scopes: RunScopes,
    ) -> Result<Self, RunError> {
        Self::create_inner(ledger, registry, project_grant, binding, scopes, None)
    }

    /// Create a run over an owner-selected project. The run's grant is the
    /// fresh per-run grant from [`ProjectRegistry::grant_for_run`], and the
    /// folder must still be the directory selected by the owner when the run
    /// opens it.
    pub fn create_for_project(
        ledger: Arc<dyn LedgerStore>,
        project: ProjectGrant,
        scopes: RunScopes,
    ) -> Result<Self, RunError> {
        let ProjectGrant {
            project,
            grant,
            binding,
            identity,
            authority,
        } = project;
        Self::create_inner(
            ledger,
            authority,
            grant,
            binding,
            scopes,
            Some((project, identity)),
        )
    }

    fn create_inner(
        ledger: Arc<dyn LedgerStore>,
        registry: Arc<WorkspaceAuthorityRegistry>,
        project_grant: WorkspaceGrantId,
        binding: WorkspaceBinding,
        scopes: RunScopes,
        selected: Option<(ProjectId, NodeIdentity)>,
    ) -> Result<Self, RunError> {
        let run = Self {
            id: RunId::generate(),
            state: RunState::Created,
            cleanup: CleanupStatus::NotStarted,
            scopes,
            profile: StructuralProfile::V1,
            ledger,
            registry,
            project_grant,
            binding,
            project: None,
            staging: None,
            base: None,
            generation_valid: true,
            next_op: 0,
            model_pin: None,
            created: BTreeSet::new(),
            verification: None,
            project_id: selected.map(|(id, _)| id),
            expected_root: selected.map(|(_, identity)| identity),
            base_contents: BTreeMap::new(),
        };
        let mut facts = json!({
            "project_grant": project_grant,
            "agent": binding.agent_id.to_string(),
            "binding_run": binding.run_id.to_string(),
            "read": run.scopes.read().describe(),
            "write": run.scopes.write().describe(),
            "protected": run.scopes.protected().describe(),
            "profile": hex::encode(run.profile.hash()),
        });
        if let Some(project) = run.project_id {
            facts["project"] = json!(project.to_string());
        }
        record(run.ledger.as_ref(), run.id.0, EventKind::RunCreated, &facts)
            .map_err(RunError::Ledger)?;
        Ok(run)
    }

    pub fn id(&self) -> RunId {
        self.id
    }

    pub fn state(&self) -> RunState {
        self.state
    }

    pub fn cleanup_status(&self) -> CleanupStatus {
        self.cleanup
    }

    pub fn scopes(&self) -> &RunScopes {
        &self.scopes
    }

    pub fn base_manifest(&self) -> Option<&Manifest> {
        self.base.as_ref()
    }

    /// The owner-selected project this run was created for, if any.
    pub fn project_id(&self) -> Option<ProjectId> {
        self.project_id
    }

    /// The local model this run is bound to, if pinned.
    pub fn model_pin(&self) -> Option<&ModelPin> {
        self.model_pin.as_ref()
    }

    /// The passing structural verification of the current candidate, once
    /// the run is `StructurallyVerified`.
    pub fn verification(&self) -> Option<&StructuralVerification> {
        self.verification.as_ref()
    }

    fn require(&self, operation: &'static str, allowed: &[RunState]) -> Result<(), RunError> {
        if allowed.contains(&self.state) {
            Ok(())
        } else {
            Err(RunError::InvalidState {
                operation,
                state: self.state,
            })
        }
    }

    // ── Authority ───────────────────────────────────────────────────────

    fn resolve_grant(&self, grant: WorkspaceGrantId) -> Result<WorkspaceGrant, RunError> {
        self.registry
            .resolve(grant, self.binding)
            .map_err(|error| match error {
                WorkspaceAuthorityError::RevokedGrant => {
                    RunError::Revoked(RevocationReason::GrantRevoked)
                }
                WorkspaceAuthorityError::ExpiredGrant => {
                    RunError::Revoked(RevocationReason::GrantExpired)
                }
                _ => RunError::AuthorityDenied,
            })
    }

    /// Re-resolve the project grant and confirm the grant root still is the
    /// directory retained when the run was granted.
    fn revalidate_project(&self) -> Result<(), RunError> {
        let grant = self.resolve_grant(self.project_grant)?;
        if *grant.permission() == FsPermissionLevel::Deny {
            return Err(RunError::AuthorityDenied);
        }
        let retained = self
            .project
            .as_ref()
            .ok_or(RunError::AuthorityDenied)?
            .identity();
        let fresh =
            DirHandle::open_absolute(grant.root()).map_err(|_| RunError::IdentityChanged)?;
        if fresh.identity() != retained {
            return Err(RunError::IdentityChanged);
        }
        Ok(())
    }

    /// Re-resolve the staging grant and confirm its root is the retained
    /// staging directory.
    fn revalidate_staging(&self) -> Result<(), RunError> {
        let staging = self.staging.as_ref().ok_or(RunError::AuthorityDenied)?;
        let staging_grant = staging.grant.ok_or(RunError::AuthorityDenied)?;
        let grant = self.resolve_grant(staging_grant)?;
        if *grant.permission() != FsPermissionLevel::ReadWrite {
            return Err(RunError::AuthorityDenied);
        }
        let fresh =
            DirHandle::open_absolute(grant.root()).map_err(|_| RunError::IdentityChanged)?;
        if fresh.identity() != staging.handle()?.identity() {
            return Err(RunError::IdentityChanged);
        }
        Ok(())
    }

    // ── Terminal transitions ───────────────────────────────────────────

    /// Enter a terminal state. Authority closure comes first and never
    /// depends on the ledger: the staging grant is revoked, and the actual
    /// resulting state is derived from that outcome (the requested state, or
    /// `RecoveryRequired(StagingRevocationFailed)` if revocation failed).
    /// Only then is that actual state recorded, once, so the ledger's final
    /// state-bearing event agrees with the backend state. If that record
    /// fails the run fails closed: `TerminalNotRecorded`, unless revocation
    /// also failed, in which case the more severe `StagingRevocationFailed`
    /// is kept. Nothing is retried.
    fn enter_terminal(&mut self, requested: RunState) {
        if terminal_event(requested).is_none() {
            return;
        }
        let closed = self.staging.as_mut().is_none_or(StagingLease::revoke_grant);
        let actual = if closed {
            requested
        } else {
            RunState::RecoveryRequired(RecoveryReason::StagingRevocationFailed)
        };
        let recorded = self.record_state(actual);
        self.state = match (recorded, actual) {
            (true, actual) => actual,
            (false, RunState::RecoveryRequired(RecoveryReason::StagingRevocationFailed)) => actual,
            (false, _) => RunState::RecoveryRequired(RecoveryReason::TerminalNotRecorded),
        };
        if matches!(
            self.state,
            RunState::RecoveryRequired(_) | RunState::Failed(_)
        ) {
            self.generation_valid = false;
        }
    }

    /// Record a terminal state as its state-bearing ledger event.
    fn record_state(&self, state: RunState) -> bool {
        let Some((event, reason)) = terminal_event(state) else {
            return false;
        };
        record(
            self.ledger.as_ref(),
            self.id.0,
            event,
            &json!({ "reason": reason }),
        )
        .is_ok()
    }

    /// Enter RecoveryRequired even if the ledger cannot record it. The
    /// staging grant is revoked, and with `discard` the staging generation
    /// is removed, first and independently of the ledger. The actual reason
    /// (`StagingRevocationFailed` if the grant could not be revoked, else
    /// `reason`) is then recorded once, best-effort: the run is already fail
    /// closed, and a failed record is not retried.
    ///
    /// Returns the error matching the resulting state.
    fn force_recovery(&mut self, reason: RecoveryReason, discard: bool) -> RunError {
        self.generation_valid = false;
        if discard {
            self.close_staging();
        } else if let Some(staging) = self.staging.as_mut() {
            staging.revoke_grant();
        }
        let actual = if self
            .staging
            .as_ref()
            .is_none_or(StagingLease::authority_closed)
        {
            reason
        } else {
            RecoveryReason::StagingRevocationFailed
        };
        self.state = RunState::RecoveryRequired(actual);
        let _ = self.record_state(self.state);
        RunError::RecoveryRequired(actual)
    }

    /// The result of a requested terminal transition: success only if the
    /// run actually ended in the requested state.
    fn terminal_result(&self, requested: RunState) -> Result<(), RunError> {
        match self.state {
            state if state == requested => Ok(()),
            RunState::RecoveryRequired(reason) => Err(RunError::RecoveryRequired(reason)),
            state => Err(RunError::InvalidState {
                operation: "terminal transition",
                state,
            }),
        }
    }

    /// Close the staging lease (revoke and remove). A fully closed lease is
    /// dropped; a lease whose cleanup failed is kept so Drop retries.
    fn close_staging(&mut self) {
        if let Some(mut staging) = self.staging.take() {
            self.cleanup = staging.close();
            if self.cleanup != CleanupStatus::Discarded {
                self.staging = Some(staging);
            }
        }
    }

    /// Turn an authority, identity or staging error into the matching
    /// terminal state; return the error unchanged.
    fn fail(&mut self, error: RunError) -> RunError {
        let terminal = match &error {
            RunError::Revoked(reason) => Some(RunState::Revoked(*reason)),
            RunError::AuthorityDenied => Some(RunState::Failed(FailureReason::AuthorityDenied)),
            RunError::IdentityChanged => Some(RunState::Failed(FailureReason::IdentityChanged)),
            RunError::Snapshot(_) => Some(RunState::Failed(FailureReason::SnapshotRejected)),
            RunError::StagingRedirect => Some(RunState::Failed(FailureReason::StagingRedirect)),
            RunError::StagingIo | RunError::StagingUnavailable => {
                Some(RunState::Failed(FailureReason::StagingIo))
            }
            _ => None,
        };
        if let Some(state) = terminal {
            if !self.state.is_terminal() {
                self.enter_terminal(state);
            }
        }
        error
    }

    /// Record a prepared staging mutation, perform it, record its outcome.
    /// If the prepared record fails, nothing is done. If the outcome record
    /// fails after the mutation ran, the staging generation is invalidated
    /// and discarded, and the run requires recovery.
    fn mutate<T>(
        &mut self,
        kind: &'static str,
        facts: Value,
        action: impl FnOnce(&Self) -> Result<T, RunError>,
        outcome_facts: impl FnOnce(&T) -> Value,
    ) -> Result<T, RunError> {
        let op = self.next_op;
        self.next_op += 1;
        record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::OperationPrepared,
            &json!({ "op": op, "kind": kind, "facts": facts }),
        )
        .map_err(RunError::Ledger)?;
        let result = action(self);
        let outcome = match &result {
            Ok(value) => json!({ "op": op, "ok": true, "facts": outcome_facts(value) }),
            Err(error) => json!({ "op": op, "ok": false, "error": error.to_string() }),
        };
        if record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::OperationOutcome,
            &outcome,
        )
        .is_err()
        {
            return Err(self.force_recovery(RecoveryReason::OutcomeNotRecorded, true));
        }
        if result.is_err() {
            self.generation_valid = false;
        }
        result
    }

    // ── Created → Granted ──────────────────────────────────────────────

    /// Resolve the project grant, retain the project directory identity,
    /// allocate the staging directory and issue its backend grant.
    pub fn grant(&mut self, parent: &StagingParent) -> Result<(), RunError> {
        self.require("grant", &[RunState::Created])?;
        let grant = match self.resolve_grant(self.project_grant) {
            Ok(grant) => grant,
            Err(error) => return Err(self.fail(error)),
        };
        if *grant.permission() == FsPermissionLevel::Deny {
            return Err(self.fail(RunError::AuthorityDenied));
        }
        let project = match DirHandle::open_absolute(grant.root()) {
            Ok(handle) => handle,
            Err(_) => return Err(self.fail(RunError::IdentityChanged)),
        };
        if self
            .expected_root
            .is_some_and(|expected| expected != project.identity())
        {
            return Err(self.fail(RunError::IdentityChanged));
        }
        self.project = Some(project);
        let expires_at = grant.expires_at();
        let name = self.id.to_string();

        // Prepared → allocation → Outcome. The lease owns the directory and
        // the grant from the moment either exists.
        let op = self.next_op;
        self.next_op += 1;
        record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::OperationPrepared,
            &json!({ "op": op, "kind": "allocate_staging", "facts": { "name": name } }),
        )
        .map_err(RunError::Ledger)?;
        let mut lease =
            match StagingLease::begin(Arc::clone(&self.registry), self.binding, parent, name) {
                Ok(lease) => lease,
                Err(error) => {
                    let _ = record(
                        self.ledger.as_ref(),
                        self.id.0,
                        EventKind::OperationOutcome,
                        &json!({ "op": op, "ok": false, "error": error.to_string() }),
                    );
                    return Err(self.fail(error));
                }
            };
        let allocated = lease.allocate(expires_at);
        let outcome = match &allocated {
            Ok(()) => json!({ "op": op, "ok": true, "facts": { "staging_grant": lease.grant } }),
            Err(error) => json!({ "op": op, "ok": false, "error": error.to_string() }),
        };
        let outcome_recorded = record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::OperationOutcome,
            &outcome,
        );
        if outcome_recorded.is_err() {
            // The lease is closed before it is ever installed.
            self.cleanup = lease.close();
            if self.cleanup != CleanupStatus::Discarded {
                self.staging = Some(lease);
            }
            return Err(self.force_recovery(RecoveryReason::OutcomeNotRecorded, true));
        }
        if let Err(error) = allocated {
            self.cleanup = lease.close();
            if self.cleanup != CleanupStatus::Discarded {
                self.staging = Some(lease);
            }
            return Err(self.fail(error));
        }
        self.staging = Some(lease);
        if let Err(error) = self.revalidate_staging() {
            let error = self.fail(error);
            self.close_staging();
            return Err(error);
        }
        let facts = json!({ "staging_grant": self.staging.as_ref().and_then(|s| s.grant) });
        if record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::RunGranted,
            &facts,
        )
        .is_err()
        {
            return Err(self.force_recovery(RecoveryReason::GrantedNotRecorded, true));
        }
        self.state = RunState::Granted;
        Ok(())
    }

    // ── Granted → Staged ───────────────────────────────────────────────

    /// Copy the frozen read scope into staging and record the base manifest.
    pub fn snapshot(&mut self) -> Result<ManifestHash, RunError> {
        self.require("snapshot", &[RunState::Granted])?;
        if let Err(error) = self
            .revalidate_project()
            .and_then(|()| self.revalidate_staging())
        {
            return Err(self.fail(error));
        }
        let copied = self.mutate(
            "snapshot",
            json!({ "read": self.scopes.read().describe() }),
            |run| run.copy_read_scope(),
            |manifest: &Manifest| {
                json!({
                    "base_manifest": manifest.hash().to_hex(),
                    "files": manifest.len(),
                    "bytes": manifest.total_bytes(),
                })
            },
        );
        let manifest = match copied {
            Ok(manifest) => manifest,
            Err(error) => return Err(self.fail(error)),
        };
        if let Err(error) = self
            .revalidate_project()
            .and_then(|()| self.revalidate_staging())
        {
            return Err(self.fail(error));
        }
        let hash = manifest.hash();
        self.base = Some(manifest);
        self.state = RunState::Staged;
        Ok(hash)
    }

    fn copy_read_scope(&self) -> Result<Manifest, RunError> {
        let project = self.project.as_ref().ok_or(RunError::AuthorityDenied)?;
        let staging = self.staging_handle()?;
        let mut copy = SnapshotCopy {
            profile: self.profile,
            manifest: Manifest::default(),
            bytes: 0,
            root_dev: project.identity().dev,
        };
        for entry in self.scopes.read().entries() {
            match entry {
                ScopeEntry::WholeProject => copy.copy_dir(project, staging, &mut Vec::new())?,
                ScopeEntry::Tree(path) | ScopeEntry::File(path) => {
                    copy.copy_entry(project, staging, path, matches!(entry, ScopeEntry::Tree(_)))?
                }
            }
        }
        Ok(copy.manifest)
    }

    // ── Staged/Candidate → Candidate ───────────────────────────────────

    /// Apply one candidate edit to staging only.
    pub fn edit(&mut self, edit: CandidateEdit) -> Result<(), RunError> {
        self.require("edit", &[RunState::Staged, RunState::Candidate])?;
        if !self.generation_valid {
            return Err(RunError::RecoveryRequired(
                RecoveryReason::OutcomeNotRecorded,
            ));
        }
        if let Some(rejection) = self.edit_policy_rejection(&edit) {
            return Err(self.reject_edit(&edit, rejection));
        }
        if let Err(error) = self
            .revalidate_project()
            .and_then(|()| self.revalidate_staging())
        {
            return Err(self.fail(error));
        }
        // Read-only probe: a redirect is an attack and ends the run; a
        // missing or existing target is a plain rejection.
        match self.probe_target(&edit) {
            Ok(()) => {}
            Err(RunError::EditRejected(rejection)) => {
                return Err(self.reject_edit(&edit, rejection))
            }
            Err(error) => return Err(self.fail(error)),
        }
        if let Err(error) = self.capture_base(&edit) {
            return Err(self.fail(error));
        }
        let facts = json!({
            "path": edit.path().as_string(),
            "kind": edit.kind(),
            "size": edit.content().len(),
            "sha256": hex::encode(ManifestEntry::of(edit.content()).sha256),
        });
        let applied = self.mutate("edit", facts, |run| run.apply_edit(&edit), |_| json!({}));
        if let Err(error) = applied {
            return Err(self.fail(error));
        }
        if let Err(error) = self.revalidate_staging() {
            return Err(self.fail(error));
        }
        if let CandidateEdit::Create { path, .. } = &edit {
            self.created.insert(path.clone());
        }
        self.state = RunState::Candidate;
        Ok(())
    }

    /// Before a base file is first replaced, keep its staged bytes (which
    /// must still hash to the base manifest entry) for the review.
    fn capture_base(&mut self, edit: &CandidateEdit) -> Result<(), RunError> {
        let CandidateEdit::Replace { path, .. } = edit else {
            return Ok(());
        };
        if self.base_contents.contains_key(path) {
            return Ok(());
        }
        let Some(entry) = self.base.as_ref().and_then(|base| base.get(path)).cloned() else {
            return Ok(());
        };
        match self.read_staged_file(path, self.profile.max_file_bytes) {
            Ok(bytes) if ManifestEntry::of(&bytes) == entry => {
                self.base_contents.insert(path.clone(), bytes);
                Ok(())
            }
            _ => Err(RunError::StagingRedirect),
        }
    }

    fn edit_policy_rejection(&self, edit: &CandidateEdit) -> Option<EditRejection> {
        let path = edit.path();
        if self.scopes.protected().covers(path) {
            return Some(EditRejection::ProtectedInput);
        }
        if !self.scopes.editable(path) {
            return Some(EditRejection::OutsideWriteScope);
        }
        if edit.content().len() as u64 > self.profile.max_file_bytes {
            return Some(EditRejection::TooLarge);
        }
        if !StructuralProfile::text_ok(edit.content()) {
            return Some(EditRejection::NotText);
        }
        None
    }

    fn reject_edit(&mut self, edit: &CandidateEdit, rejection: EditRejection) -> RunError {
        let facts = json!({
            "path": edit.path().as_string(),
            "kind": edit.kind(),
            "reason": format!("{rejection:?}"),
        });
        match record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::EditRejected,
            &facts,
        ) {
            Ok(_) => RunError::EditRejected(rejection),
            Err(_) => {
                // A required security event was not recorded: the run
                // cannot continue.
                self.force_recovery(RecoveryReason::RejectionNotRecorded, false)
            }
        }
    }

    /// Walk to the target's directory without creating anything.
    fn probe_target(&self, edit: &CandidateEdit) -> Result<(), RunError> {
        let staging = self.staging_handle()?;
        let (parents, name) = edit.path().parent_and_name();
        let mut owned: Option<DirHandle> = None;
        for part in parents {
            let dir = owned.as_ref().unwrap_or(staging);
            match dir.kind(part).map_err(|_| RunError::StagingIo)? {
                EntryKind::Directory => {
                    owned = Some(
                        dir.open_subdir(part)
                            .map_err(|_| RunError::StagingRedirect)?,
                    )
                }
                EntryKind::Missing => {
                    return match edit {
                        CandidateEdit::Create { .. } => Ok(()),
                        CandidateEdit::Replace { .. } => {
                            Err(RunError::EditRejected(EditRejection::NotStaged))
                        }
                    }
                }
                _ => return Err(RunError::StagingRedirect),
            }
        }
        let dir = owned.as_ref().unwrap_or(staging);
        match (dir.kind(name).map_err(|_| RunError::StagingIo)?, edit) {
            (EntryKind::Regular, CandidateEdit::Replace { .. }) => dir
                .regular_metadata(name)
                .map(|_| ())
                .map_err(|_| RunError::StagingRedirect),
            (EntryKind::Missing, CandidateEdit::Create { .. }) => Ok(()),
            (EntryKind::Missing, CandidateEdit::Replace { .. }) => {
                Err(RunError::EditRejected(EditRejection::NotStaged))
            }
            (EntryKind::Regular, CandidateEdit::Create { .. }) => {
                Err(RunError::EditRejected(EditRejection::AlreadyExists))
            }
            _ => Err(RunError::StagingRedirect),
        }
    }

    fn apply_edit(&self, edit: &CandidateEdit) -> Result<(), RunError> {
        let staging = self.staging_handle()?;
        let (parents, name) = edit.path().parent_and_name();
        let mut owned: Option<DirHandle> = None;
        for part in parents {
            let dir = owned.as_ref().unwrap_or(staging);
            let next = match (dir.kind(part).map_err(|_| RunError::StagingIo)?, edit) {
                (EntryKind::Directory, _) => dir.open_subdir(part),
                (EntryKind::Missing, CandidateEdit::Create { .. }) => {
                    dir.make_subdir(part).and_then(|()| dir.open_subdir(part))
                }
                _ => return Err(RunError::StagingRedirect),
            }
            .map_err(|_| RunError::StagingRedirect)?;
            owned = Some(next);
        }
        let dir = owned.as_ref().unwrap_or(staging);
        match edit {
            CandidateEdit::Replace { content, .. } => dir.replace_file(name, content),
            CandidateEdit::Create { content, .. } => dir.create_file(name, content),
        }
        .map_err(|_| RunError::StagingRedirect)
    }

    // ── Candidate → StructurallyVerified ───────────────────────────────

    /// Verify the candidate structurally and bind the result to the run, the
    /// base manifest, the exact candidate bytes and the profile.
    pub fn verify_structural(&mut self) -> Result<StructuralVerification, RunError> {
        self.require("verify_structural", &[RunState::Candidate])?;
        if !self.generation_valid {
            return Err(RunError::RecoveryRequired(
                RecoveryReason::OutcomeNotRecorded,
            ));
        }
        if let Err(error) = self
            .revalidate_project()
            .and_then(|()| self.revalidate_staging())
        {
            return Err(self.fail(error));
        }
        match ledger::analyze(self.ledger.as_ref(), self.id.0) {
            Ok(recovery) if recovery.is_clean() => {}
            Ok(_) => {
                self.enter_terminal(RunState::RecoveryRequired(
                    RecoveryReason::UnmatchedPrepared,
                ));
                return Err(RunError::RecoveryRequired(
                    RecoveryReason::UnmatchedPrepared,
                ));
            }
            Err(LedgerFailure::Integrity) => {
                self.enter_terminal(RunState::RecoveryRequired(RecoveryReason::LedgerIntegrity));
                return Err(RunError::RecoveryRequired(RecoveryReason::LedgerIntegrity));
            }
            Err(error) => return Err(RunError::Ledger(error)),
        }
        let base = self.base.clone().ok_or(RunError::AuthorityDenied)?;
        let staging = self.staging_handle()?;
        let mut changed_text = Vec::new();
        let (candidate, mut violations) =
            structural::scan_staging(staging, &self.profile, &mut changed_text, &base);
        structural::check_against_base(
            &self.scopes,
            &base,
            &candidate,
            &changed_text,
            &mut violations,
        );
        let result = StructuralVerification {
            run_id: self.id,
            base_manifest_hash: base.hash(),
            candidate_manifest_hash: candidate.hash(),
            profile_hash: self.profile.hash(),
            outcome: if violations.is_empty() {
                StructuralOutcome::Passed
            } else {
                StructuralOutcome::Rejected(violations)
            },
        };
        let facts = json!({
            "base_manifest": result.base_manifest_hash.to_hex(),
            "candidate_manifest": result.candidate_manifest_hash.to_hex(),
            "profile": hex::encode(result.profile_hash),
            "binding": hex::encode(result.binding_hash()),
            "passed": result.passed(),
            "violations": match &result.outcome {
                StructuralOutcome::Passed => 0,
                StructuralOutcome::Rejected(v) => v.len(),
            },
        });
        record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::StructuralVerification,
            &facts,
        )
        .map_err(RunError::Ledger)?;
        if result.passed() {
            // The verified candidate keeps its bytes but no write authority.
            let revoked = self
                .staging
                .as_mut()
                .is_some_and(StagingLease::revoke_grant);
            if !revoked {
                return Err(self.force_recovery(RecoveryReason::StagingRevocationFailed, false));
            }
            self.verification = Some(result.clone());
            self.state = RunState::StructurallyVerified;
        } else {
            self.enter_terminal(RunState::Failed(FailureReason::StructuralRejected));
        }
        Ok(result)
    }

    // ── Owner review (Phase One) ───────────────────────────────────────

    /// Compute the owner's review of the verified candidate. The staged
    /// candidate is re-read and must still hash to the verified candidate
    /// manifest; otherwise the verification is withdrawn and the run can
    /// never be approved. The review is recorded before it is returned.
    pub fn review(&mut self) -> Result<Review, RunError> {
        self.require("review", &[RunState::StructurallyVerified])?;
        let verification = self
            .verification
            .clone()
            .ok_or(RunError::CandidateUnavailable)?;
        if !self.generation_valid {
            return Err(RunError::CandidateChanged);
        }
        let (candidate, new_bytes) = match self.verified_candidate(&verification) {
            Ok(found) => found,
            Err(error) => return Err(self.withdraw_verification(error)),
        };
        let base = self.base.clone().ok_or(RunError::CandidateUnavailable)?;
        let binding = ReviewBinding {
            run_id: self.id,
            base_manifest_hash: verification.base_manifest_hash,
            candidate_manifest_hash: verification.candidate_manifest_hash,
            profile_hash: verification.profile_hash,
        };
        let review = review::build(
            binding,
            base.entries(),
            candidate.entries(),
            &self.base_contents,
            &new_bytes,
        );
        let facts = json!({
            "binding": hex::encode(binding.hash()),
            "base_manifest": binding.base_manifest_hash.to_hex(),
            "candidate_manifest": binding.candidate_manifest_hash.to_hex(),
            "create": review.count(ChangeKind::Create),
            "replace": review.count(ChangeKind::Replace),
            "delete": review.count(ChangeKind::Delete),
        });
        record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::ReviewComputed,
            &facts,
        )
        .map_err(RunError::Ledger)?;
        Ok(review)
    }

    /// Re-read the staged candidate: its manifest must hash to the verified
    /// candidate hash with no structural violation, and each changed file's
    /// bytes must match its manifest entry. Returns the manifest and the
    /// changed files' bytes.
    fn verified_candidate(
        &self,
        verification: &StructuralVerification,
    ) -> Result<(Manifest, BTreeMap<RelPath, Vec<u8>>), RunError> {
        let base = self.base.as_ref().ok_or(RunError::CandidateUnavailable)?;
        let staging = self
            .staging_handle()
            .map_err(|_| RunError::CandidateUnavailable)?;
        let mut changed_text = Vec::new();
        let (candidate, violations) =
            structural::scan_staging(staging, &self.profile, &mut changed_text, base);
        if !violations.is_empty() || candidate.hash() != verification.candidate_manifest_hash {
            return Err(RunError::CandidateChanged);
        }
        let mut bytes = BTreeMap::new();
        for (path, entry) in candidate.entries() {
            if base.get(path) == Some(entry) {
                continue;
            }
            let content = self
                .read_staged_file(path, self.profile.max_file_bytes)
                .map_err(|_| RunError::CandidateChanged)?;
            if ManifestEntry::of(&content) != *entry {
                return Err(RunError::CandidateChanged);
            }
            bytes.insert(path.clone(), content);
        }
        Ok((candidate, bytes))
    }

    /// The verified candidate is gone or changed: it can never be approved.
    fn withdraw_verification(&mut self, error: RunError) -> RunError {
        self.verification = None;
        self.generation_valid = false;
        let _ = record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::CandidateWithdrawn,
            &json!({ "reason": error.to_string() }),
        );
        error
    }

    // ── Worker support (Phase One) ─────────────────────────────────────

    /// Bind the run to one local model before any worker runs. Once pinned,
    /// the pin never changes; a worker with another model is refused.
    pub fn pin_model(&mut self, pin: ModelPin) -> Result<(), RunError> {
        self.require("pin_model", &[RunState::Staged])?;
        if self.model_pin.is_some() {
            return Err(RunError::InvalidState {
                operation: "pin_model",
                state: self.state,
            });
        }
        let facts = json!({
            "provider": pin.provider(),
            "endpoint": pin.endpoint(),
            "model": pin.model(),
        });
        record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::ModelPinned,
            &facts,
        )
        .map_err(RunError::Ledger)?;
        self.model_pin = Some(pin);
        Ok(())
    }

    /// Staged files with their base sizes, plus files created in staging.
    pub fn staged_files(&self) -> Vec<(RelPath, u64)> {
        let mut files: Vec<(RelPath, u64)> = self
            .base
            .iter()
            .flat_map(|base| base.entries().iter())
            .map(|(path, entry)| (path.clone(), entry.size))
            .collect();
        for path in &self.created {
            if self
                .base
                .as_ref()
                .is_none_or(|base| base.get(path).is_none())
            {
                files.push((path.clone(), 0));
            }
        }
        files.sort();
        files
    }

    /// Read one staged file inside the read scope, at most `max_bytes`. The
    /// read is recorded before its content is returned. A path outside the
    /// read scope, missing or too large is a plain refusal; a redirected
    /// staging entry ends the run.
    pub(crate) fn read_staged(
        &mut self,
        path: &RelPath,
        max_bytes: u64,
    ) -> Result<Vec<u8>, RunError> {
        self.require("read_staged", &[RunState::Staged, RunState::Candidate])?;
        if !self.generation_valid {
            return Err(RunError::RecoveryRequired(
                RecoveryReason::OutcomeNotRecorded,
            ));
        }
        if !self.scopes.read().covers(path) {
            return Err(RunError::EditRejected(EditRejection::OutsideReadScope));
        }
        if let Err(error) = self
            .revalidate_project()
            .and_then(|()| self.revalidate_staging())
        {
            return Err(self.fail(error));
        }
        let content = match self.read_staged_file(path, max_bytes) {
            Ok(content) => content,
            Err(RunError::EditRejected(rejection)) => {
                return Err(RunError::EditRejected(rejection))
            }
            Err(error) => return Err(self.fail(error)),
        };
        let facts = json!({
            "path": path.as_string(),
            "size": content.len(),
            "sha256": hex::encode(ManifestEntry::of(&content).sha256),
        });
        record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::WorkerRead,
            &facts,
        )
        .map_err(RunError::Ledger)?;
        Ok(content)
    }

    fn read_staged_file(&self, path: &RelPath, max_bytes: u64) -> Result<Vec<u8>, RunError> {
        let staging = self.staging_handle()?;
        let (parents, name) = path.parent_and_name();
        let mut owned: Option<DirHandle> = None;
        for part in parents {
            let dir = owned.as_ref().unwrap_or(staging);
            match dir.kind(part).map_err(|_| RunError::StagingIo)? {
                EntryKind::Directory => {
                    owned = Some(
                        dir.open_subdir(part)
                            .map_err(|_| RunError::StagingRedirect)?,
                    )
                }
                EntryKind::Missing => return Err(RunError::EditRejected(EditRejection::NotStaged)),
                _ => return Err(RunError::StagingRedirect),
            }
        }
        let dir = owned.as_ref().unwrap_or(staging);
        match dir.kind(name).map_err(|_| RunError::StagingIo)? {
            EntryKind::Regular => {}
            EntryKind::Missing => return Err(RunError::EditRejected(EditRejection::NotStaged)),
            _ => return Err(RunError::StagingRedirect),
        }
        let size = dir
            .regular_metadata(name)
            .map_err(|_| RunError::StagingRedirect)?
            .len();
        if size > max_bytes.min(self.profile.max_file_bytes) {
            return Err(RunError::EditRejected(EditRejection::TooLarge));
        }
        dir.read_regular(name, max_bytes.min(self.profile.max_file_bytes))
            .map_err(|_| RunError::StagingIo)
    }

    /// Record a worker event (fail closed: the caller stops on error).
    pub(crate) fn record_worker(&self, event: EventKind, facts: &Value) -> Result<(), RunError> {
        if self.state.is_terminal() {
            return Err(RunError::InvalidState {
                operation: "worker",
                state: self.state,
            });
        }
        record(self.ledger.as_ref(), self.id.0, event, facts)
            .map(|_| ())
            .map_err(RunError::Ledger)
    }

    /// End a non-terminal run for a worker-level failure.
    pub(crate) fn fail_worker(&mut self, reason: FailureReason) {
        if !self.state.is_terminal() {
            self.enter_terminal(RunState::Failed(reason));
        }
    }

    // ── Cancellation, revocation, cleanup ──────────────────────────────

    pub fn cancel(&mut self) -> Result<(), RunError> {
        if self.state.is_terminal() {
            return Err(RunError::InvalidState {
                operation: "cancel",
                state: self.state,
            });
        }
        self.enter_terminal(RunState::Cancelled);
        self.terminal_result(RunState::Cancelled)
    }

    pub fn revoke(&mut self) -> Result<(), RunError> {
        if self.state.is_terminal() {
            return Err(RunError::InvalidState {
                operation: "revoke",
                state: self.state,
            });
        }
        self.enter_terminal(RunState::Revoked(RevocationReason::Explicit));
        self.terminal_result(RunState::Revoked(RevocationReason::Explicit))
    }

    /// Revoke the staging grant and remove the disposable staging directory
    /// through retained handles. Allowed once the run is terminal; the run
    /// outcome is unchanged. Idempotent. The cleanup record is best-effort;
    /// the returned status reflects the cleanup itself.
    pub fn discard_staging(&mut self) -> Result<CleanupStatus, RunError> {
        if !self.state.is_terminal() {
            return Err(RunError::InvalidState {
                operation: "discard_staging",
                state: self.state,
            });
        }
        self.generation_valid = false;
        self.verification = None;
        self.close_staging();
        let facts = json!({ "status": format!("{:?}", self.cleanup) });
        let _ = record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::StagingDiscarded,
            &facts,
        );
        Ok(self.cleanup)
    }

    fn staging_handle(&self) -> Result<&DirHandle, RunError> {
        self.staging
            .as_ref()
            .ok_or(RunError::AuthorityDenied)?
            .handle()
    }

    #[cfg(test)]
    pub(crate) fn staging_path_for_test(&self) -> Option<PathBuf> {
        self.staging.as_ref().map(StagingLease::path)
    }

    /// Replace the staging grant id with one the registry does not know, so
    /// revocation fails (test-only authority-closure failure seam).
    #[cfg(test)]
    pub(crate) fn substitute_unrevocable_staging_grant_for_test(&mut self) {
        let unknown: WorkspaceGrantId =
            serde_json::from_value(json!(Uuid::new_v4())).expect("grant id");
        if let Some(staging) = self.staging.as_mut() {
            staging.grant = Some(unknown);
            staging.grant_revoked = false;
        }
    }

    #[cfg(test)]
    pub(crate) fn staging_grant_for_test(&self) -> Option<WorkspaceGrantId> {
        self.staging.as_ref().and_then(|staging| staging.grant)
    }
}

/// The state-bearing ledger event and reason for a terminal state.
fn terminal_event(state: RunState) -> Option<(EventKind, String)> {
    match state {
        RunState::Cancelled => Some((EventKind::RunCancelled, "cancelled".to_string())),
        RunState::Revoked(reason) => Some((EventKind::RunRevoked, format!("{reason:?}"))),
        RunState::Failed(reason) => Some((EventKind::RunFailed, format!("{reason:?}"))),
        RunState::RecoveryRequired(reason) => {
            Some((EventKind::RecoveryRequired, format!("{reason:?}")))
        }
        _ => None,
    }
}

/// State of one snapshot copy.
struct SnapshotCopy {
    profile: StructuralProfile,
    manifest: Manifest,
    bytes: u64,
    root_dev: u64,
}

impl SnapshotCopy {
    fn reject(kind: fn(String) -> SnapshotRejection, parts: &[String]) -> RunError {
        RunError::Snapshot(kind(parts.join("/")))
    }

    /// Stage one scope entry (a file or a subtree) at `path`.
    fn copy_entry(
        &mut self,
        project: &DirHandle,
        staging: &DirHandle,
        path: &RelPath,
        tree: bool,
    ) -> Result<(), RunError> {
        let (parents, name) = path.parent_and_name();
        let mut source: Option<DirHandle> = None;
        let mut target: Option<DirHandle> = None;
        let mut walked = Vec::new();
        for part in parents {
            walked.push(part.clone());
            let dir = source.as_ref().unwrap_or(project);
            match dir.kind(part).map_err(|_| RunError::StagingIo)? {
                EntryKind::Directory => {}
                EntryKind::Missing => return Ok(()),
                EntryKind::Symlink => {
                    return Err(Self::reject(SnapshotRejection::Symlink, &walked))
                }
                _ => return Err(Self::reject(SnapshotRejection::NotADirectory, &walked)),
            }
            let next = self.open_source_dir(dir, part, &walked)?;
            source = Some(next);
            let out = target.as_ref().unwrap_or(staging);
            target = Some(ensure_dir(out, part)?);
        }
        walked.push(name.to_string());
        let dir = source.as_ref().unwrap_or(project);
        let out = target.as_ref().unwrap_or(staging);
        match dir.kind(name).map_err(|_| RunError::StagingIo)? {
            EntryKind::Missing => Ok(()),
            EntryKind::Symlink => Err(Self::reject(SnapshotRejection::Symlink, &walked)),
            EntryKind::Special => Err(Self::reject(SnapshotRejection::SpecialFile, &walked)),
            EntryKind::Regular => self.copy_file(dir, out, name, &walked),
            EntryKind::Directory if tree => {
                let child = self.open_source_dir(dir, name, &walked)?;
                let child_out = ensure_dir(out, name)?;
                self.copy_dir(&child, &child_out, &mut walked)
            }
            EntryKind::Directory => Err(Self::reject(SnapshotRejection::NotADirectory, &walked)),
        }
    }

    fn open_source_dir(
        &self,
        dir: &DirHandle,
        name: &str,
        walked: &[String],
    ) -> Result<DirHandle, RunError> {
        let handle = dir
            .open_subdir(name)
            .map_err(|_| Self::reject(SnapshotRejection::OtherDevice, walked))?;
        if handle.identity().dev != self.root_dev {
            return Err(Self::reject(SnapshotRejection::OtherDevice, walked));
        }
        Ok(handle)
    }

    /// Stage a whole directory, skipping Git metadata.
    fn copy_dir(
        &mut self,
        source: &DirHandle,
        target: &DirHandle,
        prefix: &mut Vec<String>,
    ) -> Result<(), RunError> {
        let names = source
            .entries()
            .map_err(|_| Self::reject(SnapshotRejection::InvalidName, prefix))?;
        for name in names {
            if is_git_metadata(&name) {
                continue;
            }
            prefix.push(name.clone());
            if scope::validate_component(&name).is_err() {
                return Err(Self::reject(SnapshotRejection::InvalidName, prefix));
            }
            match source.kind(&name).map_err(|_| RunError::StagingIo)? {
                EntryKind::Directory => {
                    let child = self.open_source_dir(source, &name, prefix)?;
                    let child_out = ensure_dir(target, &name)?;
                    self.copy_dir(&child, &child_out, prefix)?;
                }
                EntryKind::Regular => self.copy_file(source, target, &name, prefix)?,
                EntryKind::Symlink => return Err(Self::reject(SnapshotRejection::Symlink, prefix)),
                EntryKind::Special => {
                    return Err(Self::reject(SnapshotRejection::SpecialFile, prefix))
                }
                EntryKind::Missing => {}
            }
            prefix.pop();
        }
        target.sync().map_err(|_| RunError::StagingIo)
    }

    fn copy_file(
        &mut self,
        source: &DirHandle,
        target: &DirHandle,
        name: &str,
        parts: &[String],
    ) -> Result<(), RunError> {
        let path = RelPath::from_components(parts.to_vec())
            .map_err(|_| Self::reject(SnapshotRejection::InvalidName, parts))?;
        if self.manifest.get(&path).is_some() {
            return Ok(());
        }
        let Ok(metadata) = source.regular_metadata(name) else {
            // Classify why the pre-check failed.
            return Err(self.classify_unreadable(source, name, parts));
        };
        if std::os::unix::fs::MetadataExt::size(&metadata) > self.profile.max_file_bytes {
            return Err(Self::reject(SnapshotRejection::FileTooLarge, parts));
        }
        let content = source
            .read_regular(name, self.profile.max_file_bytes)
            .map_err(|_| self.classify_unreadable(source, name, parts))?;
        self.bytes += content.len() as u64;
        if self.manifest.len() as u64 + 1 > self.profile.max_files {
            return Err(RunError::Snapshot(SnapshotRejection::FileCountCap));
        }
        if self.bytes > self.profile.max_total_bytes {
            return Err(RunError::Snapshot(SnapshotRejection::TotalBytesCap));
        }
        target
            .create_snapshot_file(name, &content)
            .map_err(|_| RunError::StagingIo)?;
        self.manifest.insert(path, ManifestEntry::of(&content));
        Ok(())
    }

    fn classify_unreadable(&self, source: &DirHandle, name: &str, parts: &[String]) -> RunError {
        use std::os::unix::fs::MetadataExt;
        match source.kind(name) {
            Ok(EntryKind::Symlink) => Self::reject(SnapshotRejection::Symlink, parts),
            Ok(EntryKind::Special) => Self::reject(SnapshotRejection::SpecialFile, parts),
            Ok(EntryKind::Regular) => match source.lstat(name) {
                Some(metadata) if metadata.nlink() != 1 => {
                    Self::reject(SnapshotRejection::HardLink, parts)
                }
                Some(metadata) if metadata.dev() != self.root_dev => {
                    Self::reject(SnapshotRejection::OtherDevice, parts)
                }
                _ => Self::reject(SnapshotRejection::Io, parts),
            },
            _ => Self::reject(SnapshotRejection::Io, parts),
        }
    }
}

/// Open a staging subdirectory, creating it if missing. Anything else at the
/// name (a symlink, a file) is a redirect.
fn ensure_dir(parent: &DirHandle, name: &str) -> Result<DirHandle, RunError> {
    match parent.kind(name).map_err(|_| RunError::StagingIo)? {
        EntryKind::Directory => {}
        EntryKind::Missing => parent.make_subdir(name).map_err(|_| RunError::StagingIo)?,
        _ => return Err(RunError::StagingRedirect),
    }
    parent
        .open_subdir(name)
        .map_err(|_| RunError::StagingRedirect)
}
