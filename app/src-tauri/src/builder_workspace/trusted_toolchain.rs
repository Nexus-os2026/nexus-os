//! P0-002C4D1A: private Builder trusted-toolchain verification primitive.
//!
//! A `VerifiedToolchain` exists only after a complete, statically validated,
//! Nexus-embedded manifest for this exact target (OS, architecture and ABI)
//! has been matched against the tree under a pinned root: exactly the
//! manifest's regular files (no missing, extra, redirected or non-regular
//! entries and no unexpected directories) with exact sizes and SHA-256
//! digests. Unix traversal is descriptor-relative and never follows links;
//! Windows traversal retains handles and never follows reparse points.
//!
//! Verification describes the tree as observed while it ran. It is not an
//! atomic snapshot against a concurrent same-user writer and grants nothing
//! once it returns; callers must re-verify before any trusted use. Production
//! has no trusted toolchain or trusted root yet (C4D2), so `verify_installed`
//! is always unavailable. Nothing here launches a process.
#![cfg_attr(not(test), allow(dead_code))] // Staged: no production caller until C4C3.

use super::directory_identity::DirectoryIdentity;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

const SCHEMA: u32 = 1;
const MAX_FILES: usize = 100_000;
const MAX_FILE_BYTES: u64 = 1 << 30;
const MAX_PATH_BYTES: usize = 200;
const MAX_COMPONENT_BYTES: usize = 255;

/// Bounded, value-free verification failures. Never carries paths or bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ToolchainError {
    Unavailable,
    PlatformMismatch,
    InvalidManifest,
    RootRejected,
    Missing,
    Unexpected,
    Redirected,
    UnsupportedKind,
    SizeMismatch,
    DigestMismatch,
    Changed,
    Io,
}

impl std::fmt::Display for ToolchainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "trusted toolchain unavailable",
            Self::PlatformMismatch => "trusted toolchain platform mismatch",
            Self::InvalidManifest => "trusted toolchain manifest invalid",
            Self::RootRejected => "trusted toolchain root rejected",
            Self::Missing => "trusted toolchain file missing",
            Self::Unexpected => "trusted toolchain entry unexpected",
            Self::Redirected => "trusted toolchain entry redirected",
            Self::UnsupportedKind => "trusted toolchain entry kind unsupported",
            Self::SizeMismatch => "trusted toolchain file size mismatch",
            Self::DigestMismatch => "trusted toolchain file digest mismatch",
            Self::Changed => "trusted toolchain changed during verification",
            Self::Io => "trusted toolchain verification failed",
        })
    }
}

/// Compile-time target identity. No caller chooses these values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ToolchainTarget<'a> {
    os: &'a str,
    arch: &'a str,
    env: &'a str,
}

const UNSUPPORTED_ENV: &str = "unsupported";
const CURRENT_TARGET: ToolchainTarget<'static> = ToolchainTarget {
    os: std::env::consts::OS,
    arch: std::env::consts::ARCH,
    env: if cfg!(target_env = "gnu") {
        "gnu"
    } else if cfg!(target_env = "musl") {
        "musl"
    } else if cfg!(target_env = "msvc") {
        "msvc"
    } else if cfg!(target_env = "") {
        "none"
    } else {
        UNSUPPORTED_ENV
    },
};

/// The complete allowed tree: directories are implied solely by file paths.
struct ToolchainManifest<'a> {
    schema: u32,
    target: ToolchainTarget<'a>,
    files: &'a [ManifestFile<'a>],
}

struct ManifestFile<'a> {
    path: &'a str,
    size: u64,
    sha256: [u8; 32],
}

/// Nexus-owned production manifest. `None`: no trusted toolchain is packaged
/// yet (C4D2). Absence is never represented as an empty manifest.
const PRODUCTION_MANIFEST: Option<&ToolchainManifest<'static>> = None;

/// Opaque backend-owned authority, constructed only at the successful end of
/// `verify_tree`. Deliberately not Clone, Copy, Default or serializable.
pub(super) struct VerifiedToolchain {
    root: PathBuf,
    root_identity: DirectoryIdentity,
}

impl VerifiedToolchain {
    #[cfg(test)]
    fn root(&self) -> &Path {
        &self.root
    }

    #[cfg(test)]
    fn root_still_pinned(&self) -> bool {
        self.root_identity.validate(&self.root).is_ok()
    }
}

/// Production verification. C4D1A has no production manifest and no trusted
/// production root, so this denies before any filesystem access.
pub(super) fn verify_installed() -> Result<VerifiedToolchain, ToolchainError> {
    let Some(manifest) = PRODUCTION_MANIFEST else {
        return Err(ToolchainError::Unavailable);
    };
    validate_manifest(manifest, &CURRENT_TARGET)?;
    // A trusted production root is derived only by C4D2.
    Err(ToolchainError::Unavailable)
}

/// Verifies `root` against `manifest`. Private: roots and manifests reach it
/// only from this module (production has none; tests use synthetic fixtures).
fn verify_tree(
    manifest: &ToolchainManifest<'_>,
    root: &Path,
) -> Result<VerifiedToolchain, ToolchainError> {
    validate_manifest(manifest, &CURRENT_TARGET)?;
    let expected = Expected::new(manifest);
    let root_identity =
        DirectoryIdentity::capture(root).map_err(|_| ToolchainError::RootRejected)?;
    let mut found = BTreeSet::new();
    native::verify(root, &root_identity, &expected, &mut found)?;
    if found.len() != expected.files.len() {
        return Err(ToolchainError::Missing);
    }
    root_identity
        .validate(root)
        .map_err(|_| ToolchainError::Changed)?;
    Ok(VerifiedToolchain {
        root: root.to_path_buf(),
        root_identity,
    })
}

fn validate_manifest(
    manifest: &ToolchainManifest<'_>,
    target: &ToolchainTarget<'_>,
) -> Result<(), ToolchainError> {
    if manifest.schema != SCHEMA {
        return Err(ToolchainError::InvalidManifest);
    }
    if target.env == UNSUPPORTED_ENV || manifest.target != *target {
        return Err(ToolchainError::PlatformMismatch);
    }
    let files = manifest.files;
    if files.is_empty() || files.len() > MAX_FILES {
        return Err(ToolchainError::InvalidManifest);
    }
    if files.windows(2).any(|pair| pair[0].path >= pair[1].path) {
        return Err(ToolchainError::InvalidManifest);
    }
    let mut folded_files = BTreeSet::new();
    for file in files {
        if !valid_path(file.path)
            || file.size > MAX_FILE_BYTES
            || !folded_files.insert(file.path.to_ascii_lowercase())
        {
            return Err(ToolchainError::InvalidManifest);
        }
    }
    // Every implied directory has one spelling under ASCII case folding, and a
    // file may not also be (case-insensitively) an implied directory.
    let mut folded_dirs = BTreeMap::new();
    for file in files {
        for prefix in directory_prefixes(file.path) {
            let lower = prefix.to_ascii_lowercase();
            if folded_files.contains(&lower)
                || *folded_dirs.entry(lower).or_insert(prefix) != prefix
            {
                return Err(ToolchainError::InvalidManifest);
            }
        }
    }
    Ok(())
}

/// Normalized relative `/`-separated ASCII path under the strict grammar.
fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_PATH_BYTES
        && path.is_ascii()
        && path.split('/').all(valid_component)
}

/// One component: `[A-Za-z0-9._@+-]+`, not `.`/`..`, no trailing dot or
/// space, and no Windows DOS device stem (case-insensitive).
fn valid_component(component: &str) -> bool {
    const DEVICES: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = component.split('.').next().unwrap_or_default();
    !component.is_empty()
        && component.len() <= MAX_COMPONENT_BYTES
        && component
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._@+-".contains(&b))
        && component != "."
        && component != ".."
        && !component.ends_with(['.', ' '])
        && !DEVICES
            .iter()
            .any(|device| stem.eq_ignore_ascii_case(device))
}

fn directory_prefixes(path: &str) -> impl Iterator<Item = &str> {
    path.match_indices('/').map(move |(at, _)| &path[..at])
}

fn child_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}/{name}")
    }
}

/// The exact expected tree derived from a validated manifest.
struct Expected<'m> {
    files: BTreeMap<&'m str, &'m ManifestFile<'m>>,
    dirs: BTreeSet<&'m str>,
}

impl<'m> Expected<'m> {
    fn new(manifest: &'m ToolchainManifest<'m>) -> Self {
        let files = manifest
            .files
            .iter()
            .map(|file| (file.path, file))
            .collect();
        let dirs = manifest
            .files
            .iter()
            .flat_map(|file| directory_prefixes(file.path))
            .collect();
        Self { files, dirs }
    }
}

/// Streams SHA-256 over an already validated open file; the byte count must
/// equal the manifest size (growth or truncation while hashing is `Changed`).
fn hash_exact(file: &mut std::fs::File, size: u64) -> Result<[u8; 32], ToolchainError> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut count: u64 = 0;
    loop {
        let read = file.read(&mut buffer).map_err(|_| ToolchainError::Io)?;
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

#[cfg(unix)]
mod native {
    //! Descriptor-relative, no-follow traversal: every child is resolved
    //! relative to its retained parent descriptor, never from a string path.
    use super::*;
    use rustix::fs::{fstat, openat, statat, AtFlags, Dir, FileType, Mode, OFlags, Stat, CWD};
    use rustix::io::Errno;
    use std::os::fd::{AsFd, BorrowedFd};

    fn directory_flags() -> OFlags {
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY
    }

    // NONBLOCK/NOCTTY: opening an unexpected FIFO or device must never block
    // or acquire a terminal; its type is rejected from the opened descriptor.
    fn file_flags() -> OFlags {
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::NOCTTY
    }

    fn open_error(error: Errno) -> ToolchainError {
        match error {
            Errno::LOOP => ToolchainError::Redirected,
            Errno::NOENT | Errno::NOTDIR => ToolchainError::Changed,
            _ => ToolchainError::Io,
        }
    }

    fn io(_: Errno) -> ToolchainError {
        ToolchainError::Io
    }

    fn kind(stat: &Stat) -> FileType {
        FileType::from_raw_mode(stat.st_mode)
    }

    fn same_object(a: &Stat, b: &Stat) -> Result<(), ToolchainError> {
        if (a.st_dev, a.st_ino) != (b.st_dev, b.st_ino) || kind(a) != kind(b) {
            return Err(ToolchainError::Changed);
        }
        Ok(())
    }

    pub(super) fn verify(
        root: &Path,
        identity: &DirectoryIdentity,
        expected: &Expected<'_>,
        found: &mut BTreeSet<String>,
    ) -> Result<(), ToolchainError> {
        // The exact no-follow root descriptor traversed below is bound directly
        // to the retained backend identity; no pathname is re-resolved for it.
        let root_dir = std::fs::File::from(
            openat(CWD, root, directory_flags(), Mode::empty())
                .map_err(|_| ToolchainError::RootRejected)?,
        );
        identity
            .validate_handle(&root_dir)
            .map_err(|_| ToolchainError::Changed)?;
        let pinned = fstat(&root_dir).map_err(io)?;
        let at_path = statat(CWD, root, AtFlags::SYMLINK_NOFOLLOW).map_err(open_error)?;
        if kind(&pinned) != FileType::Directory {
            return Err(ToolchainError::RootRejected);
        }
        same_object(&pinned, &at_path)?;
        walk(root_dir.as_fd(), "", expected, found)?;
        let after = statat(CWD, root, AtFlags::SYMLINK_NOFOLLOW).map_err(open_error)?;
        same_object(&pinned, &after)?;
        same_object(&pinned, &fstat(&root_dir).map_err(io)?)
    }

    fn walk(
        dir: BorrowedFd<'_>,
        prefix: &str,
        expected: &Expected<'_>,
        found: &mut BTreeSet<String>,
    ) -> Result<(), ToolchainError> {
        let mut names = Vec::new();
        for entry in Dir::read_from(dir).map_err(io)? {
            let entry = entry.map_err(io)?;
            let name = entry.file_name().to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            let name = std::str::from_utf8(name)
                .ok()
                .filter(|name| valid_component(name))
                .ok_or(ToolchainError::Unexpected)?;
            names.push(name.to_owned());
        }
        names.sort();
        for name in names {
            let relative = child_path(prefix, &name);
            let observed =
                statat(dir, name.as_str(), AtFlags::SYMLINK_NOFOLLOW).map_err(open_error)?;
            if let Some(file) = expected.files.get(relative.as_str()) {
                match kind(&observed) {
                    FileType::RegularFile => verify_file(dir, &name, &observed, file)?,
                    other => return Err(rejected(other)),
                }
                found.insert(relative);
            } else if expected.dirs.contains(relative.as_str()) {
                match kind(&observed) {
                    FileType::Directory => {
                        let child = openat(dir, name.as_str(), directory_flags(), Mode::empty())
                            .map_err(open_error)?;
                        same_object(&fstat(&child).map_err(io)?, &observed)?;
                        walk(child.as_fd(), &relative, expected, found)?;
                    }
                    other => return Err(rejected(other)),
                }
            } else {
                return Err(rejected(kind(&observed)));
            }
        }
        Ok(())
    }

    fn rejected(kind: FileType) -> ToolchainError {
        match kind {
            FileType::Symlink => ToolchainError::Redirected,
            FileType::RegularFile | FileType::Directory => ToolchainError::Unexpected,
            _ => ToolchainError::UnsupportedKind,
        }
    }

    fn verify_file(
        dir: BorrowedFd<'_>,
        name: &str,
        observed: &Stat,
        file: &ManifestFile<'_>,
    ) -> Result<(), ToolchainError> {
        let fd = openat(dir, name, file_flags(), Mode::empty()).map_err(open_error)?;
        let opened = fstat(&fd).map_err(io)?;
        if kind(&opened) != FileType::RegularFile {
            return Err(ToolchainError::UnsupportedKind);
        }
        same_object(&opened, observed)?;
        if u64::try_from(opened.st_size).ok() != Some(file.size) {
            return Err(ToolchainError::SizeMismatch);
        }
        // Hash the same descriptor that was validated; never reopen by path.
        let mut handle = std::fs::File::from(fd);
        let digest = hash_exact(&mut handle, file.size)?;
        let after = fstat(&handle).map_err(io)?;
        same_object(&opened, &after)?;
        if after.st_size != opened.st_size {
            return Err(ToolchainError::Changed);
        }
        if digest != file.sha256 {
            return Err(ToolchainError::DigestMismatch);
        }
        Ok(())
    }
}

#[cfg(windows)]
mod native {
    //! Handle-retaining traversal that never follows a reparse point. Every
    //! directory handle is held (read-only sharing, so it cannot be renamed,
    //! deleted or turned into a reparse point) while its children are checked,
    //! and each file is hashed from the same validated no-follow handle.
    use super::*;
    use std::fs::{File, Metadata, OpenOptions};
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_SHARE_READ,
    };

    #[derive(PartialEq, Eq)]
    enum Kind {
        File,
        Directory,
        Redirected,
        Other,
    }

    fn kind(metadata: &Metadata) -> Kind {
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            Kind::Redirected
        } else if metadata.is_dir() {
            Kind::Directory
        } else if metadata.is_file() {
            Kind::File
        } else {
            Kind::Other
        }
    }

    fn rejected(kind: Kind) -> ToolchainError {
        match kind {
            Kind::Redirected => ToolchainError::Redirected,
            Kind::File | Kind::Directory => ToolchainError::Unexpected,
            Kind::Other => ToolchainError::UnsupportedKind,
        }
    }

    // Read access, read-only sharing (denies concurrent write, delete and
    // rename) and never following a reparse point.
    fn open(path: &Path, directory: bool) -> Result<File, ToolchainError> {
        let flags = FILE_FLAG_OPEN_REPARSE_POINT
            | if directory {
                FILE_FLAG_BACKUP_SEMANTICS
            } else {
                0
            };
        OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(flags)
            .open(path)
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => ToolchainError::Changed,
                _ => ToolchainError::Io,
            })
    }

    fn handle_kind(handle: &File) -> Result<Kind, ToolchainError> {
        Ok(kind(&handle.metadata().map_err(|_| ToolchainError::Io)?))
    }

    pub(super) fn verify(
        root: &Path,
        identity: &DirectoryIdentity,
        expected: &Expected<'_>,
        found: &mut BTreeSet<String>,
    ) -> Result<(), ToolchainError> {
        let root_handle = open(root, true).map_err(|_| ToolchainError::RootRejected)?;
        if handle_kind(&root_handle)? != Kind::Directory {
            return Err(ToolchainError::RootRejected);
        }
        // Bind this exact reparse-refusing root handle to the retained identity
        // before traversal; no pathname is re-resolved for the binding.
        identity
            .validate_handle(&root_handle)
            .map_err(|_| ToolchainError::Changed)?;
        walk(root, &root_handle, "", expected, found)?;
        if handle_kind(&root_handle)? != Kind::Directory {
            return Err(ToolchainError::Changed);
        }
        Ok(())
    }

    // `_held` keeps the directory pinned while its children are processed.
    fn walk(
        dir: &Path,
        _held: &File,
        prefix: &str,
        expected: &Expected<'_>,
        found: &mut BTreeSet<String>,
    ) -> Result<(), ToolchainError> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(dir).map_err(|_| ToolchainError::Io)? {
            let entry = entry.map_err(|_| ToolchainError::Io)?;
            let name = entry
                .file_name()
                .to_str()
                .filter(|name| valid_component(name))
                .map(str::to_owned)
                .ok_or(ToolchainError::Unexpected)?;
            // Directory-entry metadata never follows a reparse point.
            let observed = kind(&entry.metadata().map_err(|_| ToolchainError::Io)?);
            entries.push((name, observed));
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, observed) in entries {
            let relative = child_path(prefix, &name);
            let path = dir.join(&name);
            if let Some(file) = expected.files.get(relative.as_str()) {
                if observed != Kind::File {
                    return Err(rejected(observed));
                }
                verify_file(&path, file)?;
                found.insert(relative);
            } else if expected.dirs.contains(relative.as_str()) {
                if observed != Kind::Directory {
                    return Err(rejected(observed));
                }
                let child = open(&path, true)?;
                match handle_kind(&child)? {
                    Kind::Directory => walk(&path, &child, &relative, expected, found)?,
                    other => return Err(rejected(other)),
                }
            } else {
                return Err(rejected(observed));
            }
        }
        Ok(())
    }

    fn verify_file(path: &Path, file: &ManifestFile<'_>) -> Result<(), ToolchainError> {
        let mut handle = open(path, false)?;
        let opened = handle.metadata().map_err(|_| ToolchainError::Io)?;
        match kind(&opened) {
            Kind::File => {}
            other => return Err(rejected(other)),
        }
        if opened.len() != file.size {
            return Err(ToolchainError::SizeMismatch);
        }
        // Hash the same validated handle; re-check it before accepting.
        let digest = hash_exact(&mut handle, file.size)?;
        let after = handle.metadata().map_err(|_| ToolchainError::Io)?;
        if kind(&after) != Kind::File || after.len() != opened.len() {
            return Err(ToolchainError::Changed);
        }
        if digest != file.sha256 {
            return Err(ToolchainError::DigestMismatch);
        }
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
mod native {
    use super::*;

    pub(super) fn verify(
        _root: &Path,
        _identity: &DirectoryIdentity,
        _expected: &Expected<'_>,
        _found: &mut BTreeSet<String>,
    ) -> Result<(), ToolchainError> {
        Err(ToolchainError::Unavailable)
    }
}

#[cfg(test)]
mod tests;
