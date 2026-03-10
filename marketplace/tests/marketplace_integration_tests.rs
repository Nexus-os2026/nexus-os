//! Marketplace integration tests covering the full publish → search → install pipeline.
//!
//! Tests: unsigned bundle rejection, crashing agent rejection, version update preserves
//! downloads, search by name and tags, and the full developer workflow roundtrip.

use ed25519_dalek::SigningKey;
use nexus_marketplace::package::{
    create_unsigned_bundle, sign_package, verify_package, PackageMetadata,
};
use nexus_marketplace::sqlite_registry::SqliteRegistry;
use nexus_marketplace::verification_pipeline::{verified_publish_sqlite, verify_bundle, Verdict};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn test_key() -> SigningKey {
    SigningKey::from_bytes(&[42u8; 32])
}

fn other_key() -> SigningKey {
    SigningKey::from_bytes(&[99u8; 32])
}

fn valid_manifest(name: &str, version: &str, capabilities: &[&str]) -> String {
    let caps: Vec<String> = capabilities.iter().map(|c| format!("\"{c}\"")).collect();
    format!(
        "name = \"{name}\"\nversion = \"{version}\"\ncapabilities = [{}]\nfuel_budget = 10000\n",
        caps.join(", ")
    )
}

fn agent_code() -> &'static str {
    "pub fn run() { /* agent logic */ }"
}

fn make_metadata(name: &str, version: &str, caps: &[&str], tags: &[&str]) -> PackageMetadata {
    PackageMetadata {
        name: name.to_string(),
        version: version.to_string(),
        description: format!("A {name} agent for testing"),
        capabilities: caps.iter().map(|s| s.to_string()).collect(),
        tags: tags.iter().map(|s| s.to_string()).collect(),
        author_id: "test-author".to_string(),
    }
}

fn publish_test_agent(
    registry: &SqliteRegistry,
    name: &str,
    version: &str,
    caps: &[&str],
    tags: &[&str],
    key: &SigningKey,
) -> String {
    let manifest = valid_manifest(name, version, caps);
    let metadata = make_metadata(name, version, caps, tags);
    let unsigned = create_unsigned_bundle(
        &manifest,
        agent_code(),
        metadata,
        &format!("local://{name}"),
        "test",
    )
    .expect("create bundle");
    let signed = sign_package(unsigned, key).expect("sign");
    registry.upsert_signed(&signed).expect("upsert");
    signed.package_id
}

// ── Test 5: Publish passes verification and appears in search ────────────────

#[test]
fn publish_passes_verification_and_appears_in_search() {
    let registry = SqliteRegistry::open_in_memory().unwrap();
    let key = test_key();

    let manifest = valid_manifest("search-bot", "1.0.0", &["llm.query"]);
    let metadata = make_metadata("search-bot", "1.0.0", &["llm.query"], &["automation"]);
    let unsigned = create_unsigned_bundle(
        &manifest,
        agent_code(),
        metadata,
        "local://search-bot",
        "test",
    )
    .unwrap();

    let (pkg_id, result) = verified_publish_sqlite(&registry, unsigned, &key).unwrap();
    assert!(result.may_list());
    assert!(pkg_id.starts_with("pkg-"));

    // Agent should appear in search
    let results = registry.search("search-bot").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "search-bot");
}

// ── Test 6: Install downloads and verifies signature ─────────────────────────

#[test]
fn install_downloads_and_verifies_signature() {
    let registry = SqliteRegistry::open_in_memory().unwrap();
    let key = test_key();

    let pkg_id = publish_test_agent(
        &registry,
        "installer-test",
        "1.0.0",
        &["llm.query"],
        &[],
        &key,
    );

    // Install should return a verified bundle and increment downloads
    let bundle = registry.install(&pkg_id).unwrap();
    assert!(verify_package(&bundle).is_ok());
    assert_eq!(bundle.metadata.name, "installer-test");

    let count = registry.download_count(&pkg_id).unwrap();
    assert_eq!(count, 1);

    // Install again increments
    let _ = registry.install(&pkg_id).unwrap();
    let count = registry.download_count(&pkg_id).unwrap();
    assert_eq!(count, 2);
}

// ── Test 7: Unsigned bundle rejected by verification ─────────────────────────

#[test]
fn unsigned_bundle_rejected_by_verification() {
    let manifest = valid_manifest("unsigned-agent", "1.0.0", &["llm.query"]);
    let metadata = make_metadata("unsigned-agent", "1.0.0", &["llm.query"], &[]);
    let unsigned = create_unsigned_bundle(
        &manifest,
        agent_code(),
        metadata,
        "local://unsigned",
        "test",
    )
    .unwrap();

    // Manually create a "signed" bundle with an invalid signature
    let key = test_key();
    let mut signed = sign_package(unsigned, &key).unwrap();
    // Corrupt the signature
    signed.signature = vec![0u8; 64];

    let result = verify_bundle(&signed);
    assert_eq!(result.verdict, Verdict::Rejected);
    assert!(!result.may_list());

    // Signature check should be the one that failed
    let sig_check = result.checks.iter().find(|c| c.name == "signature_check");
    assert!(sig_check.is_some());
    assert!(!sig_check.unwrap().passed);
}

// ── Test 8: Crashing agent rejected by sandbox test ──────────────────────────

#[test]
fn crashing_agent_rejected_by_sandbox_test() {
    let manifest = valid_manifest("crash-agent", "1.0.0", &["llm.query"]);
    let metadata = make_metadata("crash-agent", "1.0.0", &["llm.query"], &[]);

    // Empty code should fail the sandbox test
    let unsigned =
        create_unsigned_bundle(&manifest, "", metadata, "local://crash-agent", "test").unwrap();
    let key = test_key();
    let signed = sign_package(unsigned, &key).unwrap();

    let result = verify_bundle(&signed);
    assert_eq!(result.verdict, Verdict::Rejected);

    let sandbox_check = result.checks.iter().find(|c| c.name == "sandbox_test");
    assert!(sandbox_check.is_some());
    assert!(!sandbox_check.unwrap().passed);
}

// ── Test 9: Version update preserves download count ──────────────────────────

#[test]
fn version_update_preserves_download_count() {
    let registry = SqliteRegistry::open_in_memory().unwrap();
    let key = test_key();

    let pkg_id = publish_test_agent(
        &registry,
        "versioned-agent",
        "1.0.0",
        &["llm.query"],
        &["data"],
        &key,
    );

    // Install a few times to build up download count
    let _ = registry.install(&pkg_id).unwrap();
    let _ = registry.install(&pkg_id).unwrap();
    let _ = registry.install(&pkg_id).unwrap();
    assert_eq!(registry.download_count(&pkg_id).unwrap(), 3);

    // Now update to v2.0.0
    let manifest_v2 = valid_manifest("versioned-agent", "2.0.0", &["llm.query"]);
    let metadata_v2 = make_metadata("versioned-agent", "2.0.0", &["llm.query"], &["data"]);
    let unsigned_v2 = create_unsigned_bundle(
        &manifest_v2,
        agent_code(),
        metadata_v2,
        "local://versioned-agent",
        "test",
    )
    .unwrap();

    let returned_id = registry
        .update(&pkg_id, unsigned_v2, &key, "Bump to v2")
        .unwrap();
    assert_eq!(returned_id, pkg_id);

    // Downloads should be preserved
    let count = registry.download_count(&pkg_id).unwrap();
    assert_eq!(count, 3);

    // Agent detail should show new version
    let detail = registry.get_agent(&pkg_id).unwrap();
    assert_eq!(detail.version, "2.0.0");

    // Version history should have both versions
    let versions = registry.version_history(&pkg_id).unwrap();
    assert_eq!(versions.len(), 2);
}

// ── Test 10: Search finds agent by name and tags ─────────────────────────────

#[test]
fn search_finds_agent_by_name_and_tags() {
    let registry = SqliteRegistry::open_in_memory().unwrap();
    let key = test_key();

    publish_test_agent(
        &registry,
        "code-reviewer",
        "1.0.0",
        &["llm.query", "fs.read"],
        &["coding", "review"],
        &key,
    );
    publish_test_agent(
        &registry,
        "data-pipeline",
        "1.0.0",
        &["fs.read", "fs.write"],
        &["data", "etl"],
        &key,
    );
    publish_test_agent(
        &registry,
        "social-poster",
        "1.0.0",
        &["llm.query", "social.post"],
        &["social", "content"],
        &key,
    );

    // Search by name
    let results = registry.search("code-reviewer").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "code-reviewer");

    // Search by partial name
    let results = registry.search("pipeline").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "data-pipeline");

    // Search by tag
    let results = registry.search("social").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "social-poster");

    // Search by description substring (all descriptions contain "agent")
    let results = registry.search("agent").unwrap();
    assert_eq!(results.len(), 3);

    // Search with no matches
    let results = registry.search("zzz-nonexistent").unwrap();
    assert!(results.is_empty());
}

// ── Test 11: Full developer workflow roundtrip ───────────────────────────────

#[test]
fn full_developer_workflow_create_test_package_publish_install_roundtrip() {
    let registry = SqliteRegistry::open_in_memory().unwrap();
    let key = test_key();

    // Step 1: Create manifest (simulating `nexus create`)
    let manifest = valid_manifest("roundtrip-agent", "1.0.0", &["llm.query", "fs.read"]);

    // Step 2: Test — parse manifest (simulating `nexus test`)
    let parsed = nexus_kernel::manifest::parse_manifest(&manifest);
    assert!(parsed.is_ok());
    let parsed = parsed.unwrap();
    assert_eq!(parsed.name, "roundtrip-agent");
    assert_eq!(parsed.capabilities, vec!["llm.query", "fs.read"]);

    // Step 3: Package — create signed bundle (simulating `nexus package`)
    let metadata = make_metadata(
        "roundtrip-agent",
        "1.0.0",
        &["llm.query", "fs.read"],
        &["test"],
    );
    let unsigned = create_unsigned_bundle(
        &manifest,
        agent_code(),
        metadata,
        "local://roundtrip-agent",
        "test-cli",
    )
    .unwrap();
    let signed = sign_package(unsigned, &key).unwrap();
    assert!(verify_package(&signed).is_ok());

    // Step 4: Publish — verify and insert (simulating `nexus marketplace publish`)
    let verification = verify_bundle(&signed);
    assert!(verification.may_list());
    registry.upsert_signed(&signed).unwrap();

    // Step 5: Search — find in registry (simulating `nexus marketplace search`)
    let results = registry.search("roundtrip").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "roundtrip-agent");

    // Step 6: Info — get full detail (simulating `nexus marketplace info`)
    let detail = registry.get_agent(&signed.package_id).unwrap();
    assert_eq!(detail.name, "roundtrip-agent");
    assert_eq!(detail.version, "1.0.0");

    // Step 7: Install — download and verify (simulating `nexus marketplace install`)
    let installed = registry.install(&signed.package_id).unwrap();
    assert!(verify_package(&installed).is_ok());
    assert_eq!(installed.metadata.name, "roundtrip-agent");
    assert_eq!(
        installed.metadata.capabilities,
        vec!["llm.query", "fs.read"]
    );

    // Verify download count incremented
    assert_eq!(registry.download_count(&signed.package_id).unwrap(), 1);

    // Step 8: Rate — leave a review
    registry
        .rate(&signed.package_id, "tester", 5, "Great agent!")
        .unwrap();
    let reviews = registry.get_reviews(&signed.package_id).unwrap();
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0].stars, 5);

    // Verify the full chain is consistent
    let detail = registry.get_agent(&signed.package_id).unwrap();
    assert_eq!(detail.downloads, 1);
    assert_eq!(detail.review_count, 1);
    assert!((detail.rating - 5.0).abs() < f64::EPSILON);
}

// ── Additional: different author key produces different signature ─────────────

#[test]
fn different_author_key_produces_different_package_id() {
    let key1 = test_key();
    let key2 = other_key();

    let manifest = valid_manifest("multi-author", "1.0.0", &["llm.query"]);
    let meta1 = make_metadata("multi-author", "1.0.0", &["llm.query"], &[]);
    let meta2 = make_metadata("multi-author", "1.0.0", &["llm.query"], &[]);

    let unsigned1 =
        create_unsigned_bundle(&manifest, agent_code(), meta1, "local://a", "test").unwrap();
    let unsigned2 =
        create_unsigned_bundle(&manifest, agent_code(), meta2, "local://b", "test").unwrap();

    let signed1 = sign_package(unsigned1, &key1).unwrap();
    let signed2 = sign_package(unsigned2, &key2).unwrap();

    // Both should verify independently
    assert!(verify_package(&signed1).is_ok());
    assert!(verify_package(&signed2).is_ok());

    // Different keys produce different signatures
    assert_ne!(signed1.signature, signed2.signature);
}
