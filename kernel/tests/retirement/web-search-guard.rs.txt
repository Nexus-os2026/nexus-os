//! Independently runnable F3 split through K11. K03 moves only PlannedAction's
//! import to action_types; K12 retires this actuator-dependent binary and
//! activates the absence controls. No cognitive runtime/executor imports.
#[path = "retirement/web_search_source.rs"]
mod web_search_source;

use nexus_kernel::actuators::{
    Actuator, ActuatorContext, ActuatorRegistry, CurlWebBackend, GovernedWeb, WebSearchBackend,
    WebSearchResult,
};
use nexus_kernel::audit::AuditTrail;
use nexus_kernel::autonomy::AutonomyLevel;
use nexus_kernel::cognitive::types::PlannedAction;
use nexus_kernel::test_support::HermeticSearchBackend;
use std::sync::Arc;

#[test]
fn f3_source_prohibition_survives_without_the_cognitive_executor() {
    let findings =
        web_search_source::search_process_violations(include_str!("../src/actuators/web.rs"));
    assert!(findings.is_empty(), "{findings:?}");
}

fn context() -> ActuatorContext {
    ActuatorContext {
        agent_id: uuid::Uuid::new_v4().to_string(),
        agent_name: "k01-f3".into(),
        working_dir: std::env::temp_dir(),
        autonomy_level: AutonomyLevel::L3,
        capabilities: ["web.search".into(), "web.read".into()].into(),
        fuel_remaining: 1000.0,
        egress_allowlist: vec![],
        action_review_engine: None,
        hitl_approved: true,
    }
}

#[test]
fn f3_direct_default_search_refuses_without_a_connection() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    // This binary has one environment-mutating test; the other tests do not
    // read this variable. The call is synchronous, so accept needs no sleep.
    std::env::set_var(
        "SEARXNG_URL",
        format!("http://{}", listener.local_addr().unwrap()),
    );
    let action = PlannedAction::WebSearch {
        query: "k01 query".into(),
    };
    let direct = GovernedWeb::new().execute(&action, &context());
    let registry = ActuatorRegistry::with_defaults().execute_action(
        &action,
        &context(),
        &mut AuditTrail::new(),
    );
    let backend = CurlWebBackend.search("k01 query");
    std::env::remove_var("SEARXNG_URL");
    for error in [
        direct.unwrap_err().to_string(),
        registry.unwrap_err().to_string(),
        backend.unwrap_err(),
    ] {
        assert!(error.contains("web search is closed"), "{error}");
    }
    assert!(
        matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock),
        "search reached the network"
    );
}

#[test]
fn f3_injected_hermetic_search_still_works_directly() {
    let result = GovernedWeb::with_backend(Arc::new(HermeticSearchBackend))
        .execute(
            &PlannedAction::WebSearch {
                query: "rust".into(),
            },
            &context(),
        )
        .unwrap();
    assert!(
        result.output.contains("Result for: rust"),
        "{}",
        result.output
    );
}

/// The predecessor requires backend failure to stay an error with its original
/// cause, never an empty successful result or the default backend's refusal.
#[test]
fn f3_injected_backend_failure_preserves_the_original_error() {
    struct Failing;
    impl WebSearchBackend for Failing {
        fn search(&self, _: &str) -> Result<Vec<WebSearchResult>, String> {
            Err("backend down".into())
        }
        fn fetch_content(&self, _: &str) -> Result<String, String> {
            Err("backend down".into())
        }
    }
    let error = GovernedWeb::with_backend(Arc::new(Failing))
        .execute(
            &PlannedAction::WebSearch {
                query: "rust".into(),
            },
            &context(),
        )
        .expect_err("injected backend failure must not become successful empty results")
        .to_string();
    assert!(error.contains("backend down"), "{error}");
    assert!(!error.contains("web search is closed"), "{error}");
}
