//! Token → Tailwind Compiler — transforms TokenSet into Tailwind config + CSS.
//!
//! Foundation tokens become CSS custom properties in `index.css`.
//! The Tailwind config maps utility classes to `var(--token-name)` references
//! so all styling flows from a single source of truth.

use crate::tokens::TokenSet;

// ─── Tailwind Config Generation ─────────────────────────────────────────────

/// Generate a complete `tailwind.config.ts` that maps tokens to CSS custom properties.
pub fn token_set_to_tailwind_config(token_set: &TokenSet) -> String {
    // Use _ prefix to avoid unused-variable warning while still documenting intent
    let _ts = token_set;
    let mut cfg = String::with_capacity(2048);
    cfg.push_str("import type { Config } from 'tailwindcss'\n\n");
    cfg.push_str("export default {\n");
    cfg.push_str("  content: ['./index.html', './src/**/*.{ts,tsx}'],\n");
    cfg.push_str("  darkMode: 'media',\n");
    cfg.push_str("  theme: {\n");
    cfg.push_str("    extend: {\n");

    // Colors — reference CSS custom properties
    cfg.push_str("      colors: {\n");
    cfg.push_str("        primary: 'var(--color-primary)',\n");
    cfg.push_str("        secondary: 'var(--color-secondary)',\n");
    cfg.push_str("        accent: 'var(--color-accent)',\n");
    cfg.push_str("        bg: 'var(--color-bg)',\n");
    cfg.push_str("        'bg-secondary': 'var(--color-bg-secondary)',\n");
    cfg.push_str("        'text-primary': 'var(--color-text)',\n");
    cfg.push_str("        'text-secondary': 'var(--color-text-secondary)',\n");
    cfg.push_str("        border: 'var(--color-border)',\n");
    // Semantic colors
    cfg.push_str("        'btn-bg': 'var(--btn-bg)',\n");
    cfg.push_str("        'btn-text': 'var(--btn-text)',\n");
    cfg.push_str("        'card-bg': 'var(--card-bg)',\n");
    cfg.push_str("        'card-border': 'var(--card-border)',\n");
    cfg.push_str("        'hero-bg': 'var(--hero-bg)',\n");
    cfg.push_str("        'hero-text': 'var(--hero-text)',\n");
    cfg.push_str("        'nav-bg': 'var(--nav-bg)',\n");
    cfg.push_str("        'nav-text': 'var(--nav-text)',\n");
    cfg.push_str("        'footer-bg': 'var(--footer-bg)',\n");
    cfg.push_str("        'footer-text': 'var(--footer-text)',\n");
    cfg.push_str("        'section-bg': 'var(--section-bg)',\n");
    cfg.push_str("        'section-text': 'var(--section-text)',\n");
    cfg.push_str("      },\n");

    // Font families
    cfg.push_str("      fontFamily: {\n");
    cfg.push_str("        heading: 'var(--font-heading)',\n");
    cfg.push_str("        body: 'var(--font-body)',\n");
    cfg.push_str("        mono: 'var(--font-mono)',\n");
    cfg.push_str("      },\n");

    // Font sizes (type scale)
    cfg.push_str("      fontSize: {\n");
    cfg.push_str("        xs: 'var(--text-xs)',\n");
    cfg.push_str("        sm: 'var(--text-sm)',\n");
    cfg.push_str("        base: 'var(--text-base)',\n");
    cfg.push_str("        lg: 'var(--text-lg)',\n");
    cfg.push_str("        xl: 'var(--text-xl)',\n");
    cfg.push_str("        '2xl': 'var(--text-2xl)',\n");
    cfg.push_str("        '3xl': 'var(--text-3xl)',\n");
    cfg.push_str("        '4xl': 'var(--text-4xl)',\n");
    cfg.push_str("      },\n");

    // Border radius
    cfg.push_str("      borderRadius: {\n");
    cfg.push_str("        sm: 'var(--radius-sm)',\n");
    cfg.push_str("        md: 'var(--radius-md)',\n");
    cfg.push_str("        lg: 'var(--radius-lg)',\n");
    cfg.push_str("        xl: 'var(--radius-xl)',\n");
    cfg.push_str("        full: 'var(--radius-full)',\n");
    cfg.push_str("      },\n");

    // Spacing
    cfg.push_str("      spacing: {\n");
    cfg.push_str("        xs: 'var(--space-xs)',\n");
    cfg.push_str("        sm: 'var(--space-sm)',\n");
    cfg.push_str("        md: 'var(--space-md)',\n");
    cfg.push_str("        lg: 'var(--space-lg)',\n");
    cfg.push_str("        xl: 'var(--space-xl)',\n");
    cfg.push_str("        '2xl': 'var(--space-2xl)',\n");
    cfg.push_str("        section: 'var(--space-section)',\n");
    cfg.push_str("      },\n");

    // Shadows
    cfg.push_str("      boxShadow: {\n");
    cfg.push_str("        sm: 'var(--shadow-sm)',\n");
    cfg.push_str("        md: 'var(--shadow-md)',\n");
    cfg.push_str("        lg: 'var(--shadow-lg)',\n");
    cfg.push_str("        xl: 'var(--shadow-xl)',\n");
    cfg.push_str("      },\n");

    // Transition durations
    cfg.push_str("      transitionDuration: {\n");
    cfg.push_str("        fast: 'var(--duration-fast)',\n");
    cfg.push_str("        normal: 'var(--duration-normal)',\n");
    cfg.push_str("        slow: 'var(--duration-slow)',\n");
    cfg.push_str("      },\n");

    cfg.push_str("    },\n");
    cfg.push_str("  },\n");
    cfg.push_str("  plugins: [],\n");
    cfg.push_str("} satisfies Config\n");

    cfg
}

// ─── Index CSS Generation ───────────────────────────────────────────────────

/// Generate `src/index.css` with Tailwind imports + token CSS custom properties.
pub fn token_set_to_index_css(token_set: &TokenSet) -> String {
    let mut css = String::with_capacity(4096);

    // Tailwind imports
    css.push_str("@tailwind base;\n");
    css.push_str("@tailwind components;\n");
    css.push_str("@tailwind utilities;\n\n");

    // Token CSS from the 3-layer system
    css.push_str(&token_set.to_css());

    // Base styles
    css.push_str("\n/* Base styles */\n");
    css.push_str("body {\n");
    css.push_str("  font-family: var(--font-body);\n");
    css.push_str("  font-size: var(--text-base);\n");
    css.push_str("  color: var(--color-text);\n");
    css.push_str("  background-color: var(--color-bg);\n");
    css.push_str("  line-height: 1.6;\n");
    css.push_str("  -webkit-font-smoothing: antialiased;\n");
    css.push_str("  -moz-osx-font-smoothing: grayscale;\n");
    css.push_str("}\n\n");

    css.push_str("h1, h2, h3, h4, h5, h6 {\n");
    css.push_str("  font-family: var(--font-heading);\n");
    css.push_str("  line-height: 1.2;\n");
    css.push_str("}\n");

    css
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_ts() -> TokenSet {
        TokenSet::default()
    }

    #[test]
    fn test_tailwind_config_maps_all_colors() {
        let cfg = token_set_to_tailwind_config(&default_ts());
        assert!(cfg.contains("primary: 'var(--color-primary)'"));
        assert!(cfg.contains("secondary: 'var(--color-secondary)'"));
        assert!(cfg.contains("accent: 'var(--color-accent)'"));
        assert!(cfg.contains("bg: 'var(--color-bg)'"));
        assert!(cfg.contains("'text-primary': 'var(--color-text)'"));
        assert!(cfg.contains("'text-secondary': 'var(--color-text-secondary)'"));
        assert!(cfg.contains("border: 'var(--color-border)'"));
    }

    #[test]
    fn test_tailwind_config_maps_fonts() {
        let cfg = token_set_to_tailwind_config(&default_ts());
        assert!(cfg.contains("heading: 'var(--font-heading)'"));
        assert!(cfg.contains("body: 'var(--font-body)'"));
        assert!(cfg.contains("mono: 'var(--font-mono)'"));
    }

    #[test]
    fn test_tailwind_config_maps_radii() {
        let cfg = token_set_to_tailwind_config(&default_ts());
        assert!(cfg.contains("sm: 'var(--radius-sm)'"));
        assert!(cfg.contains("md: 'var(--radius-md)'"));
        assert!(cfg.contains("lg: 'var(--radius-lg)'"));
        assert!(cfg.contains("full: 'var(--radius-full)'"));
    }

    #[test]
    fn test_tailwind_config_is_valid_typescript() {
        let cfg = token_set_to_tailwind_config(&default_ts());
        assert!(cfg.starts_with("import type"));
        assert!(cfg.contains("export default"));
        assert!(cfg.contains("satisfies Config"));
    }

    #[test]
    fn test_index_css_has_tailwind_imports() {
        let css = token_set_to_index_css(&default_ts());
        assert!(css.contains("@tailwind base;"));
        assert!(css.contains("@tailwind components;"));
        assert!(css.contains("@tailwind utilities;"));
    }

    #[test]
    fn test_index_css_has_token_variables() {
        let css = token_set_to_index_css(&default_ts());
        assert!(css.contains("--color-primary:"));
        assert!(css.contains("--font-heading:"));
        assert!(css.contains("--text-4xl:"));
        assert!(css.contains("--space-section:"));
        assert!(css.contains("--shadow-xl:"));
        assert!(css.contains("--duration-fast:"));
    }

    #[test]
    fn test_index_css_dark_mode() {
        let css = token_set_to_index_css(&default_ts());
        assert!(css.contains("prefers-color-scheme: dark"));
    }

    #[test]
    fn test_index_css_reduced_motion() {
        let css = token_set_to_index_css(&default_ts());
        assert!(css.contains("prefers-reduced-motion: reduce"));
    }

    #[test]
    fn test_tailwind_config_maps_spacing() {
        let cfg = token_set_to_tailwind_config(&default_ts());
        assert!(cfg.contains("xs: 'var(--space-xs)'"));
        assert!(cfg.contains("section: 'var(--space-section)'"));
    }

    #[test]
    fn test_tailwind_config_maps_shadows() {
        let cfg = token_set_to_tailwind_config(&default_ts());
        assert!(cfg.contains("sm: 'var(--shadow-sm)'"));
        assert!(cfg.contains("xl: 'var(--shadow-xl)'"));
    }
}
