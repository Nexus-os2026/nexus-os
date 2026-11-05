//! F04: the original mission clocks bound a fresh approval continuation.
use super::*;
use crate::lane::{Lane, LaneIdentity, LaneOutput};
use crate::oracles::lite_support::files_lanes;
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::{contract_with, Yes};
use nexus_governed_control::host_session::CommandKind;
use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
const TEST: &str =
    "coordinator::lite::session_budget_fix1::fix1_fresh_approval_keeps_original_mission_clocks";
const CASE: &str = "M2DD_FIX1_MISSION_BUDGET";
thread_local! { static BEFORE: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) }; }
pub(in crate::coordinator) fn before_admit() {
    BEFORE.with(|slot| {
        if let Some(f) = slot.borrow_mut().take() {
            f();
        }
    });
}
struct CountLane {
    inner: Arc<dyn Lane>,
    calls: Arc<AtomicUsize>,
}
impl Lane for CountLane {
    fn identity(&self) -> LaneIdentity {
        self.inner.identity()
    }
    fn invoke(&self, input: &LaneInput) -> LaneOutput {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.invoke(input)
    }
}
#[test]
fn fix1_fresh_approval_keeps_original_mission_clocks() {
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
                    .contains(&format!("FIX1 F04 MOTHER VERIFIED {case}")),
                "child must execute exact registered case"
            );
        }
        return;
    };
    assert!(nexus_governed_control::files::make_non_dumpable());
    let home = crate::adapter::session_support::Fixture::new("fix1-budget");
    let calls = Arc::new(AtomicUsize::new(0));
    let lanes: Vec<Arc<dyn Lane>> = files_lanes()
        .into_iter()
        .map(|inner| {
            Arc::new(CountLane {
                inner,
                calls: calls.clone(),
            }) as Arc<dyn Lane>
        })
        .collect();
    let m = Coordinator::open(
        home.journal(),
        home.port.clone(),
        Lanes::new(lanes).unwrap(),
        home.clock.clone(),
        home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    let mut contract = contract_with("MKFILE:session.txt");
    contract.budget.max_duration_ms = 120_000;
    contract.requested_folder = Some(home.session.workspace().unwrap().name);
    let id = m.draft(contract).unwrap();
    m.submit(id).unwrap();
    answering(&Yes, || m.request_consent(id)).unwrap();
    for _ in 0..8 {
        if m.state(id).unwrap() == Some(MissionState::AwaitingApproval) {
            break;
        }
        m.advance(id).unwrap();
    }
    assert_eq!(m.state(id).unwrap(), Some(MissionState::AwaitingApproval));
    home.clock.advance(Duration::from_millis(119_999));
    let request = home
        .session
        .ingress()
        .accept(CommandKind::Approve, 100)
        .unwrap();
    home.session.approve_file(&request).unwrap();
    let continuation = home.session.approval_continuation(&request).unwrap();
    let m = m.continue_approval(continuation).unwrap();
    calls.store(0, Ordering::SeqCst);
    let clock = home.clock.clone();
    let expire = case.clone();
    BEFORE.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || match expire.as_str() {
            "wall" => clock.suspend(Duration::from_millis(1)),
            "mono" => clock.advance(Duration::from_millis(1)),
            "control" => (),
            _ => panic!("unknown case"),
        }))
    });
    for _ in 0..8 {
        match m.advance(id) {
            Ok(_) if calls.load(Ordering::SeqCst) == 0 => (),
            _ => break,
        }
    }
    let count = calls.load(Ordering::SeqCst);
    assert_eq!(
        count > 0,
        case == "control",
        "original clock expired after budget check but before final admission"
    );
    assert!(
        BEFORE.with(|slot| slot.borrow().is_none()),
        "forced final-admission boundary was reached"
    );
    println!("FIX1 F04 MOTHER VERIFIED {case} lane_calls={count}");
}

fn child(case: &str) -> std::process::Output {
    child_case(TEST, CASE, case)
}

pub(super) fn child_case(test: &str, key: &str, case: &str) -> std::process::Output {
    let mut process = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env(key, case)
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

#[test]
fn fix2_mother_consent_budget_precedes_native_approval() {
    const TEST2: &str = "coordinator::lite::session_budget_fix1::fix2_mother_consent_budget_precedes_native_approval";
    const CASE2: &str = "M2DD_FIX2_MOTHER_BUDGET";
    let Ok(case) = std::env::var(CASE2) else {
        let mut failures = Vec::new();
        for case in ["wall", "mono", "control"] {
            let out = child_case(TEST2, CASE2, case);
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            println!("{text}");
            if !out.status.success()
                || !text.contains(&format!("FIX2 MOTHER BUDGET VERIFIED {case}"))
            {
                failures.push(case);
            }
        }
        assert!(failures.is_empty(), "failed schedules: {failures:?}");
        return;
    };
    assert!(nexus_governed_control::files::make_non_dumpable());
    let home = crate::adapter::session_support::Fixture::new("fix2-mother-budget");
    let m = Coordinator::open(
        home.journal(),
        home.port.clone(),
        Lanes::new(files_lanes()).unwrap(),
        home.clock.clone(),
        home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    let mut contract = contract_with("MKFILE:session.txt");
    contract.budget.max_duration_ms = 120_000;
    contract.requested_folder = Some(home.session.workspace().unwrap().name);
    let id = m.draft(contract).unwrap();
    m.submit(id).unwrap();
    answering(&Yes, || m.request_consent(id)).unwrap();
    for _ in 0..8 {
        if m.state(id).unwrap() == Some(MissionState::AwaitingApproval) {
            break;
        }
        m.advance(id).unwrap();
    }
    assert_eq!(m.state(id).unwrap(), Some(MissionState::AwaitingApproval));
    home.clock.advance(Duration::from_millis(119_999));
    let request = home
        .session
        .ingress()
        .accept(CommandKind::Approve, 2)
        .unwrap();
    match case.as_str() {
        "wall" => home.clock.suspend(Duration::from_millis(1)),
        "mono" => home.clock.advance(Duration::from_millis(1)),
        "control" => (),
        _ => panic!("unknown clock"),
    }
    // Deliberately no Mother touch or continuation before the native call.
    let result = home.session.approve_file(&request);
    assert_eq!(
        home.workspace.join("session.txt").exists(),
        case == "control",
        "consent's original clocks must cross the pending target: {result:?}"
    );
    assert_eq!(
        home.session
            .operation_observation(&request)
            .unwrap()
            .apply_handed_off,
        case == "control"
    );
    if case == "control" {
        assert!(result.is_ok());
    }
    println!("FIX2 MOTHER BUDGET VERIFIED {case}");
}
