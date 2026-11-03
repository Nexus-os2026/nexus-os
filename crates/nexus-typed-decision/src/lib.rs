//! Typed decisions, the pure core (design v3, PR T1a).
//!
//! A lane asks a fixed, code-reviewed set of typed questions about
//! backend-owned state. This crate holds what needs no I/O: the
//! code-owned registry (`sen_1`, `ver_1`, `inj_1`), its hashes, the
//! projections, the character profile, the inventory matcher, the
//! escaping and framing, the system text and the prose-free schema
//! rendered from one registry, the fixed correction sentences and the
//! exact byte budget. It has no I/O, no network, no clock and no lock.
//! The result of a typed decision is advice; it never grants anything.
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

#![forbid(unsafe_code)]

mod budget;
mod hash;
mod ident;
mod projection;
mod registry;
mod render;

#[cfg(test)]
mod oracles;

pub use budget::{prepare, Coverage, Prepared, Refusal, MAX_CONTENT_BYTES};
pub use hash::{registry_hash, set_hash};
pub use ident::{is_ident, Label, QuestionId, SetId, IDENT_MAX_BYTES};
pub use projection::{
    Element, Facts, Inventory, NoteSource, PageTextV1, Projection, UntrustedTextV1,
};
pub use registry::{
    validate, AnswerMode, Kind, Lane, OptionDisposition, Polarity, Ppm, ProjectionId, Question,
    QuestionSet, ReadPolicy, Reading, RegistryError, INJ_1, REGISTRY, SEN_1, VER_1,
};
pub use render::{
    correction_sentence, render_correction_system, render_schema, render_shape, render_system,
    DecodeCode, RenderError,
};
