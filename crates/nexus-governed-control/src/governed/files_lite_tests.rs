//! Lite R: base-compatible, behavioral red before retention implementation.
//! Uses the real capacity, with no capacity relaxation or production seam.
use super::*;
use crate::authority::run::{RunClass, RunOrigin};

#[test]
fn lite_r_unacknowledged_file_retains_its_run_at_capacity() {
    let f = Fx::new();
    let file = f.ready(NAME, CONTENT);
    f.execute(&file).expect("real publication control");
    assert_eq!(std::fs::read(f.ws.join(NAME)).unwrap(), CONTENT.as_bytes());
    let _ = f.door.cancel_run(f.run).unwrap().write();
    let runs = f.gov.authority().runs();
    for _ in 1..4096 {
        runs.open(mother(), RunOrigin::AgentGoal, RunClass::Agent, 0)
            .expect("room before capacity");
    }
    let extra = runs.open(mother(), RunOrigin::AgentGoal, RunClass::Agent, 0);
    assert!(
        matches!(extra, Err(AuthorityError::Capacity)),
        "unacknowledged File run must be retained: capacity must refuse, got {extra:?}"
    );
    assert!(runs.view(f.run).is_some(), "publication's owner retained");
    assert!(matches!(
        f.door.commitment(file.id),
        super::super::Lookup::Mine(_)
    ));
}
