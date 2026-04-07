//! Replay harness test stub. Real replay test lands in Phase 1.4.

use nexus_ui_repair::replay::harness::ReplayHarness;
use std::path::Path;

#[test]
fn phase1_1_replay_stub() {
    let harness = ReplayHarness;
    harness
        .replay(Path::new("/dev/null"))
        .expect("Phase 1.1 stub always succeeds");
}
