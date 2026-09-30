//! The packaged verifier toolchain (Linux, x86_64).
//!
//! A [`VerifiedVerifierToolchain`] exists only after the tree at a root the
//! backend derives has matched, exactly, the manifest embedded in this
//! library at build time: exactly the manifest's regular files (no missing,
//! extra, redirected or non-regular entries and no unexpected directories),
//! with exact sizes, SHA-256 digests and executable modes. Without an
//! embedded manifest (a build without the assembled toolchain) the toolchain
//! is always unavailable, before any filesystem access. No manifest is ever
//! read from disk.
//!
//! The production root is derived only from the installed executable: the
//! Debian package's `/usr/bin/<exe>` gives `/usr/lib/NexusOS/verifier-toolchain`.
//! `/usr`, `/usr/lib` and `/usr/lib/NexusOS` must be root-owned directories
//! that no one else can write, and so must be every directory and file of
//! the tree: the installed-package invariant. A user-writable toolchain
//! (a rustup installation, a development tree) is never production
//! authority; development trees are verifiable only in builds with the
//! `development-toolchain` feature, against the same embedded manifest.
//!
//! The host runtime the toolchain's executables load (the dynamic loader and
//! seven libraries in `/usr/lib/x86_64-linux-gnu`) must be root-owned regular
//! files in root-owned directories, and the ELF interpreter path must resolve
//! to that loader. Their digests are bound into the toolchain digest.
//!
//! Traversal is descriptor-relative and never follows a link. Verification
//! describes the tree as observed while it ran: callers re-verify
//! ([`VerifiedVerifierToolchain::reverify`]) immediately before each launch,
//! and the entry executable is launched by descriptor.

mod contract;

use std::ffi::{CStr, CString};
use std::io::Read;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::policy::Role;
use crate::sys;

pub use contract::{HOST_TARGET, RUST_VERSION, VERIFIER_TARGET};

const DIGEST_DOMAIN: &[u8] = b"nexus.verifier.toolchain.v1";
const INSTALLED_BIN_DIR: &str = "/usr/bin";
const INSTALLED_ROOT: &str = "/usr/lib/NexusOS/verifier-toolchain";
const RUNTIME_DIR: &str = "/usr/lib/x86_64-linux-gnu";
const RUNTIME_LOADER: &CStr = c"ld-linux-x86-64.so.2";
const RUNTIME_LIBRARIES: [&CStr; 7] = [
    c"libc.so.6",
    c"libm.so.6",
    c"libdl.so.2",
    c"librt.so.1",
    c"libpthread.so.0",
    c"libgcc_s.so.1",
    c"libz.so.1",
];
/// The interpreter the toolchain's executables name.
const ELF_INTERPRETER: &str = "/lib64/ld-linux-x86-64.so.2";

/// The complete allowed tree; directories are implied by file paths.
#[derive(Debug)]
struct Manifest<'a> {
    schema: u32,
    rust_version: &'a str,
    host: &'a str,
    target: &'a str,
    files: &'a [ManifestFile<'a>],
}

#[derive(Debug)]
struct ManifestFile<'a> {
    path: &'a str,
    size: u64,
    executable: bool,
    sha256: [u8; 32],
}

// `PACKAGED_MANIFEST`: rendered by build.rs from the assembled packaged
// toolchain (NEXUS_VERIFIER_TOOLCHAIN=packaged), otherwise `None`.
include!(concat!(env!("OUT_DIR"), "/verifier_toolchain_manifest.rs"));
static PACKAGED: Option<Manifest<'static>> = PACKAGED_MANIFEST;

/// Bounded, value-free verification failures (never paths or bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolchainError {
    /// No embedded manifest, or no toolchain at the derived root.
    Unavailable,
    PlatformMismatch,
    InvalidManifest,
    /// The root, an ancestor or an entry is not owned as required, or can
    /// be written by others.
    OwnershipRejected,
    Missing,
    Unexpected,
    Redirected,
    UnsupportedKind,
    SizeMismatch,
    DigestMismatch,
    ModeMismatch,
    Changed,
    /// A host runtime file is not a root-owned, unwritable regular file, or
    /// the interpreter path does not resolve to the verified loader.
    RuntimeRejected,
    Io,
}

impl std::fmt::Display for ToolchainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "verifier toolchain unavailable",
            Self::PlatformMismatch => "verifier toolchain platform mismatch",
            Self::InvalidManifest => "verifier toolchain manifest invalid",
            Self::OwnershipRejected => "verifier toolchain not protected by the installed package",
            Self::Missing => "verifier toolchain file missing",
            Self::Unexpected => "verifier toolchain entry unexpected",
            Self::Redirected => "verifier toolchain entry redirected",
            Self::UnsupportedKind => "verifier toolchain entry kind unsupported",
            Self::SizeMismatch => "verifier toolchain file size mismatch",
            Self::DigestMismatch => "verifier toolchain file digest mismatch",
            Self::ModeMismatch => "verifier toolchain file mode mismatch",
            Self::Changed => "verifier toolchain changed",
            Self::RuntimeRejected => "verifier host runtime rejected",
            Self::Io => "verifier toolchain verification failed",
        })
    }
}

fn io(_: std::io::Error) -> ToolchainError {
    ToolchainError::Io
}

/// Who must own the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    /// The installed-package invariant: root.
    Root,
    /// A development tree of this user (never production).
    #[cfg(any(test, feature = "development-toolchain"))]
    CurrentUser,
}

impl Owner {
    fn uid(self) -> u32 {
        match self {
            Self::Root => 0,
            #[cfg(any(test, feature = "development-toolchain"))]
            // SAFETY: getuid has no preconditions.
            Self::CurrentUser => unsafe { libc::getuid() },
        }
    }

    /// Owned by this owner and writable by no one else. (A symlink's own
    /// mode is meaningless; only its owner and its directory protect it.)
    fn permits(self, st: &libc::stat) -> bool {
        st.st_uid == self.uid() && st.st_mode & 0o022 == 0
    }
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

fn validate(manifest: &Manifest<'_>) -> Result<(), ToolchainError> {
    if manifest.schema != contract::SCHEMA || manifest.rust_version != contract::RUST_VERSION {
        return Err(ToolchainError::InvalidManifest);
    }
    if manifest.host != contract::HOST_TARGET || manifest.target != contract::VERIFIER_TARGET {
        return Err(ToolchainError::PlatformMismatch);
    }
    if manifest.files.is_empty() || manifest.files.len() > contract::MAX_FILES {
        return Err(ToolchainError::InvalidManifest);
    }
    let mut previous: Option<&str> = None;
    for file in manifest.files {
        // Strictly sorted: no duplicates, one canonical order.
        if !contract::valid_path(file.path)
            || file.size > contract::MAX_FILE_BYTES
            || previous.is_some_and(|p| p >= file.path)
        {
            return Err(ToolchainError::InvalidManifest);
        }
        previous = Some(file.path);
    }
    let paths: std::collections::BTreeSet<&str> = manifest.files.iter().map(|f| f.path).collect();
    for file in manifest.files {
        // No file may also be a directory of another.
        if contract::directory_prefixes(file.path)
            .iter()
            .any(|dir| paths.contains(dir))
        {
            return Err(ToolchainError::InvalidManifest);
        }
    }
    for (path, executable) in contract::REQUIRED {
        match manifest.files.iter().find(|file| file.path == *path) {
            Some(file) if file.executable == *executable => {}
            _ => return Err(ToolchainError::InvalidManifest),
        }
    }
    Ok(())
}

/// A retained directory.
#[derive(Debug)]
struct Dir {
    fd: OwnedFd,
    identity: Identity,
}

impl Dir {
    fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

/// Open `path` from `/` one component at a time without following a
/// symlink; with `root_owned_ancestors`, every directory above the last must
/// be root-owned and writable by no one else.
fn open_path(path: &Path, root_owned_ancestors: bool) -> Result<Dir, ToolchainError> {
    if !path.is_absolute() {
        return Err(ToolchainError::Unavailable);
    }
    let parts: Vec<&std::ffi::OsStr> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(Ok(part)),
            Component::RootDir => None,
            _ => Some(Err(())),
        })
        .collect::<Result<_, _>>()
        .map_err(|()| ToolchainError::Unavailable)?;
    let mut fd = sys::open_fixed(c"/", libc::O_RDONLY | libc::O_DIRECTORY).map_err(io)?;
    for (index, part) in parts.iter().enumerate() {
        let name = CString::new(part.as_bytes()).map_err(|_| ToolchainError::Unavailable)?;
        fd = match sys::open_dir_at(fd.as_fd(), &name) {
            Ok(next) => next,
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {
                return Err(ToolchainError::Unavailable)
            }
            Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {
                return Err(ToolchainError::Redirected)
            }
            Err(_) => return Err(ToolchainError::Unavailable),
        };
        if root_owned_ancestors && index + 1 < parts.len() {
            let st = sys::fstat(fd.as_fd()).map_err(io)?;
            if !Owner::Root.permits(&st) {
                return Err(ToolchainError::OwnershipRejected);
            }
        }
    }
    let identity = Identity::of(&sys::fstat(fd.as_fd()).map_err(io)?);
    Ok(Dir { fd, identity })
}

/// What a walk of the tree established.
struct TreeFacts {
    cargo: Identity,
}

/// Compare the tree beneath `root` with `manifest`, exactly.
fn verify_tree(
    manifest: &Manifest<'_>,
    root: &Dir,
    owner: Owner,
) -> Result<TreeFacts, ToolchainError> {
    let root_st = sys::fstat(root.fd()).map_err(io)?;
    if !owner.permits(&root_st) {
        return Err(ToolchainError::OwnershipRejected);
    }
    let files: std::collections::BTreeMap<&str, &ManifestFile<'_>> = manifest
        .files
        .iter()
        .map(|file| (file.path, file))
        .collect();
    let dirs: std::collections::BTreeSet<&str> = manifest
        .files
        .iter()
        .flat_map(|file| contract::directory_prefixes(file.path))
        .collect();
    let mut found = 0usize;
    let mut cargo = None;
    let mut pending = vec![(
        Dir {
            fd: root.fd.try_clone().map_err(io)?,
            identity: root.identity,
        },
        String::new(),
    )];
    while let Some((dir, prefix)) = pending.pop() {
        for name in sys::directory_entries(dir.fd()).map_err(io)? {
            let text = name.to_str().map_err(|_| ToolchainError::Unexpected)?;
            let path = if prefix.is_empty() {
                text.to_owned()
            } else {
                format!("{prefix}/{text}")
            };
            let st = sys::stat_at(dir.fd(), &name).map_err(io)?;
            let kind = sys::file_kind(&st);
            if kind == libc::S_IFLNK {
                return Err(ToolchainError::Redirected);
            }
            match kind {
                libc::S_IFDIR => {
                    if !dirs.contains(path.as_str()) {
                        return Err(ToolchainError::Unexpected);
                    }
                    if !owner.permits(&st) {
                        return Err(ToolchainError::OwnershipRejected);
                    }
                    let child = sys::open_dir_at(dir.fd(), &name).map_err(io)?;
                    let identity = Identity::of(&sys::fstat(child.as_fd()).map_err(io)?);
                    if identity != Identity::of(&st) || identity.dev != root.identity.dev {
                        return Err(ToolchainError::Changed);
                    }
                    pending.push((
                        Dir {
                            fd: child,
                            identity,
                        },
                        path,
                    ));
                }
                libc::S_IFREG => {
                    let expected = files.get(path.as_str()).ok_or(ToolchainError::Unexpected)?;
                    if !owner.permits(&st) {
                        return Err(ToolchainError::OwnershipRejected);
                    }
                    if st.st_size as u64 != expected.size {
                        return Err(ToolchainError::SizeMismatch);
                    }
                    if (st.st_mode & 0o100 != 0) != expected.executable {
                        return Err(ToolchainError::ModeMismatch);
                    }
                    let file = sys::open_file_at(dir.fd(), &name).map_err(io)?;
                    let opened = sys::fstat(file.as_fd()).map_err(io)?;
                    if Identity::of(&opened) != Identity::of(&st)
                        || opened.st_dev != root.identity.dev
                    {
                        return Err(ToolchainError::Changed);
                    }
                    if hash_exact(file, expected.size)? != expected.sha256 {
                        return Err(ToolchainError::DigestMismatch);
                    }
                    if expected.path == contract::CARGO {
                        cargo = Some(Identity::of(&st));
                    }
                    found += 1;
                }
                _ => return Err(ToolchainError::UnsupportedKind),
            }
        }
    }
    if found != files.len() {
        return Err(ToolchainError::Missing);
    }
    Ok(TreeFacts {
        cargo: cargo.ok_or(ToolchainError::Missing)?,
    })
}

/// SHA-256 of exactly `size` bytes of an open file; growth or truncation is
/// `Changed`.
fn hash_exact(file: OwnedFd, size: u64) -> Result<[u8; 32], ToolchainError> {
    let mut file = std::fs::File::from(file);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut count = 0u64;
    loop {
        let read = file.read(&mut buffer).map_err(io)?;
        if read == 0 {
            break;
        }
        count += read as u64;
        if count > size {
            return Err(ToolchainError::Changed);
        }
        hasher.update(&buffer[..read]);
    }
    if count != size {
        return Err(ToolchainError::Changed);
    }
    Ok(hasher.finalize().into())
}

/// One verified host runtime file.
#[derive(Debug)]
struct RuntimeFile {
    fd: OwnedFd,
    identity: Identity,
    sha256: [u8; 32],
}

/// The host runtime the toolchain's executables load.
#[derive(Debug)]
struct HostRuntime {
    loader: RuntimeFile,
    libraries: Vec<RuntimeFile>,
}

impl HostRuntime {
    fn verify(owner: Owner) -> Result<Self, ToolchainError> {
        let dir = open_path(Path::new(RUNTIME_DIR), true).map_err(|error| match error {
            ToolchainError::Io => ToolchainError::Io,
            _ => ToolchainError::RuntimeRejected,
        })?;
        let st = sys::fstat(dir.fd()).map_err(io)?;
        if !Owner::Root.permits(&st) {
            return Err(ToolchainError::RuntimeRejected);
        }
        let loader = runtime_file(dir.fd(), RUNTIME_LOADER, owner)?;
        let libraries = RUNTIME_LIBRARIES
            .iter()
            .map(|name| runtime_file(dir.fd(), name, owner))
            .collect::<Result<Vec<_>, _>>()?;
        // The kernel loads the interpreter the executables name: it must be
        // exactly the verified loader.
        let interpreter =
            std::fs::metadata(ELF_INTERPRETER).map_err(|_| ToolchainError::RuntimeRejected)?;
        use std::os::unix::fs::MetadataExt;
        if (interpreter.dev(), interpreter.ino()) != (loader.identity.dev, loader.identity.ino) {
            return Err(ToolchainError::RuntimeRejected);
        }
        Ok(Self { loader, libraries })
    }

    fn files(&self) -> impl Iterator<Item = (&'static CStr, &RuntimeFile)> {
        std::iter::once(RUNTIME_LOADER)
            .chain(RUNTIME_LIBRARIES)
            .zip(std::iter::once(&self.loader).chain(&self.libraries))
    }
}

/// A root-owned (or, in tests, `owner`-owned) regular file in `dir`, or a
/// symlink there to one by a plain name in the same directory.
fn runtime_file(
    dir: BorrowedFd<'_>,
    name: &CStr,
    owner: Owner,
) -> Result<RuntimeFile, ToolchainError> {
    let rejected = |_| ToolchainError::RuntimeRejected;
    let mut st = sys::stat_at(dir, name).map_err(rejected)?;
    let mut target = name.to_owned();
    if sys::file_kind(&st) == libc::S_IFLNK {
        if st.st_uid != owner.uid() {
            return Err(ToolchainError::RuntimeRejected);
        }
        let link = sys::readlink_at(dir, name).map_err(rejected)?;
        if link.is_empty() || link.contains(&b'/') || link == b"." || link == b".." {
            return Err(ToolchainError::RuntimeRejected);
        }
        target = CString::new(link).map_err(|_| ToolchainError::RuntimeRejected)?;
        st = sys::stat_at(dir, &target).map_err(rejected)?;
    }
    if sys::file_kind(&st) != libc::S_IFREG || !owner.permits(&st) {
        return Err(ToolchainError::RuntimeRejected);
    }
    let fd = sys::open_file_at(dir, &target).map_err(rejected)?;
    let opened = sys::fstat(fd.as_fd()).map_err(io)?;
    if Identity::of(&opened) != Identity::of(&st) {
        return Err(ToolchainError::Changed);
    }
    let sha256 = hash_exact(fd.try_clone().map_err(io)?, opened.st_size as u64)?;
    Ok(RuntimeFile {
        fd,
        identity: Identity::of(&st),
        sha256,
    })
}

/// The digest a verification launch binds: the embedded manifest and the
/// host runtime files' digests, under a domain of their own.
fn toolchain_digest(manifest: &Manifest<'_>, runtime: &HostRuntime) -> [u8; 32] {
    let mut hasher = Sha256::new();
    let put = |hasher: &mut Sha256, bytes: &[u8]| {
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    };
    put(&mut hasher, DIGEST_DOMAIN);
    hasher.update(manifest.schema.to_be_bytes());
    put(&mut hasher, manifest.rust_version.as_bytes());
    put(&mut hasher, manifest.host.as_bytes());
    put(&mut hasher, manifest.target.as_bytes());
    hasher.update((manifest.files.len() as u64).to_be_bytes());
    for file in manifest.files {
        put(&mut hasher, file.path.as_bytes());
        hasher.update(file.size.to_be_bytes());
        hasher.update([u8::from(file.executable)]);
        hasher.update(file.sha256);
    }
    for (name, file) in runtime.files() {
        put(&mut hasher, name.to_bytes());
        hasher.update(file.sha256);
    }
    hasher.finalize().into()
}

/// Whether this build embeds a packaged toolchain manifest (a fact about the
/// build, not authority).
pub fn is_packaged() -> bool {
    PACKAGED.is_some()
}

static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The verified packaged verifier toolchain: opaque backend-owned
/// authority, constructed only at the successful end of a verification.
/// Deliberately not `Clone`, `Copy`, `Default` or serializable.
#[derive(Debug)]
pub struct VerifiedVerifierToolchain {
    manifest: &'static Manifest<'static>,
    owner: Owner,
    path: PathBuf,
    root: Dir,
    cargo: Identity,
    runtime: HostRuntime,
    digest: [u8; 32],
    generation: u64,
}

/// What a launch takes from the toolchain, after [`VerifiedVerifierToolchain::reverify`].
#[derive(Debug)]
pub struct ToolchainLaunch {
    /// The entry executable (`bin/cargo`), to be launched by descriptor.
    pub executable: OwnedFd,
    /// The tree (read and execute), the loader (read and execute) and each
    /// runtime library (read).
    pub rules: Vec<(Role, OwnedFd)>,
    /// The verified root's path, for the profile's toolchain-relative values.
    pub root: PathBuf,
    /// The compiler's path, for the fixed `RUSTC`.
    pub rustc: PathBuf,
    /// The linker's path, for the fixed target linker setting.
    pub linker: PathBuf,
}

impl VerifiedVerifierToolchain {
    /// Production verification: the embedded manifest against the toolchain
    /// installed beside this executable. Unavailable without an embedded
    /// manifest, before any filesystem access.
    pub fn verify_installed() -> Result<Self, ToolchainError> {
        let manifest = PACKAGED.as_ref().ok_or(ToolchainError::Unavailable)?;
        validate(manifest)?;
        let executable = std::env::current_exe().map_err(|_| ToolchainError::Unavailable)?;
        let executable = executable
            .canonicalize()
            .map_err(|_| ToolchainError::Unavailable)?;
        if executable.parent() != Some(Path::new(INSTALLED_BIN_DIR)) {
            return Err(ToolchainError::Unavailable);
        }
        Self::verify_at(manifest, PathBuf::from(INSTALLED_ROOT), Owner::Root)
    }

    /// Verify a development tree of this user against the same embedded
    /// manifest. Only in builds with the `development-toolchain` feature;
    /// never production authority.
    #[cfg(feature = "development-toolchain")]
    pub fn verify_development(root: &Path) -> Result<Self, ToolchainError> {
        let manifest = PACKAGED.as_ref().ok_or(ToolchainError::Unavailable)?;
        validate(manifest)?;
        let canonical = root
            .canonicalize()
            .map_err(|_| ToolchainError::Unavailable)?;
        if canonical != root {
            return Err(ToolchainError::Redirected);
        }
        Self::verify_at(manifest, canonical, Owner::CurrentUser)
    }

    fn verify_at(
        manifest: &'static Manifest<'static>,
        path: PathBuf,
        owner: Owner,
    ) -> Result<Self, ToolchainError> {
        let root = open_path(&path, owner == Owner::Root)?;
        let facts = verify_tree(manifest, &root, owner)?;
        let runtime = HostRuntime::verify(Owner::Root)?;
        if open_path(&path, false)?.identity != root.identity {
            return Err(ToolchainError::Changed);
        }
        let digest = toolchain_digest(manifest, &runtime);
        Ok(Self {
            manifest,
            owner,
            path,
            root,
            cargo: facts.cargo,
            runtime,
            digest,
            generation: GENERATION.fetch_add(1, Ordering::SeqCst) + 1,
        })
    }

    /// Verify the retained tree and host runtime again, immediately before
    /// a launch: the root must still be at its path, the tree must still
    /// match the manifest exactly and the runtime must be the same files.
    pub fn reverify(&self) -> Result<(), ToolchainError> {
        if open_path(&self.path, self.owner == Owner::Root)?.identity != self.root.identity {
            return Err(ToolchainError::Changed);
        }
        let facts = verify_tree(self.manifest, &self.root, self.owner)?;
        if facts.cargo != self.cargo {
            return Err(ToolchainError::Changed);
        }
        let runtime = HostRuntime::verify(Owner::Root)?;
        let same = runtime
            .files()
            .zip(self.runtime.files())
            .all(|((_, now), (_, then))| {
                now.identity == then.identity && now.sha256 == then.sha256
            });
        if !same {
            return Err(ToolchainError::Changed);
        }
        Ok(())
    }

    /// The digest a launch binds: the exact packaged tree and host runtime.
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }

    /// This verification's backend-owned generation.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn rust_version(&self) -> &'static str {
        self.manifest.rust_version
    }

    /// The toolchain-relative path of the entry executable.
    pub fn entry(&self) -> &'static str {
        contract::CARGO
    }

    /// The launch material: the entry executable by descriptor (it must be
    /// the verified file), the sandbox rules and the fixed paths.
    pub fn launch(&self) -> Result<ToolchainLaunch, ToolchainError> {
        let bin = sys::open_dir_at(self.root.fd(), c"bin").map_err(io)?;
        let executable = sys::open_file_at(bin.as_fd(), c"cargo").map_err(io)?;
        let opened = sys::fstat(executable.as_fd()).map_err(io)?;
        if Identity::of(&opened) != self.cargo {
            return Err(ToolchainError::Changed);
        }
        let dup = |fd: &OwnedFd| fd.try_clone().map_err(io);
        let mut rules = vec![
            (Role::ToolchainRoot, dup(&self.root.fd)?),
            (Role::RuntimeLoader, dup(&self.runtime.loader.fd)?),
        ];
        for library in &self.runtime.libraries {
            rules.push((Role::RuntimeLibrary, dup(&library.fd)?));
        }
        Ok(ToolchainLaunch {
            executable,
            rules,
            root: self.path.clone(),
            rustc: self.path.join(contract::RUSTC),
            linker: self.path.join(contract::LINKER),
        })
    }
}

#[cfg(test)]
mod tests;
