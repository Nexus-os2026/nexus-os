use serde::{Deserialize, Serialize};

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
            font_body: "Inter".to_string(),
            font_mono: "JetBrains Mono".to_string(),
            animation_timing: "cubic-bezier(0.16, 1, 0.3, 1)".to_string(),
        },
        "playful" => ThemeTokens {
            background: "#FFF8E7".to_string(),
            surface: "#FFFFFF".to_string(),
            text: "#1F2937".to_string(),
            accent: "#FF7A00".to_string(),
            accent_secondary: "#FF3D71".to_string(),
            font_display: "Baloo 2".to_string(),
            font_body: "Nunito".to_string(),
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
            font_mono: "IBM Plex Mono".to_string(),
            animation_timing: "cubic-bezier(0.2, 0.9, 0.2, 1)".to_string(),
        },
        "minimal" => ThemeTokens {
            background: "#FAFAFA".to_string(),
            surface: "#FFFFFF".to_string(),
            text: "#111111".to_string(),
            accent: "#111111".to_string(),
            accent_secondary: "#777777".to_string(),
            font_display: "Manrope".to_string(),
            font_body: "Inter".to_string(),
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
            font_body: "Inter".to_string(),
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
