//! Consciousness Kernel — agent internal psychological state model.
//!
//! Every agent gets an internal state (confidence, fatigue, frustration, etc.)
//! that affects its reasoning and behavior in real-time. The OS also reads
//! user behavior patterns and adapts responses accordingly.

pub mod empathy;
pub mod integration;
pub mod modifiers;
pub mod state;
pub mod transitions;

pub use empathy::{ResponseAdaptation, UserBehaviorState, UserInputEvent, UserMood};
pub use integration::ConsciousnessEngine;
pub use modifiers::ConsciousnessModifier;
pub use state::{ConsciousnessState, StateSnapshot, TaskContext};
