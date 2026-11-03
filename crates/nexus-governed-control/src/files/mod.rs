//! M2d-B1: the broker's Files domain, create-only (design v2 §6.1-6.2,
//! amended by the Owner's D1 with Codex's conditions): in the one empty
//! folder the Owner registered in this process, an agent run admitted to a
//! mission the Owner consented to may ask to create one new small UTF-8
//! text file at the folder's top level. Nothing here replaces, deletes,
//! renames or makes a folder.
//!
//! - Workspace selection ([`Files::register`]): the Owner's own selection,
//!   from the front door only. Exactly `/`, `$HOME` or the identity home is
//!   refused; a reserved tree (overlap in either direction) is refused; the
//!   folder must exist, be its own canonical path (no link), be the current
//!   user's and be empty. One per process; nothing is persisted.
//! - Preparation ([`Files::prepare`]): the run's admission (never the
//!   intent), the admitted workspace still the same directory, a live Files
//!   grant on it, the name and content rules, the name absent, and the
//!   complete escaped display, refused whole if it cannot be shown.
//!
//! The kernel staging of the file (§6.1 step 4's
//! `create_for_project_observing` → `review` → `single_complete`) is not in
//! this build: the broker can reach neither a coding-run ledger nor a
//! workspace binding identity without the kernel's change (M2d-A), and the
//! kernel's guard P1A-NC-30 admits no production source outside the
//! primitive's own module. So the broker makes every §6.2 check itself and
//! registers nothing with the kernel yet, and a request that passes every
//! check above is refused, fail closed, before any commitment exists.
//!
//! Path validation is not an OS sandbox: these checks bound what the broker
//! asks the kernel for; they are not confinement.

#[cfg(test)]
mod tests;

use crate::authority::commitment::MAX_SUMMARY_LINES;
use crate::authority::effect::CapabilityKind;
use crate::authority::evidence::{hint, is_plain, quoted, wrapped};
use crate::authority::ids::{random16, Digest, GrantId, RunId};
use crate::authority::policy::GrantScope;
use crate::authority::run::{AdmittedWorkspace, FilesAdmission};
use crate::authority::{Authority, AuthorityError};
use crate::control::Preparation;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The largest file Files creates.
pub const MAX_FILE_BYTES: usize = 8 * 1024;
/// The most lines a created file may have.
pub const MAX_FILE_LINES: usize = 150;
/// The longest file name.
pub const MAX_NAME_CHARS: usize = 64;
/// The kernel's staging prefix: no file name may start with it.
const TEMP_PREFIX: &str = ".nexus-coding-run-tmp-";
/// The digest domain of a Files commitment's parameters (I9).
pub(crate) const PARAMETERS_DOMAIN: &str = "nexus.files.parameters.v1";
const IDENTITY_DOMAIN: &str = "nexus.files.workspace.identity.v1";

/// An agent's request to create one file, as data. Its `Debug` shows only
/// sizes: neither the name nor the content.
#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileCreate {
    pub name: String,
    pub content: String,
}

impl fmt::Debug for FileCreate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileCreate")
            .field("name_bytes", &self.name.len())
            .field("content_bytes", &self.content.len())
            .finish()
    }
}

/// What the interface and Mother's port may know of the workspace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
}

/// What a workspace may not be (§6.2): exactly `/`, `home` or
/// `identity_home`; within or containing any of `reserved`; or owned by
/// another user than `uid`.
pub(crate) struct Exclusions {
    pub(crate) home: PathBuf,
    pub(crate) identity_home: PathBuf,
    pub(crate) reserved: Vec<PathBuf>,
    pub(crate) uid: u32,
}

/// §6.1 step 3: `[A-Za-z0-9_-][A-Za-z0-9._-]{0,63}`, not the kernel's
/// staging prefix: one component at the workspace's top level.
pub(crate) fn name_ok(name: &str) -> bool {
    let first = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_NAME_CHARS
        && first(bytes[0])
        && bytes[1..].iter().all(|&b| first(b) || b == b'.')
        && !name.starts_with(TEMP_PREFIX)
}

/// The lines of `content`, as an editor counts them (a final newline ends
/// the last line; it does not start another).
pub(crate) fn line_count(content: &str) -> usize {
    content.split_inclusive('\n').count()
}

/// §6.1 step 3: text (UTF-8 by type, no NUL), at most `MAX_FILE_BYTES`
/// and `MAX_FILE_LINES`.
pub(crate) fn content_ok(content: &str) -> Result<(), AuthorityError> {
    if content.len() > MAX_FILE_BYTES {
        return Err(AuthorityError::InvalidAction(
            "the file is larger than 8 KiB",
        ));
    }
    if line_count(content) > MAX_FILE_LINES {
        return Err(AuthorityError::InvalidAction(
            "the file has more than 150 lines",
        ));
    }
    if content.contains('\0') {
        return Err(AuthorityError::InvalidAction("the file is not text"));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    hex::encode(Sha256::digest(bytes))
}

/// `wall_ms` as a UTC date and time (days to civil date, proleptic
/// Gregorian).
fn utc(wall_ms: u64) -> String {
    let secs = wall_ms / 1000;
    let days = i64::try_from(secs / 86_400).unwrap_or(i64::MAX / 2);
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// §6.1 step 4, what the Owner reads before approving: the mission's
/// contract hash in full, the consent's time, record and kind, the request,
/// the workspace's name and id, the name, the exact bytes and lines and the
/// full sha256, then the complete file, escaped and `│`-prefixed
/// ([`quoted`]). Refused whole (`change_too_large_to_show`) when it needs
/// more than `MAX_SUMMARY_LINES` lines or a line is not plain: never cut.
pub(crate) fn display(
    admission: &FilesAdmission,
    workspace: &WorkspaceInfo,
    name: &str,
    content: &str,
) -> Result<Vec<String>, AuthorityError> {
    let mut lines = vec![
        format!("Mission contract: {}", admission.contract_hash),
        format!(
            "Consented: {} (record {}, {})",
            utc(admission.consented_wall_ms),
            admission.evidence_seq,
            admission.kind.as_str()
        ),
    ];
    lines.extend(wrapped(&format!("Request: {}", admission.request)));
    lines.extend(wrapped(&format!(
        "Workspace: {} ({})",
        workspace.name, workspace.id
    )));
    lines.extend(wrapped(&format!(
        "File: {name} (new, at the workspace's top level)"
    )));
    lines.push(format!(
        "Size: {} bytes, {} lines; ends with a newline: {}",
        content.len(),
        line_count(content),
        if content.ends_with('\n') { "yes" } else { "no" }
    ));
    lines.push(format!("SHA-256: {}", sha256_hex(content.as_bytes())));
    lines.extend(quoted("New file, complete", content));
    if lines.len() > MAX_SUMMARY_LINES || !lines.iter().all(|line| is_plain(line)) {
        return Err(AuthorityError::ChangeTooLargeToShow);
    }
    Ok(lines)
}

/// What a Files commitment's parameters digest binds (I9): the admission
/// (every field), the broker run, the kernel coding run, the Files grant,
/// both halves of the kernel workspace binding, the full review hash and
/// the name.
pub(crate) struct Components<'a> {
    pub(crate) admission: &'a FilesAdmission,
    pub(crate) broker_run: RunId,
    pub(crate) kernel_run: [u8; 16],
    pub(crate) grant: GrantId,
    pub(crate) binding_agent: [u8; 16],
    pub(crate) binding_run: [u8; 16],
    pub(crate) review_hash: [u8; 32],
    pub(crate) name: &'a str,
}

/// The parameters digest of a Files commitment: a pure function of its
/// components, each length-prefixed under `PARAMETERS_DOMAIN`. (Its
/// consumer, the staged effect, is M2d-B2's.)
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn parameters_digest(components: &Components<'_>) -> Digest {
    let admission = components.admission;
    Digest::of(
        PARAMETERS_DOMAIN,
        &[
            admission.contract_hash.as_bytes(),
            admission.request.as_bytes(),
            &admission.evidence_seq.to_be_bytes(),
            admission.kind.as_str().as_bytes(),
            &admission.consented_wall_ms.to_be_bytes(),
            admission.workspace.id.as_bytes(),
            admission.workspace.identity.as_bytes(),
            components.broker_run.as_bytes(),
            &components.kernel_run,
            components.grant.as_bytes(),
            &components.binding_agent,
            &components.binding_run,
            &components.review_hash,
            components.name.as_bytes(),
        ],
    )
}

/// The registered workspace.
struct Workspace {
    info: WorkspaceInfo,
    root: PathBuf,
    identity: Digest,
}

/// The Files domain's state: at most one workspace per process.
pub(crate) struct Files {
    runtime_root: PathBuf,
    /// Serializes registrations (nothing else takes it).
    registering: Mutex<()>,
    workspace: Mutex<Option<Arc<Workspace>>>,
}

fn refused(why: &'static str) -> AuthorityError {
    AuthorityError::InvalidAction(why)
}

/// `path` and its canonical spelling, if it has one.
fn spellings(path: &Path) -> Vec<PathBuf> {
    let mut all = vec![path.to_path_buf()];
    if let Ok(canonical) = std::fs::canonicalize(path) {
        all.push(canonical);
    }
    all
}

/// The identity of the directory `root` names now: it must still be a real
/// directory, named by its own canonical path.
#[cfg(target_os = "linux")]
fn identity_of(root: &Path) -> Result<Digest, AuthorityError> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(root).map_err(|_| AuthorityError::TargetChanged)?;
    if !meta.is_dir() || std::fs::canonicalize(root).ok().as_deref() != Some(root) {
        return Err(AuthorityError::TargetChanged);
    }
    Ok(Digest::of(
        IDENTITY_DOMAIN,
        &[&meta.dev().to_be_bytes(), &meta.ino().to_be_bytes()],
    ))
}

#[cfg(not(target_os = "linux"))]
fn identity_of(_root: &Path) -> Result<Digest, AuthorityError> {
    Err(AuthorityError::Unavailable(
        "Files are available on Linux only",
    ))
}

/// §6.2: the broker's checks of the Owner's selection, in order: absolute,
/// its own canonical path (no link), a directory, not exactly `/`, `$HOME`
/// or the identity home (in either spelling), no overlap in either
/// direction with a reserved tree (in either spelling), the current user's,
/// empty. Returns the identity of the directory it checked.
#[cfg(target_os = "linux")]
fn selectable(
    path: &Path,
    exclusions: &Exclusions,
    runtime_root: &Path,
) -> Result<Digest, AuthorityError> {
    use std::os::unix::fs::MetadataExt;
    if !path.is_absolute() {
        return Err(refused("the workspace is not an absolute path"));
    }
    let canonical = std::fs::canonicalize(path)
        .map_err(|_| refused("the workspace folder cannot be opened"))?;
    if canonical != path {
        return Err(refused(
            "the workspace is reached through a link; choose the real folder",
        ));
    }
    let meta = std::fs::symlink_metadata(path)
        .map_err(|_| refused("the workspace folder cannot be opened"))?;
    if !meta.is_dir() {
        return Err(refused("the workspace is not a folder"));
    }
    let exact = [
        Path::new("/"),
        exclusions.home.as_path(),
        exclusions.identity_home.as_path(),
    ];
    if exact
        .into_iter()
        .flat_map(spellings)
        .any(|forbidden| forbidden == path)
    {
        return Err(refused("this folder cannot be the workspace"));
    }
    let reserved = exclusions
        .reserved
        .iter()
        .map(PathBuf::as_path)
        .chain(std::iter::once(runtime_root));
    if reserved
        .flat_map(spellings)
        .any(|tree| path.starts_with(&tree) || tree.starts_with(path))
    {
        return Err(refused("the workspace overlaps a reserved location"));
    }
    if meta.uid() != exclusions.uid {
        return Err(refused("the workspace folder is not the current user's"));
    }
    let mut entries =
        std::fs::read_dir(path).map_err(|_| refused("the workspace folder cannot be opened"))?;
    if entries.next().is_some() {
        return Err(refused("the workspace folder is not empty"));
    }
    Ok(Digest::of(
        IDENTITY_DOMAIN,
        &[&meta.dev().to_be_bytes(), &meta.ino().to_be_bytes()],
    ))
}

#[cfg(not(target_os = "linux"))]
fn selectable(_: &Path, _: &Exclusions, _: &Path) -> Result<Digest, AuthorityError> {
    Err(AuthorityError::Unavailable(
        "Files are available on Linux only",
    ))
}

impl Files {
    pub(crate) fn new(runtime_root: PathBuf) -> Self {
        Self {
            runtime_root,
            registering: Mutex::new(()),
            workspace: Mutex::new(None),
        }
    }

    fn current(&self) -> Option<Arc<Workspace>> {
        self.workspace.lock().expect("files").clone()
    }

    /// The registered workspace, if any.
    pub(crate) fn info(&self) -> Option<WorkspaceInfo> {
        self.current().map(|workspace| workspace.info.clone())
    }

    /// §6.2: register the Owner's selection as this process's one
    /// workspace: refused when one is registered or by any check of
    /// [`selectable`]; the directory must keep the identity it was checked
    /// with. Its id is backend-random (`wsp-` and 32 hex); its name is the
    /// folder's own, escaped and at most 64 characters.
    pub(crate) fn register(
        &self,
        path: &Path,
        exclusions: &Exclusions,
    ) -> Result<WorkspaceInfo, AuthorityError> {
        let _one = self.registering.lock().expect("files registration");
        if self.current().is_some() {
            return Err(refused("a workspace is already registered in this process"));
        }
        let identity = selectable(path, exclusions, &self.runtime_root)?;
        if identity_of(path)? != identity {
            return Err(AuthorityError::TargetChanged);
        }
        let name = path
            .file_name()
            .map(|name| hint(&name.to_string_lossy()))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "workspace".to_string());
        let info = WorkspaceInfo {
            id: format!("wsp-{}", hex::encode(random16())),
            name,
        };
        *self.workspace.lock().expect("files") = Some(Arc::new(Workspace {
            info: info.clone(),
            root: path.to_path_buf(),
            identity,
        }));
        Ok(info)
    }

    /// The registered workspace, for a door's admission: still the same
    /// directory (checked now, with no lock held).
    pub(crate) fn admissible(&self) -> Result<AdmittedWorkspace, AuthorityError> {
        let workspace = self
            .current()
            .ok_or(AuthorityError::Unavailable("no workspace is registered"))?;
        if identity_of(&workspace.root)? != workspace.identity {
            return Err(AuthorityError::TargetChanged);
        }
        Ok(AdmittedWorkspace {
            id: workspace.info.id.clone(),
            identity: workspace.identity,
        })
    }

    /// The canonical scope of a Files grant on the registered workspace
    /// `id`, its identity checked now.
    pub(crate) fn grant_scope(&self, id: &str) -> Result<GrantScope, AuthorityError> {
        let workspace = self
            .current()
            .ok_or(AuthorityError::Unavailable("no workspace is registered"))?;
        if workspace.info.id != id {
            return Err(refused("no such workspace"));
        }
        if identity_of(&workspace.root)? != workspace.identity {
            return Err(AuthorityError::TargetChanged);
        }
        Ok(GrantScope::Files {
            workspace: workspace.info.id.clone(),
            root: workspace.root.to_string_lossy().into_owned(),
            identity: workspace.identity,
        })
    }

    /// §6.1 step 4, the broker's half of a create's preparation, in order:
    /// the run's admission (never the intent's), the admitted workspace
    /// still the registered one and the same directory, a live Files grant
    /// on it, the name, the content, the name absent, and the complete
    /// display. No lock is held across filesystem work; nothing is written.
    pub(crate) fn prepare(
        &self,
        authority: &Authority,
        run: RunId,
        create: &FileCreate,
    ) -> Result<Preparation, AuthorityError> {
        let admission = authority
            .runs()
            .files_admission(run)
            .ok_or(AuthorityError::NotAdmitted)?;
        let workspace = self
            .current()
            .ok_or(AuthorityError::Unavailable("no workspace is registered"))?;
        if workspace.info.id != admission.workspace.id
            || workspace.identity != admission.workspace.identity
            || identity_of(&workspace.root)? != admission.workspace.identity
        {
            return Err(AuthorityError::TargetChanged);
        }
        let _grant = covering_grant(authority, &admission.workspace)?;
        if !name_ok(&create.name) {
            return Err(refused("the file name is not allowed"));
        }
        content_ok(&create.content)?;
        match std::fs::symlink_metadata(workspace.root.join(&create.name)) {
            Ok(_) => return Err(AuthorityError::FileExists),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(AuthorityError::Unavailable("the workspace cannot be read")),
        }
        let _shown = display(&admission, &workspace.info, &create.name, &create.content)?;
        // The kernel staging (§6.1 step 4) needs M2d-A's change: fail closed.
        Err(AuthorityError::Unavailable(
            "file staging is not available in this build",
        ))
    }
}

/// The live Files grant on `workspace` (the lowest id, if several).
fn covering_grant(
    authority: &Authority,
    workspace: &AdmittedWorkspace,
) -> Result<GrantId, AuthorityError> {
    authority
        .grants()
        .live_of(CapabilityKind::Files)
        .into_iter()
        .find(|grant| {
            matches!(
                &grant.scope,
                GrantScope::Files { workspace: id, identity, .. }
                    if *id == workspace.id && *identity == workspace.identity
            )
        })
        .map(|grant| grant.id)
        .ok_or(AuthorityError::NoCoveringGrant)
}
