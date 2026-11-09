//! SB-3 D2 Mother close, shutdown and death oracles (spec §9.4; SB3D-R5):
//! Mother H1, H2 (with H2-held), H5, H4, X1-D, X2 through the real C1 caller
//! → SessionPort → session broker → fake loopback peer. Each schedule runs
//! in a watched child (one 30 s deadline, kill and wait, parent-PID-bound
//! marker). H3, host X1 and production wiring stay M2e-D.
use crate::adapter::c1b_coordinator_oracles::outcome;
use crate::adapter::c1b_fake_ollama::{chat, Answer, Seen, PATIENCE};
use crate::adapter::c1b_port_oracles::record_next;
use crate::adapter::session_model_bridge_support::{advance_in, signal, within, Mother, SFx};
use crate::coordinator::{CoordinatorError, PreparedSessionEnd};
use crate::journal::Event;
use crate::lane::{LaneContext, LaneOutput, ModelCallError, Verdict};
use nexus_governed_control::authority::evidence::EndStatus;
use nexus_governed_control::host_session::close::{CloseObligations, CloseSeal, FinalStatus};
use nexus_governed_control::host_session::{CommandKind, SessionRequest, SessionState};
use nexus_governed_control::inference::{ModelReply, ModelRequest, ResponseSchema};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const CASE: &str = "NEXUS_SB3D2_CASE";
const PARENT: &str = "NEXUS_SB3D2_PARENT";
const DONE: &str = "SB3D2 CASE DONE";
const MODULE: &str = "adapter::session_model_bridge_d2_tests";
const REPLY: &str = "SB3D2_REPLY";

// ---------------------------------------------------------------- harness

static WATCHED: OnceLock<String> = OnceLock::new();
/// Every Mother join helper refuses to run outside a watched child.
fn assert_watched(helper: &str) {
    assert!(
        WATCHED.get().is_some(),
        "{helper}: only inside a watched child"
    );
}
struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
/// Runs each case in a fresh watched child: a bounded deadline, kill and
/// wait, and the child's success AND its body's marker (a skipped body
/// cannot pass).
fn watched(test: &str, cases: &[&str]) {
    if let Ok(case) = std::env::var(CASE) {
        let bound = std::env::var(PARENT)
            .is_ok_and(|p| p == std::os::unix::process::parent_id().to_string());
        assert!(bound, "{case}: the watched marker is not this child's");
        assert_eq!(WATCHED.get_or_init(|| case.clone()), &case, "one case");
        run(&case);
        println!("{DONE} {case}");
        return;
    }
    for case in cases {
        let (status, out) = finished(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", &format!("{MODULE}::{test}"), "--nocapture"])
                .env(CASE, case),
            None,
            case,
        );
        let done = out.contains(&format!("{DONE} {case}"));
        assert!(status.success() && done, "{case}: {status:?}\n{out}");
    }
}
fn run(case: &str) {
    assert_watched("run");
    let (name, arg) = case.split_once('/').unwrap_or((case, ""));
    match name {
        "clean" => clean(arg),
        "unjoined" => unjoined(arg),
        "open" => open_request(arg),
        "post-seal" => post_seal(arg),
        "deadline" => first_deadline(arg),
        "unknown" => unknown(arg),
        other => panic!("unknown case {other}"),
    }
}

fn ask(context: &LaneContext<'_>) -> Result<ModelReply, ModelCallError> {
    let schema = ResponseSchema::new(r#"{"type":"object"}"#).unwrap();
    let request = ModelRequest::new("SB3D2 system".into(), "SB3D2 prompt".into(), schema).unwrap();
    context
        .model
        .expect("a model lane gets a caller")
        .call(&request)
}
fn quiet() -> LaneOutput {
    LaneOutput {
        verification: None,
        text: "sb3d2 model lane".into(),
        effects: vec![],
        verdict: Verdict::Continue,
    }
}
fn signals() -> (SyncSender<()>, Mutex<Receiver<()>>) {
    let (tx, rx) = signal();
    (tx, Mutex::new(rx))
}
/// Runs `action` when dropped: a held peer or lane is released on any exit.
struct Finally<F: FnMut()>(F);
impl<F: FnMut()> Drop for Finally<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}
fn quit(f: &SFx) -> SessionRequest {
    f.home
        .session
        .ingress()
        .accept(CommandKind::Quit, 900)
        .unwrap()
}
fn obligations(f: &SFx) -> CloseObligations {
    f.home.session.close_obligations().unwrap()
}
fn session_ended(f: &SFx) -> usize {
    let closed = |e: &Event| matches!(e, Event::SessionClosed);
    f.events().iter().filter(|e| closed(e)).count()
}
/// The Mother end, the primary end and delivery, each distinct.
fn finals(f: &SFx) -> (FinalStatus, FinalStatus, FinalStatus) {
    let r = f.home.session.close_records();
    (r.mother, r.primary, r.delivery)
}
/// Exactly this Mother is dropped by value: `prepare_session_end` exists only
/// on an unshared session, so the advance threads must have been joined.
fn sole(m: Mother) -> crate::coordinator::FileSession<crate::adapter::SessionPort> {
    assert_watched("sole");
    Arc::try_unwrap(m)
        .ok()
        .expect("every advance thread joined")
}
/// The full A6 chain: prepare, settle, seal (`post_seal` observes it), Mother
/// end, its observation, primary end, delivery. Each final fact is observed
/// distinctly at its own step.
fn close_clean(
    f: &SFx,
    m: Mother,
    request: &SessionRequest,
    post_seal: impl FnOnce(&SFx, &CloseSeal),
) {
    use FinalStatus::{Pending, Recorded, Unknown};
    let session = &f.home.session;
    let ending: PreparedSessionEnd = sole(m).prepare_session_end().expect("prepared");
    let inventory = session.settle_close_resources(request).unwrap();
    assert!(inventory.entries.iter().all(|e| e.discharged), "discharged");
    session.settle_close_authority(request).unwrap();
    let seal = session.seal_close(request).expect("the clean seal");
    assert_eq!(session.state(), SessionState::SealedFinalizing);
    assert_eq!(finals(f), (Pending, Pending, Pending), "nothing yet");
    post_seal(f, &seal);
    ending.record(&seal).unwrap();
    assert_eq!(
        finals(f),
        (Unknown, Pending, Pending),
        "the Mother end begins"
    );
    session.observe_mother_end(&seal).unwrap();
    assert_eq!(
        finals(f),
        (Recorded, Pending, Pending),
        "Mother end observed"
    );
    session.close_primary(&seal).unwrap();
    assert_eq!(finals(f), (Recorded, Recorded, Pending), "primary end");
    assert!(seal.finish_delivery(), "delivered");
    assert_eq!(finals(f), (Recorded, Recorded, Recorded), "delivery");
    assert_eq!(session.state(), SessionState::ClosedClean);
    assert_eq!(session_ended(f), 1, "one Mother SessionClosed");
    let o = session.close_obligations().unwrap();
    assert_eq!((o.producers_live, o.producers_lost), (0, 0), "joined");
}

// ------------------------------------------------------------------- H1

/// H1: a no-model session and a completed recorded call pass the actual A6
/// close through SessionPort: conclusive model slot, one Mother end, the
/// existing final records and delivery.
#[test]
fn sb3_mother_completed_call_allows_clean_close() {
    watched(
        "sb3_mother_completed_call_allows_clean_close",
        &["clean/no-model", "clean/completed"],
    );
}
fn clean(arg: &str) {
    let completed = arg == "completed";
    let f = if completed {
        SFx::canonical()
    } else {
        SFx::unregistered()
    };
    let m = if completed {
        f.mother(1, |ctx| {
            assert_eq!(outcome(&ask(ctx)), "ok", "a recorded call");
            quiet()
        })
    } else {
        f.file_mother()
    };
    if completed {
        let id = f.building(&m, f.contract());
        m.advance(id).unwrap();
        assert_eq!(f.charges(), 1, "one charged call");
        assert_eq!(f.fake.count("/api/chat"), 1);
        assert_eq!(f.model_lane_returned(), 1);
        let o = obligations(&f).model;
        assert!(
            !o.active && o.owner_complete && o.child_complete && !o.uncertain,
            "{o:?}"
        );
        assert_eq!(o.record, "recorded");
    } else {
        f.awaiting(&m);
    }
    let request = quit(&f);
    close_clean(&f, m, &request, |_, _| {});
}

// ------------------------------------------------------------------- H2

/// H2: a held chat and a held charge keep the Mother's close Busy and
/// refuse the broker seal; owed and uncertain records refuse the close as
/// Recording, whatever the meter settled. A released, joined call closes
/// clean.
#[test]
fn sb3_mother_unjoined_or_owed_prevents_clean_close() {
    watched(
        "sb3_mother_unjoined_or_owed_prevents_clean_close",
        &[
            "unjoined/chat",
            "unjoined/charge",
            "unjoined/owed",
            "unjoined/uncertain",
        ],
    );
}
fn unjoined(arg: &str) {
    let f = SFx::canonical();
    let (tx, rx) = channel();
    let m = f.mother(1, move |ctx| {
        let r = ask(ctx);
        tx.send(outcome(&r)).unwrap();
        quiet()
    });
    let id = f.building(&m, f.contract());
    match arg {
        "owed" | "uncertain" => {
            let status = [EndStatus::Owed, EndStatus::Uncertain][usize::from(arg == "uncertain")];
            record_next(f.home.port.inner_for_test(), status);
            f.fake.route(|p| (p == "/api/chat").then(|| chat(REPLY)));
            let advanced = m.advance(id);
            assert_eq!(advanced.err(), Some(CoordinatorError::Recording), "{arg}");
            assert_eq!(within(&rx), "record_unavailable", "{arg}");
            assert_eq!(f.charges(), 1, "{arg}: the charge settled and stands");
            assert_eq!(m.close(), Err(CoordinatorError::Recording), "{arg}");
            let prepared = sole(m).prepare_session_end().err();
            assert_eq!(prepared, Some(CoordinatorError::Recording), "{arg}");
            assert_eq!(session_ended(&f), 0, "{arg}: no Mother end");
            assert_ne!(f.home.session.state(), SessionState::ClosedClean, "{arg}");
        }
        "chat" => {
            let (entered_tx, entered) = signal();
            let (release, release_rx) = signals();
            f.fake.route(move |p| {
                (p == "/api/chat").then(|| {
                    let _ = entered_tx.send(());
                    let _ = release_rx.lock().unwrap().recv_timeout(PATIENCE);
                    chat(REPLY)
                })
            });
            let adv = advance_in(&m, id);
            within(&entered);
            let o = obligations(&f).model;
            assert!(o.active && !o.owner_complete && !o.child_complete, "{o:?}");
            assert_eq!(o.record, "pending", "{o:?}");
            assert_eq!(f.fake.count("/api/chat"), 1, "one open connection");
            assert_eq!(m.close(), Err(CoordinatorError::Busy), "held chat");
            release.send(()).unwrap();
            assert_eq!(within(&rx), "ok", "the released reply is recorded");
            adv.join().unwrap().unwrap();
            let request = quit(&f);
            close_clean(&f, m, &request, |_, _| {});
        }
        "charge" => {
            let (at_tx, at) = signal();
            let (resume, resume_rx) = signals();
            m.hooks_for_test().model_charging.arm(0, move || {
                let _ = at_tx.send(());
                let _ = resume_rx.lock().unwrap().recv_timeout(PATIENCE);
            });
            let adv = advance_in(&m, id);
            within(&at); // The attempt is held at its charge, inside the broker.
            let released = resume.clone();
            let finally = Finally(move || {
                let _ = released.try_send(());
            });
            let o = obligations(&f).model;
            assert!(o.active && !o.owner_complete, "held charge: {o:?}");
            assert_eq!(m.close(), Err(CoordinatorError::Busy), "held charge");
            let request = quit(&f);
            let session = &f.home.session;
            session.settle_close_resources(&request).unwrap();
            session.settle_close_authority(&request).unwrap();
            assert!(session.seal_close(&request).is_err(), "unresolved attempt");
            assert_eq!(
                f.home
                    .lines()
                    .iter()
                    .filter(|l| l.contains("model_call_charged"))
                    .count(),
                0
            );
            drop(finally);
            let got = within(&rx);
            assert_ne!(got, "ok", "the released charge is too late: {got}");
            let _ = adv.join().unwrap();
            let o = obligations(&f).model;
            assert!(!o.active && o.owner_complete, "concluded after join: {o:?}");
            seal_refused_after_cancel(&f, m, &request, true);
        }
        other => panic!("unknown case {other}"),
    }
}

/// A Quit landed during this attempt: the known F1-b item (review-f1b-3) stands.
/// The in-flight lane's mission ended Stopped with the lane unreturned, so
/// history reports it unresolved and the seal refuses. Observed behaviour,
/// pinned here; no clean close and no Mother end are produced.
fn seal_refused_after_cancel(f: &SFx, m: Mother, request: &SessionRequest, settled: bool) {
    let session = &f.home.session;
    let _ending = sole(m).prepare_session_end().expect("prepared");
    if !settled {
        session.settle_close_resources(request).unwrap();
        session.settle_close_authority(request).unwrap();
    }
    assert!(session.seal_close(request).is_err(), "the seal refuses");
    let lines = f.home.lines();
    let history = crate::coordinator::inspect_history_lines(&lines).unwrap();
    assert_eq!(history.unresolved.len(), 1, "the lane never returned");
    assert_eq!(f.model_lane_returned(), 0, "nothing was accepted");
    assert_eq!(session_ended(f), 0, "no Mother end");
    assert_eq!(
        finals(f),
        (
            FinalStatus::Pending,
            FinalStatus::Pending,
            FinalStatus::Pending
        )
    );
    assert_ne!(session.state(), SessionState::ClosedClean);
}

// -------------------------------------------------------------- H2-held

/// H2-held: the peer holds metadata or chat open. While it does, the
/// connection stands, `close_obligations()` exposes the unresolved slot and
/// the Mother's close is Busy; Quit closes the request and it joins with a
/// recorded failure. The broker's seal-at-the-deadline with the request held
/// past Quit needs its private `model.call.polled` hook: B's H2-held evidence.
#[test]
fn sb3_mother_open_model_request_blocks_close() {
    watched(
        "sb3_mother_open_model_request_blocks_close",
        &["open/api/tags", "open/api/chat"],
    );
}
fn open_request(arg: &str) {
    let path = format!("/{arg}");
    let f = SFx::new();
    let (tx, rx) = channel();
    let m = f.mother(1, move |ctx| {
        tx.send(outcome(&ask(ctx))).unwrap();
        quiet()
    });
    let id = f.building(&m, f.contract());
    let held = path.clone();
    f.fake.route(move |p| (p == held).then_some(Answer::Hold));
    f.fake.drain();
    let base = f.fake.count(&path);
    let adv = advance_in(&m, id);
    assert!(f.fake.wait(Seen::Request(path.clone())), "{path}: held");
    assert_eq!(f.fake.count(&path), base + 1, "{path}: one connection");
    let o = obligations(&f).model;
    assert!(o.active && !o.owner_complete && !o.child_complete, "{o:?}");
    assert_eq!(o.record, "pending", "{path}");
    assert_eq!(m.close(), Err(CoordinatorError::Busy), "{path}: Busy");
    let _ = quit(&f);
    assert!(f.fake.wait(Seen::Closed(path.clone())), "{path}: it closes");
    let got = within(&rx);
    assert!(
        got.starts_with("failed:"),
        "{path}: recorded failure: {got}"
    );
    assert!(adv.join().unwrap().is_err(), "{path}: refused");
    let o = obligations(&f).model;
    assert!(
        !o.active && o.child_complete && o.record == "recorded",
        "{o:?}"
    );
    assert_eq!(f.fake.count(&path), base + 1, "{path}: no retry");
}

// ------------------------------------------------------------------- H5

/// H5: a reply released after Quit loses to the close (the seal then refuses:
/// the known F1-b item, pinned in `seal_refused_after_cancel`); once a clean
/// close seals, no metadata or chat request, no attempt, no new obligation,
/// and the Mother end, primary end and delivery stay distinct at each step.
#[test]
fn sb3_mother_post_seal_model_events_are_absent() {
    watched(
        "sb3_mother_post_seal_model_events_are_absent",
        &["post-seal/late", "post-seal/completed"],
    );
}
fn post_seal(arg: &str) {
    let f = SFx::canonical();
    let (tx, rx) = channel();
    let m = f.mother(1, move |ctx| {
        let r = ask(ctx);
        tx.send(outcome(&r)).unwrap();
        quiet()
    });
    let id = f.building(&m, f.contract());
    let (release, release_rx) = signals();
    let (entered_tx, entered) = signal();
    f.fake.route(move |p| {
        (p == "/api/chat").then(|| {
            let _ = entered_tx.send(());
            let _ = release_rx.lock().unwrap().recv_timeout(PATIENCE);
            chat(REPLY)
        })
    });
    let adv = advance_in(&m, id);
    within(&entered);
    let released = release.clone();
    let finally = Finally(move || {
        let _ = released.try_send(());
    });
    let request = if arg == "late" {
        let request = quit(&f);
        drop(finally); // The reply comes back after the Quit.
        let got = within(&rx);
        assert_ne!(got, "ok", "the late reply never reaches the lane: {got}");
        let _ = adv.join().unwrap();
        assert_eq!(f.fake.count("/api/chat"), 1, "one chat");
        return seal_refused_after_cancel(&f, m, &request, false);
    } else {
        drop(finally);
        assert_eq!(within(&rx), "ok");
        let advanced = adv.join().unwrap();
        assert!(advanced.is_ok(), "{advanced:?}");
        assert_eq!(
            f.model_lane_returned(),
            1,
            "the completed reply is accepted"
        );
        quit(&f)
    };
    close_clean(&f, m, &request, move |f, _| {
        let facts = |f: &SFx| {
            let chats = f.fake.count("/api/chat");
            let metadata = f.fake.count("/api/version") + f.fake.count("/api/tags");
            (
                chats,
                metadata,
                f.charges(),
                format!("{:?}", obligations(f).model),
            )
        };
        let before = facts(f);
        assert_eq!(before.0, 1, "one chat, no retry");
        // A later admission on the sealed door is refused and asks nothing.
        let refused =
            crate::effect::EffectPort::admit_inference(&*f.home.port, id, "h", "r", [0; 32]);
        assert!(refused.is_err(), "post-seal admission is refused");
        assert_eq!(
            facts(f),
            before,
            "post-seal metadata/chat/records/obligations"
        );
        assert_eq!(f.home.session.state(), SessionState::SealedFinalizing);
    });
}

// ------------------------------------------------------------------- H4

/// H4: the first close deadline stands across repeated Quit, emergency and
/// fatal triggers at each of Bootstrap, held metadata, held chat, held
/// charge, held record and a late result. The three-second product deadline
/// and the `<5 s` harness tolerance are the retained host controls.
#[test]
fn sb3_shutdown_keeps_first_deadline() {
    watched(
        "sb3_shutdown_keeps_first_deadline",
        &[
            "deadline/bootstrap",
            "deadline/metadata",
            "deadline/chat",
            "deadline/charge",
            "deadline/record",
            "deadline/late-result",
        ],
    );
}
/// The first Quit fixes the deadline; every later trigger leaves it.
fn repeat_triggers(
    f_session: &nexus_governed_control::host_session::HostSession,
    case: &str,
    base: u64,
) {
    let ingress = f_session.ingress();
    let before = Instant::now();
    let _quit = ingress
        .accept(CommandKind::Quit, base)
        .expect("the first Quit");
    let after = Instant::now();
    let first = ingress.close_deadline().expect("the first deadline");
    let product = Duration::from_secs(3);
    assert!(
        first >= before + product && first <= after + product,
        "{case}: the three-second budget from acceptance"
    );
    assert_eq!(
        _quit.deadline(),
        first,
        "{case}: the request's own deadline"
    );
    let again = ingress
        .accept(CommandKind::Quit, base + 1)
        .expect("repeated Quit");
    assert_eq!(again.deadline(), first, "{case}: repeated Quit");
    ingress.emergency().ok();
    ingress.fail();
    ingress.emergency().ok();
    let last = ingress.accept(CommandKind::Quit, base + 2);
    assert_eq!(ingress.close_deadline(), Some(first), "{case}: triggers");
    if let Ok(request) = last {
        assert_eq!(request.deadline(), first, "{case}");
    }
}
fn first_deadline(case: &str) {
    if case == "bootstrap" {
        return bootstrap_deadline();
    }
    let f = SFx::new();
    let (tx, rx) = channel();
    let (result_go, result_go_rx) = signals();
    let late_result = case == "late-result";
    let m = f.mother(1, move |ctx| {
        let r = ask(ctx);
        tx.send(outcome(&r)).unwrap();
        if late_result {
            let _ = result_go_rx.lock().unwrap().recv_timeout(PATIENCE);
        }
        quiet()
    });
    let id = f.building(&m, f.contract());
    let (peer_release, peer_release_rx) = signals();
    let (at_tx, at) = signal();
    let (resume, resume_rx) = signals();
    match case {
        "metadata" => f.fake.route(|p| (p == "/api/tags").then_some(Answer::Hold)),
        "chat" => f.fake.route(move |p| {
            (p == "/api/chat").then(|| {
                let _ = at_tx.send(());
                let _ = peer_release_rx.lock().unwrap().recv_timeout(PATIENCE);
                chat(REPLY)
            })
        }),
        "charge" => m.hooks_for_test().model_charging.arm(0, move || {
            let _ = at_tx.send(());
            let _ = resume_rx.lock().unwrap().recv_timeout(PATIENCE);
        }),
        "record" => {
            f.fake.route(|p| (p == "/api/chat").then(|| chat(REPLY)));
            m.hooks_for_test().model_replied.arm(0, move || {
                let _ = at_tx.send(());
                let _ = resume_rx.lock().unwrap().recv_timeout(PATIENCE);
            })
        }
        _ => f.fake.route(|p| (p == "/api/chat").then(|| chat(REPLY))),
    }
    f.fake.drain();
    let adv = advance_in(&m, id);
    match case {
        "metadata" => assert!(f.fake.wait(Seen::Request("/api/tags".into())), "held"),
        "late-result" => assert_eq!(within(&rx), "ok", "the lane has the reply"),
        _ => within(&at),
    }
    repeat_triggers(&f.home.session, case, 900);
    let released = (peer_release.clone(), resume.clone(), result_go.clone());
    let finally = Finally(move || {
        let _ = released.0.try_send(());
        let _ = released.1.try_send(());
        let _ = released.2.try_send(());
    });
    drop(finally);
    let _ = adv.join().unwrap();
    if case != "late-result" {
        let got = within(&rx);
        assert_ne!(
            got, "ok",
            "{case}: the restricted attempt is not accepted: {got}"
        );
    }
    let ingress = f.home.session.ingress();
    let first = ingress.close_deadline().unwrap();
    if let Ok(later) = ingress.accept(CommandKind::Quit, 950) {
        assert_eq!(later.deadline(), first, "{case}: after the join");
    }
    assert_eq!(
        ingress.close_deadline(),
        Some(first),
        "{case}: after the join"
    );
}
/// Direct session in Bootstrap: registration held at `/api/show`, the first
/// Quit and every later trigger keep one deadline.
fn bootstrap_deadline() {
    use crate::adapter::c1b_fake_ollama::{spec, FakeOllama};
    use crate::adapter::create_private;
    use crate::oracles::session_support::Owner;
    use nexus_governed_control::authority::clock::ManualClock;
    use nexus_governed_control::evidence_store::PrimaryEvidence;
    use nexus_governed_control::host_session::{HostSession, SessionSeed};
    assert!(nexus_governed_control::files::make_non_dumpable());
    let peer = Arc::new(FakeOllama::start());
    peer.route(|p| (p == "/api/show").then_some(Answer::Hold));
    let dir = std::env::temp_dir().join(format!("sb3d2-boot-{}", std::process::id()));
    create_private(&dir).unwrap();
    let _gone = Finally({
        let dir = dir.clone();
        move || {
            let _ = std::fs::remove_dir_all(&dir);
        }
    });
    let broker = dir.join("broker");
    let session = HostSession::open_with_history(
        SessionSeed::new().unwrap(),
        &broker,
        &dir,
        PrimaryEvidence::open(&broker.join("p3-evidence/evidence.sqlite3"), vec![]).unwrap(),
        Arc::new(ManualClock::default()),
        Arc::new(Owner::default()),
    )
    .unwrap();
    let workspace = dir.join("workspace");
    create_private(&workspace).unwrap();
    session.bootstrap_workspace(&workspace, &[]).unwrap();
    let endpoint = peer.endpoint();
    std::thread::scope(|s| {
        let session = &session;
        let peer_ref = &peer;
        let registering = s.spawn(move || session.register_local_model(&endpoint, spec()));
        assert!(peer_ref.wait(Seen::Request("/api/show".into())), "held");
        assert_eq!(session.state(), SessionState::Bootstrap);
        repeat_triggers(session, "bootstrap", 900);
        assert!(peer_ref.wait(Seen::Closed("/api/show".into())), "cancelled");
        assert!(registering.join().unwrap().is_err(), "registration refused");
    });
}

// --------------------------------------------------------------- X1-D, X2

const DEATH: &str = "NEXUS_SB3D2_DEATH";
const PEER: &str = "NEXUS_SB3D2_PEER";
const HOME: &str = "SB3D2_HOME=";
const REACHED: &str = "SB3D2_REACHED=";
const X1: &str = "sb3_mother_call_death_restarts_report_only";
const RESTART: &str = "NEXUS_SB3D2_RESTART";
const RESTARTED: &str = "SB3D2_RESTART_OK";

fn reach(point: &str) {
    use std::io::Write;
    println!("{REACHED}{point}");
    std::io::stdout().flush().unwrap();
}
/// Dies at the armed journal boundary (the `lite_crash` pattern): before the
/// charge line is written, after a durable line, and never by a mock port.
struct Dying {
    file: crate::adapter::FileJournal,
    point: String,
    armed: Arc<std::sync::atomic::AtomicBool>,
    /// The model lane's `LaneReturned` is the first after its charge.
    charged: bool,
}
impl crate::journal::JournalStore for Dying {
    fn load(&self) -> Result<Vec<String>, crate::journal::JournalError> {
        self.file.load()
    }
    fn append(&mut self, line: &str) -> Result<(), crate::journal::JournalError> {
        let charged = line.contains("\"event\":\"model_call_charged\"");
        let armed = self.armed.load(std::sync::atomic::Ordering::SeqCst);
        if armed && self.point == "charge-append" && charged {
            reach("charge-append");
            std::process::abort();
        }
        self.file.append(line)?;
        self.charged |= charged;
        let durable = match self.point.as_str() {
            "charge-durable" => charged,
            "accepted" => self.charged && line.contains("\"event\":\"lane_returned\""),
            "close-end" => line.contains("\"event\":\"session_closed\""),
            _ => false,
        };
        if armed && durable {
            reach(&self.point);
            std::process::abort();
        }
        Ok(())
    }
    fn poison(&mut self) -> Result<(), crate::journal::JournalError> {
        self.file.poison()
    }
}
/// The child: a real session and Mother on the parent's peer. It announces
/// its home, runs to its death point and aborts there (or the parent kills it
/// with the chat held). `clean-*` runs a complete clean close and exits 0,
/// leaving its home for the parent's restart.
fn die(point: &str) -> ! {
    let bound =
        std::env::var(PARENT).is_ok_and(|p| p == std::os::unix::process::parent_id().to_string());
    assert!(bound, "{point}: the watched marker is not this child's");
    WATCHED.get_or_init(|| point.to_string());
    use crate::adapter::c1b_coordinator_oracles::ModelLane;
    use crate::adapter::c1b_fake_ollama::{spec, FakeOllama};
    use crate::adapter::session_model_bridge_support::Probe;
    use crate::adapter::session_support::Fixture;
    use crate::coordinator::{Coordinator, Lanes};
    use crate::lane::{Lane, LaneRole};
    use crate::oracles::keyed_lanes;
    use std::io::Write;
    assert!(nexus_governed_control::files::make_non_dumpable());
    let endpoint = std::env::var(PEER).unwrap();
    let f = SFx {
        home: Fixture::with_model_canonical("sb3d2-death", &endpoint, spec()),
        fake: Arc::new(FakeOllama::start()),
        probe: Probe::default(),
        invoked: Arc::default(),
    };
    println!("{HOME}{}", f.home.dir.display());
    std::io::stdout().flush().unwrap();
    let armed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let store = Dying {
        file: f.home.journal(),
        point: point.to_string(),
        armed: armed.clone(),
        charged: false,
    };
    let mut lanes: Vec<Arc<dyn Lane>> = keyed_lanes()
        .into_iter()
        .filter(|l| l.identity().role != LaneRole::Builder)
        .collect();
    let (tx, rx) = channel();
    lanes.push(Arc::new(ModelLane {
        cap: 1,
        script: Box::new(move |ctx| {
            let r = ask(ctx);
            tx.send(outcome(&r)).unwrap();
            quiet()
        }),
        invoked: f.invoked.clone(),
    }));
    let m: Mother = Arc::new(
        Coordinator::open(
            store,
            f.home.port.clone(),
            Lanes::new(lanes).unwrap(),
            f.home.clock.clone(),
            f.home.private(),
        )
        .unwrap()
        .into_file_session()
        .unwrap(),
    );
    let id = f.building(&m, f.contract());
    if point == "charge-success" {
        armed.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = m.advance(id);
        panic!("the parent kills this child while its chat is held");
    }
    armed.store(
        point.starts_with("charge") || point == "accepted",
        std::sync::atomic::Ordering::SeqCst,
    );
    m.advance(id).expect("the completed call");
    assert_eq!(within(&rx), "ok");
    if point == "accepted" || point.starts_with("charge") {
        panic!("the journal boundary was not reached: {point}");
    }
    armed.store(point == "close-end", std::sync::atomic::Ordering::SeqCst);
    let request = quit(&f);
    let session = &f.home.session;
    let ending = sole(m).prepare_session_end().expect("prepared");
    session.settle_close_resources(&request).unwrap();
    session.settle_close_authority(&request).unwrap();
    let seal = session.seal_close(&request).expect("sealed");
    ending.record(&seal).expect("recorded");
    assert!(
        !point.starts_with("close"),
        "close-end is reached in record"
    );
    session.observe_mother_end(&seal).unwrap();
    session.close_primary(&seal).unwrap();
    assert!(seal.finish_delivery());
    assert_eq!(session.state(), SessionState::ClosedClean);
    reach(point);
    std::process::exit(0);
}
fn lines_of(home: &std::path::Path) -> Vec<String> {
    use crate::journal::JournalStore;
    crate::adapter::FileJournal::open(&home.join("mother-journal.jsonl"))
        .unwrap()
        .load()
        .unwrap()
}
fn count(lines: &[String], want: fn(&Event) -> bool) -> usize {
    crate::journal::verify(lines)
        .unwrap()
        .iter()
        .filter(|e| want(&e.event))
        .count()
}
fn charged(e: &Event) -> bool {
    matches!(e, Event::ModelCallCharged { .. })
}
/// Waits for a child under one 30 s deadline (kill and wait), killing it when
/// the watched peer sees its chat if `kill_at_chat`; returns its output.
fn finished(
    command: &mut Command,
    kill_at_chat: Option<&crate::adapter::c1b_fake_ollama::FakeOllama>,
    what: &str,
) -> (std::process::ExitStatus, String) {
    use std::io::Read;
    let mut child = Owned(
        command
            .env(PARENT, std::process::id().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let reader = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = pipe.read_to_string(&mut text);
            text
        })
    };
    let out = reader(Box::new(child.0.stdout.take().unwrap()));
    let err = reader(Box::new(child.0.stderr.take().unwrap()));
    if let Some(peer) = kill_at_chat {
        // The durable charge is at the peer; the child dies with the chat open.
        assert!(peer.wait(Seen::Request("/api/chat".into())), "{what}: held");
        child.0.kill().unwrap();
    }
    let until = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < until, "{what}: the child exceeded 30 s");
        std::thread::sleep(Duration::from_millis(5)); // Polls exit only.
    };
    (status, out.join().unwrap() + &err.join().unwrap())
}
/// Spawns the death child for `point` on the parent's peer; the parent proves
/// the point was reached and returns the announced home.
fn death(point: &str, peer: &crate::adapter::c1b_fake_ollama::FakeOllama) -> std::path::PathBuf {
    let (status, out) = finished(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &format!("{MODULE}::{X1}"), "--nocapture"])
            .env(DEATH, point)
            .env(PEER, peer.endpoint()),
        (point == "charge-success").then_some(peer),
        point,
    );
    let context = format!("{point}: {status:?}\n{out}");
    assert_eq!(status.success(), point.starts_with("clean-"), "{context}");
    if point != "charge-success" {
        assert!(out.contains(&format!("{REACHED}{point}")), "{context}");
    }
    let home = out
        .lines()
        .find_map(|l| l.strip_prefix(HOME))
        .expect("announced");
    std::path::PathBuf::from(home)
}
/// One report-only reopen of the dead session's real home: fresh identity,
/// no ready startup, no model operation, nothing re-issued to the peer.
fn reopen(home: &std::path::Path) -> nexus_governed_control::host_session::HostSession {
    use crate::oracles::session_support::Owner;
    use nexus_governed_control::authority::clock::SystemClock;
    use nexus_governed_control::evidence_store::PrimaryEvidence;
    use nexus_governed_control::host_session::{HostSession, SessionSeed};
    let broker = home.join("broker");
    let mut session = HostSession::open_with_history(
        SessionSeed::new().unwrap(),
        &broker,
        home,
        PrimaryEvidence::open(&broker.join("p3-evidence/evidence.sqlite3"), vec![]).unwrap(),
        Arc::new(SystemClock::default()),
        Arc::new(Owner::default()),
    )
    .unwrap();
    session
        .bind_history_inspector(Arc::new(crate::adapter::SessionHistoryInspector))
        .unwrap();
    session
}

/// X1-D: a real child dies at the charge append (before and after its durable
/// line), at charge success (parent kills it with the chat open), after the
/// accepted result and after the Mother end. Two restarts each stay
/// report-only: refused startup, the mission unresolved, no model operation,
/// metadata/chat 0/0 from the restart. Native run/consent/meter restoration is
/// X1-B's broker evidence; the startup refusal is why no Mother caller exists.
#[test]
fn sb3_mother_call_death_restarts_report_only() {
    if let Ok(point) = std::env::var(DEATH) {
        die(&point);
    }
    if let Ok(spec) = std::env::var(RESTART) {
        return restart_body(&spec);
    }
    let want_chat = |point: &str| match point {
        "charge-append" | "charge-durable" => 0,
        _ => 1,
    };
    for point in [
        "charge-append",
        "charge-durable",
        "charge-success",
        "accepted",
        "close-end",
    ] {
        let peer = crate::adapter::c1b_fake_ollama::FakeOllama::start();
        match point {
            "charge-success" => peer.route(|p| (p == "/api/chat").then_some(Answer::Hold)),
            _ => peer.route(|p| (p == "/api/chat").then(|| chat(REPLY))),
        }
        let home = death(point, &peer);
        let _gone = Finally({
            let home = home.clone();
            move || {
                let _ = std::fs::remove_dir_all(&home);
            }
        });
        let chats = want_chat(point);
        assert_eq!(peer.count("/api/chat"), chats, "{point}: chat at death");
        for restart in 0..2 {
            let before = metadata(&peer);
            restart_child(point, &home);
            let got = (metadata(&peer), peer.count("/api/chat"));
            assert_eq!(got, (before, chats), "{point} {restart}: metadata/chat 0/0");
        }
    }
}
fn metadata(peer: &crate::adapter::c1b_fake_ollama::FakeOllama) -> usize {
    let at = ["/api/version", "/api/status", "/api/tags", "/api/show"];
    at.iter().map(|p| peer.count(p)).sum()
}
/// One report-only restart, in its own process: a real restart is a new
/// process, and the broker's retained native storage is process-owned.
fn restart_child(point: &str, home: &std::path::Path) {
    let args = ["--exact", &format!("{MODULE}::{X1}"), "--nocapture"];
    let (status, out) = finished(
        Command::new(std::env::current_exe().unwrap())
            .args(args)
            .env(RESTART, format!("{point}|{}", home.display())),
        None,
        point,
    );
    assert!(
        status.success() && out.contains(RESTARTED),
        "{point}: {status:?}\n{out}"
    );
}
fn restart_body(spec: &str) {
    let (point, home) = spec.split_once('|').unwrap();
    let home = std::path::Path::new(home);
    let session = reopen(home);
    assert!(!session.startup_report().ready(), "{point}: report-only");
    assert!(
        session.finish_bootstrap().is_err(),
        "{point}: no authorized startup"
    );
    let o = session.close_obligations().unwrap().model;
    assert!(
        o.operation == "none" && !o.active,
        "{point}: no model operation: {o:?}"
    );
    let lines = lines_of(home);
    let history = crate::coordinator::inspect_history_lines(&lines).unwrap();
    let charges = usize::from(point != "charge-append");
    assert_eq!(count(&lines, charged), charges, "{point}: the charge");
    let closed = point == "close-end";
    assert_eq!(history.session_end_recorded, closed, "{point}: the end");
    if !closed {
        assert!(!history.unresolved.is_empty(), "{point}: unresolved");
    }
    println!("{RESTARTED}");
}

/// X2: a missing call outcome stays unknown; only established facts remain,
/// no success is inferred. The literal `unknown` is X1-B's broker surface
/// (`historical_outcome` has no public surface, forecast Q4). A clean
/// historical Home allows only a separately fresh, authorized startup.
#[test]
fn sb3_missing_call_outcome_remains_unknown() {
    watched(
        "sb3_missing_call_outcome_remains_unknown",
        &[
            "unknown/charge",
            "unknown/clean-none",
            "unknown/clean-registered",
        ],
    );
}
fn unknown(arg: &str) {
    use crate::adapter::SessionPort;
    use crate::effect::EffectPort;
    let peer = crate::adapter::c1b_fake_ollama::FakeOllama::start();
    peer.route(|p| (p == "/api/chat").then(|| chat(REPLY)));
    let child = if arg == "charge" {
        "charge-durable"
    } else {
        "clean-home"
    };
    let home = death(child, &peer);
    let _gone = Finally({
        let home = home.clone();
        move || {
            let _ = std::fs::remove_dir_all(&home);
        }
    });
    if arg == "charge" {
        let lines = lines_of(&home);
        let history = crate::coordinator::inspect_history_lines(&lines).unwrap();
        assert_eq!(count(&lines, charged), 1, "the established charge fact");
        assert!(!history.session_end_recorded, "no end fact is invented");
        assert_eq!(history.unresolved.len(), 1, "the mission stays unresolved");
        let terminal = |e: &Event| {
            matches!(e, Event::Transition { trigger } if matches!(trigger,
            crate::state::Trigger::Stop { .. } | crate::state::Trigger::Block { .. }
            | crate::state::Trigger::Fail { .. }))
        };
        assert_eq!(count(&lines, terminal), 0, "no end is inferred");
        let session = reopen(&home);
        assert!(!session.startup_report().ready(), "report-only");
        let o = session.close_obligations().unwrap().model;
        assert_eq!(
            (o.operation, o.record),
            ("none", "not_required"),
            "no success: {o:?}"
        );
        return;
    }
    // A clean historical Home: a fresh startup is permitted, and it starts
    // with no model until a new Bootstrap registers one.
    let session = reopen(&home);
    assert!(session.startup_report().ready(), "clean history is ready");
    let workspace = home.join("workspace");
    let info = session.bootstrap_workspace(&workspace, &[]).unwrap();
    if arg == "clean-registered" {
        session
            .register_local_model(&peer.endpoint(), crate::adapter::c1b_fake_ollama::spec())
            .expect("a new Bootstrap registers");
    }
    session.finish_bootstrap().unwrap();
    let baseline = (peer.count("/api/version"), peer.count("/api/chat"));
    let request = session.ingress().accept(CommandKind::Run, 2).unwrap();
    let port = SessionPort::new(session.open_file_door(&request).unwrap(), info.name);
    let identity = port.model_identity();
    if arg == "clean-registered" {
        assert!(identity.is_some(), "the new registration is visible");
    } else {
        assert!(
            identity.is_none(),
            "no model until a new Bootstrap registers"
        );
        let late =
            session.register_local_model(&peer.endpoint(), crate::adapter::c1b_fake_ollama::spec());
        assert!(late.is_err(), "registration is Bootstrap only");
    }
    assert_eq!(
        baseline,
        (peer.count("/api/version"), peer.count("/api/chat")),
        "no request"
    );
}
