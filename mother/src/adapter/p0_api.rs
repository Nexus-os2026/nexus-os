//! P0-A oracles against the designed API (design §3 I1 to I4, §5.2, §5.4:
//! P0-O8, O9, O19, O23, and the private store of I2). Pre-registered with
//! `p0_privacy.rs` in commit 1 and not edited after a result is seen. The
//! API they name does not exist on main, so this file is declared only in
//! the implementation commit, which does not change it.

use super::p0_support::*;
use super::FilePrivate;
use crate::contract::{ContractError, EffectKind, Question};
use crate::coordinator::CoordinatorError;
use crate::effect::PendingAction;
use crate::evidence::{
    self, BlockReason, BrokerClass, ConsentRefusedReason, FailReason, HeldReason, LapseReason,
    MotherRefusal, OwnerView, SettledOutcome, StopReason, VerifyFailReason,
};
use crate::journal::{
    verify, ContractRef, Entry, Event, JournalError, MemoryJournal, MissionId, ReplayViolation,
    GENESIS, SESSION,
};
use crate::lane::{InvocationId, LaneRole};
use crate::oracles::{contract_with, RecordingPort};
use crate::private::{ContentId, ContentKind, MemoryPrivate, PrivateError, PrivateStore};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::clock::ManualClock;
use nexus_governed_control::authority::AuthorityError;
use std::sync::Arc;

#[derive(Default)]
struct Shown(Vec<(String, String)>);

impl OwnerView for Shown {
    fn line(&mut self, label: &str, value: &str) {
        self.0.push((label.to_string(), value.to_string()));
    }
}

impl Shown {
    fn holds(&self, text: &str) -> bool {
        self.0.iter().any(|(_, v)| v.contains(text))
    }
}

fn no_effects(_: MissionId) -> Vec<crate::effect::EffectRequest> {
    vec![]
}

// ---------------------------------------------------------------- P0-O8

/// P0-O8: a v1 journal is `Legacy` (the first line), in memory and on
/// disk, and nothing is written to it.
#[test]
fn p0_o8_a_v1_journal_is_legacy() {
    let lines: Vec<String> = V1_JOURNAL.lines().map(str::to_string).collect();
    assert_eq!(verify(&lines), Err(JournalError::Legacy { line: 1 }));

    let memory = MemoryJournal::new();
    memory.set_lines(lines.clone());
    let opened = crate::coordinator::Coordinator::open(
        memory.clone(),
        Arc::new(RecordingPort::default()),
        crate::coordinator::Lanes::new(crate::oracles::keyed_lanes()).unwrap(),
        Arc::new(ManualClock::default()),
        MemoryPrivate::new(),
    );
    assert!(matches!(
        opened,
        Err(CoordinatorError::Journal(JournalError::Legacy { line: 1 }))
    ));
    assert_eq!(memory.lines(), lines);
    assert!(!memory.is_poisoned());

    let w = P0World::new("o8-legacy", p0_lanes("r", no_effects));
    std::fs::write(w.journal_path(), V1_JOURNAL).unwrap();
    assert!(matches!(
        w.open(),
        Err(CoordinatorError::Journal(JournalError::Legacy { line: 1 }))
    ));
    assert_eq!(
        std::fs::read_to_string(w.journal_path()).unwrap(),
        V1_JOURNAL
    );
}

// ---------------------------------------------------------------- P0-O9

/// P0-O9: every string-typed field of every `Event` and `Trigger` (the
/// digests and the broker ids; every other field is a closed type) with
/// `C` or `Cl` in it is refused by `evidence::check`; the same fields in
/// their right form pass.
#[test]
fn p0_o9_every_string_field_is_checked() {
    let cs = canaries();
    let digest = "ab".repeat(32);
    let invocation = InvocationId {
        mission: MissionId(1),
        seq: 4,
    };
    let good = action(1);
    let events = |bad: &str| -> Vec<Event> {
        vec![
            Event::LaneInvoked {
                invocation,
                role: LaneRole::Builder,
                identity_sha256: bad.to_string(),
                input_hash: digest.clone(),
                model_cap: 0,
                model_budget: 0,
            },
            Event::LaneInvoked {
                invocation,
                role: LaneRole::Builder,
                identity_sha256: digest.clone(),
                input_hash: bad.to_string(),
                model_cap: 0,
                model_budget: 0,
            },
            Event::LaneReturned {
                invocation,
                output_hash: bad.to_string(),
            },
            Event::EffectDispatched {
                effect: 0,
                kind: EffectKind::Network,
                request_hash: bad.to_string(),
            },
            Event::ApprovalPending {
                effect: 0,
                pending: PendingAction {
                    run: bad.to_string(),
                    ..good.clone()
                },
            },
            Event::ApprovalPending {
                effect: 0,
                pending: PendingAction {
                    commitment: bad.to_string(),
                    ..good.clone()
                },
            },
        ]
    };
    for (_, bad) in cs.both() {
        for (index, event) in events(bad).iter().enumerate() {
            assert!(evidence::check(event).is_err(), "O9 field {index}: {bad}");
        }
        // A digest of the right length that is not one, and an upper-case
        // digest (Mother writes lower case only).
        for bad in [&format!("{:0<64}", &bad[..9]), &digest.to_uppercase()] {
            for event in &events(bad)[..4] {
                assert!(evidence::check(event).is_err(), "O9: {bad}");
            }
        }
    }
    for event in events(&digest)[..4].iter() {
        assert_eq!(evidence::check(event), Ok(()));
    }
    for event in [
        Event::ApprovalPending {
            effect: 0,
            pending: good.clone(),
        },
        Event::SessionOpened,
        Event::Held {
            reason: HeldReason::UncleanShutdown,
        },
        Event::Transition {
            trigger: Trigger::Stop {
                reason: StopReason::Owner,
            },
        },
    ] {
        assert_eq!(evidence::check(&event), Ok(()), "{event:?}");
    }
}

/// P0-O9: a hand-chained entry that the check refuses is refused at replay
/// (`Inconsistent`, its line and code), and an entry with a canary where a
/// closed code belongs does not even parse (`Malformed`).
#[test]
fn p0_o9_a_hand_chained_violation_is_refused_at_replay() {
    let cs = canaries();
    let w = P0World::new("o9-replay", p0_lanes("r", no_effects));
    let private = FilePrivate::open(&w.private_dir()).unwrap();
    let c = contract_with("o9");
    let bytes = serde_json::to_vec(&c).unwrap();
    let content = private.put(ContentKind::Contract, &bytes).unwrap();
    let drafted = Event::Drafted {
        contract: ContractRef {
            contract_hash: c.hash(),
            content,
            bytes: bytes.len() as u64,
        },
    };
    let m = MissionId(1);
    let t = |trigger| (m, Event::Transition { trigger });
    let mut events = vec![
        (SESSION, Event::SessionOpened),
        (m, drafted),
        t(Trigger::Submit { hash: c.hash() }),
        t(Trigger::Consent {
            hash: c.hash(),
            at_wall_ms: 1,
        }),
        t(Trigger::Start),
    ];
    let chain = |events: &[(MissionId, Event)]| {
        let mut prev = GENESIS.to_string();
        events
            .iter()
            .enumerate()
            .map(|(seq, (mission, event))| {
                let entry = Entry::chained(seq as u64, &prev, *mission, event.clone());
                prev = entry.hash.clone();
                entry.to_line()
            })
            .collect::<Vec<String>>()
    };
    let write = |lines: &[String]| {
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(w.journal_path(), text).unwrap();
    };
    // The control: this journal opens.
    write(&chain(&events));
    assert!(w.open().is_ok());
    events.push((
        m,
        Event::LaneInvoked {
            invocation: InvocationId { mission: m, seq: 5 },
            role: LaneRole::Sentinel,
            identity_sha256: "ab".repeat(32),
            input_hash: cs.cl.clone(),
            model_cap: 0,
            model_budget: 0,
        },
    ));
    write(&chain(&events));
    match w.open() {
        Err(CoordinatorError::Journal(JournalError::Inconsistent { line, code })) => {
            assert_eq!(line, 6);
            assert_eq!(code, ReplayViolation::NotEvidence);
        }
        other => panic!("O9: a hand-chained violation replayed: {:?}", other.err()),
    }

    let line = format!(
        r#"{{"format":2,"seq":0,"prev":"{GENESIS}","mission":1,"event":{{"event":"transition","trigger":{{"trigger":"stop","reason":"{}"}}}},"hash":"{}"}}"#,
        cs.cl,
        "0".repeat(64)
    );
    assert_eq!(verify(&[line]), Err(JournalError::Malformed { line: 1 }));
}

// ---------------------------------------------------------------- P0-O19

/// P0-O19: an unanswered question is named by its index; its id reaches the
/// Owner only through `WorkingDetail::show`, once, and the detail is
/// cleared by the mission's next call.
#[test]
fn p0_o19_the_question_is_an_index_and_its_id_is_working_detail() {
    let cs = canaries();
    let w = P0World::new("o19-detail", p0_lanes("r", no_effects));
    let mut c = contract_with("o19");
    c.questions.push(Question {
        id: cs.cl.clone(),
        text: "Which one?".into(),
        answer: None,
    });
    let id = w.draft(c.clone());
    assert_eq!(
        w.submit(id),
        Err(CoordinatorError::Contract(
            ContractError::UnansweredQuestion { index: 1 }
        ))
    );
    let mut shown = Shown::default();
    w.mother
        .working_detail(id)
        .expect("O19: the Owner's detail")
        .show(&mut shown);
    assert!(shown.holds(&cs.cl), "O19: the id is shown to the Owner");
    assert!(w.mother.working_detail(id).is_none(), "O19: taken once");

    assert!(w.submit(id).is_err());
    c.questions[1].answer = Some("This one".into());
    assert_eq!(w.mother.revise(id, c), Ok(MissionState::Draft));
    assert!(
        w.mother.working_detail(id).is_none(),
        "O19: cleared by the next call"
    );
    assert!(w.scan(&cs).is_empty(), "{:?}", w.scan(&cs));
}

/// P0-O19: the detail is bounded: 64 missions' details are kept, the
/// oldest dropped first.
#[test]
fn p0_o19_working_detail_is_bounded() {
    let w = P0World::new("o19-bound", p0_lanes("r", no_effects));
    let mut ids = Vec::new();
    for n in 0..65 {
        let mut c = contract_with(&format!("o19 bound {n}"));
        c.questions[0].answer = None;
        let id = w.draft(c);
        assert!(w.submit(id).is_err());
        ids.push(id);
    }
    assert!(w.mother.working_detail(ids[0]).is_none(), "O19: bounded");
    for id in &ids[1..] {
        assert!(w.mother.working_detail(*id).is_some(), "O19: {id}");
    }
}

/// P0-O19 and P0-O3: a lane's reason reaches the Owner only through
/// `WorkingDetail::show`.
#[test]
fn p0_o19_a_lane_reason_is_working_detail() {
    let cs = canaries();
    let reason = format!("why {}", cs.c);
    let w = P0World::new("o19-lane", p0_lanes(&reason, no_effects));
    let id = w.approved(contract_with("P0-HALT-SENTINEL"));
    w.advance(id).unwrap();
    assert_eq!(
        w.advance(id).map(|a| a.reason),
        Ok(Some("lane_halt:sentinel".to_string()))
    );
    let mut shown = Shown::default();
    w.mother
        .working_detail(id)
        .expect("O19: the lane's reason")
        .show(&mut shown);
    assert!(shown.holds(&cs.c));
    assert!(w.scan(&cs).is_empty(), "{:?}", w.scan(&cs));
}

// ---------------------------------------------------------------- P0-O23

fn round_trip<T: PartialEq + std::fmt::Debug + std::fmt::Display>(
    what: &str,
    every: Vec<T>,
    parse: fn(&str) -> Option<T>,
    inventory: &[&str],
    cs: &Canaries,
) {
    let codes: Vec<String> = every.iter().map(T::to_string).collect();
    for (value, code) in every.iter().zip(&codes) {
        assert_eq!(parse(code).as_ref(), Some(value), "{what}: {code}");
        assert!(code.is_ascii() && !code.is_empty(), "{what}: {code}");
    }
    for literal in inventory {
        assert!(
            codes.iter().any(|c| c == literal),
            "{what}: the coordinator's {literal} is not in the registry"
        );
    }
    for (_, canary) in cs.both() {
        assert_eq!(parse(canary), None, "{what}: {canary}");
        for prefix in [
            "lane_halt:",
            "lane_fail:",
            "broker_refused:",
            "approval_lapsed:",
            "refused:",
            "forbidden_effect:",
        ] {
            assert_eq!(parse(&format!("{prefix}{canary}")), None, "{what}");
        }
    }
}

/// P0-O23: every reason the coordinator can produce (its literal
/// inventory) is in the closed registry and parses back; `Cl` (shaped like
/// a code) parses as no code, so replay refuses it.
#[test]
fn p0_o23_every_code_round_trips_and_nothing_else_parses() {
    let cs = canaries();
    round_trip(
        "stop",
        StopReason::every(),
        StopReason::parse,
        &["owner", "emergency_stop", "private_content_unavailable"],
        &cs,
    );
    round_trip(
        "block",
        BlockReason::every(),
        BlockReason::parse,
        &[
            "wait_unknown",
            "approval_not_closed",
            "waited_rest_unreadable",
            "budget_time",
            "budget_model_calls",
            "lane_identity_changed",
            "too_many_effects",
            "journal_failed",
            "awaiting_owner_approval",
            "effect_outcome_unknown",
            "phase_interrupted",
            "lane_halt:sentinel",
            "lane_halt:builder",
            "broker_refused:no_covering_grant",
            "broker_refused:no_broker_route",
            "broker_refused:unknown",
            "forbidden_effect:network",
            "budget_money_zero",
            "spend_not_supported",
            "no_requested_folder",
            "file_request_not_relative",
        ],
        &cs,
    );
    round_trip(
        "fail",
        FailReason::every(),
        FailReason::parse,
        &["lane_fail:researcher_a", "lane_fail:sentinel"],
        &cs,
    );
    round_trip(
        "verify_fail",
        VerifyFailReason::every(),
        VerifyFailReason::parse,
        &["verifier_fail", "verifier_gave_no_verdict"],
        &cs,
    );
    round_trip(
        "lapse",
        LapseReason::every(),
        LapseReason::parse,
        &[
            "approval_expired",
            "approval_lapsed:failed",
            "approval_lapsed:port_fenced",
            "approval_lapsed:approval_outcome_unknown",
            "approval_lapsed:unknown",
            "budget_time",
            "journal_failed",
        ],
        &cs,
    );
    round_trip(
        "held",
        HeldReason::every(),
        HeldReason::parse,
        &["unclean_shutdown"],
        &cs,
    );
    round_trip(
        "consent_refused",
        ConsentRefusedReason::every(),
        ConsentRefusedReason::parse,
        &[
            "mission_changed",
            "another_request",
            "broker_refused:evidence_unavailable",
        ],
        &cs,
    );
    round_trip(
        "settled",
        SettledOutcome::every(),
        SettledOutcome::parse,
        &["done", "awaiting_approval", "refused:no_covering_grant"],
        &cs,
    );
    round_trip(
        "mother_refusal",
        MotherRefusal::every(),
        MotherRefusal::parse,
        &["forbidden_effect:spend", "budget_money_zero"],
        &cs,
    );
    round_trip(
        "broker_class",
        BrokerClass::every(),
        BrokerClass::parse,
        &[
            "port_fenced",
            "no_broker_route",
            "mission_cancelled",
            "action_not_held",
            "approval_outcome_unknown",
            "failed",
            "cancelled",
            "revoked",
            "unknown",
        ],
        &cs,
    );

    // Serialized through the code, and only from the closed set.
    assert_eq!(
        serde_json::to_value(StopReason::Owner).unwrap(),
        serde_json::json!("owner")
    );
    assert!(serde_json::from_value::<BlockReason>(serde_json::json!(cs.cl)).is_err());
    assert!(serde_json::from_value::<BrokerClass>(serde_json::json!(cs.cl)).is_err());
}

/// P0-O4 and P0-O23: `BrokerClass` mirrors every `AuthorityError::class()`
/// (plus the port's own and `unknown`); a class outside it is `Unknown`.
#[test]
fn p0_o23_broker_class_mirrors_every_authority_class() {
    let cs = canaries();
    let every = [
        AuthorityError::UnknownCommitment,
        AuthorityError::UnknownRun,
        AuthorityError::UnknownGrant,
        AuthorityError::UnknownLease,
        AuthorityError::WrongAgent,
        AuthorityError::WrongRun,
        AuthorityError::NotPending,
        AuthorityError::NotAuthorized,
        AuthorityError::Expired,
        AuthorityError::Stale,
        AuthorityError::GrantNotLive,
        AuthorityError::NoCoveringGrant,
        AuthorityError::RunCancelled,
        AuthorityError::RunNotActive,
        AuthorityError::EmergencyStopped,
        AuthorityError::ResumeSuperseded,
        AuthorityError::ApprovalRequired,
        AuthorityError::ApprovalNotApplicable,
        AuthorityError::ApprovalMismatch,
        AuthorityError::Declined,
        AuthorityError::TargetChanged,
        AuthorityError::ParametersChanged,
        AuthorityError::EvidenceUnavailable,
        AuthorityError::Capacity,
        AuthorityError::NotMember,
        AuthorityError::Halted,
        AuthorityError::Closed("x"),
        AuthorityError::InvalidAction("x"),
        AuthorityError::Unavailable("x"),
    ];
    for error in &every {
        let class = BrokerClass::of(error.class());
        assert_ne!(class, BrokerClass::Unknown, "{}", error.class());
        assert_eq!(class.to_string(), error.class());
    }
    for (_, canary) in cs.both() {
        assert_eq!(BrokerClass::of(canary), BrokerClass::Unknown);
        assert_eq!(BrokerClass::parse(canary), None);
    }
    assert_eq!(BrokerClass::parse("unknown"), Some(BrokerClass::Unknown));
}

// ---------------------------------------------------------------- I2, I3

/// I2: the private store is content addressed (`SHA-256("nexus.mother.
/// private.v1/<kind>" ‖ len ‖ bytes)`), keeps `<dir>/<kind>/<hex>` with
/// directories 0700 and files 0600, and `get` checks the content id.
#[test]
fn p0_i2_the_private_store_is_content_addressed_and_private() {
    let dir = fresh_dir("i2");
    let root = dir.join("private");
    let store = FilePrivate::open(&root).unwrap();
    let bytes = b"{\"words\":\"x\"}";
    let id = store.put(ContentKind::Contract, bytes).unwrap();
    let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
    sha2::Digest::update(&mut hasher, b"nexus.mother.private.v1/contract");
    sha2::Digest::update(&mut hasher, (bytes.len() as u64).to_be_bytes());
    sha2::Digest::update(&mut hasher, bytes);
    assert_eq!(id.to_hex(), hex::encode(sha2::Digest::finalize(hasher)));
    assert_eq!(id, ContentId::of(ContentKind::Contract, bytes));
    let path = root.join("contract").join(id.to_hex());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&root), 0o700);
        assert_eq!(mode(&root.join("contract")), 0o700);
        assert_eq!(mode(&path), 0o600);
    }
    assert_eq!(store.get(ContentKind::Contract, &id).unwrap(), bytes);
    assert_eq!(
        store.get(ContentKind::PausedPhase, &id),
        Err(PrivateError::Missing)
    );
    let rest = store.put(ContentKind::PausedPhase, bytes).unwrap();
    assert_ne!(rest, id, "the kind is in the content id");
    assert_eq!(
        std::fs::read_dir(root.join("contract")).unwrap().count(),
        1,
        "no temporary file is left"
    );
    std::fs::write(&path, b"{\"words\":\"y\"}").unwrap();
    assert_eq!(
        store.get(ContentKind::Contract, &id),
        Err(PrivateError::Mismatch)
    );
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        store.get(ContentKind::Contract, &id),
        Err(PrivateError::Missing)
    );
    #[cfg(unix)]
    {
        let link = dir.join("linked");
        std::os::unix::fs::symlink(&root, &link).unwrap();
        assert!(FilePrivate::open(&link).is_err(), "a link is refused");
    }

    let memory = MemoryPrivate::new();
    assert_eq!(memory.put(ContentKind::Contract, bytes).unwrap(), id);
    assert_eq!(memory.get(ContentKind::Contract, &id).unwrap(), bytes);
    assert_eq!(
        memory.get(ContentKind::PausedPhase, &id),
        Err(PrivateError::Missing)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// I3: the journal is format 2 and names a contract by reference: its hash
/// (recomputed from the known contract), its content id and its size.
#[test]
fn p0_i3_the_journal_names_content_by_reference() {
    let w = P0World::new("i3", p0_lanes("r", no_effects));
    let c = contract_with("i3");
    let id = w.draft(c.clone());
    let bytes = serde_json::to_vec(&c).unwrap();
    let values = w.journal_values();
    assert!(values.iter().all(|v| v["format"] == 2), "{values:?}");
    let drafted = values
        .iter()
        .find(|v| v["mission"].as_u64() == Some(id.0) && v["event"]["event"] == "drafted")
        .unwrap();
    assert_eq!(
        drafted["event"]["contract"],
        serde_json::json!({
            "contract_hash": c.hash().to_hex(),
            "content": ContentId::of(ContentKind::Contract, &bytes).to_hex(),
            "bytes": bytes.len(),
        })
    );
}
