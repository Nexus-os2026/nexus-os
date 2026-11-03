//! S1a-D+B oracles, pre-registered before the code exists: the inert page
//! check in the broker's Files path, and the consented facts it uses (S1a
//! design v2 §7 steps 1-4, §8 S-O16, S-O18 for D, S-O22 and the binding note
//! on O8; M3 design v5 §6.1-§6.2, §9.3 M3-O10 to M3-O15; OD-3).
//!
//! - O4: Files creates only `*.html` (checked) and `*.txt` (plain).
//! - The facts come only from the broker's own mission dialog: built before
//!   it (refused there), shown in a fixed block with their hash, cached with
//!   the Owner's yes (H1′), copied into the immutable Files admission, part
//!   of its identity and of the parameters digest. No admission or effect
//!   API takes facts.
//! - Prepare checks a page once with the admission's facts; execute
//!   rechecks it with them immediately before the publication.
//! - A Files create that succeeded has a receipt: the published bytes'
//!   SHA-256 and size, the verdict's policy hash, and whether a cause
//!   overtook it (the effect's own interruption codes).
//!
//! A child of the B2 oracles, for their fixture (`Fx`, `Owner`, `Cause`);
//! its own fixture adds the facts to the Owner's consent.

#[path = "files_exec_s1a_d_review1_tests.rs"]
mod review1;

use super::*;
use crate::authority::approval::{FactsView, FACTS_BLOCK_TITLE};
use crate::authority::ids::{random16, Digest};
use crate::files::{FileClass, FileReceipt, PAGE_NOT_CHECKED};
use crate::governed::HaltReason;
use crate::inert_page::{policy_hash, ApprovedFacts, LABEL_PASSED, MOTHER_CSS};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;

const PAGE: &str = "index.html";
/// S1a's reference page: canonical, inert, and showing the reference facts.
const REFERENCE: &str = include_str!("../../../../kernel/src/inert_page/fixtures/reference.html");
const ROSIE: &str = "Rosie's Bakery";
const HOURS: [&str; 3] = [
    "Monday to Friday: 07:00 – 17:00",
    "Saturday: 08:00 – 14:00",
    "Sunday: closed",
];
const PHONE: &str = "01632 960123";
/// A phone the reference page does not show.
const LONDON: &str = "020 7946 0123";
const SUMMARY: &str = "A one-page site for Rosie's bakery";
/// S1a-D's OPEN line, exactly.
const OPEN_LINE: &str =
    "not checked: how the page looks in a browser (375 px and 1366 px are OPEN)";
/// S1a's facts line for three hours lines.
const FACTS_LINE: &str = "facts present as given: name, hours (3 lines), phone";
/// The line a page checked without facts shows in their place.
const NO_FACTS_LINE: &str = "facts: not checked (none in the consent)";

fn label(n: u32) -> String {
    format!("msn-000001/1/{n}")
}

fn view(name: &str, hours: &[&str], phone: &str) -> FactsView {
    FactsView {
        name: name.into(),
        hours: hours.iter().map(|line| line.to_string()).collect(),
        phone: phone.into(),
    }
}

/// The facts the reference page shows.
fn reference() -> FactsView {
    view(ROSIE, &HOURS, PHONE)
}

/// The reference facts with a phone the page does not show.
fn london() -> FactsView {
    view(ROSIE, &HOURS, LONDON)
}

fn approved(view: &FactsView) -> ApprovedFacts {
    view.approved().expect("valid facts")
}

fn confirmation(request: &str, facts: Option<FactsView>) -> MissionConfirmation {
    MissionConfirmation {
        contract_hash: HASH.into(),
        request: request.into(),
        summary: vec![SUMMARY.into()],
        page_facts: facts,
    }
}

fn held(request: &str) -> MissionResumeConfirmation {
    MissionResumeConfirmation {
        mission: "msn-000001".into(),
        contract_hash: HASH.into(),
        request: request.into(),
        held_reason: "unclean_shutdown".into(),
        summary: vec![SUMMARY.into()],
    }
}

/// The four lines a checked page's dialog ends with.
fn page_lines(facts_line: &str) -> Vec<String> {
    vec![
        LABEL_PASSED.to_string(),
        format!(
            "policy {}  css {}",
            hex::encode(policy_hash()),
            sha256_hex(MOTHER_CSS.as_bytes())
        ),
        facts_line.to_string(),
        OPEN_LINE.to_string(),
    ]
}

fn tail(summary: &[String], n: usize) -> Vec<String> {
    summary[summary.len().saturating_sub(n)..].to_vec()
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Sixteen random lowercase letters (no digit, so never phone-like).
fn letters() -> String {
    hex::encode(random16())
        .chars()
        .map(|c| char::from(b'a' + c.to_digit(16).unwrap() as u8))
        .collect()
}

// ---------------------------------------------------------------- fixture

/// The Owner at a door's own dialogs: yes to the door and to every mission
/// and held-mission dialog, keeping each mission confirmation it was shown.
/// It is asked nothing else.
#[derive(Default)]
struct Witness {
    shown: Mutex<Vec<MissionConfirmation>>,
}

impl Witness {
    fn shown(&self) -> Vec<MissionConfirmation> {
        self.shown.lock().unwrap().clone()
    }
}

impl ControlConfirmer for Witness {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_mission(&self, request: &MissionConfirmation) -> bool {
        self.shown.lock().unwrap().push(request.clone());
        true
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        true
    }
    fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> bool {
        true
    }
}

/// The B2 fixture, with `facts` in the Owner's mission consent: the door's
/// dialogs go to the returned witness; its run is admitted to that yes.
fn fx(facts: Option<FactsView>) -> (Fx, Arc<Witness>) {
    assert!(make_non_dumpable(), "the host's startup step");
    let root = temp_root("s1a-d");
    let evidence = Arc::new(MemoryEvidence::new(100_000));
    let clock = Arc::new(ManualClock::default());
    let stop = StopAuthority::new();
    let gov = broker(&root, &evidence, &clock, &stop);
    let dirs = Dirs::new();
    let ws = dirs.0.join("home/ws");
    let info = gov
        .register_workspace_in(&ws, &dirs.exclusions())
        .expect("an empty workspace");
    let owner = Owner::new();
    let grant = gov
        .request_grant(&GrantRequest::Files { workspace: info.id }, LONG, &*owner)
        .expect("the Owner grants Files");
    let witness = Arc::new(Witness::default());
    let door = Arc::new(gov.issue_agent_door(mother(), witness.clone()).unwrap());
    door.confirm_mission(&confirmation(LABEL, facts))
        .expect("the Owner consents");
    let run = door.open_run().unwrap();
    door.admit_files(run, HASH, LABEL).unwrap();
    let f = Fx {
        gov,
        evidence,
        clock,
        stop,
        owner,
        door,
        run,
        grant,
        ws,
        _dirs: dirs,
        root,
    };
    (f, witness)
}

/// `run`'s admitted facts (`None`: admitted without facts).
fn admitted(f: &Fx, run: RunId) -> Option<ApprovedFacts> {
    f.gov
        .authority()
        .runs()
        .files_admission(run)
        .expect("admitted")
        .page_facts
}

/// The payloads of every kernel ledger record named `event`.
fn payloads(f: &Fx, event: &str) -> Vec<Value> {
    f.ledger()
        .into_iter()
        .filter(|(_, kind, _)| kind == event)
        .map(|(_, _, payload)| payload)
        .collect()
}

// ---------------------------------------------------------------- S-O16

/// S-O16 (MS-18), O4: Files creates only `*.html` (checked) and `*.txt`
/// (plain). Every other name, the upper-case and short spellings of a page
/// included, is refused `file_type_not_allowed` at prepare: no commitment,
/// no kernel run, no staging, nothing in the workspace, nothing recorded.
#[test]
fn s1a_d_s_o16_a_only_pages_and_text_are_created() {
    assert_eq!(FileClass::of(PAGE), FileClass::Page);
    assert_eq!(FileClass::of("notes.txt"), FileClass::Plain);
    let (f, _) = fx(Some(reference()));
    let records = f.evidence.records().len();
    for name in [
        "index.HTML",
        "index.Html",
        "index.htm",
        "logo.svg",
        "index.xhtml",
        "feed.xml",
        "style.css",
        "app.js",
        "README.md",
        "notes.TXT",
        "index",
    ] {
        assert_eq!(FileClass::of(name), FileClass::Refused, "{name}");
        let refused = f.propose(name, REFERENCE).expect_err("refused");
        assert_eq!(refused, AuthorityError::FileTypeNotAllowed, "{name}");
        assert_eq!(refused.class(), "file_type_not_allowed", "{name}");
    }
    assert!(f
        .gov
        .authority()
        .commitments()
        .views_of_run(f.run)
        .is_empty());
    assert!(f.events().is_empty(), "no kernel run was created");
    assert!(f.staged().is_empty());
    assert!(f.entries().is_empty());
    assert_eq!(f.evidence.records().len(), records, "nothing recorded");
}

/// S-O16: a page the inert check refuses, or one lacking an admitted fact,
/// is refused at prepare with its code, before any commitment: the check is
/// recorded once, the staging discarded, the workspace untouched.
#[test]
fn s1a_d_s_o16_b_a_refused_page_leaves_no_commitment_and_no_staging() {
    let scripted = REFERENCE.replace("</main>\n", "</main>\n<script>alert(1)</script>\n");
    for (facts, content, expected, code) in [
        (
            Some(reference()),
            scripted,
            AuthorityError::PageNotInert("script_element"),
            "script_element",
        ),
        (
            Some(london()),
            REFERENCE.to_string(),
            AuthorityError::PageFactMissing("phone_missing"),
            "phone_missing",
        ),
    ] {
        let (f, _) = fx(facts);
        let refused = f.propose(PAGE, &content).expect_err("refused at prepare");
        assert_eq!(refused, expected, "{code}");
        assert!(refused.to_string().ends_with(code), "{refused}");
        assert!(f
            .gov
            .authority()
            .commitments()
            .views_of_run(f.run)
            .is_empty());
        assert!(f.staged().is_empty(), "{code}: staging discarded");
        assert!(f.entries().is_empty(), "{code}");
        let checks = payloads(&f, "page.inert_check");
        assert_eq!(checks.len(), 1, "{code}");
        assert_eq!(checks[0]["outcome"], code);
        assert!(
            f.events().iter().any(|e| e == "staging.discarded"),
            "{code}"
        );
    }
}

/// S-O16: `.txt` is not checked (its review records no verification,
/// `NoResult`) and its dialog is M2d's; a page that passes is checked once,
/// its review is bound to the pass (`InertPage`), and its dialog ends with
/// the label, the policy and CSS hashes, the facts line and the OPEN line.
#[test]
fn s1a_d_s_o16_c_text_is_not_checked_and_a_page_shows_its_check() {
    assert_eq!(PAGE_NOT_CHECKED, OPEN_LINE);
    let (f, _) = fx(Some(reference()));
    let text = f.propose("notes.txt", "Fresh bread daily.\n").unwrap();
    assert!(payloads(&f, "page.inert_check").is_empty());
    let reviews = payloads(&f, "review.computed");
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0]["verification"], Value::Null);
    assert!(reviews[0].get("inert_page").is_none(), "{:?}", reviews[0]);
    assert!(!text
        .summary
        .iter()
        .any(|l| l == LABEL_PASSED || l == OPEN_LINE || l.starts_with("policy ")));

    let (f, _) = fx(Some(reference()));
    let page = f.propose(PAGE, REFERENCE).unwrap();
    let checks = payloads(&f, "page.inert_check");
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0]["outcome"], "passed");
    assert_eq!(checks[0]["policy"], hex::encode(policy_hash()));
    assert_eq!(
        checks[0]["facts"],
        hex::encode(approved(&reference()).hash())
    );
    let reviews = payloads(&f, "review.computed");
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0]["verification"], Value::Null);
    assert_eq!(reviews[0]["inert_page"], checks[0]["result"]);
    assert!(page
        .summary
        .contains(&format!("SHA-256: {}", sha256_hex(REFERENCE.as_bytes()))));
    assert_eq!(tail(&page.summary, 4), page_lines(FACTS_LINE));

    let (f, _) = fx(None);
    let page = f.propose(PAGE, REFERENCE).unwrap();
    assert_eq!(payloads(&f, "page.inert_check")[0]["facts"], Value::Null);
    assert_eq!(tail(&page.summary, 4), page_lines(NO_FACTS_LINE));
}

// ---------------------------------------------------------------- S-O18

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
}

/// S-O18 for D (a guard, not a proof): the broker never spells the pass
/// text, so it can only come from the kernel's `LABEL_PASSED`; the Files
/// path and its dialogs make no render or layout claim.
#[test]
fn s1a_d_s_o18_the_broker_takes_the_pass_text_from_the_kernel() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let pass = ["inert page check: ", "passed"].concat();
    let mut all = Vec::new();
    sources(&src, &mut all);
    assert!(all.len() > 50, "broker sources not found");
    for path in &all {
        let text = std::fs::read_to_string(path).unwrap();
        assert!(!text.contains(&pass), "{}", path.display());
    }
    let word = ["render", "ed"].concat();
    let claims = [
        ["verified", " layout"].concat(),
        ["layout", " verified"].concat(),
        ["page", " verified"].concat(),
        ["layout", " checked"].concat(),
    ];
    for rel in ["files/mod.rs", "authority/approval.rs"] {
        let text = std::fs::read_to_string(src.join(rel))
            .unwrap()
            .to_ascii_lowercase();
        for (at, _) in text.match_indices(&word) {
            assert!(text[..at].ends_with("not "), "{rel}: a claim at byte {at}");
        }
        for claim in &claims {
            assert!(!text.contains(claim.as_str()), "{rel}: {claim:?}");
        }
    }
    assert!(!OPEN_LINE.to_ascii_lowercase().contains(&word));
}

// ---------------------------------------------------------------- S-O22

/// S-O22 (MS-20): the execute-time recheck is load-bearing. The admission
/// holds X; a crate-private hook makes prepare pass Y, and the page shows
/// Y's facts, so prepare and the Owner's approval succeed. Execute refuses
/// with `page_recheck:facts_changed` and zero effect: no file, no temp, the
/// run's staging discarded, nothing to undo, no receipt. Control: the same
/// page under an admission of Y, with no hook, is rechecked and published.
#[test]
fn s1a_d_s_o22_the_execute_recheck_is_load_bearing() {
    let (f, _) = fx(Some(london()));
    f.gov
        .files()
        .prepare_facts_for_test(Some(approved(&reference())));
    let view = f.propose(PAGE, REFERENCE).expect("prepared with Y");
    assert_eq!(tail(&view.summary, 4), page_lines(FACTS_LINE));
    f.authorize(&view).expect("approved");
    f.execute(&view).expect_err("execute refuses");
    assert!(f.entries().is_empty(), "no file and no temp");
    assert!(f.staged().is_empty(), "the run's staging is discarded");
    let detail = f.detail(view.id).expect("the Owner's detail");
    assert!(
        detail.contains(&(
            "Outcome".to_string(),
            "page_recheck:facts_changed".to_string()
        )),
        "{detail:?}"
    );
    let rechecks = payloads(&f, "page.inert_recheck");
    assert_eq!(rechecks.len(), 1);
    assert_eq!(rechecks[0]["outcome"], "facts_changed");
    assert!(!f
        .events()
        .iter()
        .any(|e| e == "apply.prepared" || e == "apply.completed"));
    assert_eq!(f.finished(view.id).outcome, Some("failed"));
    assert_eq!(f.door.file_receipt(view.id), None);
    assert!(f.gov.files_undo(&*f.owner).is_err(), "nothing to undo");

    let (g, _) = fx(Some(reference()));
    g.create(PAGE, REFERENCE).expect("published");
    assert_eq!(
        std::fs::read(g.ws.join(PAGE)).unwrap(),
        REFERENCE.as_bytes()
    );
    let rechecks = payloads(&g, "page.inert_recheck");
    assert_eq!(rechecks.len(), 1);
    assert_eq!(rechecks[0]["outcome"], "passed");
}

// ---------------------------------------------------------------- O8

/// The binding note on O8: the admission's facts are part of its identity,
/// so the parameters digest moves with them (none, and each fact changed in
/// turn: the name, one hours line, the number of lines, the phone).
#[test]
fn s1a_d_o8_the_facts_join_the_admission_identity() {
    let (f, _) = fx(Some(reference()));
    let base = f.gov.authority().runs().files_admission(f.run).unwrap();
    assert_eq!(base.page_facts, Some(approved(&reference())));
    let mut variants = vec![base.clone()];
    let mut none = base.clone();
    none.page_facts = None;
    variants.push(none);
    let longer = "Monday to Friday: 07:00 – 18:00";
    for other in [
        view("Rosie's Cakes", &HOURS, PHONE),
        view(ROSIE, &[longer, HOURS[1], HOURS[2]], PHONE),
        view(ROSIE, &HOURS[..2], PHONE),
        london(),
    ] {
        let mut admission = base.clone();
        admission.page_facts = Some(approved(&other));
        variants.push(admission);
    }
    let digests: BTreeSet<Digest> = variants
        .iter()
        .map(|admission| {
            parameters_digest(&Components {
                admission,
                broker_run: f.run,
                kernel_run: [2; 16],
                grant: f.grant,
                binding_agent: KERNEL_AGENT,
                binding_run: *f.run.as_bytes(),
                review_hash: [6; 32],
                name: PAGE,
            })
        })
        .collect();
    assert_eq!(digests.len(), variants.len(), "each moves the digest");
}

// ---------------------------------------------------------------- M3-O10

/// The parameter list of every `fn {name}(` in `text`.
fn parameter_lists(text: &str, name: &str) -> Vec<String> {
    let needle = format!("fn {name}(");
    text.match_indices(&needle)
        .map(|(at, _)| {
            let open = at + needle.len();
            let mut depth = 1;
            let mut end = open;
            for (offset, c) in text[open..].char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                if depth == 0 {
                    end = open + offset;
                    break;
                }
            }
            text[open..end].to_string()
        })
        .collect()
}

/// Every function name declared in `text`.
fn functions(text: &str) -> Vec<String> {
    text.match_indices("fn ")
        .filter(|(at, _)| *at == 0 || !text.as_bytes()[at - 1].is_ascii_alphanumeric())
        .filter_map(|(at, _)| {
            let rest = &text[at + 3..];
            let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))?;
            (end > 0 && rest[end..].starts_with(['(', '<'])).then(|| rest[..end].to_string())
        })
        .collect()
}

/// M3-O10 (MP-M3-4): the admission's facts are the facts of the Owner's
/// dialog, and no admission or effect API takes facts from a caller. The
/// scan: no function of the front door or the pipeline has a parameter
/// naming facts; at the door only its private `cache` of the dialog's own
/// result, in the run registry only the consent cache's `fill` (fed by
/// it), and in Files only the staging step and the crate-private S-O22
/// hook. The core admission and effect routes are named and must be found.
/// `FileCreate` refuses a facts key. (The compile-time half is in the
/// crate's doc oracles.)
#[test]
fn m3b_o10_the_admission_holds_the_dialog_s_facts_and_no_api_takes_facts() {
    let (f, witness) = fx(Some(reference()));
    let shown = witness.shown();
    assert_eq!(shown.len(), 1);
    let facts = shown[0]
        .page_facts
        .as_ref()
        .expect("the dialog carried the facts")
        .approved()
        .unwrap();
    assert_eq!(admitted(&f, f.run), Some(facts.clone()));
    assert!(shown[0]
        .text()
        .header
        .contains(&("Facts hash", hex::encode(facts.hash()))));

    let (g, witness) = fx(None);
    assert_eq!(witness.shown()[0].page_facts, None);
    assert_eq!(admitted(&g, g.run), None);

    assert!(serde_json::from_str::<FileCreate>(
        r#"{"name":"index.html","content":"x","page_facts":null}"#
    )
    .is_err());

    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let scanned: [(&str, &[&str], &[&str]); 5] = [
        (
            "governed/door.rs",
            &[
                "admit_files",
                "agent_action",
                "open_run",
                "adopt",
                "file_receipt",
            ],
            &["cache"],
        ),
        (
            "governed.rs",
            &["agent_action", "propose", "authorize", "execute", "prepare"],
            &[],
        ),
        ("control.rs", &["propose", "authorize", "execute"], &[]),
        (
            "authority/run.rs",
            &["admit_files", "files_admission"],
            &["fill"],
        ),
        (
            "files/mod.rs",
            &["prepare", "revalidate", "parameters", "execute"],
            &["stage_one", "prepare_facts_for_test"],
        ),
    ];
    for (rel, named, allowed) in scanned {
        let text = std::fs::read_to_string(src.join(rel)).unwrap();
        for name in named {
            assert!(
                !parameter_lists(&text, name).is_empty(),
                "{rel}: fn {name} found"
            );
        }
        let all = functions(&text);
        assert!(all.len() > 3, "{rel}: functions found");
        for name in all {
            for params in parameter_lists(&text, &name) {
                let takes = params.to_ascii_lowercase().contains("facts");
                assert!(
                    !takes || allowed.contains(&name.as_str()),
                    "{rel}: fn {name} takes facts: ({params})"
                );
            }
        }
    }
}

// ---------------------------------------------------------------- M3-O11

/// M3-O11: facts that S1a's construction refuses open no dialog: the
/// mission is refused `facts_invalid` with S1a's code, nothing is cached
/// and nothing recorded, at a door and at the front door. Control: valid
/// facts are asked and cached.
#[test]
fn m3b_o11_invalid_facts_open_no_dialog_and_cache_nothing() {
    let (f, _) = fx(None);
    let witness = Arc::new(Witness::default());
    let door = f.gov.issue_agent_door(mother(), witness.clone()).unwrap();
    let records = f.evidence.records().len();
    let eight: Vec<String> = (1..=8).map(|d| format!("Day {d}: open")).collect();
    let eight: Vec<&str> = eight.iter().map(String::as_str).collect();
    let cases: Vec<(FactsView, &str)> = vec![
        (view(ROSIE, &["Monday 0700-1700"], PHONE), "fact_phone_like"),
        (view(ROSIE, &HOURS, "020 7946 0123 ext. 45"), "phone_format"),
        (view(ROSIE, &HOURS, "020–7946–0123"), "phone_format"),
        (view(ROSIE, &HOURS, "960123"), "phone_format"),
        (view(ROSIE, &HOURS, "0163296012345678"), "phone_format"),
        (
            view("Rosie\u{202e}s Bakery", &HOURS, PHONE),
            "facts_invalid",
        ),
        (view(ROSIE, &eight, PHONE), "facts_invalid"),
        (
            view(ROSIE, &["Order at https://example.com"], PHONE),
            "facts_invalid",
        ),
        (view("", &HOURS, PHONE), "facts_invalid"),
        (view(ROSIE, &[], PHONE), "facts_invalid"),
    ];
    for (facts, code) in cases {
        let refused = door
            .confirm_mission(&confirmation(&label(2), Some(facts)))
            .expect_err("refused before the dialog");
        assert_eq!(refused, AuthorityError::FactsInvalid(code), "{code}");
        assert_eq!(refused.class(), "facts_invalid");
    }
    assert!(witness.shown().is_empty(), "no dialog");
    assert_eq!(door.consents_cached(), 0, "nothing cached");
    assert_eq!(f.evidence.records().len(), records, "nothing recorded");

    let refused = f
        .gov
        .authority()
        .confirm_mission(
            &confirmation(&label(3), Some(view(ROSIE, &["Monday 0700-1700"], PHONE))),
            &*witness,
        )
        .expect_err("the front door refuses too");
    assert_eq!(refused, AuthorityError::FactsInvalid("fact_phone_like"));
    assert!(witness.shown().is_empty());
    assert_eq!(f.evidence.records().len(), records);

    door.confirm_mission(&confirmation(&label(2), Some(reference())))
        .expect("valid facts are asked");
    assert_eq!(witness.shown().len(), 1);
    assert_eq!(door.consents_cached(), 1);
}

// ---------------------------------------------------------------- M3-O12

/// M3-O12: the mission dialog shows the facts block exactly: its fixed
/// title, then each value escaped and marked as data, first in the details;
/// the facts hash is a header row; the consent's record keeps the hash and
/// none of the facts. A mission without facts has M2d's dialog unchanged.
#[test]
fn m3b_o12_the_dialog_shows_the_facts_block_escaped_with_its_hash() {
    assert_eq!(
        FACTS_BLOCK_TITLE,
        "Facts the page must show exactly (checked before and after writing)"
    );
    let (f, witness) = fx(Some(reference()));
    let facts = approved(&reference());
    let text = witness.shown()[0].text();
    assert_eq!(
        text.header,
        vec![
            ("Contract", HASH.to_string()),
            ("Request", LABEL.to_string()),
            ("Facts hash", hex::encode(facts.hash())),
        ]
    );
    assert_eq!(
        text.details,
        vec![
            format!("{FACTS_BLOCK_TITLE}:"),
            "│ name: Rosie's Bakery".to_string(),
            "│ hours: Monday to Friday: 07:00 – 17:00".to_string(),
            "│ hours: Saturday: 08:00 – 14:00".to_string(),
            "│ hours: Sunday: closed".to_string(),
            "│ phone: 01632 960123".to_string(),
            SUMMARY.to_string(),
        ]
    );
    let consented: Vec<EvidenceRecord> = f
        .evidence
        .records()
        .into_iter()
        .filter(|r| r.phase == EvidencePhase::MissionConsented)
        .collect();
    assert_eq!(consented.len(), 1);
    assert!(consented[0]
        .detail
        .contains(&("facts_hash".to_string(), hex::encode(facts.hash()))));

    // Escaped: a backslash is shown doubled, so a value cannot pass for an
    // escape. Private: a canary in the facts reaches no record, no error
    // and no `Debug`.
    let canary = format!("Rosie {}", letters());
    let witness = Arc::new(Witness::default());
    let door = f.gov.issue_agent_door(mother(), witness.clone()).unwrap();
    door.confirm_mission(&confirmation(
        &label(2),
        Some(view("Rosie\\s Bakery", &HOURS, PHONE)),
    ))
    .unwrap();
    assert_eq!(
        witness.shown()[0].text().details[1],
        "│ name: Rosie\\\\s Bakery"
    );
    door.confirm_mission(&confirmation(&label(3), Some(view(&canary, &HOURS, PHONE))))
        .unwrap();
    let run = door.open_run().unwrap();
    door.admit_files(run, HASH, &label(3)).unwrap();
    let mut haystack = format!(
        "{:?}\n{:?}\n{:?}\n",
        witness.shown(),
        f.gov.authority().runs().files_admission(run),
        door
    );
    for record in f.evidence.records() {
        haystack.push_str(&format!("{record:?}\n{}\n", record.to_json()));
    }
    let tail_of = canary[canary.len() - 12..].to_string();
    for form in [canary.clone(), canary.to_uppercase(), tail_of] {
        assert!(!haystack.contains(&form), "{form} leaked");
    }

    let (_g, witness) = fx(None);
    let text = witness.shown()[0].text();
    assert_eq!(
        text.header,
        vec![
            ("Contract", HASH.to_string()),
            ("Request", LABEL.to_string())
        ]
    );
    assert_eq!(text.details, vec![SUMMARY.to_string()]);
}

// ---------------------------------------------------------------- M3-O13

/// M3-O13 (MP-M3-5): the facts are part of the cached yes, so they live and
/// die with it (H1′). (a) Stop, then the Owner's resume: the old yes and
/// its facts admit nothing, and a new yes brings its own facts. (b) A halt
/// and (c) quitting admit nothing. (d) Two yeses to one hash and request
/// that disagree on the facts admit nothing; agreeing ones admit their
/// facts. (e) A held mission's resume yes shows and carries no facts.
#[test]
fn m3b_o13_stop_halt_and_closure_invalidate_the_facts_with_the_yes() {
    let (f, _) = fx(Some(london()));
    assert_eq!(admitted(&f, f.run), Some(approved(&london())));
    f.gov.emergency_stop();
    f.gov.resume(&*f.owner).unwrap();
    assert_eq!(f.door.consents_cached(), 0);
    let run = f.door.open_run().unwrap();
    assert_eq!(
        class(f.door.admit_files(run, HASH, LABEL)),
        "no_consent",
        "the old yes"
    );
    f.door
        .confirm_mission(&confirmation(LABEL, Some(reference())))
        .unwrap();
    f.door.admit_files(run, HASH, LABEL).unwrap();
    assert_eq!(admitted(&f, run), Some(approved(&reference())));

    let (g, _) = fx(Some(reference()));
    let run = g.door.open_run().unwrap();
    let _ = g.door.halt(HaltReason::JournalFailed).write();
    assert_eq!(g.door.consents_cached(), 0);
    assert_eq!(class(g.door.admit_files(run, HASH, LABEL)), "halted");
    assert!(g.gov.authority().runs().files_admission(run).is_none());

    let (h, _) = fx(Some(reference()));
    let run = h.door.open_run().unwrap();
    h.gov.shut_down();
    assert_eq!(class(h.door.admit_files(run, HASH, LABEL)), "closed");
    assert!(h.gov.authority().runs().files_admission(run).is_none());

    let (k, _) = fx(None);
    k.door
        .confirm_mission(&confirmation(&label(2), Some(reference())))
        .unwrap();
    k.door
        .confirm_mission(&confirmation(&label(2), Some(london())))
        .unwrap();
    let run = k.door.open_run().unwrap();
    assert_eq!(
        class(k.door.admit_files(run, HASH, &label(2))),
        "no_consent",
        "two yeses disagree on the facts"
    );
    assert!(k.gov.authority().runs().files_admission(run).is_none());
    for _ in 0..2 {
        k.door
            .confirm_mission(&confirmation(&label(3), Some(reference())))
            .unwrap();
    }
    k.door.admit_files(run, HASH, &label(3)).unwrap();
    assert_eq!(admitted(&k, run), Some(approved(&reference())));

    let run = k.door.open_run().unwrap();
    k.door.confirm_held_mission(&held(&label(4))).unwrap();
    k.door.admit_files(run, HASH, &label(4)).unwrap();
    assert_eq!(admitted(&k, run), None);
}

// ---------------------------------------------------------------- M3-O14

/// M3-O14 (MP-M3-11 at the broker): a Files create of a door's run that
/// succeeded has a receipt: the published bytes' SHA-256 and size, the
/// policy hash of the kernel's verdict (none for plain text), and whether a
/// cause overtook it, from the effect's own interruption codes. Not before
/// it happened, not for another door; ordering B has one, overtaken;
/// ordering A, denied, expired and lapsed creates have none.
#[test]
fn m3b_o14_the_receipt_names_the_published_bytes_and_only_a_success_has_one() {
    let (f, _) = fx(Some(reference()));
    let view = f.ready(PAGE, REFERENCE);
    assert_eq!(f.door.file_receipt(view.id), None, "not before");
    f.execute(&view).expect("published");
    let receipt = f.door.file_receipt(view.id).expect("a receipt");
    assert_eq!(
        receipt,
        FileReceipt {
            sha256: sha256(REFERENCE.as_bytes()),
            bytes: REFERENCE.len() as u64,
            policy_hash: Some(policy_hash()),
            overtaken: false,
        }
    );
    assert_eq!(
        payloads(&f, "page.inert_check")[0]["policy"],
        hex::encode(receipt.policy_hash.unwrap())
    );
    assert_eq!(
        std::fs::read(f.ws.join(PAGE)).unwrap(),
        REFERENCE.as_bytes()
    );
    let other = f
        .gov
        .issue_agent_door(mother(), Arc::new(Witness::default()))
        .unwrap();
    assert_eq!(other.file_receipt(view.id), None, "another door's");
    let text = f.ready("notes.txt", "Fresh bread daily.\n");
    f.execute(&text).expect("published");
    assert_eq!(
        f.door.file_receipt(text.id),
        Some(FileReceipt {
            sha256: sha256(b"Fresh bread daily.\n"),
            bytes: 19,
            policy_hash: None,
            overtaken: false,
        })
    );

    let (g, _) = fx(Some(reference()));
    let view = g.ready(PAGE, REFERENCE);
    Cause::EmergencyStop.arm(&g, at::FILE_AFTER_FINAL);
    g.execute(&view).expect("the one publication completes");
    let receipt = g.door.file_receipt(view.id).expect("ordering B");
    assert!(receipt.overtaken);
    assert_eq!(receipt.sha256, sha256(REFERENCE.as_bytes()));
    assert!(g
        .finished(view.id)
        .detail
        .contains(&("interrupted".to_string(), "emergency_stop".to_string())));

    let (h, _) = fx(Some(reference()));
    let view = h.ready(PAGE, REFERENCE);
    Cause::Revoke.arm(&h, at::FILE_BEFORE_FINAL);
    h.execute(&view).expect_err("ordering A");
    assert_eq!(h.door.file_receipt(view.id), None, "ordering A");

    let (d, _) = fx(Some(reference()));
    let denied = d.propose(PAGE, REFERENCE).unwrap();
    d.gov.deny(denied.id, &mother(), d.run).unwrap();
    assert_eq!(d.door.file_receipt(denied.id), None, "denied");
    let expired = d.ready("expired.txt", "e\n");
    d.clock.advance(FILE_TTL);
    d.execute(&expired).expect_err("expired");
    assert_eq!(d.door.file_receipt(expired.id), None, "expired");
    let lapsed = d.ready("lapsed.txt", "l\n");
    let _ = d.door.cancel_run(d.run).unwrap().write();
    d.execute(&lapsed).expect_err("lapsed");
    assert_eq!(d.door.file_receipt(lapsed.id), None, "lapsed");
    assert!(d.entries().is_empty());
}

// ---------------------------------------------------------------- M3-O15

/// M3-O15: S-O22 through the real consent path. The facts change only
/// through a new consent: a run admitted to X keeps X (a page showing Y is
/// refused at prepare, and a yes to Y does not re-admit it); a run admitted
/// to the yes to Y holds Y, and its page is prepared, approved, rechecked
/// and published.
#[test]
fn m3b_o15_facts_change_only_through_a_new_consent() {
    let (f, _) = fx(Some(london()));
    let missing = AuthorityError::PageFactMissing("phone_missing");
    assert_eq!(f.propose(PAGE, REFERENCE).unwrap_err(), missing);
    f.door
        .confirm_mission(&confirmation(&label(2), Some(reference())))
        .unwrap();
    assert_eq!(
        class(f.door.admit_files(f.run, HASH, &label(2))),
        "already_admitted"
    );
    assert_eq!(admitted(&f, f.run), Some(approved(&london())));
    assert_eq!(f.propose(PAGE, REFERENCE).unwrap_err(), missing);

    let run = f.door.open_run().unwrap();
    f.door.admit_files(run, HASH, &label(2)).unwrap();
    assert_eq!(admitted(&f, run), Some(approved(&reference())));
    let intent = Intent::File(FileCreate {
        name: PAGE.into(),
        content: REFERENCE.into(),
    });
    let AgentOutcome::AwaitingApproval(view) = f.door.agent_action(run, &intent).unwrap() else {
        panic!("a Files create waits for the Owner");
    };
    f.gov.authorize(view.id, &mother(), run, &*f.owner).unwrap();
    f.gov.execute(view.id, &mother(), run).expect("published");
    assert_eq!(
        std::fs::read(f.ws.join(PAGE)).unwrap(),
        REFERENCE.as_bytes()
    );
    let rechecks = payloads(&f, "page.inert_recheck");
    assert_eq!(rechecks.len(), 1);
    assert_eq!(rechecks[0]["outcome"], "passed");
}
