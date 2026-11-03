//! Owner approval, apply and single-run restore (Phase One, P1-06/P1-07).
//!
//! Approval (decision D2) comes only from a backend-invoked native
//! confirmation ([`OwnerConfirmer`]) that shows backend-computed facts about
//! the exact review binding. It yields an [`OwnerApproval`] that nothing else
//! can construct, deserialize or copy; a frontend or model claim of approval
//! is only a string and grants nothing.
//!
//! Apply is a separate authority transition over a fresh, short-lived write
//! grant on the owner-selected project. Before the first write it checks, for
//! the whole candidate: the approval binds this run, base and candidate; the
//! grant is live, writable, for this run and this project, and the project
//! root is still the selected directory; the staged candidate still hashes
//! to the verified candidate; every replaced file still has exactly its base
//! bytes and is a plain regular file; every created file is still absent; no
//! parent is a symlink or other redirect; every path is still editable. Any
//! mismatch rejects the apply with the project unchanged.
//!
//! Pre-images of replaced files are then stored in backend-private run
//! storage and recorded. Files are written through retained directory
//! descriptors: a replacement is written to an exclusive temporary file,
//! fsynced, and atomically exchanged with the target (`RENAME_EXCHANGE`); the
//! displaced file must be the very file checked in preflight, otherwise the
//! exchange is undone. A creation is published with one
//! `renameat2(RENAME_NOREPLACE)`, which never replaces (M2d). If any step
//! fails, already-applied files are restored in reverse order and the
//! outcome is reported as rolled back or as requiring recovery, never as
//! applied. Multi-file apply is not atomic; the ledger
//! and the filesystem are separate transactional domains.
//!
//! Restore ("Restore this coding run") undoes exactly one successful apply
//! of this run, after its own native confirmation and over a fresh write
//! grant. Before anything is touched, the project root must still be the
//! selected directory, every applied file must still be reachable at its
//! path through the same directories and still hold exactly the bytes Nexus
//! wrote (a created file must still be the very file Nexus created), and
//! every pre-image must be intact in the private store; otherwise nothing is
//! touched. It is not Time Machine and uses no Time Machine authority.

use serde_json::json;
use thiserror::Error;

use std::collections::BTreeMap;

use serde_json::Value;

use super::fsops::{plan_temp_name, DirHandle, EntryKind, NodeIdentity};
use super::ledger::{self, record, EventKind};
use super::manifest::{Manifest, ManifestEntry};
use super::review::{display_safe, ChangeKind, FileChange, Review, ReviewBinding};
use super::scope::{RelPath, TEMP_PREFIX};
use super::structural::StructuralProfile;
use super::{CodingRun, ProjectGrant, RunError, RunId, RunState, StagingParent};
use crate::emergency_stop::StopWatch;
use crate::manifest::FsPermissionLevel;
use crate::workspace_authority::WorkspaceAuthorityError;

const MAX_DISPLAY_PATHS: usize = 12;
const MAX_DISPLAY_NAME: usize = 64;
const CREATED_FILE_MODE: u32 = 0o644;
const CREATED_DIR_MODE: u32 = 0o755;
/// Largest file read back while writing or undoing (the profile's cap).
const MAX_FILE: u64 = StructuralProfile::V1.max_file_bytes;

/// The backend's native owner confirmation. Only the desktop's native
/// dialog adapter implements it.
pub trait OwnerConfirmer {
    /// Show the request natively; `true` only if the owner confirmed.
    fn confirm(&self, request: &ConfirmationRequest) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationKind {
    Apply,
    Restore,
}

/// Backend-computed facts shown to the owner before approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmationRequest {
    pub kind: ConfirmationKind,
    pub project_name: String,
    pub run_id: RunId,
    pub creates: usize,
    pub replaces: usize,
    pub deletes: usize,
    /// The identity of the review being approved, as the app shows it beside
    /// the reviewed changes ([`ReviewBinding::short`]).
    pub review_short: String,
    pub candidate_short: String,
    pub base_short: String,
    pub paths: Vec<String>,
    pub more_paths: usize,
    /// The full [`ReviewBinding::hash`] the request is about (M2d).
    pub review_hash: [u8; 32],
    /// The exact byte count of the one file the request is about, if it
    /// is about exactly one file.
    pub bytes: Option<u64>,
    /// The full SHA-256 (64 hex) of that one file's content.
    pub sha256_full: Option<String>,
    /// For a restore of one created file: the identity of the file that
    /// will be removed.
    pub identity: Option<FileIdentity>,
}

/// A file's device and inode, shown to the owner. It is a fact, never
/// authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIdentity {
    pub dev: u64,
    pub ino: u64,
}

/// What can interrupt an observed apply before its publication (M2d, I8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptCause {
    EmergencyStop,
    Cancelled,
    Revoked,
    Expired,
    CommitmentExpired,
}

/// The caller's view of the authority an observed apply runs under (M2d,
/// I8). The broker implements it over its execution guard.
pub trait ApplyInterrupt {
    /// Asked once, at the final check before publication: `Some` refuses
    /// the publication.
    fn interrupted(&self) -> Option<InterruptCause>;
    /// Called once the final check has passed, just before publication. A
    /// no-op in production; a test seam.
    fn passed_final_check(&self) {}
}

/// Why owned temporary files had to be removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupCause {
    /// The final check refused the publication.
    Interrupted(InterruptCause),
    /// The temporary file could not be written completely.
    TempWriteFailed,
    /// The publishing rename failed.
    PublicationFailed,
    /// The publication could not be recorded, so it was undone.
    Unrecorded,
    /// The owner's undo moved the created file aside.
    Undo,
}

impl ConfirmationRequest {
    pub fn files(&self) -> usize {
        self.creates + self.replaces + self.deletes
    }

    pub fn title(&self) -> &'static str {
        match self.kind {
            ConfirmationKind::Apply => "Apply changes to your project?",
            ConfirmationKind::Restore => "Restore this coding run?",
        }
    }

    /// The text of the native confirmation.
    pub fn message(&self) -> String {
        let action = match self.kind {
            ConfirmationKind::Apply => "Nexus will write these changes into",
            ConfirmationKind::Restore => "Nexus will undo this run's changes in",
        };
        let mut text = format!(
            "{action} the project \"{}\".\n\nRun: {}\nFiles: {} ({} created, {} replaced, {} deleted)\n\
             Review: {}\nCandidate: {}\nBase: {}\n",
            self.project_name,
            self.run_id,
            self.files(),
            self.creates,
            self.replaces,
            self.deletes,
            self.review_short,
            self.candidate_short,
            self.base_short
        );
        for path in &self.paths {
            text.push_str(&format!("\n  {path}"));
        }
        if self.more_paths > 0 {
            text.push_str(&format!("\n  … and {} more", self.more_paths));
        }
        text
    }

    fn new(kind: ConfirmationKind, project_name: &str, binding: &ReviewBinding) -> Self {
        let name: String = display_safe(project_name, false)
            .chars()
            .take(MAX_DISPLAY_NAME)
            .collect();
        Self {
            kind,
            project_name: name,
            run_id: binding.run_id,
            creates: 0,
            replaces: 0,
            deletes: 0,
            review_short: binding.short(),
            candidate_short: binding.candidate_manifest_hash.to_hex()[..12].to_string(),
            base_short: binding.base_manifest_hash.to_hex()[..12].to_string(),
            paths: Vec::new(),
            more_paths: 0,
            review_hash: binding.hash(),
            bytes: None,
            sha256_full: None,
            identity: None,
        }
    }

    fn with_paths<'a>(mut self, paths: impl Iterator<Item = (&'a RelPath, ChangeKind)>) -> Self {
        for (path, kind) in paths {
            match kind {
                ChangeKind::Create => self.creates += 1,
                ChangeKind::Replace => self.replaces += 1,
                ChangeKind::Delete => self.deletes += 1,
            }
            if self.paths.len() < MAX_DISPLAY_PATHS {
                let verb = match kind {
                    ChangeKind::Create => "create",
                    ChangeKind::Replace => "replace",
                    ChangeKind::Delete => "delete",
                };
                self.paths
                    .push(format!("{verb} {}", display_safe(&path.as_string(), false)));
            } else {
                self.more_paths += 1;
            }
        }
        self
    }
}

/// The owner's native approval of exactly one review binding. It cannot be
/// constructed, deserialized or cloned outside this module, and apply
/// consumes it.
#[derive(Debug)]
pub struct OwnerApproval {
    kind: ConfirmationKind,
    binding: ReviewBinding,
}

impl OwnerApproval {
    pub fn binding(&self) -> &ReviewBinding {
        &self.binding
    }

    pub fn kind(&self) -> ConfirmationKind {
        self.kind
    }
}

/// Where a run stands with respect to the owner's project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyState {
    NotApplied,
    Applied,
    /// An apply failed part-way and every applied file was restored.
    RolledBack,
    /// An apply or restore failed part-way and could not be fully undone.
    RecoveryRequired,
    Restored,
}

/// Why an apply or restore was refused before anything was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Declined,
    NotApproved,
    ApprovalMismatch,
    WrongProject,
    AuthorityDenied,
    IdentityChanged,
    CandidateChanged,
    /// A target no longer has the expected content or existence.
    Stale(String),
    /// A target or parent is a symlink or other non-regular entry.
    Redirect(String),
    OutsideScope(String),
    Unsupported(String),
    PreimageUnavailable,
    Unrecorded,
    /// A sandboxed verification is starting, running or finalizing.
    VerificationInProgress,
    /// A sandboxed verification's cleanup is unconfirmed.
    VerificationCleanupFailed,
    /// The owner's emergency stop ended the run or is in force (M0-FIX F5).
    EmergencyStopped,
    /// An interruption reached the operation before it touched anything
    /// (M2d: an undo's final check U1).
    Interrupted(InterruptCause),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ApplyError {
    #[error("operation not allowed in this state")]
    InvalidState,
    #[error("refused before any write: {0:?}")]
    Refused(Refusal),
    #[error("failed at {failed}; every applied change was undone")]
    RolledBack { failed: String },
    #[error("failed at {failed}; recovery required for {unrestored:?}")]
    RecoveryRequired {
        failed: String,
        unrestored: Vec<String>,
    },
    /// The filesystem work of `operation` ("restore" or "rollback")
    /// finished, but its durable completion record could not be written:
    /// the outcome is not established, so recovery is required and the
    /// private pre-image store is kept.
    #[error(
        "{operation} finished on disk but its completion could not be recorded; recovery required"
    )]
    CompletionUnrecorded { operation: &'static str },
    /// The temporary write grant could not be confirmed closed after the
    /// attempt. `after` is what the attempt itself did to the project.
    #[error("the temporary write grant could not be confirmed closed; recovery required")]
    AuthorityNotClosed {
        after: Box<Result<ApplyReport, ApplyError>>,
    },
    /// The final check refused the publication: nothing was published and
    /// the temporary file's removal was established (M2d).
    #[error("interrupted before publication: {cause:?}; nothing was published")]
    Interrupted { cause: InterruptCause },
    /// Temporary files the run created could not be confirmed removed; the
    /// run keeps owning them (backend-random temp ids) until
    /// [`CodingRun::retry_cleanup`] establishes their removal (M2d).
    #[error("cleanup owed after {cause:?}; recovery required")]
    CleanupOwed {
        cause: CleanupCause,
        temps: Vec<String>,
    },
}

/// What a successful apply or restore wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyReport {
    pub binding: ReviewBinding,
    pub files: Vec<(RelPath, ChangeKind)>,
}

/// A pre-image kept for one replaced file.
#[derive(Debug)]
struct Preimage {
    slot: String,
    entry: ManifestEntry,
    identity: NodeIdentity,
    mode: u32,
}

/// One file written by an apply.
#[derive(Debug)]
pub(crate) struct AppliedFile {
    path: RelPath,
    kind: ChangeKind,
    dir: DirHandle,
    name: String,
    candidate: ManifestEntry,
    installed: NodeIdentity,
    preimage: Option<Preimage>,
    /// The temporary name planned and recorded for this file before it was
    /// written (M6′); free again once the file is in place.
    temp: String,
}

/// A directory created by an apply.
#[derive(Debug)]
struct CreatedDir {
    parent: DirHandle,
    name: String,
    identity: NodeIdentity,
}

/// The record of one successful apply: the only restore authority.
#[derive(Debug)]
pub(crate) struct ApplyRecord {
    binding: ReviewBinding,
    files: Vec<AppliedFile>,
    dirs: Vec<CreatedDir>,
    storage: DirHandle,
    storage_parent: DirHandle,
    storage_name: String,
    /// M2d: the publication's directory sync failed; the file is in place
    /// and its undo stays available while the run requires recovery.
    unsynced: bool,
}

/// One planned write, checked in preflight.
struct Planned {
    path: RelPath,
    kind: ChangeKind,
    /// The deepest existing directory, retained from preflight.
    dir: DirHandle,
    /// Parent components to create below `dir` (creations only).
    missing: Vec<String>,
    name: String,
    content: Vec<u8>,
    candidate: ManifestEntry,
    base: Option<ManifestEntry>,
    pre: Option<(Vec<u8>, NodeIdentity, u32)>,
    /// The backend-random temporary name, recorded before creation (M6′).
    temp: String,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookPoint {
    /// Before the first write for a file (return `true` to inject failure).
    BeforeWrite,
    /// After the replacement is written, before the exchange.
    BeforeExchange,
    /// M2d: the temporary file is written and synced; the final check is
    /// next.
    BeforeFinalCheck,
    /// M2d: the final check passed; the publishing rename is next.
    AfterFinalCheck,
    /// M2d: an undo is prepared and recorded; its final check U1 is next.
    BeforeUnpublish,
}

// M2d test seams: the removal of an owned temporary file, the publishing
// rename, and directory syncs of the publication protocol fail.
#[cfg(test)]
thread_local! {
    pub(crate) static REMOVE_TEMP_FAILS: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    pub(crate) static RENAME_FAILS: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    pub(crate) static SYNC_FAILS: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    /// The workspace cannot hold an anonymous (`O_TMPFILE`) file.
    pub(crate) static TMPFILE_UNSUPPORTED: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

#[cfg(test)]
thread_local! {
    #[allow(clippy::type_complexity)]
    pub(crate) static APPLY_HOOK: std::cell::RefCell<Option<Box<dyn FnMut(HookPoint, &str) -> bool>>> =
        const { std::cell::RefCell::new(None) };
}

fn hook(_point: &str, _path: &RelPath) -> bool {
    #[cfg(test)]
    {
        let point = match _point {
            "write" => HookPoint::BeforeWrite,
            "final" => HookPoint::BeforeFinalCheck,
            "handoff" => HookPoint::AfterFinalCheck,
            "unpublish" => HookPoint::BeforeUnpublish,
            _ => HookPoint::BeforeExchange,
        };
        let path = _path.as_string();
        return APPLY_HOOK.with(|h| h.borrow_mut().as_mut().is_some_and(|f| f(point, &path)));
    }
    #[allow(unreachable_code)]
    false
}

// Test hook right after a replacement's exchange (the displaced file is at
// `temp`, the new file at `name`).
#[cfg(test)]
thread_local! {
    #[allow(clippy::type_complexity)]
    pub(crate) static AFTER_EXCHANGE: std::cell::RefCell<Option<Box<dyn FnMut(&std::path::Path, &str, &str)>>> =
        const { std::cell::RefCell::new(None) };
}

fn hook_after_exchange(_dir: &DirHandle, _temp: &str, _name: &str, _path: &RelPath) {
    #[cfg(test)]
    AFTER_EXCHANGE.with(|h| {
        if let Some(f) = h.borrow_mut().as_mut() {
            f(&_dir.anchor_for_test(), _temp, _name);
        }
    });
}

// Test seam: the temporary write grant cannot be revoked.
#[cfg(test)]
thread_local! {
    pub(crate) static WRITE_GRANT_REVOKE_FAILS: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

/// Close the temporary write grant of an apply or restore attempt,
/// independently of the ledger. Closure is confirmed only if the registry
/// revoked it, or it no longer resolves (already revoked or expired).
fn close_write_grant(grant: &ProjectGrant) -> bool {
    #[cfg(test)]
    if WRITE_GRANT_REVOKE_FAILS.with(std::cell::Cell::get) {
        return false;
    }
    grant.authority.revoke(grant.grant, grant.binding).is_ok()
        || grant.authority.resolve(grant.grant, grant.binding).is_err()
}

fn refused(refusal: Refusal) -> ApplyError {
    ApplyError::Refused(refusal)
}

// ── M2d: one publication, cleanup ownership, the final check ───────────────

#[derive(Clone, Copy)]
enum Seam {
    Remove,
    Rename,
    Sync,
    Tmpfile,
}

/// Whether a test seam injects this failure (never in production).
fn seam(_seam: Seam) -> bool {
    #[cfg(test)]
    {
        let cell = match _seam {
            Seam::Remove => &REMOVE_TEMP_FAILS,
            Seam::Rename => &RENAME_FAILS,
            Seam::Sync => &SYNC_FAILS,
            Seam::Tmpfile => &TMPFILE_UNSUPPORTED,
        };
        return cell.try_with(std::cell::Cell::get).unwrap_or(false);
    }
    #[allow(unreachable_code)]
    false
}

/// Why an owned temporary file's removal is not established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Removal {
    RemoveFailed,
    NotOurs,
    StillPresent,
    SyncFailed,
}

impl Removal {
    fn code(self) -> &'static str {
        match self {
            Self::RemoveFailed => "remove_failed",
            Self::NotOurs => "not_ours",
            Self::StillPresent => "still_present",
            Self::SyncFailed => "sync_failed",
        }
    }
}

/// Every [`Removal`] code (the ledger's `removal` registry).
pub(crate) const REMOVALS: &[&str] = &["remove_failed", "not_ours", "still_present", "sync_failed"];

/// A temporary file the run created in the owner's folder. It stays owned
/// until its removal is established: removed (only if it is still the very
/// file created), `lstat` shows the name absent, and the directory is
/// synced. `Drop` tries once more, as defense in depth only.
#[derive(Debug)]
pub(crate) struct OwnedTemp {
    dir: DirHandle,
    name: String,
    identity: Option<NodeIdentity>,
    released: bool,
}

impl OwnedTemp {
    /// The backend-random id of its name (what the ledger records).
    fn id(&self) -> String {
        temp_id(&self.name).to_string()
    }

    /// One attempt to establish the removal; once established, a no-op.
    fn release(&mut self) -> Result<(), Removal> {
        if self.released {
            return Ok(());
        }
        match self.dir.entry_metadata(&self.name) {
            Ok(None) => {}
            Ok(Some(found)) => {
                let ours =
                    found.file_type().is_file() && self.identity == Some(NodeIdentity::of(&found));
                if !ours {
                    return Err(Removal::NotOurs);
                }
                if seam(Seam::Remove) || self.dir.remove_temp(&self.name).is_err() {
                    return Err(Removal::RemoveFailed);
                }
            }
            Err(_) => return Err(Removal::StillPresent),
        }
        if !matches!(self.dir.entry_metadata(&self.name), Ok(None)) {
            return Err(Removal::StillPresent);
        }
        if seam(Seam::Sync) || self.dir.sync().is_err() {
            return Err(Removal::SyncFailed);
        }
        self.released = true;
        Ok(())
    }
}

impl Drop for OwnedTemp {
    fn drop(&mut self) {
        if !self.released {
            let _ = self.release();
        }
    }
}

/// The private pre-image store of one apply.
#[derive(Debug)]
struct Store {
    parent: DirHandle,
    name: String,
    dir: DirHandle,
}

impl Store {
    fn discard(self) {
        discard_storage(&self.parent, &self.name, &self.dir);
    }
}

/// What a run becomes once its owed removals are established.
#[derive(Debug, Clone, Copy)]
enum Settled {
    RolledBack,
    Restored {
        binding: ReviewBinding,
        files: usize,
    },
    /// Recovery stays required for other reasons.
    Recovery,
}

/// Temporary files whose removal is not established, kept by the run until
/// [`CodingRun::retry_cleanup`] establishes it.
#[derive(Debug)]
pub(crate) struct Owed {
    temps: Vec<OwnedTemp>,
    cause: CleanupCause,
    then: Settled,
    store: Option<Store>,
}

/// The stop watches an undo took for both authorities when it started,
/// before its dialog (H2′). Checked when it runs and at U1; never
/// refreshed, so a stop during the undo needs a new undo.
#[derive(Debug)]
pub(crate) struct UndoWatches {
    run: StopWatch,
    observed: Option<StopWatch>,
}

impl UndoWatches {
    pub(crate) fn new(run: StopWatch, observed: Option<StopWatch>) -> Self {
        Self { run, observed }
    }
}

/// Whether a stop was engaged since either watch was admitted, or is in
/// force.
fn stopped(run: &StopWatch, observed: Option<&StopWatch>) -> bool {
    std::iter::once(run)
        .chain(observed)
        .any(|watch| watch.halted() || watch.authority().is_stopped())
}

/// The final check (§6.3 step 3; U1 for an undo): the caller's guard (which
/// may hold, bounded, before it answers), then the kernel stop on both
/// authorities, then the write grant, whose revocation or expiry (reached
/// exactly counts) refuses. Only memory is read: no lock is held and no
/// filesystem work is done.
fn final_check(
    grant: &ProjectGrant,
    guard: Option<&dyn ApplyInterrupt>,
    run: &StopWatch,
    observed: Option<&StopWatch>,
) -> Option<InterruptCause> {
    if let Some(cause) = guard.and_then(|guard| guard.interrupted()) {
        return Some(cause);
    }
    if stopped(run, observed) {
        return Some(InterruptCause::EmergencyStop);
    }
    match grant.authority.resolve(grant.grant, grant.binding) {
        Ok(_) => None,
        Err(WorkspaceAuthorityError::ExpiredGrant) => Some(InterruptCause::Expired),
        Err(_) => Some(InterruptCause::Revoked),
    }
}

/// The id part of a temporary name.
fn temp_id(name: &str) -> &str {
    name.strip_prefix(TEMP_PREFIX).unwrap_or(name)
}

/// The candidate as an anonymous file of the workspace directory, held by
/// this descriptor alone until it is published. Dropped unpublished, it is
/// freed by the kernel: nothing is left behind and nothing is owed.
struct Anonymous {
    file: std::fs::File,
    identity: NodeIdentity,
}

impl Anonymous {
    /// Whether it is still the unlinked file created, holding exactly the
    /// approved bytes.
    fn holds(&self, approved: &ManifestEntry) -> bool {
        use std::os::unix::fs::{FileExt as _, MetadataExt as _};
        let Ok(found) = self.file.metadata() else {
            return false;
        };
        if NodeIdentity::of(&found) != self.identity
            || found.nlink() != 0
            || found.len() != approved.size
            || approved.size > MAX_FILE
        {
            return false;
        }
        let mut bytes = vec![0; approved.size as usize];
        self.file.read_exact_at(&mut bytes, 0).is_ok() && ManifestEntry::of(&bytes) == *approved
    }
}

/// Create the candidate as an anonymous file of `dir`, write and sync it.
/// `None` if the filesystem cannot hold anonymous files or a step fails:
/// Files is then refused, fail closed, never done through a named file.
fn write_candidate(dir: &DirHandle, content: &[u8]) -> Option<Anonymous> {
    use std::io::Write as _;
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    if seam(Seam::Tmpfile) {
        return None;
    }
    let mut file = dir.create_anonymous().ok()?;
    let found = file.metadata().ok()?;
    if found.dev() != dir.identity().dev || found.nlink() != 0 {
        return None;
    }
    let identity = NodeIdentity::of(&found);
    let permissions = std::fs::Permissions::from_mode(CREATED_FILE_MODE);
    let written = file.write_all(content).is_ok()
        && file.set_permissions(permissions).is_ok()
        && file.sync_all().is_ok();
    written.then_some(Anonymous { file, identity })
}

/// What moving a created file aside, to its recorded temporary name, and
/// removing it gave.
enum Unpublish {
    /// Removed, and the removal established.
    Removed,
    /// Not the file Nexus created: renamed back, nothing else done.
    NotOurs,
    /// Not ours, and it could not be renamed back: it is at the temp name.
    Stuck,
    /// It could not be moved: nothing was done.
    NotMoved,
    /// Ours and moved aside, but its removal is not established.
    Owed(OwnedTemp, Removal),
}

fn unpublish(file: &AppliedFile, temp: &str) -> Unpublish {
    let Ok(handle) = file.dir.try_clone() else {
        return Unpublish::NotMoved;
    };
    if file.dir.rename_noreplace(&file.name, temp).is_err() {
        return Unpublish::NotMoved;
    }
    let moved = file.dir.read_with_identity(temp, MAX_FILE);
    let ours = matches!(&moved, Ok((content, identity, _))
        if *identity == file.installed && ManifestEntry::of(content) == file.candidate);
    if !ours {
        // Someone else's file: put it back (compensating, never cancelled).
        return if file.dir.rename_noreplace(temp, &file.name).is_ok() {
            Unpublish::NotOurs
        } else {
            Unpublish::Stuck
        };
    }
    let mut owned = OwnedTemp {
        dir: handle,
        name: temp.to_string(),
        identity: Some(file.installed),
        released: false,
    };
    match owned.release() {
        Ok(()) => Unpublish::Removed,
        Err(removal) => Unpublish::Owed(owned, removal),
    }
}

/// The `apply.prepared` facts: every file by digest with its planned temp
/// id, recorded before anything is created.
fn prepared_facts(binding: &ReviewBinding, preimages: Vec<Value>, plan: &[Planned]) -> Value {
    json!({
        "binding": hex::encode(binding.hash()),
        "preimages": preimages,
        "files": plan.iter().map(|p| {
            let path = p.path.as_string();
            json!({
                "path_sha256": ledger::path_sha256(&path),
                "path_bytes": path.len(),
                "kind": p.kind.code(),
                "base": p.base.as_ref().map(|e| hex::encode(e.sha256)),
                "candidate": hex::encode(p.candidate.sha256),
                "temp_id": temp_id(&p.temp),
            })
        }).collect::<Vec<_>>(),
    })
}

impl CodingRun {
    /// Verification is advisory, but Apply waits for an active execution to
    /// be finalized and for an unconfirmed cleanup to be confirmed.
    fn verification_permits_apply(&self) -> Result<(), ApplyError> {
        use super::verifier::VerificationPhase;
        match self.verification_phase() {
            VerificationPhase::CleanupFailed => Err(refused(Refusal::VerificationCleanupFailed)),
            phase if phase.blocks_apply() => Err(refused(Refusal::VerificationInProgress)),
            _ => Ok(()),
        }
    }

    /// Where this run stands with respect to the owner's project.
    pub fn apply_state(&self) -> ApplyState {
        self.apply_state
    }

    /// The files this run's successful apply wrote.
    pub fn applied_files(&self) -> Vec<(RelPath, ChangeKind)> {
        self.applied
            .iter()
            .flat_map(|record| record.files.iter())
            .map(|file| (file.path.clone(), file.kind))
            .collect()
    }

    /// Ask the owner, through the backend's native confirmation, to approve
    /// the exact reviewed candidate. The review is recomputed (so the staged
    /// candidate must still be the verified one) and the decision recorded.
    pub fn request_approval(
        &mut self,
        project_name: &str,
        confirmer: &dyn OwnerConfirmer,
    ) -> Result<OwnerApproval, ApplyError> {
        if self.state != RunState::StructurallyVerified
            || self.apply_state != ApplyState::NotApplied
        {
            return Err(ApplyError::InvalidState);
        }
        self.verification_permits_apply()?;
        let review = self.review().map_err(|error| match error {
            RunError::CandidateChanged | RunError::CandidateUnavailable => {
                refused(Refusal::CandidateChanged)
            }
            RunError::Ledger(_) => refused(Refusal::Unrecorded),
            _ => ApplyError::InvalidState,
        })?;
        let request = request_for(ConfirmationKind::Apply, project_name, &review);
        let confirmed = confirmer.confirm(&request);
        let (event, result) = if confirmed {
            (EventKind::ApprovalGranted, Ok(()))
        } else {
            (EventKind::ApprovalDeclined, Err(refused(Refusal::Declined)))
        };
        record(
            self.ledger.as_ref(),
            self.id.0,
            event,
            &json!({ "binding": hex::encode(review.binding.hash()), "files": request.files() }),
        )
        .map_err(|_| refused(Refusal::Unrecorded))?;
        result?;
        self.approved = Some(review.binding);
        Ok(OwnerApproval {
            kind: ConfirmationKind::Apply,
            binding: review.binding,
        })
    }

    /// Apply the approved candidate to the owner's project.
    pub fn apply(
        &mut self,
        approval: OwnerApproval,
        grant: ProjectGrant,
        storage: &StagingParent,
    ) -> Result<ApplyReport, ApplyError> {
        // M0-FIX F5: refused, before any write, by the emergency stop.
        let result = if self.halt_if_stopped().is_err() {
            self.approved = None;
            Err(refused(Refusal::EmergencyStopped))
        } else if approval.kind == ConfirmationKind::Apply {
            self.apply_inner(approval.binding, &grant, storage)
        } else {
            self.approved = None;
            Err(refused(Refusal::ApprovalMismatch))
        };
        self.end_attempt(&grant, result)
    }

    /// Apply a candidate that is exactly one new root-level file, through
    /// one publication right after a final check of every authority (M2d
    /// §6.3, Codex D2): the candidate is written and synced first, as an
    /// anonymous `O_TMPFILE` file of the workspace (no pathname exists to
    /// substitute), and checked to hold the approved bytes; then `guard`,
    /// the kernel stop on both authorities and the write grant are checked;
    /// then one `linkat` publishes exactly that inode, never replacing an
    /// existing entry (D1), and the name is verified to hold it. A
    /// filesystem without anonymous files refuses (fail closed). An interruption before that check publishes
    /// nothing; one after it lets only this rename complete, with no retry,
    /// no other destination and no second file. Nothing waits for it.
    pub fn apply_observed(
        &mut self,
        approval: OwnerApproval,
        grant: ProjectGrant,
        storage: &StagingParent,
        guard: &dyn ApplyInterrupt,
    ) -> Result<ApplyReport, ApplyError> {
        let result = if self.halt_if_stopped().is_err() {
            self.approved = None;
            Err(refused(Refusal::EmergencyStopped))
        } else if approval.kind == ConfirmationKind::Apply {
            self.publish_one(approval.binding, &grant, storage, guard)
        } else {
            self.approved = None;
            Err(refused(Refusal::ApprovalMismatch))
        };
        self.end_attempt(&grant, result)
    }

    /// The write grant never outlives the attempt, and no outcome is clean
    /// unless its closure is confirmed; a refusal is recorded (informational:
    /// nothing was written and no state changed).
    fn end_attempt(
        &mut self,
        grant: &ProjectGrant,
        result: Result<ApplyReport, ApplyError>,
    ) -> Result<ApplyReport, ApplyError> {
        if !close_write_grant(grant) {
            return Err(self.authority_not_closed(result));
        }
        if let Err(ApplyError::Refused(refusal)) = &result {
            let _ = record(
                self.ledger.as_ref(),
                self.id.0,
                EventKind::ApplyRejected,
                &ledger::refusal_facts(refusal),
            );
        }
        result
    }

    /// The temporary files this run owes, by id (M2d).
    pub fn cleanup_owed(&self) -> Vec<String> {
        self.owed
            .iter()
            .flat_map(|owed| owed.temps.iter().map(OwnedTemp::id))
            .collect()
    }

    /// One bounded attempt, per owner command, to establish the removal of
    /// every temporary file this run owes (M2d §6.6). Once established, the
    /// outcome is recorded and the run becomes what the removal completes
    /// (rolled back, or restored for an undo); otherwise it stays owed.
    pub fn retry_cleanup(&mut self) -> Result<(), ApplyError> {
        let owed = self.owed.as_mut().ok_or(ApplyError::InvalidState)?;
        let mut established = true;
        for temp in &mut owed.temps {
            established &= temp.release().is_ok();
        }
        if !established {
            return Err(ApplyError::CleanupOwed {
                cause: owed.cause,
                temps: owed.temps.iter().map(OwnedTemp::id).collect(),
            });
        }
        let Some(mut owed) = self.owed.take() else {
            return Err(ApplyError::InvalidState);
        };
        let (event, facts, operation, state) = match owed.then {
            Settled::RolledBack => (
                EventKind::ApplyRolledBack,
                json!({ "failed": "cleanup" }),
                "rollback",
                ApplyState::RolledBack,
            ),
            Settled::Restored { binding, files } => (
                EventKind::RestoreCompleted,
                json!({ "binding": hex::encode(binding.hash()), "files": files }),
                "restore",
                ApplyState::Restored,
            ),
            Settled::Recovery => (
                EventKind::ApplyRecoveryRequired,
                json!({ "failed": "cleanup" }),
                "restore",
                ApplyState::RecoveryRequired,
            ),
        };
        if record(self.ledger.as_ref(), self.id.0, event, &facts).is_err() {
            return Err(self.completion_unrecorded(operation, &Failed::Step("cleanup")));
        }
        self.apply_state = state;
        if state != ApplyState::RecoveryRequired {
            if let Some(store) = owed.store.take() {
                store.discard();
            }
        }
        Ok(())
    }

    /// The write grant stayed live after an attempt: the run requires
    /// recovery whatever the attempt did, and the actual state is recorded
    /// once (best effort).
    fn authority_not_closed(&mut self, after: Result<ApplyReport, ApplyError>) -> ApplyError {
        self.apply_state = ApplyState::RecoveryRequired;
        let _ = record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::ApplyRecoveryRequired,
            &json!({ "failed": "write grant closure", "attempt_ok": after.is_ok() }),
        );
        ApplyError::AuthorityNotClosed {
            after: Box::new(after),
        }
    }

    /// The checks every apply starts with: the state, this run's single-use
    /// approval of the current candidate, the write grant and its root, and
    /// the verified candidate's bytes. Returns the root, the candidate, its
    /// changed files' bytes and the base.
    #[allow(clippy::type_complexity)]
    fn checked_candidate(
        &mut self,
        binding: ReviewBinding,
        grant: &ProjectGrant,
    ) -> Result<(DirHandle, Manifest, BTreeMap<RelPath, Vec<u8>>, Manifest), ApplyError> {
        if self.state != RunState::StructurallyVerified
            || self.apply_state != ApplyState::NotApplied
        {
            return Err(ApplyError::InvalidState);
        }
        // The approval is single-use and must be this run's approval of the
        // current candidate.
        let approved = self.approved.take();
        let verification = self
            .verification
            .clone()
            .ok_or(refused(Refusal::CandidateChanged))?;
        self.verification_permits_apply()?;
        let current = self
            .review_binding(&verification)
            .map_err(|_| refused(Refusal::CandidateChanged))?;
        if approved.is_none() {
            return Err(refused(Refusal::NotApproved));
        }
        if approved != Some(binding) || binding != current {
            return Err(refused(Refusal::ApprovalMismatch));
        }
        let root = self.project_root_for_write(grant)?;
        let (candidate, bytes) = match self.verified_candidate(&verification) {
            Ok(found) => found,
            Err(error) => {
                self.withdraw_verification(error);
                return Err(refused(Refusal::CandidateChanged));
            }
        };
        let base = self
            .base
            .clone()
            .ok_or(refused(Refusal::CandidateChanged))?;
        Ok((root, candidate, bytes, base))
    }

    fn apply_inner(
        &mut self,
        binding: ReviewBinding,
        grant: &ProjectGrant,
        storage: &StagingParent,
    ) -> Result<ApplyReport, ApplyError> {
        let (root, candidate, bytes, base) = self.checked_candidate(binding, grant)?;
        let mut plan = Vec::new();
        for (path, entry) in candidate.entries() {
            let before = base.get(path);
            if before == Some(entry) {
                continue;
            }
            if !self.scopes.editable(path) {
                return Err(refused(Refusal::OutsideScope(path.as_string())));
            }
            let content = bytes
                .get(path)
                .cloned()
                .ok_or(refused(Refusal::CandidateChanged))?;
            plan.push(self.preflight(&root, path, before.cloned(), entry.clone(), content)?);
        }
        if let Some(path) = base
            .entries()
            .keys()
            .find(|path| candidate.get(path).is_none())
        {
            return Err(refused(Refusal::Unsupported(path.as_string())));
        }

        // Pre-images, recorded before the first owner-project write. The
        // store's parent handle is duplicated now, so nothing after the first
        // write can fail for lack of it.
        let (storage_dir, storage_name, preimages) = self.store_preimages(storage, &plan)?;
        let Ok(storage_parent) = storage.handle.try_clone() else {
            discard_storage(&storage.handle, &storage_name, &storage_dir);
            return Err(refused(Refusal::PreimageUnavailable));
        };
        let prepared = prepared_facts(&binding, preimages, &plan);
        if record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::ApplyPrepared,
            &prepared,
        )
        .is_err()
        {
            discard_storage(&storage.handle, &storage_name, &storage_dir);
            return Err(refused(Refusal::Unrecorded));
        }

        // Writes.
        let mut applied: Vec<AppliedFile> = Vec::new();
        let mut dirs: Vec<CreatedDir> = Vec::new();
        let mut slots = preimages_slots(&plan);
        let mut failure: Option<String> = None;
        // (path, temporary name) of each owner version that stayed hidden.
        let mut hidden: Vec<(String, String)> = Vec::new();
        for planned in plan {
            let path = planned.path.clone();
            let slot = slots.remove(&path);
            let outcome = if hook("write", &path) {
                Err(WriteFailure::Failed)
            } else {
                write_one(planned, slot, &mut dirs, &mut applied)
            };
            if let Err(WriteFailure::OwnerVersionAt(temp)) = &outcome {
                hidden.push((path.as_string(), temp.clone()));
            }
            let text = path.as_string();
            let recorded = record(
                self.ledger.as_ref(),
                self.id.0,
                EventKind::ApplyOp,
                &json!({
                    "path_sha256": ledger::path_sha256(&text),
                    "path_bytes": text.len(),
                    "ok": outcome.is_ok(),
                }),
            )
            .is_ok();
            if outcome.is_err() || !recorded {
                failure = Some(path.as_string());
                break;
            }
        }
        let completed = failure.is_none()
            && record(
                self.ledger.as_ref(),
                self.id.0,
                EventKind::ApplyCompleted,
                &json!({ "binding": hex::encode(binding.hash()), "files": applied.len() }),
            )
            .is_ok();
        if !completed {
            let failed = failure.map_or(Failed::Step("apply.completed"), Failed::File);
            let unrestored = rollback(&mut applied, &mut dirs, &storage_dir);
            return Err(self.finish_failed(
                failed,
                unrestored,
                hidden,
                &storage.handle,
                storage_name,
                storage_dir,
            ));
        }
        let files = applied.iter().map(|f| (f.path.clone(), f.kind)).collect();
        self.applied = Some(ApplyRecord {
            binding,
            files: applied,
            dirs,
            storage: storage_dir,
            storage_parent,
            storage_name,
            unsynced: false,
        });
        self.apply_state = ApplyState::Applied;
        Ok(ApplyReport { binding, files })
    }

    /// M2d §6.3 for one new root-level file: preflight and `apply.prepared`
    /// (with the planned temp id), the owned temporary file, the final
    /// check, the handoff, one `rename_noreplace`, the directory sync, then
    /// the records.
    fn publish_one(
        &mut self,
        binding: ReviewBinding,
        grant: &ProjectGrant,
        storage: &StagingParent,
        guard: &dyn ApplyInterrupt,
    ) -> Result<ApplyReport, ApplyError> {
        let (root, candidate, bytes, base) = self.checked_candidate(binding, grant)?;
        if let Some(path) = base
            .entries()
            .keys()
            .find(|path| candidate.get(path).is_none())
        {
            return Err(refused(Refusal::Unsupported(path.as_string())));
        }
        let changed: Vec<(&RelPath, &ManifestEntry)> = candidate
            .entries()
            .iter()
            .filter(|(path, entry)| base.get(path) != Some(*entry))
            .collect();
        let (path, entry) = match changed.as_slice() {
            [(path, entry)] if base.get(path).is_none() && path.components().len() == 1 => {
                ((*path).clone(), (*entry).clone())
            }
            [] => return Err(refused(Refusal::Unsupported(String::new()))),
            [(path, _), ..] => return Err(refused(Refusal::Unsupported(path.as_string()))),
        };
        if !self.scopes.editable(&path) {
            return Err(refused(Refusal::OutsideScope(path.as_string())));
        }
        let content = bytes
            .get(&path)
            .cloned()
            .ok_or(refused(Refusal::CandidateChanged))?;
        let planned = self.preflight(&root, &path, None, entry, content)?;
        let plan = std::slice::from_ref(&planned);
        let (dir, name, preimages) = self.store_preimages(storage, plan)?;
        let Ok(parent) = storage.handle.try_clone() else {
            discard_storage(&storage.handle, &name, &dir);
            return Err(refused(Refusal::PreimageUnavailable));
        };
        let store = Store { parent, name, dir };
        let prepared = prepared_facts(&binding, preimages, plan);
        if record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::ApplyPrepared,
            &prepared,
        )
        .is_err()
        {
            store.discard();
            return Err(refused(Refusal::Unrecorded));
        }

        // The candidate, as an anonymous file of the workspace directory: no
        // pathname exists to substitute; it is complete and synced before the
        // handoff, and freed unless it is published.
        let Planned {
            dir,
            name,
            content,
            candidate,
            temp,
            ..
        } = planned;
        let written = if hook("write", &path) {
            None
        } else {
            write_candidate(&dir, &content)
        };
        let Some(anonymous) = written else {
            return Err(self.unpublished(CleanupCause::TempWriteFailed, store));
        };
        hook("final", &path);
        // It must still hold exactly the approved bytes.
        if !anonymous.holds(&candidate) {
            return Err(self.unpublished(CleanupCause::TempWriteFailed, store));
        }

        // FINAL CHECK, then the handoff into the one publication.
        if let Some(cause) = final_check(grant, Some(guard), &self.stop, self.observed.as_ref()) {
            return Err(self.unpublished(CleanupCause::Interrupted(cause), store));
        }
        guard.passed_final_check();
        hook("handoff", &path);

        // PUBLICATION: link exactly the inode the descriptor holds; an
        // existing entry is never replaced.
        if seam(Seam::Rename) || dir.link_anonymous(&anonymous.file, &name).is_err() {
            return Err(self.unpublished(CleanupCause::PublicationFailed, store));
        }
        let installed = anonymous.identity;
        drop(anonymous);
        // The name must now hold that inode with the approved bytes;
        // otherwise this is never Applied.
        let published = dir.read_with_identity(&name, MAX_FILE);
        let verified = matches!(&published, Ok((bytes, identity, _))
            if *identity == installed && ManifestEntry::of(bytes) == candidate);
        if !verified {
            self.apply_state = ApplyState::RecoveryRequired;
            let mut facts = ledger::path_facts(&path.as_string());
            facts["failed"] = json!("verify");
            let _ = record(
                self.ledger.as_ref(),
                self.id.0,
                EventKind::ApplyRecoveryRequired,
                &facts,
            );
            return Err(ApplyError::RecoveryRequired {
                failed: "verify".to_string(),
                unrestored: vec![path.as_string()],
            });
        }
        let file = AppliedFile {
            path: path.clone(),
            kind: ChangeKind::Create,
            dir,
            name,
            candidate,
            installed,
            preimage: None,
            temp,
        };

        // The directory sync, then the records.
        let synced = !seam(Seam::Sync) && file.dir.sync().is_ok();
        let text = path.as_string();
        let op = json!({
            "path_sha256": ledger::path_sha256(&text),
            "path_bytes": text.len(),
            "ok": true,
        });
        if record(self.ledger.as_ref(), self.id.0, EventKind::ApplyOp, &op).is_err() {
            return Err(self.compensate(file, store, Failed::Step("apply.op")));
        }
        if !synced {
            // The file is in place but not durably linked: recovery is
            // required, and its undo stays available.
            self.applied = Some(ApplyRecord {
                binding,
                files: vec![file],
                dirs: Vec::new(),
                storage: store.dir,
                storage_parent: store.parent,
                storage_name: store.name,
                unsynced: true,
            });
            self.apply_state = ApplyState::RecoveryRequired;
            let _ = record(
                self.ledger.as_ref(),
                self.id.0,
                EventKind::ApplyRecoveryRequired,
                &json!({ "failed": "dir_sync" }),
            );
            return Err(ApplyError::RecoveryRequired {
                failed: "dir_sync".to_string(),
                unrestored: Vec::new(),
            });
        }
        let completed = json!({ "binding": hex::encode(binding.hash()), "files": 1 });
        if record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::ApplyCompleted,
            &completed,
        )
        .is_err()
        {
            return Err(self.compensate(file, store, Failed::Step("apply.completed")));
        }
        self.applied = Some(ApplyRecord {
            binding,
            files: vec![file],
            dirs: Vec::new(),
            storage: store.dir,
            storage_parent: store.parent,
            storage_name: store.name,
            unsynced: false,
        });
        self.apply_state = ApplyState::Applied;
        Ok(ApplyReport {
            binding,
            files: vec![(path, ChangeKind::Create)],
        })
    }

    /// Nothing was published (the anonymous candidate, if any, is freed
    /// with its descriptor): record the outcome, `RolledBack`, or
    /// `Interrupted` for an interruption.
    fn unpublished(&mut self, cause: CleanupCause, store: Store) -> ApplyError {
        let (code, interrupted) = match cause {
            CleanupCause::Interrupted(cause) => ("interrupted", Some(cause)),
            CleanupCause::TempWriteFailed => ("temp", None),
            _ => ("publish", None),
        };
        let mut facts = json!({ "failed": code });
        if let Some(cause) = interrupted {
            facts["interrupted"] = json!(cause.code());
        }
        if record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::ApplyRolledBack,
            &facts,
        )
        .is_err()
        {
            return self.completion_unrecorded("rollback", &Failed::Step(code));
        }
        self.apply_state = ApplyState::RolledBack;
        store.discard();
        match interrupted {
            Some(cause) => ApplyError::Interrupted { cause },
            None => ApplyError::RolledBack {
                failed: code.to_string(),
            },
        }
    }

    /// The publication could not be recorded: undo it, verified, through its
    /// recorded temporary name (compensating, never cancelled).
    fn compensate(&mut self, file: AppliedFile, store: Store, failed: Failed) -> ApplyError {
        let temp = file.temp.clone();
        let path = file.path.as_string();
        let (unrestored, hidden) = match unpublish(&file, &temp) {
            Unpublish::Removed => (Vec::new(), Vec::new()),
            Unpublish::Owed(owned, removal) => {
                return self.owe(
                    vec![(owned, removal, path)],
                    CleanupCause::Unrecorded,
                    &failed.facts(),
                    Settled::RolledBack,
                    Some(store),
                );
            }
            Unpublish::NotOurs | Unpublish::NotMoved => (vec![path], Vec::new()),
            Unpublish::Stuck => (Vec::new(), vec![(path, temp)]),
        };
        let Store { parent, name, dir } = store;
        self.finish_failed(failed, unrestored, hidden, &parent, name, dir)
    }

    /// The run owes these temporary files: it requires recovery, each is
    /// recorded by its id and why its removal is not established, and the
    /// run keeps them until a retry establishes it.
    fn owe(
        &mut self,
        temps: Vec<(OwnedTemp, Removal, String)>,
        cause: CleanupCause,
        facts: &Value,
        then: Settled,
        store: Option<Store>,
    ) -> ApplyError {
        self.apply_state = ApplyState::RecoveryRequired;
        for (temp, removal, path) in &temps {
            let mut facts = facts.clone();
            facts["temp_id"] = json!(temp.id());
            facts["removal"] = json!(removal.code());
            facts["path_sha256"] = json!(ledger::path_sha256(path));
            facts["path_bytes"] = json!(path.len());
            let _ = record(
                self.ledger.as_ref(),
                self.id.0,
                EventKind::ApplyRecoveryRequired,
                &facts,
            );
        }
        let temps: Vec<OwnedTemp> = temps.into_iter().map(|(temp, ..)| temp).collect();
        let ids = temps.iter().map(OwnedTemp::id).collect();
        self.owed = Some(Owed {
            temps,
            cause,
            then,
            store,
        });
        ApplyError::CleanupOwed { cause, temps: ids }
    }

    /// Record a failed apply after rollback and set the resulting state.
    /// `unrestored` are the files the rollback could not restore, `hidden`
    /// the owner versions left under a temporary name. The returned error
    /// names them; the ledger keeps their digests and the temporary names'
    /// backend ids.
    fn finish_failed(
        &mut self,
        failed: Failed,
        unrestored: Vec<String>,
        hidden: Vec<(String, String)>,
        storage_parent: &DirHandle,
        storage_name: String,
        storage_dir: DirHandle,
    ) -> ApplyError {
        let mut facts = failed.facts();
        facts["unrestored"] = unrestored_facts(&unrestored, &hidden);
        let mut unrestored = unrestored;
        unrestored.extend(
            hidden
                .iter()
                .map(|(path, temp)| format!("{path} (your concurrent version is saved as {temp})")),
        );
        let failed_text = failed.text();
        if unrestored.is_empty() {
            // A clean rollback is claimed only once it is durably recorded;
            // the pre-image store is discarded only after that.
            if record(
                self.ledger.as_ref(),
                self.id.0,
                EventKind::ApplyRolledBack,
                &facts,
            )
            .is_ok()
            {
                self.apply_state = ApplyState::RolledBack;
                discard_storage(storage_parent, &storage_name, &storage_dir);
                return ApplyError::RolledBack {
                    failed: failed_text,
                };
            }
            return self.completion_unrecorded("rollback", &failed);
        }
        // Partial: recovery is required whether or not this is recorded, and
        // the pre-images stay in run storage for manual recovery.
        self.apply_state = ApplyState::RecoveryRequired;
        let _ = record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::ApplyRecoveryRequired,
            &facts,
        );
        ApplyError::RecoveryRequired {
            failed: failed_text,
            unrestored,
        }
    }

    /// The filesystem work of `operation` finished but its completion could
    /// not be recorded: recovery is required, the pre-image store is kept,
    /// and the actual state is recorded once (best effort, never retried).
    fn completion_unrecorded(&mut self, operation: &'static str, failed: &Failed) -> ApplyError {
        self.apply_state = ApplyState::RecoveryRequired;
        let mut facts = failed.facts();
        facts["completion_unrecorded"] = json!(operation);
        let _ = record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::ApplyRecoveryRequired,
            &facts,
        );
        ApplyError::CompletionUnrecorded { operation }
    }

    /// Resolve a write grant for this run's project: live, writable, bound
    /// to this run, for this project, and rooted at the directory the run
    /// retained. Returns a fresh handle on that root. The run's own read
    /// grant is closed by now and is never used here.
    fn project_root_for_write(&self, grant: &ProjectGrant) -> Result<DirHandle, ApplyError> {
        let retained = self.granted_root.ok_or(refused(Refusal::WrongProject))?;
        if grant.binding != self.binding
            || !std::sync::Arc::ptr_eq(&grant.authority, &self.registry)
            || self.project_id.is_some_and(|id| id != grant.project)
            || grant.identity != retained
        {
            return Err(refused(Refusal::WrongProject));
        }
        let resolved = self
            .registry
            .resolve(grant.grant, self.binding)
            .map_err(|_| refused(Refusal::AuthorityDenied))?;
        if *resolved.permission() != FsPermissionLevel::ReadWrite {
            return Err(refused(Refusal::AuthorityDenied));
        }
        let root = DirHandle::open_absolute(resolved.root())
            .map_err(|_| refused(Refusal::IdentityChanged))?;
        if root.identity() != retained {
            return Err(refused(Refusal::IdentityChanged));
        }
        Ok(root)
    }

    /// Check one target against the base and retain its directory.
    fn preflight(
        &self,
        root: &DirHandle,
        path: &RelPath,
        base: Option<ManifestEntry>,
        candidate: ManifestEntry,
        content: Vec<u8>,
    ) -> Result<Planned, ApplyError> {
        let text = path.as_string();
        let kind = if base.is_some() {
            ChangeKind::Replace
        } else {
            ChangeKind::Create
        };
        let (parents, name) = path.parent_and_name();
        let mut dir = root
            .try_clone()
            .map_err(|_| refused(Refusal::Stale(text.clone())))?;
        let mut missing = Vec::new();
        for part in parents {
            if !missing.is_empty() {
                missing.push(part.clone());
                continue;
            }
            match dir
                .kind(part)
                .map_err(|_| refused(Refusal::Stale(text.clone())))?
            {
                EntryKind::Directory => {
                    dir = dir
                        .open_subdir(part)
                        .map_err(|_| refused(Refusal::Redirect(text.clone())))?;
                }
                EntryKind::Missing if kind == ChangeKind::Create => missing.push(part.clone()),
                EntryKind::Missing => return Err(refused(Refusal::Stale(text))),
                _ => return Err(refused(Refusal::Redirect(text))),
            }
        }
        let pre = if !missing.is_empty() {
            None
        } else {
            match (dir.kind(name), kind) {
                (Ok(EntryKind::Missing), ChangeKind::Create) => None,
                (Ok(EntryKind::Regular), ChangeKind::Replace) => {
                    let (bytes, identity, mode) = dir
                        .read_with_identity(name, self.profile.max_file_bytes)
                        .map_err(|_| refused(Refusal::Redirect(text.clone())))?;
                    if Some(ManifestEntry::of(&bytes)) != base {
                        return Err(refused(Refusal::Stale(text)));
                    }
                    Some((bytes, identity, mode))
                }
                (Ok(EntryKind::Symlink | EntryKind::Special | EntryKind::Directory), _) => {
                    return Err(refused(Refusal::Redirect(text)))
                }
                _ => return Err(refused(Refusal::Stale(text))),
            }
        };
        Ok(Planned {
            path: path.clone(),
            kind,
            dir,
            missing,
            name: name.to_string(),
            content,
            candidate,
            base,
            pre,
            temp: plan_temp_name(),
        })
    }

    /// Store every pre-image in a fresh backend-private directory under the
    /// coding-runs storage parent and record where each one is.
    fn store_preimages(
        &self,
        storage: &StagingParent,
        plan: &[Planned],
    ) -> Result<(DirHandle, String, Vec<serde_json::Value>), ApplyError> {
        let name = format!("{}.apply", self.id);
        let dir = storage
            .handle
            .make_subdir_with_mode(&name, 0o700)
            .map_err(|_| refused(Refusal::PreimageUnavailable))?;
        let mut facts = Vec::new();
        for (index, planned) in plan.iter().enumerate() {
            let entry = match &planned.pre {
                Some((bytes, _, mode)) => {
                    let slot = format!("p{index:05}");
                    if dir.create_snapshot_file(&slot, bytes).is_err() {
                        discard_storage(&storage.handle, &name, &dir);
                        return Err(refused(Refusal::PreimageUnavailable));
                    }
                    let mut facts = ledger::path_facts(&planned.path.as_string());
                    facts["existed"] = json!(true);
                    facts["slot"] = json!(index);
                    facts["sha256"] = json!(hex::encode(ManifestEntry::of(bytes).sha256));
                    facts["size"] = json!(bytes.len());
                    facts["mode"] = json!(mode & 0o7777);
                    facts
                }
                None => {
                    let mut facts = ledger::path_facts(&planned.path.as_string());
                    facts["existed"] = json!(false);
                    facts
                }
            };
            facts.push(entry);
        }
        if dir.sync().is_err() {
            discard_storage(&storage.handle, &name, &dir);
            return Err(refused(Refusal::PreimageUnavailable));
        }
        Ok((dir, name, facts))
    }

    // ── Restore (P1-07) ────────────────────────────────────────────────

    /// Ask the owner, natively, to restore this run's apply.
    pub fn request_restore(
        &mut self,
        project_name: &str,
        confirmer: &dyn OwnerConfirmer,
    ) -> Result<OwnerApproval, ApplyError> {
        let record_ = self.applied.as_ref().ok_or(ApplyError::InvalidState)?;
        if !self.restorable() {
            return Err(ApplyError::InvalidState);
        }
        // H2′: this undo's own watches on both authorities, taken now,
        // before its dialog; refused while a stop is in force.
        let Some(watches) = self.undo_watches() else {
            let refusal = Refusal::EmergencyStopped;
            let _ = record(
                self.ledger.as_ref(),
                self.id.0,
                EventKind::RestoreRejected,
                &ledger::refusal_facts(&refusal),
            );
            return Err(refused(refusal));
        };
        let binding = record_.binding;
        let mut request =
            ConfirmationRequest::new(ConfirmationKind::Restore, project_name, &binding)
                .with_paths(record_.files.iter().map(|f| (&f.path, f.kind)));
        if let [file] = record_.files.as_slice() {
            request.bytes = Some(file.candidate.size);
            request.sha256_full = Some(hex::encode(file.candidate.sha256));
            if file.kind == ChangeKind::Create {
                request.identity = Some(FileIdentity {
                    dev: file.installed.dev,
                    ino: file.installed.ino,
                });
            }
        }
        let confirmed = confirmer.confirm(&request);
        let event = if confirmed {
            EventKind::RestoreApproved
        } else {
            EventKind::RestoreDeclined
        };
        record(
            self.ledger.as_ref(),
            self.id.0,
            event,
            &json!({ "binding": hex::encode(binding.hash()) }),
        )
        .map_err(|_| refused(Refusal::Unrecorded))?;
        if !confirmed {
            return Err(refused(Refusal::Declined));
        }
        self.restore_approved = Some(binding);
        self.undo_watches = Some(watches);
        Ok(OwnerApproval {
            kind: ConfirmationKind::Restore,
            binding,
        })
    }

    /// Whether this run's apply can be undone: it is applied, or its
    /// publication's directory sync failed (the file is in place).
    fn restorable(&self) -> bool {
        self.apply_state == ApplyState::Applied
            || (self.apply_state == ApplyState::RecoveryRequired
                && self
                    .applied
                    .as_ref()
                    .is_some_and(|applied| applied.unsynced))
    }

    /// Restore this run's apply: only if every applied file still has the
    /// content Nexus wrote (a created file must still be the very file Nexus
    /// created). Otherwise nothing is touched.
    pub fn restore(
        &mut self,
        approval: OwnerApproval,
        grant: ProjectGrant,
    ) -> Result<ApplyReport, ApplyError> {
        let watches = self.undo_watches.take();
        // M0-FIX F5: refused, before any write, while the stop is in force.
        let result = if self.halt_if_stopped().is_err() {
            self.restore_approved = None;
            Err(refused(Refusal::EmergencyStopped))
        } else if approval.kind == ConfirmationKind::Restore {
            self.restore_inner(approval.binding, &grant, watches)
        } else {
            self.restore_approved = None;
            Err(refused(Refusal::ApprovalMismatch))
        };
        // Clean success needs the durable completion (checked inside) and a
        // confirmed closure of the write grant; only then is the run
        // Restored and the private pre-image store discarded.
        if !close_write_grant(&grant) {
            let after = result.map(|(report, _kept)| report);
            return Err(self.authority_not_closed(after));
        }
        match result {
            Ok((report, applied)) => {
                self.apply_state = ApplyState::Restored;
                discard_storage(
                    &applied.storage_parent,
                    &applied.storage_name,
                    &applied.storage,
                );
                Ok(report)
            }
            Err(error) => {
                if let ApplyError::Refused(refusal) = &error {
                    // Informational: nothing was touched.
                    let _ = record(
                        self.ledger.as_ref(),
                        self.id.0,
                        EventKind::RestoreRejected,
                        &ledger::refusal_facts(refusal),
                    );
                }
                Err(error)
            }
        }
    }

    /// Restore on disk and record its durable completion. On success the
    /// consumed apply record (holding the pre-image store) is returned for
    /// the caller to discard once the write grant is confirmed closed.
    ///
    /// M2d: the undo's own watches (H2′) are checked first and again at U1,
    /// right before the first unpublishing rename, with the write grant.
    /// Temporary names are planned and recorded before anything moves; a
    /// created file is moved aside to its name, removed only if it is still
    /// the very file Nexus created, and owned until its removal is
    /// established. A file found not ours is renamed back: with nothing yet
    /// touched the undo is `Stale` and stays offered.
    fn restore_inner(
        &mut self,
        binding: ReviewBinding,
        grant: &ProjectGrant,
        watches: Option<UndoWatches>,
    ) -> Result<(ApplyReport, ApplyRecord), ApplyError> {
        if !self.restorable() || self.applied.is_none() {
            return Err(ApplyError::InvalidState);
        }
        let approved = self.restore_approved.take();
        let record_binding = self.applied.as_ref().map(|r| r.binding);
        if approved.is_none() {
            return Err(refused(Refusal::NotApproved));
        }
        if approved != Some(binding) || record_binding != Some(binding) {
            return Err(refused(Refusal::ApprovalMismatch));
        }
        // An approval never exists without the watches its undo took.
        let watches = watches.ok_or(refused(Refusal::NotApproved))?;
        if stopped(&watches.run, watches.observed.as_ref()) {
            return Err(refused(Refusal::Interrupted(InterruptCause::EmergencyStop)));
        }
        let root = self.project_root_for_write(grant)?;
        let max = self.profile.max_file_bytes;
        let applied = self.applied.as_ref().ok_or(ApplyError::InvalidState)?;
        // Preflight: every file must still be what Nexus wrote, reached at
        // its path through the same directories, and every pre-image must
        // still be intact, before anything is touched.
        for file in &applied.files {
            let text = file.path.as_string();
            if !reaches(&root, &file.path, file.dir.identity()) {
                return Err(refused(Refusal::Stale(text)));
            }
            let (bytes, identity, _) = file
                .dir
                .read_with_identity(&file.name, max)
                .map_err(|_| refused(Refusal::Stale(text.clone())))?;
            if ManifestEntry::of(&bytes) != file.candidate
                || (file.kind == ChangeKind::Create && identity != file.installed)
            {
                return Err(refused(Refusal::Stale(text)));
            }
            if let Some(pre) = &file.preimage {
                let bytes = applied
                    .storage
                    .read_regular(&pre.slot, max)
                    .map_err(|_| refused(Refusal::PreimageUnavailable))?;
                if ManifestEntry::of(&bytes) != pre.entry {
                    return Err(refused(Refusal::PreimageUnavailable));
                }
            }
        }
        let mut temps: Vec<String> = applied.files.iter().map(|_| plan_temp_name()).collect();
        let planned: Vec<Value> = applied
            .files
            .iter()
            .zip(&temps)
            .map(|(file, temp)| {
                let mut facts = ledger::path_facts(&file.path.as_string());
                facts["temp_id"] = json!(temp_id(temp));
                facts
            })
            .collect();
        let first = applied.files.first().map(|f| f.path.clone());
        record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::RestorePrepared,
            &json!({
                "binding": hex::encode(binding.hash()),
                "files": applied.files.len(),
                "planned": planned,
            }),
        )
        .map_err(|_| refused(Refusal::Unrecorded))?;

        // U1: the undo's final check, with the watches it took at its start.
        if let Some(path) = &first {
            hook("unpublish", path);
        }
        if let Some(cause) = final_check(grant, None, &watches.run, watches.observed.as_ref()) {
            return Err(refused(Refusal::Interrupted(cause)));
        }

        let mut applied = self.applied.take().ok_or(ApplyError::InvalidState)?;
        let files: Vec<(RelPath, ChangeKind)> = applied
            .files
            .iter()
            .map(|f| (f.path.clone(), f.kind))
            .collect();
        let (mut unrestored, mut hidden, mut owed) = (Vec::new(), Vec::new(), Vec::new());
        let mut done = 0usize;
        while let Some(file) = applied.files.pop() {
            let temp = temps.pop().unwrap_or_else(plan_temp_name);
            let path = file.path.as_string();
            match file.kind {
                ChangeKind::Create => match unpublish(&file, &temp) {
                    Unpublish::Removed => {}
                    Unpublish::Owed(owned, removal) => owed.push((owned, removal, path)),
                    Unpublish::NotOurs if done == 0 => {
                        // Nothing was touched: the undo is stale and stays
                        // offered.
                        applied.files.push(file);
                        self.applied = Some(applied);
                        return Err(refused(Refusal::Stale(path)));
                    }
                    Unpublish::NotOurs | Unpublish::NotMoved => unrestored.push(path),
                    Unpublish::Stuck => hidden.push((path, temp)),
                },
                ChangeKind::Replace => {
                    if !restore_replaced(&file, &applied.storage) {
                        unrestored.push(path);
                    }
                }
                ChangeKind::Delete => unrestored.push(path),
            }
            done += 1;
        }
        while let Some(created) = applied.dirs.pop() {
            let _ = created
                .parent
                .remove_subdir_if_identity(&created.name, created.identity);
        }
        if !unrestored.is_empty() || !hidden.is_empty() {
            // Partial: recovery is required whether or not this is recorded.
            if !owed.is_empty() {
                let _ = self.owe(
                    owed,
                    CleanupCause::Undo,
                    &json!({ "failed": "restore" }),
                    Settled::Recovery,
                    None,
                );
            }
            self.apply_state = ApplyState::RecoveryRequired;
            let _ = record(
                self.ledger.as_ref(),
                self.id.0,
                EventKind::ApplyRecoveryRequired,
                &json!({ "failed": "restore", "unrestored": unrestored_facts(&unrestored, &hidden) }),
            );
            unrestored.extend(hidden.into_iter().map(|(path, temp)| {
                format!("{path} (your concurrent version is saved as {temp})")
            }));
            return Err(ApplyError::RecoveryRequired {
                failed: "restore".to_string(),
                unrestored,
            });
        }
        if !owed.is_empty() {
            let store = Store {
                parent: applied.storage_parent,
                name: applied.storage_name,
                dir: applied.storage,
            };
            let settled = Settled::Restored {
                binding,
                files: files.len(),
            };
            return Err(self.owe(
                owed,
                CleanupCause::Undo,
                &json!({ "failed": "restore" }),
                settled,
                Some(store),
            ));
        }
        // Files are restored; success is claimed only after the durable
        // completion record. Without it the pre-image store is kept.
        if record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::RestoreCompleted,
            &json!({ "binding": hex::encode(binding.hash()), "files": files.len() }),
        )
        .is_err()
        {
            return Err(self.completion_unrecorded("restore", &Failed::Step("restore.completed")));
        }
        Ok((ApplyReport { binding, files }, applied))
    }
}

/// Where an apply, a rollback or a restore stopped, for its ledger record.
enum Failed {
    /// At one file: the returned error names it, the ledger its digest.
    File(String),
    /// At a named step (one of [`ledger::APPLY_STEPS`]).
    Step(&'static str),
}

impl Failed {
    /// What the returned error says.
    fn text(&self) -> String {
        match self {
            Self::File(path) => path.clone(),
            Self::Step(step) => (*step).to_string(),
        }
    }

    /// What the ledger records: `failed` as a code, and a file by digest.
    fn facts(&self) -> serde_json::Value {
        match self {
            Self::File(path) => {
                let mut facts = ledger::path_facts(path);
                facts["failed"] = json!("file");
                facts
            }
            Self::Step(step) => json!({ "failed": step }),
        }
    }
}

/// The unrestored files by digest, then each hidden owner version by its
/// file's digest and the backend id of the temporary name it stayed under.
fn unrestored_facts(paths: &[String], hidden: &[(String, String)]) -> serde_json::Value {
    let mut out: Vec<serde_json::Value> =
        paths.iter().map(|path| ledger::path_facts(path)).collect();
    for (path, temp) in hidden {
        let mut facts = ledger::path_facts(path);
        if let Some(id) = temp.strip_prefix(super::scope::TEMP_PREFIX) {
            facts["owner_version_temp"] = json!(id);
        }
        out.push(facts);
    }
    serde_json::Value::Array(out)
}

/// Whether `path`'s parent, walked from `root` without following links, is
/// the directory with identity `expected`.
fn reaches(root: &DirHandle, path: &RelPath, expected: NodeIdentity) -> bool {
    let (parents, _) = path.parent_and_name();
    let Ok(mut dir) = root.try_clone() else {
        return false;
    };
    for part in parents {
        match dir.kind(part) {
            Ok(EntryKind::Directory) => match dir.open_subdir(part) {
                Ok(next) => dir = next,
                Err(_) => return false,
            },
            _ => return false,
        }
    }
    dir.identity() == expected
}

fn request_for(kind: ConfirmationKind, project_name: &str, review: &Review) -> ConfirmationRequest {
    let mut request = ConfirmationRequest::new(kind, project_name, &review.binding)
        .with_paths(review.changes.iter().map(|c| (&c.path, c.kind)));
    if let [FileChange { new: Some(new), .. }] = review.changes.as_slice() {
        request.bytes = Some(new.size);
        request.sha256_full = Some(hex::encode(new.sha256));
    }
    request
}

fn preimages_slots(plan: &[Planned]) -> std::collections::BTreeMap<RelPath, Preimage> {
    plan.iter()
        .enumerate()
        .filter_map(|(index, p)| {
            p.pre.as_ref().map(|(bytes, identity, mode)| {
                (
                    p.path.clone(),
                    Preimage {
                        slot: format!("p{index:05}"),
                        entry: ManifestEntry::of(bytes),
                        identity: *identity,
                        mode: *mode,
                    },
                )
            })
        })
        .collect()
}

/// Write one planned file. A written file is pushed to `applied` as soon as
/// it exists in the project, so a later failure can undo it.
/// Why one planned write failed.
enum WriteFailure {
    Failed,
    /// The owner's concurrent version could not be put back and remains in
    /// the target's directory under this temporary name.
    OwnerVersionAt(String),
}

fn write_one(
    planned: Planned,
    preimage: Option<Preimage>,
    dirs: &mut Vec<CreatedDir>,
    applied: &mut Vec<AppliedFile>,
) -> Result<(), WriteFailure> {
    let failed = |_| WriteFailure::Failed;
    let Planned {
        path,
        kind,
        dir,
        missing,
        name,
        content,
        candidate,
        temp,
        ..
    } = planned;
    match kind {
        ChangeKind::Replace => {
            let pre = preimage.ok_or(WriteFailure::Failed)?;
            let installed = dir
                .write_temp_named_with_mode(&temp, &content, pre.mode)
                .map_err(failed)?;
            if hook("exchange", &path) {
                let _ = dir.remove_temp(&temp);
                return Err(WriteFailure::Failed);
            }
            if dir.exchange(&temp, &name).is_err() {
                let _ = dir.remove_temp(&temp);
                return Err(WriteFailure::Failed);
            }
            hook_after_exchange(&dir, &temp, &name, &path);
            // The displaced file must be the one checked in preflight.
            let displaced = dir.read_with_identity(&temp, MAX_FILE);
            let unchanged = matches!(&displaced, Ok((bytes, identity, _))
                if *identity == pre.identity && ManifestEntry::of(bytes) == pre.entry);
            if !unchanged {
                // Put the owner's concurrent version back; drop ours.
                if dir.exchange(&temp, &name).is_ok() {
                    let _ = dir.remove_temp(&temp);
                    return Err(WriteFailure::Failed);
                }
                applied.push(AppliedFile {
                    path,
                    kind,
                    dir,
                    name,
                    candidate,
                    installed,
                    preimage: Some(pre),
                    temp: temp.clone(),
                });
                return Err(WriteFailure::OwnerVersionAt(temp));
            }
            let removed = dir.remove_temp(&temp).and_then(|()| dir.sync());
            applied.push(AppliedFile {
                path,
                kind,
                dir,
                name,
                candidate,
                installed,
                preimage: Some(pre),
                temp,
            });
            removed.map_err(failed)
        }
        ChangeKind::Create => {
            let mut dir = dir;
            for part in missing {
                // A directory an earlier creation of this apply made (still
                // the same directory) is reused; anything else at the name
                // appeared concurrently and fails the apply.
                let ours = dirs
                    .iter()
                    .find(|d| d.name == part && d.parent.identity() == dir.identity())
                    .map(|d| d.identity);
                let sub = match ours {
                    Some(identity) => {
                        let sub = dir.open_subdir(&part).map_err(failed)?;
                        if sub.identity() != identity {
                            return Err(WriteFailure::Failed);
                        }
                        sub
                    }
                    None => {
                        let sub = dir
                            .make_subdir_with_mode(&part, CREATED_DIR_MODE)
                            .map_err(failed)?;
                        dirs.push(CreatedDir {
                            parent: dir.try_clone().map_err(failed)?,
                            name: part,
                            identity: sub.identity(),
                        });
                        sub
                    }
                };
                dir = sub;
            }
            let installed = dir
                .write_temp_named_with_mode(&temp, &content, CREATED_FILE_MODE)
                .map_err(failed)?;
            // M2d: published by one rename that never replaces; the
            // temporary name is consumed by it.
            if dir.rename_noreplace(&temp, &name).is_err() {
                let _ = dir.remove_temp(&temp);
                return Err(WriteFailure::Failed);
            }
            let synced = dir.sync();
            applied.push(AppliedFile {
                path,
                kind,
                dir,
                name,
                candidate,
                installed,
                preimage: None,
                temp,
            });
            synced.map_err(failed)
        }
        ChangeKind::Delete => Err(WriteFailure::Failed),
    }
}

/// Undo applied files in reverse order, then remove directories the apply
/// created (only if empty and still the same directory). Pre-images are read
/// back from the private store and hash-checked. Returns the paths that
/// could not be restored.
fn rollback(
    files: &mut Vec<AppliedFile>,
    dirs: &mut Vec<CreatedDir>,
    storage: &DirHandle,
) -> Vec<String> {
    let mut unrestored = Vec::new();
    while let Some(file) = files.pop() {
        let ok = match file.kind {
            ChangeKind::Replace => restore_replaced(&file, storage),
            ChangeKind::Create => remove_created(&file),
            ChangeKind::Delete => false,
        };
        if !ok {
            unrestored.push(file.path.as_string());
        }
    }
    while let Some(created) = dirs.pop() {
        let _ = created
            .parent
            .remove_subdir_if_identity(&created.name, created.identity);
    }
    unrestored.reverse();
    unrestored
}

/// Put a replaced file's pre-image back, if the target still holds exactly
/// what Nexus wrote.
fn restore_replaced(file: &AppliedFile, storage: &DirHandle) -> bool {
    let Some(pre) = &file.preimage else {
        return false;
    };
    let Ok(bytes) = storage.read_regular(&pre.slot, MAX_FILE) else {
        return false;
    };
    if ManifestEntry::of(&bytes) != pre.entry {
        return false;
    }
    let Ok((temp, _)) = file.dir.write_temp_with_mode(&bytes, pre.mode) else {
        return false;
    };
    if file.dir.exchange(&temp, &file.name).is_err() {
        let _ = file.dir.remove_temp(&temp);
        return false;
    }
    let displaced = file.dir.read_with_identity(&temp, MAX_FILE);
    let ours =
        matches!(&displaced, Ok((content, _, _)) if ManifestEntry::of(content) == file.candidate);
    if !ours {
        // Someone else's content: put it back and leave the file alone.
        if file.dir.exchange(&temp, &file.name).is_ok() {
            let _ = file.dir.remove_temp(&temp);
        }
        return false;
    }
    file.dir.remove_temp(&temp).is_ok() && file.dir.sync().is_ok()
}

/// Remove a created file only if it is still the very file Nexus created.
fn remove_created(file: &AppliedFile) -> bool {
    let temp = format!("{}{}", super::scope::TEMP_PREFIX, uuid::Uuid::new_v4());
    if file.dir.rename_noreplace(&file.name, &temp).is_err() {
        return false;
    }
    let moved = file.dir.read_with_identity(&temp, MAX_FILE);
    let ours = matches!(&moved, Ok((content, identity, _))
        if *identity == file.installed && ManifestEntry::of(content) == file.candidate);
    if !ours {
        let _ = file.dir.rename_noreplace(&temp, &file.name);
        return false;
    }
    file.dir.remove_temp(&temp).is_ok() && file.dir.sync().is_ok()
}

/// Remove the pre-image store (after a clean rollback or restore).
fn discard_storage(parent: &DirHandle, name: &str, dir: &DirHandle) {
    if dir.remove_all_entries().is_ok() {
        let _ = parent.remove_subdir_if_identity(name, dir.identity());
    }
}
