//! QR Code Generation — produce inline SVG QR codes for deploy URLs.
//!
//! Uses the `qrcode` crate (pure Rust, < 50KB) to generate SVG.
//! Output is embedded directly in the frontend — no image files.

use std::fmt::Write;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QrError {
    #[error("QR encoding failed: {0}")]
    EncodeFailed(String),
    #[error("URL too long for QR code (max 2048 chars)")]
    UrlTooLong,
}

/// Generate an inline SVG QR code for a URL.
///
/// `size` is the width/height of the SVG in pixels.
/// Returns an SVG string that can be embedded directly in HTML.
pub fn generate_qr_svg(url: &str, size: u32) -> Result<String, QrError> {
    if url.len() > 2048 {
        return Err(QrError::UrlTooLong);
    }

    let code =
        qrcode::QrCode::new(url.as_bytes()).map_err(|e| QrError::EncodeFailed(e.to_string()))?;

    let modules = code.width();
    let module_size = size as f64 / modules as f64;

    let mut svg = String::with_capacity(4096);
    let _ = write!(
        svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {size} {size}" width="{size}" height="{size}">"##
    );
    // White background
    let _ = write!(
        svg,
        r##"<rect width="{size}" height="{size}" fill="#ffffff"/>"##
    );

    // Draw dark modules
    for y in 0..modules {
        for x in 0..modules {
            use qrcode::Color;
            if code[(x, y)] == Color::Dark {
                let px = x as f64 * module_size;
                let py = y as f64 * module_size;
                let _ = write!(
                    svg,
                    r##"<rect x="{px:.1}" y="{py:.1}" width="{ms:.1}" height="{ms:.1}" fill="#000000"/>"##,
                    ms = module_size
                );
            }
        }
    }

    svg.push_str("</svg>");
    Ok(svg)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qr_generates_svg() {
        let svg = generate_qr_svg("https://example.com", 200).unwrap();
        assert!(svg.starts_with("<svg"), "should start with <svg");
        assert!(svg.contains("</svg>"), "should end with </svg>");
        assert!(svg.contains("<rect"), "should contain rect elements");
    }

    #[test]
    fn test_qr_encodes_url() {
        // Same URL should produce the same SVG (deterministic)
        let svg1 = generate_qr_svg("https://mysite.netlify.app", 150).unwrap();
        let svg2 = generate_qr_svg("https://mysite.netlify.app", 150).unwrap();
        assert_eq!(svg1, svg2);
    }

    #[test]
    fn test_qr_handles_long_url() {
        let long_url = format!("https://example.com/{}", "a".repeat(180));
        let svg = generate_qr_svg(&long_url, 200).unwrap();
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn test_qr_rejects_too_long_url() {
        let too_long = "x".repeat(3000);
        let result = generate_qr_svg(&too_long, 200);
        assert!(result.is_err());
    }
}
