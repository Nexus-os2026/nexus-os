//! M3-Ca: private, pure request data. No production caller or route.
#![allow(dead_code)] // Private until the separately approved Cb binding.
mod assemble;
mod draft;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod oracles;
mod prompt;

use crate::effect::EffectRequest;
use nexus_governed_control::inert_page::ApprovedFacts;

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
        if self == Self::Needle {
            "model_out_of_scope"
        } else {
            self.code()
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
    fn answer(&mut self, _: &str, _: &ApprovedFacts) -> BuildStep {
        BuildStep::Closed
    }
}
