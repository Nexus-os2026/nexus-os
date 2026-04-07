//! Variant Contract — palette, typography, layout, and motion presets.
//!
//! Defines the combinatoric space of visual variants: 6 templates × 4 palettes ×
//! 4 typography × 2-3 layouts × 3 motion = 430+ unique combinations.

use crate::tokens::{DarkModeColors, FoundationTokens, TokenSet};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Palette Presets ────────────────────────────────────────────────────────

/// A complete set of Layer 1 color tokens for light and dark modes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PalettePreset {
    pub id: &'static str,
    pub name: &'static str,
    pub template_id: &'static str,
    pub light: PaletteColors,
    pub dark: PaletteColors,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaletteColors {
    pub primary: &'static str,
    pub secondary: &'static str,
    pub accent: &'static str,
    pub bg: &'static str,
    pub bg_secondary: &'static str,
    pub text: &'static str,
    pub text_secondary: &'static str,
    pub border: &'static str,
}

/// All 8 required color token names in a palette.
pub const PALETTE_COLOR_KEYS: &[&str] = &[
    "primary",
    "secondary",
    "accent",
    "bg",
    "bg_secondary",
    "text",
    "text_secondary",
    "border",
];

impl PaletteColors {
    /// Get a color value by key name.
    pub fn get(&self, key: &str) -> Option<&'static str> {
        match key {
            "primary" => Some(self.primary),
            "secondary" => Some(self.secondary),
            "accent" => Some(self.accent),
            "bg" => Some(self.bg),
            "bg_secondary" | "bg-secondary" => Some(self.bg_secondary),
            "text" => Some(self.text),
            "text_secondary" | "text-secondary" => Some(self.text_secondary),
            "border" => Some(self.border),
            _ => None,
        }
    }
}

// 24 palettes: 4 per template
static PALETTE_PRESETS: &[PalettePreset] = &[
    // ── saas_landing ────────────────────────────────────────────────────
    PalettePreset {
        id: "saas_midnight",
        name: "Midnight",
        template_id: "saas_landing",
        light: PaletteColors {
            primary: "#4f46e5",
            secondary: "#7c3aed",
            accent: "#06b6d4",
            bg: "#f8fafc",
            bg_secondary: "#f1f5f9",
            text: "#0f172a",
            text_secondary: "#475569",
            border: "#e2e8f0",
        },
        dark: PaletteColors {
            primary: "#818cf8",
            secondary: "#a78bfa",
            accent: "#22d3ee",
            bg: "#0a0a0f",
            bg_secondary: "#12121a",
            text: "#f0f0f5",
            text_secondary: "#8888a0",
            border: "#1e1e2e",
        },
    },
    PalettePreset {
        id: "saas_ocean",
        name: "Ocean",
        template_id: "saas_landing",
        light: PaletteColors {
            primary: "#0284c7",
            secondary: "#0369a1",
            accent: "#14b8a6",
            bg: "#f0f9ff",
            bg_secondary: "#e0f2fe",
            text: "#082f49",
            text_secondary: "#475569",
            border: "#bae6fd",
        },
        dark: PaletteColors {
            primary: "#38bdf8",
            secondary: "#7dd3fc",
            accent: "#2dd4bf",
            bg: "#020617",
            bg_secondary: "#0f172a",
            text: "#e2e8f0",
            text_secondary: "#94a3b8",
            border: "#1e293b",
        },
    },
    PalettePreset {
        id: "saas_forest",
        name: "Forest",
        template_id: "saas_landing",
        light: PaletteColors {
            primary: "#16a34a",
            secondary: "#15803d",
            accent: "#d97706",
            bg: "#f0fdf4",
            bg_secondary: "#dcfce7",
            text: "#14532d",
            text_secondary: "#4b5563",
            border: "#bbf7d0",
        },
        dark: PaletteColors {
            primary: "#4ade80",
            secondary: "#86efac",
            accent: "#fbbf24",
            bg: "#0c1a0c",
            bg_secondary: "#142014",
            text: "#e8f0e8",
            text_secondary: "#7a9a7a",
            border: "#1e2e1e",
        },
    },
    PalettePreset {
        id: "saas_dawn",
        name: "Dawn",
        template_id: "saas_landing",
        light: PaletteColors {
            primary: "#e11d48",
            secondary: "#be123c",
            accent: "#f59e0b",
            bg: "#fff1f2",
            bg_secondary: "#ffe4e6",
            text: "#1c1917",
            text_secondary: "#57534e",
            border: "#fecdd3",
        },
        dark: PaletteColors {
            primary: "#fb7185",
            secondary: "#fda4af",
            accent: "#fbbf24",
            bg: "#1a0a10",
            bg_secondary: "#2d1520",
            text: "#fce7f3",
            text_secondary: "#d4a0b0",
            border: "#3d1f2e",
        },
    },
    // ── docs_site ───────────────────────────────────────────────────────
    PalettePreset {
        id: "docs_clean",
        name: "Clean",
        template_id: "docs_site",
        light: PaletteColors {
            primary: "#2563eb",
            secondary: "#3b82f6",
            accent: "#8b5cf6",
            bg: "#ffffff",
            bg_secondary: "#f8fafc",
            text: "#1e293b",
            text_secondary: "#64748b",
            border: "#e2e8f0",
        },
        dark: PaletteColors {
            primary: "#60a5fa",
            secondary: "#93c5fd",
            accent: "#a78bfa",
            bg: "#0f172a",
            bg_secondary: "#1e293b",
            text: "#f1f5f9",
            text_secondary: "#94a3b8",
            border: "#334155",
        },
    },
    PalettePreset {
        id: "docs_github",
        name: "GitHub",
        template_id: "docs_site",
        light: PaletteColors {
            primary: "#1f6feb",
            secondary: "#388bfd",
            accent: "#f78166",
            bg: "#ffffff",
            bg_secondary: "#f6f8fa",
            text: "#1f2328",
            text_secondary: "#656d76",
            border: "#d0d7de",
        },
        dark: PaletteColors {
            primary: "#58a6ff",
            secondary: "#79c0ff",
            accent: "#ffa657",
            bg: "#0d1117",
            bg_secondary: "#161b22",
            text: "#e6edf3",
            text_secondary: "#8b949e",
            border: "#30363d",
        },
    },
    PalettePreset {
        id: "docs_sepia",
        name: "Sepia",
        template_id: "docs_site",
        light: PaletteColors {
            primary: "#92400e",
            secondary: "#b45309",
            accent: "#059669",
            bg: "#fefce8",
            bg_secondary: "#fef9c3",
            text: "#422006",
            text_secondary: "#78716c",
            border: "#e5e0d0",
        },
        dark: PaletteColors {
            primary: "#fbbf24",
            secondary: "#fcd34d",
            accent: "#34d399",
            bg: "#1c1a10",
            bg_secondary: "#292518",
            text: "#fef3c7",
            text_secondary: "#a8a29e",
            border: "#44403c",
        },
    },
    PalettePreset {
        id: "docs_nord",
        name: "Nord",
        template_id: "docs_site",
        light: PaletteColors {
            primary: "#5e81ac",
            secondary: "#81a1c1",
            accent: "#bf616a",
            bg: "#eceff4",
            bg_secondary: "#e5e9f0",
            text: "#2e3440",
            text_secondary: "#4c566a",
            border: "#d8dee9",
        },
        dark: PaletteColors {
            primary: "#88c0d0",
            secondary: "#81a1c1",
            accent: "#bf616a",
            bg: "#2e3440",
            bg_secondary: "#3b4252",
            text: "#eceff4",
            text_secondary: "#d8dee9",
            border: "#434c5e",
        },
    },
    // ── portfolio ───────────────────────────────────────────────────────
    PalettePreset {
        id: "port_monochrome",
        name: "Monochrome",
        template_id: "portfolio",
        light: PaletteColors {
            primary: "#18181b",
            secondary: "#3f3f46",
            accent: "#a1a1aa",
            bg: "#fafafa",
            bg_secondary: "#f4f4f5",
            text: "#18181b",
            text_secondary: "#71717a",
            border: "#e4e4e7",
        },
        dark: PaletteColors {
            primary: "#fafafa",
            secondary: "#d4d4d8",
            accent: "#71717a",
            bg: "#09090b",
            bg_secondary: "#18181b",
            text: "#fafafa",
            text_secondary: "#a1a1aa",
            border: "#27272a",
        },
    },
    PalettePreset {
        id: "port_creative",
        name: "Creative",
        template_id: "portfolio",
        light: PaletteColors {
            primary: "#db2777",
            secondary: "#9333ea",
            accent: "#f59e0b",
            bg: "#fdf2f8",
            bg_secondary: "#fce7f3",
            text: "#1e1b4b",
            text_secondary: "#6b7280",
            border: "#fbcfe8",
        },
        dark: PaletteColors {
            primary: "#f472b6",
            secondary: "#c084fc",
            accent: "#fbbf24",
            bg: "#1a0a1e",
            bg_secondary: "#2d152e",
            text: "#f5f3ff",
            text_secondary: "#c4b5fd",
            border: "#3d1f3e",
        },
    },
    PalettePreset {
        id: "port_earth",
        name: "Earth",
        template_id: "portfolio",
        light: PaletteColors {
            primary: "#854d0e",
            secondary: "#a16207",
            accent: "#15803d",
            bg: "#fefce8",
            bg_secondary: "#fef9c3",
            text: "#1c1917",
            text_secondary: "#78716c",
            border: "#e7e5e4",
        },
        dark: PaletteColors {
            primary: "#eab308",
            secondary: "#facc15",
            accent: "#4ade80",
            bg: "#1a1810",
            bg_secondary: "#292518",
            text: "#fef3c7",
            text_secondary: "#a8a29e",
            border: "#44403c",
        },
    },
    PalettePreset {
        id: "port_frost",
        name: "Frost",
        template_id: "portfolio",
        light: PaletteColors {
            primary: "#0891b2",
            secondary: "#0e7490",
            accent: "#7c3aed",
            bg: "#ecfeff",
            bg_secondary: "#cffafe",
            text: "#164e63",
            text_secondary: "#475569",
            border: "#a5f3fc",
        },
        dark: PaletteColors {
            primary: "#22d3ee",
            secondary: "#67e8f9",
            accent: "#a78bfa",
            bg: "#042f2e",
            bg_secondary: "#083344",
            text: "#ecfeff",
            text_secondary: "#a5f3fc",
            border: "#155e75",
        },
    },
    // ── local_business ──────────────────────────────────────────────────
    PalettePreset {
        id: "biz_warm",
        name: "Warm",
        template_id: "local_business",
        light: PaletteColors {
            primary: "#c2410c",
            secondary: "#ea580c",
            accent: "#16a34a",
            bg: "#faf6f1",
            bg_secondary: "#fff8f0",
            text: "#1a1410",
            text_secondary: "#6b5e50",
            border: "#e5ddd3",
        },
        dark: PaletteColors {
            primary: "#fb923c",
            secondary: "#fdba74",
            accent: "#4ade80",
            bg: "#1c1210",
            bg_secondary: "#2a1c18",
            text: "#fde8d8",
            text_secondary: "#b8976e",
            border: "#3a2820",
        },
    },
    PalettePreset {
        id: "biz_rustic",
        name: "Rustic",
        template_id: "local_business",
        light: PaletteColors {
            primary: "#92400e",
            secondary: "#78350f",
            accent: "#b91c1c",
            bg: "#fefce8",
            bg_secondary: "#fef3c7",
            text: "#422006",
            text_secondary: "#78716c",
            border: "#d6d3d1",
        },
        dark: PaletteColors {
            primary: "#f59e0b",
            secondary: "#fbbf24",
            accent: "#ef4444",
            bg: "#1a1610",
            bg_secondary: "#292218",
            text: "#fef3c7",
            text_secondary: "#a8a29e",
            border: "#44403c",
        },
    },
    PalettePreset {
        id: "biz_fresh",
        name: "Fresh",
        template_id: "local_business",
        light: PaletteColors {
            primary: "#059669",
            secondary: "#0d9488",
            accent: "#e11d48",
            bg: "#f0fdf4",
            bg_secondary: "#ecfdf5",
            text: "#064e3b",
            text_secondary: "#4b5563",
            border: "#a7f3d0",
        },
        dark: PaletteColors {
            primary: "#34d399",
            secondary: "#2dd4bf",
            accent: "#fb7185",
            bg: "#0c1a14",
            bg_secondary: "#142018",
            text: "#d1fae5",
            text_secondary: "#6ee7b7",
            border: "#1e3028",
        },
    },
    PalettePreset {
        id: "biz_classic",
        name: "Classic",
        template_id: "local_business",
        light: PaletteColors {
            primary: "#1e40af",
            secondary: "#1d4ed8",
            accent: "#dc2626",
            bg: "#ffffff",
            bg_secondary: "#f8fafc",
            text: "#1e293b",
            text_secondary: "#64748b",
            border: "#e2e8f0",
        },
        dark: PaletteColors {
            primary: "#60a5fa",
            secondary: "#93c5fd",
            accent: "#f87171",
            bg: "#0f172a",
            bg_secondary: "#1e293b",
            text: "#f1f5f9",
            text_secondary: "#94a3b8",
            border: "#334155",
        },
    },
    // ── ecommerce ───────────────────────────────────────────────────────
    PalettePreset {
        id: "ecom_luxe",
        name: "Luxe",
        template_id: "ecommerce",
        light: PaletteColors {
            primary: "#1c1917",
            secondary: "#44403c",
            accent: "#b45309",
            bg: "#fafaf9",
            bg_secondary: "#f5f5f4",
            text: "#1c1917",
            text_secondary: "#57534e",
            border: "#e7e5e4",
        },
        dark: PaletteColors {
            primary: "#fafaf9",
            secondary: "#d6d3d1",
            accent: "#f59e0b",
            bg: "#0c0a09",
            bg_secondary: "#1c1917",
            text: "#fafaf9",
            text_secondary: "#a8a29e",
            border: "#292524",
        },
    },
    PalettePreset {
        id: "ecom_vibrant",
        name: "Vibrant",
        template_id: "ecommerce",
        light: PaletteColors {
            primary: "#7c3aed",
            secondary: "#6d28d9",
            accent: "#f43f5e",
            bg: "#faf5ff",
            bg_secondary: "#f5f3ff",
            text: "#1e1b4b",
            text_secondary: "#6b7280",
            border: "#e9d5ff",
        },
        dark: PaletteColors {
            primary: "#a78bfa",
            secondary: "#c4b5fd",
            accent: "#fb7185",
            bg: "#0f0720",
            bg_secondary: "#1e1040",
            text: "#f5f3ff",
            text_secondary: "#c4b5fd",
            border: "#2e1065",
        },
    },
    PalettePreset {
        id: "ecom_natural",
        name: "Natural",
        template_id: "ecommerce",
        light: PaletteColors {
            primary: "#65a30d",
            secondary: "#4d7c0f",
            accent: "#ea580c",
            bg: "#fefce8",
            bg_secondary: "#f7fee7",
            text: "#1a2e05",
            text_secondary: "#4b5563",
            border: "#d9f99d",
        },
        dark: PaletteColors {
            primary: "#a3e635",
            secondary: "#bef264",
            accent: "#fb923c",
            bg: "#0f1a05",
            bg_secondary: "#1a2e0a",
            text: "#f7fee7",
            text_secondary: "#bef264",
            border: "#365314",
        },
    },
    PalettePreset {
        id: "ecom_minimal",
        name: "Minimal",
        template_id: "ecommerce",
        light: PaletteColors {
            primary: "#0f172a",
            secondary: "#334155",
            accent: "#0ea5e9",
            bg: "#ffffff",
            bg_secondary: "#f8fafc",
            text: "#0f172a",
            text_secondary: "#64748b",
            border: "#e2e8f0",
        },
        dark: PaletteColors {
            primary: "#f1f5f9",
            secondary: "#cbd5e1",
            accent: "#38bdf8",
            bg: "#020617",
            bg_secondary: "#0f172a",
            text: "#f1f5f9",
            text_secondary: "#94a3b8",
            border: "#1e293b",
        },
    },
    // ── dashboard ───────────────────────────────────────────────────────
    PalettePreset {
        id: "dash_pro",
        name: "Pro",
        template_id: "dashboard",
        light: PaletteColors {
            primary: "#3b82f6",
            secondary: "#2563eb",
            accent: "#f59e0b",
            bg: "#f1f5f9",
            bg_secondary: "#ffffff",
            text: "#0f172a",
            text_secondary: "#475569",
            border: "#cbd5e1",
        },
        dark: PaletteColors {
            primary: "#60a5fa",
            secondary: "#93c5fd",
            accent: "#fbbf24",
            bg: "#0f172a",
            bg_secondary: "#1e293b",
            text: "#f1f5f9",
            text_secondary: "#94a3b8",
            border: "#334155",
        },
    },
    PalettePreset {
        id: "dash_cyber",
        name: "Cyber",
        template_id: "dashboard",
        light: PaletteColors {
            primary: "#06b6d4",
            secondary: "#0891b2",
            accent: "#f43f5e",
            bg: "#f0fdfa",
            bg_secondary: "#ccfbf1",
            text: "#134e4a",
            text_secondary: "#475569",
            border: "#99f6e4",
        },
        dark: PaletteColors {
            primary: "#22d3ee",
            secondary: "#67e8f9",
            accent: "#fb7185",
            bg: "#07010f",
            bg_secondary: "#120024",
            text: "#e8fffe",
            text_secondary: "#9ca0b0",
            border: "#1e0040",
        },
    },
    PalettePreset {
        id: "dash_slate",
        name: "Slate",
        template_id: "dashboard",
        light: PaletteColors {
            primary: "#6366f1",
            secondary: "#4f46e5",
            accent: "#10b981",
            bg: "#f8fafc",
            bg_secondary: "#ffffff",
            text: "#1e293b",
            text_secondary: "#64748b",
            border: "#e2e8f0",
        },
        dark: PaletteColors {
            primary: "#818cf8",
            secondary: "#a5b4fc",
            accent: "#34d399",
            bg: "#0f172a",
            bg_secondary: "#1e293b",
            text: "#e2e8f0",
            text_secondary: "#94a3b8",
            border: "#334155",
        },
    },
    PalettePreset {
        id: "dash_ember",
        name: "Ember",
        template_id: "dashboard",
        light: PaletteColors {
            primary: "#dc2626",
            secondary: "#b91c1c",
            accent: "#2563eb",
            bg: "#fff7ed",
            bg_secondary: "#ffffff",
            text: "#1c1917",
            text_secondary: "#57534e",
            border: "#e7e5e4",
        },
        dark: PaletteColors {
            primary: "#f87171",
            secondary: "#fca5a5",
            accent: "#60a5fa",
            bg: "#1c1210",
            bg_secondary: "#2a1c18",
            text: "#fde8d8",
            text_secondary: "#b8976e",
            border: "#3a2820",
        },
    },
];

/// Get all 24 palette presets.
pub fn all_palette_presets() -> &'static [PalettePreset] {
    PALETTE_PRESETS
}

/// Get the 4 palette presets for a specific template.
pub fn palettes_for_template(template_id: &str) -> Vec<&'static PalettePreset> {
    PALETTE_PRESETS
        .iter()
        .filter(|p| p.template_id == template_id)
        .collect()
}

// ─── Typography Presets ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypographyPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub font_heading: &'static str,
    pub font_body: &'static str,
    pub font_mono: &'static str,
    pub text_xs: &'static str,
    pub text_sm: &'static str,
    pub text_base: &'static str,
    pub text_lg: &'static str,
    pub text_xl: &'static str,
    pub text_2xl: &'static str,
    pub text_3xl: &'static str,
    pub text_4xl: &'static str,
    pub google_fonts_url: &'static str,
}

static TYPOGRAPHY_PRESETS: &[TypographyPreset] = &[
    TypographyPreset {
        id: "tech",
        name: "Tech",
        font_heading: "'Inter', system-ui, sans-serif",
        font_body: "'Inter', system-ui, sans-serif",
        font_mono: "'JetBrains Mono', ui-monospace, monospace",
        text_xs: "clamp(0.75rem, 0.7rem + 0.15vw, 0.8rem)",
        text_sm: "clamp(0.875rem, 0.8rem + 0.25vw, 1rem)",
        text_base: "clamp(1rem, 0.925rem + 0.3vw, 1.125rem)",
        text_lg: "clamp(1.125rem, 1rem + 0.4vw, 1.25rem)",
        text_xl: "clamp(1.25rem, 1.1rem + 0.5vw, 1.5rem)",
        text_2xl: "clamp(1.5rem, 1.25rem + 0.75vw, 2rem)",
        text_3xl: "clamp(1.875rem, 1.5rem + 1.2vw, 2.5rem)",
        text_4xl: "clamp(2.25rem, 1.75rem + 1.5vw, 3.5rem)",
        google_fonts_url: "https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap",
    },
    TypographyPreset {
        id: "editorial",
        name: "Editorial",
        font_heading: "'Playfair Display', Georgia, serif",
        font_body: "'Source Sans 3', 'Source Sans Pro', system-ui, sans-serif",
        font_mono: "'Fira Code', ui-monospace, monospace",
        text_xs: "clamp(0.75rem, 0.7rem + 0.15vw, 0.8rem)",
        text_sm: "clamp(0.875rem, 0.825rem + 0.2vw, 0.95rem)",
        text_base: "clamp(1.0625rem, 0.975rem + 0.35vw, 1.1875rem)",
        text_lg: "clamp(1.1875rem, 1.075rem + 0.45vw, 1.375rem)",
        text_xl: "clamp(1.375rem, 1.2rem + 0.55vw, 1.625rem)",
        text_2xl: "clamp(1.75rem, 1.4rem + 0.9vw, 2.25rem)",
        text_3xl: "clamp(2.25rem, 1.75rem + 1.5vw, 3rem)",
        text_4xl: "clamp(2.75rem, 2rem + 2vw, 4rem)",
        google_fonts_url: "https://fonts.googleapis.com/css2?family=Playfair+Display:wght@400;700;900&family=Source+Sans+3:wght@400;500;600&family=Fira+Code:wght@400;500&display=swap",
    },
    TypographyPreset {
        id: "modern",
        name: "Modern",
        font_heading: "'Plus Jakarta Sans', system-ui, sans-serif",
        font_body: "'DM Sans', system-ui, sans-serif",
        font_mono: "'DM Mono', ui-monospace, monospace",
        text_xs: "clamp(0.75rem, 0.7rem + 0.15vw, 0.8rem)",
        text_sm: "clamp(0.875rem, 0.8rem + 0.25vw, 1rem)",
        text_base: "clamp(1rem, 0.9rem + 0.35vw, 1.125rem)",
        text_lg: "clamp(1.125rem, 1.025rem + 0.35vw, 1.25rem)",
        text_xl: "clamp(1.25rem, 1.1rem + 0.5vw, 1.5rem)",
        text_2xl: "clamp(1.5rem, 1.25rem + 0.75vw, 2rem)",
        text_3xl: "clamp(2rem, 1.6rem + 1.25vw, 2.75rem)",
        text_4xl: "clamp(2.5rem, 1.9rem + 1.75vw, 3.75rem)",
        google_fonts_url: "https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700&family=DM+Sans:wght@400;500;600&family=DM+Mono:wght@400;500&display=swap",
    },
    TypographyPreset {
        id: "clean",
        name: "Clean",
        font_heading: "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif",
        font_body: "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif",
        font_mono: "ui-monospace, 'Cascadia Code', 'Fira Code', monospace",
        text_xs: "clamp(0.75rem, 0.7rem + 0.15vw, 0.8rem)",
        text_sm: "clamp(0.875rem, 0.8rem + 0.25vw, 1rem)",
        text_base: "clamp(1rem, 0.925rem + 0.3vw, 1.125rem)",
        text_lg: "clamp(1.125rem, 1rem + 0.4vw, 1.25rem)",
        text_xl: "clamp(1.25rem, 1.1rem + 0.5vw, 1.5rem)",
        text_2xl: "clamp(1.5rem, 1.25rem + 0.75vw, 2rem)",
        text_3xl: "clamp(1.875rem, 1.5rem + 1.2vw, 2.5rem)",
        text_4xl: "clamp(2.25rem, 1.75rem + 1.5vw, 3.5rem)",
        google_fonts_url: "",
    },
];

/// Get all 4 typography presets.
pub fn all_typography_presets() -> &'static [TypographyPreset] {
    TYPOGRAPHY_PRESETS
}

/// Get a typography preset by ID.
pub fn get_typography_preset(id: &str) -> Option<&'static TypographyPreset> {
    TYPOGRAPHY_PRESETS.iter().find(|t| t.id == id)
}

// ─── Layout Variants ────────────────────────────────────────────────────────

/// Layout variant for a section — different HTML structures, same slot schema.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LayoutVariant {
    pub section_id: &'static str,
    pub variant_id: &'static str,
    pub name: &'static str,
}

/// Layout variants per template. Each section has 2-3 layout options.
static LAYOUT_VARIANTS: &[LayoutVariant] = &[
    // ── saas_landing layouts ────────────────────────────────────────────
    LayoutVariant {
        section_id: "hero",
        variant_id: "centered",
        name: "Centered",
    },
    LayoutVariant {
        section_id: "hero",
        variant_id: "split_image",
        name: "Split Image",
    },
    LayoutVariant {
        section_id: "hero",
        variant_id: "video_bg",
        name: "Video Background",
    },
    LayoutVariant {
        section_id: "features",
        variant_id: "card_grid",
        name: "Card Grid",
    },
    LayoutVariant {
        section_id: "features",
        variant_id: "alternating",
        name: "Alternating",
    },
    LayoutVariant {
        section_id: "pricing",
        variant_id: "three_col",
        name: "Three Column",
    },
    LayoutVariant {
        section_id: "pricing",
        variant_id: "comparison",
        name: "Comparison Table",
    },
    LayoutVariant {
        section_id: "testimonials",
        variant_id: "carousel",
        name: "Carousel",
    },
    LayoutVariant {
        section_id: "testimonials",
        variant_id: "grid",
        name: "Grid",
    },
    LayoutVariant {
        section_id: "cta",
        variant_id: "banner",
        name: "Banner",
    },
    LayoutVariant {
        section_id: "cta",
        variant_id: "split",
        name: "Split",
    },
    LayoutVariant {
        section_id: "footer",
        variant_id: "simple",
        name: "Simple",
    },
    LayoutVariant {
        section_id: "footer",
        variant_id: "multi_col",
        name: "Multi Column",
    },
    // ── docs_site layouts ───────────────────────────────────────────────
    LayoutVariant {
        section_id: "sidebar_nav",
        variant_id: "fixed",
        name: "Fixed Sidebar",
    },
    LayoutVariant {
        section_id: "sidebar_nav",
        variant_id: "collapsible",
        name: "Collapsible",
    },
    LayoutVariant {
        section_id: "content",
        variant_id: "single_col",
        name: "Single Column",
    },
    LayoutVariant {
        section_id: "content",
        variant_id: "with_toc",
        name: "With Table of Contents",
    },
    LayoutVariant {
        section_id: "code_blocks",
        variant_id: "tabbed",
        name: "Tabbed",
    },
    LayoutVariant {
        section_id: "code_blocks",
        variant_id: "stacked",
        name: "Stacked",
    },
    // ── portfolio layouts ───────────────────────────────────────────────
    LayoutVariant {
        section_id: "projects",
        variant_id: "card_grid",
        name: "Card Grid",
    },
    LayoutVariant {
        section_id: "projects",
        variant_id: "masonry",
        name: "Masonry",
    },
    LayoutVariant {
        section_id: "projects",
        variant_id: "list",
        name: "List",
    },
    LayoutVariant {
        section_id: "about",
        variant_id: "single_col",
        name: "Single Column",
    },
    LayoutVariant {
        section_id: "about",
        variant_id: "split_photo",
        name: "Split with Photo",
    },
    LayoutVariant {
        section_id: "skills",
        variant_id: "tag_cloud",
        name: "Tag Cloud",
    },
    LayoutVariant {
        section_id: "skills",
        variant_id: "bar_chart",
        name: "Bar Chart",
    },
    LayoutVariant {
        section_id: "contact",
        variant_id: "form",
        name: "Contact Form",
    },
    LayoutVariant {
        section_id: "contact",
        variant_id: "minimal",
        name: "Minimal Links",
    },
    // ── local_business layouts ──────────────────────────────────────────
    LayoutVariant {
        section_id: "services",
        variant_id: "card_grid",
        name: "Card Grid",
    },
    LayoutVariant {
        section_id: "services",
        variant_id: "icon_list",
        name: "Icon List",
    },
    LayoutVariant {
        section_id: "gallery",
        variant_id: "masonry",
        name: "Masonry",
    },
    LayoutVariant {
        section_id: "gallery",
        variant_id: "carousel",
        name: "Carousel",
    },
    LayoutVariant {
        section_id: "gallery",
        variant_id: "grid_uniform",
        name: "Uniform Grid",
    },
    LayoutVariant {
        section_id: "hours",
        variant_id: "table",
        name: "Table",
    },
    LayoutVariant {
        section_id: "hours",
        variant_id: "card",
        name: "Card",
    },
    // ── ecommerce layouts ───────────────────────────────────────────────
    LayoutVariant {
        section_id: "categories",
        variant_id: "card_grid",
        name: "Card Grid",
    },
    LayoutVariant {
        section_id: "categories",
        variant_id: "pill_bar",
        name: "Pill Bar",
    },
    LayoutVariant {
        section_id: "products",
        variant_id: "grid_4col",
        name: "4-Column Grid",
    },
    LayoutVariant {
        section_id: "products",
        variant_id: "grid_3col",
        name: "3-Column Grid",
    },
    LayoutVariant {
        section_id: "products",
        variant_id: "list",
        name: "List View",
    },
    LayoutVariant {
        section_id: "reviews",
        variant_id: "carousel",
        name: "Carousel",
    },
    LayoutVariant {
        section_id: "reviews",
        variant_id: "stacked",
        name: "Stacked",
    },
    LayoutVariant {
        section_id: "newsletter",
        variant_id: "banner",
        name: "Banner",
    },
    LayoutVariant {
        section_id: "newsletter",
        variant_id: "inline",
        name: "Inline",
    },
    // ── dashboard layouts ───────────────────────────────────────────────
    LayoutVariant {
        section_id: "sidebar",
        variant_id: "full",
        name: "Full Sidebar",
    },
    LayoutVariant {
        section_id: "sidebar",
        variant_id: "mini",
        name: "Mini Icons",
    },
    LayoutVariant {
        section_id: "stats",
        variant_id: "four_cards",
        name: "Four Cards",
    },
    LayoutVariant {
        section_id: "stats",
        variant_id: "compact_row",
        name: "Compact Row",
    },
    LayoutVariant {
        section_id: "charts",
        variant_id: "two_col",
        name: "Two Column",
    },
    LayoutVariant {
        section_id: "charts",
        variant_id: "stacked",
        name: "Stacked",
    },
    LayoutVariant {
        section_id: "data_table",
        variant_id: "standard",
        name: "Standard Table",
    },
    LayoutVariant {
        section_id: "data_table",
        variant_id: "cards",
        name: "Card List",
    },
];

/// Get all layout variants.
pub fn all_layout_variants() -> &'static [LayoutVariant] {
    LAYOUT_VARIANTS
}

/// Get layout variants for a section.
pub fn layouts_for_section(section_id: &str) -> Vec<&'static LayoutVariant> {
    LAYOUT_VARIANTS
        .iter()
        .filter(|l| l.section_id == section_id)
        .collect()
}

// ─── Motion Profiles ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MotionProfile {
    None,
    Subtle,
    Expressive,
}

impl MotionProfile {
    /// CSS for this motion profile's transition/animation values.
    /// All profiles respect `prefers-reduced-motion: reduce` via the token system.
    pub fn duration_fast(&self) -> &'static str {
        match self {
            Self::None => "0ms",
            Self::Subtle => "150ms",
            Self::Expressive => "300ms",
        }
    }

    pub fn duration_normal(&self) -> &'static str {
        match self {
            Self::None => "0ms",
            Self::Subtle => "250ms",
            Self::Expressive => "500ms",
        }
    }

    pub fn duration_slow(&self) -> &'static str {
        match self {
            Self::None => "0ms",
            Self::Subtle => "300ms",
            Self::Expressive => "800ms",
        }
    }
}

// ─── Variant Selection ──────────────────────────────────────────────────────

/// A complete variant selection for a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantSelection {
    pub palette_id: String,
    pub typography_id: String,
    pub layout: HashMap<String, String>, // section_id → layout variant_id
    pub motion: MotionProfile,
}

impl Default for VariantSelection {
    fn default() -> Self {
        Self {
            palette_id: "indigo".into(),
            typography_id: "inter".into(),
            layout: HashMap::new(),
            motion: MotionProfile::Subtle,
        }
    }
}

impl VariantSelection {
    /// Produce a complete TokenSet from this variant selection.
    pub fn to_token_set(&self) -> Option<TokenSet> {
        let palette = all_palette_presets()
            .iter()
            .find(|p| p.id == self.palette_id)?;
        let typo = get_typography_preset(&self.typography_id)?;

        let foundation = FoundationTokens {
            color_primary: palette.light.primary.into(),
            color_secondary: palette.light.secondary.into(),
            color_accent: palette.light.accent.into(),
            color_bg: palette.light.bg.into(),
            color_bg_secondary: palette.light.bg_secondary.into(),
            color_text: palette.light.text.into(),
            color_text_secondary: palette.light.text_secondary.into(),
            color_border: palette.light.border.into(),
            font_heading: typo.font_heading.into(),
            font_body: typo.font_body.into(),
            font_mono: typo.font_mono.into(),
            text_xs: typo.text_xs.into(),
            text_sm: typo.text_sm.into(),
            text_base: typo.text_base.into(),
            text_lg: typo.text_lg.into(),
            text_xl: typo.text_xl.into(),
            text_2xl: typo.text_2xl.into(),
            text_3xl: typo.text_3xl.into(),
            text_4xl: typo.text_4xl.into(),
            duration_fast: self.motion.duration_fast().into(),
            duration_normal: self.motion.duration_normal().into(),
            duration_slow: self.motion.duration_slow().into(),
            ..FoundationTokens::default()
        };

        let dark_mode = DarkModeColors {
            color_primary: palette.dark.primary.into(),
            color_secondary: palette.dark.secondary.into(),
            color_accent: palette.dark.accent.into(),
            color_bg: palette.dark.bg.into(),
            color_bg_secondary: palette.dark.bg_secondary.into(),
            color_text: palette.dark.text.into(),
            color_text_secondary: palette.dark.text_secondary.into(),
            color_border: palette.dark.border.into(),
        };

        Some(TokenSet {
            foundation,
            dark_mode,
            semantic: Default::default(),
            overrides: Vec::new(),
        })
    }
}

// ─── Combinatorics ──────────────────────────────────────────────────────────

/// Compute the total number of unique variant combinations across all templates.
pub fn total_variant_combinations() -> usize {
    let template_ids = [
        "saas_landing",
        "docs_site",
        "portfolio",
        "local_business",
        "ecommerce",
        "dashboard",
    ];

    let num_typography = TYPOGRAPHY_PRESETS.len();
    let num_motion = 3; // None, Subtle, Expressive

    let mut total = 0;
    for template_id in &template_ids {
        let num_palettes = palettes_for_template(template_id).len();

        // For each template, find sections that have layout variants
        // and compute the product of layout choices
        let schemas = crate::slot_schema::all_template_schemas();
        let schema = schemas.iter().find(|s| s.template_id == *template_id);

        let layout_product: usize = if let Some(schema) = schema {
            schema
                .sections
                .iter()
                .map(|sec| {
                    let variants = layouts_for_section(&sec.section_id);
                    if variants.is_empty() {
                        1 // sections without explicit layout variants have 1 default
                    } else {
                        variants.len()
                    }
                })
                .product()
        } else {
            1
        };

        total += num_palettes * num_typography * layout_product * num_motion;
    }

    total
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_24_palette_presets() {
        assert_eq!(all_palette_presets().len(), 24);
    }

    #[test]
    fn test_4_palettes_per_template() {
        let template_ids = [
            "saas_landing",
            "docs_site",
            "portfolio",
            "local_business",
            "ecommerce",
            "dashboard",
        ];
        for id in &template_ids {
            let palettes = palettes_for_template(id);
            assert_eq!(
                palettes.len(),
                4,
                "Template '{}' should have 4 palettes, got {}",
                id,
                palettes.len()
            );
        }
    }

    #[test]
    fn test_every_palette_has_all_8_color_keys() {
        for palette in all_palette_presets() {
            for key in PALETTE_COLOR_KEYS {
                assert!(
                    palette.light.get(key).is_some(),
                    "Palette '{}' light mode missing key '{}'",
                    palette.id,
                    key
                );
                assert!(
                    palette.dark.get(key).is_some(),
                    "Palette '{}' dark mode missing key '{}'",
                    palette.id,
                    key
                );
            }
        }
    }

    #[test]
    fn test_every_palette_has_light_and_dark() {
        for palette in all_palette_presets() {
            // Light and dark should differ in at least the bg color
            assert_ne!(
                palette.light.bg, palette.dark.bg,
                "Palette '{}' has identical light/dark bg",
                palette.id
            );
        }
    }

    #[test]
    fn test_4_typography_presets() {
        assert_eq!(all_typography_presets().len(), 4);
        let ids: Vec<&str> = all_typography_presets().iter().map(|t| t.id).collect();
        assert!(ids.contains(&"tech"));
        assert!(ids.contains(&"editorial"));
        assert!(ids.contains(&"modern"));
        assert!(ids.contains(&"clean"));
    }

    #[test]
    fn test_typography_has_heading_body_mono_and_scale() {
        for typo in all_typography_presets() {
            assert!(!typo.font_heading.is_empty(), "{} missing heading", typo.id);
            assert!(!typo.font_body.is_empty(), "{} missing body", typo.id);
            assert!(!typo.font_mono.is_empty(), "{} missing mono", typo.id);
            assert!(
                typo.text_xs.contains("clamp("),
                "{} text_xs not fluid",
                typo.id
            );
            assert!(
                typo.text_4xl.contains("clamp("),
                "{} text_4xl not fluid",
                typo.id
            );
        }
    }

    #[test]
    fn test_layout_variants_preserve_section_ids() {
        // All layout variants for the same section_id should share that section_id
        let sections: std::collections::HashSet<&str> =
            LAYOUT_VARIANTS.iter().map(|l| l.section_id).collect();
        for section_id in &sections {
            let variants = layouts_for_section(section_id);
            assert!(
                variants.len() >= 2,
                "Section '{}' should have at least 2 layout variants",
                section_id
            );
            for v in &variants {
                assert_eq!(v.section_id, *section_id);
            }
        }
    }

    #[test]
    fn test_motion_none_zero_durations() {
        let m = MotionProfile::None;
        assert_eq!(m.duration_fast(), "0ms");
        assert_eq!(m.duration_normal(), "0ms");
        assert_eq!(m.duration_slow(), "0ms");
    }

    #[test]
    fn test_motion_subtle_short_durations() {
        let m = MotionProfile::Subtle;
        assert!(m.duration_fast().contains("150"));
        assert!(m.duration_normal().contains("250"));
    }

    #[test]
    fn test_motion_expressive_full_durations() {
        let m = MotionProfile::Expressive;
        assert!(m.duration_fast().contains("300"));
        assert!(m.duration_slow().contains("800"));
    }

    #[test]
    fn test_combinatoric_count_at_least_430() {
        let count = total_variant_combinations();
        assert!(count >= 430, "Expected >= 430 combinations, got {}", count);
    }

    #[test]
    fn test_variant_selection_to_token_set() {
        let selection = VariantSelection {
            palette_id: "saas_midnight".into(),
            typography_id: "tech".into(),
            layout: HashMap::new(),
            motion: MotionProfile::Subtle,
        };
        let ts = selection.to_token_set();
        assert!(ts.is_some(), "should produce a valid TokenSet");
        let ts = ts.unwrap();
        assert_eq!(ts.foundation.color_primary, "#4f46e5");
        assert_eq!(ts.dark_mode.color_primary, "#818cf8");
        assert!(ts.foundation.font_heading.contains("Inter"));
        assert_eq!(ts.foundation.duration_fast, "150ms");
    }

    #[test]
    fn test_variant_selection_invalid_palette() {
        let selection = VariantSelection {
            palette_id: "nonexistent".into(),
            typography_id: "tech".into(),
            layout: HashMap::new(),
            motion: MotionProfile::None,
        };
        assert!(selection.to_token_set().is_none());
    }

    #[test]
    fn test_template_schema_sections_have_layout_variants() {
        // Cross-contract: every section in every template schema either has
        // layout variants defined or is a minor section (search, map, etc.)
        let schemas = crate::slot_schema::all_template_schemas();
        for schema in &schemas {
            let mut has_any_layout = false;
            for section in &schema.sections {
                let variants = layouts_for_section(&section.section_id);
                if !variants.is_empty() {
                    has_any_layout = true;
                }
            }
            assert!(
                has_any_layout,
                "Template '{}' has no sections with layout variants",
                schema.template_id
            );
        }
    }
}
