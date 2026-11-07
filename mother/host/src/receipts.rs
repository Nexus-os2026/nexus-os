//! M3-E M-6 (rulings M3E-R1, R18): the content-free `receipts` export.
//!
//! Fixed `key: value` lines (R18 (5)) of ids, hashes, counts and closed
//! codes, each with its provenance: measured by this host at start, read
//! from Mother's verified journal, fixed in this build, not measured, or
//! unavailable. No page, notes, facts text, answers or path. Values the host
//! did not measure are never shown as zero or a hash. Only the host-measured
//! and journal fields count toward `host-measured: complete`; the export
//! never claims to be complete overall (R18 (6)).
//!
//! Data-only (R18 (1)): no terminal alias or hands projection; M-5 calls it.

use crate::binary_identity::SelfIdentityUnavailable;
use crate::inputs::{sha256_hex, PageInputs};
use nexus_governed_control::authority::evidence::escaped;
use nexus_governed_control::inert_page::{policy_hash, ApprovedFacts, MOTHER_CSS};
use nexus_mother::coordinator::page_receipt::MissionReceipt;
use sha2::{Digest, Sha256};

/// What this host measured at start (R18 (7): the self-hash printed then).
pub(crate) struct HostMeasured<'a> {
    pub(crate) binary_sha256: Result<[u8; 32], SelfIdentityUnavailable>,
    pub(crate) inputs: &'a PageInputs,
}

/// The export, for `mission` (from the verified journal).
#[allow(dead_code)] // Consumed by M-5's page-mode wiring.
pub(crate) fn receipts(host: &HostMeasured<'_>, mission: &MissionReceipt) -> String {
    let mut incomplete: Vec<String> = Vec::new();
    let mut lines =
        vec!["receipts (content-free: no page, notes, facts text or answers)".to_string()];
    let binary = match &host.binary_sha256 {
        Ok(hash) => sha256_hex(hash),
        Err(_) => {
            incomplete.push("nexus-mother sha256".into());
            "unavailable".into()
        }
    };
    lines.push(format!("nexus-mother sha256: {binary} (measured at start)"));
    lines.push("mother-hands sha256: unavailable (external)".into());
    lines.push(format!(
        "policy hash: {} (this build)",
        sha256_hex(&policy_hash())
    ));
    let css: [u8; 32] = Sha256::digest(MOTHER_CSS.as_bytes()).into();
    lines.push(format!("css sha256: {} (this build)", sha256_hex(&css)));
    let request = nexus_mother::request_identity();
    lines.push(format!(
        "system prompt sha256: {} (this build)",
        sha256_hex(&request.system)
    ));
    lines.push(format!(
        "task prompt sha256: {} (this build)",
        sha256_hex(&request.task)
    ));
    lines.push(format!(
        "schema sha256: {} (this build)",
        sha256_hex(&request.schema)
    ));
    for (code, hash) in &request.corrections {
        lines.push(format!(
            "correction {code} sha256: {} (this build)",
            sha256_hex(hash)
        ));
    }
    let inputs = host.inputs;
    lines.push(format!(
        "notes file sha256: {} (measured at start)",
        sha256_hex(&inputs.notes_sha256)
    ));
    lines.push(format!(
        "facts file sha256: {} (measured at start)",
        sha256_hex(&inputs.facts_sha256)
    ));
    let goal = inputs
        .goal_sha256
        .as_ref()
        .map_or("none".into(), sha256_hex);
    lines.push(format!("goal file sha256: {goal} (measured at start)"));
    let hours: Vec<&str> = inputs.facts.hours.iter().map(String::as_str).collect();
    let snapshot = ApprovedFacts::new(&inputs.facts.name, &hours, &inputs.facts.phone)
        .ok()
        .map(|a| sha256_hex(&a.hash()));
    if snapshot.is_none() {
        incomplete.push("facts hash, snapshot".into());
    }
    lines.push(format!(
        "facts hash, snapshot: {} (measured at start)",
        snapshot.as_deref().unwrap_or("unavailable")
    ));
    lines.extend(mission_lines(mission, &mut incomplete));
    if snapshot.is_some() && mission.facts_hash.is_some() && snapshot != mission.facts_hash {
        incomplete.push("identity:facts".into());
    }
    lines.push("model and runtime: unavailable (model not registered)".into());
    lines.push("timing (dispatch, return, t_stop): not measured (D6 deferred)".into());
    lines.push("build, CI, toolchain, installed files, host: unavailable (external)".into());
    lines.push(if incomplete.is_empty() {
        "host-measured: complete".into()
    } else {
        format!("host-measured: incomplete ({})", incomplete.join(", "))
    });
    lines.push("overall: incomplete (external and timing fields are not provided)".into());
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

fn mission_lines(m: &MissionReceipt, incomplete: &mut Vec<String>) -> Vec<String> {
    let mut lines = vec![
        format!("mission: {}", m.mission),
        format!(
            "contract hash: {} (mother journal)",
            escaped(&m.contract_hash)
        ),
        format!(
            "consent: journal seq {}, entry {}, at_wall_ms {} (mother journal)",
            m.consent_seq,
            escaped(&m.consent_entry_hash),
            m.consent_at_wall_ms
        ),
        "consent evidence_seq: unavailable (not journaled)".to_string(),
        format!(
            "facts hash, consented: {} (mother journal)",
            m.facts_hash
                .as_deref()
                .map_or("unavailable".into(), escaped)
        ),
    ];
    let calls: Vec<String> = m
        .lane_calls
        .iter()
        .map(|(role, n)| format!("{role} {n}"))
        .collect();
    lines.push(format!("lane calls: {} (mother journal)", calls.join(", ")));
    lines.push(format!(
        "model calls charged: {} (mother journal)",
        m.model_calls_charged
    ));
    lines.push(format!(
        "effects dispatched: {} (mother journal)",
        m.effects_dispatched
    ));
    let publication = match &m.publication {
        Some(p) => format!(
            "sha256 {}, bytes {}, policy {}, overtaken {}",
            escaped(&p.sha256),
            p.bytes,
            p.policy_hash
                .as_deref()
                .map_or("unavailable".into(), escaped),
            if p.overtaken { "yes" } else { "no" }
        ),
        None if m.missing.contains(&"publication_receipt") => "unavailable".into(),
        None => "none".into(),
    };
    lines.push(format!(
        "publication receipt: {publication} (mother journal)"
    ));
    let checks = match &m.checks {
        Some(c) if !c.is_empty() => c
            .iter()
            .map(|(id, code)| format!("{} {}", escaped(id), escaped(code)))
            .collect::<Vec<_>>()
            .join(", "),
        _ => "none recorded".into(),
    };
    lines.push(format!("checks: {checks} (mother journal)"));
    lines.push(format!(
        "report digest: {} (mother journal)",
        m.report_digest.as_deref().map_or("none".into(), escaped)
    ));
    lines.push(format!("end: {} (mother journal)", m.end));
    incomplete.extend(m.missing.iter().map(|f| f.to_string()));
    incomplete.extend(m.mismatched.iter().map(|f| format!("identity:{f}")));
    lines
}

// R14/R18 (2): `receipts` is crate-private, so its oracles are here.
#[cfg(test)]
mod oracles {
    use super::{receipts, HostMeasured};
    use crate::binary_identity::SelfIdentityUnavailable;
    use crate::inputs::{sha256_hex, PageInputs};
    use nexus_governed_control::inert_page::{policy_hash, ApprovedFacts, MOTHER_CSS};
    use nexus_mother::contract::Note;
    use nexus_mother::coordinator::page_receipt::{MissionReceipt, PublicationFacts};
    use nexus_mother::journal::MissionId;
    use nexus_mother::page_inputs::PageFacts;
    use sha2::{Digest, Sha256};

    const CANARIES: [&str; 4] = ["ZqNoteCanary", "ZqFactCanary", "555-0199", "/home/zq"];

    fn inputs() -> PageInputs {
        PageInputs {
            facts: PageFacts {
                name: "ZqFactCanary Bakery".into(),
                hours: vec!["Mondays 9 to 5".into()],
                phone: "555-0199".into(),
            },
            note: Note {
                label: "notes".into(),
                text: "ZqNoteCanary bread, see /home/zq/notes.txt".into(),
            },
            goal: Some("ZqNoteCanary goal".into()),
            facts_sha256: [0x11; 32],
            notes_sha256: [0x22; 32],
            goal_sha256: Some([0x33; 32]),
        }
    }
    fn snapshot_hash(i: &PageInputs) -> String {
        let hours: Vec<&str> = i.facts.hours.iter().map(String::as_str).collect();
        sha256_hex(
            &ApprovedFacts::new(&i.facts.name, &hours, &i.facts.phone)
                .unwrap()
                .hash(),
        )
    }
    fn mission(facts_hash: String) -> MissionReceipt {
        MissionReceipt {
            mission: MissionId(4),
            contract_hash: "44".repeat(32),
            consent_seq: 12,
            consent_entry_hash: "55".repeat(32),
            consent_at_wall_ms: 1_700_000,
            facts_hash: Some(facts_hash),
            lane_calls: vec![
                ("researcher_a", 1),
                ("researcher_b", 1),
                ("skeptic", 1),
                ("builder", 1),
                ("verifier", 1),
                ("sentinel", 0),
            ],
            model_calls_charged: 1,
            effects_dispatched: 1,
            publication: Some(PublicationFacts {
                sha256: "66".repeat(32),
                bytes: 1697,
                policy_hash: Some("77".repeat(32)),
                overtaken: false,
            }),
            checks: Some(
                ["v0", "v1", "v2", "v3", "v4", "v5"]
                    .map(|id| (id.to_string(), "pass".to_string()))
                    .to_vec(),
            ),
            report_digest: Some("88".repeat(32)),
            end: "done",
            missing: vec![],
            mismatched: vec![],
            invalid: vec![],
        }
    }
    fn build_lines() -> String {
        let r = nexus_mother::request_identity();
        let css: [u8; 32] = Sha256::digest(MOTHER_CSS.as_bytes()).into();
        let mut s = format!(
            "policy hash: {} (this build)\ncss sha256: {} (this build)\nsystem prompt sha256: {} (this build)\ntask prompt sha256: {} (this build)\nschema sha256: {} (this build)\n",
            sha256_hex(&policy_hash()),
            sha256_hex(&css),
            sha256_hex(&r.system),
            sha256_hex(&r.task),
            sha256_hex(&r.schema)
        );
        for (code, h) in &r.corrections {
            s.push_str(&format!(
                "correction {code} sha256: {} (this build)\n",
                sha256_hex(h)
            ));
        }
        s
    }

    #[test]
    fn m3e_u10_receipts_exact_schema_and_privacy() {
        let i = inputs();
        let host = HostMeasured {
            binary_sha256: Ok([0x99; 32]),
            inputs: &i,
        };
        let snap = snapshot_hash(&i);
        let text = receipts(&host, &mission(snap.clone()));
        let h = |b: u8| format!("{b:02x}").repeat(32);
        let expected = format!(
            "receipts (content-free: no page, notes, facts text or answers)\n\
nexus-mother sha256: {} (measured at start)\n\
mother-hands sha256: unavailable (external)\n\
{}\
notes file sha256: {} (measured at start)\n\
facts file sha256: {} (measured at start)\n\
goal file sha256: {} (measured at start)\n\
facts hash, snapshot: {snap} (measured at start)\n\
mission: msn-000004\n\
contract hash: {} (mother journal)\n\
consent: journal seq 12, entry {}, at_wall_ms 1700000 (mother journal)\n\
consent evidence_seq: unavailable (not journaled)\n\
facts hash, consented: {snap} (mother journal)\n\
lane calls: researcher_a 1, researcher_b 1, skeptic 1, builder 1, verifier 1, sentinel 0 (mother journal)\n\
model calls charged: 1 (mother journal)\n\
effects dispatched: 1 (mother journal)\n\
publication receipt: sha256 {}, bytes 1697, policy {}, overtaken no (mother journal)\n\
checks: v0 pass, v1 pass, v2 pass, v3 pass, v4 pass, v5 pass (mother journal)\n\
report digest: {} (mother journal)\n\
end: done (mother journal)\n\
model and runtime: unavailable (model not registered)\n\
timing (dispatch, return, t_stop): not measured (D6 deferred)\n\
build, CI, toolchain, installed files, host: unavailable (external)\n\
host-measured: complete\n\
overall: incomplete (external and timing fields are not provided)\n",
            h(0x99),
            build_lines(),
            h(0x22),
            h(0x11),
            h(0x33),
            h(0x44),
            h(0x55),
            h(0x66),
            h(0x77),
            h(0x88),
        );
        assert_eq!(text, expected);
        // Content-free: no note, fact or path canary, nor in Debug.
        for canary in CANARIES {
            assert!(!text.contains(canary), "{canary}");
            assert!(!format!("{i:?} {:?}", mission(snap.clone())).contains(canary));
        }
        // Repeated: identical.
        assert_eq!(receipts(&host, &mission(snap)), text);
    }

    #[test]
    fn m3e_u10_receipts_missing_is_not_zero() {
        let i = inputs();
        let host = HostMeasured {
            binary_sha256: Err(SelfIdentityUnavailable),
            inputs: &i,
        };
        let mut m = mission(snapshot_hash(&i));
        m.publication = None;
        m.missing = vec!["publication_receipt"];
        m.facts_hash = None;
        let text = receipts(&host, &m);
        assert!(text.contains("nexus-mother sha256: unavailable (measured at start)\n"));
        assert!(text.contains("publication receipt: unavailable (mother journal)\n"));
        assert!(text.contains("facts hash, consented: unavailable (mother journal)\n"));
        assert!(
            text.contains("host-measured: incomplete (nexus-mother sha256, publication_receipt)\n")
        );
        assert!(!text.contains(&"0".repeat(64)), "no invented zero hash");
        assert!(!text.contains("host-measured: complete"));
        // Not applicable is `none`, never `unavailable`; timing never a number.
        let mut declined = mission(snapshot_hash(&i));
        declined.publication = None;
        declined.checks = None;
        declined.report_digest = None;
        let ok = HostMeasured {
            binary_sha256: Ok([1; 32]),
            inputs: &i,
        };
        let text = receipts(&ok, &declined);
        assert!(text.contains("publication receipt: none (mother journal)\n"));
        assert!(text.contains("checks: none recorded (mother journal)\n"));
        assert!(text.contains("timing (dispatch, return, t_stop): not measured (D6 deferred)\n"));
        assert!(text.contains("overall: incomplete"));
        assert!(text.contains("host-measured: complete\n"));
    }

    #[test]
    fn m3e_u10_timestamps_and_identities_join() {
        let i = inputs();
        let host = HostMeasured {
            binary_sha256: Ok([1; 32]),
            inputs: &i,
        };
        // The consented facts are not the snapshot the host read: incomplete.
        let text = receipts(&host, &mission("ab".repeat(32)));
        assert!(
            text.contains("host-measured: incomplete (identity:facts)\n"),
            "{text}"
        );
        // A receipt or record for another effect: incomplete, named.
        let mut m = mission(snapshot_hash(&i));
        m.mismatched = vec!["receipt_effect", "verification_effect"];
        let text = receipts(&host, &m);
        assert!(text.contains(
            "host-measured: incomplete (identity:receipt_effect, identity:verification_effect)\n"
        ));
        // No dispatch, return or t_stop value is ever exported (D6).
        for word in ["sent_mono_ms", "returned_mono_ms", "stop_mono_ms"] {
            assert!(!text.contains(word));
        }
    }

    // M3E-R19: each gap is named by its kind; invalid values are never shown.
    #[test]
    fn m3e_u10_r19_host_names_each_gap() {
        let i = inputs();
        let host = HostMeasured {
            binary_sha256: Ok([1; 32]),
            inputs: &i,
        };
        let mut m = mission(snapshot_hash(&i));
        m.publication = None;
        m.checks = None;
        m.report_digest = None;
        m.missing = vec!["publication_receipt", "publication_policy", "action-link"];
        m.mismatched = vec!["action"];
        m.invalid = vec!["checks", "report_digest"];
        let text = receipts(&host, &m);
        assert!(
            text.contains("publication receipt: unavailable (mother journal)\n"),
            "{text}"
        );
        assert!(
            text.contains("checks: invalid (mother journal)\n"),
            "{text}"
        );
        assert!(
            text.contains("report digest: invalid (mother journal)\n"),
            "{text}"
        );
        assert!(text.contains("host-measured: incomplete (missing:publication_receipt, missing:publication_policy, missing:action-link, mismatched:action, invalid:checks, invalid:report_digest)\n"), "{text}");
    }
}
