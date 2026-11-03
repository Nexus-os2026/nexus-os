//! M2d-C (Codex review 2 of PR #78, HIGH 3; the Architect's decision C): a
//! door's commitment read that the broker cannot answer returns a typed
//! error, never a panic. A panic while the commitments registry is held
//! poisons it; `AgentDoor::commitment` then panics on its `expect`, and a
//! Mother Stop that reconciles a File would abort before it ended the
//! mission. `AgentDoor::try_commitment` answers `Unavailable` instead, and
//! answers as `commitment` does otherwise. Pre-registered before it exists.
//! A child of the B2 oracles, for their fixture.

use super::*;
use crate::governed::Lookup;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[test]
fn m2d_c_r2_a_poisoned_registry_is_a_typed_door_error() {
    let f = Fx::new();
    let view = f.propose(NAME, CONTENT).expect("prepared");
    assert!(matches!(
        f.door.try_commitment(view.id),
        Ok(Lookup::Mine(v)) if v.id == view.id
    ));
    let other = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    assert!(matches!(
        other.try_commitment(view.id),
        Ok(Lookup::NotMine { .. })
    ));
    let poisoned = catch_unwind(AssertUnwindSafe(|| {
        f.gov
            .authority()
            .commitments()
            .holding_view(view.id, |_| panic!("poison the registry"))
    }));
    assert!(poisoned.is_err());
    let read = catch_unwind(AssertUnwindSafe(|| f.door.try_commitment(view.id)));
    assert!(
        matches!(read, Ok(Err(AuthorityError::Unavailable(_)))),
        "a typed error, not a panic"
    );
}
