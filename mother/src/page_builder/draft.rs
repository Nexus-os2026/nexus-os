//! Strict draft value; no content-bearing diagnostics.
use super::DraftCode;
use nexus_governed_control::inert_page::POLICY_V1;
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
impl PageDraft {
    fn texts(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.lead.as_str())
            .chain(self.sections.iter().flat_map(|s| {
                std::iter::once(s.heading.as_str())
                    .chain(s.paragraphs.iter().map(String::as_str))
                    .chain(s.items.iter().map(String::as_str))
            }))
            .chain(std::iter::once(self.footer.as_str()))
    }
}
fn normalize(s: &mut String) {
    *s = s
        .replace(['\t', '\n', '\r'], " ")
        .split(' ')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
}
fn scalar_len(s: &str) -> usize {
    s.chars().count()
}
pub(super) fn parse_draft(answer: &str) -> Result<PageDraft, DraftCode> {
    let mut d: PageDraft = serde_json::from_str(answer).map_err(|_| DraftCode::NotOneObject)?;
    normalize(&mut d.lead);
    normalize(&mut d.footer);
    for s in &mut d.sections {
        normalize(&mut s.heading);
        for t in s.paragraphs.iter_mut().chain(&mut s.items) {
            normalize(t);
        }
    }
    // Each pass spans all fields: an earlier field cannot hide a higher priority code.
    if d.texts().any(|s| {
        POLICY_V1
            .needles
            .iter()
            .any(|(n, _)| s.to_ascii_lowercase().contains(n))
    }) {
        return Err(DraftCode::Needle);
    }
    if d.texts().any(|s| s.contains(['<', '>'])) {
        return Err(DraftCode::Markup);
    }
    if d.texts().any(|s| {
        s.chars().any(|c| {
            !POLICY_V1
                .profile_ranges
                .iter()
                .any(|&(a, b)| (a..=b).contains(&(c as u32)))
        })
    }) {
        return Err(DraftCode::CharNotAllowed);
    }
    if d.texts().any(|s| s.split(' ').any(|w| scalar_len(w) > 40)) {
        return Err(DraftCode::WordTooLong);
    }
    if d.texts().any(str::is_empty) {
        return Err(DraftCode::Empty);
    }
    if scalar_len(&d.lead) > 240
        || scalar_len(&d.footer) > 160
        || !(1..=4).contains(&d.sections.len())
        || d.sections.iter().any(|s| {
            scalar_len(&s.heading) > 80
                || !(1..=2).contains(&s.paragraphs.len())
                || s.items.len() > 8
                || s.paragraphs.iter().any(|s| scalar_len(s) > 400)
                || s.items.iter().any(|s| scalar_len(s) > 120)
        })
    {
        return Err(DraftCode::TooLarge);
    }
    Ok(d)
}
