//! Modal handler — Hole B Layer 2 of the Destructive Action Policy.
//! See v1.1 amendment §6.5.
//!
//! When the scout's ACT phase produces a modal dialog, the modal is
//! classified against a small set of known patterns and the handler
//! decides an action:
//!
//! - `Confirmation` with a cancel/dismiss control → `ClickCancel`
//! - `Confirmation` without a cancel control → `Hitl`
//! - `Login | Error | Info` → `Hitl` (the scout never auto-handles
//!   these — they indicate something out-of-band has happened)
//! - `Unrecognized` → `Hitl` for the first N−1 occurrences, then
//!   `Halt` on the Nth (default N = 3)
//!
//! Phase 1.3 ships this as pure logic over parsed HTML fixtures; the
//! real DOM path wires in Phase 1.3.5 alongside `nexus-computer-use`.
//! Classification is conservative: when a modal is ambiguous, classify
//! as `Unrecognized` so it routes to HITL.

use regex::Regex;
use scraper::{Html, Selector};

/// Classification of a modal dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalKind {
    Login,
    Confirmation,
    Error,
    Info,
    Unrecognized,
}

/// Action the scout should take on a classified modal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalAction {
    ClickCancel { control_id: String },
    ClickDismiss { control_id: String },
    Hitl { reason: String },
    Halt { reason: String },
}

/// Modal handler. Holds the running count of unrecognized modals in
/// the current session so repeated confusion escalates to HALT.
#[derive(Debug)]
pub struct ModalHandler {
    unrecognized_count: u32,
    halt_threshold: u32,
}

impl Default for ModalHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ModalHandler {
    /// New handler with the default halt threshold (3 unrecognized
    /// modals per session).
    pub fn new() -> Self {
        Self {
            unrecognized_count: 0,
            halt_threshold: 3,
        }
    }

    /// New handler with a custom halt threshold.
    pub fn with_halt_threshold(threshold: u32) -> Self {
        Self {
            unrecognized_count: 0,
            halt_threshold: threshold,
        }
    }

    /// Parse modal HTML and classify it.
    ///
    /// Heuristics (applied in priority order):
    ///
    /// - Contains `<input type="password">` → `Login`.
    /// - Contains any button whose label matches `confirm|are you sure|
    ///   delete\?|permanent` (case-insensitive) → `Confirmation`.
    /// - Contains text matching `error|failed|exception` → `Error`.
    /// - Contains text matching `info|notice|fyi` → `Info`.
    /// - Anything else that has a `role="dialog"` or `aria-modal`
    ///   wrapper → `Unrecognized` (routes to HITL).
    /// - Anything else → `Unrecognized`.
    ///
    /// When in doubt, this method **returns `Unrecognized`** rather
    /// than guessing, so ambiguous modals route to HITL instead of
    /// being auto-handled.
    pub fn classify_modal(&self, modal_html: &str) -> ModalKind {
        let doc = Html::parse_fragment(modal_html);

        // 1. Login — has a password input.
        if let Ok(sel) = Selector::parse("input[type=\"password\"]") {
            if doc.select(&sel).next().is_some() {
                return ModalKind::Login;
            }
        }

        let text = text_content(&doc);
        let text_lower = text.to_lowercase();

        // 2. Confirmation — look at button labels specifically so the
        // ambient body text (e.g., "Xyzzy plugh") does not accidentally
        // trip the confirmation regex. Also accept body text like "Are
        // you sure you want to delete team X?".
        let confirmation_pattern =
            Regex::new(r"(?i)(confirm|are you sure|delete\?|permanent)").unwrap();
        if confirmation_pattern.is_match(&text) {
            // But only if there's at least one button — a confirmation
            // modal without buttons is unrecognized.
            if let Ok(btn_sel) = Selector::parse("button") {
                if doc.select(&btn_sel).next().is_some() {
                    return ModalKind::Confirmation;
                }
            }
        }

        // 3. Error.
        if text_lower.contains("error")
            || text_lower.contains("failed")
            || text_lower.contains("exception")
        {
            return ModalKind::Error;
        }

        // 4. Info.
        if text_lower.contains("notice") || text_lower.contains("fyi") {
            // "info" alone is too common; require stronger markers.
            return ModalKind::Info;
        }

        ModalKind::Unrecognized
    }

    /// Decide what action to take for a classified modal.
    ///
    /// Also updates the handler's unrecognized counter when the kind
    /// is `Unrecognized`, so repeated ambiguity escalates to HALT.
    pub fn decide_action(&mut self, kind: ModalKind, modal_html: &str) -> ModalAction {
        match kind {
            ModalKind::Confirmation => match find_cancel_control(modal_html) {
                Some(control_id) => ModalAction::ClickCancel { control_id },
                None => ModalAction::Hitl {
                    reason: "Confirmation modal has no cancel control".to_string(),
                },
            },
            ModalKind::Unrecognized => {
                self.unrecognized_count += 1;
                if self.unrecognized_count >= self.halt_threshold {
                    ModalAction::Halt {
                        reason: format!(
                            "{} unrecognized modals in this session, exceeded threshold {}",
                            self.unrecognized_count, self.halt_threshold
                        ),
                    }
                } else {
                    ModalAction::Hitl {
                        reason: format!(
                            "Unrecognized modal #{} (threshold {})",
                            self.unrecognized_count, self.halt_threshold
                        ),
                    }
                }
            }
            ModalKind::Login | ModalKind::Error | ModalKind::Info => ModalAction::Hitl {
                reason: format!("{:?} modal: scout never auto-handles these", kind),
            },
        }
    }

    /// Current count of unrecognized modals seen in this session.
    pub fn unrecognized_count(&self) -> u32 {
        self.unrecognized_count
    }
}

/// Find a cancel/dismiss control inside modal HTML.
///
/// Returns the button's `id` attribute, or `None` if no matching
/// button exists. The label match is against `cancel|dismiss|close|no
/// |back` as a case-insensitive whole-token match.
fn find_cancel_control(modal_html: &str) -> Option<String> {
    let doc = Html::parse_fragment(modal_html);
    let btn_sel = Selector::parse("button, [role=\"button\"]").ok()?;
    let label_pattern = Regex::new(r"(?i)^\s*(cancel|dismiss|close|no|back)\s*$").unwrap();

    for el in doc.select(&btn_sel) {
        let aria = el.value().attr("aria-label").unwrap_or("").trim();
        let text: String = el.text().collect::<String>().trim().to_string();

        let matches_aria = !aria.is_empty() && label_pattern.is_match(aria);
        let matches_text = !text.is_empty() && label_pattern.is_match(&text);

        if matches_aria || matches_text {
            let id = el.value().attr("id").unwrap_or("").to_string();
            return Some(if id.is_empty() {
                format!("auto_cancel_{}", text.replace(' ', "_"))
            } else {
                id
            });
        }
    }
    None
}

/// Concatenate the visible text content of a parsed fragment,
/// preserving spaces between nodes.
fn text_content(doc: &Html) -> String {
    doc.root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
