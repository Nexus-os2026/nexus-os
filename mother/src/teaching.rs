//! M3-Dc2: what Mother says. The frozen teaching texts (design §7.1,
//! sheet §4, rulings Dc-R1 and Dc2-R1), the explanation built only from
//! journaled check results, and the claims guard (§7.2).
//!
//! T13–T15 are constants only (M3-E wires them); T16 is the summary (M-6);
//! T6 and T8 are the broker's own dialogs. The claims guard is a pure
//! function with no runtime call site here.

use crate::contract::MissionContract;
use crate::journal::MissionId;
use nexus_governed_control::authority::evidence::escaped;

/// The S1a label of a passed inert check, mandatory wherever it applies.
pub const LABEL_PASSED: &str = nexus_governed_control::inert_page::LABEL_PASSED;
/// The S1a-D OPEN line, and the other OPEN statements of T12.
pub const OPEN_LINES: &[&str] = &[
    "not checked: how the page looks in a browser (375 px and 1366 px are OPEN)",
    "OPEN: the page's other sentences have not been verified.",
    "OPEN: no backup yet.",
];
/// T12's last line, after a written page.
pub const LOOK_YOURSELF: &str =
    "To look at it, open the file in your browser; that look is your observation, not my check.";

/// T2: the fixed plan.
pub const T2_PLAN: &str = "My plan, fixed for this job (I do not choose plans myself yet): 1. the local model on this computer writes the words from your notes; 2. my code puts your name, hours and phone in exactly as you typed them, in my fixed inert format: no scripts, links or pictures, nothing loaded from the internet; 3. the page is checked; 4. you see the whole file, and nothing is written without your code; 5. I create index.html only if it does not exist; 6. a second checker checks it again; 7. you make one change yourself and check it.";
/// T3: the facts.
pub const T3_FACTS: &str = "These facts must appear exactly as typed. The other words come from the model. I check that no foreign phone-like token and no email-shaped token is detected. I cannot check that every sentence is true, and a number written in words or split across lines is not detected. Please read them.";
/// T4: consent.
pub const T4_CONSENT: &str =
    "To start, type the code in the box. Anything else means no, and nothing happens.";
/// T5: drafting.
pub const T5_DRAFTING: &str =
    "Drafting on this computer (usually 15-60 s). Type stop at any time: I stop and write nothing.";
/// T7: before the file dialog.
pub const T7_BEFORE_FILE: &str = "Next you will see the whole file, every line: these exact bytes would be written. The top part is the same for every Mother page. If anything looks wrong or you are unsure, say no; nothing is written, and you can start again with changed notes.";
/// T9d: declined.
pub const T9_DECLINED: &str =
    "You said no. Nothing was written; the folder is still empty. Saying no is always safe.";
/// T9: the file dialog expired.
pub const T9_EXPIRED: &str =
    "The file dialog timed out. That counts as no: nothing was written; the folder is still empty.";
/// T9: stopped before the final check.
pub const T9_STOPPED: &str = "You stopped. Nothing was written; the folder is still empty.";
/// T9: uncertain or lapsed (never a claim of nonpublication).
pub const T9_UNCERTAIN: &str = "I could not confirm what became of the request, so I do not say whether index.html was written. Look in the folder.";
/// T11: only after a journaled V0–V5 all-pass.
pub const T11_CHECKED: &str = "Checked again after writing, by a second checker that reads the page's text separately from the first: the file written is the file you approved (the broker's record and mine agree); inert page check: passed (not rendered); your name, each hours line and the phone are in the page's text; no foreign phone-like token detected; no email-shaped token detected.";
/// T12's success opening.
pub const T12_PASSED: &str = "This page passed the fixed inert-format check. Its other sentences and browser appearance have not been verified.";
/// T15: undo after the Owner's edit.
pub const T15_UNDO_REFUSED: &str =
    "Undo now refuses: the file has changed, so it is yours now, and I will not remove your work.";

/// T16's closed «backend outcome» phrases (M3E-R18 (3)).
pub const T16_PASSED: &str = "index.html was written and the second check passed";
pub const T16_UNCHECKED: &str = "index.html was written; no second check is recorded";
pub const T16_UNCONFIRMED: &str = "I could not confirm what became of the request";
pub const T16_DECLINED: &str = "you said no; nothing was written";
pub const T16_EXPIRED: &str = "the file dialog timed out; nothing was written";
pub const T16_STOPPED: &str = "you stopped; nothing was written";
/// T16's «recorded checks» when none is recorded.
pub const T16_NONE_RECORDED: &str = "none recorded";

/// T16's outcome after a recorded failed check `code` (escaped by the caller).
pub fn t16_failed(code: &str) -> String {
    let _ = code;
    String::new()
}

/// T16, the summary: `outcome` is one of the closed phrases above, `checked`
/// the recorded checks or [`T16_NONE_RECORDED`]. It labels nothing OBSERVED
/// or CORROBORATED; it says when each would apply.
pub fn t16(outcome: &str, checked: &str) -> String {
    let _ = (outcome, checked);
    String::new()
}

/// The claims guard's refusals (design §7.2), ASCII case-insensitive.
pub const BANNED: [&str; 16] = [
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

const HEADER_CHECKED: &str = "The Verifier checked every pass criterion you set before work began:";
const HEADER_PLAIN: &str = "The pass criteria you set before work began:";
const NOT_RECORDED: &str = "not independently recorded as checked";

/// T1, after the goal: `goal` verbatim.
pub fn t1(goal: &str) -> String {
    format!("You asked: {goal}. This week I can do one kind of job: one simple web page from your notes and facts, saved as one new file in the folder you chose. I will not publish it or put it on the internet.")
}

/// T10, written: broker-backed facts only.
pub fn t10(folder: &str, bytes: u64, digest: &str) -> String {
    format!("index.html was created in {folder}: {bytes} bytes, sha256 {digest}. It is the only file this mission created in that folder. Undo is conditional: the last create of this session, once, while the file is unchanged.")
}

/// `first`, then the OPEN lines, then T12's look line.
fn t12(first: &[&str]) -> String {
    let mut lines = first.to_vec();
    lines.extend(OPEN_LINES.iter());
    lines.push(LOOK_YOURSELF);
    lines.join("\n")
}

/// T12 after a recorded all-pass: the five lines.
pub fn t12_passed() -> String {
    t12(&[T12_PASSED])
}

/// T12 after a recorded failed check `code` (bare check code).
pub fn t12_failed(code: &str) -> String {
    let status =
        format!("index.html was written, but the second check did not pass: it reported {code}.");
    t12(&[&status, "You can undo it as described above."])
}

/// T12 for a written page with no check recorded.
pub fn t12_unchecked() -> String {
    t12(&["index.html was written, but no second check is recorded for it, so I make no claim that it passed."])
}

/// T13, the Owner's edit (constant template; M3-E wires it).
pub fn t13(facts_file: &str) -> String {
    format!("Now change one small thing yourself, so you know how. First keep a copy: `cp index.html ~/index.html.copy`. Change words only inside one `<p>` or `<li>` line that is not the name, hours or phone. Keep it on one line; write & as `&amp;` and ' as `&#39;`; add no tags. Then run `nexus-mother check-page index.html --facts {facts_file}`.")
}

/// T14 PASS: `facts_hash` is the full lowercase 64-hex.
pub fn t14_passed(facts_file: &str, facts_hash: &str) -> String {
    format!("Your edit passed: {LABEL_PASSED}; the facts in {facts_file} (facts hash {facts_hash}) are in the page's text as given.")
}

/// T14 REFUSED: the location only when the checker has one.
pub fn t14_refused(code: &str, at: Option<(u64, u64)>) -> String {
    let at = at.map_or(String::new(), |(line, column)| {
        format!(", at line {line}, column {column}")
    });
    format!("The edit did not pass; the checker changed nothing. It reported {code}{at}. Fix that spot or put your copy back, then run it again.")
}

/// The claims guard: `Err(phrase)` for the first banned phrase found
/// (ASCII case-insensitive) once the mandatory strings are removed.
pub fn claims_guard(text: &str) -> Result<(), &'static str> {
    let text = text.replace(LABEL_PASSED, "").replace(OPEN_LINES[0], "");
    let text = text.to_ascii_lowercase();
    match BANNED
        .iter()
        .find(|phrase| text.contains(&phrase.to_ascii_lowercase()))
    {
        Some(phrase) => Err(phrase),
        None => Ok(()),
    }
}

/// Mother's explanation of a finished mission, from the journaled checks
/// (`checks`: the recorded `(id, code)` pairs, if any). A criterion holds
/// only on a page mission whose check of the same id is recorded `pass`.
pub fn explain(
    id: MissionId,
    contract: &MissionContract,
    checks: Option<&[(String, String)]>,
    model_calls: u32,
    effects: u64,
) -> String {
    let page = contract.page_facts.is_some();
    let holds = |criterion: &str| {
        page && checks.is_some_and(|checks| {
            checks
                .iter()
                .any(|(of, code)| of.eq_ignore_ascii_case(criterion) && code == "pass")
        })
    };
    let all =
        !contract.pass_criteria.is_empty() && contract.pass_criteria.iter().all(|c| holds(&c.id));
    let mut lines = vec![
        format!("Mission {id} is done."),
        // M3E-R17: untrusted fields are escaped before assembly, so a line
        // break in them is a visible escape and never starts a line.
        format!("You asked: {}", escaped(contract.owner_words.trim())),
        format!("I understood: {}", escaped(&contract.restatement)),
        if all { HEADER_CHECKED } else { HEADER_PLAIN }.into(),
    ];
    for criterion in &contract.pass_criteria {
        let state = if holds(&criterion.id) {
            "holds"
        } else {
            NOT_RECORDED
        };
        lines.push(format!("  - {}: {state}", escaped(&criterion.text)));
    }
    if page && all {
        lines.push(T11_CHECKED.into());
        lines.push(t12_passed());
    } else if page {
        lines.extend(OPEN_LINES.iter().map(|line| line.to_string()));
    }
    lines.push(format!(
        "Lane calls: {} of {}. Effects sent to the broker: {}. Money spent: 0.",
        model_calls, contract.budget.max_model_calls, effects
    ));
    lines.join("\n")
}
