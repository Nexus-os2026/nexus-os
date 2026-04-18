//! Six provider wrappers over the existing `nexus-connectors-llm` providers,
//! plus one fresh HuggingFace implementation.
//!
//! No Claude CLI — that is an interactive-only integration and explicitly
//! excluded from the autonomous swarm.

pub mod anthropic;
pub mod codex_cli;
pub mod huggingface;
pub mod ollama;
pub mod openai;
pub mod openrouter;

pub use anthropic::AnthropicProvider;
pub use codex_cli::CodexCliProvider;
pub use huggingface::HuggingFaceProvider;
pub use ollama::OllamaSwarmProvider;
pub use openai::OpenAiSwarmProvider;
pub use openrouter::OpenRouterSwarmProvider;
