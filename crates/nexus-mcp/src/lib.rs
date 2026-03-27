pub mod client;
pub mod server;
pub mod tauri_commands;
pub mod tools;
pub mod transport;
pub mod types;

pub use client::{ExternalServerConfig, McpClient};
pub use server::McpServer;
pub use tauri_commands::McpState;
pub use tools::ToolRegistry;
pub use types::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpContent, McpPrompt, McpResource, McpTool,
    McpToolResult, ServerCapabilities, ServerInfo,
};
