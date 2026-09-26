//! Backend-owned filesystem authority, independent of path containment policy.
//!
//! Only a trusted backend issuer may choose a root and supply an authenticated
//! binding. Neither a path nor a deserialized handle proves authority. Keep the
//! registry in trusted application state; downstream requests carry only
//! [`WorkspaceGrantId`], resolved against the backend's current agent/run binding.
//! This module does not authenticate callers or expose an IPC registration API.
//!
//! The registry is deliberately in-memory and independent of audit/supervisor
//! locks. Higher-level issuers must record issuance, narrowing, revocation and
//! denials through the existing audit pathway *after* registry methods return.
//! No callbacks or other subsystem locks are acquired here.
//!
//! Canonical paths are not OS isolation: filesystem replacement races, hard
//! links, and shell/process containment require separate enforcement. Consumers
//! must re-resolve the handle for each operation and still apply containment and
//! their existing permission checks. A resolved snapshot is not a lasting lease.

use crate::manifest::FsPermissionLevel;
use crate::supervisor::AgentId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::SystemTime;
use thiserror::Error;
use uuid::Uuid;

/// Untrusted, opaque reference suitable for downstream/IPC input. Parsing or
/// guessing one never inserts a grant; only the owning registry can resolve it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceGrantId(Uuid);

/// Existing kernel agent identity plus the UUID run identity used by replay.
/// The backend must obtain both from trusted execution state, not request JSON.
/// There is no shared kernel project/mission identity; independent project runs
/// use distinct run IDs. Children retain the run and may bind a different agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceBinding {
    pub agent_id: AgentId,
    pub run_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceAuthoritySource {
    BackendAllocated,
    /// Reserved for a backend flow that has verified authenticated user intent.
    UserSelected,
    /// Only narrowing can produce this source; root issuance rejects it.
    ParentGrant,
}

/// Immutable snapshot of a registry-issued grant. All fields are private and
/// there is no public constructor, deserializer, or registry import API.
/// Changing a cloned permission value cannot change the stored authority.
///
/// The canonical root is the scope. C1 reuses the kernel's existing permission
/// levels: ReadOnly covers reading/enumeration/search; ReadWrite additionally
/// covers creation/write/delete/mutation; Deny permits no filesystem operation.
/// This does not convert arbitrary manifest glob rules into whole-root grants.
/// Finer path policies must still be intersected by consumers during migration.
///
/// ```compile_fail
/// use nexus_kernel::workspace_authority::WorkspaceGrant;
/// let forged: WorkspaceGrant = serde_json::from_str(r#"{"root":"/"}"#).unwrap();
/// ```
///
/// ```compile_fail
/// use nexus_kernel::workspace_authority::WorkspaceGrant;
/// fn widen(grant: &mut WorkspaceGrant) {
///     grant.root = std::path::PathBuf::from("/");
/// }
/// ```
#[derive(Debug, Clone)]
pub struct WorkspaceGrant {
    id: WorkspaceGrantId,
    root: PathBuf,
    source: WorkspaceAuthoritySource,
    binding: WorkspaceBinding,
    permission: FsPermissionLevel,
    parent: Option<WorkspaceGrantId>,
    issued_at: SystemTime,
    expires_at: Option<SystemTime>,
    revoked_at: Option<SystemTime>,
}

impl WorkspaceGrant {
    pub fn id(&self) -> WorkspaceGrantId {
        self.id
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn source(&self) -> WorkspaceAuthoritySource {
        self.source
    }

    pub fn binding(&self) -> WorkspaceBinding {
        self.binding
    }

    pub fn permission(&self) -> &FsPermissionLevel {
        &self.permission
    }

    pub fn parent(&self) -> Option<WorkspaceGrantId> {
        self.parent
    }

    pub fn issued_at(&self) -> SystemTime {
        self.issued_at
    }

    pub fn expires_at(&self) -> Option<SystemTime> {
        self.expires_at
    }
}

/// Path-free failures, safe to map to downstream denials without disclosing
/// host paths. Lock poisoning fails closed rather than recovering stale state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum WorkspaceAuthorityError {
    #[error("workspace root must be an existing absolute directory")]
    InvalidRoot,
    #[error("workspace binding requires an agent and run")]
    InvalidBinding,
    #[error("a root grant requires trusted root provenance")]
    InvalidSource,
    #[error("workspace grant expiry must be in the future")]
    InvalidExpiry,
    #[error("workspace grant is unknown")]
    UnknownGrant,
    #[error("workspace grant belongs to another agent")]
    WrongOwner,
    #[error("workspace grant belongs to another run")]
    WrongContext,
    #[error("workspace grant or its ancestor is revoked")]
    RevokedGrant,
    #[error("workspace grant or its ancestor has expired")]
    ExpiredGrant,
    #[error("child workspace exceeds its parent root")]
    RootWidening,
    #[error("child workspace exceeds its parent permissions")]
    PermissionWidening,
    #[error("workspace authority registry is unavailable")]
    RegistryUnavailable,
}

/// Concurrent backend storage. Issuance, narrowing, and revocation are atomic
/// with respect to resolution. Every resolution checks the complete parent
/// chain, so revoking any ancestor invalidates all descendants without a race
/// between separate cascade updates. Revocation is permanent and idempotent.
#[derive(Debug, Default)]
pub struct WorkspaceAuthorityRegistry {
    grants: RwLock<HashMap<WorkspaceGrantId, WorkspaceGrant>>,
}

impl WorkspaceAuthorityRegistry {
    /// Starts empty. No cwd, home directory, or repository is implicitly trusted.
    pub fn new() -> Self {
        Self::default()
    }

    /// Privileged backend operation, never an unvalidated frontend/IPC adapter.
    /// The caller must authenticate the binding and authorize this root and
    /// permission independently. Canonicalization alone is not authorization.
    /// No missing directory is created, even on failure.
    pub fn issue_trusted_root(
        &self,
        root: &Path,
        binding: WorkspaceBinding,
        source: WorkspaceAuthoritySource,
        permission: FsPermissionLevel,
        expires_at: Option<SystemTime>,
    ) -> Result<WorkspaceGrantId, WorkspaceAuthorityError> {
        validate_binding(binding)?;
        if source == WorkspaceAuthoritySource::ParentGrant {
            return Err(WorkspaceAuthorityError::InvalidSource);
        }
        let root = canonical_directory(root)?;
        let mut grants = self
            .grants
            .write()
            .map_err(|_| WorkspaceAuthorityError::RegistryUnavailable)?;
        let now = SystemTime::now();
        if expires_at.is_some_and(|expiry| expiry <= now) {
            return Err(WorkspaceAuthorityError::InvalidExpiry);
        }
        let id = unused_id(&grants);
        grants.insert(
            id,
            WorkspaceGrant {
                id,
                root,
                source,
                binding,
                permission,
                parent: None,
                issued_at: now,
                expires_at,
                revoked_at: None,
            },
        );
        Ok(id)
    }

    /// Resolve using the backend's authenticated execution binding. Does no
    /// filesystem I/O or mutation. Never cache this snapshot as authorization
    /// for subsequent operations: revocation/expiry requires a new resolution.
    pub fn resolve(
        &self,
        id: WorkspaceGrantId,
        binding: WorkspaceBinding,
    ) -> Result<WorkspaceGrant, WorkspaceAuthorityError> {
        let grants = self
            .grants
            .read()
            .map_err(|_| WorkspaceAuthorityError::RegistryUnavailable)?;
        Ok(active_grant(&grants, id, binding, SystemTime::now())?.clone())
    }

    /// Delegate to an agent in the same run, retaining or narrowing the root
    /// and permissions. The expiry is inherited unchanged. Only existing,
    /// absolute directories are accepted, including for same-root children.
    /// This backend API must receive the parent's authenticated binding.
    pub fn narrow(
        &self,
        parent_id: WorkspaceGrantId,
        parent_binding: WorkspaceBinding,
        child_agent_id: AgentId,
        root: &Path,
        permission: FsPermissionLevel,
    ) -> Result<WorkspaceGrantId, WorkspaceAuthorityError> {
        let binding = WorkspaceBinding {
            agent_id: child_agent_id,
            run_id: parent_binding.run_id,
        };
        validate_binding(binding)?;
        let mut grants = self
            .grants
            .write()
            .map_err(|_| WorkspaceAuthorityError::RegistryUnavailable)?;
        let parent = active_grant(&grants, parent_id, parent_binding, SystemTime::now())?;
        if !permission_is_subset(&permission, &parent.permission) {
            return Err(WorkspaceAuthorityError::PermissionWidening);
        }
        // Do not use workspace::resolve_path: it creates missing roots. Match
        // its canonical, component-based containment semantics without writes.
        let root = canonical_directory(root)?;
        if !root.starts_with(&parent.root) {
            return Err(WorkspaceAuthorityError::RootWidening);
        }
        // Filesystem resolution may take time. Recheck expiry before insertion;
        // the write lock excludes concurrent parent revocation throughout.
        let now = SystemTime::now();
        let expires_at = active_grant(&grants, parent_id, parent_binding, now)?.expires_at;
        let id = unused_id(&grants);
        grants.insert(
            id,
            WorkspaceGrant {
                id,
                root,
                source: WorkspaceAuthoritySource::ParentGrant,
                binding,
                permission,
                parent: Some(parent_id),
                issued_at: now,
                expires_at,
                revoked_at: None,
            },
        );
        Ok(id)
    }

    /// Revoke the bound grant. Descendants fail subsequent resolution/narrowing
    /// through their ancestor chain; other grants remain independent. Revoking
    /// an already revoked/expired grant is allowed, but never as another owner.
    pub fn revoke(
        &self,
        id: WorkspaceGrantId,
        binding: WorkspaceBinding,
    ) -> Result<(), WorkspaceAuthorityError> {
        let mut grants = self
            .grants
            .write()
            .map_err(|_| WorkspaceAuthorityError::RegistryUnavailable)?;
        let grant = grants
            .get_mut(&id)
            .ok_or(WorkspaceAuthorityError::UnknownGrant)?;
        verify_binding(grant, binding)?;
        grant.revoked_at.get_or_insert_with(SystemTime::now);
        Ok(())
    }
}

fn canonical_directory(root: &Path) -> Result<PathBuf, WorkspaceAuthorityError> {
    if !root.is_absolute() {
        return Err(WorkspaceAuthorityError::InvalidRoot);
    }
    let root = root
        .canonicalize()
        .map_err(|_| WorkspaceAuthorityError::InvalidRoot)?;
    if !root.is_dir() {
        return Err(WorkspaceAuthorityError::InvalidRoot);
    }
    Ok(root)
}

fn validate_binding(binding: WorkspaceBinding) -> Result<(), WorkspaceAuthorityError> {
    if binding.agent_id.is_nil() || binding.run_id.is_nil() {
        return Err(WorkspaceAuthorityError::InvalidBinding);
    }
    Ok(())
}

fn verify_binding(
    grant: &WorkspaceGrant,
    binding: WorkspaceBinding,
) -> Result<(), WorkspaceAuthorityError> {
    if grant.binding.agent_id != binding.agent_id {
        return Err(WorkspaceAuthorityError::WrongOwner);
    }
    if grant.binding.run_id != binding.run_id {
        return Err(WorkspaceAuthorityError::WrongContext);
    }
    Ok(())
}

fn active_grant(
    grants: &HashMap<WorkspaceGrantId, WorkspaceGrant>,
    id: WorkspaceGrantId,
    binding: WorkspaceBinding,
    now: SystemTime,
) -> Result<&WorkspaceGrant, WorkspaceAuthorityError> {
    let grant = grants
        .get(&id)
        .ok_or(WorkspaceAuthorityError::UnknownGrant)?;
    verify_binding(grant, binding)?;
    let mut ancestor = grant;
    loop {
        if ancestor.revoked_at.is_some() {
            return Err(WorkspaceAuthorityError::RevokedGrant);
        }
        if ancestor.expires_at.is_some_and(|expiry| expiry <= now) {
            return Err(WorkspaceAuthorityError::ExpiredGrant);
        }
        match ancestor.parent {
            Some(parent) => {
                ancestor = grants
                    .get(&parent)
                    .ok_or(WorkspaceAuthorityError::UnknownGrant)?;
            }
            None => return Ok(grant),
        }
    }
}

fn permission_is_subset(child: &FsPermissionLevel, parent: &FsPermissionLevel) -> bool {
    use FsPermissionLevel::{Deny, ReadOnly, ReadWrite};
    matches!(
        (child, parent),
        (Deny, _) | (ReadOnly, ReadOnly | ReadWrite) | (ReadWrite, ReadWrite)
    )
}

fn unused_id(grants: &HashMap<WorkspaceGrantId, WorkspaceGrant>) -> WorkspaceGrantId {
    loop {
        let id = WorkspaceGrantId(Uuid::new_v4());
        if !grants.contains_key(&id) {
            return id;
        }
    }
}

#[cfg(test)]
mod tests;
