//! Typed decisions, the pure core (design v4, PRs T1a and T1b).
//!
//! A lane asks a fixed, code-reviewed set of typed questions about
//! backend-owned state. This crate holds what needs no I/O: the
//! code-owned registry (`sen_1`, `ver_1`, `inj_1`), its hashes, the
//! projections, the character profile, the inventory matcher, the
//! escaping and framing, the system text and the prose-free schema
//! rendered from one registry, the fixed correction sentences and the
//! exact byte budget (T1a); the strict reader, exact fixed-point numbers,
//! normalization, readings, the lane tables, the assessment slot with its
//! tokens and acknowledgements, and the display-only record view (T1b). It
//! has no I/O, no network, no clock and no lock. The result of a typed
//! decision is advice; it never grants anything.
//!
//! The question-and-answer model, the per-request schema, the document
//! escaping, the corrective retry and parts of the prompt wording follow
//! system-one-adapter-python v0.2.1 (`e1d4cc93`, MIT, Copyright (c) 2026
//! TypeSafe AI); see `THIRD-PARTY-NOTICES` and the notices in `render.rs`
//! and `projection.rs`.
//!
//! The registry is code-owned. It is readable from outside the crate:
//! ```
//! let set = &nexus_typed_decision::SEN_1;
//! assert_eq!(set.id().as_str(), "sen_1");
//! let _hash = nexus_typed_decision::set_hash(set);
//! ```
//! but no public constructor takes a runtime string (TD-O1):
//! ```compile_fail
//! let text: &'static str = Box::leak(String::from("q").into_boxed_str());
//! let _id = nexus_typed_decision::QuestionId::new(text);
//! ```
//! a question cannot be assembled outside the crate:
//! ```compile_fail
//! let q = nexus_typed_decision::SEN_1.questions()[0];
//! let _forged = nexus_typed_decision::Question { instructions: "x", ..q };
//! ```
//! and saved JSON cannot become a question set:
//! ```compile_fail
//! let _set: nexus_typed_decision::QuestionSet = serde_json::from_str("{}").unwrap();
//! ```
//!
//! A decision comes only from the strict reader. Its record view is for
//! display (TD-O10):
//! ```
//! use nexus_typed_decision::{decode, Attempt, Reading, INJ_1};
//! let d = decode(&INJ_1, r#"{"instructs_ai":0.1}"#, [0; 32], Attempt::First).unwrap();
//! assert_eq!(d.readings()[0].1, Reading::No);
//! assert_eq!(d.record_view().answers()[0].permille(), 100);
//! ```
//! and there is no way back from a record to a decision:
//! ```compile_fail
//! use nexus_typed_decision::{Decision, RecordView};
//! fn rebuild(view: RecordView) -> Decision { view.into() }
//! ```
//! a saved record cannot be deserialized into one:
//! ```compile_fail
//! let _d: nexus_typed_decision::Decision = serde_json::from_str("{}").unwrap();
//! ```
//! a decision cannot be copied:
//! ```compile_fail
//! fn copy(d: &nexus_typed_decision::Decision) -> nexus_typed_decision::Decision { d.clone() }
//! ```
//! and no lane reads a record:
//! ```compile_fail
//! use nexus_typed_decision::{sentinel, Advice, Check, RecordView};
//! fn gate(view: &RecordView) -> bool { sentinel(Check::Pass, Some(Advice::Decided(view))).eligible() }
//! ```
//!
//! An assessment token is private to the slot that issued it (TD-O12): it
//! cannot be made outside the crate, cloned or copied.
//! ```compile_fail
//! let _t = nexus_typed_decision::AssessmentToken { epoch: 1, binding: [0; 32] };
//! ```
//! ```compile_fail
//! use nexus_typed_decision::AssessmentToken;
//! fn keep(t: &AssessmentToken) -> AssessmentToken { t.clone() }
//! ```
//! ```compile_fail
//! use nexus_typed_decision::AssessmentToken;
//! fn twice(t: AssessmentToken) -> (AssessmentToken, AssessmentToken) { (t, t) }
//! ```

#![forbid(unsafe_code)]

mod budget;
mod decode;
mod hash;
mod ident;
mod lane;
mod projection;
mod read;
mod record;
mod registry;
mod render;
mod slot;

#[cfg(test)]
mod oracles;

pub use budget::{prepare, Coverage, Prepared, Refusal, MAX_CONTENT_BYTES};
pub use decode::{
    decode, Answer, Attempt, Decision, Distribution, ReplyError, Top, MAX_REPLY_BYTES,
};
pub use hash::{registry_hash, set_hash};
pub use ident::{is_ident, Label, QuestionId, SetId, IDENT_MAX_BYTES};
pub use lane::{
    compose, notes_screen, sentinel, verifier, Advice, Check, Disposition, Failure, PreflightCode,
    Reason, SentinelResult, TransportCode, VerifierResult,
};
pub use projection::{
    Element, Facts, Inventory, NoteSource, PageTextV1, Projection, UntrustedTextV1,
};
pub use read::{read, verdict, Verdict};
pub use record::{AnswerView, RecordView};
pub use registry::{
    validate, AnswerMode, Kind, Lane, OptionDisposition, Polarity, Ppm, ProjectionId, Question,
    QuestionSet, ReadPolicy, Reading, RegistryError, INJ_1, REGISTRY, SEN_1, VER_1,
};
pub use render::{
    correction_sentence, render_correction_system, render_schema, render_shape, render_system,
    DecodeCode, RenderError,
};
pub use slot::{
    AckToken, Admission, AssessmentToken, Begin, Installed, Outcome, Refused, Restricted,
    SettleToken, Slot, SlotKind, SlotState, SpentReason,
};
