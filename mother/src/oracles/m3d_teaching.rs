//! M3-Dc2 oracles frozen before implementation (rulings Dc-R1, Dc2-R1):
//! Mother's exact teaching texts, the claims guard, and an explanation
//! that says `holds` only for a criterion with a recorded passing check.
//! Every golden below is written out here from the design §7.1, the sheet
//! §4 and the rulings, independently of the implementation.
use crate::contract::MissionContract;
use crate::journal::MissionId;
use crate::oracles::contract_with;
use crate::page_inputs::{page_criteria, PageFacts};
use crate::teaching::*;
use nexus_governed_control::authority::evidence::escaped;

const LABEL: &str = "inert page check: passed (not rendered)";
const OPEN_LOOK: &str =
    "not checked: how the page looks in a browser (375 px and 1366 px are OPEN)";
const OPEN_SENTENCES: &str = "OPEN: the page's other sentences have not been verified.";
const OPEN_BACKUP: &str = "OPEN: no backup yet.";
const LOOK: &str =
    "To look at it, open the file in your browser; that look is your observation, not my check.";
const PASSED: &str = "This page passed the fixed inert-format check. Its other sentences and browser appearance have not been verified.";
const T11: &str = "Checked again after writing, by a second checker that reads the page's text separately from the first: the file written is the file you approved (the broker's record and mine agree); inert page check: passed (not rendered); your name, each hours line and the phone are in the page's text; no foreign phone-like token detected; no email-shaped token detected.";
const VERIFIER_HEADER: &str =
    "The Verifier checked every pass criterion you set before work began:";
const PLAIN_HEADER: &str = "The pass criteria you set before work began:";
const NOT_RECORDED: &str = ": not independently recorded as checked";
/// v1's T12 (design v1 line 213), superseded: it must never pass.
const V1_T12: &str = "Why it looks like this: a page that cannot run code cannot harm the people who open it. Not checked (OPEN): how it looks at phone (375 px) and laptop (1366 px) widths; whether the other sentences are true; there is no backup yet. To look at it, open the file in your browser; that look is your observation, not my check.";
/// Design §7.2, step 2.
const BANNED: [&str; 16] = [
    "rendered",
    "layout",
    "looks right",
    "looks good",
    "displays correctly",
    "cannot harm",
    "harmless",
    "safe for",
    "guarantee",
    "100%",
    "no other phone",
    "verified the content",
    "I feel",
    "I'm happy",
    "I am happy",
    "I love",
];
const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn page_contract() -> MissionContract {
    let mut c = contract_with("");
    c.page_facts = Some(PageFacts {
        name: "Rosa's Bakery".into(),
        hours: vec!["Tuesday to Sunday, 7am to 3pm".into()],
        phone: "555-0142".into(),
    });
    c.pass_criteria = page_criteria();
    c
}

fn checks(codes: [&str; 6]) -> Vec<(String, String)> {
    codes
        .iter()
        .enumerate()
        .map(|(k, code)| (format!("v{k}"), code.to_string()))
        .collect()
}

const ALL_PASS: [&str; 6] = ["pass"; 6];

/// The explanation's head (unchanged lines), then `body`, then the tally.
fn expected(id: MissionId, c: &MissionContract, body: &[String], tally: &str) -> String {
    let mut lines = vec![
        format!("Mission {id} is done."),
        // M3E-R17 (b): untrusted fields are escaped before assembly.
        format!("You asked: {}", escaped(c.owner_words.trim())),
        format!("I understood: {}", escaped(&c.restatement)),
    ];
    lines.extend(body.iter().cloned());
    lines.push(tally.to_string());
    lines.join("\n")
}

fn tally(c: &MissionContract, calls: u32, effects: u64) -> String {
    format!(
        "Lane calls: {calls} of {}. Effects sent to the broker: {effects}. Money spent: 0.",
        c.budget.max_model_calls
    )
}

/// `holds[k]`: whether criterion k may say `holds`.
fn criteria(c: &MissionContract, holds: &[bool]) -> Vec<String> {
    let all = holds.iter().all(|h| *h);
    let mut body = vec![if all { VERIFIER_HEADER } else { PLAIN_HEADER }.to_string()];
    for (criterion, holds) in c.pass_criteria.iter().zip(holds) {
        let tail = if *holds { ": holds" } else { NOT_RECORDED };
        body.push(format!("  - {}{tail}", criterion.text));
    }
    body
}

/// The explanation without each complete Owner-approved criterion line
/// (rulings Dc2-R1 Q6 and Dc2-R3): only a whole line equal to a criterion
/// with its permitted status is left out; everything else, including
/// embedded or extended criterion text, is Mother's own wording.
fn mothers_own(text: &str, c: &MissionContract) -> String {
    let approved: Vec<String> = c
        .pass_criteria
        .iter()
        .flat_map(|k| {
            [
                format!("  - {}: holds", k.text),
                format!("  - {}{NOT_RECORDED}", k.text),
            ]
        })
        .collect();
    text.lines()
        .filter(|line| !approved.iter().any(|a| a == line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn open_lines_once(text: &str) {
    for open in [OPEN_LOOK, OPEN_SENTENCES, OPEN_BACKUP] {
        let n = text.lines().filter(|l| *l == open).count();
        assert_eq!(n, 1, "{open}\n{text}");
    }
}

#[test]
fn m3d_explain_uses_only_journaled_checks() {
    let id = MissionId(7);
    let page = page_contract();
    let legacy = contract_with("");
    let explain = |c: &MissionContract, k: Option<&[(String, String)]>| {
        crate::teaching::explain(id, c, k, 2, 1)
    };
    let opens = || -> Vec<String> {
        [OPEN_LOOK, OPEN_SENTENCES, OPEN_BACKUP]
            .map(String::from)
            .to_vec()
    };
    let mut outputs: Vec<(String, &MissionContract)> = vec![];

    // No report: no `holds`, the plain header, the OPEN lines only.
    let none = explain(&page, None);
    let mut body = criteria(&page, &[false; 6]);
    body.extend(opens());
    assert_eq!(none, expected(id, &page, &body, &tally(&page, 2, 1)));
    outputs.push((none, &page));

    // A matching all-pass: holds, the Verifier header, T11, T12's five lines.
    let pass = checks(ALL_PASS);
    let all = explain(&page, Some(&pass));
    let mut body = criteria(&page, &[true; 6]);
    body.push(T11.into());
    body.push(PASSED.into());
    body.extend(opens());
    body.push(LOOK.into());
    assert_eq!(all, expected(id, &page, &body, &tally(&page, 2, 1)));
    outputs.push((all, &page));

    // Failed and not-run criteria: no `holds` for them; nothing written
    // or checked is claimed; the OPEN lines stay.
    let failed = checks([
        "pass",
        "pass",
        "pass",
        "fail:phone_missing",
        "pass",
        "not_run",
    ]);
    let some = explain(&page, Some(&failed));
    let mut body = criteria(&page, &[true, true, true, false, true, false]);
    body.extend(opens());
    assert_eq!(some, expected(id, &page, &body, &tally(&page, 2, 1)));
    outputs.push((some, &page));

    // Unrelated reports make no claim: other check ids on a page mission,
    // and any report on a legacy mission.
    let other: Vec<(String, String)> = (0..6)
        .map(|k| (format!("x{k}"), "pass".to_string()))
        .collect();
    let unrelated = explain(&page, Some(&other));
    assert_eq!(unrelated, outputs[0].0, "an unrelated report is no report");
    outputs.push((unrelated, &page));
    let legacy_pass = explain(&legacy, Some(&pass));
    let legacy_none = explain(&legacy, None);
    let body = criteria(&legacy, &[false]);
    assert_eq!(
        legacy_none,
        expected(id, &legacy, &body, &tally(&legacy, 2, 1))
    );
    assert_eq!(legacy_pass, legacy_none);
    outputs.push((legacy_none, &legacy));

    for (text, c) in &outputs {
        let page_mission = c.page_facts.is_some();
        if page_mission {
            open_lines_once(text);
        } else {
            assert!(!text.contains("OPEN"), "{text}");
        }
        let holds = text.lines().filter(|l| l.ends_with(": holds")).count();
        let claimed = text.contains(VERIFIER_HEADER);
        assert_eq!(claimed, holds == c.pass_criteria.len(), "{text}");
        assert_eq!(text.contains(T11), claimed && page_mission, "{text}");
        assert_eq!(text.contains(LOOK), claimed && page_mission, "{text}");
        assert!(!text.contains("index.html was written"), "{text}");
        assert_eq!(claims_guard(&mothers_own(text, c)), Ok(()), "{text}");
    }
    #[cfg(target_os = "linux")]
    for case in ["clean", "legacy"] {
        crate::adapter::m3d_publication_oracles::run_child(CHILD, case);
    }
}

#[test]
fn m3d_teaching_goldens_and_claims_guard() {
    assert_eq!(LABEL, nexus_governed_control::inert_page::LABEL_PASSED);
    assert_eq!(LABEL_PASSED, LABEL);
    assert_eq!(OPEN_LINES, [OPEN_LOOK, OPEN_SENTENCES, OPEN_BACKUP]);
    assert_eq!(LOOK_YOURSELF, LOOK);
    assert_eq!(T12_PASSED, PASSED);
    assert_eq!(T11_CHECKED, T11);
    // Design §7.1, exact (T5 with the ASCII dash).
    assert_eq!(t1("Make me a page for my bakery"), "You asked: Make me a page for my bakery. This week I can do one kind of job: one simple web page from your notes and facts, saved as one new file in the folder you chose. I will not publish it or put it on the internet.");
    assert_eq!(T2_PLAN, "My plan, fixed for this job (I do not choose plans myself yet): 1. the local model on this computer writes the words from your notes; 2. my code puts your name, hours and phone in exactly as you typed them, in my fixed inert format: no scripts, links or pictures, nothing loaded from the internet; 3. the page is checked; 4. you see the whole file, and nothing is written without your code; 5. I create index.html only if it does not exist; 6. a second checker checks it again; 7. you make one change yourself and check it.");
    assert_eq!(T3_FACTS, "These facts must appear exactly as typed. The other words come from the model. I check that no foreign phone-like token and no email-shaped token is detected. I cannot check that every sentence is true, and a number written in words or split across lines is not detected. Please read them.");
    assert_eq!(
        T4_CONSENT,
        "To start, type the code in the box. Anything else means no, and nothing happens."
    );
    assert_eq!(T5_DRAFTING, "Drafting on this computer (usually 15-60 s). Type stop at any time: I stop and write nothing.");
    assert_eq!(T7_BEFORE_FILE, "Next you will see the whole file, every line: these exact bytes would be written. The top part is the same for every Mother page. If anything looks wrong or you are unsure, say no; nothing is written, and you can start again with changed notes.");
    assert_eq!(
        T9_DECLINED,
        "You said no. Nothing was written; the folder is still empty. Saying no is always safe."
    );
    // Ruling Dc-R1.
    assert_eq!(T9_EXPIRED, "The file dialog timed out. That counts as no: nothing was written; the folder is still empty.");
    assert_eq!(
        T9_STOPPED,
        "You stopped. Nothing was written; the folder is still empty."
    );
    assert_eq!(T9_UNCERTAIN, "I could not confirm what became of the request, so I do not say whether index.html was written. Look in the folder.");
    // Sheet §4 (ruling 8).
    assert_eq!(t10("rosas-bakery", 2048, HASH), format!("index.html was created in rosas-bakery: 2048 bytes, sha256 {HASH}. It is the only file this mission created in that folder. Undo is conditional: the last create of this session, once, while the file is unchanged."));
    let passed = [PASSED, OPEN_LOOK, OPEN_SENTENCES, OPEN_BACKUP, LOOK].join("\n");
    assert_eq!(t12_passed(), passed);
    // Rulings Dc-R1 and Dc2-R1 Q3: post-publication failure says written.
    let failed = [
        "index.html was written, but the second check did not pass: it reported phone_not_visible.",
        "You can undo it as described above.",
        OPEN_LOOK,
        OPEN_SENTENCES,
        OPEN_BACKUP,
        LOOK,
    ]
    .join("\n");
    assert_eq!(t12_failed("phone_not_visible"), failed);
    let unchecked = [
        "index.html was written, but no second check is recorded for it, so I make no claim that it passed.",
        OPEN_LOOK,
        OPEN_SENTENCES,
        OPEN_BACKUP,
        LOOK,
    ]
    .join("\n");
    assert_eq!(t12_unchecked(), unchecked);
    for text in [t12_failed("published_mismatch"), t12_unchecked()] {
        assert!(text.starts_with("index.html was written, but "), "{text}");
        assert!(!text.contains(PASSED), "{text}");
    }
    // T13 with its backticks (Dc2-R1 Q4); T14 (Q5); T15.
    assert_eq!(t13("facts.txt"), "Now change one small thing yourself, so you know how. First keep a copy: `cp index.html ~/index.html.copy`. Change words only inside one `<p>` or `<li>` line that is not the name, hours or phone. Keep it on one line; write & as `&amp;` and ' as `&#39;`; add no tags. Then run `nexus-mother check-page index.html --facts facts.txt`.");
    assert_eq!(t14_passed("facts.txt", HASH), format!("Your edit passed: inert page check: passed (not rendered); the facts in facts.txt (facts hash {HASH}) are in the page's text as given."));
    assert_eq!(t14_refused("script_element", Some((3, 7))), "The edit did not pass; the checker changed nothing. It reported script_element, at line 3, column 7. Fix that spot or put your copy back, then run it again.");
    assert_eq!(t14_refused("not_utf8", None), "The edit did not pass; the checker changed nothing. It reported not_utf8. Fix that spot or put your copy back, then run it again.");
    assert_eq!(T15_UNDO_REFUSED, "Undo now refuses: the file has changed, so it is yours now, and I will not remove your work.");

    // The guard: the mandatory strings pass, exactly.
    assert_eq!(claims_guard(LABEL), Ok(()));
    assert_eq!(claims_guard(OPEN_LOOK), Ok(()));
    for text in [
        t1("Make me a page"),
        T2_PLAN.into(),
        T3_FACTS.into(),
        T4_CONSENT.into(),
        T5_DRAFTING.into(),
        T7_BEFORE_FILE.into(),
        T9_DECLINED.into(),
        T9_EXPIRED.into(),
        T9_STOPPED.into(),
        T9_UNCERTAIN.into(),
        t10("rosas-bakery", 2048, HASH),
        T11_CHECKED.into(),
        t12_passed(),
        t12_failed("phone_not_visible"),
        t12_unchecked(),
        t13("facts.txt"),
        t14_passed("facts.txt", HASH),
        t14_refused("script_element", Some((3, 7))),
        T15_UNDO_REFUSED.into(),
    ] {
        assert!(!text.is_empty());
        assert_eq!(claims_guard(&text), Ok(()), "{text}");
    }
    // The exclusion (Dc2-R3): only a complete approved criterion line with
    // its permitted status is left out; embedded or extended criterion
    // text stays Mother's own, and is scanned.
    let page = page_contract();
    let v3 = &page.pass_criteria[3].text;
    for exact in [
        format!("  - {v3}: holds"),
        format!("  - {v3}{NOT_RECORDED}"),
    ] {
        assert_eq!(claims_guard(&mothers_own(&exact, &page)), Ok(()), "{exact}");
    }
    for crafted in [
        format!("Mother says  - {v3}: holds"),
        format!("I add:  - {v3}: not independently recorded as checked"),
        format!("  - {v3}: holds, and more"),
        format!("  - {v3}: holds."),
        format!("  - {v3}"),
        format!("  - {v3}: "),
        format!("  -  {v3}: holds"),
    ] {
        let text = [crafted.as_str(), "Lane calls: 2 of 50."].join("\n");
        assert_eq!(
            claims_guard(&mothers_own(&text, &page)),
            Err("no other phone"),
            "{crafted}"
        );
    }
    // A free "rendered", v1's T12, and every banned phrase in any case.
    assert_eq!(claims_guard("The page is rendered."), Err("rendered"));
    assert_eq!(claims_guard(V1_T12), Err("cannot harm"));
    assert_eq!(
        claims_guard(&format!("{LABEL} and RENDERED")),
        Err("rendered")
    );
    assert_eq!(
        claims_guard("inert page check: passed (not Rendered)"),
        Err("rendered"),
        "only the exact label is removed"
    );
    for phrase in BANNED {
        for form in [
            phrase.to_string(),
            phrase.to_ascii_uppercase(),
            phrase.to_ascii_lowercase(),
        ] {
            let text = format!("{LABEL}. Then: {form}, {OPEN_LOOK}");
            assert_eq!(claims_guard(&text), Err(phrase), "{text}");
        }
    }
}

#[cfg(target_os = "linux")]
const CHILD: &str = "oracles::m3d_teaching::m3d_dc2_child";

/// One fresh broker per process (as Db/Dc1): the explanation of a live,
/// clean File page mission, and of a legacy scripted mission.
#[cfg(target_os = "linux")]
#[test]
fn m3d_dc2_child() {
    use crate::adapter::m2c_support::consent;
    use crate::adapter::m3d_publication_oracles::{child_case, PageFx, BAKERY};
    use crate::coordinator::{Coordinator, Lanes};
    use crate::lane::{Lane, LaneRole, ScriptedLane, Verdict};
    use crate::oracles::lite_support::file;
    use crate::oracles::{keyed_lanes, world, Yes};
    use crate::page_verifier::PageVerifierLane;
    use crate::state::MissionState;
    use nexus_governed_control::authority::clock::SystemClock;
    use std::sync::Arc;

    let Some(case) = child_case() else {
        return;
    };
    let last = |text: &str| text.lines().last().unwrap().to_string();
    match case.as_str() {
        "clean" => {
            let f = PageFx::new();
            let mut lanes: Vec<Arc<dyn Lane>> = keyed_lanes()
                .into_iter()
                .filter(|l| !matches!(l.identity().role, LaneRole::Builder | LaneRole::Verifier))
                .collect();
            let builder = ScriptedLane::new(LaneRole::Builder, "built", Verdict::Continue);
            lanes.push(Arc::new(builder.asking(vec![file("index.html", BAKERY)])));
            lanes.push(Arc::new(PageVerifierLane));
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
            assert_eq!(m.advance(id).unwrap().to, MissionState::Verifying);
            assert_eq!(m.advance(id).unwrap().to, MissionState::Explaining);
            let done = m.advance(id).unwrap();
            assert_eq!(done.to, MissionState::Done);
            let text = done.explanation.expect("Mother explains the result");
            let c = f.contract();
            let mut body = criteria(&c, &[true; 6]);
            body.push(T11.into());
            body.push(PASSED.into());
            body.extend([OPEN_LOOK, OPEN_SENTENCES, OPEN_BACKUP, LOOK].map(String::from));
            assert_eq!(text, expected(id, &c, &body, &last(&text)));
            assert!(last(&text).starts_with("Lane calls: "), "{text}");
            assert_eq!(claims_guard(&mothers_own(&text, &c)), Ok(()), "{text}");
        }
        "legacy" => {
            let w = world();
            let c = crate::oracles::contract();
            let id = w.mother.draft(c.clone()).unwrap();
            w.mother.submit(id).unwrap();
            consent(&w.mother, id, &Yes).unwrap();
            let mut text = None;
            for _ in 0..5 {
                text = w.mother.advance(id).unwrap().explanation.or(text);
            }
            let text = text.expect("Mother explains the result");
            let body = criteria(&c, &[false]);
            assert_eq!(text, expected(id, &c, &body, &last(&text)));
            assert!(last(&text).starts_with("Lane calls: "), "{text}");
            assert!(!text.contains(": holds"), "{text}");
        }
        other => panic!("unknown case {other}"),
    }
    println!("m3d-db-receipt:{case}");
}
