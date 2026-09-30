//! Owner approval and apply (Phase One, P1-06).
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
//! exchange is undone. A creation is published with `link`, which never
//! replaces. If any step fails, already-applied files are restored in reverse
//! order and the outcome is reported as rolled back or as requiring
//! recovery, never as applied. Multi-file apply is not atomic; the ledger
//! and the filesystem are separate transactional domains.
//!
//! The apply record kept by the run is the only material a later restore of
//! this one run may use.

use serde_json::json;
use thiserror::Error;

use super::fsops::{DirHandle, EntryKind, NodeIdentity};
use super::ledger::{record, EventKind};
use super::manifest::ManifestEntry;
use super::review::{ChangeKind, Review, ReviewBinding};
use super::scope::RelPath;
use super::structural::StructuralProfile;
use super::{CodingRun, ProjectGrant, RunError, RunId, RunState, StagingParent};
use crate::manifest::FsPermissionLevel;

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
    pub candidate_short: String,
    pub base_short: String,
    pub paths: Vec<String>,
    pub more_paths: usize,
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
             Candidate: {}\nBase: {}\n",
            self.project_name,
            self.run_id,
            self.files(),
            self.creates,
            self.replaces,
            self.deletes,
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
        let name: String = project_name
            .chars()
            .map(|c| if c.is_control() { '\u{fffd}' } else { c })
            .take(MAX_DISPLAY_NAME)
            .collect();
        Self {
            kind,
            project_name: name,
            run_id: binding.run_id,
            creates: 0,
            replaces: 0,
            deletes: 0,
            candidate_short: binding.candidate_manifest_hash.to_hex()[..12].to_string(),
            base_short: binding.base_manifest_hash.to_hex()[..12].to_string(),
            paths: Vec::new(),
            more_paths: 0,
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
                self.paths.push(format!("{verb} {}", path.as_string()));
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
    binding: ReviewBinding,
}

impl OwnerApproval {
    pub fn binding(&self) -> &ReviewBinding {
        &self.binding
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
}

/// A directory created by an apply.
#[derive(Debug)]
struct CreatedDir {
    parent: DirHandle,
    name: String,
    identity: NodeIdentity,
}

/// The record of one successful apply: the only restore authority.
// The pre-image store and created directories are read by restore (P1-07).
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct ApplyRecord {
    binding: ReviewBinding,
    files: Vec<AppliedFile>,
    dirs: Vec<CreatedDir>,
    storage: DirHandle,
    storage_parent: DirHandle,
    storage_name: String,
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
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookPoint {
    /// Before the first write for a file (return `true` to inject failure).
    BeforeWrite,
    /// After the replacement is written, before the exchange.
    BeforeExchange,
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
            _ => HookPoint::BeforeExchange,
        };
        let path = _path.as_string();
        return APPLY_HOOK.with(|h| h.borrow_mut().as_mut().is_some_and(|f| f(point, &path)));
    }
    #[allow(unreachable_code)]
    false
}

fn refused(refusal: Refusal) -> ApplyError {
    ApplyError::Refused(refusal)
}

impl CodingRun {
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
        let result = self.apply_inner(approval.binding, &grant, storage);
        // The write grant never outlives the attempt.
        let _ = grant.authority.revoke(grant.grant, grant.binding);
        if let Err(ApplyError::Refused(refusal)) = &result {
            let _ = record(
                self.ledger.as_ref(),
                self.id.0,
                EventKind::ApplyRejected,
                &json!({ "reason": format!("{refusal:?}") }),
            );
        }
        result
    }

    fn apply_inner(
        &mut self,
        binding: ReviewBinding,
        grant: &ProjectGrant,
        storage: &StagingParent,
    ) -> Result<ApplyReport, ApplyError> {
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
        let current = ReviewBinding {
            run_id: self.id,
            base_manifest_hash: verification.base_manifest_hash,
            candidate_manifest_hash: verification.candidate_manifest_hash,
            profile_hash: verification.profile_hash,
        };
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

        // Pre-images, recorded before the first owner-project write.
        let (storage_dir, storage_name, preimages) = self.store_preimages(storage, &plan)?;
        let prepared = json!({
            "binding": hex::encode(binding.hash()),
            "preimages": preimages,
            "files": plan.iter().map(|p| json!({
                "path": p.path.as_string(),
                "kind": format!("{:?}", p.kind),
                "base": p.base.as_ref().map(|e| hex::encode(e.sha256)),
                "candidate": hex::encode(p.candidate.sha256),
            })).collect::<Vec<_>>(),
        });
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
        for planned in plan {
            let path = planned.path.clone();
            let slot = slots.remove(&path);
            let outcome = if hook("write", &path) {
                Err(())
            } else {
                write_one(planned, slot, &mut dirs, &mut applied)
            };
            let recorded = record(
                self.ledger.as_ref(),
                self.id.0,
                EventKind::ApplyOp,
                &json!({ "path": path.as_string(), "ok": outcome.is_ok() }),
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
            let failed = failure.unwrap_or_else(|| "apply.completed".to_string());
            let unrestored = rollback(&mut applied, &mut dirs, &storage_dir);
            return Err(self.finish_failed(
                failed,
                unrestored,
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
            storage_parent: storage
                .handle
                .try_clone()
                .map_err(|_| refused(Refusal::PreimageUnavailable))?,
            storage_name,
        });
        self.apply_state = ApplyState::Applied;
        Ok(ApplyReport { binding, files })
    }

    /// Record a failed apply after rollback and set the resulting state.
    fn finish_failed(
        &mut self,
        failed: String,
        unrestored: Vec<String>,
        storage_parent: &DirHandle,
        storage_name: String,
        storage_dir: DirHandle,
    ) -> ApplyError {
        let (event, error) = if unrestored.is_empty() {
            self.apply_state = ApplyState::RolledBack;
            discard_storage(storage_parent, &storage_name, &storage_dir);
            (
                EventKind::ApplyRolledBack,
                ApplyError::RolledBack {
                    failed: failed.clone(),
                },
            )
        } else {
            // Pre-images stay in run storage for manual recovery.
            self.apply_state = ApplyState::RecoveryRequired;
            (
                EventKind::ApplyRecoveryRequired,
                ApplyError::RecoveryRequired {
                    failed: failed.clone(),
                    unrestored: unrestored.clone(),
                },
            )
        };
        let _ = record(
            self.ledger.as_ref(),
            self.id.0,
            event,
            &json!({ "failed": failed, "unrestored": unrestored }),
        );
        error
    }

    /// Resolve a write grant for this run's project: live, writable, bound
    /// to this run, for this project, and rooted at the directory the run
    /// retained. Returns a fresh handle on that root.
    fn project_root_for_write(&self, grant: &ProjectGrant) -> Result<DirHandle, ApplyError> {
        let retained = self
            .project
            .as_ref()
            .ok_or(refused(Refusal::WrongProject))?
            .identity();
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
                    json!({
                        "path": planned.path.as_string(),
                        "existed": true,
                        "slot": slot,
                        "sha256": hex::encode(ManifestEntry::of(bytes).sha256),
                        "size": bytes.len(),
                        "mode": format!("{:o}", mode & 0o7777),
                    })
                }
                None => json!({ "path": planned.path.as_string(), "existed": false }),
            };
            facts.push(entry);
        }
        if dir.sync().is_err() {
            discard_storage(&storage.handle, &name, &dir);
            return Err(refused(Refusal::PreimageUnavailable));
        }
        Ok((dir, name, facts))
    }
}

fn request_for(kind: ConfirmationKind, project_name: &str, review: &Review) -> ConfirmationRequest {
    ConfirmationRequest::new(kind, project_name, &review.binding)
        .with_paths(review.changes.iter().map(|c| (&c.path, c.kind)))
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
fn write_one(
    planned: Planned,
    preimage: Option<Preimage>,
    dirs: &mut Vec<CreatedDir>,
    applied: &mut Vec<AppliedFile>,
) -> Result<(), ()> {
    let Planned {
        path,
        kind,
        dir,
        missing,
        name,
        content,
        candidate,
        ..
    } = planned;
    match kind {
        ChangeKind::Replace => {
            let pre = preimage.ok_or(())?;
            let (temp, installed) = dir
                .write_temp_with_mode(&content, pre.mode)
                .map_err(|_| ())?;
            if hook("exchange", &path) {
                let _ = dir.remove_temp(&temp);
                return Err(());
            }
            if dir.exchange(&temp, &name).is_err() {
                let _ = dir.remove_temp(&temp);
                return Err(());
            }
            // The displaced file must be the one checked in preflight.
            let displaced = dir.read_with_identity(&temp, MAX_FILE);
            let unchanged = matches!(&displaced, Ok((bytes, identity, _))
                if *identity == pre.identity && ManifestEntry::of(bytes) == pre.entry);
            if !unchanged {
                // Put the owner's concurrent version back; drop ours.
                if dir.exchange(&temp, &name).is_ok() {
                    let _ = dir.remove_temp(&temp);
                } else {
                    applied.push(AppliedFile {
                        path,
                        kind,
                        dir,
                        name,
                        candidate,
                        installed,
                        preimage: Some(pre),
                    });
                }
                return Err(());
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
            });
            removed.map_err(|_| ())
        }
        ChangeKind::Create => {
            let mut dir = dir;
            for part in missing {
                let sub = dir
                    .make_subdir_with_mode(&part, CREATED_DIR_MODE)
                    .map_err(|_| ())?;
                dirs.push(CreatedDir {
                    parent: dir,
                    name: part,
                    identity: sub.identity(),
                });
                dir = sub;
            }
            let (temp, installed) = dir
                .write_temp_with_mode(&content, CREATED_FILE_MODE)
                .map_err(|_| ())?;
            if dir.link_temp(&temp, &name).is_err() {
                let _ = dir.remove_temp(&temp);
                return Err(());
            }
            let removed = dir.remove_temp(&temp).and_then(|()| dir.sync());
            applied.push(AppliedFile {
                path,
                kind,
                dir,
                name,
                candidate,
                installed,
                preimage: None,
            });
            removed.map_err(|_| ())
        }
        ChangeKind::Delete => Err(()),
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
