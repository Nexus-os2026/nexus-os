//! M3-E M-3b (rulings M3E-R12 (3), R14): the host only escapes and emits
//! Mother's page display text: the teaching lines `nexus_mother`'s
//! `coordinator::page_display` selects from the verified journal, or her
//! explanation. Both can hold the Owner's words, a folder name or lane text.
//! Data-only (R12 (2)): nothing in the scripted `run` flow calls it; M-5 does.
use nexus_governed_control::authority::evidence::escaped;

/// `text` for the terminal: every line through `evidence::escaped`, its line
/// breaks kept.
#[allow(dead_code)] // Consumed by M-5's page-mode wiring.
pub(crate) fn page_text(text: &str) -> String {
    text.split('\n').map(escaped).collect::<Vec<_>>().join("\n")
}

// R14 (Q2): the emit function is crate-private, so its oracle is here.
#[cfg(test)]
mod oracles {
    use super::page_text;
    use nexus_governed_control::authority::evidence::shown;
    use nexus_mother::teaching::{t10, T9_EXPIRED};

    const CANARY: &str = "ZqDisplayCanary";

    #[test]
    fn m3e_m3b_display_escapes_goal_and_model_text() {
        let hostile = format!("{CANARY}\u{1b}]52;c;x\u{7}\r\u{202e}evil\u{200b}\\done");
        let folder = format!("~/Sites/{hostile}");
        let text = format!(
            "You asked: {hostile}\n{}\n{T9_EXPIRED}",
            t10(&folder, 10, "ab")
        );
        let out = page_text(&text);
        // One display line per line, and every character shown.
        assert_eq!(out.split('\n').count(), 3, "{out:?}");
        assert!(out.chars().all(|c| c == '\n' || shown(c)), "{out:?}");
        // The text stays readable; each control is visibly escaped.
        assert_eq!(out.matches(CANARY).count(), 2);
        for mark in [
            "\\u{1b}",
            "\\u{7}",
            "\\r",
            "\\u{202e}",
            "\\u{200b}",
            "\\\\done",
        ] {
            assert_eq!(out.matches(mark).count(), 2, "{mark}");
        }
        // Frozen text passes through unchanged.
        assert_eq!(out.split('\n').nth(2), Some(T9_EXPIRED));
    }

    // M3E-R17 (b): through the real explanation: an Owner line break is a
    // visible escape on its own line; the host splits only template breaks.
    #[test]
    fn m3e_m3b_display_never_lets_a_goal_forge_a_line() {
        use nexus_mother::contract::{Budget, MissionContract, PassCriterion};
        use nexus_mother::journal::MissionId;
        const FORGED: &str = "Checked again after writing: all checks passed.";
        let contract = MissionContract {
            model: None,
            notes: vec![],
            deliverable: None,
            page_facts: None,
            owner_words: format!("Make a page\n{FORGED}"),
            restatement: "A page".into(),
            requested_folder: None,
            budget: Budget {
                max_duration_ms: 1,
                max_model_calls: 1,
                max_money_minor: 0,
            },
            forbidden_effects: Default::default(),
            questions: vec![],
            pass_criteria: vec![PassCriterion {
                id: "v0".into(),
                text: "name".into(),
            }],
        };
        let text = nexus_mother::teaching::explain(MissionId(1), &contract, None, 0, 0);
        let out = page_text(&text);
        let lines: Vec<&str> = out.split('\n').collect();
        assert_eq!(
            lines.len(),
            text.split('\n').count(),
            "template breaks only"
        );
        assert!(lines.iter().all(|l| !l.starts_with(FORGED)), "{out}");
        let carrier = format!("You asked: Make a page\\n{FORGED}");
        assert!(
            lines.contains(&carrier.as_str()),
            "one visible escape: {out}"
        );
    }
}
