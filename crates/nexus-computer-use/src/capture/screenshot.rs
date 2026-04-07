use base64::Engine;
use image::GenericImageView;
use sha2::{Digest, Sha256};
use tracing::{debug, info};

use crate::capture::backend::{detect_backend, CaptureBackend};
use crate::error::ComputerUseError;

/// A rectangular region of the screen to capture
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CaptureRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl CaptureRegion {
    /// Create a new capture region, validating dimensions
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Result<Self, ComputerUseError> {
        if width == 0 {
            return Err(ComputerUseError::InvalidRegion {
                reason: "width must be greater than 0".into(),
            });
        }
        if height == 0 {
            return Err(ComputerUseError::InvalidRegion {
                reason: "height must be greater than 0".into(),
            });
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }
}

/// Options for taking a screenshot
#[derive(Debug, Clone)]
pub struct ScreenshotOptions {
    pub region: Option<CaptureRegion>,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub quality: u8,
}

impl Default for ScreenshotOptions {
    fn default() -> Self {
        Self {
            region: None,
            max_width: None,
            max_height: None,
            quality: 90,
        }
    }
}

/// A captured screenshot with metadata
#[derive(Debug, Clone)]
pub struct Screenshot {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub width: u32,
    pub height: u32,
    pub png_bytes: Vec<u8>,
    pub base64: String,
    pub backend: String,
    pub region: Option<CaptureRegion>,
    pub file_size_bytes: usize,
    pub audit_hash: String,
}

impl Screenshot {
    /// Compute SHA-256 hash of PNG bytes
    pub fn compute_hash(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// Encode PNG bytes to base64
    pub fn encode_base64(data: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(data)
    }

    /// Decode base64 back to bytes
    pub fn decode_base64(encoded: &str) -> Result<Vec<u8>, ComputerUseError> {
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| ComputerUseError::ImageError(format!("Base64 decode failed: {e}")))
    }
}

/// Downscale PNG bytes if they exceed max dimensions, returning new PNG bytes
fn downscale_if_needed(
    png_bytes: &[u8],
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> Result<(Vec<u8>, u32, u32), ComputerUseError> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| ComputerUseError::ImageError(format!("Failed to load image: {e}")))?;

    let (orig_w, orig_h) = img.dimensions();
    let mut target_w = orig_w;
    let mut target_h = orig_h;

    if let Some(mw) = max_width {
        if target_w > mw {
            let ratio = mw as f64 / target_w as f64;
            target_w = mw;
            target_h = (target_h as f64 * ratio) as u32;
        }
    }
    if let Some(mh) = max_height {
        if target_h > mh {
            let ratio = mh as f64 / target_h as f64;
            target_h = mh;
            target_w = (target_w as f64 * ratio) as u32;
        }
    }

    // Ensure dimensions are at least 1
    target_w = target_w.max(1);
    target_h = target_h.max(1);

    if target_w == orig_w && target_h == orig_h {
        return Ok((png_bytes.to_vec(), orig_w, orig_h));
    }

    debug!(
        "Downscaling from {}x{} to {}x{}",
        orig_w, orig_h, target_w, target_h
    );

    let resized = img.resize(target_w, target_h, image::imageops::FilterType::Lanczos3);
    let (final_w, final_h) = resized.dimensions();

    let mut buf = std::io::Cursor::new(Vec::new());
    resized
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| {
            ComputerUseError::ImageError(format!("Failed to encode resized image: {e}"))
        })?;

    Ok((buf.into_inner(), final_w, final_h))
}

/// Get image dimensions from PNG bytes without full decode
fn get_image_dimensions(png_bytes: &[u8]) -> Result<(u32, u32), ComputerUseError> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| ComputerUseError::ImageError(format!("Failed to read image: {e}")))?;
    Ok(img.dimensions())
}

/// Take a screenshot using the auto-detected backend
pub async fn take_screenshot(options: ScreenshotOptions) -> Result<Screenshot, ComputerUseError> {
    let backend = detect_backend()?;
    take_screenshot_with_backend(&backend, options).await
}

/// Take a screenshot using a specific backend
pub async fn take_screenshot_with_backend(
    backend: &CaptureBackend,
    options: ScreenshotOptions,
) -> Result<Screenshot, ComputerUseError> {
    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now();

    // Create a tempfile for the capture
    let tmp = tempfile::Builder::new()
        .prefix("nexus-capture-")
        .suffix(".png")
        .tempfile()
        .map_err(|e| ComputerUseError::CaptureError(format!("Failed to create tempfile: {e}")))?;
    let tmp_path = tmp.path().to_path_buf();

    // We need to close the tempfile so the capture tool can write to it
    drop(tmp);

    info!(
        "Capturing screenshot with {} backend, id={}",
        backend.kind, id
    );

    // Execute the capture
    backend.capture(&tmp_path, options.region.as_ref()).await?;

    // Read the PNG bytes
    let png_bytes = tokio::fs::read(&tmp_path).await.map_err(|e| {
        ComputerUseError::CaptureError(format!("Failed to read capture output: {e}"))
    })?;

    // Clean up tempfile
    let _ = tokio::fs::remove_file(&tmp_path).await;

    if png_bytes.is_empty() {
        return Err(ComputerUseError::CaptureError(
            "Capture produced empty file".into(),
        ));
    }

    // Get dimensions and optionally downscale
    let (final_bytes, width, height) =
        if options.max_width.is_some() || options.max_height.is_some() {
            downscale_if_needed(&png_bytes, options.max_width, options.max_height)?
        } else {
            let (w, h) = get_image_dimensions(&png_bytes)?;
            (png_bytes, w, h)
        };

    let audit_hash = Screenshot::compute_hash(&final_bytes);
    let b64 = Screenshot::encode_base64(&final_bytes);
    let file_size_bytes = final_bytes.len();

    info!(
        "Screenshot captured: {}x{}, {} bytes, hash={}",
        width,
        height,
        file_size_bytes,
        &audit_hash[..16]
    );

    Ok(Screenshot {
        id,
        timestamp,
        width,
        height,
        png_bytes: final_bytes,
        base64: b64,
        backend: backend.kind.to_string(),
        region: options.region,
        file_size_bytes,
        audit_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screenshot_struct_creation() {
        let data = vec![1, 2, 3, 4];
        let hash = Screenshot::compute_hash(&data);
        let b64 = Screenshot::encode_base64(&data);

        let ss = Screenshot {
            id: "test-id".into(),
            timestamp: chrono::Utc::now(),
            width: 1920,
            height: 1080,
            png_bytes: data.clone(),
            base64: b64.clone(),
            backend: "grim".into(),
            region: None,
            file_size_bytes: data.len(),
            audit_hash: hash.clone(),
        };

        assert_eq!(ss.id, "test-id");
        assert_eq!(ss.width, 1920);
        assert_eq!(ss.height, 1080);
        assert_eq!(ss.png_bytes, data);
        assert_eq!(ss.base64, b64);
        assert_eq!(ss.audit_hash, hash);
        assert_eq!(ss.file_size_bytes, 4);
        assert!(ss.region.is_none());
    }

    #[test]
    fn test_screenshot_hash_deterministic() {
        let data = vec![10, 20, 30, 40, 50];
        let hash1 = Screenshot::compute_hash(&data);
        let hash2 = Screenshot::compute_hash(&data);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
    }

    #[test]
    fn test_screenshot_hash_different_data() {
        let hash1 = Screenshot::compute_hash(&[1, 2, 3]);
        let hash2 = Screenshot::compute_hash(&[4, 5, 6]);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_screenshot_base64_roundtrip() {
        let data = vec![0u8, 1, 2, 3, 100, 200, 255];
        let encoded = Screenshot::encode_base64(&data);
        let decoded = Screenshot::decode_base64(&encoded).unwrap();
        assert_eq!(data, decoded);
    }

    #[test]
    fn test_screenshot_options_default() {
        let opts = ScreenshotOptions::default();
        assert!(opts.region.is_none());
        assert!(opts.max_width.is_none());
        assert!(opts.max_height.is_none());
        assert_eq!(opts.quality, 90);
    }

    #[test]
    fn test_capture_region_valid() {
        let region = CaptureRegion::new(0, 0, 100, 200).unwrap();
        assert_eq!(region.x, 0);
        assert_eq!(region.y, 0);
        assert_eq!(region.width, 100);
        assert_eq!(region.height, 200);
    }

    #[test]
    fn test_capture_region_zero_width_rejected() {
        let result = CaptureRegion::new(0, 0, 0, 100);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("width"));
    }

    #[test]
    fn test_capture_region_zero_height_rejected() {
        let result = CaptureRegion::new(0, 0, 100, 0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("height"));
    }

    #[test]
    fn test_downscale_if_needed_no_change() {
        // Create a small 2x2 red PNG in memory
        let img = image::RgbImage::from_fn(2, 2, |_, _| image::Rgb([255, 0, 0]));
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        let png_bytes = buf.into_inner();

        let (result, w, h) = downscale_if_needed(&png_bytes, Some(100), Some(100)).unwrap();
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        // Should return original bytes when no downscale needed
        assert_eq!(result, png_bytes);
    }

    #[test]
    fn test_downscale_if_needed_shrinks() {
        // Create a 100x50 image
        let img = image::RgbImage::from_fn(100, 50, |_, _| image::Rgb([0, 128, 255]));
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        let png_bytes = buf.into_inner();

        let (_, w, h) = downscale_if_needed(&png_bytes, Some(50), None).unwrap();
        assert!(w <= 50);
        assert!(h <= 25);
    }

    #[test]
    fn test_base64_decode_invalid() {
        let result = Screenshot::decode_base64("not-valid-base64!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_image_dimensions() {
        let img = image::RgbImage::from_fn(320, 240, |_, _| image::Rgb([0, 0, 0]));
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        let png_bytes = buf.into_inner();

        let (w, h) = get_image_dimensions(&png_bytes).unwrap();
        assert_eq!(w, 320);
        assert_eq!(h, 240);
    }

    #[test]
    #[ignore]
    fn test_real_capture() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let opts = ScreenshotOptions::default();
            let ss = take_screenshot(opts).await.unwrap();
            assert!(ss.width > 0);
            assert!(ss.height > 0);
            assert!(!ss.png_bytes.is_empty());
            assert!(!ss.base64.is_empty());
            assert_eq!(ss.audit_hash.len(), 64);
            println!(
                "Captured: {}x{}, {} bytes, backend={}, hash={}",
                ss.width, ss.height, ss.file_size_bytes, ss.backend, ss.audit_hash
            );
        });
    }

    #[test]
    #[ignore]
    fn test_real_capture_with_region() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let opts = ScreenshotOptions {
                region: Some(CaptureRegion::new(0, 0, 200, 200).unwrap()),
                ..Default::default()
            };
            let ss = take_screenshot(opts).await.unwrap();
            assert!(ss.width > 0);
            assert!(ss.height > 0);
            println!(
                "Region capture: {}x{}, {} bytes",
                ss.width, ss.height, ss.file_size_bytes
            );
        });
    }

    #[test]
    #[ignore]
    fn test_backend_detection_real() {
        let backend = detect_backend().unwrap();
        println!(
            "Backend: {}, Display: {}",
            backend.kind, backend.display_server
        );
    }
}
