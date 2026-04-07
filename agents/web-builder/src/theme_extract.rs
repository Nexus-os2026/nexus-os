//! Theme Extraction — extract a Theme from a URL or raw CSS.
//!
//! Pipeline: fetch HTML → extract `<style>` + linked CSS → parse colors/fonts →
//! heuristic mapping to Theme. Uses the existing token_extractor for CSS analysis.

use crate::design_import::token_extractor::{extract_tokens, map_to_foundation_tokens};
use crate::theme::Theme;
use crate::tokens::DarkModeColors;
use regex::Regex;
use thiserror::Error;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ThemeExtractError {
    #[error("URL must use HTTPS: {0}")]
    NotHttps(String),
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    #[error("fetch failed: {0}")]
    FetchFailed(String),
    #[error("no design tokens found in page")]
    NoTokensFound,
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Extract a Theme from a URL by fetching its HTML/CSS and analyzing design tokens.
///
/// HTTPS only. Returns a Theme with heuristically-mapped colors and fonts.
pub async fn extract_theme_from_url(url: &str) -> Result<Theme, ThemeExtractError> {
    // Validate HTTPS
    if !url.starts_with("https://") {
        return Err(ThemeExtractError::NotHttps(url.into()));
    }

    // Basic URL validation
    if url.len() < 12 || !url.contains('.') {
        return Err(ThemeExtractError::InvalidUrl(url.into()));
    }

    // Fetch HTML
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| ThemeExtractError::FetchFailed(e.to_string()))?;

    let resp = client
        .get(url)
        .header("User-Agent", "NexusBuilder/1.0")
        .send()
        .await
        .map_err(|e| ThemeExtractError::FetchFailed(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(ThemeExtractError::FetchFailed(format!(
            "HTTP {}",
            resp.status()
        )));
    }

    let html = resp
        .text()
        .await
        .map_err(|e| ThemeExtractError::FetchFailed(e.to_string()))?;

    // Extract CSS from <style> blocks
    let css = extract_inline_css(&html);

    extract_theme_from_css(&css, url)
}

/// Extract a Theme from raw CSS content (no network fetch).
pub fn extract_theme_from_css(css: &str, source_name: &str) -> Result<Theme, ThemeExtractError> {
    let extracted = extract_tokens("", css, None);

    if extracted.colors.is_empty() && extracted.fonts.is_empty() {
        return Err(ThemeExtractError::NoTokensFound);
    }

    let foundation = map_to_foundation_tokens(&extracted);

    // For dark mode, generate lighter variants of the extracted colors heuristically
    let dark = generate_dark_counterparts(&foundation);

    let mut theme = Theme::from_foundation_tokens(&foundation, &dark, source_name);
    theme.name = format!("Extracted from {source_name}");

    Ok(theme)
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Extract CSS from all `<style>` blocks in HTML.
fn extract_inline_css(html: &str) -> String {
    let re =
        Regex::new(r"(?s)<style[^>]*>(.*?)</style>").unwrap_or_else(|_| Regex::new(r"$^").unwrap());
    let mut css = String::new();
    for cap in re.captures_iter(html) {
        css.push_str(&cap[1]);
        css.push('\n');
    }
    css
}

/// Generate dark mode counterparts by adjusting brightness heuristically.
fn generate_dark_counterparts(foundation: &crate::tokens::FoundationTokens) -> DarkModeColors {
    DarkModeColors {
        color_primary: lighten_hex(&foundation.color_primary),
        color_secondary: lighten_hex(&foundation.color_secondary),
        color_accent: lighten_hex(&foundation.color_accent),
        color_bg: darken_hex(&foundation.color_bg),
        color_bg_secondary: darken_hex(&foundation.color_bg_secondary),
        color_text: invert_brightness(&foundation.color_text),
        color_text_secondary: invert_brightness(&foundation.color_text_secondary),
        color_border: darken_hex(&foundation.color_border),
    }
}

/// Lighten a hex color by ~30%.
fn lighten_hex(hex: &str) -> String {
    adjust_hex(hex, 1.3)
}

/// Darken a hex color by ~40%.
fn darken_hex(hex: &str) -> String {
    adjust_hex(hex, 0.3)
}

/// Invert brightness: dark → light, light → dark.
fn invert_brightness(hex: &str) -> String {
    let trimmed = hex.trim_start_matches('#');
    if trimmed.len() < 6 {
        return hex.into();
    }
    let r = u8::from_str_radix(&trimmed[0..2], 16).unwrap_or(128);
    let g = u8::from_str_radix(&trimmed[2..4], 16).unwrap_or(128);
    let b = u8::from_str_radix(&trimmed[4..6], 16).unwrap_or(128);
    let brightness = (r as f32 * 0.299 + g as f32 * 0.587 + b as f32 * 0.114) / 255.0;
    if brightness > 0.5 {
        // Light color → make dark
        adjust_hex(hex, 0.2)
    } else {
        // Dark color → make light
        adjust_hex(hex, 3.0)
    }
}

/// Multiply each RGB channel by factor, clamping to [0, 255].
fn adjust_hex(hex: &str, factor: f32) -> String {
    let trimmed = hex.trim_start_matches('#');
    if trimmed.len() < 6 {
        return hex.into();
    }
    let r = u8::from_str_radix(&trimmed[0..2], 16).unwrap_or(128);
    let g = u8::from_str_radix(&trimmed[2..4], 16).unwrap_or(128);
    let b = u8::from_str_radix(&trimmed[4..6], 16).unwrap_or(128);
    let nr = (r as f32 * factor).clamp(0.0, 255.0) as u8;
    let ng = (g as f32 * factor).clamp(0.0, 255.0) as u8;
    let nb = (b as f32 * factor).clamp(0.0, 255.0) as u8;
    format!("#{nr:02x}{ng:02x}{nb:02x}")
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_from_css_finds_colors() {
        let css = r#"
            body { color: #1a1a2e; background: #f0f0f5; }
            h1 { color: #4f46e5; }
            .btn { background: #06b6d4; border: 1px solid #e2e8f0; }
            .card { background: #f8fafc; color: #64748b; }
            a { color: #7c3aed; }
            footer { background: #1e293b; }
        "#;
        let theme = extract_theme_from_css(css, "test").unwrap();
        // Should have extracted colors
        assert!(!theme.colors.primary.is_empty());
    }

    #[test]
    fn test_extract_from_css_finds_fonts() {
        let css = r#"
            body { font-family: 'Inter', sans-serif; color: #333; }
            h1 { font-family: 'Playfair Display', serif; }
        "#;
        let theme = extract_theme_from_css(css, "test").unwrap();
        assert!(
            theme.typography.heading_font.contains("Inter")
                || theme.typography.heading_font.contains("Playfair"),
            "should extract a font"
        );
    }

    #[test]
    fn test_extract_maps_primary_color() {
        let css = r#"
            .brand { color: #4f46e5; }
            .brand-alt { color: #4f46e5; }
            .brand-btn { background: #4f46e5; }
            body { color: #1a1a2e; background: #ffffff; }
        "#;
        let theme = extract_theme_from_css(css, "test").unwrap();
        // Most frequent color (#4f46e5 appears 3x) → maps to primary
        assert_eq!(theme.colors.primary, "#4f46e5");
    }

    #[test]
    fn test_extract_handles_empty_css() {
        let result = extract_theme_from_css("", "empty");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ThemeExtractError::NoTokensFound
        ));
    }

    #[test]
    fn test_extract_url_validates_https() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(extract_theme_from_url("http://example.com"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ThemeExtractError::NotHttps(_)
        ));
    }

    #[test]
    fn test_extract_inline_css() {
        let html = r#"
            <html><head>
            <style>body { color: #333; }</style>
            <style>.x { background: red; }</style>
            </head><body></body></html>
        "#;
        let css = extract_inline_css(html);
        assert!(css.contains("color: #333"));
        assert!(css.contains("background: red"));
    }

    #[test]
    fn test_lighten_hex() {
        let light = lighten_hex("#4f46e5");
        assert!(light.starts_with('#'));
        assert_eq!(light.len(), 7);
    }

    #[test]
    fn test_darken_hex() {
        let dark = darken_hex("#f0f0f5");
        assert!(dark.starts_with('#'));
        assert_eq!(dark.len(), 7);
    }

    #[test]
    fn test_adjust_hex_clamps() {
        // White * 2.0 should clamp to #ffffff
        let result = adjust_hex("#ffffff", 2.0);
        assert_eq!(result, "#ffffff");
        // Black * 0.5 should stay #000000
        let result = adjust_hex("#000000", 0.5);
        assert_eq!(result, "#000000");
    }
}
