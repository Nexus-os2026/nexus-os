//! M2a: Mother's consent exists only from the broker's `confirm_mission`
//! yes for that exact contract hash and request. A different hash, an
//! answer to another request (a replay) or a broker that cannot record
//! gives no consent. The broker records every decision.

use super::*;
use crate::consent::Consent;
use crate::journal::Event;
use nexus_governed_control::authority::evidence::{EvidencePhase, EvidenceRecord};
use nexus_governed_control::authority::AuthorityError;

const COORDINATOR: &str = include_str!("../coordinator.rs");
const CONSENT: &str = include_str!("../consent.rs");
const EFFECT: &str = include_str!("../effect.rs");

fn code(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn decisions(w: &World) -> Vec<EvidenceRecord> {
    w.port
        .evidence
        .records()
        .into_iter()
        .filter(|r| {
            matches!(
                r.phase,
                EvidencePhase::MissionConsented | EvidencePhase::MissionDeclined
            )
        })
        .collect()
}

fn detail<'a>(record: &'a EvidenceRecord, key: &str) -> Option<&'a str> {
    record
        .detail
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn refused_entries(w: &World, id: MissionId) -> usize {
    w.mother
        .journal()
        .unwrap()
        .iter()
        .filter(|e| e.mission == id && matches!(e.event, Event::ConsentRefused { .. }))
        .count()
}

/// Says yes to every dialog it knows and never implemented the mission
/// dialog (the broker's default answers it).
struct Legacy;
impl ControlConfirmer for Legacy {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        true
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        true
    }
}

#[test]
fn m2a_consent_comes_from_the_broker_s_yes_and_the_broker_records_it() {
    let w = world();
    let c = contract();
    let id = w.submitted(c.clone());
    assert_eq!(decisions(&w).len(), 0);
    assert_eq!(
        w.mother.request_consent(id, &Yes),
        Ok(MissionState::Approved)
    );
    assert_eq!(w.mother.consented_hash(id), Some(c.hash()));
    let records = decisions(&w);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].phase, EvidencePhase::MissionConsented);
    assert_eq!(
        detail(&records[0], "contract_hash"),
        Some(c.hash().to_hex().as_str())
    );
    let request = detail(&records[0], "request").expect("the request is recorded");
    assert!(request.starts_with(&id.to_string()), "{request}");
    // Nothing reached the effect port.
    assert_eq!(w.port.calls(), vec![]);
}

#[test]
fn m2a_the_broker_s_no_is_a_denial_and_the_broker_records_it() {
    let w = world();
    let c = contract();
    let id = w.submitted(c.clone());
    assert_eq!(
        w.mother.request_consent(id, &No),
        Err(CoordinatorError::Denied)
    );
    assert_eq!(w.mother.state(id), Some(MissionState::Denied));
    assert_eq!(w.mother.consented_hash(id), None);
    let records = decisions(&w);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].phase, EvidencePhase::MissionDeclined);
    assert_eq!(
        detail(&records[0], "contract_hash"),
        Some(c.hash().to_hex().as_str())
    );
    assert_eq!(w.port.calls(), vec![]);
}

#[test]
fn m2a_a_confirmer_without_a_mission_dialog_gives_no_consent() {
    let w = world();
    let id = w.submitted(contract());
    assert_eq!(
        w.mother.request_consent(id, &Legacy),
        Err(CoordinatorError::Denied)
    );
    assert_eq!(w.mother.state(id), Some(MissionState::Denied));
    assert_eq!(w.mother.consented_hash(id), None);
    assert!(w.mother.advance(id).is_err());
    assert_eq!(w.port.calls(), vec![]);
}

#[test]
fn m2a_a_yes_the_broker_cannot_record_gives_no_consent() {
    let w = world();
    let id = w.submitted(contract());
    w.port.evidence.fail_from_now();
    assert_eq!(
        w.mother.request_consent(id, &Yes),
        Err(CoordinatorError::Broker("evidence_unavailable".into()))
    );
    // Not a denial: the mission still awaits a consent the broker records.
    assert_eq!(w.mother.state(id), Some(MissionState::AwaitingConsent));
    assert_eq!(w.mother.consented_hash(id), None);
    assert!(w.mother.advance(id).is_err());
    assert_eq!(refused_entries(&w, id), 1);
    let w = w.restart();
    assert_eq!(w.mother.state(id), Some(MissionState::AwaitingConsent));
    assert_eq!(w.mother.consented_hash(id), None);
    assert_eq!(w.port.calls(), vec![]);
}

#[test]
fn m2a_a_broker_answer_for_another_contract_hash_is_refused() {
    let w = world();
    let a = w.submitted(contract_with("first"));
    let b = w.submitted(contract_with("second"));
    let (request_a, _) = w.mother.consent_request(a).unwrap();
    let (request_b, generation_b) = w.mother.consent_request(b).unwrap();
    assert_ne!(request_a.contract_hash, request_b.contract_hash);
    let for_a = w.port.broker.confirm_mission(&request_a, &Yes).unwrap();
    assert_eq!(
        w.mother.take_answer(b, generation_b, Ok(for_a)),
        Err(CoordinatorError::ConsentMismatch)
    );
    assert_eq!(w.mother.state(b), Some(MissionState::AwaitingConsent));
    assert_eq!(w.mother.consented_hash(b), None);
    assert!(w.mother.advance(b).is_err());
    assert_eq!(refused_entries(&w, b), 1);
    // The mission can still be consented, through its own request.
    assert_eq!(
        w.mother.request_consent(b, &Yes),
        Ok(MissionState::Approved)
    );
    assert_eq!(
        w.mother.consented_hash(b),
        Some(contract_with("second").hash())
    );
    assert_eq!(w.mother.consented_hash(a), None);
    assert_eq!(w.port.calls(), vec![]);
}

#[test]
fn m2a_a_replayed_broker_answer_is_refused() {
    let w = world();
    // Two missions with the same contract: the same hash.
    let a = w.submitted(contract());
    let b = w.submitted(contract());
    let (request_a, _) = w.mother.consent_request(a).unwrap();
    let (request_b, generation_b) = w.mother.consent_request(b).unwrap();
    assert_eq!(request_a.contract_hash, request_b.contract_hash);
    assert_ne!(request_a.request, request_b.request);
    // A's answer presented for B.
    let for_a = w.port.broker.confirm_mission(&request_a, &Yes).unwrap();
    assert_eq!(
        w.mother.take_answer(b, generation_b, Ok(for_a)),
        Err(CoordinatorError::ConsentMismatch)
    );
    assert_eq!(w.mother.consented_hash(b), None);
    assert_eq!(w.mother.state(b), Some(MissionState::AwaitingConsent));

    // An answer to an earlier request of the same mission, once a newer
    // request was made.
    let (first, _) = w.mother.consent_request(a).unwrap();
    let old = w.port.broker.confirm_mission(&first, &Yes).unwrap();
    let (second, generation) = w.mother.consent_request(a).unwrap();
    assert_ne!(first.request, second.request);
    assert_eq!(
        w.mother.take_answer(a, generation, Ok(old)),
        Err(CoordinatorError::ConsentMismatch)
    );
    assert_eq!(w.mother.consented_hash(a), None);
    assert_eq!(w.mother.state(a), Some(MissionState::AwaitingConsent));

    // The right answer to the latest request is accepted, exactly once.
    let fresh = w.port.broker.confirm_mission(&second, &Yes).unwrap();
    assert_eq!(
        w.mother.take_answer(a, generation, Ok(fresh)),
        Ok(MissionState::Approved)
    );
    let again = w.port.broker.confirm_mission(&second, &Yes).unwrap();
    assert!(w.mother.take_answer(a, generation, Ok(again)).is_err());
    assert_eq!(w.mother.state(a), Some(MissionState::Approved));
    assert_eq!(w.port.calls(), vec![]);
}

#[test]
fn m2a_a_broker_refusal_is_not_a_consent() {
    let w = world();
    let id = w.submitted(contract());
    let (_, generation) = w.mother.consent_request(id).unwrap();
    assert_eq!(
        w.mother
            .take_answer(id, generation, Err(AuthorityError::EmergencyStopped)),
        Err(CoordinatorError::Broker("emergency_stopped".into()))
    );
    assert_eq!(w.mother.state(id), Some(MissionState::AwaitingConsent));
    assert_eq!(w.mother.consented_hash(id), None);
}

#[test]
fn m2a_consent_from_broker_binds_hash_and_request() {
    let w = world();
    let id = w.submitted(contract());
    let (request, _) = w.mother.consent_request(id).unwrap();
    let approval = w.port.broker.confirm_mission(&request, &Yes).unwrap();
    let consent = Consent::from_broker(approval, &request).expect("the exact request");
    assert_eq!(consent.hash(), contract().hash());

    let mut other_hash = request.clone();
    other_hash.contract_hash = contract_with("x").hash().to_hex();
    let approval = w.port.broker.confirm_mission(&request, &Yes).unwrap();
    assert!(Consent::from_broker(approval, &other_hash).is_none());

    let mut other_request = request.clone();
    other_request.request.push('x');
    let approval = w.port.broker.confirm_mission(&request, &Yes).unwrap();
    assert!(Consent::from_broker(approval, &other_request).is_none());
}

#[test]
fn m2a_live_consent_is_minted_only_from_the_broker_s_approval() {
    let coordinator = code(COORDINATOR);
    // The broker is asked in exactly one place.
    assert_eq!(coordinator.matches("confirm_mission(").count(), 1);
    // A live consent is made from the broker's approval; `minted` is only
    // the replay of a verified journal.
    assert_eq!(coordinator.matches("Consent::from_broker(").count(), 1);
    assert_eq!(coordinator.matches("Consent::minted(").count(), 1);
    let replay = coordinator.find("\nfn replay(").expect("replay");
    assert!(coordinator.find("Consent::minted(").unwrap() > replay);
    assert!(coordinator.find("Consent::from_broker(").unwrap() < replay);
    // The approval is taken by value: it is consumed.
    let consent = code(CONSENT);
    let squeezed: String = consent.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(squeezed.contains("pub(crate)fnfrom_broker(approval:MissionApproval,"));
    // Mother's own confirmer trait is gone; the port reaches the broker.
    assert!(!consent.contains("pub trait"));
    assert!(code(EFFECT).contains("fn broker(&self) -> &Authority;"));
}
