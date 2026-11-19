//! M3-Dd composed oracles (DD-R1, SB3E-R153): M3-O35 and the SB-3 §11
//! consumer table through PageBuilderLane -> C1 Caller -> SessionPort ->
//! session broker -> fake peer -> session File -> Db proof -> Dc verifier.
//! One case per child process (one broker per process), bounded by the C1b
//! child runner. The Owner's File answer is an oracles confirmer passed to
//! the public `approve_file_confirmed`; Stop is Mother's own `stop`.
//! Claim limit: composed O35 on an UNQUALIFIED fixture route only.
use super::c1b_fake_ollama::{chat, Answer, Seen as Peer};
use super::c1b_port_oracles::{children, record_next};
use super::m3cb_support::pause;
use super::m3d_composed_support::*;
use crate::coordinator::{Coordinator, CoordinatorError, Lanes};
use crate::journal::{Event, MemoryJournal};
use crate::lane::Verdict;
use crate::oracles::lite_support::FilesOwner;
use crate::page_builder::cb_fixtures::{block, page, rejected, DRAFT};
use crate::state::MissionState;
use nexus_governed_control::authority::evidence::EndStatus;
use nexus_governed_control::authority::file_session::FileSessionError;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CASE: &str = "NEXUS_M3DD_CASE";
const MODULE: &str = "adapter::m3d_composed_oracles";
/// The broker's File dialog line, shown only when the consent that admitted
/// the File carried the page facts (kernel inert-page policy template).
const FACTS_LINE: &str = "facts present as given: name, hours (1 lines), phone";
const LEAD: &str = "Welcome to our bakery.";

fn run(case: &str) {
    let (kind, arg) = case.split_once('/').unwrap_or((case, ""));
    match kind {
        "o35" => o35(arg),
        "t11" => t11(arg),
        other => panic!("unknown case {other}"),
    }
}
fn is_receipt(e: &Event) -> bool {
    matches!(e, Event::PublicationReceipt { .. })
}
fn is_verification(e: &Event) -> bool {
    matches!(e, Event::VerificationRecorded { .. })
}
fn is_retained(e: &Event) -> bool {
    matches!(e, Event::DeliverableRetained { .. })
}
fn sha(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}
/// Advance until terminal (bounded); every explanation returned.
fn finish(p: &Page) -> Vec<String> {
    let mut said = Vec::new();
    for _ in 0..6 {
        if p.m.state(p.id).unwrap().is_some_and(|s| s.is_terminal()) {
            break;
        }
        match p.m.advance(p.id) {
            Ok(a) => said.extend(a.explanation),
            Err(e) => said.push(format!("err:{e:?}")),
        }
    }
    said
}
fn awaiting(p: &Page) {
    let r = within(&p.advance_bg());
    assert!(r.is_ok(), "{r:?}");
    assert_eq!(
        p.m.state(p.id).unwrap(),
        Some(MissionState::AwaitingApproval)
    );
}
/// Hold `/api/chat` at the peer until the connection ends.
fn hold_chat(p: &Page) {
    p.f.fake
        .route(|path| (path == "/api/chat").then_some(Answer::Hold));
    p.f.fake.drain();
}

// ------------------------------------------------------------------ O35

/// M3-O35: approve -> one publication and journaled verification; decline
/// -> zero publication and Verifier calls; held-chat Stop -> zero
/// publication and no accepted late result; pre-final-check Stop -> zero
/// publication; final-check wins -> at most one truthful publication and no
/// later Verifier. Charges stay recorded, never refunded.
#[test]
fn m3d_session_composed_approve_decline_and_stop() {
    children(
        MODULE,
        CASE,
        "m3d_session_composed_approve_decline_and_stop",
        &[
            "o35/approve",
            "o35/decline",
            "o35/held-chat",
            "o35/pre-lane",
            "o35/pre-perform",
            "o35/final",
        ],
        run,
    );
}
fn o35(arg: &str) {
    match arg {
        "approve" => approve(),
        "decline" => {
            let p = Page::new(vec![chat(DRAFT)]);
            awaiting(&p);
            assert!(
                !p.approve(&crate::oracles::No),
                "a declined File is not run"
            );
            let said = finish(&p);
            assert_eq!(p.end(), "action_denied", "{said:?}");
            assert!(p.index().is_none(), "publication 0");
            assert_eq!((p.count(is_receipt), p.count(is_verification)), (0, 0));
            assert_eq!(p.verifier(), 0, "verifier calls 0");
            assert_eq!(p.f.charges(), 1, "the charge stays recorded");
        }
        "held-chat" => {
            let p = Page::new(vec![]);
            hold_chat(&p);
            let adv = p.advance_bg();
            assert!(p.f.fake.wait(Peer::Request("/api/chat".into())), "held");
            assert_eq!(p.m.stop(p.id), Ok(MissionState::Stopped));
            assert!(p.f.fake.wait(Peer::Closed("/api/chat".into())), "closed");
            let r = within(&adv);
            assert_eq!(p.results(), ["failed:model_cancelled"], "{r:?}");
            assert_eq!(p.tally(), (1, 1, 1), "charge recorded, never refunded");
            // F5: no accepted output, LaneReturned or File dispatch.
            let o = p.output();
            assert!(matches!(o.verdict, Verdict::Fail { .. }) && o.effects.is_empty());
            p.nothing_accepted();
            assert_eq!((p.count(is_receipt), p.verifier()), (0, 0), "publication 0");
            assert_eq!(p.native(), "recorded", "the conclusive native record");
            assert_eq!(p.end(), "stop:owner");
        }
        "pre-lane" | "pre-perform" => pre_final_check(arg),
        "final" => {
            let p = Page::new(vec![chat(DRAFT)]);
            awaiting(&p);
            // The broker's final check passed and the page was published.
            assert!(p.approve(&FilesOwner::default()), "published");
            // Stop lands after it: the publication stands, overtaken.
            assert_eq!(p.m.stop(p.id), Ok(MissionState::Blocked));
            let said = finish(&p);
            let written = p.index().expect("the one truthful publication");
            let events = p.f.events();
            let receipts: Vec<_> = events.iter().filter(|e| is_receipt(e)).collect();
            assert_eq!(receipts.len(), 1, "{said:?}");
            assert!(matches!(
                receipts[0],
                Event::PublicationReceipt { sha256, overtaken: true, .. } if *sha256 == sha(&written)
            ));
            assert_eq!(p.end(), "settled");
            let later = (p.verifier(), p.count(is_verification));
            assert_eq!(later, (0, 0), "no later verifier");
            assert_eq!(p.tally(), (1, 1, 1), "charge recorded, never refunded");
        }
        _ => panic!("unknown o35 {arg}"),
    }
}
fn approve() {
    let p = Page::new(vec![chat(DRAFT)]);
    awaiting(&p);
    // DD-R1 STOP check, lane side: the actual Builder got the consented snapshot.
    let bound = p.seen.lock().unwrap().bound.clone();
    assert_eq!(bound, [true], "the consented input at the lane");
    let owner = FilesOwner::default();
    assert!(p.approve(&owner), "the Owner's yes publishes");
    // DD-R1 STOP check, broker side: the session consent carried the page
    // facts into the File admission.
    let shown = owner.0.lock().unwrap().clone();
    assert_eq!(shown.len(), 1, "one checked File request");
    let lines = &shown[0].summary;
    assert!(lines.iter().any(|l| l == FACTS_LINE), "{lines:?}");
    let said = finish(&p);
    let done = p.m.state(p.id).unwrap();
    assert_eq!(done, Some(MissionState::Done), "{said:?}");
    // Caller entry, chat, charge; the native count is the broker's record
    // state (its store is locked while the session is open).
    assert_eq!(p.tally(), (1, 1, 1));
    assert_eq!(p.results(), ["ok"]);
    assert_eq!(p.native(), "recorded");
    let written = p.index().expect("one publication");
    assert_eq!(written, page(LEAD).as_bytes());
    assert_eq!((p.count(is_retained), p.f.dispatched()), (1, 1));
    let events = p.f.events();
    let receipts: Vec<_> = events.iter().filter(|e| is_receipt(e)).collect();
    assert_eq!(receipts.len(), 1);
    assert!(matches!(
        receipts[0],
        Event::PublicationReceipt { sha256, overtaken: false, policy_hash: Some(_), .. }
            if *sha256 == sha(&written)
    ));
    let checks: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::VerificationRecorded { checks, .. } => Some(checks.clone()),
            _ => None,
        })
        .collect();
    let all = ["v0", "v1", "v2", "v3", "v4", "v5"].map(|v| (v.to_string(), "pass".to_string()));
    assert_eq!(checks, [all.to_vec()]);
    assert_eq!(p.verifier(), 1);
    let t11 = said
        .iter()
        .any(|s| s.contains(crate::teaching::T11_CHECKED));
    assert!(t11, "T11: {said:?}");
}
/// Stop before the File's final check: before the Builder
/// (`file_before_lane`), or after the File was staged at the broker and
/// before Mother settles it (`file_after_perform_check`).
fn pre_final_check(arg: &str) {
    let p = Page::new(vec![chat(DRAFT)]);
    let (at_tx, at) = channel();
    let (go, wait_go) = channel::<()>();
    let hooks = p.m.hooks_for_test();
    let slot = if arg == "pre-lane" {
        &hooks.file_before_lane
    } else {
        &hooks.file_after_perform_check
    };
    slot.arm(0, move || {
        at_tx.send(()).unwrap();
        let _ = wait_go.recv_timeout(Duration::from_secs(30));
    });
    let adv = p.advance_bg();
    within(&at);
    assert_eq!(p.m.stop(p.id), Ok(MissionState::Stopped));
    go.send(()).unwrap();
    assert!(within(&adv).is_err(), "the advance is refused");
    assert!(
        !p.approve(&FilesOwner::default()),
        "nothing left to approve"
    );
    let said = finish(&p);
    assert!(p.index().is_none(), "publication 0: {said:?}");
    assert_eq!((p.count(is_receipt), p.verifier()), (0, 0));
    if arg == "pre-lane" {
        assert_eq!(p.tally(), (0, 0, 0), "no model call");
        assert_eq!(p.end(), "stop:owner");
    } else {
        assert_eq!(p.tally(), (1, 1, 1), "charge recorded, never refunded");
        assert_eq!((p.count(is_retained), p.f.dispatched()), (1, 1));
        assert_eq!(p.end(), "settled");
    }
}

// --------------------------------------------------- SB-3 §11 consumer table

/// SB-3 §11 consumer rows on the session route, the actual Builder each
/// time: first draft; malformed then valid (one correction); two malformed
/// (exactly two caller entries and attempts); Stop between attempts; Stop
/// during the correction after its charge; recording debt (F7, injected at
/// the adapter through SB3D-R1 Q2, Uncertain and Owed; native debt is B's
/// evidence); a late successful reply (F5); replay.
#[test]
fn m3dd_session_route_consumer_cases() {
    children(
        MODULE,
        CASE,
        "m3dd_session_route_consumer_cases",
        &[
            "t11/first",
            "t11/malformed",
            "t11/two",
            "t11/between",
            "t11/during",
            "t11/debt-uncertain",
            "t11/debt-owed",
            "t11/late",
            "t11/replay",
        ],
        run,
    );
}
fn failed(p: &Page, code: &str) {
    let o = p.output();
    let want = Verdict::Fail {
        reason: code.into(),
    };
    assert_eq!(o.verdict, want);
    assert!(o.effects.is_empty());
}
fn t11(arg: &str) {
    match arg {
        "first" | "malformed" | "two" => drafts(arg),
        "between" => {
            let p = Page::new(vec![chat(&rejected(2)), chat(DRAFT)]);
            let (pz, held) = pause();
            p.seen.lock().unwrap().pause_call = Some((1, pz));
            let adv = p.advance_bg();
            held.reached();
            assert_eq!(p.m.stop(p.id), Ok(MissionState::Stopped));
            held.release();
            let _ = within(&adv);
            assert_eq!(p.results(), ["ok", "invalidated"]);
            failed(&p, "model_invalidated");
            assert_eq!(p.tally(), (2, 1, 1), "no correction chat or charge");
            p.nothing_accepted();
            assert_eq!(p.end(), "stop:owner");
        }
        "during" => {
            let p = Page::new(vec![]);
            let n = AtomicUsize::new(0);
            p.f.fake.route(move |path| {
                (path == "/api/chat").then(|| match n.fetch_add(1, Ordering::SeqCst) {
                    0 => chat(&rejected(2)),
                    _ => Answer::Hold,
                })
            });
            let adv = p.advance_bg();
            assert!(p.f.fake.wait(Peer::Request("/api/chat".into())));
            assert!(p.f.fake.wait(Peer::Request("/api/chat".into())));
            assert_eq!(p.f.charges(), 2, "the correction's charge");
            assert_eq!(p.m.stop(p.id), Ok(MissionState::Stopped));
            assert!(p.f.fake.wait(Peer::Closed("/api/chat".into())));
            let _ = within(&adv);
            assert_eq!(p.results(), ["ok", "failed:model_cancelled"]);
            assert_eq!(p.tally(), (2, 2, 2), "charges recorded, never refunded");
            p.nothing_accepted();
            assert_eq!(p.end(), "stop:owner");
        }
        "debt-uncertain" | "debt-owed" => {
            let p = Page::new(vec![chat(DRAFT)]);
            let status = if arg == "debt-uncertain" {
                EndStatus::Uncertain
            } else {
                EndStatus::Owed
            };
            record_next(p.f.home.port.inner_for_test(), status);
            let advanced = within(&p.advance_bg());
            assert_eq!(advanced.err(), Some(CoordinatorError::Recording));
            assert_eq!(p.results(), ["record_unavailable"]);
            failed(&p, "model_record_unavailable");
            assert_eq!(p.tally(), (1, 1, 1), "no correction; the charge stands");
            p.nothing_accepted();
            let close = p.m.close();
            assert_eq!(close, Err(CoordinatorError::Recording), "no clean close");
            assert_eq!(p.end(), "block:broker_refused:evidence_unavailable");
        }
        "late" => {
            let p = Page::new(vec![chat(DRAFT)]);
            let (pz, held) = pause();
            p.m.hooks_for_test().model_replied.arm(0, move || pz.hold());
            let adv = p.advance_bg();
            held.reached();
            assert_eq!(p.m.stop(p.id), Ok(MissionState::Stopped));
            held.release();
            let _ = within(&adv);
            assert_eq!(
                p.results(),
                ["invalidated"],
                "the recorded reply is not used"
            );
            assert_eq!(p.tally(), (1, 1, 1));
            assert_eq!(p.native(), "recorded", "the native record exists");
            p.nothing_accepted();
        }
        "replay" => replay(),
        _ => panic!("unknown t11 {arg}"),
    }
}
fn drafts(arg: &str) {
    let replies = match arg {
        "first" => vec![chat(DRAFT)],
        "malformed" => vec![chat(&rejected(2)), chat(DRAFT)],
        _ => vec![chat(&rejected(2)), chat(&rejected(7))],
    };
    let p = Page::new(replies);
    let _ = within(&p.advance_bg());
    let n = if arg == "first" { 1 } else { 2 };
    assert_eq!(p.tally(), (n, n, n), "caller entries, chats, charges");
    if arg == "two" {
        failed(&p, "model_draft_email");
        p.nothing_accepted();
        assert_eq!(p.end(), "fail:lane_fail:builder");
        return;
    }
    assert_eq!(p.f.dispatched(), 1);
    let state = p.m.state(p.id).unwrap();
    assert_eq!(state, Some(MissionState::AwaitingApproval));
    if arg == "malformed" {
        let bodies = p.f.fake.bodies("/api/chat");
        let v: serde_json::Value = serde_json::from_slice(&bodies[1]).unwrap();
        let user = v["messages"][1]["content"].as_str().unwrap();
        assert!(user.ends_with(&block(2)), "one correction");
    }
}
/// A second coordinator over the same SessionPort and a copy of the journal
/// is refused: no caller is restored and no metadata or chat is sent.
fn replay() {
    let p = Page::new(vec![]);
    hold_chat(&p);
    let adv = p.advance_bg();
    assert!(p.f.fake.wait(Peer::Request("/api/chat".into())));
    let peer = || ["/api/chat", "/api/tags", "/api/version"].map(|x| p.f.fake.count(x));
    let before = (peer(), p.f.home.lines());
    let copy = MemoryJournal::new();
    copy.set_lines(before.1.clone());
    let fresh = Arc::new(Mutex::new(Seen::default()));
    let reopened = Coordinator::open(
        copy.clone(),
        p.f.home.port.clone(),
        Lanes::new(lanes(&fresh)).unwrap(),
        p.f.home.clock.clone(),
        p.f.home.private(),
    );
    let refused = matches!(
        reopened,
        Err(CoordinatorError::FileSession(FileSessionError::Reopened))
    );
    assert!(refused, "reopen refused");
    assert_eq!(fresh.lock().unwrap().calls, 0, "no restored caller");
    assert_eq!((peer(), copy.lines()), before, "no new metadata or chat");
    let _ = p.m.stop(p.id);
    let _ = within(&adv);
}
