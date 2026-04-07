//! State machine integration test. See v1.1 §2.

use nexus_ui_repair::driver::state::DriverState;

#[test]
fn enumerate_walks_through_to_terminal() {
    let mut s = Some(DriverState::Enumerate);
    let mut visited = Vec::new();
    while let Some(current) = s {
        visited.push(current);
        s = current.next();
    }
    assert_eq!(
        visited,
        vec![
            DriverState::Enumerate,
            DriverState::Plan,
            DriverState::Act,
            DriverState::Observe,
            DriverState::Classify,
            DriverState::Report,
        ]
    );
    // Sanity: a 7th .next() call from the terminal state stays None.
    assert!(DriverState::Report.next().is_none());
}
