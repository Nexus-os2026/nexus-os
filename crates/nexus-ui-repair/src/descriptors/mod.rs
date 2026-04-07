//! Page descriptors for the scout.
//!
//! A `PageDescriptor` describes a single page the scout is expected to
//! exercise: its route, expected elements, critical flows, available
//! fixtures, and (rarely) per-element destructive opt-ins. See v1.1
//! amendment §6.5 Layer 3 for the opt-in semantics.

pub mod page_descriptor;

pub use page_descriptor::{DestructiveOptIn, FixtureKind, FixtureRef, PageDescriptor};
