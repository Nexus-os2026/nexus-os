//! Consented page data. A snapshot is data, never broker authority.
use crate::contract::{ContractError, ContractHash, MissionContract, Note, PassCriterion};
use crate::lane::{InvocationId, LaneRole};
use nexus_governed_control::{authority::approval::FactsView, inert_page::ApprovedFacts};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageFacts {
    pub name: String,
    pub hours: Vec<String>,
    pub phone: String,
}
impl fmt::Debug for PageFacts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PageFacts { redacted }")
    }
}
impl PageFacts {
    pub(crate) fn view(&self) -> FactsView {
        FactsView {
            name: self.name.clone(),
            hours: self.hours.clone(),
            phone: self.phone.clone(),
        }
    }
    pub(crate) fn approved(&self) -> Result<ApprovedFacts, ContractError> {
        self.view()
            .approved()
            .map_err(|_| ContractError::FactsNotPageReady)
    }
}

/// Only the coordinator constructs this immutable, invocation-bound snapshot.
/// The digest is public evidence; the Owner's data is skipped in lane hashing.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct PageBuildInput {
    contract_hash: ContractHash,
    invocation: InvocationId,
    content_sha256: String,
    #[serde(skip)]
    note: Note,
    #[serde(skip)]
    facts: PageFacts,
}
impl fmt::Debug for PageBuildInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PageBuildInput { redacted }")
    }
}
impl PageBuildInput {
    #[allow(dead_code)] // Consumed by the separately approved Cb caller.
    pub(crate) fn borrowed(&self) -> (&Note, &PageFacts) {
        (&self.note, &self.facts)
    }
    #[cfg(test)]
    pub(crate) fn unvalidated_for_test(c: &MissionContract, invocation: InvocationId) -> Self {
        let note = c.notes[0].clone();
        let facts = c.page_facts.clone().unwrap();
        Self {
            contract_hash: c.hash(),
            invocation,
            content_sha256: content_digest(&note, &facts),
            note,
            facts,
        }
    }
    #[cfg(test)]
    pub(crate) fn alter_skipped_for_test(&mut self, facts: bool) {
        if facts {
            self.facts.name.push('z');
        } else {
            self.note.text.push('z');
        }
    }
    pub(crate) fn validate(
        &self,
        c: &MissionContract,
        contract_hash: ContractHash,
        invocation: InvocationId,
    ) -> Result<(), ContractError> {
        if self.contract_hash != contract_hash
            || c.hash() != contract_hash
            || self.invocation != invocation
            || self.content_sha256 != content_digest(&self.note, &self.facts)
            || c.notes.as_slice() != [self.note.clone()]
            || c.page_facts.as_ref() != Some(&self.facts)
        {
            return Err(ContractError::PageInputIdentity);
        }
        c.validate_for_consent()
    }
}

// This order and the literal NUL-terminated domain are pinned independently.
fn content_digest(note: &Note, facts: &PageFacts) -> String {
    #[derive(Serialize)]
    struct Payload<'a> {
        note: &'a Note,
        facts: &'a PageFacts,
    }
    let bytes = serde_json::to_vec(&Payload { note, facts }).expect("page data serializes");
    let mut hash = Sha256::new();
    hash.update(b"nexus-m3/page-build-input/1\0");
    hash.update(bytes);
    hex::encode(hash.finalize())
}

/// The Owner-facing V0..V5 registry frozen in RULING Q1.
pub fn page_criteria() -> Vec<PassCriterion> {
    [
        ("V0", "index.html was created once, with your approval, and its receipt is recorded."),
        ("V1", "The written file is exactly the file you approved (same SHA-256 and size)."),
        ("V2", "The page passes the fixed inert-format check, under the same policy that was approved."),
        ("V3", "Your name, every hours line and the phone appear exactly as you typed them, and no other phone-like number appears."),
        ("V4", "A second, separate reader finds your name, every hours line and the phone in the page's text."),
        ("V5", "The page contains no email-shaped text."),
    ].into_iter().map(|(id, text)| PassCriterion { id: id.into(), text: text.into() }).collect()
}
pub(crate) fn validate_page(c: &MissionContract) -> Result<(), ContractError> {
    if c.pass_criteria != page_criteria() {
        return Err(ContractError::CriterionNotCheckable);
    }
    if c.notes.len() != 1 || c.notes[0].label != "notes" || c.notes[0].text.trim().is_empty() {
        return Err(ContractError::PageNoteInvalid);
    }
    let f = c
        .page_facts
        .as_ref()
        .ok_or(ContractError::FactsNotPageReady)?;
    let approved = f.approved()?;
    if !crate::page_builder::facts_page_ready(&approved) {
        return Err(ContractError::FactsNotPageReady);
    }
    if rendered_size(f, &c.notes[0]) > 2432 {
        return Err(ContractError::NotesAndFactsTooLarge);
    }
    Ok(())
}
pub(crate) fn rendered_size(f: &PageFacts, note: &Note) -> usize {
    let mut facts = format!("name: {}", f.name);
    for h in &f.hours {
        facts.push_str(&format!("\nhours: {h}"));
    }
    facts.push_str(&format!("\nphone: {}", f.phone));
    facts.len() + format!("--- {} ---\n{}\n", note.label, note.text).len()
}
pub(crate) fn prepare(
    c: &MissionContract,
    hash: ContractHash,
    invocation: InvocationId,
    role: LaneRole,
) -> Result<Option<PageBuildInput>, ContractError> {
    if role != LaneRole::Builder || c.page_facts.is_none() {
        return Ok(None);
    }
    c.validate_for_consent()?;
    let note = c.notes[0].clone();
    let facts = c
        .page_facts
        .clone()
        .ok_or(ContractError::FactsNotPageReady)?;
    let snapshot = PageBuildInput {
        contract_hash: hash,
        invocation,
        content_sha256: content_digest(&note, &facts),
        note,
        facts,
    };
    snapshot.validate(c, hash, invocation)?;
    Ok(Some(snapshot))
}

#[path = "oracles/m3d_inputs.rs"]
#[cfg(test)]
mod oracles;
