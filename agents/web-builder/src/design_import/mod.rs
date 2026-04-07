//! Design Import — sanitized pipeline for importing external designs.
//!
//! Three import sources converge into one pipeline:
//! - Stitch MCP (HTML + CSS + DESIGN.md)
//! - Raw HTML/CSS paste
//! - Figma HTML export
//!
//! Pipeline: Receive → Sanitize → Extract Tokens → Detect Sections → Wrap Components → Quality Check
//!
//! Cost: $0 (pure parsing + heuristics, no LLM calls)

pub mod component_wrapper;
pub mod design_md;
pub mod mcp_server;
pub mod sanitizer;
pub mod section_detector;
pub mod token_extractor;

use crate::react_gen::ReactProject;
use crate::tokens::TokenSet;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("sanitization failed: {0}")]
    SanitizationFailed(String),
    #[error("token extraction failed: {0}")]
    TokenExtractionFailed(String),
    #[error("section detection failed: {0}")]
    SectionDetectionFailed(String),
    #[error("component generation failed: {0}")]
    ComponentGenFailed(String),
    #[error("empty input: {0}")]
    EmptyInput(String),
}

// ─── Types ──────────────────────────────────────────────────────────────────

/// Import source identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportSource {
    Stitch,
    Figma,
    Paste,
    Url,
}

/// Result of the import pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub project_id: String,
    pub sections_detected: usize,
    pub tokens_extracted: usize,
    pub sanitized_elements_removed: Vec<String>,
    pub quality_score: Option<u32>,
    pub warnings: Vec<String>,
}

/// Complete import output including the generated project.
#[derive(Debug)]
pub struct ImportOutput {
    pub result: ImportResult,
    pub project: ReactProject,
    pub token_set: TokenSet,
    pub html: String,
}

// ─── Orchestrator ───────────────────────────────────────────────────────────

/// Run the full design import pipeline.
///
/// 1. Sanitize HTML/CSS (security-critical)
/// 2. Extract design tokens
/// 3. Detect page sections
/// 4. Generate React components from sections
/// 5. Return governed ReactProject
pub fn import_design(
    project_id: &str,
    raw_html: &str,
    raw_css: Option<&str>,
    design_md_content: Option<&str>,
    _source: ImportSource,
) -> Result<ImportOutput, ImportError> {
    if raw_html.trim().is_empty() {
        return Err(ImportError::EmptyInput("HTML content is required".into()));
    }

    // Step 1: Sanitize
    let sanitized = sanitizer::sanitize_html(raw_html);
    let css = raw_css.unwrap_or("");
    let clean_css = sanitizer::sanitize_css(css);

    // Step 2: Parse DESIGN.md if provided
    let design_tokens =
        design_md_content.and_then(|content| design_md::parse_design_md(content).ok());

    // Step 3: Extract tokens
    let extracted =
        token_extractor::extract_tokens(&sanitized.clean_html, &clean_css, design_tokens.as_ref());
    let foundation = token_extractor::map_to_foundation_tokens(&extracted);
    let token_set = TokenSet {
        foundation,
        ..Default::default()
    };

    // Step 4: Detect sections
    let sections = section_detector::detect_sections(&sanitized.clean_html);

    // Step 5: Generate React components
    let project_name = format!("import-{project_id}");
    let project =
        component_wrapper::generate_components_from_import(&sections, &token_set, &project_name)?;

    // Assemble result
    let result = ImportResult {
        project_id: project_id.to_string(),
        sections_detected: sections.len(),
        tokens_extracted: extracted.colors.len()
            + extracted.fonts.len()
            + extracted.spacing_scale.len(),
        sanitized_elements_removed: sanitized.removed_elements,
        quality_score: None, // caller runs quality checks separately
        warnings: sanitized.warnings,
    };

    Ok(ImportOutput {
        result,
        project,
        token_set,
        html: sanitized.clean_html,
    })
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_import_pipeline() {
        let html = r##"
        <html><body>
            <header><nav><a href="/">Home</a></nav></header>
            <section><h1>Welcome</h1><p>Hello world</p><a href="#start">Get Started</a></section>
            <section><h2>Features</h2><div class="grid"><div>Fast</div><div>Secure</div></div></section>
            <footer><p>Copyright 2026</p></footer>
        </body></html>"##;

        let css = "body { color: #1a1a2e; background: #f0f0f5; font-family: Inter, sans-serif; }
            h1 { color: #4f46e5; } .grid { display: grid; }";

        let result = import_design("test-001", html, Some(css), None, ImportSource::Paste);
        assert!(result.is_ok(), "import failed: {result:?}");
        let output = result.unwrap();
        assert!(output.result.sections_detected > 0);
        assert!(!output.project.files.is_empty());
        assert!(!output.html.is_empty());
    }

    #[test]
    fn test_import_strips_scripts() {
        let html = r#"<div><script>alert('xss')</script><p>Safe content</p></div>"#;
        let result = import_design("test-002", html, None, None, ImportSource::Paste);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.html.contains("<script>"));
        assert!(output.html.contains("Safe content"));
    }

    #[test]
    fn test_import_empty_html_error() {
        let result = import_design("test-003", "", None, None, ImportSource::Paste);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ImportError::EmptyInput(_)));
    }

    #[test]
    fn test_import_preserves_visual_structure() {
        let html = r#"
        <header><nav>Nav</nav></header>
        <section id="hero"><h1>Hero</h1></section>
        <section id="features"><h2>Features</h2></section>
        <footer>Footer</footer>"#;

        let output = import_design("test-004", html, None, None, ImportSource::Paste).unwrap();
        // Should detect at least header, 2 sections, footer
        assert!(
            output.result.sections_detected >= 3,
            "expected >= 3 sections, got {}",
            output.result.sections_detected
        );
    }

    #[test]
    fn test_stitch_design_md_flow() {
        let html = "<section><h1>Hello</h1></section>";
        let design_md = r#"
# Colors
- primary: #4f46e5
- secondary: #7c3aed
- accent: #06b6d4
- background: #f8fafc
- text: #0f172a

# Typography
- heading: Inter
- body: Inter
"#;

        let output = import_design(
            "test-005",
            html,
            None,
            Some(design_md),
            ImportSource::Stitch,
        )
        .unwrap();

        // Tokens should be extracted from DESIGN.md
        assert!(output.result.tokens_extracted > 0);
        // Foundation tokens should reflect DESIGN.md values
        assert_eq!(output.token_set.foundation.color_primary, "#4f46e5");
    }

    #[test]
    fn test_import_and_quality_check_compatible() {
        let html = r#"<section><h1>Test</h1><p>Content</p></section>"#;
        let output = import_design("test-006", html, None, None, ImportSource::Paste).unwrap();
        // The generated project should have files
        assert!(!output.project.files.is_empty());
        // HTML output should be valid for quality checking
        assert!(!output.html.is_empty());
    }
}
