//! Theme — first-class design system object for the Theme Panel.
//!
//! A Theme maps 1:1 to FoundationTokens (Layer 1) + DarkModeColors.
//! `Theme::to_foundation_tokens()` and `Theme::from_foundation_tokens()` are
//! reversible conversions. The Theme Panel writes Layer 1 tokens in bulk.

use crate::design_import::design_md::{self, DesignMd};
use crate::token_tailwind::token_set_to_tailwind_config;
use crate::tokens::{DarkModeColors, FoundationTokens, TokenSet};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use thiserror::Error;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ThemeError {
    #[error("invalid theme JSON: {0}")]
    InvalidJson(String),
    #[error("invalid DESIGN.md: {0}")]
    InvalidDesignMd(String),
    #[error("invalid DTCG JSON: {0}")]
    InvalidDtcg(String),
    #[error("token error: {0}")]
    TokenError(#[from] crate::tokens::TokenError),
}

// ─── Theme Colors ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeColors {
    pub primary: String,
    pub secondary: String,
    pub accent: String,
    pub bg: String,
    pub bg_secondary: String,
    pub text: String,
    pub text_secondary: String,
    pub border: String,
    // Dark mode counterparts
    pub dark_primary: String,
    pub dark_secondary: String,
    pub dark_accent: String,
    pub dark_bg: String,
    pub dark_bg_secondary: String,
    pub dark_text: String,
    pub dark_text_secondary: String,
    pub dark_border: String,
}

impl Default for ThemeColors {
    fn default() -> Self {
        let ft = FoundationTokens::default();
        let dm = DarkModeColors::default();
        Self {
            primary: ft.color_primary,
            secondary: ft.color_secondary,
            accent: ft.color_accent,
            bg: ft.color_bg,
            bg_secondary: ft.color_bg_secondary,
            text: ft.color_text,
            text_secondary: ft.color_text_secondary,
            border: ft.color_border,
            dark_primary: dm.color_primary,
            dark_secondary: dm.color_secondary,
            dark_accent: dm.color_accent,
            dark_bg: dm.color_bg,
            dark_bg_secondary: dm.color_bg_secondary,
            dark_text: dm.color_text,
            dark_text_secondary: dm.color_text_secondary,
            dark_border: dm.color_border,
        }
    }
}

// ─── Theme Typography ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeTypography {
    pub heading_font: String,
    pub body_font: String,
    pub mono_font: String,
    pub text_xs: String,
    pub text_sm: String,
    pub text_base: String,
    pub text_lg: String,
    pub text_xl: String,
    pub text_2xl: String,
    pub text_3xl: String,
    pub text_4xl: String,
}

impl Default for ThemeTypography {
    fn default() -> Self {
        let ft = FoundationTokens::default();
        Self {
            heading_font: ft.font_heading,
            body_font: ft.font_body,
            mono_font: ft.font_mono,
            text_xs: ft.text_xs,
            text_sm: ft.text_sm,
            text_base: ft.text_base,
            text_lg: ft.text_lg,
            text_xl: ft.text_xl,
            text_2xl: ft.text_2xl,
            text_3xl: ft.text_3xl,
            text_4xl: ft.text_4xl,
        }
    }
}

// ─── Theme Spacing ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeSpacing {
    pub xs: String,
    pub sm: String,
    pub md: String,
    pub lg: String,
    pub xl: String,
    pub xxl: String,
    pub section: String,
}

impl Default for ThemeSpacing {
    fn default() -> Self {
        let ft = FoundationTokens::default();
        Self {
            xs: ft.space_xs,
            sm: ft.space_sm,
            md: ft.space_md,
            lg: ft.space_lg,
            xl: ft.space_xl,
            xxl: ft.space_2xl,
            section: ft.space_section,
        }
    }
}

// ─── Theme Radii ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeRadii {
    pub sm: String,
    pub md: String,
    pub lg: String,
    pub xl: String,
    pub full: String,
}

impl Default for ThemeRadii {
    fn default() -> Self {
        let ft = FoundationTokens::default();
        Self {
            sm: ft.radius_sm,
            md: ft.radius_md,
            lg: ft.radius_lg,
            xl: ft.radius_xl,
            full: ft.radius_full,
        }
    }
}

// ─── Theme Shadows ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeShadows {
    pub sm: String,
    pub md: String,
    pub lg: String,
    pub xl: String,
}

impl Default for ThemeShadows {
    fn default() -> Self {
        let ft = FoundationTokens::default();
        Self {
            sm: ft.shadow_sm,
            md: ft.shadow_md,
            lg: ft.shadow_lg,
            xl: ft.shadow_xl,
        }
    }
}

// ─── Theme Motion ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeMotion {
    pub duration_fast: String,
    pub duration_normal: String,
    pub duration_slow: String,
    pub ease_default: String,
}

impl Default for ThemeMotion {
    fn default() -> Self {
        let ft = FoundationTokens::default();
        Self {
            duration_fast: ft.duration_fast,
            duration_normal: ft.duration_normal,
            duration_slow: ft.duration_slow,
            ease_default: ft.ease_default,
        }
    }
}

// ─── Theme ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Theme {
    pub name: String,
    pub colors: ThemeColors,
    pub typography: ThemeTypography,
    pub spacing: ThemeSpacing,
    pub radii: ThemeRadii,
    pub shadows: ThemeShadows,
    pub motion: ThemeMotion,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "Default".into(),
            colors: ThemeColors::default(),
            typography: ThemeTypography::default(),
            spacing: ThemeSpacing::default(),
            radii: ThemeRadii::default(),
            shadows: ThemeShadows::default(),
            motion: ThemeMotion::default(),
        }
    }
}

impl Theme {
    /// Convert theme to Layer 1 foundation tokens.
    pub fn to_foundation_tokens(&self) -> FoundationTokens {
        FoundationTokens {
            color_primary: self.colors.primary.clone(),
            color_secondary: self.colors.secondary.clone(),
            color_accent: self.colors.accent.clone(),
            color_bg: self.colors.bg.clone(),
            color_bg_secondary: self.colors.bg_secondary.clone(),
            color_text: self.colors.text.clone(),
            color_text_secondary: self.colors.text_secondary.clone(),
            color_border: self.colors.border.clone(),
            font_heading: self.typography.heading_font.clone(),
            font_body: self.typography.body_font.clone(),
            font_mono: self.typography.mono_font.clone(),
            text_xs: self.typography.text_xs.clone(),
            text_sm: self.typography.text_sm.clone(),
            text_base: self.typography.text_base.clone(),
            text_lg: self.typography.text_lg.clone(),
            text_xl: self.typography.text_xl.clone(),
            text_2xl: self.typography.text_2xl.clone(),
            text_3xl: self.typography.text_3xl.clone(),
            text_4xl: self.typography.text_4xl.clone(),
            space_xs: self.spacing.xs.clone(),
            space_sm: self.spacing.sm.clone(),
            space_md: self.spacing.md.clone(),
            space_lg: self.spacing.lg.clone(),
            space_xl: self.spacing.xl.clone(),
            space_2xl: self.spacing.xxl.clone(),
            space_section: self.spacing.section.clone(),
            radius_sm: self.radii.sm.clone(),
            radius_md: self.radii.md.clone(),
            radius_lg: self.radii.lg.clone(),
            radius_xl: self.radii.xl.clone(),
            radius_full: self.radii.full.clone(),
            shadow_sm: self.shadows.sm.clone(),
            shadow_md: self.shadows.md.clone(),
            shadow_lg: self.shadows.lg.clone(),
            shadow_xl: self.shadows.xl.clone(),
            duration_fast: self.motion.duration_fast.clone(),
            duration_normal: self.motion.duration_normal.clone(),
            duration_slow: self.motion.duration_slow.clone(),
            ease_default: self.motion.ease_default.clone(),
        }
    }

    /// Convert theme to DarkModeColors.
    pub fn to_dark_mode_colors(&self) -> DarkModeColors {
        DarkModeColors {
            color_primary: self.colors.dark_primary.clone(),
            color_secondary: self.colors.dark_secondary.clone(),
            color_accent: self.colors.dark_accent.clone(),
            color_bg: self.colors.dark_bg.clone(),
            color_bg_secondary: self.colors.dark_bg_secondary.clone(),
            color_text: self.colors.dark_text.clone(),
            color_text_secondary: self.colors.dark_text_secondary.clone(),
            color_border: self.colors.dark_border.clone(),
        }
    }

    /// Build a Theme from existing FoundationTokens + DarkModeColors.
    pub fn from_foundation_tokens(
        tokens: &FoundationTokens,
        dark: &DarkModeColors,
        name: &str,
    ) -> Self {
        Self {
            name: name.into(),
            colors: ThemeColors {
                primary: tokens.color_primary.clone(),
                secondary: tokens.color_secondary.clone(),
                accent: tokens.color_accent.clone(),
                bg: tokens.color_bg.clone(),
                bg_secondary: tokens.color_bg_secondary.clone(),
                text: tokens.color_text.clone(),
                text_secondary: tokens.color_text_secondary.clone(),
                border: tokens.color_border.clone(),
                dark_primary: dark.color_primary.clone(),
                dark_secondary: dark.color_secondary.clone(),
                dark_accent: dark.color_accent.clone(),
                dark_bg: dark.color_bg.clone(),
                dark_bg_secondary: dark.color_bg_secondary.clone(),
                dark_text: dark.color_text.clone(),
                dark_text_secondary: dark.color_text_secondary.clone(),
                dark_border: dark.color_border.clone(),
            },
            typography: ThemeTypography {
                heading_font: tokens.font_heading.clone(),
                body_font: tokens.font_body.clone(),
                mono_font: tokens.font_mono.clone(),
                text_xs: tokens.text_xs.clone(),
                text_sm: tokens.text_sm.clone(),
                text_base: tokens.text_base.clone(),
                text_lg: tokens.text_lg.clone(),
                text_xl: tokens.text_xl.clone(),
                text_2xl: tokens.text_2xl.clone(),
                text_3xl: tokens.text_3xl.clone(),
                text_4xl: tokens.text_4xl.clone(),
            },
            spacing: ThemeSpacing {
                xs: tokens.space_xs.clone(),
                sm: tokens.space_sm.clone(),
                md: tokens.space_md.clone(),
                lg: tokens.space_lg.clone(),
                xl: tokens.space_xl.clone(),
                xxl: tokens.space_2xl.clone(),
                section: tokens.space_section.clone(),
            },
            radii: ThemeRadii {
                sm: tokens.radius_sm.clone(),
                md: tokens.radius_md.clone(),
                lg: tokens.radius_lg.clone(),
                xl: tokens.radius_xl.clone(),
                full: tokens.radius_full.clone(),
            },
            shadows: ThemeShadows {
                sm: tokens.shadow_sm.clone(),
                md: tokens.shadow_md.clone(),
                lg: tokens.shadow_lg.clone(),
                xl: tokens.shadow_xl.clone(),
            },
            motion: ThemeMotion {
                duration_fast: tokens.duration_fast.clone(),
                duration_normal: tokens.duration_normal.clone(),
                duration_slow: tokens.duration_slow.clone(),
                ease_default: tokens.ease_default.clone(),
            },
        }
    }

    // ─── Export Formats ─────────────────────────────────────────────────

    /// Export as CSS custom properties in a `:root {}` block.
    pub fn to_css_variables(&self) -> String {
        let ts = TokenSet {
            foundation: self.to_foundation_tokens(),
            dark_mode: self.to_dark_mode_colors(),
            semantic: Default::default(),
            overrides: Vec::new(),
        };
        ts.to_css()
    }

    /// Export as Tailwind config.
    pub fn to_tailwind_config(&self) -> String {
        let ts = TokenSet {
            foundation: self.to_foundation_tokens(),
            dark_mode: self.to_dark_mode_colors(),
            semantic: Default::default(),
            overrides: Vec::new(),
        };
        token_set_to_tailwind_config(&ts)
    }

    /// Export as DESIGN.md (Stitch-compatible).
    pub fn to_design_md(&self) -> String {
        let mut md = String::with_capacity(2048);
        let _ = writeln!(md, "# Design System — {}\n", self.name);

        let _ = writeln!(md, "## Colors\n");
        let _ = writeln!(md, "| Token | Light | Dark |");
        let _ = writeln!(md, "|-------|-------|------|");
        let _ = writeln!(
            md,
            "| primary | {} | {} |",
            self.colors.primary, self.colors.dark_primary
        );
        let _ = writeln!(
            md,
            "| secondary | {} | {} |",
            self.colors.secondary, self.colors.dark_secondary
        );
        let _ = writeln!(
            md,
            "| accent | {} | {} |",
            self.colors.accent, self.colors.dark_accent
        );
        let _ = writeln!(
            md,
            "| background | {} | {} |",
            self.colors.bg, self.colors.dark_bg
        );
        let _ = writeln!(
            md,
            "| bg-secondary | {} | {} |",
            self.colors.bg_secondary, self.colors.dark_bg_secondary
        );
        let _ = writeln!(
            md,
            "| text | {} | {} |",
            self.colors.text, self.colors.dark_text
        );
        let _ = writeln!(
            md,
            "| text-secondary | {} | {} |",
            self.colors.text_secondary, self.colors.dark_text_secondary
        );
        let _ = writeln!(
            md,
            "| border | {} | {} |",
            self.colors.border, self.colors.dark_border
        );

        let _ = writeln!(md, "\n## Typography\n");
        let _ = writeln!(md, "- heading: {}", self.typography.heading_font);
        let _ = writeln!(md, "- body: {}", self.typography.body_font);
        let _ = writeln!(md, "- mono: {}", self.typography.mono_font);

        let _ = writeln!(md, "\n## Spacing\n");
        let _ = writeln!(md, "- xs: {}", self.spacing.xs);
        let _ = writeln!(md, "- sm: {}", self.spacing.sm);
        let _ = writeln!(md, "- md: {}", self.spacing.md);
        let _ = writeln!(md, "- lg: {}", self.spacing.lg);
        let _ = writeln!(md, "- xl: {}", self.spacing.xl);
        let _ = writeln!(md, "- 2xl: {}", self.spacing.xxl);
        let _ = writeln!(md, "- section: {}", self.spacing.section);

        let _ = writeln!(md, "\n## Border Radius\n");
        let _ = writeln!(md, "- sm: {}", self.radii.sm);
        let _ = writeln!(md, "- md: {}", self.radii.md);
        let _ = writeln!(md, "- lg: {}", self.radii.lg);
        let _ = writeln!(md, "- xl: {}", self.radii.xl);
        let _ = writeln!(md, "- full: {}", self.radii.full);

        md
    }

    /// Export as W3C DTCG JSON.
    pub fn to_dtcg_json(&self) -> String {
        let dtcg = serde_json::json!({
            "$schema": "https://design-tokens.github.io/community-group/format/",
            "color": {
                "primary": { "$type": "color", "$value": &self.colors.primary },
                "secondary": { "$type": "color", "$value": &self.colors.secondary },
                "accent": { "$type": "color", "$value": &self.colors.accent },
                "bg": { "$type": "color", "$value": &self.colors.bg },
                "bg-secondary": { "$type": "color", "$value": &self.colors.bg_secondary },
                "text": { "$type": "color", "$value": &self.colors.text },
                "text-secondary": { "$type": "color", "$value": &self.colors.text_secondary },
                "border": { "$type": "color", "$value": &self.colors.border }
            },
            "color-dark": {
                "primary": { "$type": "color", "$value": &self.colors.dark_primary },
                "secondary": { "$type": "color", "$value": &self.colors.dark_secondary },
                "accent": { "$type": "color", "$value": &self.colors.dark_accent },
                "bg": { "$type": "color", "$value": &self.colors.dark_bg },
                "bg-secondary": { "$type": "color", "$value": &self.colors.dark_bg_secondary },
                "text": { "$type": "color", "$value": &self.colors.dark_text },
                "text-secondary": { "$type": "color", "$value": &self.colors.dark_text_secondary },
                "border": { "$type": "color", "$value": &self.colors.dark_border }
            },
            "font": {
                "heading": { "$type": "fontFamily", "$value": &self.typography.heading_font },
                "body": { "$type": "fontFamily", "$value": &self.typography.body_font },
                "mono": { "$type": "fontFamily", "$value": &self.typography.mono_font }
            },
            "spacing": {
                "xs": { "$type": "dimension", "$value": &self.spacing.xs },
                "sm": { "$type": "dimension", "$value": &self.spacing.sm },
                "md": { "$type": "dimension", "$value": &self.spacing.md },
                "lg": { "$type": "dimension", "$value": &self.spacing.lg },
                "xl": { "$type": "dimension", "$value": &self.spacing.xl },
                "2xl": { "$type": "dimension", "$value": &self.spacing.xxl },
                "section": { "$type": "dimension", "$value": &self.spacing.section }
            },
            "radii": {
                "sm": { "$type": "dimension", "$value": &self.radii.sm },
                "md": { "$type": "dimension", "$value": &self.radii.md },
                "lg": { "$type": "dimension", "$value": &self.radii.lg },
                "xl": { "$type": "dimension", "$value": &self.radii.xl },
                "full": { "$type": "dimension", "$value": &self.radii.full }
            }
        });
        serde_json::to_string_pretty(&dtcg).unwrap_or_default()
    }

    // ─── Import Formats ─────────────────────────────────────────────────

    /// Import from DESIGN.md content.
    pub fn from_design_md(content: &str) -> Result<Self, ThemeError> {
        let dm = design_md::parse_design_md(content)
            .map_err(|e| ThemeError::InvalidDesignMd(e.to_string()))?;

        let mut theme = Theme {
            name: "Imported".into(),
            ..Theme::default()
        };

        apply_design_md_to_theme(&dm, &mut theme);
        Ok(theme)
    }

    /// Import from W3C DTCG JSON.
    pub fn from_dtcg_json(content: &str) -> Result<Self, ThemeError> {
        let v: serde_json::Value =
            serde_json::from_str(content).map_err(|e| ThemeError::InvalidDtcg(e.to_string()))?;

        let mut theme = Theme {
            name: "Imported DTCG".into(),
            ..Theme::default()
        };

        // Parse color group
        if let Some(colors) = v.get("color").and_then(|c| c.as_object()) {
            for (key, val) in colors {
                if let Some(value) = val.get("$value").and_then(|v| v.as_str()) {
                    match key.as_str() {
                        "primary" => theme.colors.primary = value.into(),
                        "secondary" => theme.colors.secondary = value.into(),
                        "accent" => theme.colors.accent = value.into(),
                        "bg" => theme.colors.bg = value.into(),
                        "bg-secondary" => theme.colors.bg_secondary = value.into(),
                        "text" => theme.colors.text = value.into(),
                        "text-secondary" => theme.colors.text_secondary = value.into(),
                        "border" => theme.colors.border = value.into(),
                        _ => {}
                    }
                }
            }
        }

        // Parse dark color group
        if let Some(colors) = v.get("color-dark").and_then(|c| c.as_object()) {
            for (key, val) in colors {
                if let Some(value) = val.get("$value").and_then(|v| v.as_str()) {
                    match key.as_str() {
                        "primary" => theme.colors.dark_primary = value.into(),
                        "secondary" => theme.colors.dark_secondary = value.into(),
                        "accent" => theme.colors.dark_accent = value.into(),
                        "bg" => theme.colors.dark_bg = value.into(),
                        "bg-secondary" => theme.colors.dark_bg_secondary = value.into(),
                        "text" => theme.colors.dark_text = value.into(),
                        "text-secondary" => theme.colors.dark_text_secondary = value.into(),
                        "border" => theme.colors.dark_border = value.into(),
                        _ => {}
                    }
                }
            }
        }

        // Parse font group
        if let Some(fonts) = v.get("font").and_then(|f| f.as_object()) {
            for (key, val) in fonts {
                if let Some(value) = val.get("$value").and_then(|v| v.as_str()) {
                    match key.as_str() {
                        "heading" => theme.typography.heading_font = value.into(),
                        "body" => theme.typography.body_font = value.into(),
                        "mono" => theme.typography.mono_font = value.into(),
                        _ => {}
                    }
                }
            }
        }

        // Parse spacing group
        if let Some(spacing) = v.get("spacing").and_then(|s| s.as_object()) {
            for (key, val) in spacing {
                if let Some(value) = val.get("$value").and_then(|v| v.as_str()) {
                    match key.as_str() {
                        "xs" => theme.spacing.xs = value.into(),
                        "sm" => theme.spacing.sm = value.into(),
                        "md" => theme.spacing.md = value.into(),
                        "lg" => theme.spacing.lg = value.into(),
                        "xl" => theme.spacing.xl = value.into(),
                        "2xl" => theme.spacing.xxl = value.into(),
                        "section" => theme.spacing.section = value.into(),
                        _ => {}
                    }
                }
            }
        }

        // Parse radii group
        if let Some(radii) = v.get("radii").and_then(|r| r.as_object()) {
            for (key, val) in radii {
                if let Some(value) = val.get("$value").and_then(|v| v.as_str()) {
                    match key.as_str() {
                        "sm" => theme.radii.sm = value.into(),
                        "md" => theme.radii.md = value.into(),
                        "lg" => theme.radii.lg = value.into(),
                        "xl" => theme.radii.xl = value.into(),
                        "full" => theme.radii.full = value.into(),
                        _ => {}
                    }
                }
            }
        }

        Ok(theme)
    }
}

// ─── Apply / Extract ────────────────────────────────────────────────────────

/// Apply a theme to a project's TokenSet (writes Layer 1 + dark mode).
pub fn apply_theme(token_set: &mut TokenSet, theme: &Theme) -> Result<(), ThemeError> {
    token_set.foundation = theme.to_foundation_tokens();
    token_set.dark_mode = theme.to_dark_mode_colors();
    Ok(())
}

/// Extract the current theme from a project's TokenSet.
pub fn extract_theme(token_set: &TokenSet) -> Theme {
    Theme::from_foundation_tokens(&token_set.foundation, &token_set.dark_mode, "Current")
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn apply_design_md_to_theme(dm: &DesignMd, theme: &mut Theme) {
    for (key, value) in &dm.colors {
        let lower = key.to_lowercase();
        if lower.contains("primary") {
            theme.colors.primary = value.clone();
        } else if lower.contains("secondary") && !lower.contains("text") && !lower.contains("bg") {
            theme.colors.secondary = value.clone();
        } else if lower.contains("accent") {
            theme.colors.accent = value.clone();
        } else if lower.contains("background") || lower == "bg" {
            theme.colors.bg = value.clone();
        } else if lower.contains("bg-secondary") || lower.contains("bg_secondary") {
            theme.colors.bg_secondary = value.clone();
        } else if lower.contains("text-secondary") || lower.contains("text_secondary") {
            theme.colors.text_secondary = value.clone();
        } else if lower.contains("text") {
            theme.colors.text = value.clone();
        } else if lower.contains("border") {
            theme.colors.border = value.clone();
        }
    }

    for (key, value) in &dm.fonts {
        let lower = key.to_lowercase();
        if lower.contains("heading") || lower.contains("display") {
            theme.typography.heading_font = value.clone();
        } else if lower.contains("body") || lower.contains("sans") {
            theme.typography.body_font = value.clone();
        } else if lower.contains("mono") || lower.contains("code") {
            theme.typography.mono_font = value.clone();
        }
    }

    for (key, value) in &dm.spacing {
        let lower = key.to_lowercase();
        match lower.as_str() {
            "xs" => theme.spacing.xs = value.clone(),
            "sm" => theme.spacing.sm = value.clone(),
            "md" => theme.spacing.md = value.clone(),
            "lg" => theme.spacing.lg = value.clone(),
            "xl" => theme.spacing.xl = value.clone(),
            "2xl" => theme.spacing.xxl = value.clone(),
            "section" => theme.spacing.section = value.clone(),
            _ => {}
        }
    }

    for (key, value) in &dm.radii {
        let lower = key.to_lowercase();
        match lower.as_str() {
            "sm" => theme.radii.sm = value.clone(),
            "md" => theme.radii.md = value.clone(),
            "lg" => theme.radii.lg = value.clone(),
            "xl" => theme.radii.xl = value.clone(),
            "full" => theme.radii.full = value.clone(),
            _ => {}
        }
    }
}

/// Info struct for preset listing (lightweight, for UI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemePresetInfo {
    pub name: String,
    pub primary: String,
    pub secondary: String,
    pub accent: String,
    pub bg: String,
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_to_foundation_tokens_roundtrip() {
        let theme = Theme {
            name: "Test".into(),
            colors: ThemeColors {
                primary: "#3b82f6".into(),
                secondary: "#8b5cf6".into(),
                accent: "#06b6d4".into(),
                bg: "#ffffff".into(),
                bg_secondary: "#f8fafc".into(),
                text: "#0f172a".into(),
                text_secondary: "#64748b".into(),
                border: "#e2e8f0".into(),
                dark_primary: "#60a5fa".into(),
                dark_secondary: "#a78bfa".into(),
                dark_accent: "#22d3ee".into(),
                dark_bg: "#0f172a".into(),
                dark_bg_secondary: "#1e293b".into(),
                dark_text: "#f8fafc".into(),
                dark_text_secondary: "#94a3b8".into(),
                dark_border: "#334155".into(),
            },
            ..Theme::default()
        };

        let ft = theme.to_foundation_tokens();
        let dm = theme.to_dark_mode_colors();
        let roundtrip = Theme::from_foundation_tokens(&ft, &dm, "Test");

        assert_eq!(theme.colors, roundtrip.colors);
        assert_eq!(theme.typography, roundtrip.typography);
        assert_eq!(theme.spacing, roundtrip.spacing);
        assert_eq!(theme.radii, roundtrip.radii);
        assert_eq!(theme.shadows, roundtrip.shadows);
        assert_eq!(theme.motion, roundtrip.motion);
    }

    #[test]
    fn test_apply_theme_updates_token_set() {
        let mut ts = TokenSet::default();
        let theme = Theme {
            colors: ThemeColors {
                primary: "#ff0000".into(),
                ..ThemeColors::default()
            },
            ..Theme::default()
        };
        apply_theme(&mut ts, &theme).unwrap();
        assert_eq!(ts.foundation.color_primary, "#ff0000");
    }

    #[test]
    fn test_extract_theme_from_token_set() {
        let mut ts = TokenSet::default();
        ts.foundation.color_primary = "#deadbe".into();
        ts.dark_mode.color_primary = "#beefca".into();
        let theme = extract_theme(&ts);
        assert_eq!(theme.colors.primary, "#deadbe");
        assert_eq!(theme.colors.dark_primary, "#beefca");
    }

    #[test]
    fn test_to_css_variables() {
        let theme = Theme::default();
        let css = theme.to_css_variables();
        assert!(css.contains(":root {"), "missing :root");
        assert!(css.contains("--color-primary:"));
        assert!(css.contains("--font-heading:"));
        assert!(css.contains("prefers-color-scheme: dark"));
    }

    #[test]
    fn test_to_tailwind_config() {
        let theme = Theme::default();
        let cfg = theme.to_tailwind_config();
        assert!(cfg.contains("export default"));
        assert!(cfg.contains("primary: 'var(--color-primary)'"));
        assert!(cfg.contains("satisfies Config"));
    }

    #[test]
    fn test_to_design_md() {
        let theme = Theme {
            name: "TestTheme".into(),
            colors: ThemeColors {
                primary: "#4f46e5".into(),
                ..ThemeColors::default()
            },
            ..Theme::default()
        };
        let md = theme.to_design_md();
        assert!(md.contains("# Design System — TestTheme"));
        assert!(md.contains("#4f46e5"));
        assert!(md.contains("## Colors"));
        assert!(md.contains("## Typography"));
        assert!(md.contains("## Spacing"));
    }

    #[test]
    fn test_to_dtcg_json() {
        let theme = Theme::default();
        let json = theme.to_dtcg_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v.get("color").is_some());
        assert!(v.get("color-dark").is_some());
        assert!(v.get("font").is_some());
        assert!(v.get("spacing").is_some());
        assert!(v.get("radii").is_some());
    }

    #[test]
    fn test_from_design_md() {
        let content = r#"
# Colors
- primary: #ff0000
- secondary: #00ff00
- accent: #0000ff
- background: #ffffff
- text: #000000

# Typography
- heading: Inter
- body: Source Sans
"#;
        let theme = Theme::from_design_md(content).unwrap();
        assert_eq!(theme.colors.primary, "#ff0000");
        assert_eq!(theme.colors.secondary, "#00ff00");
        assert_eq!(theme.colors.accent, "#0000ff");
        assert_eq!(theme.colors.bg, "#ffffff");
        assert_eq!(theme.colors.text, "#000000");
        assert_eq!(theme.typography.heading_font, "Inter");
        assert_eq!(theme.typography.body_font, "Source Sans");
    }

    #[test]
    fn test_from_dtcg_json() {
        let theme_orig = Theme {
            name: "Roundtrip".into(),
            colors: ThemeColors {
                primary: "#aabbcc".into(),
                secondary: "#112233".into(),
                ..ThemeColors::default()
            },
            ..Theme::default()
        };
        let json = theme_orig.to_dtcg_json();
        let theme_back = Theme::from_dtcg_json(&json).unwrap();
        assert_eq!(theme_back.colors.primary, "#aabbcc");
        assert_eq!(theme_back.colors.secondary, "#112233");
    }

    #[test]
    fn test_design_md_roundtrip() {
        let theme = Theme {
            name: "Roundtrip".into(),
            colors: ThemeColors {
                primary: "#4f46e5".into(),
                secondary: "#7c3aed".into(),
                accent: "#06b6d4".into(),
                bg: "#ffffff".into(),
                bg_secondary: "#f8fafc".into(),
                text: "#0f172a".into(),
                text_secondary: "#64748b".into(),
                border: "#e2e8f0".into(),
                ..ThemeColors::default()
            },
            ..Theme::default()
        };
        let md = theme.to_design_md();
        let imported = Theme::from_design_md(&md).unwrap();
        assert_eq!(theme.colors.primary, imported.colors.primary);
        assert_eq!(theme.colors.secondary, imported.colors.secondary);
        assert_eq!(theme.colors.accent, imported.colors.accent);
        assert_eq!(theme.colors.text, imported.colors.text);
    }

    #[test]
    fn test_dtcg_roundtrip() {
        let theme = Theme {
            name: "DTCG RT".into(),
            colors: ThemeColors {
                primary: "#aaa111".into(),
                dark_primary: "#bbb222".into(),
                ..ThemeColors::default()
            },
            ..Theme::default()
        };
        let json = theme.to_dtcg_json();
        let back = Theme::from_dtcg_json(&json).unwrap();
        assert_eq!(theme.colors.primary, back.colors.primary);
        assert_eq!(theme.colors.dark_primary, back.colors.dark_primary);
    }

    #[test]
    fn test_theme_has_dark_mode() {
        let theme = Theme::default();
        assert!(!theme.colors.dark_primary.is_empty());
        assert!(!theme.colors.dark_secondary.is_empty());
        assert!(!theme.colors.dark_accent.is_empty());
        assert!(!theme.colors.dark_bg.is_empty());
        assert!(!theme.colors.dark_bg_secondary.is_empty());
        assert!(!theme.colors.dark_text.is_empty());
        assert!(!theme.colors.dark_text_secondary.is_empty());
        assert!(!theme.colors.dark_border.is_empty());
    }
}
