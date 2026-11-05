//! R2-F02: a refused Done admission cannot mask a later durable terminal.
use super::*;
use crate::oracles::{contract_with, lite_support::files_lanes, m2c_confirmers::answering, Yes};
use nexus_governed_control::host_session::CommandKind;
use std::cell::RefCell;
use std::time::Duration;
const TEST: &str =
    "coordinator::lite::session_terminal_fix2::fix2_refused_done_preserves_actual_terminal";
const CASE: &str = "M2DD_FIX2_TERMINAL";
thread_local! { static BEFORE: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) }; }
pub(in crate::coordinator) fn before(entry: &Entry) {
    if matches!(
        entry.event,
        Event::Transition {
            trigger: Trigger::PhaseDone
        }
    ) {
        BEFORE.with(|slot| {
            if let Some(f) = slot.borrow_mut().take() {
                f();
            }
        });
    }
}
#[test]
fn fix2_refused_done_preserves_actual_terminal() {
    let Ok(case) = std::env::var(CASE) else {
        let mut failures = Vec::new();
        for case in ["stop", "quit", "mono", "wall", "control"] {
            let output = super::session_budget_fix1::child_case(TEST, CASE, case);
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            println!("{text}");
            if !output.status.success() || !text.contains(&format!("FIX2 TERMINAL VERIFIED {case}"))
            {
                failures.push(case);
            }
        }
        assert!(failures.is_empty(), "failed schedules: {failures:?}");
        return;
    };
    assert!(nexus_governed_control::files::make_non_dumpable());
    let home = crate::adapter::session_support::Fixture::new("fix2-terminal");
    let mut m = Coordinator::open(
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
    let request = home
        .session
        .ingress()
        .accept(CommandKind::Approve, 2)
        .unwrap();
    home.session.approve_file(&request).unwrap();
    m = m
        .continue_approval(home.session.approval_continuation(&request).unwrap())
        .unwrap();
    for _ in 0..8 {
        if m.state(id).unwrap() == Some(MissionState::Explaining) {
            break;
        }
        m.advance(id).unwrap();
    }
    assert_eq!(m.state(id).unwrap(), Some(MissionState::Explaining));
    assert_eq!(
        std::fs::read(home.workspace.join("session.txt")).unwrap(),
        b"session.txt: made by Mother\n"
    );
    assert!(m.coordinator.inner.lock().unwrap().missions[&id]
        .acks
        .is_empty());
    home.clock.advance(Duration::from_millis(119_999));
    let ingress = home.session.ingress();
    let clock = home.clock.clone();
    let race = case.clone();
    BEFORE.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || match race.as_str() {
            "stop" => {
                ingress.emergency().unwrap();
            }
            "quit" => {
                ingress.accept(CommandKind::Quit, 3).unwrap();
            }
            "mono" => clock.advance(Duration::from_millis(1)),
            "wall" => clock.suspend(Duration::from_millis(1)),
            "control" => (),
            _ => panic!("unknown case"),
        }))
    });
    let done = m.advance(id);
    assert!(
        BEFORE.with(|slot| slot.borrow().is_none()),
        "Done final admission reached"
    );
    assert_eq!(done.is_ok(), case == "control");
    // Hands observes after an advance refusal; normal close selects Owner Stop.
    if case == "quit" {
        m.coordinator
            .file_stop(id, SettleCause::Stop(StopReason::Owner))
            .unwrap();
    }
    let state = m.state(id).unwrap();
    let expected = match case.as_str() {
        "control" => (MissionState::Done, Trigger::PhaseDone),
        "stop" => (
            MissionState::Stopped,
            Trigger::Stop {
                reason: StopReason::EmergencyStop,
            },
        ),
        "quit" => (
            MissionState::Stopped,
            Trigger::Stop {
                reason: StopReason::Owner,
            },
        ),
        _ => (
            MissionState::Blocked,
            Trigger::Block {
                reason: BlockReason::BudgetTime,
            },
        ),
    };
    assert_eq!(state, Some(expected.0));
    let entries = verify(&home.lines()).unwrap();
    let terminal = entries
        .iter()
        .rev()
        .find(|e| matches!(&e.event, Event::Transition { trigger } if *trigger == expected.1))
        .expect("actual durable expected terminal");
    let facts = m.terminal_facts(id).unwrap();
    assert_eq!(
        facts.first_code(),
        expected.1.name(),
        "a refused PhaseDone is not terminal intent"
    );
    assert_eq!(facts.recorded_sequence, Some(terminal.seq));
    assert!(
        !facts.recording_unknown,
        "successful actual terminal append is known durable"
    );
    assert_eq!(facts.state, state);
    println!(
        "FIX2 TERMINAL VERIFIED {case} first={} seq={} state={state:?}",
        facts.first_code(),
        terminal.seq
    );
}
