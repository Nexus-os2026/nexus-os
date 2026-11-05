//! F04 final owning-hold oracle, paired with the real Mother continuation.
use super::*;
use crate::authority::clock::Clock;
use crate::host_session::{MotherStep, SessionContext, SessionError};
use std::sync::mpsc::sync_channel;
const CASE: &str = "M2DD_FIX1_FINAL_BUDGET";
const TEST: &str =
    "governed::session_routes::budget_fix1::fix1_original_budget_is_rechecked_under_final_run_hold";

// Before the production API exists this test reports unavailable narrowing,
// separately from the real Mother's behavioral red receipt. An inherent
// production method supersedes this fail-closed oracle fallback automatically.
#[allow(dead_code)]
trait MissingDeadlineNarrowing {
    fn narrow_mission_deadlines(
        &self,
        _: Arc<dyn Clock>,
        _: u64,
        _: u64,
    ) -> Result<(), SessionError> {
        Err(SessionError::Restricted)
    }
}
impl MissingDeadlineNarrowing for SessionContext {}

#[test]
fn fix1_original_budget_is_rechecked_under_final_run_hold() {
    let Ok(case) = std::env::var(CASE) else {
        for case in ["wall", "mono", "control"] {
            let output = child(case);
            assert!(
                output.status.success(),
                "{case}: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                String::from_utf8_lossy(&output.stdout)
                    .contains(&format!("FIX1 F04 FINAL HOLD VERIFIED {case}")),
                "child must execute exact registered case"
            );
        }
        return;
    };
    let f = Fixture::new();
    let context = f.door.context();
    context
        .narrow_mission_deadlines(
            f.clock.clone(),
            f.clock.monotonic_ms() + 100,
            f.clock.wall_ms() + 100,
        )
        .expect("original mission deadline narrowing API must be available");
    f.clock.advance(Duration::from_millis(99));
    let (held_tx, held_rx) = sync_channel(1);
    let (release_tx, release_rx) = sync_channel(1);
    f.gov
        .authority()
        .runs()
        .review2_hooks
        .arm("request.final_hold", move || {
            held_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        });
    let worker = std::thread::spawn(move || context.admit(MotherStep::LaneInvocation));
    held_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    match case.as_str() {
        "wall" => f.clock.suspend(Duration::from_millis(1)),
        "mono" => f.clock.advance(Duration::from_millis(1)),
        "control" => (),
        _ => panic!("unknown case"),
    }
    release_tx.send(()).unwrap();
    assert_eq!(
        worker.join().unwrap().is_ok(),
        case == "control",
        "clock prerequisites are validated after final owning hold acquisition"
    );
    println!("FIX1 F04 FINAL HOLD VERIFIED {case}");
}

fn child(case: &str) -> std::process::Output {
    let mut process = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST, "--nocapture"])
        .env(CASE, case)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while process.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            process.kill().unwrap();
            process.wait().unwrap();
            panic!("bounded fix1 oracle child timed out: {case}");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    process.wait_with_output().unwrap()
}
