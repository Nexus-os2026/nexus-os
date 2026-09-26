//! P0-002C4D1B: governed Builder React + private runtime provisioning.
//!
//! For a current registration only, the backend exclusively creates `react/`
//! (project content) and a private `runtime/` tree (`home`, `tmp`,
//! `vite-cache`, `env`) directly beneath the retained project directory,
//! persists only policy-approved scaffold content into `react/`, and retains
//! native identities for `react/` and `runtime/`. The C3 write path and
//! C4A/C4B selection accept a registration only after the whole operation
//! succeeded and its temporary C1 grant was revoked. C3 grants are rooted at
//! `react/`, so project writes can never reach `runtime/`.
//!
//! Pre-existing controlled entries are never adopted, and a failure rolls back
//! only entries this operation created (a substituted object is never
//! removed). Like the C3 write path this is not an atomic namespace guarantee
//! against a concurrent same-user writer. Nothing here launches a process.
use super::directory_identity::{DirectoryIdentity, IdentityError};
use super::{
    event, validate_relative_file, Audit, BuilderWorkspaceAuthority, RegisteredBuilderProject,
    WriteResult,
};
use nexus_kernel::manifest::FsPermissionLevel;
use nexus_kernel::workspace_authority::{
    WorkspaceAuthoritySource, WorkspaceBinding, WorkspaceGrantId,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

pub(super) const REACT: &str = "react";
pub(super) const RUNTIME: &str = "runtime";
/// Private runtime children reserved for a later sealed launch (C4D2/C4C3).
const RUNTIME_CHILDREN: [&str; 4] = ["home", "tmp", "vite-cache", "env"];

/// Retained identities of a fully provisioned workspace. Set exactly once, only
/// by successful provisioning; never serialized, cloned or exposed.
pub(super) struct ProvisionedWorkspace {
    react: DirectoryIdentity,
    runtime: DirectoryIdentity,
}

impl ProvisionedWorkspace {
    pub(super) fn validate_react(&self, project_root: &Path) -> Result<(), IdentityError> {
        self.react.validate(&project_root.join(REACT))
    }

    pub(super) fn validate_runtime(&self, project_root: &Path) -> Result<(), IdentityError> {
        self.runtime.validate(&project_root.join(RUNTIME))
    }
}

// ── Scaffold policy ─────────────────────────────────────────────────────────

const MAX_SCAFFOLD_FILES: usize = 512;
const MAX_SCAFFOLD_FILE_BYTES: usize = 1 << 20;
const MAX_SCAFFOLD_BYTES: usize = 16 << 20;
const MAX_SCAFFOLD_PATH: usize = 240;

/// Exact root-level files the deterministic scaffold generator emits that are
/// package/toolchain metadata rather than project content: dropped
/// deliberately, never persisted.
const DROPPED_GENERATOR_METADATA: [&str; 6] = [
    "package.json",
    "tsconfig.json",
    "vite.config.ts",
    "tailwind.config.ts",
    "postcss.config.js",
    "README.md",
];

/// Package, toolchain, runtime-configuration and environment names that are
/// never project content at any depth (ASCII case-insensitive).
fn forbidden_name(name: &str) -> bool {
    const EXACT: [&str; 10] = [
        "node_modules",
        "package.json",
        "package-lock.json",
        "npm-shrinkwrap.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "bun.lockb",
        ".npmrc",
        ".yarnrc",
        "jsconfig.json",
    ];
    const PREFIXES: [&str; 14] = [
        ".env",
        ".babelrc",
        ".postcssrc",
        ".yarnrc.",
        ".pnpmfile",
        "tsconfig.",
        "vite.config.",
        "vitest.config.",
        "postcss.config.",
        "tailwind.config.",
        "babel.config.",
        "esbuild.config.",
        "webpack.config.",
        "rollup.config.",
    ];
    let lower = name.to_ascii_lowercase();
    EXACT.contains(&lower.as_str()) || PREFIXES.iter().any(|prefix| lower.starts_with(prefix))
}

/// A policy-approved content file, persisted beneath `react/`.
struct ContentFile<'a> {
    path: &'a str,
    bytes: &'a [u8],
}

/// The P0-002C4D1B scaffold policy, applied to the whole scaffold before any
/// mutation. Persisted: `index.html`, `src/**` and `public/**`. Dropped: the
/// exact root-level generator metadata above. Anything else rejects the whole
/// scaffold: other root files, forbidden names at any depth, traversal,
/// absolute, drive, UNC, ADS, device, non-ASCII, oversized, duplicate or
/// case-aliased paths, and a file that is also an implied directory.
fn scaffold_content<'a>(scaffold: &[(&'a str, &'a [u8])]) -> WriteResult<Vec<ContentFile<'a>>> {
    if scaffold.len() > MAX_SCAFFOLD_FILES {
        return Err("scaffold denied");
    }
    let mut content = Vec::new();
    let mut total = 0usize;
    for &(path, bytes) in scaffold {
        if DROPPED_GENERATOR_METADATA.contains(&path) {
            continue;
        }
        validate_relative_file(path).map_err(|_| "scaffold path denied")?;
        if !path.is_ascii() || path.len() > MAX_SCAFFOLD_PATH {
            return Err("scaffold path denied");
        }
        let components: Vec<&str> = path.split('/').collect();
        if components.iter().any(|component| forbidden_name(component)) {
            return Err("scaffold configuration denied");
        }
        let allowed = match components.as_slice() {
            ["index.html"] => true,
            [first, _, ..] => *first == "src" || *first == "public",
            _ => false,
        };
        if !allowed {
            return Err("scaffold path denied");
        }
        total = total.saturating_add(bytes.len());
        if bytes.len() > MAX_SCAFFOLD_FILE_BYTES || total > MAX_SCAFFOLD_BYTES {
            return Err("scaffold size denied");
        }
        content.push(ContentFile { path, bytes });
    }
    let mut files = BTreeSet::new();
    for file in &content {
        if !files.insert(file.path.to_ascii_lowercase()) {
            return Err("scaffold path denied");
        }
    }
    let mut dirs = BTreeMap::new();
    for file in &content {
        for (at, _) in file.path.match_indices('/') {
            let dir = &file.path[..at];
            let lower = dir.to_ascii_lowercase();
            if files.contains(&lower) || *dirs.entry(lower).or_insert(dir) != dir {
                return Err("scaffold path denied");
            }
        }
    }
    content.sort_by(|a, b| a.path.cmp(b.path));
    Ok(content)
}

// ── Provisioning ───────────────────────────────────────────────────────────

impl BuilderWorkspaceAuthority {
    /// Governed provisioning of a registered project's React and private
    /// runtime workspace. The selector only identifies a current private
    /// registration; every location is derived from it. Returns only success
    /// or a bounded denial: no path, handle or authority leaves this method.
    pub(super) fn provision_workspace(
        &self,
        selector: &str,
        scaffold: &[(&str, &[u8])],
        audit: Audit,
    ) -> WriteResult<()> {
        let result = self.provision_registered(selector, scaffold, Arc::clone(&audit));
        event(
            &audit,
            Uuid::parse_str(selector).ok(),
            "workspace",
            if result.is_ok() {
                "provisioned"
            } else {
                "denied"
            },
        );
        result
    }

    fn provision_registered(
        &self,
        selector: &str,
        scaffold: &[(&str, &[u8])],
        audit: Audit,
    ) -> WriteResult<()> {
        let id = Uuid::parse_str(selector).map_err(|_| "project not registered")?;
        let project = self.catalog.lookup(id)?;
        self.catalog.validate(&project, &audit)?;
        if project.workspace.get().is_some() {
            return Err("workspace already provisioned");
        }
        // The complete policy decision precedes any mutation.
        let content = scaffold_content(scaffold)?;
        ProvisioningExecution::begin(self, project, audit)?.run(&content)
    }
}

/// One bounded provisioning execution: a fresh short-lived C1 grant rooted at
/// the registered project, re-resolved before every mutation and revoked
/// before any registration of the result.
struct ProvisioningExecution<'a> {
    authority: &'a BuilderWorkspaceAuthority,
    project: Arc<RegisteredBuilderProject>,
    binding: WorkspaceBinding,
    grant: WorkspaceGrantId,
    audit: Audit,
    revoked: bool,
}

impl<'a> ProvisioningExecution<'a> {
    fn begin(
        authority: &'a BuilderWorkspaceAuthority,
        project: Arc<RegisteredBuilderProject>,
        audit: Audit,
    ) -> WriteResult<Self> {
        let binding = WorkspaceBinding {
            agent_id: authority.provisioner,
            run_id: Uuid::new_v4(),
        };
        let expiry = SystemTime::now()
            .checked_add(Duration::from_secs(60))
            .ok_or("expiry unavailable")?;
        let grant = authority
            .registry
            .issue_trusted_root(
                &project.root,
                binding,
                WorkspaceAuthoritySource::BackendAllocated,
                FsPermissionLevel::ReadWrite,
                Some(expiry),
            )
            .map_err(|_| "provisioning issuance denied")?;
        let execution = Self {
            authority,
            project,
            binding,
            grant,
            audit,
            revoked: false,
        };
        execution.event("issue", "authorized");
        Ok(execution)
    }

    fn event(&self, operation: &str, outcome: &str) {
        event(
            &self.audit,
            Some(self.project.project_id),
            &format!("workspace.{operation}"),
            outcome,
        );
    }

    /// Current authority, registration and identities before every mutation.
    fn check(&self) -> WriteResult<()> {
        let grant = self
            .authority
            .registry
            .resolve(self.grant, self.binding)
            .map_err(|_| "provisioning authority denied")?;
        if grant.permission() != &FsPermissionLevel::ReadWrite || grant.root() != self.project.root
        {
            return Err("provisioning authority scope denied");
        }
        self.authority
            .catalog
            .validate(&self.project, &self.audit)?;
        if self.project.workspace.get().is_some() {
            return Err("workspace already provisioned");
        }
        Ok(())
    }

    fn run(mut self, content: &[ContentFile<'_>]) -> WriteResult<()> {
        let bound = self.check().and_then(|()| {
            Tree::bind(&self.project.root, &self.project.identity)
                .map_err(|_| "project identity denied")
        });
        let (mut tree, populated) = match bound {
            Ok(mut tree) => {
                let populated = self.populate(&mut tree, content);
                (Some(tree), populated)
            }
            Err(error) => (None, Err(error)),
        };
        // The temporary grant is revoked before any registration of the result.
        let revoked = self.revoke();
        let outcome = match (populated, revoked) {
            (Ok(workspace), Ok(())) => self.register(workspace),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        };
        if outcome.is_err() {
            if let Some(tree) = tree.take() {
                let rollback = tree.rollback();
                self.event(
                    "rollback",
                    if rollback.is_ok() {
                        "completed"
                    } else {
                        "failed"
                    },
                );
                rollback.map_err(|_| "provisioning failed; rollback incomplete")?;
            }
        }
        self.event(
            "complete",
            if outcome.is_ok() {
                "succeeded"
            } else {
                "failed"
            },
        );
        outcome
    }

    fn populate(
        &self,
        tree: &mut Tree,
        content: &[ContentFile<'_>],
    ) -> WriteResult<ProvisionedWorkspace> {
        self.check()?;
        let react = tree.create_dir(Tree::PROJECT, REACT, false)?;
        self.event("react", "created");
        self.check()?;
        let runtime = tree.create_dir(Tree::PROJECT, RUNTIME, true)?;
        for child in RUNTIME_CHILDREN {
            self.check()?;
            tree.create_dir(runtime, child, true)?;
        }
        self.event("runtime", "created");
        let mut dirs = BTreeMap::new();
        for file in content {
            let (parent_path, name) = file.path.rsplit_once('/').unwrap_or(("", file.path));
            let mut parent = react;
            let mut at = String::new();
            for component in parent_path.split('/').filter(|part| !part.is_empty()) {
                if !at.is_empty() {
                    at.push('/');
                }
                at.push_str(component);
                parent = match dirs.get(&at) {
                    Some(&index) => index,
                    None => {
                        self.check()?;
                        let index = tree.create_dir(parent, component, false)?;
                        dirs.insert(at.clone(), index);
                        index
                    }
                };
            }
            self.check()?;
            tree.create_file(parent, name, file.bytes)?;
        }
        self.event("content", "persisted");
        // Retain exactly the directories created and still held here.
        let root = &self.project.root;
        let react_identity =
            DirectoryIdentity::capture(&root.join(REACT)).map_err(|_| "React identity denied")?;
        react_identity
            .validate_handle(tree.handle(react))
            .map_err(|_| "React identity denied")?;
        let runtime_identity = DirectoryIdentity::capture(&root.join(RUNTIME))
            .map_err(|_| "runtime identity denied")?;
        runtime_identity
            .validate_handle(tree.handle(runtime))
            .map_err(|_| "runtime identity denied")?;
        let workspace = ProvisionedWorkspace {
            react: react_identity,
            runtime: runtime_identity,
        };
        self.check()?;
        workspace
            .validate_react(root)
            .map_err(|_| "React identity denied")?;
        workspace
            .validate_runtime(root)
            .map_err(|_| "runtime identity denied")?;
        Ok(workspace)
    }

    /// Publishes the retained identities on the still-current registration.
    fn register(&self, workspace: ProvisionedWorkspace) -> WriteResult<()> {
        self.authority.catalog.active(&self.project)?;
        self.project
            .workspace
            .set(workspace)
            .map_err(|_| "workspace already provisioned")
    }

    fn revoke(&mut self) -> WriteResult<()> {
        if self.revoked {
            return Ok(());
        }
        let result = self
            .authority
            .registry
            .revoke(self.grant, self.binding)
            .map_err(|_| "provisioning revocation failed");
        if result.is_ok() {
            self.revoked = true;
        }
        self.event("revoke", if result.is_ok() { "revoked" } else { "failed" });
        result
    }
}

impl Drop for ProvisioningExecution<'_> {
    fn drop(&mut self) {
        if !self.revoked {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.revoke()));
            if !matches!(result, Ok(Ok(()))) {
                eprintln!("Builder provisioning abnormal-exit revocation failed");
            }
        }
    }
}

/// Directories used for creation (index 0 is the bound project) and every
/// entry created here, in creation order, for a bounded rollback.
struct Tree {
    dirs: Vec<Option<native::Dir>>,
    created: Vec<Created>,
}

struct Created {
    parent: usize,
    name: String,
    directory: Option<usize>,
    mark: native::Mark,
}

impl Tree {
    const PROJECT: usize = 0;

    fn bind(root: &Path, identity: &DirectoryIdentity) -> Result<Self, &'static str> {
        Ok(Self {
            dirs: vec![Some(native::bind(root, identity)?)],
            created: Vec::new(),
        })
    }

    fn dir(&self, index: usize) -> WriteResult<&native::Dir> {
        self.dirs
            .get(index)
            .and_then(Option::as_ref)
            .ok_or("provisioning directory unavailable")
    }

    fn handle(&self, index: usize) -> &std::fs::File {
        match self.dirs.get(index).and_then(Option::as_ref) {
            Some(dir) => dir.file(),
            None => unreachable!("provisioning directory {index} released early"),
        }
    }

    fn create_dir(&mut self, parent: usize, name: &str, private: bool) -> WriteResult<usize> {
        let (dir, mark) = native::create_dir(self.dir(parent)?, name, private)?;
        let index = self.dirs.len();
        self.dirs.push(Some(dir));
        self.created.push(Created {
            parent,
            name: name.to_owned(),
            directory: Some(index),
            mark,
        });
        Ok(index)
    }

    fn create_file(&mut self, parent: usize, name: &str, bytes: &[u8]) -> WriteResult<()> {
        let (mut file, mark) = native::create_file(self.dir(parent)?, name)?;
        // Recorded before writing, so a failed write is rolled back too.
        self.created.push(Created {
            parent,
            name: name.to_owned(),
            directory: None,
            mark,
        });
        file.write_all(bytes).map_err(|_| "file write failed")
    }

    /// Removes, newest first, only entries created here and still the same
    /// objects. Anything else (a substituted object or new content) stops at
    /// that entry and reports an incomplete rollback.
    fn rollback(mut self) -> WriteResult<()> {
        let mut complete = true;
        while let Some(entry) = self.created.pop() {
            if let Some(index) = entry.directory {
                // Release our own handle first (Windows sharing).
                self.dirs[index] = None;
            }
            let removed = self.dir(entry.parent).and_then(|parent| {
                native::remove(parent, &entry.name, entry.directory.is_some(), &entry.mark)
            });
            complete &= removed.is_ok();
        }
        if complete {
            Ok(())
        } else {
            Err("rollback incomplete")
        }
    }
}

#[cfg(unix)]
mod native {
    //! Descriptor-relative creation beneath a directory bound to a retained
    //! identity: children are created and opened relative to their parent's
    //! descriptor with O_NOFOLLOW, never resolved from a string path.
    use super::super::directory_identity::DirectoryIdentity;
    use rustix::fs::{fstat, mkdirat, openat, statat, unlinkat, AtFlags, Mode, OFlags, Stat, CWD};
    use rustix::io::Errno;
    use std::fs::File;
    use std::os::fd::AsFd;
    use std::path::Path;

    pub(super) struct Dir(File);

    impl Dir {
        pub(super) fn file(&self) -> &File {
            &self.0
        }
    }

    /// Native identity of a created entry (never trusted for removal otherwise).
    pub(super) type Mark = Stat;

    fn dir_flags() -> OFlags {
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY
    }

    fn created_error(error: Errno) -> &'static str {
        match error {
            Errno::EXIST => "controlled entry already exists",
            _ => "controlled entry creation failed",
        }
    }

    /// Opens `path` without following a final link and binds the opened
    /// directory to `identity` by native identity equality.
    pub(super) fn bind(path: &Path, identity: &DirectoryIdentity) -> Result<Dir, &'static str> {
        let fd =
            openat(CWD, path, dir_flags(), Mode::empty()).map_err(|_| "project identity denied")?;
        let dir = File::from(fd);
        identity
            .validate_handle(&dir)
            .map_err(|_| "project identity denied")?;
        Ok(Dir(dir))
    }

    pub(super) fn create_dir(
        parent: &Dir,
        name: &str,
        private: bool,
    ) -> Result<(Dir, Mark), &'static str> {
        let mode = Mode::from_raw_mode(if private { 0o700 } else { 0o755 });
        mkdirat(parent.0.as_fd(), name, mode).map_err(created_error)?;
        let opened = openat(parent.0.as_fd(), name, dir_flags(), Mode::empty())
            .map_err(|_| "controlled entry creation failed")
            .and_then(|fd| {
                fstat(&fd)
                    .map(|mark| (Dir(File::from(fd)), mark))
                    .map_err(|_| "controlled entry creation failed")
            });
        if opened.is_err() {
            // Unrecorded: remove only an empty directory at that name.
            let _ = unlinkat(parent.0.as_fd(), name, AtFlags::REMOVEDIR);
        }
        opened
    }

    pub(super) fn create_file(parent: &Dir, name: &str) -> Result<(File, Mark), &'static str> {
        let flags =
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let fd = openat(parent.0.as_fd(), name, flags, Mode::from_raw_mode(0o644))
            .map_err(created_error)?;
        match fstat(&fd) {
            Ok(mark) => Ok((File::from(fd), mark)),
            Err(_) => {
                drop(fd);
                let _ = unlinkat(parent.0.as_fd(), name, AtFlags::empty());
                Err("controlled entry creation failed")
            }
        }
    }

    pub(super) fn remove(
        parent: &Dir,
        name: &str,
        directory: bool,
        mark: &Mark,
    ) -> Result<(), &'static str> {
        let current = statat(parent.0.as_fd(), name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| "rollback entry unavailable")?;
        if (current.st_dev, current.st_ino) != (mark.st_dev, mark.st_ino) {
            return Err("rollback entry substituted");
        }
        let flags = if directory {
            AtFlags::REMOVEDIR
        } else {
            AtFlags::empty()
        };
        unlinkat(parent.0.as_fd(), name, flags).map_err(|_| "rollback removal failed")
    }
}

#[cfg(windows)]
mod native {
    //! Handle-pinned creation: every directory used for creation is held open
    //! without delete sharing (it cannot be renamed, deleted or turned into a
    //! reparse point meanwhile), children are created exclusively, and no
    //! reparse point is ever followed or accepted.
    use super::super::directory_identity::DirectoryIdentity;
    use std::fs::{File, OpenOptions};
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use std::path::{Path, PathBuf};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    pub(super) struct Dir {
        path: PathBuf,
        handle: File,
    }

    impl Dir {
        pub(super) fn file(&self) -> &File {
            &self.handle
        }
    }

    /// Created entries are re-checked by kind and non-redirection on removal.
    pub(super) type Mark = ();

    fn redirected(metadata: &std::fs::Metadata) -> bool {
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }

    fn open_dir(path: &Path) -> Result<File, &'static str> {
        let handle = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .map_err(|_| "controlled entry creation failed")?;
        let metadata = handle
            .metadata()
            .map_err(|_| "controlled entry creation failed")?;
        if redirected(&metadata) || !metadata.is_dir() {
            return Err("controlled entry redirected");
        }
        Ok(handle)
    }

    pub(super) fn bind(path: &Path, identity: &DirectoryIdentity) -> Result<Dir, &'static str> {
        let handle = open_dir(path).map_err(|_| "project identity denied")?;
        identity
            .validate_handle(&handle)
            .map_err(|_| "project identity denied")?;
        Ok(Dir {
            path: path.to_path_buf(),
            handle,
        })
    }

    pub(super) fn create_dir(
        parent: &Dir,
        name: &str,
        _private: bool,
    ) -> Result<(Dir, Mark), &'static str> {
        let path = parent.path.join(name);
        std::fs::create_dir(&path).map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => "controlled entry already exists",
            _ => "controlled entry creation failed",
        })?;
        match open_dir(&path) {
            Ok(handle) => Ok((Dir { path, handle }, ())),
            Err(error) => {
                // Unrecorded: remove only an empty directory at that name.
                let _ = std::fs::remove_dir(&path);
                Err(error)
            }
        }
    }

    pub(super) fn create_file(parent: &Dir, name: &str) -> Result<(File, Mark), &'static str> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(parent.path.join(name))
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::AlreadyExists => "controlled entry already exists",
                _ => "controlled entry creation failed",
            })?;
        Ok((file, ()))
    }

    pub(super) fn remove(
        parent: &Dir,
        name: &str,
        directory: bool,
        _mark: &Mark,
    ) -> Result<(), &'static str> {
        let path = parent.path.join(name);
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|_| "rollback entry unavailable")?;
        if redirected(&metadata) || metadata.is_dir() != directory {
            return Err("rollback entry substituted");
        }
        if directory {
            std::fs::remove_dir(&path)
        } else {
            std::fs::remove_file(&path)
        }
        .map_err(|_| "rollback removal failed")
    }
}

#[cfg(test)]
mod tests;
