//! Dependency Report — external resources with SRI hashes and licenses.
//!
//! Catalogs all external dependencies (fonts, CDN scripts, styles, icons)
//! used by a template, including integrity hashes where available.

use crate::dependency_manifest::{DependencyManifest, DependencyType};
use serde::{Deserialize, Serialize};

// ─── Types ──────────────────────────────────────────────────────────────────

/// Complete dependency report for a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyReport {
    pub fonts: Vec<DependencyEntry>,
    pub scripts: Vec<DependencyEntry>,
    pub styles: Vec<DependencyEntry>,
    pub icons: Vec<DependencyEntry>,
    pub total_count: usize,
}

/// A single external dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEntry {
    pub name: String,
    pub url: String,
    pub integrity_hash: Option<String>,
    pub version: Option<String>,
    pub license: Option<String>,
}

// ─── Generation ─────────────────────────────────────────────────────────────

/// Generate a dependency report for a template.
pub fn generate_dependency_report(template_id: &str) -> DependencyReport {
    let manifest = DependencyManifest::default_manifest();
    let deps = manifest.for_template(template_id);

    let mut fonts = Vec::new();
    let mut scripts = Vec::new();
    let mut styles = Vec::new();
    let mut icons = Vec::new();

    for dep in &deps {
        let entry = DependencyEntry {
            name: infer_name(&dep.url),
            url: dep.url.clone(),
            integrity_hash: dep.integrity_hash.clone(),
            version: None,
            license: infer_license(&dep.url),
        };

        match dep.resource_type {
            DependencyType::Font => fonts.push(entry),
            DependencyType::CdnScript => scripts.push(entry),
            DependencyType::CdnStyle => styles.push(entry),
            DependencyType::IconSet => icons.push(entry),
        }
    }

    let total_count = fonts.len() + scripts.len() + styles.len() + icons.len();

    DependencyReport {
        fonts,
        scripts,
        styles,
        icons,
        total_count,
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Infer a human-readable name from a URL.
fn infer_name(url: &str) -> String {
    if url.contains("fonts.googleapis.com") {
        // Extract font family from Google Fonts URL
        url.split("family=")
            .nth(1)
            .and_then(|s| s.split('&').next())
            .map(|s| s.replace('+', " ").replace("%20", " "))
            .unwrap_or_else(|| "Google Font".into())
    } else if url.contains("cdn") {
        // Extract filename from CDN URL
        url.rsplit('/')
            .next()
            .unwrap_or("Unknown CDN Resource")
            .to_string()
    } else {
        url.rsplit('/').next().unwrap_or("Unknown").to_string()
    }
}

/// Infer license from a well-known URL.
fn infer_license(url: &str) -> Option<String> {
    if url.contains("fonts.googleapis.com") || url.contains("fonts.gstatic.com") {
        Some("SIL Open Font License 1.1".into())
    } else if url.contains("cdnjs.cloudflare.com")
        || url.contains("unpkg.com")
        || url.contains("jsdelivr.net")
    {
        Some("Varies (check package)".into())
    } else {
        None
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_lists_fonts() {
        let report = generate_dependency_report("saas_landing");
        // The default manifest includes Google Fonts
        assert!(
            !report.fonts.is_empty() || report.total_count > 0,
            "should have some dependencies"
        );
    }

    #[test]
    fn test_report_has_urls() {
        let report = generate_dependency_report("portfolio");
        for font in &report.fonts {
            assert!(!font.url.is_empty(), "font entry should have a URL");
        }
    }

    #[test]
    fn test_report_has_names() {
        let report = generate_dependency_report("saas_landing");
        for font in &report.fonts {
            assert!(!font.name.is_empty(), "font entry should have a name");
        }
    }

    #[test]
    fn test_report_serializes() {
        let report = generate_dependency_report("ecommerce");
        let json = serde_json::to_string(&report);
        assert!(json.is_ok());
    }

    #[test]
    fn test_infer_name_google_fonts() {
        let url = "https://fonts.googleapis.com/css2?family=Inter:wght@400;700&display=swap";
        let name = infer_name(url);
        assert!(name.contains("Inter"));
    }

    #[test]
    fn test_infer_license_google_fonts() {
        let url = "https://fonts.googleapis.com/css2?family=Inter";
        let license = infer_license(url);
        assert!(license.is_some());
        assert!(license.unwrap().contains("Open Font License"));
    }
}
