//! P3-ENTRY-G1 Phase Three entry guards: test code only, granting nothing.
//!
//! A string is never authority. These guards read production source and the
//! authority structure it builds; they never decide or grant anything.
//!
//! - `p3e_g1_*` (G1): every `PlannedAction` variant has an explicit security
//!   decision in `expected_decision`, an exhaustive match with no wildcard,
//!   so a variant added to the kernel enum does not compile here until it is
//!   classified. Production keeps its own fail-closed wildcard. For every
//!   variant the production decision must equal this one, and the executor
//!   must consult it before its inner registry. Only inert fixtures ever
//!   reach the real executor.
//! - `p3e_g3_*` (G3): the Phase Zero closures as a semantic inventory, not a
//!   count. Every canonical closed handler (its whole body the denial), every
//!   conditional `closed(..)` call and every function that computes a closure
//!   is classified by name and closure; registration order is irrelevant;
//!   every `Closure` variant is classified, bounded and used.
//! - `p3e_g5_*` (G5): the Phase Two launch approval, pinned from outside the
//!   Phase Two code. Only the native dialogs confirm; nothing but the
//!   kernel, after a recorded native confirmation, can produce an approval;
//!   it cannot be cloned or deserialized; a launch consumes it and re-checks
//!   the live binding first.
//! - `p3e_scan_*`: the scanners, including module-local copies of the Phase
//!   Zero production-text scanner, proven to be the same code.
//!
//! Matching is whitespace- and module-qualification-insensitive. Assertions
//! are about sets and structure; counts are printed as diagnostics only. A
//! failure here is a trust-surface change: review it with the Architect,
//! then update the classification it names.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use nexus_kernel::cognitive::PlannedAction;

use crate::phase0_surface::{closed, Closure};

// ---------------------------------------------------------------------------
// G1: the security decision for every planned action
// ---------------------------------------------------------------------------

/// The decision the production agent executor must reach for an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decision {
    /// Class A, permitted in Phase Zero: the action holds no filesystem,
    /// process, OS-input, screen, browser or model-chosen network authority.
    Permitted,
    /// Class B, authority-bearing: refused, with this closure, before the
    /// kernel action registry is reached.
    Refused(Closure),
}

/// Class A, exactly.
const PERMITTED: [&str; 10] = [
    "LlmQuery",
    "Noop",
    "MemoryStore",
    "MemoryRecall",
    "SendNotification",
    "AgentMessage",
    "HitlRequest",
    "WebSearch",
    "KnowledgeGraphUpdate",
    "KnowledgeGraphQuery",
];

/// A web fetch's closure. An http(s) URL is a destination the model chose,
/// not an egress grant; any other scheme (`file:`, `data:`, none) names
/// something that is not the web at all.
fn web_fetch_closure(url: &str) -> Closure {
    let url = url.trim_start().to_ascii_lowercase();
    if url.starts_with("https://") || url.starts_with("http://") {
        Closure::NetworkDestination
    } else {
        Closure::AgentExecution
    }
}

/// The explicit security decision for every `PlannedAction` variant.
///
/// No wildcard and no catch-all binding (`p3e_g1_01` scans this function):
/// a variant added to the kernel enum does not compile here until it is
/// classified, while production keeps refusing it through its own wildcard.
fn expected_decision(action: &PlannedAction) -> Decision {
    use Decision::{Permitted, Refused};
    match action {
        // Class A: the model, the agent's own memory and knowledge graph, and
        // the governed notification, messaging and approval channels. Web
        // search reaches fixed or operator-configured hosts only.
        PlannedAction::LlmQuery { .. } => Permitted,
        PlannedAction::Noop => Permitted,
        PlannedAction::MemoryStore { .. } => Permitted,
        PlannedAction::MemoryRecall { .. } => Permitted,
        PlannedAction::SendNotification { .. } => Permitted,
        PlannedAction::AgentMessage { .. } => Permitted,
        PlannedAction::HitlRequest { .. } => Permitted,
        PlannedAction::WebSearch { .. } => Permitted,
        PlannedAction::KnowledgeGraphUpdate { .. } => Permitted,
        PlannedAction::KnowledgeGraphQuery { .. } => Permitted,
        // Class B, network: the destination or peer is the model's choice.
        PlannedAction::WebFetch { url } => Refused(web_fetch_closure(url)),
        PlannedAction::ApiCall { .. } => Refused(Closure::AgentExecution),
        PlannedAction::A2aDelegation { .. } => Refused(Closure::AgentExecution),
        // Class B, filesystem (and provider egress for generated media).
        PlannedAction::FileRead { .. } => Refused(Closure::AgentExecution),
        PlannedAction::FileWrite { .. } => Refused(Closure::AgentExecution),
        PlannedAction::ImageGenerate { .. } => Refused(Closure::AgentExecution),
        PlannedAction::TextToSpeech { .. } => Refused(Closure::AgentExecution),
        // Class B, processes and code.
        PlannedAction::ShellCommand { .. } => Refused(Closure::AgentExecution),
        PlannedAction::DockerCommand { .. } => Refused(Closure::AgentExecution),
        PlannedAction::CodeExecute { .. } => Refused(Closure::AgentExecution),
        // Class B, the browser, the screen and OS input.
        PlannedAction::BrowserAutomate { .. } => Refused(Closure::AgentExecution),
        PlannedAction::CaptureScreen { .. } => Refused(Closure::AgentExecution),
        PlannedAction::CaptureWindow { .. } => Refused(Closure::AgentExecution),
        PlannedAction::AnalyzeScreen { .. } => Refused(Closure::AgentExecution),
        PlannedAction::MouseMove { .. } => Refused(Closure::AgentExecution),
        PlannedAction::MouseClick { .. } => Refused(Closure::AgentExecution),
        PlannedAction::MouseDoubleClick { .. } => Refused(Closure::AgentExecution),
        PlannedAction::MouseDrag { .. } => Refused(Closure::AgentExecution),
        PlannedAction::KeyboardType { .. } => Refused(Closure::AgentExecution),
        PlannedAction::KeyboardPress { .. } => Refused(Closure::AgentExecution),
        PlannedAction::KeyboardShortcut { .. } => Refused(Closure::AgentExecution),
        PlannedAction::ScrollWheel { .. } => Refused(Closure::AgentExecution),
        PlannedAction::ComputerAction { .. } => Refused(Closure::AgentExecution),
        // Class B, self-modification, sub-agents, fuel, governance and the
        // agent's own cognition.
        PlannedAction::SelfModifyDescription { .. } => Refused(Closure::AgentExecution),
        PlannedAction::SelfModifyStrategy { .. } => Refused(Closure::AgentExecution),
        PlannedAction::CreateSubAgent { .. } => Refused(Closure::AgentExecution),
        PlannedAction::DestroySubAgent { .. } => Refused(Closure::AgentExecution),
        PlannedAction::RunEvolutionTournament { .. } => Refused(Closure::AgentExecution),
        PlannedAction::ModifyGovernancePolicy { .. } => Refused(Closure::AgentExecution),
        PlannedAction::AllocateEcosystemFuel { .. } => Refused(Closure::AgentExecution),
        PlannedAction::ModifyCognitiveParams { .. } => Refused(Closure::AgentExecution),
        PlannedAction::SelectLlmProvider { .. } => Refused(Closure::AgentExecution),
        PlannedAction::SelectAlgorithm { .. } => Refused(Closure::AgentExecution),
        PlannedAction::DesignAgentEcosystem { .. } => Refused(Closure::AgentExecution),
        PlannedAction::RunCounterfactual { .. } => Refused(Closure::AgentExecution),
        PlannedAction::TemporalPlan { .. } => Refused(Closure::AgentExecution),
    }
}

fn probe() -> String {
    "p3e-g1".to_string()
}

/// One compile-checked fixture per `PlannedAction` variant, in kernel enum
/// order. These are only classified and serialized; nothing here executes
/// them.
fn every_action() -> Vec<PlannedAction> {
    let url = || "https://p3e-g1.invalid/".to_string();
    vec![
        PlannedAction::LlmQuery {
            prompt: probe(),
            context: vec![],
        },
        PlannedAction::FileRead { path: probe() },
        PlannedAction::FileWrite {
            path: probe(),
            content: probe(),
        },
        PlannedAction::ShellCommand {
            command: probe(),
            args: vec![],
        },
        PlannedAction::DockerCommand {
            subcommand: probe(),
            args: vec![],
        },
        PlannedAction::WebSearch { query: probe() },
        PlannedAction::WebFetch { url: url() },
        PlannedAction::ApiCall {
            method: "GET".into(),
            url: url(),
            body: None,
            headers: None,
        },
        PlannedAction::ImageGenerate {
            prompt: probe(),
            output_path: probe(),
            provider: None,
            model: None,
            size: None,
        },
        PlannedAction::TextToSpeech {
            text: probe(),
            output_path: probe(),
            provider: None,
            voice: None,
            model: None,
        },
        PlannedAction::KnowledgeGraphUpdate {
            entities: vec![],
            relationships: vec![],
        },
        PlannedAction::KnowledgeGraphQuery { query: probe() },
        PlannedAction::BrowserAutomate {
            start_url: url(),
            actions: vec![],
            screenshot_dir: None,
        },
        PlannedAction::CaptureScreen { region: None },
        PlannedAction::CaptureWindow {
            window_title: probe(),
        },
        PlannedAction::AnalyzeScreen { query: probe() },
        PlannedAction::MouseMove { x: 1, y: 1 },
        PlannedAction::MouseClick {
            x: 1,
            y: 1,
            button: "left".into(),
        },
        PlannedAction::MouseDoubleClick { x: 1, y: 1 },
        PlannedAction::MouseDrag {
            from_x: 1,
            from_y: 1,
            to_x: 2,
            to_y: 2,
        },
        PlannedAction::KeyboardType { text: probe() },
        PlannedAction::KeyboardPress {
            key: "enter".into(),
        },
        PlannedAction::KeyboardShortcut {
            keys: vec!["ctrl".into(), "c".into()],
        },
        PlannedAction::ScrollWheel {
            direction: "down".into(),
            amount: 1,
        },
        PlannedAction::ComputerAction {
            description: probe(),
            max_steps: 1,
        },
        PlannedAction::AgentMessage {
            target_agent: probe(),
            message: probe(),
        },
        PlannedAction::HitlRequest {
            question: probe(),
            options: vec![],
        },
        PlannedAction::MemoryStore {
            key: probe(),
            value: probe(),
            memory_type: "episodic".into(),
        },
        PlannedAction::MemoryRecall {
            query: probe(),
            memory_type: None,
        },
        PlannedAction::SendNotification {
            title: probe(),
            body: probe(),
            level: "info".into(),
        },
        PlannedAction::CodeExecute {
            language: "python3".into(),
            code: probe(),
            timeout_secs: None,
        },
        PlannedAction::Noop,
        PlannedAction::SelfModifyDescription {
            new_description: probe(),
        },
        PlannedAction::SelfModifyStrategy {
            strategy_key: probe(),
            new_strategy: probe(),
        },
        PlannedAction::CreateSubAgent {
            manifest_json: "{}".into(),
        },
        PlannedAction::DestroySubAgent { agent_id: probe() },
        PlannedAction::RunEvolutionTournament {
            variants: vec![],
            task: probe(),
            rounds: 1,
        },
        PlannedAction::ModifyGovernancePolicy {
            policy_key: probe(),
            policy_value: probe(),
        },
        PlannedAction::AllocateEcosystemFuel {
            agent_id: probe(),
            amount: 1.0,
        },
        PlannedAction::ModifyCognitiveParams {
            param_key: probe(),
            param_value: probe(),
        },
        PlannedAction::SelectLlmProvider {
            phase: probe(),
            provider: probe(),
            model: probe(),
        },
        PlannedAction::SelectAlgorithm {
            algorithm: probe(),
            config_json: "{}".into(),
        },
        PlannedAction::DesignAgentEcosystem {
            ecosystem_json: "{}".into(),
        },
        PlannedAction::RunCounterfactual {
            decision_id: probe(),
            alternatives: vec![],
        },
        PlannedAction::TemporalPlan {
            immediate: probe(),
            short_term: probe(),
            medium_term: probe(),
            long_term: probe(),
        },
        PlannedAction::A2aDelegation {
            agent_url: url(),
            message: probe(),
        },
    ]
}

/// Web-fetch scheme edges and the closure each must be refused with.
const WEB_FETCH_EDGES: &[(&str, Closure)] = &[
    ("https://p3e-g1.invalid/", Closure::NetworkDestination),
    ("http://p3e-g1.invalid/", Closure::NetworkDestination),
    ("HTTP://P3E-G1.INVALID/", Closure::NetworkDestination),
    ("HtTpS://p3e-g1.invalid/", Closure::NetworkDestination),
    ("  https://p3e-g1.invalid/", Closure::NetworkDestination),
    ("\n\thttp://p3e-g1.invalid/", Closure::NetworkDestination),
    ("file:///etc/passwd", Closure::AgentExecution),
    (" FILE:///etc/passwd", Closure::AgentExecution),
    ("data:text/plain,p3e-g1", Closure::AgentExecution),
    ("", Closure::AgentExecution),
    ("   ", Closure::AgentExecution),
    ("ftp://p3e-g1.invalid/", Closure::AgentExecution),
    ("https:p3e-g1.invalid", Closure::AgentExecution),
    ("javascript:void(0)", Closure::AgentExecution),
    ("p3e-g1.invalid", Closure::AgentExecution),
];

/// The `type` tag of an action's wire form: its variant name.
fn wire_tag(action: &PlannedAction) -> String {
    serde_json::to_value(action).expect("an action serializes")["type"]
        .as_str()
        .expect("the wire form is tagged")
        .to_string()
}

/// The normalized body of `Phase0AgentExecutor::execute`: the closure is
/// consulted, and a refusal returned, before `self.inner` is touched.
const EXECUTOR_BODY: &str = "ifletSome(closure)=phase0_agent_action_closure(action){returnErr(closed(action.action_type(),closure));}self.inner.execute(agent_id,action,audit,hitl_approved)";

#[test]
fn p3e_g1_01_every_planned_action_is_classified_without_a_wildcard() {
    let declared = enum_variants(
        production_source("kernel/src/cognitive/types.rs"),
        "PlannedAction",
    );
    let declared_set: BTreeSet<&str> = declared.iter().map(String::as_str).collect();
    assert_eq!(declared_set.len(), declared.len(), "distinct variants");

    // One fixture per variant.
    let fixtures = every_action();
    let tags: Vec<String> = fixtures.iter().map(wire_tag).collect();
    let tag_set: BTreeSet<&str> = tags.iter().map(String::as_str).collect();
    assert_eq!(tag_set.len(), tags.len(), "one fixture per variant");
    assert_eq!(tag_set, declared_set, "the fixtures cover the enum");

    // The classifier names every variant in exactly one explicit arm, and has
    // no wildcard or catch-all binding.
    let own = production_text(&workspace_file(OWN_SOURCE));
    let mut classified = Vec::new();
    for (pattern, _) in match_arms(&own, "expected_decision") {
        classified.extend(
            pattern_variants(&pattern)
                .unwrap_or_else(|| panic!("expected_decision has a catch-all arm `{pattern}`")),
        );
    }
    let classified_set: BTreeSet<&str> = classified.iter().map(String::as_str).collect();
    assert_eq!(
        classified_set.len(),
        classified.len(),
        "one arm per variant"
    );
    assert_eq!(classified_set, declared_set, "every variant is classified");

    // Class A is exactly the permitted set; everything else is Class B.
    let permitted: BTreeSet<&str> = fixtures
        .iter()
        .zip(&tags)
        .filter(|(action, _)| expected_decision(action) == Decision::Permitted)
        .map(|(_, tag)| tag.as_str())
        .collect();
    assert_eq!(permitted, BTreeSet::from(PERMITTED));
    eprintln!(
        "p3e_g1: {} PlannedAction variants: {} permitted (class A), {} refused (class B)",
        declared.len(),
        permitted.len(),
        declared.len() - permitted.len()
    );
}

#[test]
fn p3e_g1_02_production_decides_every_variant_as_classified() {
    // A pure function: this only classifies, whatever the variant.
    for action in every_action() {
        let expected = match expected_decision(&action) {
            Decision::Permitted => None,
            Decision::Refused(closure) => Some(closure),
        };
        assert_eq!(
            crate::phase0_agent_action_closure(&action),
            expected,
            "{}",
            action.action_type()
        );
    }
    for &(url, closure) in WEB_FETCH_EDGES {
        let fetch = PlannedAction::WebFetch {
            url: url.to_string(),
        };
        assert_eq!(
            expected_decision(&fetch),
            Decision::Refused(closure),
            "{url:?}"
        );
        assert_eq!(
            crate::phase0_agent_action_closure(&fetch),
            Some(closure),
            "{url:?}"
        );
    }
}

#[test]
fn p3e_g1_03_the_wire_form_round_trips_to_the_same_variant_and_decision() {
    let mut actions = every_action();
    actions.extend(
        WEB_FETCH_EDGES
            .iter()
            .map(|&(url, _)| PlannedAction::WebFetch {
                url: url.to_string(),
            }),
    );
    for action in &actions {
        let wire = serde_json::to_value(action).expect("an action serializes");
        let parsed: PlannedAction =
            serde_json::from_value(wire.clone()).unwrap_or_else(|error| panic!("{wire}: {error}"));
        assert_eq!(
            serde_json::to_value(&parsed).unwrap(),
            wire,
            "stable wire form"
        );
        assert_eq!(parsed.action_type(), action.action_type(), "{wire}");
        assert_eq!(
            expected_decision(&parsed),
            expected_decision(action),
            "{wire}"
        );
        assert_eq!(
            crate::phase0_agent_action_closure(&parsed),
            crate::phase0_agent_action_closure(action),
            "{wire}"
        );
    }
}

/// The real production executor refuses with exactly the classified closure,
/// even when the step is marked approved, so the refusal is the wrapper's
/// and not the registry's. Every fixture here is inert even if it reached
/// the registry: the read targets an absent file for an agent the supervisor
/// does not know (the registry stops at that lookup, before any actuator),
/// and the registry routes none of the governance actions. Input, screen,
/// shell, browser, network and write actions are never executed here; the
/// classification and structure tests cover them.
#[test]
fn p3e_g1_04_the_executor_refuses_before_its_registry_even_when_approved() {
    use nexus_kernel::cognitive::loop_runtime::ActionExecutor;

    let state = crate::AppState::new_in_memory();
    let memory = std::sync::Arc::new(nexus_kernel::cognitive::AgentMemoryManager::new(Box::new(
        crate::DbMemoryStore {
            db: state.db.clone(),
        },
    )));
    let executor = crate::phase0_agent_executor(&state, memory);
    let mut audit = state.audit.clone();
    let agent = uuid::Uuid::new_v4().to_string();
    let absent_dir = std::env::temp_dir().join(format!("nexus-p3e-g1-{}", uuid::Uuid::new_v4()));
    let refused = [
        PlannedAction::FileRead {
            path: absent_dir.join("absent.txt").to_string_lossy().into_owned(),
        },
        PlannedAction::SelfModifyDescription {
            new_description: probe(),
        },
        PlannedAction::SelfModifyStrategy {
            strategy_key: probe(),
            new_strategy: probe(),
        },
        PlannedAction::CreateSubAgent {
            manifest_json: "{}".into(),
        },
        PlannedAction::DestroySubAgent {
            agent_id: agent.clone(),
        },
        PlannedAction::RunEvolutionTournament {
            variants: vec![],
            task: probe(),
            rounds: 1,
        },
        PlannedAction::ModifyGovernancePolicy {
            policy_key: probe(),
            policy_value: probe(),
        },
        PlannedAction::AllocateEcosystemFuel {
            agent_id: agent.clone(),
            amount: 1.0,
        },
    ];
    for action in &refused {
        let Decision::Refused(closure) = expected_decision(action) else {
            panic!("{} must be refused", action.action_type());
        };
        for approved in [true, false] {
            assert_eq!(
                executor.execute(&agent, action, &mut audit, approved),
                Err(closed(action.action_type(), closure)),
                "{} (approved: {approved})",
                action.action_type()
            );
        }
    }
    assert!(!absent_dir.exists(), "nothing was created");

    // Permitted actions still run (none reaches outside the in-memory state).
    let permitted = [
        PlannedAction::Noop,
        PlannedAction::MemoryStore {
            key: probe(),
            value: "inert".into(),
            memory_type: "episodic".into(),
        },
        PlannedAction::MemoryRecall {
            query: probe(),
            memory_type: None,
        },
        PlannedAction::SendNotification {
            title: probe(),
            body: "inert".into(),
            level: "info".into(),
        },
        PlannedAction::AgentMessage {
            target_agent: agent.clone(),
            message: "inert".into(),
        },
        PlannedAction::HitlRequest {
            question: "inert?".into(),
            options: vec![],
        },
    ];
    for action in &permitted {
        assert_eq!(expected_decision(action), Decision::Permitted);
        let result = executor.execute(&agent, action, &mut audit, false);
        assert!(result.is_ok(), "{}: {result:?}", action.action_type());
    }
    assert_eq!(
        executor.execute(&agent, &PlannedAction::Noop, &mut audit, false),
        Ok("ok".to_string())
    );
}

#[test]
fn p3e_g1_05_the_executor_consults_the_closure_before_its_inner_registry() {
    let path = "app/src-tauri/src/commands/cognitive.rs";
    let cognitive = production_source(path);
    let methods: Vec<FnItem> = fn_items(cognitive)
        .into_iter()
        .filter(|item| item.path.starts_with("Phase0AgentExecutor::"))
        .collect();
    assert_eq!(
        methods.len(),
        1,
        "Phase0AgentExecutor has exactly one method"
    );
    assert_eq!(methods[0].path, "Phase0AgentExecutor::execute");
    assert_eq!(methods[0].trait_name, "ActionExecutor");
    assert_eq!(compact(methods[0].body_text(cognitive)), EXECUTOR_BODY);

    // Its only state is the inner executor, and it has one impl block.
    assert_eq!(struct_parts(cognitive, "Phase0AgentExecutor").1, "inner:E");
    let mut blocks = Vec::new();
    for file in files_naming("Phase0AgentExecutor") {
        for block in impl_blocks(&masked(production_source(file))) {
            if block.self_type == "Phase0AgentExecutor" {
                blocks.push((file, block.trait_name));
            }
        }
    }
    assert_eq!(blocks, [(path, "ActionExecutor".to_string())]);
}

#[test]
fn p3e_g1_06_the_production_permitted_arm_and_default_deny_are_pinned() {
    let cognitive = production_source("app/src-tauri/src/commands/cognitive.rs");
    let functions: Vec<FnItem> = fn_items(cognitive)
        .into_iter()
        .filter(|item| item.path == "phase0_agent_action_closure")
        .collect();
    assert_eq!(functions.len(), 1, "one production closure function");
    let body = compact(functions[0].body_text(cognitive));

    // The body is its `use` declarations and the match; the match is its
    // only value and the permitting arm its only `None`.
    let decision = after_uses(&body);
    assert!(decision.starts_with("matchaction{"), "{body}");
    let m = masked(decision);
    assert_eq!(close_of(&m, "matchaction".len()), decision.len(), "{body}");
    assert_eq!(
        words(&masked(&body), "None").len(),
        1,
        "one permitting branch"
    );

    let arms = match_arms(cognitive, "phase0_agent_action_closure");
    let (last_pattern, last_value) = arms.last().expect("arms");
    assert_eq!(
        (last_pattern.as_str(), last_value.as_str()),
        ("_", "Some(Closure::AgentExecution)"),
        "the default refuses"
    );
    for (pattern, _) in &arms[..arms.len() - 1] {
        assert!(
            pattern_variants(pattern).is_some(),
            "only the last arm may be a catch-all: `{pattern}`"
        );
    }
    let permitting: Vec<&(String, String)> =
        arms.iter().filter(|(_, value)| value == "None").collect();
    assert_eq!(permitting.len(), 1, "one permitting arm");
    let permitted: BTreeSet<String> = pattern_variants(&permitting[0].0)
        .expect("explicit variants")
        .into_iter()
        .collect();
    assert_eq!(
        permitted,
        PERMITTED.iter().map(|name| name.to_string()).collect(),
        "the permitted arm"
    );
    let fetch: Vec<&(String, String)> = arms
        .iter()
        .filter(|(pattern, _)| pattern_variants(pattern) == Some(vec!["WebFetch".to_string()]))
        .collect();
    assert_eq!(fetch.len(), 1, "one fetch arm");
    assert!(
        fetch[0].1.contains("Some(Closure::NetworkDestination)")
            && fetch[0].1.contains("Some(Closure::AgentExecution)")
            && words(&masked(&fetch[0].1), "None").is_empty(),
        "a fetch is refused for every scheme: {}",
        fetch[0].1
    );
}

// ---------------------------------------------------------------------------
// G3: the semantic Phase Zero closure inventory
// ---------------------------------------------------------------------------

/// The gate that closed each `Closure` variant. No wildcard: a variant added
/// to the enum does not compile here until it is classified (`p3e_g3_05`
/// also scans this function for a catch-all).
fn closure_gate(closure: Closure) -> &'static str {
    match closure {
        Closure::FileSelection => "P0-002C5A E1: a raw path is not authority",
        Closure::LegacyBuilder => "P0-002C5A E2: the retired raw-path Builder",
        Closure::ProcessExecution => "P0-002C5A E3: program text is not process authority",
        Closure::ApprovalRequired => "P0-002C5A E3: a caller's assertion is not approval",
        Closure::ExternalCliAgent => "P0-002C5A E3: external CLI agents",
        Closure::AgentExecution => "P0-002C5A E4: agent filesystem and process actions",
        Closure::AmbientResource => "P0-002C5A E5: ambient resource locations",
        Closure::OsInput => "C5C: OS input from the interface or a model",
        Closure::ScreenObservation => "C5C: screen observation over IPC",
        Closure::NetworkDestination => "Final Gate B: a chosen destination is not egress",
        Closure::PeerTransfer => "Final Gate F: an unpaired, unauthenticated peer",
        Closure::CredentialTransport => "Final Gate C: a credential on a command line",
        Closure::HelperLaunch => "Final Gate I: helper programs",
        Closure::SecretStorage => "Final Gate A and H: a secret outside an approved store",
        Closure::SimulationReplay => "Phase One charter 12: time-machine what-if",
        Closure::CheckpointReplay => "P2-ENTRY-H1: checkpoint replay",
    }
}

/// Every `Closure` variant (`p3e_g3_05` proves the list complete).
const ALL_CLOSURES: [Closure; 16] = [
    Closure::FileSelection,
    Closure::LegacyBuilder,
    Closure::ProcessExecution,
    Closure::ApprovalRequired,
    Closure::ExternalCliAgent,
    Closure::AgentExecution,
    Closure::AmbientResource,
    Closure::OsInput,
    Closure::ScreenObservation,
    Closure::NetworkDestination,
    Closure::PeerTransfer,
    Closure::CredentialTransport,
    Closure::HelperLaunch,
    Closure::SecretStorage,
    Closure::SimulationReplay,
    Closure::CheckpointReplay,
];

/// The Phase Three entry inventory of canonical closed commands, as
/// (defining file under the desktop `src`, closure, commands). Each command's
/// handler takes no input and its whole body is the denial for its closure.
/// Reopening one fails `p3e_g3_02` until it leaves this inventory with an
/// Architect-approved authority mechanism.
const CANONICAL_CLOSED: &[(&str, Closure, &[&str])] = &[
    (
        "lib.rs",
        Closure::AmbientResource,
        &[
            "breed_agents",
            "evolve_population",
            "force_evolve_agent",
            "generate_all_genomes",
            "genesis_analyze_gap",
            "genesis_create_agent",
            "genesis_delete_agent",
            "genesis_list_generated",
            "genesis_preview_agent",
            "genesis_store_pattern",
            "get_agent_genome",
            "get_agent_lineage",
            "get_git_repo_status",
            "mutate_agent",
            "self_rewrite_analyze",
            "self_rewrite_rollback",
            "self_rewrite_test_patch",
            "transcribe_push_to_talk",
            "trigger_immune_scan",
            "voice_pipeline_health",
            "voice_start_listening",
        ],
    ),
    (
        "lib.rs",
        Closure::ApprovalRequired,
        &["self_rewrite_apply_patch", "terminal_execute_approved"],
    ),
    (
        "lib.rs",
        Closure::CheckpointReplay,
        &[
            "time_machine_redo",
            "time_machine_undo",
            "time_machine_undo_checkpoint",
        ],
    ),
    (
        "lib.rs",
        Closure::ExternalCliAgent,
        &[
            "builder_authenticate_cli",
            "builder_check_cli_auth",
            "detect_claude_code_cli",
            "detect_codex_cli",
            "trigger_claude_code_login",
            "trigger_codex_cli_login",
        ],
    ),
    (
        "lib.rs",
        Closure::FileSelection,
        &[
            "airgap_create_bundle",
            "airgap_install_bundle",
            "airgap_validate_bundle",
            "analyze_media_file",
            "backup_restore",
            "backup_verify",
            "cogfs_index_file",
            "cogfs_watch_directory",
            "db_connect",
            "db_disconnect",
            "db_execute_query",
            "db_export_table",
            "db_list_tables",
            "factory_create_project",
            "file_manager_create_dir",
            "file_manager_delete",
            "file_manager_list",
            "file_manager_read",
            "file_manager_rename",
            "file_manager_write",
            "index_document",
            "voice_load_whisper_model",
        ],
    ),
    ("lib.rs", Closure::HelperLaunch, &["is_ollama_installed"]),
    (
        "lib.rs",
        Closure::LegacyBuilder,
        &[
            "builder_archive_project",
            "builder_collab_add_comment",
            "builder_collab_get_comments",
            "builder_collab_invite",
            "builder_collab_leave",
            "builder_collab_resolve_comment",
            "builder_collab_set_role",
            "builder_collab_start_hosting",
            "builder_conversion_auto_fix",
            "builder_conversion_check",
            "builder_delete_project",
            "builder_deploy",
            "builder_deploy_diff",
            "builder_deploy_drift",
            "builder_deploy_history",
            "builder_deploy_rollback",
            "builder_deploy_rollback_to",
            "builder_deploy_share_info",
            "builder_export_audit_trail",
            "builder_export_project",
            "builder_generate_all_images",
            "builder_generate_image",
            "builder_generate_section_variants",
            "builder_generate_trust_pack",
            "builder_generate_variants",
            "builder_get_audit_trail",
            "builder_import_design",
            "builder_init_checkpoint",
            "builder_iterate",
            "builder_list_checkpoints",
            "builder_list_projects",
            "builder_load_plan",
            "builder_load_project",
            "builder_load_state",
            "builder_quality_auto_fix",
            "builder_quality_auto_fix_all",
            "builder_quality_check",
            "builder_read_preview",
            "builder_rollback",
            "builder_save_state",
            "builder_theme_apply",
            "builder_theme_export",
            "builder_theme_get_current",
            "builder_unarchive_project",
            "builder_visual_edit_text",
            "builder_visual_edit_token",
            "conduct_build",
            "conduct_build_streaming",
            "read_build_file",
        ],
    ),
    (
        "lib.rs",
        Closure::NetworkDestination,
        &[
            "a2a_cancel_task",
            "a2a_discover_agent",
            "a2a_get_task_status",
            "a2a_send_task",
            "api_client_request",
            "builder_theme_extract_from_url",
            "mcp_host_call_tool",
            "mcp_host_connect",
        ],
    ),
    (
        "lib.rs",
        Closure::OsInput,
        &["computer_control_execute_action", "start_computer_action"],
    ),
    ("lib.rs", Closure::PeerTransfer, &["nexus_link_send_model"]),
    (
        "lib.rs",
        Closure::ProcessExecution,
        &[
            "factory_build_project",
            "factory_run_pipeline",
            "factory_test_project",
            "terminal_execute",
        ],
    ),
    (
        "lib.rs",
        Closure::ScreenObservation,
        &[
            "analyze_screen",
            "capture_screen",
            "computer_control_capture_screen",
        ],
    ),
    (
        "lib.rs",
        Closure::SecretStorage,
        &[
            "builder_backend_connect",
            "builder_deploy_store_credentials",
            "email_start_oauth",
            "integration_start_oauth",
        ],
    ),
    (
        "lib.rs",
        Closure::SimulationReplay,
        &["time_machine_what_if"],
    ),
    (
        "commands/crate_bridges.rs",
        Closure::AmbientResource,
        &[
            "cm_execute_validation_run",
            "cm_get_validation_run",
            "cm_list_validation_runs",
            "cm_run_ab_validation",
            "cm_three_way_comparison",
            "mcp2_server_handle",
            "memory_list_agents",
            "memory_load",
            "memory_save",
        ],
    ),
    (
        "commands/crate_bridges.rs",
        Closure::CredentialTransport,
        &["perception_init"],
    ),
    (
        "commands/crate_bridges.rs",
        Closure::FileSelection,
        &["browser_screenshot"],
    ),
    (
        "commands/crate_bridges.rs",
        Closure::NetworkDestination,
        &[
            "a2a_crate_discover_agent",
            "a2a_crate_get_task",
            "a2a_crate_send_task",
        ],
    ),
    (
        "commands/crate_bridges.rs",
        Closure::ProcessExecution,
        &[
            "cc_execute_action",
            "mcp2_client_add",
            "mcp2_client_call",
            "mcp2_client_discover",
        ],
    ),
    (
        "commands/flash.rs",
        Closure::FileSelection,
        &[
            "flash_auto_configure",
            "flash_create_session",
            "flash_enable_speculative",
            "flash_estimate_performance",
            "flash_profile_model",
            "flash_run_benchmark",
        ],
    ),
    (
        "commands/orchestration.rs",
        Closure::AgentExecution,
        &["run_content_pipeline"],
    ),
    (
        "nx_bridge/commands.rs",
        Closure::AgentExecution,
        &["nx_chat", "nx_tool"],
    ),
    (
        "nx_bridge/commands.rs",
        Closure::ApprovalRequired,
        &["nx_agent_approve", "nx_consent_respond"],
    ),
    (
        "nx_bridge/commands.rs",
        Closure::ProcessExecution,
        &["nx_agent_run"],
    ),
    (
        "nx_bridge/commands.rs",
        Closure::ScreenObservation,
        &["nx_computer_use_screenshot"],
    ),
];

/// Every other production `closed(..)` call, as (file, function, surface,
/// closure), normalized. A surface or closure that is not a literal is the
/// expression that computes it.
const CONDITIONAL_CLOSED: &[(&str, &str, &str, &str)] = &[
    (
        "commands/agents.rs",
        "create_agent",
        "\"create_agent\"",
        "Closure::ApprovalRequired",
    ),
    (
        "commands/agents.rs",
        "resume_agent",
        "\"resume_agent\"",
        "Closure::ApprovalRequired",
    ),
    (
        "commands/agents.rs",
        "start_agent",
        "\"start_agent\"",
        "Closure::ApprovalRequired",
    ),
    (
        "commands/chat_llm.rs",
        "ollama_base_url_for",
        "surface",
        "Closure::NetworkDestination",
    ),
    (
        "commands/chat_llm.rs",
        "ollama_service_answers",
        "\"ensure_ollama\"",
        "Closure::HelperLaunch",
    ),
    (
        "commands/chat_llm.rs",
        "provider_from_prefixed_model",
        "\"flash model\"",
        "Closure::FileSelection",
    ),
    (
        "commands/cognitive.rs",
        "Phase0AgentExecutor::execute",
        "action.action_type()",
        "closure",
    ),
    (
        "commands/cognitive.rs",
        "ScheduledGoalExecutor::execute",
        "\"assign_agent_goal\"",
        "Closure::ApprovalRequired",
    ),
    (
        "commands/cognitive.rs",
        "assign_agent_goal",
        "\"assign_agent_goal\"",
        "Closure::ApprovalRequired",
    ),
    (
        "commands/cognitive.rs",
        "start_autonomous_loop",
        "\"start_autonomous_loop\"",
        "Closure::ApprovalRequired",
    ),
    (
        "commands/consent.rs",
        "approve_consent_request",
        "\"approve_consent_request\"",
        "Closure::ApprovalRequired",
    ),
    (
        "commands/consent.rs",
        "batch_approve_consents",
        "\"batch_approve_consents\"",
        "Closure::ApprovalRequired",
    ),
    (
        "commands/consent.rs",
        "review_consent_batch",
        "\"review_consent_batch\"",
        "Closure::ApprovalRequired",
    ),
    (
        "commands/crate_bridges.rs",
        "tool_call_autonomy",
        "\"tools_execute\"",
        "Closure::ApprovalRequired",
    ),
    (
        "commands/tools_infra.rs",
        "execute_tool",
        "\"execute_tool\"",
        "closure",
    ),
    (
        "commands/trust_security.rs",
        "computer_control_toggle",
        "\"computer_control_toggle\"",
        "Closure::ScreenObservation",
    ),
];

/// Functions that compute a closure without calling `closed(..)` (their
/// callers are pinned conditional sites).
const COMPUTED_CLOSURES: &[(&str, &str)] =
    &[("commands/cognitive.rs", "phase0_agent_action_closure")];

/// Registered handlers outside the closure framework whose whole body is a
/// constant error, as (file, command, whether the body reads an argument).
const STUB_DENIALS: &[(&str, &str, bool)] = &[("lib.rs", "builder_build_static", false)];

/// The closure structure of the desktop's production source.
#[derive(Default)]
struct ClosureInventory {
    /// (file, command, closure variant, parameters) of each handler whose
    /// whole body is a canonical denial.
    canonical: BTreeSet<(String, String, String, String)>,
    /// (file, function, surface, closure) of every other `closed(..)` call,
    /// sorted.
    conditional: Vec<(String, String, String, String)>,
    /// Each function naming `Closure`, as (file, function), with the
    /// variants it names.
    naming: BTreeMap<(String, String), BTreeSet<String>>,
    /// What could not be classified.
    unclassified: Vec<String>,
}

/// Every production `closed` and `Closure` reference of `sources` (path,
/// production text), classified. A reference the inventory cannot classify
/// is recorded as unclassified: a second function named `closed`, a rename,
/// a path or an import that does not lead to `phase0_surface`, `closed` used
/// other than by a call, a call outside a function, or `Closure` named
/// outside a function or a `use`.
fn classify_closures(sources: &[(&str, &str)]) -> ClosureInventory {
    let mut inventory = ClosureInventory::default();
    for &(file, text) in sources {
        let m = masked(text);
        let (calls, names) = (words(&m, "closed"), words(&m, "Closure"));
        if calls.is_empty() && names.is_empty() {
            continue;
        }
        let functions = fn_items(text);
        let uses = use_spans(&m);
        let use_at = |at: usize| {
            uses.iter()
                .find(|&&(start, end)| start <= at && at < end)
                .copied()
        };
        // An import or a path names the item only through `phase0_surface`.
        let foreign = |at: usize| match use_at(at) {
            Some((start, end)) => words(&m[start..end], "phase0_surface").is_empty(),
            None => path_before(&m, at)
                .last()
                .is_some_and(|&segment| segment != "phase0_surface"),
        };
        for at in calls {
            if previous_byte(&m, at) == Some(b'.') {
                continue; // a method of some other type
            }
            let next = skip_ws(&m, at + "closed".len());
            let unclassified = if previous_word(&m, at) == "fn" {
                Some("a function named `closed`")
            } else if word_at(&m, next, "as") {
                Some("`closed` renamed")
            } else if foreign(at) {
                Some("`closed` reached through another path")
            } else if m.get(next) != Some(&b'(') {
                use_at(at)
                    .is_none()
                    .then_some("`closed` used other than by a call")
            } else if let Some(function) = enclosing(&functions, at) {
                let args: Vec<String> = split_top_level(&m, next + 1, close_of(&m, next) - 1)
                    .into_iter()
                    .map(|(start, end)| normalized(&text[start..end]))
                    .collect();
                match canonical_denial(function.body_text(text)) {
                    Some((surface, closure))
                        if surface == function.name && function.path == function.name =>
                    {
                        inventory.canonical.insert((
                            file.to_string(),
                            function.name.clone(),
                            closure,
                            function.params.clone(),
                        ));
                        None
                    }
                    _ if args.len() == 2 => {
                        inventory.conditional.push((
                            file.to_string(),
                            function.path.clone(),
                            args[0].clone(),
                            args[1].clone(),
                        ));
                        None
                    }
                    _ => Some("`closed` called without exactly two arguments"),
                }
            } else {
                Some("`closed(..)` outside any function")
            };
            if let Some(what) = unclassified {
                inventory.unclassified.push(format!("{file}: {what}"));
            }
        }
        for at in names {
            let next = skip_ws(&m, at + "Closure".len());
            let unclassified = if word_at(&m, next, "as") {
                Some("`Closure` renamed")
            } else if foreign(at) {
                Some("`Closure` reached through another path")
            } else if let Some(function) = enclosing(&functions, at) {
                let named = inventory
                    .naming
                    .entry((file.to_string(), function.path.clone()))
                    .or_default();
                if m[next..].starts_with(b"::") {
                    named.insert(ident_at(&m, skip_ws(&m, next + 2)).to_string());
                }
                None
            } else {
                use_at(at)
                    .is_none()
                    .then_some("`Closure` named outside any function")
            };
            if let Some(what) = unclassified {
                inventory.unclassified.push(format!("{file}: {what}"));
            }
        }
    }
    inventory.conditional.sort();
    inventory.unclassified.sort();
    inventory
}

/// The desktop's closure inventory. `phase0_surface.rs` holds the
/// definitions themselves and is not an inventory entry.
fn closure_inventory() -> ClosureInventory {
    let sources: Vec<(&str, &str)> = desktop_sources()
        .into_iter()
        .filter(|&(file, _)| file != "phase0_surface.rs")
        .collect();
    classify_closures(&sources)
}

/// The (surface, closure variant) of a function body that is exactly a
/// canonical denial, `Err(closed("<surface>", Closure::<variant>))`, however
/// it is spaced, qualified or trailing-comma'd.
fn canonical_denial(body: &str) -> Option<(String, String)> {
    let body = compact(body);
    let inner = body.strip_prefix("Err(closed(\"")?.strip_suffix("))")?;
    let (surface, variant) = inner.split_once("\",Closure::")?;
    let surface_ok = !surface.is_empty()
        && surface
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_');
    let variant_ok = variant
        .bytes()
        .next()
        .is_some_and(|c| c.is_ascii_uppercase())
        && variant.bytes().all(is_ident_byte);
    (surface_ok && variant_ok).then(|| (surface.to_string(), variant.to_string()))
}

/// `CLOSED_COMMANDS` of the Phase Zero reachability guard, as (command,
/// closure variant).
fn phase_zero_closed_commands() -> BTreeSet<(String, String)> {
    let text = production_text(&workspace_file("app/src-tauri/src/phase0_surface/tests.rs"));
    let m = masked(&text);
    let at = words(&m, "CLOSED_COMMANDS")
        .into_iter()
        .find(|&at| previous_word(&m, at) == "const")
        .expect("CLOSED_COMMANDS");
    let equals = at + m[at..].iter().position(|&c| c == b'=').expect("a value");
    let open = equals + m[equals..].iter().position(|&c| c == b'[').expect("a list");
    split_top_level(&m, open + 1, close_of(&m, open) - 1)
        .into_iter()
        .map(|(start, end)| {
            let entry = compact(&text[start..end]);
            let pair = entry
                .strip_prefix("(\"")
                .and_then(|rest| rest.strip_suffix(')'))
                .and_then(|rest| rest.split_once("\",Closure::"))
                .unwrap_or_else(|| panic!("CLOSED_COMMANDS entry {entry}"));
            (pair.0.to_string(), pair.1.to_string())
        })
        .collect()
}

/// Registered handlers outside the closure framework whose whole body,
/// after any `let _ = ..;`, is one constant `Err(..)`, as (file, command,
/// whether the body reads an argument).
fn stub_denials() -> BTreeSet<(String, String, bool)> {
    let mut stubs = BTreeSet::new();
    for (file, text) in desktop_sources() {
        if !text.contains("command") {
            continue;
        }
        for function in fn_items(text) {
            if !function.head.contains("#[command") {
                continue;
            }
            let body = function.body_text(text);
            if canonical_denial(body).is_some() {
                continue;
            }
            let normalized_body = normalized(body);
            let mut rest = normalized_body.as_str();
            while let Some(after) = rest.strip_prefix("let_=") {
                rest = after.split_once(';').map_or("", |(_, tail)| tail);
            }
            let is_one_error = rest.starts_with("Err(") && close_of(&masked(rest), 3) == rest.len();
            if is_one_error && constant_expression(&rest[4..rest.len() - 1]) {
                let m = masked(body);
                let reads = parameter_names(&function.params)
                    .iter()
                    .any(|name| !words(&m, name).is_empty());
                stubs.insert((file.to_string(), function.name.clone(), reads));
            }
        }
    }
    stubs
}

/// Whether `expression` builds only a constant: literals, upper-case
/// constants and string conversions.
fn constant_expression(expression: &str) -> bool {
    let m = masked(expression);
    let mut i = 0;
    while i < m.len() {
        if is_ident_byte(m[i]) && (i == 0 || !is_ident_byte(m[i - 1])) {
            let word = ident_at(&m, i);
            let conversion =
                ["into", "to_string", "to_owned", "String", "from", "format"].contains(&word);
            let constant = word
                .bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_');
            if !conversion && !constant {
                return false;
            }
            i += word.len();
        } else {
            i += 1;
        }
    }
    true
}

/// The names bound by a normalized parameter list (`self` excluded).
fn parameter_names(params: &str) -> Vec<String> {
    let inner = params
        .strip_prefix('(')
        .and_then(|rest| rest.strip_suffix(')'))
        .unwrap_or_default();
    let m = masked(inner);
    split_top_level(&m, 0, m.len())
        .into_iter()
        .filter_map(|(start, end)| {
            let part = &inner[start..end];
            let name = ident_at(part.as_bytes(), 0);
            let typed =
                part[name.len()..].starts_with(':') && !part[name.len()..].starts_with("::");
            (typed && !name.is_empty() && name != "self").then(|| name.to_string())
        })
        .collect()
}

/// The permissions `capabilities/app-commands.json` grants.
fn granted_permissions() -> BTreeSet<String> {
    let capability: serde_json::Value = serde_json::from_str(&workspace_file(
        "app/src-tauri/capabilities/app-commands.json",
    ))
    .expect("the app capability is JSON");
    capability["permissions"]
        .as_array()
        .expect("a permission list")
        .iter()
        .filter_map(|permission| permission.as_str().map(str::to_string))
        .collect()
}

#[test]
fn p3e_g3_01_the_registry_is_parsed_whole_and_its_order_is_irrelevant() {
    let entries = registry_entries(production_source("app/src-tauri/src/lib.rs"));
    let names: Vec<&str> = entries
        .iter()
        .map(String::as_str)
        .map(split_entry)
        .map(|(_, name)| name)
        .collect();
    let distinct: BTreeSet<&str> = names.iter().copied().collect();
    eprintln!(
        "p3e_g3: {} registered commands, {} distinct",
        names.len(),
        distinct.len()
    );
    // The whole registration, not one cut short at a bracket, is exactly the
    // app manifest.
    let manifest: BTreeSet<&str> = crate::webview_boundary::APP_COMMANDS
        .iter()
        .copied()
        .collect();
    assert_eq!(distinct, manifest);
    let registering: Vec<&str> = files_naming("generate_handler")
        .into_iter()
        .filter(|file| file.starts_with(DESKTOP_SRC))
        .collect();
    assert_eq!(
        registering,
        ["app/src-tauri/src/lib.rs"],
        "one registration"
    );

    // An attribute holding brackets and a commented `]` do not end the list,
    // and the order of the entries does not matter.
    let listed = production_text(
        "fn run() { b.invoke_handler(tauri::generate_handler![\n    a,\n    \
         #[cfg(any(target_os = \"linux\", target_os = \"macos\"))]\n    b::c,\n    \
         // ] is not the end\n    crate::d::e,\n    f,\n]); }",
    );
    assert_eq!(registry_entries(&listed), ["a", "b::c", "crate::d::e", "f"]);
    let reordered = production_text(
        "fn run() { b.invoke_handler(tauri::generate_handler![f, crate::d::e, \
         #[cfg(any(target_os = \"linux\", target_os = \"macos\"))] b::c, a]); }",
    );
    let set = |entries: Vec<String>| entries.into_iter().collect::<BTreeSet<String>>();
    assert_eq!(
        set(registry_entries(&listed)),
        set(registry_entries(&reordered))
    );
}

#[test]
fn p3e_g3_02_canonical_closed_handlers_are_exactly_the_semantic_inventory() {
    let inventory = closure_inventory();
    assert!(
        inventory.unclassified.is_empty(),
        "{:#?}",
        inventory.unclassified
    );
    let found: BTreeSet<(String, String, String)> = inventory
        .canonical
        .iter()
        .map(|(file, command, closure, _)| (file.clone(), command.clone(), closure.clone()))
        .collect();
    let listed: Vec<(String, String, String)> = CANONICAL_CLOSED
        .iter()
        .flat_map(|&(file, closure, commands)| {
            commands.iter().map(move |command| {
                (
                    file.to_string(),
                    command.to_string(),
                    format!("{closure:?}"),
                )
            })
        })
        .collect();
    let expected: BTreeSet<(String, String, String)> = listed.iter().cloned().collect();
    assert_eq!(expected.len(), listed.len(), "each command is listed once");
    let reopened: Vec<_> = expected.difference(&found).collect();
    let unlisted: Vec<_> = found.difference(&expected).collect();
    assert!(
        reopened.is_empty() && unlisted.is_empty(),
        "no longer canonically closed: {reopened:#?}\nclosed but not in the inventory: {unlisted:#?}"
    );
    for (file, command, _, params) in &inventory.canonical {
        assert_eq!(
            params, "()",
            "{file}: {command} must accept no caller input"
        );
    }
    // The Phase Zero reachability guard's table agrees entry for entry.
    let ours: BTreeSet<(String, String)> = expected
        .iter()
        .map(|(_, command, closure)| (command.clone(), closure.clone()))
        .collect();
    assert_eq!(ours, phase_zero_closed_commands(), "CLOSED_COMMANDS");
    eprintln!("p3e_g3: {} canonical closed commands", found.len());
}

#[test]
fn p3e_g3_03_every_canonical_closed_command_is_registered_once_granted_and_listed() {
    let entries = registry_entries(production_source("app/src-tauri/src/lib.rs"));
    let manifest: BTreeSet<&str> = crate::webview_boundary::APP_COMMANDS
        .iter()
        .copied()
        .collect();
    let granted = granted_permissions();
    for &(file, _, commands) in CANONICAL_CLOSED {
        for &command in commands {
            let registered: Vec<(&str, &str)> = entries
                .iter()
                .map(String::as_str)
                .map(split_entry)
                .filter(|&(_, name)| name == command)
                .collect();
            assert_eq!(registered.len(), 1, "{command} is registered once");
            assert_eq!(module_file(registered[0].0), file, "{command}: its module");
            assert!(manifest.contains(command), "{command}: in the app manifest");
            assert!(
                granted.contains(&format!("allow-{}", command.replace('_', "-"))),
                "{command}: granted, so callers get its bounded reason"
            );
        }
    }
}

#[test]
fn p3e_g3_04_conditional_and_computed_closures_are_pinned() {
    let inventory = closure_inventory();
    assert!(
        inventory.unclassified.is_empty(),
        "{:#?}",
        inventory.unclassified
    );
    let mut expected: Vec<(String, String, String, String)> = CONDITIONAL_CLOSED
        .iter()
        .map(|&(file, function, surface, closure)| {
            (file.into(), function.into(), surface.into(), closure.into())
        })
        .collect();
    expected.sort();
    assert_eq!(
        inventory.conditional, expected,
        "conditional closed(..) calls"
    );

    let canonical: BTreeSet<(String, String)> = inventory
        .canonical
        .iter()
        .map(|(file, command, _, _)| (file.clone(), command.clone()))
        .collect();
    let conditional: BTreeSet<(String, String)> = inventory
        .conditional
        .iter()
        .map(|(file, function, _, _)| (file.clone(), function.clone()))
        .collect();
    let computed: BTreeSet<(String, String)> = inventory
        .naming
        .keys()
        .filter(|key| !canonical.contains(*key) && !conditional.contains(*key))
        .cloned()
        .collect();
    let expected_computed: BTreeSet<(String, String)> = COMPUTED_CLOSURES
        .iter()
        .map(|&(file, function)| (file.into(), function.into()))
        .collect();
    assert_eq!(computed, expected_computed, "functions computing a closure");
    eprintln!(
        "p3e_g3: {} conditional closed(..) calls in {} functions; {} computed-closure functions",
        inventory.conditional.len(),
        conditional.len(),
        computed.len()
    );
}

#[test]
fn p3e_g3_05_every_closure_variant_is_classified_bounded_and_used() {
    let declared = enum_variants(
        production_source("app/src-tauri/src/phase0_surface.rs"),
        "Closure",
    );
    let declared: BTreeSet<&str> = declared.iter().map(String::as_str).collect();
    let listed: BTreeSet<String> = ALL_CLOSURES.iter().map(|c| format!("{c:?}")).collect();
    assert_eq!(
        listed.len(),
        ALL_CLOSURES.len(),
        "ALL_CLOSURES repeats a variant"
    );
    assert_eq!(
        listed
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<&str>>(),
        declared,
        "ALL_CLOSURES is every variant"
    );
    let own = production_text(&workspace_file(OWN_SOURCE));
    let mut gated = Vec::new();
    for (pattern, _) in match_arms(&own, "closure_gate") {
        gated.extend(
            pattern_variants(&pattern)
                .unwrap_or_else(|| panic!("closure_gate has a catch-all arm `{pattern}`")),
        );
    }
    let gated_set: BTreeSet<&str> = gated.iter().map(String::as_str).collect();
    assert_eq!(gated_set.len(), gated.len(), "one arm per variant");
    assert_eq!(gated_set, declared, "every variant is classified");

    // The module defines the reasons and a `closed` that only formats.
    let surface = production_source("app/src-tauri/src/phase0_surface.rs");
    let defined: Vec<(String, String, String)> = fn_items(surface)
        .into_iter()
        .map(|item| {
            let body = compact(item.body_text(surface));
            (item.path, item.params, body)
        })
        .collect();
    assert_eq!(defined.len(), 2, "{defined:?}");
    assert_eq!(defined[0].0, "Closure::reason");
    assert_eq!(
        defined[1],
        (
            "closed".to_string(),
            "(surface:&'staticstr,closure:Closure)".to_string(),
            "format!(\"{surface}: {}\",closure.reason())".to_string()
        )
    );

    // Every reason is bounded, distinct and echoes nothing.
    let mut reasons = BTreeSet::new();
    for closure in ALL_CLOSURES {
        assert!(!closure_gate(closure).is_empty());
        let reason = closure.reason();
        assert!(
            !reason.is_empty() && reason.len() <= 160,
            "{closure:?}: {reason}"
        );
        assert!(
            reason.chars().all(|c| c.is_ascii_graphic() || c == ' '),
            "{closure:?}: one plain line"
        );
        assert!(
            !reason.contains(['/', '\\', '{', '}']),
            "{closure:?}: no path or placeholder"
        );
        assert!(reasons.insert(reason), "{closure:?}: a distinct reason");
        assert_eq!(
            closed("p3e_surface", closure),
            format!("p3e_surface: {reason}")
        );
    }

    // Every variant keeps a production use: a canonical handler, a pinned
    // conditional call, or a pinned computed closure.
    let inventory = closure_inventory();
    let mut used: BTreeSet<String> = inventory
        .canonical
        .iter()
        .map(|(_, _, closure, _)| closure.clone())
        .collect();
    for (file, function, _, closure) in &inventory.conditional {
        if let Some(variant) = closure.strip_prefix("Closure::") {
            used.insert(variant.to_string());
        }
        if let Some(named) = inventory.naming.get(&(file.clone(), function.clone())) {
            used.extend(named.iter().cloned());
        }
    }
    for &(file, function) in COMPUTED_CLOSURES {
        if let Some(named) = inventory
            .naming
            .get(&(file.to_string(), function.to_string()))
        {
            used.extend(named.iter().cloned());
        }
    }
    let unused: Vec<&&str> = declared
        .iter()
        .filter(|variant| !used.contains(**variant))
        .collect();
    assert!(
        unused.is_empty(),
        "closures with no production use: {unused:?}"
    );
}

#[test]
fn p3e_g3_06_stub_denials_outside_the_framework_are_inventoried() {
    let expected: BTreeSet<(String, String, bool)> = STUB_DENIALS
        .iter()
        .map(|&(file, command, reads)| (file.into(), command.into(), reads))
        .collect();
    assert_eq!(stub_denials(), expected);
    let entries = registry_entries(production_source("app/src-tauri/src/lib.rs"));
    for &(_, command, _) in STUB_DENIALS {
        let registered = entries
            .iter()
            .map(String::as_str)
            .map(split_entry)
            .filter(|&(_, name)| name == command)
            .count();
        assert_eq!(registered, 1, "{command} is registered once");
    }
}

// ---------------------------------------------------------------------------
// G5: the Phase Two launch approval, pinned from outside Phase Two
// ---------------------------------------------------------------------------

/// The native-confirmation traits, as (trait, defining file, the only files
/// that may name it). Each has exactly one production implementation: the
/// desktop's native dialogs.
const CONFIRMERS: [(&str, &str, &[&str]); 3] = [
    (
        "VerifierLaunchConfirmer",
        "kernel/src/coding_run/verifier.rs",
        &[
            "app/src-tauri/src/coding_flow.rs",
            "app/src-tauri/src/coding_flow/verification.rs",
            "kernel/src/coding_run.rs",
            "kernel/src/coding_run/verifier.rs",
        ],
    ),
    (
        "OwnerConfirmer",
        "kernel/src/coding_run/apply.rs",
        &[
            "app/src-tauri/src/coding_flow.rs",
            "kernel/src/coding_run.rs",
            "kernel/src/coding_run/apply.rs",
        ],
    ),
    (
        "FolderPicker",
        "kernel/src/coding_run/project.rs",
        &[
            "app/src-tauri/src/coding_flow.rs",
            "kernel/src/coding_run.rs",
            "kernel/src/coding_run/project.rs",
        ],
    ),
];

/// The only files that may name `VerifierLaunchApproval`.
const APPROVAL_FILES: [&str; 2] = [
    "kernel/src/coding_run.rs",
    "kernel/src/coding_run/verifier.rs",
];

const VERIFIER: &str = "kernel/src/coding_run/verifier.rs";

/// Workspace production files naming the whole word `word`.
fn files_naming(word: &str) -> BTreeSet<&'static str> {
    workspace_sources()
        .iter()
        .filter(|(_, text)| text.contains(word) && !words(&masked(text), word).is_empty())
        .map(|(path, _)| path.as_str())
        .collect()
}

/// (file, implementing type) of every production `impl` of `trait_name`,
/// however the trait's path is written.
fn trait_impls(trait_name: &str) -> Vec<(&'static str, String)> {
    let mut impls = Vec::new();
    for file in files_naming(trait_name) {
        for block in impl_blocks(&masked(production_source(file))) {
            if block.trait_name == trait_name {
                impls.push((file, block.self_type));
            }
        }
    }
    impls
}

/// Files renaming `word` with `as`, or defining a trait named `word`.
fn renaming(word: &str) -> Vec<&'static str> {
    files_naming(word)
        .into_iter()
        .filter(|file| {
            let m = masked(production_source(file));
            words(&m, word)
                .into_iter()
                .any(|at| word_at(&m, skip_ws(&m, at + word.len()), "as"))
        })
        .collect()
}

/// (file, function) of every production reference to `name` through `.` or
/// a path (`x.name(..)`, `T::name(..)`, or `T::name` as a value) in the
/// sources whose path starts with `prefix`. A definition is not a reference.
fn references(prefix: &str, name: &str) -> Vec<(&'static str, String)> {
    let mut found = Vec::new();
    for (file, text) in workspace_sources() {
        if !file.starts_with(prefix) || !text.contains(name) {
            continue;
        }
        let m = masked(text);
        let sites: Vec<usize> = words(&m, name)
            .into_iter()
            .filter(|&at| {
                previous_word(&m, at) != "fn" && matches!(previous_byte(&m, at), Some(b'.' | b':'))
            })
            .collect();
        if sites.is_empty() {
            continue;
        }
        let functions = fn_items(text);
        for at in sites {
            let function = enclosing(&functions, at).map_or_else(String::new, |f| f.path.clone());
            found.push((file.as_str(), function));
        }
    }
    found
}

/// The one function at `path` in production `text`.
fn one_fn(text: &str, path: &str) -> FnItem {
    let mut found: Vec<FnItem> = fn_items(text)
        .into_iter()
        .filter(|item| item.path == path)
        .collect();
    assert_eq!(found.len(), 1, "one fn {path}");
    found.remove(0)
}

/// Asserts each fragment occurs in `text`, each after the one before.
fn assert_in_order(text: &str, fragments: &[&str], what: &str) {
    let mut from = 0;
    for fragment in fragments {
        let at = text[from..]
            .find(fragment)
            .unwrap_or_else(|| panic!("{what}: `{fragment}` missing or out of order in {text}"));
        from += at + fragment.len();
    }
}

#[test]
fn p3e_g5_01_only_the_native_dialogs_confirm() {
    for (name, defining, allowed) in CONFIRMERS {
        assert_eq!(
            trait_impls(name),
            [(
                "app/src-tauri/src/coding_flow.rs",
                "NativeDialogs".to_string()
            )],
            "{name}: the native dialogs are the only implementation"
        );
        let defined: Vec<&str> = files_naming(name)
            .into_iter()
            .filter(|file| {
                let m = masked(production_source(file));
                words(&m, name)
                    .into_iter()
                    .any(|at| previous_word(&m, at) == "trait")
            })
            .collect();
        assert_eq!(defined, [defining], "{name} is defined once");
        let extra: Vec<&str> = files_naming(name)
            .into_iter()
            .filter(|file| !allowed.contains(file))
            .collect();
        assert!(extra.is_empty(), "{name} is named by {extra:?}");
        assert!(renaming(name).is_empty(), "{name} is renamed");
    }
    // The launch confirmer declares the confirmation only, with no default.
    assert_eq!(
        trait_body(production_source(VERIFIER), "VerifierLaunchConfirmer"),
        "fnconfirm_launch(&self,request:&VerifierLaunchRequest)->bool;"
    );
    // Each implementation returns the native dialog's answer and reads only
    // the dialog handle and the backend's request.
    let flow = production_source("app/src-tauri/src/coding_flow.rs");
    for (path, trait_name, answer) in [
        (
            "NativeDialogs::confirm_launch",
            "VerifierLaunchConfirmer",
            ".blocking_show()",
        ),
        (
            "NativeDialogs::confirm",
            "OwnerConfirmer",
            ".blocking_show()",
        ),
        (
            "NativeDialogs::pick_folder",
            "FolderPicker",
            ".blocking_pick_folder()",
        ),
    ] {
        let function = one_fn(flow, path);
        assert_eq!(function.trait_name, trait_name, "{path}");
        let body = compact(function.body_text(flow));
        let value = tail(after_uses(&body));
        assert!(value.starts_with("self.0.dialog()"), "{path}: {value}");
        assert!(value.contains(answer), "{path}: {value}");
        assert_eq!(body.matches("self.").count(), 1, "{path}: {body}");
        let m = masked(&body);
        for word in ["true", "false", "return", "unsafe"] {
            assert!(words(&m, word).is_empty(), "{path}: `{word}` in {body}");
        }
        assert!(
            !value.contains("||") && !value.contains("&&"),
            "{path}: {value}"
        );
    }
    let launch = compact(one_fn(flow, "NativeDialogs::confirm_launch").body_text(flow));
    let shown = tail(after_uses(&launch));
    assert!(
        shown.ends_with(".blocking_show()")
            && shown.contains(".message(native_dialog_text(&request.message()))")
            && !shown.contains([';', '!']),
        "{shown}"
    );
}

#[test]
fn p3e_g5_02_the_launch_approval_cannot_be_built_cloned_or_deserialized() {
    let verifier = production_source(VERIFIER);
    // No derive beyond Debug (no Clone, Copy, Default or serde), and the
    // one field is private.
    assert_eq!(
        struct_parts(verifier, "VerifierLaunchApproval"),
        (
            "#[derive(Debug)]pub".to_string(),
            "binding:VerifierLaunchBinding".to_string()
        )
    );
    // One block of methods and no trait implementation anywhere.
    let mut blocks = Vec::new();
    for file in files_naming("VerifierLaunchApproval") {
        for block in impl_blocks(&masked(production_source(file))) {
            if block.self_type == "VerifierLaunchApproval" {
                blocks.push((file, block.trait_name));
            }
        }
    }
    assert_eq!(blocks, [(VERIFIER, String::new())]);
    let methods: BTreeMap<String, (String, String, String)> = fn_items(verifier)
        .into_iter()
        .filter(|item| item.path.starts_with("VerifierLaunchApproval::"))
        .map(|item| {
            let body = compact(item.body_text(verifier));
            (item.name, (item.head, item.params, body))
        })
        .collect();
    let names: Vec<&str> = methods.keys().map(String::as_str).collect();
    assert_eq!(names, ["binding", "confirmed"]);
    // The constructor is restricted to the kernel and only wraps a binding.
    let (head, params, body) = &methods["confirmed"];
    assert!(
        head.is_empty() || head.starts_with("pub("),
        "the constructor is not public: `{head}`"
    );
    assert_eq!(
        (params.as_str(), body.as_str()),
        ("(binding:VerifierLaunchBinding)", "Self{binding}")
    );
    let (head, params, body) = &methods["binding"];
    assert_eq!(
        (head.as_str(), params.as_str(), body.as_str()),
        ("pub", "(&self)", "&self.binding")
    );
    // Named only by the kernel's run module, never renamed, and never built
    // by a struct literal outside its own definition.
    let naming = files_naming("VerifierLaunchApproval");
    assert!(
        naming.iter().all(|file| APPROVAL_FILES.contains(file)),
        "{naming:?}"
    );
    assert!(renaming("VerifierLaunchApproval").is_empty());
    for file in APPROVAL_FILES {
        let m = masked(production_source(file));
        for word in ["unsafe", "transmute"] {
            assert!(words(&m, word).is_empty(), "{file}: `{word}`");
        }
    }
    for file in naming {
        let m = masked(production_source(file));
        for at in words(&m, "VerifierLaunchApproval") {
            if m.get(skip_ws(&m, at + "VerifierLaunchApproval".len())) == Some(&b'{') {
                assert!(
                    matches!(previous_word(&m, at), "struct" | "impl"),
                    "{file}: a VerifierLaunchApproval struct literal"
                );
            }
        }
    }
}

#[test]
fn p3e_g5_03_an_approval_is_minted_only_after_a_recorded_native_confirmation() {
    // Minted in exactly one place: every reference to the constructor, a
    // call or a function value, in the files that may name the approval.
    let mints: Vec<(&str, String)> = APPROVAL_FILES
        .iter()
        .flat_map(|file| references(file, "confirmed"))
        .collect();
    assert_eq!(
        mints,
        [(
            VERIFIER,
            "CodingRun::request_verification_approval".to_string()
        )]
    );
    // There, only after the backend asked the confirmer, recorded the
    // answer, and the answer was yes.
    let verifier = production_source(VERIFIER);
    let request = one_fn(verifier, "CodingRun::request_verification_approval");
    assert_eq!(
        request.params,
        "(&mutself,facts:&VerifierLaunchFacts,confirmer:&dynVerifierLaunchConfirmer)"
    );
    let body = compact(request.body_text(verifier));
    assert_in_order(
        &body,
        &[
            "(VerificationPhase::Materialized,Some(binding))=>binding",
            "Ok(verification)ifVerifierLaunchBinding::new(&verification,binding.inputs)==binding=>{}",
            "letconfirmed=confirmer.confirm_launch(&request);",
            "ifrecorded.is_err(){self.verifier.reset();returnErr(VerificationError::Unrecorded);}",
            "if!confirmed{self.verifier.reset();returnErr(VerificationError::Declined);}",
            "self.verifier.phase=VerificationPhase::Approved;",
        ],
        "request_verification_approval",
    );
    assert!(
        body.ends_with("Ok(VerifierLaunchApproval::confirmed(binding))"),
        "{body}"
    );
    // The kernel consults a launch confirmer nowhere else.
    assert_eq!(
        references("kernel/src/", "confirm_launch"),
        [(
            VERIFIER,
            "CodingRun::request_verification_approval".to_string()
        )]
    );
}

#[test]
fn p3e_g5_04_a_launch_consumes_the_approval_and_rechecks_the_live_binding() {
    let verifier = production_source(VERIFIER);
    let begin = one_fn(verifier, "CodingRun::begin_verification");
    assert_eq!(
        begin.params, "(&mutself,approval:VerifierLaunchApproval,inputs:VerifierInputs)",
        "the approval is taken by value"
    );
    let body = compact(begin.body_text(verifier));
    assert_in_order(
        &body,
        &[
            "(VerificationPhase::Approved,Some(binding))=>binding",
            "self.current_structural(\"begin_verification\")",
            "letcurrent=VerifierLaunchBinding::new(&verification,inputs);",
            "ifapproval.binding!=approved||approval.binding!=current{self.verifier.reset();returnErr(VerificationError::Stale);}",
            "EventKind::VerificationLaunched",
            "self.verifier.phase=VerificationPhase::Starting;",
        ],
        "begin_verification",
    );
    // The approval is only compared, never kept or passed on.
    let m = masked(&body);
    for at in words(&m, "approval") {
        assert!(body[at..].starts_with("approval.binding!="), "{body}");
    }

    // The desktop requests the approval and launches in one place, computing
    // the inputs again immediately before the launch.
    let path = "app/src-tauri/src/coding_flow/verification.rs";
    let flow = production_source(path);
    let start = one_fn(flow, "CodingFlow::start_verification");
    assert!(
        start
            .params
            .contains("confirmer:&dynVerifierLaunchConfirmer"),
        "{}",
        start.params
    );
    assert_in_order(
        &compact(start.body_text(flow)),
        &[
            "run.request_verification_approval(&facts_for(profile),confirmer)",
            "run.begin_verification(approval,inputs_for(profile,&toolchain))",
        ],
        "start_verification",
    );
    for method in ["request_verification_approval", "begin_verification"] {
        assert_eq!(
            references("", method),
            [(path, "CodingFlow::start_verification".to_string())],
            "{method}"
        );
    }
    // The flow's launch is reached once, from the IPC, with the native
    // dialogs as the confirmer; any path reference is to the IPC entry.
    let mut confirmers = Vec::new();
    for (file, text) in desktop_sources() {
        let m = masked(text);
        for at in words(&m, "start_verification") {
            if previous_word(&m, at) == "fn" {
                continue;
            }
            match previous_byte(&m, at) {
                Some(b'.') => {
                    let open = skip_ws(&m, at + "start_verification".len());
                    let args = split_top_level(&m, open + 1, close_of(&m, open) - 1);
                    let &(start, end) = args.last().expect("arguments");
                    confirmers.push((file, normalized(&text[start..end])));
                }
                Some(b':') => assert_eq!(
                    path_before(&m, at).last(),
                    Some(&"ipc"),
                    "{file}: start_verification reached other than through the IPC"
                ),
                _ => panic!("{file}: start_verification used unqualified"),
            }
        }
    }
    assert_eq!(
        confirmers,
        [("coding_flow.rs", "&NativeDialogs(app)".to_string())]
    );
}

#[test]
fn p3e_g5_05_the_compile_fail_non_forgeability_doctests_remain_and_run() {
    let blocks = compile_fail_blocks(&workspace_file("kernel/src/coding_run.rs"));
    let mut matched = BTreeSet::new();
    for (what, marker) in [
        ("struct literal", "VerifierLaunchApproval{binding}"),
        (
            "crate-private constructor",
            "VerifierLaunchApproval::confirmed(binding)",
        ),
        ("deserializer", "let_:VerifierLaunchApproval=from_str("),
        ("clone", "(approval.clone(),approval)"),
    ] {
        let block = blocks
            .iter()
            .position(|block| block.contains("VerifierLaunchApproval") && block.contains(marker))
            .unwrap_or_else(|| panic!("no compile_fail doctest forges the approval by {what}"));
        assert!(matched.insert(block), "{what}: its own doctest");
    }
    // The kernel's doctests are not disabled, and its run module is public,
    // so those blocks keep compiling (and failing) under `cargo test`.
    let manifest = workspace_file("kernel/Cargo.toml");
    let lib = manifest
        .split("\n[")
        .find(|section| section.starts_with("lib]"))
        .expect("[lib]");
    assert!(
        !lib.lines()
            .any(|line| line.trim_start().starts_with("doctest")),
        "{lib}"
    );
    assert!(normalized(production_source("kernel/src/lib.rs")).contains("pubmodcoding_run;"));
}

// ---------------------------------------------------------------------------
// Sources
// ---------------------------------------------------------------------------

/// This module's own source (scanned for its classifiers' arms).
const OWN_SOURCE: &str = "app/src-tauri/src/p3_entry_g1_tests.rs";

/// The desktop crate's production sources, relative to the workspace root.
const DESKTOP_SRC: &str = "app/src-tauri/src/";

/// (workspace-relative path, production text) of every workspace production
/// source, scanned once per test run.
fn workspace_sources() -> &'static [(String, String)] {
    static SOURCES: OnceLock<Vec<(String, String)>> = OnceLock::new();
    SOURCES.get_or_init(workspace_production_sources)
}

/// The desktop's production sources, as (path under its `src`, text).
fn desktop_sources() -> Vec<(&'static str, &'static str)> {
    workspace_sources()
        .iter()
        .filter_map(|(path, text)| Some((path.strip_prefix(DESKTOP_SRC)?, text.as_str())))
        .collect()
}

/// The production text of one workspace production source.
fn production_source(path: &str) -> &'static str {
    workspace_sources()
        .iter()
        .find(|(scanned, _)| scanned == path)
        .map(|(_, text)| text.as_str())
        .unwrap_or_else(|| panic!("{path} is not a scanned production source"))
}

/// A workspace file exactly as it is on disk, comments included.
fn workspace_file(path: &str) -> String {
    let full = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn without_whitespace(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

// ---------------------------------------------------------------------------
// Module-local copies of the Phase Zero production-text scanner
// (`phase0_surface/tests.rs`). `p3e_scan_01` proves each is the same code.
// ---------------------------------------------------------------------------

/// Directories that hold no production source: build output, dependencies,
/// benchmarks, integration tests, examples, benches and fixtures. Dot
/// directories are skipped as well.
const NOT_PRODUCTION_DIRS: &[&str] = &[
    "target",
    "node_modules",
    "dist",
    "benchmarks",
    "tests",
    "examples",
    "benches",
    "fixtures",
];

/// (workspace-relative path, production text) of every production Rust
/// source beneath a `src` directory of the workspace.
fn workspace_production_sources() -> Vec<(String, String)> {
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                if !name.starts_with('.') && !NOT_PRODUCTION_DIRS.contains(&name.as_str()) {
                    walk(root, &path, out);
                }
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let in_src = relative.split('/').any(|component| component == "src");
            if in_src
                && name.ends_with(".rs")
                && !name.ends_with("tests.rs")
                && !name.ends_with("_test.rs")
                && name != "build.rs"
            {
                let text = std::fs::read_to_string(&path).unwrap();
                out.push((relative, production_text(&text)));
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    assert!(out.len() > 500, "workspace sources not found");
    out
}

/// End of the string, raw-string or char literal starting at `i`, if one
/// starts there (a lifetime is not a literal).
fn literal_end(b: &[u8], i: usize) -> Option<usize> {
    let ident = |at: usize| b[at].is_ascii_alphanumeric() || b[at] == b'_';
    match b[i] {
        b'"' => {
            let mut j = i + 1;
            while j < b.len() {
                match b[j] {
                    b'\\' => j += 2,
                    b'"' => return Some(j + 1),
                    _ => j += 1,
                }
            }
            Some(b.len())
        }
        b'r' if i == 0 || !ident(i - 1) || (b[i - 1] == b'b' && (i == 1 || !ident(i - 2))) => {
            let mut j = i + 1;
            while j < b.len() && b[j] == b'#' {
                j += 1;
            }
            if j >= b.len() || b[j] != b'"' {
                return None;
            }
            let hashes = j - i - 1;
            let mut k = j + 1;
            while k < b.len() {
                if b[k] == b'"' && b[k + 1..].iter().take_while(|&&c| c == b'#').count() >= hashes {
                    return Some(k + 1 + hashes);
                }
                k += 1;
            }
            Some(b.len())
        }
        b'\'' if i + 2 < b.len() => {
            if b[i + 1] == b'\\' {
                let close = b[i + 3..].iter().position(|&c| c == b'\'')?;
                return Some(i + 3 + close + 1);
            }
            let width = match b[i + 1] {
                0x00..=0x7f => 1,
                0xc0..=0xdf => 2,
                0xe0..=0xef => 3,
                _ => 4,
            };
            (b.get(i + 1 + width) == Some(&b'\'')).then_some(i + 2 + width)
        }
        _ => None,
    }
}

/// End of the comment starting at `i`, if one starts there. A line comment
/// ends before its newline; block comments nest.
fn comment_end(b: &[u8], i: usize) -> Option<usize> {
    if b[i..].starts_with(b"//") {
        return Some(
            b[i..]
                .iter()
                .position(|&c| c == b'\n')
                .map_or(b.len(), |at| i + at),
        );
    }
    if !b[i..].starts_with(b"/*") {
        return None;
    }
    let (mut depth, mut j) = (0usize, i);
    while j < b.len() {
        if b[j..].starts_with(b"/*") {
            depth += 1;
            j += 2;
        } else if b[j..].starts_with(b"*/") {
            depth -= 1;
            j += 2;
            if depth == 0 {
                return Some(j);
            }
        } else {
            j += 1;
        }
    }
    Some(b.len())
}

/// End of what a `#[cfg(..)]` attribute ending at `i` applies to. An item
/// (or statement) ends at its `;` outside any bracket or at the brace closing
/// its block. A field, variant, match arm or argument also ends at its `,`,
/// or just before the bracket closing the enclosing list.
fn item_end(b: &[u8], mut i: usize) -> usize {
    let first = b[i..]
        .iter()
        .position(|c| !c.is_ascii_whitespace())
        .map_or(b.len(), |at| i + at);
    let word: String = b[first..]
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric() || **c == b'_')
        .map(|&c| c as char)
        .collect();
    let item = b.get(first) == Some(&b'#')
        || [
            "mod",
            "fn",
            "pub",
            "use",
            "impl",
            "struct",
            "enum",
            "const",
            "static",
            "type",
            "trait",
            "macro_rules",
            "async",
            "unsafe",
            "extern",
            "let",
        ]
        .contains(&word.as_str());
    let (mut nesting, mut braces) = (0usize, 0usize);
    while i < b.len() {
        if let Some(end) = literal_end(b, i).or_else(|| comment_end(b, i)) {
            i = end;
            continue;
        }
        let outermost = nesting == 0 && braces == 0;
        match b[i] {
            b'(' | b'[' => nesting += 1,
            b')' | b']' if outermost => return i,
            b')' | b']' => nesting = nesting.saturating_sub(1),
            b'{' => braces += 1,
            b'}' if braces == 0 => return i,
            b'}' => {
                braces -= 1;
                if braces == 0 && nesting == 0 {
                    return i + 1;
                }
            }
            b';' if outermost => return i + 1,
            b',' if outermost && !item => return i + 1,
            _ => {}
        }
        i += 1;
    }
    b.len()
}

/// Production text of a Rust source: comments removed, and every item under
/// `#[cfg(test)]` or `#[cfg(any(test, ..))]` removed. Literals are kept and
/// are never read as delimiters or attributes.
fn production_text(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if let Some(end) = literal_end(b, i) {
            out.extend_from_slice(&b[i..end]);
            i = end;
        } else if let Some(end) = comment_end(b, i) {
            out.push(b' ');
            i = end;
        } else if b[i..].starts_with(b"#[cfg(test)]") || b[i..].starts_with(b"#[cfg(any(test") {
            let attribute = i + b[i..].windows(2).position(|w| w == b")]").unwrap() + 2;
            i = item_end(b, attribute);
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("cuts fall on ASCII boundaries")
}

// ---------------------------------------------------------------------------
// Structure, read from production text
// ---------------------------------------------------------------------------

/// `text` with the contents of every string, raw-string and char literal
/// blanked (delimiters kept, length unchanged): structure is never read
/// inside a literal.
fn masked(text: &str) -> Vec<u8> {
    let b = text.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    while i < b.len() {
        match literal_end(b, i) {
            Some(end) => {
                if end > i + 2 {
                    out[i + 1..end - 1].fill(b' ');
                }
                i = end;
            }
            None => i += 1,
        }
    }
    out
}

fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Whether the whole word `word` starts at `at`.
fn word_at(m: &[u8], at: usize, word: &str) -> bool {
    m.get(at..)
        .is_some_and(|rest| rest.starts_with(word.as_bytes()))
        && (at == 0 || !is_ident_byte(m[at - 1]))
        && m.get(at + word.len()).is_none_or(|&c| !is_ident_byte(c))
}

/// Offsets of every occurrence of the whole word `word`.
fn words(m: &[u8], word: &str) -> Vec<usize> {
    m.windows(word.len())
        .enumerate()
        .filter(|&(at, window)| window == word.as_bytes() && word_at(m, at, word))
        .map(|(at, _)| at)
        .collect()
}

/// The first offset at or after `i` that is not whitespace.
fn skip_ws(m: &[u8], mut i: usize) -> usize {
    while m.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    i
}

/// The last byte before `at` that is not whitespace.
fn previous_byte(m: &[u8], at: usize) -> Option<u8> {
    m[..at]
        .iter()
        .rev()
        .copied()
        .find(|c| !c.is_ascii_whitespace())
}

/// The identifier starting at `at` (empty if none does).
fn ident_at(m: &[u8], at: usize) -> &str {
    let rest = m.get(at..).unwrap_or_default();
    let len = rest
        .iter()
        .position(|&c| !is_ident_byte(c))
        .unwrap_or(rest.len());
    std::str::from_utf8(&rest[..len]).unwrap_or_default()
}

/// The identifier ending just before `at`, whitespace skipped.
fn previous_word(m: &[u8], at: usize) -> &str {
    let end = m[..at]
        .iter()
        .rposition(|c| !c.is_ascii_whitespace())
        .map_or(0, |i| i + 1);
    let start = m[..end]
        .iter()
        .rposition(|&c| !is_ident_byte(c))
        .map_or(0, |i| i + 1);
    std::str::from_utf8(&m[start..end]).unwrap_or_default()
}

/// The path segments written before the word at `at` (`a::b::word` gives
/// `["a", "b"]`; a leading `::` or a generic segment ends the path).
fn path_before(m: &[u8], at: usize) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut end = at;
    loop {
        let before = m[..end]
            .iter()
            .rposition(|c| !c.is_ascii_whitespace())
            .map_or(0, |i| i + 1);
        if before < 2 || &m[before - 2..before] != b"::" {
            break;
        }
        let segment = previous_word(m, before - 2);
        if segment.is_empty() {
            break;
        }
        segments.insert(0, segment);
        end = m[..before - 2]
            .iter()
            .rposition(|c| !c.is_ascii_whitespace())
            .map_or(0, |i| i + 1)
            - segment.len();
    }
    segments
}

/// Offset just past the delimiter closing the `(`, `[` or `{` at `open`.
fn close_of(m: &[u8], open: usize) -> usize {
    let mut depth = 0usize;
    for (i, &c) in m.iter().enumerate().skip(open) {
        match c {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced delimiter at offset {open}");
}

/// Offset just past the `<..>` opening at `open` (`->` closes nothing).
fn angle_end(m: &[u8], open: usize) -> usize {
    let (mut depth, mut i) = (0usize, open);
    while i < m.len() {
        match m[i] {
            b'<' => depth += 1,
            b'>' if m[i - 1] == b'-' => {}
            b'>' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            b'(' | b'[' | b'{' => {
                i = close_of(m, i);
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    panic!("unbalanced generics at offset {open}");
}

/// Ranges of the comma-separated parts of `m[start..end]`. Commas inside
/// brackets do not split; empty parts are dropped.
fn split_top_level(m: &[u8], start: usize, end: usize) -> Vec<(usize, usize)> {
    let (mut parts, mut part, mut i) = (Vec::new(), start, start);
    while i < end {
        match m[i] {
            b'(' | b'[' | b'{' => {
                i = close_of(m, i);
                continue;
            }
            b',' => {
                parts.push((part, i));
                part = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push((part, end));
    parts.retain(|&(a, b)| m[a..b].iter().any(|c| !c.is_ascii_whitespace()));
    parts
}

/// Whitespace-free `text` with every module path prefix dropped: a
/// lower-case segment followed by `::` (`crate::`, `super::`,
/// `nexus_kernel::coding_run::`) and a leading `::`. Type and variant
/// segments stay, so a comparison is qualification-insensitive without
/// becoming type-blind. Literals are kept exactly as written.
fn normalized(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = String::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if let Some(end) = literal_end(b, i) {
            out.push_str(&text[i..end]);
            i = end;
        } else if b[i].is_ascii_whitespace() {
            i += 1;
        } else if b[i..].starts_with(b"::")
            && !out.ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '>')
        {
            i += 2;
        } else if (b[i].is_ascii_lowercase() || b[i] == b'_')
            && (i == 0 || !is_ident_byte(b[i - 1]))
        {
            let end = i + ident_at(b, i).len();
            let next = skip_ws(b, end);
            if b[next..].starts_with(b"::") && b.get(skip_ws(b, next + 2)) != Some(&b'<') {
                i = skip_ws(b, next + 2);
            } else {
                out.push_str(&text[i..end]);
                i = end;
            }
        } else {
            let c = text[i..].chars().next().unwrap_or(' ');
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

/// `normalized` text without the trailing comma of any list, so a
/// reformatting that only wraps a list compares equal.
fn compact(text: &str) -> String {
    normalized(text)
        .replace(",)", ")")
        .replace(",]", "]")
        .replace(",}", "}")
}

/// An `impl` block.
struct ImplBlock {
    /// The trait's last path segment; empty for an inherent impl.
    trait_name: String,
    /// The implementing type's last path segment.
    self_type: String,
    /// Offsets of the block's `{` and just past its `}`.
    block: (usize, usize),
}

/// Every `impl` block of masked production text. Traits and types are named
/// by their last path segment, so `impl Trait for X`, `impl a::b::Trait for
/// X` and `impl ::a::Trait for X` are alike.
fn impl_blocks(m: &[u8]) -> Vec<ImplBlock> {
    let mut blocks = Vec::new();
    for at in words(m, "impl") {
        // An item, not `impl Trait` in a type: it follows an item boundary,
        // an attribute or `unsafe`.
        let item = matches!(previous_byte(m, at), None | Some(b'}' | b';' | b'{' | b']'))
            || previous_word(m, at) == "unsafe";
        if !item {
            continue;
        }
        let mut i = at + "impl".len();
        let open = loop {
            match m.get(i) {
                None | Some(b';') => break None,
                Some(b'{') => break Some(i),
                Some(b'<') => i = angle_end(m, i),
                Some(b'(' | b'[') => i = close_of(m, i),
                Some(_) => i += 1,
            }
        };
        let Some(open) = open else { continue };
        let (trait_name, self_type) = impl_header(&m[at + "impl".len()..open]);
        blocks.push(ImplBlock {
            trait_name,
            self_type,
            block: (open, close_of(m, open)),
        });
    }
    blocks
}

/// (trait, type) of an `impl` header (`<generics> Trait for Type where ..` or
/// `<generics> Type`), each by its last path segment.
fn impl_header(h: &[u8]) -> (String, String) {
    let mut i = skip_ws(h, 0);
    if h.get(i) == Some(&b'<') {
        i = angle_end(h, i);
    }
    let (mut split, mut end, mut j) = (None, h.len(), i);
    while j < h.len() {
        match h[j] {
            b'<' => {
                j = angle_end(h, j);
                continue;
            }
            b'(' | b'[' | b'{' => {
                j = close_of(h, j);
                continue;
            }
            _ if word_at(h, j, "where") => {
                end = j;
                break;
            }
            _ if split.is_none()
                && word_at(h, j, "for")
                && h.get(skip_ws(h, j + 3)) != Some(&b'<') =>
            {
                split = Some(j);
            }
            _ => {}
        }
        j += 1;
    }
    let segment = |part: &[u8]| last_segment(&String::from_utf8_lossy(part));
    match split {
        Some(at) => (segment(&h[i..at]), segment(&h[at + 3..end])),
        None => (String::new(), segment(&h[i..end])),
    }
}

/// The last segment of a type or trait path, without generic arguments,
/// references, `dyn` or a negative-impl `!`.
fn last_segment(path: &str) -> String {
    let path = path.split('<').next().unwrap_or_default();
    path.rsplit("::")
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .trim_start_matches(['&', '!'])
        .to_string()
}

/// A function with a body.
struct FnItem {
    name: String,
    /// `Type::name` inside an `impl Type` block, else `name`.
    path: String,
    /// The trait of that `impl` block (empty if none).
    trait_name: String,
    /// Attributes, visibility and qualifiers before `fn`, normalized.
    head: String,
    /// The parameter list, normalized, without a trailing comma.
    params: String,
    /// Offsets of `fn` and just past the body's `}`.
    span: (usize, usize),
    /// Offsets of the body's `{` and just past its `}`.
    body: (usize, usize),
}

impl FnItem {
    /// The body's text, without its braces.
    fn body_text<'a>(&self, text: &'a str) -> &'a str {
        &text[self.body.0 + 1..self.body.1 - 1]
    }
}

/// Every function with a body in production `text`.
fn fn_items(text: &str) -> Vec<FnItem> {
    let m = masked(text);
    let impls = impl_blocks(&m);
    let mut items = Vec::new();
    for at in words(&m, "fn") {
        let name_at = skip_ws(&m, at + 2);
        let name = ident_at(&m, name_at);
        if name_at == at + 2 || name.is_empty() {
            continue; // a `fn(..)` pointer type, or a macro's `fn $name`
        }
        let mut i = skip_ws(&m, name_at + name.len());
        if m.get(i) == Some(&b'<') {
            i = skip_ws(&m, angle_end(&m, i));
        }
        if m.get(i) != Some(&b'(') {
            continue;
        }
        let params_end = close_of(&m, i);
        let mut j = params_end;
        let open = loop {
            match m.get(j) {
                None | Some(b';') => break None,
                Some(b'{') => break Some(j),
                Some(b'<') => j = angle_end(&m, j),
                Some(b'(' | b'[') => j = close_of(&m, j),
                Some(_) => j += 1,
            }
        };
        let Some(open) = open else { continue };
        let end = close_of(&m, open);
        let head_start = m[..at]
            .iter()
            .rposition(|c| matches!(c, b'}' | b';' | b'{'))
            .map_or(0, |p| p + 1);
        let owner = impls
            .iter()
            .filter(|block| block.block.0 < at && at < block.block.1)
            .min_by_key(|block| block.block.1 - block.block.0);
        items.push(FnItem {
            name: name.to_string(),
            path: owner.map_or_else(
                || name.to_string(),
                |block| format!("{}::{name}", block.self_type),
            ),
            trait_name: owner
                .map(|block| block.trait_name.clone())
                .unwrap_or_default(),
            head: normalized(&text[head_start..at]),
            params: compact(&text[i..params_end]),
            span: (at, end),
            body: (open, end),
        });
    }
    items
}

/// The innermost function whose signature or body holds offset `at`.
fn enclosing(functions: &[FnItem], at: usize) -> Option<&FnItem> {
    functions
        .iter()
        .filter(|f| f.span.0 < at && at < f.span.1)
        .min_by_key(|f| f.span.1 - f.span.0)
}

/// Ranges of every `use` declaration.
fn use_spans(m: &[u8]) -> Vec<(usize, usize)> {
    words(m, "use")
        .into_iter()
        .filter(|&at| {
            matches!(
                previous_byte(m, at),
                None | Some(b'}' | b';' | b'{' | b']' | b')')
            ) || previous_word(m, at) == "pub"
        })
        .map(|at| {
            let mut i = at;
            while i < m.len() && m[i] != b';' {
                i = if m[i] == b'{' { close_of(m, i) } else { i + 1 };
            }
            (at, i)
        })
        .collect()
}

/// The entries of the desktop's one `generate_handler!` registration,
/// parsed whole with bracket matching (an entry may carry an attribute
/// holding brackets), whitespace-free and without attributes, in source
/// order. Module qualification is kept: it locates the handler.
fn registry_entries(text: &str) -> Vec<String> {
    let m = masked(text);
    let sites: Vec<usize> = words(&m, "generate_handler")
        .into_iter()
        .filter(|&at| m.get(skip_ws(&m, at + "generate_handler".len())) == Some(&b'!'))
        .collect();
    assert_eq!(sites.len(), 1, "exactly one command registration");
    let bang = skip_ws(&m, sites[0] + "generate_handler".len());
    let open = skip_ws(&m, bang + 1);
    assert!(
        matches!(m.get(open), Some(b'[' | b'(' | b'{')),
        "the registration list"
    );
    split_top_level(&m, open + 1, close_of(&m, open) - 1)
        .into_iter()
        .map(|(start, end)| {
            let mut at = skip_ws(&m, start);
            while m[at] == b'#' {
                at = skip_ws(&m, close_of(&m, skip_ws(&m, at + 1)));
            }
            without_whitespace(&text[at..end])
        })
        .filter(|entry| !entry.is_empty())
        .collect()
}

/// (module, command) of a registration entry; the module is empty for a
/// handler named without a path.
fn split_entry(entry: &str) -> (&str, &str) {
    let path = entry.trim_start_matches("crate::");
    path.rsplit_once("::").unwrap_or(("", path))
}

/// The desktop source file that defines a registered handler's module.
fn module_file(module: &str) -> String {
    if module.is_empty() {
        "lib.rs".to_string()
    } else {
        format!("{}.rs", module.replace("::", "/"))
    }
}

/// The variants of `enum <name>` in production `text`, in order.
fn enum_variants(text: &str, name: &str) -> Vec<String> {
    let m = masked(text);
    let at = words(&m, "enum")
        .into_iter()
        .find(|&at| ident_at(&m, skip_ws(&m, at + "enum".len())) == name)
        .unwrap_or_else(|| panic!("enum {name}"));
    let open = at + m[at..].iter().position(|&c| c == b'{').expect("a body");
    split_top_level(&m, open + 1, close_of(&m, open) - 1)
        .into_iter()
        .map(|(start, _)| {
            let mut i = skip_ws(&m, start);
            while m[i] == b'#' {
                i = skip_ws(&m, close_of(&m, skip_ws(&m, i + 1)));
            }
            ident_at(&m, i).to_string()
        })
        .collect()
}

/// (attributes and visibility, fields) of `struct <name>` in production
/// `text`, both normalized; the fields without a trailing comma.
fn struct_parts(text: &str, name: &str) -> (String, String) {
    let m = masked(text);
    let at = words(&m, "struct")
        .into_iter()
        .find(|&at| ident_at(&m, skip_ws(&m, at + "struct".len())) == name)
        .unwrap_or_else(|| panic!("struct {name}"));
    let head_start = m[..at]
        .iter()
        .rposition(|c| matches!(c, b'}' | b';' | b'{'))
        .map_or(0, |p| p + 1);
    let open = at
        + m[at..]
            .iter()
            .position(|c| matches!(c, b'{' | b'(' | b';'))
            .expect("a struct body");
    let fields = if m[open] == b';' {
        String::new()
    } else {
        compact(&text[open + 1..close_of(&m, open) - 1])
            .trim_end_matches(',')
            .to_string()
    };
    (normalized(&text[head_start..at]), fields)
}

/// The normalized body of `trait <name>` in production `text`.
fn trait_body(text: &str, name: &str) -> String {
    let m = masked(text);
    let at = words(&m, "trait")
        .into_iter()
        .find(|&at| ident_at(&m, skip_ws(&m, at + "trait".len())) == name)
        .unwrap_or_else(|| panic!("trait {name}"));
    let open = at
        + m[at..]
            .iter()
            .position(|&c| c == b'{')
            .expect("a trait body");
    compact(&text[open + 1..close_of(&m, open) - 1])
}

/// (pattern, value) of each arm of the first `match` in the body of the one
/// function named `function` in production `text`, both compacted.
fn match_arms(text: &str, function: &str) -> Vec<(String, String)> {
    let m = masked(text);
    let found: Vec<FnItem> = fn_items(text)
        .into_iter()
        .filter(|item| item.name == function)
        .collect();
    assert_eq!(found.len(), 1, "one fn {function}");
    let body = found[0].body;
    let at = words(&m, "match")
        .into_iter()
        .find(|&at| body.0 < at && at < body.1)
        .unwrap_or_else(|| panic!("{function}: no match"));
    let open = at + m[at..].iter().position(|&c| c == b'{').expect("match arms");
    let close = close_of(&m, open) - 1;
    let mut arms = Vec::new();
    let mut i = skip_ws(&m, open + 1);
    while i < close {
        let mut arrow = i;
        while !m[arrow..].starts_with(b"=>") {
            assert!(arrow < close, "{function}: an arm without `=>`");
            arrow = match m[arrow] {
                b'(' | b'[' | b'{' => close_of(&m, arrow),
                _ => arrow + 1,
            };
        }
        let start = skip_ws(&m, arrow + 2);
        let mut end = start;
        if m[start] == b'{' {
            end = close_of(&m, start);
        } else {
            while end < close && m[end] != b',' {
                end = match m[end] {
                    b'(' | b'[' | b'{' => close_of(&m, end),
                    _ => end + 1,
                };
            }
        }
        arms.push((compact(&text[i..arrow]), compact(&text[start..end])));
        i = skip_ws(&m, end);
        if m.get(i) == Some(&b',') {
            i = skip_ws(&m, i + 1);
        }
    }
    arms
}

/// The enum variants a normalized match-arm pattern names, each through an
/// explicit `Enum::Variant` path; `None` for a wildcard, a binding, a guard
/// or anything else that could match more than the variants it names.
fn pattern_variants(pattern: &str) -> Option<Vec<String>> {
    let m = masked(pattern);
    let mut alternatives = Vec::new();
    let (mut start, mut i) = (0, 0);
    while i < m.len() {
        match m[i] {
            b'(' | b'[' | b'{' => {
                i = close_of(&m, i);
                continue;
            }
            b'|' => {
                alternatives.push((start, i));
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    alternatives.push((start, m.len()));
    let mut variants = Vec::new();
    for (start, end) in alternatives {
        let alternative = &m[start..end];
        let path_end = alternative
            .iter()
            .position(|&c| c == b'{' || c == b'(')
            .unwrap_or(alternative.len());
        let path = std::str::from_utf8(&alternative[..path_end]).ok()?;
        let (_, variant) = path.rsplit_once("::")?;
        if variant.is_empty() || !variant.bytes().all(is_ident_byte) {
            return None;
        }
        if path_end < alternative.len() && close_of(alternative, path_end) != alternative.len() {
            return None;
        }
        variants.push(variant.to_string());
    }
    Some(variants)
}

/// A normalized body without its leading `use` declarations.
fn after_uses(body: &str) -> &str {
    let mut rest = body;
    while let Some(after) = rest.strip_prefix("use") {
        match after.split_once(';') {
            Some((path, tail)) if !path.contains(['(', '=', '.']) => rest = tail,
            _ => break,
        }
    }
    rest
}

/// The last statement, or the tail value, of a normalized body.
fn tail(body: &str) -> &str {
    let m = masked(body);
    let (mut last, mut i) = (0, 0);
    while i < m.len() {
        match m[i] {
            b'(' | b'[' | b'{' => {
                i = close_of(&m, i);
                continue;
            }
            b';' if i + 1 < m.len() => last = i + 1,
            _ => {}
        }
        i += 1;
    }
    &body[last..]
}

/// Each ```` ```compile_fail ```` block of a source's module docs (`//!`),
/// normalized.
fn compile_fail_blocks(source: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in source.lines() {
        let Some(doc) = line.trim_start().strip_prefix("//!") else {
            continue;
        };
        let doc = doc.trim();
        match current.take() {
            None => {
                if doc.starts_with("```compile_fail") {
                    current = Some(String::new());
                }
            }
            Some(block) if doc.starts_with("```") => blocks.push(normalized(&block)),
            Some(mut block) => {
                block.push_str(doc);
                block.push('\n');
                current = Some(block);
            }
        }
    }
    blocks
}

// ---------------------------------------------------------------------------
// The scanners themselves
// ---------------------------------------------------------------------------

/// The production text, whitespace-free, of the top-level item whose first
/// line is `head`.
fn item_source(text: &str, head: &str) -> String {
    let at = text
        .find(&format!("\n{head}"))
        .unwrap_or_else(|| panic!("{head}"))
        + 1;
    without_whitespace(&production_text(&text[at..item_end(text.as_bytes(), at)]))
}

#[test]
fn p3e_scan_01_the_local_scanner_is_the_phase_zero_scanner() {
    let original = workspace_file("app/src-tauri/src/phase0_surface/tests.rs");
    let copy = workspace_file(OWN_SOURCE);
    for head in [
        "const NOT_PRODUCTION_DIRS: &[&str] = &[",
        "fn workspace_production_sources() -> Vec<(String, String)> {",
        "fn literal_end(b: &[u8], i: usize) -> Option<usize> {",
        "fn comment_end(b: &[u8], i: usize) -> Option<usize> {",
        "fn item_end(b: &[u8], mut i: usize) -> usize {",
        "fn production_text(src: &str) -> String {",
    ] {
        assert_eq!(
            item_source(&copy, head),
            item_source(&original, head),
            "{head}"
        );
    }

    // The original's own fixture, on the copy.
    let src = concat!(
        "fn a() { let s = \"// not a comment #[cfg(test)]\"; } // tail\n",
        "/* block /* nested */ */ fn b<'a>(x: &'a str) -> char { '}' }\n",
        "#[cfg(test)]\nmod tests { fn t() { let _ = \"}\"; } }\n",
        "#[cfg(any(test, feature = \"x\"))]\npub fn helper() -> [u8; 2] { [1, 2] }\n",
        "#[cfg(test)]\nuse std::fmt;\n",
        "fn c() -> &'static str { r#\"raw \" // kept\"# }\n",
        "fn d() -> S { S { #[cfg(test)] probe: 1, kept_field: 2 } }\n",
        "fn e(x: u8) { match x { #[cfg(test)] 0 => gone(), _ => kept_arm() } }\n",
    );
    let text = production_text(src);
    assert!(text.contains("\"// not a comment #[cfg(test)]\""), "{text}");
    assert!(!text.contains("tail") && !text.contains("nested"), "{text}");
    assert!(
        text.contains("fn b<'a>(x: &'a str) -> char { '}' }"),
        "{text}"
    );
    assert!(
        !text.contains("mod tests") && !text.contains("helper"),
        "{text}"
    );
    assert!(!text.contains("std::fmt"), "{text}");
    assert!(text.contains("r#\"raw \" // kept\"#"), "{text}");
    assert!(
        !text.contains("probe") && text.contains("kept_field: 2 } }"),
        "{text}"
    );
    assert!(
        !text.contains("gone") && text.contains("_ => kept_arm() } }"),
        "{text}"
    );
}

#[test]
fn p3e_scan_02_the_parsers_are_whitespace_and_qualification_insensitive() {
    // `impl` headers, however the trait's path is written.
    for (header, expected) in [
        ("impl FolderPicker for NativeDialogs {}", ("FolderPicker", "NativeDialogs")),
        (
            "impl nexus_kernel::coding_run::VerifierLaunchConfirmer for NativeDialogs {}",
            ("VerifierLaunchConfirmer", "NativeDialogs"),
        ),
        (
            "impl ::nexus_kernel::coding_run::verifier::VerifierLaunchConfirmer\n    for crate::a::Dialogs<'_> {}",
            ("VerifierLaunchConfirmer", "Dialogs"),
        ),
        (
            "impl<T: Send> coding_run :: VerifierLaunchConfirmer for T where T: Sync {}",
            ("VerifierLaunchConfirmer", "T"),
        ),
        (
            "impl<F: for<'a> Fn(&'a str)> OwnerConfirmer for F {}",
            ("OwnerConfirmer", "F"),
        ),
        (
            "impl<E: a::ActionExecutor>\n    a::ActionExecutor for Phase0AgentExecutor<E>\n{}",
            ("ActionExecutor", "Phase0AgentExecutor"),
        ),
        ("impl VerifierLaunchApproval {}", ("", "VerifierLaunchApproval")),
        ("unsafe impl Send for Holder {}", ("Send", "Holder")),
    ] {
        let blocks = impl_blocks(&masked(header));
        assert_eq!(blocks.len(), 1, "{header}");
        assert_eq!(
            (blocks[0].trait_name.as_str(), blocks[0].self_type.as_str()),
            expected,
            "{header}"
        );
    }
    assert!(impl_blocks(&masked(
        "fn f() -> impl Iterator<Item = u8> { None.into_iter() }"
    ))
    .is_empty());

    // Functions: owners, generic signatures, pointer types and declarations.
    let sample = production_text(
        "impl A for One { fn execute(&self) -> u8 { 1 } }\n\
         impl<E: x::B> x::A for Two<E> {\n    fn execute(&self, f: fn(u8) -> u8) -> Option<crate::y::Closure> { None }\n}\n\
         fn free<T: Into<u8>>(t: T) -> u8 where T: Copy { t.into() }\n\
         trait C { fn declared(&self); }\n",
    );
    let items = fn_items(&sample);
    let paths: Vec<(&str, &str)> = items
        .iter()
        .map(|item| (item.path.as_str(), item.trait_name.as_str()))
        .collect();
    assert_eq!(
        paths,
        [("One::execute", "A"), ("Two::execute", "A"), ("free", "")]
    );
    let closure_at = sample.find("Closure").unwrap();
    assert_eq!(
        enclosing(&items, closure_at).map(|f| f.path.as_str()),
        Some("Two::execute")
    );

    // Normalization drops module paths only.
    assert_eq!(
        normalized("crate :: phase0_surface :: closed ( \"a b\" ,\n Closure :: OsInput , )"),
        "closed(\"a b\",Closure::OsInput,)"
    );
    assert_eq!(
        normalized("::nexus_kernel::coding_run::VerifierLaunchApproval::confirmed(x)"),
        "VerifierLaunchApproval::confirmed(x)"
    );
    assert_eq!(
        compact("f(\n    a,\n    [b, c,],\n    S { d, },\n)"),
        "f(a,[b,c],S{d})"
    );
    assert_eq!(
        normalized("v.iter().collect::<Vec<_>>()"),
        "v.iter().collect::<Vec<_>>()"
    );

    // Canonical denials, however written; anything more is not canonical.
    for body in [
        "Err(crate::phase0_surface::closed(\"x_y\", crate::phase0_surface::Closure::FileSelection))",
        "Err(closed(\n    \"x_y\",\n    Closure::FileSelection,\n))",
        "Err(super::phase0_surface::closed(\"x_y\", phase0_surface::Closure::FileSelection))",
    ] {
        assert_eq!(
            canonical_denial(body),
            Some(("x_y".to_string(), "FileSelection".to_string())),
            "{body}"
        );
    }
    for body in [
        "let _ = path; Err(closed(\"x\", Closure::OsInput))",
        "Err(closed(name, Closure::OsInput))",
        "if a { Err(closed(\"x\", Closure::OsInput)) } else { Ok(()) }",
        "Ok(closed(\"x\", Closure::OsInput))",
    ] {
        assert_eq!(canonical_denial(body), None, "{body}");
    }

    // The closure classifier: canonical and conditional calls are told apart,
    // and a call outside a function, a rename or a value use is unclassified.
    let synthetic = production_text(
        "#[tauri::command]\nfn c() -> Result<(), String> {\n    Err(crate::phase0_surface::closed(\n        \"c\",\n        crate::phase0_surface::Closure::OsInput,\n    ))\n}\n\
         fn d(x: bool) -> Result<(), String> { if x { return Err(closed(\"d\", Closure::FileSelection)); } Ok(()) }\n\
         static F: fn(&'static str, Closure) -> String = closed;\n\
         use crate::phase0_surface::closed as deny;\n\
         use other::closed;\n\
         fn g() -> Result<(), String> { Err(other::closed(\"g\", elsewhere::Closure::OsInput)) }\n\
         fn e() -> String { sender.closed(); String::new() }\n",
    );
    let inventory = classify_closures(&[("synthetic.rs", &synthetic)]);
    assert_eq!(
        inventory.canonical.into_iter().collect::<Vec<_>>(),
        [(
            "synthetic.rs".to_string(),
            "c".to_string(),
            "OsInput".to_string(),
            "()".to_string()
        )]
    );
    assert_eq!(
        inventory.conditional,
        [(
            "synthetic.rs".to_string(),
            "d".to_string(),
            "\"d\"".to_string(),
            "Closure::FileSelection".to_string()
        )]
    );
    assert_eq!(
        inventory.unclassified,
        [
            "synthetic.rs: `Closure` named outside any function",
            "synthetic.rs: `Closure` reached through another path",
            "synthetic.rs: `closed` reached through another path",
            "synthetic.rs: `closed` reached through another path",
            "synthetic.rs: `closed` renamed",
            "synthetic.rs: `closed` used other than by a call",
        ]
    );
    // Paths: `a::b::word` gives `["a", "b"]`; generics and a leading `::` end it.
    let path = masked("crate :: phase0_surface ::closed Vec::<u8>::new = ::a::b");
    let segments = |word: &str| path_before(&path, words(&path, word)[0]);
    assert_eq!(segments("closed"), ["crate", "phase0_surface"]);
    assert!(segments("new").is_empty());
    assert_eq!(segments("b"), ["a"]);

    // Arms: only explicit `Enum::Variant` patterns name variants.
    let arms = match_arms(
        &production_text(
            "fn k(a: A) -> u8 { match a { A::X { .. } => 1, A::Y | A::Z(_) => { 2 } other => 3, A::W { .. } if true => 4, y @ _ => 5, _ => 6 } }",
        ),
        "k",
    );
    let named: Vec<Option<Vec<String>>> = arms
        .iter()
        .map(|(pattern, _)| pattern_variants(pattern))
        .collect();
    assert_eq!(
        named,
        [
            Some(vec!["X".to_string()]),
            Some(vec!["Y".to_string(), "Z".to_string()]),
            None,
            None,
            None,
            None,
        ]
    );
}
