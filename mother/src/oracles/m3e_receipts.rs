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
        invalid: vec![],
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

// M3E-R19: confirmed vs unavailable publication evidence, the action link,
// joined evidence for T16, and value domains.
fn receipt_with(effect: u64, sha256: &str, policy: Option<&str>, act: PendingAction) -> Entry {
    entry(
        6,
        Event::PublicationReceipt {
            effect,
            action: act,
            sha256: sha256.into(),
            bytes: 1697,
            policy_hash: policy.map(str::to_string),
            overtaken: false,
        },
    )
}
fn r(j: &[Entry], c: &MissionContract) -> MissionReceipt {
    mission_receipt(j, MissionId(ID), c).unwrap()
}
fn settled(outcome: SettlementOutcome, act: Option<PendingAction>) -> Entry {
    transition(
        8,
        Trigger::Settled {
            effect: 1,
            action: act,
            outcome,
            end: SettlementEnd::Stop(StopReason::Owner),
        },
    )
}

#[test]
fn m3e_u10_r19_unavailable_publication_is_missing() {
    let c = page_contract();
    let none: Vec<&str> = vec![];
    // Unavailable: an Uncertain settlement with no receipt; an absent policy.
    let j = [asked(&c), vec![settled(SettlementOutcome::Uncertain, None)]].concat();
    assert_eq!(r(&j, &c).missing, vec!["publication_receipt"]);
    let j = [asked(&c), vec![receipt_with(1, SHA, None, action())]].concat();
    assert_eq!(r(&j, &c).missing, vec!["publication_policy"]);
    // Confirmed non-publication: nothing missing.
    let expired = Trigger::ApprovalLapsed {
        effect: 1,
        reason: LapseReason::ApprovalExpired,
    };
    for end in [
        transition(8, Trigger::ActionDenied { effect: 1 }),
        transition(8, expired),
    ] {
        assert_eq!(r(&[asked(&c), vec![end]].concat(), &c).missing, none);
    }
    let j = [
        asked(&c),
        vec![settled(SettlementOutcome::NotPublished, None)],
    ]
    .concat();
    assert_eq!(r(&j, &c).missing, none);
    let stop = transition(
        2,
        Trigger::Stop {
            reason: StopReason::Owner,
        },
    );
    assert_eq!(
        r(&[asked(&c)[..1].to_vec(), vec![stop]].concat(), &c).missing,
        none
    );
}

#[test]
fn m3e_u10_r19_every_receipt_needs_its_action_link() {
    let c = page_contract();
    let other = PendingAction {
        run: "run".into(),
        commitment: "another".into(),
        deadline_mono_ms: 1,
    };
    // No pending and no settlement identity for the receipt's effect.
    let mut j = asked(&c);
    j.retain(|e| !matches!(e.event, Event::ApprovalPending { .. }));
    j.push(receipt(1, false));
    assert_eq!(r(&j, &c).missing, vec!["action-link"]);
    // A pending for another effect only.
    let mut j = asked(&c);
    if let Event::ApprovalPending { effect, .. } = &mut j[4].event {
        *effect = 2;
    }
    j.push(receipt(1, false));
    assert_eq!(r(&j, &c).missing, vec!["action-link"]);
    // The pending names another action.
    let j = [
        asked(&c),
        vec![receipt_with(1, SHA, Some(&"bb".repeat(32)), other)],
    ]
    .concat();
    assert_eq!(r(&j, &c).mismatched, vec!["action"]);
    // A settlement identity alone links it (early settlement).
    let mut j = asked(&c);
    j.retain(|e| !matches!(e.event, Event::ApprovalPending { .. }));
    j.push(receipt(1, false));
    let published = SettlementOutcome::Published {
        recovery_required: false,
        interrupted: vec![],
    };
    j.push(settled(published, Some(action())));
    let got = r(&j, &c);
    assert_eq!((got.missing, got.mismatched), (vec![], vec![]));
}

#[test]
fn m3e_u10_r19_summary_success_only_on_joined_evidence() {
    let c = page_contract();
    // All-pass checks recorded for another effect: no success phrase.
    let j = [asked(&c), vec![receipt(1, false), recorded(2, PASS)]].concat();
    assert_eq!(
        summary(&j, &c),
        Ok(Some(t16(T16_UNCHECKED, T16_NONE_RECORDED)))
    );
    // A receipt for another effect than the File's: unconfirmed.
    let j = [asked(&c), vec![receipt(2, false), recorded(2, PASS)]].concat();
    let unconfirmed = "I could not confirm what became of the request";
    assert_eq!(
        summary(&j, &c),
        Ok(Some(t16(unconfirmed, T16_NONE_RECORDED)))
    );
}

#[test]
fn m3e_u10_r19_values_are_validated_before_projection() {
    let c = page_contract();
    let canary = "PRIVATE_CANARY_R19 page text";
    let bad = |j: Vec<Entry>, field: &str| {
        let got = r(&j, &c);
        assert_eq!(got.invalid, vec![field], "{field}");
        assert!(
            !format!("{got:?}").contains("PRIVATE_CANARY_R19"),
            "{field}"
        );
        let shown = summary(&j, &c).unwrap().unwrap_or_default();
        assert!(!shown.contains("PRIVATE_CANARY_R19"), "{field}");
        if field != "report_digest" {
            // An invalid receipt or check record is no passed check.
            assert!(!shown.contains(T16_PASSED), "{field}");
        }
        got
    };
    let got = bad(
        [
            asked(&c),
            vec![receipt_with(1, canary, Some(&"bb".repeat(32)), action())],
        ]
        .concat(),
        "receipt_sha256",
    );
    assert_eq!(got.publication, None);
    bad(
        [
            asked(&c),
            vec![receipt_with(1, SHA, Some(canary), action())],
        ]
        .concat(),
        "receipt_policy_hash",
    );
    let mut checks = recorded(
        1,
        [
            "pass",
            "pass",
            "pass",
            "fail:PRIVATE_CANARY_R19",
            "pass",
            "pass",
        ],
    );
    let got = bad(
        [asked(&c), vec![receipt(1, false), checks.clone()]].concat(),
        "checks",
    );
    assert_eq!(got.checks, None);
    if let Event::VerificationRecorded { report_digest, .. } = &mut checks.event {
        *report_digest = canary.into();
    }
    if let Event::VerificationRecorded { checks: list, .. } = &mut checks.event {
        list[3].1 = "pass".into();
    }
    bad(
        [asked(&c), vec![receipt(1, false), checks]].concat(),
        "report_digest",
    );
    // Upper-case hex is outside the domain too.
    let upper = "AA".repeat(32);
    bad(
        [
            asked(&c),
            vec![receipt_with(1, &upper, Some(&"bb".repeat(32)), action())],
        ]
        .concat(),
        "receipt_sha256",
    );
}

// M3E-R21 (1): T16's three-way join needs a journaled File dispatch; an
// ApprovalPending never stands in for it.
#[test]
fn m3e_u10_r21_t16_join_needs_a_file_dispatch() {
    let c = page_contract();
    let dispatched = |e: &Entry| matches!(e.event, Event::EffectDispatched { .. });
    let mut j = asked(&c);
    j.retain(|e| !dispatched(e));
    j.extend([receipt(1, false), recorded(1, PASS)]);
    let unconfirmed = "I could not confirm what became of the request";
    assert_eq!(
        summary(&j, &c),
        Ok(Some(t16(unconfirmed, T16_NONE_RECORDED)))
    );
    assert!(r(&j, &c).mismatched.contains(&"receipt_effect"));
    // Control: the same journal with its File dispatch passes.
    let j = [asked(&c), vec![receipt(1, false), recorded(1, PASS)]].concat();
    assert_eq!(summary(&j, &c), Ok(Some(t16(T16_PASSED, ALL_PASS))));
}

// M3E-R21 (2): any conflicting action identity for the receipt's effect is
// reported, even when another identity matches.
#[test]
fn m3e_u10_r21_any_conflicting_action_is_reported() {
    let c = page_contract();
    let other = PendingAction {
        run: "run".into(),
        commitment: "another".into(),
        deadline_mono_ms: 1,
    };
    let published = || SettlementOutcome::Published {
        recovery_required: false,
        interrupted: vec![],
    };
    // Pending A, receipt A, settlement B: mismatched.
    let j = [
        asked(&c),
        vec![receipt(1, false), settled(published(), Some(other))],
    ]
    .concat();
    let got = r(&j, &c);
    assert_eq!((got.missing, got.mismatched), (vec![], vec!["action"]));
    // Pending A, receipt A, settlement A: linked, nothing reported.
    let j = [
        asked(&c),
        vec![receipt(1, false), settled(published(), Some(action()))],
    ]
    .concat();
    let got = r(&j, &c);
    assert_eq!((got.missing, got.mismatched), (vec![], vec![]));
}

/// A real broker page mission, run to Done (one broker per process).
const CHILD: &str = "oracles::m3e_receipts::m3e_u10_child";

#[cfg(target_os = "linux")]
#[test]
fn m3e_u10_receipts_from_a_real_page_mission() {
    crate::adapter::m3d_publication_oracles::run_child(CHILD, "done");
}

/// M3E-R19: the real settlement path (lite_settle) ends a File whose broker
/// action is still executing at Mother's held deadline `Uncertain`, with no
/// receipt: the receipt lists it missing, and T16 claims nothing.
#[cfg(target_os = "linux")]
#[test]
fn m3e_u10_r19_real_uncertain_settlement_is_missing_evidence() {
    crate::adapter::m3d_publication_oracles::run_child(CHILD, "uncertain");
}

#[cfg(target_os = "linux")]
fn uncertain_case() {
    use crate::adapter::lite_clock::{past_the_start, LiteHome};
    use crate::adapter::m2c_support::port_for;
    use crate::adapter::m3d_publication_oracles::{scratch, BAKERY};
    use crate::adapter::{create_private, mother_agent};
    use crate::coordinator::{Coordinator, Lanes};
    use crate::effect::EffectPort;
    use crate::journal::{verify, MemoryJournal};
    use crate::lane::{Lane, ScriptedLane, Verdict};
    use crate::oracles::lite_support::{file, FilesOwner};
    use crate::oracles::{m2c_confirmers::answering, Yes};
    use nexus_governed_control::authority::clock::{Clock, SystemClock};
    use nexus_governed_control::authority::ids::{CommitmentId, RunId};
    use nexus_governed_control::governed::GrantRequest;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc};
    use std::time::Duration;
    let temp = scratch("m3e-r19");
    let home = LiteHome::open(&temp.join("home")).unwrap();
    let ws = temp.join("workspace");
    create_private(&ws).unwrap();
    let info = home.control.register_workspace(&ws, &[]).unwrap();
    let owner = FilesOwner::default();
    let files = GrantRequest::Files { workspace: info.id };
    home.control
        .request_grant(&files, Duration::from_secs(86400), &owner)
        .unwrap();
    let port = Arc::new(port_for(&home.control, true).with_workspace(info.name.clone()));
    let mut lanes: Vec<Arc<dyn Lane>> = crate::oracles::keyed_lanes()
        .into_iter()
        .filter(|l| !matches!(l.identity().role, LaneRole::Builder | LaneRole::Verifier))
        .collect();
    let builder = ScriptedLane::new(LaneRole::Builder, "built", Verdict::Continue);
    lanes.push(Arc::new(builder.asking(vec![file("index.html", BAKERY)])));
    lanes.push(Arc::new(crate::page_verifier::PageVerifierLane));
    let journal = MemoryJournal::default();
    let m = Coordinator::open(
        journal.clone(),
        port.clone(),
        Lanes::new(lanes).unwrap(),
        Arc::new(SystemClock::default()),
        home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    let mut c = page_contract();
    c.requested_folder = Some(info.name.clone());
    let id = m.draft(c.clone()).unwrap();
    m.submit(id).unwrap();
    answering(&Yes, || m.request_consent(id)).unwrap();
    // As lite_recognition's deadline cases: the broker prepares 30 s later.
    let clock = home.clock.clone();
    port.in_broker_action.arm(0, move || {
        let reads = AtomicUsize::new(0);
        let advance = clock.clone();
        clock.arm(
            move || reads.fetch_add(1, Ordering::SeqCst) == 1,
            move || advance.advance(Duration::from_secs(30)),
        );
    });
    for _ in 0..3 {
        m.advance(id).unwrap();
    }
    let pending = port.pending(id).unwrap();
    let run = RunId::parse(&pending.run).unwrap();
    let commitment = CommitmentId::parse(&pending.commitment).unwrap();
    home.control
        .authorize(commitment, &mother_agent(), run, &owner)
        .unwrap();
    // The approved action is held executing at the broker.
    let (entered_tx, entered) = mpsc::sync_channel(1);
    let (release, release_rx) = mpsc::sync_channel::<()>(1);
    home.clock.arm(past_the_start(&home, run), move || {
        entered_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    });
    let broker = home.control.clone();
    let worker = std::thread::spawn(move || broker.execute(commitment, &mother_agent(), run));
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    let left = pending
        .deadline_mono_ms
        .saturating_sub(home.clock.monotonic_ms());
    home.clock.advance(Duration::from_millis(left + 1));
    m.advance(id).unwrap();
    assert_eq!(
        m.reason(id).unwrap().as_deref(),
        Some("file_publication_uncertain")
    );
    let entries = verify(&journal.lines()).unwrap();
    release.send(()).unwrap();
    let _ = worker.join().unwrap();
    let uncertain = |e: &Entry| {
        matches!(
            &e.event,
            Event::Transition {
                trigger: Trigger::Settled {
                    outcome: SettlementOutcome::Uncertain,
                    ..
                }
            }
        )
    };
    assert!(
        entries.iter().any(uncertain),
        "the real Uncertain settlement"
    );
    let got = mission_receipt(&entries, id, &c).unwrap();
    assert_eq!(
        (got.publication.clone(), got.missing.clone()),
        (None, vec!["publication_receipt"])
    );
    let unconfirmed = "I could not confirm what became of the request";
    assert_eq!(
        page_view(&entries, id).summary(&c),
        Ok(Some(t16(unconfirmed, T16_NONE_RECORDED)))
    );
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
    if case == "uncertain" {
        uncertain_case();
        println!("m3d-db-receipt:{case}");
        return;
    }
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
