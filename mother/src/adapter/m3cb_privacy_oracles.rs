//! Cb2 privacy composition: each lifecycle schedule (with its own judgments)
//! reruns with canary notes/facts, then every evidence surface is scanned.
use super::{
    c1b_port_oracles::children,
    m3cb_lifecycle_oracles::{run as schedule, staged, FACT, LATE},
    m3cb_support::Scenario,
};
use crate::effect::EffectRequest;
use std::path::Path;

const VAR: &str = "NEXUS_M3CB2_PRIVACY";
const REJECT: &str = "CB_REJECT_CANARY";
const TRANSPORT: &str = "CB_TRANSPORT_CANARY";
fn cases(test: &str, list: &[&str]) {
    children("adapter::m3cb_privacy_oracles", VAR, test, list, check);
}
/// Every regular file below `dir`: (count, lossy text).
fn tree(dir: &Path) -> (usize, String) {
    let (mut n, mut text, mut pending) = (0, String::new(), vec![dir.to_path_buf()]);
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                n += 1;
                text.push_str(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
            }
        }
    }
    (n, text)
}
fn absent(what: &str, text: &str, canaries: &[&str]) {
    for canary in canaries {
        assert!(!text.contains(canary), "{canary} leaked into {what}");
        let hex = hex::encode(canary);
        assert!(!text.contains(&hex), "{canary} leaked (hex) into {what}");
        assert!(
            !text.contains(&hex.to_uppercase()),
            "{canary} leaked (HEX) into {what}"
        );
    }
}
fn check(case: &str) {
    let (kind, rest) = case.split_once(':').unwrap();
    let (s, diag) = schedule(rest, true);
    // The canary contract was the one consented and lent (positive control).
    assert!(s.c.notes[0].text.starts_with("CB_NOTE_CANARY"));
    assert_eq!(s.c.page_facts.as_ref().unwrap().hours, [FACT]);
    if kind == "records" {
        records(&s, &diag);
    } else {
        drafts(&s, rest);
    }
}
fn records(s: &Scenario, diag: &str) {
    let all = ["CB_NOTE_CANARY", "面包", FACT, REJECT, LATE, TRANSPORT];
    let o = s.output();
    let observed = s.seen.lock().unwrap().results.join(" ");
    let surfaces = [
        ("returned errors", diag.to_string()),
        ("caller results", observed),
        ("lane verdict/text", format!("{:?} {:?}", o.verdict, o.text)),
        ("reason", format!("{:?}", s.m.reason(s.id))),
        ("close", format!("{:?}", s.m.close())),
    ];
    for (what, text) in &surfaces {
        absent(what, text, &all);
    }
    // The page (facts + accepted prose) is the only legitimate effect content.
    absent("lane effects", &format!("{:?}", o.effects), &all[..2]);
    absent("lane effects", &format!("{:?}", o.effects), &all[3..]);
    let journal = s.fx.journal.lines();
    assert!(
        !journal.is_empty() && s.fx.model_invocations() == 1,
        "journal created"
    );
    absent("Mother journal", &journal.concat(), &all);
    let broker = s.fx.home.dir.join("broker");
    assert!(
        broker.join("p3-evidence/evidence.sqlite3").is_file(),
        "evidence store"
    );
    let ledger = broker.join("files/ledger.db").is_file();
    assert_eq!(
        ledger,
        !staged(s).is_empty(),
        "the file ledger exists with a File"
    );
    let (n, mut stores) = tree(&broker);
    assert!(n > 1, "broker ledger/evidence files: {n}");
    // The staged File (facts + accepted prose) is the deliverable, not a record.
    for page in staged(s) {
        let page = std::fs::read_to_string(page).unwrap();
        assert!(page.contains(FACT) && page.contains("Welcome to our bakery."));
        absent("staged File", &page, &all[..2]);
        absent("staged File", &page, &all[3..]);
        stores = stores.replacen(&page, "", 1);
    }
    absent("broker ledger/evidence", &stores, &all);
}
fn drafts(s: &Scenario, case: &str) {
    let o = s.output();
    // Post-answer invalidation: the candidate is request data (§5.6), judged on disk.
    let returned = if case.starts_with("post/") {
        &[][..]
    } else {
        &o.effects[..]
    };
    for effect in returned {
        let EffectRequest::CreateFile(f) = effect else {
            panic!("only the File request")
        };
        absent("returned File content", &f.content, &[REJECT, LATE]);
        assert!(f.content.contains("Welcome to our bakery.") && f.content.contains(FACT));
    }
    let home = &s.fx.home.dir;
    let (n, private) = tree(&home.join("private"));
    assert!(n > 0, "the private store holds the consented contract");
    absent("private store", &private, &[REJECT, LATE]);
    let (_, everything) = tree(home);
    absent("test home", &everything, &[REJECT, LATE]);
    let page = home.join("workspace/index.html");
    if case == "plain/two" {
        assert_eq!(o.effects.len(), 1, "accepted correction");
        let pages = staged(s);
        assert_eq!(pages.len(), 1, "the one staged File");
        let written = std::fs::read_to_string(&pages[0]).unwrap();
        assert_eq!(
            o.effects,
            [EffectRequest::CreateFile(
                nexus_governed_control::files::FileCreate {
                    name: "index.html".into(),
                    content: written
                }
            )]
        );
    } else {
        assert!(staged(s).is_empty(), "no deliverable");
    }
    assert!(!page.exists(), "nothing published without the Owner");
}

#[test]
fn m3cb_canaries_absent_from_records_and_diagnostics() {
    cases(
        "m3cb_canaries_absent_from_records_and_diagnostics",
        &[
            "records:plain/one",
            "records:plain/two",
            "records:plain/bad",
            "records:plain/transport",
            "records:invalidated",
            "records:between/stop",
            "records:during/held",
            "records:during/late",
            "records:journal",
            "records:debt/owed",
        ],
    );
}
#[test]
fn m3cb_rejected_and_late_drafts_are_not_deliverables() {
    cases(
        "m3cb_rejected_and_late_drafts_are_not_deliverables",
        &[
            "drafts:plain/two",
            "drafts:plain/bad",
            "drafts:invalidated",
            "drafts:during/late",
            "drafts:post/invalidate",
        ],
    );
}
