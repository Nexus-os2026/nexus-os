//! M3-E M-3b oracles (rulings M3E-R1, R8, R12; register
//! docs/security/m3-e-oracles.md): the page teaching display is selected
//! only from journaled facts, through a view that changes nothing, and the
//! native `approval_expired` path (O34 carry) runs no Verifier.
use crate::contract::{MissionContract, Note};
use crate::coordinator::page_display::{page_view, phase_line, PagePhase, ViewError};
use crate::effect::PendingAction;
use crate::evidence::{LapseReason, SettlementEnd, SettlementOutcome, StopReason};
use crate::journal::{Entry, Event, MissionId, FORMAT};
use crate::lane::InvocationId;
use crate::oracles::contract_with;
use crate::page_inputs::{page_criteria, PageFacts};
use crate::private::{ContentId, ContentKind};
use crate::state::Trigger;
use crate::teaching::{
    t10, t12_failed, t12_passed, t12_unchecked, t13, OPEN_LINES, T11_CHECKED, T15_UNDO_REFUSED,
    T5_DRAFTING, T7_BEFORE_FILE, T9_DECLINED, T9_EXPIRED, T9_STOPPED, T9_UNCERTAIN,
};

const ID: u64 = 7;
const FOLDER: &str = "rosas-bakery";
const FACTS_FILE: &str = "facts.txt";

fn page_contract(folder: &str) -> MissionContract {
    let mut c = contract_with("");
    c.requested_folder = Some(folder.into());
    c.deliverable = Some("index.html".into());
    c.notes = vec![Note {
        label: "notes".into(),
        text: "Fresh bread every morning.".into(),
    }];
    c.page_facts = Some(PageFacts {
        name: "Rosa's Bakery".into(),
        hours: vec![
            "Tuesday to Sunday, 7am to 3pm".into(),
            "Closed on Mondays".into(),
        ],
        phone: "555-0142".into(),
    });
    c.pass_criteria = page_criteria();
    c
}
fn entry(event: Event) -> Entry {
    Entry {
        format: FORMAT,
        seq: 0,
        prev: String::new(),
        mission: MissionId(ID),
        event,
        hash: String::new(),
    }
}
fn transition(trigger: Trigger) -> Entry {
    entry(Event::Transition { trigger })
}
fn pending() -> PendingAction {
    PendingAction {
        run: "run".into(),
        commitment: "commitment".into(),
        deadline_mono_ms: 1,
    }
}
/// Consented, and the File asked for at the broker.
fn asked(c: &MissionContract) -> Vec<Entry> {
    vec![
        transition(Trigger::Consent {
            hash: c.hash(),
            at_wall_ms: 1,
        }),
        entry(Event::ApprovalPending {
            effect: 1,
            pending: pending(),
        }),
    ]
}
fn receipt(sha256: &str, bytes: u64, overtaken: bool) -> Entry {
    entry(Event::PublicationReceipt {
        effect: 1,
        action: pending(),
        sha256: sha256.into(),
        bytes,
        policy_hash: None,
        overtaken,
    })
}
fn checks(codes: [&str; 6]) -> Entry {
    let checks = ["v0", "v1", "v2", "v3", "v4", "v5"]
        .iter()
        .zip(codes)
        .map(|(id, code)| (id.to_string(), code.to_string()))
        .collect();
    entry(Event::VerificationRecorded {
        invocation: InvocationId {
            mission: MissionId(ID),
            seq: 9,
        },
        effect: 1,
        report_digest: String::new(),
        checks,
    })
}
fn lines(entries: &[Entry], c: &MissionContract) -> Result<Vec<String>, ViewError> {
    page_view(entries, MissionId(ID)).lines(c, FACTS_FILE)
}
const RECEIPT_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CANDIDATE_SHA: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const PASS: [&str; 6] = ["pass"; 6];

#[test]
fn m3e_m3b_t10_requires_a_journaled_broker_receipt() {
    let c = page_contract(FOLDER);
    // Asked and pending: no T10, no T9.
    assert_eq!(lines(&asked(&c), &c), Ok(vec![]));
    // The broker's receipt alone gives T10's bytes and digest; Mother's own
    // candidate digest never does.
    let mut journal = asked(&c);
    journal.push(entry(Event::DeliverableRetained {
        invocation: InvocationId {
            mission: MissionId(ID),
            seq: 3,
        },
        effect: 1,
        content: ContentId::of(ContentKind::Deliverable, b"candidate"),
        sha256: CANDIDATE_SHA.into(),
        bytes: 999,
    }));
    journal.push(receipt(RECEIPT_SHA, 1697, false));
    let got = lines(&journal, &c).unwrap();
    assert_eq!(got.first(), Some(&t10(FOLDER, 1697, RECEIPT_SHA)));
    assert!(got
        .iter()
        .all(|l| !l.contains(CANDIDATE_SHA) && !l.contains("999 bytes")));
    // The folder is the consented contract's; another contract is refused.
    let mut other = c.clone();
    other.requested_folder = Some("elsewhere".into());
    assert_eq!(lines(&journal, &other), Err(ViewError::NotConsented));
    // Overtaken: written, never T10 or success.
    let mut overtaken = asked(&c);
    overtaken.push(receipt(RECEIPT_SHA, 1697, true));
    assert_eq!(lines(&overtaken, &c), Ok(vec![t12_unchecked()]));
    // Not a page mission: nothing page-specific is said.
    let plain = contract_with("");
    let mut journal = vec![transition(Trigger::Consent {
        hash: plain.hash(),
        at_wall_ms: 1,
    })];
    journal.push(receipt(RECEIPT_SHA, 10, false));
    assert_eq!(lines(&journal, &plain), Ok(vec![]));
}

#[test]
fn m3e_m3b_teaching_follows_recorded_checks() {
    let c = page_contract(FOLDER);
    let written = || {
        let mut j = asked(&c);
        j.push(receipt(RECEIPT_SHA, 1697, false));
        j
    };
    let t10 = t10(FOLDER, 1697, RECEIPT_SHA);
    // A recorded all-pass: T10, T11, T12 passed (its OPEN lines), T13.
    let mut j = written();
    j.push(checks(PASS));
    j.push(transition(Trigger::VerifyPass));
    let all = vec![
        t10.clone(),
        T11_CHECKED.into(),
        t12_passed(),
        t13(FACTS_FILE),
    ];
    assert_eq!(lines(&j, &c), Ok(all));
    // A recorded failure: its bare code, no T11, no T13.
    let mut j = written();
    j.push(checks([
        "pass",
        "pass",
        "pass",
        "fail:name_not_heading",
        "fail:name_not_visible",
        "pass",
    ]));
    assert_eq!(
        lines(&j, &c),
        Ok(vec![t10.clone(), t12_failed("name_not_heading")])
    );
    // A missing check is not a pass.
    let mut j = written();
    let mut partial = checks(PASS);
    if let Event::VerificationRecorded { checks, .. } = &mut partial.event {
        checks.pop();
    }
    j.push(partial);
    assert_eq!(
        lines(&j, &c),
        Ok(vec![t10.clone(), t12_failed("not_recorded")])
    );
    // A scripted verdict is no recorded check: written, no claim.
    let mut j = written();
    j.push(transition(Trigger::VerifyPass));
    assert_eq!(lines(&j, &c), Ok(vec![t10, t12_unchecked()]));
    // Every T12 form keeps every OPEN line.
    for line in [t12_passed(), t12_unchecked(), t12_failed("x")] {
        assert!(
            OPEN_LINES.iter().all(|open| line.contains(open)),
            "OPEN kept"
        );
    }
    // Phase lines are the frozen ones.
    assert_eq!(phase_line(PagePhase::Drafting), T5_DRAFTING);
    assert_eq!(phase_line(PagePhase::BeforeFileDialog), T7_BEFORE_FILE);
    assert_eq!(
        phase_line(PagePhase::UndoRefusedAfterEdit),
        T15_UNDO_REFUSED
    );
}

#[test]
fn m3e_m3b_t9_follows_the_recorded_end() {
    let c = page_contract(FOLDER);
    let consent = transition(Trigger::Consent {
        hash: c.hash(),
        at_wall_ms: 1,
    });
    let deny_consent = vec![
        transition(Trigger::Submit { hash: c.hash() }),
        transition(Trigger::Deny { hash: c.hash() }),
    ];
    // A declined consent never consented: no page lines can be selected.
    assert_eq!(lines(&deny_consent, &c), Err(ViewError::NotConsented));
    let ends: Vec<(Vec<Entry>, &str)> = vec![
        (
            [
                asked(&c),
                vec![transition(Trigger::ActionDenied { effect: 1 })],
            ]
            .concat(),
            T9_DECLINED,
        ),
        (
            [
                asked(&c),
                vec![transition(Trigger::ApprovalLapsed {
                    effect: 1,
                    reason: LapseReason::ApprovalExpired,
                })],
            ]
            .concat(),
            T9_EXPIRED,
        ),
        (
            vec![
                consent.clone(),
                transition(Trigger::Stop {
                    reason: StopReason::Owner,
                }),
            ],
            T9_STOPPED,
        ),
        (
            [
                asked(&c),
                vec![transition(Trigger::Settled {
                    effect: 1,
                    action: None,
                    outcome: SettlementOutcome::NotPublished,
                    end: SettlementEnd::Stop(StopReason::Owner),
                })],
            ]
            .concat(),
            T9_STOPPED,
        ),
        (
            [
                asked(&c),
                vec![transition(Trigger::Settled {
                    effect: 1,
                    action: None,
                    outcome: SettlementOutcome::Uncertain,
                    end: SettlementEnd::Stop(StopReason::Owner),
                })],
            ]
            .concat(),
            T9_UNCERTAIN,
        ),
        // A Stop once a File was asked for, with no settlement: uncertain.
        (
            [
                asked(&c),
                vec![transition(Trigger::Stop {
                    reason: StopReason::Owner,
                })],
            ]
            .concat(),
            T9_UNCERTAIN,
        ),
    ];
    for (journal, line) in ends {
        assert_eq!(lines(&journal, &c), Ok(vec![line.to_string()]), "{line}");
    }
}

/// Composed cases run one broker per process.
const CHILD: &str = "oracles::m3e_display::m3e_m3b_child";

#[cfg(target_os = "linux")]
#[test]
fn m3e_m3b_view_is_read_only_and_the_next_call_still_enforces_the_budget() {
    crate::adapter::m3d_publication_oracles::run_child(CHILD, "view");
}

/// O34 (M3E-R14): broker-clock expiry is unreachable by design, since
/// Mother's held deadline is never later than the broker's.
#[cfg(target_os = "linux")]
#[test]
fn m3e_m3b_native_approval_expired_runs_no_verifier() {
    crate::adapter::m3d_publication_oracles::run_child(CHILD, "before-expiry");
    crate::adapter::m3d_publication_oracles::run_child(CHILD, "expired");
}

#[cfg(target_os = "linux")]
#[test]
fn m3e_m3b_child() {
    let Some(case) = crate::adapter::m3d_publication_oracles::child_case() else {
        return;
    };
    match case.as_str() {
        "view" => view_case(),
        "before-expiry" => expiry_case(false),
        "expired" => expiry_case(true),
        other => panic!("unknown case {other}"),
    }
    println!("m3d-db-receipt:{case}");
}

/// The lanes of a page mission: the Builder asks for `index.html` with the
/// bakery page, the real page Verifier checks it; the others are scripted.
#[cfg(target_os = "linux")]
fn page_lanes() -> crate::coordinator::Lanes {
    use crate::lane::{Lane, LaneRole, ScriptedLane, Verdict};
    use std::sync::Arc;
    let page = crate::adapter::m3d_publication_oracles::BAKERY;
    let mut lanes: Vec<Arc<dyn Lane>> = crate::oracles::keyed_lanes()
        .into_iter()
        .filter(|l| !matches!(l.identity().role, LaneRole::Builder | LaneRole::Verifier))
        .collect();
    let builder = ScriptedLane::new(LaneRole::Builder, "built", Verdict::Continue);
    let file = crate::oracles::lite_support::file("index.html", page);
    lanes.push(Arc::new(builder.asking(vec![file])));
    lanes.push(Arc::new(crate::page_verifier::PageVerifierLane));
    crate::coordinator::Lanes::new(lanes).unwrap()
}

#[cfg(target_os = "linux")]
fn verifier_calls(entries: &[Entry]) -> usize {
    use crate::lane::LaneRole;
    let verifier = |e: &&Entry| matches!(e.event, Event::LaneInvoked { role, .. } if role == LaneRole::Verifier);
    entries.iter().filter(verifier).count()
}

/// R12 (1): views change nothing, before and after a lapse; the next real
/// interaction still records it.
#[cfg(target_os = "linux")]
fn view_case() {
    use crate::adapter::lite_clock::TestClock;
    use crate::adapter::m3d_publication_oracles::PageFx;
    use crate::coordinator::Coordinator;
    use crate::effect::EffectPort;
    use crate::evidence::BlockReason;
    use crate::oracles::{m2c_confirmers::answering, Yes};
    use crate::state::MissionState;
    use std::sync::Arc;
    use std::time::Duration;
    let f = PageFx::new();
    let clock = Arc::new(TestClock::default());
    let m = Coordinator::open(
        f.journal.clone(),
        f.port.clone(),
        page_lanes(),
        clock.clone(),
        f.home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    let mut c = f.contract();
    c.budget.max_duration_ms = 60_000;
    let id = m.draft(c).unwrap();
    m.submit(id).unwrap();
    assert_eq!(
        answering(&Yes, || m.request_consent(id)),
        Ok(MissionState::Approved)
    );
    for _ in 0..3 {
        m.advance(id).unwrap();
    }
    assert_eq!(m.state(id), Ok(Some(MissionState::AwaitingApproval)));
    let first = m.journal_view().expect("a view");
    let (lanes, acks) = (lane_calls(&first), f.port.acks_tried());
    let pending = f.port.pending(id);
    assert!(pending.is_some());
    for lapse in [false, true] {
        if lapse {
            clock.advance(Duration::from_secs(120));
        }
        for _ in 0..3 {
            let view = m.journal_view().expect("a view");
            assert_eq!(view, first, "the view changes nothing (lapse {lapse})");
            assert_eq!(f.journal.lines().len(), first.len());
            assert_eq!((lane_calls(&view), f.port.acks_tried()), (lanes, acks));
            assert_eq!(f.port.pending(id), pending, "the action still waits");
        }
    }
    // The next real interaction enforces the lapse.
    assert!(m.state(id).unwrap().is_some_and(|s| s.is_terminal()));
    let after = m.journal_view().unwrap();
    let budget = Trigger::Block {
        reason: BlockReason::BudgetTime,
    };
    let recorded = |e: &Entry| {
        matches!(&e.event, Event::Transition { trigger } if *trigger == budget)
            || format!("{:?}", e.event).contains("BudgetTime")
    };
    assert!(after[first.len()..].iter().any(recorded), "BudgetTime");
}

#[cfg(target_os = "linux")]
fn lane_calls(entries: &[Entry]) -> usize {
    entries
        .iter()
        .filter(|e| matches!(e.event, Event::LaneInvoked { .. }))
        .count()
}

/// O34 carry (R14): `expired`, the broker's clock passes Mother's held
/// deadline while the broker's own record is live (Prepared) and the
/// mission budget is live: approval_expired, no Verifier, no check record,
/// no VerifyPass, no page, T9 expired, and the broker record ends Revoked
/// (withdrawn), never Expired. Otherwise the counter-control 1 s before:
/// the approval path, the page, and the real Verifier's recorded checks.
#[cfg(target_os = "linux")]
fn expiry_case(expired: bool) {
    use crate::adapter::lite_clock::LiteHome;
    use crate::adapter::m2c_support::port_for;
    use crate::adapter::{create_private, mother_agent};
    use crate::coordinator::Coordinator;
    use crate::effect::EffectPort;
    use crate::journal::MemoryJournal;
    use crate::oracles::lite_support::FilesOwner;
    use crate::oracles::{m2c_confirmers::answering, Yes};
    use crate::state::MissionState;
    use nexus_governed_control::authority::clock::{Clock, SystemClock};
    use nexus_governed_control::authority::commitment::CommitmentState;
    use nexus_governed_control::authority::ids::{CommitmentId, RunId};
    use nexus_governed_control::governed::GrantRequest;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    let temp = crate::adapter::m3d_publication_oracles::scratch("m3e-o34");
    let home = LiteHome::open(&temp.join("home")).unwrap();
    let ws = temp.join("workspace");
    create_private(&ws).unwrap();
    let info = home.control.register_workspace(&ws, &[]).unwrap();
    let owner = FilesOwner::default();
    home.control
        .request_grant(
            &GrantRequest::Files { workspace: info.id },
            Duration::from_secs(86400),
            &owner,
        )
        .unwrap();
    let port = Arc::new(port_for(&home.control, true).with_workspace(info.name.clone()));
    let journal = MemoryJournal::default();
    // Mother keeps the system clock: her budget stays live.
    let m = Coordinator::open(
        journal.clone(),
        port.clone(),
        page_lanes(),
        Arc::new(SystemClock::default()),
        home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    let c = page_contract(&info.name);
    let id = m.draft(c.clone()).unwrap();
    m.submit(id).unwrap();
    assert_eq!(
        answering(&Yes, || m.request_consent(id)),
        Ok(MissionState::Approved)
    );
    // As lite_recognition's deadline cases: the broker prepares 30 s after
    // the port's reading, so its record outlives Mother's held deadline.
    let clock = home.clock.clone();
    port.in_broker_action.arm(0, move || {
        let reads = AtomicUsize::new(0);
        let advance = clock.clone();
        clock.arm(
            move || reads.fetch_add(1, Ordering::SeqCst) == 1,
            move || advance.advance(Duration::from_secs(30)),
        );
    });
    for _ in 0..3 {
        m.advance(id).unwrap();
    }
    assert_eq!(m.state(id), Ok(Some(MissionState::AwaitingApproval)));
    assert!(!home.clock.disarm(), "the 30 s gap was taken");
    let pending = port.pending(id).unwrap();
    let run = RunId::parse(&pending.run).unwrap();
    let commitment = CommitmentId::parse(&pending.commitment).unwrap();
    let record = || {
        home.control
            .authority()
            .commitments()
            .view(commitment)
            .unwrap()
    };
    let left = pending
        .deadline_mono_ms
        .saturating_sub(home.clock.monotonic_ms());
    // The clocks keep running in real time, so the counter-control stands
    // one second (not one millisecond) before Mother's deadline.
    home.clock.advance(Duration::from_millis(if expired {
        left
    } else {
        left - 1000
    }));
    let live = record();
    assert_eq!(
        live.state,
        CommitmentState::Prepared,
        "the broker record is live"
    );
    assert!(live.expires_wall_ms > home.clock.wall_ms());
    if !expired {
        assert_eq!(m.advance(id).unwrap().to, MissionState::AwaitingApproval);
        home.control
            .authorize(commitment, &mother_agent(), run, &owner)
            .unwrap();
        home.control
            .execute(commitment, &mother_agent(), run)
            .unwrap();
        assert_eq!(m.advance(id).unwrap().to, MissionState::Verifying);
        assert_eq!(m.advance(id).unwrap().to, MissionState::Explaining);
        assert_eq!(m.advance(id).unwrap().to, MissionState::Done);
        let view = m.journal_view().unwrap();
        assert_eq!(verifier_calls(&view), 1, "the approval path verifies");
        assert!(view
            .iter()
            .any(|e| matches!(e.event, Event::VerificationRecorded { .. })));
        assert!(ws.join("index.html").exists());
        return;
    }
    let result = m.advance(id).unwrap();
    assert_eq!(result.to, MissionState::Blocked);
    assert_eq!(m.reason(id).unwrap().as_deref(), Some("approval_expired"));
    let view = m.journal_view().unwrap();
    assert_eq!(verifier_calls(&view), 0, "no Verifier ran");
    assert!(!view
        .iter()
        .any(|e| matches!(e.event, Event::VerificationRecorded { .. })));
    let pass = |e: &Entry| matches!(&e.event, Event::Transition { trigger } if *trigger == Trigger::VerifyPass);
    assert!(!view.iter().any(pass), "no VerifyPass");
    assert!(!ws.join("index.html").exists(), "no page");
    assert_eq!(
        page_view(&view, id).lines(&c, FACTS_FILE),
        Ok(vec![T9_EXPIRED.to_string()])
    );
    assert_eq!(
        record().state,
        CommitmentState::Revoked,
        "withdrawn, never Expired"
    );
}
