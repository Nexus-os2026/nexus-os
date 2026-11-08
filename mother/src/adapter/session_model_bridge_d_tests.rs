//! SB-3 D1 Mother composition oracles (spec §9.3; SB3D-R1/R2): O1, O2, O3,
//! V1 and V2 through the real C1 caller → SessionPort → session broker →
//! fake loopback peer. Each case runs in its own child process (one broker
//! per process, bounded by the C1b child runner). Native record debt is
//! unreachable from Mother (the primary store is held EXCLUSIVE), so
//! Owed/Uncertain are injected at the adapter through the SB3D-R1 Q2 seam;
//! the native debt itself is B's evidence (SB-3B2-C `seal/owed`, SB-3B2-M
//! `settled:ok:uncertain`, B1e `evidence_failure`).
use crate::adapter::c1b_coordinator_oracles::{draft, outcome};
use crate::adapter::c1b_fake_ollama::{chat, Answer, Seen, NAME, PATIENCE};
use crate::adapter::c1b_port_oracles::{children as run_children, record_next};
use crate::adapter::session_model_bridge_support::{advance_in, signal, within, Mother, SFx};
use crate::contract::Note;
use crate::coordinator::CoordinatorError;
use crate::journal::{verify, Event, MissionId};
use crate::lane::{LaneContext, LaneOutput, ModelCallError, Verdict};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::evidence::EndStatus;
use nexus_governed_control::host_session::{CommandKind, SessionState};
use nexus_governed_control::inference::{ModelReply, ModelRequest, ResponseSchema};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CASE: &str = "NEXUS_SB3D1_CASE";
const MODULE: &str = "adapter::session_model_bridge_d_tests";
const NOTE: &str = "SB3D1_NOTE_CANARY";
const PROMPT: &str = "SB3D1_PROMPT_CANARY";
const REPLY: &str = "SB3D1_REPLY_CANARY";
const ERROR: &str = "SB3D1_ERROR_CANARY";
const METADATA: &str = "SB3D1_METADATA_CANARY";
/// The fake peer's runtime version, which its `/api/version` must keep.
const PEER_VERSION: &str = "0.40.13";
/// SB3C-R5: the one known production defect, asserted exactly as it is
/// today and fixed by unit SB3C-F1. Only this cell may carry the tag.
const KNOWN_DEFECT: [(&str, &str); 1] = [("first/budget-then-stop", "OPEN:SB3C-F1")];

fn children(test: &str, cases: &[&str]) {
    run_children(MODULE, CASE, test, cases, run);
}
fn run(case: &str) {
    let (name, arg) = case.split_once('/').unwrap_or((case, ""));
    match name {
        "debt" => debt(arg),
        "late" => late(arg),
        "first" => first(arg),
        "privacy" => privacy(arg),
        "memory" => memory(arg),
        "classify" => classified(arg),
        "recheck" => rechecked(arg),
        "after" => after_recheck(arg),
        other => panic!("unknown case {other}"),
    }
}

fn ask(context: &LaneContext<'_>) -> Result<ModelReply, ModelCallError> {
    let schema = ResponseSchema::new(r#"{"type":"object"}"#).unwrap();
    let request = ModelRequest::new("SB3D1 system".into(), PROMPT.into(), schema).unwrap();
    context
        .model
        .expect("a model lane gets a caller")
        .call(&request)
}
fn quiet() -> LaneOutput {
    LaneOutput {
        verification: None,
        text: "sb3d1 model lane".into(),
        effects: vec![],
        verdict: Verdict::Continue,
    }
}
fn events(f: &SFx) -> Vec<(MissionId, Event)> {
    let entries = verify(&f.home.lines()).unwrap();
    entries.into_iter().map(|e| (e.mission, e.event)).collect()
}
/// The terminal transition of `id`, as `kind:code`.
fn end(f: &SFx, id: MissionId) -> String {
    let trigger = events(f)
        .into_iter()
        .filter(|(m, _)| *m == id)
        .filter_map(|(_, e)| match e {
            Event::Transition { trigger } => Some(trigger),
            _ => None,
        })
        .next_back()
        .unwrap();
    match trigger {
        Trigger::Stop { reason } => format!("stop:{reason}"),
        Trigger::Block { reason } => format!("block:{reason}"),
        Trigger::Fail { reason } => format!("fail:{reason}"),
        other => other.name().to_string(),
    }
}
fn terminal(f: &SFx, id: MissionId) -> bool {
    events(f).iter().any(|(m, e)| {
        *m == id
            && matches!(e, Event::Transition { trigger } if matches!(
                trigger,
                Trigger::Stop { .. } | Trigger::Block { .. } | Trigger::Fail { .. }
            ))
    })
}
/// Answers journaled for the model lane's invocations.
fn returned(f: &SFx) -> usize {
    let events = events(f);
    let model: Vec<_> = events
        .iter()
        .filter_map(|(_, e)| match e {
            Event::LaneInvoked {
                invocation,
                model_cap,
                ..
            } if *model_cap > 0 => Some(*invocation),
            _ => None,
        })
        .collect();
    let back = |e: &Event| matches!(e, Event::LaneReturned { invocation, .. } if model.contains(invocation));
    events.iter().filter(|(_, e)| back(e)).count()
}
fn dispatched(f: &SFx) -> usize {
    let sent = |e: &Event| matches!(e, Event::EffectDispatched { .. });
    events(f).iter().filter(|(_, e)| sent(e)).count()
}
/// The broker's own native result record for the model slot.
fn native(f: &SFx) -> &'static str {
    f.home.session.close_obligations().unwrap().model.record
}
fn inject(f: &SFx, status: EndStatus) {
    record_next(f.home.port.inner_for_test(), status);
}
fn index(f: &SFx) -> bool {
    f.home.workspace.join("index.html").exists()
}
/// Every file under `dir`, concatenated (lossy).
fn walk(dir: &Path) -> String {
    let mut text = String::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                text.push_str(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
            }
        }
    }
    text
}
fn walk_file(path: &Path) -> String {
    String::from_utf8_lossy(&std::fs::read(path).unwrap()).into_owned()
}
/// The private store, where only the approved contract and draft may rest.
fn private(f: &SFx) -> String {
    walk(&f.home.dir.join("private"))
}
/// The peer holds `/api/chat` until released, then answers `content`.
fn hold_chat(
    f: &SFx,
    content: &'static str,
) -> (
    std::sync::mpsc::Receiver<()>,
    std::sync::mpsc::SyncSender<()>,
) {
    let (entered_tx, entered_rx) = signal();
    let (release_tx, release_rx) = signal();
    let release_rx = Mutex::new(release_rx);
    f.fake.route(move |p| {
        (p == "/api/chat").then(|| {
            let release = release_rx.lock().unwrap();
            let _ = entered_tx.send(());
            let _ = release.recv_timeout(PATIENCE);
            chat(content)
        })
    });
    (entered_rx, release_tx)
}

// ------------------------------------------------------------------- O1

/// O1: a transport success that is Owed or Uncertain is unusable through
/// SessionPort: `record_unavailable`, the debt latches, later work and the
/// clean close are refused; the charge and the broker's own record stay.
#[test]
fn sb3_unrecorded_reply_is_unusable() {
    children(
        "sb3_unrecorded_reply_is_unusable",
        &["debt/recorded", "debt/owed", "debt/uncertain"],
    );
}
fn debt(arg: &str) {
    let f = SFx::new();
    let (tx, rx) = channel();
    let m = f.mother(2, move |ctx| {
        let first = ask(ctx);
        let exact = first.as_ref().is_ok_and(|r| r.content() == REPLY);
        let second = outcome(&ask(ctx));
        tx.send((outcome(&first), second, exact)).unwrap();
        draft(&first.map(ModelReply::into_content).unwrap_or_default())
    });
    let id = f.building(&m, f.contract());
    f.fake.route(|p| (p == "/api/chat").then(|| chat(REPLY)));
    match arg {
        "owed" => inject(&f, EndStatus::Owed),
        "uncertain" => inject(&f, EndStatus::Uncertain),
        _ => {}
    }
    let advanced = m.advance(id);
    let (first, second, exact) = within(&rx);
    assert_eq!(f.home.session.state(), SessionState::Normal, "{arg}");
    if arg == "recorded" {
        assert_eq!(
            (first.as_str(), second.as_str(), exact),
            ("ok", "ok", true),
            "the positive: the exact reply"
        );
        advanced.unwrap();
        assert_eq!(returned(&f), 1);
        assert_eq!(f.charges(), 2);
        return;
    }
    let pair = (first.as_str(), second.as_str());
    assert_eq!(
        pair,
        ("record_unavailable", "invalidated"),
        "{arg}: unusable"
    );
    assert_eq!(advanced.err(), Some(CoordinatorError::Recording), "{arg}");
    assert_eq!(
        end(&f, id),
        "block:broker_refused:evidence_unavailable",
        "{arg}"
    );
    assert_eq!(
        (returned(&f), dispatched(&f)),
        (0, 0),
        "{arg}: nothing accepted"
    );
    assert!(!index(&f), "{arg}: no File effect");
    assert!(
        !private(&f).contains(REPLY) && !f.home.lines().concat().contains(REPLY),
        "{arg}: no private rest"
    );
    assert_eq!(f.charges(), 1, "{arg}: the charge is retained");
    assert_eq!(f.fake.count("/api/chat"), 1, "{arg}: no second chat");
    assert_eq!(
        native(&f),
        "recorded",
        "{arg}: the broker's own record stays"
    );
    assert!(
        m.draft(f.contract()).is_err(),
        "{arg}: later work is refused"
    );
    assert_eq!(
        m.close(),
        Err(CoordinatorError::Recording),
        "{arg}: no clean close"
    );
    let closed = events(&f)
        .iter()
        .any(|(_, e)| matches!(e, Event::SessionClosed));
    assert!(!closed, "{arg}: no session end");
}

// ------------------------------------------------------------------- O2

/// O2: invalidation wins at each Mother-reachable point (the broker
/// interior is B2's reply:after/record:before cells, SB3D-R1 Q3): with the
/// chat held, at `model_replied` (after the record, before acceptance) and
/// after the lane returned (final result admission). No accepted output,
/// LaneReturned or File dispatch; the native record stays conclusive.
#[test]
fn sb3_late_content_never_projects() {
    children(
        "sb3_late_content_never_projects",
        &["late/chat", "late/replied", "late/returned"],
    );
}
fn late(point: &str) {
    let f = SFx::new();
    let (seen_tx, seen_rx) = channel();
    let (go_tx, go_rx) = signal();
    let go_rx = Mutex::new(go_rx);
    let after = point == "returned";
    let m = f.mother(1, move |ctx| {
        let r = ask(ctx);
        let content = r
            .as_ref()
            .is_ok_and(|reply| reply.content().contains(REPLY));
        seen_tx.send((outcome(&r), content)).unwrap();
        if after {
            let _ = go_rx.lock().unwrap().recv_timeout(PATIENCE);
        }
        draft(&r.map(ModelReply::into_content).unwrap_or_default())
    });
    let id = f.building(&m, f.contract());
    let (entered, release) = hold_chat(&f, REPLY);
    let (replied_tx, replied_rx) = signal();
    let (resume_tx, resume_rx) = signal();
    let resume_rx = Mutex::new(resume_rx);
    let pause = point == "replied";
    m.hooks_for_test().model_replied.arm(0, move || {
        let _ = replied_tx.send(());
        if pause {
            let _ = resume_rx.lock().unwrap().recv_timeout(PATIENCE);
        }
    });
    let adv = advance_in(&m, id);
    within(&entered);
    if point == "chat" {
        m.overtake_model_for_test(id).unwrap();
    }
    release.send(()).unwrap();
    within(&replied_rx); // The broker released a recorded success.
    if point == "replied" {
        m.overtake_model_for_test(id).unwrap();
        resume_tx.send(()).unwrap();
    }
    let (got, content) = within(&seen_rx);
    if after {
        assert_eq!(
            (got.as_str(), content),
            ("ok", true),
            "returned: the lane had the reply"
        );
        m.overtake_model_for_test(id).unwrap();
        go_tx.send(()).unwrap();
    } else {
        let seen = (got.as_str(), content);
        assert_eq!(
            seen,
            ("invalidated", false),
            "{point}: never reaches the lane"
        );
    }
    assert!(
        adv.join().unwrap().is_err(),
        "{point}: the advance is refused"
    );
    assert_eq!(returned(&f), 0, "{point}: no LaneReturned");
    assert_eq!(dispatched(&f), 0, "{point}: no File dispatch");
    assert!(!index(&f), "{point}: no accepted output");
    assert!(
        !f.home.lines().concat().contains(REPLY),
        "{point}: no content"
    );
    assert!(!private(&f).contains(REPLY), "{point}: no private draft");
    assert_eq!(f.fake.count("/api/chat"), 1, "{point}: one chat");
    assert_eq!(
        native(&f),
        "recorded",
        "{point}: the conclusive native record"
    );
}

// ------------------------------------------------------------------- O3

/// O3: the first terminal cause keeps its literal through SessionPort;
/// the reverse order keeps the earlier cause, and recording debt stays a
/// separate fact.
#[test]
fn sb3_first_terminal_cause_survives_model_failure() {
    children(
        "sb3_first_terminal_cause_survives_model_failure",
        &[
            "first/debt-then-stop",
            "first/stop-then-debt",
            "first/budget-then-stop",
            "first/budget",
            "first/emergency",
            "first/fail-then-stop",
            "first/stop-then-fail",
        ],
    );
}
fn first(arg: &str) {
    let f = Arc::new(SFx::new());
    let (tx, rx) = channel();
    let (go_tx, go_rx) = signal();
    let go_rx = Mutex::new(go_rx);
    let (ready_tx, ready_rx) = signal();
    let (fx, mode) = (f.clone(), arg.to_string());
    let m = f.mother(1, move |ctx| {
        if mode == "stop-then-fail" {
            let _ = ready_tx.send(());
            let _ = go_rx.lock().unwrap().recv_timeout(PATIENCE);
            return LaneOutput {
                verdict: Verdict::Fail {
                    reason: ERROR.into(),
                },
                ..quiet()
            };
        }
        let first = outcome(&ask(ctx));
        tx.send((first, terminal(&fx, MissionId(1)))).unwrap();
        if mode != "stop-then-debt" && mode != "emergency" {
            let _ = go_rx.lock().unwrap().recv_timeout(PATIENCE);
        }
        match mode.as_str() {
            "fail-then-stop" => LaneOutput {
                verdict: Verdict::Fail {
                    reason: ERROR.into(),
                },
                ..quiet()
            },
            _ => draft("x"),
        }
    });
    let id = f.building(&m, f.contract());
    assert_eq!(id, MissionId(1));
    let want = match arg {
        "debt-then-stop" => {
            inject(&f, EndStatus::Owed);
            let adv = advance_in(&m, id);
            let (first, terminal) = within(&rx);
            assert_eq!(first, "record_unavailable");
            assert!(!terminal, "denial before the generation moves");
            assert_eq!(m.stop(id), Ok(MissionState::Blocked), "Stop arrives second");
            go_tx.send(()).unwrap();
            assert_eq!(adv.join().unwrap().err(), Some(CoordinatorError::Recording));
            "block:broker_refused:evidence_unavailable"
        }
        "stop-then-debt" => {
            inject(&f, EndStatus::Owed);
            let (entered, release) = hold_chat(&f, "{}");
            let adv = advance_in(&m, id);
            within(&entered);
            let (paused_tx, paused_rx) = signal();
            let (resume_tx, resume_rx) = signal();
            let resume_rx = Mutex::new(resume_rx);
            m.hooks_for_test().file_after_halt.arm(0, move || {
                let _ = paused_tx.send(());
                let _ = resume_rx.lock().unwrap().recv_timeout(PATIENCE);
            });
            let stopper = {
                let m = m.clone();
                std::thread::spawn(move || m.stop(id))
            };
            within(&paused_rx); // The Owner's Stop is engaged, not yet recorded.
            release.send(()).unwrap();
            let (first, _) = within(&rx);
            assert_eq!(first, "record_unavailable");
            assert_eq!(adv.join().unwrap().err(), Some(CoordinatorError::Recording));
            resume_tx.send(()).unwrap();
            assert_eq!(stopper.join().unwrap(), Ok(MissionState::Stopped));
            "stop:owner"
        }
        "budget-then-stop" => {
            let (entered, release) = hold_chat(&f, "{}");
            let adv = advance_in(&m, id);
            within(&entered);
            f.home.clock.advance(Duration::from_millis(61_000));
            let (first, terminal) = within(&rx);
            assert_eq!(first, "failed:model_timeout");
            assert!(!terminal, "denial before the generation moves");
            let engaged = m.cell_for_test().is_engaged();
            assert!(engaged, "the budget denial is engaged before Stop");
            // OPEN:SB3C-F1 — today a later Stop replaces the budget cause on
            // the session route (the ordinary route keeps Blocked). SB3C-F1
            // flips this cell to Blocked / block:budget_time.
            let open = KNOWN_DEFECT.contains(&("first/budget-then-stop", "OPEN:SB3C-F1"));
            let (state, cause) = match open {
                true => (MissionState::Stopped, "stop:owner"),
                false => (MissionState::Blocked, "block:budget_time"),
            };
            assert_eq!(m.stop(id), Ok(state), "Stop arrives second");
            go_tx.send(()).unwrap();
            drop(release);
            assert!(adv.join().unwrap().is_err());
            cause
        }
        "budget" => {
            let (entered, release) = hold_chat(&f, "{}");
            let adv = advance_in(&m, id);
            within(&entered);
            f.home.clock.advance(Duration::from_millis(61_000));
            let (first, _) = within(&rx);
            assert_eq!(first, "failed:model_timeout");
            go_tx.send(()).unwrap();
            drop(release);
            assert!(adv.join().unwrap().is_err());
            "block:budget_time"
        }
        "emergency" => {
            f.fake.route(|p| (p == "/api/chat").then_some(Answer::Hold));
            f.fake.drain();
            let adv = advance_in(&m, id);
            assert!(
                f.fake.wait(Seen::Request("/api/chat".into())),
                "the chat is held"
            );
            f.home.session.ingress().emergency().unwrap();
            assert!(
                f.fake.wait(Seen::Closed("/api/chat".into())),
                "the held chat closes"
            );
            let (first, _) = within(&rx);
            assert_eq!(first, "failed:model_cancelled", "the emergency's reply");
            let _ = adv.join().unwrap();
            "stop:emergency_stop"
        }
        "fail-then-stop" => {
            let adv = advance_in(&m, id);
            let (first, _) = within(&rx);
            assert_eq!(first, "ok");
            let (paused_tx, paused_rx) = signal();
            let (resume_tx, resume_rx) = signal();
            let resume_rx = Mutex::new(resume_rx);
            m.hooks_for_test().file_after_halt.arm(0, move || {
                let _ = paused_tx.send(());
                let _ = resume_rx.lock().unwrap().recv_timeout(PATIENCE);
            });
            go_tx.send(()).unwrap();
            within(&paused_rx);
            assert_eq!(m.stop(id), Ok(MissionState::Failed), "Stop arrives second");
            resume_tx.send(()).unwrap();
            let _ = adv.join().unwrap();
            "fail:lane_fail:builder"
        }
        _ => {
            let adv = advance_in(&m, id);
            within(&ready_rx);
            assert_eq!(m.stop(id), Ok(MissionState::Stopped), "Stop first");
            go_tx.send(()).unwrap();
            let _ = adv.join().unwrap();
            "stop:owner"
        }
    };
    assert_eq!(end(&f, id), want, "{arg}: the first cause is kept");
    assert_eq!(dispatched(&f), 0, "{arg}: no File dispatch");
    assert!(
        !f.home.lines().concat().contains(ERROR),
        "{arg}: no reason prose"
    );
    let recording = matches!(arg, "debt-then-stop" | "stop-then-debt");
    let debt = m.close().err() == Some(CoordinatorError::Recording);
    assert_eq!(debt, recording, "{arg}: the recording fact is separate");
}

// ------------------------------------------------------------------- V1

/// V1: note, prompt, reply and error canaries (and their hex) stay out of
/// the Mother journal, the session broker's stores (SQLite, WAL, SHM),
/// Advance, errors, reasons and the close/startup diagnostics, on a
/// recorded success and error and on an owed record. The expected stores
/// exist and hold the expected records.
#[test]
fn sb3_canaries_stay_out_of_model_evidence() {
    children(
        "sb3_canaries_stay_out_of_model_evidence",
        &["privacy/replied", "privacy/owed"],
    );
}
fn privacy(arg: &str) {
    let f = SFx::new();
    let (tx, rx) = channel();
    let m = f.mother(2, move |ctx| {
        let first = ask(ctx);
        let second = ask(ctx);
        let saw = first.as_ref().is_ok_and(|r| r.content().contains(REPLY));
        let errors = format!("{first:?} {second:?}");
        tx.send((errors, saw)).unwrap();
        quiet()
    });
    let mut c = f.contract();
    c.notes = vec![Note {
        label: "notes".into(),
        text: format!("{NOTE} 面包"),
    }];
    let id = f.building(&m, c);
    let calls = AtomicUsize::new(0);
    let metadata = format!("{{\"version\":\"{PEER_VERSION}\",\"note\":\"{METADATA}\"}}");
    f.fake.route(move |p| match p {
        "/api/version" => Some(Answer::Json(200, metadata.clone())),
        "/api/chat" => Some(match calls.fetch_add(1, Ordering::SeqCst) {
            0 => chat(REPLY),
            _ => Answer::Json(500, format!("{{\"error\":\"{ERROR}\"}}")),
        }),
        _ => None,
    });
    if arg == "owed" {
        inject(&f, EndStatus::Owed);
    }
    let advance = advance_in(&m, id).join().unwrap();
    let (errors, saw) = within(&rx);
    assert_eq!(
        saw,
        arg == "replied",
        "only the lane, on a recorded attempt"
    );
    let broker = f.home.dir.join("broker");
    let sqlite = broker.join("p3-evidence/evidence.sqlite3");
    assert!(
        std::fs::metadata(&sqlite).unwrap().len() > 0,
        "the native store exists"
    );
    assert_eq!(native(&f), "recorded", "the native record exists");
    assert!(f.charges() >= 1, "the journal holds the charge");
    assert!(f.fake.count("/api/version") >= 1, "the metadata was served");
    let rest = private(&f);
    assert!(rest.contains(NOTE), "the approved contract rests privately");
    for canary in [PROMPT, REPLY, ERROR, METADATA] {
        assert!(!rest.contains(canary), "{arg}: {canary} at private rest");
        let hex = hex::encode(canary);
        assert!(
            !rest.contains(&hex),
            "{arg}: {canary} at private rest (hex)"
        );
    }
    let texts = [
        errors,
        format!("{advance:?}"),
        format!("{:?}", m.reason(id)),
        f.home.lines().concat(),
        walk(&broker),
        format!("{:?}", f.home.session.close_obligations()),
        format!("{:?}", f.home.session.startup_report()),
    ];
    for text in &texts {
        for canary in [NOTE, PROMPT, REPLY, ERROR, METADATA, "面包"] {
            assert!(!text.contains(canary), "{arg}: {canary} leaked");
            let hex = hex::encode(canary);
            assert!(!text.contains(&hex), "{arg}: {canary} leaked (hex)");
        }
    }
}

// ------------------------------------------------------------------- V2

/// V2: a recorded reply reaches the intended caller and its draft is kept
/// only on the private path (owner-only modes); a failed, unrecorded or
/// late reply becomes no deliverable anywhere.
#[test]
fn sb3_positive_reply_uses_only_intended_memory_path() {
    children(
        "sb3_positive_reply_uses_only_intended_memory_path",
        &[
            "memory/recorded",
            "memory/failed",
            "memory/rejected",
            "memory/unrecorded",
            "memory/late",
        ],
    );
}
fn memory(arg: &str) {
    let f = SFx::new();
    let (tx, rx) = channel();
    let m: Mother = f.mother(1, move |ctx| {
        let r = ask(ctx);
        tx.send(outcome(&r)).unwrap();
        r.map_or_else(|_| quiet(), |reply| draft(&reply.into_content()))
    });
    let id = f.building(&m, f.contract());
    let answer = match arg {
        "failed" => Answer::Json(500, format!("{{\"error\":\"{ERROR}\"}}")),
        // Unfinished, so the transport rejects it: the bytes reach Mother's
        // peer connection and must never become a reply.
        "rejected" => Answer::Json(
            200,
            serde_json::json!({
                "model": NAME,
                "message": {"role": "assistant", "content": REPLY},
                "done": false,
            })
            .to_string(),
        ),
        _ => chat(REPLY),
    };
    f.fake
        .route(move |p| (p == "/api/chat").then(|| answer.clone()));
    if arg == "unrecorded" {
        inject(&f, EndStatus::Owed);
    }
    let (replied_tx, replied_rx) = signal();
    let (resume_tx, resume_rx) = signal();
    let resume_rx = Mutex::new(resume_rx);
    if arg == "late" {
        m.hooks_for_test().model_replied.arm(0, move || {
            let _ = replied_tx.send(());
            let _ = resume_rx.lock().unwrap().recv_timeout(PATIENCE);
        });
    }
    let adv = advance_in(&m, id);
    if arg == "late" {
        within(&replied_rx);
        m.overtake_model_for_test(id).unwrap();
        resume_tx.send(()).unwrap();
    }
    let got = within(&rx);
    let advanced = adv.join().unwrap();
    // The intended paths: Mother's private store and the File run's staging.
    let private = f.home.dir.join("private");
    let staging = f.home.dir.join("broker/files");
    let kept = walk(&private) + &walk(&staging);
    let staged = std::fs::read_dir(f.home.dir.join("broker")).unwrap();
    let mut elsewhere = f.home.lines().concat();
    for entry in staged {
        let path = entry.unwrap().path();
        if path != staging {
            elsewhere += &if path.is_dir() {
                walk(&path)
            } else {
                String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned()
            };
        }
    }
    for canary in [REPLY, ERROR] {
        assert!(
            !elsewhere.contains(canary),
            "{arg}: {canary} outside the intended paths"
        );
    }
    let kept = kept.contains(REPLY);
    // Everything private, and in the staging tree its folders and every file
    // holding the accepted bytes, is owner-only.
    let owner_only = |path: &Path| {
        let mode = std::os::unix::fs::PermissionsExt::mode(&path.metadata().unwrap().permissions());
        assert_eq!(mode & 0o077, 0, "{arg}: owner-only {}", path.display());
    };
    owner_only(&staging);
    let mut pending = vec![private, staging.clone()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let holds_reply = || !path.is_dir() && walk_file(&path).contains(REPLY);
            if path.is_dir() || !path.starts_with(&staging) || holds_reply() {
                owner_only(&path);
            }
            if path.is_dir() {
                pending.push(path);
            }
        }
    }
    if arg == "recorded" {
        assert_eq!(got, "ok", "the recorded reply reaches the caller");
        assert!(advanced.is_ok(), "recorded: the lane result is admitted");
        assert_eq!(returned(&f), 1, "recorded: one LaneReturned");
        assert!(kept, "recorded: the draft is kept on the intended path");
        return;
    }
    assert_ne!(got, "ok", "{arg}: no usable reply");
    if !matches!(arg, "failed" | "rejected") {
        assert_eq!(returned(&f), 0, "{arg}: no LaneReturned");
    }
    assert_eq!(dispatched(&f), 0, "{arg}: no File dispatch");
    assert!(!kept, "{arg}: no deliverable on any path");
    assert!(!index(&f), "{arg}: no File effect");
}

// ------------------------------------------------- the acceptance sinks

/// One real session restriction; the state it leaves. `emergency` is the one
/// primitive CR, LF and CRLF reduce to (the terminal bytes are the retained
/// host evidence), `fatal` is `SessionIngress::fail`, `budget` is the mission
/// deadline on the shared clock.
fn restrict(f: &SFx, kind: &str) -> SessionState {
    let ingress = f.home.session.ingress();
    let quit = || drop(ingress.accept(CommandKind::Quit, 900).unwrap());
    match kind {
        "emergency" => {
            ingress.emergency().unwrap();
            SessionState::Emergency
        }
        "fatal" => {
            ingress.fail();
            SessionState::Failed
        }
        "quit" => {
            quit();
            SessionState::ClosingNormal
        }
        "restricted" => {
            quit();
            ingress.emergency().unwrap();
            SessionState::ClosingRestricted
        }
        "budget" => {
            f.home.clock.advance(Duration::from_millis(61_000));
            SessionState::Normal
        }
        other => panic!("unknown restriction {other}"),
    }
}
/// Nothing of the reply was accepted, and the attempt stayed recorded.
fn nothing_accepted(f: &SFx, m: &Mother, id: MissionId, case: &str) {
    assert_eq!(returned(f), 0, "{case}: no LaneReturned");
    assert_eq!(dispatched(f), 0, "{case}: no File dispatch");
    assert!(!index(f), "{case}: no File effect");
    assert!(
        !f.home.lines().concat().contains(REPLY) && !private(f).contains(REPLY),
        "{case}: no content at rest"
    );
    assert_eq!(
        native(f),
        "recorded",
        "{case}: the conclusive native record"
    );
    assert!(terminal(f, id), "{case}: the mission ends");
    assert!(m.state(id).unwrap().is_some_and(|s| s.is_terminal()));
}

/// Record-first classification (`adapter::classify`) under each restriction
/// landing while the admitted chat is held. `failed:` is produced only by
/// classify's recorded-error arm (a refusal is `refused:`, an unrecorded
/// attempt `record_unavailable`), so each case proves the sink ran with the
/// session already in the named state.
#[test]
fn sb3_restriction_before_the_record_is_classified() {
    children(
        "sb3_restriction_before_the_record_is_classified",
        &[
            "classify/emergency",
            "classify/fatal",
            "classify/quit",
            "classify/restricted",
            "classify/budget",
        ],
    );
}
fn classified(kind: &str) {
    let f = SFx::new();
    let (tx, rx) = channel();
    let m = f.mother(1, move |ctx| {
        tx.send(outcome(&ask(ctx))).unwrap();
        quiet()
    });
    let id = f.building(&m, f.contract());
    f.fake.route(|p| (p == "/api/chat").then_some(Answer::Hold));
    f.fake.drain();
    let adv = advance_in(&m, id);
    let held = f.fake.wait(Seen::Request("/api/chat".into()));
    assert!(held, "{kind}: the attempt is admitted and held");
    let state = restrict(&f, kind);
    assert_eq!(f.home.session.state(), state, "{kind}: before the reply");
    let closed = f.fake.wait(Seen::Closed("/api/chat".into()));
    assert!(closed, "{kind}: the held chat closes");
    let want = match kind {
        "budget" => "failed:model_timeout",
        _ => "failed:model_cancelled",
    };
    assert_eq!(within(&rx), want, "{kind}: classified as recorded failure");
    assert!(
        adv.join().unwrap().is_err(),
        "{kind}: the advance is refused"
    );
    assert_eq!(f.home.session.state(), state, "{kind}: still that state");
    nothing_accepted(&f, &m, id, kind);
}

/// The post-record recheck in `Caller::call`, restricted in the one window
/// between the record and the recheck (the `model_replied` hook, which fires
/// only for a recorded `Replied`). Without a restriction it releases the
/// reply; with one, `invalidated`, and only the recheck can produce that here.
#[test]
fn sb3_restriction_at_the_post_record_recheck() {
    children(
        "sb3_restriction_at_the_post_record_recheck",
        &[
            "recheck/none",
            "recheck/emergency",
            "recheck/fatal",
            "recheck/quit",
            "recheck/restricted",
            "recheck/budget",
        ],
    );
}
fn rechecked(kind: &str) {
    let f = SFx::new();
    let (tx, rx) = channel();
    let m = f.mother(1, move |ctx| {
        let r = ask(ctx);
        let content = r.as_ref().is_ok_and(|x| x.content().contains(REPLY));
        tx.send((outcome(&r), content)).unwrap();
        quiet()
    });
    let id = f.building(&m, f.contract());
    let (entered, release) = hold_chat(&f, REPLY);
    let (replied_tx, replied_rx) = signal();
    let (resume_tx, resume_rx) = signal();
    let resume_rx = Mutex::new(resume_rx);
    m.hooks_for_test().model_replied.arm(0, move || {
        let _ = replied_tx.send(());
        let _ = resume_rx.lock().unwrap().recv_timeout(PATIENCE);
    });
    let adv = advance_in(&m, id);
    within(&entered);
    release.send(()).unwrap();
    within(&replied_rx);
    let state = match kind {
        "none" => SessionState::Normal,
        restricted => restrict(&f, restricted),
    };
    assert_eq!(f.home.session.state(), state, "{kind}: at the recheck");
    resume_tx.send(()).unwrap();
    let (got, content) = within(&rx);
    if kind == "none" {
        assert_eq!((got.as_str(), content), ("ok", true), "the positive");
        adv.join().unwrap().unwrap();
        assert_eq!(returned(&f), 1, "the positive is admitted");
        return;
    }
    let seen = (got.as_str(), content);
    assert_eq!(seen, ("invalidated", false), "{kind}: the recheck refuses");
    assert!(
        adv.join().unwrap().is_err(),
        "{kind}: the advance is refused"
    );
    assert_eq!(f.home.session.state(), state, "{kind}: still that state");
    nothing_accepted(&f, &m, id, kind);
}

/// A restriction after the recheck released the reply, before the lane
/// returns: the lane held the reply and still nothing is accepted. Then a
/// restriction after the LaneReturned append: the admitted answer stands,
/// once, and no File is published.
#[test]
fn sb3_restriction_after_the_recheck_or_the_append() {
    children(
        "sb3_restriction_after_the_recheck_or_the_append",
        &["after/recheck", "after/append"],
    );
}
fn after_recheck(point: &str) {
    let f = SFx::new();
    let (tx, rx) = channel();
    let (go_tx, go_rx) = signal();
    let go_rx = Mutex::new(go_rx);
    let wait = point == "recheck";
    let m = f.mother(1, move |ctx| {
        let r = ask(ctx);
        let content = r.as_ref().is_ok_and(|x| x.content().contains(REPLY));
        tx.send((outcome(&r), content)).unwrap();
        if wait {
            let _ = go_rx.lock().unwrap().recv_timeout(PATIENCE);
        }
        draft(&r.map(ModelReply::into_content).unwrap_or_default())
    });
    let id = f.building(&m, f.contract());
    f.fake.route(|p| (p == "/api/chat").then(|| chat(REPLY)));
    if point == "append" {
        let ingress = f.home.session.ingress();
        f.probe.at("effect_dispatched", move || {
            ingress.emergency().unwrap();
        });
    }
    let adv = advance_in(&m, id);
    let seen = within(&rx);
    assert_eq!((seen.0.as_str(), seen.1), ("ok", true), "the lane has it");
    if point == "recheck" {
        assert_eq!(restrict(&f, "emergency"), f.home.session.state());
        go_tx.send(()).unwrap();
        assert!(adv.join().unwrap().is_err(), "the result is refused");
        assert_eq!(end(&f, id), "stop:emergency_stop");
        return nothing_accepted(&f, &m, id, "after the recheck");
    }
    let advanced = adv.join().unwrap();
    assert_eq!(f.home.session.state(), SessionState::Emergency);
    assert_eq!(returned(&f), 1, "the admitted answer stands once");
    assert!(advanced.is_err(), "the advance ends stopped");
    assert_eq!(m.state(id).unwrap(), Some(MissionState::Stopped));
    assert_eq!(dispatched(&f), 1, "the restriction lands at that dispatch");
    assert!(!index(&f), "no File is published");
}

// --------------------------------------------------------------- guard

/// Every D1 test body is exactly its child-runner call, so each schedule
/// (its joins and held peers included) runs in a bounded child process.
/// The broker-side watched-join guard scans only broker test files; this is
/// its Mother-side counterpart for this file (SB3D-R2).
#[test]
fn sb3_d1_schedules_run_in_child_processes() {
    let source = include_str!("session_model_bridge_d_tests.rs");
    // The OPEN tag is bound to its one listed cell: the table and its use.
    let cases = source.split("&[\n            \"first/").count() > 1;
    assert!(cases && KNOWN_DEFECT.len() == 1, "one known-defect cell");
    assert!(
        source.contains("\"first/budget-then-stop\",\n"),
        "the cell is executed"
    );
    assert_eq!(
        source.matches(concat!("OPEN:", "SB3C-F1")).count(),
        3,
        "tag bound to its cell"
    );
    assert_eq!(
        source.matches(concat!("OPEN:", "SB3C")).count(),
        3,
        "no other OPEN cell"
    );
    let tests: Vec<&str> = source.split("#[test]\nfn ").skip(1).collect();
    assert!(tests.len() >= 6, "the D1 tests are found");
    for test in tests {
        let name = test.split('(').next().unwrap();
        if name == "sb3_d1_schedules_run_in_child_processes" {
            continue;
        }
        let body = test.split_once("{\n").unwrap().1;
        let body = &body[..body.find("\n}\n").unwrap()];
        let runs = body.trim_start().starts_with("children(");
        let named = body.contains(&format!("\"{name}\""));
        assert!(runs && named, "{name}: not in a child process");
        assert_eq!(body.matches(';').count(), 1, "{name}: one statement");
    }
}
