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
        &["s2/unconfigured", "s2/restricted"],
    );
}
fn s2(arg: &str) {
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
    let m = f.mother(1, |ctx| {
        let _ = ask(ctx);
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
    let _ = adv.join().unwrap();
    assert_eq!(f.fake.count("/api/version"), 2, "the entered request once");
    assert_eq!(
        f.fake.count("/api/status"),
        1,
        "no successor after revocation"
    );
    assert_eq!(f.fake.count("/api/chat"), 0, "no chat");
    assert_eq!(f.charges(), 0, "no charge");
}
fn l2(arg: &str) {
    assert_eq!(arg, "metadata");
    l2_metadata();
}
