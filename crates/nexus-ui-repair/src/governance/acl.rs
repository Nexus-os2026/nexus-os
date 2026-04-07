//! I-2 Layer 1 — filesystem ACL. See v1.1 §3.1.
//!
//! This is the in-process, defense-in-depth filesystem allowlist. It is
//! *not* a security boundary against malicious code in the scout — it
//! is a structural guarantee against accidental writes by correct code.
//! Layers 2 and 3 (per-app input governance + OS-level isolation) land
//! in Phase 1.3 alongside the first `nexus-computer-use` import.

use std::path::{Component, Path, PathBuf};

/// Filesystem ACL controlling where the scout is permitted to write.
///
/// Holds an allowlist of roots. Every write path must, after traversal
/// rejection and canonicalization of its deepest existing ancestor,
/// canonicalize to a path under one of those roots.
#[derive(Debug, Clone)]
pub struct Acl {
    allowed_write_roots: Vec<PathBuf>,
}

impl Acl {
    /// Construct an ACL with a custom set of allowed write roots.
    ///
    /// Unlike Phase 1.1, roots are **not** canonicalized at construction
    /// time — they may not exist yet on first run, and
    /// `std::fs::canonicalize` fails on non-existent paths. Canonicalization
    /// happens lazily in `check_write`.
    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        Self {
            allowed_write_roots: roots,
        }
    }

    /// The default scout ACL: `~/.nexus/ui-repair/reports/` and
    /// `~/.nexus/ui-repair/sessions/`.
    ///
    /// `HOME` is read from the environment. If `HOME` is unset the scout
    /// cannot safely derive a sandbox, so we panic — this is a test/dev
    /// concern, not a runtime concern the driver can recover from.
    pub fn default_scout() -> Self {
        let home = std::env::var("HOME")
            .expect("HOME environment variable must be set for Acl::default_scout()");
        let base = PathBuf::from(home).join(".nexus").join("ui-repair");
        let reports = base.join("reports");
        let sessions = base.join("sessions");
        Self::with_roots(vec![reports, sessions])
    }

    /// Verify that `path` may be written.
    ///
    /// Enforcement strategy (v1.1 §3.1 Layer 1):
    ///
    /// 1. The path is made absolute (joined against `current_dir` if
    ///    relative).
    /// 2. Any `..` component anywhere in the path triggers immediate
    ///    `Error::AclDenied`. This rejects traversal attempts even if
    ///    the path doesn't exist yet — `canonicalize` would fail with
    ///    `Io` for non-existent paths, which we don't want.
    /// 3. The deepest existing ancestor of the absolute path is
    ///    canonicalized (resolving any symlinks in the prefix that does
    ///    exist).
    /// 4. The canonical ancestor must be a descendant of at least one
    ///    allowed root. Roots that exist are canonicalized; roots that
    ///    do not yet exist are compared in their lexical absolute form
    ///    so that first-run setup (before any writes) still works.
    pub fn check_write(&self, path: &Path) -> crate::Result<()> {
        // Step 1: absolutize.
        let abs = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };

        // Step 2: reject `..` traversal anywhere in the path.
        for component in abs.components() {
            if matches!(component, Component::ParentDir) {
                return Err(crate::Error::AclDenied(path.to_path_buf()));
            }
        }

        // Step 3: canonicalize the deepest existing ancestor.
        let mut ancestor = abs.clone();
        while !ancestor.exists() {
            if !ancestor.pop() {
                return Err(crate::Error::AclDenied(path.to_path_buf()));
            }
        }
        let canonical_ancestor = std::fs::canonicalize(&ancestor)?;

        // The canonical form of the full target path is the canonical
        // ancestor joined with the tail that didn't yet exist.
        let tail = abs
            .strip_prefix(&ancestor)
            .unwrap_or_else(|_| Path::new(""));
        let canonical_target = canonical_ancestor.join(tail);

        // Step 4: test against each allowed root.
        for root in &self.allowed_write_roots {
            let canonical_root = match std::fs::canonicalize(root) {
                Ok(c) => c,
                Err(_) => {
                    // Root doesn't exist yet — fall back to the lexical
                    // absolute form so that first-run writes (before
                    // any files land on disk) still work.
                    if root.is_absolute() {
                        root.clone()
                    } else {
                        match std::env::current_dir() {
                            Ok(cwd) => cwd.join(root),
                            Err(_) => continue,
                        }
                    }
                }
            };

            if canonical_target.starts_with(&canonical_root) {
                return Ok(());
            }
        }

        Err(crate::Error::AclDenied(path.to_path_buf()))
    }

    /// Check that `path` may be written, then create any missing parent
    /// directories.
    ///
    /// The ACL check runs **first** — we never create directories
    /// outside the sandbox, even transiently.
    pub fn ensure_parent_dirs(&self, path: &Path) -> crate::Result<()> {
        self.check_write(path)?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }
        Ok(())
    }

    /// The current set of allowed roots (as provided to the constructor,
    /// not canonicalized). Useful for debugging and for tests.
    pub fn allowed_roots(&self) -> &[PathBuf] {
        &self.allowed_write_roots
    }
}
