//! Protocol server implementations for Nexus OS.
//!
//! This crate provides the async HTTP gateway and protocol servers:
//! - A2A (Agent-to-Agent) JSON-RPC server
//! - MCP (Model Context Protocol) tool server
//! - JWT-based authentication middleware
//!
//! All protocol endpoints route through kernel governance (capability checks,
//! fuel accounting, audit trail) before executing agent operations.

pub mod http_gateway;

// Re-export core types from kernel for convenience.
pub use nexus_kernel::protocols::a2a;
pub use nexus_kernel::protocols::mcp;
