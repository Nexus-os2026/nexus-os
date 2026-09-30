//! Backend-owned project registration (Phase One, decision D1).
//!
//! The owner chooses a project folder through a native folder picker that
//! the backend itself invokes ([`FolderPicker`]); no API accepts a path from
//! anyone else. The picked folder is converted at once into a canonical path
//! (the pick must already be its own canonical spelling, so a symlink or
//! other redirect anywhere in it is refused), a retained directory identity
//! and an opaque [`ProjectId`]. The frontend receives only the id and a
//! display name.
//!
//! Registration grants nothing by itself. Each coding run receives a fresh,
//! read-only, expiring `UserSelected` workspace grant bound to that run
//! ([`ProjectRegistry::grant_for_run`]), issued only after the folder still
//! has the identity it had when it was picked; the run then re-checks that
//! identity when it opens the folder. A moved or replaced folder fails. The
//! run owns that grant and revokes it as soon as it no longer needs project
//! read authority; the expiry is only a backstop.
//!
//! Registrations live only as long as the process. A [`ProjectId`] can be
//! displayed and parsed, but it has no serialized form that restores a
//! registration: after a restart the owner selects the folder again.
//!
//! ```compile_fail
//! // A path string cannot be registered: there is no such API.
//! let registry: nexus_kernel::coding_run::ProjectRegistry = todo!();
//! let _ = registry.register(std::path::PathBuf::from("/home/owner/project"));
//! ```
//!
//! ```compile_fail
//! // A project id has no deserializer that could restore authority.
//! let _: nexus_kernel::coding_run::ProjectId = serde_json::from_str("\"x\"").unwrap();
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use thiserror::Error;
use uuid::Uuid;

use super::fsops::{DirHandle, NodeIdentity};
use crate::manifest::FsPermissionLevel;
use crate::workspace_authority::{
    WorkspaceAuthorityRegistry, WorkspaceAuthoritySource, WorkspaceBinding, WorkspaceGrantId,
};

/// The longest a run's read grant on its project can live. The run revokes
/// the grant as soon as it no longer needs project read authority; this
/// expiry is only a backstop.
pub const RUN_GRANT_LIFETIME: Duration = Duration::from_secs(4 * 60 * 60);
/// How long an apply grant on a project lives.
pub const APPLY_GRANT_LIFETIME: Duration = Duration::from_secs(120);

const MAX_NAME_CHARS: usize = 64;

/// The backend's native folder picker. Only the desktop's native dialog
/// adapter implements it; the result is the owner's choice, not a caller's
/// string.
pub trait FolderPicker {
    /// Show the native picker; `None` if the owner cancelled.
    fn pick_folder(&self) -> Option<PathBuf>;
}

/// Opaque project name. Grants nothing; unknown to any other registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProjectId(Uuid);

impl ProjectId {
    /// Parse a project name received from the frontend. Grants nothing: it
    /// only names a registration that must still exist in this process.
    pub fn parse(text: &str) -> Option<Self> {
        Uuid::parse_str(text)
            .ok()
            .filter(|uuid| !uuid.is_nil())
            .map(Self)
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// What the frontend may see about a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInfo {
    pub id: ProjectId,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProjectError {
    #[error("no folder was selected")]
    Cancelled,
    #[error("the selected folder path is not absolute")]
    NotAbsolute,
    #[error("the selected folder is reached through a link; choose the real folder")]
    NotCanonical,
    #[error("the selection is not a folder")]
    NotADirectory,
    #[error("this folder cannot be used as a project")]
    Forbidden,
    #[error("the folder cannot be opened")]
    Unavailable,
    #[error("unknown project; select the folder again")]
    UnknownProject,
    #[error("the project folder was moved or replaced; select it again")]
    IdentityChanged,
    #[error("project authority could not be issued")]
    Authority,
}

#[derive(Debug)]
struct Registered {
    root: PathBuf,
    identity: NodeIdentity,
    name: String,
}

/// Process-lifetime registry of owner-selected projects.
#[derive(Debug)]
pub struct ProjectRegistry {
    authority: Arc<WorkspaceAuthorityRegistry>,
    /// Canonical locations no project may contain or lie within (the Nexus
    /// state directory).
    forbidden: Vec<PathBuf>,
    projects: Mutex<HashMap<ProjectId, Registered>>,
}

/// A run's authority over its project: a fresh workspace grant bound to the
/// run, and the directory identity the project had when it was selected.
#[derive(Debug)]
pub struct ProjectGrant {
    pub(crate) project: ProjectId,
    pub(crate) grant: WorkspaceGrantId,
    pub(crate) binding: WorkspaceBinding,
    pub(crate) identity: NodeIdentity,
    pub(crate) authority: Arc<WorkspaceAuthorityRegistry>,
}

impl ProjectGrant {
    pub fn project(&self) -> ProjectId {
        self.project
    }

    pub fn grant_id(&self) -> WorkspaceGrantId {
        self.grant
    }

    pub fn binding(&self) -> WorkspaceBinding {
        self.binding
    }
}

impl ProjectRegistry {
    /// A registry issuing grants from `authority`. No project may contain or
    /// lie within `nexus_state_dir`.
    pub fn new(authority: Arc<WorkspaceAuthorityRegistry>, nexus_state_dir: &Path) -> Self {
        let forbidden = std::fs::canonicalize(nexus_state_dir)
            .into_iter()
            .chain(std::iter::once(nexus_state_dir.to_path_buf()))
            .collect();
        Self {
            authority,
            forbidden,
            projects: Mutex::new(HashMap::new()),
        }
    }

    fn projects(&self) -> std::sync::MutexGuard<'_, HashMap<ProjectId, Registered>> {
        self.projects.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Invoke the native picker and register the owner's choice.
    pub fn select(&self, picker: &dyn FolderPicker) -> Result<ProjectInfo, ProjectError> {
        let picked = picker.pick_folder().ok_or(ProjectError::Cancelled)?;
        self.register(picked)
    }

    /// Register a natively picked folder. Private: only [`Self::select`]
    /// reaches it.
    fn register(&self, picked: PathBuf) -> Result<ProjectInfo, ProjectError> {
        if !picked.is_absolute() {
            return Err(ProjectError::NotAbsolute);
        }
        let canonical = std::fs::canonicalize(&picked).map_err(|_| ProjectError::Unavailable)?;
        if canonical != picked {
            return Err(ProjectError::NotCanonical);
        }
        if !std::fs::symlink_metadata(&canonical)
            .map_err(|_| ProjectError::Unavailable)?
            .is_dir()
        {
            return Err(ProjectError::NotADirectory);
        }
        if canonical.parent().is_none()
            || self
                .forbidden
                .iter()
                .any(|f| canonical.starts_with(f) || f.starts_with(&canonical))
        {
            return Err(ProjectError::Forbidden);
        }
        let identity = Self::identity_at(&canonical)?;
        let name = display_name(&canonical);
        let mut projects = self.projects();
        if let Some((id, existing)) = projects
            .iter()
            .find(|(_, p)| p.identity == identity && p.root == canonical)
        {
            return Ok(ProjectInfo {
                id: *id,
                name: existing.name.clone(),
            });
        }
        let id = ProjectId(Uuid::new_v4());
        projects.insert(
            id,
            Registered {
                root: canonical,
                identity,
                name: name.clone(),
            },
        );
        Ok(ProjectInfo { id, name })
    }

    /// The identity of the directory at `root`, which must still be its own
    /// canonical spelling and not a symlink.
    fn identity_at(root: &Path) -> Result<NodeIdentity, ProjectError> {
        let handle = DirHandle::open_absolute(root).map_err(|_| ProjectError::IdentityChanged)?;
        match std::fs::canonicalize(root) {
            Ok(canonical) if canonical == root => Ok(handle.identity()),
            _ => Err(ProjectError::IdentityChanged),
        }
    }

    pub fn info(&self, id: ProjectId) -> Option<ProjectInfo> {
        self.projects().get(&id).map(|p| ProjectInfo {
            id,
            name: p.name.clone(),
        })
    }

    pub fn list(&self) -> Vec<ProjectInfo> {
        let mut list: Vec<ProjectInfo> = self
            .projects()
            .iter()
            .map(|(id, p)| ProjectInfo {
                id: *id,
                name: p.name.clone(),
            })
            .collect();
        list.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.0.cmp(&b.id.0)));
        list
    }

    /// Forget a registration. Grants already issued stay governed by their
    /// own expiry and revocation.
    pub fn forget(&self, id: ProjectId) -> bool {
        self.projects().remove(&id).is_some()
    }

    /// A fresh read-only grant on the project for one run.
    pub fn grant_for_run(
        &self,
        id: ProjectId,
        binding: WorkspaceBinding,
    ) -> Result<ProjectGrant, ProjectError> {
        self.issue(id, binding, FsPermissionLevel::ReadOnly, RUN_GRANT_LIFETIME)
    }

    /// A fresh, short-lived write grant on the project for applying one
    /// approved candidate.
    pub fn grant_for_apply(
        &self,
        id: ProjectId,
        binding: WorkspaceBinding,
    ) -> Result<ProjectGrant, ProjectError> {
        self.issue(
            id,
            binding,
            FsPermissionLevel::ReadWrite,
            APPLY_GRANT_LIFETIME,
        )
    }

    fn issue(
        &self,
        id: ProjectId,
        binding: WorkspaceBinding,
        permission: FsPermissionLevel,
        lifetime: Duration,
    ) -> Result<ProjectGrant, ProjectError> {
        let (root, identity) = {
            let projects = self.projects();
            let project = projects.get(&id).ok_or(ProjectError::UnknownProject)?;
            (project.root.clone(), project.identity)
        };
        if Self::identity_at(&root)? != identity {
            return Err(ProjectError::IdentityChanged);
        }
        let grant = self
            .authority
            .issue_trusted_root(
                &root,
                binding,
                WorkspaceAuthoritySource::UserSelected,
                permission,
                Some(SystemTime::now() + lifetime),
            )
            .map_err(|_| ProjectError::Authority)?;
        Ok(ProjectGrant {
            project: id,
            grant,
            binding,
            identity,
            authority: Arc::clone(&self.authority),
        })
    }
}

/// The folder's own name, bounded and without control characters.
fn display_name(root: &Path) -> String {
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let cleaned: String = super::review::display_safe(&name, false)
        .chars()
        .take(MAX_NAME_CHARS)
        .collect();
    if cleaned.is_empty() {
        "project".to_string()
    } else {
        cleaned
    }
}
