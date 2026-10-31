//! M0-FIX2 F10 review (MINOR-2): the host contract when the front door is
//! dropped. The governed-control front door holds the process's only stop
//! controller, which is minted once and never handed back. Dropping the
//! front door therefore fails closed: a stop in force stays in force for
//! the rest of the process, new kernel work stays refused, and no later
//! front door can be built (it would hold no controller). A host builds
//! its front door once and keeps it for the life of the process.
//!
//! This binary drives the process's own stop authority, so it holds
//! exactly one test.

use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::AuthorityError;
use nexus_governed_control::broker::Vault;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::governed::GovernedControl;
use nexus_kernel::emergency_stop::StopAuthority;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "nexus-m0-fix2-f10-drop-{name}-{}",
        std::process::id()
    ))
}

fn front_door(root: &Path) -> Result<GovernedControl, AuthorityError> {
    let evidence = PrimaryEvidence::open(
        &root.join("p3-evidence").join("evidence.sqlite3"),
        Vec::new(),
    )
    .unwrap();
    GovernedControl::new(
        root,
        Vault::Disabled,
        evidence,
        Arc::new(SystemClock::default()),
    )
}

#[test]
fn m0_fix2_f10_a_dropped_front_door_leaves_the_kernel_stopped() {
    let first = root("first");
    let second = root("second");
    let control = front_door(&first).expect("the front door is built");
    control.emergency_stop();
    drop(control);

    let stop = StopAuthority::global();
    assert!(stop.is_stopped(), "the stop ended with its front door");
    assert!(stop.admit().is_err(), "kernel work was admitted");
    assert!(
        StopAuthority::claim_global_controller().is_none(),
        "the controller came back when its front door was dropped"
    );
    match front_door(&second) {
        Err(AuthorityError::Unavailable(_)) => {}
        Err(other) => panic!("refused for the wrong reason: {other}"),
        Ok(_) => panic!("a later front door was built without the stop controller"),
    }
    assert!(stop.is_stopped());

    let _ = std::fs::remove_dir_all(&first);
    let _ = std::fs::remove_dir_all(&second);
}
