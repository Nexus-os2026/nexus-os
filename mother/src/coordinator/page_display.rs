//! M3-E M-3b (rulings M3E-R1, R8, R12): what Mother shows about a page
//! mission, read only from the verified journal (`FileSession::journal_view`,
//! which never touches the lifecycle) and the consented contract.
//!
//! A [`PageView`] is a content-free projection: the broker receipt's digest
//! and size, the recorded check codes, and how the mission ended. It has no
//! coordinator, door, grant or store, and building it changes nothing.
//! [`PageView::lines`] selects the frozen teaching texts (`teaching.rs`) whose
//! predicates hold; the host only escapes and prints them (M-5 wires it).
//!
//! Frozen rules (R12 (4), D8):
//! - T10 only from a journaled `PublicationReceipt` (bytes and digest from it
//!   alone), with the folder of the consented contract; no receipt, no T10;
//! - an overtaken receipt shows written without any claim (`t12_unchecked`),
//!   never T10 or success;
//! - T11 and `t12_passed` only when V0..V5 are all recorded `pass`; a
//!   recorded failure is `t12_failed(code)`; written with nothing recorded is
//!   `t12_unchecked`; a scripted verdict is never a recorded check;
//! - T9 by the recorded end: declined (consent or action denied), expired
//!   (the approval expired), stopped (a Stop with nothing published), and
//!   otherwise, once a File was asked for, uncertain;
//! - T13 follows only a recorded all-pass; T5, T7 and T15 are phase lines for
//!   M-5 to call.

use crate::contract::MissionContract;
use crate::journal::{Entry, MissionId};

/// The publication the broker recorded for the mission, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Publication {
    None,
    Written { sha256: String, bytes: u64 },
    Overtaken,
}

/// How the mission ended, from its journaled triggers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageEnd {
    Open,
    Declined,
    Expired,
    Stopped,
    Uncertain,
    Done,
    Other,
}

/// Content-free projection of one page mission's journal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageView {
    consented: Option<String>,
    file_asked: bool,
    publication: Publication,
    checks: Option<Vec<(String, String)>>,
    end: PageEnd,
}

/// Why no page lines can be selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewError {
    /// The contract given is not the one the journal records consent to.
    NotConsented,
}

/// A phase whose line M-5 prints at that moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PagePhase {
    /// T5: while the model drafts, before any File is asked for.
    Drafting,
    /// T7: just before the broker's file dialog.
    BeforeFileDialog,
    /// T15: undo refused because the Owner changed the file.
    UndoRefusedAfterEdit,
}

/// The frozen line of `phase`.
pub fn phase_line(_phase: PagePhase) -> &'static str {
    ""
}

/// Project mission `id` from `entries` (a verified journal).
pub fn page_view(_entries: &[Entry], _id: MissionId) -> PageView {
    PageView {
        consented: None,
        file_asked: false,
        publication: Publication::None,
        checks: None,
        end: PageEnd::Open,
    }
}

impl PageView {
    pub fn publication(&self) -> &Publication {
        &self.publication
    }

    pub fn end(&self) -> PageEnd {
        self.end
    }

    /// The recorded checks, if the Verifier's record exists.
    pub fn checks(&self) -> Option<&[(String, String)]> {
        self.checks.as_deref()
    }

    /// The frozen teaching lines whose predicates hold, for `contract` (the
    /// consented one) and the Owner's `facts_file` name. Not a page mission:
    /// no lines. Text is raw: the caller escapes it for display.
    pub fn lines(
        &self,
        _contract: &MissionContract,
        _facts_file: &str,
    ) -> Result<Vec<String>, ViewError> {
        let _ = (&self.consented, self.file_asked);
        Ok(Vec::new())
    }
}
