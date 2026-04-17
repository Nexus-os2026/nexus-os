//! BUG C deterministic verification — FileWrite actuator must permit writes to
//! absolute paths under /tmp/ after the workspace containment check was removed
//! in commit b8e5fc47.
//!
//! This test exercises the FileWrite execution path of `GovernedFilesystem`
//! directly (not via planner, Tauri, or MCP) and fails loudly if
//! `ActuatorError::PathTraversal` is emitted.

use nexus_kernel::actuators::types::{ActionResult, Actuator};
use nexus_kernel::actuators::{ActuatorContext, ActuatorError, GovernedFilesystem};
use nexus_kernel::autonomy::AutonomyLevel;
use nexus_kernel::cognitive::PlannedAction;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Removes the target file when dropped, even on assertion panic.
struct FileCleanup(PathBuf);

impl Drop for FileCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn make_fs_write_context(workspace: &Path) -> ActuatorContext {
    let mut caps = HashSet::new();
    caps.insert("fs.read".to_string());
    caps.insert("fs.write".to_string());
    ActuatorContext {
        agent_id: "bug-c-verify".into(),
        agent_name: "bug-c-verify".into(),
        working_dir: workspace.to_path_buf(),
        autonomy_level: AutonomyLevel::L2,
        capabilities: caps,
        fuel_remaining: 1000.0,
        egress_allowlist: vec![],
        action_review_engine: None,
        hitl_approved: true,
    }
}

#[test]
fn test_bug_c_tmp_write_succeeds() {
    let workspace = TempDir::new().expect("failed to create workspace tempdir");
    let ctx = make_fs_write_context(workspace.path());
    let fs = GovernedFilesystem;

    let target = format!("/tmp/bug_c_verify_{}.txt", std::process::id());
    let _cleanup = FileCleanup(PathBuf::from(&target));

    let action = PlannedAction::FileWrite {
        path: target.clone(),
        content: "bug_c_verified".into(),
    };

    let result: Result<ActionResult, ActuatorError> = fs.execute(&action, &ctx);

    // Assertion 1: Result is Ok(_). On Err, print the error Debug and fail.
    assert!(
        result.is_ok(),
        "BUG C regression: FileWrite to {target} returned Err: {:?}",
        result.as_ref().err()
    );

    // Assertion 2: Specifically NOT PathTraversal. Even if Ok(_) above passed,
    // guard the exact invariant in a second explicit check for clarity.
    assert!(
        !matches!(&result, Err(ActuatorError::PathTraversal(_))),
        "BUG C regression: resolve_safe_path rejected /tmp/ write with PathTraversal. \
         The workspace containment check is back. Result: {result:?}"
    );

    // Assertion 3: File exists on disk.
    assert!(
        Path::new(&target).exists(),
        "BUG C regression: FileWrite returned Ok but {target} does not exist on disk"
    );

    // Assertion 4: Content matches exactly.
    let on_disk = std::fs::read_to_string(&target)
        .unwrap_or_else(|e| panic!("failed to read back {target}: {e}"));
    assert_eq!(
        on_disk, "bug_c_verified",
        "BUG C regression: content mismatch at {target}"
    );
}
