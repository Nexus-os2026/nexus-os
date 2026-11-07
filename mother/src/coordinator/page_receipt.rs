//! M3-E M-6 (rulings M3E-R1, R18): the mission part of the content-free
//! `receipts` export, read only from the verified journal
//! (`FileSession::journal_view`) and the consented contract.
//!
//! A [`MissionReceipt`] holds ids, hashes, counts and closed codes only: no
//! page, notes, facts text, answers or contract body. Building it changes
//! nothing and consumes nothing (no take-once working detail). A field the
//! journal should hold but does not is listed in `missing`; identities that
//! do not join are listed in `mismatched`. Neither is ever a zero.
//!
//! The consent's broker `evidence_seq` is not journaled (R18 (4)): the
//! receipt carries the consent's Mother journal seq and entry hash instead.

use crate::contract::{EffectKind, MissionContract};
use crate::coordinator::page_display::{page_view, ViewError};
use crate::effect::PendingAction;
use crate::evidence::SettlementOutcome;
use crate::journal::{Entry, Event, MissionId};
use crate::lane::LaneRole;
use crate::state::Trigger;
use nexus_governed_control::inert_page::ApprovedFacts;

/// The broker's publication receipt, as journaled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationFacts {
    pub sha256: String,
    pub bytes: u64,
    pub policy_hash: Option<String>,
    pub overtaken: bool,
}

/// The content-free mission facts of one page mission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionReceipt {
    pub mission: MissionId,
    /// The consented contract's hash.
    pub contract_hash: String,
    /// The consent's Mother journal entry: seq, hash and wall time.
    pub consent_seq: u64,
    pub consent_entry_hash: String,
    pub consent_at_wall_ms: u64,
    /// The consented facts' hash (`None`: not computable).
    pub facts_hash: Option<String>,
    /// Lane invocations per role, in `LaneRole::ALL` order.
    pub lane_calls: Vec<(&'static str, u32)>,
    pub model_calls_charged: u32,
    pub effects_dispatched: u32,
    pub publication: Option<PublicationFacts>,
    pub checks: Option<Vec<(String, String)>>,
    pub report_digest: Option<String>,
    /// The recorded end (`PageEnd` code).
    pub end: &'static str,
    pub missing: Vec<&'static str>,
    pub mismatched: Vec<&'static str>,
}

/// Mission `id`'s receipt from `entries` (a verified journal), for
/// `contract`, which must be the consented one.
pub fn mission_receipt(
    entries: &[Entry],
    id: MissionId,
    contract: &MissionContract,
) -> Result<MissionReceipt, ViewError> {
    let _ = (entries, id, contract);
    Err(ViewError::NotConsented)
}
