//! M3-E M-6 oracles (rulings M3E-R1, R18; register
//! docs/security/m3-e-oracles.md): T16 is selected only from journaled
//! facts and promotes no evidence; the mission receipt is content-free,
//! joins its identities, and lists what is missing instead of a zero.
use crate::contract::{EffectKind, MissionContract, Note};
use crate::coordinator::page_display::{page_view, ViewError};
use crate::coordinator::page_receipt::{mission_receipt, MissionReceipt, PublicationFacts};
use crate::effect::PendingAction;
use crate::evidence::{LapseReason, SettlementEnd, SettlementOutcome, StopReason};
use crate::journal::{Entry, Event, MissionId, FORMAT};
use crate::lane::{InvocationId, LaneRole};
use crate::oracles::contract_with;
use crate::page_inputs::{page_criteria, PageFacts};
use crate::state::Trigger;
use crate::teaching::{t16, t16_failed, T16_NONE_RECORDED, T16_PASSED, T16_UNCHECKED};
use nexus_governed_control::inert_page::ApprovedFacts;

const ID: u64 = 9;
const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn page_contract() -> MissionContract {
    let mut c = contract_with("");
    c.requested_folder = Some("rosas-bakery".into());
    c.deliverable = Some("index.html".into());
    c.notes = vec![Note {
        label: "notes".into(),
        text: "NOTE_CANARY_RCPT Fresh bread every morning.".into(),
    }];
    c.page_facts = Some(PageFacts {
        name: "Rosa's Bakery".into(),
        hours: vec!["Tuesday to Sunday, 7am to 3pm".into()],
        phone: "555-0142".into(),
    });
    c.pass_criteria = page_criteria();
    c
}
fn entry(seq: u64, event: Event) -> Entry {
    Entry {
        format: FORMAT,
        seq,
        prev: String::new(),
        mission: MissionId(ID),
        event,
        hash: format!("{seq:064x}"),
    }
}
fn transition(seq: u64, trigger: Trigger) -> Entry {
    entry(seq, Event::Transition { trigger })
}
fn action() -> PendingAction {
    PendingAction {
        run: "run".into(),
        commitment: "commitment".into(),
        deadline_mono_ms: 1,
    }
}
/// Consented; the Builder invoked; the File dispatched and asked for.
fn asked(c: &MissionContract) -> Vec<Entry> {
    let invocation = InvocationId {
        mission: MissionId(ID),
        seq: 2,
    };
    vec![
        transition(
            1,
            Trigger::Consent {
                hash: c.hash(),
                at_wall_ms: 1000,
            },
        ),
        entry(
            2,
            Event::LaneInvoked {
                invocation,
                role: LaneRole::Builder,
                identity_sha256: String::new(),
                input_hash: String::new(),
                model_cap: 0,
                model_budget: 0,
            },
        ),
        entry(
            3,
            Event::ModelCallCharged {
                invocation,
                call: 1,
            },
        ),
        entry(
            4,
            Event::EffectDispatched {
                effect: 1,
                kind: EffectKind::File,
                request_hash: String::new(),
            },
        ),
        entry(
            5,
            Event::ApprovalPending {
                effect: 1,
                pending: action(),
            },
        ),
    ]
}
fn receipt(effect: u64, overtaken: bool) -> Entry {
    entry(
        6,
        Event::PublicationReceipt {
            effect,
            action: action(),
            sha256: SHA.into(),
            bytes: 1697,
            policy_hash: Some("bb".repeat(32)),
            overtaken,
        },
    )
}
fn recorded(effect: u64, codes: [&str; 6]) -> Entry {
    let checks = ["v0", "v1", "v2", "v3", "v4", "v5"]
        .iter()
        .zip(codes)
        .map(|(id, code)| (id.to_string(), code.to_string()))
        .collect();
    entry(
        7,
        Event::VerificationRecorded {
            invocation: InvocationId {
                mission: MissionId(ID),
                seq: 7,
            },
            effect,
            report_digest: "cc".repeat(32),
            checks,
        },
    )
}
fn summary(entries: &[Entry], c: &MissionContract) -> Result<Option<String>, ViewError> {
    page_view(entries, MissionId(ID)).summary(c)
}
const PASS: [&str; 6] = ["pass"; 6];
const ALL_PASS: &str = "v0 pass, v1 pass, v2 pass, v3 pass, v4 pass, v5 pass";

/// The frozen T16 text, byte for byte (mission U10 row, D8, R18 (3)).
const T16_GOLDEN: &str = "What happened: index.html was written and the second check passed.\nChecked by my code in this run: v0 pass, v1 pass, v2 pass, v3 pass, v4 pass, v5 pass.\nFor you to confirm: the page's other sentences and how it looks in your browser.\nOPEN: the 375 px and 1366 px views; the page's other sentences; no backup yet.\nThis run is a demonstration: it counts as OBSERVED when you confirm it, and CORROBORATED only after the records and identities are checked independently.";

#[test]
fn m3e_u10_summary_never_promotes_evidence() {
    let c = page_contract();
    let with = |extra: Vec<Entry>| [asked(&c), extra].concat();
    // All-pass: the golden.
    let pass = with(vec![
        receipt(1, false),
        recorded(1, PASS),
        transition(8, Trigger::VerifyPass),
    ]);
    let before = pass.clone();
    assert_eq!(summary(&pass, &c), Ok(Some(T16_GOLDEN.to_string())));
    assert_eq!(pass, before, "rendering changes nothing");
    // Failed: its first code; the recorded checks as recorded.
    let failed = with(vec![
        receipt(1, false),
        recorded(
            1,
            [
                "pass",
                "pass",
                "pass",
                "fail:name_not_heading",
                "pass",
                "pass",
            ],
        ),
    ]);
    let failed_checks = "v0 pass, v1 pass, v2 pass, v3 fail:name_not_heading, v4 pass, v5 pass";
    assert_eq!(
        summary(&failed, &c),
        Ok(Some(t16(&t16_failed("name_not_heading"), failed_checks)))
    );
    // Written, nothing recorded; a scripted VerifyPass is no check.
    let unrecorded = with(vec![receipt(1, false), transition(8, Trigger::VerifyPass)]);
    assert_eq!(
        summary(&unrecorded, &c),
        Ok(Some(t16(T16_UNCHECKED, T16_NONE_RECORDED)))
    );
    // Overtaken (post-publication Stop): no success.
    let overtaken = with(vec![receipt(1, true)]);
    let unconfirmed = "I could not confirm what became of the request";
    assert_eq!(
        summary(&overtaken, &c),
        Ok(Some(t16(unconfirmed, T16_NONE_RECORDED)))
    );
    // Not written: the recorded end, in its closed phrase.
    for (end, outcome) in [
        (
            Trigger::ActionDenied { effect: 1 },
            "you said no; nothing was written",
        ),
        (
            Trigger::ApprovalLapsed {
                effect: 1,
                reason: LapseReason::ApprovalExpired,
            },
            "the file dialog timed out; nothing was written",
        ),
        (
            Trigger::Settled {
                effect: 1,
                action: None,
                outcome: SettlementOutcome::NotPublished,
                end: SettlementEnd::Stop(StopReason::Owner),
            },
            "you stopped; nothing was written",
        ),
        (
            Trigger::Settled {
                effect: 1,
                action: None,
                outcome: SettlementOutcome::Uncertain,
                end: SettlementEnd::Stop(StopReason::Owner),
            },
            unconfirmed,
        ),
    ] {
        let j = with(vec![transition(8, end)]);
        assert_eq!(
            summary(&j, &c),
            Ok(Some(t16(outcome, T16_NONE_RECORDED))),
            "{outcome}"
        );
    }
    // Still waiting: no summary yet. Another contract: refused.
    assert_eq!(summary(&asked(&c), &c), Ok(None));
    let mut other = c.clone();
    other.restatement.push('!');
    assert_eq!(summary(&pass, &other), Err(ViewError::NotConsented));
    // Never a current OBSERVED/CORROBORATED label, never more than checked.
    for text in [T16_GOLDEN.to_string(), t16(T16_PASSED, ALL_PASS)] {
        for banned in [
            "is OBSERVED",
            "is CORROBORATED",
            "was OBSERVED",
            "was CORROBORATED",
        ] {
            assert!(!text.contains(banned), "{banned}");
        }
        assert_eq!(crate::teaching::claims_guard(&text), Ok(()));
    }
}

#[test]
fn m3e_u10_receipt_identities_join_and_missing_is_listed() {
    let c = page_contract();
    let hours = ["Tuesday to Sunday, 7am to 3pm"];
    let facts_hash = hex::encode(
        ApprovedFacts::new("Rosa's Bakery", &hours, "555-0142")
            .unwrap()
            .hash(),
    );
    let j = [
        asked(&c),
        vec![
            receipt(1, false),
            recorded(1, PASS),
            transition(8, Trigger::VerifyPass),
        ],
    ]
    .concat();
    let got = mission_receipt(&j, MissionId(ID), &c).unwrap();
    let expected = MissionReceipt {
        mission: MissionId(ID),
        contract_hash: c.hash().to_hex(),
        consent_seq: 1,
        consent_entry_hash: format!("{:064x}", 1),
        consent_at_wall_ms: 1000,
        facts_hash: Some(facts_hash),
        lane_calls: vec![
            ("researcher_a", 0),
            ("researcher_b", 0),
            ("skeptic", 0),
            ("builder", 1),
            ("verifier", 0),
            ("sentinel", 0),
        ],
        model_calls_charged: 1,
        effects_dispatched: 1,
        publication: Some(PublicationFacts {
            sha256: SHA.into(),
            bytes: 1697,
            policy_hash: Some("bb".repeat(32)),
            overtaken: false,
        }),
        checks: Some(
            ["v0", "v1", "v2", "v3", "v4", "v5"]
                .map(|id| (id.to_string(), "pass".to_string()))
                .to_vec(),
        ),
        report_digest: Some("cc".repeat(32)),
        end: "done",
        missing: vec![],
        mismatched: vec![],
    };
    assert_eq!(got, expected);
    // Repeated: identical; the journal unchanged.
    let before = j.clone();
    assert_eq!(mission_receipt(&j, MissionId(ID), &c), Ok(got));
    assert_eq!(j, before);
    // A receipt for another effect, a record for another: not joined.
    let j = [asked(&c), vec![receipt(2, false), recorded(3, PASS)]].concat();
    let got = mission_receipt(&j, MissionId(ID), &c).unwrap();
    assert_eq!(
        got.mismatched,
        vec!["receipt_effect", "verification_effect"]
    );
    // Published (approved) with no receipt journaled: missing, not none.
    let approved = Trigger::ActionApproved {
        effect: 1,
        phase: crate::state::MissionState::Building,
    };
    let j = [asked(&c), vec![transition(6, approved)]].concat();
    let got = mission_receipt(&j, MissionId(ID), &c).unwrap();
    assert_eq!(got.publication, None);
    assert_eq!(got.missing, vec!["publication_receipt"]);
    // Declined: no receipt and none missing.
    let j = [
        asked(&c),
        vec![transition(6, Trigger::ActionDenied { effect: 1 })],
    ]
    .concat();
    assert_eq!(
        mission_receipt(&j, MissionId(ID), &c).unwrap().missing,
        Vec::<&str>::new()
    );
    // Not consented, not a page.
    let mut other = c.clone();
    other.restatement.push('!');
    assert_eq!(
        mission_receipt(&j, MissionId(ID), &other),
        Err(ViewError::NotConsented)
    );
    assert_eq!(
        mission_receipt(&j, MissionId(ID), &contract_with("")),
        Err(ViewError::NotAPage)
    );
}

#[test]
fn m3e_u10_request_identity_is_the_frozen_bytes() {
    let id = crate::request_identity();
    assert_eq!(
        hex::encode(id.system),
        "ec1b50642f9c77a54318fcd94daa8ac8c7424cffda1181fd62d2735d4a55ede9"
    );
    assert_eq!(
        hex::encode(id.task),
        "d63a0249f74fcc32f2cca81894cb6beeb49b6d9d5b49b7a6db7187daa4211528"
    );
    assert_eq!(
        hex::encode(id.schema),
        "e7d163a2958ae6211fefaa0f69028915071654657e0fdb2e484c26e9fd8b0ce1"
    );
    let corrections: Vec<(&str, String)> = id
        .corrections
        .iter()
        .map(|(code, h)| (*code, hex::encode(h)))
        .collect();
    let frozen = [
        (
            "not_one_object",
            "91fe7622a5648a70e83e266de072c339bb9e1c73c6b2b1ef3b425ce786403815",
        ),
        (
            "draft_empty",
            "8bfeec91f7a2f4063efeed57207204ac7c3f8ae69ccfec0334a98cd2728611cf",
        ),
        (
            "draft_markup",
            "8af4ffb7eec9c9471496a19297d3e1d161a0d3e252923d0d48d2c452154c71ef",
        ),
        (
            "draft_char_not_allowed",
            "ccb0ce77d64791a25c5916b26d676369cf9c0b73931abf7fbe564540ef87aa6d",
        ),
        (
            "draft_word_too_long",
            "7b5f1cf4b144e9f1f0af5bdd2d9c439897503ce7d9dc9eaa967f27f7a361fdda",
        ),
        (
            "draft_too_large",
            "814eff225a89c3f1edd414e4859c9c2d01880d664bf3b91b234e228a433378f5",
        ),
        (
            "draft_foreign_number",
            "e7beb519c76889e04fb8873529da5a8747cbcca235b9f67cfba25be47e1c64c1",
        ),
        (
            "draft_email",
            "78cb9780f356bc924c1a163f1856c7b6f1dbee86a45a710ad2247d6770910c63",
        ),
    ];
    let frozen: Vec<(&str, String)> = frozen.iter().map(|(c, h)| (*c, h.to_string())).collect();
    assert_eq!(corrections, frozen);
}

/// A real broker page mission, run to Done (one broker per process).
const CHILD: &str = "oracles::m3e_receipts::m3e_u10_child";

#[cfg(target_os = "linux")]
#[test]
fn m3e_u10_receipts_from_a_real_page_mission() {
    crate::adapter::m3d_publication_oracles::run_child(CHILD, "done");
}

#[cfg(target_os = "linux")]
#[test]
fn m3e_u10_child() {
    use crate::adapter::m3d_publication_oracles::{child_case, leaks, sha, PageFx, BAKERY};
    use crate::coordinator::{Coordinator, Lanes};
    use crate::lane::{Lane, ScriptedLane, Verdict};
    use crate::state::MissionState;
    use nexus_governed_control::authority::clock::SystemClock;
    use std::sync::Arc;
    let Some(case) = child_case() else {
        return;
    };
    let f = PageFx::new();
    let mut lanes: Vec<Arc<dyn Lane>> = crate::oracles::keyed_lanes()
        .into_iter()
        .filter(|l| !matches!(l.identity().role, LaneRole::Builder | LaneRole::Verifier))
        .collect();
    let builder = ScriptedLane::new(LaneRole::Builder, "built", Verdict::Continue);
    let file = crate::oracles::lite_support::file("index.html", BAKERY);
    lanes.push(Arc::new(builder.asking(vec![file])));
    lanes.push(Arc::new(crate::page_verifier::PageVerifierLane));
    let m = Coordinator::open(
        f.journal.clone(),
        f.port.clone(),
        Lanes::new(lanes).unwrap(),
        Arc::new(SystemClock::default()),
        f.home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    let id = f.waiting(&m);
    f.publish(id);
    for _ in 0..3 {
        m.advance(id).unwrap();
    }
    assert_eq!(m.state(id), Ok(Some(MissionState::Done)));
    let c = f.contract();
    let view = m.journal_view().unwrap();
    let got = mission_receipt(&view, id, &c).unwrap();
    let publication = got.publication.clone().expect("the broker's receipt");
    assert_eq!(publication.sha256, sha(BAKERY.as_bytes()));
    assert_eq!(publication.bytes, BAKERY.len() as u64);
    assert!(!publication.overtaken);
    let all_pass =
        ["v0", "v1", "v2", "v3", "v4", "v5"].map(|id| (id.to_string(), "pass".to_string()));
    assert_eq!(got.checks.as_deref(), Some(&all_pass[..]));
    let facts = c.page_facts.as_ref().unwrap();
    let hours: Vec<&str> = facts.hours.iter().map(String::as_str).collect();
    let hash = ApprovedFacts::new(&facts.name, &hours, &facts.phone)
        .unwrap()
        .hash();
    assert_eq!(got.facts_hash, Some(hex::encode(hash)));
    assert_eq!(got.contract_hash, c.hash().to_hex());
    let verifier = got.lane_calls.iter().find(|(r, _)| *r == "verifier");
    assert_eq!(verifier, Some(&("verifier", 1)));
    assert_eq!(
        (got.end, got.missing.len(), got.mismatched.len()),
        ("done", 0, 0)
    );
    // Content-free: no page, fact or note canary in the receipt or its Debug.
    assert_eq!(leaks(&format!("{got:?}")), Vec::<&str>::new());
    // Repeated: identical, and the journal unchanged.
    assert_eq!(mission_receipt(&m.journal_view().unwrap(), id, &c), Ok(got));
    assert_eq!(m.journal_view().unwrap(), view);
    assert_eq!(
        page_view(&view, id).summary(&c),
        Ok(Some(t16(T16_PASSED, ALL_PASS)))
    );
    println!("m3d-db-receipt:{case}");
}
