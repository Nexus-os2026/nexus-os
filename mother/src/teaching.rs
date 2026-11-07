//! M3-Dc2: what Mother says. The frozen teaching texts (design §7.1,
//! sheet §4, rulings Dc-R1 and Dc2-R1), the explanation built only from
//! journaled check results, and the claims guard (§7.2).
//!
//! T13–T15 are constants only (M3-E wires them); T16 is deferred to M3-E;
//! T6 and T8 are the broker's own dialogs. The claims guard is a pure
//! function with no runtime call site here.

use crate::contract::MissionContract;
use crate::journal::MissionId;

/// The S1a label of a passed inert check, mandatory wherever it applies.
pub const LABEL_PASSED: &str = "";
/// The S1a-D OPEN line, and the other OPEN statements of T12.
pub const OPEN_LINES: [&str; 3] = ["", "", ""];
/// T12's last line, after a written page.
pub const LOOK_YOURSELF: &str = "";

/// T2: the fixed plan.
pub const T2_PLAN: &str = "";
/// T3: the facts.
pub const T3_FACTS: &str = "";
/// T4: consent.
pub const T4_CONSENT: &str = "";
/// T5: drafting.
pub const T5_DRAFTING: &str = "";
/// T7: before the file dialog.
pub const T7_BEFORE_FILE: &str = "";
/// T9d: declined.
pub const T9_DECLINED: &str = "";
/// T9: the file dialog expired.
pub const T9_EXPIRED: &str = "";
/// T9: stopped before the final check.
pub const T9_STOPPED: &str = "";
/// T9: uncertain or lapsed (never a claim of nonpublication).
pub const T9_UNCERTAIN: &str = "";
/// T11: only after a journaled V0–V5 all-pass.
pub const T11_CHECKED: &str = "";
/// T12's success opening.
pub const T12_PASSED: &str = "";
/// T15: undo after the Owner's edit.
pub const T15_UNDO_REFUSED: &str = "";

/// T1, after the goal: `goal` verbatim.
pub fn t1(goal: &str) -> String {
    let _ = goal;
    String::new()
}

/// T10, written: broker-backed facts only.
pub fn t10(folder: &str, bytes: u64, digest: &str) -> String {
    let _ = (folder, bytes, digest);
    String::new()
}

/// T12 after a recorded all-pass: the five lines.
pub fn t12_passed() -> String {
    String::new()
}

/// T12 after a recorded failed check `code` (bare check code).
pub fn t12_failed(code: &str) -> String {
    let _ = code;
    String::new()
}

/// T12 for a written page with no check recorded.
pub fn t12_unchecked() -> String {
    String::new()
}

/// T13, the Owner's edit (constant template; M3-E wires it).
pub fn t13(facts_file: &str) -> String {
    let _ = facts_file;
    String::new()
}

/// T14 PASS: `facts_hash` is the full lowercase 64-hex.
pub fn t14_passed(facts_file: &str, facts_hash: &str) -> String {
    let _ = (facts_file, facts_hash);
    String::new()
}

/// T14 REFUSED: the location only when the checker has one.
pub fn t14_refused(code: &str, at: Option<(u64, u64)>) -> String {
    let _ = (code, at);
    String::new()
}

/// The claims guard: `Err(phrase)` for the first banned phrase found
/// (ASCII case-insensitive) once the mandatory strings are removed.
pub fn claims_guard(text: &str) -> Result<(), &'static str> {
    // M3-Dc2 inert stub: nothing is refused.
    let _ = text;
    Ok(())
}

/// Mother's explanation of a finished mission, from the journaled checks
/// (`checks`: the recorded `(id, code)` pairs, if any).
pub fn explain(
    id: MissionId,
    contract: &MissionContract,
    checks: Option<&[(String, String)]>,
    model_calls: u32,
    effects: u64,
) -> String {
    // M3-Dc2 inert stub: the unconditional explanation.
    let _ = checks;
    let mut lines = vec![
        format!("Mission {id} is done."),
        format!("You asked: {}", contract.owner_words.trim()),
        format!("I understood: {}", contract.restatement),
        "The Verifier checked every pass criterion you set before work began:".into(),
    ];
    for criterion in &contract.pass_criteria {
        lines.push(format!("  - {}: holds", criterion.text));
    }
    lines.push(format!(
        "Lane calls: {} of {}. Effects sent to the broker: {}. Money spent: 0.",
        model_calls, contract.budget.max_model_calls, effects
    ));
    lines.join("\n")
}
