//! M3-E M-3b (rulings M3E-R12 (3), R14): the host only escapes and emits
//! Mother's page display text: the teaching lines `nexus_mother`'s
//! `coordinator::page_display` selects from the verified journal, or her
//! explanation. Both can hold the Owner's words, a folder name or lane text.
//! Data-only (R12 (2)): nothing in the scripted `run` flow calls it; M-5 does.

/// `text` for the terminal: every line through `evidence::escaped`, its line
/// breaks kept.
#[allow(dead_code)] // Consumed by M-5's page-mode wiring.
pub(crate) fn page_text(text: &str) -> String {
    let _ = text;
    String::new()
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
        for mark in ["\\u{1b}", "\\u{7}", "\\r", "\\u{202e}", "\\u{200b}", "\\\\done"] {
            assert_eq!(out.matches(mark).count(), 2, "{mark}");
        }
        // Frozen text passes through unchanged.
        assert_eq!(out.split('\n').nth(2), Some(T9_EXPIRED));
    }
}
