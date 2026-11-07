//! M3-Db oracles frozen before implementation. A child of the proof module
//! only so that one control can alter a live mission's retained reference;
//! everything else goes through the public lifecycle or the journal.
use crate::adapter::m3d_publication_oracles::{
    at, count, is_approved, is_dispatch, is_receipt, is_retained, lanes_after, other_page,
    run_child, sha, PageFx, BAKERY, WRITTEN,
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
        policy_hash: "ab".repeat(32),
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
    bad(&|e| {
        if let Event::PublicationReceipt {
            policy_hash,
            action,
            ..
        } = e
        {
            *policy_hash = "Lemon drizzle".into();
            action.run = "run-x".into();
        }
    });
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

#[test]
fn m3d_replay_rejects_relinked_or_out_of_order_proof() {
    run_child(CHILD, "dr1");
}

type Records = Vec<(MissionId, Event)>;

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
    let f = PageFx::new();
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
                field(p, |e| {
                    if let Event::PublicationReceipt { policy_hash, .. } = e {
                        policy_hash.clear();
                    }
                }),
            ),
        ];
        for (name, expected, edit) in cases {
            assert_eq!(code(&*edit), Some(expected), "{name}");
        }
    }
    println!("m3d-db-receipt:{case}");
}
