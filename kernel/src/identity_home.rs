//! The validated identity home: the backend-owned root for per-user Nexus
//! application state (P0-002C5B).
//!
//! HOME must be a non-empty absolute path. On Windows only, when HOME is
//! absent, the native profile folder is used under the same check. A malformed
//! HOME never selects another root, and nothing falls back to the process
//! working directory, a shared temporary directory or a literal `~`.
//!
//! This is an application-state location, not workspace authority: nothing
//! derived here is a [`crate::workspace_authority::WorkspaceGrant`].

use std::ffi::OsString;
use std::path::PathBuf;

/// No valid identity home could be resolved. Carries no path text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("no valid absolute identity home directory could be resolved")]
pub struct IdentityHomeMissing;

/// The validated identity home of the current process.
pub fn identity_home() -> Result<PathBuf, IdentityHomeMissing> {
    let home = std::env::var_os("HOME");
    #[cfg(windows)]
    let native_windows_home = if home.is_none() {
        dirs::home_dir()
    } else {
        None
    };
    #[cfg(not(windows))]
    let native_windows_home = None;
    resolve_identity_home_from(home, native_windows_home, cfg!(windows))
}

/// Select once, then validate. A malformed HOME must never select another root.
pub fn resolve_identity_home_from(
    home: Option<OsString>,
    native_windows_home: Option<PathBuf>,
    allow_windows_fallback: bool,
) -> Result<PathBuf, IdentityHomeMissing> {
    let home = match home {
        Some(home) => PathBuf::from(home),
        None if allow_windows_fallback => native_windows_home.ok_or(IdentityHomeMissing)?,
        None => return Err(IdentityHomeMissing),
    };
    if home.as_os_str().is_empty() || !home.is_absolute() {
        return Err(IdentityHomeMissing);
    }
    Ok(home)
}

/// `<identity home>/.nexus`.
pub fn nexus_state_dir() -> Result<PathBuf, IdentityHomeMissing> {
    Ok(identity_home()?.join(".nexus"))
}

/// `<identity home>/.nexus/<relative>` for a fixed, backend-chosen relative
/// location such as `"notes"` or `"desktop-backend/computer-control"`.
pub fn nexus_state_path(relative: &'static str) -> Result<PathBuf, IdentityHomeMissing> {
    let dir = nexus_state_dir()?;
    crate::governed_path::join_relative(&dir, relative).map_err(|_| IdentityHomeMissing)
}

/// The Nexus database. `NEXUS_DB_PATH` remains the recorded operator
/// state-location override (see the C5 authority inventory); otherwise the
/// database lives under the validated identity home.
pub fn nexus_db_path() -> Result<PathBuf, IdentityHomeMissing> {
    if let Some(path) = std::env::var_os("NEXUS_DB_PATH") {
        return operator_override(path);
    }
    nexus_state_path("nexus.db")
}

/// An operator state-location override (`NEXUS_DB_PATH`,
/// `NEXUS_CONFIG_PATH`). It is process configuration set by the operator,
/// never by the interface or a model, and relocates application state only.
/// It must be a non-empty absolute path: an empty or relative value yields no
/// location rather than one resolved against the working directory
/// (P0-002C5C).
pub fn operator_override(value: OsString) -> Result<PathBuf, IdentityHomeMissing> {
    let path = PathBuf::from(value);
    if path.as_os_str().is_empty() || !path.is_absolute() {
        return Err(IdentityHomeMissing);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn absolute(name: &str) -> PathBuf {
        let base = if cfg!(windows) { "C:\\" } else { "/" };
        PathBuf::from(base).join(name)
    }

    #[test]
    fn a_valid_home_is_used_exactly() {
        let home = absolute("users/nexus");
        assert_eq!(
            resolve_identity_home_from(Some(home.clone().into()), None, false),
            Ok(home)
        );
    }

    #[test]
    fn empty_relative_and_absent_homes_never_select_another_root() {
        for home in ["", ".", "relative/home", "~", "~/nexus"] {
            assert_eq!(
                resolve_identity_home_from(Some(home.into()), Some(absolute("native")), true),
                Err(IdentityHomeMissing),
                "{home:?}"
            );
        }
        assert_eq!(
            resolve_identity_home_from(None, Some(absolute("native")), false),
            Err(IdentityHomeMissing)
        );
        assert_eq!(
            resolve_identity_home_from(None, None, true),
            Err(IdentityHomeMissing)
        );
        assert_eq!(
            resolve_identity_home_from(None, Some(PathBuf::from("relative")), true),
            Err(IdentityHomeMissing)
        );
    }

    #[test]
    fn only_an_absent_home_may_use_the_native_windows_profile() {
        assert_eq!(
            resolve_identity_home_from(None, Some(absolute("native")), true),
            Ok(absolute("native"))
        );
        assert_eq!(
            resolve_identity_home_from(
                Some(absolute("home").into()),
                Some(absolute("native")),
                true
            ),
            Ok(absolute("home"))
        );
    }

    #[test]
    fn p0_002c5c_operator_overrides_are_absolute_or_no_location() {
        let path = absolute("srv/nexus/nexus.db");
        assert_eq!(operator_override(path.clone().into()), Ok(path));
        for value in [
            "",
            ".",
            "nexus.db",
            "./nexus.db",
            "../nexus.db",
            "~/nexus.db",
        ] {
            assert_eq!(
                operator_override(value.into()),
                Err(IdentityHomeMissing),
                "{value:?}"
            );
        }
    }
}
