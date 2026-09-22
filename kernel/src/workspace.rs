//! Shared path containment for native agent filesystem operations.
//!
//! The caller supplies the trusted workspace root; a model-provided path must
//! never be promoted to a new root. This is path validation, not an OS sandbox:
//! callers must not assume it prevents concurrent filesystem replacement races.

use crate::errors::AgentError;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

fn denied() -> AgentError {
    AgentError::CapabilityDenied(
        "filesystem path is outside the workspace or unsafe to resolve".into(),
    )
}

/// Resolve an existing target, or a new target's nearest existing ancestor,
/// within the canonical workspace. `Path::starts_with` compares components,
/// not strings. Absolute paths inside the workspace are accepted.
///
/// Only the configured root may be initialized here. No requested target or
/// parent is created during validation. Missing suffixes must consist entirely
/// of normal components; unresolved `..` and dangling symlinks fail closed.
pub fn resolve_path(workspace: &Path, requested: &Path) -> Result<PathBuf, AgentError> {
    std::fs::create_dir_all(workspace)
        .map_err(|e| AgentError::SupervisorError(format!("initialize workspace: {e}")))?;
    let root = workspace
        .canonicalize()
        .map_err(|e| AgentError::SupervisorError(format!("resolve workspace: {e}")))?;
    resolve_under_root(&root, requested)
}

/// Resolve a relative target under an already-existing canonical authority root.
/// Never creates the root or target. A missing/repointed root fails closed.
/// Parent, absolute and platform-prefix components are rejected, even if they
/// could normalize inside the root. Shares the P0-002A symlink/containment rules.
/// This is pathname validation, not protection against concurrent namespace
/// replacement or hard links.
pub fn resolve_existing_relative(root: &Path, requested: &Path) -> Result<PathBuf, AgentError> {
    if !root.is_absolute()
        || !root.is_dir()
        || root.canonicalize().map_err(|_| denied())? != root
        || requested.as_os_str().is_empty()
        || requested
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(denied());
    }
    resolve_under_root(root, requested)
}

fn resolve_under_root(root: &Path, requested: &Path) -> Result<PathBuf, AgentError> {
    let mut ancestor = root.join(requested);
    let mut suffix = Vec::new();

    let canonical = loop {
        match std::fs::symlink_metadata(&ancestor) {
            Ok(metadata) => {
                let canonical = ancestor.canonicalize().map_err(|e| {
                    if metadata.file_type().is_symlink() {
                        denied()
                    } else {
                        AgentError::SupervisorError(format!("resolve filesystem target: {e}"))
                    }
                })?;
                break canonical;
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {
                match ancestor.components().next_back() {
                    Some(Component::Normal(name)) => suffix.push(name.to_os_string()),
                    _ => return Err(denied()),
                }
                if !ancestor.pop() {
                    return Err(denied());
                }
            }
            Err(e) => {
                return Err(AgentError::SupervisorError(format!(
                    "inspect filesystem target: {e}"
                )))
            }
        }
    };

    if !canonical.starts_with(root) {
        return Err(denied());
    }
    append_missing_components(root, canonical, suffix)
}

fn append_missing_components(
    root: &Path,
    mut resolved: PathBuf,
    suffix: Vec<OsString>,
) -> Result<PathBuf, AgentError> {
    for name in suffix.into_iter().rev() {
        resolved.push(name);
    }
    // Re-parsing an OsString in push() can have platform-specific prefix/root
    // semantics. Recheck the assembled path, not just the existing ancestor.
    if !resolved.starts_with(root) {
        return Err(denied());
    }
    Ok(resolved)
}

/// Resolve the directory entry to unlink, without following its final symlink.
/// The root and effective parent must exist; neither is created here. A missing
/// leaf may be returned so callers can retain their no-op-on-missing policy.
/// Like resolve_path(), this does not prevent concurrent namespace replacement.
pub fn resolve_entry_for_unlink(workspace: &Path, requested: &Path) -> Result<PathBuf, AgentError> {
    let root = workspace
        .canonicalize()
        .map_err(|e| AgentError::SupervisorError(format!("resolve workspace: {e}")))?;
    let leaf = requested.file_name().ok_or_else(denied)?;
    let mut components = Path::new(leaf).components();
    if !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        // Do not reinterpret `file/` or `file/.` as an unlink of `file`.
        || !requested.as_os_str().as_encoded_bytes().ends_with(leaf.as_encoded_bytes())
    {
        return Err(denied());
    }

    let candidate = root.join(requested);
    let parent = candidate.parent().ok_or_else(denied)?;
    let parent = parent
        .canonicalize()
        .map_err(|e| AgentError::SupervisorError(format!("resolve unlink parent: {e}")))?;
    if !parent.starts_with(&root) {
        return Err(denied());
    }
    if !parent.is_dir() {
        return Err(AgentError::SupervisorError(
            "unlink parent is not a directory".into(),
        ));
    }
    let entry = parent.join(leaf);
    if !entry.starts_with(&root) {
        return Err(denied());
    }
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p0_002a_reconstruction_rechecks_assembled_path() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path().join("workspace");
        let outside = temp.path().join("outside");
        assert!(matches!(
            append_missing_components(&root, root.clone(), vec![outside.into_os_string()]),
            Err(AgentError::CapabilityDenied(_))
        ));
        assert_eq!(
            append_missing_components(
                &root,
                root.clone(),
                vec!["file.txt".into(), "nested".into()]
            )
            .unwrap(),
            root.join("nested").join("file.txt")
        );
    }

    #[test]
    fn p0_002a_unlink_requires_an_exact_leaf_and_existing_parent() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("file.txt"), b"inside evidence").unwrap();
        for path in ["", ".", "..", "file.txt/", "file.txt/."] {
            assert!(
                matches!(
                    resolve_entry_for_unlink(temp.path(), Path::new(path)),
                    Err(AgentError::CapabilityDenied(_))
                ),
                "{path:?}"
            );
        }
        assert!(resolve_entry_for_unlink(temp.path(), Path::new("missing/leaf")).is_err());
        assert!(!temp.path().join("missing").exists());
        assert_eq!(
            std::fs::read(temp.path().join("file.txt")).unwrap(),
            b"inside evidence"
        );
    }

    #[cfg(windows)]
    #[test]
    fn p0_002a_reconstruction_rechecks_windows_prefix_and_root_components() {
        let root = PathBuf::from(r"C:\workspace");
        // These components exercise the actual assembly function without
        // depending on Windows returning NotFound rather than InvalidFilename.
        for suffix in [r"D:escape", r"D:\escape", r"\escape"] {
            assert!(matches!(
                append_missing_components(&root, root.clone(), vec![suffix.into()]),
                Err(AgentError::CapabilityDenied(_))
            ));
        }
        assert_eq!(
            append_missing_components(
                &root,
                root.clone(),
                vec!["file.txt".into(), "nested".into()]
            )
            .unwrap(),
            root.join("nested").join("file.txt")
        );
    }
}
