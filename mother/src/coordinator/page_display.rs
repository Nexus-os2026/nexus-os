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
//!   (the approval expired), stopped (a Stop before any File was asked for,
//!   or a settlement confirmed not published), and otherwise, once a File was
//!   asked for, uncertain;
//! - T13 follows only a recorded all-pass; T5, T7 and T15 are phase lines for
//!   M-5 to call.

use crate::contract::MissionContract;
use crate::evidence::{LapseReason, SettlementOutcome};
use crate::journal::{Entry, Event, MissionId};
use crate::state::Trigger;
use crate::teaching::{
    t10, t12_failed, t12_passed, t12_unchecked, t13, t16, t16_failed, T11_CHECKED,
    T15_UNDO_REFUSED, T16_DECLINED, T16_EXPIRED, T16_NONE_RECORDED, T16_PASSED, T16_STOPPED,
    T16_UNCHECKED, T16_UNCONFIRMED, T5_DRAFTING, T7_BEFORE_FILE, T9_DECLINED, T9_EXPIRED,
    T9_STOPPED, T9_UNCERTAIN,
};
use crate::verifier::CheckId;
use nexus_governed_control::authority::evidence::escaped;

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

impl PageEnd {
    /// Its closed code (M-6 receipts).
    pub fn as_str(self) -> &'static str {
        match self {
            PageEnd::Open => "open",
            PageEnd::Declined => "declined",
            PageEnd::Expired => "expired",
            PageEnd::Stopped => "stopped",
            PageEnd::Uncertain => "uncertain",
            PageEnd::Done => "done",
            PageEnd::Other => "other",
        }
    }
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
    /// The contract is not a page mission's.
    NotAPage,
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
pub fn phase_line(phase: PagePhase) -> &'static str {
    match phase {
        PagePhase::Drafting => T5_DRAFTING,
        PagePhase::BeforeFileDialog => T7_BEFORE_FILE,
        PagePhase::UndoRefusedAfterEdit => T15_UNDO_REFUSED,
    }
}

/// Project mission `id` from `entries` (a verified journal).
pub fn page_view(entries: &[Entry], id: MissionId) -> PageView {
    let mut view = PageView {
        consented: None,
        file_asked: false,
        publication: Publication::None,
        checks: None,
        end: PageEnd::Open,
    };
    for entry in entries.iter().filter(|e| e.mission == id) {
        match &entry.event {
            Event::Transition { trigger } => match trigger {
                Trigger::Consent { hash, .. } => view.consented = Some(hash.to_hex()),
                Trigger::Deny { .. } | Trigger::ActionDenied { .. } => view.end = PageEnd::Declined,
                Trigger::AwaitApproval { .. } => view.file_asked = true,
                Trigger::ApprovalLapsed { reason, .. } => {
                    view.end = if *reason == LapseReason::ApprovalExpired {
                        PageEnd::Expired
                    } else {
                        PageEnd::Uncertain
                    }
                }
                // A Stop once a File was asked for, with no settlement
                // recorded, says nothing of what became of it.
                Trigger::Stop { .. } if view.file_asked => view.end = PageEnd::Uncertain,
                Trigger::Stop { .. } => view.end = PageEnd::Stopped,
                Trigger::Settled { outcome, .. } => {
                    view.file_asked = true;
                    view.end = match outcome {
                        SettlementOutcome::NotPublished => PageEnd::Stopped,
                        SettlementOutcome::Uncertain => PageEnd::Uncertain,
                        SettlementOutcome::Published { .. } => PageEnd::Other,
                    }
                }
                Trigger::VerifyPass => view.end = PageEnd::Done,
                Trigger::Block { .. } | Trigger::Fail { .. } | Trigger::VerifyFail { .. } => {
                    view.end = PageEnd::Other
                }
                _ => {}
            },
            Event::ApprovalPending { .. } | Event::EffectDispatched { .. } => {
                view.file_asked = true
            }
            Event::PublicationReceipt {
                sha256,
                bytes,
                overtaken,
                ..
            } => {
                view.publication = if *overtaken {
                    Publication::Overtaken
                } else {
                    Publication::Written {
                        sha256: sha256.clone(),
                        bytes: *bytes,
                    }
                }
            }
            Event::VerificationRecorded { checks, .. } => view.checks = Some(checks.clone()),
            _ => {}
        }
    }
    view
}

impl PageView {
    pub fn publication(&self) -> &Publication {
        &self.publication
    }

    pub fn end(&self) -> PageEnd {
        self.end
    }

    /// T16 (M3E-R18 (3)) for `contract` (the consented one), from the
    /// journaled facts only. `None` while nothing has ended or been written,
    /// or for a mission that is not a page.
    pub fn summary(&self, contract: &MissionContract) -> Result<Option<String>, ViewError> {
        if contract.page_facts.is_none() {
            return Ok(None);
        }
        if self.consented.as_deref() != Some(contract.hash().to_hex().as_str()) {
            return Err(ViewError::NotConsented);
        }
        let outcome = match &self.publication {
            Publication::Written { .. } => match self.recorded() {
                Some(Ok(())) => T16_PASSED.to_string(),
                Some(Err(code)) => t16_failed(&escaped(&code)),
                None => T16_UNCHECKED.to_string(),
            },
            Publication::Overtaken => T16_UNCONFIRMED.to_string(),
            Publication::None => match self.end {
                PageEnd::Declined => T16_DECLINED.to_string(),
                PageEnd::Expired => T16_EXPIRED.to_string(),
                PageEnd::Stopped => T16_STOPPED.to_string(),
                PageEnd::Uncertain => T16_UNCONFIRMED.to_string(),
                PageEnd::Done | PageEnd::Other if self.file_asked => T16_UNCONFIRMED.to_string(),
                PageEnd::Open | PageEnd::Done | PageEnd::Other => return Ok(None),
            },
        };
        let checked = match &self.checks {
            Some(checks) if !checks.is_empty() => checks
                .iter()
                .map(|(id, code)| format!("{} {}", escaped(id), escaped(code)))
                .collect::<Vec<_>>()
                .join(", "),
            _ => T16_NONE_RECORDED.to_string(),
        };
        Ok(Some(t16(&outcome, &checked)))
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
        contract: &MissionContract,
        facts_file: &str,
    ) -> Result<Vec<String>, ViewError> {
        if contract.page_facts.is_none() {
            return Ok(Vec::new());
        }
        if self.consented.as_deref() != Some(contract.hash().to_hex().as_str()) {
            return Err(ViewError::NotConsented);
        }
        let mut lines = Vec::new();
        match &self.publication {
            Publication::Written { sha256, bytes } => {
                // M3E-R17: untrusted fields are escaped before assembly.
                let folder = contract.requested_folder.as_deref().unwrap_or_default();
                lines.push(t10(&escaped(folder), *bytes, &escaped(sha256)));
                match self.recorded() {
                    Some(Ok(())) => {
                        lines.push(T11_CHECKED.into());
                        lines.push(t12_passed());
                        lines.push(t13(&escaped(facts_file)));
                    }
                    Some(Err(code)) => lines.push(t12_failed(&escaped(&code))),
                    None => lines.push(t12_unchecked()),
                }
            }
            Publication::Overtaken => lines.push(t12_unchecked()),
            Publication::None => match self.end {
                PageEnd::Declined => lines.push(T9_DECLINED.into()),
                PageEnd::Expired => lines.push(T9_EXPIRED.into()),
                PageEnd::Stopped => lines.push(T9_STOPPED.into()),
                PageEnd::Uncertain => lines.push(T9_UNCERTAIN.into()),
                PageEnd::Done | PageEnd::Other if self.file_asked => {
                    lines.push(T9_UNCERTAIN.into())
                }
                PageEnd::Open | PageEnd::Done | PageEnd::Other => {}
            },
        }
        Ok(lines)
    }

    /// `Ok` when V0..V5 are all recorded `pass`; the first recorded failure's
    /// bare code otherwise; `None` with no record.
    fn recorded(&self) -> Option<Result<(), String>> {
        let checks = self.checks.as_ref()?;
        for id in CheckId::ALL.map(CheckId::as_str) {
            match checks.iter().find(|(of, _)| of == id) {
                Some((_, code)) if code == "pass" => {}
                Some((_, code)) => {
                    let bare = code.strip_prefix("fail:").unwrap_or(code);
                    return Some(Err(bare.to_string()));
                }
                None => return Some(Err("not_recorded".into())),
            }
        }
        Some(Ok(()))
    }
}
