//! P3 D-2: an agent whose configuration is local-only never has a prompt
//! dispatched to a cloud provider.
//!
//! The unit tests drive the planner's dispatch boundary with a fake local
//! runtime and a recording dispatch, so nothing is sent anywhere. The
//! end-to-end tests run the real route resolution, policy, per-unit
//! dispatch contexts and planner in a child process whose only reachable services
//! are a fake Ollama runtime on loopback and a recording proxy that every
//! non-loopback request goes to; a remote-provider invocation is a proxy
//! connection, and no prompt can leave the machine.

use super::*;
use nexus_connectors_llm::gateway::{
    local_only_ollama_base, ollama_model_name_is_cloud, ollama_show_names_remote,
};
use nexus_connectors_llm::providers::LlmResponse;
use nexus_kernel::cognitive::{AgentGoal, PlannerLlm};
use std::cell::RefCell;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Stdio;
use std::sync::mpsc;
use std::time::Duration;

const PROMPT: &str = "P3 D-2 synthetic prompt";
const LOOPBACK: &str = "http://localhost:11434";
const FAKE_KEY: &str = "p3-d2-fake-key-not-a-credential";

/// A provider configuration in which every cloud provider and both CLI
/// providers are configured.
fn cloud_keys() -> ProviderSelectionConfig {
    let key = || Some(FAKE_KEY.to_string());
    ProviderSelectionConfig {
        provider: None,
        ollama_url: Some(LOOPBACK.to_string()),
        deepseek_api_key: key(),
        anthropic_api_key: key(),
        openai_api_key: key(),
        gemini_api_key: key(),
        groq_api_key: key(),
        mistral_api_key: key(),
        together_api_key: key(),
        fireworks_api_key: key(),
        perplexity_api_key: key(),
        cohere_api_key: key(),
        openrouter_api_key: key(),
        nvidia_api_key: key(),
        flash_model_path: None,
        claude_code_enabled: true,
        codex_cli_enabled: true,
    }
}

/// A local runtime: its installed models, the ones it reports as remote,
/// and every question asked of it.
#[derive(Default)]
struct FakeRuntime {
    installed: Vec<&'static str>,
    remote: Vec<&'static str>,
    unavailable: bool,
    asked: RefCell<Vec<String>>,
}

impl FakeRuntime {
    fn with(installed: &[&'static str]) -> Self {
        Self {
            installed: installed.to_vec(),
            ..Self::default()
        }
    }
}

impl LocalOllamaRuntime for FakeRuntime {
    fn installed_models(&self, base: &str) -> Result<Vec<String>, String> {
        self.asked.borrow_mut().push(format!("tags {base}"));
        if self.unavailable {
            return Err("not running".into());
        }
        Ok(self.installed.iter().map(|m| m.to_string()).collect())
    }

    fn verify_local_model(&self, base: &str, model: &str) -> Result<(), String> {
        self.asked.borrow_mut().push(format!("show {base} {model}"));
        if self.unavailable {
            return Err("not running".into());
        }
        if !self.installed.contains(&model) {
            return Err(format!("'{model}' is not installed"));
        }
        if self.remote.contains(&model) {
            return Err(format!("'{model}' runs remotely"));
        }
        Ok(())
    }
}

/// One dispatch the planner made: provider name, endpoint, model.
type Dispatched = (String, String, String);

/// Runs the planner's dispatch with a recording dispatch that answers
/// `answer` (or fails when `None`).
fn dispatch(
    policy: AgentLlmPolicy,
    route: Option<&str>,
    base: Result<String, String>,
    runtime: &FakeRuntime,
    answer: Option<&str>,
) -> (Result<String, AgentError>, Vec<Dispatched>) {
    let calls = RefCell::new(Vec::new());
    let record = |provider: &dyn LlmProvider, prompt: &str, model: &str| {
        assert_eq!(prompt, PROMPT);
        calls.borrow_mut().push((
            provider.name().to_string(),
            provider.endpoint_url(),
            model.to_string(),
        ));
        match answer {
            Some(text) => Ok(LlmResponse {
                output_text: text.to_string(),
                token_count: 1,
                model_name: model.to_string(),
                tool_calls: Vec::new(),
                input_tokens: None,
            }),
            None => Err(AgentError::SupervisorError(
                "the local runtime failed".into(),
            )),
        }
    };
    let result = planner_dispatch(
        policy,
        route.map(str::to_string),
        &cloud_keys(),
        base,
        runtime,
        PROMPT,
        &record,
    );
    (result, calls.into_inner())
}

/// The route policy of `agent_id` as the issuer resolves it (§4.2; ported
/// from the removed `resolve_agent_llm_policy`, mission R12): an id that is
/// not a registered agent, or a policy that cannot be established, is
/// `Unresolved`.
fn issuer_policy(state: &AppState, agent_id: &str) -> AgentLlmPolicy {
    let Ok(agent) = Uuid::parse_str(agent_id) else {
        return AgentLlmPolicy::Unresolved;
    };
    match state.hm_issuer().unwrap().resolve_agent_route_policy(agent) {
        Ok(policy) if policy.local_only() => AgentLlmPolicy::LocalOnly,
        Ok(_) => AgentLlmPolicy::Unrestricted,
        Err(_) => AgentLlmPolicy::Unresolved,
    }
}

fn loopback() -> Result<String, String> {
    Ok(LOOPBACK.to_string())
}

fn assert_refused(result: &Result<String, AgentError>, context: &str) {
    let error = result
        .as_ref()
        .expect_err(&format!("{context}: must be refused"))
        .to_string();
    assert!(
        error.contains("local-only"),
        "{context}: refusal must name the local-only policy: {error}"
    );
}

const CLOUD_ROUTES: [&str; 20] = [
    "anthropic/claude-sonnet-4-6",
    "claude",
    "anthropic",
    "openai",
    "openai/gpt-4o",
    "openai/gpt-5",
    "deepseek",
    "deepseek/deepseek-chat",
    "gemini",
    "google/gemini-2.5-flash",
    "groq/llama-3.3-70b-versatile",
    "nvidia/meta/llama-3.3-70b-instruct",
    "openrouter/meta-llama/llama-3.3-70b-instruct:free",
    "claude-code/sonnet",
    "codex-cli/gpt-5",
    "codex",
    "claude-sonnet-4-6",
    "gpt-4o",
    "mock-1",
    "https://api.example.com/v1:model",
];

#[test]
fn p3_d2_explicit_cloud_model_under_local_only_is_refused() {
    for route in CLOUD_ROUTES {
        let runtime = FakeRuntime::with(&["qwen3:8b"]);
        let (result, calls) = dispatch(
            AgentLlmPolicy::LocalOnly,
            Some(route),
            loopback(),
            &runtime,
            Some("must not answer"),
        );
        assert_refused(&result, route);
        assert!(calls.is_empty(), "{route}: dispatched {calls:?}");
        assert!(
            runtime.asked.borrow().is_empty(),
            "{route}: a refused route asks nothing of the runtime"
        );
    }
}

#[test]
fn p3_d2_cloud_configured_and_local_model_absent_is_refused() {
    for route in [None, Some("ollama"), Some("auto"), Some("")] {
        for runtime in [
            FakeRuntime::with(&[]),
            FakeRuntime {
                unavailable: true,
                ..FakeRuntime::default()
            },
        ] {
            let (result, calls) = dispatch(
                AgentLlmPolicy::LocalOnly,
                route,
                loopback(),
                &runtime,
                Some("must not answer"),
            );
            assert_refused(&result, &format!("{route:?}"));
            assert!(calls.is_empty(), "{route:?}: dispatched {calls:?}");
        }
    }
    // A named local model that is not installed is refused, not replaced.
    let runtime = FakeRuntime::with(&["llama3.2:3b"]);
    let (result, calls) = dispatch(
        AgentLlmPolicy::LocalOnly,
        Some("ollama/qwen3:8b"),
        loopback(),
        &runtime,
        Some("must not answer"),
    );
    assert!(result.is_err());
    assert!(calls.is_empty(), "dispatched {calls:?}");
}

#[test]
fn p3_d2_auto_routing_with_cloud_credentials_never_reaches_a_cloud_model() {
    // No route (auto found nothing local) with every cloud key configured,
    // and runtimes whose only models are cloud models.
    let cloud_named = FakeRuntime::with(&["gpt-oss:120b-cloud", "deepseek-v3.1:671b-cloud"]);
    let (result, calls) = dispatch(
        AgentLlmPolicy::LocalOnly,
        None,
        loopback(),
        &cloud_named,
        Some("must not answer"),
    );
    assert_refused(&result, "cloud-named models only");
    assert!(calls.is_empty(), "dispatched {calls:?}");

    let relayed = FakeRuntime {
        installed: vec!["qwen3:8b"],
        remote: vec!["qwen3:8b"],
        ..FakeRuntime::default()
    };
    let (result, calls) = dispatch(
        AgentLlmPolicy::LocalOnly,
        None,
        loopback(),
        &relayed,
        Some("must not answer"),
    );
    assert!(result.is_err(), "a relayed model is refused");
    assert!(calls.is_empty(), "dispatched {calls:?}");

    // A cloud model named as an Ollama route is refused before the runtime
    // is asked, and so is one auto-selection would pick by size.
    let runtime = FakeRuntime::with(&["kimi-k2:1t-cloud"]);
    for route in [
        "kimi-k2:1t-cloud",
        "ollama/kimi-k2:1t-cloud",
        "glm-4.6:cloud",
    ] {
        let (result, calls) = dispatch(
            AgentLlmPolicy::LocalOnly,
            Some(route),
            loopback(),
            &runtime,
            Some("must not answer"),
        );
        assert_refused(&result, route);
        assert!(calls.is_empty(), "{route}: dispatched {calls:?}");
    }
    assert!(
        runtime.asked.borrow().is_empty(),
        "a cloud-named model is refused before the runtime is asked"
    );
}

#[test]
fn p3_d2_goal_override_memory_mapping_and_manifest_selecting_cloud_are_refused() {
    // The goal override and the manifest give the route verbatim; a memory
    // mapping gives `provider/model`.
    let mapped = [
        serde_json::json!({"provider": "openai", "model": "gpt-4o"}),
        serde_json::json!({"planning": {"provider": "anthropic", "model": "claude-sonnet-4-6"}}),
        serde_json::json!({"any": {"provider": "groq", "model": "llama-3.3-70b-versatile"}}),
    ]
    .map(|value| crate::route_from_model_mapping(&value).unwrap());
    assert_eq!(mapped[0], "openai/gpt-4o");
    let routes = mapped.iter().map(String::as_str).chain([
        "anthropic/claude-sonnet-4-6", // goal override from the Chat page dropdown
        "groq/llama-3.3-70b-versatile", // manifest llm_model
    ]);
    for route in routes {
        let runtime = FakeRuntime::with(&["qwen3:8b"]);
        let (result, calls) = dispatch(
            AgentLlmPolicy::LocalOnly,
            Some(route),
            loopback(),
            &runtime,
            Some("must not answer"),
        );
        assert_refused(&result, route);
        assert!(calls.is_empty(), "{route}: dispatched {calls:?}");
    }
}

#[test]
fn p3_d2_remote_or_ambiguous_ollama_endpoint_is_refused() {
    let bases = [
        "http://10.0.0.5:11434",
        "http://192.168.1.2:11434",
        "http://ollama.example.com:11434",
        "http://localhost.example.com:11434",
        "http://127.0.0.1.nip.io:11434",
        "http://localhost.:11434",
        "http://0.0.0.0:11434",
        "http://[::]:11434",
        "http://[::ffff:127.0.0.1]:11434",
        "http://[::ffff:7f00:1]:11434",
        "http://user@127.0.0.1:11434",
        "http://127.0.0.1:11434/relay",
        "http://127.0.0.1:11434/?to=remote",
        "http://127.0.0.1:11434/#frag",
        "ftp://127.0.0.1:11434",
        "127.0.0.1:11434",
        "",
    ];
    for base in bases {
        let runtime = FakeRuntime::with(&["qwen3:8b"]);
        for route in [Some("ollama/qwen3:8b"), Some("ollama"), None] {
            let (result, calls) = dispatch(
                AgentLlmPolicy::LocalOnly,
                route,
                Ok(base.to_string()),
                &runtime,
                Some("must not answer"),
            );
            assert_refused(&result, base);
            assert!(calls.is_empty(), "{base}: dispatched {calls:?}");
        }
        assert!(
            runtime.asked.borrow().is_empty(),
            "{base}: nothing is asked of a non-loopback address"
        );
    }
    // An operator address that is set but unusable leaves Ollama unavailable.
    let runtime = FakeRuntime::with(&["qwen3:8b"]);
    let (result, calls) = dispatch(
        AgentLlmPolicy::LocalOnly,
        Some("ollama/qwen3:8b"),
        Err("the Ollama address is unusable".into()),
        &runtime,
        Some("must not answer"),
    );
    assert_refused(&result, "unusable address");
    assert!(calls.is_empty());

    // The loopback forms accepted, each naming the address connected to.
    for (base, expected) in [
        ("http://localhost:11434", "http://127.0.0.1:11434"),
        ("HTTP://LocalHost:11434/", "http://127.0.0.1:11434"),
        ("http://127.0.0.1:11434", "http://127.0.0.1:11434"),
        ("http://127.8.9.10:8080", "http://127.8.9.10:8080"),
        ("http://[::1]:11434", "http://[::1]:11434"),
        ("http://localhost", "http://127.0.0.1"),
    ] {
        assert_eq!(local_only_ollama_base(base).unwrap(), expected, "{base}");
    }
}

#[test]
fn p3_d2_local_provider_unavailable_after_selection_fails_without_fallback() {
    let runtime = FakeRuntime::with(&["qwen3:8b"]);
    for attempt in 0..3 {
        let (result, calls) = dispatch(
            AgentLlmPolicy::LocalOnly,
            Some("ollama/qwen3:8b"),
            loopback(),
            &runtime,
            None,
        );
        assert_eq!(
            result.unwrap_err().to_string(),
            AgentError::SupervisorError("the local runtime failed".into()).to_string(),
            "attempt {attempt}: the local failure is the result"
        );
        assert_eq!(
            calls,
            vec![(
                "ollama".to_string(),
                "http://127.0.0.1:11434".to_string(),
                "qwen3:8b".to_string()
            )],
            "attempt {attempt}: one local dispatch, no fallback"
        );
    }
}

#[test]
fn p3_d2_fallback_after_local_failure_never_reaches_cloud() {
    // A failed verification, a failed listing and an unloaded Flash model
    // each end the request; with every cloud key configured, none of them
    // becomes a cloud dispatch.
    let flaky = FakeRuntime {
        installed: vec!["qwen3:8b"],
        unavailable: true,
        ..FakeRuntime::default()
    };
    for route in [Some("ollama/qwen3:8b"), Some("ollama"), None] {
        let (result, calls) = dispatch(
            AgentLlmPolicy::LocalOnly,
            route,
            loopback(),
            &flaky,
            Some("must not answer"),
        );
        assert!(result.is_err(), "{route:?}");
        assert!(calls.is_empty(), "{route:?}: dispatched {calls:?}");
    }
    for route in ["flash", "flash:session", "flash/model.gguf"] {
        let (result, calls) = dispatch(
            AgentLlmPolicy::LocalOnly,
            Some(route),
            loopback(),
            &FakeRuntime::with(&["qwen3:8b"]),
            Some("must not answer"),
        );
        assert!(result.is_err(), "{route}");
        assert!(calls.is_empty(), "{route}: dispatched {calls:?}");
    }
}

#[test]
fn p3_d2_unresolved_policy_dispatches_nothing() {
    let runtime = FakeRuntime::with(&["qwen3:8b"]);
    for route in [
        None,
        Some("ollama/qwen3:8b"),
        Some("anthropic/claude-sonnet-4-6"),
    ] {
        let (result, calls) = dispatch(
            AgentLlmPolicy::Unresolved,
            route,
            loopback(),
            &runtime,
            Some("must not answer"),
        );
        assert_refused(&result, &format!("{route:?}"));
        assert!(calls.is_empty(), "{route:?}: dispatched {calls:?}");
    }
    assert!(runtime.asked.borrow().is_empty());

    // The planner itself refuses before any provider or Flash session: an
    // agent whose policy cannot be established gets no unit to dispatch in
    // (P3-HM-CONTRACT-02 §4.2; the thread-local policy no longer exists).
    let state = AppState::new_in_memory();
    let result =
        crate::hm_wiring::test_support::plan_text(&state, &Uuid::new_v4().to_string(), PROMPT);
    assert_refused(&result, "planner for an agent with no established policy");

    // An id that is not a registered agent has no policy to establish.
    assert_eq!(
        issuer_policy(&state, "not-a-uuid"),
        AgentLlmPolicy::Unresolved
    );
    assert_eq!(
        issuer_policy(&state, &Uuid::new_v4().to_string()),
        AgentLlmPolicy::Unresolved
    );
}

#[test]
fn p3_d2_policy_is_local_only_when_any_assignment_for_the_agent_is() {
    use nexus_kernel::config::{AgentLlmAssignment, NexusConfig};
    let assignment = |local_only| AgentLlmAssignment {
        provider_id: "claude".into(),
        local_only,
        budget_dollars: 0,
        budget_tokens: 0,
    };
    let keys = [
        "id-1".to_string(),
        "Nexus Forge".to_string(),
        "nexus_forge".to_string(),
    ];
    let mut config = NexusConfig::default();
    assert_eq!(
        crate::agent_llm_policy_in(&config, &keys),
        AgentLlmPolicy::Unrestricted
    );
    config
        .agent_llm_assignments
        .insert("id-1".into(), assignment(false));
    assert_eq!(
        crate::agent_llm_policy_in(&config, &keys),
        AgentLlmPolicy::Unrestricted
    );
    config
        .agent_llm_assignments
        .insert("nexus_forge".into(), assignment(true));
    assert_eq!(
        crate::agent_llm_policy_in(&config, &keys),
        AgentLlmPolicy::LocalOnly,
        "a local-only assignment under any key is not shadowed by another"
    );
}

/// Ported (mission R12; P3-HM-CONTRACT-02 §11.0, R4): the policy is no
/// longer a thread-local scope to restore; it is carried by each unit's
/// context. A panic unwinding out of a local-only agent's dispatch leaves
/// nothing behind: the next unit of an unrestricted agent on the same
/// thread keeps its cloud route, and the local-only agent's next unit is
/// local again.
#[test]
fn p3_d2_policy_is_restored_after_the_work_even_when_it_unwinds() {
    let Some(w) = e2e(
        "p3_d2_policy_is_restored_after_the_work_even_when_it_unwinds",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            use crate::hm_wiring::test_support::{goal_unit, plan_text};
            let state = AppState::new_in_memory();
            let local = register_by_supervisor(&state, "p3-d2-local", true);
            let open = register_by_supervisor(&state, "p3-d2-open", false);
            let mut config = nexus_kernel::config::load_config().unwrap();
            config
                .agent_llm_assignments
                .get_mut(&open)
                .unwrap()
                .provider_id = "anthropic/claude-sonnet-4-6".into();
            nexus_kernel::config::save_config(&config).unwrap();
            assert!(goal_unit(&state, &local)
                .unwrap()
                .restrictions()
                .local_only());
            assert!(!goal_unit(&state, &open)
                .unwrap()
                .restrictions()
                .local_only());
            let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut ctx = goal_unit(&state, &local).unwrap();
                let _ = GatewayPlannerLlm::for_loop(&state, None).plan_query(
                    &mut ctx,
                    &mut |_: &mut nexus_kernel::hm::DispatchContext| {
                        panic!("unwind through the unit's dispatch")
                    },
                );
            }));
            assert!(unwound.is_err());
            let cloud = plan_text(&state, &open, PROMPT);
            let local = plan_text(&state, &local, PROMPT);
            Ok(format!("cloud={cloud:?} local={local:?}"))
        },
    ) else {
        return;
    };
    assert!(w.result.starts_with("OK cloud=Err("), "{}", w.result);
    assert!(
        w.result.contains(r#"local=Ok("local answer")"#),
        "{}",
        w.result
    );
    assert!(
        w.proxy
            .iter()
            .any(|line| line.contains("api.anthropic.com")),
        "the unrestricted unit kept its cloud route: {:?}",
        w.proxy
    );
    assert_eq!(w.generates().len(), 1, "{:?}", w.ollama);
}

#[test]
fn p3_d2_permitted_local_request_dispatches_to_the_loopback_runtime() {
    for (route, model) in [
        (Some("ollama/qwen3:8b"), "qwen3:8b"),
        (Some("qwen3:8b"), "qwen3:8b"),
        (Some("ollama"), "qwen2.5-coder:14b"),
        (Some("auto"), "qwen2.5-coder:14b"),
        (None, "qwen2.5-coder:14b"),
    ] {
        let runtime = FakeRuntime::with(&[
            "llama3.2:3b",
            "qwen2.5-coder:14b",
            "gpt-oss:120b-cloud",
            "qwen3:8b",
        ]);
        let (result, calls) = dispatch(
            AgentLlmPolicy::LocalOnly,
            route,
            loopback(),
            &runtime,
            Some("<think>x</think>local answer"),
        );
        assert_eq!(result.unwrap(), "local answer", "{route:?}");
        assert_eq!(
            calls,
            vec![(
                "ollama".to_string(),
                "http://127.0.0.1:11434".to_string(),
                model.to_string()
            )],
            "{route:?}"
        );
        assert_eq!(
            runtime.asked.borrow().last().unwrap(),
            &format!("show http://127.0.0.1:11434 {model}"),
            "{route:?}: the model is verified before the prompt is sent"
        );
    }
}

#[test]
fn p3_d2_unrestricted_agent_keeps_its_explicit_cloud_choice() {
    for (route, provider, model) in [
        ("anthropic/claude-sonnet-4-6", "claude", "claude-sonnet-4-6"),
        (
            "groq/llama-3.3-70b-versatile",
            "groq",
            "llama-3.3-70b-versatile",
        ),
        ("openai/gpt-4o", "openai", "gpt-4o"),
        ("ollama/qwen3:8b", "ollama", "qwen3:8b"),
    ] {
        let runtime = FakeRuntime::with(&[]);
        let (result, calls) = dispatch(
            AgentLlmPolicy::Unrestricted,
            Some(route),
            loopback(),
            &runtime,
            Some("answer"),
        );
        assert_eq!(result.unwrap(), "answer", "{route}");
        assert_eq!(calls.len(), 1, "{route}");
        assert_eq!(
            (calls[0].0.as_str(), calls[0].2.as_str()),
            (provider, model)
        );
        assert!(
            runtime.asked.borrow().is_empty(),
            "{route}: an unrestricted agent is not held to the local checks"
        );
    }
}

#[test]
fn p3_d2_ollama_cloud_names_and_remote_replies_are_recognised() {
    for name in [
        "gpt-oss:120b-cloud",
        "qwen3-coder:480b-cloud",
        "glm-4.6:cloud",
        "library/cloud:latest",
        "CLOUD-model:7b",
    ] {
        assert!(ollama_model_name_is_cloud(name), "{name}");
    }
    for name in ["qwen3:8b", "llama3.2:3b", "cloudy:7b", "mycloudmodel:1b"] {
        assert!(!ollama_model_name_is_cloud(name), "{name}");
    }
    for reply in [
        serde_json::json!({"remote_host": "https://ollama.com:443"}),
        serde_json::json!({"remote_model": "gpt-oss:120b"}),
        serde_json::json!({"remote_host": {"url": "x"}}),
    ] {
        assert!(ollama_show_names_remote(&reply), "{reply}");
    }
    for reply in [
        serde_json::json!({"modelfile": "FROM x", "details": {}}),
        serde_json::json!({"remote_host": "", "remote_model": null}),
    ] {
        assert!(!ollama_show_names_remote(&reply), "{reply}");
    }
}

// ── End to end, in a child process ───────────────────────────────────────

/// What the fake Ollama runtime serves.
#[derive(Clone)]
struct FakeOllama {
    models: Vec<&'static str>,
    remote: Vec<&'static str>,
    generate_status: u16,
    /// P3 HiveMind INFOFLOW: the recording proxy also answers a plain HTTP
    /// request (one that would have left the machine) as this runtime
    /// would, so work on a configured route runs on and every one of its
    /// prompts is recorded with its content.
    proxy_answers: bool,
    /// P3 HiveMind INFOFLOW: a policy-tightening event at the proxy.
    tighten: Option<Tighten>,
    /// P3-C11-HM-M3 N6: the configuration made unreadable at the proxy
    /// when it receives the `nth` matching request (its `name` unused).
    break_config: Option<Tighten>,
    /// P3-C11-HM-M3 N6 (A1.6): after this many generations the runtime
    /// reports every model as remote.
    remote_after_generates: Option<usize>,
}

impl FakeOllama {
    fn serving(models: &[&'static str]) -> Self {
        Self {
            models: models.to_vec(),
            remote: Vec::new(),
            generate_status: 200,
            proxy_answers: false,
            tighten: None,
            break_config: None,
            remote_after_generates: None,
        }
    }
}

/// A policy-tightening event (P3 HiveMind INFOFLOW): when the recording
/// proxy receives the `nth` request (from 1) whose body `matches`, it first
/// makes the agent named `name` local-only in the scenario's configuration,
/// as an owner would, then answers it.
#[derive(Clone, Copy)]
struct Tighten {
    name: &'static str,
    matches: fn(&str) -> bool,
    nth: usize,
}

/// What a child scenario did: its result line, the requests the fake
/// Ollama runtime received (`METHOD path body`), and the connections the
/// recording proxy received (their first line).
struct Witness {
    result: String,
    ollama: Vec<String>,
    proxy: Vec<String>,
}

impl Witness {
    fn generates(&self) -> Vec<&String> {
        self.ollama
            .iter()
            .filter(|r| r.starts_with("POST /api/generate"))
            .collect()
    }

    fn assert_refused_without_remote(&self, context: &str) {
        assert!(
            self.result.starts_with("ERR ") && self.result.contains("local-only"),
            "{context}: {}",
            self.result
        );
        assert!(
            self.proxy.is_empty(),
            "{context}: remote invocation {:?}",
            self.proxy
        );
        assert!(
            self.generates().is_empty(),
            "{context}: prompt sent {:?}",
            self.ollama
        );
    }
}

fn read_request(stream: &TcpStream) -> Option<(String, String)> {
    let mut reader = BufReader::new(stream);
    let mut head = String::new();
    if reader.read_line(&mut head).ok()? == 0 {
        return None;
    }
    if head == "\r\n" {
        return Some((String::new(), String::new()));
    }
    let body = read_rest(&mut reader)?;
    Some((head.trim_end().to_string(), body))
}

/// The rest of a request whose first line `reader` has read: its headers,
/// then the body its `content-length` gives.
fn read_rest(reader: &mut BufReader<&TcpStream>) -> Option<String> {
    let mut length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        if line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().ok()?;
            }
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(String::from_utf8_lossy(&body).into_owned())
}

fn serve_ollama(listener: TcpListener, serves: FakeOllama, log: mpsc::Sender<String>) {
    let mut generated = 0usize;
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        // A health probe connects and closes without a request.
        let Some((head, body)) = read_request(&stream) else {
            continue;
        };
        let path = head.split(' ').nth(1).unwrap_or_default().to_string();
        let method = head.split(' ').next().unwrap_or_default().to_string();
        if path == "/stop" {
            return;
        }
        let _ = log.send(format!("{method} {path} {body}"));
        if path == "/api/show"
            && serves
                .remote_after_generates
                .is_some_and(|n| generated >= n)
        {
            let model = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("model").and_then(|m| m.as_str()).map(str::to_string))
                .unwrap_or_default();
            let remote =
                serde_json::json!({"remote_host": "https://ollama.com:443", "remote_model": model});
            respond(&mut stream, 200, &remote);
            continue;
        }
        let (status, reply) = runtime_reply(&serves, &path, &body);
        if path == "/api/generate" {
            generated += 1;
        }
        respond(&mut stream, status, &reply);
    }
}
/// The fake runtime's answer to a request for `path` with `body`.
fn runtime_reply(serves: &FakeOllama, path: &str, body: &str) -> (u16, serde_json::Value) {
    let model = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("model").and_then(|m| m.as_str()).map(str::to_string))
        .unwrap_or_default();
    match path {
        "/api/tags" => (
            200,
            serde_json::json!({"models": serves.models.iter()
                .map(|name| serde_json::json!({"name": name}))
                .collect::<Vec<_>>()}),
        ),
        "/api/show" if !serves.models.contains(&model.as_str()) => {
            (404, serde_json::json!({"error": "model not found"}))
        }
        "/api/show" if serves.remote.contains(&model.as_str()) => (
            200,
            serde_json::json!({"remote_host": "https://ollama.com:443", "remote_model": model}),
        ),
        "/api/show" => (
            200,
            serde_json::json!({"modelfile": "FROM local", "details": {}}),
        ),
        // P3-C11-HM-M3 N6: the plan of a goal naming one of the N6 goals.
        "/api/generate" if serves.generate_status == 200 && hm_plan_for(body).is_some() => (
            200,
            serde_json::json!({"response": hm_plan_for(body), "eval_count": 2}),
        ),
        // P3 HiveMind: a member loop's plan, one model step.
        "/api/generate"
            if serves.generate_status == 200
                && (body.contains("Output ONLY a JSON array")
                    || body.contains("ONLY a valid JSON array")) =>
        {
            (
                200,
                serde_json::json!({"response": MEMBER_PLAN, "eval_count": 2}),
            )
        }
        // P3 HiveMind INFOFLOW: the decomposition of a goal carrying
        // restricted information carries it into each sub-task.
        "/api/generate"
            if serves.generate_status == 200
                && body.contains("task decomposition engine")
                && body.contains(RESTRICTED) =>
        {
            (
                200,
                serde_json::json!({"response": RESTRICTED_PLAN, "eval_count": 2}),
            )
        }
        // P3 D-2 R2: a team decomposition, two sub-tasks any member does.
        "/api/generate"
            if serves.generate_status == 200 && body.contains("task decomposition engine") =>
        {
            (
                200,
                serde_json::json!({"response": TEAM_PLAN, "eval_count": 2}),
            )
        }
        "/api/generate" if serves.generate_status == 200 => (
            200,
            serde_json::json!({"response": "local answer", "eval_count": 2}),
        ),
        "/api/generate" => (
            serves.generate_status,
            serde_json::json!({"error": "failed"}),
        ),
        _ => (404, serde_json::json!({"error": "not found"})),
    }
}

fn respond(stream: &mut TcpStream, status: u16, reply: &serde_json::Value) {
    let body = reply.to_string();
    let _ = write!(
        stream,
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
}

fn serve_proxy(
    listener: TcpListener,
    serves: FakeOllama,
    config: std::path::PathBuf,
    log: mpsc::Sender<String>,
) {
    let mut matched = 0usize;
    let mut broke = 0usize;
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
        let mut reader = BufReader::new(&stream);
        let mut first = String::new();
        let _ = reader.read_line(&mut first);
        if first.starts_with("GET /stop") {
            return;
        }
        let first = first.trim_end().to_string();
        let plain = !first.is_empty() && !first.starts_with("CONNECT ");
        // P3 HiveMind INFOFLOW: a plain HTTP request is read whole, so the
        // prompt it carries is recorded.
        let body = if plain {
            read_rest(&mut reader).unwrap_or_default()
        } else {
            String::new()
        };
        drop(reader);
        // Every connection is a request that would have left the machine.
        let _ = log.send(if body.is_empty() {
            first.clone()
        } else {
            format!("{first} {body}")
        });
        if let Some(tighten) = serves.tighten {
            if (tighten.matches)(&body) {
                matched += 1;
                if matched == tighten.nth {
                    make_local_only(&config, tighten.name);
                }
            }
        }
        if let Some(breaks) = serves.break_config {
            if (breaks.matches)(&body) {
                broke += 1;
                if broke == breaks.nth {
                    std::fs::write(&config, "local_only = [").unwrap();
                }
            }
        }
        if serves.proxy_answers && plain {
            // The request names its absolute URL: `http://host:port/path`.
            let path = first
                .split(' ')
                .nth(1)
                .and_then(|target| target.splitn(4, '/').nth(3))
                .map(|path| format!("/{path}"))
                .unwrap_or_default();
            let (status, reply) = runtime_reply(&serves, &path, &body);
            respond(&mut stream, status, &reply);
        }
    }
}

/// The owner makes the agent named `name` local-only (P3 HiveMind
/// INFOFLOW): an assignment under its name, in the scenario's
/// configuration.
fn make_local_only(config: &std::path::Path, name: &str) {
    let mut settings = nexus_kernel::config::load_config_from_path(config).unwrap();
    settings.agent_llm_assignments.insert(
        name.to_string(),
        nexus_kernel::config::AgentLlmAssignment {
            provider_id: String::new(),
            local_only: true,
            budget_dollars: 0,
            budget_tokens: 0,
        },
    );
    nexus_kernel::config::save_config_to_path(config, &settings).unwrap();
}

fn stop(addr: std::net::SocketAddr) {
    if let Ok(mut stream) = TcpStream::connect(addr) {
        let _ = write!(stream, "GET /stop HTTP/1.1\r\n\r\n");
    }
}

/// Runs `scenario` in a child process wired to a fake Ollama runtime and a
/// recording proxy, and returns what it did. In the child it runs the
/// scenario and returns `None`.
fn e2e(
    name: &str,
    serves: FakeOllama,
    scenario: impl FnOnce() -> Result<String, AgentError>,
) -> Option<Witness> {
    let full_name = format!("{}::{name}", module_path!().split_once("::").unwrap().1);
    if std::env::var("NEXUS_P3_D2_CHILD").as_deref() == Ok(full_name.as_str()) {
        let line = match scenario() {
            Ok(text) => format!("OK {text}"),
            Err(error) => format!("ERR {error}"),
        };
        eprintln!("\nP3-D2-RESULT {line}");
        return None;
    }
    let ollama = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
    let (ollama_addr, proxy_addr) = (ollama.local_addr().unwrap(), proxy.local_addr().unwrap());
    let (ollama_log, ollama_seen) = mpsc::channel();
    let (proxy_log, proxy_seen) = mpsc::channel();
    let config = std::env::temp_dir().join(format!("p3-d2-{}.toml", Uuid::new_v4()));
    let proxy_serves = serves.clone();
    let proxy_config = config.clone();
    let ollama_thread = std::thread::spawn(move || serve_ollama(ollama, serves, ollama_log));
    let proxy_thread =
        std::thread::spawn(move || serve_proxy(proxy, proxy_serves, proxy_config, proxy_log));

    let mut child = Command::new(std::env::current_exe().unwrap());
    child
        .args(["--exact", &full_name, "--nocapture", "--test-threads=1"])
        .env("NEXUS_P3_D2_CHILD", &full_name)
        .env("NEXUS_ORACLE_EPHEMERAL", "1")
        .env("NEXUS_DB_PATH", ":memory:")
        .env("NEXUS_CONFIG_PATH", &config)
        .env("OLLAMA_URL", format!("http://{ollama_addr}"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for key in [
        "LLM_PROVIDER",
        "CLAUDE_CODE_ENABLED",
        "CODEX_CLI_ENABLED",
        "FLASH_MODEL_PATH",
    ] {
        child.env_remove(key);
    }
    for key in [
        "DEEPSEEK_API_KEY",
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GEMINI_API_KEY",
        "GROQ_API_KEY",
        "MISTRAL_API_KEY",
        "TOGETHER_API_KEY",
        "FIREWORKS_API_KEY",
        "PERPLEXITY_API_KEY",
        "COHERE_API_KEY",
        "OPENROUTER_API_KEY",
        "NVIDIA_NIM_API_KEY",
    ] {
        child.env(key, FAKE_KEY);
    }
    let proxy_url = format!("http://{proxy_addr}");
    for key in [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ] {
        child.env(key, &proxy_url);
    }
    // P3 D-2 R2: no proxy exemption is configured. A local-only request must
    // reach the loopback runtime directly whatever the proxy settings say.
    for key in ["NO_PROXY", "no_proxy"] {
        child.env(key, "");
    }
    let mut child = child.spawn().unwrap();
    let (done, finished) = mpsc::channel();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let err = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        let _ = done.send(());
        text
    });
    if finished.recv_timeout(Duration::from_secs(90)).is_err() {
        let _ = child.kill();
    }
    let status = child.wait().unwrap();
    let (stdout, stderr) = (out.join().unwrap(), err.join().unwrap());
    stop(ollama_addr);
    stop(proxy_addr);
    ollama_thread.join().unwrap();
    proxy_thread.join().unwrap();
    assert!(status.success(), "{name}: {status}\n{stdout}\n{stderr}");
    let results: Vec<&str> = stderr
        .lines()
        .filter_map(|line| line.strip_prefix("P3-D2-RESULT "))
        .collect();
    assert_eq!(results.len(), 1, "{name}: one result\n{stdout}\n{stderr}");
    let witness = Witness {
        result: results[0].to_string(),
        ollama: ollama_seen.try_iter().collect(),
        proxy: proxy_seen.try_iter().collect(),
    };
    let requests: Vec<String> = witness
        .ollama
        .iter()
        .map(|r| r.chars().take(90).collect())
        .collect();
    eprintln!(
        "RECEIPT {name} result={} proxy={:?} runtime={requests:?}",
        witness.result.chars().take(300).collect::<String>(),
        witness.proxy
    );
    Some(witness)
}

/// How a child scenario sets up its agent.
#[derive(Default)]
struct Setup {
    assignment: Option<(&'static str, bool)>,
    manifest_model: Option<&'static str>,
    goal_override: Option<&'static str>,
    unregistered: bool,
    unreadable_config: bool,
}

fn run(setup: Setup) -> Result<String, AgentError> {
    let state = AppState::new_in_memory();
    let id = if setup.unregistered {
        Uuid::new_v4().to_string()
    } else {
        let mut manifest = parse_manifest(
            r#"
name = "p3-d2-agent"
version = "1.0.0"
capabilities = ["llm.query"]
fuel_budget = 10000
autonomy_level = 3
"#,
        )
        .unwrap();
        manifest.llm_model = setup.manifest_model.map(str::to_string);
        state
            .supervisor
            .lock()
            .unwrap()
            .start_agent(manifest)
            .unwrap()
            .to_string()
    };
    if let Some((provider_id, local_only)) = setup.assignment {
        let mut config = nexus_kernel::config::load_config().unwrap();
        config.agent_llm_assignments.insert(
            id.clone(),
            nexus_kernel::config::AgentLlmAssignment {
                provider_id: provider_id.into(),
                local_only,
                budget_dollars: 0,
                budget_tokens: 0,
            },
        );
        nexus_kernel::config::save_config(&config).unwrap();
        if !local_only {
            // AMENDMENT 1 A1.9: an unrestricted agent is an Owner loosening.
            crate::hm_wiring::test_support::owner_loosen(&state, &id);
        }
    }
    if setup.unreadable_config {
        std::fs::write(
            std::env::var("NEXUS_CONFIG_PATH").unwrap(),
            "local_only = [",
        )
        .unwrap();
    }
    if let Some(model) = setup.goal_override {
        let mut goal = AgentGoal::new("P3 D-2".into(), 5);
        goal.model_override = Some(model.into());
        state.cognitive_runtime.assign_goal(&id, goal).unwrap();
    }
    crate::hm_wiring::test_support::plan_text(&state, &id, PROMPT)
}

#[test]
fn p3_d2_e2e_goal_override_selecting_cloud_is_refused() {
    let Some(w) = e2e(
        "p3_d2_e2e_goal_override_selecting_cloud_is_refused",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            run(Setup {
                assignment: Some(("", true)),
                goal_override: Some("anthropic/claude-sonnet-4-6"),
                ..Setup::default()
            })
        },
    ) else {
        return;
    };
    w.assert_refused_without_remote("goal override");
}

#[test]
fn p3_d2_e2e_auto_routing_with_cloud_credentials_and_no_local_model_is_refused() {
    let Some(w) = e2e(
        "p3_d2_e2e_auto_routing_with_cloud_credentials_and_no_local_model_is_refused",
        FakeOllama::serving(&[]),
        || {
            run(Setup {
                assignment: Some(("auto", true)),
                ..Setup::default()
            })
        },
    ) else {
        return;
    };
    w.assert_refused_without_remote("auto routing");
}

#[test]
fn p3_d2_e2e_manifest_and_assignment_selecting_cloud_are_refused() {
    let Some(w) = e2e(
        "p3_d2_e2e_manifest_and_assignment_selecting_cloud_are_refused",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            let manifest = run(Setup {
                assignment: Some(("", true)),
                manifest_model: Some("groq/llama-3.3-70b-versatile"),
                ..Setup::default()
            });
            let assignment = run(Setup {
                assignment: Some(("claude", true)),
                ..Setup::default()
            });
            match (manifest, assignment) {
                (Err(a), Err(b)) => Err(AgentError::SupervisorError(format!("{a} | {b}"))),
                other => Ok(format!("not refused: {other:?}")),
            }
        },
    ) else {
        return;
    };
    w.assert_refused_without_remote("manifest and assignment");
    assert_eq!(w.result.matches("local-only").count(), 2, "{}", w.result);
}

#[test]
fn p3_d2_e2e_relayed_ollama_model_is_refused() {
    let mut serves = FakeOllama::serving(&["qwen3:8b"]);
    serves.remote = vec!["qwen3:8b"];
    let Some(w) = e2e("p3_d2_e2e_relayed_ollama_model_is_refused", serves, || {
        run(Setup {
            assignment: Some(("ollama", true)),
            ..Setup::default()
        })
    }) else {
        return;
    };
    w.assert_refused_without_remote("relayed model");
    assert!(
        w.ollama.iter().any(|r| r.starts_with("POST /api/show")),
        "the model was checked: {:?}",
        w.ollama
    );
}

#[test]
fn p3_d2_e2e_unregistered_agent_or_unreadable_config_dispatches_nothing() {
    let Some(w) = e2e(
        "p3_d2_e2e_unregistered_agent_or_unreadable_config_dispatches_nothing",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            let unregistered = run(Setup {
                unregistered: true,
                ..Setup::default()
            });
            let unreadable = run(Setup {
                unreadable_config: true,
                ..Setup::default()
            });
            match (unregistered, unreadable) {
                (Err(a), Err(b)) => Err(AgentError::SupervisorError(format!("{a} | {b}"))),
                other => Ok(format!("not refused: {other:?}")),
            }
        },
    ) else {
        return;
    };
    w.assert_refused_without_remote("unestablished policy");
    assert_eq!(
        w.result
            .matches("routing policy could not be established")
            .count(),
        2,
        "{}",
        w.result
    );
}

#[test]
fn p3_d2_e2e_local_failure_after_selection_is_not_a_cloud_fallback() {
    let mut serves = FakeOllama::serving(&["qwen3:8b"]);
    serves.generate_status = 500;
    let Some(w) = e2e(
        "p3_d2_e2e_local_failure_after_selection_is_not_a_cloud_fallback",
        serves,
        || {
            run(Setup {
                assignment: Some(("ollama/qwen3:8b", true)),
                ..Setup::default()
            })
        },
    ) else {
        return;
    };
    assert!(w.result.starts_with("ERR "), "{}", w.result);
    assert!(w.proxy.is_empty(), "remote invocation {:?}", w.proxy);
    assert_eq!(w.generates().len(), 1, "one local attempt: {:?}", w.ollama);
}

#[test]
fn p3_d2_e2e_permitted_local_request_works() {
    let Some(w) = e2e(
        "p3_d2_e2e_permitted_local_request_works",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            run(Setup {
                assignment: Some(("ollama", true)),
                ..Setup::default()
            })
        },
    ) else {
        return;
    };
    assert_eq!(w.result, "OK local answer");
    assert!(w.proxy.is_empty(), "remote invocation {:?}", w.proxy);
    let generates = w.generates();
    assert_eq!(generates.len(), 1, "{:?}", w.ollama);
    assert!(generates[0].contains(PROMPT) && generates[0].contains("qwen3:8b"));
}

/// The witness's own control: an unrestricted agent's explicit cloud route
/// is dispatched, and the proxy sees it.
#[test]
fn p3_d2_e2e_unrestricted_cloud_route_reaches_the_witness() {
    let Some(w) = e2e(
        "p3_d2_e2e_unrestricted_cloud_route_reaches_the_witness",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            run(Setup {
                assignment: Some(("anthropic/claude-sonnet-4-6", false)),
                ..Setup::default()
            })
        },
    ) else {
        return;
    };
    assert!(w.result.starts_with("ERR "), "{}", w.result);
    assert!(
        w.proxy
            .iter()
            .any(|line| line.contains("api.anthropic.com")),
        "the cloud dispatch reached the witness: {:?}",
        w.proxy
    );
    assert!(w.generates().is_empty(), "{:?}", w.ollama);
}

// ── P3 D-2 R2: a chat routed to an agent by its name ─────────────────────

/// Registers `name` with the supervisor only (no desktop metadata entry)
/// and keys its assignment by its UUID, as the agents page does.
fn register_by_supervisor(state: &AppState, name: &str, local_only: bool) -> String {
    let manifest = parse_manifest(&format!(
        r#"
name = "{name}"
version = "1.0.0"
capabilities = ["llm.query"]
fuel_budget = 10000
autonomy_level = 3
"#
    ))
    .unwrap();
    let id = state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(manifest)
        .unwrap()
        .to_string();
    let mut config = nexus_kernel::config::load_config().unwrap();
    config.agent_llm_assignments.insert(
        id.clone(),
        nexus_kernel::config::AgentLlmAssignment {
            provider_id: String::new(),
            local_only,
            budget_dollars: 0,
            budget_tokens: 0,
        },
    );
    nexus_kernel::config::save_config(&config).unwrap();
    if !local_only {
        // AMENDMENT 1 A1.9: an unrestricted agent is an Owner loosening.
        crate::hm_wiring::test_support::owner_loosen(state, &id);
    }
    id
}

/// The desktop chat, routed to `agent` by name (no model chosen, no
/// auto-routing).
fn chat(state: &AppState, agent: Option<&str>) -> Result<String, AgentError> {
    crate::commands::chat_llm::send_chat(
        state,
        PROMPT.to_string(),
        None,
        agent.map(str::to_string),
        Some(false),
    )
    .map(|response| response.text)
    .map_err(AgentError::SupervisorError)
}

impl Witness {
    fn assert_answered_locally(&self, context: &str) {
        assert!(
            self.result.starts_with("OK ") && self.result.contains("local answer"),
            "{context}: {}",
            self.result
        );
        assert!(
            self.proxy.is_empty(),
            "{context}: remote invocation {:?}",
            self.proxy
        );
        assert!(
            self.generates().iter().any(|r| r.contains(PROMPT)),
            "{context}: {:?}",
            self.ollama
        );
    }
}

/// The reviewed counterexample: a supervisor-registered agent whose
/// UUID-keyed assignment is local-only, absent from the desktop metadata,
/// chatted to by its registered name. The chat is local-only.
#[test]
fn p3_d2r2_e2e_chat_by_registered_name_follows_the_uuid_assignment() {
    let Some(w) = e2e(
        "p3_d2r2_e2e_chat_by_registered_name_follows_the_uuid_assignment",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            let state = AppState::new_in_memory();
            register_by_supervisor(&state, "p3-d2r2-named", true);
            assert!(state.meta.lock().unwrap().is_empty());
            chat(&state, Some("p3-d2r2-named"))
        },
    ) else {
        return;
    };
    w.assert_answered_locally("supervisor-registered local-only agent");
}

/// Two registered agents share a name (after normalization); one is
/// local-only. A chat by that name is local-only.
#[test]
fn p3_d2r2_e2e_a_shared_name_is_local_only_when_any_agent_is() {
    let Some(w) = e2e(
        "p3_d2r2_e2e_a_shared_name_is_local_only_when_any_agent_is",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            let state = AppState::new_in_memory();
            register_by_supervisor(&state, "p3-d2r2-twin", false);
            register_by_supervisor(&state, "P3-D2R2-Twin", true);
            chat(&state, Some("p3-d2r2-twin"))
        },
    ) else {
        return;
    };
    w.assert_answered_locally("shared name");
}

/// A name no registered agent has is not an identity: the agent-scoped chat
/// is refused and nothing is dispatched.
#[test]
fn p3_d2r2_e2e_chat_to_an_unregistered_agent_name_is_refused() {
    let Some(w) = e2e(
        "p3_d2r2_e2e_chat_to_an_unregistered_agent_name_is_refused",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            let state = AppState::new_in_memory();
            register_by_supervisor(&state, "p3-d2r2-named", true);
            chat(&state, Some("p3-d2r2-nobody"))
        },
    ) else {
        return;
    };
    w.assert_refused_without_remote("unregistered name");
    assert!(
        w.result.contains("routing policy could not be established"),
        "{}",
        w.result
    );
}

/// The intentional unrestricted behaviour is kept, and is the witness's
/// control: a chat with no agent goes to the configured cloud provider.
#[test]
fn p3_d2r2_e2e_non_agent_chat_keeps_the_configured_provider() {
    let Some(w) = e2e(
        "p3_d2r2_e2e_non_agent_chat_keeps_the_configured_provider",
        FakeOllama::serving(&["qwen3:8b"]),
        || chat(&AppState::new_in_memory(), None),
    ) else {
        return;
    };
    assert!(
        w.proxy.iter().any(|line| line.starts_with("CONNECT ")),
        "the cloud dispatch reached the witness: {:?} {}",
        w.proxy,
        w.result
    );
    assert!(w.generates().is_empty(), "{:?}", w.ollama);
}

/// A registered unrestricted agent chatted to by name keeps the configured
/// cloud provider.
#[test]
fn p3_d2r2_e2e_unrestricted_named_agent_keeps_the_configured_provider() {
    let Some(w) = e2e(
        "p3_d2r2_e2e_unrestricted_named_agent_keeps_the_configured_provider",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            let state = AppState::new_in_memory();
            register_by_supervisor(&state, "p3-d2r2-open", false);
            chat(&state, Some("p3-d2r2-open"))
        },
    ) else {
        return;
    };
    assert!(
        w.proxy.iter().any(|line| line.starts_with("CONNECT ")),
        "the cloud dispatch reached the witness: {:?} {}",
        w.proxy,
        w.result
    );
    assert!(w.generates().is_empty(), "{:?}", w.ollama);
}

// ── P3 D-2 R2: team workflows ─────────────────────────────────────────────

const TEAM_PLAN: &str = r#"[
  {"id": "t1", "description": "Summarize the synthetic note", "required_capabilities": ["llm.query"], "estimated_fuel": 1.0, "dependencies": []},
  {"id": "t2", "description": "List the synthetic note's points", "required_capabilities": ["llm.query"], "estimated_fuel": 1.0, "dependencies": []}
]"#;

/// Runs a team workflow of a director and two members, registered with the
/// given local-only settings, through the desktop's team command path.
fn team(director: bool, members: [bool; 2]) -> Result<String, AgentError> {
    let state = AppState::new_in_memory();
    let director_id = register_by_supervisor(&state, "p3-d2r2-director", director);
    let member_ids = vec![
        register_by_supervisor(&state, "p3-d2r2-member-a", members[0]),
        register_by_supervisor(&state, "p3-d2r2-member-b", members[1]),
    ];
    crate::commands::orchestration::run_team_workflow(&state, &director_id, PROMPT, &member_ids)
        .map(|result| result.to_string())
        .map_err(AgentError::SupervisorError)
}

impl Witness {
    /// Every model call of a team workflow went to the local runtime: the
    /// decomposition, both member tasks and the merge, and nothing else.
    fn assert_team_ran_locally(&self, context: &str) {
        assert!(self.result.starts_with("OK "), "{context}: {}", self.result);
        let result: serde_json::Value =
            serde_json::from_str(self.result.trim_start_matches("OK ")).unwrap();
        assert_eq!(result["success"], true, "{context}: {result}");
        assert!(
            self.proxy.is_empty(),
            "{context}: remote invocation {:?}",
            self.proxy
        );
        let generates = self.generates();
        let count = |marker: &str| generates.iter().filter(|r| r.contains(marker)).count();
        assert_eq!(
            count("task decomposition engine"),
            1,
            "{context}: {generates:?}"
        );
        assert_eq!(
            count("Execute this task assigned by the Director"),
            2,
            "{context}: {generates:?}"
        );
        assert_eq!(
            count("Combine these sub-task results"),
            1,
            "{context}: {generates:?}"
        );
        assert_eq!(generates.len(), 4, "{context}: {generates:?}");
    }
}

/// A local-only member: the decomposition, every member's task and the
/// merge of their results stay on the local runtime.
#[test]
fn p3_d2r2_e2e_team_with_a_local_only_member_stays_local() {
    let Some(w) = e2e(
        "p3_d2r2_e2e_team_with_a_local_only_member_stays_local",
        FakeOllama::serving(&["qwen3:8b"]),
        || team(false, [true, false]),
    ) else {
        return;
    };
    w.assert_team_ran_locally("local-only member");
}

/// A local-only director: its decomposition, and so the whole workflow,
/// stays on the local runtime.
#[test]
fn p3_d2r2_e2e_team_with_a_local_only_director_stays_local() {
    let Some(w) = e2e(
        "p3_d2r2_e2e_team_with_a_local_only_director_stays_local",
        FakeOllama::serving(&["qwen3:8b"]),
        || team(true, [false, false]),
    ) else {
        return;
    };
    w.assert_team_ran_locally("local-only director");
}

/// The local runtime fails every call of a local-only team: the workflow
/// fails, nothing reaches the cloud, and nothing is reported successful.
#[test]
fn p3_d2r2_e2e_local_only_team_failure_is_not_success_or_cloud_fallback() {
    let mut serves = FakeOllama::serving(&["qwen3:8b"]);
    serves.generate_status = 500;
    let Some(w) = e2e(
        "p3_d2r2_e2e_local_only_team_failure_is_not_success_or_cloud_fallback",
        serves,
        || team(false, [true, false]),
    ) else {
        return;
    };
    assert!(w.result.starts_with("ERR "), "{}", w.result);
    assert!(w.proxy.is_empty(), "remote invocation {:?}", w.proxy);
    assert_eq!(w.generates().len(), 1, "one local attempt: {:?}", w.ollama);
}

/// A director or member that is not a registered agent, or a configuration
/// that cannot be read, refuses the workflow before any model call.
#[test]
fn p3_d2r2_e2e_team_without_established_identities_dispatches_nothing() {
    let Some(w) = e2e(
        "p3_d2r2_e2e_team_without_established_identities_dispatches_nothing",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            let state = AppState::new_in_memory();
            let known = register_by_supervisor(&state, "p3-d2r2-known", false);
            let stranger = Uuid::new_v4().to_string();
            let run = |director: &str, members: Vec<String>| {
                crate::commands::orchestration::run_team_workflow(
                    &state, director, PROMPT, &members,
                )
            };
            let unregistered_member = run(&known, vec![known.clone(), stranger.clone()]);
            let unregistered_director = run(&stranger, vec![known.clone()]);
            std::fs::write(
                std::env::var("NEXUS_CONFIG_PATH").unwrap(),
                "local_only = [",
            )
            .unwrap();
            let unreadable = run(&known, vec![known.clone()]);
            match (unregistered_member, unregistered_director, unreadable) {
                (Err(a), Err(b), Err(c)) => {
                    Err(AgentError::SupervisorError(format!("{a} | {b} | {c}")))
                }
                other => Ok(format!("not refused: {other:?}")),
            }
        },
    ) else {
        return;
    };
    assert!(w.result.starts_with("ERR "), "{}", w.result);
    assert_eq!(
        w.result.matches("not a registered agent").count(),
        2,
        "{}",
        w.result
    );
    assert!(
        w.result.contains("routing policy could not be established"),
        "{}",
        w.result
    );
    assert!(w.proxy.is_empty(), "remote invocation {:?}", w.proxy);
    assert!(w.ollama.is_empty(), "{:?}", w.ollama);
}

/// The witness's control, and the unrestricted behaviour kept: a team with
/// no local-only participant uses the configured cloud provider.
#[test]
fn p3_d2r2_e2e_unrestricted_team_keeps_the_configured_provider() {
    let Some(w) = e2e(
        "p3_d2r2_e2e_unrestricted_team_keeps_the_configured_provider",
        FakeOllama::serving(&["qwen3:8b"]),
        || team(false, [false, false]),
    ) else {
        return;
    };
    assert!(
        w.proxy.iter().any(|line| line.starts_with("CONNECT ")),
        "the cloud dispatch reached the witness: {:?} {}",
        w.proxy,
        w.result
    );
    assert!(w.generates().is_empty(), "{:?}", w.ollama);
}

// ── P3 HiveMind: sessions run under their participants' policy ───────────

const MEMBER_PLAN: &str = r#"[{"action": {"type": "LlmQuery", "prompt": "P3 HiveMind synthetic step", "context": []}, "description": "Answer the sub-task"}]"#;

/// A HiveMind participant: its name, whether it is local-only, and the
/// model its assignment names ("" for none).
type Participant = (&'static str, bool, &'static str);

/// Registers `name` with the supervisor (autonomy 3, `llm.query`) and an
/// assignment keyed by its UUID.
fn register_participant(state: &AppState, (name, local_only, model): Participant) -> String {
    let id = register_by_supervisor(state, name, local_only);
    if !model.is_empty() {
        let mut config = nexus_kernel::config::load_config().unwrap();
        config
            .agent_llm_assignments
            .get_mut(&id)
            .unwrap()
            .provider_id = model.to_string();
        nexus_kernel::config::save_config(&config).unwrap();
    }
    id
}

/// Runs one HiveMind session through the desktop's own path (admission,
/// the real coordinator with `GatewayPlannerLlm::for_session`, real
/// sub-task loops).
fn hive_session(state: &AppState, agent_ids: Vec<String>) -> Result<String, String> {
    hive_session_on(state, PROMPT, agent_ids)
}

/// [`hive_session`] for `goal`.
fn hive_session_on(state: &AppState, goal: &str, agent_ids: Vec<String>) -> Result<String, String> {
    let session = admit_hivemind(state)?;
    start_hivemind(state, &session, goal.to_string(), agent_ids).map(|o| o.to_string())
}

fn hive(participants: &[Participant]) -> Result<String, AgentError> {
    let state = AppState::new_in_memory();
    let ids = participants
        .iter()
        .map(|p| register_participant(&state, *p))
        .collect();
    hive_session(&state, ids).map_err(AgentError::SupervisorError)
}

impl Witness {
    /// Every model call of a session went to the local runtime, none to the
    /// proxy: the decomposition, a plan and a step per sub-task, the merge.
    fn assert_hive_ran_locally(&self, context: &str) {
        assert!(self.result.starts_with("OK "), "{context}: {}", self.result);
        let outcome: serde_json::Value =
            serde_json::from_str(self.result.trim_start_matches("OK ")).unwrap();
        assert_eq!(outcome["status"], "Completed", "{context}: {outcome}");
        assert!(
            self.proxy.is_empty(),
            "{context}: remote invocation {:?}",
            self.proxy
        );
        let generates = self.generates();
        let count = |marker: &str| generates.iter().filter(|r| r.contains(marker)).count();
        assert_eq!(
            count("task decomposition engine"),
            1,
            "{context}: {generates:?}"
        );
        assert_eq!(
            count("Combine these sub-task results"),
            1,
            "{context}: {generates:?}"
        );
        for task in ["Summarize the synthetic note", "List the synthetic note"] {
            assert!(
                generates
                    .iter()
                    .any(|r| r.contains("Output ONLY a JSON array") && r.contains(task)),
                "{context}: no local plan for {task}: {generates:?}"
            );
        }
        assert!(
            count("P3 HiveMind synthetic step") >= 2,
            "{context}: {generates:?}"
        );
    }
}

/// A and B. A session that mixes local-only and unrestricted participants
/// is refused (P3 HiveMind INFOFLOW, provisional), whichever is listed
/// first: no model is asked anything, nothing reaches the proxy, and
/// nothing of the session is stored for any participant.
#[test]
fn p3_hive_e2e_a_mixed_session_is_refused_before_any_model_call() {
    let Some(w) = e2e(
        "p3_hive_e2e_a_mixed_session_is_refused_before_any_model_call",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            let state = AppState::new_in_memory();
            let open = register_participant(&state, ("p3-hive-open", false, ""));
            let local = register_participant(&state, ("p3-hive-local", true, ""));
            let before = stored_rows(&state, &[&open, &local]);
            let open_first = hive_session(&state, vec![open.clone(), local.clone()]);
            let local_first = hive_session(&state, vec![local.clone(), open.clone()]);
            let after = stored_rows(&state, &[&open, &local]);
            Ok(format!(
                "open_first={open_first:?} local_first={local_first:?} stored={before}->{after}"
            ))
        },
    ) else {
        return;
    };
    assert!(w.ollama.is_empty(), "a model was asked: {:?}", w.ollama);
    assert!(w.proxy.is_empty(), "remote invocation {:?}", w.proxy);
    let stored = w.result.rsplit("stored=").next().unwrap();
    let (before, after) = stored.split_once("->").unwrap();
    assert_eq!(before, after, "the refused sessions stored something");
    for order in ["open_first", "local_first"] {
        assert!(
            w.result.contains(&format!(
                "{order}=Err(\"hivemind session refused: the session mixes local-only and unrestricted participants"
            )),
            "{order}: {}",
            w.result
        );
    }
}

/// The rows the desktop's database holds for `agents` (tasks and memories)
/// and its HiveMind sessions, as one count.
fn stored_rows(state: &AppState, agents: &[&String]) -> usize {
    let mut rows = state.db.list_hivemind_sessions().unwrap().len();
    for agent in agents {
        rows += state.db.load_tasks_by_agent(agent, 1000).unwrap().len();
        rows += state.db.load_memories(agent, None, 1000).unwrap().len();
    }
    rows
}

/// A and B, admitted: a session whose participants are all local-only
/// runs, and its loops, the decomposition and the merge all stay on the
/// local runtime.
#[test]
fn p3_hive_e2e_an_all_local_session_stays_local() {
    let Some(w) = e2e(
        "p3_hive_e2e_an_all_local_session_stays_local",
        FakeOllama::serving(&["qwen3:8b"]),
        || hive(&[("p3-hive-local", true, ""), ("p3-hive-local-2", true, "")]),
    ) else {
        return;
    };
    w.assert_hive_ran_locally("all-local session");
}

/// The goal of the session P4-S1-H1 refuses (its decomposition would carry
/// it to the model).
const H1_REFUSED_GOAL: &str = "P4-S1-H1 synthetic goal of a stopped lead";

/// P4-S1-H1, on the desktop's own path (the session's adapter, real
/// sub-task loops): a session whose lead is stopped is refused before any
/// model call (nothing reaches the local runtime or the proxy, nothing is
/// stored), while a session of the same participants led by the running
/// one runs, planned and merged on the local runtime.
#[test]
fn p4_s1_h1_hive_e2e_a_stopped_lead_is_refused_before_any_model_call() {
    let Some(w) = e2e(
        "p4_s1_h1_hive_e2e_a_stopped_lead_is_refused_before_any_model_call",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            let state = AppState::new_in_memory();
            let lead = register_participant(&state, ("p4-h1-lead", true, ""));
            let member = register_participant(&state, ("p4-h1-member", true, ""));
            state
                .supervisor
                .lock()
                .unwrap()
                .stop_agent(Uuid::parse_str(&lead).unwrap())
                .unwrap();
            let before = stored_rows(&state, &[&lead, &member]);
            let refused =
                hive_session_on(&state, H1_REFUSED_GOAL, vec![lead.clone(), member.clone()]);
            let after = stored_rows(&state, &[&lead, &member]);
            let led_by_member = match hive_session(&state, vec![member.clone(), lead.clone()]) {
                Ok(outcome) => {
                    let outcome: serde_json::Value = serde_json::from_str(&outcome).unwrap();
                    outcome["status"].to_string()
                }
                Err(error) => format!("Err({error})"),
            };
            Ok(format!(
                "refused={refused:?} stored={before}->{after} led_by_member={led_by_member}"
            ))
        },
    ) else {
        return;
    };
    assert!(w.proxy.is_empty(), "remote invocation {:?}", w.proxy);
    for part in [
        "refused=Err(\"hivemind session refused: its lead agent '",
        "' is Stopped: it takes no new goal or task\")",
    ] {
        assert!(w.result.contains(part), "{part}: {}", w.result);
    }
    let stored = w.result.split("stored=").nth(1).unwrap();
    let (before, after) = stored.split_once(' ').unwrap().0.split_once("->").unwrap();
    assert_eq!(before, after, "the refused session stored something");
    let generates = w.generates();
    assert!(
        !generates.iter().any(|r| r.contains(H1_REFUSED_GOAL)),
        "a model was asked for the stopped lead's session: {generates:?}"
    );
    // The control: the same participants, led by the running one.
    assert!(
        w.result.contains("led_by_member=\"Completed\""),
        "{}",
        w.result
    );
    let count = |kind: fn(&str) -> bool| generates.iter().filter(|r| kind(r.as_str())).count();
    assert_eq!(count(is_decomposition), 1, "{generates:?}");
    assert_eq!(count(is_merge), 1, "{generates:?}");
}

/// C and D. The local-only participant assigned every sub-task has a cloud
/// model assigned: its loop is refused that model (no cloud call), and each
/// sub-task is reassigned to the other local-only participant, which runs
/// it locally. Nothing reaches the proxy.
#[test]
fn p3_hive_e2e_a_member_refused_its_cloud_model_and_reassignment_stay_local() {
    let Some(w) = e2e(
        "p3_hive_e2e_a_member_refused_its_cloud_model_and_reassignment_stay_local",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            let state = AppState::new_in_memory();
            let cloud = register_participant(
                &state,
                ("p3-hive-cloud", true, "anthropic/claude-sonnet-4-6"),
            );
            let local = register_participant(&state, ("p3-hive-local", true, ""));
            let outcome = hive_session(&state, vec![cloud.clone(), local.clone()])
                .map_err(AgentError::SupervisorError)?;
            // The cloud-assigned member was given each sub-task first and
            // refused it under its local-only policy; each was reassigned to
            // the other member.
            let value: serde_json::Value = serde_json::from_str(&outcome).unwrap();
            let assignments = value["assignments"].as_object().unwrap();
            let reassigned =
                !assignments.is_empty() && assignments.values().all(|agent| agent == &local);
            let refusals = state
                .db
                .load_tasks_by_agent(&cloud, 10)
                .unwrap()
                .into_iter()
                .filter(|task| {
                    !task.success
                        && task
                            .result_json
                            .as_deref()
                            .is_some_and(|json| json.contains("local-only"))
                })
                .count();
            if reassigned && refusals == 2 {
                Ok(outcome)
            } else {
                Err(AgentError::SupervisorError(format!(
                    "reassigned={reassigned} cloud_refusals={refusals} outcome={outcome}"
                )))
            }
        },
    ) else {
        return;
    };
    w.assert_hive_ran_locally("cloud-assigned member, reassigned");
}

/// E and F. An invalid, unknown, repeated or missing participant, or a
/// configuration that cannot be read, refuses the session before any model
/// call.
#[test]
fn p3_hive_e2e_unestablished_participants_refuse_the_session() {
    let Some(w) = e2e(
        "p3_hive_e2e_unestablished_participants_refuse_the_session",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            let state = AppState::new_in_memory();
            let known = register_participant(&state, ("p3-hive-local", true, ""));
            let mut outcomes = vec![
                hive_session(&state, vec![known.clone(), "not-an-id".into()]),
                hive_session(&state, vec![known.clone(), Uuid::new_v4().to_string()]),
                hive_session(&state, vec![known.clone(), known.to_uppercase()]),
                hive_session(&state, vec![]),
            ];
            std::fs::write(
                std::env::var("NEXUS_CONFIG_PATH").unwrap(),
                "local_only = [",
            )
            .unwrap();
            outcomes.push(hive_session(&state, vec![known.clone()]));
            let refused: Vec<String> = outcomes.iter().filter_map(|o| o.clone().err()).collect();
            if refused.len() == outcomes.len() {
                Err(AgentError::SupervisorError(refused.join(" | ")))
            } else {
                Ok(format!("not refused: {outcomes:?}"))
            }
        },
    ) else {
        return;
    };
    assert!(w.result.starts_with("ERR "), "{}", w.result);
    for reason in [
        "is not an agent id",
        "is not a registered agent",
        "is named more than once",
        "no participant was named",
        "routing policy could not be established",
    ] {
        assert!(w.result.contains(reason), "{reason}: {}", w.result);
    }
    assert!(w.proxy.is_empty(), "remote invocation {:?}", w.proxy);
    assert!(w.ollama.is_empty(), "a model was asked: {:?}", w.ollama);
}

/// G. The local runtime fails: the session fails at its first call, with no
/// cloud fallback.
#[test]
fn p3_hive_e2e_a_failed_local_runtime_is_not_a_cloud_fallback() {
    let mut serves = FakeOllama::serving(&["qwen3:8b"]);
    serves.generate_status = 500;
    let Some(w) = e2e(
        "p3_hive_e2e_a_failed_local_runtime_is_not_a_cloud_fallback",
        serves,
        || hive(&[("p3-hive-local", true, ""), ("p3-hive-local-2", true, "")]),
    ) else {
        return;
    };
    assert!(w.result.starts_with("ERR "), "{}", w.result);
    assert!(w.proxy.is_empty(), "remote invocation {:?}", w.proxy);
    assert_eq!(w.generates().len(), 1, "one local attempt: {:?}", w.ollama);
}

/// I and J. A local-only session, then an unrestricted session on the same
/// thread: the first stays local, and the second keeps the configured cloud
/// provider (it inherits nothing; the witness sees its decomposition).
#[test]
fn p3_hive_e2e_a_later_unrestricted_session_inherits_no_policy() {
    let Some(w) = e2e(
        "p3_hive_e2e_a_later_unrestricted_session_inherits_no_policy",
        FakeOllama::serving(&["qwen3:8b"]),
        || {
            let state = AppState::new_in_memory();
            let local = register_participant(&state, ("p3-hive-local", true, ""));
            let other = register_participant(&state, ("p3-hive-local-2", true, ""));
            let open = register_participant(&state, ("p3-hive-open", false, ""));
            let first = hive_session(&state, vec![local, other]);
            let second = hive_session(&state, vec![open]);
            Ok(format!("first={first:?} second={second:?}"))
        },
    ) else {
        return;
    };
    assert!(w.result.contains("first=Ok("), "{}", w.result);
    assert!(w.result.contains("second=Err("), "{}", w.result);
    assert!(
        w.proxy.iter().any(|line| line.starts_with("CONNECT ")),
        "the later unrestricted session reached the cloud witness: {:?}",
        w.proxy
    );
    let decompositions = w
        .generates()
        .iter()
        .filter(|r| r.contains("task decomposition engine"))
        .count();
    assert_eq!(
        decompositions, 1,
        "only the local-only session was local: {:?}",
        w.ollama
    );
}

/// J. An unrestricted session keeps the configured cloud provider.
#[test]
fn p3_hive_e2e_unrestricted_session_keeps_the_configured_provider() {
    let Some(w) = e2e(
        "p3_hive_e2e_unrestricted_session_keeps_the_configured_provider",
        FakeOllama::serving(&["qwen3:8b"]),
        || hive(&[("p3-hive-open", false, ""), ("p3-hive-other", false, "")]),
    ) else {
        return;
    };
    assert!(w.result.starts_with("ERR "), "{}", w.result);
    assert!(
        w.proxy.iter().any(|line| line.starts_with("CONNECT ")),
        "the cloud dispatch reached the witness: {:?} {}",
        w.proxy,
        w.result
    );
    assert!(w.generates().is_empty(), "{:?}", w.ollama);
}

// ── P3 HiveMind INFOFLOW ─────────────────────────────────────────────────

/// Synthetic restricted information: a local-only session's goal carries
/// it, and its decomposition carries it into each sub-task.
const RESTRICTED: &str = "P3-INFOFLOW-SYNTHETIC-RESTRICTED-7f3a";
const RESTRICTED_PLAN: &str = r#"[
  {"id": "t1", "description": "Summarize the synthetic note P3-INFOFLOW-SYNTHETIC-RESTRICTED-7f3a", "required_capabilities": ["llm.query"], "estimated_fuel": 1.0, "dependencies": []},
  {"id": "t2", "description": "List the synthetic note's points P3-INFOFLOW-SYNTHETIC-RESTRICTED-7f3a", "required_capabilities": ["llm.query"], "estimated_fuel": 1.0, "dependencies": []}
]"#;

fn is_decomposition(body: &str) -> bool {
    body.contains("task decomposition engine")
}

fn is_plan(body: &str) -> bool {
    body.contains("Output ONLY a JSON array")
}

fn is_merge(body: &str) -> bool {
    body.contains("Combine these sub-task results")
}

fn is_step(body: &str) -> bool {
    body.contains("P3 HiveMind synthetic step") && !is_plan(body) && !is_merge(body)
}

/// Unrestricted work's default model is an Ollama model reached on the
/// configured route: with no proxy exemption its every request goes to the
/// recording proxy, as plain HTTP whose prompt the proxy records.
fn route_unrestricted_work_through_the_proxy() {
    let mut config = nexus_kernel::config::load_config().unwrap();
    config.llm.default_model = "ollama/qwen3:8b".to_string();
    nexus_kernel::config::save_config(&config).unwrap();
}

/// An agent's own goal, outside any session: assigned, its loop started,
/// and waited for (within 30 s) until its task completes.
fn run_own_goal(state: &AppState, agent_id: &str, goal: &str) -> Result<String, String> {
    let goal_id = execute_agent_goal(state, agent_id.to_string(), goal.to_string(), 5, None)?;
    spawn_cognitive_loop_with_bridge(
        BackendEventBridge::default(),
        state.clone(),
        agent_id.to_string(),
        goal_id.clone(),
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        let tasks = state.db.load_tasks_by_agent(agent_id, 100).unwrap();
        if let Some(task) = tasks.into_iter().find(|task| task.id == goal_id) {
            if task.completed_at.is_some() {
                let result = task.result_json.unwrap_or_default();
                return if task.success {
                    Ok(result)
                } else {
                    Err(result)
                };
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("the goal did not complete within 30 s".to_string())
}

impl Witness {
    /// The proxy connections whose recorded request `matches`.
    fn proxied(&self, matches: fn(&str) -> bool) -> usize {
        self.proxy.iter().filter(|line| matches(line)).count()
    }
}

/// Stage A. A mixed session carrying restricted information, then the
/// unrestricted participant's own later goal on its configured route.
/// Before the admission rule the session ran (under its local-only floor),
/// the unrestricted participant stored the restricted sub-tasks in its
/// tasks and memories, and its later planning recalled them into a prompt
/// on the cloud route. Now the session is refused, nothing restricted is
/// stored for it, and its later cloud prompts carry nothing restricted;
/// the witness does see those prompts.
#[test]
fn p3_hive_e2e_a_mixed_session_leaves_nothing_restricted_for_later_cloud_work() {
    let mut serves = FakeOllama::serving(&["qwen3:8b"]);
    serves.proxy_answers = true;
    let Some(w) = e2e(
        "p3_hive_e2e_a_mixed_session_leaves_nothing_restricted_for_later_cloud_work",
        serves,
        || {
            let state = AppState::new_in_memory();
            let open = register_participant(&state, ("p3-hive-open", false, ""));
            let local = register_participant(&state, ("p3-hive-local", true, ""));
            route_unrestricted_work_through_the_proxy();
            let goal = format!("{PROMPT}: {RESTRICTED}");
            let session = hive_session_on(&state, &goal, vec![open.clone(), local]);
            let tasks = state.db.load_tasks_by_agent(&open, 1000).unwrap();
            let memories = state.db.load_memories(&open, None, 1000).unwrap();
            let stored = tasks
                .iter()
                .filter(|task| {
                    task.goal.contains(RESTRICTED)
                        || task
                            .result_json
                            .as_deref()
                            .is_some_and(|json| json.contains(RESTRICTED))
                })
                .count()
                + memories
                    .iter()
                    .filter(|memory| {
                        memory.key.contains(RESTRICTED) || memory.value_json.contains(RESTRICTED)
                    })
                    .count();
            let later = run_own_goal(&state, &open, "Summarize the synthetic note again");
            Ok(format!(
                "session={session:?} restricted_rows_of_unrestricted={stored} later={later:?}"
            ))
        },
    ) else {
        return;
    };
    // What reached the cloud route first: the witness saw the later
    // planning, and none of it carried the restricted information.
    assert!(
        w.proxied(is_plan) >= 1,
        "the witness saw the later cloud planning: {:?}",
        w.proxy
    );
    let carried: Vec<&String> = w
        .proxy
        .iter()
        .filter(|line| line.contains(RESTRICTED))
        .collect();
    assert!(
        carried.is_empty(),
        "restricted information on the cloud route: {carried:?}"
    );
    assert!(
        !w.ollama.iter().any(|r| r.contains(RESTRICTED)),
        "the refused session asked a model: {:?}",
        w.ollama
    );
    assert!(
        w.result.contains("restricted_rows_of_unrestricted=0 "),
        "{}",
        w.result
    );
    assert!(
        w.result.contains(
            "session=Err(\"hivemind session refused: the session mixes local-only and unrestricted participants"
        ),
        "{}",
        w.result
    );
}

/// An unrestricted session of "p3-hive-open" (listed first, so assigned
/// every sub-task) and "p3-hive-other", whose work runs on the configured
/// route, which the recording proxy answers.
fn configured_route_session() -> Result<String, AgentError> {
    let state = AppState::new_in_memory();
    let open = register_participant(&state, ("p3-hive-open", false, ""));
    let other = register_participant(&state, ("p3-hive-other", false, ""));
    route_unrestricted_work_through_the_proxy();
    let status = match hive_session(&state, vec![open.clone(), other.clone()]) {
        Ok(outcome) => {
            serde_json::from_str::<serde_json::Value>(&outcome).unwrap()["status"].to_string()
        }
        Err(error) => format!("Err({error})"),
    };
    let open_tasks = state.db.load_tasks_by_agent(&open, 100).unwrap().len();
    // Whether the owner's change took effect.
    let other_policy = issuer_policy(&state, &other);
    Ok(format!(
        "status={status} open_tasks={open_tasks} other_policy={other_policy:?}"
    ))
}

fn configured_route(tighten: Option<Tighten>) -> FakeOllama {
    let mut serves = FakeOllama::serving(&["qwen3:8b"]);
    serves.proxy_answers = true;
    serves.tighten = tighten;
    serves
}

/// J, on a configured route that answers: an unrestricted session runs to
/// completion, every model call of it on that route.
#[test]
fn p3_hive_e2e_an_unrestricted_session_completes_on_its_configured_route() {
    let Some(w) = e2e(
        "p3_hive_e2e_an_unrestricted_session_completes_on_its_configured_route",
        configured_route(None),
        configured_route_session,
    ) else {
        return;
    };
    assert!(w.result.contains("status=\"Completed\" "), "{}", w.result);
    assert_eq!(w.proxied(is_decomposition), 1, "{:?}", w.proxy);
    assert_eq!(w.proxied(is_plan), 2, "{:?}", w.proxy);
    assert!(w.proxied(is_step) >= 2, "{:?}", w.proxy);
    assert_eq!(w.proxied(is_merge), 1, "{:?}", w.proxy);
    assert!(w.generates().is_empty(), "{:?}", w.ollama);
    assert!(
        w.result.contains("other_policy=Unrestricted"),
        "{}",
        w.result
    );
}

/// Stage C. The owner makes "p3-hive-other" local-only during the
/// session, when the proxy receives the `nth` request that `matches`. The
/// session no longer qualifies: no later model call of it is dispatched,
/// on any route, and no later sub-task is started.
fn assert_nothing_after_tightening(w: &Witness, plans: usize, steps: usize, open_tasks: usize) {
    // What was dispatched first, on either route.
    assert_eq!(
        w.proxied(is_merge),
        0,
        "merge after tightening: {:?}",
        w.proxy
    );
    assert_eq!(w.proxied(is_step), steps, "{:?}", w.proxy);
    assert_eq!(w.proxied(is_plan), plans, "{:?}", w.proxy);
    assert_eq!(w.proxied(is_decomposition), 1, "{:?}", w.proxy);
    assert!(w.generates().is_empty(), "{:?}", w.ollama);
    assert!(
        w.result.contains(&format!("open_tasks={open_tasks} ")),
        "{}",
        w.result
    );
    assert!(w.result.contains("other_policy=LocalOnly"), "{}", w.result);
    assert!(!w.result.contains("status=\"Completed\""), "{}", w.result);
}

fn tightened(matches: fn(&str) -> bool, nth: usize) -> FakeOllama {
    configured_route(Some(Tighten {
        name: "p3-hive-other",
        matches,
        nth,
    }))
}

/// Stage C, after the decomposition: no sub-task starts, no merge.
#[test]
fn p3_hive_e2e_tightened_after_decomposition_dispatches_nothing_more() {
    let Some(w) = e2e(
        "p3_hive_e2e_tightened_after_decomposition_dispatches_nothing_more",
        tightened(is_decomposition, 1),
        configured_route_session,
    ) else {
        return;
    };
    assert_nothing_after_tightening(&w, 0, 0, 0);
}

/// Stage C, inside a sub-task loop (after its plan): the loop's next model
/// call (its step) is not dispatched, and nothing after it.
#[test]
fn p3_hive_e2e_tightened_inside_a_subtask_loop_dispatches_nothing_more() {
    let Some(w) = e2e(
        "p3_hive_e2e_tightened_inside_a_subtask_loop_dispatches_nothing_more",
        tightened(is_plan, 1),
        configured_route_session,
    ) else {
        return;
    };
    assert_nothing_after_tightening(&w, 1, 0, 1);
}

/// Stage C, between sub-tasks: the second sub-task does not start, and no
/// merge.
#[test]
fn p3_hive_e2e_tightened_between_subtasks_dispatches_nothing_more() {
    let Some(w) = e2e(
        "p3_hive_e2e_tightened_between_subtasks_dispatches_nothing_more",
        tightened(is_step, 1),
        configured_route_session,
    ) else {
        return;
    };
    assert_nothing_after_tightening(&w, 1, 1, 1);
}

/// Stage C, before the merge: the merge is not dispatched.
#[test]
fn p3_hive_e2e_tightened_before_the_merge_does_not_merge() {
    let Some(w) = e2e(
        "p3_hive_e2e_tightened_before_the_merge_does_not_merge",
        tightened(is_step, 2),
        configured_route_session,
    ) else {
        return;
    };
    assert_nothing_after_tightening(&w, 2, 2, 2);
    assert!(
        w.result
            .contains("routing policy changed during the session"),
        "{}",
        w.result
    );
}

// ── P3-C11-HM-M3 N6: production acceptance tests (P3-HM-CONTRACT-02 §18.2) ──
//
// The historical-flow probes N1..N11 and HF-1/HF-2 (retained pre-repair
// receipts: contract §18.1, `raw/H10`, `raw/H11`, `raw/H31`, `raw/H41`;
// definitions `diffs/H00`, `diffs/H30`, `diffs/H40`) ported to the
// production paths: the real goal loop, the desktop's adapters, the loopback
// runtime and the recording proxy (every non-local route). A loosening is
// the Owner's (AMENDMENT 1 A1.9: the TEST FIXTURE `owner_loosen`); the
// configuration only tightens (AD-3). Each asserts its preconditions first,
// so a failure of the final assertion is the defect itself.

const HM: &str = "P3-HM-SYNTH-RESTRICTED-5d1e";
const HIST: &str = "HIST-SYNTH-LEGACY-91c2";
const HM_KG: &str = "P3-HM-SYNTH-KG-3c7a";

/// `HM_RECALL_GOAL`: a MemoryRecall step, then a model step whose context
/// carries the recall's records.
const HM_RECALL_GOAL: &str = "P3-HM-RECALL-GOAL";
const HM_RECALL_PLAN: &str = r#"[{"action": {"type": "MemoryRecall", "query": "synthetic", "memory_type": null}, "description": "Recall"}, {"action": {"type": "LlmQuery", "prompt": "P3 HiveMind synthetic step", "context": []}, "description": "Answer"}]"#;
/// `HM_KG_WRITE_GOAL`: a knowledge-graph update naming `HM_KG`.
const HM_KG_WRITE_GOAL: &str = "P3-HM-KG-WRITE-GOAL";
const HM_KG_WRITE_PLAN: &str = r#"[{"action": {"type": "KnowledgeGraphUpdate", "entities": ["synthetic entity P3-HM-SYNTH-KG-3c7a"], "relationships": ["synthetic relation P3-HM-SYNTH-KG-3c7a"]}, "description": "Store"}, {"action": {"type": "LlmQuery", "prompt": "P3 HiveMind synthetic step", "context": []}, "description": "Answer"}]"#;
/// `HM_KG_READ_GOAL`: a knowledge-graph query, then a model step whose
/// prompt carries the query's records.
const HM_KG_READ_GOAL: &str = "P3-HM-KG-READ-GOAL";
const HM_KG_READ_PLAN: &str = r#"[{"action": {"type": "KnowledgeGraphQuery", "query": "synthetic"}, "description": "Query"}, {"action": {"type": "LlmQuery", "prompt": "P3 HiveMind synthetic step", "context": []}, "description": "Answer"}]"#;
/// `HM_PAUSE_GOAL`: step one, an approval the loop waits for, step two.
const HM_PAUSE_GOAL: &str = "P3-HM-PAUSE-GOAL";
const HM_PAUSE_PLAN: &str = r#"[{"action": {"type": "LlmQuery", "prompt": "P3 HM step one", "context": []}, "description": "One"}, {"action": {"type": "HitlRequest", "question": "Continue?", "options": ["yes", "no"]}, "description": "Pause"}, {"action": {"type": "LlmQuery", "prompt": "P3 HM step two", "context": []}, "description": "Two"}]"#;

/// The plan the fake runtime answers for a planning prompt naming one of
/// the N6 goals.
fn hm_plan_for(body: &str) -> Option<&'static str> {
    if !body.contains("Output ONLY a JSON array") {
        return None;
    }
    [
        (HM_RECALL_GOAL, HM_RECALL_PLAN),
        (HM_KG_WRITE_GOAL, HM_KG_WRITE_PLAN),
        (HM_KG_READ_GOAL, HM_KG_READ_PLAN),
        (HM_PAUSE_GOAL, HM_PAUSE_PLAN),
    ]
    .into_iter()
    .find(|(goal, _)| body.contains(&format!("GOAL: {goal}")))
    .map(|(_, plan)| plan)
}

/// A row with no provenance (the probes' seed): the goal-completion shape
/// the loop writes. Written raw, with no label, it is UNBOUND (§6.4).
fn hm_seed(state: &AppState, agent: &str, extra: &str) {
    state
        .db
        .save_memory(
            agent,
            "episodic",
            "episode:1700000000",
            &format!(
                "{{\"event\":\"goal completed: Summarize the synthetic note {HM}\",\"outcome\":\"steps=1\"{extra}}}"
            ),
        )
        .unwrap();
}

/// HF-1/HF-2's seed: an unclassified row (no label: UNBOUND).
fn hm_seed_legacy(state: &AppState, agent: &str) {
    state
        .db
        .save_memory(
            agent,
            "episodic",
            &format!("goal completed: Summarize the synthetic note {HIST}"),
            &format!("{{\"note\":\"synthetic legacy content {HIST}\"}}"),
        )
        .unwrap();
}

/// The Owner loosens `agent` (TEST FIXTURE, A1.9): the store's loosening
/// with its history row, and the configuration's mirror no longer naming
/// the agent local-only (a configuration that does is a tightening, §4.2,
/// and wins).
fn hm_loosen(state: &AppState, agent: &str) {
    let mut config = nexus_kernel::config::load_config().unwrap();
    if let Some(assignment) = config.agent_llm_assignments.get_mut(agent) {
        assignment.local_only = false;
    }
    nexus_kernel::config::save_config(&config).unwrap();
    crate::hm_wiring::test_support::owner_loosen(state, agent);
}

fn hm_rows_with(state: &AppState, agent: &str, marker: &str) -> usize {
    state
        .db
        .load_memories(agent, None, 1000)
        .unwrap()
        .iter()
        .filter(|m| m.key.contains(marker) || m.value_json.contains(marker))
        .count()
}

fn hm_await_row(state: &AppState, agent: &str, marker: &str) -> usize {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let n = hm_rows_with(state, agent, marker);
        if n > 0 || std::time::Instant::now() >= deadline {
            return n;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn hm_route() -> FakeOllama {
    let mut serves = FakeOllama::serving(&["qwen3:8b"]);
    serves.proxy_answers = true;
    serves
}

/// The raw network witness, kept whole in the receipt.
fn hm_dump(name: &str, w: &Witness) {
    for line in &w.proxy {
        eprintln!("HMWITNESS {name} proxy {line}");
    }
    for line in &w.ollama {
        eprintln!("HMWITNESS {name} loopback {line}");
    }
    eprintln!("HMWITNESS {name} result {}", w.result);
}

/// The rule: the witness did see non-local planning (so the test is not
/// vacuous), and none of it carried a prohibited marker.
fn hm_assert_no_prohibited_dispatch(w: &Witness, markers: &[&str], context: &str) {
    assert!(
        w.proxied(is_plan) >= 1,
        "{context}: precondition: the witness saw no non-local planning: {:?}",
        w.proxy
    );
    for marker in markers {
        let carried: Vec<&String> = w.proxy.iter().filter(|l| l.contains(marker)).collect();
        assert!(
            carried.is_empty(),
            "{context}: PROHIBITED CLOUD DISPATCH: {} non-local request(s) carried {marker}: {carried:?}",
            carried.len()
        );
    }
}

/// A supervisor-registered agent at `autonomy` with `capabilities`, its
/// assignment keyed by its UUID; an unrestricted one is an Owner loosening.
fn hm_register(
    state: &AppState,
    name: &str,
    local_only: bool,
    autonomy: u8,
    capabilities: &[&str],
) -> String {
    // Built in code, not parsed: `knowledge.graph` is not a registered
    // manifest capability (a parsed manifest naming it is refused).
    let manifest = nexus_kernel::manifest::AgentManifest {
        name: name.to_string(),
        version: "1.0.0".into(),
        capabilities: capabilities.iter().map(|c| c.to_string()).collect(),
        fuel_budget: 10_000,
        autonomy_level: Some(autonomy),
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
    };
    let id = state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(manifest)
        .unwrap()
        .to_string();
    let mut config = nexus_kernel::config::load_config().unwrap();
    config.agent_llm_assignments.insert(
        id.clone(),
        nexus_kernel::config::AgentLlmAssignment {
            provider_id: String::new(),
            local_only,
            budget_dollars: 0,
            budget_tokens: 0,
        },
    );
    nexus_kernel::config::save_config(&config).unwrap();
    if !local_only {
        hm_loosen(state, &id);
    }
    id
}

/// An agent's own goal whose loop waits for an approval between its steps:
/// `between` runs while it waits, then the approval is given and the goal
/// is waited for (within 30 s).
fn run_paused_goal(
    state: &AppState,
    agent_id: &str,
    goal: &str,
    between: impl FnOnce(),
) -> Result<String, String> {
    let goal_id = execute_agent_goal(state, agent_id.to_string(), goal.to_string(), 5, None)?;
    spawn_cognitive_loop_with_bridge(
        BackendEventBridge::default(),
        state.clone(),
        agent_id.to_string(),
        goal_id.clone(),
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let consent = loop {
        let pending = state.db.load_pending_consent().unwrap();
        if let Some(row) = pending
            .into_iter()
            .find(|r| r.agent_id == agent_id && r.operation_type == "cognitive.hitl_approval")
        {
            break row.id;
        }
        if std::time::Instant::now() >= deadline {
            return Err("the loop never waited for an approval".to_string());
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    between();
    crate::commands::consent::approve_consent_request(state, consent)?;
    while std::time::Instant::now() < deadline {
        let tasks = state.db.load_tasks_by_agent(agent_id, 100).unwrap();
        if let Some(task) = tasks.into_iter().find(|task| task.id == goal_id) {
            if task.completed_at.is_some() {
                let result = task.result_json.unwrap_or_default();
                return if task.success {
                    Ok(result)
                } else {
                    Err(result)
                };
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("the goal did not complete within 30 s".to_string())
}

/// The origins of `agent`'s label rows of `table` with `source_kind`.
fn hm_origins(state: &AppState, agent: &str, table: &str, source_kind: &str) -> Vec<String> {
    crate::hm_m3_tests::labels(&state.db)
        .into_iter()
        .filter(|l| l.agent_id == agent && l.record_table == table && l.source_kind == source_kind)
        .map(|l| l.origin)
        .collect()
}

fn hm_field(result: &str, key: &str) -> String {
    result
        .split(' ')
        .find_map(|f| f.strip_prefix(key))
        .unwrap_or_default()
        .to_string()
}

/// A-N1 (N1; HM-3, HM-4). A row of unknown provenance, for an agent that
/// is unrestricted now, is not placed in its non-local planning prompt.
#[test]
fn p3_hm_a_n1_unknown_provenance_is_not_cloud_eligible() {
    let name = "p3_hm_a_n1_unknown_provenance_is_not_cloud_eligible";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let open = register_participant(&state, ("p3-hm-open", false, ""));
        route_unrestricted_work_through_the_proxy();
        hm_seed(&state, &open, "");
        let later = run_own_goal(&state, &open, "Summarize the synthetic note again");
        let observed = crate::hm_m3_tests::event_kinds(&state.db)
            .iter()
            .filter(|(kind, _)| kind == "memory.unlabelled_record_observed")
            .count();
        Ok(format!("observed={observed} later={later:?}"))
    }) else {
        return;
    };
    hm_dump(name, &w);
    hm_assert_no_prohibited_dispatch(&w, &[HM], "A-N1");
    assert!(
        !w.result.contains("observed=0 "),
        "A-N1: the unbound row's reader emitted no memory.unlabelled_record_observed: {}",
        w.result
    );
}

/// A-HF1 (HF-1 inverted). An unclassified row already stored for an agent
/// that is now unrestricted is not recalled into its next planning prompt
/// on the configured (non-local) route.
#[test]
fn p3_hm_a_hf1_legacy_row_does_not_reach_cloud_planning() {
    let name = "p3_hm_a_hf1_legacy_row_does_not_reach_cloud_planning";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let open = register_participant(&state, ("p3-hist-open", false, ""));
        route_unrestricted_work_through_the_proxy();
        hm_seed_legacy(&state, &open);
        let later = run_own_goal(&state, &open, "Summarize the synthetic note again");
        Ok(format!("later={later:?}"))
    }) else {
        return;
    };
    hm_dump(name, &w);
    hm_assert_no_prohibited_dispatch(&w, &[HIST], "A-HF1");
}

/// A-N2 (N2; HM-5). A row stored while the agent was local-only stays
/// restricted after the Owner loosens the agent.
#[test]
fn p3_hm_a_n2_local_only_record_stays_restricted_after_loosening() {
    let name = "p3_hm_a_n2_local_only_record_stays_restricted_after_loosening";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        route_unrestricted_work_through_the_proxy();
        hm_seed(&state, &agent, "");
        let while_local = run_own_goal(&state, &agent, "Summarize the synthetic note again");
        eprintln!("HM-A-N2 PHASE-BOUNDARY");
        hm_loosen(&state, &agent);
        let after = run_own_goal(&state, &agent, "Summarize the synthetic note again");
        Ok(format!("while_local={while_local:?} after={after:?}"))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert!(
        w.ollama.iter().any(|r| r.contains(HM)),
        "A-N2: precondition: the local-only phase did not plan on loopback: {:?}",
        w.ollama
    );
    hm_assert_no_prohibited_dispatch(&w, &[HM], "A-N2");
}

/// A-N3 (N3; HM-5, HM-9). Work done under local-only (the loop's own
/// labelled goal rows) is not recalled into a later goal's non-local
/// planning after loosening.
#[test]
fn p3_hm_a_n3_restricted_work_is_not_recalled_into_a_later_cloud_goal() {
    let name = "p3_hm_a_n3_restricted_work_is_not_recalled_into_a_later_cloud_goal";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        route_unrestricted_work_through_the_proxy();
        let first = run_own_goal(
            &state,
            &agent,
            &format!("Summarize the synthetic note {HM}"),
        );
        let stored = hm_rows_with(&state, &agent, HM);
        let origins = hm_origins(&state, &agent, "agent_memory", "loop_episodic");
        eprintln!("HM-A-N3 PHASE-BOUNDARY");
        hm_loosen(&state, &agent);
        let later = run_own_goal(&state, &agent, "List the synthetic note points");
        Ok(format!(
            "first={first:?} stored_restricted_rows={stored} origins={} later={later:?}",
            origins.join(",")
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert!(
        !w.result.contains("stored_restricted_rows=0 "),
        "A-N3: precondition: the local-only goal stored nothing: {}",
        w.result
    );
    let origins = hm_field(&w.result, "origins=");
    assert!(
        !origins.is_empty() && origins.split(',').all(|o| o == "LOCAL_ONLY_ORIGIN"),
        "A-N3: the local-only goal's rows are not LOCAL_ONLY: {}",
        w.result
    );
    hm_assert_no_prohibited_dispatch(&w, &[HM], "A-N3");
}

/// A-N4a (N4a; HM-9). Every cloud credential is configured and the Owner
/// names a non-local route in the agent's own assignment: the restricted
/// row is still not sent there.
#[test]
fn p3_hm_a_n4a_configured_cloud_route_receives_no_restricted_memory() {
    let name = "p3_hm_a_n4a_configured_cloud_route_receives_no_restricted_memory";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        hm_seed(&state, &agent, "");
        let mut config = nexus_kernel::config::load_config().unwrap();
        let a = config.agent_llm_assignments.get_mut(&agent).unwrap();
        a.local_only = false;
        a.provider_id = "ollama/qwen3:8b".to_string();
        nexus_kernel::config::save_config(&config).unwrap();
        hm_loosen(&state, &agent);
        let later = run_own_goal(&state, &agent, "Summarize the synthetic note again");
        Ok(format!("later={later:?}"))
    }) else {
        return;
    };
    hm_dump(name, &w);
    hm_assert_no_prohibited_dispatch(&w, &[HM], "A-N4a");
}

/// A-N5 (N5; HM-8, §14 rule 3). Cross-agent learning from a local-only
/// source carries nothing derived from the source's restricted work to an
/// unrestricted agent's non-local planning: the copy is refused (its label
/// is not UNRESTRICTED and the target is not local-only).
#[test]
fn p3_hm_a_n5_cross_agent_transfer_from_a_local_only_source_stays_off_the_cloud() {
    let name = "p3_hm_a_n5_cross_agent_transfer_from_a_local_only_source_stays_off_the_cloud";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let source = hm_register(&state, "p3-hm-source", true, 4, &["llm.query"]);
        let target = register_participant(&state, ("p3-hm-target", false, ""));
        route_unrestricted_work_through_the_proxy();
        // The source's own local-only goal: post-goal evolution records its
        // strategy (labelled LOCAL_ONLY) the way the loop records it.
        let goal = format!("Summarize the synthetic note {HM}");
        let first = run_own_goal(&state, &source, &goal);
        let hash = nexus_kernel::cognitive::hash_strategy(&goal.to_lowercase());
        let lowered = HM.to_lowercase();
        let derived = hm_await_row(&state, &source, &lowered);
        let strategy_origins = hm_origins(&state, &source, "strategy_scores", "strategy_score");
        // The transfer primitive the cross-agent command calls, in a unit of
        // the target (its goal unit, derived for evolution as the command
        // derives one).
        let ctx = crate::hm_wiring::test_support::goal_unit(&state, &target)?;
        let issuer = state.hm_issuer().unwrap();
        let shared = state
            .evolution_tracker
            .share_learning(issuer, &ctx, &source, &hash, 0.95);
        let received = hm_rows_with(&state, &target, &hash);
        let later = run_own_goal(&state, &target, "Summarize the synthetic note again");
        Ok(format!(
            "source={source} hash={hash} derived={derived} strategy={} shared_ok={} received={received} first={first:?} later={later:?}",
            strategy_origins.join(","),
            shared.is_ok()
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    let (source, hash) = (hm_field(&w.result, "source="), hm_field(&w.result, "hash="));
    assert!(
        hash.len() == 16 && !w.result.contains("derived=0 "),
        "A-N5: precondition: the source's goal derived nothing: {}",
        w.result
    );
    assert_eq!(
        hm_field(&w.result, "strategy="),
        "LOCAL_ONLY_ORIGIN",
        "A-N5: precondition: the source's strategy is not LOCAL_ONLY: {}",
        w.result
    );
    assert!(
        w.result.contains("shared_ok=false ") && w.result.contains("received=0 "),
        "A-N5: a LOCAL_ONLY learning was copied to an unrestricted agent: {}",
        w.result
    );
    hm_assert_no_prohibited_dispatch(&w, &[hash.as_str(), source.as_str(), HM], "A-N5");
}

/// A-N6 (N6; HM-1, HM-6). A string in the record claiming declassification
/// is not authority: the row stays restricted after loosening.
#[test]
fn p3_hm_a_n6_a_stored_string_does_not_declassify() {
    let name = "p3_hm_a_n6_a_stored_string_does_not_declassify";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        route_unrestricted_work_through_the_proxy();
        hm_seed(
            &state,
            &agent,
            ",\"classification\":\"public\",\"cloud_eligible\":true,\"declassified_by\":\"owner\",\"origin\":\"UNRESTRICTED_ORIGIN\"",
        );
        hm_loosen(&state, &agent);
        let later = run_own_goal(&state, &agent, "Summarize the synthetic note again");
        Ok(format!("later={later:?}"))
    }) else {
        return;
    };
    hm_dump(name, &w);
    hm_assert_no_prohibited_dispatch(&w, &[HM], "A-N6");
}

/// A-N7 (N7; HM-7). The Owner makes the agent local-only while its plan is
/// in flight on the non-local route: the reply is discarded at acceptance
/// (O3) and no later dispatch of that work uses the revoked authorization.
#[test]
fn p3_hm_a_n7_a_revoked_authorization_is_not_used_again() {
    let name = "p3_hm_a_n7_a_revoked_authorization_is_not_used_again";
    let mut serves = hm_route();
    serves.tighten = Some(Tighten {
        name: "p3-hm-agent",
        matches: is_plan,
        nth: 1,
    });
    let Some(w) = e2e(name, serves, || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", false, ""));
        route_unrestricted_work_through_the_proxy();
        let run = run_own_goal(
            &state,
            &agent,
            &format!("Summarize the synthetic note {HM}"),
        );
        let policy = issuer_policy(&state, &agent);
        let outcomes: Vec<String> = crate::hm_m3_tests::admissions(&state.db)
            .into_iter()
            .map(|a| format!("{}:{}", a.route_class, a.outcome))
            .collect();
        Ok(format!(
            "run={run:?} policy_after={policy:?} outcomes={}",
            outcomes.join(",")
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert!(
        w.result.contains("policy_after=LocalOnly"),
        "A-N7: precondition: the revocation did not take effect: {}",
        w.result
    );
    assert_eq!(
        w.proxied(is_plan),
        1,
        "A-N7: precondition: one plan in flight at revocation: {:?}",
        w.proxy
    );
    let revoked_at = w.proxy.iter().position(|l| is_plan(l)).unwrap();
    let after: Vec<&String> = w.proxy[revoked_at + 1..].iter().collect();
    assert!(
        after.is_empty(),
        "A-N7: PROHIBITED CLOUD DISPATCH under a revoked authorization: {after:?}"
    );
    assert!(
        hm_field(&w.result, "outcomes=").starts_with("non_local:reply_discarded"),
        "A-N7: the in-flight non-local reply was not discarded: {}",
        w.result
    );
}

/// A-N8 (N8; HM-11; stays PASS). No readable policy: nothing is dispatched
/// anywhere, and the restricted row reaches no model.
#[test]
fn p3_hm_a_n8_no_readable_policy_dispatches_nothing() {
    let name = "p3_hm_a_n8_no_readable_policy_dispatches_nothing";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        route_unrestricted_work_through_the_proxy();
        hm_seed(&state, &agent, "");
        std::fs::write(
            std::env::var("NEXUS_CONFIG_PATH").unwrap(),
            "local_only = [",
        )
        .unwrap();
        let later = run_own_goal(&state, &agent, "Summarize the synthetic note again");
        Ok(format!("later={later:?}"))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert!(
        w.proxy.is_empty(),
        "A-N8: PROHIBITED CLOUD DISPATCH without a readable policy: {:?}",
        w.proxy
    );
    assert!(
        !w.ollama.iter().any(|r| r.contains(HM)),
        "A-N8: the restricted row reached a model: {:?}",
        w.ollama
    );
}

/// A-N9a (N9a; HM-11; stays PASS). A local-only agent with no available
/// local model: no cloud fallback carries anything.
#[test]
fn p3_hm_a_n9a_no_local_model_is_not_a_cloud_fallback() {
    let name = "p3_hm_a_n9a_no_local_model_is_not_a_cloud_fallback";
    let mut serves = FakeOllama::serving(&[]);
    serves.proxy_answers = true;
    let Some(w) = e2e(name, serves, || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        route_unrestricted_work_through_the_proxy();
        hm_seed(&state, &agent, "");
        let later = run_own_goal(&state, &agent, "Summarize the synthetic note again");
        Ok(format!("later={later:?}"))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert!(
        w.proxy.is_empty(),
        "A-N9a: PROHIBITED CLOUD DISPATCH as a fallback: {:?}",
        w.proxy
    );
    assert!(w.generates().is_empty(), "A-N9a: {:?}", w.ollama);
}

/// A-N9b (N9b; HM-5, HM-11). After loosening, the agent's local model is
/// unavailable: its local-only-era row does not go to the non-local route
/// in place of the local one.
#[test]
fn p3_hm_a_n9b_no_local_model_does_not_release_restricted_memory() {
    let name = "p3_hm_a_n9b_no_local_model_does_not_release_restricted_memory";
    let mut serves = FakeOllama::serving(&[]);
    serves.proxy_answers = true;
    let Some(w) = e2e(name, serves, || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        route_unrestricted_work_through_the_proxy();
        hm_seed(&state, &agent, "");
        hm_loosen(&state, &agent);
        let later = run_own_goal(&state, &agent, "Summarize the synthetic note again");
        Ok(format!("later={later:?}"))
    }) else {
        return;
    };
    hm_dump(name, &w);
    hm_assert_no_prohibited_dispatch(&w, &[HM], "A-N9b");
}

/// A-HF2 (HF-2 inverted). A row present while the agent is local-only
/// reaches only the loopback runtime; after the Owner loosens the agent the
/// next planning prompt on the configured route does not carry it.
#[test]
fn p3_hm_a_hf2_loosening_does_not_expose_rows_stored_while_local() {
    let name = "p3_hm_a_hf2_loosening_does_not_expose_rows_stored_while_local";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hist-agent", true, ""));
        route_unrestricted_work_through_the_proxy();
        hm_seed_legacy(&state, &agent);
        let while_local = run_own_goal(&state, &agent, "Summarize the synthetic note again");
        eprintln!("HM-A-HF2 PHASE-BOUNDARY");
        hm_loosen(&state, &agent);
        let after = run_own_goal(&state, &agent, "Summarize the synthetic note again");
        Ok(format!(
            "while_local={while_local:?} after_loosening={after:?}"
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert!(
        w.ollama.iter().any(|r| r.contains(HIST)),
        "A-HF2: precondition: the local-only phase did not plan on the loopback runtime: {:?}",
        w.ollama
    );
    hm_assert_no_prohibited_dispatch(&w, &[HIST], "A-HF2");
}

/// A-N10 (N10; HM-8, derived data). The desktop's post-goal evolution
/// derives a procedural row from a local-only goal (`strategy:<hash>
/// goal_type:<lowercased goal>`); it is labelled from its parents. After
/// loosening, a later goal's non-local planning does not carry it.
#[test]
fn p3_hm_a_n10_derived_record_inherits_local_only_restriction() {
    let name = "p3_hm_a_n10_derived_record_inherits_local_only_restriction";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = hm_register(&state, "p3-hm-agent", true, 4, &["llm.query"]);
        route_unrestricted_work_through_the_proxy();
        let first = run_own_goal(
            &state,
            &agent,
            &format!("Summarize the synthetic note {HM}"),
        );
        let lowered = HM.to_lowercase();
        let derived = hm_await_row(&state, &agent, &lowered);
        let hash = state
            .db
            .load_memories(&agent, None, 1000)
            .unwrap()
            .iter()
            .filter(|m| m.value_json.contains(&lowered))
            .find_map(|m| {
                m.value_json
                    .split("strategy:")
                    .nth(1)
                    .and_then(|rest| rest.split(' ').next())
                    .map(str::to_string)
            })
            .unwrap_or_default();
        let procedural = hm_origins(&state, &agent, "agent_memory", "evolution_procedural");
        eprintln!("HM-A-N10 PHASE-BOUNDARY");
        hm_loosen(&state, &agent);
        let ids = || -> std::collections::BTreeMap<String, String> {
            crate::hm_m3_tests::labels(&state.db)
                .into_iter()
                .filter(|l| {
                    l.agent_id == agent
                        && l.record_table == "agent_memory"
                        && l.source_kind == "evolution_procedural"
                })
                .map(|l| (l.record_id, l.origin))
                .collect()
        };
        // The in-between goal's own procedural row is written by its post-goal
        // evolution after its task completes, so possibly after run_own_goal
        // returns: it is awaited here, before the snapshot that would
        // otherwise count it as the re-run goal's.
        let before_later = ids();
        let later = run_own_goal(
            &state,
            &agent,
            "Review strategy: goal_type: composite: entries",
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut with_later = ids();
        while with_later.keys().all(|id| before_later.contains_key(id))
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(20));
            with_later = ids();
        }
        let later_rows: Vec<String> = with_later
            .into_iter()
            .filter(|(id, _)| !before_later.contains_key(id))
            .map(|(_, origin)| origin)
            .collect();
        // Phase 2 (mutant 3): the same goal again, now in an unrestricted
        // unit. Its post-goal evolution writes a new procedural row derived
        // from the LOCAL_ONLY strategy aggregate (its parent): the row is
        // LOCAL_ONLY by that parent alone, and a later non-local goal does
        // not carry it.
        let before = ids();
        let again = run_own_goal(
            &state,
            &agent,
            &format!("Summarize the synthetic note {HM}"),
        );
        // The wait ends at the re-run goal's own row: it carries the
        // lowercased local-only goal type.
        let local_goal_type = format!("goal_type:summarize the synthetic note {lowered}");
        let has_again_row = |rows: &std::collections::BTreeMap<String, String>| {
            let memories = state.db.load_memories(&agent, None, 1000).unwrap();
            rows.keys()
                .filter(|id| !before.contains_key(*id))
                .any(|id| {
                    memories
                        .iter()
                        .any(|m| m.id.to_string() == *id && m.value_json.contains(&local_goal_type))
                })
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut after = ids();
        while !has_again_row(&after) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
            after = ids();
        }
        // The rows the re-run goal added (not the in-between goal's own,
        // which derive from its own unrestricted strategy).
        let again_rows: Vec<String> = after
            .into_iter()
            .filter(|(id, _)| !before.contains_key(id))
            .map(|(_, origin)| origin)
            .collect();
        eprintln!("HM-A-N10 PHASE-2-BOUNDARY");
        let third = run_own_goal(
            &state,
            &agent,
            "Review strategy: goal_type: composite: entries",
        );
        Ok(format!(
            "hash={hash} derived_rows={derived} procedural={} later_rows={} again_rows={} first={first:?} later={later:?} again={again:?} third={third:?}",
            procedural.join(","),
            later_rows.join(","),
            again_rows.join(",")
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    let hash = hm_field(&w.result, "hash=");
    assert!(
        !w.result.contains("derived_rows=0 ") && hash.len() == 16,
        "A-N10: precondition: evolution derived no row from the local-only goal: {}",
        w.result
    );
    assert!(
        w.ollama.iter().any(|r| r.contains(HM)),
        "A-N10: precondition: the local-only phase did not plan on loopback: {:?}",
        w.ollama
    );
    assert!(
        !hm_field(&w.result, "later_rows=").is_empty(),
        "A-N10: harness precondition: the in-between goal's own evolution row did not appear within 10 s: {}",
        w.result
    );
    let procedural = hm_field(&w.result, "procedural=");
    assert!(
        !procedural.is_empty() && procedural.split(',').all(|o| o == "LOCAL_ONLY_ORIGIN"),
        "A-N10: the derived procedural row is not LOCAL_ONLY: {}",
        w.result
    );
    let again = hm_field(&w.result, "again_rows=");
    assert!(
        !again.is_empty() && again.split(',').all(|o| o == "LOCAL_ONLY_ORIGIN"),
        "A-N10: the procedural row derived from the LOCAL_ONLY aggregate in an unrestricted unit is not LOCAL_ONLY: {}",
        w.result
    );
    let lowered = HM.to_lowercase();
    hm_assert_no_prohibited_dispatch(&w, &[lowered.as_str(), hash.as_str()], "A-N10");
}

/// A-N11 (N11; HM-9, the MemoryRecall read site). A local-only-era row is
/// not placed, through a MemoryRecall step's records, in a later non-local
/// model step. The final assertion looks only at non-plan requests.
#[test]
fn p3_hm_a_n11_memory_recall_output_does_not_reach_a_non_local_step() {
    let name = "p3_hm_a_n11_memory_recall_output_does_not_reach_a_non_local_step";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        route_unrestricted_work_through_the_proxy();
        hm_seed(&state, &agent, "");
        hm_loosen(&state, &agent);
        let later = run_own_goal(&state, &agent, &format!("{HM_RECALL_GOAL} review the note"));
        Ok(format!("later={later:?}"))
    }) else {
        return;
    };
    hm_dump(name, &w);
    let steps: Vec<&String> = w.proxy.iter().filter(|l| !is_plan(l)).collect();
    assert!(
        w.proxied(is_plan) >= 1
            && steps
                .iter()
                .any(|l| l.contains("P3 HiveMind synthetic step")),
        "A-N11: precondition: no non-local model step after the MemoryRecall step: {:?}",
        w.proxy
    );
    let carried: Vec<&&String> = steps.iter().filter(|l| l.contains(HM)).collect();
    assert!(
        carried.is_empty(),
        "A-N11: PROHIBITED CLOUD DISPATCH: {} non-local step(s) carried {HM} via MemoryRecall: {carried:?}",
        carried.len()
    );
}

fn is_step_two(body: &str) -> bool {
    body.contains("P3 HM step two") && !is_plan(body)
}

/// A-N12 (new; FAIL before). The Owner loosens the agent between step one
/// and step two of one goal: step two stays on the local route (the goal's
/// unit started local-only; a loosening applies only to units that start
/// after it), and what the goal writes after the loosening is LOCAL_ONLY.
#[test]
fn p3_hm_a_n12_loosening_mid_goal_keeps_step_two_local() {
    let name = "p3_hm_a_n12_loosening_mid_goal_keeps_step_two_local";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = hm_register(&state, "p3-hm-agent", true, 2, &["llm.query"]);
        route_unrestricted_work_through_the_proxy();
        let run = run_paused_goal(&state, &agent, &format!("{HM_PAUSE_GOAL} {HM}"), || {
            eprintln!("HM-A-N12 LOOSENED BETWEEN THE STEPS");
            hm_loosen(&state, &agent);
        });
        let policy = issuer_policy(&state, &agent);
        let episodic = hm_origins(&state, &agent, "agent_memory", "loop_episodic");
        Ok(format!(
            "policy_after={policy:?} episodic={} run={run:?}",
            episodic.join(",")
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert!(
        w.result.contains("policy_after=Unrestricted"),
        "A-N12: precondition: the loosening did not take effect: {}",
        w.result
    );
    assert!(
        w.ollama.iter().any(|r| r.contains("P3 HM step one")),
        "A-N12: precondition: step one did not run on the loopback runtime: {:?}",
        w.ollama
    );
    assert!(
        w.proxied(is_step_two) == 0,
        "A-N12: step two left the machine after the mid-goal loosening: {:?}",
        w.proxy
    );
    assert!(
        w.ollama.iter().any(|r| is_step_two(r)),
        "A-N12: step two did not run on the local route: {:?}",
        w.ollama
    );
    assert!(w.proxy.is_empty(), "A-N12: {:?}", w.proxy);
    let episodic = hm_field(&w.result, "episodic=");
    assert!(
        !episodic.is_empty() && episodic.split(',').all(|o| o == "LOCAL_ONLY_ORIGIN"),
        "A-N12: a write of the goal after the loosening is not LOCAL_ONLY: {}",
        w.result
    );
}

/// A-K1 (new; FAIL before). Knowledge-graph rows written while the agent is
/// local-only are absent, after the Owner loosens it, from what a
/// KnowledgeGraphQuery step hands a later non-local model step.
#[test]
fn p3_hm_a_k1_local_only_graph_rows_stay_out_of_a_later_non_local_step() {
    let name = "p3_hm_a_k1_local_only_graph_rows_stay_out_of_a_later_non_local_step";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = hm_register(
            &state,
            "p3-hm-agent",
            true,
            3,
            &["llm.query", "knowledge.graph"],
        );
        route_unrestricted_work_through_the_proxy();
        let wrote = run_own_goal(&state, &agent, HM_KG_WRITE_GOAL);
        let graph = crate::hm_m3_tests::labels(&state.db)
            .into_iter()
            .filter(|l| l.agent_id == agent && l.record_table.starts_with("world_model_"))
            .map(|l| l.origin)
            .collect::<Vec<_>>();
        eprintln!("HM-A-K1 PHASE-BOUNDARY");
        hm_loosen(&state, &agent);
        let read = run_own_goal(&state, &agent, HM_KG_READ_GOAL);
        Ok(format!(
            "graph={} wrote={wrote:?} read={read:?}",
            graph.join(",")
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    let graph = hm_field(&w.result, "graph=");
    assert!(
        graph.split(',').count() == 2 && graph.split(',').all(|o| o == "LOCAL_ONLY_ORIGIN"),
        "A-K1: precondition: the local-only goal stored no LOCAL_ONLY graph rows: {}",
        w.result
    );
    let steps: Vec<&String> = w.proxy.iter().filter(|l| !is_plan(l)).collect();
    assert!(
        w.proxied(is_plan) >= 1
            && steps
                .iter()
                .any(|l| l.contains("P3 HiveMind synthetic step")),
        "A-K1: precondition: no non-local model step after the graph query: {:?}",
        w.proxy
    );
    let carried: Vec<&String> = w.proxy.iter().filter(|l| l.contains(HM_KG)).collect();
    assert!(
        carried.is_empty(),
        "A-K1: PROHIBITED CLOUD DISPATCH: {} non-local request(s) carried a LOCAL_ONLY graph row: {carried:?}",
        carried.len()
    );
}

/// A-C2 (production): a record whose content was altered after it was
/// labelled (digest mismatch) is UNBOUND: it is not placed in a non-local
/// planning prompt, and its reader emits memory.unlabelled_record_observed.
#[test]
fn p3_hm_a_c2_altered_record_is_unbound_and_not_eligible() {
    let name = "p3_hm_a_c2_altered_record_is_unbound_and_not_eligible";
    let marker = "P3-HM-SYNTH-ALTERED-9e41";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-open", false, ""));
        route_unrestricted_work_through_the_proxy();
        // An unrestricted agent's own goal writes UNRESTRICTED rows.
        let first = run_own_goal(&state, &agent, "Summarize the synthetic note");
        let origins = hm_origins(&state, &agent, "agent_memory", "loop_episodic");
        // Out of band, the content of each changes (its digest no longer
        // matches its label).
        state
            .db
            .with_transaction(|tx| {
                let altered = format!("altered {marker}");
                tx.execute(
                    "UPDATE agent_memory SET value_json = json_set(value_json, '$.event', ?2)
                     WHERE agent_id = ?1",
                    [agent.as_str(), altered.as_str()],
                )?;
                Ok(())
            })
            .unwrap();
        let later = run_own_goal(&state, &agent, "Summarize the synthetic note again");
        let observed = crate::hm_m3_tests::event_kinds(&state.db)
            .iter()
            .filter(|(kind, _)| kind == "memory.unlabelled_record_observed")
            .count();
        Ok(format!(
            "origins={} observed={observed} first={first:?} later={later:?}",
            origins.join(",")
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    let origins = hm_field(&w.result, "origins=");
    assert!(
        !origins.is_empty() && origins.split(',').all(|o| o == "UNRESTRICTED_ORIGIN"),
        "A-C2: precondition: the unrestricted goal's rows are not UNRESTRICTED: {}",
        w.result
    );
    hm_assert_no_prohibited_dispatch(&w, &[marker], "A-C2");
    assert!(
        !w.result.contains("observed=0 "),
        "A-C2: no memory.unlabelled_record_observed for the altered rows: {}",
        w.result
    );
}

const HM_DREAM: &str = "P3-HM-SYNTH-DREAM-41b0";

/// A-D3 (production). A chat routed to a local-only agent queues a dream
/// entry owned by that agent and labelled LOCAL_ONLY; its dream dispatch is
/// never non-local.
#[test]
fn p3_hm_a_d3_dream_from_a_local_only_chat_stays_local() {
    let name = "p3_hm_a_d3_dream_from_a_local_only_chat_stays_local";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-dreamer", true, ""));
        route_unrestricted_work_through_the_proxy();
        let message = format!("Recall the synthetic note {HM_DREAM}");
        let turn = hm_turn(
            &state,
            crate::hm_dispatch::chat_turn(&state, Some("p3-hm-dreamer"))
                .map_err(AgentError::SupervisorError)?,
            &message,
            None,
        )
        .map_err(AgentError::SupervisorError)?;
        let entries = hm_queue_only(&state, &turn, &message);
        let dreamt = crate::commands::simulation::trigger_dream_now(&state);
        Ok(format!("agent={agent} entries={entries} dreamt={dreamt:?}"))
    }) else {
        return;
    };
    hm_dump(name, &w);
    let agent = hm_field(&w.result, "agent=");
    let entries = hm_field(&w.result, "entries=");
    assert!(
        !entries.is_empty()
            && entries
                .split(';')
                .all(|e| e == format!("{agent}|{agent}|LOCAL_ONLY_ORIGIN")),
        "A-D3: precondition: the turn queued no LOCAL_ONLY entry owned by the agent: {}",
        w.result
    );
    assert!(
        w.ollama.iter().filter(|r| r.contains(HM_DREAM)).count() >= 2,
        "A-D3: precondition: the turn and its dream did not run on the loopback runtime: {:?}",
        w.ollama
    );
    // Nothing of this scenario may leave the machine: the chat is routed to
    // a local-only agent, and its dream entry is that agent's (a TLS request
    // hides its content, so any non-local request counts).
    assert!(
        w.proxy.is_empty(),
        "A-D3: PROHIBITED CLOUD DISPATCH of a LOCAL_ONLY dream entry: {:?}",
        w.proxy
    );
}

/// A-D4 (production). An OwnerDirect chat whose turn went to the attested
/// local transport queues an OwnerDirect entry labelled LOCAL_ONLY, and its
/// dream dispatch is never non-local; an agent-owned entry carries the real
/// agent id (never the chat's ephemeral id).
#[test]
fn p3_hm_a_d4_owner_direct_local_turn_dream_never_leaves_and_agent_entries_carry_the_agent() {
    let name =
        "p3_hm_a_d4_owner_direct_local_turn_dream_never_leaves_and_agent_entries_carry_the_agent";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-open", false, ""));
        // The OwnerDirect turn's model is the loopback runtime, attested
        // local (no agent is named).
        let mut config = nexus_kernel::config::load_config().unwrap();
        config.llm.default_model = "ollama/qwen3:8b".to_string();
        nexus_kernel::config::save_config(&config).unwrap();
        let owner = {
            let mut turn =
                crate::hm_dispatch::chat_turn(&state, None).map_err(AgentError::SupervisorError)?;
            // The OwnerDirect turn is made local-only through the context's
            // own (monotone) tightening, so its transport is the attested
            // loopback runtime.
            turn.tighten_local_only();
            let message = format!("Recall the synthetic note {HM_DREAM}");
            let turn =
                hm_turn(&state, turn, &message, None).map_err(AgentError::SupervisorError)?;
            let entries = hm_queue_only(&state, &turn, &message);
            let dreamt = crate::commands::simulation::trigger_dream_now(&state);
            format!("{entries} dreamt={dreamt:?}")
        };
        let agent_entries = {
            let turn = crate::hm_dispatch::chat_turn(&state, Some("p3-hm-open"))
                .map_err(AgentError::SupervisorError)?;
            let turn = hm_turn(&state, turn, "An agent turn", Some("ollama/qwen3:8b"))
                .map_err(AgentError::SupervisorError)?;
            hm_queue_only(&state, &turn, "An agent turn")
        };
        Ok(format!(
            "agent={agent} owner={owner} agent_entries={agent_entries}"
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    let owner = hm_field(&w.result, "owner=");
    assert!(
        owner.starts_with("owner|OwnerDirect|LOCAL_ONLY_ORIGIN"),
        "A-D4: precondition: the OwnerDirect local turn queued no LOCAL_ONLY OwnerDirect entry: {}",
        w.result
    );
    let carried: Vec<&String> = w.proxy.iter().filter(|l| l.contains(HM_DREAM)).collect();
    assert!(
        carried.is_empty(),
        "A-D4: PROHIBITED CLOUD DISPATCH of an OwnerDirect LOCAL_ONLY dream: {carried:?}"
    );
    assert!(
        w.ollama.iter().filter(|r| r.contains(HM_DREAM)).count() >= 2,
        "A-D4: precondition: the turn and its dream did not run locally: {:?}",
        w.ollama
    );
    let agent = hm_field(&w.result, "agent=");
    let agent_entries = hm_field(&w.result, "agent_entries=");
    assert!(
        !agent_entries.is_empty()
            && agent_entries
                .split(';')
                .all(|e| e.starts_with(&format!("{agent}|{agent}|"))),
        "A-D4: an agent-owned entry does not carry the real agent id: {}",
        w.result
    );
}

/// One chat turn through the production turn path: `message` dispatched
/// through the issuer on the transport the turn's context selects (its
/// entry label is set by that dispatch, A1.8). Returns the turn.
fn hm_turn(
    state: &AppState,
    mut turn: nexus_kernel::hm::DispatchContext,
    message: &str,
    model: Option<&str>,
) -> Result<nexus_kernel::hm::DispatchContext, String> {
    use nexus_connectors_llm::providers::LlmProvider;
    crate::hm_dispatch::dispatch_text(
        state,
        &mut turn,
        message,
        &mut |ctx| {
            let (provider, model, attestation) = crate::hm_dispatch::human_provider(ctx, model)?;
            Ok(((provider, model), attestation))
        },
        &mut |(provider, model): (Box<dyn LlmProvider>, String), admitted: &str| {
            provider
                .query(admitted, 256, &model)
                .map(|r| r.output_text)
                .map_err(|e| e.to_string())
        },
    )?;
    Ok(turn)
}

/// The production dream queueing of `turn` after a failed interaction (a
/// Replay dream), and the queued entries: `source|principal|origin;...`.
fn hm_queue_only(
    state: &AppState,
    turn: &nexus_kernel::hm::DispatchContext,
    message: &str,
) -> String {
    let mut consciousness = nexus_kernel::consciousness::ConsciousnessState::new("synthetic");
    consciousness.frustration = 0.0;
    let interaction = nexus_kernel::dreams::auto_queue::ChatInteraction {
        user_message: message.to_string(),
        agent_response: String::new(),
        was_error: true,
        error_message: Some("synthetic failure".to_string()),
        topic_detected: None,
        token_count: 1,
    };
    let mut engine = state.dream_engine.lock().unwrap();
    engine.scheduler.priority_queue.clear();
    nexus_kernel::dreams::queue_dreams_from_interaction(
        &mut engine.scheduler,
        turn,
        &consciousness,
        &interaction,
    );
    engine
        .scheduler
        .priority_queue
        .iter()
        .map(|t| {
            let (principal, origin) =
                t.label()
                    .map_or(("none".to_string(), "none".to_string()), |l| {
                        (
                            match l.principal() {
                                nexus_kernel::hm::Principal::Agent(id) => id.to_string(),
                                other => format!("{other:?}"),
                            },
                            l.label().class_name().to_string(),
                        )
                    });
            format!("{}|{principal}|{origin}", t.source_agent)
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// A-T1 (production). A team whose director is unrestricted and one member
/// local-only: no member dispatch is non-local, every member dispatch was
/// admitted (one admission row per member task), and the director's merge
/// (whose taint joins the members') is local.
#[test]
fn p3_hm_a_t1_team_member_local_only_director_unrestricted() {
    let name = "p3_hm_a_t1_team_member_local_only_director_unrestricted";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let director = register_by_supervisor(&state, "p3-d2r2-director", false);
        let members = vec![
            register_by_supervisor(&state, "p3-d2r2-member-a", true),
            register_by_supervisor(&state, "p3-d2r2-member-b", false),
        ];
        route_unrestricted_work_through_the_proxy();
        let result =
            crate::commands::orchestration::run_team_workflow(&state, &director, PROMPT, &members);
        let admitted: Vec<String> = crate::hm_m3_tests::admissions(&state.db)
            .into_iter()
            .map(|a| {
                format!(
                    "{}:{}:{}:{}",
                    a.agent_id.unwrap_or_default(),
                    a.unit_kind,
                    a.route_class,
                    a.outcome
                )
            })
            .collect();
        Ok(format!(
            "director={director} members={} admitted={} result={result:?}",
            members.join(","),
            admitted.join(";")
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    let member_tasks: Vec<&String> = w
        .ollama
        .iter()
        .filter(|r| r.contains("Execute this task assigned by the Director"))
        .collect();
    assert_eq!(
        member_tasks.len(),
        2,
        "A-T1: precondition: both member tasks ran locally: {:?}",
        w.ollama
    );
    let proxied_member = w
        .proxy
        .iter()
        .filter(|l| l.contains("Execute this task assigned by the Director"))
        .count();
    assert_eq!(
        proxied_member, 0,
        "A-T1: a member dispatch left the machine: {:?}",
        w.proxy
    );
    let merges_proxied = w
        .proxy
        .iter()
        .filter(|l| l.contains("Combine these sub-task results"))
        .count();
    assert_eq!(
        merges_proxied, 0,
        "A-T1: the merge left the machine: {:?}",
        w.proxy
    );
    // Every member dispatch was admitted: one local admission row of a
    // member for each member task the witness saw (the director's rows are
    // its decomposition and its merge).
    let admitted = hm_field(&w.result, "admitted=");
    let director = hm_field(&w.result, "director=");
    let members: Vec<String> = hm_field(&w.result, "members=")
        .split(',')
        .map(str::to_string)
        .collect();
    let member_rows: Vec<&str> = admitted
        .split(';')
        .filter(|row| members.iter().any(|m| row.starts_with(&format!("{m}:"))))
        .collect();
    assert_eq!(
        member_rows.len(),
        member_tasks.len(),
        "A-T1: a member dispatch was not admitted in the issuer: {admitted}"
    );
    assert!(
        member_rows.iter().all(|r| r.contains(":team_task:local:")),
        "A-T1: a member admission is not local: {admitted}"
    );
    let director_rows: Vec<&str> = admitted
        .split(';')
        .filter(|row| row.starts_with(&format!("{director}:")))
        .collect();
    assert!(
        !director_rows.is_empty() && director_rows.iter().all(|r| r.contains(":local:")),
        "A-T1: the director's dispatches (decomposition, merge) are not local: {admitted}"
    );
}

/// A-T1h (production). `temporal_fork` with a caller id naming a
/// local-only agent sends no non-local request; with an unknown id the
/// request is OwnerDirect with the default projection. The projection type
/// has no text field: it is exactly two `f64` drives.
#[test]
fn p3_hm_a_t1h_temporal_fork_follows_the_registered_caller() {
    assert_eq!(
        std::mem::size_of::<nexus_kernel::temporal::DriveProjection>(),
        2 * std::mem::size_of::<f64>(),
        "A-T1h: the drive projection gained a field"
    );
    let name = "p3_hm_a_t1h_temporal_fork_follows_the_registered_caller";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        route_unrestricted_work_through_the_proxy();
        let local = crate::commands::simulation::temporal_fork(
            &state,
            format!("Plan the synthetic note {HM}"),
            agent.clone(),
            Some(2),
        );
        eprintln!("HM-A-T1H PHASE-BOUNDARY");
        let unknown = crate::commands::simulation::temporal_fork(
            &state,
            "Plan an unrelated task".to_string(),
            "not-a-registered-agent".to_string(),
            Some(2),
        );
        Ok(format!(
            "local={} unknown={}",
            local.is_ok(),
            unknown.is_ok()
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    let carried: Vec<&String> = w.proxy.iter().filter(|l| l.contains(HM)).collect();
    assert!(
        carried.is_empty(),
        "A-T1h: a temporal fork of a local-only caller left the machine: {carried:?}"
    );
    assert!(
        w.ollama.iter().any(|r| r.contains(HM)),
        "A-T1h: precondition: the local-only caller's fork did not run locally: {:?}",
        w.ollama
    );
    // A local-only caller cannot leave the machine, so every request the
    // proxy saw is the unknown caller's: an id that names no registered
    // agent is no principal (an unregistered agent unit is refused), so its
    // request is OwnerDirect, admitted on the configured (non-local) route.
    assert!(
        !w.proxy.is_empty(),
        "A-T1h: the unknown caller's OwnerDirect request did not take the configured route: {:?}",
        w.proxy
    );
}

/// P3-C11-HM-M4 N1 C-3 (P3-HM-CONTRACT-02 §12.4 steps 1 and 5; §5.1: the
/// admission row's agent is NULL for an OwnerDirect admission). A temporal
/// fork whose caller names no registered agent is an `OwnerDirect` request
/// of class HB: each of its dispatches writes one admission row (agent
/// NULL, the request's unit kind, outcome updated as for HR) and its
/// `memory.dispatch_admitted` event. HD, SY and XS requests write none.
#[test]
fn p3_hm_m4_c3_an_owner_direct_temporal_fork_records_each_admission() {
    let name = "p3_hm_m4_c3_an_owner_direct_temporal_fork_records_each_admission";
    let marker = "P3-HM-M4-SYNTH-C3-2b6f";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        route_unrestricted_work_through_the_proxy();
        // A human request with no model takes the selected provider: here
        // the plain Ollama provider on the configured route, whose requests
        // the recording proxy reads and answers (a cloud provider's TLS
        // request would hide its content and get no answer).
        std::env::set_var("LLM_PROVIDER", "ollama");
        let fork = crate::commands::simulation::temporal_fork(
            &state,
            format!("Plan the synthetic task {marker}"),
            "not-a-registered-agent".to_string(),
            Some(2),
        );
        let rows: Vec<String> = crate::hm_m3_tests::admissions(&state.db)
            .into_iter()
            .map(|a| {
                format!(
                    "{}|{}|{}|{}",
                    a.agent_id.as_deref().unwrap_or("NULL"),
                    a.unit_kind,
                    a.route_class,
                    a.outcome
                )
            })
            .collect();
        let fork_events = crate::hm_m3_tests::event_kinds(&state.db)
            .into_iter()
            .filter(|(kind, agent)| kind == "memory.dispatch_admitted" && agent.is_none())
            .count();
        eprintln!("HM-M4-C3 PHASE-BOUNDARY");
        // HD: an Owner's chat turn with no agent, and an Owner's request.
        let hd_turn = hm_turn(
            &state,
            crate::hm_dispatch::chat_turn(&state, None).map_err(AgentError::SupervisorError)?,
            "An HD chat turn",
            None,
        )
        .is_ok();
        let hd_request = hm_turn(
            &state,
            crate::hm_dispatch::request_unit(&state, nexus_kernel::hm::Principal::OwnerDirect)
                .map_err(AgentError::SupervisorError)?,
            "An HD request",
            None,
        )
        .is_ok();
        // SY: the fixed connection test.
        let sy = crate::commands::chat_llm::test_llm_connection(&state, "ollama".to_string())
            .map(|r| r.success)
            .unwrap_or(false);
        // XS: a provider handed to an excluded subsystem.
        let xs = {
            let config = nexus_kernel::config::load_config().unwrap();
            let (provider, model) = crate::provider_from_prefixed_model(
                "ollama/qwen3:8b",
                &crate::build_provider_config(&config),
            )
            .map_err(AgentError::SupervisorError)?;
            crate::hm_dispatch::IssuedProvider::new(&state, "p3-hm-m4-xs", provider)
                .query("An XS request", 64, &model)
                .is_ok()
        };
        let after = crate::hm_m3_tests::admissions(&state.db).len();
        Ok(format!(
            "fork_ok={} rows={} fork_events={fork_events} hd_turn={hd_turn} hd_request={hd_request} sy={sy} xs={xs} after={after}",
            fork.is_ok(),
            rows.join(";")
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    let rows = hm_field(&w.result, "rows=");
    let rows: Vec<&str> = rows.split(';').filter(|r| !r.is_empty()).collect();
    let fork_requests = w.proxy.iter().filter(|l| l.contains(marker)).count();
    assert!(
        fork_requests >= 1,
        "C-3: precondition: the fork's request did not take the configured route: {:?}",
        w.proxy
    );
    assert!(
        !rows.is_empty(),
        "C-3: an OwnerDirect temporal fork (HB) wrote no admission row: {}",
        w.result
    );
    assert!(
        rows.iter()
            .all(|r| *r == "NULL|request|non_local|reply_used"),
        "C-3: an HB admission row is not (agent NULL, request, non_local, reply_used): {rows:?}"
    );
    // Every proxied request other than the four HD, SY and XS prompts is a
    // dispatch of the fork: each has exactly one admission row.
    let others = [
        "An HD chat turn",
        "An HD request",
        "Reply with exactly: ok",
        "An XS request",
    ];
    let fork_dispatches = w
        .proxy
        .iter()
        .filter(|l| {
            l.contains("/api/generate")
                && !others
                    .iter()
                    .any(|o| l.contains(&format!("\"prompt\":\"{o}\"")))
        })
        .count();
    assert_eq!(
        rows.len(),
        fork_dispatches,
        "C-3: the fork's {fork_dispatches} dispatches do not have one admission row each: {rows:?}"
    );
    assert_eq!(
        hm_field(&w.result, "fork_events="),
        rows.len().to_string(),
        "C-3: an HB admission has no memory.dispatch_admitted event: {}",
        w.result
    );
    for (key, what) in [
        ("hd_turn=", "HD chat turn"),
        ("hd_request=", "HD request"),
        ("sy=", "SY request"),
        ("xs=", "XS request"),
    ] {
        assert_eq!(
            hm_field(&w.result, key),
            "true",
            "C-3: precondition: the {what} did not complete: {}",
            w.result
        );
    }
    assert_eq!(
        hm_field(&w.result, "after="),
        rows.len().to_string(),
        "C-3: an HD, SY or XS request wrote an admission row: {}",
        w.result
    );
}

/// A-C1 (production; mutant 8). Work whose policy cannot be established
/// dispatches nothing (the default policy is `Unresolved`), and a local-only
/// unit's dispatch takes the attested local transport and completes there.
#[test]
fn p3_hm_a_c1_unresolved_default_and_local_unit_dispatch_locally() {
    assert_eq!(AgentLlmPolicy::default(), AgentLlmPolicy::Unresolved);
    let name = "p3_hm_a_c1_unresolved_default_and_local_unit_dispatch_locally";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        route_unrestricted_work_through_the_proxy();
        let unregistered = crate::hm_wiring::test_support::plan_text(
            &state,
            &Uuid::new_v4().to_string(),
            "P3 HM unresolved",
        );
        let local = crate::hm_wiring::test_support::plan_text(&state, &agent, "P3 HM local unit");
        Ok(format!(
            "unregistered_ok={} local={local:?}",
            unregistered.is_ok()
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert!(
        w.result.contains("unregistered_ok=false "),
        "A-C1: an unestablished policy dispatched: {}",
        w.result
    );
    assert!(
        !w.ollama.iter().any(|r| r.contains("P3 HM unresolved"))
            && !w.proxy.iter().any(|l| l.contains("P3 HM unresolved")),
        "A-C1: an unestablished policy reached a model"
    );
    assert!(
        w.result.contains("local=Ok(") && w.ollama.iter().any(|r| r.contains("P3 HM local unit")),
        "A-C1: the local-only unit did not dispatch on the attested local transport: {} {:?}",
        w.result,
        w.ollama
    );
    assert!(w.proxy.is_empty(), "A-C1: {:?}", w.proxy);
}

// ── N6: the dispatch generation checks, Flash and the L1 proof through the
// production adapters (§18.2 A-P1, A-P2, A-P3, A-F1, A-F3; A1.6), with the
// loopback runtime and the recording proxy as the injected transports ──

/// One planning dispatch of backend `text` in the existing unit `ctx`,
/// through the production adapter of a goal's loop.
fn hm_plan_in(
    state: &AppState,
    ctx: &mut nexus_kernel::hm::DispatchContext,
    text: &str,
) -> Result<String, AgentError> {
    crate::hm_dispatch::GatewayPlannerLlm::for_loop(state, None)
        .plan_query(ctx, &mut |c: &mut nexus_kernel::hm::DispatchContext| {
            let mut b = nexus_kernel::hm::PromptBuilder::new(c);
            b.text(text);
            Some(b.build())
        })
        .map(|reply| reply.text().to_string())
}

/// `(local_only, policy_generation)` of `agent`'s stored policy row.
fn hm_policy(state: &AppState, agent: &str) -> (i64, i64) {
    crate::hm_m3_tests::policies(&state.db)
        .into_iter()
        .find(|p| p.agent_id == agent)
        .map(|p| {
            (
                p.local_only.unwrap_or(-1),
                p.policy_generation.unwrap_or(-1),
            )
        })
        .unwrap_or((-1, -1))
}

/// The configuration names `agent` local-only (an out-of-band edit).
fn hm_config_local_only(agent: &str, local_only: bool) {
    let mut config = nexus_kernel::config::load_config().unwrap();
    config
        .agent_llm_assignments
        .get_mut(agent)
        .unwrap()
        .local_only = local_only;
    nexus_kernel::config::save_config(&config).unwrap();
}

/// A-P1 (i of three) and A-P3 (ii). The configuration tightens the agent
/// after its unit started (O1), before its dispatch: at the dispatch's
/// observation point (O2) the tightening is committed with a generation
/// bump, the dispatch is rebuilt local-only, and nothing is sent to a
/// non-local transport.
#[test]
fn p3_hm_a_p1_tightened_before_admission_sends_nothing_non_local() {
    let name = "p3_hm_a_p1_tightened_before_admission_sends_nothing_non_local";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", false, ""));
        route_unrestricted_work_through_the_proxy();
        let mut ctx = crate::hm_wiring::test_support::goal_unit(&state, &agent)?;
        let before = hm_policy(&state, &agent);
        hm_config_local_only(&agent, true);
        let reply = hm_plan_in(&state, &mut ctx, "P3 HM A-P1 before");
        let after = hm_policy(&state, &agent);
        Ok(format!(
            "before={}:{} after={}:{} reply={reply:?}",
            before.0, before.1, after.0, after.1
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    // The first build selected a non-local transport (its selection may
    // probe the configured runtime's model list); no request was sent on
    // it: no proxied request is a dispatch or carries the prompt.
    let sent: Vec<&String> = w
        .proxy
        .iter()
        .filter(|l| l.starts_with("POST ") || l.contains("P3 HM A-P1 before"))
        .collect();
    assert!(
        sent.is_empty(),
        "A-P1: a non-local request was sent for a dispatch tightened before admission: {sent:?}"
    );
    assert!(
        w.result.contains("reply=Ok(")
            && w.generates()
                .iter()
                .any(|r| r.contains("P3 HM A-P1 before")),
        "A-P1: the tightened dispatch was not rebuilt on the local transport: {} {:?}",
        w.result,
        w.ollama
    );
    let before = hm_field(&w.result, "before=");
    let after = hm_field(&w.result, "after=");
    let parse = |s: &str| -> (i64, i64) {
        let (a, b) = s.split_once(':').unwrap();
        (a.parse().unwrap(), b.parse().unwrap())
    };
    let (before, after) = (parse(&before), parse(&after));
    assert_eq!(before.0, 0, "A-P3 (ii): precondition: {}", w.result);
    assert!(
        after.0 == 1 && after.1 > before.1,
        "A-P3 (ii): the tightening was not committed with a generation bump: {}",
        w.result
    );
}

fn is_p1_after(body: &str) -> bool {
    body.contains("P3 HM A-P1 after")
}

/// A-P1 (ii of three) and A-P3 (i, iii). The configuration tightens the
/// agent after its non-local dispatch was admitted, before the reply: the
/// admitted request was sent (no interruption is claimed), and the reply is
/// discarded at acceptance (O3), recorded `reply_discarded`, with
/// `memory.reply_discarded` emitted.
#[test]
fn p3_hm_a_p1_tightened_after_admission_discards_the_reply() {
    let name = "p3_hm_a_p1_tightened_after_admission_discards_the_reply";
    let mut serves = hm_route();
    serves.tighten = Some(Tighten {
        name: "p3-hm-agent",
        matches: is_p1_after,
        nth: 1,
    });
    let Some(w) = e2e(name, serves, || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", false, ""));
        route_unrestricted_work_through_the_proxy();
        let mut ctx = crate::hm_wiring::test_support::goal_unit(&state, &agent)?;
        let reply = hm_plan_in(&state, &mut ctx, "P3 HM A-P1 after");
        let outcomes: Vec<String> = crate::hm_m3_tests::admissions(&state.db)
            .into_iter()
            .map(|a| format!("{}:{}", a.route_class, a.outcome))
            .collect();
        let discarded = crate::hm_m3_tests::event_kinds(&state.db)
            .iter()
            .filter(|(kind, _)| kind == "memory.reply_discarded")
            .count();
        Ok(format!(
            "outcomes={} discarded_events={discarded} reply_ok={}",
            outcomes.join(","),
            reply.is_ok()
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert_eq!(
        w.proxied(is_p1_after),
        1,
        "A-P3 (i): the admitted request was not sent: {:?}",
        w.proxy
    );
    assert!(
        w.result.contains("reply_ok=false"),
        "A-P1: a reply arriving after the tightening was used: {}",
        w.result
    );
    assert_eq!(
        hm_field(&w.result, "outcomes="),
        "non_local:reply_discarded",
        "A-P1: the discarded reply was not recorded: {}",
        w.result
    );
    assert!(
        !w.result.contains("discarded_events=0 "),
        "A-P1: no memory.reply_discarded: {}",
        w.result
    );
}

/// A-P1 (iii of three). The policy generation moves at admission, and
/// again at the one rebuild: the dispatch is refused `generation_unstable`
/// and nothing is sent anywhere.
#[test]
fn p3_hm_a_p1_generation_moved_twice_is_unstable_and_sends_nothing() {
    let name = "p3_hm_a_p1_generation_moved_twice_is_unstable_and_sends_nothing";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", false, ""));
        route_unrestricted_work_through_the_proxy();
        let mut ctx = crate::hm_wiring::test_support::goal_unit(&state, &agent)?;
        let mut built = 0;
        let reply = crate::hm_dispatch::GatewayPlannerLlm::for_loop(&state, None).plan_query(
            &mut ctx,
            &mut |c: &mut nexus_kernel::hm::DispatchContext| {
                // Each build of the prompt sees the generation moved since
                // the unit's context was fixed (an Owner change, TEST
                // FIXTURE).
                built += 1;
                crate::hm_m3_tests::fixture_owner_loosen(&state.db, &agent);
                let mut b = nexus_kernel::hm::PromptBuilder::new(c);
                b.text("P3 HM A-P1 unstable");
                Some(b.build())
            },
        );
        Ok(format!(
            "built={built} reply={:?}",
            reply.map(|r| r.text().to_string())
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert!(
        w.result.contains("built=2 ") && w.result.contains("generation_unstable"),
        "A-P1: the twice-moved generation was not refused after one rebuild: {}",
        w.result
    );
    assert!(
        !w.proxy.iter().any(|l| l.contains("P3 HM A-P1 unstable"))
            && !w.ollama.iter().any(|r| r.contains("P3 HM A-P1 unstable")),
        "A-P1: an unstable dispatch was sent: {:?} {:?}",
        w.proxy,
        w.ollama
    );
}

fn is_p3_o3(body: &str) -> bool {
    body.contains("P3 HM A-P3 o3")
}

/// A-P3 (iv). An unreadable configuration at the dispatch's observation
/// point (O2) refuses it before anything is sent; one that becomes
/// unreadable while a non-local reply is in flight discards the reply at
/// acceptance (O3).
#[test]
fn p3_hm_a_p3_unreadable_config_refuses_at_o2_and_discards_at_o3() {
    let name = "p3_hm_a_p3_unreadable_config_refuses_at_o2_and_discards_at_o3";
    let mut serves = hm_route();
    serves.break_config = Some(Tighten {
        name: "",
        matches: is_p3_o3,
        nth: 1,
    });
    let Some(w) = e2e(name, serves, || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", false, ""));
        route_unrestricted_work_through_the_proxy();
        let path = std::env::var("NEXUS_CONFIG_PATH").unwrap();
        let readable = std::fs::read_to_string(&path).unwrap();
        // O2: unreadable before the dispatch.
        let mut ctx = crate::hm_wiring::test_support::goal_unit(&state, &agent)?;
        std::fs::write(&path, "local_only = [").unwrap();
        let o2 = hm_plan_in(&state, &mut ctx, "P3 HM A-P3 o2");
        std::fs::write(&path, &readable).unwrap();
        eprintln!("HM-A-P3 PHASE-BOUNDARY");
        // O3: unreadable while the reply is in flight (the proxy breaks it).
        let mut ctx = crate::hm_wiring::test_support::goal_unit(&state, &agent)?;
        let o3 = hm_plan_in(&state, &mut ctx, "P3 HM A-P3 o3");
        let outcomes: Vec<String> = crate::hm_m3_tests::admissions(&state.db)
            .into_iter()
            .map(|a| format!("{}:{}", a.route_class, a.outcome))
            .collect();
        Ok(format!(
            "o2_ok={} o3_ok={} outcomes={}",
            o2.is_ok(),
            o3.is_ok(),
            outcomes.join(",")
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert!(
        w.result.contains("o2_ok=false")
            && !w.proxy.iter().any(|l| l.contains("P3 HM A-P3 o2"))
            && !w.ollama.iter().any(|r| r.contains("P3 HM A-P3 o2")),
        "A-P3 (iv): an unreadable configuration at O2 did not refuse before sending: {} {:?}",
        w.result,
        w.proxy
    );
    assert_eq!(
        w.proxied(is_p3_o3),
        1,
        "A-P3 (iv): precondition: the O3 dispatch was not sent: {:?}",
        w.proxy
    );
    assert!(
        w.result.contains("o3_ok=false")
            && hm_field(&w.result, "outcomes=") == "non_local:reply_discarded",
        "A-P3 (iv): an unreadable configuration at O3 did not discard the reply: {}",
        w.result
    );
}

/// A-P3 (v; mutant 9). A configuration edit to `local_only = false` never
/// loosens, at any observation point: the agent stays local-only, its
/// generation does not move, and its dispatch stays local.
#[test]
fn p3_hm_a_p3_a_config_loosening_never_loosens() {
    let name = "p3_hm_a_p3_a_config_loosening_never_loosens";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        route_unrestricted_work_through_the_proxy();
        let _ = issuer_policy(&state, &agent);
        let before = hm_policy(&state, &agent);
        hm_config_local_only(&agent, false);
        let mut ctx = crate::hm_wiring::test_support::goal_unit(&state, &agent)?;
        let reply = hm_plan_in(&state, &mut ctx, "P3 HM A-P3 v");
        let policy = issuer_policy(&state, &agent);
        let after = hm_policy(&state, &agent);
        Ok(format!(
            "policy={policy:?} before={before:?} after={after:?} reply={reply:?}"
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert!(
        w.result.contains("policy=LocalOnly "),
        "A-P3 (v): a configuration edit loosened the agent: {}",
        w.result
    );
    assert_eq!(
        hm_field(&w.result, "before="),
        hm_field(&w.result, "after="),
        "A-P3 (v): the configuration edit changed the stored policy: {}",
        w.result
    );
    assert!(
        w.proxy.is_empty() && w.generates().iter().any(|r| r.contains("P3 HM A-P3 v")),
        "A-P3 (v): the dispatch left the local route: {:?}",
        w.proxy
    );
}

/// A Flash provider of the backend-owned cache over `model` (with
/// in-process inference compiled in; not loaded).
fn hm_flash(state: &AppState, model: &std::path::Path) {
    let model = model.to_string_lossy().into_owned();
    let provider = nexus_connectors_llm::providers::FlashProvider::new(
        model.clone(),
        nexus_flash_infer::LoadConfig {
            model_path: model,
            ..Default::default()
        },
        nexus_flash_infer::GenerationConfig::fast(),
    );
    state
        .flash_providers
        .lock()
        .unwrap()
        .insert("p3-hm-flash-session".to_string(), Arc::new(provider));
}

/// The agent's assignment names a Flash route.
fn hm_flash_route(agent: &str) {
    let mut config = nexus_kernel::config::load_config().unwrap();
    config
        .agent_llm_assignments
        .get_mut(agent)
        .unwrap()
        .provider_id = "flash/synthetic".to_string();
    nexus_kernel::config::save_config(&config).unwrap();
}

/// A-P2. A local-only agent's route selects the cached Flash provider,
/// whose (L2) attestation is incomplete (its model is not in the model
/// directory): the transport's attested class (non-local) differs from the
/// unit's (local), and the dispatch is refused; nothing is sent.
#[test]
fn p3_hm_a_p2_flash_override_without_l2_is_refused_for_a_local_unit() {
    let name = "p3_hm_a_p2_flash_override_without_l2_is_refused_for_a_local_unit";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        let dir = std::env::temp_dir().join(format!("p3-hm-a-p2-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let model = dir.join("synthetic.gguf");
        std::fs::write(&model, b"SYNTH").unwrap();
        hm_flash(&state, &model);
        hm_flash_route(&agent);
        let reply = crate::hm_wiring::test_support::plan_text(&state, &agent, "P3 HM A-P2");
        let admissions = crate::hm_m3_tests::admissions(&state.db).len();
        std::fs::remove_dir_all(&dir).unwrap();
        Ok(format!("admissions={admissions} reply={reply:?}"))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert!(
        w.result.contains("reply=Err(") && w.result.contains("route_mismatch"),
        "A-P2: a non-local Flash transport was accepted for a local unit: {}",
        w.result
    );
    assert!(
        w.result.contains("admissions=0 "),
        "A-P2: a refused dispatch was admitted: {}",
        w.result
    );
    assert!(
        w.proxy.is_empty() && w.generates().is_empty(),
        "A-P2: another transport was used: {:?} {:?}",
        w.proxy,
        w.ollama
    );
}

/// A-F3 (A1.7). A local-only agent's Flash route with no model loaded is
/// an error, and no other provider is called.
#[test]
fn p3_hm_a_f3_flash_route_without_a_loaded_model_errors_and_calls_nothing() {
    let name = "p3_hm_a_f3_flash_route_without_a_loaded_model_errors_and_calls_nothing";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        hm_flash_route(&agent);
        let reply = crate::hm_wiring::test_support::plan_text(&state, &agent, "P3 HM A-F3");
        Ok(format!("reply={reply:?}"))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert!(
        w.result.contains("reply=Err(") && w.result.contains("no model is loaded"),
        "A-F3: {}",
        w.result
    );
    assert!(
        w.proxy.is_empty() && w.generates().is_empty(),
        "A-F3: another provider was called: {:?} {:?}",
        w.proxy,
        w.ollama
    );
}

/// A-F1 (local-first preserved; no real model). A local-only agent whose
/// route is an (L2)-attested Flash provider (its model a regular file in
/// the identity home's model directory) has its planning admitted on the
/// local route with its history included; the dispatch goes to Flash and
/// to no other provider. (The synthetic model cannot be loaded, so the
/// transport fails after admission: `transport_failed`.)
#[test]
fn p3_hm_a_f1_attested_flash_admits_a_local_only_agent_with_its_history() {
    let name = "p3_hm_a_f1_attested_flash_admits_a_local_only_agent_with_its_history";
    let Some(w) = e2e(name, hm_route(), || {
        // A test-owned identity home for this child process.
        let home = std::env::temp_dir().join(format!("p3-hm-a-f1-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        let models = nexus_connectors_llm::attest::flash_model_dir_for_home(&home).unwrap();
        std::fs::create_dir_all(&models).unwrap();
        let model = models.join("synthetic.gguf");
        std::fs::write(&model, b"SYNTH").unwrap();
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        hm_flash(&state, &model);
        hm_flash_route(&agent);
        hm_seed(&state, &agent, "");
        let run = run_own_goal(&state, &agent, "Summarize the synthetic note again");
        let admitted: Vec<String> = crate::hm_m3_tests::admissions(&state.db)
            .into_iter()
            .map(|a| format!("{}:{}:{}", a.route_class, a.record_count, a.outcome))
            .collect();
        std::fs::remove_dir_all(&home).unwrap();
        Ok(format!("admitted={} run={run:?}", admitted.join(",")))
    }) else {
        return;
    };
    hm_dump(name, &w);
    let admitted = hm_field(&w.result, "admitted=");
    let first = admitted.split(',').next().unwrap_or_default();
    let parts: Vec<&str> = first.split(':').collect();
    assert!(
        parts.len() == 3
            && parts[0] == "local"
            && parts[1].parse::<i64>().unwrap_or(0) >= 1
            && parts[2] == "transport_failed",
        "A-F1: the planning was not admitted on the attested local Flash with its history: {}",
        w.result
    );
    assert!(
        w.proxy.is_empty() && w.generates().is_empty(),
        "A-F1: another provider was called: {:?} {:?}",
        w.proxy,
        w.ollama
    );
}

/// A1.6 (mutant 16). The (L1) proof is made for each dispatch: after the
/// loopback runtime starts reporting the model as remote, the next dispatch
/// of a local-only agent is refused, and nothing more is sent to it.
#[test]
fn p3_hm_a1_6_the_l1_proof_is_made_for_each_dispatch() {
    let name = "p3_hm_a1_6_the_l1_proof_is_made_for_each_dispatch";
    let mut serves = FakeOllama::serving(&["qwen3:8b"]);
    serves.remote_after_generates = Some(1);
    let Some(w) = e2e(name, serves, || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        let one = crate::hm_wiring::test_support::plan_text(&state, &agent, "P3 HM L1 one");
        let two = crate::hm_wiring::test_support::plan_text(&state, &agent, "P3 HM L1 two");
        Ok(format!("one_ok={} two_ok={}", one.is_ok(), two.is_ok()))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert!(
        w.result.contains("one_ok=true")
            && w.generates()
                .iter()
                .filter(|r| r.contains("P3 HM L1 one"))
                .count()
                == 1,
        "A1.6: precondition: the first dispatch did not run on the verified runtime: {} {:?}",
        w.result,
        w.ollama
    );
    assert!(
        w.result.contains("two_ok=false")
            && !w.generates().iter().any(|r| r.contains("P3 HM L1 two")),
        "A1.6: a dispatch used a proof made before the runtime turned remote: {} {:?}",
        w.result,
        w.ollama
    );
    assert!(w.proxy.is_empty(), "A1.6: {:?}", w.proxy);
}

/// §12.3 item 4, the `model_mapping` read site. A model mapping the agent
/// stored while local-only (LOCAL_ONLY) never selects a non-local route for
/// a later non-local unit: after the Owner loosens it, its planning takes
/// the configured route, never the mapped cloud model.
#[test]
fn p3_hm_s12_3_local_only_model_mapping_never_selects_a_non_local_route() {
    let name = "p3_hm_s12_3_local_only_model_mapping_never_selects_a_non_local_route";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-agent", true, ""));
        route_unrestricted_work_through_the_proxy();
        // The desktop executor closes SelectLlmProvider (L6), so the row is
        // written in a unit of the local-only agent through the labelled
        // writer the L6 actuators use (§6.5 `model_mapping`).
        let ctx = crate::hm_wiring::test_support::goal_unit(&state, &agent)?;
        let mapped = state.hm_issuer().unwrap().write_memory(
            &ctx,
            &nexus_kernel::hm::NewMemory {
                source_kind: nexus_kernel::hm::SourceKind::ModelMapping,
                memory_type: "model_mapping",
                key: "model_mapping:planning",
                value_json: r#"{"provider":"groq","model":"llama-3.3-70b-versatile"}"#,
                relevance_score: 1.0,
            },
            &[],
        );
        let mapping = hm_origins(&state, &agent, "agent_memory", "model_mapping");
        eprintln!("HM-MAPPING PHASE-BOUNDARY");
        hm_loosen(&state, &agent);
        let later = run_own_goal(&state, &agent, "Summarize the synthetic note again");
        Ok(format!(
            "mapping={} mapped={} later={later:?}",
            mapping.join(","),
            mapped.is_ok()
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    assert_eq!(
        hm_field(&w.result, "mapping="),
        "LOCAL_ONLY_ORIGIN",
        "model_mapping: precondition: the local-only goal stored no LOCAL_ONLY mapping: {}",
        w.result
    );
    assert!(
        w.proxied(is_plan) >= 1,
        "model_mapping: precondition: the later goal did not plan on the configured route: {:?}",
        w.proxy
    );
    let mapped: Vec<&String> = w.proxy.iter().filter(|l| l.contains("groq")).collect();
    assert!(
        mapped.is_empty(),
        "model_mapping: a LOCAL_ONLY mapping selected a non-local route: {mapped:?}"
    );
}

// ── P3-C11-HM-M4 N2: the governed policy change (§4.3–§4.6, §15.3, §16) ─────

/// `local_only|policy_generation|basis` of `agent`'s policy row, read
/// through the store without an observation point.
fn m4_policy(state: &AppState, agent: &str) -> String {
    crate::hm_m3_tests::policies(&state.db)
        .into_iter()
        .find(|p| p.agent_id == agent)
        .map(|p| {
            format!(
                "{}|{}|{}",
                p.local_only.unwrap_or(-1),
                p.policy_generation.unwrap_or(-1),
                p.basis.unwrap_or_default()
            )
        })
        .unwrap_or_else(|| "none".to_string())
}

/// How many authority events of `kind` describe `agent`.
fn m4_events(state: &AppState, agent: &str, kind: &str) -> usize {
    crate::hm_m3_tests::event_kinds(&state.db)
        .into_iter()
        .filter(|(k, a)| k == kind && a.as_deref() == Some(agent))
        .count()
}

/// `provider|local_only|budget_dollars|budget_tokens` of the assignment
/// under `key` in the scenario's configuration.
fn m4_assignment(key: &str) -> String {
    nexus_kernel::config::load_config()
        .unwrap()
        .agent_llm_assignments
        .get(key)
        .map(|a| {
            format!(
                "{}|{}|{}|{}",
                a.provider_id, a.local_only, a.budget_dollars, a.budget_tokens
            )
        })
        .unwrap_or_else(|| "none".to_string())
}

/// The policy row of a registered agent, created restrictive by its first
/// resolution.
fn m4_resolve(state: &AppState, agent: &str) {
    state
        .hm_issuer()
        .unwrap()
        .resolve_agent_route_policy(Uuid::parse_str(agent).unwrap())
        .unwrap();
}

/// N2 (§4.4, §4.5 row 1, §7.1): a webview `local_only = true` is a governed
/// tighten: the store commits it first (local-only, generation + 1, basis
/// `tightened`, its `memory.policy_tightened` event), then the config mirror
/// is written; the provider and budgets are saved as before.
#[test]
fn p3_hm_m4_n2_a_webview_tightening_commits_in_the_store_first() {
    let name = "p3_hm_m4_n2_a_webview_tightening_commits_in_the_store_first";
    let Some(w) = e2e(name, FakeOllama::serving(&[]), || {
        let state = AppState::new_in_memory();
        let agent = register_by_supervisor(&state, "p3-hm-m4-open", false);
        let before = m4_policy(&state, &agent);
        let set = crate::commands::chat_llm::set_agent_llm_provider(
            &state,
            None,
            agent.clone(),
            "anthropic".to_string(),
            true,
            3,
            4,
        );
        // Read at once: no observation point runs in between.
        let after = m4_policy(&state, &agent);
        Ok(format!(
            "before={before} set_ok={} after={after} tightened={} assignment={}",
            set.is_ok(),
            m4_events(&state, &agent, "memory.policy_tightened"),
            m4_assignment(&agent)
        ))
    }) else {
        return;
    };
    assert_eq!(
        hm_field(&w.result, "before="),
        "0|2|owner_confirmed",
        "N2: precondition: the agent is unrestricted: {}",
        w.result
    );
    assert_eq!(
        hm_field(&w.result, "after="),
        "1|3|tightened",
        "N2: the webview tightening was not committed in the store first: {}",
        w.result
    );
    assert_eq!(hm_field(&w.result, "tightened="), "1", "{}", w.result);
    assert_eq!(hm_field(&w.result, "set_ok="), "true", "{}", w.result);
    assert_eq!(
        hm_field(&w.result, "assignment="),
        "anthropic|true|3|4",
        "{}",
        w.result
    );
}

/// N2 (§4.4, §16 "Webview local_only:false", "No native confirmer"): a
/// webview `local_only = false` without a native confirmation loosens
/// nothing: it is refused and reported, no store row changes, and the
/// config keeps `local_only = true` (the provider and budgets are saved).
#[test]
fn p3_hm_m4_n2_a_webview_loosening_without_confirmation_loosens_nothing() {
    let name = "p3_hm_m4_n2_a_webview_loosening_without_confirmation_loosens_nothing";
    let Some(w) = e2e(name, FakeOllama::serving(&[]), || {
        let state = AppState::new_in_memory();
        let agent = register_by_supervisor(&state, "p3-hm-m4-local", true);
        m4_resolve(&state, &agent);
        let before = m4_policy(&state, &agent);
        let set = crate::commands::chat_llm::set_agent_llm_provider(
            &state,
            None,
            agent.clone(),
            "anthropic".to_string(),
            false,
            3,
            4,
        );
        Ok(format!(
            "before={before} set_ok={} after={} loosened={} assignment={}",
            set.is_ok(),
            m4_policy(&state, &agent),
            m4_events(&state, &agent, "memory.policy_loosened"),
            m4_assignment(&agent)
        ))
    }) else {
        return;
    };
    assert_eq!(
        hm_field(&w.result, "before="),
        "1|1|default_restrictive",
        "{}",
        w.result
    );
    assert_eq!(
        hm_field(&w.result, "set_ok="),
        "false",
        "N2: an unconfirmed loosening was not refused: {}",
        w.result
    );
    assert_eq!(
        hm_field(&w.result, "after="),
        hm_field(&w.result, "before="),
        "N2: a store row changed: {}",
        w.result
    );
    assert_eq!(hm_field(&w.result, "loosened="), "0", "{}", w.result);
    assert_eq!(
        hm_field(&w.result, "assignment="),
        "anthropic|true|3|4",
        "N2: the webview argument loosened the config mirror: {}",
        w.result
    );
}

/// N2: a loosening request whose key names two registered agents is
/// refused and reported; neither agent changes, and the config keeps the
/// key local-only.
#[test]
fn p3_hm_m4_n2_a_loosening_key_matching_two_agents_is_refused() {
    let name = "p3_hm_m4_n2_a_loosening_key_matching_two_agents_is_refused";
    let Some(w) = e2e(name, FakeOllama::serving(&[]), || {
        let state = AppState::new_in_memory();
        let first = register_by_supervisor(&state, "p3-hm-m4-shared", true);
        let second = register_by_supervisor(&state, "p3-hm-m4-shared", true);
        m4_resolve(&state, &first);
        m4_resolve(&state, &second);
        let set = crate::commands::chat_llm::set_agent_llm_provider(
            &state,
            None,
            "p3-hm-m4-shared".to_string(),
            "anthropic".to_string(),
            false,
            0,
            0,
        );
        Ok(format!(
            "set_ok={} first={} second={} assignment={}",
            set.is_ok(),
            m4_policy(&state, &first),
            m4_policy(&state, &second),
            m4_assignment("p3-hm-m4-shared")
        ))
    }) else {
        return;
    };
    assert_eq!(
        hm_field(&w.result, "set_ok="),
        "false",
        "N2: a key naming two agents was not refused: {}",
        w.result
    );
    for key in ["first=", "second="] {
        assert_eq!(
            hm_field(&w.result, key),
            "1|1|default_restrictive",
            "{}",
            w.result
        );
    }
    assert_eq!(
        hm_field(&w.result, "assignment="),
        "anthropic|true|0|0",
        "N2: the refused loosening loosened the config mirror: {}",
        w.result
    );
}

/// P3-C11-M5 N1 K-2 (an M4 carry-over): a loosening key that names two
/// registered agents is refused even when the test confirmer would answer
/// yes. The confirmer is never asked; neither agent's store row changes, no
/// loosening event is written, and the config keeps the key local-only.
#[test]
fn p3_hm_m5_n1_k2_a_two_agent_loosening_key_is_refused_without_asking_a_willing_confirmer() {
    let name =
        "p3_hm_m5_n1_k2_a_two_agent_loosening_key_is_refused_without_asking_a_willing_confirmer";
    let Some(w) = e2e(name, FakeOllama::serving(&[]), || {
        let state = AppState::new_in_memory();
        let first = register_by_supervisor(&state, "p3-hm-m5-shared", true);
        let second = register_by_supervisor(&state, "p3-hm-m5-shared", true);
        m4_resolve(&state, &first);
        m4_resolve(&state, &second);
        let willing = M4TestConfirmer::answering(true);
        let set = crate::commands::chat_llm::set_agent_llm_provider(
            &state,
            Some(&willing),
            "p3-hm-m5-shared".to_string(),
            "anthropic".to_string(),
            false,
            0,
            0,
        );
        Ok(format!(
            "set_ok={} asked={} first={} second={} loosened={} assignment={}",
            set.is_ok(),
            willing.shown().len(),
            m4_policy(&state, &first),
            m4_policy(&state, &second),
            m4_events(&state, &first, "memory.policy_loosened")
                + m4_events(&state, &second, "memory.policy_loosened"),
            m4_assignment("p3-hm-m5-shared")
        ))
    }) else {
        return;
    };
    let field = |k: &str| hm_field(&w.result, k);
    assert_eq!(
        field("set_ok="),
        "false",
        "K-2: a key naming two agents was not refused: {}",
        w.result
    );
    assert_eq!(
        field("asked="),
        "0",
        "K-2: the confirmer was asked for a key naming two agents: {}",
        w.result
    );
    for key in ["first=", "second="] {
        assert_eq!(field(key), "1|1|default_restrictive", "{}", w.result);
    }
    assert_eq!(field("loosened="), "0", "{}", w.result);
    assert_eq!(
        field("assignment="),
        "anthropic|true|0|0",
        "K-2: the refused loosening loosened the config mirror: {}",
        w.result
    );
}

/// N2 (the other production writer of `local_only`: the interface's
/// whole-configuration save): a key flipped to `true` is a governed tighten
/// (store first); a key flipped to `false` without a native confirmation is
/// refused and reported, and the config keeps it local-only; the rest of
/// the save is written.
#[test]
fn p3_hm_m4_n2_an_interface_config_save_tightens_store_first_and_never_loosens_unconfirmed() {
    let name =
        "p3_hm_m4_n2_an_interface_config_save_tightens_store_first_and_never_loosens_unconfirmed";
    let Some(w) = e2e(name, FakeOllama::serving(&[]), || {
        let state = AppState::new_in_memory();
        let open = register_by_supervisor(&state, "p3-hm-m4-open", false);
        let local = register_by_supervisor(&state, "p3-hm-m4-local", true);
        m4_resolve(&state, &local);
        let mut config = nexus_kernel::config::load_config().unwrap();
        config
            .agent_llm_assignments
            .get_mut(&open)
            .unwrap()
            .local_only = true;
        let entry = config.agent_llm_assignments.get_mut(&local).unwrap();
        entry.local_only = false;
        entry.provider_id = "anthropic".to_string();
        // The production path of the interface's save (the IPC command's
        // body), with no native confirmer.
        let saved =
            crate::hm_policy::save_config(&state, None, &crate::hm_policy::LiveConfigFile, config);
        Ok(format!(
            "saved_ok={} open={} tightened={} open_assignment={} local={} local_assignment={}",
            saved.is_ok(),
            m4_policy(&state, &open),
            m4_events(&state, &open, "memory.policy_tightened"),
            m4_assignment(&open),
            m4_policy(&state, &local),
            m4_assignment(&local)
        ))
    }) else {
        return;
    };
    assert_eq!(
        hm_field(&w.result, "open_assignment="),
        "|true|0|0",
        "N2: precondition: the save was not written: {}",
        w.result
    );
    assert_eq!(
        hm_field(&w.result, "open="),
        "1|3|tightened",
        "N2: the interface save's tightening was not committed in the store first: {}",
        w.result
    );
    assert_eq!(hm_field(&w.result, "tightened="), "1", "{}", w.result);
    assert_eq!(
        hm_field(&w.result, "local="),
        "1|1|default_restrictive",
        "{}",
        w.result
    );
    assert_eq!(
        hm_field(&w.result, "local_assignment="),
        "anthropic|true|0|0",
        "N2: the interface save loosened the config mirror without confirmation: {}",
        w.result
    );
    assert_eq!(
        hm_field(&w.result, "saved_ok="),
        "false",
        "N2: the refused loosening was not reported: {}",
        w.result
    );
}

/// The test confirmer (`cfg(test)` only): answers `yes` and records every
/// request it is shown (title, then message).
struct M4TestConfirmer {
    yes: bool,
    shown: std::sync::Mutex<Vec<(String, String)>>,
}

impl M4TestConfirmer {
    fn answering(yes: bool) -> Self {
        Self {
            yes,
            shown: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn shown(&self) -> Vec<(String, String)> {
        self.shown.lock().unwrap().clone()
    }
}

impl nexus_kernel::hm::RoutePolicyConfirmer for M4TestConfirmer {
    fn confirm_loosening(&self, request: &nexus_kernel::hm::LooseningRequest) -> bool {
        self.shown
            .lock()
            .unwrap()
            .push((request.title().to_string(), request.message()));
        self.yes
    }
}

/// The exact dialog text of a loosening of the agent `name` (§4.4).
fn m4_dialog_text(name: &str, agent: &str) -> String {
    format!(
        "Agent: {name} ({})\nScope: this agent's route policy, from local-only to unrestricted, \
for its new work only.\nNew work by this agent may be sent to non-local providers. Records \
made while it was local-only are not released.",
        &agent[..8]
    )
}

/// N2 (§4.4, §4.5 row 2, §7.1; mutant 1): a loosening through the production
/// path with the test confirmer. The Owner is shown the exact dialog (the
/// registered name, the UUID prefix, the scope and the §4.4 consequence)
/// once; on a yes the config mirror is written, then the store commits the
/// loosening with its history row and event. Declined or with no confirmer,
/// nothing changes and the config keeps `local_only = true`.
#[test]
fn p3_hm_m4_n2_a_native_confirmation_loosens_through_the_production_path() {
    let name = "p3_hm_m4_n2_a_native_confirmation_loosens_through_the_production_path";
    let Some(w) = e2e(name, FakeOllama::serving(&[]), || {
        let state = AppState::new_in_memory();
        let agent = register_by_supervisor(&state, "p3-hm-m4-agent", true);
        // Its desktop name (the registered name the dialog shows first)
        // carries a `%`, which the Linux dialog prints only escaped.
        state.meta.lock().unwrap().insert(
            Uuid::parse_str(&agent).unwrap(),
            crate::AgentMeta {
                name: "p3-hm-m4 50% agent".to_string(),
                last_action: String::new(),
            },
        );
        m4_resolve(&state, &agent);
        let set = |confirmer: Option<&dyn nexus_kernel::hm::RoutePolicyConfirmer>| {
            crate::commands::chat_llm::set_agent_llm_provider(
                &state,
                confirmer,
                agent.clone(),
                "anthropic".to_string(),
                false,
                3,
                4,
            )
        };
        let declining = M4TestConfirmer::answering(false);
        let declined = set(Some(&declining));
        let after_decline = format!("{}/{}", m4_policy(&state, &agent), m4_assignment(&agent));
        let confirming = M4TestConfirmer::answering(true);
        let confirmed = set(Some(&confirming));
        // A later observation point keeps it unrestricted: the mirror
        // removed every local-only assignment of the agent.
        m4_resolve(&state, &agent);
        let shown = confirming.shown();
        let expected = (
            "Allow non-local providers for this agent?".to_string(),
            m4_dialog_text("p3-hm-m4 50% agent", &agent),
        );
        let escaped = crate::hm_policy::dialog_text(&shown[0].1);
        Ok(format!(
            "declined_ok={} declined_asked={} after_decline={after_decline} confirmed_ok={} \
             confirmed_asked={} dialog_exact={} escaped={} after={} loosened={} assignment={}",
            declined.is_ok(),
            declining.shown().len(),
            confirmed.is_ok(),
            shown.len(),
            shown[0] == expected,
            escaped.contains("p3-hm-m4 50%% agent") && !escaped.contains("50% "),
            m4_policy(&state, &agent),
            m4_events(&state, &agent, "memory.policy_loosened"),
            m4_assignment(&agent)
        ))
    }) else {
        return;
    };
    let field = |k: &str| hm_field(&w.result, k);
    assert_eq!(field("declined_ok="), "false", "{}", w.result);
    assert_eq!(field("declined_asked="), "1", "{}", w.result);
    assert_eq!(
        field("after_decline="),
        "1|1|default_restrictive/anthropic|true|3|4",
        "N2: a declined loosening changed something: {}",
        w.result
    );
    assert_eq!(field("confirmed_ok="), "true", "{}", w.result);
    assert_eq!(field("confirmed_asked="), "1", "{}", w.result);
    assert_eq!(
        field("dialog_exact="),
        "true",
        "N2: dialog text: {}",
        w.result
    );
    assert_eq!(
        field("escaped="),
        "true",
        "N2: Linux %-escaping: {}",
        w.result
    );
    assert_eq!(
        field("after="),
        "0|2|owner_confirmed",
        "N2: the confirmed loosening was not committed: {}",
        w.result
    );
    assert_eq!(field("loosened="), "1", "{}", w.result);
    assert_eq!(field("assignment="), "anthropic|false|3|4", "{}", w.result);
}

/// A config file whose every write fails (its reads are the live file's).
struct M4FailingSave;

impl crate::hm_policy::ConfigFile for M4FailingSave {
    fn load(&self) -> Result<nexus_kernel::config::NexusConfig, String> {
        nexus_kernel::config::load_config().map_err(|e| e.to_string())
    }

    fn save(&self, _: &nexus_kernel::config::NexusConfig) -> Result<(), String> {
        Err("synthetic config write failure".to_string())
    }
}

/// N2 (§4.5): a config mirror write that fails is reported and never
/// changes the store decision: a confirmed loosening still commits, a
/// governed tighten still stands, and the call reports the failure.
#[test]
fn p3_hm_m4_n2_a_failed_config_mirror_is_reported_and_never_changes_the_store_decision() {
    let name =
        "p3_hm_m4_n2_a_failed_config_mirror_is_reported_and_never_changes_the_store_decision";
    let Some(w) = e2e(name, FakeOllama::serving(&[]), || {
        let state = AppState::new_in_memory();
        let local = register_by_supervisor(&state, "p3-hm-m4-local", true);
        let open = register_by_supervisor(&state, "p3-hm-m4-open", false);
        m4_resolve(&state, &local);
        let confirming = M4TestConfirmer::answering(true);
        let loosen = crate::hm_policy::set_agent_llm_provider(
            &state,
            Some(&confirming),
            &M4FailingSave,
            local.clone(),
            "anthropic".to_string(),
            false,
            0,
            0,
        );
        let local_after = m4_policy(&state, &local);
        let tighten = crate::hm_policy::set_agent_llm_provider(
            &state,
            None,
            &M4FailingSave,
            open.clone(),
            "anthropic".to_string(),
            true,
            0,
            0,
        );
        Ok(format!(
            "loosen_err={} local_after={local_after} tighten_err={} open_after={}",
            loosen.err().unwrap_or_default().replace(' ', "_"),
            tighten.err().unwrap_or_default().replace(' ', "_"),
            m4_policy(&state, &open)
        ))
    }) else {
        return;
    };
    let field = |k: &str| hm_field(&w.result, k);
    assert!(
        field("loosen_err=").contains("config_mirror_was_not_written"),
        "N2: the failed mirror of a loosening was not reported: {}",
        w.result
    );
    assert_eq!(
        field("local_after="),
        "0|2|owner_confirmed",
        "N2: the failed mirror changed the loosening's store decision: {}",
        w.result
    );
    assert!(
        field("tighten_err=").contains("config_mirror_was_not_written"),
        "N2: the failed mirror of a tighten was not reported: {}",
        w.result
    );
    assert_eq!(
        field("open_after="),
        "1|3|tightened",
        "N2: the failed mirror changed the tighten's store decision: {}",
        w.result
    );
}

/// N2: the interface's whole-configuration save loosens a key only through
/// the native confirmation: confirmed, the store commits the loosening and
/// the save writes `false`.
#[test]
fn p3_hm_m4_n2_an_interface_config_save_loosens_only_on_native_confirmation() {
    let name = "p3_hm_m4_n2_an_interface_config_save_loosens_only_on_native_confirmation";
    let Some(w) = e2e(name, FakeOllama::serving(&[]), || {
        let state = AppState::new_in_memory();
        let local = register_by_supervisor(&state, "p3-hm-m4-local", true);
        m4_resolve(&state, &local);
        let mut config = nexus_kernel::config::load_config().unwrap();
        config
            .agent_llm_assignments
            .get_mut(&local)
            .unwrap()
            .local_only = false;
        let confirming = M4TestConfirmer::answering(true);
        let saved = crate::hm_policy::save_config(
            &state,
            Some(&confirming),
            &crate::hm_policy::LiveConfigFile,
            config,
        );
        Ok(format!(
            "saved_ok={} asked={} local={} assignment={}",
            saved.is_ok(),
            confirming.shown().len(),
            m4_policy(&state, &local),
            m4_assignment(&local)
        ))
    }) else {
        return;
    };
    let field = |k: &str| hm_field(&w.result, k);
    assert_eq!(field("saved_ok="), "true", "{}", w.result);
    assert_eq!(field("asked="), "1", "{}", w.result);
    assert_eq!(field("local="), "0|2|owner_confirmed", "{}", w.result);
    assert_eq!(field("assignment="), "|false|0|0", "{}", w.result);
}

// ── P3-C11-HM-M4 N3: audit content, A-A1 (P3-HM-CONTRACT-02 §15.2) ──────────

/// The payloads of the desktop's audit events whose `action` is `action`.
fn m4_audit_actions(state: &AppState, action: &str) -> Vec<serde_json::Value> {
    state
        .audit
        .lock()
        .unwrap()
        .events()
        .iter()
        .filter(|e| e.payload.get("action").and_then(|v| v.as_str()) == Some(action))
        .map(|e| e.payload.clone())
        .collect()
}

/// A-A1, row D3: `agent_goal_completed` carries the goal id, its status and
/// its success, never the result summary.
#[test]
fn p3_hm_m4_n3_a_a1_d3_a_goal_completion_carries_no_result() {
    const MARKER: &str = "P3-HM-M4-A-A1-RESULT-5d70";
    let state = AppState::new_in_memory();
    let agent = Uuid::new_v4().to_string();
    let goal = Uuid::new_v4().to_string();
    persist_task_completion(
        &state,
        &agent,
        &goal,
        "completed",
        &format!("The synthetic result {MARKER}"),
        true,
        0.0,
    );
    let completions = m4_audit_actions(&state, "agent_goal_completed");
    assert_eq!(completions.len(), 1);
    let payload = completions[0].to_string();
    assert!(
        !payload.contains(MARKER) && completions[0].get("result_summary").is_none(),
        "A-A1: the result in an audit payload: {payload}"
    );
    assert_eq!(completions[0]["goal_id"], goal);
    assert_eq!(completions[0]["success"], true);
}

/// A-A1, row D4: the audit payload of `agent_evolved_strategy` carries no
/// strategy hash (the frontend emit is local UI delivery, unchanged).
#[test]
fn p3_hm_m4_n3_a_a1_d4_an_evolved_strategy_audit_carries_no_strategy_hash() {
    const MARKER: &str = "P3-HM-M4-A-A1-EVOLVE-GOAL-3a9c";
    let state = AppState::new_in_memory();
    let agent = {
        let manifest = parse_manifest(
            r#"
name = "p3-hm-m4-evolver"
version = "1.0.0"
capabilities = ["llm.query"]
fuel_budget = 10000
autonomy_level = 4
"#,
        )
        .unwrap();
        state
            .supervisor
            .lock()
            .unwrap()
            .start_agent(manifest)
            .unwrap()
            .to_string()
    };
    let description = format!("Plan the synthetic goal {MARKER}");
    // Five completed goals: evolution selects a strategy on every fifth.
    let mut goal_id = String::new();
    for _ in 0..5 {
        goal_id = execute_agent_goal(&state, agent.clone(), description.clone(), 5, None).unwrap();
        persist_task_completion(&state, &agent, &goal_id, "completed", "done", true, 0.0);
    }
    run_post_goal_evolution(
        &BackendEventBridge::default(),
        &state,
        &agent,
        &goal_id,
        true,
        0.0,
        None,
    );
    let evolved = m4_audit_actions(&state, "agent_evolved_strategy");
    assert_eq!(evolved.len(), 1, "precondition: a strategy evolved");
    let hash = nexus_kernel::cognitive::hash_strategy(&description.to_lowercase());
    let payload = evolved[0].to_string();
    assert!(
        !payload.contains(&hash) && evolved[0].get("strategy_hash").is_none(),
        "A-A1: the strategy hash in an audit payload: {payload}"
    );
    assert!(evolved[0]["new_score"].as_f64().is_some());
}

/// A-A1, row D6: `start_hivemind` carries the session id, never the goal.
#[test]
fn p3_hm_m4_n3_a_a1_d6_start_hivemind_carries_no_goal() {
    const MARKER: &str = "P3-HM-M4-A-A1-HIVE-SESSION-GOAL-19e4";
    let name = "p3_hm_m4_n3_a_a1_d6_start_hivemind_carries_no_goal";
    let Some(w) = e2e(name, FakeOllama::serving(&["qwen3:8b"]), || {
        let state = AppState::new_in_memory();
        let a = register_participant(&state, ("p3-hm-m4-hive-a", true, ""));
        let b = register_participant(&state, ("p3-hm-m4-hive-b", true, ""));
        let goal = format!("Answer the synthetic question {MARKER}");
        let session = hive_session_on(&state, &goal, vec![a, b]);
        let starts = m4_audit_actions(&state, "start_hivemind");
        let leaks = starts
            .iter()
            .filter(|p| p.to_string().contains(MARKER) || p.get("goal").is_some())
            .count();
        let with_session = starts
            .iter()
            .filter(|p| p["session_id"].as_str().is_some())
            .count();
        Ok(format!(
            "session_ok={} starts={} leaks={leaks} with_session={with_session}",
            session.is_ok(),
            starts.len()
        ))
    }) else {
        return;
    };
    assert_eq!(
        hm_field(&w.result, "starts="),
        "1",
        "precondition: the session ran: {}",
        w.result
    );
    assert_eq!(
        hm_field(&w.result, "leaks="),
        "0",
        "A-A1: the goal in a start_hivemind payload: {}",
        w.result
    );
    assert_eq!(hm_field(&w.result, "with_session="), "1", "{}", w.result);
}

// ── P3-C11-HM-M4 N4: the test-only capture seam, A-S5 (§18.2, AD-6) ─────────

/// A-S5 through the desktop's production adapters: at the transport
/// boundary every AR request (an agent's goal: planning and steps) and every
/// AC request (its chat turn) carries the admission id it was admitted by;
/// HD (the Owner's chat), SY (the connection test) and XS (a subsystem's
/// provider) requests carry no retained-record marker; an HB request (a
/// temporal fork naming no agent) carries its admission id.
#[test]
fn p3_hm_m4_n4_a_s5_every_ar_ac_request_carries_an_admission_id() {
    const MARKER: &str = "P3-HM-M4-A-S5-SYNTH-NOTE-4f1b";
    let name = "p3_hm_m4_n4_a_s5_every_ar_ac_request_carries_an_admission_id";
    let Some(w) = e2e(name, hm_route(), || {
        let state = AppState::new_in_memory();
        let agent = register_participant(&state, ("p3-hm-m4-a-s5", false, ""));
        route_unrestricted_work_through_the_proxy();
        std::env::set_var("LLM_PROVIDER", "ollama");
        // Retained records of the agent carrying the marker, then a goal
        // that recalls them (AR).
        let first = run_own_goal(
            &state,
            &agent,
            &format!("Summarize the synthetic note {MARKER}"),
        );
        let second = run_own_goal(&state, &agent, "Summarize the synthetic note again");
        // AC: the agent's chat turn.
        let ac = hm_turn(
            &state,
            crate::hm_dispatch::chat_turn(&state, Some("p3-hm-m4-a-s5"))
                .map_err(AgentError::SupervisorError)?,
            "An AC chat turn",
            None,
        )
        .is_ok();
        // HD, SY, XS, HB.
        let hd = hm_turn(
            &state,
            crate::hm_dispatch::chat_turn(&state, None).map_err(AgentError::SupervisorError)?,
            "An HD chat turn",
            None,
        )
        .is_ok();
        let sy = crate::commands::chat_llm::test_llm_connection(&state, "ollama".to_string())
            .map(|r| r.success)
            .unwrap_or(false);
        let xs = {
            let config = nexus_kernel::config::load_config().unwrap();
            let (provider, model) = crate::provider_from_prefixed_model(
                "ollama/qwen3:8b",
                &crate::build_provider_config(&config),
            )
            .map_err(AgentError::SupervisorError)?;
            crate::hm_dispatch::IssuedProvider::new(&state, "p3-hm-m4-xs", provider)
                .query("An XS request", 64, &model)
                .is_ok()
        };
        let hb = crate::commands::simulation::temporal_fork(
            &state,
            "Plan an unrelated task".to_string(),
            "not-a-registered-agent".to_string(),
            Some(2),
        )
        .is_ok();
        let admitted: Vec<String> = crate::hm_m3_tests::admissions(&state.db)
            .into_iter()
            .map(|a| a.admission_id)
            .collect();
        let captured = crate::hm_dispatch::capture::take();
        let count = |class: &str| captured.iter().filter(|c| c.class == class).count();
        let ar_ac_without = captured
            .iter()
            .filter(|c| c.class == "AR" || c.class == "AC")
            .filter(|c| {
                !c.admission_id
                    .as_ref()
                    .is_some_and(|id| admitted.contains(id))
            })
            .count();
        let ar_marker = captured
            .iter()
            .filter(|c| c.class == "AR" && c.prompt.contains(MARKER))
            .count();
        let plain_marker = captured
            .iter()
            .filter(|c| c.class == "HD" || c.class == "SY/XS")
            .filter(|c| c.prompt.contains(MARKER) || c.admission_id.is_some())
            .count();
        let hb_without = captured
            .iter()
            .filter(|c| c.class == "HB" && c.admission_id.is_none())
            .count();
        let unmarked = count("NONE");
        Ok(format!(
            "goals={}/{} ac={ac} hd={hd} sy={sy} xs={xs} hb={hb} ar={} ac_n={} hd_n={} sy_n={} hb_n={} \
             ar_ac_without={ar_ac_without} ar_marker={ar_marker} plain_marker={plain_marker} \
             hb_without={hb_without} unmarked={unmarked}",
            first.is_ok(),
            second.is_ok(),
            count("AR"),
            count("AC"),
            count("HD"),
            count("SY/XS"),
            count("HB")
        ))
    }) else {
        return;
    };
    hm_dump(name, &w);
    let field = |k: &str| hm_field(&w.result, k);
    for (key, what) in [
        ("ac=", "AC turn"),
        ("hd=", "HD turn"),
        ("sy=", "SY request"),
        ("xs=", "XS request"),
    ] {
        assert_eq!(field(key), "true", "precondition: the {what}: {}", w.result);
    }
    for key in ["ar=", "ac_n=", "hd_n=", "sy_n=", "hb_n="] {
        assert_ne!(
            field(key),
            "0",
            "precondition: {key} captured none: {}",
            w.result
        );
    }
    assert_ne!(
        field("ar_marker="),
        "0",
        "precondition: no AR request carried the retained marker: {}",
        w.result
    );
    assert_eq!(
        field("ar_ac_without="),
        "0",
        "A-S5: an AR or AC request without a committed admission id: {}",
        w.result
    );
    assert_eq!(
        field("plain_marker="),
        "0",
        "A-S5: an HD or SY/XS request carried a retained record or an admission: {}",
        w.result
    );
    assert_eq!(field("hb_without="), "0", "{}", w.result);
    assert_eq!(
        field("unmarked="),
        "0",
        "A-S5: a request reached a transport outside the issuer's dispatch: {}",
        w.result
    );
}

// ── P3-C11-HM-M4 N5: the dream engine's lock (§13.2 dream LK, A-L4) ─────────

/// A loopback Ollama runtime inside the child scenario whose generations
/// announce they arrived and wait to be released (the barrier of A-L4).
/// Returns its base URL, the arrival channel and the release sender.
fn m5_gated_runtime() -> (String, mpsc::Receiver<()>, mpsc::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (arrived_tx, arrived_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        let serves = FakeOllama::serving(&["qwen3:8b"]);
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
            let Some((head, body)) = read_request(&stream) else {
                continue;
            };
            let path = head.split(' ').nth(1).unwrap_or_default().to_string();
            if path == "/api/generate" {
                let _ = arrived_tx.send(());
                let _ = release_rx.recv_timeout(Duration::from_secs(60));
            }
            let (status, reply) = runtime_reply(&serves, &path, &body);
            respond(&mut stream, status, &reply);
        }
    });
    (base, arrived_rx, release_tx)
}

/// A-L4 (§13.2 dream LK; mutant 14): while a dream's model call (and the
/// morning briefing's) is on the wire, the dream engine's lock is
/// acquirable; the dream's result is recorded afterwards.
#[test]
fn p3_hm_m4_n5_a_l4_the_dream_engine_is_not_held_across_a_dream_dispatch() {
    let name = "p3_hm_m4_n5_a_l4_the_dream_engine_is_not_held_across_a_dream_dispatch";
    let Some(w) = e2e(name, FakeOllama::serving(&[]), || {
        let state = AppState::new_in_memory();
        let (base, arrived, release) = m5_gated_runtime();
        std::env::set_var("OLLAMA_URL", &base);
        // One dream entry, queued by the production queueing (an Owner's
        // chat turn's Replay dream).
        let turn =
            crate::hm_dispatch::chat_turn(&state, None).map_err(AgentError::SupervisorError)?;
        hm_queue_only(&state, &turn, "A synthetic turn");
        let acquirable_during = |what: &str, call: Box<dyn FnOnce() -> bool + Send>| {
            let worker = std::thread::spawn(call);
            let arrived_ok = arrived.recv_timeout(Duration::from_secs(10)).is_ok();
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let free = loop {
                if state.dream_engine.try_lock().is_ok() {
                    break true;
                }
                if std::time::Instant::now() >= deadline {
                    break false;
                }
                std::thread::sleep(Duration::from_millis(1));
            };
            let _ = release.send(());
            let returned = worker.join().unwrap();
            format!("{what}_arrived={arrived_ok} {what}_free={free} {what}_ok={returned}")
        };
        let dreamer = state.clone();
        let dream = acquirable_during(
            "dream",
            Box::new(move || crate::commands::simulation::trigger_dream_now(&dreamer).is_ok()),
        );
        let recorded = state
            .dream_engine
            .lock()
            .unwrap()
            .scheduler
            .completed_dreams
            .len();
        let briefer = state.clone();
        let briefing = acquirable_during(
            "briefing",
            Box::new(move || crate::commands::simulation::get_morning_briefing(&briefer).is_ok()),
        );
        Ok(format!("{dream} recorded={recorded} {briefing}"))
    }) else {
        return;
    };
    let field = |k: &str| hm_field(&w.result, k);
    for what in ["dream", "briefing"] {
        assert_eq!(
            field(&format!("{what}_arrived=")),
            "true",
            "precondition: the {what}'s model call reached the runtime: {}",
            w.result
        );
        assert_eq!(
            field(&format!("{what}_free=")),
            "true",
            "A-L4: the dream engine was held across the {what}'s dispatch: {}",
            w.result
        );
        assert_eq!(field(&format!("{what}_ok=")), "true", "{}", w.result);
    }
    assert_eq!(
        field("recorded="),
        "1",
        "A-L4: the dream's result was not recorded: {}",
        w.result
    );
}
