//! Da oracles frozen before implementation; child module can mutate private
//! snapshot fields solely to exercise skipped-payload substitution.
use super::*;
use crate::adapter::m2c_support::consent;
use crate::consent::MissionSummary;
use crate::coordinator::CoordinatorError;
use crate::journal::{Event, MissionId};
use crate::lane::{input_hash, InvocationId, Lane, LaneIdentity, LaneInput, LaneOutput, Verdict};
use crate::oracles::*;
use crate::state::MissionState;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

const LEGACY: &str = r#"{"owner_words":"Make a page.","restatement":"A small page.","requested_folder":null,"budget":{"max_duration_ms":1000,"max_model_calls":9,"max_money_minor":0},"forbidden_effects":[],"questions":[],"pass_criteria":[{"id":"p1","text":"A page exists."}]}"#;
const LEGACY_HASH: &str = "f3fbd219f71e73f42a5e19b333616b7771a8d2e5bba92e0aed32d0d2a32f392f";
const LEGACY_INPUT: &str = r#"{"invocation":{"mission":7,"seq":11},"role":"builder","contract_hash":"f3fbd219f71e73f42a5e19b333616b7771a8d2e5bba92e0aed32d0d2a32f392f","owner_words":"Make a page.","restatement":"A small page.","pass_criteria":[{"id":"p1","text":"A page exists."}],"prior_outputs":[]}"#;
const LEGACY_INPUT_HASH: &str = "406b2c4bacbb706031f7766951ebeb011439bd27e79a9d8eea221a886620b93b";
const PAYLOAD_BYTES: &str =
    r#"{"note":{"label":"notes","text":"a"},"facts":{"name":"a","hours":["a"],"phone":"1234567"}}"#;
const PAYLOAD_HASH: &str = "928644071ff49ea8fda56421100856f11cdd1fde5c598fdcab34036c51aee18f";

fn expected_criteria() -> Vec<PassCriterion> {
    [
        ("V0", "index.html was created once, with your approval, and its receipt is recorded."),
        ("V1", "The written file is exactly the file you approved (same SHA-256 and size)."),
        ("V2", "The page passes the fixed inert-format check, under the same policy that was approved."),
        ("V3", "Your name, every hours line and the phone appear exactly as you typed them, and no other phone-like number appears."),
        ("V4", "A second, separate reader finds your name, every hours line and the phone in the page's text."),
        ("V5", "The page contains no email-shaped text."),
    ].into_iter().map(|(id,text)| PassCriterion {id:id.into(),text:text.into()}).collect()
}
fn page() -> MissionContract {
    let mut c: MissionContract = serde_json::from_str(LEGACY).unwrap();
    c.page_facts = Some(PageFacts {
        name: "a".into(),
        hours: vec!["a".into()],
        phone: "1234567".into(),
    });
    c.notes = vec![Note {
        label: "notes".into(),
        text: "a".into(),
    }];
    c.deliverable = Some("index.html".into());
    c.pass_criteria = expected_criteria();
    c
}
fn invocation() -> InvocationId {
    InvocationId {
        mission: MissionId(7),
        seq: 11,
    }
}
fn input(c: &MissionContract) -> LaneInput {
    LaneInput {
        invocation: invocation(),
        role: LaneRole::Builder,
        contract_hash: c.hash(),
        owner_words: c.owner_words.clone(),
        restatement: c.restatement.clone(),
        pass_criteria: c.pass_criteria.clone(),
        prior_outputs: vec![],
        builder_input: None,
    }
}
fn no_work(w: &World) {
    assert_eq!(w.keyed_calls(), 0);
    assert!(w.port.effects().is_empty());
    assert!(!w.mother.journal().unwrap().iter().any(|e| matches!(
        e.event,
        Event::LaneInvoked { .. } | Event::ModelCallCharged { .. } | Event::EffectDispatched { .. }
    )));
}

#[test]
fn m3d_absent_facts_preserve_contract_bytes() {
    let c: MissionContract = serde_json::from_str(LEGACY).unwrap();
    assert_eq!(c.page_facts, None);
    assert_eq!(c.validate(), Ok(()));
    assert_eq!(serde_json::to_string(&c).unwrap(), LEGACY);
    assert_eq!(c.hash().to_hex(), LEGACY_HASH);
    let i = input(&c);
    assert_eq!(serde_json::to_string(&i).unwrap(), LEGACY_INPUT);
    assert_eq!(input_hash(&i), LEGACY_INPUT_HASH);
}

#[test]
fn m3d_present_facts_bind_every_value() {
    let c = page();
    assert_eq!(c.validate_for_consent(), Ok(()));
    let v = serde_json::to_value(&c).unwrap();
    assert_eq!(
        v["page_facts"],
        serde_json::json!({"name":"a","hours":["a"],"phone":"1234567"})
    );
    let mut c = c;
    c.page_facts.as_mut().unwrap().hours = vec!["Open weekdays".into(), "Closed weekends".into()];
    for k in 0..6 {
        let mut d = c.clone();
        let f = d.page_facts.as_mut().unwrap();
        match k {
            0 => f.name.push('b'),
            1 => f.phone.push('8'),
            2 => f.hours[0].push('b'),
            3 => f.hours.push("By appointment".into()),
            4 => {
                f.hours.pop();
            }
            _ => f.hours.swap(0, 1),
        }
        assert_ne!(c.hash(), d.hash(), "field {k}");
    }
    let mut unknown = v;
    unknown["page_facts"]["secret"] = true.into();
    assert!(serde_json::from_value::<MissionContract>(unknown).is_err());
}

#[test]
fn m3d_summary_and_confirmation_share_contract_facts() {
    let mut c = page();
    c.page_facts.as_mut().unwrap().name = "Rosa's & \"Bakery\"".into();
    let w = world();
    let id = w.submitted(c.clone());
    let owner = Recording::default();
    assert_eq!(consent(&w.mother, id, &owner), Ok(MissionState::Approved));
    let summary = w.mother.confirmation(id).unwrap();
    assert_eq!(summary.page_facts, c.page_facts);
    let asked = owner.0.lock().unwrap()[0].clone();
    let view = asked.page_facts.as_ref().expect("facts reach broker");
    assert_eq!(view, &c.page_facts.as_ref().unwrap().view());
    let facts_hash = hex::encode(view.approved().unwrap().hash());
    assert!(asked.message().contains(&facts_hash));
    assert!(asked.message().contains("Rosa's & \"Bakery\""));
    assert_eq!(asked.contract_hash, c.hash().to_hex());
    c.page_facts
        .as_mut()
        .unwrap()
        .hours
        .push("By appointment".into());
    w.mother.revise(id, c.clone()).unwrap();
    assert_eq!(w.mother.consented_hash(id), None);
    let new = MissionSummary::of(id, &c).confirmation("next".into());
    assert_ne!(asked.contract_hash, new.contract_hash);
    assert_ne!(asked.page_facts, new.page_facts);
}

#[test]
fn m3d_contract_counts_rendered_facts_and_notes() {
    let mut c = crate::oracles::m3d_manifest::maximum_contract('A');
    assert_eq!(c.validate_for_consent(), Ok(()));
    let f = c.page_facts.as_ref().unwrap();
    assert_eq!(rendered_size(f, &c.notes[0]), 2432);
    c.notes[0].text.push('.');
    assert_eq!(c.validate(), Err(ContractError::NotesAndFactsTooLarge));
    let w = world();
    assert!(matches!(
        w.mother.draft(c),
        Err(CoordinatorError::Contract(
            ContractError::NotesAndFactsTooLarge
        ))
    ));
    no_work(&w);
}

#[test]
fn m3d_facts_not_page_ready_precedes_consent_and_calls() {
    let mut valid = page();
    assert_eq!(valid.validate_for_consent(), Ok(()));
    let accepted = world();
    assert!(accepted.mother.draft(valid.clone()).is_ok());
    for name in ["info@bakery.com".to_string(), "a".repeat(41)] {
        valid.page_facts.as_mut().unwrap().name = name;
        assert_eq!(
            valid.validate_for_consent(),
            Err(ContractError::FactsNotPageReady)
        );
        let w = world();
        let owner = Recording::default();
        if let Ok(id) = w.mother.draft(valid.clone()) {
            if w.mother.submit(id).is_ok() {
                let _ = consent(&w.mother, id, &owner);
            }
        }
        assert!(owner.0.lock().unwrap().is_empty(), "no dialog");
        no_work(&w); // Also zero correction requests: no lane or model dispatch.
    }
}

#[test]
fn m3d_builder_input_is_the_consented_snapshot() {
    let c = page();
    let inv = invocation();
    let p = prepare(&c, c.hash(), inv, LaneRole::Builder)
        .unwrap()
        .unwrap();
    assert_eq!(p.borrowed(), (&c.notes[0], c.page_facts.as_ref().unwrap()));
    assert_eq!(p.validate(&c, c.hash(), inv), Ok(()));
    // Independently frozen domain + bytes, with no legacy length framing.
    let mut h = Sha256::new();
    h.update(b"nexus-m3/page-build-input/1\0");
    h.update(PAYLOAD_BYTES);
    assert_eq!(hex::encode(h.finalize()), PAYLOAD_HASH);
    assert_eq!(p.content_sha256, PAYLOAD_HASH);
    let identity = format!(
        r#"{{"contract_hash":"{}","invocation":{{"mission":7,"seq":11}},"content_sha256":"{}"}}"#,
        c.hash(),
        PAYLOAD_HASH
    )
    .replace("sha256:", "");
    assert_eq!(serde_json::to_string(&p).unwrap(), identity);
    let wrong = contract().hash();
    assert_eq!(
        p.validate(&c, wrong, inv),
        Err(ContractError::PageInputIdentity)
    );
    assert_eq!(
        p.validate(&c, c.hash(), InvocationId { seq: 12, ..inv }),
        Err(ContractError::PageInputIdentity)
    );
    assert_eq!(
        p.validate(
            &c,
            c.hash(),
            InvocationId {
                mission: MissionId(8),
                ..inv
            }
        ),
        Err(ContractError::PageInputIdentity)
    );
    let mut d = c.clone();
    d.notes[0].text.push('b');
    assert_eq!(
        p.validate(&d, c.hash(), inv),
        Err(ContractError::PageInputIdentity)
    );
    for k in 0..5 {
        let mut changed = p.clone();
        match k {
            0 => changed.note.text.push('b'),
            1 => changed.facts.name.push('b'),
            2 => changed.facts.hours[0].push('b'),
            3 => changed.facts.phone.push('8'),
            _ => changed.content_sha256 = "0".repeat(64),
        }
        assert_eq!(
            changed.validate(&c, c.hash(), inv),
            Err(ContractError::PageInputIdentity)
        );
    }
    for role in LaneRole::ALL
        .into_iter()
        .filter(|r| *r != LaneRole::Builder)
    {
        assert_eq!(prepare(&c, c.hash(), inv, role).unwrap(), None);
    }
    let legacy = contract();
    assert_eq!(
        prepare(&legacy, legacy.hash(), inv, LaneRole::Builder).unwrap(),
        None
    );
    // Actual coordinator input and LaneInvoked hash, not just constructors.
    let seen = Arc::new(Mutex::new(Vec::new()));
    let lanes = LaneRole::ALL
        .into_iter()
        .map(|role| {
            Arc::new(Capture {
                role,
                seen: seen.clone(),
            }) as Arc<dyn Lane>
        })
        .collect();
    let w = world_with(lanes);
    let id = w.approved(c.clone());
    for _ in 0..5 {
        w.mother.advance(id).unwrap();
    }
    let inputs = seen.lock().unwrap();
    assert_eq!(inputs.len(), 8);
    for i in inputs.iter() {
        assert_eq!(i.builder_input.is_some(), i.role == LaneRole::Builder);
        if let Some(p) = &i.builder_input {
            assert_eq!(p.validate(&c, i.contract_hash, i.invocation), Ok(()));
            assert_eq!(p.borrowed(), (&c.notes[0], c.page_facts.as_ref().unwrap()));
            let mut altered = i.clone();
            altered.builder_input.as_mut().unwrap().note.text.push('b');
            assert_eq!(
                input_hash(&altered),
                input_hash(i),
                "skipped content needs identity checking"
            );
            assert_eq!(
                altered
                    .builder_input
                    .as_ref()
                    .unwrap()
                    .validate(&c, i.contract_hash, i.invocation),
                Err(ContractError::PageInputIdentity)
            );
        }
        assert!(w.mother.journal().unwrap().iter().any(|e| matches!(&e.event,Event::LaneInvoked{invocation,input_hash:h,..} if *invocation==i.invocation && *h==input_hash(i))));
    }
    assert!(!format!("{p:?}").contains("1234567"));
}
struct Capture {
    role: LaneRole,
    seen: Arc<Mutex<Vec<LaneInput>>>,
}
impl Lane for Capture {
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: self.role,
            name: "da-capture".into(),
            version: "1".into(),
        }
    }
    fn invoke(&self, i: &LaneInput) -> LaneOutput {
        self.seen.lock().unwrap().push(i.clone());
        LaneOutput {
            text: "done".into(),
            effects: vec![],
            verdict: if self.role == LaneRole::Verifier {
                Verdict::Pass
            } else {
                Verdict::Continue
            },
        }
    }
}

#[test]
fn m3d_page_criteria_are_fixed_and_checkable() {
    let c = page();
    assert_eq!(page_criteria(), expected_criteria());
    assert_eq!(c.validate(), Ok(()));
    for k in 0..7 {
        let mut d = c.clone();
        match k {
            0 => d.pass_criteria[0].id = "other".into(),
            1 => d.pass_criteria[0].text.push('!'),
            2 => d.pass_criteria.swap(0, 1),
            3 => {
                d.pass_criteria.pop();
            }
            4 => d.pass_criteria.push(PassCriterion {
                id: "extra".into(),
                text: "Looks good".into(),
            }),
            5 => d.pass_criteria.clear(),
            _ => {
                d.pass_criteria = vec![PassCriterion {
                    id: "free".into(),
                    text: "Looks nice".into(),
                }]
            }
        }
        assert_eq!(
            d.validate(),
            Err(ContractError::CriterionNotCheckable),
            "case {k}"
        );
    }
    assert_eq!(contract().validate(), Ok(()));
}

#[test]
fn m3d_input_diagnostics_and_records_are_content_free() {
    let mut c = page();
    c.notes[0].text = "NOTE_CANARY_DA".into();
    c.page_facts.as_mut().unwrap().name = "FACT_CANARY_DA".into();
    let w = world();
    let id = w.approved(c.clone());
    for _ in 0..4 {
        w.mother.advance(id).unwrap();
    }
    let p = prepare(&c, c.hash(), invocation(), LaneRole::Builder)
        .unwrap()
        .unwrap();
    let surfaces = format!(
        "{c:?} {p:?} {:?} {:?} {}",
        c.page_facts,
        w.mother.confirmation(id).unwrap(),
        serde_json::to_string(&w.mother.journal().unwrap()).unwrap()
    );
    for canary in ["NOTE_CANARY_DA", "FACT_CANARY_DA"] {
        assert!(!surfaces.contains(canary));
    }
}
