//! M3-Dc1 oracles frozen before implementation: the page Verifier's
//! identity-bound input, its typed report, the durable checks that gate
//! VerifyPass, and their replay. A child of the proof module only so that
//! the tamper controls can alter a live mission's proof in memory;
//! everything else goes through the lane, the lifecycle or the journal.
use crate::adapter::m3d_publication_oracles::{
    at, child_case, count, other_page, run_child, sha, FailOn, PageFx, BAKERY, WRITTEN,
};
use crate::adapter::GovernedPort;
use crate::coordinator::{inspect_history_lines, Coordinator, FileSession, Lanes};
use crate::effect::{EffectPort, EffectRequest};
use crate::evidence::{check, check_codes, is_check_result, VerifyCode, VerifyFailReason};
use crate::journal::GENESIS;
use crate::journal::{Entry, Event, JournalError, JournalStore, MissionId, ReplayViolation};
use crate::lane::{input_hash, output_hash, InvocationId, Lane, LaneContext, LaneIdentity};
use crate::lane::{LaneInput, LaneOutput, LaneRole, ModelCallError, ModelCaller};
use crate::lane::{ScriptedLane, Verdict};
use crate::oracles::lite_support::file;
use crate::oracles::{contract_with, keyed_lanes};
use crate::page_inputs::PageFacts;
use crate::page_verifier::VerificationInput;
use crate::page_verifier::{PageVerifierLane, ReceiptFields, VerificationIdentity};
use crate::state::{MissionState, Trigger};
use crate::verifier::{verify_page, CheckCode, CheckId, Receipt, VerifierReport};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::ids::RunId;
use nexus_governed_control::inert_page::policy_hash;
use nexus_governed_control::inference::{ModelReply, ModelRequest};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::Arc;

const CHILD: &str = "coordinator::page_proof::verifier_oracles::m3d_dc1_child";
/// Independent Python hashlib: SHA-256 of `nexus-m3/verification-report/1`,
/// one NUL byte, and `[["v0","pass"],["v1","pass"],...,["v5","pass"]]`.
const PASS_DIGEST: &str = "715740861cb7845e91da2cbca04a73feaf4b594cdcf9019a6a3459945b2d740f";
const LEGACY_OUTPUT: &str = r#"{"text":"t","effects":[],"verdict":{"verdict":"continue"}}"#;
/// Independent Python hashlib: `nexus-mother-lane-output-v1\n`, u64 BE
/// length, then LEGACY_OUTPUT.
const LEGACY_OUTPUT_HASH: &str = "abf61dd9a0980c89b28da8396cee48f9fdaaa177885baed2f51631605ef28512";
const CANARY: &str = "CANARY_DC1_LANE_REASON";

fn facts() -> PageFacts {
    PageFacts {
        name: "Rosa's Bakery".into(),
        hours: vec![
            "Tuesday to Sunday, 7am to 3pm".into(),
            "Closed on Mondays".into(),
        ],
        phone: "555-0142".into(),
    }
}
fn receipt_of(page: &[u8]) -> ReceiptFields {
    ReceiptFields {
        sha256: sha(page),
        bytes: page.len() as u64,
        policy_hash: hex::encode(policy_hash()),
        overtaken: false,
    }
}
fn input_for(page: &str) -> VerificationInput {
    let r = receipt_of(BAKERY.as_bytes());
    VerificationInput::new(page.as_bytes().to_vec(), &facts(), &r).expect("an input")
}
fn lane_input(verification: Option<VerificationInput>) -> LaneInput {
    LaneInput {
        invocation: InvocationId {
            mission: MissionId(3),
            seq: 40,
        },
        role: LaneRole::Verifier,
        contract_hash: contract_with("").hash(),
        owner_words: "w".into(),
        restatement: "r".into(),
        pass_criteria: vec![],
        prior_outputs: vec![],
        builder_input: None,
        verification,
    }
}
/// M3-A's own report on the same values (the reference, not the lane).
fn m3a(page: &str) -> VerifierReport {
    let receipt = Receipt {
        sha256: hex::decode(sha(BAKERY.as_bytes()))
            .unwrap()
            .try_into()
            .unwrap(),
        bytes: BAKERY.len() as u64,
        policy_hash: policy_hash(),
    };
    verify_page(
        page.as_bytes(),
        &facts().approved().unwrap(),
        Some(&receipt),
    )
}
fn entries(report: &VerifierReport) -> Vec<(String, String)> {
    report
        .checks()
        .iter()
        .map(|(id, result)| (id.as_str().to_string(), result.to_string()))
        .collect()
}

#[test]
fn m3d_verification_identity_hashes_every_field() {
    // Absent payloads: the lane input's legacy golden is Da's
    // m3d_absent_facts_preserve_contract_bytes; the output's is here.
    let legacy = LaneOutput {
        text: "t".into(),
        effects: vec![],
        verdict: Verdict::Continue,
        verification: None,
    };
    assert_eq!(serde_json::to_string(&legacy).unwrap(), LEGACY_OUTPUT);
    assert_eq!(output_hash(&legacy), LEGACY_OUTPUT_HASH);
    assert!(!serde_json::to_string(&lane_input(None))
        .unwrap()
        .contains("verification"));
    let v = input_for(BAKERY);
    let page = sha(BAKERY.as_bytes());
    let json = format!(
        r#"{{"candidate_sha256":"{page}","candidate_bytes":{n},"facts_hash":"{}","receipt_sha256":"{page}","receipt_bytes":{n},"receipt_policy_hash":"{}","receipt_overtaken":false}}"#,
        hex::encode(facts().approved().unwrap().hash()),
        hex::encode(policy_hash()),
        n = BAKERY.len(),
    );
    assert_eq!(
        serde_json::to_string(&v).unwrap(),
        format!(r#"{{"identity":{json}}}"#)
    );
    let base = lane_input(Some(v));
    let with = serde_json::to_string(&base).unwrap();
    assert!(with.ends_with(&format!(r#","verification":{{"identity":{json}}}}}"#)));
    for canary in ["Lemon drizzle", "555-0142", "Rosa"] {
        assert!(!with.contains(canary) && !format!("{base:?}").contains(canary));
    }
    type Edit = fn(&mut VerificationIdentity);
    let edits: [(&str, Edit); 7] = [
        ("candidate_sha256", |i| i.candidate_sha256 = "0".repeat(64)),
        ("candidate_bytes", |i| i.candidate_bytes += 1),
        ("facts_hash", |i| i.facts_hash = "1".repeat(64)),
        ("receipt_sha256", |i| i.receipt_sha256 = "2".repeat(64)),
        ("receipt_bytes", |i| i.receipt_bytes += 1),
        ("receipt_policy_hash", |i| {
            i.receipt_policy_hash = "3".repeat(64)
        }),
        ("receipt_overtaken", |i| i.receipt_overtaken = true),
    ];
    for (name, edit) in edits {
        let mut changed = base.clone();
        edit(changed.verification.as_mut().unwrap().identity_mut());
        assert_ne!(input_hash(&changed), input_hash(&base), "{name}");
    }
}

#[test]
fn m3d_verification_payload_mismatch_is_refused() {
    let lane = PageVerifierLane;
    for k in 0..3 {
        let mut v = input_for(BAKERY);
        let (candidate, facts) = v.payload_mut();
        match k {
            0 => *candidate = other_page().into_bytes(),
            1 => facts.phone = "555-0199".into(),
            _ => facts.hours.truncate(1),
        }
        let r = lane.invoke(&lane_input(Some(v.clone()))).verification;
        let r = r.expect("a typed refusal");
        assert_eq!(r.identity(), v.identity());
        assert_eq!(
            r.entries(),
            None,
            "{k}: nothing runs on an unidentified payload"
        );
        assert_eq!(r.failure(), Some(VerifyCode::Identity), "{k}");
    }
    assert_eq!(
        VerifyFailReason::Check(VerifyCode::Identity).to_string(),
        "check:verification_identity"
    );
    // A correctly identified candidate against a different genuine receipt.
    let alt = other_page();
    let v = input_for(&alt);
    assert_eq!(v.identity().candidate_sha256, sha(alt.as_bytes()));
    assert_eq!(v.identity().receipt_sha256, sha(BAKERY.as_bytes()));
    let r = lane.invoke(&lane_input(Some(v))).verification.unwrap();
    let mismatch = VerifyCode::Check(CheckCode::PublishedMismatch);
    assert_eq!(r.failure(), Some(mismatch));
    assert_eq!(r.entries().unwrap(), entries(&m3a(&alt)));
}

struct Calls(AtomicUsize);
impl ModelCaller for Calls {
    fn call(&self, _: &ModelRequest) -> Result<ModelReply, ModelCallError> {
        self.0.fetch_add(1, SeqCst);
        Err(ModelCallError::RecordUnavailable)
    }
}

#[test]
fn m3d_page_verifier_is_zero_call_and_effect_free() {
    let lane = PageVerifierLane;
    let identity = LaneIdentity {
        role: LaneRole::Verifier,
        name: "page-verifier".into(),
        version: "m3-1".into(),
    };
    assert_eq!(lane.identity(), identity);
    assert_eq!(lane.max_model_calls(), 0);
    let calls = Calls(AtomicUsize::new(0));
    let context = LaneContext {
        model: Some(&calls),
    };
    for page in [BAKERY.to_string(), other_page()] {
        let v = input_for(&page);
        let out = lane.invoke_in(&lane_input(Some(v.clone())), &context);
        assert!(out.effects.is_empty());
        let expected = m3a(&page);
        assert_eq!(out.verdict, expected.verdict());
        let report = out.verification.expect("a typed report");
        assert_eq!(report.identity(), v.identity());
        assert_eq!(report.entries().unwrap(), entries(&expected));
        assert_eq!(report.passed(), expected.passed());
    }
    assert_eq!(calls.0.load(SeqCst), 0, "no model is called");
    let out = lane.invoke(&lane_input(None));
    assert_eq!(out.verification, None, "no input, no report");
    assert_ne!(out.verdict, Verdict::Pass);
}

#[test]
fn m3d_verifier_requires_live_clean_publication() {
    for case in [
        "clean",
        "o34-denied",
        "o34-lapsed",
        "o34-stopped",
        "o34-overtaken",
        "o34-nopub",
        "o34-overtaken-proof",
        "o34-other-action",
        "o34-other-mission",
        "o34-no-policy",
    ] {
        run_child(CHILD, case);
    }
}

#[test]
fn m3d_durable_checks_gate_verify_pass() {
    // The closed registry: every code round-trips; lane text never parses.
    assert_eq!(check_codes().len(), 69);
    let every = VerifyFailReason::every();
    assert_eq!(every.len(), 72);
    for r in &every {
        assert_eq!(VerifyFailReason::parse(&r.to_string()).as_ref(), Some(r));
    }
    let canary = format!("check:{CANARY}");
    for (text, ok) in [
        ("check:published_mismatch", true),
        ("check:verification_identity", true),
        ("check:page_not_inert:script_element", true),
        ("check:hours_line_missing:6", true),
        ("check:hours_line_missing:7", false),
        ("check:hours_line_not_visible:0", true),
        ("check:email_not_supported", true),
        ("check:fail:no_publication", false),
        ("check:", false),
        (canary.as_str(), false),
    ] {
        assert_eq!(VerifyFailReason::parse(text).is_some(), ok, "{text}");
    }
    for (id, code, ok) in [
        (CheckId::V1, "fail:published_mismatch", true),
        (CheckId::V0, "fail:published_mismatch", false),
        (CheckId::V3, "pass", true),
        (CheckId::V2, "not_run", true),
        (CheckId::V4, "fail:hours_line_not_visible:6", true),
        (CheckId::V5, "fail:CANARY_DC1_LANE_REASON", false),
    ] {
        assert_eq!(is_check_result(id, code), ok, "{code}");
    }
    for case in [
        "clean",
        "dv2-mismatch",
        "dv2-scripted-fail",
        "dv2-scripted-pass",
    ] {
        run_child(CHILD, case);
    }
}

#[test]
fn m3d_replay_rejects_relinked_verification_proof() {
    let checks: Vec<(String, String)> = (0..6)
        .map(|k| (format!("v{k}"), "pass".to_string()))
        .collect();
    let record = Event::VerificationRecorded {
        invocation: InvocationId {
            mission: MissionId(1),
            seq: 30,
        },
        effect: 0,
        report_digest: PASS_DIGEST.into(),
        checks: checks.clone(),
    };
    let json = format!(
        r#"{{"event":"verification_recorded","invocation":{{"mission":1,"seq":30}},"effect":0,"report_digest":"{PASS_DIGEST}","checks":[["v0","pass"],["v1","pass"],["v2","pass"],["v3","pass"],["v4","pass"],["v5","pass"]]}}"#
    );
    assert_eq!(serde_json::to_string(&record).unwrap(), json);
    assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), record);
    assert_eq!(check(&record), Ok(()));
    let bad = |edit: &dyn Fn(&mut Vec<(String, String)>, &mut String)| {
        let (mut c, mut d) = (checks.clone(), PASS_DIGEST.to_string());
        edit(&mut c, &mut d);
        let e = Event::VerificationRecorded {
            invocation: InvocationId {
                mission: MissionId(1),
                seq: 30,
            },
            effect: 0,
            report_digest: d,
            checks: c,
        };
        assert_eq!(check(&e), Err(ReplayViolation::NotEvidence), "{e:?}");
    };
    bad(&|_, d| *d = "0".repeat(64));
    bad(&|c, _| c[3].1 = format!("fail:{CANARY}"));
    bad(&|c, _| c.swap(0, 1));
    bad(&|c, _| drop(c.pop()));
    bad(&|c, _| c[1].1 = "fail:no_publication".into());
    run_child(CHILD, "dr1v");
}

#[test]
fn m3d_verification_append_failure_prevents_success() {
    run_child(CHILD, "dp3v");
}

/// Counts the verifier's calls.
struct Counted(Arc<AtomicUsize>, Arc<dyn Lane>);
impl Lane for Counted {
    fn identity(&self) -> LaneIdentity {
        self.1.identity()
    }
    fn invoke(&self, input: &LaneInput) -> LaneOutput {
        self.0.fetch_add(1, SeqCst);
        self.1.invoke(input)
    }
}

fn session(
    f: &PageFx,
    effects: Vec<EffectRequest>,
    store: impl JournalStore + 'static,
    verifier: Arc<dyn Lane>,
) -> FileSession<GovernedPort> {
    let mut lanes: Vec<Arc<dyn Lane>> = keyed_lanes()
        .into_iter()
        .filter(|l| !matches!(l.identity().role, LaneRole::Builder | LaneRole::Verifier))
        .collect();
    let builder = ScriptedLane::new(LaneRole::Builder, "built", Verdict::Continue);
    lanes.push(Arc::new(builder.asking(effects)));
    lanes.push(verifier);
    let clock = Arc::new(SystemClock::default());
    let private = f.home.private();
    let lanes = Lanes::new(lanes).unwrap();
    Coordinator::open(store, f.port.clone(), lanes, clock, private)
        .unwrap()
        .into_file_session()
        .unwrap()
}

fn is_verification(e: &Event) -> bool {
    matches!(e, Event::VerificationRecorded { .. })
}
fn is_pass(e: &Event) -> bool {
    matches!(
        e,
        Event::Transition {
            trigger: Trigger::VerifyPass
        }
    )
}

type Records = Vec<(MissionId, Event)>;

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
fn m3d_dc1_child() {
    let Some(case) = child_case() else {
        return;
    };
    let f = PageFx::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let inner: Arc<dyn Lane> = match case.as_str() {
        "dv2-scripted-fail" => {
            let verdict = Verdict::Fail {
                reason: CANARY.into(),
            };
            Arc::new(ScriptedLane::new(LaneRole::Verifier, CANARY, verdict))
        }
        "dv2-scripted-pass" => {
            Arc::new(ScriptedLane::new(LaneRole::Verifier, CANARY, Verdict::Pass))
        }
        _ => Arc::new(PageVerifierLane),
    };
    let verifier = Arc::new(Counted(calls.clone(), inner));
    let effects = match case.as_str() {
        "o34-nopub" => vec![],
        _ => vec![file("index.html", BAKERY)],
    };
    let m = if case == "dp3v" {
        let needle = "\"event\":\"verification_recorded\"";
        let store = FailOn {
            memory: f.journal.clone(),
            needle,
        };
        session(&f, effects, store, verifier)
    } else {
        session(&f, effects, f.journal.clone(), verifier)
    };
    let tamper = |id: MissionId, edit: &dyn Fn(&mut super::Proof)| {
        let mut inner = m.coordinator.inner.lock().unwrap();
        edit(&mut inner.missions.get_mut(&id).unwrap().proof);
    };
    // Waiting, then published and through to Verifying.
    let verifying = || {
        let id = f.waiting(&m);
        f.publish(id);
        assert_eq!(m.advance(id).unwrap().to, MissionState::Verifying);
        assert_eq!(calls.load(SeqCst), 0);
        id
    };
    let no_verifier = |id: MissionId| {
        let _ = m.advance(id);
        let events = f.events();
        assert_eq!(calls.load(SeqCst), 0, "{case}: the verifier is not called");
        assert_eq!(count(&events, is_verification), 0);
        assert_eq!(count(&events, is_pass), 0);
        assert!(m.state(id).unwrap().unwrap().is_terminal());
    };
    let refused_unpublished = |id: MissionId| {
        let r = m.advance(id).unwrap();
        assert_eq!(r.to, MissionState::Failed);
        assert_eq!(r.reason.as_deref(), Some("check:no_publication"));
        no_verifier(id);
    };
    match case.as_str() {
        "clean" => {
            let id = verifying();
            assert_eq!(m.advance(id).unwrap().to, MissionState::Explaining);
            assert_eq!(calls.load(SeqCst), 1);
            let events = f.events();
            assert_eq!(count(&events, is_verification), 1);
            let v = at(&events, is_verification).unwrap();
            let Event::VerificationRecorded {
                invocation,
                effect,
                report_digest,
                checks,
            } = events[v].clone()
            else {
                unreachable!()
            };
            let pass: Vec<(String, String)> = (0..6)
                .map(|k| (format!("v{k}"), "pass".to_string()))
                .collect();
            assert_eq!(checks, pass);
            assert_eq!(report_digest, PASS_DIGEST);
            assert_eq!(invocation.mission, id);
            assert!(
                matches!(&events[v - 1], Event::LaneReturned { invocation: i, .. } if *i == invocation)
            );
            assert!(events[..v].iter().any(|e| matches!(e, Event::LaneInvoked { invocation: i, role: LaneRole::Verifier, .. } if *i == invocation)));
            assert!(events[..v].iter().any(|e| matches!(e, Event::PublicationReceipt { effect: e2, overtaken: false, .. } if *e2 == effect)));
            let t = at(&events, is_pass).expect("VerifyPass");
            assert!(v < t, "the checks are journaled before VerifyPass");
            let lines = f.journal.lines().join("\n");
            for canary in ["Lemon drizzle", "555-0142", "Rosa's Bakery"] {
                assert!(!lines.contains(canary), "{canary}");
            }
            assert_eq!(m.advance(id).unwrap().to, MissionState::Done);
            assert_eq!(calls.load(SeqCst), 1);
        }
        "o34-denied" | "o34-lapsed" | "o34-stopped" => {
            let id = f.waiting(&m);
            match case.as_str() {
                "o34-denied" => f.deny(id),
                "o34-lapsed" => {
                    let run = RunId::parse(&f.port.pending(id).unwrap().run).unwrap();
                    f.home.control.cancel_run(run).unwrap();
                }
                _ => assert_eq!(m.stop(id), Ok(MissionState::Stopped)),
            }
            let _ = m.advance(id);
            assert_ne!(m.state(id), Ok(Some(MissionState::Verifying)));
            no_verifier(id);
            assert_eq!(f.written(), None);
        }
        "o34-overtaken" => {
            let id = f.waiting(&m);
            f.publish(id);
            assert_eq!(m.stop(id), Ok(MissionState::Blocked));
            no_verifier(id);
            assert_eq!(f.written().unwrap(), BAKERY.as_bytes());
        }
        "o34-nopub" => {
            let id = f.started(&m, 3);
            assert_eq!(m.state(id), Ok(Some(MissionState::Verifying)));
            refused_unpublished(id);
        }
        "o34-overtaken-proof" | "o34-other-action" | "o34-other-mission" | "o34-no-policy" => {
            let id = verifying();
            tamper(id, &|p| match case.as_str() {
                "o34-overtaken-proof" => p.receipt.as_mut().unwrap().overtaken = true,
                "o34-other-action" => p.receipt.as_mut().unwrap().effect += 1,
                "o34-other-mission" => {
                    p.retained.as_mut().unwrap().invocation.mission = MissionId(id.0 + 1)
                }
                _ => p.receipt.as_mut().unwrap().policy_hash = None,
            });
            refused_unpublished(id);
        }
        "dv2-mismatch" => {
            let id = verifying();
            let other = sha(other_page().as_bytes());
            tamper(id, &|p| p.receipt.as_mut().unwrap().sha256 = other.clone());
            let r = m.advance(id).unwrap();
            assert_eq!(r.to, MissionState::Failed);
            assert_eq!(r.reason.as_deref(), Some("check:published_mismatch"));
            assert_eq!(calls.load(SeqCst), 1);
            let events = f.events();
            let v = at(&events, is_verification).expect("the failing checks");
            let Event::VerificationRecorded { checks, .. } = &events[v] else {
                unreachable!()
            };
            assert_eq!(checks[0], ("v0".into(), "pass".into()));
            assert_eq!(checks[1], ("v1".into(), "fail:published_mismatch".into()));
            assert_eq!(count(&events, is_pass), 0);
            let failed = Trigger::VerifyFail {
                reason: VerifyFailReason::Check(VerifyCode::Check(CheckCode::PublishedMismatch)),
            };
            let t = at(&events, |e| {
                *e == Event::Transition {
                    trigger: failed.clone(),
                }
            });
            assert!(t.is_some_and(|t| v < t));
        }
        "dv2-scripted-fail" | "dv2-scripted-pass" => {
            let id = verifying();
            let r = m.advance(id).unwrap();
            assert_eq!(r.to, MissionState::Failed);
            assert_eq!(r.reason.as_deref(), Some("verifier_gave_no_verdict"));
            assert_eq!(calls.load(SeqCst), 1);
            let events = f.events();
            assert_eq!(count(&events, is_verification), 0);
            assert_eq!(count(&events, is_pass), 0);
            assert!(!f.journal.lines().join("\n").contains(CANARY));
        }
        "dr1v" => {
            let id = verifying();
            m.advance(id).unwrap();
            assert_eq!(m.advance(id).unwrap().to, MissionState::Done);
            let records: Records = f
                .entries()
                .into_iter()
                .map(|e| (e.mission, e.event))
                .collect();
            let events: Vec<Event> = records.iter().map(|r| r.1.clone()).collect();
            let p = at(&events, |e| matches!(e, Event::PublicationReceipt { .. })).unwrap();
            let v = at(&events, is_verification).expect("the checks");
            let t = at(&events, is_pass).unwrap();
            // The Sentinel's answer just before the Verifier's call.
            let s = (0..v)
                .rev()
                .find(|k| {
                    matches!(
                        &events[*k],
                        Event::LaneInvoked {
                            role: LaneRole::Sentinel,
                            ..
                        }
                    )
                })
                .unwrap();
            let Event::LaneInvoked {
                invocation: sentinel,
                ..
            } = events[s].clone()
            else {
                unreachable!()
            };
            assert!(inspect_history_lines(&rechain(&records)).is_ok());
            let code = |edit: &dyn Fn(&mut Records)| {
                let mut changed = records.clone();
                edit(&mut changed);
                match inspect_history_lines(&rechain(&changed)) {
                    Err(JournalError::Inconsistent { code, .. }) => Some(code),
                    _ => None,
                }
            };
            fn record(v: &mut Records, at: usize) -> &mut Event {
                &mut v[at].1
            }
            use ReplayViolation::{NotEvidence, ProofConflict, ProofOutOfPlace};
            type Case = (&'static str, ReplayViolation, Box<dyn Fn(&mut Records)>);
            let cases: Vec<Case> = vec![
                (
                    "checks before the receipt",
                    ProofOutOfPlace,
                    Box::new(move |r: &mut Records| {
                        let x = r.remove(v);
                        r.insert(p, x);
                    }),
                ),
                (
                    "second checks",
                    ProofConflict,
                    Box::new(move |r: &mut Records| {
                        let x = r[v].clone();
                        r.insert(v + 1, x);
                    }),
                ),
                (
                    "checks of the sentinel's answer",
                    ProofOutOfPlace,
                    Box::new(move |r: &mut Records| {
                        let mut x = r.remove(v);
                        if let Event::VerificationRecorded { invocation, .. } = &mut x.1 {
                            *invocation = sentinel;
                        }
                        r.insert(s + 2, x);
                    }),
                ),
                (
                    "checks of another invocation",
                    ProofOutOfPlace,
                    Box::new(move |r: &mut Records| {
                        if let Event::VerificationRecorded { invocation, .. } = record(r, v) {
                            invocation.seq -= 1;
                        }
                    }),
                ),
                (
                    "checks of another mission",
                    ProofOutOfPlace,
                    Box::new(move |r: &mut Records| {
                        if let Event::VerificationRecorded { invocation, .. } = record(r, v) {
                            invocation.mission = MissionId(9);
                        }
                    }),
                ),
                (
                    "checks of another effect",
                    ProofOutOfPlace,
                    Box::new(move |r: &mut Records| {
                        if let Event::VerificationRecorded { effect, .. } = record(r, v) {
                            *effect += 1;
                        }
                    }),
                ),
                (
                    "checks digest",
                    NotEvidence,
                    Box::new(move |r: &mut Records| {
                        if let Event::VerificationRecorded { report_digest, .. } = record(r, v) {
                            *report_digest = "0".repeat(64);
                        }
                    }),
                ),
                (
                    "checks code",
                    NotEvidence,
                    Box::new(move |r: &mut Records| {
                        if let Event::VerificationRecorded { checks, .. } = record(r, v) {
                            checks[2].1 = format!("fail:{CANARY}");
                        }
                    }),
                ),
                (
                    "pass without its checks",
                    ProofOutOfPlace,
                    Box::new(move |r: &mut Records| drop(r.remove(v))),
                ),
            ];
            assert!(v < t);
            for (name, expected, edit) in cases {
                assert_eq!(code(&*edit), Some(expected), "{name}");
            }
        }
        "dp3v" => {
            let id = verifying();
            let r = m.advance(id).unwrap();
            assert_eq!(r.to, MissionState::Failed);
            assert_eq!(r.reason.as_deref(), Some("journal_failed"));
            assert_eq!(r.explanation.as_deref(), Some(WRITTEN));
            assert_eq!(f.written().unwrap(), BAKERY.as_bytes());
            let events = f.events();
            assert_eq!(count(&events, is_verification), 0);
            assert_eq!(count(&events, is_pass), 0);
            assert!(f.journal.is_poisoned());
            assert!(m.advance(id).is_err(), "nothing continues");
        }
        other => panic!("unknown case {other}"),
    }
    println!("m3d-db-receipt:{case}");
}
