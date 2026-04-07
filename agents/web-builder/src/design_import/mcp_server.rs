//! MCP Server — tool registration for design import.
//!
//! Registers the `import_design` tool with the Nexus MCP server infrastructure.
//! If the MCP server is not available, the Tauri command path is the fallback.

use serde::{Deserialize, Serialize};

/// MCP tool schema for the import_design tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportDesignInput {
    pub html: String,
    pub css: Option<String>,
    pub design_tokens: Option<String>,
    pub screens: Option<Vec<ScreenInput>>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenInput {
    pub name: String,
    pub html: String,
    pub css: Option<String>,
}

/// MCP tool output for the import_design tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportDesignOutput {
    pub project_id: String,
    pub preview_url: Option<String>,
    pub quality_score: Option<u32>,
    pub sections_detected: usize,
    pub issues: Vec<String>,
}

/// MCP tool definition for registration with the Nexus MCP server.
pub fn import_design_tool_definition() -> serde_json::Value {
    serde_json::json!({
        "name": "import_design",
        "description": "Import an external design (HTML/CSS/DESIGN.md) into a governed Nexus Builder project. Sanitizes all input, extracts design tokens, detects sections, and generates a React project with quality checks.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "html": {
                    "type": "string",
                    "description": "The HTML markup to import"
                },
                "css": {
                    "type": "string",
                    "description": "Associated CSS stylesheet (optional)"
                },
                "design_tokens": {
                    "type": "string",
                    "description": "DESIGN.md content with design tokens (optional, Stitch format)"
                },
                "screens": {
                    "type": "array",
                    "description": "Multiple screens/pages (optional)",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "html": { "type": "string" },
                            "css": { "type": "string" }
                        },
                        "required": ["name", "html"]
                    }
                },
                "source": {
                    "type": "string",
                    "enum": ["stitch", "figma", "paste", "url"],
                    "description": "Import source identifier"
                }
            },
            "required": ["html", "source"]
        }
    })
}

/// MCP resource definitions.
pub fn project_status_resource_definition() -> serde_json::Value {
    serde_json::json!({
        "uri": "project/{id}/status",
        "name": "Project Status",
        "description": "Current build status of a Nexus Builder project",
        "mimeType": "application/json"
    })
}

pub fn project_quality_resource_definition() -> serde_json::Value {
    serde_json::json!({
        "uri": "project/{id}/quality",
        "name": "Quality Report",
        "description": "Quality check results for a Nexus Builder project",
        "mimeType": "application/json"
    })
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition_valid_json() {
        let def = import_design_tool_definition();
        assert_eq!(def["name"], "import_design");
        assert!(def["inputSchema"]["properties"]["html"].is_object());
        assert!(def["inputSchema"]["required"].is_array());
    }

    #[test]
    fn test_resource_definitions() {
        let status = project_status_resource_definition();
        assert!(status["uri"].as_str().unwrap().contains("status"));

        let quality = project_quality_resource_definition();
        assert!(quality["uri"].as_str().unwrap().contains("quality"));
    }

    #[test]
    fn test_import_input_deserializes() {
        let json = serde_json::json!({
            "html": "<div>Hello</div>",
            "css": "div { color: red; }",
            "source": "paste"
        });
        let input: ImportDesignInput = serde_json::from_value(json).unwrap();
        assert_eq!(input.html, "<div>Hello</div>");
        assert_eq!(input.source, "paste");
    }
}
