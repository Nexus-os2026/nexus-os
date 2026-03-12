//! Governed LLM connector with provider abstraction, policy gateway, and prompt-injection defense.

pub mod chunking;
pub mod circuit_breaker;
pub mod defense;
pub mod gateway;
pub mod governance_slm;
pub mod inference_queue;
pub mod model_hub;
pub mod model_registry;
pub mod nexus_link;
pub mod providers;
pub mod rag;
pub mod routing;
pub mod vector_store;
pub mod whisper;
