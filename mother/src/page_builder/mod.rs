//! M3-Ca: private, pure request data. No production caller or route.
#![allow(dead_code)] // Private until the separately approved Cb binding.
mod assemble;
#[cfg(test)]
pub(crate) mod cb_fixtures;
#[cfg(test)]
mod cb_oracles;
mod draft;
#[cfg(test)]
mod fixtures;
pub(crate) mod lane;
#[cfg(test)]
mod oracles;
mod prompt;

use crate::effect::EffectRequest;
use nexus_governed_control::inert_page::ApprovedFacts;

/// M3E-R18 (8): the SHA-256 of each fixed part of the Builder's request (the
/// system prompt, the task template, the schema and each correction block),
/// never their text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestIdentity {
    pub system: [u8; 32],
    pub task: [u8; 32],
    pub schema: [u8; 32],
    /// `(code, sha256)` of each correction block, in the codes' order.
    pub corrections: Vec<(&'static str, [u8; 32])>,
}

pub fn request_identity() -> RequestIdentity {
    use sha2::{Digest, Sha256};
    use DraftCode::*;
    let hash = |text: &str| -> [u8; 32] { Sha256::digest(text.as_bytes()).into() };
    let codes = [
        NotOneObject,
        Empty,
        Markup,
        CharNotAllowed,
        WordTooLong,
        TooLarge,
        ForeignNumber,
        Email,
    ];
    RequestIdentity {
        system: hash(prompt::SYSTEM_M3),
        task: hash(prompt::TASK_M3),
        schema: hash(prompt::SCHEMA),
        corrections: codes
            .iter()
            .filter_map(|c| c.correction().map(|text| (c.code(), hash(&text))))
            .collect(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DraftCode {
    NotOneObject,
    Empty,
    Markup,
    CharNotAllowed,
    WordTooLong,
    TooLarge,
    ForeignNumber,
    Email,
    Needle,
    AssemblerFault,
}
impl DraftCode {
    fn code(self) -> &'static str {
        match self {
            Self::NotOneObject => "not_one_object",
            Self::Empty => "draft_empty",
            Self::Markup => "draft_markup",
            Self::CharNotAllowed => "draft_char_not_allowed",
            Self::WordTooLong => "draft_word_too_long",
            Self::TooLarge => "draft_too_large",
            Self::ForeignNumber => "draft_foreign_number",
            Self::Email => "draft_email",
            Self::Needle => "draft_needle",
            Self::AssemblerFault => "assembler_fault",
        }
    }
    fn correction(self) -> Option<String> {
        let text = match self {
            Self::NotOneObject => "answer with exactly one JSON object that matches the schema.",
            Self::Empty => "a part of the answer is empty.",
            Self::Markup => "write plain text only, with no HTML tags and no angle brackets.",
            Self::CharNotAllowed => {
                "use only Latin letters, digits and common punctuation; no emoji or other scripts."
            }
            Self::WordTooLong => "every word must have at most 40 characters.",
            Self::TooLarge => "the page is too long; write fewer and shorter parts.",
            Self::ForeignNumber => {
                "write no phone numbers and no long numbers; the system adds the phone number."
            }
            Self::Email => "write no email addresses.",
            Self::Needle | Self::AssemblerFault => return None,
        };
        Some(format!("\n\nNexus backend result:\nRejected: {text}"))
    }
    fn failure(self) -> &'static str {
        match self {
            Self::NotOneObject => "model_not_one_object",
            Self::Empty => "model_draft_empty",
            Self::Markup => "model_draft_markup",
            Self::CharNotAllowed => "model_draft_char_not_allowed",
            Self::WordTooLong => "model_draft_word_too_long",
            Self::TooLarge => "model_draft_too_large",
            Self::ForeignNumber => "model_draft_foreign_number",
            Self::Email => "model_draft_email",
            Self::Needle => "model_out_of_scope",
            Self::AssemblerFault => "assembler_fault",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BuildInputCode {
    FactsNotPageReady,
    InvalidNote,
    InvalidMarker,
    NotesAndFactsTooLarge,
    RequestTooLarge,
    Internal,
}
#[derive(Default)]
struct Builder {
    attempts: u8,
    done: bool,
}
enum BuildStep {
    Correction(String),
    File(Vec<EffectRequest>),
    Failed(DraftCode),
    Closed,
}
impl std::fmt::Debug for BuildStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Correction(_) => "Correction",
            Self::File(_) => "File",
            Self::Failed(_) => "Failed",
            Self::Closed => "Closed",
        })
    }
}
impl Builder {
    fn answer(&mut self, answer: &str, facts: &ApprovedFacts) -> BuildStep {
        if self.done {
            return BuildStep::Closed;
        }
        self.attempts += 1;
        match draft::parse_draft(answer).and_then(|d| assemble::file_requests(&d, facts)) {
            Ok(files) => {
                self.done = true;
                BuildStep::File(files)
            }
            Err(code) => {
                if self.attempts < 2 {
                    if let Some(text) = code.correction() {
                        return BuildStep::Correction(text);
                    }
                }
                self.done = true;
                BuildStep::Failed(code)
            }
        }
    }
}

/// Readiness only: no caller, model route, correction or effect construction.
pub(crate) fn facts_page_ready(facts: &ApprovedFacts) -> bool {
    assemble::facts_ready(facts).is_ok()
}

#[cfg(test)]
pub(crate) fn take_file_constructions_for_test() -> usize {
    assemble::constructions()
}
