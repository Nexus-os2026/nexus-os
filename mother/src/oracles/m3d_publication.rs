//! M3-Db oracles frozen before implementation. A child of the proof module
//! only so that one control can alter a live mission's retained reference;
//! everything else goes through the public lifecycle or the journal.
use crate::adapter::m3d_publication_oracles::{
    at, count, is_approved, is_dispatch, is_receipt, is_retained, lanes_after, load_lines,
    other_page, run_child, run_child_with, save_lines, scratch, sha, FailOn, PageFx, BAKERY,
    WRITTEN,
};
use crate::coordinator::inspect_history_lines;
use crate::effect::PendingAction;
use crate::evidence::{check, FailReason, StopReason};
use crate::journal::{Entry, Event, JournalError, MissionId, ReplayViolation, GENESIS};
use crate::lane::InvocationId;
use crate::oracles::lite_support::file;
use crate::private::{ContentId, ContentKind, PrivateStore, MAX_CONTENT_BYTES};
use crate::state::{MissionState, Trigger};

const CHILD: &str = "coordinator::page_proof::oracles::m3d_db_internal_child";
const RUN: &str = "run-000102030405060708090a0b0c0d0e0f";
const CMT: &str = "cmt-f0e0d0c0b0a090807060504030201000";
// Independent Python hashlib: SHA-256 of the domain
// `nexus.mother.private.v1/deliverable`, u64 BE length 8, `<p>x</p>`.
const CONTENT_ID: &str = "8f8831ac4c018f19fa2bf1dae5b3b57458203b44c146db1b9ebf5921c54406f9";
const RAW_SHA: &str = "31d8e07ec305ac4e2515d1f0b1c8c603b3858044eb7d97807d6d0970838cb0be";

#[test]
fn m3d_proof_records_are_frozen_evidence() {
    let content = ContentId::of(ContentKind::Deliverable, b"<p>x</p>");
    assert_eq!(content.to_hex(), CONTENT_ID);
    assert_eq!(sha(b"<p>x</p>"), RAW_SHA);
    let retained = Event::DeliverableRetained {
        invocation: InvocationId {
            mission: MissionId(1),
            seq: 9,
        },
        effect: 0,
        content,
        sha256: RAW_SHA.into(),
        bytes: 8,
    };
    let retained_json = format!(
        r#"{{"event":"deliverable_retained","invocation":{{"mission":1,"seq":9}},"effect":0,"content":"{CONTENT_ID}","sha256":"{RAW_SHA}","bytes":8}}"#
    );
    let receipt = Event::PublicationReceipt {
        effect: 0,
        action: PendingAction {
            run: RUN.into(),
            commitment: CMT.into(),
            deadline_mono_ms: 5,
        },
        sha256: RAW_SHA.into(),
        bytes: 8,
        policy_hash: "ab".repeat(32).into(),
        overtaken: false,
    };
    let receipt_json = format!(
        r#"{{"event":"publication_receipt","effect":0,"action":{{"run":"{RUN}","commitment":"{CMT}","deadline_mono_ms":5}},"sha256":"{RAW_SHA}","bytes":8,"policy_hash":"{}","overtaken":false}}"#,
        "ab".repeat(32)
    );
    for (event, json) in [(&retained, &retained_json), (&receipt, &receipt_json)] {
        assert_eq!(&serde_json::to_string(event).unwrap(), json);
        assert_eq!(&serde_json::from_str::<Event>(json).unwrap(), event);
        assert_eq!(check(event), Ok(()));
        let extra = json.replacen("{", r#"{"page":"x","#, 1);
        assert!(serde_json::from_str::<Event>(&extra).is_err());
    }
    let bad = |edit: &dyn Fn(&mut Event)| {
        for base in [&retained, &receipt] {
            let mut e = base.clone();
            edit(&mut e);
            if &e != base {
                assert_eq!(check(&e), Err(ReplayViolation::NotEvidence), "{e:?}");
            }
        }
    };
    bad(&|e| match e {
        Event::DeliverableRetained { sha256, .. } | Event::PublicationReceipt { sha256, .. } => {
            *sha256 = RAW_SHA.to_uppercase()
        }
        _ => {}
    });
    bad(&|e| match e {
        Event::DeliverableRetained { bytes, .. } | Event::PublicationReceipt { bytes, .. } => {
            *bytes = MAX_CONTENT_BYTES + 1
        }
        _ => {}
    });
    // Db-Q4: written through JSON so the policy field's type is not assumed.
    let policy = with_json(&receipt, "policy_hash", "Lemon drizzle".into());
    assert_eq!(check(&policy), Err(ReplayViolation::NotEvidence));
    let run = serde_json::json!({"run": "run-x", "commitment": CMT, "deadline_mono_ms": 5});
    assert_eq!(
        check(&with_json(&receipt, "action", run)),
        Err(ReplayViolation::NotEvidence)
    );
    for (reason, code) in [
        (FailReason::ReceiptMissing, "receipt_missing"),
        (FailReason::ReceiptPolicyMissing, "receipt_policy_missing"),
        (FailReason::DeliverableInvalid, "deliverable_invalid"),
    ] {
        assert_eq!(reason.to_string(), code);
        assert_eq!(FailReason::parse(code), Some(reason));
        assert!(FailReason::every().contains(&reason));
    }
}

#[test]
fn m3d_deliverable_tamper_stops_without_fallback() {
    for case in ["removed", "changed", "substituted", "digest", "size"] {
        run_child(CHILD, &format!("o33b-{case}"));
    }
}

/// Db-R1 H1 / Db-R1a: a crash between ActionApproved and its receipt. Every
/// prefix between them and closure is recovered by a fresh broker; the
/// unproven one twice, identically, with no closure or ack.
#[test]
fn m3d_recovery_never_closes_an_unproven_publication() {
    let dir = scratch("h1");
    let env = [("M3D_DB_DIR", dir.display().to_string())];
    run_child_with(CHILD, "h1-publish", &env);
    let lines = load_lines(&dir);
    let events: Vec<Event> = crate::journal::verify(&lines)
        .unwrap()
        .into_iter()
        .map(|e| e.event)
        .collect();
    let a = at(&events, is_approved).unwrap();
    let closed = at(&events, |e| matches!(e, Event::ApprovalClosed { .. })).unwrap();
    assert_eq!(at(&events, is_receipt), Some(a + 1));
    let recover = |n: usize| {
        let mut env = env.to_vec();
        env.push(("M3D_DB_PREFIX", n.to_string()));
        let out = run_child_with(CHILD, "h1-recover", &env);
        let at = out.find("H1-OBS ").expect("observation");
        out[at..].lines().next().unwrap().to_string()
    };
    let unproven = format!(
        "H1-OBS open=ok state=Some(Failed) reason=Some(\"journal_failed\") notice=Ok(Some({WRITTEN:?})) close=err closures=0 acks=0"
    );
    let first = recover(a + 1);
    assert_eq!(first, unproven);
    assert_eq!(
        recover(a + 1),
        first,
        "the same crash prefix recovers the same"
    );
    // Db2-R1 L2: later public lifecycle calls cross no ack or closure
    // boundary, and a second recovery of what they left finds the same.
    let after = scratch("h1-after");
    let mut env = env.to_vec();
    env.push(("M3D_DB_PREFIX", (a + 1).to_string()));
    env.push(("M3D_DB_OUT", after.display().to_string()));
    let out = run_child_with(CHILD, "h1-lifecycle", &env);
    let at = out.find("H1-LIFE ").expect("lifecycle");
    assert_eq!(
        out[at..].lines().next().unwrap(),
        format!(
            "H1-LIFE into=ok stop=Ok(Failed) notice=Ok(Some({WRITTEN:?})) reason=Ok(Some(\"journal_failed\")) close=err closures=0 acks=0"
        )
    );
    let left = load_lines(&after).len();
    let again = run_child_with(
        CHILD,
        "h1-recover",
        &[
            ("M3D_DB_DIR", after.display().to_string()),
            ("M3D_DB_PREFIX", left.to_string()),
        ],
    );
    let at = again.find("H1-OBS ").expect("second recovery");
    assert_eq!(again[at..].lines().next().unwrap(), first);
    for n in a + 2..=closed {
        let proven = recover(n);
        assert!(
            proven.ends_with("closures=1 acks=1"),
            "prefix {n}: {proven}"
        );
    }
}

/// Db-R1 M2: the settlement receipt append fails (ruling 7 handling).
#[test]
fn m3d_settlement_receipt_append_failure_reports_written() {
    run_child(CHILD, "m2-stop");
}

#[test]
fn m3d_replay_rejects_relinked_or_out_of_order_proof() {
    run_child(CHILD, "dr1");
    // Db2-R1 L3: the early-publication history (receipt with no journaled
    // pending action). Relinking only its receipt to another valid identity
    // must not replay.
    let dir = scratch("l3");
    let env = [("M3D_DB_DIR", dir.display().to_string())];
    run_child_with(
        "adapter::m3d_publication_oracles::m3d_db_broker_child",
        "l4-early",
        &env,
    );
    let records: Records = crate::journal::verify(&load_lines(&dir))
        .unwrap()
        .into_iter()
        .map(|e| (e.mission, e.event))
        .collect();
    let p = records.iter().position(|r| is_receipt(&r.1)).unwrap();
    assert!(inspect_history_lines(&rechain(&records)).is_ok());
    let relinked: [(&str, fn(&mut PendingAction)); 2] = [
        ("early receipt of another commitment", |a| {
            a.commitment = CMT.into()
        }),
        ("early receipt of another run", |a| a.run = RUN.into()),
    ];
    for (name, edit) in relinked {
        let mut changed = records.clone();
        if let Event::PublicationReceipt { action, .. } = &mut changed[p].1 {
            edit(action);
        }
        assert_ne!(changed, records, "{name}");
        assert!(
            matches!(
                inspect_history_lines(&rechain(&changed)),
                Err(JournalError::Inconsistent {
                    code: ReplayViolation::ProofConflict,
                    ..
                })
            ),
            "{name}"
        );
    }
}

type Records = Vec<(MissionId, Event)>;

type Opened = (
    Result<
        crate::coordinator::Coordinator<crate::adapter::GovernedPort>,
        crate::coordinator::CoordinatorError,
    >,
    crate::journal::MemoryJournal,
    std::sync::Arc<crate::adapter::GovernedPort>,
    crate::adapter::m2c_support::Home,
);

/// A fresh broker recovers the saved journal's first `M3D_DB_PREFIX` lines;
/// `h1-lifecycle` then drives the public lifecycle and saves what it left.
fn recover_prefix(dir: &std::path::Path, lifecycle: bool) {
    let n: usize = std::env::var("M3D_DB_PREFIX").unwrap().parse().unwrap();
    let (opened, journal, port, _home) = reopen(load_lines(dir)[..n].to_vec());
    if !lifecycle {
        observe(&opened, &journal, &port, n);
        return;
    }
    let id = MissionId(1);
    let s = opened.unwrap().into_file_session();
    let seen = match &s {
        Ok(s) => format!(
            "into=ok stop={:?} notice={:?} reason={:?} close={}",
            s.stop(id),
            s.notice(id),
            s.reason(id),
            if s.close().is_err() { "err" } else { "ok" }
        ),
        Err(e) => format!("into=err {e:?}"),
    };
    let lines = journal.lines();
    let added: Vec<Event> = crate::journal::verify(&lines).unwrap()[n..]
        .iter()
        .map(|e| e.event.clone())
        .collect();
    let closures = count(&added, |e| matches!(e, Event::ApprovalClosed { .. }));
    println!(
        "H1-LIFE {seen} closures={closures} acks={}",
        port.acks_tried()
    );
    save_lines(
        std::path::Path::new(&std::env::var("M3D_DB_OUT").unwrap()),
        &lines,
    );
}

fn reopen(lines: Vec<String>) -> Opened {
    use crate::adapter::m2c_support::{port_for, Home};
    let journal = crate::journal::MemoryJournal::new();
    journal.set_lines(lines);
    let home = Home::fresh_temp("m3db-h1").unwrap();
    let port = std::sync::Arc::new(port_for(&home.control, true));
    let opened = crate::coordinator::Coordinator::open(
        journal.clone(),
        port.clone(),
        crate::coordinator::Lanes::new(crate::oracles::keyed_lanes()).unwrap(),
        std::sync::Arc::new(nexus_governed_control::authority::clock::SystemClock::default()),
        home.private(),
    );
    (opened, journal, port, home)
}

fn observe(
    opened: &Result<
        crate::coordinator::Coordinator<crate::adapter::GovernedPort>,
        crate::coordinator::CoordinatorError,
    >,
    journal: &crate::journal::MemoryJournal,
    port: &crate::adapter::GovernedPort,
    n: usize,
) {
    let added: Vec<Event> = crate::journal::verify(&journal.lines()).unwrap()[n..]
        .iter()
        .map(|e| e.event.clone())
        .collect();
    let closures = count(&added, |e| matches!(e, Event::ApprovalClosed { .. }));
    let id = MissionId(1);
    let seen = match opened {
        Ok(c) => format!(
            "open=ok state={:?} reason={:?} notice={:?} close={}",
            c.state(id),
            c.reason(id),
            c.file_notice(id),
            if c.close_ref().is_err() { "err" } else { "ok" }
        ),
        Err(_) => "open=err".into(),
    };
    println!(
        "H1-OBS {seen} closures={closures} acks={}",
        port.acks_tried()
    );
}

/// `event` with its JSON field `key` set to `value`.
fn with_json(event: &Event, key: &str, value: serde_json::Value) -> Event {
    let mut v = serde_json::to_value(event).unwrap();
    v[key] = value;
    serde_json::from_value(v).unwrap()
}

fn op(edit: impl Fn(&mut Records) + 'static) -> Box<dyn Fn(&mut Records)> {
    Box::new(edit)
}
fn field(at: usize, edit: fn(&mut Event)) -> Box<dyn Fn(&mut Records)> {
    op(move |v: &mut Records| edit(&mut v[at].1))
}

/// Re-chain `records` from genesis (an adversary with write access).
fn rechain(records: &[(MissionId, Event)]) -> Vec<String> {
    let mut prev = GENESIS.to_string();
    records
        .iter()
        .enumerate()
        .map(|(seq, (mission, event))| {
            let entry = Entry::chained(seq as u64, &prev, *mission, event.clone());
            prev = entry.hash.clone();
            entry.to_line()
        })
        .collect()
}

#[test]
fn m3d_db_internal_child() {
    let Some(case) = crate::adapter::m3d_publication_oracles::child_case() else {
        return;
    };
    let dir = std::env::var("M3D_DB_DIR").map(std::path::PathBuf::from);
    if case == "h1-recover" || case == "h1-lifecycle" {
        recover_prefix(&dir.unwrap(), case == "h1-lifecycle");
        println!("m3d-db-receipt:{case}");
        return;
    }
    let f = PageFx::new();
    if case == "h1-publish" || case == "m2-stop" {
        let m = if case == "m2-stop" {
            let needle = "\"event\":\"publication_receipt\"";
            f.session(
                vec![file("index.html", BAKERY)],
                FailOn {
                    memory: f.journal.clone(),
                    needle,
                },
            )
        } else {
            f.session(vec![file("index.html", BAKERY)], f.journal.clone())
        };
        let id = f.waiting(&m);
        f.publish(id);
        if case == "h1-publish" {
            m.advance(id).unwrap();
            save_lines(&dir.unwrap(), &f.journal.lines());
        } else {
            assert_eq!(m.stop(id), Ok(MissionState::Failed));
            // Db2-R1 M1: the public, read-only observation; asked twice, it
            // never retries settlement against the poisoned journal.
            let lines = f.journal.lines().len();
            for _ in 0..2 {
                assert_eq!(m.notice(id), Ok(Some(WRITTEN)));
                assert_eq!(m.reason(id), Ok(Some("journal_failed".into())));
            }
            assert_eq!(f.journal.lines().len(), lines);
            let events = f.events();
            assert_eq!(count(&events, is_receipt), 0);
            let ended = |e: &Event| {
                matches!(e, Event::ApprovalClosed { .. })
                    || matches!(
                        e,
                        Event::Transition {
                            trigger: Trigger::Settled { .. }
                        }
                    )
            };
            assert_eq!(
                count(&events, ended),
                0,
                "no closure and no unproven terminal"
            );
            assert_eq!(f.port.acks_tried(), 0);
            assert!(f.journal.is_poisoned());
            assert_eq!(f.written().unwrap(), BAKERY.as_bytes());
        }
        println!("m3d-db-receipt:{case}");
        return;
    }
    let m = f.session(vec![file("index.html", BAKERY)], f.journal.clone());
    let id = f.waiting(&m);
    if let Some(kind) = case.strip_prefix("o33b-") {
        let alt = other_page();
        let substitute = f
            .home
            .private()
            .put(ContentKind::Deliverable, alt.as_bytes());
        {
            let mut inner = m.coordinator.inner.lock().unwrap();
            let mission = inner.missions.get_mut(&id).unwrap();
            let r = mission.proof.retained.as_mut().expect("retained");
            match kind {
                "removed" => f.alter_blob(&r.content, None),
                "changed" => f.alter_blob(&r.content, Some(alt.as_bytes())),
                "substituted" => r.content = substitute.unwrap(),
                "digest" => r.sha256 = sha(alt.as_bytes()),
                _ => r.bytes += 1,
            }
        }
        f.publish(id);
        let r = m.advance(id).unwrap();
        assert_eq!(r.to, MissionState::Stopped);
        assert_eq!(r.explanation.as_deref(), Some(WRITTEN));
        assert_eq!(f.written().unwrap(), BAKERY.as_bytes(), "still published");
        let events = f.events();
        let approved = at(&events, is_approved).expect("the publication is recorded");
        assert_eq!(count(&events, is_receipt), 0);
        assert_eq!(
            lanes_after(&events, approved),
            0,
            "no regeneration, no verifier"
        );
        assert_eq!(count(&events, is_dispatch), 1);
        assert!(events.contains(&Event::Transition {
            trigger: Trigger::Stop {
                reason: StopReason::PrivateContentUnavailable
            }
        }));
        assert_eq!(m.state(id), Ok(Some(MissionState::Stopped)));
    } else {
        assert_eq!(case, "dr1");
        f.publish(id);
        m.advance(id).unwrap();
        let records: Records = f
            .entries()
            .into_iter()
            .map(|e| (e.mission, e.event))
            .collect();
        let events: Vec<Event> = records.iter().map(|r| r.1.clone()).collect();
        let r = at(&events, is_retained).expect("retention");
        let p = at(&events, is_receipt).expect("receipt");
        let closed = at(&events, |e| matches!(e, Event::ApprovalClosed { .. })).unwrap();
        assert!(inspect_history_lines(&f.journal.lines()).is_ok());
        assert!(inspect_history_lines(&rechain(&records[..=p])).is_ok());
        let code = |edit: &dyn Fn(&mut Records)| {
            let mut changed = records.clone();
            edit(&mut changed);
            match inspect_history_lines(&rechain(&changed)) {
                Err(JournalError::Inconsistent { code, .. }) => Some(code),
                _ => None,
            }
        };
        use ReplayViolation::{NotEvidence, ProofConflict, ProofOutOfPlace};
        type Case = (&'static str, ReplayViolation, Box<dyn Fn(&mut Records)>);
        let cases: Vec<Case> = vec![
            (
                "receipt before retention",
                ProofOutOfPlace,
                op(move |v: &mut Records| {
                    let x = v.remove(p);
                    v.insert(r, x);
                }),
            ),
            (
                "retention after dispatch",
                ProofOutOfPlace,
                op(move |v: &mut Records| v.swap(r, r + 1)),
            ),
            (
                "second retention",
                ProofConflict,
                op(move |v: &mut Records| {
                    let x = v[r].clone();
                    v.insert(r + 1, x);
                }),
            ),
            (
                "second receipt",
                ProofConflict,
                op(move |v: &mut Records| {
                    let x = v[p].clone();
                    v.insert(p + 1, x);
                }),
            ),
            (
                "receipt after closure",
                ProofConflict,
                op(move |v: &mut Records| {
                    let x = v.remove(p);
                    v.insert(closed, x);
                }),
            ),
            (
                "receipt of another action",
                ProofConflict,
                field(p, |e| {
                    if let Event::PublicationReceipt { action, .. } = e {
                        action.commitment = CMT.into();
                    }
                }),
            ),
            (
                "receipt of another effect",
                ProofOutOfPlace,
                field(p, |e| {
                    if let Event::PublicationReceipt { effect, .. } = e {
                        *effect += 1;
                    }
                }),
            ),
            (
                "retention of another effect",
                ProofOutOfPlace,
                field(r, |e| {
                    if let Event::DeliverableRetained { effect, .. } = e {
                        *effect += 1;
                    }
                }),
            ),
            (
                "retention of another invocation",
                ProofOutOfPlace,
                field(r, |e| {
                    if let Event::DeliverableRetained { invocation, .. } = e {
                        invocation.seq -= 1;
                    }
                }),
            ),
            (
                "retention of another mission",
                ProofOutOfPlace,
                field(r, |e| {
                    if let Event::DeliverableRetained { invocation, .. } = e {
                        invocation.mission = MissionId(9);
                    }
                }),
            ),
            (
                "retention digest",
                NotEvidence,
                field(r, |e| {
                    if let Event::DeliverableRetained { sha256, .. } = e {
                        *sha256 = "zz".into();
                    }
                }),
            ),
            (
                "receipt policy",
                NotEvidence,
                field(p, |e| *e = with_json(e, "policy_hash", "".into())),
            ),
            (
                "clean receipt without policy",
                ProofOutOfPlace,
                field(p, |e| {
                    let mut v = serde_json::to_value(&*e).unwrap();
                    v.as_object_mut().unwrap().remove("policy_hash");
                    *e = serde_json::from_value(v).expect("a receipt may lack a policy");
                }),
            ),
        ];
        for (name, expected, edit) in cases {
            assert_eq!(code(&*edit), Some(expected), "{name}");
        }
    }
    println!("m3d-db-receipt:{case}");
}
