use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorTokens {
    pub primary: String,
    pub secondary: String,
    pub background: String,
    pub foreground: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypographyTokens {
    pub display: String,
    pub body: String,
    pub mono: String,
    pub scale_px: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpacingTokens {
    pub scale_px: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowTokens {
    pub sm: String,
    pub md: String,
    pub lg: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorderTokens {
    pub sm_radius_px: u8,
    pub md_radius_px: u8,
    pub lg_radius_px: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationTokens {
    pub fast_ms: u16,
    pub normal_ms: u16,
    pub slow_ms: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignTokens {
    pub colors: ColorTokens,
    pub typography: TypographyTokens,
    pub spacing: SpacingTokens,
    pub shadows: ShadowTokens,
    pub borders: BorderTokens,
    pub animations: AnimationTokens,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenExports {
    pub css_variables: String,
    pub tailwind_config: String,
    pub json_tokens: String,
}

pub fn generate_tokens(brand_description: &str) -> DesignTokens {
    let lower = brand_description.to_ascii_lowercase();
    let colors = if lower.contains("blue") {
        ColorTokens {
            primary: "#2563EB".to_string(),
            secondary: "#1D4ED8".to_string(),
            background: "#F8FAFC".to_string(),
            foreground: "#0F172A".to_string(),
        }
    } else if lower.contains("cyberpunk") {
        ColorTokens {
            primary: "#00F5D4".to_string(),
            secondary: "#FF00A8".to_string(),
            background: "#07010F".to_string(),
            foreground: "#E2E8F0".to_string(),
        }
    } else {
        ColorTokens {
            primary: "#7C3AED".to_string(),
            secondary: "#4F46E5".to_string(),
            background: "#FFFFFF".to_string(),
            foreground: "#111827".to_string(),
        }
    };

    DesignTokens {
        colors,
        typography: TypographyTokens {
            display: "Sora".to_string(),
            body: "Inter".to_string(),
            mono: "JetBrains Mono".to_string(),
            scale_px: vec![12, 14, 16, 20, 24, 32],
        },
        spacing: SpacingTokens {
            scale_px: vec![4, 8, 12, 16, 24, 32, 40],
        },
        shadows: ShadowTokens {
            sm: "0 1px 2px rgba(15, 23, 42, 0.08)".to_string(),
            md: "0 8px 24px rgba(15, 23, 42, 0.12)".to_string(),
            lg: "0 16px 40px rgba(15, 23, 42, 0.18)".to_string(),
        },
        borders: BorderTokens {
            sm_radius_px: 8,
            md_radius_px: 12,
            lg_radius_px: 16,
        },
        animations: AnimationTokens {
            fast_ms: 120,
            normal_ms: 220,
            slow_ms: 360,
        },
    }
}

pub fn export_tokens(tokens: &DesignTokens) -> Result<TokenExports, AgentError> {
    let css_variables = format!(
        ":root {{
  --color-primary: {};
  --color-secondary: {};
  --color-background: {};
  --color-foreground: {};
  --radius-sm: {}px;
  --radius-md: {}px;
  --radius-lg: {}px;
}}",
        tokens.colors.primary,
        tokens.colors.secondary,
        tokens.colors.background,
        tokens.colors.foreground,
        tokens.borders.sm_radius_px,
        tokens.borders.md_radius_px,
        tokens.borders.lg_radius_px
    );

    let tailwind_config = format!(
        "export default {{
  theme: {{
    extend: {{
      colors: {{
        primary: \"{}\",
        secondary: \"{}\",
        background: \"{}\",
        foreground: \"{}\",
      }},
      borderRadius: {{
        sm: \"{}px\",
        md: \"{}px\",
        lg: \"{}px\",
      }},
    }},
  }},
}};",
        tokens.colors.primary,
        tokens.colors.secondary,
        tokens.colors.background,
        tokens.colors.foreground,
        tokens.borders.sm_radius_px,
        tokens.borders.md_radius_px,
        tokens.borders.lg_radius_px
    );

    let json_tokens = serde_json::to_string_pretty(tokens).map_err(|error| {
        AgentError::SupervisorError(format!("failed to serialize design tokens: {error}"))
    })?;

    Ok(TokenExports {
        css_variables,
        tailwind_config,
        json_tokens,
    })
}
