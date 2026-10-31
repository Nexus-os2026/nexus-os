//! M0-FIX F3 oracle: cognitive-loop web search no longer shells out to curl.
//!
//! The broker (`crates/nexus-governed-control`) has no governed web-search
//! capability (`planned::classify` keeps `WebSearch` inert), and the kernel
//! cannot call the broker. So web search is closed: there is no built-in
//! search backend, and without a backend the host injects, a `WebSearch`
//! step is refused with an honest error before any process or connection.
//! An injected backend (here the hermetic test backend) still serves it.
//!
//! The source half is a narrow guard over the production part of
//! `kernel/src/actuators/web.rs`, not a Rust parser. It allows the one curl
//! request builder that agent `WebFetch` still uses (`curl_get`, reached
//! only from `fetch_content`); WebFetch is a recorded follow-up, not part of
//! this fix.

use nexus_kernel::actuators::{
    ActuatorContext, ActuatorRegistry, GovernedWeb, WebSearchBackend, WebSearchResult,
};
use nexus_kernel::audit::AuditTrail;
use nexus_kernel::autonomy::AutonomyLevel;
use nexus_kernel::cognitive::{ActionExecutor, PlannedAction, RegistryExecutor};
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::supervisor::Supervisor;
use nexus_kernel::test_support::HermeticSearchBackend;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

const CLOSED: &str = "web search is closed";

const WEB_RS: &str = include_str!("../src/actuators/web.rs");

/// The production part of `web.rs`: everything before its test module.
fn production(source: &str) -> String {
    let source = source.replace("\r\n", "\n");
    source
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap()
        .to_string()
}

/// The span of the top-level `fn name(` item: up to its closing brace at
/// column 0.
fn top_level_fn(source: &str, name: &str) -> Option<std::ops::Range<usize>> {
    let start = source.find(&format!("\nfn {name}("))?;
    let end = start + source[start..].find("\n}\n")? + 3;
    Some(start..end)
}

/// The spans of every method `fn name(` with a body inside an impl block:
/// up to its closing brace at four spaces. A trait declaration
/// (`fn name(..) -> T;`) has no body and no span: its span would run on to
/// the next method's closing brace and hide a call there.
fn methods(source: &str, name: &str) -> Vec<std::ops::Range<usize>> {
    let needle = format!("\n    fn {name}(");
    let mut spans = Vec::new();
    let mut from = 0;
    while let Some(at) = source[from..].find(&needle) {
        let start = from + at;
        from = start + needle.len();
        let line_end = source[start + 1..]
            .find('\n')
            .map_or(source.len(), |i| start + 1 + i);
        if !source[start..line_end].trim_end().ends_with('{') {
            continue;
        }
        let Some(len) = source[start..].find("\n    }\n") else {
            break;
        };
        spans.push(start..start + len + 7);
    }
    spans
}

/// Every reason the production source still lets search reach a process.
fn search_process_violations(source: &str) -> Vec<String> {
    let prod = production(source);
    let mut violations = Vec::new();
    for banned in [
        "duckduckgo",
        "hnrss",
        "SEARXNG_URL",
        "searxng",
        "with curl",
        "ShellCommand",
    ] {
        if prod
            .to_ascii_lowercase()
            .contains(&banned.to_ascii_lowercase())
        {
            violations.push(format!("names {banned}"));
        }
    }
    // One process builder only: curl_get's, for WebFetch.
    let spawns: Vec<usize> = prod.match_indices("Command::new").map(|(i, _)| i).collect();
    let curl_get = top_level_fn(&prod, "curl_get");
    match (&curl_get, spawns.as_slice()) {
        (Some(span), [at]) if span.contains(at) => {}
        _ => violations.push(format!(
            "{} process builders, expected one inside curl_get",
            spawns.len()
        )),
    }
    // curl_get is called only from a fetch_content method.
    let fetches = methods(&prod, "fetch_content");
    for (at, _) in prod.match_indices("curl_get(") {
        let is_definition = prod[..at].ends_with("fn ");
        if !is_definition && !fetches.iter().any(|span| span.contains(&at)) {
            violations.push(format!(
                "curl_get called outside fetch_content at byte {at}"
            ));
        }
    }
    violations
}

#[test]
fn m0_f3_web_search_builds_no_process() {
    // Planted controls: each way back to curl is caught.
    let shape = |prod: &str| format!("{prod}#[cfg(test)]\nmod tests {{}}\n");
    // The trait declaration comes first, as in web.rs: a body-less
    // `fn fetch_content(..);` must not cover the next method's body.
    let clean = "pub trait WebSearchBackend {\n    fn search(&self, q: &str) -> R;\n\n    fn fetch_content(&self, url: &str) -> R;\n}\n\nimpl WebSearchBackend for B {\n    fn search(&self, q: &str) -> R {\n        Err(CLOSED.into())\n    }\n\n    fn fetch_content(&self, url: &str) -> R {\n        curl_get(url, false)\n    }\n}\n\nfn curl_get(url: &str, f: bool) -> R {\n    let mut command = Command::new(\"curl\");\n}\n";
    assert!(search_process_violations(&shape(clean)).is_empty());
    for planted in [
        clean.replace("Err(CLOSED.into())", "curl_get(q, true)"),
        clean.replace(
            "Err(CLOSED.into())",
            "Command::new(\"curl\").arg(q).output()",
        ),
        clean.replace("Err(CLOSED.into())", "Command::new(\"sh\").arg(q).output()"),
        clean.replace("Err(CLOSED.into())", "get(\"https://html.duckduckgo.com\")"),
        clean.replace("Err(CLOSED.into())", "Err(\"use ShellCommand with curl\")"),
        clean.replace("Err(CLOSED.into())", "std::env::var(\"SEARXNG_URL\")"),
    ] {
        assert!(
            !search_process_violations(&shape(&planted)).is_empty(),
            "{planted}"
        );
    }
    let found = search_process_violations(WEB_RS);
    assert!(
        found.is_empty(),
        "web search can still reach a process: {found:#?}"
    );
}

fn web_context() -> ActuatorContext {
    let mut capabilities = HashSet::new();
    capabilities.insert("web.search".to_string());
    capabilities.insert("web.read".to_string());
    ActuatorContext {
        agent_id: uuid::Uuid::new_v4().to_string(),
        agent_name: "f3-agent".into(),
        working_dir: std::env::temp_dir(),
        autonomy_level: AutonomyLevel::L3,
        capabilities,
        fuel_remaining: 1000.0,
        egress_allowlist: vec![],
        action_review_engine: None,
        hitl_approved: true,
    }
}

fn search(query: &str) -> PlannedAction {
    PlannedAction::WebSearch {
        query: query.to_string(),
    }
}

fn web_agent() -> (Arc<Mutex<Supervisor>>, String) {
    let mut sup = Supervisor::new();
    let id = sup
        .start_agent(AgentManifest {
            name: "f3-agent".into(),
            version: "1.0.0".into(),
            capabilities: vec!["web.search".into(), "web.read".into()],
            fuel_budget: 10000,
            autonomy_level: Some(3),
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            default_goal: None,
            llm_model: None,
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
            filesystem_permissions: vec![],
        })
        .unwrap();
    (Arc::new(Mutex::new(sup)), id.to_string())
}

/// With the default registry and no injected backend, a search is refused
/// with the honest error, and no connection reaches even an operator
/// SearXNG address (the old backend probed it with curl first).
#[test]
fn m0_f3_default_web_search_is_refused_without_a_connection() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let operator = format!("http://{}", listener.local_addr().unwrap());
    std::env::set_var("SEARXNG_URL", &operator);

    let registry = ActuatorRegistry::with_defaults();
    let mut audit = AuditTrail::new();
    let refused = registry.execute_action(&search("f3 oracle query"), &web_context(), &mut audit);

    let (sup, agent_id) = web_agent();
    let executor = RegistryExecutor::new(
        std::env::temp_dir(),
        Arc::new(Mutex::new(AuditTrail::new())),
        sup,
        None,
    );
    let through_loop = executor.execute(&agent_id, &search("f3 oracle query"), &mut audit, true);
    std::env::remove_var("SEARXNG_URL");

    let err = refused.unwrap_err().to_string();
    assert!(err.contains(CLOSED), "{err}");
    let err = through_loop.unwrap_err();
    assert!(err.contains(CLOSED), "{err}");
    assert!(
        matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock),
        "the search reached the network"
    );
}

/// An injected backend still serves search, directly and through the
/// cognitive executor.
#[test]
fn m0_f3_injected_hermetic_backend_still_searches() {
    let web = GovernedWeb::with_backend(Arc::new(HermeticSearchBackend));
    use nexus_kernel::actuators::Actuator;
    let out = web.execute(&search("rust"), &web_context()).unwrap();
    assert!(out.output.contains("Result for: rust"), "{}", out.output);

    let (sup, agent_id) = web_agent();
    let executor = RegistryExecutor::new(
        std::env::temp_dir(),
        Arc::new(Mutex::new(AuditTrail::new())),
        sup,
        None,
    )
    .with_web_backend(Arc::new(HermeticSearchBackend));
    let mut audit = AuditTrail::new();
    let out = executor
        .execute(&agent_id, &search("rust"), &mut audit, true)
        .unwrap();
    assert!(out.contains("Result for: rust"), "{out}");

    // A failing injected backend is reported as such, not as closed.
    struct Failing;
    impl WebSearchBackend for Failing {
        fn search(&self, _: &str) -> Result<Vec<WebSearchResult>, String> {
            Err("backend down".into())
        }
        fn fetch_content(&self, _: &str) -> Result<String, String> {
            Err("backend down".into())
        }
    }
    let err = GovernedWeb::with_backend(Arc::new(Failing))
        .execute(&search("rust"), &web_context())
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("backend down") && !err.contains(CLOSED),
        "{err}"
    );
}
