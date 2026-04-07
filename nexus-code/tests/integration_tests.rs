use nexus_code::config::NxConfig;
use nexus_code::governance::{AuthorizationResult, GovernanceKernel};

#[test]
fn test_governance_kernel_creation() {
    let kernel = GovernanceKernel::new(50_000).unwrap();
    assert_eq!(kernel.audit.len(), 1);
    let first = &kernel.audit.entries()[0];
    assert_eq!(first.sequence, 0);
    assert_eq!(
        first.previous_hash,
        "0000000000000000000000000000000000000000000000000000000000000000"
    );
    assert!(kernel.audit.verify_chain().is_ok());
}

#[test]
fn test_config_defaults() {
    let config = NxConfig::default();
    assert_eq!(config.fuel_budget, 50_000);
    assert_eq!(config.default_provider, "anthropic");
    assert_eq!(config.default_model, "claude-sonnet-4-20250514");
    assert!(config.blocked_paths.is_empty());
    assert!(config.auto_approve.contains(&"file_read".to_string()));
}

#[test]
fn test_audit_chain_after_governance_pipeline() {
    let mut kernel = GovernanceKernel::new(50_000).unwrap();

    let result = kernel.authorize_tool("file_read", "/test/path", 100);
    assert!(result.is_ok());

    assert!(kernel.audit.verify_chain().is_ok());
    assert!(kernel.audit.len() > 1);
}

#[test]
fn test_full_governance_flow() {
    let mut kernel = GovernanceKernel::new(10_000).unwrap();

    // Read operations are auto-approved
    assert!(kernel
        .authorize_tool("file_read", "/src/main.rs", 50)
        .is_ok());
    assert!(kernel.authorize_tool("search", "search_term", 50).is_ok());
    assert!(kernel.authorize_tool("glob", "*.rs", 50).is_ok());

    assert!(kernel.audit.len() >= 4);
    assert!(kernel.audit.verify_chain().is_ok());
    assert!(kernel.fuel.remaining() < 10_000);
}

#[test]
fn test_two_phase_consent_flow() {
    let mut kernel = GovernanceKernel::new(50_000).unwrap();

    // Grant file_write capability so capability check passes
    kernel.capabilities.grant(
        nexus_code::governance::Capability::FileWrite,
        nexus_code::governance::CapabilityScope::Full,
    );

    // Phase 1: authorize returns ConsentNeeded
    let result = kernel
        .authorize_tool("file_write", "/test.rs", 100)
        .unwrap();
    match result {
        AuthorizationResult::ConsentNeeded(req) => {
            // Phase 2: user approves
            let decision = kernel.finalize_authorization(&req, true, 100).unwrap();
            assert!(decision.granted);
        }
        AuthorizationResult::Authorized(_) => panic!("Expected consent needed"),
    }

    assert!(kernel.audit.verify_chain().is_ok());
}

#[test]
fn test_denied_capability_audit() {
    let mut kernel = GovernanceKernel::new(50_000).unwrap();

    let result = kernel.authorize_tool("file_delete", "/important/file", 100);
    assert!(result.is_err());

    assert!(kernel.audit.verify_chain().is_ok());
}

#[test]
fn test_config_load_defaults() {
    let config = NxConfig::load().unwrap();
    assert_eq!(config.fuel_budget, 50_000);
    // Provider is auto-detected: claude_cli if available, then anthropic, openai, ollama
    let valid_providers = ["claude_cli", "anthropic", "openai", "ollama"];
    assert!(
        valid_providers.contains(&config.default_provider.as_str()),
        "unexpected default provider: {}",
        config.default_provider
    );
}

#[test]
fn test_session_identity_in_audit() {
    let kernel = GovernanceKernel::new(50_000).unwrap();
    let session_id = kernel.identity.session_id().to_string();

    let first = &kernel.audit.entries()[0];
    assert_eq!(first.session_id, session_id);
}

#[test]
fn test_record_fuel_creates_audit_entry() {
    let mut kernel = GovernanceKernel::new(50_000).unwrap();
    let len_before = kernel.audit.len();
    kernel.record_fuel(
        "anthropic",
        nexus_code::governance::FuelCost {
            input_tokens: 100,
            output_tokens: 200,
            fuel_units: 300,
            estimated_usd: 0.001,
        },
    );
    assert_eq!(kernel.audit.len(), len_before + 1);
    assert!(kernel.audit.verify_chain().is_ok());
}
