//! Experience Layer — zero-code, intent-driven project building for everyone.
//!
//! Provides conversational project creation, live preview, natural-language
//! remixing, business problem solving, marketplace publishing, and a
//! teach-me mentor mode.  All user-facing text avoids technical jargon.

pub mod conversational_builder;
pub mod live_preview;
pub mod marketplace_publish;
pub mod problem_solver;
pub mod remix;
pub mod teach_mode;

pub use conversational_builder::{
    BudgetRange, BuilderResponse, BuilderState, ConversationalBuilder, Requirements,
};
pub use live_preview::{LivePreviewEngine, PreviewFrame};
pub use marketplace_publish::{MarketplaceListing, MarketplacePublisher, Pricing};
pub use problem_solver::{ProblemAnalysis, ProblemSolver, ProposedSolution, UserProfile};
pub use remix::{ChangeClassification, RemixEngine, RemixResult};
pub use teach_mode::{TeachMode, TeachStep};
