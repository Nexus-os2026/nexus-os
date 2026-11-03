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
//! Overlap with a protected or reserved location is decided by directory
//! identity (`st_dev`, `st_ino`) along each directory's ancestor chain, not
//! by its spelling (Codex review 2 of PR #57): a bind mount or a casefolded
//! name gives a directory a second canonical spelling, never a second
//! identity. The spelling checks stay as defense in depth. A location
//! reached through a mount that is not a whole filesystem (a subtree bind
//! mount, whose root in `/proc/self/mountinfo` is not `/`) has ancestors no
//! `..` walk visits, so it is refused as unsupported topology (Codex review
//! 3 of PR #57), as it is when the mount table cannot be read, parsed or
//! matched; the mount table is read at selection. Not claimed: a
//! mount, link, rename or casefold change made after the selection (by the
//! Owner, root or any other process), or an inode number reused after its
//! directory is deleted.
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

/// `path`'s canonical spelling (`std::fs::canonicalize`). Tests can make a
/// directory reachable under a second canonical spelling, as a bind mount
/// or a case-insensitive name makes it ([`mount_alias_for_test`]).
fn canonical(path: &Path) -> std::io::Result<PathBuf> {
    #[cfg(test)]
    if let Some(alias) = MOUNTED.with(|mounted| {
        mounted
            .borrow()
            .iter()
            .find(|alias| path.starts_with(alias))
            .cloned()
    }) {
        let target = std::fs::canonicalize(&alias)?;
        let below = path.strip_prefix(&alias).unwrap_or(Path::new(""));
        let real = std::fs::canonicalize(target.join(below))?;
        return Ok(match real.strip_prefix(&target) {
            Ok(rest) if rest.as_os_str().is_empty() => alias,
            Ok(rest) => alias.join(rest),
            Err(_) => real,
        });
    }
    std::fs::canonicalize(path)
}

#[cfg(test)]
thread_local! {
    /// Tests only: symlinks this thread treats as a second canonical
    /// spelling of the directory they name (an unprivileged stand-in for a
    /// bind mount or a casefolded name): the link's own spelling is kept by
    /// [`canonical`], while opening it reaches the same directory.
    static MOUNTED: std::cell::RefCell<Vec<PathBuf>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Tests only: treat the symlink at `link` (an absolute, canonical-parent
/// path) as a second canonical spelling of its directory, on this thread.
#[cfg(test)]
pub(crate) fn mount_alias_for_test(link: PathBuf) {
    MOUNTED.with(|mounted| mounted.borrow_mut().push(link));
}

/// One mount, as `/proc/self/mountinfo` describes it: its device
/// (`major:minor`), its root within that filesystem (field 4) and its mount
/// point (field 5), both unescaped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Mount {
    pub(crate) major: u64,
    pub(crate) minor: u64,
    pub(crate) root: std::ffi::OsString,
    pub(crate) mount_point: PathBuf,
}

/// `st_dev` as (`major`, `minor`), in the encoding Linux and glibc use.
#[cfg(target_os = "linux")]
fn split_dev(dev: u64) -> (u64, u64) {
    let major = ((dev >> 8) & 0xfff) | ((dev >> 32) & !0xfff);
    let minor = (dev & 0xff) | ((dev >> 12) & !0xff);
    (major, minor)
}

/// A mountinfo field as the kernel escapes it (space, tab, newline and
/// backslash as `\ooo`, three octal digits), unescaped; `None` if an
/// escape is malformed.
fn unescape(field: &[u8]) -> Option<std::ffi::OsString> {
    use std::os::unix::ffi::OsStringExt;
    let mut out = Vec::with_capacity(field.len());
    let mut at = 0;
    while at < field.len() {
        if field[at] == b'\\' {
            let digits = field.get(at + 1..at + 4)?;
            if !digits.iter().all(|d| (b'0'..=b'7').contains(d)) {
                return None;
            }
            let value = digits
                .iter()
                .fold(0u32, |value, d| value * 8 + u32::from(d - b'0'));
            out.push(u8::try_from(value).ok()?);
            at += 4;
        } else {
            out.push(field[at]);
            at += 1;
        }
    }
    Some(std::ffi::OsString::from_vec(out))
}

/// Every mount `/proc/self/mountinfo`'s `text` lists (proc(5): mount id,
/// parent id, `major:minor`, root, mount point, options, optional fields up
/// to `-`, then the filesystem type and source). `None` if any line is
/// malformed or none is listed: callers refuse (fail closed).
fn parse_mountinfo(text: &[u8]) -> Option<Vec<Mount>> {
    let number = |field: &[u8]| std::str::from_utf8(field).ok()?.parse::<u64>().ok();
    let mut mounts = Vec::new();
    for line in text.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        let fields: Vec<&[u8]> = line.split(|b| *b == b' ').collect();
        let separator = 6 + fields.get(6..)?.iter().position(|field| *field == b"-")?;
        if fields.len() < separator + 3 {
            return None;
        }
        number(fields[0])?;
        number(fields[1])?;
        let (major, minor) = std::str::from_utf8(fields[2]).ok()?.split_once(':')?;
        let mount_point = PathBuf::from(unescape(fields[4])?);
        if !mount_point.is_absolute() {
            return None;
        }
        mounts.push(Mount {
            major: major.parse().ok()?,
            minor: minor.parse().ok()?,
            root: unescape(fields[3])?,
            mount_point,
        });
    }
    (!mounts.is_empty()).then_some(mounts)
}

/// The mount table this process sees.
fn mountinfo() -> Option<Vec<u8>> {
    #[cfg(test)]
    if let Some(given) = MOUNTINFO.with(|mountinfo| mountinfo.borrow().clone()) {
        return given.ok().map(String::into_bytes);
    }
    std::fs::read("/proc/self/mountinfo").ok()
}

/// The mount that contains the directory at `path` (canonical): of the
/// mounts with the path's `major:minor`, the one whose mount point is the
/// longest prefix of the path (the later one, mounted on top, when two
/// share it). Refused (`unsupported_mount_topology`) if the path cannot be
/// stated, the mount table cannot be read or parsed, or no mount matches.
#[cfg(target_os = "linux")]
pub(crate) fn mount_of(path: &Path) -> Result<Mount, AuthorityError> {
    use std::os::unix::fs::MetadataExt;
    let unsupported = || AuthorityError::UnsupportedMountTopology;
    let (major, minor) = split_dev(std::fs::metadata(path).map_err(|_| unsupported())?.dev());
    let mounts = mountinfo()
        .and_then(|text| parse_mountinfo(&text))
        .ok_or_else(unsupported)?;
    let mut found: Option<&Mount> = None;
    for mount in mounts
        .iter()
        .filter(|m| m.major == major && m.minor == minor && path.starts_with(&m.mount_point))
    {
        let depth = mount.mount_point.components().count();
        if found.is_none_or(|best| depth >= best.mount_point.components().count()) {
            found = Some(mount);
        }
    }
    found.cloned().ok_or_else(unsupported)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn mount_of(_path: &Path) -> Result<Mount, AuthorityError> {
    Err(AuthorityError::UnsupportedMountTopology)
}

/// The directory at `path` is reached through a whole-filesystem mount
/// (root `/`), or refused as unsupported topology.
fn whole_filesystem(path: &Path) -> Result<(), AuthorityError> {
    if mount_of(path)?.root == std::ffi::OsStr::new("/") {
        Ok(())
    } else {
        Err(AuthorityError::UnsupportedMountTopology)
    }
}

#[cfg(test)]
thread_local! {
    /// Tests only: the text this thread reads in place of
    /// `/proc/self/mountinfo` (`Err`: unreadable), when set.
    static MOUNTINFO: std::cell::RefCell<Option<Result<String, ()>>> =
        const { std::cell::RefCell::new(None) };
}

/// Tests only: read `text` in place of `/proc/self/mountinfo` on this
/// thread (`Some(Err(()))`: it cannot be read; `None`: the real one).
#[cfg(test)]
pub(crate) fn mountinfo_for_test(text: Option<Result<String, ()>>) {
    MOUNTINFO.with(|mountinfo| *mountinfo.borrow_mut() = text);
}

/// Where a protected or reserved location is: its longest existing
/// ancestor, canonical (`existing`), the location itself (`real`: `existing`
/// with the missing components appended), and whether it exists completely
/// (`existing == real`).
struct Located {
    existing: PathBuf,
    real: PathBuf,
    complete: bool,
}

/// Where `path` really is (Codex review 1 of PR #57): its longest existing
/// ancestor resolved canonically (links included), with the components
/// below it appended, none of which exists yet. `None` when that cannot be
/// established: a relative path; the first missing component present after
/// all (a dangling or looping link, an entry that cannot be read); or a `.`
/// or `..` below the existing part. A caller refuses on `None`.
fn established(path: &Path) -> Option<Located> {
    use std::path::Component;
    if !path.is_absolute() {
        return None;
    }
    let mut missing = Vec::new();
    let mut existing = path;
    loop {
        if let Ok(canonical) = canonical(existing) {
            if let Some(first) = missing.last() {
                match std::fs::symlink_metadata(canonical.join(first)) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    _ => return None,
                }
            }
            let mut real = canonical.clone();
            for part in missing.iter().rev() {
                real.push(part);
            }
            return Some(Located {
                existing: canonical,
                real,
                complete: missing.is_empty(),
            });
        }
        match existing.components().next_back()? {
            Component::Normal(part) => missing.push(part),
            _ => return None,
        }
        existing = existing.parent()?;
    }
}

/// Most directories an ancestry walk visits.
#[cfg(target_os = "linux")]
const MAX_ANCESTRY: usize = 4096;

/// The identities (`st_dev`, `st_ino`) of the directory `dir` opens and of
/// every directory above it, walking `..` from the open directory itself
/// (`/proc/self/fd/<fd>/..`, as `openat(fd, "..")` does) up to the root,
/// whose `..` is itself. `None` if an open or a stat fails, an entry is not
/// a directory, or the walk does not end: callers refuse (fail closed).
#[cfg(target_os = "linux")]
fn ancestry(dir: &Path) -> Option<Vec<(u64, u64)>> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let open = |path: &Path| {
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_PATH | libc::O_DIRECTORY)
            .open(path)
            .ok()
    };
    let mut current = open(dir)?;
    let mut chain: Vec<(u64, u64)> = Vec::new();
    for _ in 0..MAX_ANCESTRY {
        let meta = current.metadata().ok()?;
        if !meta.is_dir() {
            return None;
        }
        let id = (meta.dev(), meta.ino());
        if chain.last() == Some(&id) {
            return Some(chain);
        }
        if chain.contains(&id) {
            return None;
        }
        chain.push(id);
        let parent = format!("/proc/self/fd/{}/..", current.as_raw_fd());
        current = open(Path::new(&parent))?;
    }
    None
}

/// The identity of the directory `root` names now: it must still be a real
/// directory, named by its own canonical path.
#[cfg(target_os = "linux")]
fn identity_of(root: &Path) -> Result<Digest, AuthorityError> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(root).map_err(|_| AuthorityError::TargetChanged)?;
    if !meta.is_dir() || canonical(root).ok().as_deref() != Some(root) {
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
/// its own canonical path (no link), a directory; not `/`, `$HOME` or the
/// identity home, and no overlap in either direction with a reserved tree,
/// each decided by directory identity ([`ancestry`]: the workspace is a
/// protected location if their identities are equal; a reserved location
/// is inside or equal to the workspace if the workspace's identity is in
/// the ancestor-or-self chain of the location's longest existing ancestor;
/// the workspace is inside or equal to a reserved location that exists
/// completely if the location's identity is in the workspace's chain), and
/// by spelling too (as given, or where it really is, [`established`]); a
/// location that cannot be established or opened refuses; the current
/// user's; empty. Returns the identity of the directory it checked.
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
    let canonical =
        canonical(path).map_err(|_| refused("the workspace folder cannot be opened"))?;
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
    let unresolved = || refused("a protected or reserved location cannot be resolved");
    // The workspace and every directory above it, by identity, on a
    // whole-filesystem mount.
    let workspace = ancestry(path).ok_or_else(unresolved)?;
    whole_filesystem(path)?;
    let exact = [
        Path::new("/"),
        exclusions.home.as_path(),
        exclusions.identity_home.as_path(),
    ];
    for forbidden in exact {
        let located = established(forbidden).ok_or_else(unresolved)?;
        whole_filesystem(&located.existing)?;
        let same = located.complete
            && ancestry(&located.existing).ok_or_else(unresolved)?[0] == workspace[0];
        if same || forbidden == path || located.real == path {
            return Err(refused("this folder cannot be the workspace"));
        }
    }
    let reserved = exclusions
        .reserved
        .iter()
        .map(PathBuf::as_path)
        .chain(std::iter::once(runtime_root));
    for tree in reserved {
        let located = established(tree).ok_or_else(unresolved)?;
        whole_filesystem(&located.existing)?;
        let above = ancestry(&located.existing).ok_or_else(unresolved)?;
        let inside_workspace = above.contains(&workspace[0]);
        let contains_workspace = located.complete && workspace.contains(&above[0]);
        let by_spelling = [tree, located.real.as_path()]
            .iter()
            .any(|tree| path.starts_with(tree) || tree.starts_with(path));
        if inside_workspace || contains_workspace || by_spelling {
            return Err(refused("the workspace overlaps a reserved location"));
        }
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
