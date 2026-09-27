//! Lexical and native checks for deriving files beneath a backend-owned root
//! (P0-002C5B). A caller string is never a path: it may name something beneath
//! a root only after one of these grammars accepts it, and a root must be an
//! existing canonical directory that is not a redirect.
//!
//! These are pathname checks, not OS isolation. A same-user process that
//! changes the namespace between a check and its use is out of scope.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::Metadata;
use std::io;
use std::path::{Path, PathBuf};

/// Why a governed path was refused. Carries no path text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PathDenied {
    #[error("not a valid relative path")]
    InvalidRelative,
    #[error("not a valid name")]
    InvalidName,
    #[error("root is not an existing canonical directory")]
    InvalidRoot,
    #[error("path is a symbolic link or reparse point")]
    Redirected,
    #[error("path is not the expected kind of object")]
    WrongKind,
    #[error("filesystem object is unavailable")]
    Unavailable,
}

/// Longest governed relative path, in bytes.
pub const MAX_RELATIVE_BYTES: usize = 240;
/// Longest single component, in bytes.
pub const MAX_COMPONENT_BYTES: usize = 128;
/// Deepest governed relative path, in components.
pub const MAX_DEPTH: usize = 16;

/// A portable relative path: `/`-separated components, each accepted by
/// [`validate_component`]. No absolute, prefixed, `.`, `..`, empty or
/// backslash-separated form passes.
pub fn validate_relative(relative: &str) -> Result<(), PathDenied> {
    if relative.is_empty() || relative.len() > MAX_RELATIVE_BYTES {
        return Err(PathDenied::InvalidRelative);
    }
    let mut depth = 0;
    for component in relative.split('/') {
        depth += 1;
        if depth > MAX_DEPTH || validate_component(component).is_err() {
            return Err(PathDenied::InvalidRelative);
        }
    }
    Ok(())
}

/// One portable path component, the C4 Builder grammar: not empty, `.` or
/// `..`; no control character or any of `/\:<>"|?*` (so no separator, drive,
/// UNC or alternate-data-stream form); no trailing dot or space; not a DOS
/// device name.
pub fn validate_component(component: &str) -> Result<(), PathDenied> {
    if component.is_empty()
        || component.len() > MAX_COMPONENT_BYTES
        || component == "."
        || component == ".."
        || component.ends_with(['.', ' '])
        || component
            .chars()
            .any(|c| c.is_control() || "/\\:<>\"|?*".contains(c))
        || is_device_name(component)
    {
        return Err(PathDenied::InvalidName);
    }
    Ok(())
}

/// A caller identifier used as a storage file stem: ASCII letters, digits,
/// `.`, `_` and `-`, starting with a letter or digit, at most `max_len` bytes,
/// no trailing dot, and not a DOS device name.
pub fn validate_identifier(id: &str, max_len: usize) -> Result<(), PathDenied> {
    let bytes = id.as_bytes();
    if bytes.is_empty()
        || bytes.len() > max_len
        || !bytes[0].is_ascii_alphanumeric()
        || id.ends_with('.')
        || !bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        || is_device_name(id)
    {
        return Err(PathDenied::InvalidName);
    }
    Ok(())
}

/// A deterministic storage stem for an identifier that may legitimately fall
/// outside [`validate_identifier`] (for example a remote provider's message
/// id). A valid identifier is used as is unless it starts with `h-`; every
/// other value, and every `h-` value, becomes `h-` plus its SHA-256, so two
/// different identifiers never share a stem.
pub fn storage_stem(id: &str) -> String {
    if !id.starts_with("h-") && validate_identifier(id, MAX_COMPONENT_BYTES - 16).is_ok() {
        return id.to_string();
    }
    format!("h-{:x}", Sha256::digest(id.as_bytes()))
}

fn is_device_name(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_uppercase();
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    })
}

/// `root` joined with a relative path that [`validate_relative`] accepted,
/// one component at a time.
pub fn join_relative(root: &Path, relative: &str) -> Result<PathBuf, PathDenied> {
    validate_relative(relative)?;
    let mut path = root.to_path_buf();
    for component in relative.split('/') {
        path.push(component);
    }
    Ok(path)
}

/// An existing absolute directory that is its own canonical spelling and not
/// itself a symbolic link or reparse point.
pub fn existing_root(path: &Path) -> Result<(), PathDenied> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| PathDenied::InvalidRoot)?;
    if is_redirect(&metadata) {
        return Err(PathDenied::Redirected);
    }
    if !path.is_absolute()
        || !metadata.is_dir()
        || path.canonicalize().map_err(|_| PathDenied::InvalidRoot)? != path
    {
        return Err(PathDenied::InvalidRoot);
    }
    Ok(())
}

/// The regular file at a validated relative path beneath `root`, which must be
/// an existing absolute directory that is not itself a redirect. Every
/// directory on the way must be a real directory and the target a regular
/// file; none may be a symbolic link or reparse point.
pub fn regular_file_beneath(root: &Path, relative: &str) -> Result<PathBuf, PathDenied> {
    let root_metadata = std::fs::symlink_metadata(root).map_err(|_| PathDenied::InvalidRoot)?;
    if is_redirect(&root_metadata) {
        return Err(PathDenied::Redirected);
    }
    if !root.is_absolute() || !root_metadata.is_dir() {
        return Err(PathDenied::InvalidRoot);
    }
    validate_relative(relative)?;
    let components: Vec<&str> = relative.split('/').collect();
    let mut path = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        path.push(component);
        let metadata = std::fs::symlink_metadata(&path).map_err(unavailable)?;
        if is_redirect(&metadata) {
            return Err(PathDenied::Redirected);
        }
        let last = index + 1 == components.len();
        if (last && !metadata.is_file()) || (!last && !metadata.is_dir()) {
            return Err(PathDenied::WrongKind);
        }
    }
    Ok(path)
}

/// Whether metadata taken without following the final component describes a
/// symbolic link (Unix) or a reparse point (Windows).
pub fn is_redirect(metadata: &Metadata) -> bool {
    #[cfg(unix)]
    {
        metadata.file_type().is_symlink()
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
}

/// Native identity of a filesystem object: device and inode on Unix, volume
/// serial and 128-bit file id on Windows. Metadata only; never a credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileIdentity {
    device: u64,
    file: [u8; 16],
}

/// The identity of the object at `path`, which must not itself be a symbolic
/// link or reparse point.
pub fn identity_of(path: &Path) -> Result<FileIdentity, PathDenied> {
    native_identity(path)
}

#[cfg(unix)]
fn native_identity(path: &Path) -> Result<FileIdentity, PathDenied> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::symlink_metadata(path).map_err(unavailable)?;
    if is_redirect(&metadata) {
        return Err(PathDenied::Redirected);
    }
    let mut file = [0u8; 16];
    file[..8].copy_from_slice(&metadata.ino().to_le_bytes());
    Ok(FileIdentity {
        device: metadata.dev(),
        file,
    })
}

#[cfg(windows)]
fn native_identity(path: &Path) -> Result<FileIdentity, PathDenied> {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileIdInfo, GetFileInformationByHandleEx, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_INFO, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE,
    };
    let handle = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(path)
        .map_err(unavailable)?;
    if is_redirect(&handle.metadata().map_err(unavailable)?) {
        return Err(PathDenied::Redirected);
    }
    let mut info = std::mem::MaybeUninit::<FILE_ID_INFO>::uninit();
    // SAFETY: `handle` keeps the handle alive. The output buffer has the exact
    // type and size FileIdInfo requires, and is read only after success.
    let success = unsafe {
        GetFileInformationByHandleEx(
            handle.as_raw_handle(),
            FileIdInfo,
            info.as_mut_ptr().cast(),
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    };
    if success == 0 {
        return Err(PathDenied::Unavailable);
    }
    // SAFETY: the successful call initialized the entire FILE_ID_INFO buffer.
    let info = unsafe { info.assume_init() };
    Ok(FileIdentity {
        device: info.VolumeSerialNumber,
        file: info.FileId.Identifier,
    })
}

fn unavailable(_: io::Error) -> PathDenied {
    PathDenied::Unavailable
}

/// A private temporary directory with an unpredictable name, created
/// exclusively (owner-only on Unix) and removed when dropped. It is scratch
/// space for one operation, never workspace authority.
pub fn private_temp_dir(prefix: &str) -> io::Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(prefix);
    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt;
        std::fs::Permissions::from_mode(0o700)
    };
    #[cfg(unix)]
    builder.permissions(permissions);
    builder.tempdir()
}

#[cfg(test)]
mod tests;
