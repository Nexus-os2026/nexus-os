//! Typed page construction and checked File request data, without effects.
use super::{draft::PageDraft, BuildInputCode, DraftCode};
use crate::effect::EffectRequest;
use nexus_governed_control::inert_page::ApprovedFacts;
pub(super) fn facts_ready(_: &ApprovedFacts) -> Result<(), BuildInputCode> {
    Err(BuildInputCode::FactsNotPageReady)
}
pub(super) fn assemble(_: &PageDraft, _: &ApprovedFacts) -> Result<Vec<u8>, DraftCode> {
    Err(DraftCode::AssemblerFault)
}
pub(super) fn file_limits(_: &str) -> Result<(), DraftCode> {
    Err(DraftCode::TooLarge)
}
pub(super) fn file_requests(
    _: &PageDraft,
    _: &ApprovedFacts,
) -> Result<Vec<EffectRequest>, DraftCode> {
    Err(DraftCode::AssemblerFault)
}
#[cfg(test)]
thread_local! { static CONSTRUCTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(super) fn constructions() -> usize {
    CONSTRUCTIONS.with(|c| c.replace(0))
}
