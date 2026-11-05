//! Exact generated-combination coverage, independent of production admission.
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug)]
enum Pause {
    Accepted,
    Captured,
    BeforeDialogue,
    AfterDialogue,
    BeforeSlow,
    AfterSlow,
    BeforeHold,
    Reserved,
    BeforeAdmission,
    AfterAdmission,
    KernelEntry,
    PublicationObservation,
    PhysicalEffect,
    Successor,
    Transfer,
    CloseAssessment,
    Quiescence,
    Seal,
    EndRecord,
    Unwind,
}
const PAUSES: [Pause; 20] = [
    Pause::Accepted,
    Pause::Captured,
    Pause::BeforeDialogue,
    Pause::AfterDialogue,
    Pause::BeforeSlow,
    Pause::AfterSlow,
    Pause::BeforeHold,
    Pause::Reserved,
    Pause::BeforeAdmission,
    Pause::AfterAdmission,
    Pause::KernelEntry,
    Pause::PublicationObservation,
    Pause::PhysicalEffect,
    Pause::Successor,
    Pause::Transfer,
    Pause::CloseAssessment,
    Pause::Quiescence,
    Pause::Seal,
    Pause::EndRecord,
    Pause::Unwind,
];
const INGRESS: [&str; 17] = [
    "CR",
    "LF",
    "CRLF",
    "original_cell_unscheduled",
    "repeated",
    "EOF",
    "partial_Ctrl_D",
    "master_close",
    "read_failure",
    "mission_stop",
    "stop_all",
    "door_fence",
    "budget",
    "grant_revocation",
    "grant_expiry",
    "quit",
    "fatal",
];
const FAILURES: [&str; 6] = [
    "none",
    "decline",
    "record_failed",
    "record_unknown",
    "poison",
    "panic",
];

pub fn required() -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for line in include_str!("sinks.tsv").lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        if !matches!(fields[1], "W" | "R") {
            continue;
        }
        for pause in PAUSES {
            for ingress in INGRESS {
                for failure in FAILURES {
                    keys.insert(format!(
                        "{}:{}:{pause:?}:{ingress}:{failure}",
                        fields[0], fields[2]
                    ));
                }
            }
        }
    }
    keys
}

// Baseline extracted without changing the old coverage predicate. The red
// oracle below demonstrates that it accepts an undisposed required key.
pub fn verify(required: &BTreeSet<String>, _dispositions: &str) -> Result<usize, String> {
    if required.iter().any(|key| key.is_empty()) {
        return Err("empty planning key".to_owned());
    }
    Ok(required.len())
}

#[test]
fn f11_orphan_generated_combination_is_rejected() {
    let key = "R37:private_write:BeforeHold:EOF:none";
    let required = BTreeSet::from([key.to_owned()]);
    assert!(
        verify(&required, "").is_err(),
        "F11: required {key} has neither execution nor justified exclusion"
    );
}

#[test]
fn f11_every_required_combination_needs_a_disposition() {
    let required = required();
    assert_eq!(required.len(), 153_000);
    let dispositions = include_str!("combination-dispositions.tsv");
    assert_eq!(verify(&required, dispositions).unwrap(), required.len());
}
