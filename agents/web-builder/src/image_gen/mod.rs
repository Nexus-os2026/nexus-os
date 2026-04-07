//! Image Generation Module — orchestrates local SD, API, and placeholder tiers.
//!
//! Tier selection: local SD → API (DALL-E) → SVG placeholder.
//! Never fails — always produces at least a placeholder.

pub mod api;
pub mod local;
pub mod optimize;
pub mod placeholder;
pub mod prompt;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ImageGenError {
    #[error("all generation tiers failed: {0}")]
    AllTiersFailed(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

// ─── Types ──────────────────────────────────────────────────────────────────

/// The type of image being generated — determines dimensions and prompt hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageType {
    Hero,
    Feature,
    Product,
    Gallery,
    Background,
    Avatar,
    Logo,
}

impl ImageType {
    /// Default dimensions (width, height) for this image type.
    pub fn default_dimensions(&self) -> (u32, u32) {
        match self {
            ImageType::Hero => (1920, 1080),
            ImageType::Feature => (128, 128),
            ImageType::Product => (800, 800),
            ImageType::Gallery => (1200, 800),
            ImageType::Background => (1920, 1080),
            ImageType::Avatar => (256, 256),
            ImageType::Logo => (512, 512),
        }
    }

    /// Srcset target sizes for responsive images.
    pub fn srcset_sizes(&self) -> Vec<(u32, u32)> {
        match self {
            ImageType::Hero => vec![(1920, 1080), (1280, 720), (640, 360)],
            ImageType::Feature => vec![(128, 128), (64, 64)],
            ImageType::Product => vec![(800, 800), (400, 400), (200, 200)],
            ImageType::Gallery => vec![(1200, 800), (600, 400)],
            ImageType::Background => vec![(1920, 1080), (960, 540)],
            ImageType::Avatar => vec![(256, 256), (128, 128)],
            ImageType::Logo => vec![(512, 512), (256, 256)],
        }
    }
}

/// Aspect ratio hint for image generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AspectRatio {
    Wide,      // 16:9
    Square,    // 1:1
    Portrait,  // 3:4
    Panoramic, // 21:9
}

impl AspectRatio {
    pub fn from_image_type(image_type: ImageType) -> Self {
        match image_type {
            ImageType::Hero | ImageType::Background => AspectRatio::Wide,
            ImageType::Feature | ImageType::Product | ImageType::Avatar | ImageType::Logo => {
                AspectRatio::Square
            }
            ImageType::Gallery => AspectRatio::Wide,
        }
    }
}

/// The format of a generated image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    WebP,
    Png,
    Svg,
}

impl ImageFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::WebP => "webp",
            ImageFormat::Png => "png",
            ImageFormat::Svg => "svg",
        }
    }
}

/// A request to generate an image for a specific slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRequest {
    pub prompt: String,
    pub slot_name: String,
    pub section_id: String,
    pub image_type: ImageType,
    pub aspect_ratio: AspectRatio,
}

/// A successfully generated image with all metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedImage {
    pub primary_path: String,
    pub srcset_paths: Vec<(String, u32)>,
    pub alt_text: String,
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    pub generation_method: String,
    pub cost: f64,
}

impl GeneratedImage {
    /// Build an HTML `<img>` tag with srcset, alt, dimensions, and lazy loading.
    pub fn to_img_tag(&self) -> String {
        let srcset_attr = if !self.srcset_paths.is_empty() {
            let parts: Vec<String> = self
                .srcset_paths
                .iter()
                .map(|(path, w)| format!("{path} {w}w"))
                .collect();
            format!(" srcset=\"{}\"", parts.join(", "))
        } else {
            String::new()
        };

        let sizes_attr = if !self.srcset_paths.is_empty() {
            " sizes=\"100vw\""
        } else {
            ""
        };

        format!(
            "<img src=\"{}\" alt=\"{}\"{}{} width=\"{}\" height=\"{}\" loading=\"lazy\" style=\"max-width:100%;height:auto;\">",
            self.primary_path,
            crate::slot_schema::html_escape(&self.alt_text),
            srcset_attr,
            sizes_attr,
            self.width,
            self.height,
        )
    }
}

/// Status of available image generation tiers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenStatus {
    pub local_sd_available: bool,
    pub api_available: bool,
    pub placeholder_available: bool, // always true
}

impl Default for ImageGenStatus {
    fn default() -> Self {
        Self {
            local_sd_available: false,
            api_available: false,
            placeholder_available: true,
        }
    }
}

/// Configuration for image generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenConfig {
    pub openai_api_key: Option<String>,
    pub prefer_local: bool,
    pub placeholder_only: bool,
    /// Maximum cost in USD for a single API image generation call.
    /// Defaults to $1.00 if not set. Defense-in-depth fuel gate.
    #[serde(default)]
    pub fuel_budget_usd: Option<f64>,
}

impl Default for ImageGenConfig {
    fn default() -> Self {
        Self {
            openai_api_key: None,
            prefer_local: true,
            placeholder_only: false,
            fuel_budget_usd: None,
        }
    }
}

/// Theme colors passed to placeholder generation.
#[derive(Debug, Clone, Default)]
pub struct ThemeColors {
    pub bg: String,
    pub bg_secondary: String,
    pub text: String,
    pub text_secondary: String,
    pub primary: String,
    pub accent: String,
}

// ─── Slot → ImageType Inference ─────────────────────────────────────────────

/// Infer ImageType from a slot name.
pub fn infer_image_type(slot_name: &str) -> ImageType {
    let lower = slot_name.to_lowercase();
    if lower.contains("hero") || lower.contains("banner") {
        ImageType::Hero
    } else if lower.contains("product") {
        ImageType::Product
    } else if lower.contains("gallery") {
        ImageType::Gallery
    } else if lower.contains("avatar") || lower.contains("headshot") {
        ImageType::Avatar
    } else if lower.contains("logo") {
        ImageType::Logo
    } else if lower.contains("background") || lower.contains("bg") {
        ImageType::Background
    } else if lower.contains("icon") || lower.contains("feature") || lower.contains("category") {
        ImageType::Feature
    } else if lower.contains("media") {
        ImageType::Hero
    } else {
        ImageType::Gallery
    }
}

// ─── Orchestrator ───────────────────────────────────────────────────────────

/// Generate an image using the tiered approach: local → API → placeholder.
///
/// Never fails — always produces at least a placeholder SVG.
pub async fn generate_image(
    request: &ImageRequest,
    output_dir: &Path,
    config: &ImageGenConfig,
    theme_colors: &ThemeColors,
) -> GeneratedImage {
    let (width, height) = request.image_type.default_dimensions();
    let enhanced_prompt =
        prompt::enhance_prompt(&request.prompt, &request.image_type, &request.section_id);

    // Ensure images directory exists
    let images_dir = output_dir.join("images");
    let _ = std::fs::create_dir_all(&images_dir);

    let base_name = sanitize_filename(&request.slot_name);

    // Tier 1: Local SD (if enabled and not placeholder-only)
    if !config.placeholder_only && config.prefer_local {
        if let Ok(()) = local::generate_local(
            &enhanced_prompt,
            width,
            height,
            &images_dir.join(format!("{base_name}.png")),
        )
        .await
        {
            // Optimize the generated image
            if let Ok(optimized) = optimize::optimize_image(
                &images_dir.join(format!("{base_name}.png")),
                &images_dir,
                &base_name,
                &request.image_type.srcset_sizes(),
                needs_transparency(request.image_type),
            ) {
                return GeneratedImage {
                    primary_path: format!("images/{}", optimized.primary_name),
                    srcset_paths: optimized
                        .variants
                        .iter()
                        .map(|v| (format!("images/{}", v.name), v.width))
                        .collect(),
                    alt_text: request.prompt.clone(),
                    width,
                    height,
                    format: if needs_transparency(request.image_type) {
                        ImageFormat::Png
                    } else {
                        ImageFormat::WebP
                    },
                    generation_method: "local_sd".into(),
                    cost: 0.0,
                };
            }
        }
    }

    // Tier 2: API generation (if key available and not placeholder-only)
    if !config.placeholder_only {
        if let Some(ref api_key) = config.openai_api_key {
            if let Ok(cost) = api::generate_api(
                &enhanced_prompt,
                width,
                height,
                &images_dir.join(format!("{base_name}.png")),
                api_key,
                config.fuel_budget_usd.unwrap_or(1.0),
            )
            .await
            {
                // Optimize the generated image
                if let Ok(optimized) = optimize::optimize_image(
                    &images_dir.join(format!("{base_name}.png")),
                    &images_dir,
                    &base_name,
                    &request.image_type.srcset_sizes(),
                    needs_transparency(request.image_type),
                ) {
                    return GeneratedImage {
                        primary_path: format!("images/{}", optimized.primary_name),
                        srcset_paths: optimized
                            .variants
                            .iter()
                            .map(|v| (format!("images/{}", v.name), v.width))
                            .collect(),
                        alt_text: request.prompt.clone(),
                        width,
                        height,
                        format: if needs_transparency(request.image_type) {
                            ImageFormat::Png
                        } else {
                            ImageFormat::WebP
                        },
                        generation_method: "api_openai".into(),
                        cost,
                    };
                }
            }
        }
    }

    // Tier 0: Placeholder (always works)
    let svg = placeholder::generate_placeholder(&request.prompt, width, height, theme_colors);
    let svg_name = format!("{base_name}.svg");
    let svg_path = images_dir.join(&svg_name);
    eprintln!(
        "[nexus-builder][governance] write_placeholder_svg path={}",
        svg_path.display()
    );
    let _ = std::fs::write(&svg_path, &svg);

    GeneratedImage {
        primary_path: format!("images/{svg_name}"),
        srcset_paths: vec![],
        alt_text: request.prompt.clone(),
        width,
        height,
        format: ImageFormat::Svg,
        generation_method: "placeholder".into(),
        cost: 0.0,
    }
}

/// Generate images for all ImagePrompt slots in a content payload.
pub async fn generate_all_images(
    payload: &crate::content_payload::ContentPayload,
    schema: &crate::slot_schema::TemplateSchema,
    output_dir: &Path,
    config: &ImageGenConfig,
    theme_colors: &ThemeColors,
    on_progress: &dyn Fn(usize, usize, &str),
) -> Vec<(String, String, GeneratedImage)> {
    let mut requests: Vec<(String, String, ImageRequest)> = Vec::new();

    for section in &payload.sections {
        let section_schema = schema
            .sections
            .iter()
            .find(|s| s.section_id == section.section_id);

        if let Some(ss) = section_schema {
            for (slot_name, value) in &section.slots {
                if let Some(constraint) = ss.slots.get(slot_name.as_str()) {
                    if constraint.slot_type == crate::slot_schema::SlotType::ImagePrompt {
                        let image_type = infer_image_type(slot_name);
                        requests.push((
                            section.section_id.clone(),
                            slot_name.clone(),
                            ImageRequest {
                                prompt: value.clone(),
                                slot_name: slot_name.clone(),
                                section_id: section.section_id.clone(),
                                image_type,
                                aspect_ratio: AspectRatio::from_image_type(image_type),
                            },
                        ));
                    }
                }
            }
        }
    }

    let total = requests.len();
    let mut results = Vec::new();

    for (i, (section_id, slot_name, request)) in requests.into_iter().enumerate() {
        on_progress(i + 1, total, &request.prompt);
        let generated = generate_image(&request, output_dir, config, theme_colors).await;
        results.push((section_id, slot_name, generated));
    }

    results
}

/// Check the current availability of image generation tiers.
pub async fn check_status(config: &ImageGenConfig) -> ImageGenStatus {
    ImageGenStatus {
        local_sd_available: local::is_available().await,
        api_available: config.openai_api_key.is_some(),
        placeholder_available: true,
    }
}

/// Build a lookup map from (section_id, slot_name) → GeneratedImage for the assembler.
pub fn build_image_map(
    generated: &[(String, String, GeneratedImage)],
) -> HashMap<(String, String), GeneratedImage> {
    let mut map = HashMap::new();
    for (section_id, slot_name, img) in generated {
        map.insert((section_id.clone(), slot_name.clone()), img.clone());
    }
    map
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn needs_transparency(image_type: ImageType) -> bool {
    matches!(image_type, ImageType::Logo | ImageType::Feature)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_type_dimensions() {
        assert_eq!(ImageType::Hero.default_dimensions(), (1920, 1080));
        assert_eq!(ImageType::Feature.default_dimensions(), (128, 128));
        assert_eq!(ImageType::Product.default_dimensions(), (800, 800));
        assert_eq!(ImageType::Gallery.default_dimensions(), (1200, 800));
        assert_eq!(ImageType::Avatar.default_dimensions(), (256, 256));
        assert_eq!(ImageType::Logo.default_dimensions(), (512, 512));
    }

    #[test]
    fn test_aspect_ratio_from_image_type() {
        assert_eq!(
            AspectRatio::from_image_type(ImageType::Hero),
            AspectRatio::Wide
        );
        assert_eq!(
            AspectRatio::from_image_type(ImageType::Product),
            AspectRatio::Square
        );
        assert_eq!(
            AspectRatio::from_image_type(ImageType::Feature),
            AspectRatio::Square
        );
        assert_eq!(
            AspectRatio::from_image_type(ImageType::Gallery),
            AspectRatio::Wide
        );
    }

    #[test]
    fn test_infer_image_type() {
        assert_eq!(infer_image_type("hero_image"), ImageType::Hero);
        assert_eq!(infer_image_type("product_1_image"), ImageType::Product);
        assert_eq!(infer_image_type("gallery_3"), ImageType::Gallery);
        assert_eq!(infer_image_type("category_1_image"), ImageType::Feature);
        assert_eq!(infer_image_type("media"), ImageType::Hero);
        assert_eq!(infer_image_type("some_unknown"), ImageType::Gallery);
    }

    #[test]
    fn test_image_format_extension() {
        assert_eq!(ImageFormat::WebP.extension(), "webp");
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::Svg.extension(), "svg");
    }

    #[test]
    fn test_srcset_sizes_hero() {
        let sizes = ImageType::Hero.srcset_sizes();
        assert_eq!(sizes.len(), 3);
        assert_eq!(sizes[0], (1920, 1080));
        assert_eq!(sizes[1], (1280, 720));
        assert_eq!(sizes[2], (640, 360));
    }

    #[test]
    fn test_generated_image_to_img_tag() {
        let img = GeneratedImage {
            primary_path: "images/hero.webp".into(),
            srcset_paths: vec![
                ("images/hero-1920.webp".into(), 1920),
                ("images/hero-1280.webp".into(), 1280),
                ("images/hero-640.webp".into(), 640),
            ],
            alt_text: "A modern dashboard".into(),
            width: 1920,
            height: 1080,
            format: ImageFormat::WebP,
            generation_method: "placeholder".into(),
            cost: 0.0,
        };
        let tag = img.to_img_tag();
        assert!(tag.contains("src=\"images/hero.webp\""));
        assert!(tag.contains("srcset=\"images/hero-1920.webp 1920w"));
        assert!(tag.contains("alt=\"A modern dashboard\""));
        assert!(tag.contains("loading=\"lazy\""));
        assert!(tag.contains("width=\"1920\""));
        assert!(tag.contains("height=\"1080\""));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("hero_image"), "hero_image");
        assert_eq!(sanitize_filename("product 1"), "product_1");
        assert_eq!(sanitize_filename("file/name"), "file_name");
    }

    #[test]
    fn test_needs_transparency() {
        assert!(needs_transparency(ImageType::Logo));
        assert!(needs_transparency(ImageType::Feature));
        assert!(!needs_transparency(ImageType::Hero));
        assert!(!needs_transparency(ImageType::Product));
    }

    #[test]
    fn test_image_gen_status_default() {
        let status = ImageGenStatus::default();
        assert!(!status.local_sd_available);
        assert!(!status.api_available);
        assert!(status.placeholder_available);
    }

    #[test]
    fn test_tier_fallback_to_placeholder() {
        // With placeholder_only config, always get placeholder
        let config = ImageGenConfig {
            openai_api_key: None,
            prefer_local: false,
            placeholder_only: true,
            fuel_budget_usd: None,
        };
        let request = ImageRequest {
            prompt: "Test image".into(),
            slot_name: "hero_image".into(),
            section_id: "hero".into(),
            image_type: ImageType::Hero,
            aspect_ratio: AspectRatio::Wide,
        };
        let theme = ThemeColors::default();
        let dir = std::env::temp_dir().join(format!("nexus-img-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(generate_image(&request, &dir, &config, &theme));

        assert_eq!(result.generation_method, "placeholder");
        assert_eq!(result.cost, 0.0);
        assert_eq!(result.format, ImageFormat::Svg);
        assert!(result.primary_path.ends_with(".svg"));
        // Verify file exists on disk
        assert!(dir.join(&result.primary_path).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_build_image_map() {
        let generated = vec![(
            "hero".to_string(),
            "hero_image".to_string(),
            GeneratedImage {
                primary_path: "images/hero.svg".into(),
                srcset_paths: vec![],
                alt_text: "Hero".into(),
                width: 1920,
                height: 1080,
                format: ImageFormat::Svg,
                generation_method: "placeholder".into(),
                cost: 0.0,
            },
        )];
        let map = build_image_map(&generated);
        assert!(map.contains_key(&("hero".to_string(), "hero_image".to_string())));
    }
}
