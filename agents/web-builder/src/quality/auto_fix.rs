//! Auto-fix — apply deterministic fixes to HTML and tokens.
//!
//! Fix types:
//! - MetaFix: add/modify meta tags in <head>
//! - AttributeFix: add/modify attributes on elements
//! - TokenFix: update CSS token values
//!
//! Safety: auto-fix NEVER removes content. Only adds/modifies attributes,
//! meta tags, and token values.

use super::AutoFix;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoFixResult {
    pub fixed_html: String,
    pub fixes_applied: Vec<String>,
    pub fixes_failed: Vec<String>,
}

/// Apply a list of auto-fixes to HTML. Returns the fixed HTML and a summary.
pub fn apply_auto_fixes(html: &str, fixes: &[AutoFix]) -> AutoFixResult {
    let mut result_html = html.to_string();
    let mut applied = Vec::new();
    let mut failed = Vec::new();

    for fix in fixes {
        match fix {
            AutoFix::MetaFix {
                name,
                content,
                description,
            } => {
                if apply_meta_fix(&mut result_html, name, content) {
                    applied.push(description.clone());
                } else {
                    failed.push(format!("Could not apply: {description}"));
                }
            }
            AutoFix::AttributeFix {
                selector: _,
                attribute,
                value,
                description,
            } => {
                if apply_attribute_fix(&mut result_html, attribute, value, fix) {
                    applied.push(description.clone());
                } else {
                    failed.push(format!("Could not apply: {description}"));
                }
            }
            AutoFix::TokenFix {
                token_name,
                value,
                description,
            } => {
                if apply_token_fix(&mut result_html, token_name, value) {
                    applied.push(description.clone());
                } else {
                    failed.push(format!("Could not apply: {description}"));
                }
            }
            AutoFix::ContentFix {
                slot_name: _,
                section_id: _,
                suggested_text: _,
                description,
            } => {
                // Content fixes are advisory — they require user action to replace
                // slot content via the content payload pipeline, not raw HTML surgery.
                // Record as applied (the suggestion is surfaced in the UI).
                applied.push(description.clone());
            }
        }
    }

    AutoFixResult {
        fixed_html: result_html,
        fixes_applied: applied,
        fixes_failed: failed,
    }
}

/// Insert or update a meta tag in <head>.
fn apply_meta_fix(html: &mut String, name: &str, content: &str) -> bool {
    // Special handling for charset
    if name == "charset" {
        if !html.to_lowercase().contains("meta charset") {
            if let Some(pos) = html.to_lowercase().find("<head>") {
                let insert_pos = pos + 6;
                html.insert_str(insert_pos, &format!("\n<meta charset=\"{content}\">"));
                return true;
            }
        }
        return false;
    }

    // Special handling for title
    if name == "title" {
        if !html.to_lowercase().contains("<title>") {
            if let Some(pos) = html.to_lowercase().find("</head>") {
                html.insert_str(pos, &format!("<title>{content}</title>\n"));
                return true;
            }
        }
        return false;
    }

    // Special handling for Content-Security-Policy (http-equiv)
    if name == "Content-Security-Policy" {
        if !html.to_lowercase().contains("content-security-policy") {
            if let Some(pos) = html.to_lowercase().find("<head>") {
                let insert_pos = pos + 6;
                html.insert_str(
                    insert_pos,
                    &format!(
                        "\n<meta http-equiv=\"Content-Security-Policy\" content=\"{content}\">"
                    ),
                );
                return true;
            }
        }
        return false;
    }

    // Standard name= meta tag
    let lower = html.to_lowercase();
    let search = format!("name=\"{name}\"");
    if lower.contains(&search) {
        // Update existing meta tag content
        // Find the meta tag and update its content attribute
        if let Some(meta_pos) = lower.find(&search) {
            if let Some(content_pos) = lower[meta_pos..].find("content=\"") {
                let abs_content_start = meta_pos + content_pos + 9;
                if let Some(content_end) = html[abs_content_start..].find('"') {
                    let abs_content_end = abs_content_start + content_end;
                    html.replace_range(abs_content_start..abs_content_end, content);
                    return true;
                }
            }
        }
        return false;
    }

    // Check for OG property tags
    if name.starts_with("og:") {
        if let Some(pos) = html.to_lowercase().find("</head>") {
            html.insert_str(
                pos,
                &format!("<meta property=\"{name}\" content=\"{content}\">\n"),
            );
            return true;
        }
        return false;
    }

    // Insert new meta tag before </head>
    if let Some(pos) = html.to_lowercase().find("</head>") {
        html.insert_str(
            pos,
            &format!("<meta name=\"{name}\" content=\"{content}\">\n"),
        );
        return true;
    }

    false
}

/// Add an attribute to an element identified by the fix context.
fn apply_attribute_fix(html: &mut String, attribute: &str, value: &str, fix: &AutoFix) -> bool {
    let AutoFix::AttributeFix { selector, .. } = fix else {
        return false;
    };

    // Handle html lang attribute
    if selector == "html" && attribute == "lang" {
        let lower = html.to_lowercase();
        if let Some(pos) = lower.find("<html") {
            if let Some(close) = html[pos..].find('>') {
                let tag_end = pos + close;
                // Check if lang already exists
                let tag = &lower[pos..tag_end];
                if !tag.contains("lang=") {
                    html.insert_str(tag_end, &format!(" {attribute}=\"{value}\""));
                    return true;
                }
            }
        }
        return false;
    }

    // Handle img alt text
    if selector.starts_with("img[src=") && attribute == "alt" {
        // Extract src from selector
        let src = selector
            .trim_start_matches("img[src=\"")
            .trim_end_matches("\"]");
        if let Some(pos) = html.find(src) {
            // Find the img tag containing this src
            // Walk backwards to find <img
            let before = &html[..pos];
            if let Some(img_pos) = before.rfind("<img") {
                if let Some(close) = html[img_pos..].find('>') {
                    let tag_end = img_pos + close;
                    let tag = &html[img_pos..tag_end];
                    if !tag.to_lowercase().contains("alt=") {
                        html.insert_str(tag_end, &format!(" alt=\"{value}\""));
                        return true;
                    }
                }
            }
        }
        return false;
    }

    // Handle target="_blank" rel attribute
    if attribute == "rel" && selector.contains("target=\"_blank\"") {
        let lower = html.to_lowercase();
        // Find links with target="_blank" missing rel
        if let Some(pos) = lower.find("target=\"_blank\"") {
            // Check if rel already present in this tag
            let before_tag = &lower[..pos];
            if let Some(tag_start) = before_tag.rfind('<') {
                let tag = &lower[tag_start..pos + 15];
                if !tag.contains("rel=") {
                    // Insert rel before target
                    html.insert_str(pos, &format!("rel=\"{value}\" "));
                    return true;
                }
            }
        }
        return false;
    }

    // Generic: try to find the element and add the attribute
    // For boolean attributes (defer, etc.) with empty value
    if value.is_empty() {
        // Try to find script src and add defer
        if attribute == "defer" && selector.starts_with("script[src=") {
            let src = selector
                .trim_start_matches("script[src=\"")
                .trim_end_matches("\"]");
            if let Some(pos) = html.find(src) {
                let before = &html[..pos];
                if let Some(script_pos) = before.rfind("<script") {
                    if let Some(close) = html[script_pos..].find('>') {
                        let tag_end = script_pos + close;
                        html.insert_str(tag_end, " defer");
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Update a CSS custom property value in :root.
fn apply_token_fix(html: &mut String, token_name: &str, value: &str) -> bool {
    let prop = format!("--{token_name}:");
    if let Some(pos) = html.find(&prop) {
        // Find the end of this property value (;)
        if let Some(semi) = html[pos..].find(';') {
            let abs_semi = pos + semi;
            let replace_start = pos + prop.len();
            html.replace_range(replace_start..abs_semi, &format!(" {value}"));
            return true;
        }
    }
    false
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_meta_fix_adds_tag() {
        let html =
            "<!DOCTYPE html><html><head><title>T</title></head><body></body></html>".to_string();
        let fixes = vec![AutoFix::MetaFix {
            name: "description".into(),
            content: "My great page".into(),
            description: "Add meta description".into(),
        }];
        let result = apply_auto_fixes(&html, &fixes);
        assert!(result.fixed_html.contains("name=\"description\""));
        assert!(result.fixed_html.contains("My great page"));
        assert_eq!(result.fixes_applied.len(), 1);
        assert!(result.fixes_failed.is_empty());
    }

    #[test]
    fn test_apply_meta_fix_adds_charset() {
        let fixes = vec![AutoFix::MetaFix {
            name: "charset".into(),
            content: "UTF-8".into(),
            description: "Add charset".into(),
        }];
        let html = "<!DOCTYPE html><html><head><title>T</title></head><body></body></html>";
        let result = apply_auto_fixes(html, &fixes);
        assert!(result.fixed_html.contains("charset=\"UTF-8\""));
    }

    #[test]
    fn test_apply_attribute_fix_lang() {
        let fixes = vec![AutoFix::AttributeFix {
            selector: "html".into(),
            attribute: "lang".into(),
            value: "en".into(),
            description: "Add lang".into(),
        }];
        let html = "<!DOCTYPE html><html><head></head><body></body></html>";
        let result = apply_auto_fixes(html, &fixes);
        assert!(result.fixed_html.contains("lang=\"en\""));
    }

    #[test]
    fn test_apply_token_fix() {
        let fixes = vec![AutoFix::TokenFix {
            token_name: "color-primary".into(),
            value: "#ff0000".into(),
            description: "Fix primary color".into(),
        }];
        let html = "<style>:root { --color-primary: #6366f1; }</style>";
        let result = apply_auto_fixes(html, &fixes);
        assert!(result.fixed_html.contains("--color-primary: #ff0000;"));
        assert_eq!(result.fixes_applied.len(), 1);
    }

    #[test]
    fn test_apply_multiple_fixes() {
        let fixes = vec![
            AutoFix::MetaFix {
                name: "viewport".into(),
                content: "width=device-width, initial-scale=1".into(),
                description: "Add viewport".into(),
            },
            AutoFix::MetaFix {
                name: "description".into(),
                content: "Hello world".into(),
                description: "Add description".into(),
            },
        ];
        let html = "<!DOCTYPE html><html><head><title>T</title></head><body></body></html>";
        let result = apply_auto_fixes(html, &fixes);
        assert!(result.fixed_html.contains("viewport"));
        assert!(result.fixed_html.contains("Hello world"));
        assert_eq!(result.fixes_applied.len(), 2);
    }

    #[test]
    fn test_fix_never_removes_content() {
        let original = "<!DOCTYPE html><html><head><title>Keep me</title></head><body><h1>Important</h1><p>Content here</p></body></html>";
        let fixes = vec![
            AutoFix::MetaFix {
                name: "description".into(),
                content: "New desc".into(),
                description: "Add desc".into(),
            },
            AutoFix::AttributeFix {
                selector: "html".into(),
                attribute: "lang".into(),
                value: "en".into(),
                description: "Add lang".into(),
            },
        ];
        let result = apply_auto_fixes(original, &fixes);
        // All original content preserved
        assert!(result.fixed_html.contains("Keep me"));
        assert!(result.fixed_html.contains("Important"));
        assert!(result.fixed_html.contains("Content here"));
        assert!(result.fixed_html.len() >= original.len());
    }

    #[test]
    fn test_apply_csp_meta() {
        let fixes = vec![AutoFix::MetaFix {
            name: "Content-Security-Policy".into(),
            content: "default-src 'self'".into(),
            description: "Add CSP".into(),
        }];
        let html = "<!DOCTYPE html><html><head><title>T</title></head><body></body></html>";
        let result = apply_auto_fixes(html, &fixes);
        assert!(result.fixed_html.contains("Content-Security-Policy"));
    }

    #[test]
    fn test_apply_og_tags() {
        let fixes = vec![AutoFix::MetaFix {
            name: "og:title".into(),
            content: "My Site".into(),
            description: "Add OG title".into(),
        }];
        let html = "<!DOCTYPE html><html><head><title>T</title></head><body></body></html>";
        let result = apply_auto_fixes(html, &fixes);
        assert!(result.fixed_html.contains("og:title"));
        assert!(result.fixed_html.contains("My Site"));
    }
}
