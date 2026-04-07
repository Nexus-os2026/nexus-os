//! I-2 Layer 1 — filesystem ACL. See v1.1 §3.1.
//!
//! This is the in-process, defense-in-depth filesystem allowlist. It is
//! *not* a security boundary against malicious code in the scout — it
//! is a structural guarantee against accidental writes by correct code.
//! Layers 2 and 3 (per-app input governance + OS-level isolation) land
//! in Phase 1.3 alongside the first `nexus-computer-use` import.

use std::path::{Path, PathBuf};

/// Filesystem ACL controlling where the scout is permitted to write.
///
/// Holds an allowlist of canonical roots. Every write path's parent
/// directory must canonicalize to a path under one of those roots.
#[derive(Debug, Clone)]
pub struct Acl {
    allowed_write_roots: Vec<PathBuf>,
}

impl Acl {
    /// Construct an ACL with a custom set of allowed write roots.
    ///
    /// Each root is canonicalized at construction time so subsequent
    /// `check_write` calls can do a direct prefix comparison against a
    /// stable canonical form. Non-existent roots are skipped silently
    /// (the constructor does not create directories).
    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        let canonical_roots: Vec<PathBuf> = roots
            .into_iter()
            .filter_map(|p| std::fs::canonicalize(&p).ok())
            .collect();
        Self {
            allowed_write_roots: canonical_roots,
        }
    }

    /// The default scout ACL: `~/.nexus/ui-repair/reports/` and
    /// `~/.nexus/ui-repair/sessions/`. The home directory is read from
    /// the `HOME` environment variable. Roots that do not exist on disk
    /// are skipped — the caller is expected to create them first.
    pub fn default_scout() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
        let base = PathBuf::from(home).join(".nexus").join("ui-repair");
        let reports = base.join("reports");
        let sessions = base.join("sessions");
        Self::with_roots(vec![reports, sessions])
    }

    /// Verify that `path` may be written.
    ///
    /// The check canonicalizes the *parent* of the requested path and
    /// asserts that the canonical parent starts with at least one of
    /// the allowed roots. This rejects `../` traversal attempts because
    /// canonicalization resolves them.
    ///
    /// Returns `Error::AclDenied` if the parent cannot be canonicalized
    /// (typically because it does not exist) or does not lie under any
    /// allowed root.
    pub fn check_write(&self, path: &Path) -> crate::Result<()> {
        let parent = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => return Err(crate::Error::AclDenied(path.to_path_buf())),
        };

        let canonical_parent = match std::fs::canonicalize(parent) {
            Ok(p) => p,
            Err(_) => return Err(crate::Error::AclDenied(path.to_path_buf())),
        };

        for root in &self.allowed_write_roots {
            if canonical_parent.starts_with(root) {
                return Ok(());
            }
        }

        Err(crate::Error::AclDenied(path.to_path_buf()))
    }

    /// The current set of (canonical) allowed roots. Useful for
    /// debugging and for tests.
    pub fn allowed_roots(&self) -> &[PathBuf] {
        &self.allowed_write_roots
    }
}
