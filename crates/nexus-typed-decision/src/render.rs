// The system-prompt wording, the per-request output schema and the
// corrective-retry texts in this file follow system-one-adapter-python
// v0.2.1 (commit e1d4cc93, https://github.com/typesafe-ai/system-one-adapter-python),
// ported to Rust and changed for Mother. No adapter code is vendored.
//
// MIT License
//
// Copyright (c) 2026 TypeSafe AI
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! Two artifacts from one registry (§6): the system text, which carries
//! every instruction, and the prose-free response schema. Also the fixed
//! correction sentence (§8).

use crate::registry::QuestionSet;

/// The fixed texts of `td-prompt-v1` (Appendix A).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Texts {
    pub(crate) preamble: &'static str,
    pub(crate) rule_noul_probabilities: &'static str,
    pub(crate) describe_page_text_v1: &'static str,
    pub(crate) describe_untrusted_text_v1: &'static str,
    pub(crate) shape_intro: &'static str,
    pub(crate) shape_ending: &'static str,
    pub(crate) correction_prefix: &'static str,
    pub(crate) correction_suffix: &'static str,
    /// One phrase per [`DecodeCode`], in [`DecodeCode::ALL`] order.
    pub(crate) phrases: [&'static str; 9],
}

// STUB (oracle commit): the texts are not written yet.
pub(crate) const TEXTS_V1: Texts = Texts {
    preamble: "",
    rule_noul_probabilities: "",
    describe_page_text_v1: "",
    describe_untrusted_text_v1: "",
    shape_intro: "",
    shape_ending: "",
    correction_prefix: "",
    correction_suffix: "",
    phrases: [""; 9],
};

/// The typed decoder's failure codes (§7); each has one fixed correction
/// phrase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeCode {
    NotOneObject,
    DuplicateKey,
    UnknownKey,
    MissingKey,
    WrongType,
    BadNumber,
    OutOfRange,
    BadSum,
    TooDeep,
}

impl DecodeCode {
    pub const ALL: [Self; 9] = [
        Self::NotOneObject,
        Self::DuplicateKey,
        Self::UnknownKey,
        Self::MissingKey,
        Self::WrongType,
        Self::BadNumber,
        Self::OutOfRange,
        Self::BadSum,
        Self::TooDeep,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            Self::NotOneObject => "not_one_object",
            Self::DuplicateKey => "duplicate_key",
            Self::UnknownKey => "unknown_key",
            Self::MissingKey => "missing_key",
            Self::WrongType => "wrong_type",
            Self::BadNumber => "bad_number",
            Self::OutOfRange => "out_of_range",
            Self::BadSum => "bad_sum",
            Self::TooDeep => "too_deep",
        }
    }

    /// The fixed phrase for this code.
    pub fn phrase(self) -> &'static str {
        TEXTS_V1.phrases[self as usize]
    }
}

/// A set this version cannot render.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    /// Appendix A defines the system text for Noul questions in
    /// probabilities mode only.
    #[error("td_render:no_registered_text")]
    NoRegisteredText,
}

/// The attempt-1 system text.
pub fn render_system(set: &QuestionSet) -> Result<String, RenderError> {
    render_system_with(set, &TEXTS_V1)
}

pub(crate) fn render_system_with(set: &QuestionSet, texts: &Texts) -> Result<String, RenderError> {
    // STUB (oracle commit).
    let _ = (set, texts);
    Err(RenderError::NoRegisteredText)
}

/// The shape line, such as `{"name_shown":P,"hours_shown":P}`.
pub fn render_shape(set: &QuestionSet) -> Result<String, RenderError> {
    // STUB (oracle commit).
    let _ = set;
    Err(RenderError::NoRegisteredText)
}

/// The prose-free response schema, in registry order.
pub fn render_schema(set: &QuestionSet) -> String {
    // STUB (oracle commit).
    let _ = set;
    String::new()
}

/// The fixed correction sentence for a decoder code: prefix, phrase,
/// suffix. It takes only the code, so no reply byte can reach it.
pub fn correction_sentence(code: DecodeCode) -> String {
    // STUB (oracle commit).
    let _ = code;
    String::new()
}

/// The attempt-2 system text: attempt 1's, two newlines, the sentence.
pub fn render_correction_system(
    set: &QuestionSet,
    code: DecodeCode,
) -> Result<String, RenderError> {
    let first = render_system(set)?;
    Ok(format!("{first}\n\n{}", correction_sentence(code)))
}
