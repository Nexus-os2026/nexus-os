//! Mother Nexus M2a oracle (host, public API): mission consent in the
//! broker. `ControlConfirmer::confirm_mission` asks the owner about one
//! exact contract hash; the broker records the decision in the evidence
//! store of record and, only on a recorded yes, returns a `MissionApproval`
//! naming that hash and request. Pre-registered: committed before the
//! implementation and not edited after a result is seen.
//!
//! One test: a non-test build of the broker claims the process's
//! emergency-stop controller, so one front door per process. The scenarios
//! run in order on it, the emergency stop last.
#![cfg(target_os = "linux")]

use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ControlConfirmer, GrantConfirmation,
    MissionConfirmation, ResumeConfirmation,
};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::evidence::{EvidencePhase, EvidenceRecord, MemoryEvidence};
use nexus_governed_control::authority::AuthorityError;
use nexus_governed_control::broker::Vault;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::governed::GovernedControl;
use std::sync::{Arc, Mutex};

const HASH_A: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const HASH_B: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

/// Answers the mission dialog with `mission` and records what it was shown;
/// every other dialog is answered yes, so that only `confirm_mission`
/// can be what a mission's consent comes from.
struct Owner {
    mission: bool,
    shown: Mutex<Vec<MissionConfirmation>>,
}

impl Owner {
    fn new(mission: bool) -> Self {
        Self {
            mission,
            shown: Mutex::new(Vec::new()),
        }
    }
    fn asked(&self) -> usize {
        self.shown.lock().unwrap().len()
    }
}

impl ControlConfirmer for Owner {
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
    fn confirm_mission(&self, request: &MissionConfirmation) -> bool {
        self.shown.lock().unwrap().push(request.clone());
        self.mission
    }
}

/// A confirmer written before mission consent existed: it says yes to
/// every dialog it knows and does not implement `confirm_mission`. The
/// trait's own answer must be no.
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

fn request(hash: &str, request: &str) -> MissionConfirmation {
    MissionConfirmation {
        contract_hash: hash.into(),
        request: request.into(),
        summary: vec![
            "Your words, exactly:".into(),
            "│ Make me a small website for my bakery.".into(),
            "Budget: 30 minutes, 20 lane calls, money 0".into(),
        ],
    }
}

fn mission_records(history: &MemoryEvidence) -> Vec<EvidenceRecord> {
    history
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

/// The phases the store of record holds, in order.
fn stored_phases(path: &std::path::Path) -> Vec<String> {
    let db =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let mut statement = db.prepare("SELECT phase FROM record ORDER BY seq").unwrap();
    let phases = statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    phases
}

#[test]
fn m2a_mission_consent_is_the_broker_s_and_is_recorded() {
    let root = std::env::temp_dir().join(format!("nexus-m2a-consent-{}", std::process::id()));
    let store = root.join("p3-evidence").join("evidence.sqlite3");
    let history = Arc::new(MemoryEvidence::new(256));
    let evidence = PrimaryEvidence::open(&store, vec![history.clone()]).unwrap();
    let control = GovernedControl::new(
        &root,
        Vault::Disabled,
        evidence,
        Arc::new(SystemClock::default()),
    )
    .unwrap();

    // The dialog's text: the hash whole in the header, the summary in the
    // details.
    let shown = request(HASH_A, "msn-000001/2/1");
    assert_eq!(shown.title(), "Start this mission?");
    let text = shown.text();
    assert!(
        text.header.iter().any(|(_, value)| value == HASH_A),
        "{text:?}"
    );
    assert!(text
        .header
        .iter()
        .any(|(_, value)| value == "msn-000001/2/1"));
    assert_eq!(text.details, shown.summary);

    // Yes: an approval naming exactly the hash and the request asked, and
    // the decision in the store of record.
    let owner = Owner::new(true);
    let approval = control.confirm_mission(&shown, &owner).unwrap();
    assert_eq!(owner.asked(), 1);
    assert_eq!(owner.shown.lock().unwrap()[0], shown);
    assert_eq!(approval.contract_hash(), HASH_A);
    assert_eq!(approval.request(), "msn-000001/2/1");
    let records = mission_records(&history);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].phase, EvidencePhase::MissionConsented);
    assert_eq!(detail(&records[0], "contract_hash"), Some(HASH_A));
    assert_eq!(detail(&records[0], "request"), Some("msn-000001/2/1"));
    assert!(detail(&records[0], "summary_digest").is_some_and(|d| d.len() == 64));
    assert!(approval.evidence_seq() > 0);

    // Each answer is its own: a second yes is a second approval, for the
    // hash it was asked about.
    let second = control
        .confirm_mission(&request(HASH_B, "msn-000002/9/2"), &owner)
        .unwrap();
    assert_eq!(second.contract_hash(), HASH_B);
    assert_eq!(second.request(), "msn-000002/9/2");
    assert_ne!(second.evidence_seq(), approval.evidence_seq());

    // No: refused as declined, and the decline is recorded.
    let no = Owner::new(false);
    assert_eq!(
        control
            .confirm_mission(&request(HASH_A, "msn-000003/4/3"), &no)
            .err(),
        Some(AuthorityError::Declined)
    );
    assert_eq!(no.asked(), 1);
    let records = mission_records(&history);
    assert_eq!(records.len(), 3);
    assert_eq!(records[2].phase, EvidencePhase::MissionDeclined);
    assert_eq!(detail(&records[2], "contract_hash"), Some(HASH_A));

    // A confirmer that never implemented the mission dialog declines it,
    // whatever it answers to every other dialog.
    assert_eq!(
        control
            .confirm_mission(&request(HASH_A, "msn-000004/5/4"), &Legacy)
            .err(),
        Some(AuthorityError::Declined)
    );

    // Refused before the owner is asked: a hash that is not a SHA-256 hex
    // digest, an empty or hidden-character request or summary.
    let asked = owner.asked();
    let mut bad = vec![
        request("", "r"),
        request(&HASH_A[..63], "r"),
        request(&HASH_A.to_uppercase(), "r"),
        request(&format!("{}g", &HASH_A[..63]), "r"),
        request(HASH_A, ""),
        request(HASH_A, "r\u{202e}x"),
        request(HASH_A, "r\nx"),
    ];
    let mut empty = request(HASH_A, "r");
    empty.summary.clear();
    bad.push(empty);
    let mut hidden = request(HASH_A, "r");
    hidden.summary.push("approved\u{200b}".into());
    bad.push(hidden);
    let mut newline = request(HASH_A, "r");
    newline.summary.push("line one\nline two".into());
    bad.push(newline);
    let mut long = request(HASH_A, "r");
    long.summary.push("x".repeat(257));
    bad.push(long);
    for bad in bad {
        assert!(
            matches!(
                control.confirm_mission(&bad, &owner),
                Err(AuthorityError::InvalidAction(_))
            ),
            "{bad:?}"
        );
    }
    assert_eq!(
        owner.asked(),
        asked,
        "the owner was asked an invalid request"
    );
    assert_eq!(mission_records(&history).len(), 4);

    // The store of record holds each decision.
    let phases = stored_phases(&store);
    assert_eq!(
        phases.iter().filter(|p| *p == "mission_consented").count(),
        2,
        "{phases:?}"
    );
    assert_eq!(
        phases.iter().filter(|p| *p == "mission_declined").count(),
        2,
        "{phases:?}"
    );

    // Under the owner's emergency stop no mission is asked about (last: it
    // is not lifted).
    control.emergency_stop();
    assert_eq!(
        control
            .confirm_mission(&request(HASH_A, "msn-000005/6/5"), &owner)
            .err(),
        Some(AuthorityError::EmergencyStopped)
    );
    assert_eq!(owner.asked(), asked);
    assert_eq!(mission_records(&history).len(), 4);
    let _ = std::fs::remove_dir_all(&root);
}
