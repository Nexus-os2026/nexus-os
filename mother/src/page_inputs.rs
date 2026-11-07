//! Consented page data. A snapshot is data, never broker authority.
use crate::contract::{ContractError, ContractHash, MissionContract, Note, PassCriterion};
use crate::lane::{InvocationId, LaneRole};
use nexus_governed_control::{authority::approval::FactsView, inert_page::ApprovedFacts};
use serde::{Deserialize, Serialize};
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
    pub(crate) fn validate(
        &self,
        _: &MissionContract,
        _: ContractHash,
        _: InvocationId,
    ) -> Result<(), ContractError> {
        Err(ContractError::PageInputIdentity)
    }
}

pub fn page_criteria() -> Vec<PassCriterion> {
    vec![]
}
pub(crate) fn validate_page(_: &MissionContract) -> Result<(), ContractError> {
    Err(ContractError::FactsNotPageReady)
}
pub(crate) fn rendered_size(_: &PageFacts, _: &Note) -> usize {
    0
}
pub(crate) fn prepare(
    _: &MissionContract,
    _: ContractHash,
    _: InvocationId,
    _: LaneRole,
) -> Result<Option<PageBuildInput>, ContractError> {
    Err(ContractError::PageInputIdentity)
}

#[cfg(test)]
#[path = "oracles/m3d_inputs.rs"]
mod oracles;
