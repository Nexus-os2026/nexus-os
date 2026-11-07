//! Cb2 lifecycle composition: the actual lane, real coordinator, ordinary
//! broker and fake peer; observers only pause and record (one case per child).
use super::{
    c1b_coordinator_oracles::within,
    c1b_fake_ollama::{chat, spec, Answer, Seen},
    c1b_port_oracles::{children, record_next},
    m2c_support::port_for,
    m3cb_support::*,
};
use crate::coordinator::{inspect_history_lines, Coordinator, CoordinatorError, Lanes};
use crate::effect::{EffectPort, ModelAttempt};
use crate::journal::{verify, Event, MemoryJournal};
use crate::lane::{Lane, LaneContext, LaneRole, ModelCallError, ModelCaller, Verdict};
use crate::page_builder::{cb_fixtures::*, lane::PageBuilderLane};
use crate::state::MissionState;
use nexus_governed_control::authority::{evidence::EndStatus, file_session::FileSessionError};
use nexus_governed_control::inference::{
    CallDeadline, CallMeter, Charged, InvocationOwner, MeterRefusal, ModelIdentity, ModelReply,
    ModelRequest,
};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{mpsc::channel, Arc};
use std::time::Duration;

const VAR: &str = "NEXUS_M3CB2_LIFE";
pub(super) const NOTE: &str = "CB_NOTE_CANARY 面包";
pub(super) const FACT: &str = "CBFACTCANARY";
pub(super) const LATE: &str = "CB_LATE_CANARY";
fn cases(test: &str, list: &[&str]) {
    children("adapter::m3cb_lifecycle_oracles", VAR, test, list, |c| {
        run(c, false);
    });
}
fn scenario(replies: Vec<Answer>, private: bool) -> Scenario {
    let edit = |c: &mut crate::contract::MissionContract| {
        if private {
            c.notes[0].text = NOTE.into();
            c.page_facts.as_mut().unwrap().hours = vec![FACT.into()];
        }
    };
    Scenario::with(replies, edit, None)
}
fn late() -> String {
    with_lead(&format!("{LATE} arrives."))
}
/// Chat `n` (0-based) is counted by the peer, then waits until released.
fn gated(s: &Scenario, n: usize, replies: Vec<Answer>) -> Held {
    let (p, held) = pause();
    let calls = AtomicUsize::new(0);
    s.fx.fake.route(move |path| {
        (path == "/api/chat").then(|| {
            let i = calls.fetch_add(1, Ordering::SeqCst);
            if i == n {
                p.hold();
            }
            replies[i.min(replies.len() - 1)].clone()
        })
    });
    held
}
fn pause_call(s: &Scenario, at: usize) -> Held {
    let (p, held) = pause();
    s.seen.lock().unwrap().pause_call = Some((at, p));
    held
}
fn failed(s: &Scenario, code: &str) {
    let o = s.output();
    assert_eq!(
        o.verdict,
        Verdict::Fail {
            reason: code.into()
        }
    );
    assert_eq!((o.text.as_str(), o.effects.len()), ("", 0));
}
/// The File effect's broker-staged copies (published only after the Owner).
pub(super) fn staged(s: &Scenario) -> Vec<std::path::PathBuf> {
    let runs = s.fx.home.dir.join("broker/files/coding-runs");
    let Ok(dirs) = std::fs::read_dir(runs) else {
        return vec![];
    };
    let pages = dirs.map(|d| d.unwrap().path().join("index.html"));
    pages.filter(|p| p.exists()).collect()
}
fn nothing_accepted(s: &Scenario) {
    assert_eq!(s.fx.model_lane_returned(), 0, "no accepted Builder output");
    assert_eq!(s.fx.dispatched(), 0, "no File dispatch");
    assert!(!s.fx.index_exists() && staged(s).is_empty(), "no File");
}
/// Caller entries, peer chats, `ModelCallCharged`, native attempt records.
fn tally(s: &Scenario) -> (usize, usize, usize, usize) {
    let calls = s.seen.lock().unwrap().calls;
    let (chats, charges) = (s.fx.fake.count("/api/chat"), s.fx.charges());
    (calls, chats, charges, s.fx.attempts())
}
fn results(s: &Scenario) -> Vec<String> {
    s.seen.lock().unwrap().results.clone()
}
/// Runs one schedule with its judgments; returns it and its returned errors.
pub(super) fn run(case: &str, p: bool) -> (Scenario, String) {
    let (kind, arg) = case.split_once('/').unwrap_or((case, ""));
    match kind {
        "plain" => {
            let replies = match arg {
                "one" => vec![chat(DRAFT)],
                "two" => vec![chat(&rejected(2)), chat(DRAFT)],
                "bad" => vec![chat(&rejected(2)), chat(&rejected(7))],
                _ => vec![Answer::Json(503, "CB_TRANSPORT_CANARY".into())],
            };
            let s = scenario(replies, p);
            let diag = format!("{:?}", within(&s.advance_bg()));
            assert_eq!(s.fx.model_invocations(), 1);
            // Each schedule's exact response path (Cb2-R1).
            let (want, n): (&[&str], usize) = match arg {
                "one" => (&["ok"], 1),
                "two" | "bad" => (&["ok", "ok"], 2),
                _ => (&["failed:model_unavailable"], 1),
            };
            assert_eq!(results(&s), want);
            assert_eq!(tally(&s), (n, n, n, n));
            if arg == "one" || arg == "two" {
                let o = s.output();
                assert_eq!((o.verdict, o.effects.len()), (Verdict::Continue, 1));
                assert_eq!((s.fx.dispatched(), staged(&s).len()), (1, 1), "staged File");
                assert_eq!(s.m.state(s.id), Ok(Some(MissionState::AwaitingApproval)));
            } else {
                let code = if arg == "bad" {
                    "model_draft_email"
                } else {
                    "model_unavailable"
                };
                failed(&s, code);
                nothing_accepted(&s);
                assert_eq!(s.fx.end(s.id), "fail:lane_fail:builder");
            }
            (s, diag)
        }
        "invalidated" => invalidated(p),
        "between" => between(arg, p),
        "during" => during(arg, p),
        "terminal" => terminal(arg, p),
        "post" => post(arg, p),
        "debt" => debt(arg, p),
        "journal" => journal(p),
        "replay" => replay(),
        "fresh3" => fresh3(),
        "verifier" => verifier(),
        _ => panic!("unregistered {case}"),
    }
}
fn invalidated(p: bool) -> (Scenario, String) {
    let s = scenario(vec![], p);
    let gate = gated(&s, 0, vec![chat(&late())]);
    let replied = Arc::new(AtomicUsize::new(0));
    let r = replied.clone();
    s.m.hooks_for_test().model_replied.arm(0, move || {
        r.fetch_add(1, Ordering::SeqCst);
    });
    let adv = s.advance_bg();
    gate.reached();
    s.m.overtake_model_for_test(s.id).unwrap();
    assert!(!s.m.cell_for_test().is_engaged(), "invalidation, not Stop");
    gate.release();
    let diag = format!("{:?}", within(&adv));
    assert_eq!(
        replied.load(Ordering::SeqCst),
        1,
        "recorded successful transport"
    );
    assert_eq!(results(&s), ["invalidated"]);
    failed(&s, "model_invalidated");
    assert_eq!(tally(&s), (1, 1, 1, 1), "no correction; charge retained");
    nothing_accepted(&s);
    assert_eq!(s.fx.end(s.id), "fail:lane_fail:builder");
    (s, diag)
}
fn between(arg: &str, p: bool) -> (Scenario, String) {
    let s = scenario(vec![chat(&rejected(2)), chat(DRAFT)], p);
    let held = pause_call(&s, 1);
    let adv = s.advance_bg();
    held.reached();
    let end = match arg {
        "stop" => {
            assert_eq!(s.m.stop(s.id), Ok(MissionState::Stopped));
            "stop:owner"
        }
        "invalidate" => {
            s.m.overtake_model_for_test(s.id).unwrap();
            "fail:lane_fail:builder"
        }
        _ => {
            // The observed deadline: FileSession::state runs the budget touch.
            s.fx.clock.advance(Duration::from_millis(60_000));
            let _ = s.m.state(s.id);
            assert!(s.m.cell_for_test().is_engaged(), "deadline latched");
            "block:budget_time"
        }
    };
    held.release();
    let diag = format!("{:?}", within(&adv));
    assert_eq!(results(&s), ["ok", "invalidated"]);
    failed(&s, "model_invalidated");
    assert_eq!(tally(&s), (2, 1, 1, 1), "no correction chat or charge");
    nothing_accepted(&s);
    assert_eq!(s.fx.end(s.id), end);
    (s, diag)
}
fn during(arg: &str, p: bool) -> (Scenario, String) {
    let s = scenario(vec![chat(&rejected(2)), chat(&late())], p);
    let (pz, held) = pause();
    let want = if arg == "held" {
        let n = AtomicUsize::new(0);
        s.fx.fake.route(move |path| {
            (path == "/api/chat").then(|| match n.fetch_add(1, Ordering::SeqCst) {
                0 => chat(&rejected(2)),
                _ => Answer::Hold,
            })
        });
        "failed:model_cancelled"
    } else {
        // The second reply is recorded; Stop lands before C1's release check.
        s.m.hooks_for_test().model_replied.arm(1, move || pz.hold());
        "invalidated"
    };
    let adv = s.advance_bg();
    if arg == "held" {
        assert!(s.fx.fake.wait(Seen::Request("/api/chat".into())));
        assert!(s.fx.fake.wait(Seen::Request("/api/chat".into())));
    } else {
        held.reached();
    }
    assert_eq!(s.fx.charges(), 2);
    assert_eq!(s.m.stop(s.id), Ok(MissionState::Stopped));
    if arg == "held" {
        assert!(s.fx.fake.wait(Seen::Closed("/api/chat".into())));
    } else {
        held.release();
    }
    let diag = format!("{:?}", within(&adv));
    assert_eq!(results(&s), ["ok", want]);
    failed(
        &s,
        want.strip_prefix("failed:").unwrap_or("model_invalidated"),
    );
    assert_eq!(tally(&s), (2, 2, 2, 2), "no third call");
    nothing_accepted(&s);
    assert_eq!(s.fx.end(s.id), "stop:owner");
    (s, diag)
}
fn terminal(arg: &str, p: bool) -> (Scenario, String) {
    let (how, held) = arg.split_once('-').unwrap();
    let s = scenario(vec![chat(DRAFT)], p);
    let path: &'static str = if held == "chat" {
        "/api/chat"
    } else {
        "/api/tags"
    };
    s.fx.hold(path);
    let adv = s.advance_bg();
    assert!(s.fx.fake.wait(Seen::Request(path.into())));
    let charged = usize::from(held == "chat");
    assert_eq!(s.fx.charges(), charged);
    match how {
        "stop" => assert_eq!(s.m.stop(s.id), Ok(MissionState::Stopped)),
        "halt" => super::written(
            s.fx.port
                .door
                .halt(nexus_governed_control::governed::HaltReason::JournalFailed)
                .unwrap(),
        ),
        "emergency" => {
            s.fx.home.control.emergency_stop();
        }
        "cancel" => {
            s.fx.home
                .control
                .cancel_run(s.fx.port.run_of(s.id).unwrap())
                .unwrap()
        }
        _ => s.fx.clock.advance(Duration::from_millis(60_000)),
    }
    assert!(s.fx.fake.wait(Seen::Closed(path.into())));
    let diag = format!("{:?}", within(&adv));
    let code = match how {
        "emergency" => "model_stopped",
        "deadline" => "model_timeout",
        _ => "model_cancelled",
    };
    assert_eq!(results(&s), [format!("failed:{code}")]);
    failed(&s, code);
    let (calls, chats, charges, _) = tally(&s);
    assert_eq!((calls, chats, charges), (1, charged, charged));
    nothing_accepted(&s);
    let end = match how {
        "stop" => "stop:owner",
        "emergency" => "stop:emergency_stop",
        "halt" => "stop:door_halted",
        "deadline" => "block:budget_time",
        _ => "fail:lane_fail:builder",
    };
    assert_eq!(s.fx.end(s.id), end);
    (s, diag)
}
fn post(arg: &str, p: bool) -> (Scenario, String) {
    let s = scenario(vec![chat(&late())], p);
    let (pz, held) = pause();
    s.seen.lock().unwrap().pause_answer = Some(pz);
    let adv = s.advance_bg();
    held.reached();
    // Candidate request data exists; zero assembly is not claimed.
    let o = s.output();
    assert_eq!((o.verdict, o.effects.len()), (Verdict::Continue, 1));
    if arg == "stop" {
        assert_eq!(s.m.stop(s.id), Ok(MissionState::Stopped));
    } else {
        s.m.overtake_model_for_test(s.id).unwrap();
    }
    held.release();
    let diag = format!("{:?}", within(&adv));
    assert_eq!(tally(&s), (1, 1, 1, 1));
    nothing_accepted(&s);
    if arg == "stop" {
        assert_eq!(s.fx.end(s.id), "stop:owner");
    }
    (s, diag)
}
fn debt(arg: &str, p: bool) -> (Scenario, String) {
    let s = scenario(vec![chat(DRAFT)], p);
    record_next(
        &s.fx.port,
        if arg == "uncertain" {
            EndStatus::Uncertain
        } else {
            EndStatus::Owed
        },
    );
    let mut diag = String::new();
    let advanced = if arg == "stop" {
        let gate = gated(&s, 0, vec![chat(DRAFT)]);
        let adv = s.advance_bg();
        gate.reached();
        // The Owner's Stop engages first, then pauses before it records.
        let (pz, stopping) = pause();
        s.m.hooks_for_test()
            .file_after_halt
            .arm(0, move || pz.hold());
        let (tx, stopped) = channel();
        let (m, id) = (s.m.clone(), s.id);
        std::thread::spawn(move || tx.send(m.stop(id)).unwrap());
        stopping.reached();
        gate.release();
        let advanced = within(&adv);
        stopping.release();
        let stop = within(&stopped);
        diag.push_str(&format!("{stop:?}"));
        assert_eq!(stop, Ok(MissionState::Stopped));
        advanced
    } else {
        within(&s.advance_bg())
    };
    diag.push_str(&format!("{advanced:?}"));
    assert_eq!(advanced.err(), Some(CoordinatorError::Recording));
    assert_eq!(results(&s), ["record_unavailable"]);
    failed(&s, "model_record_unavailable");
    assert_eq!(tally(&s), (1, 1, 1, 1), "charge retained, no retry");
    nothing_accepted(&s);
    let close = s.m.close();
    diag.push_str(&format!("{close:?}"));
    assert_eq!(close, Err(CoordinatorError::Recording), "no clean close");
    let end = if arg == "stop" {
        "stop:owner"
    } else {
        "block:broker_refused:evidence_unavailable"
    };
    assert_eq!(s.fx.end(s.id), end);
    (s, diag)
}
fn journal(p: bool) -> (Scenario, String) {
    let s = scenario(vec![chat(DRAFT)], p);
    let held = pause_call(&s, 0);
    let adv = s.advance_bg();
    held.reached();
    s.fx.journal.fail_after(0);
    held.release();
    let advanced = within(&adv);
    assert_eq!(
        s.fx.fake.count("/api/chat"),
        0,
        "an unrecorded charge sends nothing"
    );
    assert_eq!(s.fx.charges(), 0, "no appended charge");
    assert!(advanced.is_err(), "the recording error is observable");
    let r = results(&s);
    assert!(r.len() == 1 && r[0] != "ok", "no answer: {r:?}");
    let o = s.output();
    assert!(matches!(o.verdict, Verdict::Fail { .. }) && o.effects.is_empty());
    assert_eq!(s.seen.lock().unwrap().calls, 1, "no correction");
    nothing_accepted(&s);
    let close = s.m.close();
    assert!(close.is_err(), "no clean close after a failed append");
    (s, format!("{advanced:?} {close:?} {r:?}"))
}
fn replay() -> (Scenario, String) {
    let s = scenario(vec![], false);
    s.fx.hold("/api/chat");
    let adv = s.advance_bg();
    assert!(s.fx.fake.wait(Seen::Request("/api/chat".into())));
    let count = |lines: &[String]| {
        let events = verify(lines).unwrap();
        let n = |f: fn(&Event) -> bool| events.iter().filter(|e| f(&e.event)).count();
        (
            n(|e| matches!(e, Event::LaneInvoked { model_cap, .. } if *model_cap > 0)),
            n(|e| matches!(e, Event::ModelCallCharged { .. })),
            n(|e| matches!(e, Event::EffectDispatched { .. })),
        )
    };
    let mut lines = s.fx.journal.lines();
    let before = count(&lines);
    assert_eq!(before, (s.fx.model_invocations(), 1, 0));
    for _start in 0..2 {
        assert!(inspect_history_lines(&lines)
            .unwrap()
            .unresolved
            .contains(&s.id));
        let copy = MemoryJournal::new();
        copy.set_lines(lines.clone());
        let port = Arc::new(port_for(&s.fx.home.control, true).with_workspace(s.fx.folder.clone()));
        let mut lanes = crate::oracles::keyed_lanes();
        lanes.retain(|l| l.identity().role != LaneRole::Builder);
        lanes.push(Arc::new(PageBuilderLane));
        let reopened = Coordinator::open(
            copy.clone(),
            port,
            Lanes::new(lanes).unwrap(),
            s.fx.clock.clone(),
            s.fx.home.private(),
        );
        assert!(
            matches!(
                reopened,
                Err(CoordinatorError::FileSession(FileSessionError::Reopened))
            ),
            "reopen refused"
        );
        lines = copy.lines();
        assert_eq!(
            count(&lines),
            before,
            "no invocation, charge or File restored"
        );
    }
    assert!(inspect_history_lines(&lines)
        .unwrap()
        .unresolved
        .contains(&s.id));
    assert_eq!(s.fx.fake.count("/api/chat"), 1);
    let _ = s.m.stop(s.id);
    let diag = format!("{:?}", within(&adv));
    assert_eq!(s.fx.attempts(), 1, "only the held call's own record");
    assert_eq!((s.fx.fake.count("/api/chat"), s.fx.dispatched()), (1, 0));
    (s, diag)
}
/// L2: three direct actual-lane invocations in one process, default marker path.
struct Direct<'a>(&'a Scenario, AtomicU32);
impl CallMeter for Direct<'_> {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        Ok(Charged {
            call: self.1.fetch_add(1, Ordering::SeqCst) + 1,
        })
    }
    fn settle(&self, _: u32) {}
}
impl CallDeadline for Direct<'_> {
    fn expired(&self) -> bool {
        false
    }
}
impl ModelCaller for Direct<'_> {
    fn call(&self, r: &ModelRequest) -> Result<ModelReply, ModelCallError> {
        // SB-3A (#127): the ordinary port ignores the caller's observer; keep one live.
        let owner = InvocationOwner::new();
        match self
            .0
            .fx
            .port
            .model_call(self.0.id, r, self, self, &owner.observer())
        {
            Ok(ModelAttempt::Replied(reply)) => Ok(reply),
            _ => Err(ModelCallError::Invalidated),
        }
    }
}
fn fresh3() -> (Scenario, String) {
    let s = scenario(vec![chat(DRAFT), chat(DRAFT), chat(DRAFT)], false);
    let identity = ModelIdentity::from_spec(&spec()).unwrap().digest();
    let a = &s.asked;
    s.fx.port
        .admit_inference(s.id, &a.contract_hash, &a.request, identity)
        .unwrap();
    let caller = Direct(&s, AtomicU32::new(0));
    for _ in 0..3 {
        let context = LaneContext {
            model: Some(&caller),
            contract: Some(&s.c),
        };
        let o = PageBuilderLane.invoke_in(&input(&s.c), &context);
        assert_eq!((o.verdict, o.effects.len()), (Verdict::Continue, 1));
    }
    let markers: Vec<String> = s
        .bodies()
        .iter()
        .map(|raw| {
            let v: serde_json::Value = serde_json::from_slice(raw).unwrap();
            let user = v["messages"][1]["content"].as_str().unwrap();
            let m = user.split("-----BEGIN FACTS ").nth(1).unwrap()[..32].to_string();
            v4(&m);
            assert_eq!(user.matches(&m).count(), 4);
            m
        })
        .collect();
    assert_eq!(markers.len(), 3);
    assert!(markers[0] != markers[1] && markers[0] != markers[2] && markers[1] != markers[2]);
    (s, String::new())
}
/// L3: a cap-positive Verifier never receives the lent contract.
fn verifier() -> (Scenario, String) {
    let s = Scenario::with(vec![chat(DRAFT)], |_| {}, Some(1));
    s.advance();
    assert_eq!((s.fx.dispatched(), staged(&s).len()), (1, 1), "actual File");
    // The Owner authorizes publication; the mission continues to its Verifier.
    let p = s.fx.port.pending(s.id).unwrap();
    let run = nexus_governed_control::authority::ids::RunId::parse(&p.run).unwrap();
    let c = nexus_governed_control::authority::ids::CommitmentId::parse(&p.commitment).unwrap();
    let (control, agent) = (&s.fx.home.control, super::mother_agent());
    let owner = crate::oracles::lite_support::FilesOwner::default();
    control.authorize(c, &agent, run, &owner).unwrap();
    control.execute(c, &agent, run).unwrap();
    for _ in 0..8 {
        if s.m.state(s.id).unwrap().is_some_and(|m| m.is_terminal()) {
            break;
        }
        s.advance();
    }
    let seen = s.seen.lock().unwrap().contexts.clone();
    assert!(seen
        .iter()
        .any(|(r, n, c)| *r == LaneRole::Builder && *n == 2 && c.as_ref() == Some(&s.c)));
    assert!(seen
        .iter()
        .any(|(r, n, c)| *r == LaneRole::Verifier && *n == 1 && c.is_none()));
    (s, String::new())
}

#[test]
fn m3cb_invalidated_reply_cannot_publish() {
    cases("m3cb_invalidated_reply_cannot_publish", &["invalidated"]);
}
#[test]
fn m3cb_stop_between_attempts_sends_no_correction() {
    cases(
        "m3cb_stop_between_attempts_sends_no_correction",
        &["between/stop", "between/invalidate", "between/deadline"],
    );
}
#[test]
fn m3cb_stop_during_correction_discards_reply() {
    cases(
        "m3cb_stop_during_correction_discards_reply",
        &["during/held", "during/late"],
    );
}
#[test]
fn m3cb_stop_cancel_halt_and_deadline_are_terminal() {
    let mut list = vec![];
    for how in ["stop", "emergency", "halt", "cancel", "deadline"] {
        for held in ["tags", "chat"] {
            list.push(format!("terminal/{how}-{held}"));
        }
    }
    let list: Vec<&str> = list.iter().map(String::as_str).collect();
    cases("m3cb_stop_cancel_halt_and_deadline_are_terminal", &list);
}
#[test]
fn m3cb_invalidation_after_answer_discards_output() {
    cases(
        "m3cb_invalidation_after_answer_discards_output",
        &["post/invalidate", "post/stop"],
    );
}
#[test]
fn m3cb_record_debt_retains_charge_and_first_cause() {
    cases(
        "m3cb_record_debt_retains_charge_and_first_cause",
        &["debt/owed", "debt/uncertain", "debt/stop"],
    );
}
#[test]
fn m3cb_unrecorded_charge_sends_nothing() {
    cases("m3cb_unrecorded_charge_sends_nothing", &["journal"]);
}
#[test]
fn m3cb_replay_does_not_restore_caller() {
    cases("m3cb_replay_does_not_restore_caller", &["replay"]);
}
#[test]
fn m3cb_markers_fresh_across_invocations() {
    cases("m3cb_markers_fresh_across_invocations", &["fresh3"]);
}
#[test]
fn m3cb_context_not_lent_to_cap_positive_verifier() {
    cases(
        "m3cb_context_not_lent_to_cap_positive_verifier",
        &["verifier"],
    );
}
