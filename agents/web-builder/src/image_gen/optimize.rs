//! Image Optimization — resize, format conversion, and srcset generation.
//!
//! Uses the `image` crate for pure-Rust image processing.

use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OptimizeError {
    #[error("failed to open image: {0}")]
    OpenFailed(String),
    #[error("failed to save image: {0}")]
    SaveFailed(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result of optimizing a single image into multiple sizes.
#[derive(Debug, Clone)]
pub struct OptimizedResult {
    pub primary_name: String,
    pub variants: Vec<OptimizedVariant>,
}

/// A single optimized variant (one size in the srcset).
#[derive(Debug, Clone)]
pub struct OptimizedVariant {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
}

/// Optimize an input image: resize to target sizes and save.
///
/// For each target size, generates a resized variant.
/// The primary image is the first (largest) target size.
/// If `transparent` is true, saves as PNG; otherwise saves as JPEG (WebP
/// requires the image crate's `webp` feature which may not be available).
pub fn optimize_image(
    input_path: &Path,
    output_dir: &Path,
    base_name: &str,
    target_sizes: &[(u32, u32)],
    transparent: bool,
) -> Result<OptimizedResult, OptimizeError> {
    let img = image::open(input_path).map_err(|e| OptimizeError::OpenFailed(e.to_string()))?;

    let ext = if transparent { "png" } else { "webp" };
    let mut variants = Vec::new();

    for &(tw, th) in target_sizes {
        let resized = img.resize_exact(tw, th, image::imageops::FilterType::Lanczos3);
        let variant_name = format!("{base_name}-{tw}.{ext}");
        let variant_path = output_dir.join(&variant_name);

        resized
            .save(&variant_path)
            .map_err(|e| OptimizeError::SaveFailed(e.to_string()))?;

        let size_bytes = std::fs::metadata(&variant_path)
            .map(|m| m.len())
            .unwrap_or(0);

        variants.push(OptimizedVariant {
            name: variant_name,
            width: tw,
            height: th,
            size_bytes,
        });
    }

    let primary_name = variants
        .first()
        .map(|v| v.name.clone())
        .unwrap_or_else(|| format!("{base_name}.{ext}"));

    Ok(OptimizedResult {
        primary_name,
        variants,
    })
}

/// Create a simple test image (solid color) for testing purposes.
#[cfg(test)]
pub fn create_test_image(path: &Path, width: u32, height: u32) {
    let img = image::RgbaImage::from_fn(width, height, |_x, _y| {
        image::Rgba([100, 149, 237, 255]) // cornflower blue
    });
    img.save(path).expect("failed to save test image");
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resize_generates_multiple_sizes() {
        let dir = std::env::temp_dir().join(format!("nexus-opt-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let input = dir.join("source.png");
        create_test_image(&input, 800, 600);

        let targets = vec![(400, 300), (200, 150), (100, 75)];
        let result = optimize_image(&input, &dir, "test", &targets, false).unwrap();

        assert_eq!(result.variants.len(), 3);
        for variant in &result.variants {
            assert!(
                dir.join(&variant.name).exists(),
                "missing: {}",
                variant.name
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_output_format_png() {
        let dir = std::env::temp_dir().join(format!("nexus-opt-png-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let input = dir.join("source.png");
        create_test_image(&input, 200, 200);

        let result = optimize_image(&input, &dir, "icon", &[(128, 128)], true).unwrap();

        assert!(result.primary_name.ends_with(".png"));
        for variant in &result.variants {
            assert!(variant.name.ends_with(".png"));
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_output_format_webp() {
        let dir = std::env::temp_dir().join(format!("nexus-opt-webp-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let input = dir.join("source.png");
        create_test_image(&input, 400, 300);

        let result = optimize_image(&input, &dir, "hero", &[(400, 300)], false).unwrap();

        assert!(result.primary_name.ends_with(".webp"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_dimensions_correct() {
        let dir = std::env::temp_dir().join(format!("nexus-opt-dim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let input = dir.join("source.png");
        create_test_image(&input, 1920, 1080);

        let targets = vec![(640, 360), (320, 180)];
        let result = optimize_image(&input, &dir, "resized", &targets, false).unwrap();

        assert_eq!(result.variants[0].width, 640);
        assert_eq!(result.variants[0].height, 360);
        assert_eq!(result.variants[1].width, 320);
        assert_eq!(result.variants[1].height, 180);

        // Verify actual image dimensions
        let img = image::open(dir.join(&result.variants[0].name)).unwrap();
        assert_eq!(img.width(), 640);
        assert_eq!(img.height(), 360);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_optimize_nonexistent_input() {
        let dir = std::env::temp_dir().join(format!("nexus-opt-nofile-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let result = optimize_image(
            &dir.join("does_not_exist.png"),
            &dir,
            "fail",
            &[(100, 100)],
            false,
        );
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
