//! Placeholder SVG generation — zero-cost fallback that always works.
//!
//! Generates styled SVG placeholders color-matched to the theme.

use super::ThemeColors;

/// Generate an SVG placeholder image with theme colors and description text.
///
/// The SVG uses the theme's background/text colors, shows a camera icon,
/// and displays the image description as subtle text.
pub fn generate_placeholder(
    prompt: &str,
    width: u32,
    height: u32,
    theme_colors: &ThemeColors,
) -> String {
    let bg = if theme_colors.bg_secondary.is_empty() {
        "#1a1a2e"
    } else {
        &theme_colors.bg_secondary
    };
    let text_color = if theme_colors.text_secondary.is_empty() {
        "#a0a0b0"
    } else {
        &theme_colors.text_secondary
    };
    let accent = if theme_colors.primary.is_empty() {
        "#6366f1"
    } else {
        &theme_colors.primary
    };

    // Truncate prompt text for display
    let display_text = if prompt.len() > 60 {
        format!("{}...", &prompt[..57])
    } else {
        prompt.to_string()
    };
    let escaped_text = svg_escape(&display_text);

    let icon_y = height / 2 - 20;
    let text_y = height / 2 + 30;
    let icon_x = width / 2;

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
  <rect width="{width}" height="{height}" fill="{bg}" rx="8"/>
  <rect x="4" y="4" width="{w4}" height="{h4}" fill="none" stroke="{accent}" stroke-width="1" stroke-dasharray="8 4" rx="6" opacity="0.3"/>
  <g transform="translate({icon_x},{icon_y})" fill="{accent}" opacity="0.5">
    <rect x="-20" y="-14" width="40" height="28" rx="4" fill="none" stroke="{accent}" stroke-width="2"/>
    <circle cx="0" cy="0" r="8" fill="none" stroke="{accent}" stroke-width="2"/>
    <circle cx="12" cy="-8" r="3" fill="{accent}"/>
  </g>
  <text x="{icon_x}" y="{text_y}" text-anchor="middle" fill="{text_color}" font-family="system-ui, sans-serif" font-size="14" opacity="0.7">{escaped_text}</text>
</svg>"##,
        w4 = width - 8,
        h4 = height - 8,
    )
}

/// Escape special characters for SVG text content.
fn svg_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme() -> ThemeColors {
        ThemeColors {
            bg: "#0f0f23".into(),
            bg_secondary: "#1a1a2e".into(),
            text: "#e0e0e0".into(),
            text_secondary: "#a0a0b0".into(),
            primary: "#6366f1".into(),
            accent: "#8b5cf6".into(),
        }
    }

    #[test]
    fn test_placeholder_valid_svg() {
        let svg = generate_placeholder("Test image", 800, 600, &test_theme());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
    }

    #[test]
    fn test_placeholder_uses_theme_colors() {
        let theme = test_theme();
        let svg = generate_placeholder("Test", 400, 300, &theme);
        assert!(svg.contains("#1a1a2e"), "should use bg_secondary");
        assert!(svg.contains("#a0a0b0"), "should use text_secondary");
        assert!(svg.contains("#6366f1"), "should use primary");
    }

    #[test]
    fn test_placeholder_shows_description() {
        let svg = generate_placeholder("Modern dashboard screenshot", 800, 600, &test_theme());
        assert!(
            svg.contains("Modern dashboard screenshot"),
            "should contain prompt text"
        );
    }

    #[test]
    fn test_placeholder_respects_dimensions() {
        let svg = generate_placeholder("Test", 1920, 1080, &test_theme());
        assert!(svg.contains("width=\"1920\""));
        assert!(svg.contains("height=\"1080\""));
        assert!(svg.contains("viewBox=\"0 0 1920 1080\""));
    }

    #[test]
    fn test_placeholder_truncates_long_text() {
        let long_prompt = "A".repeat(100);
        let svg = generate_placeholder(&long_prompt, 800, 600, &test_theme());
        assert!(svg.contains("..."), "long text should be truncated");
    }

    #[test]
    fn test_placeholder_escapes_special_chars() {
        let svg = generate_placeholder("Image <with> &special \"chars\"", 400, 300, &test_theme());
        assert!(!svg.contains("<with>"), "should escape angle brackets");
        assert!(svg.contains("&amp;"), "should escape ampersand");
    }

    #[test]
    fn test_placeholder_default_colors() {
        let empty_theme = ThemeColors::default();
        let svg = generate_placeholder("Test", 400, 300, &empty_theme);
        // Should use fallback colors
        assert!(svg.contains("#1a1a2e"), "should use fallback bg");
        assert!(svg.contains("#a0a0b0"), "should use fallback text");
        assert!(svg.contains("#6366f1"), "should use fallback accent");
    }

    #[test]
    fn test_placeholder_small_dimensions() {
        let svg = generate_placeholder("Icon", 64, 64, &test_theme());
        assert!(svg.contains("width=\"64\""));
        assert!(svg.contains("height=\"64\""));
    }
}
