//! Governed LLM connector with provider abstraction, policy gateway, and prompt-injection defense.

pub mod circuit_breaker;
pub mod defense;
pub mod gateway;
pub mod governance_slm;
pub mod model_registry;
pub mod providers;
pub mod routing;
