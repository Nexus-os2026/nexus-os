//! SB-3 C1 composition oracles (spec §9.1–9.3, SB3C-R1/R2): the real C1
//! caller → SessionPort → session broker → fake loopback peer. Each case
//! runs in its own child process (one broker per process). C1, C3, O4 and
//! L2's chat half are C2's composition evidence (SB3C-R2).
use crate::adapter::c1b_coordinator_oracles::{ask, draft, outcome, request, REPLY};
use crate::adapter::c1b_fake_ollama::{chat, spec};
use crate::adapter::c1b_port_oracles::children as run_children;
use crate::adapter::session_model_bridge_support::{advance_in, signal, within, SFx};
use crate::effect::EffectPort;
use crate::evidence::BrokerClass;
use crate::lane::{LaneOutput, Verdict};
use crate::state::MissionState;
use nexus_governed_control::host_session::CommandKind;
use nexus_governed_control::inference::{
    CallDeadline, CallMeter, Charged, InvocationOwner, MeterRefusal, ModelIdentity,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

const CASE: &str = "NEXUS_SB3C_CASE";
const MODULE: &str = "adapter::session_model_bridge_tests";
const REGISTRATION: [&str; 4] = ["/api/version", "/api/status", "/api/tags", "/api/show"];

fn children(test: &str, cases: &[&str]) {
    run_children(MODULE, CASE, test, cases, run);
}
fn run(case: &str) {
    let (name, arg) = case.split_once('/').unwrap_or((case, ""));
    match name {
        "s1" => s1(),
        "s2" => s2(arg),
        "r4" => r4(arg),
        "l2" => l2(arg),
        other => panic!("unknown case {other}"),
    }
}

#[derive(Default)]
struct Meter(AtomicU32);
impl CallMeter for Meter {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        Ok(Charged {
            call: self.0.fetch_add(1, Ordering::SeqCst) + 1,
        })
    }
    fn settle(&self, _: u32) {}
}
struct Never;
impl CallDeadline for Never {
    fn expired(&self) -> bool {
        false
    }
}
fn identity_bytes() -> [u8; 32] {
    ModelIdentity::from_spec(&spec()).unwrap().digest()
}
fn quiet() -> LaneOutput {
    LaneOutput {
        verification: None,
        text: "sb3c model lane".into(),
        effects: vec![],
        verdict: Verdict::Continue,
    }
}
/// The lane's outcomes, in order.
type Seen = Arc<Mutex<Vec<String>>>;

// ------------------------------------------------------------------- S1

/// S1: one recorded call through SessionPort: registration's four requests,
/// then version/status/tags/chat; one charge; the exact reply reaches the
/// lane; the original File run only.
#[test]
fn sb3_registered_session_executes_one_recorded_call() {
    children("sb3_registered_session_executes_one_recorded_call", &["s1"]);
}
fn s1() {
    let f = SFx::new();
    for path in REGISTRATION {
        assert_eq!(f.fake.count(path), 1, "registration {path}");
    }
    f.fake.route(|p| (p == "/api/chat").then(|| chat(REPLY)));
    let seen: Seen = Arc::default();
    let s = seen.clone();
    let m = f.mother(1, move |ctx| {
        let r = ask(ctx);
        let exact = r
            .as_ref()
            .is_ok_and(|reply| reply.content().contains(REPLY));
        s.lock().unwrap().push(format!("{}:{exact}", outcome(&r)));
        draft("sb3c")
    });
    assert!(
        f.home.port.model_identity().is_some(),
        "the registered model"
    );
    let id = f.building(&m, f.contract());
    m.advance(id).unwrap();
    assert_eq!(
        *seen.lock().unwrap(),
        ["ok:true"],
        "the exact recorded reply"
    );
    for (path, n) in [
        ("/api/version", 2),
        ("/api/status", 2),
        ("/api/tags", 2),
        ("/api/show", 1),
        ("/api/chat", 1),
    ] {
        assert_eq!(f.fake.count(path), n, "{path}");
    }
    assert_eq!(f.charges(), 1, "one durable charge");
    assert_eq!(f.home.session.run_count(), 1, "the original File run only");
}

// ------------------------------------------------------------------- S2

/// S2: an unconfigured session keeps its default denial at SessionPort on
/// its existing File run; only absence reads as no broker route (a
/// restricted, registered session keeps its own class: MP-SB-C-absence).
#[test]
fn sb3_unconfigured_session_retains_default_denial() {
    children(
        "sb3_unconfigured_session_retains_default_denial",
        &["s2/unconfigured", "s2/restricted", "s2/debt"],
    );
}
fn s2(arg: &str) {
    if arg == "debt" {
        return s2_debt();
    }
    let f = if arg == "unconfigured" {
        SFx::unregistered()
    } else {
        SFx::new()
    };
    let m = f.file_mother();
    let id = f.awaiting(&m);
    if arg == "restricted" {
        f.home.session.ingress().emergency().unwrap();
    }
    let port = f.home.port.clone();
    let (meter, owner) = (Meter::default(), InvocationOwner::new());
    let admit = port
        .admit_inference(id, &"ab".repeat(32), "request", identity_bytes())
        .map_err(|r| r.class);
    let call = port
        .model_call(id, &request("SB3C"), &meter, &Never, &owner.observer())
        .map(|_| ())
        .map_err(|r| r.class);
    let want = match arg {
        "unconfigured" => {
            assert!(port.model_identity().is_none(), "no model identity");
            BrokerClass::NoBrokerRoute
        }
        _ => BrokerClass::Halted,
    };
    assert_eq!(admit, Err(want), "{arg}: admission");
    assert_eq!(call, Err(want), "{arg}: call");
    assert_eq!(meter.0.load(Ordering::SeqCst), 0, "{arg}: nothing charged");
    assert_eq!(f.fake.count("/api/chat"), 0, "{arg}: no chat");
    assert_eq!(f.home.session.run_count(), 1, "{arg}: no new run");
}

/// A meter whose charge unwinds: the broker's attempt is dropped unfinished.
struct Unwinding;
impl CallMeter for Unwinding {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        panic!("the meter unwinds (test)")
    }
    fn settle(&self, _: u32) {}
}
/// SB3C-R4 (5): real debt through SessionPort. Inside the live invocation a
/// bound run's attempt unwinds in its meter (the broker drops the ticket
/// unfinished, so debt latches); SessionPort then keeps the broker's own
/// class, never the absence mapping. A poisoned broker hold is not
/// reachable here: no broker hold crosses a caller callback (P1).
fn s2_debt() {
    let f = SFx::new();
    f.fake.route(|p| (p == "/api/chat").then(|| chat(REPLY)));
    let (port, mission) = (f.home.port.clone(), Arc::new(Mutex::new(None)));
    let (seen, at): (Seen, _) = (Arc::default(), mission.clone());
    let s = seen.clone();
    let m = f.mother(2, move |ctx| {
        let first = ask(ctx);
        let id = at.lock().unwrap().unwrap();
        let owner = InvocationOwner::new();
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            port.model_call(id, &request("SB3C"), &Unwinding, &Never, &owner.observer())
                .map(|_| ())
                .map_err(|r| r.class)
        }));
        let meter = Meter::default();
        let admit = port.admit_inference(id, &"ab".repeat(32), "request", identity_bytes());
        let call = port.model_call(id, &request("SB3C"), &meter, &Never, &owner.observer());
        let second = ask(ctx);
        assert!(unwound.is_err(), "the attempt unwound in its meter");
        assert_eq!(
            admit.map_err(|r| r.class),
            Err(BrokerClass::Halted),
            "debt: admission"
        );
        let call = call.map(|_| ()).map_err(|r| r.class);
        assert_eq!(call, Err(BrokerClass::Halted), "debt: call");
        assert!(port.model_identity().is_none(), "debt: no identity");
        assert_eq!(meter.0.load(Ordering::SeqCst), 0, "debt: nothing charged");
        s.lock()
            .unwrap()
            .extend([outcome(&first), outcome(&second)]);
        draft("sb3c")
    });
    let id = f.building(&m, f.contract());
    *mission.lock().unwrap() = Some(id);
    assert!(m.advance(id).is_err(), "the debt stops the mission");
    assert_eq!(
        *seen.lock().unwrap(),
        ["ok", "invalidated"],
        "Mother's calls"
    );
    assert_eq!(
        (f.charges(), f.fake.count("/api/chat")),
        (1, 1),
        "debt: one call"
    );
}

// ------------------------------------------------------------------- R4

/// R4: the real continuation constructor's port has no identity and its
/// admission and call are halted. (Mother builds no conversation request;
/// the conversation refusal is the broker's public-route oracle.)
#[test]
fn sb3_continuation_and_conversation_never_call() {
    children(
        "sb3_continuation_and_conversation_never_call",
        &["r4/continuation"],
    );
}
fn r4(arg: &str) {
    let f = SFx::new();
    if arg == "continuation" {
        let m = f.file_mother();
        let id = f.awaiting(&m);
        let request_ = f
            .home
            .session
            .ingress()
            .accept(CommandKind::Approve, 100)
            .unwrap();
        f.home.session.approve_file(&request_).unwrap();
        let continuation = f.home.session.approval_continuation(&request_).unwrap();
        let (generation, effect) = (continuation.generation(), continuation.effect());
        let (port, _cell) = f
            .home
            .port
            .continue_approval(continuation, id, generation, effect)
            .unwrap();
        let (meter, owner) = (Meter::default(), InvocationOwner::new());
        assert!(port.model_identity().is_none(), "Continuation: no identity");
        let admit = port.admit_inference(id, &"ab".repeat(32), "request", identity_bytes());
        assert_eq!(admit.map_err(|r| r.class), Err(BrokerClass::Halted));
        let call = port.model_call(id, &request("SB3C"), &meter, &Never, &owner.observer());
        assert_eq!(
            call.map(|_| ()).map_err(|r| r.class),
            Err(BrokerClass::Halted)
        );
        assert_eq!(meter.0.load(Ordering::SeqCst), 0);
        assert_eq!(f.fake.count("/api/chat"), 0, "Continuation: no chat");
        return;
    }
    unreachable!("r4 case {arg}");
}

// ------------------------------------------------------------------- L2

/// Composed L2, metadata half (SB3B-R3 Q1 carry; the chat half is C2's
/// composition evidence).
#[test]
fn sb3_revocation_after_entry_is_entered_only() {
    children(
        "sb3_revocation_after_entry_is_entered_only",
        &["l2/metadata"],
    );
}
/// The invocation is revoked while the call's first request (version) is
/// entered: the session gates observe the caller's own observer, so no
/// successor request is sent (MP-SB-C-observer's named assertion).
fn l2_metadata() {
    let f = Arc::new(SFx::new());
    let seen: Seen = Arc::default();
    let s = seen.clone();
    let m = f.mother(1, move |ctx| {
        let r = ask(ctx);
        s.lock().unwrap().push(outcome(&r));
        quiet()
    });
    let id = f.building(&m, f.contract());
    let (entered_tx, entered_rx) = signal();
    let (release_tx, release_rx) = signal();
    let release_rx = Mutex::new(release_rx);
    f.fake.route(move |p| {
        if p == "/api/version" {
            let release = release_rx.lock().unwrap();
            let _ = entered_tx.send(());
            let _ = release.recv_timeout(crate::adapter::c1b_fake_ollama::PATIENCE);
        }
        None
    });
    let adv = advance_in(&m, id);
    within(&entered_rx);
    m.overtake_model_for_test(id).unwrap();
    release_tx.send(()).unwrap();
    let advanced = adv.join().unwrap();
    assert_eq!(f.fake.count("/api/version"), 2, "the entered request once");
    assert_eq!(
        f.fake.count("/api/status"),
        1,
        "no successor after revocation"
    );
    assert_eq!(f.fake.count("/api/chat"), 0, "no chat");
    assert_eq!(f.charges(), 0, "no charge");
    // SB3C-R4 (4): the call's and the advance's results, and Mother's zeros.
    assert_eq!(
        *seen.lock().unwrap(),
        ["failed:model_cancelled"],
        "the call result"
    );
    let building = m.state(id).unwrap() == Some(MissionState::Building);
    assert!(advanced.is_err() || !building, "the advance: {advanced:?}");
    assert_eq!(f.model_lane_returned(), 0, "no LaneReturned");
    assert_eq!(f.dispatched(), 0, "no File dispatch");
    let journal = f.home.lines().concat();
    assert!(!journal.contains("sb3c model lane"), "no accepted output");
}
fn l2(arg: &str) {
    assert_eq!(arg, "metadata");
    l2_metadata();
}
