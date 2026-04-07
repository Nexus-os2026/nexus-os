//! Token Contract — 3-layer CSS custom property system.
//!
//! Layer 1: Foundation tokens (global :root values)
//! Layer 2: Semantic tokens (component aliases → foundation)
//! Layer 3: Instance overrides (scoped to specific sections)

use serde::{Deserialize, Serialize};
use std::fmt::Write;
use thiserror::Error;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("unknown foundation token: {0}")]
    UnknownFoundationToken(String),
    #[error("unknown semantic token: {0}")]
    UnknownSemanticToken(String),
    #[error("invalid token value for {name}: {reason}")]
    InvalidValue { name: String, reason: String },
}

// ─── Foundation Token Names ─────────────────────────────────────────────────

/// All valid Layer 1 (foundation) token names.
pub const FOUNDATION_TOKEN_NAMES: &[&str] = &[
    // Colors
    "color-primary",
    "color-secondary",
    "color-accent",
    "color-bg",
    "color-bg-secondary",
    "color-text",
    "color-text-secondary",
    "color-border",
    // Typography families
    "font-heading",
    "font-body",
    "font-mono",
    // Type scale
    "text-xs",
    "text-sm",
    "text-base",
    "text-lg",
    "text-xl",
    "text-2xl",
    "text-3xl",
    "text-4xl",
    // Spacing
    "space-xs",
    "space-sm",
    "space-md",
    "space-lg",
    "space-xl",
    "space-2xl",
    "space-section",
    // Radii
    "radius-sm",
    "radius-md",
    "radius-lg",
    "radius-xl",
    "radius-full",
    // Shadows
    "shadow-sm",
    "shadow-md",
    "shadow-lg",
    "shadow-xl",
    // Motion
    "duration-fast",
    "duration-normal",
    "duration-slow",
    "ease-default",
];

/// All valid Layer 2 (semantic) token names.
pub const SEMANTIC_TOKEN_NAMES: &[&str] = &[
    "btn-bg",
    "btn-text",
    "btn-border",
    "btn-hover-bg",
    "card-bg",
    "card-border",
    "card-shadow",
    "hero-bg",
    "hero-text",
    "hero-accent",
    "nav-bg",
    "nav-text",
    "nav-border",
    "footer-bg",
    "footer-text",
    "section-bg",
    "section-text",
    "input-bg",
    "input-border",
    "input-text",
    "badge-bg",
    "badge-text",
];

// ─── Foundation Tokens ──────────────────────────────────────────────────────

/// Layer 1 — global design primitives at :root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoundationTokens {
    // Colors
    pub color_primary: String,
    pub color_secondary: String,
    pub color_accent: String,
    pub color_bg: String,
    pub color_bg_secondary: String,
    pub color_text: String,
    pub color_text_secondary: String,
    pub color_border: String,
    // Typography families
    pub font_heading: String,
    pub font_body: String,
    pub font_mono: String,
    // Type scale (fluid clamp values)
    pub text_xs: String,
    pub text_sm: String,
    pub text_base: String,
    pub text_lg: String,
    pub text_xl: String,
    pub text_2xl: String,
    pub text_3xl: String,
    pub text_4xl: String,
    // Spacing
    pub space_xs: String,
    pub space_sm: String,
    pub space_md: String,
    pub space_lg: String,
    pub space_xl: String,
    pub space_2xl: String,
    pub space_section: String,
    // Radii
    pub radius_sm: String,
    pub radius_md: String,
    pub radius_lg: String,
    pub radius_xl: String,
    pub radius_full: String,
    // Shadows
    pub shadow_sm: String,
    pub shadow_md: String,
    pub shadow_lg: String,
    pub shadow_xl: String,
    // Motion
    pub duration_fast: String,
    pub duration_normal: String,
    pub duration_slow: String,
    pub ease_default: String,
}

impl FoundationTokens {
    /// Get a token value by its CSS custom property name (without `--` prefix).
    pub fn get(&self, name: &str) -> Option<&str> {
        match name {
            "color-primary" => Some(&self.color_primary),
            "color-secondary" => Some(&self.color_secondary),
            "color-accent" => Some(&self.color_accent),
            "color-bg" => Some(&self.color_bg),
            "color-bg-secondary" => Some(&self.color_bg_secondary),
            "color-text" => Some(&self.color_text),
            "color-text-secondary" => Some(&self.color_text_secondary),
            "color-border" => Some(&self.color_border),
            "font-heading" => Some(&self.font_heading),
            "font-body" => Some(&self.font_body),
            "font-mono" => Some(&self.font_mono),
            "text-xs" => Some(&self.text_xs),
            "text-sm" => Some(&self.text_sm),
            "text-base" => Some(&self.text_base),
            "text-lg" => Some(&self.text_lg),
            "text-xl" => Some(&self.text_xl),
            "text-2xl" => Some(&self.text_2xl),
            "text-3xl" => Some(&self.text_3xl),
            "text-4xl" => Some(&self.text_4xl),
            "space-xs" => Some(&self.space_xs),
            "space-sm" => Some(&self.space_sm),
            "space-md" => Some(&self.space_md),
            "space-lg" => Some(&self.space_lg),
            "space-xl" => Some(&self.space_xl),
            "space-2xl" => Some(&self.space_2xl),
            "space-section" => Some(&self.space_section),
            "radius-sm" => Some(&self.radius_sm),
            "radius-md" => Some(&self.radius_md),
            "radius-lg" => Some(&self.radius_lg),
            "radius-xl" => Some(&self.radius_xl),
            "radius-full" => Some(&self.radius_full),
            "shadow-sm" => Some(&self.shadow_sm),
            "shadow-md" => Some(&self.shadow_md),
            "shadow-lg" => Some(&self.shadow_lg),
            "shadow-xl" => Some(&self.shadow_xl),
            "duration-fast" => Some(&self.duration_fast),
            "duration-normal" => Some(&self.duration_normal),
            "duration-slow" => Some(&self.duration_slow),
            "ease-default" => Some(&self.ease_default),
            _ => None,
        }
    }

    /// Set a token value by name. Returns error if the name is unknown.
    pub fn set(&mut self, name: &str, value: &str) -> Result<(), TokenError> {
        let field = match name {
            "color-primary" => &mut self.color_primary,
            "color-secondary" => &mut self.color_secondary,
            "color-accent" => &mut self.color_accent,
            "color-bg" => &mut self.color_bg,
            "color-bg-secondary" => &mut self.color_bg_secondary,
            "color-text" => &mut self.color_text,
            "color-text-secondary" => &mut self.color_text_secondary,
            "color-border" => &mut self.color_border,
            "font-heading" => &mut self.font_heading,
            "font-body" => &mut self.font_body,
            "font-mono" => &mut self.font_mono,
            "text-xs" => &mut self.text_xs,
            "text-sm" => &mut self.text_sm,
            "text-base" => &mut self.text_base,
            "text-lg" => &mut self.text_lg,
            "text-xl" => &mut self.text_xl,
            "text-2xl" => &mut self.text_2xl,
            "text-3xl" => &mut self.text_3xl,
            "text-4xl" => &mut self.text_4xl,
            "space-xs" => &mut self.space_xs,
            "space-sm" => &mut self.space_sm,
            "space-md" => &mut self.space_md,
            "space-lg" => &mut self.space_lg,
            "space-xl" => &mut self.space_xl,
            "space-2xl" => &mut self.space_2xl,
            "space-section" => &mut self.space_section,
            "radius-sm" => &mut self.radius_sm,
            "radius-md" => &mut self.radius_md,
            "radius-lg" => &mut self.radius_lg,
            "radius-xl" => &mut self.radius_xl,
            "radius-full" => &mut self.radius_full,
            "shadow-sm" => &mut self.shadow_sm,
            "shadow-md" => &mut self.shadow_md,
            "shadow-lg" => &mut self.shadow_lg,
            "shadow-xl" => &mut self.shadow_xl,
            "duration-fast" => &mut self.duration_fast,
            "duration-normal" => &mut self.duration_normal,
            "duration-slow" => &mut self.duration_slow,
            "ease-default" => &mut self.ease_default,
            _ => return Err(TokenError::UnknownFoundationToken(name.to_string())),
        };
        *field = value.to_string();
        Ok(())
    }

    /// Emit all tokens as CSS custom property declarations (no selector wrapper).
    fn to_css_declarations(&self) -> String {
        let mut css = String::with_capacity(2048);
        for name in FOUNDATION_TOKEN_NAMES {
            if let Some(val) = self.get(name) {
                let _ = writeln!(css, "  --{name}: {val};");
            }
        }
        css
    }
}

impl Default for FoundationTokens {
    fn default() -> Self {
        Self {
            color_primary: "#6366f1".into(),
            color_secondary: "#8b5cf6".into(),
            color_accent: "#06b6d4".into(),
            color_bg: "#0a0a0f".into(),
            color_bg_secondary: "#12121a".into(),
            color_text: "#f0f0f5".into(),
            color_text_secondary: "#8888a0".into(),
            color_border: "#1e1e2e".into(),
            font_heading: "'Inter', system-ui, sans-serif".into(),
            font_body: "'Inter', system-ui, sans-serif".into(),
            font_mono: "'JetBrains Mono', ui-monospace, monospace".into(),
            text_xs: "clamp(0.75rem, 0.7rem + 0.15vw, 0.8rem)".into(),
            text_sm: "clamp(0.875rem, 0.8rem + 0.25vw, 1rem)".into(),
            text_base: "clamp(1rem, 0.925rem + 0.3vw, 1.125rem)".into(),
            text_lg: "clamp(1.125rem, 1rem + 0.4vw, 1.25rem)".into(),
            text_xl: "clamp(1.25rem, 1.1rem + 0.5vw, 1.5rem)".into(),
            text_2xl: "clamp(1.5rem, 1.25rem + 0.75vw, 2rem)".into(),
            text_3xl: "clamp(1.875rem, 1.5rem + 1.2vw, 2.5rem)".into(),
            text_4xl: "clamp(2.25rem, 1.75rem + 1.5vw, 3.5rem)".into(),
            space_xs: "0.25rem".into(),
            space_sm: "0.5rem".into(),
            space_md: "1rem".into(),
            space_lg: "1.5rem".into(),
            space_xl: "2rem".into(),
            space_2xl: "3rem".into(),
            space_section: "4rem".into(),
            radius_sm: "0.25rem".into(),
            radius_md: "0.5rem".into(),
            radius_lg: "0.75rem".into(),
            radius_xl: "1rem".into(),
            radius_full: "9999px".into(),
            shadow_sm: "0 1px 2px rgba(0,0,0,0.05)".into(),
            shadow_md: "0 4px 6px rgba(0,0,0,0.1)".into(),
            shadow_lg: "0 10px 15px rgba(0,0,0,0.15)".into(),
            shadow_xl: "0 20px 25px rgba(0,0,0,0.2)".into(),
            duration_fast: "150ms".into(),
            duration_normal: "300ms".into(),
            duration_slow: "500ms".into(),
            ease_default: "cubic-bezier(0.4, 0, 0.2, 1)".into(),
        }
    }
}

// ─── Dark Mode Foundation Tokens ────────────────────────────────────────────

/// Dark mode overrides — only color tokens swap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DarkModeColors {
    pub color_primary: String,
    pub color_secondary: String,
    pub color_accent: String,
    pub color_bg: String,
    pub color_bg_secondary: String,
    pub color_text: String,
    pub color_text_secondary: String,
    pub color_border: String,
}

impl Default for DarkModeColors {
    fn default() -> Self {
        Self {
            color_primary: "#818cf8".into(),
            color_secondary: "#a78bfa".into(),
            color_accent: "#22d3ee".into(),
            color_bg: "#0a0a0f".into(),
            color_bg_secondary: "#12121a".into(),
            color_text: "#f0f0f5".into(),
            color_text_secondary: "#8888a0".into(),
            color_border: "#1e1e2e".into(),
        }
    }
}

impl DarkModeColors {
    fn to_css_declarations(&self) -> String {
        let mut css = String::with_capacity(512);
        let _ = writeln!(css, "  --color-primary: {};", self.color_primary);
        let _ = writeln!(css, "  --color-secondary: {};", self.color_secondary);
        let _ = writeln!(css, "  --color-accent: {};", self.color_accent);
        let _ = writeln!(css, "  --color-bg: {};", self.color_bg);
        let _ = writeln!(css, "  --color-bg-secondary: {};", self.color_bg_secondary);
        let _ = writeln!(css, "  --color-text: {};", self.color_text);
        let _ = writeln!(
            css,
            "  --color-text-secondary: {};",
            self.color_text_secondary
        );
        let _ = writeln!(css, "  --color-border: {};", self.color_border);
        css
    }
}

// ─── Semantic Tokens ────────────────────────────────────────────────────────

/// Layer 2 — component-level aliases that reference foundation tokens.
/// Each value is the foundation token name (e.g., "color-primary").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticTokens {
    pub btn_bg: String,
    pub btn_text: String,
    pub btn_border: String,
    pub btn_hover_bg: String,
    pub card_bg: String,
    pub card_border: String,
    pub card_shadow: String,
    pub hero_bg: String,
    pub hero_text: String,
    pub hero_accent: String,
    pub nav_bg: String,
    pub nav_text: String,
    pub nav_border: String,
    pub footer_bg: String,
    pub footer_text: String,
    pub section_bg: String,
    pub section_text: String,
    pub input_bg: String,
    pub input_border: String,
    pub input_text: String,
    pub badge_bg: String,
    pub badge_text: String,
}

impl Default for SemanticTokens {
    fn default() -> Self {
        Self {
            btn_bg: "color-primary".into(),
            btn_text: "color-bg".into(),
            btn_border: "color-primary".into(),
            btn_hover_bg: "color-accent".into(),
            card_bg: "color-bg-secondary".into(),
            card_border: "color-border".into(),
            card_shadow: "shadow-md".into(),
            hero_bg: "color-bg".into(),
            hero_text: "color-text".into(),
            hero_accent: "color-primary".into(),
            nav_bg: "color-bg".into(),
            nav_text: "color-text".into(),
            nav_border: "color-border".into(),
            footer_bg: "color-bg-secondary".into(),
            footer_text: "color-text-secondary".into(),
            section_bg: "color-bg".into(),
            section_text: "color-text".into(),
            input_bg: "color-bg-secondary".into(),
            input_border: "color-border".into(),
            input_text: "color-text".into(),
            badge_bg: "color-accent".into(),
            badge_text: "color-bg".into(),
        }
    }
}

impl SemanticTokens {
    /// Get the foundation token name that a semantic token aliases.
    pub fn get_alias(&self, name: &str) -> Option<&str> {
        match name {
            "btn-bg" => Some(&self.btn_bg),
            "btn-text" => Some(&self.btn_text),
            "btn-border" => Some(&self.btn_border),
            "btn-hover-bg" => Some(&self.btn_hover_bg),
            "card-bg" => Some(&self.card_bg),
            "card-border" => Some(&self.card_border),
            "card-shadow" => Some(&self.card_shadow),
            "hero-bg" => Some(&self.hero_bg),
            "hero-text" => Some(&self.hero_text),
            "hero-accent" => Some(&self.hero_accent),
            "nav-bg" => Some(&self.nav_bg),
            "nav-text" => Some(&self.nav_text),
            "nav-border" => Some(&self.nav_border),
            "footer-bg" => Some(&self.footer_bg),
            "footer-text" => Some(&self.footer_text),
            "section-bg" => Some(&self.section_bg),
            "section-text" => Some(&self.section_text),
            "input-bg" => Some(&self.input_bg),
            "input-border" => Some(&self.input_border),
            "input-text" => Some(&self.input_text),
            "badge-bg" => Some(&self.badge_bg),
            "badge-text" => Some(&self.badge_text),
            _ => None,
        }
    }

    /// Emit semantic tokens as CSS declarations using var() references.
    fn to_css_declarations(&self) -> String {
        let mut css = String::with_capacity(1024);
        let pairs: &[(&str, &str)] = &[
            ("btn-bg", &self.btn_bg),
            ("btn-text", &self.btn_text),
            ("btn-border", &self.btn_border),
            ("btn-hover-bg", &self.btn_hover_bg),
            ("card-bg", &self.card_bg),
            ("card-border", &self.card_border),
            ("card-shadow", &self.card_shadow),
            ("hero-bg", &self.hero_bg),
            ("hero-text", &self.hero_text),
            ("hero-accent", &self.hero_accent),
            ("nav-bg", &self.nav_bg),
            ("nav-text", &self.nav_text),
            ("nav-border", &self.nav_border),
            ("footer-bg", &self.footer_bg),
            ("footer-text", &self.footer_text),
            ("section-bg", &self.section_bg),
            ("section-text", &self.section_text),
            ("input-bg", &self.input_bg),
            ("input-border", &self.input_border),
            ("input-text", &self.input_text),
            ("badge-bg", &self.badge_bg),
            ("badge-text", &self.badge_text),
        ];
        for (name, alias) in pairs {
            let _ = writeln!(css, "  --{name}: var(--{alias});");
        }
        css
    }
}

// ─── Instance Overrides ─────────────────────────────────────────────────────

/// Layer 3 — scoped overrides for specific sections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceOverride {
    pub section_id: String,
    pub token_name: String,
    pub value: String,
}

// ─── TokenSet ───────────────────────────────────────────────────────────────

/// Complete 3-layer token system.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenSet {
    pub foundation: FoundationTokens,
    pub dark_mode: DarkModeColors,
    pub semantic: SemanticTokens,
    pub overrides: Vec<InstanceOverride>,
}

impl TokenSet {
    /// Render the full CSS block: :root + semantic + dark mode + reduced motion + overrides.
    pub fn to_css(&self) -> String {
        let mut css = String::with_capacity(4096);

        // Layer 1 + 2 inside :root
        let _ = writeln!(css, ":root {{");
        let _ = writeln!(css, "  color-scheme: light dark;");
        css.push_str(&self.foundation.to_css_declarations());
        let _ = writeln!(css);
        let _ = writeln!(css, "  /* Semantic tokens */");
        css.push_str(&self.semantic.to_css_declarations());
        let _ = writeln!(css, "}}");

        // Dark mode
        let _ = writeln!(css);
        let _ = writeln!(css, "@media (prefers-color-scheme: dark) {{");
        let _ = writeln!(css, "  :root {{");
        css.push_str(
            &self
                .dark_mode
                .to_css_declarations()
                .lines()
                .map(|l| format!("  {l}\n"))
                .collect::<String>(),
        );
        let _ = writeln!(css, "  }}");
        let _ = writeln!(css, "}}");

        // Reduced motion
        let _ = writeln!(css);
        let _ = writeln!(css, "@media (prefers-reduced-motion: reduce) {{");
        let _ = writeln!(css, "  :root {{");
        let _ = writeln!(css, "    --duration-fast: 0ms;");
        let _ = writeln!(css, "    --duration-normal: 0ms;");
        let _ = writeln!(css, "    --duration-slow: 0ms;");
        let _ = writeln!(css, "  }}");
        let _ = writeln!(css, "}}");

        // Layer 3 — instance overrides
        if !self.overrides.is_empty() {
            let _ = writeln!(css);
            let _ = writeln!(css, "/* Instance overrides */");
            for ovr in &self.overrides {
                let _ = writeln!(css, "[data-nexus-section=\"{}\"] {{", ovr.section_id);
                let _ = writeln!(css, "  --{}: {};", ovr.token_name, ovr.value);
                let _ = writeln!(css, "}}");
            }
        }

        css
    }

    /// Resolve a token through the chain: instance override → semantic → foundation.
    /// Pass `section_id` to check instance overrides for that section.
    pub fn resolve(&self, token_name: &str) -> Option<String> {
        self.resolve_for_section(token_name, None)
    }

    /// Resolve a token with optional section context for instance overrides.
    pub fn resolve_for_section(
        &self,
        token_name: &str,
        section_id: Option<&str>,
    ) -> Option<String> {
        // Layer 3: check instance overrides first
        if let Some(sid) = section_id {
            for ovr in &self.overrides {
                if ovr.section_id == sid && ovr.token_name == token_name {
                    return Some(ovr.value.clone());
                }
            }
        }

        // Layer 2: check semantic aliases
        if let Some(alias) = self.semantic.get_alias(token_name) {
            // The alias points to a foundation token
            if let Some(val) = self.foundation.get(alias) {
                return Some(val.to_string());
            }
        }

        // Layer 1: direct foundation lookup
        self.foundation.get(token_name).map(|v| v.to_string())
    }

    /// Update a Layer 1 foundation token. Returns error if the name is unknown.
    pub fn set_foundation(&mut self, name: &str, value: &str) -> Result<(), TokenError> {
        self.foundation.set(name, value)
    }

    /// Add or update a Layer 3 instance override.
    pub fn set_override(&mut self, section_id: &str, token_name: &str, value: &str) {
        // Update existing override if present
        for ovr in &mut self.overrides {
            if ovr.section_id == section_id && ovr.token_name == token_name {
                ovr.value = value.to_string();
                return;
            }
        }
        // Otherwise add new
        self.overrides.push(InstanceOverride {
            section_id: section_id.to_string(),
            token_name: token_name.to_string(),
            value: value.to_string(),
        });
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_css_produces_valid_three_layers() {
        let ts = TokenSet::default();
        let css = ts.to_css();
        // Layer 1
        assert!(css.contains(":root {"), "missing :root block");
        assert!(css.contains("color-scheme: light dark;"));
        assert!(css.contains("--color-primary:"));
        assert!(css.contains("--font-heading:"));
        assert!(css.contains("--text-4xl:"));
        assert!(css.contains("--space-section:"));
        assert!(css.contains("--shadow-xl:"));
        assert!(css.contains("--duration-fast:"));
        // Layer 2
        assert!(
            css.contains("--btn-bg: var(--color-primary);"),
            "missing semantic alias"
        );
        assert!(css.contains("--card-bg: var(--color-bg-secondary);"));
        assert!(css.contains("--hero-text: var(--color-text);"));
        // Dark mode
        assert!(css.contains("prefers-color-scheme: dark"));
        // Reduced motion
        assert!(css.contains("prefers-reduced-motion: reduce"));
    }

    #[test]
    fn test_to_css_with_instance_overrides() {
        let mut ts = TokenSet::default();
        ts.set_override("hero", "hero-bg", "#0f172a");
        let css = ts.to_css();
        assert!(css.contains("[data-nexus-section=\"hero\"]"));
        assert!(css.contains("--hero-bg: #0f172a;"));
    }

    #[test]
    fn test_resolve_foundation_direct() {
        let ts = TokenSet::default();
        let val = ts.resolve("color-primary").unwrap();
        assert_eq!(val, "#6366f1");
    }

    #[test]
    fn test_resolve_semantic_chain() {
        let ts = TokenSet::default();
        // btn-bg → color-primary → #6366f1
        let val = ts.resolve("btn-bg").unwrap();
        assert_eq!(val, "#6366f1");
    }

    #[test]
    fn test_resolve_instance_override_wins() {
        let mut ts = TokenSet::default();
        ts.set_override("hero", "hero-bg", "#ff0000");
        // With section context, override wins
        let val = ts.resolve_for_section("hero-bg", Some("hero")).unwrap();
        assert_eq!(val, "#ff0000");
        // Without section context, falls through to semantic → foundation
        let val_no_section = ts.resolve("hero-bg").unwrap();
        assert_eq!(val_no_section, ts.foundation.get("color-bg").unwrap());
    }

    #[test]
    fn test_set_foundation_rejects_unknown() {
        let mut ts = TokenSet::default();
        let result = ts.set_foundation("nonexistent-token", "#fff");
        assert!(result.is_err());
        match result.unwrap_err() {
            TokenError::UnknownFoundationToken(name) => assert_eq!(name, "nonexistent-token"),
            other => panic!("expected UnknownFoundationToken, got: {other}"),
        }
    }

    #[test]
    fn test_set_foundation_valid() {
        let mut ts = TokenSet::default();
        ts.set_foundation("color-primary", "#ff00ff").unwrap();
        assert_eq!(ts.foundation.get("color-primary").unwrap(), "#ff00ff");
    }

    #[test]
    fn test_dark_mode_css_swaps_colors() {
        let mut ts = TokenSet::default();
        ts.dark_mode.color_bg = "#000000".into();
        ts.dark_mode.color_text = "#ffffff".into();
        let css = ts.to_css();
        // Ensure dark mode block has the swapped values
        let dark_block_start = css.find("prefers-color-scheme: dark").unwrap();
        let dark_section = &css[dark_block_start..];
        assert!(dark_section.contains("--color-bg: #000000;"));
        assert!(dark_section.contains("--color-text: #ffffff;"));
    }

    #[test]
    fn test_reduced_motion_zeros_durations() {
        let ts = TokenSet::default();
        let css = ts.to_css();
        let rm_start = css.find("prefers-reduced-motion: reduce").unwrap();
        let rm_section = &css[rm_start..];
        assert!(rm_section.contains("--duration-fast: 0ms;"));
        assert!(rm_section.contains("--duration-normal: 0ms;"));
        assert!(rm_section.contains("--duration-slow: 0ms;"));
    }

    #[test]
    fn test_set_override_updates_existing() {
        let mut ts = TokenSet::default();
        ts.set_override("hero", "hero-bg", "#111");
        ts.set_override("hero", "hero-bg", "#222");
        assert_eq!(
            ts.overrides
                .iter()
                .filter(|o| o.section_id == "hero" && o.token_name == "hero-bg")
                .count(),
            1,
            "should update in-place, not duplicate"
        );
        assert_eq!(ts.overrides[0].value, "#222");
    }

    #[test]
    fn test_resolve_unknown_returns_none() {
        let ts = TokenSet::default();
        assert!(ts.resolve("totally-fake").is_none());
    }
}
