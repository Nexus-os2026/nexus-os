//! M0-FIX2 F10 oracle (production construction): the governed-control
//! front door, built as production builds it, is the one holder of the
//! process's emergency-stop controller. Nothing else in the process can
//! claim it afterwards, a second front door is refused (fail closed: it
//! could neither lift nor be the owner's path), and the owner's Stop and
//! confirmed resume engage and lift the process's own authority.
//!
//! This binary drives the process's own stop authority, so it holds
//! exactly one test.

use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ControlConfirmer, GrantConfirmation,
    ResumeConfirmation,
};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::AuthorityError;
use nexus_governed_control::broker::Vault;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::governed::GovernedControl;
use nexus_kernel::emergency_stop::StopAuthority;
use std::path::PathBuf;
use std::sync::Arc;

struct Owner;
impl ControlConfirmer for Owner {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        true
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
}

fn root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("nexus-m0-fix2-f10-{name}-{}", std::process::id()))
}

fn front_door(root: &PathBuf) -> Result<GovernedControl, AuthorityError> {
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
fn m0_fix2_f10_the_owner_path_alone_holds_the_process_stop_controller() {
    let first = root("first");
    let second = root("second");
    let control = front_door(&first).expect("the front door is built");

    // The owner's path holds the controller: nobody else can claim it.
    assert!(
        StopAuthority::claim_global_controller().is_none(),
        "the process's stop controller was handed out twice"
    );
    assert!(
        StopAuthority::global().claim_controller().is_none(),
        "the process's authority handed its controller to a generic claim"
    );
    // A second front door would hold no controller: refused.
    match front_door(&second) {
        Err(AuthorityError::Unavailable(_)) => {}
        Err(other) => panic!("refused for the wrong reason: {other}"),
        Ok(_) => panic!("a second front door was built without the stop controller"),
    }

    // The owner's Stop engages the process's authority; the owner's
    // confirmed resume lifts it; work the stop ended stays ended.
    let stop = StopAuthority::global();
    let admitted = stop.admit().expect("not stopped yet");
    control.emergency_stop();
    assert!(stop.is_stopped(), "the owner's Stop missed the kernel");
    assert!(admitted.halted());
    control.resume(&Owner).expect("the owner's resume");
    assert!(!stop.is_stopped(), "the owner's resume did not lift");
    assert!(admitted.halted(), "halted work came back by itself");

    drop(control);
    let _ = std::fs::remove_dir_all(&first);
    let _ = std::fs::remove_dir_all(&second);
}
