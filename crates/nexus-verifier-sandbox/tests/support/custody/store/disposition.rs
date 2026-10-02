//! The store's [`DispositionValidator`] (design sections 10.2 step 9 and
//! 13.4): it returns a [`ValidatedDisposition`] only for a binding that has
//! a root-owned disposition file, read and parsed under the store lock after
//! activation, whose fields restate exactly the incident the store computed
//! (`recorded-unsettled` included), and only with that file's reason. A
//! binding, a name, a string or a decoded record never stands in for one.
//!
//! Provenance (P2-V1-R3B-I3-I1-R1). Two things are kept apart:
//! - **The data comparison.** [`exact_restatement`] is a pure comparison of
//!   plain data. It returns a `bool` and authorizes nothing.
//! - **The authority.** A [`StoreValidator`] exists only as a borrow of one
//!   opening's own verified state, through
//!   [`super::open::Opened::validator`].
//!
//! The validator's facts:
//! - **Owner.** The opening owns the scan's incidents and dispositions. They
//!   were read under its store lock, after its activation, for its
//!   revalidated selection and its root.
//! - **Lifetime.** The validator cannot outlive the opening, and so not the
//!   lock. No constructor takes a `ScanResult`: a scan copy, edited or not,
//!   is plain data and constructs nothing.
//! - **Stale or foreign state.** It is never consulted: the validator reads
//!   only the opening's own state, and checks the root and the
//!   content-addressed binding.
//!
//! This is the adapter's API provenance only. It does not authenticate
//! storage the store uid can write (G-AUTH stays open). The core's own
//! `ValidatedDisposition::new` and `DispositionValidator` remain public: the
//! store's guarantee is that the custody it retains is only ever asked of
//! this validator ([`super::owner::start_owner`]).

use std::collections::BTreeMap;

use super::super::{DispositionValidator, IncidentBinding, ValidatedDisposition};
use super::classify::Incident;
use super::format::Disposition;

/// Whether `disposition` restates exactly `incident`, under `binding` and
/// `root_id`: the same root, the same binding, the same facts (every field,
/// `recorded-unsettled` included), and a binding that the facts recompute
/// to. A comparison of plain data: it authorizes nothing.
pub fn exact_restatement(
    root_id: &[u8; 16],
    binding: &[u8; 32],
    incident: &Incident,
    disposition: &Disposition,
) -> bool {
    disposition.root == *root_id
        && disposition.binding == *binding
        && disposition.facts == incident.facts
        && disposition.facts.binding(root_id) == Some(*binding)
}

/// The validator over one opening's verified incidents and dispositions,
/// borrowed from that opening.
pub struct StoreValidator<'a> {
    root_id: [u8; 16],
    incidents: &'a BTreeMap<[u8; 32], Incident>,
    dispositions: &'a BTreeMap<[u8; 32], Disposition>,
}

impl<'a> StoreValidator<'a> {
    /// Internal: only [`super::open::Opened::validator`] calls it, with that
    /// opening's own root and verified state.
    pub(super) fn of(
        root_id: &[u8; 16],
        incidents: &'a BTreeMap<[u8; 32], Incident>,
        dispositions: &'a BTreeMap<[u8; 32], Disposition>,
    ) -> StoreValidator<'a> {
        StoreValidator {
            root_id: *root_id,
            incidents,
            dispositions,
        }
    }
}

impl DispositionValidator for StoreValidator<'_> {
    fn validate(&self, binding: &IncidentBinding) -> Option<ValidatedDisposition> {
        let (bytes, incident) = self
            .incidents
            .iter()
            .find(|(bytes, _)| IncidentBinding::new(**bytes) == *binding)?;
        let disposition = self.dispositions.get(bytes)?;
        exact_restatement(&self.root_id, bytes, incident, disposition)
            .then(|| ValidatedDisposition::new(*binding, disposition.reason))
    }
}
