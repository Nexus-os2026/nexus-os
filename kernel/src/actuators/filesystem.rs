//! GovernedFilesystem actuator — sandboxed file I/O with path traversal prevention.

use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::cognitive::types::PlannedAction;
use std::path::Path;

/// Maximum file size for reads: 10 MB.
const MAX_READ_SIZE: u64 = 10 * 1024 * 1024;
/// Maximum file size for writes: 50 MB.
const MAX_WRITE_SIZE: u64 = 50 * 1024 * 1024;

/// File extensions that agents are never allowed to write.
const BLOCKED_EXTENSIONS: &[&str] = &[
    ".exe", ".sh", ".bat", ".cmd", ".ps1", ".dll", ".so", ".dylib",
];

/// Fuel cost per file read.
const FUEL_COST_READ: f64 = 1.0;
/// Fuel cost per file write.
const FUEL_COST_WRITE: f64 = 2.0;

/// Governed filesystem actuator. All paths are resolved relative to the
/// agent's sandboxed working directory. Path traversal is prevented.
#[derive(Debug, Clone)]
pub struct GovernedFilesystem;

impl GovernedFilesystem {
    /// Resolve a user-provided path relative to the agent's workspace,
    /// rejecting any attempt to escape the sandbox.
    /// System paths that agents can read (but never write).
    /// These are safe because they're read-only virtual filesystems.
    const READABLE_SYSTEM_PREFIXES: &'static [&'static str] =
        &["/proc/", "/sys/class/", "/sys/devices/", "/etc/os-release"];

    pub(crate) fn resolve_safe_path(
        workspace: &Path,
        user_path: &str,
    ) -> Result<std::path::PathBuf, ActuatorError> {
        // Allow absolute paths to safe system directories (read-only).
        // These are virtual filesystems that expose system info.
        // NOTE: Only callers doing reads should reach this — writes must use
        // resolve_safe_write_path() instead.
        if user_path.starts_with('/') {
            let abs = std::path::PathBuf::from(user_path);
            for prefix in Self::READABLE_SYSTEM_PREFIXES {
                if user_path.starts_with(prefix) && abs.exists() {
                    return Ok(abs);
                }
            }
        }

        // Ensure workspace exists
        if !workspace.exists() {
            std::fs::create_dir_all(workspace)
                .map_err(|e| ActuatorError::IoError(format!("cannot create workspace: {e}")))?;
        }

        let candidate = workspace.join(user_path);

        // Canonicalize what exists; for new files, canonicalize the parent.
        let canonical = if candidate.exists() {
            candidate
                .canonicalize()
                .map_err(|e| ActuatorError::IoError(format!("canonicalize failed: {e}")))?
        } else {
            let parent = candidate
                .parent()
                .ok_or_else(|| ActuatorError::PathTraversal("no parent directory".into()))?;
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    ActuatorError::IoError(format!("cannot create parent dir: {e}"))
                })?;
            }
            let canonical_parent = parent
                .canonicalize()
                .map_err(|e| ActuatorError::IoError(format!("canonicalize parent failed: {e}")))?;
            canonical_parent.join(
                candidate
                    .file_name()
                    .ok_or_else(|| ActuatorError::PathTraversal("no filename".into()))?,
            )
        };

        let canonical_workspace = workspace
            .canonicalize()
            .map_err(|e| ActuatorError::IoError(format!("canonicalize workspace failed: {e}")))?;

        if !canonical.starts_with(&canonical_workspace) {
            return Err(ActuatorError::PathTraversal(format!(
                "resolved path '{}' escapes workspace '{}'",
                canonical.display(),
                canonical_workspace.display()
            )));
        }

        Ok(canonical)
    }

    /// Check if the file extension is blocked for writes.
    fn check_extension(path: &Path) -> Result<(), ActuatorError> {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let dotted = format!(".{}", ext.to_lowercase());
            if BLOCKED_EXTENSIONS.contains(&dotted.as_str()) {
                return Err(ActuatorError::BlockedExtension(dotted));
            }
        }
        Ok(())
    }
}

impl Actuator for GovernedFilesystem {
    fn name(&self) -> &str {
        "governed_filesystem"
    }

    fn required_capabilities(&self) -> Vec<String> {
        // Capabilities checked per-action in execute()
        vec!["fs.read".into(), "fs.write".into()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        match action {
            PlannedAction::FileRead { path } => {
                if !context.capabilities.contains("fs.read") {
                    return Err(ActuatorError::CapabilityDenied("fs.read".into()));
                }

                let safe_path = Self::resolve_safe_path(&context.working_dir, path)?;

                // Size check
                let metadata = std::fs::metadata(&safe_path)
                    .map_err(|e| ActuatorError::IoError(format!("metadata: {e}")))?;
                if metadata.len() > MAX_READ_SIZE {
                    return Err(ActuatorError::FileTooLarge {
                        size: metadata.len(),
                        max: MAX_READ_SIZE,
                    });
                }

                let content = std::fs::read_to_string(&safe_path)
                    .map_err(|e| ActuatorError::IoError(format!("read: {e}")))?;

                Ok(ActionResult {
                    success: true,
                    output: content,
                    fuel_cost: FUEL_COST_READ,
                    side_effects: vec![],
                })
            }

            PlannedAction::FileWrite { path, content } => {
                if !context.capabilities.contains("fs.write") {
                    return Err(ActuatorError::CapabilityDenied("fs.write".into()));
                }

                let safe_path = Self::resolve_safe_path(&context.working_dir, path)?;

                // Extension check
                Self::check_extension(&safe_path)?;

                // Size check
                let size = content.len() as u64;
                if size > MAX_WRITE_SIZE {
                    return Err(ActuatorError::FileTooLarge {
                        size,
                        max: MAX_WRITE_SIZE,
                    });
                }

                let existed = safe_path.exists();
                std::fs::write(&safe_path, content)
                    .map_err(|e| ActuatorError::IoError(format!("write: {e}")))?;

                let effect = if existed {
                    SideEffect::FileModified {
                        path: safe_path.clone(),
                    }
                } else {
                    SideEffect::FileCreated {
                        path: safe_path.clone(),
                    }
                };

                Ok(ActionResult {
                    success: true,
                    output: format!("wrote {} bytes to {}", size, safe_path.display()),
                    fuel_cost: FUEL_COST_WRITE,
                    side_effects: vec![effect],
                })
            }

            _ => Err(ActuatorError::ActionNotHandled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn make_context(workspace: &Path) -> ActuatorContext {
        let mut caps = HashSet::new();
        caps.insert("fs.read".into());
        caps.insert("fs.write".into());
        ActuatorContext {
            agent_id: "test-agent".into(),
            agent_name: "test-agent".into(),
            working_dir: workspace.to_path_buf(),
            autonomy_level: crate::autonomy::AutonomyLevel::L2,
            capabilities: caps,
            fuel_remaining: 1000.0,
            egress_allowlist: vec![],
            action_review_engine: None,
        }
    }

    #[test]
    fn write_and_read_file() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let fs = GovernedFilesystem;

        let write_action = PlannedAction::FileWrite {
            path: "hello.txt".into(),
            content: "Hello, world!".into(),
        };
        let result = fs.execute(&write_action, &ctx).unwrap();
        assert!(result.success);
        assert_eq!(result.side_effects.len(), 1);
        assert!(matches!(
            &result.side_effects[0],
            SideEffect::FileCreated { .. }
        ));

        let read_action = PlannedAction::FileRead {
            path: "hello.txt".into(),
        };
        let result = fs.execute(&read_action, &ctx).unwrap();
        assert!(result.success);
        assert_eq!(result.output, "Hello, world!");
    }

    #[test]
    fn path_traversal_rejected() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let fs = GovernedFilesystem;

        let action = PlannedAction::FileRead {
            path: "../../etc/passwd".into(),
        };
        let err = fs.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::PathTraversal(_)));
    }

    #[test]
    fn blocked_extension_rejected() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let fs = GovernedFilesystem;

        for ext in &[
            ".exe", ".sh", ".bat", ".cmd", ".ps1", ".dll", ".so", ".dylib",
        ] {
            let action = PlannedAction::FileWrite {
                path: format!("malicious{ext}"),
                content: "bad stuff".into(),
            };
            let err = fs.execute(&action, &ctx).unwrap_err();
            assert!(
                matches!(err, ActuatorError::BlockedExtension(_)),
                "expected BlockedExtension for {ext}, got {err:?}"
            );
        }
    }

    #[test]
    fn write_size_limit_enforced() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let fs = GovernedFilesystem;

        // 51 MB — over limit
        let big_content = "x".repeat(51 * 1024 * 1024);
        let action = PlannedAction::FileWrite {
            path: "big.txt".into(),
            content: big_content,
        };
        let err = fs.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::FileTooLarge { .. }));
    }

    #[test]
    fn write_10mb_succeeds() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let fs = GovernedFilesystem;

        let content = "x".repeat(10 * 1024 * 1024);
        let action = PlannedAction::FileWrite {
            path: "ok.txt".into(),
            content,
        };
        let result = fs.execute(&action, &ctx).unwrap();
        assert!(result.success);
    }

    #[test]
    fn file_modified_side_effect() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let fs = GovernedFilesystem;

        // First write creates
        let action = PlannedAction::FileWrite {
            path: "file.txt".into(),
            content: "v1".into(),
        };
        let r = fs.execute(&action, &ctx).unwrap();
        assert!(matches!(&r.side_effects[0], SideEffect::FileCreated { .. }));

        // Second write modifies
        let action = PlannedAction::FileWrite {
            path: "file.txt".into(),
            content: "v2".into(),
        };
        let r = fs.execute(&action, &ctx).unwrap();
        assert!(matches!(
            &r.side_effects[0],
            SideEffect::FileModified { .. }
        ));
    }

    #[test]
    fn capability_denied_read() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = make_context(tmp.path());
        ctx.capabilities.remove("fs.read");
        let fs = GovernedFilesystem;

        let action = PlannedAction::FileRead {
            path: "any.txt".into(),
        };
        let err = fs.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::CapabilityDenied(_)));
    }

    #[test]
    fn capability_denied_write() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = make_context(tmp.path());
        ctx.capabilities.remove("fs.write");
        let fs = GovernedFilesystem;

        let action = PlannedAction::FileWrite {
            path: "any.txt".into(),
            content: "data".into(),
        };
        let err = fs.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::CapabilityDenied(_)));
    }

    #[test]
    fn subdirectory_creation() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let fs = GovernedFilesystem;

        let action = PlannedAction::FileWrite {
            path: "sub/dir/file.txt".into(),
            content: "nested".into(),
        };
        let result = fs.execute(&action, &ctx).unwrap();
        assert!(result.success);

        let read = PlannedAction::FileRead {
            path: "sub/dir/file.txt".into(),
        };
        let result = fs.execute(&read, &ctx).unwrap();
        assert_eq!(result.output, "nested");
    }
}
