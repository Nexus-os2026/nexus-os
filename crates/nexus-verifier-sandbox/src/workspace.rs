//! The verification workspace (Linux).
//!
//! Each verifier execution gets a fresh, backend-owned tree under the user
//! runtime directory, which is derived from this process's real uid, never
//! from `$XDG_RUNTIME_DIR` or `$HOME`:
//!
//! ```text
//! /run/user/<uid>/nexus-verifier/ws-<128-bit random>/
//!     input/       the materialized candidate; the verifier may only read it
//!     scratch/     verifier-writable state
//!     home/        the verifier's HOME
//!     tmp/         its TMPDIR
//!     cargo-home/  an empty private CARGO_HOME
//!     target/      build output; the verifier may also execute from it
//! ```
//!
//! The runtime directory is reached from `/` one component at a time without
//! following symlinks: `/run` and `/run/user` must be root-owned and writable
//! by no one else, and `/run/user/<uid>` must be a tmpfs directory owned by
//! the uid with mode 0700. `nexus-verifier` is opened or created owner-only
//! and must be the same; each workspace and its six areas are created
//! exclusively, owner-only, on that filesystem, and retained by descriptor.
//! Paths are derived only for the verifier's environment, and each must
//! still resolve to its retained directory before a launch.
//!
//! Removal is identity-bound and explicit. It works only through retained
//! descriptors, never follows a symlink and never enters another filesystem.
//! It removes whatever the verifier left in its areas: names that are not
//! UTF-8, directories without permissions and any nesting depth (a subtree
//! deeper than a fixed bound is moved up into the workspace and removed from
//! there, so the number of open descriptors stays bounded). A directory is
//! removed by name only while that name still refers to the retained
//! directory, and its removal is confirmed through the retained descriptor
//! (a removed directory has no links). A removal that cannot be confirmed
//! keeps the workspace as a retained boundary for a retry; dropping a
//! workspace that was never removed is only a backstop. Not a claim: a
//! same-uid process racing a rename into the instant between the identity
//! check and the removal is not excluded.

use std::ffi::{CStr, CString};
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use crate::policy::Role;
use crate::sys;

const TMPFS_MAGIC: i64 = 0x0102_1994;
/// The directory under the user runtime directory holding the workspaces.
pub const WORKSPACES: &str = "nexus-verifier";
/// Subtrees deeper than this are moved up before they are removed.
const MAX_REMOVAL_DEPTH: usize = 32;

/// One area of a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    Input,
    Scratch,
    Home,
    Tmp,
    CargoHome,
    Target,
}

impl Area {
    pub const ALL: [Area; 6] = [
        Area::Input,
        Area::Scratch,
        Area::Home,
        Area::Tmp,
        Area::CargoHome,
        Area::Target,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Scratch => "scratch",
            Self::Home => "home",
            Self::Tmp => "tmp",
            Self::CargoHome => "cargo-home",
            Self::Target => "target",
        }
    }

    fn c_name(self) -> &'static CStr {
        match self {
            Self::Input => c"input",
            Self::Scratch => c"scratch",
            Self::Home => c"home",
            Self::Tmp => c"tmp",
            Self::CargoHome => c"cargo-home",
            Self::Target => c"target",
        }
    }

    /// The sandbox role the verifier holds beneath this area.
    pub fn role(self) -> Role {
        match self {
            Self::Input => Role::CandidateInput,
            Self::Target => Role::Target,
            Self::Scratch | Self::Home | Self::Tmp | Self::CargoHome => Role::Scratch,
        }
    }
}

#[derive(Debug)]
pub enum WorkspaceError {
    /// The user runtime directory or the workspaces directory is missing or
    /// is not what it must be: verification is unavailable.
    Unavailable(&'static str),
    /// A workspace could not be created; anything created was removed.
    Create(io::Error),
    /// A workspace could not be created and what was created could not be
    /// removed; it is retained.
    CreateCleanupFailed(RetainedWorkspace),
    /// A derived path no longer resolves to its retained directory.
    PathChanged(Area),
    Io(io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Identity {
    dev: u64,
    ino: u64,
}

impl Identity {
    fn of(st: &libc::stat) -> Self {
        Self {
            dev: st.st_dev,
            ino: st.st_ino,
        }
    }
}

/// A directory retained by descriptor, with its identity.
#[derive(Debug)]
struct Dir {
    fd: OwnedFd,
    identity: Identity,
}

impl Dir {
    fn open_at(parent: BorrowedFd<'_>, name: &CStr) -> io::Result<Self> {
        let fd = sys::open_dir_at(parent, name)?;
        let identity = Identity::of(&sys::fstat(fd.as_fd())?);
        Ok(Self { fd, identity })
    }

    fn dup(&self) -> io::Result<Self> {
        Ok(Self {
            fd: self.fd.try_clone()?,
            identity: self.identity,
        })
    }

    /// Whether the directory has been removed (it has no links left).
    fn removed(&self) -> io::Result<bool> {
        Ok(sys::fstat(self.fd.as_fd())?.st_nlink == 0)
    }

    fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

fn not_owned(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, reason)
}

/// An owner-only directory of this uid on `dev`.
fn check_private(dir: &Dir, uid: u32, dev: u64) -> io::Result<()> {
    let st = sys::fstat(dir.fd())?;
    if st.st_uid != uid || st.st_mode & 0o7777 != 0o700 || st.st_dev != dev {
        return Err(not_owned("not an owner-only directory of this user"));
    }
    Ok(())
}

/// `/run/user/<uid>/nexus-verifier`, retained.
#[derive(Debug)]
pub struct WorkspaceRoot {
    uid: u32,
    path: PathBuf,
    dir: Dir,
}

impl WorkspaceRoot {
    /// Derive and retain the workspaces directory of this process's real
    /// uid, creating it owner-only if it is missing.
    pub fn derive() -> Result<Self, WorkspaceError> {
        use WorkspaceError::Unavailable;
        // SAFETY: getuid has no preconditions.
        let uid = unsafe { libc::getuid() };
        let slash = sys::open_fixed(c"/", libc::O_RDONLY | libc::O_DIRECTORY)
            .map_err(|_| Unavailable("filesystem root"))?;
        let mut parent = slash;
        for name in [c"run", c"user"] {
            let dir = Dir::open_at(parent.as_fd(), name).map_err(|_| Unavailable("/run/user"))?;
            let st = sys::fstat(dir.fd()).map_err(|_| Unavailable("/run/user"))?;
            if st.st_uid != 0 || st.st_mode & 0o022 != 0 {
                return Err(Unavailable("/run/user is not root-owned"));
            }
            parent = dir.fd;
        }
        let uid_name = CString::new(uid.to_string()).map_err(|_| Unavailable("uid"))?;
        let runtime = Dir::open_at(parent.as_fd(), &uid_name)
            .map_err(|_| Unavailable("no user runtime directory"))?;
        let st = sys::fstat(runtime.fd()).map_err(|_| Unavailable("user runtime directory"))?;
        if st.st_uid != uid || st.st_mode & 0o7777 != 0o700 {
            return Err(Unavailable("user runtime directory is not private"));
        }
        if sys::filesystem_type(runtime.fd()).ok() != Some(TMPFS_MAGIC) {
            return Err(Unavailable("user runtime directory is not a tmpfs"));
        }
        let name = c"nexus-verifier";
        match sys::mkdir_at(runtime.fd(), name, 0o700) {
            Ok(()) => {}
            Err(error) if error.raw_os_error() == Some(libc::EEXIST) => {}
            Err(_) => return Err(Unavailable("workspaces directory")),
        }
        let dir =
            Dir::open_at(runtime.fd(), name).map_err(|_| Unavailable("workspaces directory"))?;
        check_private(&dir, uid, runtime.identity.dev)
            .map_err(|_| Unavailable("workspaces directory is not private"))?;
        Ok(Self {
            uid,
            path: PathBuf::from(format!("/run/user/{uid}/{WORKSPACES}")),
            dir,
        })
    }
}

/// One execution's workspace, retained by descriptor.
#[derive(Debug)]
pub struct Workspace {
    uid: u32,
    root: Dir,
    name: CString,
    path: PathBuf,
    dir: Dir,
    areas: Vec<(Area, Dir)>,
    removed: bool,
}

impl Workspace {
    /// Create a fresh workspace and its six areas, exclusively and
    /// owner-only.
    pub fn create(root: &WorkspaceRoot) -> Result<Self, WorkspaceError> {
        let name = format!("ws-{}", sys::random_hex(16).map_err(WorkspaceError::Io)?);
        let c_name = CString::new(name.clone()).map_err(|_| WorkspaceError::Unavailable("name"))?;
        sys::mkdir_at(root.dir.fd(), &c_name, 0o700).map_err(WorkspaceError::Create)?;
        // A directory this call created but cannot open has no retained
        // identity; it is left empty and is never removed by name.
        let dir = Dir::open_at(root.dir.fd(), &c_name).map_err(WorkspaceError::Create)?;
        let mut workspace = Self {
            uid: root.uid,
            root: root.dir.dup().map_err(WorkspaceError::Io)?,
            name: c_name,
            path: root.path.join(&name),
            dir,
            areas: Vec::new(),
            removed: false,
        };
        let created = check_private(&workspace.dir, root.uid, root.dir.identity.dev)
            .and_then(|()| workspace.create_areas());
        match created {
            Ok(()) => Ok(workspace),
            Err(error) => match workspace.remove() {
                Ok(()) => Err(WorkspaceError::Create(error)),
                Err(retained) => Err(WorkspaceError::CreateCleanupFailed(retained)),
            },
        }
    }

    fn create_areas(&mut self) -> io::Result<()> {
        for area in Area::ALL {
            sys::mkdir_at(self.dir.fd(), area.c_name(), 0o700)?;
            let dir = Dir::open_at(self.dir.fd(), area.c_name())?;
            check_private(&dir, self.uid, self.dir.identity.dev)?;
            self.areas.push((area, dir));
        }
        Ok(())
    }

    fn area(&self, area: Area) -> io::Result<&Dir> {
        self.areas
            .iter()
            .find(|(candidate, _)| *candidate == area)
            .map(|(_, dir)| dir)
            .ok_or_else(|| io::Error::from_raw_os_error(libc::ENOENT))
    }

    /// A descriptor of the area's retained directory.
    pub fn directory(&self, area: Area) -> io::Result<OwnedFd> {
        self.area(area)?.fd.try_clone()
    }

    /// The area's path, for the verifier's environment only. It locates the
    /// retained directory (see [`Self::verify_paths`]) and grants nothing.
    pub fn env_path(&self, area: Area) -> PathBuf {
        self.path.join(area.name())
    }

    /// Each area's retained directory with the role the verifier holds
    /// beneath it.
    pub fn rules(&self) -> io::Result<Vec<(Role, OwnedFd)>> {
        self.areas
            .iter()
            .map(|(area, dir)| Ok((area.role(), dir.fd.try_clone()?)))
            .collect()
    }

    /// Confirm every area's path still resolves, without following a
    /// symlink, to exactly its retained directory.
    pub fn verify_paths(&self) -> Result<(), WorkspaceError> {
        for (area, dir) in &self.areas {
            match resolve_nofollow(&self.env_path(*area)) {
                Ok(identity) if identity == dir.identity => {}
                _ => return Err(WorkspaceError::PathChanged(*area)),
            }
        }
        Ok(())
    }

    /// Remove the workspace (see the module documentation).
    pub fn remove(mut self) -> Result<(), RetainedWorkspace> {
        match self.remove_all() {
            Ok(()) => {
                self.removed = true;
                Ok(())
            }
            Err(error) => Err(RetainedWorkspace {
                workspace: Box::new(self),
                error,
            }),
        }
    }

    fn remove_all(&self) -> io::Result<()> {
        for (area, dir) in &self.areas {
            empty(dir, &self.dir)?;
            remove_dir_if_identity(self.dir.fd(), area.c_name(), dir)?;
        }
        // Moved-up subtrees and anything never retained.
        empty(&self.dir, &self.dir)?;
        remove_dir_if_identity(self.root.fd(), &self.name, &self.dir)
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        if !self.removed {
            let _ = self.remove_all();
        }
    }
}

/// A workspace whose removal could not be confirmed.
#[derive(Debug)]
pub struct RetainedWorkspace {
    workspace: Box<Workspace>,
    error: io::Error,
}

impl RetainedWorkspace {
    pub fn error(&self) -> &io::Error {
        &self.error
    }

    /// Try the removal again.
    pub fn retry(self) -> Result<(), RetainedWorkspace> {
        (*self.workspace).remove()
    }
}

/// The identity `path` resolves to from `/`, opening each component as a
/// directory without following a symlink.
fn resolve_nofollow(path: &Path) -> io::Result<Identity> {
    let mut dir = sys::open_fixed(c"/", libc::O_RDONLY | libc::O_DIRECTORY)?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                let name = CString::new(name.as_bytes())
                    .map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
                dir = sys::open_dir_at(dir.as_fd(), &name)?;
            }
            _ => return Err(io::Error::from_raw_os_error(libc::EINVAL)),
        }
    }
    Ok(Identity::of(&sys::fstat(dir.as_fd())?))
}

/// Remove the empty directory `name` beneath `parent` only while that name
/// still refers to `dir`, and confirm through `dir` that it is gone.
fn remove_dir_if_identity(parent: BorrowedFd<'_>, name: &CStr, dir: &Dir) -> io::Result<()> {
    if dir.removed()? {
        return Ok(());
    }
    let st = sys::stat_at(parent, name)?;
    if sys::file_kind(&st) != libc::S_IFDIR || Identity::of(&st) != dir.identity {
        return Err(io::Error::other(
            "the name no longer refers to the directory",
        ));
    }
    sys::unlink_at(parent, name, true)?;
    if dir.removed()? {
        Ok(())
    } else {
        Err(io::Error::other("the directory was not removed"))
    }
}

struct Frame {
    dir: Dir,
    name: Option<CString>,
}

enum Step {
    /// The directory on top is empty.
    Emptied,
    /// Descend into this subdirectory.
    Descend(Frame),
    /// Entries were removed or moved; list again.
    Continue,
}

/// Remove everything beneath `top` through descriptors only. A subdirectory
/// found at the depth bound is moved up into `spill` under a fresh name, to
/// be removed from there.
fn empty(top: &Dir, spill: &Dir) -> io::Result<()> {
    let mut stack = vec![Frame {
        dir: top.dup()?,
        name: None,
    }];
    loop {
        let Some(frame) = stack.last() else {
            return Ok(());
        };
        match step(frame, spill, stack.len())? {
            Step::Continue => {}
            Step::Descend(child) => stack.push(child),
            Step::Emptied => {
                let done = stack.pop().expect("the frame just inspected");
                if let (Some(name), Some(parent)) = (done.name, stack.last()) {
                    remove_dir_if_identity(parent.dir.fd(), &name, &done.dir)?;
                }
            }
        }
    }
}

fn step(frame: &Frame, spill: &Dir, depth: usize) -> io::Result<Step> {
    let names = sys::directory_batch(frame.dir.fd())?;
    if names.is_empty() {
        return Ok(Step::Emptied);
    }
    for name in names {
        let st = match sys::stat_at(frame.dir.fd(), &name) {
            Ok(st) => st,
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => continue,
            Err(error) => return Err(error),
        };
        if sys::file_kind(&st) != libc::S_IFDIR {
            sys::unlink_at(frame.dir.fd(), &name, false)?;
            continue;
        }
        if st.st_dev != frame.dir.identity.dev {
            return Err(io::Error::other("a directory on another filesystem"));
        }
        if depth >= MAX_REMOVAL_DEPTH {
            let moved = CString::new(format!(".nexus-remove-{}", sys::random_hex(8)?))
                .map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
            sys::rename_noreplace_at(frame.dir.fd(), &name, spill.fd(), &moved)?;
            continue;
        }
        if st.st_mode & 0o700 != 0o700 {
            sys::chmod_at_nofollow(frame.dir.fd(), &name, 0o700)?;
        }
        let child = Dir::open_at(frame.dir.fd(), &name)?;
        if child.identity != Identity::of(&st) {
            return Err(io::Error::other(
                "the directory changed while it was opened",
            ));
        }
        return Ok(Step::Descend(Frame {
            dir: child,
            name: Some(name),
        }));
    }
    Ok(Step::Continue)
}

#[cfg(test)]
mod tests;
