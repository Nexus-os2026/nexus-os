//! Token Extractor — extract design tokens from HTML/CSS/DESIGN.md.
//!
//! Priority: DESIGN.md > CSS custom properties > color frequency analysis.

use crate::design_import::design_md::DesignMd;
use crate::tokens::FoundationTokens;
use regex::Regex;
use std::collections::HashMap;

/// Extracted design tokens from an import source.
#[derive(Debug, Clone, Default)]
pub struct ExtractedTokens {
    pub colors: HashMap<String, String>,
    pub fonts: Vec<String>,
    pub spacing_scale: Vec<String>,
    pub border_radii: Vec<String>,
    pub confidence: f32,
}

/// Extract tokens from HTML, CSS, and optionally DESIGN.md.
pub fn extract_tokens(_html: &str, css: &str, design_md: Option<&DesignMd>) -> ExtractedTokens {
    // Priority 1: DESIGN.md tokens
    if let Some(dm) = design_md {
        return extract_from_design_md(dm);
    }

    // Priority 2: CSS custom properties + frequency analysis
    extract_from_css(css)
}

/// Extract tokens from DESIGN.md (highest confidence).
fn extract_from_design_md(dm: &DesignMd) -> ExtractedTokens {
    let mut colors = HashMap::new();

    for (name, value) in &dm.colors {
        let token_name = map_color_name(name);
        colors.insert(token_name, value.clone());
    }

    let fonts: Vec<String> = dm.fonts.values().cloned().collect();

    let spacing_scale: Vec<String> = dm.spacing.values().cloned().collect();

    let border_radii: Vec<String> = dm.radii.values().cloned().collect();

    ExtractedTokens {
        colors,
        fonts,
        spacing_scale,
        border_radii,
        confidence: 0.9, // high confidence from structured source
    }
}

/// Map common color names to our token names.
fn map_color_name(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("primary") {
        "color-primary".into()
    } else if lower.contains("secondary") {
        "color-secondary".into()
    } else if lower.contains("accent") {
        "color-accent".into()
    } else if lower.contains("background") || lower == "bg" {
        "color-bg".into()
    } else if lower.contains("text") && !lower.contains("secondary") {
        "color-text".into()
    } else if lower.contains("text") && lower.contains("secondary") {
        "color-text-secondary".into()
    } else if lower.contains("border") {
        "color-border".into()
    } else {
        format!("color-{}", lower.replace(' ', "-"))
    }
}

/// Extract tokens from CSS by analyzing declarations (lower confidence).
fn extract_from_css(css: &str) -> ExtractedTokens {
    let mut colors = HashMap::new();
    let mut fonts = Vec::new();
    let mut spacing = Vec::new();
    let mut radii = Vec::new();

    // Extract hex colors
    let hex_re =
        Regex::new(r"#([0-9a-fA-F]{3,8})\b").unwrap_or_else(|_| Regex::new(r"$^").unwrap());
    let mut color_freq: HashMap<String, usize> = HashMap::new();
    for cap in hex_re.captures_iter(css) {
        let color = format!("#{}", &cap[1]);
        *color_freq.entry(color).or_insert(0) += 1;
    }

    // Sort by frequency, map top colors to tokens
    let mut sorted_colors: Vec<(String, usize)> = color_freq.into_iter().collect();
    sorted_colors.sort_by(|a, b| b.1.cmp(&a.1));

    // Heuristic mapping by context
    for (i, (color, _)) in sorted_colors.iter().take(8).enumerate() {
        let token = match i {
            0 => "color-primary",
            1 => "color-text",
            2 => "color-bg",
            3 => "color-secondary",
            4 => "color-accent",
            5 => "color-text-secondary",
            6 => "color-bg-secondary",
            7 => "color-border",
            _ => continue,
        };
        colors.insert(token.into(), color.clone());
    }

    // Extract font families
    let font_re =
        Regex::new(r"font-family:\s*([^;]+)").unwrap_or_else(|_| Regex::new(r"$^").unwrap());
    for cap in font_re.captures_iter(css) {
        let font = cap[1]
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_string();
        if !fonts.contains(&font) {
            fonts.push(font);
        }
    }

    // Extract spacing values (margin, padding, gap)
    let spacing_re = Regex::new(r"(?:margin|padding|gap):\s*([^;]+)")
        .unwrap_or_else(|_| Regex::new(r"$^").unwrap());
    for cap in spacing_re.captures_iter(css) {
        let val = cap[1].trim().to_string();
        if !spacing.contains(&val) {
            spacing.push(val);
        }
    }

    // Extract border-radius values
    let radius_re =
        Regex::new(r"border-radius:\s*([^;]+)").unwrap_or_else(|_| Regex::new(r"$^").unwrap());
    for cap in radius_re.captures_iter(css) {
        let val = cap[1].trim().to_string();
        if !radii.contains(&val) {
            radii.push(val);
        }
    }

    let confidence = if colors.is_empty() { 0.1 } else { 0.5 };
    ExtractedTokens {
        colors,
        fonts,
        spacing_scale: spacing,
        border_radii: radii,
        confidence,
    }
}

/// Map extracted tokens to FoundationTokens.
pub fn map_to_foundation_tokens(extracted: &ExtractedTokens) -> FoundationTokens {
    let mut ft = FoundationTokens::default();

    // Apply extracted colors
    for (name, value) in &extracted.colors {
        match name.as_str() {
            "color-primary" => ft.color_primary = value.clone(),
            "color-secondary" => ft.color_secondary = value.clone(),
            "color-accent" => ft.color_accent = value.clone(),
            "color-bg" => ft.color_bg = value.clone(),
            "color-bg-secondary" => ft.color_bg_secondary = value.clone(),
            "color-text" => ft.color_text = value.clone(),
            "color-text-secondary" => ft.color_text_secondary = value.clone(),
            "color-border" => ft.color_border = value.clone(),
            _ => {}
        }
    }

    // Apply extracted fonts
    if let Some(first_font) = extracted.fonts.first() {
        ft.font_heading = format!("'{first_font}', system-ui, sans-serif");
        ft.font_body = format!("'{first_font}', system-ui, sans-serif");
    }
    if let Some(second_font) = extracted.fonts.get(1) {
        ft.font_body = format!("'{second_font}', system-ui, sans-serif");
    }

    ft
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_colors_from_css() {
        let css = "body { color: #1a1a2e; background: #f0f0f5; }
            h1 { color: #4f46e5; }
            .btn { background: #06b6d4; }";
        let extracted = extract_from_css(css);
        assert!(!extracted.colors.is_empty(), "should detect colors");
        assert!(extracted.confidence > 0.0);
    }

    #[test]
    fn test_extract_fonts_from_css() {
        let css = "body { font-family: 'Inter', sans-serif; }
            h1 { font-family: 'Playfair Display', serif; }";
        let extracted = extract_from_css(css);
        assert!(!extracted.fonts.is_empty(), "should detect fonts");
        assert!(extracted.fonts.iter().any(|f| f.contains("Inter")));
    }

    #[test]
    fn test_design_md_overrides_css_extraction() {
        let dm = DesignMd {
            colors: HashMap::from([
                ("primary".into(), "#ff0000".into()),
                ("text".into(), "#000000".into()),
            ]),
            fonts: HashMap::from([("heading".into(), "CustomFont".into())]),
            spacing: HashMap::new(),
            radii: HashMap::new(),
            raw_content: String::new(),
        };

        let extracted = extract_tokens("", "body { color: #333; }", Some(&dm));
        assert_eq!(extracted.confidence, 0.9);
        assert_eq!(
            extracted.colors.get("color-primary"),
            Some(&"#ff0000".into())
        );
    }

    #[test]
    fn test_maps_to_foundation_tokens() {
        let extracted = ExtractedTokens {
            colors: HashMap::from([
                ("color-primary".into(), "#4f46e5".into()),
                ("color-bg".into(), "#ffffff".into()),
                ("color-text".into(), "#1a1a2e".into()),
            ]),
            fonts: vec!["Inter".into()],
            spacing_scale: vec![],
            border_radii: vec![],
            confidence: 0.8,
        };

        let ft = map_to_foundation_tokens(&extracted);
        assert_eq!(ft.color_primary, "#4f46e5");
        assert_eq!(ft.color_bg, "#ffffff");
        assert_eq!(ft.color_text, "#1a1a2e");
        assert!(ft.font_heading.contains("Inter"));
    }

    #[test]
    fn test_handles_empty_css() {
        let extracted = extract_from_css("");
        assert!(extracted.colors.is_empty());
        assert!(extracted.fonts.is_empty());
        assert_eq!(extracted.confidence, 0.1);
    }

    #[test]
    fn test_handles_no_design_md() {
        let extracted = extract_tokens("", "body { color: #333; }", None);
        // Should still work with CSS-only extraction
        assert!(extracted.confidence > 0.0 || extracted.colors.is_empty());
    }
}
