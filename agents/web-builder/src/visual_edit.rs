//! Visual Edit — token-aware edit operations for the visual editor surface.
//!
//! All visual edits are token operations:
//! - Layer 1 (Foundation): theme-wide changes at `:root`
//! - Layer 3 (Instance): section-scoped overrides via `[data-nexus-section]`
//! - Layer 2 is NEVER edited directly — it flows from Layer 1
//!
//! These functions are called by Tauri commands to persist edits to the
//! project's TokenSet and ContentPayload.

use crate::content_payload::{ContentPayload, SectionContent};
use crate::slot_schema::{html_escape, validate_slot_value, TemplateSchema};
use crate::tokens::TokenSet;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum VisualEditError {
    #[error("unknown token: {0}")]
    UnknownToken(String),
    #[error("text validation failed for slot '{slot}': {reason}")]
    TextValidation { slot: String, reason: String },
    #[error("section not found: {0}")]
    SectionNotFound(String),
    #[error("slot not found: section='{section}', slot='{slot}'")]
    SlotNotFound { section: String, slot: String },
}

// ─── Visual Edit State ──────────────────────────────────────────────────────

/// Persisted visual edit overrides for a project.
/// Stored alongside ProjectState in `visual_edits.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VisualEditState {
    /// Layer 1 foundation token overrides (user's theme changes).
    pub foundation_overrides: HashMap<String, String>,
    /// Layer 3 instance overrides (section-scoped changes).
    pub instance_overrides: Vec<InstanceEdit>,
    /// Content payload text edits (slot text changes).
    pub text_edits: Vec<TextEdit>,
}

/// A Layer 3 instance override.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceEdit {
    pub section_id: String,
    pub token_name: String,
    pub value: String,
}

/// A text content edit to a slot in the ContentPayload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    pub section_id: String,
    pub slot_name: String,
    pub new_text: String,
}

// ─── Token Edit Operations ──────────────────────────────────────────────────

/// Apply a Layer 1 (foundation) token edit to a TokenSet.
///
/// Returns the updated CSS string.
pub fn apply_foundation_edit(
    token_set: &mut TokenSet,
    token_name: &str,
    value: &str,
) -> Result<String, VisualEditError> {
    token_set
        .set_foundation(token_name, value)
        .map_err(|_| VisualEditError::UnknownToken(token_name.to_string()))?;
    Ok(token_set.to_css())
}

/// Apply a Layer 3 (instance) token edit to a TokenSet.
///
/// Returns the updated CSS string.
pub fn apply_instance_edit(
    token_set: &mut TokenSet,
    section_id: &str,
    token_name: &str,
    value: &str,
) -> String {
    token_set.set_override(section_id, token_name, value);
    token_set.to_css()
}

/// Apply a token edit (Layer 1 or 3) and record it in the VisualEditState.
pub fn apply_token_edit(
    token_set: &mut TokenSet,
    edit_state: &mut VisualEditState,
    layer: u8,
    section_id: Option<&str>,
    token_name: &str,
    value: &str,
) -> Result<String, VisualEditError> {
    match layer {
        1 => {
            let css = apply_foundation_edit(token_set, token_name, value)?;
            edit_state
                .foundation_overrides
                .insert(token_name.to_string(), value.to_string());
            Ok(css)
        }
        3 => {
            let sid = section_id.unwrap_or("unknown");
            let css = apply_instance_edit(token_set, sid, token_name, value);
            // Update or add instance override
            if let Some(existing) = edit_state
                .instance_overrides
                .iter_mut()
                .find(|o| o.section_id == sid && o.token_name == token_name)
            {
                existing.value = value.to_string();
            } else {
                edit_state.instance_overrides.push(InstanceEdit {
                    section_id: sid.to_string(),
                    token_name: token_name.to_string(),
                    value: value.to_string(),
                });
            }
            Ok(css)
        }
        _ => Err(VisualEditError::UnknownToken(format!(
            "invalid layer: {layer}"
        ))),
    }
}

// ─── Text Edit Operations ───────────────────────────────────────────────────

/// Apply a text edit to a ContentPayload, validating against the schema.
///
/// Returns the HTML-escaped new text value.
pub fn apply_text_edit(
    payload: &mut ContentPayload,
    edit_state: &mut VisualEditState,
    schema: &TemplateSchema,
    section_id: &str,
    slot_name: &str,
    new_text: &str,
) -> Result<String, VisualEditError> {
    // Find the section schema to validate against
    let section_schema = schema
        .sections
        .iter()
        .find(|s| s.section_id == section_id)
        .ok_or_else(|| VisualEditError::SectionNotFound(section_id.to_string()))?;

    let constraint =
        section_schema
            .slots
            .get(slot_name)
            .ok_or_else(|| VisualEditError::SlotNotFound {
                section: section_id.to_string(),
                slot: slot_name.to_string(),
            })?;

    // Validate the new text against slot constraints
    validate_slot_value(slot_name, new_text, constraint).map_err(|e| {
        VisualEditError::TextValidation {
            slot: slot_name.to_string(),
            reason: e.to_string(),
        }
    })?;

    // Update the payload
    let section = payload
        .sections
        .iter_mut()
        .find(|s| s.section_id == section_id);

    match section {
        Some(s) => {
            s.slots.insert(slot_name.to_string(), new_text.to_string());
        }
        None => {
            // Section doesn't exist in payload yet — create it
            payload.sections.push(SectionContent {
                section_id: section_id.to_string(),
                slots: HashMap::from([(slot_name.to_string(), new_text.to_string())]),
            });
        }
    }

    // Record the text edit
    if let Some(existing) = edit_state
        .text_edits
        .iter_mut()
        .find(|e| e.section_id == section_id && e.slot_name == slot_name)
    {
        existing.new_text = new_text.to_string();
    } else {
        edit_state.text_edits.push(TextEdit {
            section_id: section_id.to_string(),
            slot_name: slot_name.to_string(),
            new_text: new_text.to_string(),
        });
    }

    // Return HTML-escaped text for safe injection
    Ok(html_escape(new_text))
}

// ─── Persistence ────────────────────────────────────────────────────────────

/// Save visual edit state to `{project_dir}/visual_edits.json`.
pub fn save_visual_edit_state(
    project_dir: &std::path::Path,
    state: &VisualEditState,
) -> Result<(), String> {
    let path = project_dir.join("visual_edits.json");
    let json = serde_json::to_string_pretty(state).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write visual_edits.json: {e}"))
}

/// Load visual edit state from `{project_dir}/visual_edits.json`.
pub fn load_visual_edit_state(project_dir: &std::path::Path) -> Result<VisualEditState, String> {
    let path = project_dir.join("visual_edits.json");
    if !path.exists() {
        return Ok(VisualEditState::default());
    }
    let json =
        std::fs::read_to_string(&path).map_err(|e| format!("read visual_edits.json: {e}"))?;
    serde_json::from_str(&json).map_err(|e| format!("parse visual_edits.json: {e}"))
}

/// Apply all saved visual edits to a TokenSet (restoring state after reload).
pub fn restore_visual_edits(token_set: &mut TokenSet, edit_state: &VisualEditState) {
    // Restore Layer 1 foundation overrides
    for (name, value) in &edit_state.foundation_overrides {
        let _ = token_set.set_foundation(name, value);
    }
    // Restore Layer 3 instance overrides
    for ovr in &edit_state.instance_overrides {
        token_set.set_override(&ovr.section_id, &ovr.token_name, &ovr.value);
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_payload::SectionContent;
    use crate::slot_schema::get_template_schema;
    use crate::variant::{MotionProfile, VariantSelection};

    fn default_variant() -> VariantSelection {
        VariantSelection {
            palette_id: "saas_midnight".into(),
            typography_id: "modern".into(),
            layout: HashMap::new(),
            motion: MotionProfile::Subtle,
        }
    }

    fn default_token_set() -> TokenSet {
        TokenSet::default()
    }

    fn test_payload() -> ContentPayload {
        ContentPayload {
            template_id: "saas_landing".into(),
            variant: default_variant(),
            sections: vec![SectionContent {
                section_id: "hero".into(),
                slots: HashMap::from([
                    ("headline".into(), "Build Faster with AI".into()),
                    (
                        "subtitle".into(),
                        "The platform for modern developers.".into(),
                    ),
                    ("cta_primary".into(), "Start Free Trial".into()),
                ]),
            }],
        }
    }

    #[test]
    fn test_visual_edit_token_layer1() {
        let mut ts = default_token_set();
        let mut es = VisualEditState::default();
        let result = apply_token_edit(&mut ts, &mut es, 1, None, "color-primary", "#3b82f6");
        assert!(result.is_ok());
        let css = result.unwrap();
        assert!(
            css.contains("#3b82f6"),
            "CSS should contain new primary color"
        );
        assert_eq!(
            es.foundation_overrides.get("color-primary"),
            Some(&"#3b82f6".to_string())
        );
    }

    #[test]
    fn test_visual_edit_token_layer3() {
        let mut ts = default_token_set();
        let mut es = VisualEditState::default();
        let result = apply_token_edit(&mut ts, &mut es, 3, Some("hero"), "section-bg", "#0f172a");
        assert!(result.is_ok());
        let css = result.unwrap();
        assert!(
            css.contains("[data-nexus-section=\"hero\"]"),
            "CSS should contain section override"
        );
        assert!(css.contains("#0f172a"));
        assert_eq!(es.instance_overrides.len(), 1);
        assert_eq!(es.instance_overrides[0].section_id, "hero");
    }

    #[test]
    fn test_visual_edit_token_invalid_name() {
        let mut ts = default_token_set();
        let mut es = VisualEditState::default();
        let result = apply_token_edit(&mut ts, &mut es, 1, None, "totally-fake-token", "#fff");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            VisualEditError::UnknownToken(_)
        ));
    }

    #[test]
    fn test_visual_edit_text_valid() {
        let schema = get_template_schema("saas_landing").unwrap();
        let mut payload = test_payload();
        let mut es = VisualEditState::default();
        let result = apply_text_edit(
            &mut payload,
            &mut es,
            &schema,
            "hero",
            "headline",
            "Ship Code 10x Faster",
        );
        assert!(result.is_ok());
        // Check payload was updated
        let hero = payload
            .sections
            .iter()
            .find(|s| s.section_id == "hero")
            .unwrap();
        assert_eq!(hero.slots.get("headline").unwrap(), "Ship Code 10x Faster");
        // Check edit was recorded
        assert_eq!(es.text_edits.len(), 1);
    }

    #[test]
    fn test_visual_edit_text_exceeds_limit() {
        let schema = get_template_schema("saas_landing").unwrap();
        let mut payload = test_payload();
        let mut es = VisualEditState::default();
        // hero headline max is 80 chars
        let long_text = "x".repeat(100);
        let result = apply_text_edit(
            &mut payload,
            &mut es,
            &schema,
            "hero",
            "headline",
            &long_text,
        );
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            VisualEditError::TextValidation { .. }
        ));
    }

    #[test]
    fn test_visual_edit_text_xss_escaped() {
        let schema = get_template_schema("saas_landing").unwrap();
        let mut payload = test_payload();
        let mut es = VisualEditState::default();
        // Note: validate_slot_value rejects HTML tags in Text slots,
        // so XSS content won't pass validation. Test that it's rejected.
        let result = apply_text_edit(
            &mut payload,
            &mut es,
            &schema,
            "hero",
            "headline",
            "<script>alert('xss')</script>",
        );
        // Should be rejected by validation (Text slot disallows HTML)
        assert!(result.is_err());
    }

    #[test]
    fn test_visual_edit_persists_to_project() {
        let dir =
            std::env::temp_dir().join(format!("nexus-visual-edit-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut ts = default_token_set();
        let mut es = VisualEditState::default();
        let _ = apply_token_edit(&mut ts, &mut es, 1, None, "color-primary", "#ff0000");
        let _ = apply_token_edit(&mut ts, &mut es, 3, Some("hero"), "hero-bg", "#001122");

        // Save
        save_visual_edit_state(&dir, &es).unwrap();

        // Reload
        let loaded = load_visual_edit_state(&dir).unwrap();
        assert_eq!(
            loaded.foundation_overrides.get("color-primary"),
            Some(&"#ff0000".to_string())
        );
        assert_eq!(loaded.instance_overrides.len(), 1);
        assert_eq!(loaded.instance_overrides[0].value, "#001122");

        // Restore into a fresh TokenSet
        let mut ts2 = default_token_set();
        restore_visual_edits(&mut ts2, &loaded);
        assert_eq!(ts2.foundation.get("color-primary").unwrap(), "#ff0000");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_visual_edit_audit_trail() {
        // Visual edits are logged via the existing audit trail in Tauri commands.
        // This test verifies the edit state captures enough info for auditing.
        let mut ts = default_token_set();
        let mut es = VisualEditState::default();
        let _ = apply_token_edit(&mut ts, &mut es, 1, None, "color-accent", "#06b6d4");
        let _ = apply_token_edit(&mut ts, &mut es, 3, Some("features"), "card-bg", "#1a1a2e");

        // Verify we have audit-worthy data
        assert_eq!(es.foundation_overrides.len(), 1);
        assert_eq!(es.instance_overrides.len(), 1);
        assert_eq!(es.instance_overrides[0].section_id, "features");
        assert_eq!(es.instance_overrides[0].token_name, "card-bg");
    }
}
