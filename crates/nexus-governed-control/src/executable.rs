//! Executable identity: what an owner grant pins, and what every launch
//! checks again immediately before it happens.
//!
//! An executable is named by its canonical absolute path (no symbolic link,
//! no `PATH` lookup). A tool lists its trusted locations and is resolved
//! among them ([`resolve`]): the first present location decides, and a
//! link is followed only to another listed location. Its identity is the digest of that path, its device,
//! inode, size, modification time, mode and owner, and the SHA-256 of its
//! contents, read through one open handle whose metadata must not change
//! while it is read. A production executable must belong to root, as must
//! every directory above it, and none of them may be writable by anyone
//! else: only root could swap it between the check and the launch.

use crate::authority::ids::Digest;
use crate::authority::AuthorityError;
use std::path::{Path, PathBuf};

/// The largest executable read for its digest.
const MAX_EXECUTABLE: u64 = 768 * 1024 * 1024;

/// Who may own an executable and the directories above it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Trust {
    /// Root, and writable by no one else, up to `/`.
    System,
    /// Test fixtures only: any owner, writable by no one else; the
    /// directories above are not checked.
    #[cfg(test)]
    Fixture,
}

/// A pinned executable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutableIdentity {
    pub path: PathBuf,
    pub digest: Digest,
}

/// Inspect `path` and return its identity.
#[cfg(target_os = "linux")]
pub(crate) fn inspect(path: &Path, trust: Trust) -> Result<ExecutableIdentity, AuthorityError> {
    use sha2::{Digest as _, Sha256};
    use std::io::Read;
    use std::os::unix::fs::MetadataExt;

    let untrusted = AuthorityError::Closed("the executable is not in a trusted location");
    if !path.is_absolute() {
        return Err(AuthorityError::InvalidAction(
            "an executable path must be absolute",
        ));
    }
    let canonical = std::fs::canonicalize(path)
        .map_err(|_| AuthorityError::Unavailable("the executable is not installed"))?;
    if canonical != path {
        return Err(AuthorityError::Closed(
            "an executable is named by its canonical path",
        ));
    }
    let writable_by_others = |mode: u32| mode & 0o022 != 0;
    if trust == Trust::System {
        let mut dir = canonical.parent();
        while let Some(current) = dir {
            let meta = std::fs::symlink_metadata(current).map_err(|_| untrusted.clone())?;
            if !meta.is_dir() || meta.uid() != 0 || writable_by_others(meta.mode()) {
                return Err(untrusted);
            }
            dir = current.parent();
        }
    }
    let mut file = std::fs::File::open(&canonical)
        .map_err(|_| AuthorityError::Unavailable("the executable cannot be opened"))?;
    let before = file
        .metadata()
        .map_err(|_| AuthorityError::Unavailable("the executable cannot be read"))?;
    let owner_ok = match trust {
        Trust::System => before.uid() == 0,
        #[cfg(test)]
        Trust::Fixture => true,
    };
    if !before.is_file()
        || !owner_ok
        || writable_by_others(before.mode())
        || before.mode() & 0o111 == 0
    {
        return Err(untrusted);
    }
    if before.len() > MAX_EXECUTABLE {
        return Err(AuthorityError::Closed("the executable is too large to pin"));
    }
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut read_total = 0u64;
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|_| AuthorityError::Unavailable("the executable cannot be read"))?;
        if n == 0 {
            break;
        }
        read_total += n as u64;
        if read_total > before.len() {
            return Err(AuthorityError::TargetChanged);
        }
        hasher.update(&buffer[..n]);
    }
    let after = file
        .metadata()
        .map_err(|_| AuthorityError::Unavailable("the executable cannot be read"))?;
    let unchanged = read_total == before.len()
        && (
            before.dev(),
            before.ino(),
            before.len(),
            before.mtime(),
            before.mtime_nsec(),
        ) == (
            after.dev(),
            after.ino(),
            after.len(),
            after.mtime(),
            after.mtime_nsec(),
        );
    if !unchanged {
        return Err(AuthorityError::TargetChanged);
    }
    let sha256: [u8; 32] = hasher.finalize().into();
    let digest = Digest::of(
        "nexus.p3.executable.identity.v1",
        &[
            canonical.as_os_str().as_encoded_bytes(),
            &before.dev().to_be_bytes(),
            &before.ino().to_be_bytes(),
            &before.len().to_be_bytes(),
            &before.mtime().to_be_bytes(),
            &before.mtime_nsec().to_be_bytes(),
            &before.mode().to_be_bytes(),
            &before.uid().to_be_bytes(),
            &sha256,
        ],
    );
    Ok(ExecutableIdentity {
        path: canonical,
        digest,
    })
}

/// Resolve a tool's executable from its fixed list of trusted absolute
/// locations (M0-FIX2 F13), in order. The first location that is present
/// (anything at that path, even a dangling link) decides; only a location
/// with nothing at it is passed over. The present location is accepted
/// only if its canonical path is itself one of the listed locations (so a
/// symbolic link is followed only to another listed location, never to an
/// arbitrary target) and that canonical path passes [`inspect`]; otherwise
/// the tool is refused with that reason, and no later location is tried,
/// so a tampered or misconfigured location is never hidden by another.
/// With no location present the tool is refused. There is no `PATH` lookup
/// and no other fallback, and what is returned (and pinned, and launched)
/// is the canonical path.
///
/// Hard links: `inspect` does not refuse a file with several names (the
/// rust-coreutils multicall binary is one inode with a name per tool).
/// Every name of it is root-owned in root-owned directories, its identity
/// pins the inode and its contents, and the name launched is the listed
/// canonical path, whose final component (`argv[0]`) is what the multicall
/// binary dispatches on.
#[cfg(target_os = "linux")]
pub(crate) fn resolve(
    candidates: &[&str],
    trust: Trust,
) -> Result<ExecutableIdentity, AuthorityError> {
    if candidates.is_empty() {
        return Err(AuthorityError::Closed("the tool names no trusted location"));
    }
    if candidates.iter().any(|c| !Path::new(c).is_absolute()) {
        return Err(AuthorityError::InvalidAction(
            "an executable path must be absolute",
        ));
    }
    for candidate in candidates {
        match std::fs::symlink_metadata(candidate) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                return Err(AuthorityError::Closed(
                    "the executable is not in a trusted location",
                ))
            }
        }
        // Present: this location decides.
        let canonical = std::fs::canonicalize(candidate)
            .map_err(|_| AuthorityError::Unavailable("the executable is not installed"))?;
        if !candidates
            .iter()
            .any(|listed| Path::new(listed) == canonical)
        {
            return Err(AuthorityError::Closed(
                "the executable resolves outside its trusted locations",
            ));
        }
        return inspect(&canonical, trust);
    }
    Err(AuthorityError::Unavailable(
        "the executable is not installed",
    ))
}

/// The most entries an installation directory may hold.
#[cfg(target_os = "linux")]
const MAX_INSTALLATION: usize = 8192;

/// Every entry under `dir` (an executable's own installation directory)
/// belongs to root and, unless it is a symbolic link, is writable by no one
/// else: what the executable loads or runs from beside itself cannot be
/// replaced by another user either. The directory itself is one of the
/// executable's ancestors, which `inspect` checks.
#[cfg(target_os = "linux")]
pub(crate) fn inspect_tree(dir: &Path) -> Result<(), AuthorityError> {
    use std::os::unix::fs::MetadataExt;
    let untrusted =
        AuthorityError::Closed("the executable's installation is not in a trusted location");
    let mut pending = vec![dir.to_path_buf()];
    let mut seen = 0usize;
    while let Some(current) = pending.pop() {
        for entry in std::fs::read_dir(&current).map_err(|_| untrusted.clone())? {
            let path = entry.map_err(|_| untrusted.clone())?.path();
            seen += 1;
            if seen > MAX_INSTALLATION {
                return Err(AuthorityError::Closed(
                    "the executable's installation is too large to check",
                ));
            }
            let meta = std::fs::symlink_metadata(&path).map_err(|_| untrusted.clone())?;
            let link = meta.file_type().is_symlink();
            if meta.uid() != 0 || (!link && meta.mode() & 0o022 != 0) {
                return Err(untrusted);
            }
            if meta.is_dir() {
                pending.push(path);
            }
        }
    }
    Ok(())
}

/// Other platforms: governed launches are not available.
#[cfg(not(target_os = "linux"))]
pub(crate) fn inspect(_path: &Path, _trust: Trust) -> Result<ExecutableIdentity, AuthorityError> {
    Err(AuthorityError::Unavailable(
        "governed real-world control is available on Linux only",
    ))
}

/// Other platforms: governed launches are not available.
#[cfg(not(target_os = "linux"))]
pub(crate) fn resolve(
    _candidates: &[&str],
    _trust: Trust,
) -> Result<ExecutableIdentity, AuthorityError> {
    Err(AuthorityError::Unavailable(
        "governed real-world control is available on Linux only",
    ))
}

/// Other platforms: governed launches are not available.
#[cfg(not(target_os = "linux"))]
pub(crate) fn inspect_tree(_dir: &Path) -> Result<(), AuthorityError> {
    Err(AuthorityError::Unavailable(
        "governed real-world control is available on Linux only",
    ))
}

#[cfg(all(test, target_os = "linux"))]
mod tests;
