//! Protocol type definitions for A2A (Agent-to-Agent) and MCP (Model Context Protocol).
//!
//! These are pure data types with no async runtime dependency, suitable for use
//! throughout the kernel and by downstream crates.

pub mod a2a;
pub mod bridge;
pub mod mcp;
