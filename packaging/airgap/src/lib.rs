//! Air-gap deployment infrastructure for Nexus OS.
//!
//! Produces self-contained, directory-based bundles that run Nexus OS in fully
//! disconnected environments — no internet, no cloud, no external dependencies.
//! The bundle format is a plain directory with `manifest.json` at its root.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

// ── Types ───────────────────────────────────────────────────────────────

/// A complete air-gap deployment bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirgapBundle {
    pub id: String,
    pub version: String,
    pub created_at: u64,
    pub components: Vec<BundleComponent>,
    pub total_size_bytes: u64,
    pub checksum: String,
    pub manifest: AirgapManifest,
}

/// A single component inside the bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleComponent {
    pub name: String,
    pub component_type: ComponentType,
    pub path: String,
    pub size_bytes: u64,
    pub checksum: String,
    pub required: bool,
}

/// Classification of bundle components.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentType {
    Binary,
    Model,
    Config,
    AgentManifest,
    Frontend,
    Documentation,
    Certificate,
}

/// Metadata about the target environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirgapManifest {
    pub target_os: String,
    pub target_arch: String,
    pub min_ram_gb: u32,
    pub min_disk_gb: u32,
    pub included_models: Vec<String>,
    pub included_agents: Vec<String>,
    pub offline_mode: bool,
    pub self_signed_tls: bool,
}

impl Default for AirgapManifest {
    fn default() -> Self {
        Self {
            target_os: current_os().to_string(),
            target_arch: current_arch().to_string(),
            min_ram_gb: 4,
            min_disk_gb: 10,
            included_models: Vec::new(),
            included_agents: Vec::new(),
            offline_mode: true,
            self_signed_tls: true,
        }
    }
}

/// Result of a validation check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub issues: Vec<String>,
    pub bundle: Option<AirgapBundle>,
}

/// System information for compatibility checking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub ram_gb: u32,
    pub disk_gb: u32,
    pub offline_capable: bool,
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn current_os() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    }
}

fn current_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    }
}

/// Compute SHA-256 of a byte slice.
fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Compute SHA-256 of a file. Returns error if file can't be read.
fn sha256_file(path: &Path) -> Result<String, String> {
    let data = fs::read(path).map_err(|e| format!("cannot read '{}': {e}", path.display()))?;
    Ok(sha256_bytes(&data))
}

/// Compute a bundle-level checksum from all component checksums.
fn compute_bundle_checksum(components: &[BundleComponent]) -> String {
    let mut hasher = Sha256::new();
    for comp in components {
        hasher.update(comp.checksum.as_bytes());
        hasher.update(comp.name.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

// ── AirgapBuilder ───────────────────────────────────────────────────────

/// Builder for constructing air-gap bundles.
pub struct AirgapBuilder {
    components: Vec<BundleComponent>,
    manifest: AirgapManifest,
}

impl AirgapBuilder {
    pub fn new(target_os: &str, target_arch: &str) -> Self {
        Self {
            components: Vec::new(),
            manifest: AirgapManifest {
                target_os: target_os.to_string(),
                target_arch: target_arch.to_string(),
                ..AirgapManifest::default()
            },
        }
    }

    /// Add a binary component from a file path.
    pub fn add_binary(&mut self, path: &str) -> Result<&mut Self, String> {
        let comp = scan_file(path, ComponentType::Binary, true)?;
        self.components.push(comp);
        Ok(self)
    }

    /// Add a GGUF model file.
    pub fn add_model(&mut self, path: &str) -> Result<&mut Self, String> {
        let comp = scan_file(path, ComponentType::Model, false)?;
        self.manifest.included_models.push(comp.name.clone());
        self.components.push(comp);
        Ok(self)
    }

    /// Scan a config directory and add all .toml / .json files.
    pub fn add_configs(&mut self, dir: &str) -> Result<&mut Self, String> {
        let dir_path = Path::new(dir);
        if !dir_path.is_dir() {
            return Err(format!("'{}' is not a directory", dir));
        }

        let entries =
            fs::read_dir(dir_path).map_err(|e| format!("cannot read dir '{}': {e}", dir))?;

        for entry in entries.flatten() {
            let p = entry.path();
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                let (ctype, is_agent) = match ext {
                    "toml" => (ComponentType::AgentManifest, true),
                    "json" => (ComponentType::Config, false),
                    _ => continue,
                };
                let path_str = p.display().to_string();
                let comp = scan_file(&path_str, ctype, false)?;
                if is_agent {
                    self.manifest.included_agents.push(comp.name.clone());
                }
                self.components.push(comp);
            }
        }
        Ok(self)
    }

    /// Add the built frontend dist directory.
    pub fn add_frontend(&mut self, dist_dir: &str) -> Result<&mut Self, String> {
        let dir_path = Path::new(dist_dir);
        if !dir_path.is_dir() {
            return Err(format!("'{}' is not a directory", dist_dir));
        }

        // Compute a checksum of the directory listing + file sizes
        let mut hasher = Sha256::new();
        let mut total_size = 0u64;

        visit_dir_recursive(dir_path, &mut |file_path| {
            if let Ok(meta) = fs::metadata(file_path) {
                let size = meta.len();
                total_size += size;
                hasher.update(file_path.display().to_string().as_bytes());
                hasher.update(size.to_le_bytes());
            }
        });

        let checksum = format!("{:x}", hasher.finalize());

        self.components.push(BundleComponent {
            name: "frontend-dist".into(),
            component_type: ComponentType::Frontend,
            path: dist_dir.to_string(),
            size_bytes: total_size,
            checksum,
            required: true,
        });
        Ok(self)
    }

    /// Add documentation directory.
    pub fn add_docs(&mut self, docs_dir: &str) -> Result<&mut Self, String> {
        let dir_path = Path::new(docs_dir);
        if !dir_path.is_dir() {
            return Err(format!("'{}' is not a directory", docs_dir));
        }

        let mut hasher = Sha256::new();
        let mut total_size = 0u64;

        visit_dir_recursive(dir_path, &mut |file_path| {
            if let Ok(meta) = fs::metadata(file_path) {
                let size = meta.len();
                total_size += size;
                hasher.update(file_path.display().to_string().as_bytes());
                hasher.update(size.to_le_bytes());
            }
        });

        let checksum = format!("{:x}", hasher.finalize());

        self.components.push(BundleComponent {
            name: "documentation".into(),
            component_type: ComponentType::Documentation,
            path: docs_dir.to_string(),
            size_bytes: total_size,
            checksum,
            required: false,
        });
        Ok(self)
    }

    /// Add a component directly (for testing or custom components).
    pub fn add_component(&mut self, component: BundleComponent) -> &mut Self {
        self.components.push(component);
        self
    }

    /// Set minimum RAM requirement.
    pub fn set_min_ram_gb(&mut self, gb: u32) -> &mut Self {
        self.manifest.min_ram_gb = gb;
        self
    }

    /// Set minimum disk requirement.
    pub fn set_min_disk_gb(&mut self, gb: u32) -> &mut Self {
        self.manifest.min_disk_gb = gb;
        self
    }

    /// Build the bundle. Writes manifest.json to output_path.
    pub fn build(&self, output_path: &str) -> Result<AirgapBundle, String> {
        if self.components.is_empty() {
            return Err("cannot build empty bundle — add at least one component".into());
        }

        let total_size: u64 = self.components.iter().map(|c| c.size_bytes).sum();
        let checksum = compute_bundle_checksum(&self.components);

        let bundle = AirgapBundle {
            id: uuid::Uuid::new_v4().to_string(),
            version: "7.0.0".into(),
            created_at: now_secs(),
            components: self.components.clone(),
            total_size_bytes: total_size,
            checksum,
            manifest: self.manifest.clone(),
        };

        // Write manifest.json to output directory
        let out_dir = Path::new(output_path);
        if !out_dir.exists() {
            fs::create_dir_all(out_dir).map_err(|e| format!("cannot create output dir: {e}"))?;
        }

        let manifest_path = out_dir.join("manifest.json");
        let json = serde_json::to_string_pretty(&bundle).map_err(|e| format!("serialize: {e}"))?;
        fs::write(&manifest_path, json).map_err(|e| format!("cannot write manifest: {e}"))?;

        Ok(bundle)
    }

    /// Return the current component count.
    pub fn component_count(&self) -> usize {
        self.components.len()
    }
}

/// Scan a single file and create a BundleComponent.
fn scan_file(
    path: &str,
    component_type: ComponentType,
    required: bool,
) -> Result<BundleComponent, String> {
    let file_path = Path::new(path);
    if !file_path.exists() {
        return Err(format!("file not found: {path}"));
    }

    let meta = fs::metadata(file_path).map_err(|e| format!("cannot stat '{}': {e}", path))?;
    let size_bytes = meta.len();
    let checksum = sha256_file(file_path)?;

    let name = file_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());

    Ok(BundleComponent {
        name,
        component_type,
        path: path.to_string(),
        size_bytes,
        checksum,
        required,
    })
}

/// Recursively visit all files in a directory.
fn visit_dir_recursive(dir: &Path, visitor: &mut dyn FnMut(&Path)) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                visit_dir_recursive(&p, visitor);
            } else {
                visitor(&p);
            }
        }
    }
}

// ── AirgapInstaller ─────────────────────────────────────────────────────

/// Validates and installs air-gap bundles.
pub struct AirgapInstaller;

impl AirgapInstaller {
    /// Validate a bundle at the given path by reading its manifest.json
    /// and checking component integrity.
    pub fn validate_bundle(bundle_path: &str) -> ValidationResult {
        let manifest_path = Path::new(bundle_path).join("manifest.json");
        let mut issues = Vec::new();

        // Read manifest
        let bundle: AirgapBundle = match fs::read_to_string(&manifest_path) {
            Ok(json) => match serde_json::from_str(&json) {
                Ok(b) => b,
                Err(e) => {
                    issues.push(format!("invalid manifest.json: {e}"));
                    return ValidationResult {
                        valid: false,
                        issues,
                        bundle: None,
                    };
                }
            },
            Err(e) => {
                issues.push(format!("cannot read manifest.json: {e}"));
                return ValidationResult {
                    valid: false,
                    issues,
                    bundle: None,
                };
            }
        };

        // Verify bundle-level checksum
        let recomputed = compute_bundle_checksum(&bundle.components);
        if recomputed != bundle.checksum {
            issues.push(format!(
                "bundle checksum mismatch: expected '{}…', got '{}…'",
                &bundle.checksum[..bundle.checksum.len().min(16)],
                &recomputed[..recomputed.len().min(16)]
            ));
        }

        // Check required components have valid paths
        for comp in &bundle.components {
            if comp.required {
                let p = Path::new(&comp.path);
                if !p.exists() {
                    issues.push(format!(
                        "required component '{}' missing at '{}'",
                        comp.name, comp.path
                    ));
                }
            }
        }

        // Check system compatibility
        if bundle.manifest.target_os != current_os() {
            issues.push(format!(
                "target OS '{}' does not match current '{}'",
                bundle.manifest.target_os,
                current_os()
            ));
        }
        if bundle.manifest.target_arch != current_arch() {
            issues.push(format!(
                "target arch '{}' does not match current '{}'",
                bundle.manifest.target_arch,
                current_arch()
            ));
        }

        ValidationResult {
            valid: issues.is_empty(),
            issues,
            bundle: Some(bundle),
        }
    }

    /// Install a bundle into the given directory.
    /// Copies manifest.json and verifies integrity.
    pub fn install(bundle_path: &str, install_dir: &str) -> Result<AirgapBundle, String> {
        let result = Self::validate_bundle(bundle_path);
        if !result.valid {
            return Err(format!("validation failed: {}", result.issues.join("; ")));
        }

        let bundle = result.bundle.ok_or("Bundle missing after successful validation")?;

        // Create install directory
        let install = Path::new(install_dir);
        fs::create_dir_all(install).map_err(|e| format!("cannot create install dir: {e}"))?;

        // Copy manifest
        let src_manifest = Path::new(bundle_path).join("manifest.json");
        let dst_manifest = install.join("manifest.json");
        fs::copy(&src_manifest, &dst_manifest).map_err(|e| format!("cannot copy manifest: {e}"))?;

        // Write an install marker
        let marker = install.join(".nexus-installed");
        fs::write(
            &marker,
            format!("installed_at={}\nbundle_id={}\n", now_secs(), bundle.id),
        )
        .map_err(|e| format!("cannot write install marker: {e}"))?;

        Ok(bundle)
    }

    /// Verify an existing installation.
    pub fn verify_installation(install_dir: &str) -> ValidationResult {
        let marker_path = Path::new(install_dir).join(".nexus-installed");
        let manifest_path = Path::new(install_dir).join("manifest.json");
        let mut issues = Vec::new();

        if !marker_path.exists() {
            issues.push("no .nexus-installed marker found — not a valid installation".into());
        }

        if !manifest_path.exists() {
            issues.push("manifest.json missing from installation".into());
            return ValidationResult {
                valid: false,
                issues,
                bundle: None,
            };
        }

        let bundle: Option<AirgapBundle> = fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok());

        if bundle.is_none() {
            issues.push("corrupt manifest.json in installation".into());
        }

        ValidationResult {
            valid: issues.is_empty(),
            issues,
            bundle,
        }
    }
}

/// Return current system information.
pub fn get_system_info() -> SystemInfo {
    SystemInfo {
        os: current_os().to_string(),
        arch: current_arch().to_string(),
        ram_gb: 0, // would require sys-info crate; stub for now
        disk_gb: 0,
        offline_capable: true,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nexus-airgap-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn create_temp_file(dir: &Path, name: &str, content: &[u8]) -> String {
        let path = dir.join(name);
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(content).unwrap();
        path.display().to_string()
    }

    #[test]
    fn test_bundle_component_creation() {
        let dir = tmp_dir();
        let path = create_temp_file(&dir, "nexus-os", b"fake binary content");

        let comp = scan_file(&path, ComponentType::Binary, true).unwrap();
        assert_eq!(comp.name, "nexus-os");
        assert_eq!(comp.component_type, ComponentType::Binary);
        assert!(comp.required);
        assert_eq!(comp.size_bytes, 19); // "fake binary content".len()
        assert!(!comp.checksum.is_empty());
        assert_eq!(comp.checksum.len(), 64); // SHA-256 hex

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_manifest_defaults() {
        let manifest = AirgapManifest::default();
        assert!(manifest.offline_mode);
        assert!(manifest.self_signed_tls);
        assert_eq!(manifest.min_ram_gb, 4);
        assert_eq!(manifest.min_disk_gb, 10);
        assert!(manifest.included_models.is_empty());
        assert!(manifest.included_agents.is_empty());
        // OS/arch should match current platform
        assert_eq!(manifest.target_os, current_os());
        assert_eq!(manifest.target_arch, current_arch());
    }

    #[test]
    fn test_builder_add_components() {
        let dir = tmp_dir();
        let bin_path = create_temp_file(&dir, "nexus-os", b"binary");
        let model_path = create_temp_file(&dir, "model.gguf", b"model data");

        let mut builder = AirgapBuilder::new("linux", "x86_64");
        builder.add_binary(&bin_path).unwrap();
        builder.add_model(&model_path).unwrap();

        assert_eq!(builder.component_count(), 2);
        assert_eq!(builder.manifest.included_models, vec!["model.gguf"]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_checksum_computation() {
        let c1 = BundleComponent {
            name: "a".into(),
            component_type: ComponentType::Binary,
            path: "/tmp/a".into(),
            size_bytes: 100,
            checksum: "abc123".into(),
            required: true,
        };
        let c2 = BundleComponent {
            name: "b".into(),
            component_type: ComponentType::Model,
            path: "/tmp/b".into(),
            size_bytes: 200,
            checksum: "def456".into(),
            required: false,
        };

        let cs1 = compute_bundle_checksum(&[c1.clone(), c2.clone()]);
        let cs2 = compute_bundle_checksum(&[c1.clone(), c2.clone()]);
        assert_eq!(cs1, cs2); // deterministic

        // Different order → different checksum
        let cs3 = compute_bundle_checksum(&[c2, c1]);
        assert_ne!(cs1, cs3);
    }

    #[test]
    fn test_validate_intact_bundle() {
        let dir = tmp_dir();
        let bin_path = create_temp_file(&dir, "nexus-os", b"binary");

        let mut builder = AirgapBuilder::new(current_os(), current_arch());
        builder.add_binary(&bin_path).unwrap();

        let output_dir = dir.join("bundle");
        let bundle = builder.build(output_dir.to_str().unwrap()).unwrap();

        assert!(!bundle.id.is_empty());
        assert_eq!(bundle.components.len(), 1);
        assert!(bundle.total_size_bytes > 0);

        let result = AirgapInstaller::validate_bundle(output_dir.to_str().unwrap());
        assert!(result.valid, "issues: {:?}", result.issues);
        assert!(result.bundle.is_some());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_validate_corrupted_component() {
        let dir = tmp_dir();
        let bin_path = create_temp_file(&dir, "nexus-os", b"binary");

        let mut builder = AirgapBuilder::new(current_os(), current_arch());
        builder.add_binary(&bin_path).unwrap();

        let output_dir = dir.join("bundle");
        builder.build(output_dir.to_str().unwrap()).unwrap();

        // Corrupt the manifest by changing a checksum
        let manifest_path = output_dir.join("manifest.json");
        let json = fs::read_to_string(&manifest_path).unwrap();
        let mut bundle: AirgapBundle = serde_json::from_str(&json).unwrap();
        bundle.components[0].checksum = "corrupted".into();
        // Rewrite with corrupted checksum (bundle-level checksum will mismatch)
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&bundle).unwrap(),
        )
        .unwrap();

        let result = AirgapInstaller::validate_bundle(output_dir.to_str().unwrap());
        assert!(!result.valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("checksum mismatch")));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_component_types_serde() {
        let types = vec![
            ComponentType::Binary,
            ComponentType::Model,
            ComponentType::Config,
            ComponentType::AgentManifest,
            ComponentType::Frontend,
            ComponentType::Documentation,
            ComponentType::Certificate,
        ];
        for ct in &types {
            let json = serde_json::to_string(ct).unwrap();
            let back: ComponentType = serde_json::from_str(&json).unwrap();
            assert_eq!(*ct, back);
        }
    }

    #[test]
    fn test_required_components_check() {
        let dir = tmp_dir();
        let output_dir = dir.join("bundle");
        fs::create_dir_all(&output_dir).unwrap();

        // Create a bundle with a required component pointing to a nonexistent file
        let bundle = AirgapBundle {
            id: "test".into(),
            version: "7.0.0".into(),
            created_at: now_secs(),
            components: vec![BundleComponent {
                name: "nexus-os".into(),
                component_type: ComponentType::Binary,
                path: "/nonexistent/nexus-os".into(),
                size_bytes: 100,
                checksum: "abc".into(),
                required: true,
            }],
            total_size_bytes: 100,
            checksum: compute_bundle_checksum(&[BundleComponent {
                name: "nexus-os".into(),
                component_type: ComponentType::Binary,
                path: "/nonexistent/nexus-os".into(),
                size_bytes: 100,
                checksum: "abc".into(),
                required: true,
            }]),
            manifest: AirgapManifest::default(),
        };

        let json = serde_json::to_string_pretty(&bundle).unwrap();
        fs::write(output_dir.join("manifest.json"), json).unwrap();

        let result = AirgapInstaller::validate_bundle(output_dir.to_str().unwrap());
        assert!(!result.valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("required component") && i.contains("missing")));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_system_compatibility_check() {
        let dir = tmp_dir();
        let output_dir = dir.join("bundle");
        fs::create_dir_all(&output_dir).unwrap();

        // Create a bundle targeting a different OS
        let bundle = AirgapBundle {
            id: "test".into(),
            version: "7.0.0".into(),
            created_at: now_secs(),
            components: vec![],
            total_size_bytes: 0,
            checksum: compute_bundle_checksum(&[]),
            manifest: AirgapManifest {
                target_os: "haiku-os".into(), // definitely not current
                target_arch: "mips64".into(),
                ..AirgapManifest::default()
            },
        };

        let json = serde_json::to_string_pretty(&bundle).unwrap();
        fs::write(output_dir.join("manifest.json"), json).unwrap();

        let result = AirgapInstaller::validate_bundle(output_dir.to_str().unwrap());
        assert!(!result.valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("target OS") && i.contains("haiku-os")));
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("target arch") && i.contains("mips64")));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_builder_empty_is_error() {
        let builder = AirgapBuilder::new("linux", "x86_64");
        let dir = tmp_dir();
        let result = builder.build(dir.join("empty").to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty bundle"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_install_and_verify() {
        let dir = tmp_dir();
        let bin_path = create_temp_file(&dir, "nexus-os", b"binary data here");

        let mut builder = AirgapBuilder::new(current_os(), current_arch());
        builder.add_binary(&bin_path).unwrap();

        let bundle_dir = dir.join("bundle");
        let bundle = builder.build(bundle_dir.to_str().unwrap()).unwrap();

        let install_dir = dir.join("install");
        let installed =
            AirgapInstaller::install(bundle_dir.to_str().unwrap(), install_dir.to_str().unwrap())
                .unwrap();
        assert_eq!(installed.id, bundle.id);

        let verify = AirgapInstaller::verify_installation(install_dir.to_str().unwrap());
        assert!(verify.valid, "issues: {:?}", verify.issues);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_system_info() {
        let info = get_system_info();
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
        assert!(info.offline_capable);
    }

    #[test]
    fn test_airgap_bundle_serde_roundtrip() {
        let bundle = AirgapBundle {
            id: "test-id".into(),
            version: "7.0.0".into(),
            created_at: 1000,
            components: vec![BundleComponent {
                name: "test".into(),
                component_type: ComponentType::Binary,
                path: "/tmp/test".into(),
                size_bytes: 42,
                checksum: "abc123".into(),
                required: true,
            }],
            total_size_bytes: 42,
            checksum: "bundle-hash".into(),
            manifest: AirgapManifest::default(),
        };

        let json = serde_json::to_string(&bundle).unwrap();
        let back: AirgapBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "test-id");
        assert_eq!(back.components.len(), 1);
        assert_eq!(back.components[0].component_type, ComponentType::Binary);
    }

    #[test]
    fn test_add_configs_directory() {
        let dir = tmp_dir();
        let config_dir = dir.join("configs");
        fs::create_dir_all(&config_dir).unwrap();

        create_temp_file(&config_dir, "agent1.toml", b"[agent]\nname = \"a1\"");
        create_temp_file(&config_dir, "settings.json", b"{\"key\": \"value\"}");
        create_temp_file(&config_dir, "readme.md", b"ignore me"); // should be skipped

        let mut builder = AirgapBuilder::new("linux", "x86_64");
        builder.add_configs(config_dir.to_str().unwrap()).unwrap();

        assert_eq!(builder.component_count(), 2); // .toml + .json, not .md
        assert_eq!(builder.manifest.included_agents, vec!["agent1.toml"]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_sha256_deterministic() {
        let h1 = sha256_bytes(b"hello world");
        let h2 = sha256_bytes(b"hello world");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);

        let h3 = sha256_bytes(b"different");
        assert_ne!(h1, h3);
    }
}
