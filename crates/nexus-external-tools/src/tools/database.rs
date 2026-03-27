use crate::adapter::{HttpRequest, ToolError};
use std::collections::HashMap;

pub struct DatabaseTool;

impl DatabaseTool {
    pub fn build_request(
        params: &serde_json::Value,
        _auth_token: &str,
    ) -> Result<HttpRequest, ToolError> {
        let query = params
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParameters("query required".into()))?;
        let _database = params
            .get("database")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParameters("database required".into()))?;

        let read_only = params
            .get("read_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        if read_only {
            let upper = query.trim().to_uppercase();
            if !upper.starts_with("SELECT")
                && !upper.starts_with("EXPLAIN")
                && !upper.starts_with("SHOW")
                && !upper.starts_with("DESCRIBE")
            {
                return Err(ToolError::GovernanceDenied(
                    "Read-only mode: only SELECT/EXPLAIN/SHOW/DESCRIBE allowed".into(),
                ));
            }
        }

        // Database queries are executed via subprocess, not HTTP.
        // We encode the query as a pseudo-request for the execution engine.
        Ok(HttpRequest {
            url: "local://database".into(),
            method: "QUERY".into(),
            headers: HashMap::new(),
            body: Some(serde_json::json!({"query": query, "read_only": read_only}).to_string()),
            timeout_secs: Some(30),
        })
    }
}
