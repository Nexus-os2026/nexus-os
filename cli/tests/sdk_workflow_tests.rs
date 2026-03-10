//! SDK developer workflow tests covering:
//! - `nexus create` generates compilable agent projects
//! - All 6 templates produce valid manifests
//! - `nexus test` runs template agents successfully
//! - `nexus package` produces signed bundles
//! - Full create → test → package → publish → install roundtrip

use ed25519_dalek::SigningKey;
use nexus_cli::packager::{build_signed_bundle, package_agent};
use nexus_cli::scaffold::scaffold_agent_project;
use nexus_cli::templates::template_names;
use nexus_cli::test_runner::run_agent_test_from_str;
use nexus_marketplace::package::verify_package;
use nexus_marketplace::sqlite_registry::SqliteRegistry;
use nexus_marketplace::verification_pipeline::{verify_bundle, Verdict};

fn test_key() -> SigningKey {
    SigningKey::from_bytes(&[42u8; 32])
}

// ── Test 1: nexus create generates compilable agent project ──────────────────

#[test]
fn create_generates_compilable_agent_project() {
    let tmp = std::env::temp_dir().join("nexus-sdk-test-create");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let result = scaffold_agent_project("my-test-agent", "basic", &tmp).unwrap();

    // All 4 expected files created
    assert_eq!(result.files_created.len(), 4);
    assert!(result.project_dir.join("Cargo.toml").exists());
    assert!(result.project_dir.join("manifest.toml").exists());
    assert!(result.project_dir.join("src/lib.rs").exists());
    assert!(result.project_dir.join("README.md").exists());

    // Cargo.toml has correct package name and sdk dependency
    let cargo_content = std::fs::read_to_string(result.project_dir.join("Cargo.toml")).unwrap();
    assert!(cargo_content.contains("name = \"my-test-agent\""));
    assert!(cargo_content.contains("nexus-sdk"));
    assert!(cargo_content.contains("edition = \"2021\""));

    // manifest.toml has valid TOML with required fields
    let manifest_content =
        std::fs::read_to_string(result.project_dir.join("manifest.toml")).unwrap();
    assert!(manifest_content.contains("name = \"my-test-agent\""));
    assert!(manifest_content.contains("version = \"0.1.0\""));
    assert!(manifest_content.contains("fuel_budget"));
    assert!(manifest_content.contains("capabilities"));

    // src/lib.rs has actual agent code (not empty)
    let lib_content = std::fs::read_to_string(result.project_dir.join("src/lib.rs")).unwrap();
    assert!(lib_content.len() > 50);

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── Test 2: All 6 templates produce valid manifests ──────────────────────────

#[test]
fn all_templates_produce_valid_manifests() {
    let tmp = std::env::temp_dir().join("nexus-sdk-test-templates");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let templates = template_names();
    assert_eq!(
        templates.len(),
        6,
        "Expected 6 templates, got {}",
        templates.len()
    );

    for tmpl_name in &templates {
        let agent_name = format!("tmpl-{tmpl_name}");
        let result = scaffold_agent_project(&agent_name, tmpl_name, &tmp)
            .unwrap_or_else(|e| panic!("Failed to scaffold template '{tmpl_name}': {e}"));

        let manifest_content =
            std::fs::read_to_string(result.project_dir.join("manifest.toml")).unwrap();

        // Parse with kernel parser — this is what the runtime uses
        let parsed = nexus_kernel::manifest::parse_manifest(&manifest_content);
        assert!(
            parsed.is_ok(),
            "Template '{tmpl_name}' produced unparseable manifest: {:?}",
            parsed.err()
        );

        let manifest = parsed.unwrap();
        assert_eq!(manifest.name, agent_name);
        assert_eq!(manifest.version, "0.1.0");
        assert!(
            !manifest.capabilities.is_empty(),
            "Template '{tmpl_name}' has no capabilities"
        );
        assert!(
            manifest.fuel_budget > 0,
            "Template '{tmpl_name}' has zero fuel budget"
        );
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── Test 3: nexus test runs template agent successfully ──────────────────────

#[test]
fn test_runs_template_agent_successfully() {
    let tmp = std::env::temp_dir().join("nexus-sdk-test-runner");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    // Test with basic template
    let result = scaffold_agent_project("test-basic", "basic", &tmp).unwrap();
    let manifest_content =
        std::fs::read_to_string(result.project_dir.join("manifest.toml")).unwrap();

    let report = run_agent_test_from_str(&manifest_content).unwrap();
    assert!(
        report.passed,
        "Basic template test failed: {:?}",
        report.error
    );
    assert_eq!(report.agent_name, "test-basic");
    assert_eq!(report.phase_results.len(), 3); // init, execute, shutdown
    assert!(report.phase_results.iter().all(|p| p.passed));
    assert!(report.fuel_consumed > 0);

    // Test with data-analyst template (has fs.read + llm.query)
    let result = scaffold_agent_project("test-analyst", "data-analyst", &tmp).unwrap();
    let manifest_content =
        std::fs::read_to_string(result.project_dir.join("manifest.toml")).unwrap();

    let report = run_agent_test_from_str(&manifest_content).unwrap();
    assert!(
        report.passed,
        "Data-analyst template test failed: {:?}",
        report.error
    );
    assert!(report.outputs_count >= 2); // fs.read + llm.query

    // Test with file-organizer template (has fs.read + fs.write)
    let result = scaffold_agent_project("test-organizer", "file-organizer", &tmp).unwrap();
    let manifest_content =
        std::fs::read_to_string(result.project_dir.join("manifest.toml")).unwrap();

    let report = run_agent_test_from_str(&manifest_content).unwrap();
    assert!(
        report.passed,
        "File-organizer template test failed: {:?}",
        report.error
    );
    assert!(report.outputs_count >= 2); // fs.read + fs.write

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── Test 4: nexus package produces signed bundle ─────────────────────────────

#[test]
fn package_produces_signed_bundle() {
    let tmp = std::env::temp_dir().join("nexus-sdk-test-package");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let scaffold = scaffold_agent_project("pkg-agent", "basic", &tmp).unwrap();
    let key = test_key();

    let result = package_agent(&scaffold.project_dir, &key).unwrap();
    assert_eq!(result.agent_name, "pkg-agent");
    assert_eq!(result.version, "0.1.0");
    assert!(result.package_id.starts_with("pkg-"));
    assert!(result.output_path.ends_with(".nexus-agent"));
    assert!(result.bundle_size > 0);

    // Read the bundle back and verify it
    let bundle_bytes = std::fs::read(&result.output_path).unwrap();
    let bundle: nexus_marketplace::package::SignedPackageBundle =
        serde_json::from_slice(&bundle_bytes).unwrap();
    assert!(verify_package(&bundle).is_ok());
    assert_eq!(bundle.metadata.name, "pkg-agent");
    assert_eq!(bundle.metadata.version, "0.1.0");

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── Test 11: Full developer workflow roundtrip ───────────────────────────────

#[test]
fn full_create_test_package_publish_install_roundtrip() {
    let tmp = std::env::temp_dir().join("nexus-sdk-full-roundtrip");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let key = test_key();
    let registry = SqliteRegistry::open_in_memory().unwrap();

    // 1. CREATE — scaffold a new agent project
    let scaffold = scaffold_agent_project("roundtrip-dev", "basic", &tmp).unwrap();
    assert!(scaffold.project_dir.join("manifest.toml").exists());

    // 2. TEST — run agent test on the scaffolded manifest
    let manifest_content =
        std::fs::read_to_string(scaffold.project_dir.join("manifest.toml")).unwrap();
    let report = run_agent_test_from_str(&manifest_content).unwrap();
    assert!(report.passed, "Agent test failed: {:?}", report.error);

    // 3. PACKAGE — produce signed bundle
    let pkg_result = package_agent(&scaffold.project_dir, &key).unwrap();
    assert!(pkg_result.bundle_size > 0);

    // 4. Read bundle for publish
    let bundle_bytes = std::fs::read(&pkg_result.output_path).unwrap();
    let bundle: nexus_marketplace::package::SignedPackageBundle =
        serde_json::from_slice(&bundle_bytes).unwrap();

    // 5. VERIFY — run verification pipeline
    let verification = verify_bundle(&bundle);
    assert!(
        verification.may_list(),
        "Verification rejected: {:?}",
        verification.verdict
    );
    assert_ne!(verification.verdict, Verdict::Rejected);

    // 6. PUBLISH — insert into registry
    registry.upsert_signed(&bundle).unwrap();

    // 7. SEARCH — find in marketplace
    let results = registry.search("roundtrip-dev").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "roundtrip-dev");

    // 8. INFO — get full details
    let detail = registry.get_agent(&bundle.package_id).unwrap();
    assert_eq!(detail.name, "roundtrip-dev");
    assert_eq!(detail.version, "0.1.0");

    // 9. INSTALL — download and verify
    let installed = registry.install(&bundle.package_id).unwrap();
    assert!(verify_package(&installed).is_ok());
    assert_eq!(installed.metadata.name, "roundtrip-dev");

    // 10. Verify download count
    let count = registry.download_count(&bundle.package_id).unwrap();
    assert_eq!(count, 1);

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── Additional: build_signed_bundle API works standalone ─────────────────────

#[test]
fn build_signed_bundle_api_produces_verifiable_bundle() {
    let key = test_key();
    let manifest = "name = \"api-test\"\nversion = \"0.1.0\"\ncapabilities = [\"llm.query\"]\nfuel_budget = 10000\n";

    let bundle = build_signed_bundle(
        manifest,
        "pub fn run() {}",
        "api-test",
        "0.1.0",
        "Test agent",
        &["llm.query".to_string()],
        &key,
    )
    .unwrap();

    assert!(verify_package(&bundle).is_ok());
    assert_eq!(bundle.metadata.name, "api-test");
    assert!(bundle.package_id.starts_with("pkg-"));
}

// ── Additional: test all 6 templates can be packaged ─────────────────────────

#[test]
fn all_templates_can_be_packaged() {
    let tmp = std::env::temp_dir().join("nexus-sdk-test-all-pkg");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let key = test_key();

    for tmpl_name in template_names() {
        let agent_name = format!("pkg-{tmpl_name}");
        let scaffold = scaffold_agent_project(&agent_name, tmpl_name, &tmp)
            .unwrap_or_else(|e| panic!("Scaffold '{tmpl_name}' failed: {e}"));

        let result = package_agent(&scaffold.project_dir, &key)
            .unwrap_or_else(|e| panic!("Package '{tmpl_name}' failed: {e}"));

        assert!(
            result.bundle_size > 0,
            "Template '{tmpl_name}' produced empty bundle"
        );
        assert!(
            result.output_path.ends_with(".nexus-agent"),
            "Template '{tmpl_name}' output not .nexus-agent"
        );

        // Verify the bundle
        let bundle_bytes = std::fs::read(&result.output_path).unwrap();
        let bundle: nexus_marketplace::package::SignedPackageBundle =
            serde_json::from_slice(&bundle_bytes).unwrap();
        assert!(
            verify_package(&bundle).is_ok(),
            "Template '{tmpl_name}' bundle failed verification"
        );
    }

    let _ = std::fs::remove_dir_all(&tmp);
}
