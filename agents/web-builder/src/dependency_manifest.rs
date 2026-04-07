//! External Dependency Schema — tracks CDN resources loaded by templates.
//!
//! Typography presets reference Google Fonts, icon sets, etc. This manifest
//! feeds governance metadata and the Enterprise Trust Pack.

use serde::{Deserialize, Serialize};

// ─── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyType {
    Font,
    CdnScript,
    CdnStyle,
    IconSet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalDependency {
    pub resource_type: DependencyType,
    pub url: String,
    pub integrity_hash: Option<String>,
    pub required_by: Vec<String>,
}

/// Registry of all external dependencies used by templates.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DependencyManifest {
    pub dependencies: Vec<ExternalDependency>,
}

impl DependencyManifest {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a dependency, merging `required_by` if the URL already exists.
    pub fn add(
        &mut self,
        resource_type: DependencyType,
        url: &str,
        integrity_hash: Option<&str>,
        required_by: &[&str],
    ) {
        for dep in &mut self.dependencies {
            if dep.url == url {
                for &template_id in required_by {
                    let id = template_id.to_string();
                    if !dep.required_by.contains(&id) {
                        dep.required_by.push(id);
                    }
                }
                return;
            }
        }
        self.dependencies.push(ExternalDependency {
            resource_type,
            url: url.to_string(),
            integrity_hash: integrity_hash.map(|s| s.to_string()),
            required_by: required_by.iter().map(|s| s.to_string()).collect(),
        });
    }

    /// Get all dependencies needed for a specific template.
    pub fn for_template(&self, template_id: &str) -> Vec<&ExternalDependency> {
        self.dependencies
            .iter()
            .filter(|d| d.required_by.iter().any(|r| r == template_id))
            .collect()
    }

    /// Build the default manifest with Google Fonts used by typography presets.
    pub fn default_manifest() -> Self {
        let mut m = Self::new();

        // Tech preset: Inter + JetBrains Mono
        m.add(
            DependencyType::Font,
            "https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap",
            None,
            &["saas_landing", "docs_site", "portfolio", "local_business", "ecommerce", "dashboard"],
        );

        // Editorial preset: Playfair Display + Source Sans 3
        m.add(
            DependencyType::Font,
            "https://fonts.googleapis.com/css2?family=Playfair+Display:wght@400;700;900&family=Source+Sans+3:wght@400;500;600&display=swap",
            None,
            &["saas_landing", "docs_site", "portfolio", "local_business", "ecommerce", "dashboard"],
        );

        // Modern preset: Plus Jakarta Sans + DM Sans
        m.add(
            DependencyType::Font,
            "https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700&family=DM+Sans:wght@400;500;600&display=swap",
            None,
            &["saas_landing", "docs_site", "portfolio", "local_business", "ecommerce", "dashboard"],
        );

        m
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_merges_required_by() {
        let mut m = DependencyManifest::new();
        m.add(
            DependencyType::Font,
            "https://fonts.example.com/a",
            None,
            &["saas_landing"],
        );
        m.add(
            DependencyType::Font,
            "https://fonts.example.com/a",
            None,
            &["portfolio"],
        );
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].required_by.len(), 2);
    }

    #[test]
    fn test_for_template_filters() {
        let m = DependencyManifest::default_manifest();
        let deps = m.for_template("dashboard");
        assert!(!deps.is_empty());
        for dep in &deps {
            assert!(dep.required_by.contains(&"dashboard".to_string()));
        }
    }

    #[test]
    fn test_default_manifest_has_font_deps() {
        let m = DependencyManifest::default_manifest();
        assert!(
            m.dependencies.len() >= 3,
            "should have at least 3 font presets"
        );
        for dep in &m.dependencies {
            assert_eq!(dep.resource_type, DependencyType::Font);
            assert!(dep.url.starts_with("https://"));
        }
    }
}
