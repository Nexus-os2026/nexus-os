//! SB-3 C2 composition evidence (spec §9.2–9.3; SB3C-R2 moved C1, C3, O4
//! and composed L2's chat half here from C1): the real C1 caller →
//! SessionPort → session broker → fake loopback peer, one child process per
//! case (one broker per process).
use crate::adapter::c1b_coordinator_oracles::{ask, draft, outcome, request, REPLY};
use crate::adapter::c1b_fake_ollama::{chat, Answer};
use crate::adapter::c1b_port_oracles::children as run_children;
use crate::adapter::session_model_bridge_support::{advance_in, count, signal, within, SFx};
use crate::lane::{LaneOutput, Verdict};
use crate::state::MissionState;
use nexus_governed_control::inference::ModelReply;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const CASE: &str = "NEXUS_SB3C2_CASE";
const MODULE: &str = "adapter::session_model_composition_tests";

fn children(test: &str, cases: &[&str]) {
    run_children(MODULE, CASE, test, cases, run);
}
fn run(case: &str) {
    let (name, arg) = case.split_once('/').unwrap_or((case, ""));
    match name {
        "c1" => c1(arg),
        "c3" => c3(arg),
        "o4" => o4(arg),
        "l2" => l2(arg),
        other => panic!("unknown case {other}"),
    }
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

// ------------------------------------------------------------------- C1

/// C1: at the peer's chat the journal already holds its exact charge; a
/// metadata failure charges nothing; a failed charge append sends no chat.
#[test]
fn sb3_durable_charge_precedes_chat() {
    children(
        "sb3_durable_charge_precedes_chat",
        &["c1/ordered", "c1/metadata", "c1/append"],
    );
}
fn c1(arg: &str) {
    let f = Arc::new(SFx::new());
    let at_chat = Arc::new(Mutex::new(None));
    match arg {
        "ordered" => {
            let (seen, g) = (at_chat.clone(), f.clone());
            f.fake.route(move |p| {
                (p == "/api/chat").then(|| {
                    *seen.lock().unwrap() = Some(g.charges());
                    chat(REPLY)
                })
            });
        }
        "metadata" => f
            .fake
            .route(|p| (p == "/api/tags").then(|| Answer::Json(500, "{}".into()))),
        _ => f.probe.refuse("\"model_call_charged\""),
    }
    let seen: Seen = Arc::default();
    let s = seen.clone();
    let m = f.mother(1, move |ctx| {
        let r = ask(ctx);
        s.lock().unwrap().push(outcome(&r));
        quiet()
    });
    let id = f.building(&m, f.contract());
    let _ = m.advance(id);
    match arg {
        "ordered" => {
            assert_eq!(
                *at_chat.lock().unwrap(),
                Some(1),
                "charge journaled before chat"
            );
            assert_eq!(*seen.lock().unwrap(), ["ok"]);
        }
        "metadata" => {
            assert_eq!(f.charges(), 0, "metadata failure: no charge");
            assert_eq!(f.fake.count("/api/chat"), 0, "metadata failure: no chat");
            assert_eq!(f.fake.count("/api/tags"), 2, "the one attempt reached tags");
        }
        _ => {
            // The failed append poisons the journal; read its raw lines.
            let raw = std::fs::read_to_string(f.home.dir.join("mother-journal.jsonl")).unwrap();
            let charged = raw.matches("\"model_call_charged\"").count();
            assert_eq!(charged, 0, "failed append: no charge acknowledged");
            assert_eq!(f.fake.count("/api/chat"), 0, "failed append: no chat");
            let ok = seen.lock().unwrap().iter().any(|o| o == "ok");
            assert!(!ok, "failed append: no reply");
        }
    }
}

// ------------------------------------------------------------------- C3

/// C3: cap 0 has no caller; caps 1/2/4 exact-fit positives pass and the
/// next call refuses with no chat or refund.
#[test]
fn sb3_caps_and_budget_keep_existing_goldens() {
    children(
        "sb3_caps_and_budget_keep_existing_goldens",
        &["c3/0", "c3/1", "c3/2", "c3/4"],
    );
}
fn c3(arg: &str) {
    let cap: u32 = arg.parse().unwrap();
    let f = SFx::new();
    let seen: Seen = Arc::default();
    let (s, none) = (seen.clone(), Arc::new(AtomicBool::new(false)));
    let n = none.clone();
    let m = f.mother(cap, move |ctx| {
        let Some(model) = ctx.model else {
            n.store(true, Ordering::SeqCst);
            return quiet();
        };
        for _ in 0..=cap {
            let r = model.call(&request("SB3C"));
            s.lock().unwrap().push(outcome(&r));
        }
        quiet()
    });
    if cap == 0 {
        let id = f.building(&m, f.contract());
        let _ = m.advance(id);
        assert!(none.load(Ordering::SeqCst), "cap 0: the lane has no caller");
        assert_eq!(
            count(&f.invoked),
            1,
            "cap 0: invoked once, without a caller"
        );
        assert_eq!(f.fake.count("/api/chat"), 0);
        return;
    }
    let id = f.building(&m, f.contract());
    let _ = m.advance(id);
    let seen = seen.lock().unwrap().clone();
    let ok = seen.iter().filter(|o| *o == "ok").count();
    assert_eq!(ok, cap as usize, "cap {cap}: exact fit");
    assert!(
        seen.last().is_some_and(|o| o != "ok"),
        "cap {cap}: next refuses"
    );
    assert_eq!(f.fake.count("/api/chat"), cap as usize, "cap {cap}: chats");
    assert_eq!(f.charges(), cap as usize, "cap {cap}: charges, no refund");
}

// ------------------------------------------------------------------- O4

/// O4: an ingress before the charge append's admission (at the last
/// metadata reply) leaves charges and chat 0/0; an append already admitted
/// (the ingress inside the append) may finish, but the chat refuses. The
/// append is Work, not settlement.
#[test]
fn sb3_charge_is_work_not_settlement() {
    children(
        "sb3_charge_is_work_not_settlement",
        &["o4/before", "o4/admitted"],
    );
}
fn o4(arg: &str) {
    let f = SFx::new();
    let ingress = f.home.session.ingress();
    if arg == "before" {
        let fired = AtomicBool::new(false);
        f.fake.route(move |p| {
            if p == "/api/tags" && !fired.swap(true, Ordering::SeqCst) {
                // The call's tags request (registration's was earlier).
                ingress.emergency().unwrap();
            }
            None
        });
    } else {
        f.probe.at("\"model_call_charged\"", move || {
            ingress.emergency().unwrap();
        });
    }
    let seen: Seen = Arc::default();
    let s = seen.clone();
    let m = f.mother(1, move |ctx| {
        let r = ask(ctx);
        s.lock().unwrap().push(outcome(&r));
        quiet()
    });
    let id = f.building(&m, f.contract());
    let _ = m.advance(id);
    let charges = usize::from(arg == "admitted");
    assert_eq!(f.charges(), charges, "{arg}: charges");
    assert_eq!(
        f.fake.count("/api/chat"),
        0,
        "{arg}: no chat after the ingress"
    );
    let ok = seen.lock().unwrap().iter().any(|o| o == "ok");
    assert!(!ok, "{arg}: no reply");
}

// ------------------------------------------------------------------- L2

/// Composed L2, chat half (SB3B-R3 Q1 carry; SB3C-R2: C2's composition
/// evidence; C1 keeps the metadata half): a reply released after the actual
/// invocation is revoked, at the entered request, is never used: at most
/// that one chat; accepted output, LaneReturned and File dispatch 0.
#[test]
fn sb3_revocation_after_chat_entry_is_entered_only() {
    children(
        "sb3_revocation_after_chat_entry_is_entered_only",
        &["l2/chat"],
    );
}
fn l2(arg: &str) {
    assert_eq!(arg, "chat");
    let f = SFx::new();
    let seen: Seen = Arc::default();
    let (returned_tx, returned_rx) = signal();
    let s = seen.clone();
    let m = f.mother(2, move |ctx| {
        let r = ask(ctx);
        s.lock().unwrap().push(outcome(&r));
        let second = ask(ctx);
        s.lock().unwrap().push(outcome(&second));
        let _ = returned_tx.send(());
        draft(&r.map(ModelReply::into_content).unwrap_or_default())
    });
    let id = f.building(&m, f.contract());
    let (entered_tx, entered_rx) = signal();
    let (release_tx, release_rx) = signal();
    let release_rx = Mutex::new(release_rx);
    f.fake.route(move |p| {
        (p == "/api/chat").then(|| {
            let release = release_rx.lock().unwrap();
            let _ = entered_tx.send(());
            let _ = release.recv_timeout(crate::adapter::c1b_fake_ollama::PATIENCE);
            chat(REPLY)
        })
    });
    let adv = advance_in(&m, id);
    within(&entered_rx);
    m.overtake_model_for_test(id).unwrap();
    release_tx.send(()).unwrap();
    within(&returned_rx);
    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen[0], "invalidated", "the released reply is never used");
    assert_ne!(seen[1], "ok", "no successor call is admitted");
    assert!(adv.join().unwrap().is_err() || m.state(id).unwrap() != Some(MissionState::Building));
    assert_eq!(f.fake.count("/api/chat"), 1, "at most the entered request");
    assert_eq!(f.model_lane_returned(), 0, "no LaneReturned");
    assert_eq!(f.dispatched(), 0, "no File dispatch");
    assert!(
        !f.home.lines().concat().contains(REPLY),
        "no accepted output"
    );
}
