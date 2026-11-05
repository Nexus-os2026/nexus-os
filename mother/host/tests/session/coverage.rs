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

// Every key is joined exactly once. The checked-in receipts contain actual
// runner output and its source-log digest; equivalences/exclusions cite a
// named structural rule as well as evidence at the affected sealed boundary.
pub fn verify(required: &BTreeSet<String>, dispositions: &str) -> Result<usize, String> {
    let mut receipts = std::collections::BTreeMap::new();
    for line in include_str!("combination-receipts.tsv").lines().skip(1) {
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 4
            || f[0].is_empty()
            || f[1].len() != 64
            || !f[1].bytes().all(|b| b.is_ascii_hexdigit())
            || !(f[3].contains("conforms=true") || f[3].ends_with(" ... ok"))
            || receipts.insert(f[0], f[3]).is_some()
        {
            return Err(format!("invalid execution receipt: {line}"));
        }
    }
    let mut rules = std::collections::BTreeMap::new();
    for line in include_str!("combination-rules.tsv").lines().skip(1) {
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 3
            || !matches!(f[1], "equivalence" | "exclusion")
            || f[2].len() < 80
            || rules.insert(f[0], f[1]).is_some()
        {
            return Err(format!("invalid structural justification: {line}"));
        }
    }
    let mut boundaries = std::collections::BTreeMap::new();
    for line in include_str!("combination-boundaries.tsv").lines().skip(1) {
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 4
            || f[1].is_empty()
            || f[3].is_empty()
            || f[2].split(',').any(|r| !receipts.contains_key(r))
            || boundaries
                .insert(f[0], f[2].split(',').collect::<BTreeSet<_>>())
                .is_some()
        {
            return Err(format!("invalid sealed-boundary witness: {line}"));
        }
    }
    let mut seen = BTreeSet::new();
    for line in dispositions.lines().skip(1) {
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 4 || !required.contains(f[0]) || !seen.insert(f[0].to_owned()) {
            return Err(format!("unexpected or duplicate combination: {line}"));
        }
        if f[3]
            .split(',')
            .any(|receipt| !receipts.contains_key(receipt))
        {
            return Err(format!("combination has no real receipt: {}", f[0]));
        }
        let sink = f[0].split(':').take(2).collect::<Vec<_>>().join(":");
        if !boundaries
            .get(sink.as_str())
            .is_some_and(|allowed| f[3].split(',').any(|r| allowed.contains(r)))
        {
            return Err(format!(
                "receipt does not witness the required sink: {}",
                f[0]
            ));
        }
        match f[1] {
            "execution" if f[2] == "exact" => {}
            "equivalence" | "exclusion" if rules.get(f[2]) == Some(&f[1]) => {}
            _ => {
                return Err(format!(
                    "combination has no justified disposition: {}",
                    f[0]
                ))
            }
        }
    }
    if seen != *required {
        return Err(format!(
            "missing combination dispositions: {:?}",
            required.difference(&seen).take(8).collect::<Vec<_>>()
        ));
    }
    Ok(seen.len())
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
