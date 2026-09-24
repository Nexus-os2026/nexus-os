//! P0-002 supersedes the old Option B outside-workspace write expectation.

use nexus_kernel::actuators::types::Actuator;
use nexus_kernel::actuators::{ActuatorContext, ActuatorError, GovernedFilesystem};
use nexus_kernel::autonomy::AutonomyLevel;
use nexus_kernel::cognitive::PlannedAction;
use std::collections::HashSet;
use std::path::Path;
use tempfile::TempDir;

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
fn test_bug_c_outside_write_is_denied() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let target = temp.path().join("outside.txt");
    std::fs::write(&target, "outside evidence").unwrap();
    let result = GovernedFilesystem.execute(
        &PlannedAction::FileWrite {
            path: target.to_str().unwrap().into(),
            content: "must not be written".into(),
        },
        &make_fs_write_context(&workspace),
    );
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "outside evidence"
    );
    assert!(matches!(result, Err(ActuatorError::PathTraversal(_))));
}
