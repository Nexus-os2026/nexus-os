//! P3 D-2: an agent whose configuration is local-only never has a prompt
//! dispatched to a cloud provider.
//!
//! The unit tests drive the planner's dispatch boundary with a fake local
//! runtime and a recording dispatch, so nothing is sent anywhere. The
//! end-to-end tests run the real route resolution, policy, thread-local
//! propagation and planner in a child process whose only reachable services
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

    // The planner itself refuses before any provider or Flash session.
    let result = with_agent_llm_policy(AgentLlmPolicy::Unresolved, || {
        GatewayPlannerLlm.plan_query(PROMPT)
    });
    assert_refused(&result, "planner under an unresolved policy");

    // An id that is not a registered agent has no policy to establish.
    let state = AppState::new_in_memory();
    assert_eq!(
        crate::resolve_agent_llm_policy(&state, "not-a-uuid"),
        AgentLlmPolicy::Unresolved
    );
    assert_eq!(
        crate::resolve_agent_llm_policy(&state, &Uuid::new_v4().to_string()),
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

#[test]
fn p3_d2_policy_is_restored_after_the_work_even_when_it_unwinds() {
    assert_eq!(active_agent_llm_policy(), AgentLlmPolicy::Unrestricted);
    let unwound = std::panic::catch_unwind(|| {
        with_agent_llm_policy(AgentLlmPolicy::LocalOnly, || {
            assert_eq!(active_agent_llm_policy(), AgentLlmPolicy::LocalOnly);
            with_agent_llm_policy(AgentLlmPolicy::Unresolved, || {
                assert_eq!(active_agent_llm_policy(), AgentLlmPolicy::Unresolved);
            });
            assert_eq!(active_agent_llm_policy(), AgentLlmPolicy::LocalOnly);
            panic!("unwind through the policy scope");
        })
    });
    assert!(unwound.is_err());
    assert_eq!(active_agent_llm_policy(), AgentLlmPolicy::Unrestricted);
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
}

impl FakeOllama {
    fn serving(models: &[&'static str]) -> Self {
        Self {
            models: models.to_vec(),
            remote: Vec::new(),
            generate_status: 200,
            proxy_answers: false,
            tighten: None,
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
        let (status, reply) = runtime_reply(&serves, &path, &body);
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
    crate::with_agent_llm_route(&state, &id, || GatewayPlannerLlm.plan_query(PROMPT))
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
/// the real coordinator with `GatewayHivemindLlm`, real sub-task loops).
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
    let other_policy = crate::resolve_agent_llm_policy(&state, &other);
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
