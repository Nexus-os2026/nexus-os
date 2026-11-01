//! M2a-S oracle added after mutation testing (it does not replace or edit
//! the earlier ones): the re-check after the approval query, on its own.
//! On the real broker the fence already turns the broker's answer into a
//! lapse; here the port ignores the fence and still says waiting, so only
//! the coordinator's own re-check can refuse the answer.

use super::*;
use MissionState::*;

#[test]
fn m2as_postcall_a_waiting_answer_after_the_break_is_an_error_without_the_fence() {
    let w = world();
    let victim = w.approved(contract());
    w.mother.advance(victim).unwrap();
    let c = w.mission_in(AwaitingApproval);
    w.port.hold_approval();
    let mother = w.mother.clone();
    let asking = std::thread::spawn(move || mother.advance(c));
    assert!(w.port.wait_approval_entered());
    // The other thread: a Stop the journal cannot record breaks Mother.
    w.journal.fail_after(0);
    assert!(w.mother.stop(victim).is_err());
    let answer = asking.join().unwrap();
    assert!(
        answer.is_err(),
        "a broken coordinator answered {:?}",
        answer.map(|a| a.to)
    );
    assert_eq!(
        w.port.released_by_event(),
        vec![true],
        "released by the fence"
    );
}
