//! DOM enumerator. Phase 1.2 implements fixture-based enumeration: the
//! enumerator reads a static HTML snapshot, walks every interactive
//! element, and produces stable fingerprints plus a destructive-kind
//! tag from `destructive_policy`.
//!
//! Live DOM scraping via `nexus-computer-use` lands in Phase 1.3. The
//! `enumerate_page` method is kept as a Phase 1.1 stub so downstream
//! callers that predate the fixture path continue to compile.

use std::path::Path;

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::specialists::destructive_policy::{classify_element_kind, ElementKind};
use crate::Error;

/// One enumerated UI element.
///
/// `fingerprint` is a SHA-256 hex digest of `tag|id|label`. The scout
/// keys every per-element observation off this fingerprint so that
/// cosmetic DOM churn (reordered siblings, class name changes) does
/// not silently break the observation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    /// DOM id if present, otherwise a deterministic `auto_<prefix>`
    /// derived from the fingerprint.
    pub id: String,
    /// Full 64-char hex SHA-256 of `tag|id|label`.
    pub fingerprint: String,
    /// Classification: destructive wins over tag-based variants.
    pub kind: ElementKind,
    /// Visible label: first non-empty of aria-label, alt, placeholder,
    /// or the trimmed inner text.
    pub label: String,
    /// Geometric bounds. Always `None` for fixture enumeration — the
    /// HTML snapshot has no layout information. Populated in Phase 1.3
    /// when the live DOM path ships.
    pub bounds: Option<(i32, i32, i32, i32)>,
}

/// DOM enumerator. Holds no state.
#[derive(Debug, Default)]
pub struct Enumerator;

impl Enumerator {
    pub fn new() -> Self {
        Self
    }

    /// Enumerate every interactive element in an HTML fixture file.
    ///
    /// "Interactive" is defined as the CSS selector
    /// `button, input, select, a[href], [role="button"]`.
    pub fn enumerate_fixture(&self, html_path: &Path) -> crate::Result<Vec<Element>> {
        let html_text = std::fs::read_to_string(html_path)?;
        let doc = Html::parse_document(&html_text);

        let selector = Selector::parse("button, input, select, a[href], [role=\"button\"]")
            .map_err(|e| Error::InvariantViolation(format!("selector parse: {:?}", e)))?;

        let mut elements = Vec::new();
        for el in doc.select(&selector) {
            let tag = el.value().name().to_string();
            let dom_id = el.value().attr("id").unwrap_or("").to_string();
            let label = extract_label(&el);
            let fingerprint = compute_fingerprint(&tag, &dom_id, &label);
            let id = if dom_id.is_empty() {
                format!("auto_{}", &fingerprint[..8])
            } else {
                dom_id
            };
            let kind = classify_element_kind(&label, &tag);

            elements.push(Element {
                id,
                fingerprint,
                kind,
                label,
                bounds: None,
            });
        }
        Ok(elements)
    }

    /// Phase 1.1 stub kept for backward compatibility. Live DOM
    /// enumeration lands in Phase 1.3 alongside `nexus-computer-use`.
    pub fn enumerate_page(&self, _page: &str) -> crate::Result<Vec<Element>> {
        Ok(Vec::new())
    }
}

/// Extract a visible label for an element.
///
/// Preference order: aria-label → alt → placeholder → trimmed inner
/// text. Returns an empty string if none of those is present.
fn extract_label(el: &scraper::ElementRef) -> String {
    if let Some(aria) = el.value().attr("aria-label") {
        let t = aria.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    if let Some(alt) = el.value().attr("alt") {
        let t = alt.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    if let Some(placeholder) = el.value().attr("placeholder") {
        let t = placeholder.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    let text: String = el.text().collect::<String>().trim().to_string();
    text
}

/// Compute a stable 64-char hex SHA-256 digest of `tag|id|label`.
fn compute_fingerprint(tag: &str, id: &str, label: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{}|{}|{}", tag, id, label).as_bytes());
    let digest = hasher.finalize();
    hex_encode(&digest)
}

/// Lower-case hex encoding without pulling in a `hex` crate dependency.
fn hex_encode(bytes: &[u8]) -> String {
    const CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(CHARS[(b >> 4) as usize] as char);
        out.push(CHARS[(b & 0x0f) as usize] as char);
    }
    out
}
