//! Strict draft value; no content-bearing diagnostics.
use super::DraftCode;
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PageDraft {
    pub lead: String,
    pub sections: Vec<DraftSection>,
    pub footer: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DraftSection {
    pub heading: String,
    pub paragraphs: Vec<String>,
    pub items: Vec<String>,
}
pub(super) fn parse_draft(_: &str) -> Result<PageDraft, DraftCode> {
    Err(DraftCode::NotOneObject)
}
