//! DESIGN.md Parser — parse structured design tokens from DESIGN.md format.
//!
//! Handles both YAML-style key-value blocks and markdown list-style tokens.

use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("empty content")]
    EmptyContent,
    #[error("no tokens found")]
    NoTokensFound,
}

/// Parsed design tokens from a DESIGN.md file.
#[derive(Debug, Clone, Default)]
pub struct DesignMd {
    pub colors: HashMap<String, String>,
    pub fonts: HashMap<String, String>,
    pub spacing: HashMap<String, String>,
    pub radii: HashMap<String, String>,
    pub raw_content: String,
}

/// Parse a DESIGN.md content string into structured tokens.
pub fn parse_design_md(content: &str) -> Result<DesignMd, ParseError> {
    if content.trim().is_empty() {
        return Err(ParseError::EmptyContent);
    }

    let mut dm = DesignMd {
        raw_content: content.to_string(),
        ..Default::default()
    };

    let mut current_section = Section::None;

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect section headers
        if let Some(section) = detect_section(trimmed) {
            current_section = section;
            continue;
        }

        // Parse key-value pairs within sections
        if let Some((key, value)) = parse_key_value(trimmed) {
            match current_section {
                Section::Colors => {
                    dm.colors.insert(key, value);
                }
                Section::Typography | Section::Fonts => {
                    dm.fonts.insert(key, value);
                }
                Section::Spacing => {
                    dm.spacing.insert(key, value);
                }
                Section::Radii | Section::BorderRadius => {
                    dm.radii.insert(key, value);
                }
                Section::None => {
                    // Try to infer section from key name
                    let lower_key = key.to_lowercase();
                    if lower_key.contains("color") || is_hex_color(&value) {
                        dm.colors.insert(key, value);
                    } else if lower_key.contains("font") {
                        dm.fonts.insert(key, value);
                    } else if lower_key.contains("spacing") || lower_key.contains("gap") {
                        dm.spacing.insert(key, value);
                    } else if lower_key.contains("radius") {
                        dm.radii.insert(key, value);
                    }
                }
            }
        }
    }

    // Also try parsing markdown tables
    parse_markdown_tables(content, &mut dm);

    if dm.colors.is_empty() && dm.fonts.is_empty() && dm.spacing.is_empty() && dm.radii.is_empty() {
        return Err(ParseError::NoTokensFound);
    }

    Ok(dm)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Section {
    None,
    Colors,
    Typography,
    Fonts,
    Spacing,
    Radii,
    BorderRadius,
}

fn detect_section(line: &str) -> Option<Section> {
    let lower = line.to_lowercase();
    // Match markdown headers: # Colors, ## Typography, etc.
    if !lower.starts_with('#') {
        return None;
    }
    let heading = lower.trim_start_matches('#').trim();
    match heading {
        "colors" | "colour" | "color palette" | "color tokens" | "palette" => Some(Section::Colors),
        "typography" | "type" | "type scale" | "fonts" | "font" => Some(Section::Typography),
        "spacing" | "space" | "spacing scale" => Some(Section::Spacing),
        "radii" | "border radius" | "border-radius" | "corners" => Some(Section::Radii),
        _ => {
            if heading.contains("color") {
                Some(Section::Colors)
            } else if heading.contains("font") || heading.contains("typo") {
                Some(Section::Fonts)
            } else if heading.contains("spac") {
                Some(Section::Spacing)
            } else if heading.contains("radi") || heading.contains("corner") {
                Some(Section::BorderRadius)
            } else {
                None
            }
        }
    }
}

/// Parse a key-value pair from a line.
/// Supports:
/// - `key: value`
/// - `- key: value`
/// - `| key | value |` (table row)
/// - `key = value`
fn parse_key_value(line: &str) -> Option<(String, String)> {
    let trimmed = line
        .trim()
        .trim_start_matches('-')
        .trim_start_matches('*')
        .trim();

    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("---") {
        return None;
    }

    // Table row: | key | value |
    if trimmed.starts_with('|') && trimmed.ends_with('|') {
        let parts: Vec<&str> = trimmed
            .split('|')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if parts.len() >= 2 {
            let key = parts[0].trim().to_string();
            let value = parts[1].trim().to_string();
            if !key.is_empty() && !value.is_empty() && !key.contains("---") {
                return Some((key, value));
            }
        }
        return None;
    }

    // key: value
    if let Some(colon_pos) = trimmed.find(':') {
        let key = trimmed[..colon_pos].trim().to_string();
        let value = trimmed[colon_pos + 1..].trim().to_string();
        if !key.is_empty() && !value.is_empty() {
            return Some((key, value));
        }
    }

    // key = value
    if let Some(eq_pos) = trimmed.find('=') {
        let key = trimmed[..eq_pos].trim().to_string();
        let value = trimmed[eq_pos + 1..].trim().to_string();
        if !key.is_empty() && !value.is_empty() {
            return Some((key, value));
        }
    }

    None
}

/// Parse markdown tables for tokens.
fn parse_markdown_tables(content: &str, dm: &mut DesignMd) {
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    let mut current_section = Section::None;

    while i < lines.len() {
        let line = lines[i].trim();

        // Check for section header
        if let Some(section) = detect_section(line) {
            current_section = section;
            i += 1;
            continue;
        }

        // Check for table header row (has |)
        if line.starts_with('|') && i + 2 < lines.len() {
            let separator = lines[i + 1].trim();
            if separator.contains("---") {
                // Table found — parse remaining rows
                let mut j = i + 2;
                while j < lines.len() {
                    let row = lines[j].trim();
                    if !row.starts_with('|') {
                        break;
                    }
                    if let Some((key, value)) = parse_key_value(row) {
                        match current_section {
                            Section::Colors => {
                                dm.colors.insert(key, value);
                            }
                            Section::Typography | Section::Fonts => {
                                dm.fonts.insert(key, value);
                            }
                            Section::Spacing => {
                                dm.spacing.insert(key, value);
                            }
                            Section::Radii | Section::BorderRadius => {
                                dm.radii.insert(key, value);
                            }
                            Section::None => {
                                if is_hex_color(&value) {
                                    dm.colors.insert(key, value);
                                }
                            }
                        }
                    }
                    j += 1;
                }
                i = j;
                continue;
            }
        }

        i += 1;
    }
}

fn is_hex_color(s: &str) -> bool {
    let trimmed = s.trim();
    trimmed.starts_with('#')
        && trimmed.len() >= 4
        && trimmed[1..].chars().all(|c| c.is_ascii_hexdigit())
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color_tokens() {
        let content = "# Colors\n- primary: #4f46e5\n- secondary: #7c3aed\n- accent: #06b6d4\n";
        let dm = parse_design_md(content).unwrap();
        assert_eq!(dm.colors.get("primary"), Some(&"#4f46e5".into()));
        assert_eq!(dm.colors.get("secondary"), Some(&"#7c3aed".into()));
        assert_eq!(dm.colors.get("accent"), Some(&"#06b6d4".into()));
    }

    #[test]
    fn test_parse_font_tokens() {
        let content = "# Typography\n- heading: Inter\n- body: Source Sans Pro\n";
        let dm = parse_design_md(content).unwrap();
        assert_eq!(dm.fonts.get("heading"), Some(&"Inter".into()));
        assert_eq!(dm.fonts.get("body"), Some(&"Source Sans Pro".into()));
    }

    #[test]
    fn test_handles_yaml_frontmatter() {
        let content =
            "# Colors\nprimary: #ff0000\nsecondary: #00ff00\n\n# Typography\nheading: Roboto\n";
        let dm = parse_design_md(content).unwrap();
        assert_eq!(dm.colors.get("primary"), Some(&"#ff0000".into()));
        assert_eq!(dm.fonts.get("heading"), Some(&"Roboto".into()));
    }

    #[test]
    fn test_handles_markdown_tables() {
        let content = "# Colors\n| Name | Value |\n|------|-------|\n| primary | #4f46e5 |\n| accent | #06b6d4 |\n";
        let dm = parse_design_md(content).unwrap();
        assert_eq!(dm.colors.get("primary"), Some(&"#4f46e5".into()));
        assert_eq!(dm.colors.get("accent"), Some(&"#06b6d4".into()));
    }

    #[test]
    fn test_handles_empty_content() {
        let result = parse_design_md("");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ParseError::EmptyContent));
    }

    #[test]
    fn test_handles_no_tokens() {
        let result = parse_design_md("# Just a heading\nSome text without tokens.");
        assert!(result.is_err());
    }

    #[test]
    fn test_mixed_sections() {
        let content = r#"
# Colors
- primary: #4f46e5
- background: #f8fafc
- text: #0f172a

# Typography
- heading: Inter
- body: Inter

# Spacing
- sm: 0.5rem
- md: 1rem
- lg: 1.5rem
"#;
        let dm = parse_design_md(content).unwrap();
        assert_eq!(dm.colors.len(), 3);
        assert_eq!(dm.fonts.len(), 2);
        assert_eq!(dm.spacing.len(), 3);
    }
}
