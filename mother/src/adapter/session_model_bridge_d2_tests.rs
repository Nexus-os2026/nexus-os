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
use crate::evidence::BrokerClass;
use crate::journal::Event;
use crate::lane::{LaneContext, LaneOutput, ModelCallError, Verdict};
use crate::state::MissionState;
use nexus_governed_control::authority::evidence::EndStatus;
use nexus_governed_control::authority::ids::RunId;
use nexus_governed_control::authority::AuthorityError;
use nexus_governed_control::host_session::close::{CloseObligations, CloseSeal, FinalStatus};
use nexus_governed_control::host_session::HostSession;
use nexus_governed_control::host_session::{CommandKind, SessionRequest, SessionState};
use nexus_governed_control::inference::{
    CallDeadline, CallMeter, Charged, InvocationOwner, MeterRefusal, ModelReply, ModelRequest,
    ResponseSchema,
};
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

fn request() -> ModelRequest {
    let schema = ResponseSchema::new(r#"{"type":"object"}"#).unwrap();
    ModelRequest::new("SB3D2 system".into(), "SB3D2 prompt".into(), schema).unwrap()
}
fn ask(context: &LaneContext<'_>) -> Result<ModelReply, ModelCallError> {
    context
        .model
        .expect("a model lane gets a caller")
        .call(&request())
}
fn quiet() -> LaneOutput {
    LaneOutput {
        verification: None,
        text: "sb3d2 model lane".into(),
        effects: vec![],
        verdict: Verdict::Continue,
    }
}
/// An acknowledged hold: the held side reports arrival, and fails if it is not
/// released in time (a dropped `Hold` included).
struct Hold {
    at: Receiver<()>,
    go: SyncSender<()>,
}
impl Hold {
    fn reached(&self) {
        within(&self.at);
    }
    fn release(&self) {
        self.go.send(()).unwrap();
    }
}
fn hold() -> (Hold, impl FnOnce() + Send + 'static) {
    let (arrived, at) = signal();
    let (go, released) = signal();
    let held = move || {
        arrived.send(()).unwrap();
        released.recv_timeout(PATIENCE).expect("released in time");
    };
    (Hold { at, go }, held)
}
/// `/api/chat` held at the peer until released, then answered.
fn held_chat(f: &SFx) -> Hold {
    let (held, arrive) = hold();
    let arrive = Mutex::new(Some(arrive));
    f.fake.route(move |p| {
        (p == "/api/chat").then(|| {
            (arrive.lock().unwrap().take().expect("one chat"))();
            chat(REPLY)
        })
    });
    held
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
    let events = f.events();
    events
        .iter()
        .filter(|e| matches!(e, Event::SessionClosed))
        .count()
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
    use FinalStatus::{Pending as P, Recorded as R, Unknown as U};
    let at = |want, step: &str| assert_eq!(finals(f), want, "{step}");
    let session = &f.home.session;
    let ending: PreparedSessionEnd = sole(m).prepare_session_end().expect("prepared");
    let inventory = session.settle_close_resources(request).unwrap();
    assert!(inventory.entries.iter().all(|e| e.discharged), "discharged");
    session.settle_close_authority(request).unwrap();
    let seal = session.seal_close(request).expect("the clean seal");
    assert_eq!(session.state(), SessionState::SealedFinalizing);
    at((P, P, P), "nothing yet");
    post_seal(f, &seal);
    ending.record(&seal).unwrap();
    at((U, P, P), "the Mother end begins");
    session.observe_mother_end(&seal).unwrap();
    at((R, P, P), "Mother end observed");
    session.close_primary(&seal).unwrap();
    at((R, R, P), "primary end");
    assert!(seal.finish_delivery(), "delivered");
    at((R, R, R), "delivery");
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
    let f = [SFx::unregistered, SFx::canonical][usize::from(completed)]();
    if completed {
        let m = f.mother(1, |ctx| {
            assert_eq!(outcome(&ask(ctx)), "ok", "a recorded call");
            quiet()
        });
        let id = f.building(&m, f.contract());
        assert_eq!(f.home.session.state(), SessionState::Normal);
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
        let request = quit(&f);
        return close_clean(&f, m, &request, |_, _| {});
    }
    let m = f.file_mother();
    f.awaiting(&m);
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
            "unjoined/chat-seal",
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
            let held = held_chat(&f);
            let adv = advance_in(&m, id);
            held.reached();
            let o = obligations(&f).model;
            assert!(o.active && !o.owner_complete && !o.child_complete, "{o:?}");
            assert_eq!(o.record, "pending", "{o:?}");
            assert_eq!(f.fake.count("/api/chat"), 1, "one open connection");
            assert_eq!(m.close(), Err(CoordinatorError::Busy), "held chat");
            held.release();
            assert_eq!(within(&rx), "ok", "the released reply is recorded");
            adv.join().unwrap().unwrap();
            let request = quit(&f);
            close_clean(&f, m, &request, |_, _| {});
        }
        "chat-seal" | "charge" => {
            let (held, arrive) = hold();
            let chat_held = arg == "chat-seal";
            let adv = if chat_held {
                f.fake.route(|p| (p == "/api/chat").then_some(Answer::Hold));
                f.fake.drain();
                let adv = advance_in(&m, id);
                assert!(f.fake.wait(Seen::Request("/api/chat".into())), "held");
                // Frozen in the broker's poll, the attempt cannot see the Quit.
                m.hooks_for_test().model_deadline_sampled.arm(0, arrive);
                adv
            } else {
                m.hooks_for_test().model_charging.arm(0, arrive);
                advance_in(&m, id)
            };
            held.reached();
            let o = obligations(&f).model;
            assert!(o.active && !o.owner_complete, "{arg}: {o:?}");
            assert_eq!(m.close(), Err(CoordinatorError::Busy), "{arg}");
            let request = quit(&f);
            let session = &f.home.session;
            session.settle_close_resources(&request).unwrap();
            session.settle_close_authority(&request).unwrap();
            assert!(session.seal_close(&request).is_err(), "{arg}: unresolved");
            assert!(obligations(&f).model.active, "{arg}: refused while held");
            assert_eq!(f.charges(), usize::from(chat_held), "{arg}: the charge");
            f.fake.drain(); // A close before this point fails the wait below.
            held.release();
            if chat_held {
                let closed = f.fake.wait(Seen::Closed("/api/chat".into()));
                assert!(closed, "the chat closes only after the refused seal");
            }
            let got = within(&rx);
            assert_ne!(got, "ok", "{arg}: too late to be accepted: {got}");
            let advanced = adv.join().unwrap();
            assert!(advanced.is_err(), "{arg}: {advanced:?}");
            let o = obligations(&f).model;
            assert!(!o.active && o.owner_complete, "after the join: {o:?}");
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
    let pending = FinalStatus::Pending;
    assert_eq!(finals(f), (pending, pending, pending));
    assert_ne!(session.state(), SessionState::ClosedClean);
}

// -------------------------------------------------------------- H2-held

/// H2-held: an open metadata or chat request is an unresolved slot and Busy;
/// Quit closes it and it joins with a recorded failure. The connection held
/// to the deadline stays B's evidence (its `model.call.polled` hook is private).
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
    assert_eq!(
        f.home.session.state(),
        SessionState::ClosingNormal,
        "{path}"
    );
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

/// H5: a reply released after Quit has no successful seal (F1-b item, pinned in
/// `seal_refused_after_cancel`). After sealing, retained calls return Halted with no callbacks;
/// request counts, native-store model_call occurrences and model-obligation snapshots are unchanged.
#[test]
fn sb3_mother_post_seal_model_events_are_absent() {
    watched(
        "sb3_mother_post_seal_model_events_are_absent",
        &[
            "post-seal/late",
            "post-seal/completed",
            "post-seal/file",
            "post-seal/refused-file",
            "post-seal/staged-file",
        ],
    );
}
/// R6 (e), R7: a pending page closes clean; a File refused before anything was
/// staged closes clean with its refusal still journaled; a File refused after
/// staging stays unacknowledged and the seal refuses.
fn file_at_quit(arg: &str) {
    use crate::adapter::c1b_coordinator_oracles::draft;
    use crate::adapter::m3d_publication_oracles::BAKERY;
    let f = SFx::canonical();
    let page = arg == "file";
    let m = f.mother(1, move |ctx| {
        assert_eq!(outcome(&ask(ctx)), "ok");
        draft(if page { BAKERY.trim_end() } else { "not inert" })
    });
    let mut c = f.contract();
    c.page_facts = page.then(|| crate::page_inputs::PageFacts {
        name: "Rosa's Bakery".into(),
        hours: vec![
            "Tuesday to Sunday, 7am to 3pm".into(),
            "Closed on Mondays".into(),
        ],
        phone: "555-0142".into(),
    });
    if page {
        c.pass_criteria = crate::page_inputs::page_criteria();
        c.notes = vec![crate::contract::Note {
            label: "notes".into(),
            text: "Fresh bread.".into(),
        }];
    }
    let id = f.building(&m, c);
    let existing = f.home.workspace.join("index.html");
    let exists = arg == "refused-file";
    if exists {
        std::fs::write(&existing, "already here\n").unwrap();
    }
    f.fake.route(|p| (p == "/api/chat").then(|| chat(REPLY)));
    m.advance(id).expect("the advance");
    let want = [MissionState::Blocked, MissionState::AwaitingApproval][usize::from(page)];
    assert_eq!(m.state(id).unwrap(), Some(want));
    let request = quit(&f);
    if page {
        return close_clean(&f, m, &request, |_, _| {});
    }
    if exists {
        close_clean(&f, m, &request, |_, _| {});
        let lines = f.home.lines();
        let refusal = lines.iter().filter(|line| {
            line.contains("effect_settled") && line.contains("refused:file_exists")
        });
        assert_eq!(refusal.count(), 1, "the refusal stays in the record");
        let kept = std::fs::read_to_string(&existing).unwrap();
        return assert_eq!(kept, "already here\n", "nothing was written");
    }
    let staged = f.home.lines();
    let staged = staged.iter().filter(|line| {
        line.contains("effect_settled") && line.contains("refused:page_not_inert")
    });
    assert_eq!(staged.count(), 1, "refused only after it was staged");
    let _ending = sole(m).prepare_session_end().expect("prepared");
    let s = &f.home.session;
    s.settle_close_resources(&request).unwrap();
    s.settle_close_authority(&request).unwrap();
    let o = s.close_obligations().unwrap();
    assert_eq!(
        (o.producers_live, o.producers_lost, o.actions_unacknowledged),
        (0, 0, 1)
    );
    assert!(
        !o.model.active && !o.model.uncertain && o.authority_closed,
        "{o:?}"
    );
    let (stop, held) = (
        s.stop_obligations().unwrap(),
        s.resource_obligations().unwrap(),
    );
    assert_eq!(
        (stop.received, held.unresolved, held.uncertain),
        (stop.recorded, 0, 0)
    );
    assert!(s.inventory().unwrap().entries.iter().all(|e| e.discharged));
    assert!(s.seal_close(&request).is_err(), "the action branch refuses");
}
fn post_seal(arg: &str) {
    if arg.ends_with("file") {
        return file_at_quit(arg);
    }
    let f = SFx::canonical();
    let (tx, rx) = channel();
    let m = f.mother(1, move |ctx| {
        tx.send(outcome(&ask(ctx))).unwrap();
        quiet()
    });
    let id = f.building(&m, f.contract());
    if arg == "late" {
        let held = held_chat(&f);
        let adv = advance_in(&m, id);
        held.reached();
        let request = quit(&f);
        held.release(); // The reply comes back after the Quit.
        let got = within(&rx);
        assert_ne!(got, "ok", "the late reply never reaches the lane: {got}");
        assert!(adv.join().unwrap().is_err(), "the advance is refused");
        assert_eq!(f.fake.count("/api/chat"), 1, "one chat");
        return seal_refused_after_cancel(&f, m, &request, false);
    }
    f.fake.route(|p| (p == "/api/chat").then(|| chat(REPLY)));
    m.advance(id).expect("the completed call");
    assert_eq!(within(&rx), "ok");
    assert_eq!(f.model_lane_returned(), 1, "the reply is accepted");
    let run = f.home.port.inner_for_test().model_run(id).unwrap();
    let request = quit(&f);
    close_clean(&f, m, &request, move |f, _| late_attempt(f, id, run));
    let calls = f
        .home
        .port
        .inner_for_test()
        .model_observer
        .lock()
        .unwrap()
        .len();
    assert_eq!(calls, 2, "the lane's call and the late attempt: it ran");
}
/// Counts every meter and deadline callback a late attempt makes.
#[derive(Default)]
struct Late(std::sync::atomic::AtomicUsize);
impl CallMeter for Late {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(MeterRefusal::Stale)
    }
    fn settle(&self, _: u32) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}
impl CallDeadline for Late {
    fn expired(&self) -> bool {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        false
    }
}
/// `model_call` substring occurrences across the native store files: a limited
/// sentinel, not a count of logical records.
fn native_records(home: &std::path::Path) -> usize {
    std::fs::read_dir(home.join("broker/p3-evidence"))
        .unwrap()
        .map(|e| std::fs::read(e.unwrap().path()).unwrap())
        .map(|b| b.windows(10).filter(|w| w == b"model_call").count())
        .sum()
}
fn late_attempt(f: &SFx, id: crate::journal::MissionId, run: RunId) {
    let facts = |f: &SFx| {
        let chats = f.fake.count("/api/chat");
        let native = native_records(&f.home.dir);
        let model = format!("{:?}", obligations(f).model);
        (chats, metadata(&f.fake), native, model)
    };
    let before = facts(f);
    assert_eq!(before.0, 1, "one chat, no retry");
    assert!(before.2 > 0, "the model_call occurrence count is nonzero");
    let (late, owner, body) = (Late::default(), InvocationOwner::new(), request());
    let invocation = owner.observer();
    let port = f.home.port.inner_for_test();
    let seen = port.model_observer.lock().unwrap().len();
    let attempt =
        crate::effect::EffectPort::model_call(&*f.home.port, id, &body, &late, &late, &invocation);
    let entered = port.model_observer.lock().unwrap().len();
    assert_eq!(entered, seen + 1, "the late attempt entered the port");
    assert_eq!(attempt.err().map(|r| r.class), Some(BrokerClass::Halted));
    let door = port.door.model_call(run, &body, &late, &late, &invocation);
    assert!(
        matches!(door, Err(AuthorityError::Halted)),
        "the sealed broker door refuses"
    );
    assert_eq!(
        late.0.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "no callback"
    );
    assert_eq!(facts(f), before, "post-seal counts and obligations");
    assert_eq!(f.home.session.state(), SessionState::SealedFinalizing);
}

// ------------------------------------------------------------------- H4

/// H4: the first deadline stands across repeated Quit, emergency and fatal at
/// Bootstrap, held metadata, chat, charge, record and a held late result.
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
/// The first Quit fixes the deadline; every later trigger, each required to
/// take effect, leaves it.
fn repeat_triggers(session: &HostSession, case: &str, base: u64) {
    let ingress = session.ingress();
    let closing = |want: SessionState| assert_eq!(session.state(), want, "{case}");
    let before = Instant::now();
    let quit = ingress.accept(CommandKind::Quit, base).expect("first Quit");
    let after = Instant::now();
    let first = ingress.close_deadline().expect("the first deadline");
    let product = Duration::from_secs(3);
    let budget = first >= before + product && first <= after + product;
    assert!(budget, "{case}: three seconds from acceptance");
    assert_eq!(quit.deadline(), first, "{case}: the request's own");
    closing(SessionState::ClosingNormal);
    let again = ingress.accept(CommandKind::Quit, base + 1).expect("again");
    assert_eq!(again.deadline(), first, "{case}: repeated Quit");
    closing(SessionState::ClosingNormal);
    assert!(!ingress.emergency_seen(), "{case}: no emergency yet");
    ingress.emergency().expect("the emergency");
    assert!(ingress.emergency_seen(), "{case}: the emergency restricts");
    closing(SessionState::ClosingRestricted);
    assert!(
        !ingress.failed(),
        "{case}: not failed before the fatal trigger"
    );
    ingress.fail();
    assert!(ingress.failed(), "{case}: the fatal trigger takes effect");
    closing(SessionState::ClosingRestricted);
    ingress.emergency().expect("the second emergency");
    let last = ingress
        .accept(CommandKind::Quit, base + 2)
        .expect("last Quit");
    assert_eq!(last.deadline(), first, "{case}: the final Quit");
    assert_eq!(ingress.close_deadline(), Some(first), "{case}: triggers");
}
fn first_deadline(case: &str) {
    if case == "bootstrap" {
        return bootstrap_deadline();
    }
    let f = SFx::new();
    let (tx, rx) = channel();
    let (held, arrive) = hold();
    let arrive = Arc::new(Mutex::new(Some(arrive)));
    let fire = move || (arrive.lock().unwrap().take().expect("one hold"))();
    let lane_fire = fire.clone();
    let late = case == "late-result";
    let m = f.mother(1, move |ctx| {
        tx.send(outcome(&ask(ctx))).unwrap();
        if late {
            lane_fire();
        }
        quiet()
    });
    let id = f.building(&m, f.contract());
    let at = match case {
        "metadata" => Some("/api/tags"),
        "chat" => Some("/api/chat"),
        _ => None,
    };
    let hooks = m.hooks_for_test();
    match case {
        "metadata" | "chat" => f
            .fake
            .route(move |p| (Some(p) == at).then_some(Answer::Hold)),
        "charge" => hooks.model_charging.arm(0, fire),
        "record" => {
            f.fake.route(|p| (p == "/api/chat").then(|| chat(REPLY)));
            hooks.model_replied.arm(0, fire);
        }
        _ => f.fake.route(|p| (p == "/api/chat").then(|| chat(REPLY))),
    }
    f.fake.drain();
    let adv = advance_in(&m, id);
    match at {
        Some(path) => assert!(f.fake.wait(Seen::Request(path.into())), "{case}: held"),
        None => held.reached(),
    }
    repeat_triggers(&f.home.session, case, 900);
    held.release();
    let advanced = adv.join().unwrap();
    assert_eq!(advanced.err(), Some(CoordinatorError::Stopped), "{case}");
    let got = within(&rx);
    assert_eq!(
        got == "ok",
        late,
        "{case}: only the held result is the reply: {got}"
    );
    let ingress = f.home.session.ingress();
    let first = ingress.close_deadline().unwrap();
    let later = ingress
        .accept(CommandKind::Quit, 950)
        .expect("after the join");
    assert_eq!(later.deadline(), first, "{case}: after the join");
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
/// Dies at the armed journal boundary, before or after the durable line.
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
/// The child: a real session and Mother on the parent's peer. It dies at its
/// point (or is killed with the chat held); `clean-*` closes clean and exits 0.
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
/// Waits for a child under one 30 s deadline; returns its output.
fn finished(
    command: &mut Command,
    kill_at_chat: Option<&crate::adapter::c1b_fake_ollama::FakeOllama>,
    what: &str,
) -> (std::process::ExitStatus, String) {
    use std::io::Read;
    let until = Instant::now() + Duration::from_secs(30); // Before the spawn.
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

/// X1-D: a real child dies at the charge append (either side of its durable
/// line), at charge success, after the accepted result and after the Mother
/// end. Two restarts stay report-only; native restoration is X1-B's evidence.
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
/// One report-only restart, in its own process: the kernel Stop controller is
/// claimed once per process, so a second front door fails closed by design.
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
    let bound =
        std::env::var(PARENT).is_ok_and(|p| p == std::os::unix::process::parent_id().to_string());
    assert!(bound, "{point}: the watched marker is not this child's");
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

/// X2: a missing outcome infers no success (literal `unknown` is X1-B's, Q4);
/// a clean Home allows only a fresh, authorized startup.
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
