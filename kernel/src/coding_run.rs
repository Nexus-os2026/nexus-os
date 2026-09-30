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
//! checkpoint issues no `UserSelected` grant, exposes no IPC, runs no process,
//! makes no network or model call and never writes to the project.
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
mod manifest;
mod scope;
mod structural;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use thiserror::Error;
use uuid::Uuid;

use crate::manifest::FsPermissionLevel;
use crate::workspace_authority::{
    WorkspaceAuthorityError, WorkspaceAuthorityRegistry, WorkspaceAuthoritySource,
    WorkspaceBinding, WorkspaceGrant, WorkspaceGrantId,
};
use fsops::{DirHandle, EntryKind};

pub use ledger::{analyze as analyze_ledger, LedgerFailure, LedgerRecovery, LedgerStore};
pub use manifest::{Manifest, ManifestEntry, ManifestHash};
pub use scope::{RelPath, RunScopes, ScopeEntry, ScopeError, ScopeSet};
pub use structural::{
    StructuralOutcome, StructuralProfile, StructuralVerification, StructuralViolation,
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
    /// `<identity home>/.nexus/coding-runs`, created owner-only if missing.
    pub fn from_identity_home() -> Result<Self, RunError> {
        let path = crate::identity_home::nexus_state_path("coding-runs")
            .map_err(|_| RunError::StagingUnavailable)?;
        std::os::unix::fs::DirBuilderExt::mode(&mut std::fs::DirBuilder::new(), 0o700)
            .recursive(true)
            .create(&path)
            .map_err(|_| RunError::StagingUnavailable)?;
        Self::open(path)
    }

    fn open(path: PathBuf) -> Result<Self, RunError> {
        let handle = DirHandle::open_absolute(&path).map_err(|_| RunError::StagingUnavailable)?;
        Ok(Self { handle, path })
    }

    #[cfg(test)]
    pub(crate) fn for_test(path: PathBuf) -> Self {
        Self::open(path).expect("test staging parent")
    }
}

/// A run's staging area: its own grant, retained identity and parent.
#[derive(Debug)]
struct Staging {
    grant: WorkspaceGrantId,
    handle: DirHandle,
    parent: DirHandle,
    name: String,
    #[cfg(test)]
    path: PathBuf,
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
    staging: Option<Staging>,
    base: Option<Manifest>,
    generation_valid: bool,
    next_op: u64,
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
        };
        let facts = json!({
            "project_grant": project_grant,
            "agent": binding.agent_id.to_string(),
            "binding_run": binding.run_id.to_string(),
            "read": run.scopes.read().describe(),
            "write": run.scopes.write().describe(),
            "protected": run.scopes.protected().describe(),
            "profile": hex::encode(run.profile.hash()),
        });
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
        let grant = self.resolve_grant(staging.grant)?;
        if *grant.permission() != FsPermissionLevel::ReadWrite {
            return Err(RunError::AuthorityDenied);
        }
        let fresh =
            DirHandle::open_absolute(grant.root()).map_err(|_| RunError::IdentityChanged)?;
        if fresh.identity() != staging.handle.identity() {
            return Err(RunError::IdentityChanged);
        }
        Ok(())
    }

    // ── Terminal transitions ───────────────────────────────────────────

    /// Enter a terminal state, recording it first. If the record fails the
    /// run needs recovery instead. The staging grant is revoked either way.
    fn enter_terminal(&mut self, state: RunState) {
        let (event, reason) = match state {
            RunState::Cancelled => (EventKind::RunCancelled, "cancelled".to_string()),
            RunState::Revoked(reason) => (EventKind::RunRevoked, format!("{reason:?}")),
            RunState::Failed(reason) => (EventKind::RunFailed, format!("{reason:?}")),
            RunState::RecoveryRequired(reason) => {
                (EventKind::RecoveryRequired, format!("{reason:?}"))
            }
            _ => return,
        };
        let recorded = record(
            self.ledger.as_ref(),
            self.id.0,
            event,
            &json!({ "reason": reason }),
        );
        self.state = if recorded.is_ok() {
            state
        } else {
            RunState::RecoveryRequired(RecoveryReason::TerminalNotRecorded)
        };
        if matches!(
            self.state,
            RunState::RecoveryRequired(_) | RunState::Failed(_)
        ) {
            self.generation_valid = false;
        }
        if let Some(staging) = &self.staging {
            let _ = self.registry.revoke(staging.grant, self.binding);
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
            self.generation_valid = false;
            self.state = RunState::RecoveryRequired(RecoveryReason::OutcomeNotRecorded);
            self.discard_quietly();
            if let Some(staging) = &self.staging {
                let _ = self.registry.revoke(staging.grant, self.binding);
            }
            return Err(RunError::RecoveryRequired(
                RecoveryReason::OutcomeNotRecorded,
            ));
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
        self.project = Some(project);
        let expires_at = grant.expires_at();
        let name = self.id.to_string();
        let registry = Arc::clone(&self.registry);
        let binding = self.binding;
        let allocated = self.mutate(
            "allocate_staging",
            json!({ "name": name }),
            |_| {
                parent
                    .handle
                    .make_subdir(&name)
                    .map_err(|_| RunError::StagingIo)?;
                let handle = parent
                    .handle
                    .open_subdir(&name)
                    .map_err(|_| RunError::StagingIo)?;
                let own_parent =
                    DirHandle::open_absolute(&parent.path).map_err(|_| RunError::StagingIo)?;
                if own_parent.identity() != parent.handle.identity() {
                    return Err(RunError::IdentityChanged);
                }
                let path = parent.path.join(&name);
                let staging_grant = registry
                    .issue_trusted_root(
                        &path,
                        binding,
                        WorkspaceAuthoritySource::BackendAllocated,
                        FsPermissionLevel::ReadWrite,
                        expires_at,
                    )
                    .map_err(|_| RunError::StagingIo)?;
                Ok(Staging {
                    grant: staging_grant,
                    handle,
                    parent: own_parent,
                    name: name.clone(),
                    #[cfg(test)]
                    path,
                })
            },
            |staging: &Staging| json!({ "staging_grant": staging.grant }),
        );
        match allocated {
            Ok(staging) => {
                self.staging = Some(staging);
                if let Err(error) = self.revalidate_staging() {
                    return Err(self.fail(error));
                }
                let facts = json!({ "staging_grant": self.staging.as_ref().map(|s| s.grant) });
                if let Err(error) = record(
                    self.ledger.as_ref(),
                    self.id.0,
                    EventKind::RunGranted,
                    &facts,
                ) {
                    return Err(RunError::Ledger(error));
                }
                self.state = RunState::Granted;
                Ok(())
            }
            Err(error) => Err(self.fail(error)),
        }
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
        let staging = &self
            .staging
            .as_ref()
            .ok_or(RunError::AuthorityDenied)?
            .handle;
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
        self.state = RunState::Candidate;
        Ok(())
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
            Err(error) => RunError::Ledger(error),
        }
    }

    /// Walk to the target's directory without creating anything.
    fn probe_target(&self, edit: &CandidateEdit) -> Result<(), RunError> {
        let staging = &self
            .staging
            .as_ref()
            .ok_or(RunError::AuthorityDenied)?
            .handle;
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
        let staging = &self
            .staging
            .as_ref()
            .ok_or(RunError::AuthorityDenied)?
            .handle;
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
        let staging = &self
            .staging
            .as_ref()
            .ok_or(RunError::AuthorityDenied)?
            .handle;
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
            self.state = RunState::StructurallyVerified;
        } else {
            self.enter_terminal(RunState::Failed(FailureReason::StructuralRejected));
        }
        Ok(result)
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
        Ok(())
    }

    pub fn revoke(&mut self) -> Result<(), RunError> {
        if self.state.is_terminal() {
            return Err(RunError::InvalidState {
                operation: "revoke",
                state: self.state,
            });
        }
        self.enter_terminal(RunState::Revoked(RevocationReason::Explicit));
        Ok(())
    }

    /// Remove the disposable staging directory. Allowed once the run is
    /// terminal; the run outcome is unchanged.
    pub fn discard_staging(&mut self) -> Result<CleanupStatus, RunError> {
        if !self.state.is_terminal() {
            return Err(RunError::InvalidState {
                operation: "discard_staging",
                state: self.state,
            });
        }
        self.discard_quietly();
        let facts = json!({ "status": format!("{:?}", self.cleanup) });
        let _ = record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::StagingDiscarded,
            &facts,
        );
        Ok(self.cleanup)
    }

    fn discard_quietly(&mut self) {
        let Some(staging) = &self.staging else {
            return;
        };
        let removed = staging
            .handle
            .remove_all_entries()
            .and_then(|()| staging.parent.remove_subdir(&staging.name));
        self.cleanup = if removed.is_ok() {
            CleanupStatus::Discarded
        } else {
            CleanupStatus::DiscardFailed
        };
    }

    #[cfg(test)]
    pub(crate) fn staging_path_for_test(&self) -> Option<PathBuf> {
        self.staging.as_ref().map(|staging| staging.path.clone())
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
