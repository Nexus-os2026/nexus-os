//! # nexus-a2a — A2A (Agent-to-Agent) Protocol for Nexus OS
//!
//! Implements Google's A2A open protocol (now under Linux Foundation) for
//! agent interoperability.  A2A is the complement to MCP — where MCP connects
//! agents to tools, A2A connects agents to other agents.
//!
//! ## Modules
//!
//! - [`types`] — A2A protocol types (re-exports from kernel + extensions)
//! - [`server`] — Skill registry and AgentCard builder for discovery
//! - [`client`] — Batch discovery, ranked agent selection, send-and-wait
//! - [`bridge`] — Routes incoming A2A tasks to the best Nexus OS agent
//! - [`tauri_commands`] — 6 Tauri commands for the desktop frontend

pub mod bridge;
pub mod client;
pub mod server;
pub mod tauri_commands;
pub mod types;

// Convenience re-exports
pub use bridge::{A2aBridge, BridgeError, RoutedTask};
pub use client::{batch_discover, rank_agents_by_tags, send_and_wait, BatchDiscoveryResult};
pub use server::{RegisteredAgent, SkillRegistry};
pub use tauri_commands::A2aState;
pub use types::{
    A2aServerStatus, AgentCard, AgentSkill, Artifact, MessagePart, SkillSummary, TaskStatus,
    A2A_PROTOCOL_VERSION,
};
