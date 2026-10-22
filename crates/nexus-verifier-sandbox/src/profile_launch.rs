//! A verifier launch built only from backend-owned objects (Linux).
//!
//! The compiled-in profile fixes the executable, the arguments, the working
//! directory and the environment; the verified toolchain and the
//! verification workspace supply the descriptors and the paths those values
//! name. The toolchain is re-verified and every workspace path re-checked
//! while the launch is built; nothing comes from a caller.

use std::collections::BTreeSet;
use std::ffi::{CString, OsStr};
use std::io::Read;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use crate::applicability::{self, NotApplicable, RustPackage};
use crate::launcher::LaunchSpec;
use crate::policy::Role;
use crate::profile::{Location, Value, VerifierProfile};
use crate::sys;
use crate::toolchain::{ToolchainError, VerifiedVerifierToolchain, HOST_TARGET, VERIFIER_TARGET};
use crate::workspace::{Area, Workspace, WorkspaceError};

#[derive(Debug)]
pub enum LaunchSetupError {
    /// The verified toolchain is not the one the profile requires.
    ToolchainMismatch,
    Toolchain(ToolchainError),
    Workspace(WorkspaceError),
    /// `/dev/null` or `/dev/urandom` is not the expected device.
    Device,
    Io(std::io::Error),
}

fn area(location: Location) -> Option<Area> {
    match location {
        Location::ToolchainRoot => None,
        Location::Input => Some(Area::Input),
        Location::Target => Some(Area::Target),
        Location::Home => Some(Area::Home),
        Location::Tmp => Some(Area::Tmp),
        Location::CargoHome => Some(Area::CargoHome),
    }
}

/// A fixed character device, checked by its device number.
fn device(path: &std::ffi::CStr, major: u32, minor: u32) -> Result<OwnedFd, LaunchSetupError> {
    let fd = sys::open_fixed(path, libc::O_PATH).map_err(LaunchSetupError::Io)?;
    let st = sys::fstat(fd.as_fd()).map_err(LaunchSetupError::Io)?;
    if sys::file_kind(&st) != libc::S_IFCHR || st.st_rdev != libc::makedev(major, minor) {
        return Err(LaunchSetupError::Device);
    }
    Ok(fd)
}

/// Build the launch of `profile` for execution `generation`.
pub fn launch_spec(
    profile: &VerifierProfile,
    toolchain: &VerifiedVerifierToolchain,
    workspace: &Workspace,
    generation: u64,
) -> Result<LaunchSpec, LaunchSetupError> {
    let required = profile.toolchain;
    if required.name != "rust"
        || required.version != toolchain.rust_version()
        || required.host != HOST_TARGET
        || required.target != VERIFIER_TARGET
        || profile.executable != toolchain.entry()
    {
        return Err(LaunchSetupError::ToolchainMismatch);
    }
    toolchain.reverify().map_err(LaunchSetupError::Toolchain)?;
    workspace
        .verify_paths()
        .map_err(LaunchSetupError::Workspace)?;
    let material = toolchain.launch().map_err(LaunchSetupError::Toolchain)?;
    let base = |location: Location| -> PathBuf {
        match area(location) {
            Some(area) => workspace.env_path(area),
            None => material.root.clone(),
        }
    };
    let mut env = Vec::with_capacity(profile.env.len());
    for var in profile.env {
        let mut entry = format!("{}=", var.key).into_bytes();
        match var.value {
            Value::Literal(text) => entry.extend_from_slice(text.as_bytes()),
            Value::Path(location, relative) => {
                let mut path = base(location);
                if !relative.is_empty() {
                    path.push(relative);
                }
                entry.extend_from_slice(OsStr::new(&path).as_bytes());
            }
        }
        env.push(entry);
    }
    let mut argv = vec![profile.argv0.as_bytes().to_vec()];
    argv.extend(profile.args.iter().map(|arg| arg.as_bytes().to_vec()));
    let cwd = area(profile.cwd).ok_or(LaunchSetupError::ToolchainMismatch)?;
    let mut rules = material.rules;
    rules.extend(workspace.rules().map_err(LaunchSetupError::Io)?);
    rules.push((Role::DevNull, device(c"/dev/null", 1, 3)?));
    rules.push((Role::DevUrandom, device(c"/dev/urandom", 1, 9)?));
    Ok(LaunchSpec {
        generation,
        executable: material.executable,
        working_directory: workspace.directory(cwd).map_err(LaunchSetupError::Io)?,
        argv,
        env,
        rules,
    })
}

/// The largest manifest or lock file read for applicability.
const MAX_METADATA_BYTES: u64 = 1 << 20;

/// Whether `rust.cargo-test.offline.v1` applies to the candidate
/// materialized in `workspace`'s input, read through its retained
/// descriptor.
pub fn applicability(
    workspace: &Workspace,
) -> Result<Result<RustPackage, NotApplicable>, std::io::Error> {
    let input = workspace.directory(Area::Input)?;
    let mut files = BTreeSet::new();
    if !list(input.as_fd(), "", &mut files)? {
        return Ok(Err(NotApplicable::UnsupportedPath));
    }
    let read = |path: &str| read_file(input.as_fd(), path);
    Ok(applicability::check(&files, &read))
}

/// Every regular file beneath `dir`; `false` for a name that is not UTF-8.
/// A symlink or special file is an error (the input was proven free of
/// them).
fn list(dir: BorrowedFd<'_>, prefix: &str, files: &mut BTreeSet<String>) -> std::io::Result<bool> {
    for name in sys::directory_entries(dir)? {
        let Ok(text) = name.to_str() else {
            return Ok(false);
        };
        let path = if prefix.is_empty() {
            text.to_owned()
        } else {
            format!("{prefix}/{text}")
        };
        let st = sys::stat_at(dir, &name)?;
        match sys::file_kind(&st) {
            libc::S_IFDIR => {
                let child = sys::open_dir_at(dir, &name)?;
                if !list(child.as_fd(), &path, files)? {
                    return Ok(false);
                }
            }
            libc::S_IFREG => {
                files.insert(path);
            }
            _ => {
                return Err(std::io::Error::other(
                    "input entry is not a file or directory",
                ))
            }
        }
    }
    Ok(true)
}

/// A small regular file beneath `dir` by relative path, never following a
/// link.
fn read_file(dir: BorrowedFd<'_>, path: &str) -> Option<Vec<u8>> {
    let mut parts: Vec<&str> = path.split('/').collect();
    let name = CString::new(parts.pop()?).ok()?;
    let mut owned: Option<OwnedFd> = None;
    for part in parts {
        let parent = owned.as_ref().map_or(dir, |fd| fd.as_fd());
        owned = Some(sys::open_dir_at(parent, &CString::new(part).ok()?).ok()?);
    }
    let parent = owned.as_ref().map_or(dir, |fd| fd.as_fd());
    let file = sys::open_file_at(parent, &name).ok()?;
    let st = sys::fstat(file.as_fd()).ok()?;
    if sys::file_kind(&st) != libc::S_IFREG || st.st_size as u64 > MAX_METADATA_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::from(file)
        .take(MAX_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(bytes)
}
