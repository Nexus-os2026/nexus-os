//! The store's [`DispositionValidator`] (design sections 10.2 step 9 and
//! 13.4): it returns a [`ValidatedDisposition`] only for a binding that has
//! a root-owned disposition file, read and parsed under the store lock after
//! activation, whose fields restate exactly the incident the store computed
//! (`recorded-unsettled` included), and only with that file's reason. A
//! binding, a name, a string or a decoded record never stands in for one.

use std::collections::BTreeMap;

use super::super::{DispositionValidator, IncidentBinding, ValidatedDisposition};
use super::classify::Incident;
use super::format::Disposition;
use super::open::ScanResult;

/// The validator over one opening's scan.
pub struct StoreValidator {
    root_id: [u8; 16],
    incidents: BTreeMap<[u8; 32], Incident>,
    dispositions: BTreeMap<[u8; 32], Disposition>,
}

impl StoreValidator {
    pub fn new(root_id: &[u8; 16], scan: &ScanResult) -> StoreValidator {
        StoreValidator {
            root_id: *root_id,
            incidents: scan.level.incidents.clone(),
            dispositions: scan.dispositions.clone(),
        }
    }
}

impl DispositionValidator for StoreValidator {
    fn validate(&self, binding: &IncidentBinding) -> Option<ValidatedDisposition> {
        let (bytes, incident) = self
            .incidents
            .iter()
            .find(|(bytes, _)| IncidentBinding::new(**bytes) == *binding)?;
        let disposition = self.dispositions.get(bytes)?;
        let exact = disposition.root == self.root_id
            && disposition.binding == *bytes
            && disposition.facts == incident.facts
            && disposition.facts.binding(&self.root_id) == Some(*bytes);
        exact.then(|| ValidatedDisposition::new(*binding, disposition.reason))
    }
}
