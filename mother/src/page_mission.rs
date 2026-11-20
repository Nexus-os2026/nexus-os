//! M3-E M-5a (SB3E-R152; U7 `m3e-mission-draft.md:339`, D9 `:477`): the
//! fixed one-page-site contract, as data, and its pre-consent preview.
//!
//! [`page_contract`] assembles a value from data the host already holds (the
//! parsed snapshot, the goal, the backend's verified model view and the
//! workspace name). It opens, registers and authorizes nothing; the contract
//! still passes `validate_for_consent` and the broker's own checks later.
//! [`preview`] is the text shown before any consent, built only from that
//! contract; every untrusted field is escaped before assembly (M3E-R17).

use crate::contract::{Budget, EffectKind, MissionContract, ModelChoice, Note};
use crate::page_inputs::{page_criteria, PageFacts};
use crate::teaching::{T2_PLAN, T3_FACTS};
use nexus_governed_control::authority::evidence::escaped;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// The one deliverable.
pub const PAGE_FILE: &str = "index.html";
/// 30 minutes, 9 lane invocations, zero money (U7 frozen).
pub const PAGE_BUDGET: Budget = Budget {
    max_duration_ms: 1_800_000,
    max_model_calls: 9,
    max_money_minor: 0,
};
const RESTATEMENT: &str =
    "Make one simple web page, index.html, from your notes and facts, in the folder you chose.";

/// The fixed page contract for `goal`, `note`, `facts`, the workspace
/// `folder` and the verified `model`.
pub fn page_contract(
    goal: &str,
    note: Note,
    facts: PageFacts,
    folder: &str,
    model: ModelChoice,
) -> MissionContract {
    MissionContract {
        owner_words: goal.into(),
        restatement: RESTATEMENT.into(),
        requested_folder: Some(folder.into()),
        budget: PAGE_BUDGET,
        forbidden_effects: BTreeSet::from([
            EffectKind::ModelTool,
            EffectKind::Network,
            EffectKind::Process,
            EffectKind::Spend,
        ]),
        questions: vec![],
        pass_criteria: page_criteria(),
        model: Some(model),
        notes: vec![note],
        deliverable: Some(PAGE_FILE.into()),
        page_facts: Some(facts),
    }
}

fn digest(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

/// What the Owner sees before any consent: every field from `c` alone.
pub fn preview(c: &MissionContract) -> String {
    let mut lines = vec![
        "PAGE PREVIEW".to_string(),
        format!("goal: {}", escaped(&c.owner_words)),
        format!(
            "goal: {} bytes, sha256 {}",
            c.owner_words.len(),
            digest(&c.owner_words)
        ),
    ];
    for note in &c.notes {
        lines.push(format!(
            "note {}: {} bytes, sha256 {}",
            escaped(&note.label),
            note.text.len(),
            digest(&note.text)
        ));
    }
    if let Some(f) = &c.page_facts {
        lines.push(format!("facts: name: {}", escaped(&f.name)));
        for hours in &f.hours {
            lines.push(format!("facts: hours: {}", escaped(hours)));
        }
        lines.push(format!("facts: phone: {}", escaped(&f.phone)));
        lines.push(T3_FACTS.into());
    }
    lines.push(format!(
        "folder: {}",
        escaped(c.requested_folder.as_deref().unwrap_or("none"))
    ));
    if let Some(m) = &c.model {
        lines.push(format!(
            "model: name={} digest={} identity={}",
            escaped(&m.name),
            m.digest,
            m.identity
        ));
    }
    lines.push(format!(
        "file: {}",
        escaped(c.deliverable.as_deref().unwrap_or("none"))
    ));
    lines.push(T2_PLAN.into());
    for criterion in &c.pass_criteria {
        lines.push(format!(
            "check {}: {}",
            escaped(&criterion.id),
            escaped(&criterion.text)
        ));
    }
    let forbidden: Vec<&str> = c.forbidden_effects.iter().map(|e| e.as_str()).collect();
    lines.push(format!("forbidden: {}", forbidden.join(", ")));
    lines.push(format!(
        "budget: {} ms, {} lane invocations, money {}",
        c.budget.max_duration_ms, c.budget.max_model_calls, c.budget.max_money_minor
    ));
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> MissionContract {
        page_contract(
            "Make a page for my bakery",
            Note {
                label: "notes".into(),
                text: "Fresh bread every morning.".into(),
            },
            PageFacts {
                name: "Rosa's Bakery".into(),
                hours: vec!["Tuesday to Sunday, 7am to 3pm".into()],
                phone: "555-0142".into(),
            },
            "rosa-site",
            ModelChoice {
                name: "qwen3.6:35b-a3b".into(),
                digest: "a".repeat(64),
                identity: "b".repeat(64),
            },
        )
    }

    #[test]
    fn m5a_page_contract_is_the_fixed_contract() {
        let c = sample();
        assert_eq!(c.validate_for_consent(), Ok(()));
        assert_eq!(
            (
                c.budget.max_duration_ms,
                c.budget.max_model_calls,
                c.budget.max_money_minor
            ),
            (1_800_000, 9, 0)
        );
        assert_eq!(c.deliverable.as_deref(), Some("index.html"));
        assert_eq!(c.pass_criteria, page_criteria());
        for kind in [
            EffectKind::ModelTool,
            EffectKind::Network,
            EffectKind::Process,
            EffectKind::Spend,
        ] {
            assert!(c.forbidden_effects.contains(&kind));
        }
        assert!(!c.forbidden_effects.contains(&EffectKind::File));
    }
}
