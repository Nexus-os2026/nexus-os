//! Prompt Enhancement — deterministic string formatting for better image generation.
//!
//! No LLM needed. Appends context-appropriate quality/style hints based on
//! the image type and section context.

use super::ImageType;

/// Enhance a raw image prompt with context-appropriate quality hints.
///
/// This is deterministic string formatting — no LLM call.
pub fn enhance_prompt(raw: &str, image_type: &ImageType, section_id: &str) -> String {
    let base = raw.trim();
    if base.is_empty() {
        return default_prompt(image_type, section_id);
    }

    let suffix = match image_type {
        ImageType::Hero => {
            "wide cinematic composition, professional lighting, high quality, safe for work, professional"
        }
        ImageType::Feature => {
            "simple flat icon, minimal design, clean lines, safe for work, professional"
        }
        ImageType::Product => {
            "clean white background, product photography, studio lighting, high detail, safe for work, professional"
        }
        ImageType::Gallery => {
            "high quality photograph, natural lighting, professional composition, safe for work"
        }
        ImageType::Background => {
            "abstract, subtle, seamless pattern, muted colors, safe for work, professional"
        }
        ImageType::Avatar => {
            "professional headshot, studio portrait, neutral background, safe for work"
        }
        ImageType::Logo => {
            "simple logo design, minimal, vector style, transparent background, safe for work, professional"
        }
    };

    format!("{base}, {suffix}")
}

/// Generate a default prompt when the raw prompt is empty.
fn default_prompt(image_type: &ImageType, section_id: &str) -> String {
    let section = section_id.replace('_', " ");
    match image_type {
        ImageType::Hero => format!("Professional hero image for {section} section, wide cinematic composition, modern design, safe for work"),
        ImageType::Feature => format!("Simple flat icon for {section}, minimal design, clean lines, safe for work"),
        ImageType::Product => format!("Product photo for {section}, clean white background, studio lighting, safe for work"),
        ImageType::Gallery => format!("Gallery photo for {section}, professional photography, safe for work"),
        ImageType::Background => format!("Abstract background pattern for {section}, subtle, muted colors, safe for work"),
        ImageType::Avatar => format!("Professional avatar for {section}, studio portrait, safe for work"),
        ImageType::Logo => format!("Simple logo for {section}, minimal vector design, transparent background, safe for work"),
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhance_hero_prompt() {
        let result = enhance_prompt("Modern SaaS dashboard", &ImageType::Hero, "hero");
        assert!(result.contains("Modern SaaS dashboard"));
        assert!(result.contains("cinematic"));
        assert!(result.contains("professional"));
    }

    #[test]
    fn test_enhance_icon_prompt() {
        let result = enhance_prompt("Rocket icon", &ImageType::Feature, "features");
        assert!(result.contains("Rocket icon"));
        assert!(result.contains("flat icon"));
        assert!(result.contains("minimal"));
    }

    #[test]
    fn test_enhance_product_prompt() {
        let result = enhance_prompt("Organic cotton dress", &ImageType::Product, "products");
        assert!(result.contains("Organic cotton dress"));
        assert!(result.contains("studio lighting"));
        assert!(result.contains("white background"));
    }

    #[test]
    fn test_enhance_preserves_original() {
        let original = "A beautiful sunset over mountains";
        let result = enhance_prompt(original, &ImageType::Gallery, "gallery");
        assert!(result.starts_with(original));
    }

    #[test]
    fn test_enhance_empty_prompt_gets_default() {
        let result = enhance_prompt("", &ImageType::Hero, "hero");
        assert!(result.contains("hero"));
        assert!(result.contains("safe for work"));
    }

    #[test]
    fn test_enhance_whitespace_prompt_gets_default() {
        let result = enhance_prompt("   ", &ImageType::Product, "products");
        assert!(result.contains("Product photo"));
    }

    #[test]
    fn test_all_types_include_safe_for_work() {
        let types = [
            ImageType::Hero,
            ImageType::Feature,
            ImageType::Product,
            ImageType::Gallery,
            ImageType::Background,
            ImageType::Avatar,
            ImageType::Logo,
        ];
        for img_type in &types {
            let result = enhance_prompt("test", img_type, "test");
            assert!(
                result.contains("safe for work"),
                "ImageType::{img_type:?} missing safe-for-work hint"
            );
        }
    }
}
