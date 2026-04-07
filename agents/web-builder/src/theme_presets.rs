//! Theme Presets — named Theme objects derived from existing palette + typography presets.
//!
//! Converts the 24 palette presets × 4 typography presets into named Theme objects.
//! All presets are available to all templates.

use crate::theme::{Theme, ThemeColors, ThemePresetInfo, ThemeTypography};
use crate::variant::{all_palette_presets, all_typography_presets};

/// Get all preset themes. Each palette preset is combined with the "tech"
/// typography preset to produce a named Theme with dark mode colors.
pub fn get_preset_themes() -> Vec<Theme> {
    let typo = all_typography_presets()
        .first()
        .expect("at least one typography preset");

    all_palette_presets()
        .iter()
        .map(|palette| {
            let display_name = format!(
                "{} {}",
                capitalize(palette.template_id.replace('_', " ").trim()),
                palette.name
            );

            Theme {
                name: display_name,
                colors: ThemeColors {
                    primary: palette.light.primary.into(),
                    secondary: palette.light.secondary.into(),
                    accent: palette.light.accent.into(),
                    bg: palette.light.bg.into(),
                    bg_secondary: palette.light.bg_secondary.into(),
                    text: palette.light.text.into(),
                    text_secondary: palette.light.text_secondary.into(),
                    border: palette.light.border.into(),
                    dark_primary: palette.dark.primary.into(),
                    dark_secondary: palette.dark.secondary.into(),
                    dark_accent: palette.dark.accent.into(),
                    dark_bg: palette.dark.bg.into(),
                    dark_bg_secondary: palette.dark.bg_secondary.into(),
                    dark_text: palette.dark.text.into(),
                    dark_text_secondary: palette.dark.text_secondary.into(),
                    dark_border: palette.dark.border.into(),
                },
                typography: ThemeTypography {
                    heading_font: typo.font_heading.into(),
                    body_font: typo.font_body.into(),
                    mono_font: typo.font_mono.into(),
                    text_xs: typo.text_xs.into(),
                    text_sm: typo.text_sm.into(),
                    text_base: typo.text_base.into(),
                    text_lg: typo.text_lg.into(),
                    text_xl: typo.text_xl.into(),
                    text_2xl: typo.text_2xl.into(),
                    text_3xl: typo.text_3xl.into(),
                    text_4xl: typo.text_4xl.into(),
                },
                ..Theme::default()
            }
        })
        .collect()
}

/// Get lightweight preset info for the UI (name + swatch colors).
pub fn get_preset_info_list() -> Vec<ThemePresetInfo> {
    get_preset_themes()
        .iter()
        .map(|t| ThemePresetInfo {
            name: t.name.clone(),
            primary: t.colors.primary.clone(),
            secondary: t.colors.secondary.clone(),
            accent: t.colors.accent.clone(),
            bg: t.colors.bg.clone(),
        })
        .collect()
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut result = first.to_uppercase().to_string();
            result.extend(chars);
            result
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_count() {
        let presets = get_preset_themes();
        assert!(
            presets.len() >= 8,
            "expected at least 8 presets, got {}",
            presets.len()
        );
    }

    #[test]
    fn test_all_presets_valid() {
        for theme in get_preset_themes() {
            assert!(!theme.name.is_empty(), "preset name should not be empty");
            assert!(
                !theme.colors.primary.is_empty(),
                "primary color missing for {}",
                theme.name
            );
            assert!(
                !theme.colors.bg.is_empty(),
                "bg color missing for {}",
                theme.name
            );
            assert!(
                !theme.colors.text.is_empty(),
                "text color missing for {}",
                theme.name
            );
            assert!(
                !theme.typography.heading_font.is_empty(),
                "heading font missing for {}",
                theme.name
            );
            assert!(
                !theme.typography.body_font.is_empty(),
                "body font missing for {}",
                theme.name
            );
        }
    }

    #[test]
    fn test_presets_have_dark_mode() {
        for theme in get_preset_themes() {
            assert!(
                !theme.colors.dark_primary.is_empty(),
                "dark primary missing for {}",
                theme.name
            );
            assert!(
                !theme.colors.dark_bg.is_empty(),
                "dark bg missing for {}",
                theme.name
            );
            assert!(
                !theme.colors.dark_text.is_empty(),
                "dark text missing for {}",
                theme.name
            );
            assert!(
                !theme.colors.dark_border.is_empty(),
                "dark border missing for {}",
                theme.name
            );
        }
    }

    #[test]
    fn test_preset_names_unique() {
        let presets = get_preset_themes();
        let names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
        let mut deduped = names.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(names.len(), deduped.len(), "duplicate preset names found");
    }

    #[test]
    fn test_preset_info_list() {
        let info = get_preset_info_list();
        assert!(info.len() >= 8);
        for item in &info {
            assert!(!item.name.is_empty());
            assert!(item.primary.starts_with('#'));
        }
    }

    #[test]
    fn test_preset_produces_valid_token_set() {
        use crate::theme::apply_theme;
        use crate::tokens::TokenSet;

        for theme in get_preset_themes() {
            let mut ts = TokenSet::default();
            apply_theme(&mut ts, &theme).unwrap();
            let css = ts.to_css();
            assert!(
                css.contains(":root {"),
                "preset {} produces invalid CSS",
                theme.name
            );
            assert!(
                css.contains(&format!("--color-primary: {};", theme.colors.primary)),
                "preset {} primary color not in CSS",
                theme.name
            );
        }
    }
}
