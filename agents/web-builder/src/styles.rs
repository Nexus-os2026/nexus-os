use serde::{Deserialize, Serialize};

// ─── Design Token Types ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeTokens {
    pub background: String,
    pub surface: String,
    pub text: String,
    pub accent: String,
    pub accent_secondary: String,
    pub font_display: String,
    pub font_body: String,
    pub font_mono: String,
    pub animation_timing: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeOutput {
    pub tokens: ThemeTokens,
    pub tailwind_config: String,
    pub css: String,
}

/// Extended design token set for Phase 0c conductor v2.
/// Selected per-generation from the user's brief via `select_design_tokens`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignTokenSet {
    pub palette_name: String,
    pub bg: String,
    pub surface: String,
    pub text_primary: String,
    pub text_secondary: String,
    pub accent: String,
    pub accent_hover: String,
    pub border: String,
    pub font_display: String,
    pub font_display_weights: String,
    pub font_body: String,
    pub font_body_weights: String,
    pub radius: String,
    pub shadow_style: ShadowStyle,
    pub motion_level: MotionLevel,
    pub texture: TextureKind,
    pub mood: Mood,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShadowStyle {
    None,
    Subtle,
    Layered,
    Dramatic,
    Glow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MotionLevel {
    None,
    Subtle,
    Moderate,
    Dramatic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextureKind {
    None,
    Grain,
    GradientMesh,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mood {
    Minimal,
    Bold,
    Luxury,
    Friendly,
    Editorial,
    Brutalist,
    Organic,
    Tech,
}

// ─── Palettes ──────────────────────────────────────────────────────────────
// [bg, surface, text_primary, text_secondary, accent, accent_hover, border]

struct Palette {
    name: &'static str,
    bg: &'static str,
    surface: &'static str,
    text_primary: &'static str,
    text_secondary: &'static str,
    accent: &'static str,
    accent_hover: &'static str,
    border: &'static str,
}

const PALETTES: &[Palette] = &[
    Palette {
        name: "midnight",
        bg: "#0a0a0f",
        surface: "#12121a",
        text_primary: "#f0f0f5",
        text_secondary: "#8888a0",
        accent: "#6366f1",
        accent_hover: "#818cf8",
        border: "#1e1e2e",
    },
    Palette {
        name: "warm_sand",
        bg: "#faf6f1",
        surface: "#fff8f0",
        text_primary: "#1a1410",
        text_secondary: "#6b5e50",
        accent: "#c2410c",
        accent_hover: "#ea580c",
        border: "#e5ddd3",
    },
    Palette {
        name: "forest",
        bg: "#0c1a0c",
        surface: "#142014",
        text_primary: "#e8f0e8",
        text_secondary: "#7a9a7a",
        accent: "#22c55e",
        accent_hover: "#4ade80",
        border: "#1e2e1e",
    },
    Palette {
        name: "arctic",
        bg: "#f8fafc",
        surface: "#ffffff",
        text_primary: "#0f172a",
        text_secondary: "#64748b",
        accent: "#0ea5e9",
        accent_hover: "#38bdf8",
        border: "#e2e8f0",
    },
    Palette {
        name: "noir",
        bg: "#000000",
        surface: "#0a0a0a",
        text_primary: "#fafafa",
        text_secondary: "#737373",
        accent: "#f5f5f5",
        accent_hover: "#ffffff",
        border: "#1a1a1a",
    },
    Palette {
        name: "clay",
        bg: "#f5ebe0",
        surface: "#fefcf8",
        text_primary: "#292524",
        text_secondary: "#78716c",
        accent: "#b45309",
        accent_hover: "#d97706",
        border: "#e7ddd0",
    },
    Palette {
        name: "ocean_depth",
        bg: "#020617",
        surface: "#0f172a",
        text_primary: "#e2e8f0",
        text_secondary: "#94a3b8",
        accent: "#06b6d4",
        accent_hover: "#22d3ee",
        border: "#1e293b",
    },
    Palette {
        name: "rose_gold",
        bg: "#1a0a10",
        surface: "#2d1520",
        text_primary: "#fce7f3",
        text_secondary: "#d4a0b0",
        accent: "#f43f5e",
        accent_hover: "#fb7185",
        border: "#3d1f2e",
    },
    Palette {
        name: "ember",
        bg: "#1c1210",
        surface: "#2a1c18",
        text_primary: "#fde8d8",
        text_secondary: "#b8976e",
        accent: "#ef4444",
        accent_hover: "#f87171",
        border: "#3a2820",
    },
    Palette {
        name: "sage",
        bg: "#f2f5f0",
        surface: "#fafcf8",
        text_primary: "#1a2e1a",
        text_secondary: "#5a7a5a",
        accent: "#16a34a",
        accent_hover: "#22c55e",
        border: "#d4e0d0",
    },
    Palette {
        name: "slate_pro",
        bg: "#f1f5f9",
        surface: "#ffffff",
        text_primary: "#0f172a",
        text_secondary: "#475569",
        accent: "#3b82f6",
        accent_hover: "#60a5fa",
        border: "#cbd5e1",
    },
    Palette {
        name: "neon_night",
        bg: "#07010f",
        surface: "#120024",
        text_primary: "#e8fffe",
        text_secondary: "#9ca0b0",
        accent: "#00f5d4",
        accent_hover: "#5ffbdb",
        border: "#1e0040",
    },
];

// ─── Font Pairings ─────────────────────────────────────────────────────────

struct FontPairing {
    display: &'static str,
    display_weights: &'static str,
    body: &'static str,
    body_weights: &'static str,
    vibe: &'static str, // for matching mood
}

const FONT_PAIRINGS: &[FontPairing] = &[
    FontPairing {
        display: "Playfair Display",
        display_weights: "700,800",
        body: "DM Sans",
        body_weights: "400,500,600",
        vibe: "luxury",
    },
    FontPairing {
        display: "Clash Display",
        display_weights: "600,700",
        body: "Outfit",
        body_weights: "400,500,600",
        vibe: "bold",
    },
    FontPairing {
        display: "Cabinet Grotesk",
        display_weights: "700,800",
        body: "DM Sans",
        body_weights: "400,500",
        vibe: "tech",
    },
    FontPairing {
        display: "Satoshi",
        display_weights: "700,900",
        body: "Manrope",
        body_weights: "400,500",
        vibe: "minimal",
    },
    FontPairing {
        display: "Instrument Serif",
        display_weights: "400",
        body: "Figtree",
        body_weights: "400,500,600",
        vibe: "editorial",
    },
    FontPairing {
        display: "Plus Jakarta Sans",
        display_weights: "700,800",
        body: "Geist",
        body_weights: "400,500",
        vibe: "friendly",
    },
    FontPairing {
        display: "Syne",
        display_weights: "700,800",
        body: "Outfit",
        body_weights: "400,500",
        vibe: "bold",
    },
    FontPairing {
        display: "General Sans",
        display_weights: "600,700",
        body: "Nunito Sans",
        body_weights: "400,500,600",
        vibe: "friendly",
    },
    FontPairing {
        display: "Space Grotesk",
        display_weights: "600,700",
        body: "Space Mono",
        body_weights: "400,700",
        vibe: "brutalist",
    },
    FontPairing {
        display: "Cormorant Garamond",
        display_weights: "600,700",
        body: "Lora",
        body_weights: "400,500",
        vibe: "luxury",
    },
    FontPairing {
        display: "Fraunces",
        display_weights: "700,900",
        body: "Source Serif 4",
        body_weights: "400,500",
        vibe: "organic",
    },
    FontPairing {
        display: "Audiowide",
        display_weights: "400",
        body: "Space Grotesk",
        body_weights: "400,500",
        vibe: "tech",
    },
    FontPairing {
        display: "Sora",
        display_weights: "600,700",
        body: "DM Sans",
        body_weights: "400,500",
        vibe: "tech",
    },
];

// ─── Shadow Presets ────────────────────────────────────────────────────────

impl ShadowStyle {
    pub fn css_value(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Subtle => "0 1px 2px rgba(0,0,0,0.05)",
            Self::Layered => "0 1px 3px rgba(0,0,0,0.06), 0 8px 24px rgba(0,0,0,0.08)",
            Self::Dramatic => "0 20px 60px rgba(0,0,0,0.3)",
            Self::Glow => "0 0 30px rgba(99,102,241,0.15)",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Subtle => "subtle",
            Self::Layered => "layered",
            Self::Dramatic => "dramatic",
            Self::Glow => "glow",
        }
    }
}

impl MotionLevel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Subtle => "subtle",
            Self::Moderate => "moderate",
            Self::Dramatic => "dramatic",
        }
    }
}

impl TextureKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Grain => "grain",
            Self::GradientMesh => "gradient_mesh",
            Self::Grid => "grid",
        }
    }

    pub fn css_overlay(&self) -> &'static str {
        match self {
            Self::None => "",
            Self::Grain => "background-image: url(\"data:image/svg+xml,%3Csvg viewBox='0 0 256 256' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='noise'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23noise)' opacity='0.04'/%3E%3C/svg%3E\"); background-repeat: repeat; background-size: 256px 256px;",
            Self::GradientMesh => "background-image: radial-gradient(ellipse at 20% 50%, rgba(99,102,241,0.08) 0%, transparent 50%), radial-gradient(ellipse at 80% 20%, rgba(6,182,212,0.06) 0%, transparent 50%);",
            Self::Grid => "background-image: repeating-linear-gradient(0deg, transparent, transparent 59px, rgba(255,255,255,0.03) 59px, rgba(255,255,255,0.03) 60px), repeating-linear-gradient(90deg, transparent, transparent 59px, rgba(255,255,255,0.03) 59px, rgba(255,255,255,0.03) 60px);",
        }
    }
}

impl Mood {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Bold => "bold",
            Self::Luxury => "luxury",
            Self::Friendly => "friendly",
            Self::Editorial => "editorial",
            Self::Brutalist => "brutalist",
            Self::Organic => "organic",
            Self::Tech => "tech",
        }
    }
}

// ─── Brief Analysis & Token Selection ──────────────────────────────────────

/// Detect industry from the user's brief.
fn detect_industry(brief: &str) -> &'static str {
    let lower = brief.to_ascii_lowercase();
    let keywords: &[(&[&str], &str)] = &[
        (
            &[
                "restaurant",
                "cafe",
                "coffee",
                "food",
                "bakery",
                "bistro",
                "dining",
                "menu",
            ],
            "food",
        ),
        (
            &[
                "portfolio",
                "photographer",
                "artist",
                "creative",
                "gallery",
                "design studio",
            ],
            "creative",
        ),
        (
            &[
                "ecommerce",
                "e-commerce",
                "shop",
                "store",
                "product",
                "buy",
                "sell",
                "retail",
            ],
            "ecommerce",
        ),
        (
            &[
                "finance",
                "bank",
                "invest",
                "trading",
                "fintech",
                "insurance",
                "enterprise",
            ],
            "finance",
        ),
        (
            &[
                "health", "wellness", "fitness", "yoga", "medical", "clinic", "therapy",
            ],
            "health",
        ),
        (
            &[
                "saas",
                "api",
                "developer",
                "dev tool",
                "devtool",
                "sdk",
                "platform",
                "dashboard",
                "analytics",
            ],
            "saas",
        ),
        (
            &[
                "documentation",
                "docs",
                "technical",
                "open source",
                "library",
            ],
            "docs",
        ),
        (&["agency", "consulting", "marketing", "brand"], "agency"),
    ];
    for (kws, industry) in keywords {
        if kws.iter().any(|kw| lower.contains(kw)) {
            return industry;
        }
    }
    "saas" // default
}

/// Detect mood from the user's brief keywords.
fn detect_mood(brief: &str) -> Mood {
    let lower = brief.to_ascii_lowercase();
    let mood_keywords: &[(&[&str], Mood)] = &[
        (
            &[
                "luxury",
                "premium",
                "elegant",
                "upscale",
                "exclusive",
                "haute",
            ],
            Mood::Luxury,
        ),
        (
            &["bold", "creative", "fun", "vibrant", "energetic", "playful"],
            Mood::Bold,
        ),
        (
            &["brutalist", "raw", "experimental", "punk"],
            Mood::Brutalist,
        ),
        (
            &["editorial", "magazine", "journal", "publication"],
            Mood::Editorial,
        ),
        (
            &["organic", "natural", "earthy", "warm", "handcraft"],
            Mood::Organic,
        ),
        (
            &["friendly", "approachable", "welcoming", "casual"],
            Mood::Friendly,
        ),
        (
            &["minimal", "clean", "simple", "zen", "sparse"],
            Mood::Minimal,
        ),
    ];
    for (kws, mood) in mood_keywords {
        if kws.iter().any(|kw| lower.contains(kw)) {
            return *mood;
        }
    }
    Mood::Tech // default for SaaS/tech
}

/// Select a palette name based on industry and mood.
fn select_palette_name(industry: &str, mood: Mood) -> &'static str {
    // Mood overrides take priority for strong aesthetic signals
    match mood {
        Mood::Luxury => return "rose_gold",
        Mood::Brutalist => return "noir",
        Mood::Organic => return "clay",
        _ => {}
    }
    match industry {
        "food" => "warm_sand",
        "creative" => "noir",
        "ecommerce" => "warm_sand",
        "finance" | "agency" => "slate_pro",
        "health" => "sage",
        "saas" | "docs" => match mood {
            Mood::Bold => "midnight",
            Mood::Minimal => "arctic",
            _ => "ocean_depth",
        },
        _ => "ocean_depth",
    }
}

/// Select font pairing based on mood.
fn select_font_pairing(mood: Mood) -> &'static FontPairing {
    let target_vibe = match mood {
        Mood::Minimal => "minimal",
        Mood::Bold => "bold",
        Mood::Luxury => "luxury",
        Mood::Friendly => "friendly",
        Mood::Editorial => "editorial",
        Mood::Brutalist => "brutalist",
        Mood::Organic => "organic",
        Mood::Tech => "tech",
    };
    FONT_PAIRINGS
        .iter()
        .find(|fp| fp.vibe == target_vibe)
        .unwrap_or(&FONT_PAIRINGS[2]) // Cabinet Grotesk / DM Sans fallback
}

fn select_radius(mood: Mood) -> &'static str {
    match mood {
        Mood::Brutalist | Mood::Editorial => "0px",
        Mood::Minimal | Mood::Tech => "6px",
        Mood::Friendly | Mood::Organic => "12px",
        Mood::Bold => "9999px",
        Mood::Luxury => "6px",
    }
}

fn select_shadow(mood: Mood) -> ShadowStyle {
    match mood {
        Mood::Brutalist => ShadowStyle::None,
        Mood::Minimal | Mood::Editorial => ShadowStyle::Subtle,
        Mood::Tech | Mood::Friendly | Mood::Organic => ShadowStyle::Layered,
        Mood::Bold => ShadowStyle::Dramatic,
        Mood::Luxury => ShadowStyle::Layered,
    }
}

fn select_motion(mood: Mood) -> MotionLevel {
    match mood {
        Mood::Brutalist => MotionLevel::None,
        Mood::Minimal => MotionLevel::Subtle,
        Mood::Tech | Mood::Friendly | Mood::Editorial | Mood::Organic => MotionLevel::Moderate,
        Mood::Bold | Mood::Luxury => MotionLevel::Dramatic,
    }
}

fn select_texture(mood: Mood) -> TextureKind {
    match mood {
        Mood::Minimal | Mood::Friendly => TextureKind::None,
        Mood::Tech | Mood::Bold => TextureKind::Grid,
        Mood::Luxury | Mood::Editorial | Mood::Organic => TextureKind::Grain,
        Mood::Brutalist => TextureKind::None,
    }
}

// ─── Public API ────────────────────────────────────────────────────────────

/// Analyze a user brief and select a complete, cohesive design token set.
/// This is the primary entry point for the Phase 0c conductor v2.
pub fn select_design_tokens(brief: &str) -> DesignTokenSet {
    let industry = detect_industry(brief);
    let mood = detect_mood(brief);
    let palette_name = select_palette_name(industry, mood);
    let palette = PALETTES
        .iter()
        .find(|p| p.name == palette_name)
        .unwrap_or(&PALETTES[0]);
    let fonts = select_font_pairing(mood);

    DesignTokenSet {
        palette_name: palette.name.to_string(),
        bg: palette.bg.to_string(),
        surface: palette.surface.to_string(),
        text_primary: palette.text_primary.to_string(),
        text_secondary: palette.text_secondary.to_string(),
        accent: palette.accent.to_string(),
        accent_hover: palette.accent_hover.to_string(),
        border: palette.border.to_string(),
        font_display: fonts.display.to_string(),
        font_display_weights: fonts.display_weights.to_string(),
        font_body: fonts.body.to_string(),
        font_body_weights: fonts.body_weights.to_string(),
        radius: select_radius(mood).to_string(),
        shadow_style: select_shadow(mood),
        motion_level: select_motion(mood),
        texture: select_texture(mood),
        mood,
    }
}

impl DesignTokenSet {
    /// Render as CSS custom properties block for injection into LLM prompts.
    pub fn to_css_variables(&self) -> String {
        format!(
            ":root {{\n\
             \x20 --font-display: '{}', sans-serif;\n\
             \x20 --font-body: '{}', sans-serif;\n\
             \x20 --color-bg: {};\n\
             \x20 --color-surface: {};\n\
             \x20 --color-text: {};\n\
             \x20 --color-text-secondary: {};\n\
             \x20 --color-accent: {};\n\
             \x20 --color-accent-hover: {};\n\
             \x20 --color-border: {};\n\
             \x20 --radius: {};\n\
             \x20 --shadow: {};\n\
             }}",
            self.font_display,
            self.font_body,
            self.bg,
            self.surface,
            self.text_primary,
            self.text_secondary,
            self.accent,
            self.accent_hover,
            self.border,
            self.radius,
            self.shadow_style.css_value(),
        )
    }

    /// Google Fonts URL for the selected pairing.
    pub fn google_fonts_url(&self) -> String {
        let display_encoded = self.font_display.replace(' ', "+");
        let body_encoded = self.font_body.replace(' ', "+");
        format!(
            "https://fonts.googleapis.com/css2?family={}:wght@{}&family={}:wght@{}&display=swap",
            display_encoded, self.font_display_weights, body_encoded, self.font_body_weights,
        )
    }

    /// Convert to legacy ThemeTokens for backward compatibility with codegen.rs.
    pub fn to_theme_tokens(&self) -> ThemeTokens {
        ThemeTokens {
            background: self.bg.clone(),
            surface: self.surface.clone(),
            text: self.text_primary.clone(),
            accent: self.accent.clone(),
            accent_secondary: self.accent_hover.clone(),
            font_display: self.font_display.clone(),
            font_body: self.font_body.clone(),
            font_mono: "JetBrains Mono".to_string(),
            animation_timing: "cubic-bezier(0.4, 0, 0.2, 1)".to_string(),
        }
    }
}

/// Anti-pattern block to inject into every section prompt.
pub const ANTI_PATTERNS: &str = "\
DO NOT use: Inter, Roboto, Arial, or system fonts. \
DO NOT use: purple-on-white gradients, evenly-distributed rainbow palettes. \
DO NOT use: identical card sizes in a grid. Vary scale for visual hierarchy. \
DO NOT use: rounded-lg shadow-md p-4 as your default container style. \
DO NOT center every element. Use asymmetric layouts where appropriate. \
DO NOT use emoji as visual elements in professional sites.";

/// Positive design directives to inject into every section prompt.
pub const DESIGN_DIRECTIVES: &str = "\
USE the exact CSS custom properties defined above for ALL colors, fonts, radii. \
USE generous whitespace — minimum 96px vertical padding between sections. \
USE one dominant visual element per section (large heading, hero image, feature grid). \
USE the provided shadow style consistently. \
USE font-weight contrast: display at 700-800, body at 400-500. \
USE color-accent ONLY for CTAs and key interactive elements. Max 10% of visible area. \
ENSURE text contrast ratio >= 4.5:1 for body, >= 3:1 for large text.";

// ─── Legacy API (backward compat) ─────────────────────────────────────────

pub fn generate_theme(mood: &str, reference_url: Option<&str>) -> ThemeOutput {
    let lower = mood.to_ascii_lowercase();
    let tokens = match lower.as_str() {
        "corporate" => ThemeTokens {
            background: "#F7F9FC".to_string(),
            surface: "#FFFFFF".to_string(),
            text: "#10213A".to_string(),
            accent: "#1E5EFF".to_string(),
            accent_secondary: "#00A3FF".to_string(),
            font_display: "Sora".to_string(),
            font_body: "DM Sans".to_string(),
            font_mono: "JetBrains Mono".to_string(),
            animation_timing: "cubic-bezier(0.16, 1, 0.3, 1)".to_string(),
        },
        "playful" => ThemeTokens {
            background: "#FFF8E7".to_string(),
            surface: "#FFFFFF".to_string(),
            text: "#1F2937".to_string(),
            accent: "#FF7A00".to_string(),
            accent_secondary: "#FF3D71".to_string(),
            font_display: "Plus Jakarta Sans".to_string(),
            font_body: "Nunito Sans".to_string(),
            font_mono: "Fira Code".to_string(),
            animation_timing: "cubic-bezier(0.22, 1, 0.36, 1)".to_string(),
        },
        "luxury" => ThemeTokens {
            background: "#0F0C0A".to_string(),
            surface: "#1A1511".to_string(),
            text: "#F4E7C5".to_string(),
            accent: "#D4AF37".to_string(),
            accent_secondary: "#C08457".to_string(),
            font_display: "Cormorant Garamond".to_string(),
            font_body: "Lora".to_string(),
            font_mono: "JetBrains Mono".to_string(),
            animation_timing: "cubic-bezier(0.2, 0.9, 0.2, 1)".to_string(),
        },
        "minimal" => ThemeTokens {
            background: "#FAFAFA".to_string(),
            surface: "#FFFFFF".to_string(),
            text: "#111111".to_string(),
            accent: "#111111".to_string(),
            accent_secondary: "#777777".to_string(),
            font_display: "Satoshi".to_string(),
            font_body: "Manrope".to_string(),
            font_mono: "JetBrains Mono".to_string(),
            animation_timing: "cubic-bezier(0.2, 0.8, 0.2, 1)".to_string(),
        },
        "brutalist" => ThemeTokens {
            background: "#FDE047".to_string(),
            surface: "#FFFFFF".to_string(),
            text: "#111111".to_string(),
            accent: "#FF0040".to_string(),
            accent_secondary: "#1D4ED8".to_string(),
            font_display: "Space Grotesk".to_string(),
            font_body: "Space Mono".to_string(),
            font_mono: "Space Mono".to_string(),
            animation_timing: "linear".to_string(),
        },
        "organic" | "warm-organic" | "warm-dark" | "retro" | "editorial" => ThemeTokens {
            background: if lower == "warm-dark" {
                "#1A130F".to_string()
            } else {
                "#F5E8D8".to_string()
            },
            surface: if lower == "warm-dark" {
                "#2B201A".to_string()
            } else {
                "#FFF8EE".to_string()
            },
            text: if lower == "warm-dark" {
                "#F5E8D8".to_string()
            } else {
                "#2E1E12".to_string()
            },
            accent: "#C9752E".to_string(),
            accent_secondary: "#8A4B25".to_string(),
            font_display: "Fraunces".to_string(),
            font_body: "Source Serif 4".to_string(),
            font_mono: "JetBrains Mono".to_string(),
            animation_timing: "cubic-bezier(0.22, 1, 0.36, 1)".to_string(),
        },
        "cyberpunk" => ThemeTokens {
            background: "#07010F".to_string(),
            surface: "#120024".to_string(),
            text: "#E8FFFE".to_string(),
            accent: "#00F5D4".to_string(),
            accent_secondary: "#FF00A8".to_string(),
            font_display: "Audiowide".to_string(),
            font_body: "Space Grotesk".to_string(),
            font_mono: "JetBrains Mono".to_string(),
            animation_timing: "cubic-bezier(0.16, 1, 0.3, 1)".to_string(),
        },
        _ => ThemeTokens {
            background: "#0A1228".to_string(),
            surface: "#101B3D".to_string(),
            text: "#EAF2FF".to_string(),
            accent: "#4CC9F0".to_string(),
            accent_secondary: "#4361EE".to_string(),
            font_display: "Sora".to_string(),
            font_body: "DM Sans".to_string(),
            font_mono: "JetBrains Mono".to_string(),
            animation_timing: "cubic-bezier(0.2, 0.8, 0.2, 1)".to_string(),
        },
    };

    let mut css = String::new();
    css.push_str(":root {\n");
    css.push_str(format!("  --bg: {};\n", tokens.background).as_str());
    css.push_str(format!("  --surface: {};\n", tokens.surface).as_str());
    css.push_str(format!("  --text: {};\n", tokens.text).as_str());
    css.push_str(format!("  --accent: {};\n", tokens.accent).as_str());
    css.push_str(format!("  --accent-2: {};\n", tokens.accent_secondary).as_str());
    css.push_str(format!("  --motion: {};\n", tokens.animation_timing).as_str());
    css.push_str("}\n\n");
    css.push_str("body {\n  background: var(--bg);\n  color: var(--text);\n  font-family: var(--font-body, sans-serif);\n}\n");

    if let Some(reference) = reference_url {
        css.push_str(format!("/* reference: {} */\n", reference).as_str());
    }

    let tailwind_config = format!(
        "import type {{ Config }} from 'tailwindcss';\n\
\n\
const config: Config = {{\n\
  content: ['./index.html', './src/**/*.{{ts,tsx}}'],\n\
  theme: {{\n\
    extend: {{\n\
      colors: {{\n\
        bg: '{bg}',\n\
        surface: '{surface}',\n\
        text: '{text}',\n\
        accent: '{accent}',\n\
        accent2: '{accent2}',\n\
      }},\n\
      fontFamily: {{\n\
        display: ['{font_display}', 'sans-serif'],\n\
        body: ['{font_body}', 'sans-serif'],\n\
        mono: ['{font_mono}', 'monospace'],\n\
      }},\n\
      transitionTimingFunction: {{\n\
        brand: '{timing}',\n\
      }},\n\
      screens: {{\n\
        xs: '420px',\n\
        sm: '640px',\n\
        md: '768px',\n\
        lg: '1024px',\n\
        xl: '1280px',\n\
      }},\n\
    }},\n\
  }},\n\
  plugins: [],\n\
}};\n\
\n\
export default config;\n",
        bg = tokens.background,
        surface = tokens.surface,
        text = tokens.text,
        accent = tokens.accent,
        accent2 = tokens.accent_secondary,
        font_display = tokens.font_display,
        font_body = tokens.font_body,
        font_mono = tokens.font_mono,
        timing = tokens.animation_timing,
    );

    ThemeOutput {
        tokens,
        tailwind_config,
        css,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_tokens_saas() {
        let tokens = select_design_tokens("Landing page for an AI governance platform");
        assert_eq!(tokens.palette_name, "ocean_depth");
        assert_eq!(tokens.mood, Mood::Tech);
        assert!(!tokens.font_display.contains("Inter"));
        assert!(!tokens.font_display.contains("Roboto"));
    }

    #[test]
    fn test_select_tokens_restaurant() {
        let tokens = select_design_tokens("Website for a cozy Italian restaurant");
        assert_eq!(tokens.palette_name, "warm_sand");
        assert!(tokens.radius == "12px" || tokens.radius == "6px");
    }

    #[test]
    fn test_select_tokens_upscale_restaurant() {
        // "upscale" triggers luxury mood, which overrides to rose_gold
        let tokens = select_design_tokens("Website for an upscale Italian restaurant");
        assert_eq!(tokens.mood, Mood::Luxury);
        assert_eq!(tokens.palette_name, "rose_gold");
    }

    #[test]
    fn test_select_tokens_luxury() {
        let tokens = select_design_tokens("Premium luxury brand landing page");
        assert_eq!(tokens.mood, Mood::Luxury);
        assert_eq!(tokens.palette_name, "rose_gold");
        assert!(
            tokens.font_display.contains("Playfair") || tokens.font_display.contains("Cormorant")
        );
    }

    #[test]
    fn test_select_tokens_creative() {
        let tokens = select_design_tokens("Portfolio site for a freelance photographer");
        assert_eq!(tokens.palette_name, "noir");
    }

    #[test]
    fn test_select_tokens_docs() {
        let tokens = select_design_tokens("Developer documentation landing page");
        assert_eq!(tokens.mood, Mood::Tech);
    }

    #[test]
    fn test_css_variables_output() {
        let tokens = select_design_tokens("A SaaS platform landing page");
        let css = tokens.to_css_variables();
        assert!(css.contains("--font-display"));
        assert!(css.contains("--color-bg"));
        assert!(css.contains("--color-accent"));
        assert!(css.contains("--radius"));
        assert!(css.contains("--shadow"));
    }

    #[test]
    fn test_google_fonts_url() {
        let tokens = select_design_tokens("A tech startup landing page");
        let url = tokens.google_fonts_url();
        assert!(url.starts_with("https://fonts.googleapis.com/css2"));
        assert!(url.contains("display=swap"));
    }

    #[test]
    fn test_anti_patterns_no_inter() {
        assert!(ANTI_PATTERNS.contains("Inter"));
        assert!(ANTI_PATTERNS.contains("Roboto"));
    }

    #[test]
    fn test_to_theme_tokens_compat() {
        let tokens = select_design_tokens("A modern SaaS platform");
        let legacy = tokens.to_theme_tokens();
        assert_eq!(legacy.background, tokens.bg);
        assert_eq!(legacy.accent, tokens.accent);
        assert_eq!(legacy.font_display, tokens.font_display);
    }

    #[test]
    fn test_texture_css_overlay() {
        assert!(TextureKind::None.css_overlay().is_empty());
        assert!(TextureKind::Grain.css_overlay().contains("feTurbulence"));
        assert!(TextureKind::GradientMesh
            .css_overlay()
            .contains("radial-gradient"));
        assert!(TextureKind::Grid
            .css_overlay()
            .contains("repeating-linear-gradient"));
    }

    #[test]
    fn test_legacy_generate_theme() {
        let theme = generate_theme("cyberpunk", None);
        assert_eq!(theme.tokens.background, "#07010F");
        assert_eq!(theme.tokens.accent, "#00F5D4");
    }

    #[test]
    fn test_legacy_no_inter_in_new_moods() {
        // New legacy moods should not use Inter
        let corporate = generate_theme("corporate", None);
        assert_eq!(corporate.tokens.font_body, "DM Sans");
        let playful = generate_theme("playful", None);
        assert_eq!(playful.tokens.font_body, "Nunito Sans");
        let minimal = generate_theme("minimal", None);
        assert_eq!(minimal.tokens.font_body, "Manrope");
    }
}
