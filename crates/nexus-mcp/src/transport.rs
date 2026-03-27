use std::io::{BufRead, BufReader, Write};

use crate::server::McpServer;

/// Run the MCP server over stdio (stdin/stdout).
/// Each line of stdin is a JSON-RPC request; each line of stdout is a response.
pub fn run_stdio(server: &McpServer) -> Result<(), String> {
    let stdin = std::io::stdin();
    let reader = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();

    for line in reader.lines() {
        let line = line.map_err(|e| format!("stdin read error: {e}"))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let response = server.handle_raw(trimmed);
        writeln!(writer, "{response}").map_err(|e| format!("stdout write error: {e}"))?;
        writer
            .flush()
            .map_err(|e| format!("stdout flush error: {e}"))?;
    }

    Ok(())
}

/// Run the MCP server over HTTP (single-shot request/response).
/// Returns the JSON-RPC response for a given request body.
pub fn handle_http_request(server: &McpServer, body: &str) -> String {
    server.handle_raw(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_request_handler() {
        let server = McpServer::new();
        let response = handle_http_request(
            &server,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#,
        );
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert!(parsed["result"]["tools"].is_array());
    }

    #[test]
    fn test_http_malformed_request() {
        let server = McpServer::new();
        let response = handle_http_request(&server, "not json");
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], -32700);
    }
}
