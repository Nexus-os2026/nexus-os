//! Shared interactive-element types produced by both the fixture
//! `Enumerator` and the live AT-SPI `LiveEnumerator`. See v1.1 §4
//! and Phase 1.5 Group B architecture notes.
//!
//! These types intentionally live in their own module so both
//! enumerators can depend on them without introducing a cycle.

use serde::{Deserialize, Serialize};

/// Axis-aligned bounding box in screen coordinates (pixels).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// One interactive element captured from the live accessibility tree
/// or from a pre-recorded fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveElement {
    /// Stable identifier composed as `"role:accessible_name"`,
    /// e.g. `"button:New Compare"`. Used as the `selector` field
    /// in `RepairTicket.dom_context`.
    pub element_id: String,
    /// AT-SPI role string, lower-cased (e.g. `"button"`, `"link"`,
    /// `"text"`, `"combo box"`).
    pub role: String,
    /// The accessible name of the element (from `aria-label`, text
    /// content, or placeholder).
    pub accessible_name: String,
    /// The accessible description, if any. Empty string if absent.
    pub description: String,
    /// Axis-aligned bounding box in screen coordinates.
    pub bounding_box: BoundingBox,
    /// `true` if the element is currently enabled (not greyed out).
    pub is_enabled: bool,
    /// `true` if the element is currently visible on screen.
    pub is_visible: bool,
    /// Free-form text content, truncated to 200 chars. May be empty.
    pub text_content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(role: &str, name: &str) -> InteractiveElement {
        InteractiveElement {
            element_id: format!("{role}:{name}"),
            role: role.to_string(),
            accessible_name: name.to_string(),
            description: String::new(),
            bounding_box: BoundingBox {
                x: 10,
                y: 20,
                width: 100,
                height: 30,
            },
            is_enabled: true,
            is_visible: true,
            text_content: String::new(),
        }
    }

    #[test]
    fn test_element_id_format() {
        let el = sample("button", "New Compare");
        assert_eq!(el.element_id, "button:New Compare");
    }

    #[test]
    fn test_element_round_trip() {
        let el = sample("link", "History");
        let json = serde_json::to_string(&el).expect("serialize");
        let back: InteractiveElement = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(el, back);
    }

    #[test]
    fn test_bounding_box_zero() {
        let bb = BoundingBox {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };
        let json = serde_json::to_string(&bb).unwrap();
        let back: BoundingBox = serde_json::from_str(&json).unwrap();
        assert_eq!(bb, back);
    }

    #[test]
    fn test_interactive_element_empty_name_fallback() {
        // When accessible_name and description are both empty, the
        // live enumerator is expected to fall back to "role:index".
        // This test documents that convention using a hand-built
        // element — the live walker enforces it at build time.
        let el = InteractiveElement {
            element_id: "button:3".to_string(),
            role: "button".to_string(),
            accessible_name: String::new(),
            description: String::new(),
            bounding_box: BoundingBox {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            is_enabled: true,
            is_visible: true,
            text_content: String::new(),
        };
        assert!(el.accessible_name.is_empty());
        assert!(el.description.is_empty());
        assert_eq!(el.element_id, "button:3");
    }
}
