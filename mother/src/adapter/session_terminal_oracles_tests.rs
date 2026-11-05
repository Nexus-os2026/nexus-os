//! Independent literal-state comparison, using real coordinator/broker calls.
//! The baseline fixture is legacy File mode; migrate the fixture to the
//! mandatory session facade, preserving these primitive inputs and answers.
use super::*;
use crate::adapter::m2c_support::{port_for, Home};
use crate::adapter::{create_private, mother_agent};
use crate::journal::MemoryJournal;
use crate::lane::{Lane, LaneIdentity, LaneOutput};
use crate::oracles::lite_support::{files_lanes, FilesOwner};
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::{contract_with, No, Yes};
use nexus_governed_control::authority::clock::ManualClock;
use nexus_governed_control::governed::GrantRequest;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;

#[path = "../../host/tests/session/model.rs"]
#[allow(dead_code)]
mod model;

const TEST: &str = "coordinator::lite::session_terminal_oracles::session_literal_thirteen_states_against_real_terminal_paths";
const CASE: &str = "M2DD_SESSION_LITERAL_STATE";
const STATES: [&str; 13] = [
    "Draft",
    "AwaitingConsent",
    "Approved",
    "Investigating",
    "Building",
    "AwaitingApproval",
    "Verifying",
    "Explaining",
    "Done",
    "Stopped",
    "Denied",
    "Failed",
    "Blocked",
];
const CAUSES: [&str; 4] = ["owner", "emergency", "budget", "fence"];

struct FailVerifier;
impl Lane for FailVerifier {
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: LaneRole::Verifier,
            name: "session-literal-verifier".into(),
            version: "1".into(),
        }
    }
    fn invoke(&self, _: &LaneInput) -> LaneOutput {
        LaneOutput {
            text: "fixture failure".into(),
            effects: vec![],
            verdict: Verdict::Fail {
                reason: "fixture".into(),
            },
        }
    }
}

fn replay_all_prefixes(lines: &[String]) {
    let entries = verify(lines).unwrap();
    for count in 0..=entries.len() {
        let (mut missions, mut next, mut session) = (BTreeMap::new(), 1, Session::Never);
        for entry in &entries[..count] {
            replay(&mut missions, &mut next, &mut session, entry).unwrap();
        }
    }
}

fn child(case: &str) {
    let parts: Vec<_> = case.split('/').collect();
    let (row, column) = (
        parts[0].parse::<usize>().unwrap(),
        parts[1].parse::<usize>().unwrap(),
    );
    let initial = STATES[row];
    assert!(row < 13 && column < 4);
    assert!(nexus_governed_control::files::make_non_dumpable());
    let home = Home::fresh_temp("session-literal-state").unwrap();
    let workspace = home.dir.join("workspace");
    create_private(&workspace).unwrap();
    let info = home.control.register_workspace(&workspace, &[]).unwrap();
    let owner = FilesOwner::default();
    home.control
        .request_grant(
            &GrantRequest::Files { workspace: info.id },
            Duration::from_secs(86400),
            &owner,
        )
        .unwrap();
    let port = Arc::new(port_for(&home.control, true).with_workspace(info.name.clone()));
    let mut lanes = files_lanes();
    if initial == "Failed" {
        lanes.retain(|lane| lane.identity().role != LaneRole::Verifier);
        lanes.push(Arc::new(FailVerifier));
    }
    let clock = Arc::new(ManualClock::default());
    let journal = MemoryJournal::default();
    let m = Coordinator::open(
        journal.clone(),
        port.clone(),
        Lanes::new(lanes).unwrap(),
        clock.clone(),
        home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    // The budget is genuinely armed before Draft/AwaitingConsent are reached
    // again by revision. No test overwrites a production mission's state.
    let wants_file = matches!(initial, "AwaitingApproval" | "Done" | "Denied");
    let mut contract = contract_with(if wants_file {
        "MKFILE:session.txt"
    } else {
        "literal terminal fixture"
    });
    contract.requested_folder = Some(info.name);
    let id = m.draft(contract.clone()).unwrap();
    m.submit(id).unwrap();
    answering(&Yes, || m.request_consent(id)).unwrap();
    match initial {
        "Draft" | "AwaitingConsent" => {
            m.revise(id, contract).unwrap();
            if initial == "AwaitingConsent" {
                m.submit(id).unwrap();
            }
        }
        "Stopped" => {
            m.stop(id).unwrap();
        }
        "Blocked" => {
            m.end_for_test(id, BlockReason::TooManyEffects).unwrap();
        }
        _ => {
            for _ in 0..10 {
                let actual = format!("{:?}", m.state(id).unwrap().unwrap());
                if actual == initial {
                    break;
                }
                if actual == "AwaitingApproval" {
                    let pending = port.pending(id).unwrap();
                    let run = RunId::parse(&pending.run).unwrap();
                    let action = CommitmentId::parse(&pending.commitment).unwrap();
                    if initial == "Denied" {
                        assert!(home
                            .control
                            .authorize(action, &mother_agent(), run, &No)
                            .is_err());
                    } else {
                        home.control
                            .authorize(action, &mother_agent(), run, &owner)
                            .unwrap();
                        home.control.execute(action, &mother_agent(), run).unwrap();
                    }
                }
                m.advance(id).unwrap();
            }
        }
    }
    assert_eq!(
        format!("{:?}", m.state(id).unwrap().unwrap()),
        initial,
        "fixture must reach its literal input state"
    );
    let before = journal.lines();
    replay_all_prefixes(&before);
    let file = workspace.join("session.txt");
    let physical = std::fs::read(&file).ok();
    assert_eq!(
        physical.is_some(),
        initial == "Done",
        "fixture controls publication independently"
    );
    match CAUSES[column] {
        "owner" => {
            m.stop(id).unwrap();
        }
        "emergency" => {
            home.control.file_session_halt().engage_emergency();
            m.state(id).unwrap();
        }
        "budget" => {
            clock.advance(Duration::from_secs(86400));
            m.state(id).unwrap();
        }
        "fence" => {
            port.fence().write_checked().unwrap();
            m.state(id).unwrap();
        }
        _ => unreachable!(),
    }
    let expected =
        model::TERMINALS[row][column].expect("registered applicable literal cause/state row");
    assert_eq!(
        format!("{:?}", m.state(id).unwrap().unwrap()),
        format!("{expected:?}"),
        "{case}: independent table, not a production transition or SettleCause::every"
    );
    assert_eq!(
        std::fs::read(&file).ok(),
        physical,
        "terminal touch never changes the published Owner file"
    );
    let after = journal.lines();
    replay_all_prefixes(&after);
    if row >= 8 {
        assert_eq!(
            after, before,
            "terminal history cannot be rewritten by a later cause"
        );
    }
    home.control.file_session_halt().engage_emergency();
    m.stop(id).unwrap();
    assert_eq!(
        format!("{:?}", m.state(id).unwrap().unwrap()),
        format!("{expected:?}")
    );
    assert_eq!(
        journal.lines(),
        after,
        "later cause creates no duplicate terminal transition"
    );
    assert!(
        !home.control.stop_snapshot().kernel_stopped,
        "native worker remains unscheduled"
    );
    println!(
        "SESSION LITERAL VERIFIED {case} state={initial} cause={} prefixes={} published={}",
        CAUSES[column],
        after.len() + 1,
        physical.is_some()
    );
}

#[test]
fn session_literal_thirteen_states_against_real_terminal_paths() {
    if let Ok(case) = std::env::var(CASE) {
        child(&case);
        return;
    }
    let mut failures = Vec::new();
    let mut count = 0;
    for row in 0..13 {
        for column in 0..4 {
            let case = format!("{row}/{column}");
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CASE, &case)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let mut out = child.stdout.take().unwrap();
            let mut err = child.stderr.take().unwrap();
            let output = std::thread::spawn(move || {
                let mut text = String::new();
                out.read_to_string(&mut text).unwrap();
                text
            });
            let errors = std::thread::spawn(move || {
                let mut text = String::new();
                err.read_to_string(&mut text).unwrap();
                text
            });
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if std::time::Instant::now() >= deadline {
                    child.kill().unwrap();
                    break child.wait().unwrap();
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            let out = output.join().unwrap();
            let err = errors.join().unwrap();
            let correct =
                status.success() && out.contains(&format!("SESSION LITERAL VERIFIED {case}"));
            println!("LITERAL/{case}: conforms={correct}");
            if !correct {
                failures.push(format!("{case}: {out}{err}"));
            }
            count += 1;
        }
    }
    assert_eq!(count, 52);
    assert!(
        failures.is_empty(),
        "52 executed literal comparisons: {}",
        failures.join("\n")
    );
}
