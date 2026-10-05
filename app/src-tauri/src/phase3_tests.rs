//! Phase Three trust guards (re-derived from the parked P3-ENTRY-G1 guards,
//! `implement/p3-entry-g1-guards` e8a2b80a, on the post-Phase-Two forward
//! line). Test code only; it grants nothing.
//!
//! A string is never authority. These guards read production source and the
//! authority structure it builds; they never decide or grant anything.
//!
//! - `p3e_g1_*` (G1): every `PlannedAction` variant carries two explicit
//!   classifications, each an exhaustive match with no wildcard, so a variant
//!   added to the kernel enum does not compile here until it is classified:
//!   its Phase Zero decision (`expected_decision`, what production enforces
//!   before Phase Three opens an actuator) and its Phase Three disposition
//!   (`expected_disposition`: inert, governed R0/R1/R2, or still closed).
//!   Production keeps its own fail-closed wildcard.
//! - `p3e_g3_*` (G3): the Phase Zero closures as a semantic inventory, not a
//!   count; registration order is irrelevant.
//! - `p3e_g5_*` (G5): the Phase Two launch approval, pinned from outside the
//!   Phase Two code.
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

/// An effect class (Phase Three D4): R0 local observation or computation,
/// R1 bounded external or reversible effect, R2 sensitive, irreversible or
/// privileged effect (exact one-shot native approval).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Effect {
    R0,
    R1,
    R2,
}

/// The Phase Three disposition of a planned action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Disposition {
    /// No real-world authority; permitted since Phase Zero.
    Inert,
    /// Opened only through the Phase Three authority (commitment, grant,
    /// approval for R2), with this effect class.
    Governed(Effect),
    /// Closed in Phase Three.
    StillClosed,
}

/// Governed, exactly.
const GOVERNED: [&str; 16] = [
    "WebFetch",
    "ApiCall",
    "BrowserAutomate",
    "CaptureScreen",
    "CaptureWindow",
    "AnalyzeScreen",
    "MouseMove",
    "MouseClick",
    "MouseDoubleClick",
    "MouseDrag",
    "KeyboardType",
    "KeyboardPress",
    "KeyboardShortcut",
    "ScrollWheel",
    "ComputerAction",
    "TextToSpeech",
];

/// Still closed, exactly.
const STILL_CLOSED: [&str; 20] = [
    "FileRead",
    "FileWrite",
    "ShellCommand",
    "DockerCommand",
    "CodeExecute",
    "ImageGenerate",
    "A2aDelegation",
    "SelfModifyDescription",
    "SelfModifyStrategy",
    "CreateSubAgent",
    "DestroySubAgent",
    "RunEvolutionTournament",
    "ModifyGovernancePolicy",
    "AllocateEcosystemFuel",
    "ModifyCognitiveParams",
    "SelectLlmProvider",
    "SelectAlgorithm",
    "DesignAgentEcosystem",
    "RunCounterfactual",
    "TemporalPlan",
];

/// An API call's class: a safe method is R1, any other method R2.
fn api_call_effect(method: &str) -> Effect {
    match method.trim().to_ascii_uppercase().as_str() {
        "GET" | "HEAD" | "OPTIONS" => Effect::R1,
        _ => Effect::R2,
    }
}

/// A browser task's class: navigating, waiting and reading are R1; any
/// interaction with the page (click, fill, key press) is R2. No wildcard.
fn browser_effect(actions: &[nexus_kernel::cognitive::types::BrowserAction]) -> Effect {
    use nexus_kernel::cognitive::types::BrowserAction;
    actions
        .iter()
        .map(|action| match action {
            BrowserAction::Navigate { .. } => Effect::R1,
            BrowserAction::WaitFor { .. } => Effect::R1,
            BrowserAction::ExtractText { .. } => Effect::R1,
            BrowserAction::Click { .. } => Effect::R2,
            BrowserAction::Fill { .. } => Effect::R2,
            BrowserAction::Press { .. } => Effect::R2,
        })
        .max()
        .unwrap_or(Effect::R1)
}

/// The Phase Three disposition of every `PlannedAction` variant (ARCHITECTURE
/// D4). No wildcard and no catch-all binding (`p3e_g1_01` scans this
/// function).
fn expected_disposition(action: &PlannedAction) -> Disposition {
    use Disposition::{Governed, Inert, StillClosed};
    match action {
        // Inert: unchanged since Phase Zero.
        PlannedAction::LlmQuery { .. } => Inert,
        PlannedAction::Noop => Inert,
        PlannedAction::MemoryStore { .. } => Inert,
        PlannedAction::MemoryRecall { .. } => Inert,
        PlannedAction::SendNotification { .. } => Inert,
        PlannedAction::AgentMessage { .. } => Inert,
        PlannedAction::HitlRequest { .. } => Inert,
        PlannedAction::WebSearch { .. } => Inert,
        PlannedAction::KnowledgeGraphUpdate { .. } => Inert,
        PlannedAction::KnowledgeGraphQuery { .. } => Inert,
        // Governed egress and browser (P3-B, P3-C).
        PlannedAction::WebFetch { .. } => Governed(Effect::R1),
        PlannedAction::ApiCall { method, .. } => Governed(api_call_effect(method)),
        PlannedAction::BrowserAutomate { actions, .. } => Governed(browser_effect(actions)),
        // Governed perception on the agent display (P3-D).
        PlannedAction::CaptureScreen { .. } => Governed(Effect::R0),
        PlannedAction::CaptureWindow { .. } => Governed(Effect::R0),
        PlannedAction::AnalyzeScreen { .. } => Governed(Effect::R0),
        // Governed input on the agent display (P3-E).
        PlannedAction::MouseMove { .. } => Governed(Effect::R1),
        PlannedAction::ScrollWheel { .. } => Governed(Effect::R1),
        PlannedAction::MouseClick { .. } => Governed(Effect::R2),
        PlannedAction::MouseDoubleClick { .. } => Governed(Effect::R2),
        PlannedAction::MouseDrag { .. } => Governed(Effect::R2),
        PlannedAction::KeyboardType { .. } => Governed(Effect::R2),
        PlannedAction::KeyboardPress { .. } => Governed(Effect::R2),
        PlannedAction::KeyboardShortcut { .. } => Governed(Effect::R2),
        PlannedAction::ComputerAction { .. } => Governed(Effect::R2),
        // Governed tool (P3-A).
        PlannedAction::TextToSpeech { .. } => Governed(Effect::R1),
        // Still closed: free-form shell, code, containers, raw files, model
        // media generation, peer delegation, self-modification, sub-agents,
        // fuel, governance and the agent's own cognition.
        PlannedAction::FileRead { .. } => StillClosed,
        PlannedAction::FileWrite { .. } => StillClosed,
        PlannedAction::ShellCommand { .. } => StillClosed,
        PlannedAction::DockerCommand { .. } => StillClosed,
        PlannedAction::CodeExecute { .. } => StillClosed,
        PlannedAction::ImageGenerate { .. } => StillClosed,
        PlannedAction::A2aDelegation { .. } => StillClosed,
        PlannedAction::SelfModifyDescription { .. } => StillClosed,
        PlannedAction::SelfModifyStrategy { .. } => StillClosed,
        PlannedAction::CreateSubAgent { .. } => StillClosed,
        PlannedAction::DestroySubAgent { .. } => StillClosed,
        PlannedAction::RunEvolutionTournament { .. } => StillClosed,
        PlannedAction::ModifyGovernancePolicy { .. } => StillClosed,
        PlannedAction::AllocateEcosystemFuel { .. } => StillClosed,
        PlannedAction::ModifyCognitiveParams { .. } => StillClosed,
        PlannedAction::SelectLlmProvider { .. } => StillClosed,
        PlannedAction::SelectAlgorithm { .. } => StillClosed,
        PlannedAction::DesignAgentEcosystem { .. } => StillClosed,
        PlannedAction::RunCounterfactual { .. } => StillClosed,
        PlannedAction::TemporalPlan { .. } => StillClosed,
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

/// The normalized body of `Phase0AgentExecutor::execute`. Without Phase
/// Three it is the Phase Zero executor (closure first, then `self.inner`).
/// With it, the Phase Three classification decides: only inert actions
/// reach the closure and then `self.inner`, governed actions reach only the
/// Phase Three bridge, and closed actions keep their Phase Zero refusal.
const EXECUTOR_BODY: &str = "use{classify,Disposition};letSome(bridge)=&self.governedelse{ifletSome(closure)=phase0_agent_action_closure(action){returnErr(closed(action.action_type(),closure));}returnself.inner.execute(agent_id,action,audit,hitl_approved);};matchclassify(action){Disposition::Inert=>{ifletSome(closure)=phase0_agent_action_closure(action){returnErr(closed(action.action_type(),closure));}self.inner.execute(agent_id,action,audit,hitl_approved)}Disposition::Governed(intent)=>bridge.act(agent_id,&intent),Disposition::Orchestrated{max_steps}=>Ok(orchestration_guidance(max_steps)),Disposition::Closed(_)=>Err(closed(action.action_type(),phase0_agent_action_closure(action).unwrap_or(Closure::AgentExecution)))}";

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

    // The Phase Three disposition: the same exhaustive, explicit form.
    for function in ["expected_disposition", "browser_effect"] {
        let arms = match_arms(&own, function);
        let mut named = Vec::new();
        for (pattern, _) in arms {
            named.extend(
                pattern_variants(&pattern)
                    .unwrap_or_else(|| panic!("{function} has a catch-all arm `{pattern}`")),
            );
        }
        let named_set: BTreeSet<&str> = named.iter().map(String::as_str).collect();
        assert_eq!(
            named_set.len(),
            named.len(),
            "{function}: one arm per variant"
        );
        if function == "expected_disposition" {
            assert_eq!(named_set, declared_set, "every variant has a disposition");
        } else {
            let browser = enum_variants(
                production_source("kernel/src/cognitive/types.rs"),
                "BrowserAction",
            );
            let browser: BTreeSet<&str> = browser.iter().map(String::as_str).collect();
            assert_eq!(named_set, browser, "every browser action has a class");
        }
    }
    let partition = |wanted: fn(Disposition) -> bool| -> BTreeSet<&str> {
        fixtures
            .iter()
            .zip(&tags)
            .filter(|(action, _)| wanted(expected_disposition(action)))
            .map(|(_, tag)| tag.as_str())
            .collect()
    };
    let inert = partition(|d| d == Disposition::Inert);
    let governed = partition(|d| matches!(d, Disposition::Governed(_)));
    let closed = partition(|d| d == Disposition::StillClosed);
    assert_eq!(inert, BTreeSet::from(PERMITTED), "inert is exactly Class A");
    assert_eq!(governed, BTreeSet::from(GOVERNED), "governed, exactly");
    assert_eq!(
        closed,
        BTreeSet::from(STILL_CLOSED),
        "still closed, exactly"
    );
    eprintln!(
        "p3e_g1: Phase Three disposition: {} inert, {} governed, {} still closed",
        inert.len(),
        governed.len(),
        closed.len()
    );
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

/// Governed variants whose content keeps them closed in Phase Three: speech
/// written to a file or by a non-local provider, browser screenshots to a
/// directory (file writes stay closed).
fn closed_by_content(action: &PlannedAction) -> bool {
    match action {
        PlannedAction::TextToSpeech {
            output_path,
            provider,
            ..
        } => {
            !output_path.trim().is_empty()
                || provider
                    .as_deref()
                    .is_some_and(|p| !matches!(p.trim(), "" | "local" | "espeak" | "espeak-ng"))
        }
        PlannedAction::BrowserAutomate { screenshot_dir, .. } => screenshot_dir
            .as_deref()
            .is_some_and(|dir| !dir.trim().is_empty()),
        _ => false,
    }
}

/// Phase Three routes every governed variant to its pipeline and nothing
/// else: the crate's final classification agrees with this independent
/// table for every variant, and the Phase Zero closure still refuses every
/// governed and every closed variant, so even a routing slip could not
/// reach the kernel action registry.
#[test]
fn p3e_g1_07_no_governed_or_closed_variant_has_a_direct_path() {
    use nexus_governed_control::planned::{classify, Disposition as Routed};
    for action in every_action() {
        let disposition = expected_disposition(&action);
        let production = crate::phase0_agent_action_closure(&action);
        let routed = classify(&action);
        match disposition {
            Disposition::Inert => {
                assert_eq!(production, None, "{}", action.action_type());
                assert_eq!(routed, Routed::Inert, "{}", action.action_type());
            }
            Disposition::Governed(_) => {
                assert!(
                    production.is_some(),
                    "{} has a direct path",
                    action.action_type()
                );
                if closed_by_content(&action) {
                    assert!(
                        matches!(routed, Routed::Closed(_)),
                        "{} stays closed with this content: {routed:?}",
                        action.action_type()
                    );
                } else {
                    assert!(
                        matches!(routed, Routed::Governed(_) | Routed::Orchestrated { .. }),
                        "{} is governed: {routed:?}",
                        action.action_type()
                    );
                }
            }
            Disposition::StillClosed => {
                assert!(
                    production.is_some(),
                    "{} has a direct path",
                    action.action_type()
                );
                assert!(
                    matches!(routed, Routed::Closed(_)),
                    "{} stays closed: {routed:?}",
                    action.action_type()
                );
            }
        }
    }
    // Speech is governed only as audio returned as data.
    let speech = |output_path: &str, provider: Option<&str>| PlannedAction::TextToSpeech {
        text: probe(),
        output_path: output_path.into(),
        provider: provider.map(Into::into),
        voice: None,
        model: None,
    };
    assert!(matches!(classify(&speech("", None)), Routed::Governed(_)));
    assert!(matches!(
        classify(&speech("", Some("local"))),
        Routed::Governed(_)
    ));
    assert!(matches!(
        classify(&speech(&probe(), None)),
        Routed::Closed(_)
    ));
    assert!(matches!(
        classify(&speech("", Some("cloud"))),
        Routed::Closed(_)
    ));
    // Content-dependent classes.
    use nexus_kernel::cognitive::types::BrowserAction;
    assert_eq!(api_call_effect("get"), Effect::R1);
    assert_eq!(api_call_effect(" HEAD "), Effect::R1);
    assert_eq!(api_call_effect("POST"), Effect::R2);
    assert_eq!(api_call_effect("delete"), Effect::R2);
    assert_eq!(api_call_effect(""), Effect::R2);
    assert_eq!(browser_effect(&[]), Effect::R1);
    assert_eq!(
        browser_effect(&[
            BrowserAction::Navigate { url: probe() },
            BrowserAction::ExtractText { selector: probe() },
        ]),
        Effect::R1
    );
    assert_eq!(
        browser_effect(&[
            BrowserAction::Navigate { url: probe() },
            BrowserAction::Fill {
                selector: probe(),
                text: probe(),
            },
        ]),
        Effect::R2
    );
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
            expected_disposition(&parsed),
            expected_disposition(action),
            "{wire}"
        );
        assert_eq!(
            crate::phase0_agent_action_closure(&parsed),
            crate::phase0_agent_action_closure(action),
            "{wire}"
        );
    }
}

/// The real production executor never lets a closed or governed action
/// reach its registry, even when the step is marked approved. Without the
/// Phase Three control (a test state has none) it is exactly the Phase Zero
/// executor: every such action is refused with its Phase Zero closure. Every fixture here is inert even if it reached
/// the registry: the read targets an absent file for an agent the
/// supervisor does not know, and the registry routes none of the
/// governance actions.
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
    // Governed actions never fall back to the registry: without Phase Three
    // (a test state has none) the Phase Zero closure stands, approved or not.
    let governed = [
        PlannedAction::WebFetch {
            url: "https://example.invalid/".into(),
        },
        PlannedAction::CaptureScreen { region: None },
        PlannedAction::MouseClick {
            x: 1,
            y: 1,
            button: "left".into(),
        },
        PlannedAction::KeyboardType { text: probe() },
    ];
    for action in &governed {
        let Decision::Refused(closure) = expected_decision(action) else {
            panic!("{} is refused without Phase Three", action.action_type());
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
fn p3e_g1_05_the_executor_classifies_before_any_route() {
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
    // The classification it calls is the Phase Three one.
    assert_eq!(
        cognitive
            .matches("use nexus_governed_control::planned::{classify, Disposition};")
            .count(),
        1
    );

    // Its only state is the inner executor and the Phase Three bridge, and
    // it has one impl block.
    assert_eq!(
        struct_parts(cognitive, "Phase0AgentExecutor").1,
        "inner:E,governed:Option<AgentBridge>"
    );
    assert_eq!(
        cognitive
            .matches("governed: Option<crate::governed_real_world::AgentBridge>,")
            .count(),
        1,
        "the bridge is the Phase Three one"
    );
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
        Closure::GovernedRoute => "Phase Three G-INV: a legacy direct real-world route",
    }
}

/// Every `Closure` variant (`p3e_g3_05` proves the list complete).
const ALL_CLOSURES: [Closure; 17] = [
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
    Closure::GovernedRoute,
];

/// The Phase Three entry inventory of canonical closed commands, as
/// (defining file under the desktop `src`, closure, commands). Each command's
/// handler takes no input and its whole body is the denial for its closure.
/// Reopening one fails `p3e_g3_02` until it leaves this inventory with an
/// Architect-approved authority mechanism.
const CANONICAL_CLOSED: &[(&str, Closure, &[&str])] = &[
    // Phase Three G-INV-1..4.
    (
        "lib.rs",
        Closure::GovernedRoute,
        &[
            "email_fetch_messages",
            "email_search_messages",
            "email_send_message",
            "messaging_connect_platform",
            "messaging_poll_messages",
            "messaging_send",
        ],
    ),
    (
        "commands/crate_bridges.rs",
        Closure::GovernedRoute,
        &[
            "browser_close_session",
            "browser_create_session",
            "browser_execute_task",
            "browser_get_content",
            "browser_get_policy",
            "browser_navigate",
            "browser_session_count",
        ],
    ),
    (
        "nx_bridge/commands.rs",
        Closure::GovernedRoute,
        &["nx_computer_use_status"],
    ),
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
    (
        "lib.rs",
        Closure::HelperLaunch,
        &["builder_image_gen_status", "is_ollama_installed"],
    ),
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
    // Phase Three: the Phase Zero closure stands without Phase Three, still
    // gates inert actions with it, and is the refusal of closed actions.
    (
        "commands/cognitive.rs",
        "Phase0AgentExecutor::execute",
        "action.action_type()",
        "closure",
    ),
    (
        "commands/cognitive.rs",
        "Phase0AgentExecutor::execute",
        "action.action_type()",
        "closure",
    ),
    (
        "commands/cognitive.rs",
        "Phase0AgentExecutor::execute",
        "action.action_type()",
        "phase0_agent_action_closure(action).unwrap_or(Closure::AgentExecution)",
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
const OWN_SOURCE: &str = "app/src-tauri/src/phase3_tests.rs";

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

// ---------------------------------------------------------------------------
// G6: the Phase Three authority surface (PHASE-THREE-FINAL §22, §24).
// ---------------------------------------------------------------------------

const P3_CRATE: &str = "crates/nexus-governed-control/src/";
const P3_FRONT_DOOR: &str = "app/src-tauri/src/governed_real_world.rs";

/// The crate's production sources, as (path under its `src`, text).
fn p3_sources() -> Vec<(&'static str, &'static str)> {
    workspace_sources()
        .iter()
        .filter_map(|(path, text)| Some((path.strip_prefix(P3_CRATE)?, text.as_str())))
        .collect()
}

/// R2 approval, grants and resuming after a stop are confirmed only by the
/// owner's native dialogs; the crate's one other confirmer declines all.
#[test]
fn p3_g6_01_the_native_dialogs_are_the_only_control_confirmer() {
    let mut impls = trait_impls("ControlConfirmer");
    impls.sort();
    assert_eq!(
        impls,
        [
            (P3_FRONT_DOOR, "ControlDialogs".to_string()),
            (
                "crates/nexus-governed-control/src/governed.rs",
                "NeverAsk".to_string()
            ),
        ]
    );
    let governed = production_source("crates/nexus-governed-control/src/governed.rs");
    for method in ["confirm_action", "confirm_grant", "confirm_resume"] {
        let never = one_fn(governed, &format!("NeverAsk::{method}"));
        assert_eq!(
            compact(never.body_text(governed)),
            "false",
            "NeverAsk::{method}"
        );
        let front = production_source(P3_FRONT_DOOR);
        let dialog = one_fn(front, &format!("ControlDialogs::{method}"));
        let body = compact(dialog.body_text(front));
        assert!(
            body.starts_with(
                "use{DialogExt,MessageDialogButtons,MessageDialogKind};self.0.dialog().message(dialog_text(&request.message())).title(request.title()).kind(MessageDialogKind::Warning).buttons(MessageDialogButtons::OkCancelCustom("
            ) && body.ends_with(".blocking_show()"),
            "{method}: {body}"
        );
        for forbidden in ["true", "false", "return", "||", "&&", "unsafe"] {
            assert!(!body.contains(forbidden), "{method}: {forbidden}");
        }
        assert_eq!(
            body.matches("self.").count(),
            1,
            "{method}: only its own dialog"
        );
    }
    assert_eq!(
        compact(
            one_fn(production_source(P3_FRONT_DOOR), "dialog_text")
                .body_text(production_source(P3_FRONT_DOOR))
        ),
        "message.replace('%',\"%%\")"
    );
    // The trait is defined once and never renamed.
    assert_eq!(
        renaming("ControlConfirmer"),
        Vec::<&str>::new(),
        "ControlConfirmer is renamed somewhere"
    );
}

/// An R2 approval exists only after the owner's native answer to exactly
/// that commitment; it cannot be built, copied or deserialized elsewhere.
#[test]
fn p3_g6_02_an_r2_approval_is_minted_once_after_the_native_answer() {
    let commitment = production_source("crates/nexus-governed-control/src/authority/commitment.rs");
    let mints: Vec<(&str, String)> = references(P3_CRATE, "confirmed")
        .into_iter()
        .filter(|(file, _)| {
            file.ends_with("authority/commitment.rs") || !file.ends_with("approval.rs")
        })
        .collect();
    assert_eq!(
        mints,
        [(
            "crates/nexus-governed-control/src/authority/commitment.rs",
            "CommitmentRegistry::request_approval".to_string()
        )]
    );
    let request =
        compact(one_fn(commitment, "CommitmentRegistry::request_approval").body_text(commitment));
    assert_in_order(
        &request,
        &[
            "letconfirmed=confirmer.confirm_action(&request);",
            "ifentry.state!=CommitmentState::Prepared||entry.binding!=binding{returnErr(AuthorityError::NotPending);}",
            "self.live_check(id,entry,agent,run)?;",
            "if!confirmed{",
            "self.record(&record)?;",
            "Ok(R2Approval::confirmed(id,binding))",
        ],
        "request_approval",
    );
    let approval = production_source("crates/nexus-governed-control/src/authority/approval.rs");
    let at = approval.find("pub struct R2Approval").expect("R2Approval");
    let head = &approval[..at];
    let derive = &head[head.rfind("#[").unwrap()..];
    assert_eq!(
        compact(derive),
        "#[derive(Debug)]",
        "no Clone, Copy, Default or serde"
    );
    assert!(compact(approval)
        .contains("pub(crate)fnconfirmed(commitment:CommitmentId,binding:Digest)->Self"));
    // Authorizing consumes it and checks it names this commitment's binding.
    let authorize =
        compact(one_fn(commitment, "CommitmentRegistry::authorize").body_text(commitment));
    assert!(authorize.contains(
        "ifapproval.commitment()!=id||approval.binding()!=&entry.binding{returnErr(AuthorityError::ApprovalMismatch);}"
    ));
}

/// Every governed effect is a pending effect held by the one pipeline and
/// executed only by it, once, after `begin`; the commitment lifecycle is
/// crate-private.
#[test]
fn p3_g6_03_every_governed_effect_runs_only_through_the_pipeline() {
    let mut effects = trait_impls("PendingEffect");
    effects.sort();
    assert_eq!(
        effects,
        [
            (
                "crates/nexus-governed-control/src/browser/mod.rs",
                "Session".to_string()
            ),
            (
                "crates/nexus-governed-control/src/display/mod.rs",
                "Input".to_string()
            ),
            (
                "crates/nexus-governed-control/src/display/mod.rs",
                "Observe".to_string()
            ),
            (
                "crates/nexus-governed-control/src/egress/mod.rs",
                "EgressEffect".to_string()
            ),
            (
                "crates/nexus-governed-control/src/tool/mod.rs",
                "ToolEffect".to_string()
            ),
        ]
    );
    let control = production_source("crates/nexus-governed-control/src/control.rs");
    let execute = compact(one_fn(control, "Control::execute").body_text(control));
    assert_in_order(
        &execute,
        &[
            "pending.remove(&id).expect(\"present\").effect",
            "effect.revalidate()",
            ".begin(id,agent,run,&target,&parameters)",
            "effect.execute(&guard)",
            "guard.finish(",
        ],
        "Control::execute",
    );
    // `begin` and an effect's `execute` are called from the pipeline only.
    assert_eq!(
        references(P3_CRATE, "begin"),
        [(
            "crates/nexus-governed-control/src/control.rs",
            "Control::execute".to_string()
        )]
    );
    let executed: Vec<(&str, String)> = references(P3_CRATE, "execute");
    assert!(
        executed.iter().all(|(file, function)| {
            (file.ends_with("control.rs") && function == "Control::execute")
                || (file.ends_with("governed.rs")
                    && matches!(
                        function.as_str(),
                        "GovernedControl::execute" | "GovernedControl::agent_action"
                    ))
        }),
        "{executed:?}"
    );
    // The lifecycle is crate-private.
    let commitment = production_source("crates/nexus-governed-control/src/authority/commitment.rs");
    for method in [
        "prepare",
        "request_approval",
        "authorize",
        "begin",
        "fail_unstarted",
        "deny",
    ] {
        let item = one_fn(commitment, &format!("CommitmentRegistry::{method}"));
        assert!(item.head.contains("pub(crate)"), "{method}: {}", item.head);
    }
}

/// Within the crate, each real-world mechanism lives in exactly one place:
/// processes in the launcher (and the kernel's sealed spawn for tools),
/// network in egress, the browser proxy and the X11 socket, the vault in
/// the broker, X11 in the agent display.
#[test]
fn p3_g6_04_the_mechanisms_are_confined_to_their_modules() {
    use crate::phase0_surface::rust_paths::{
        constructs_process, ends_process, opens_network, Analysis, Declaration,
    };
    let mut found: BTreeMap<(String, &str), usize> = BTreeMap::new();
    for (file, _) in p3_sources() {
        let src = workspace_file(&format!("{P3_CRATE}{file}"));
        let analysis = Analysis::new(&src, &["crate"]);
        for o in analysis.production() {
            if o.declaration == Some(Declaration::Use) && !o.public && o.item.is_none() {
                continue;
            }
            let mut kinds = BTreeSet::new();
            for path in &o.resolved {
                let shown = path.join("::");
                if constructs_process(path) {
                    kinds.insert("process");
                }
                if ends_process(path) {
                    kinds.insert("ends");
                }
                if opens_network(path) {
                    kinds.insert("network");
                }
                if shown.starts_with("x11rb") {
                    kinds.insert("x11");
                }
                if shown.starts_with("nexus_kernel::secrets") {
                    kinds.insert("vault");
                }
                if shown.starts_with("nexus_kernel::resource_limiter") {
                    kinds.insert("sealed");
                }
                if shown.starts_with("libc") {
                    kinds.insert("libc");
                }
            }
            for kind in kinds {
                *found.entry((file.to_string(), kind)).or_insert(0) += 1;
            }
        }
    }
    let allowed: BTreeMap<(String, &str), usize> = [
        ("broker.rs", "network", 2), // header types for the released credential
        ("broker.rs", "vault", 3),
        ("browser/proxy.rs", "network", 9),
        ("display/server.rs", "network", 1),
        ("display/server.rs", "x11", 18),
        ("egress/mod.rs", "network", 8), // header types
        ("egress/transport.rs", "network", 19),
        ("launcher.rs", "ends", 1),
        ("launcher.rs", "libc", 13),
        ("launcher.rs", "process", 1),
        ("tool/mod.rs", "sealed", 6),
    ]
    .into_iter()
    .map(|(file, kind, count)| ((file.to_string(), kind), count))
    .collect();
    assert_eq!(found, allowed);
    // Unsafe code is denied crate-wide and allowed only in the launcher.
    let lib = production_source("crates/nexus-governed-control/src/lib.rs");
    assert!(lib.contains("#![deny(unsafe_code)]"));
    let allowing: Vec<&str> = p3_sources()
        .into_iter()
        .filter(|(_, text)| text.contains("allow(unsafe_code)"))
        .map(|(file, _)| file)
        .collect();
    assert_eq!(allowing, ["launcher.rs"]);
}

/// Nothing in Phase Three reads the process environment: not the owner's
/// display or X authority, not proxies, not credentials; nor does it set
/// any variable.
#[test]
fn p3_g6_05_phase_three_reads_no_ambient_environment() {
    for (file, text) in p3_sources() {
        for ambient in [
            "env::var",
            "var_os(",
            "set_var(",
            "remove_var(",
            "\"DISPLAY\"",
            "\"XAUTHORITY\"",
            "home_dir()",
            "temp_dir()",
            "current_dir()",
            "current_exe()",
        ] {
            assert!(!text.contains(ambient), "{file}: {ambient}");
        }
    }
    let front = production_source(P3_FRONT_DOOR);
    for ambient in [
        "env::var",
        "var_os(",
        "set_var(",
        "\"DISPLAY\"",
        "temp_dir()",
    ] {
        assert!(!front.contains(ambient), "front door: {ambient}");
    }
}

/// The desktop reaches Phase Three only through its front door and the
/// executor's classification; the IPC commands take data only.
#[test]
fn p3_g6_06_the_desktop_reaches_phase_three_only_through_its_front_door() {
    let naming: BTreeSet<&str> = files_naming("nexus_governed_control")
        .into_iter()
        .filter(|file| file.starts_with(DESKTOP_SRC))
        .collect();
    assert_eq!(
        naming,
        BTreeSet::from([P3_FRONT_DOOR, "app/src-tauri/src/commands/cognitive.rs"])
    );
    let cognitive = production_source("app/src-tauri/src/commands/cognitive.rs");
    assert_eq!(cognitive.matches("nexus_governed_control").count(), 1);
    // The commands and their parameters, exactly.
    let front = production_source(P3_FRONT_DOOR);
    let mut commands: Vec<(String, String)> = fn_items(front)
        .into_iter()
        .filter(|item| item.head.contains("#[command]"))
        .map(|item| (item.name.clone(), item.params.clone()))
        .collect();
    commands.sort();
    let expected: Vec<(String, String)> = [
        (
            "p3_approve",
            "(app:App,state:State<'_,AppState>,commitment:String)",
        ),
        ("p3_cancel_run", "(state:State<'_,AppState>,run:String)"),
        ("p3_deny", "(state:State<'_,AppState>,commitment:String)"),
        ("p3_display_start", "(state:State<'_,AppState>)"),
        ("p3_display_stop", "(state:State<'_,AppState>)"),
        ("p3_emergency_stop", "(state:State<'_,AppState>)"),
        ("p3_evidence", "(state:State<'_,AppState>)"),
        ("p3_import_attachment", "(app:App,state:State<'_,AppState>)"),
        (
            "p3_request_grant",
            "(app:App,state:State<'_,AppState>,request:GrantRequest,ttl_secs:u64)",
        ),
        ("p3_resume", "(app:App,state:State<'_,AppState>)"),
        ("p3_revoke_grant", "(state:State<'_,AppState>,grant:String)"),
        ("p3_status", "(state:State<'_,AppState>)"),
        (
            "p3_submit",
            "(state:State<'_,AppState>,envelope:CommandEnvelope)",
        ),
    ]
    .into_iter()
    .map(|(name, params)| (name.to_string(), params.to_string()))
    .collect();
    assert_eq!(commands, expected);
    // Every one is registered, listed and granted, and no other command
    // carries the prefix.
    let registered: BTreeSet<String> =
        registry_entries(production_source("app/src-tauri/src/lib.rs"))
            .iter()
            .map(|entry| split_entry(entry).1.to_string())
            .filter(|name| name.starts_with("p3_"))
            .collect();
    let names: BTreeSet<String> = expected.iter().map(|(name, _)| name.clone()).collect();
    assert_eq!(registered, names);
    for name in &names {
        assert!(
            crate::webview_boundary::APP_COMMANDS.contains(&name.as_str()),
            "{name}"
        );
    }
    // The dialog-bound commands run on the blocking pool with the native dialogs.
    for name in ["p3_approve", "p3_resume", "p3_request_grant"] {
        let item = one_fn(front, name);
        let body = compact(item.body_text(front));
        assert!(body.contains("blocking(move||"), "{name}");
        assert!(body.contains("ControlDialogs(app)"), "{name}: {body}");
    }
}

/// The final classification is exhaustive and has no wildcard: a new
/// `PlannedAction` variant fails to compile there until it is classified,
/// and every variant is named.
#[test]
fn p3_g6_07_the_final_classification_names_every_variant() {
    let planned = production_source("crates/nexus-governed-control/src/planned.rs");
    let arms = match_arms(planned, "classify");
    let mut named = Vec::new();
    for (pattern, _) in &arms {
        let variants = pattern_variants(pattern)
            .unwrap_or_else(|| panic!("classify has a catch-all arm `{pattern}`"));
        named.extend(variants);
    }
    let named: BTreeSet<String> = named.into_iter().collect();
    let declared: BTreeSet<String> = enum_variants(
        production_source("kernel/src/cognitive/types.rs"),
        "PlannedAction",
    )
    .into_iter()
    .collect();
    assert_eq!(named, declared);
}

// ---------------------------------------------------------------------------
// §22 direct-bypass closure: every production real-world mechanism in the
// workspace has exactly one of four classes.
// ---------------------------------------------------------------------------

/// The four classes of PHASE-THREE-FINAL §22. There is no fifth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    /// Phase Three governed control: `crates/nexus-governed-control`, and
    /// nothing else.
    Phase3,
    /// An existing, stronger governed route: Phase Zero, One or Two
    /// governance or an Architect decision, named in the reason.
    Governed,
    /// Intentionally closed: every desktop route to it is closed, or it has
    /// no production caller (latent, kept so by a needle or a closed
    /// command).
    Closed,
    /// Outside the shipped product: a withdrawn binary or a developer tool.
    NonProduction,
}

/// The kinds of real-world mechanism a resolved path names. Phase Zero's
/// process, termination and network predicates, extended with WebSocket
/// clients; the X server; the credential vault (its global facade and the
/// OS keyring); sealed spawns; launching the OS's opener or browser; and
/// direct screen, input, clipboard, audio and browser-driver crates.
fn effect_kinds(path: &[String]) -> BTreeSet<&'static str> {
    use crate::phase0_surface::rust_paths::{
        constructs_process, ends_process, opens_network, starts_with,
    };
    // A crate's item has at least two segments; a lone name is a local.
    let any = |prefixes: &[&str]| {
        path.len() > 1 && prefixes.iter().any(|prefix| starts_with(path, prefix))
    };
    let shown = path.join("::");
    let mut kinds = BTreeSet::new();
    if constructs_process(path)
        || any(&[
            "open::that",
            "open::that_detached",
            "open::that_in_background",
            "open::with",
            "open::with_detached",
            "open::with_in_background",
            "open::commands",
            "opener",
            "webbrowser",
        ])
    {
        kinds.insert("process");
    }
    if ends_process(path) {
        kinds.insert("ends");
    }
    if opens_network(path) || any(&["tokio_tungstenite", "tungstenite", "async_tungstenite"]) {
        kinds.insert("network");
    }
    if starts_with(path, "x11rb") {
        kinds.insert("x11");
    }
    if shown.contains("secrets::global::try_facade")
        || shown.contains("secrets::global::facade")
        || any(&["keyring"])
    {
        kinds.insert("vault");
    }
    if shown.ends_with("SealedSpawnSpec") {
        kinds.insert("sealed");
    }
    if any(&[
        "xcap",
        "screenshots",
        "scrap",
        "enigo",
        "rdev",
        "device_query",
        "inputbot",
        "autopilot",
        "mouse_rs",
        "uinput",
        "evdev",
        "arboard",
        "copypasta",
        "cli_clipboard",
        "cpal",
        "rodio",
        "portaudio",
    ]) {
        kinds.insert("device");
    }
    if any(&[
        "headless_chrome",
        "chromiumoxide",
        "fantoccini",
        "thirtyfour",
    ]) {
        kinds.insert("browser");
    }
    kinds
}

/// (file, kind) -> occurrences, over every workspace production source: each
/// production occurrence that resolves to a mechanism counts once per kind
/// (a private `use` only binds a name, so its uses count instead).
fn measured_effect_sites() -> BTreeMap<(String, &'static str), usize> {
    use crate::phase0_surface::rust_paths::{Analysis, Declaration};
    let mut sites = BTreeMap::new();
    for (file, _) in workspace_sources() {
        let src = workspace_file(file);
        let analysis = Analysis::new(&src, &["crate"]);
        for o in analysis.production() {
            if o.declaration == Some(Declaration::Use) && !o.public && o.item.is_none() {
                continue;
            }
            let kinds: BTreeSet<&str> = o.resolved.iter().flat_map(|p| effect_kinds(p)).collect();
            for kind in kinds {
                *sites.entry((file.clone(), kind)).or_insert(0) += 1;
            }
        }
    }
    sites
}

/// Every production real-world mechanism in the workspace, resolved
/// structurally: (file, kind, count). A new occurrence anywhere fails.
const EFFECT_SITES: &[(&str, &str, usize)] = &[
    ("agents/coder/src/context.rs", "process", 1),
    ("agents/coder/src/git.rs", "process", 1),
    ("agents/coder/src/scanner.rs", "process", 1),
    ("agents/coder/src/test_runner.rs", "process", 2),
    ("agents/coding-agent/src/lib.rs", "process", 2),
    ("agents/web-builder/src/deploy/cloudflare.rs", "network", 7),
    ("agents/web-builder/src/deploy/mod.rs", "network", 6),
    ("agents/web-builder/src/deploy/netlify.rs", "network", 5),
    ("agents/web-builder/src/deploy/vercel.rs", "network", 4),
    ("agents/web-builder/src/dev_server.rs", "ends", 3),
    ("agents/web-builder/src/dev_server.rs", "network", 2),
    ("agents/web-builder/src/dev_server.rs", "process", 3),
    ("agents/web-builder/src/image_gen/api.rs", "network", 1),
    ("agents/web-builder/src/image_gen/local.rs", "network", 1),
    ("agents/web-builder/src/image_gen/local.rs", "process", 1),
    ("agents/web-builder/src/theme_extract.rs", "network", 4),
    (
        "app/src-tauri/src/builder_workspace/dev_server_launch.rs",
        "network",
        1,
    ),
    (
        "app/src-tauri/src/builder_workspace/dev_server_launch.rs",
        "sealed",
        2,
    ),
    ("app/src-tauri/src/commands/agents.rs", "vault", 1),
    ("app/src-tauri/src/commands/apps.rs", "network", 2),
    ("app/src-tauri/src/commands/chat_llm.rs", "process", 2),
    ("app/src-tauri/src/commands/chat_llm.rs", "vault", 2),
    ("app/src-tauri/src/commands/flash.rs", "process", 1),
    ("app/src-tauri/src/commands/trust_security.rs", "process", 1),
    ("auth/src/config.rs", "vault", 1),
    ("auth/src/error.rs", "network", 2),
    ("auth/src/oidc.rs", "network", 2),
    ("cli/src/lib.rs", "process", 1),
    ("connectors/core/src/http_connector.rs", "vault", 1),
    ("connectors/core/src/validation.rs", "process", 2),
    ("connectors/llm/src/model_hub.rs", "process", 4),
    ("connectors/llm/src/nexus_link.rs", "network", 2),
    ("connectors/llm/src/providers/claude.rs", "network", 2),
    ("connectors/llm/src/providers/mod.rs", "network", 13),
    ("connectors/llm/src/providers/mod.rs", "process", 3),
    ("connectors/llm/src/providers/ollama.rs", "network", 1),
    ("connectors/llm/src/providers/ollama.rs", "process", 3),
    ("connectors/messaging/src/discord.rs", "network", 3),
    ("connectors/messaging/src/matrix.rs", "network", 3),
    ("connectors/messaging/src/matrix.rs", "vault", 1),
    ("connectors/messaging/src/slack.rs", "network", 3),
    ("connectors/messaging/src/telegram.rs", "network", 3),
    ("connectors/messaging/src/webhook.rs", "network", 3),
    ("connectors/messaging/src/whatsapp.rs", "network", 3),
    ("connectors/messaging/src/whatsapp.rs", "vault", 2),
    ("connectors/web/src/reader.rs", "process", 1),
    ("connectors/web/src/search.rs", "process", 2),
    ("connectors/web/src/twitter.rs", "network", 9),
    ("connectors/web/src/twitter.rs", "vault", 1),
    ("crates/nexus-browser-agent/src/bridge.rs", "process", 1),
    (
        "crates/nexus-capability-measurement/src/evaluation/nim_client.rs",
        "process",
        1,
    ),
    (
        "crates/nexus-capability-measurement/src/evaluation/openrouter_client.rs",
        "process",
        1,
    ),
    ("crates/nexus-computer-control/src/engine.rs", "process", 1),
    (
        "crates/nexus-computer-use/src/agent/vision.rs",
        "process",
        1,
    ),
    ("crates/nexus-computer-use/src/capability.rs", "process", 2),
    (
        "crates/nexus-computer-use/src/capture/backend.rs",
        "process",
        7,
    ),
    (
        "crates/nexus-computer-use/src/governance/app_registry.rs",
        "process",
        2,
    ),
    (
        "crates/nexus-computer-use/src/input/backend.rs",
        "process",
        3,
    ),
    (
        "crates/nexus-computer-use/src/input/keyboard.rs",
        "process",
        1,
    ),
    ("crates/nexus-computer-use/src/input/mouse.rs", "process", 1),
    ("crates/nexus-external-tools/src/adapter.rs", "process", 1),
    ("crates/nexus-flash-infer/src/downloader.rs", "network", 4),
    ("crates/nexus-flash-infer/src/downloader.rs", "process", 2),
    ("crates/nexus-flash-infer/src/hardware.rs", "process", 2),
    ("crates/nexus-governed-control/src/broker.rs", "network", 2),
    ("crates/nexus-governed-control/src/broker.rs", "vault", 1),
    (
        "crates/nexus-governed-control/src/browser/proxy.rs",
        "network",
        9,
    ),
    (
        "crates/nexus-governed-control/src/display/server.rs",
        "network",
        1,
    ),
    (
        "crates/nexus-governed-control/src/display/server.rs",
        "x11",
        18,
    ),
    (
        "crates/nexus-governed-control/src/egress/mod.rs",
        "network",
        8,
    ),
    (
        "crates/nexus-governed-control/src/egress/transport.rs",
        "network",
        19,
    ),
    ("crates/nexus-governed-control/src/launcher.rs", "ends", 1),
    (
        "crates/nexus-governed-control/src/launcher.rs",
        "process",
        1,
    ),
    ("crates/nexus-governed-control/src/tool/mod.rs", "sealed", 1),
    ("crates/nexus-mcp/src/client.rs", "process", 1),
    ("crates/nexus-mcp/src/tools.rs", "process", 2),
    ("crates/nexus-memory/src/embedding.rs", "process", 1),
    ("crates/nexus-perception/src/vision.rs", "process", 1),
    ("crates/nexus-swarm/src/adapters/herald.rs", "vault", 1),
    (
        "crates/nexus-swarm/src/providers/anthropic.rs",
        "network",
        3,
    ),
    ("crates/nexus-swarm/src/providers/anthropic.rs", "vault", 1),
    (
        "crates/nexus-swarm/src/providers/codex_cli.rs",
        "process",
        2,
    ),
    (
        "crates/nexus-swarm/src/providers/huggingface.rs",
        "network",
        4,
    ),
    (
        "crates/nexus-swarm/src/providers/huggingface.rs",
        "vault",
        1,
    ),
    ("crates/nexus-swarm/src/providers/mod.rs", "network", 2),
    ("crates/nexus-swarm/src/providers/ollama.rs", "network", 3),
    ("crates/nexus-swarm/src/providers/openai.rs", "network", 4),
    ("crates/nexus-swarm/src/providers/openai.rs", "vault", 1),
    (
        "crates/nexus-swarm/src/providers/openrouter.rs",
        "network",
        4,
    ),
    ("crates/nexus-swarm/src/providers/openrouter.rs", "vault", 1),
    (
        "crates/nexus-ui-repair/src/governance/xvfb_session.rs",
        "process",
        1,
    ),
    (
        "crates/nexus-ui-repair/src/specialists/vision_judge.rs",
        "network",
        2,
    ),
    (
        "crates/nexus-ui-repair/src/specialists/vision_judge.rs",
        "process",
        1,
    ),
    (
        "crates/nexus-verifier-sandbox/src/launcher.rs",
        "process",
        1,
    ),
    ("crates/nexus-verifier-sandbox/src/sys.rs", "network", 4),
    ("crates/nexus-verifier-sandbox/src/sys.rs", "process", 1),
    ("distributed/src/tcp_transport.rs", "network", 6),
    ("factory/src/pipeline.rs", "process", 1),
    ("integrations/src/providers/discord.rs", "network", 2),
    ("integrations/src/providers/github.rs", "network", 2),
    ("integrations/src/providers/gitlab.rs", "network", 2),
    ("integrations/src/providers/jira.rs", "network", 2),
    ("integrations/src/providers/servicenow.rs", "network", 2),
    ("integrations/src/providers/slack.rs", "network", 2),
    ("integrations/src/providers/teams.rs", "network", 4),
    ("integrations/src/providers/telegram.rs", "network", 2),
    ("integrations/src/providers/webhook.rs", "network", 2),
    ("kernel/src/actuators/api.rs", "process", 1),
    ("kernel/src/actuators/browser.rs", "process", 1),
    ("kernel/src/actuators/code_exec.rs", "process", 1),
    ("kernel/src/actuators/docker.rs", "process", 1),
    ("kernel/src/actuators/image_gen.rs", "process", 3),
    ("kernel/src/actuators/shell.rs", "process", 1),
    ("kernel/src/actuators/tts.rs", "process", 3),
    ("kernel/src/actuators/web.rs", "process", 2),
    ("kernel/src/coding_run/local_model.rs", "network", 6),
    ("kernel/src/computer_control.rs", "process", 5),
    ("kernel/src/hardware.rs", "process", 5),
    ("kernel/src/protocols/a2a_client.rs", "process", 2),
    ("kernel/src/resource_limiter.rs", "sealed", 1),
    ("kernel/src/resource_limiter/unix.rs", "ends", 1),
    ("kernel/src/resource_limiter/unix.rs", "process", 4),
    ("kernel/src/resource_limiter/unix.rs", "sealed", 1),
    ("kernel/src/resource_limiter/windows.rs", "ends", 1),
    ("kernel/src/resource_limiter/windows.rs", "process", 1),
    ("kernel/src/resource_limiter/windows.rs", "sealed", 1),
    ("kernel/src/secrets/backend_keyring.rs", "vault", 4),
    ("kernel/src/typed_tools.rs", "process", 1),
    ("llama-bridge/src/model.rs", "process", 1),
    ("nexus-code/src/bench/swe_bench.rs", "process", 5),
    ("nexus-code/src/commands/diff.rs", "process", 1),
    ("nexus-code/src/error.rs", "network", 3),
    ("nexus-code/src/llm/provider.rs", "network", 1),
    ("nexus-code/src/llm/providers/anthropic.rs", "network", 4),
    ("nexus-code/src/llm/providers/claude_cli.rs", "process", 3),
    ("nexus-code/src/llm/providers/google.rs", "network", 2),
    (
        "nexus-code/src/llm/providers/openai_compat.rs",
        "network",
        6,
    ),
    ("nexus-code/src/llm/router.rs", "network", 1),
    ("nexus-code/src/llm/streaming.rs", "network", 5),
    ("nexus-code/src/mcp/transport.rs", "network", 2),
    ("nexus-code/src/mcp/transport.rs", "process", 4),
    ("nexus-code/src/setup.rs", "process", 2),
    ("nexus-code/src/tools/bash.rs", "process", 1),
    ("nexus-code/src/tools/git.rs", "process", 1),
    ("nexus-code/src/tools/screen_analyze.rs", "process", 1),
    ("nexus-code/src/tools/screen_capture.rs", "process", 3),
    ("nexus-code/src/tools/screen_interact.rs", "process", 6),
    ("nexus-code/src/tools/search.rs", "process", 2),
    ("nexus-code/src/tools/test_runner.rs", "process", 1),
    ("nexus-code/src/tools/web_fetch.rs", "network", 1),
    ("protocols/src/mcp_client.rs", "process", 3),
    ("protocols/src/server_runtime.rs", "network", 1),
    ("sdk/src/typed_tools.rs", "process", 13),
];

/// The class of every file holding a mechanism, and why (no fifth class).
const EFFECT_FILES: &[(&str, Route, &str)] = &[
    (
        "agents/coder/src/context.rs",
        Route::Closed,
        "latent: the coder conductor is never constructed by the desktop (only the withdrawn CLI and tests)",
    ),
    (
        "agents/coder/src/git.rs",
        Route::Closed,
        "latent: no production caller (agents/coder tests only)",
    ),
    (
        "agents/coder/src/scanner.rs",
        Route::Closed,
        "latent: reached only from the coder conductor, which the desktop never constructs",
    ),
    (
        "agents/coder/src/test_runner.rs",
        Route::Closed,
        "latent: reached only from the coder conductor, which the desktop never constructs",
    ),
    (
        "agents/coding-agent/src/lib.rs",
        Route::NonProduction,
        "not in the desktop dependency closure (withdrawn CLI and integration tests only)",
    ),
    (
        "agents/web-builder/src/deploy/cloudflare.rs",
        Route::Governed,
        "Builder deploy reads only (check token, list sites): fixed host, bounded no-redirect api_client; uploads closed (LegacyBuilder)",
    ),
    (
        "agents/web-builder/src/deploy/mod.rs",
        Route::Governed,
        "the Builder deploy api_client (no redirect, no referer, 30 s, 8 MiB) for the fixed-host reads; uploads closed",
    ),
    (
        "agents/web-builder/src/deploy/netlify.rs",
        Route::Governed,
        "Builder deploy reads only: fixed host, bounded no-redirect api_client; uploads closed",
    ),
    (
        "agents/web-builder/src/deploy/vercel.rs",
        Route::Governed,
        "Builder deploy reads only: fixed host, bounded no-redirect api_client; deploy closed",
    ),
    (
        "agents/web-builder/src/dev_server.rs",
        Route::Closed,
        "latent legacy launcher: no caller; the desktop launches dev servers only through its sealed Builder lifecycle",
    ),
    (
        "agents/web-builder/src/image_gen/api.rs",
        Route::Closed,
        "image generation commands closed (LegacyBuilder)",
    ),
    (
        "agents/web-builder/src/image_gen/local.rs",
        Route::Closed,
        "its only route, builder_image_gen_status, is closed (HelperLaunch, Phase Three G-INV-5)",
    ),
    (
        "agents/web-builder/src/theme_extract.rs",
        Route::Closed,
        "builder_theme_extract_from_url closed (NetworkDestination)",
    ),
    (
        "app/src-tauri/src/builder_workspace/dev_server_launch.rs",
        Route::Governed,
        "Phase Zero C4C3 Builder lifecycle: revalidated reservation, verified packaged toolchain, sealed Node spawn, loopback readiness probe, no redirect",
    ),
    (
        "app/src-tauri/src/commands/agents.rs",
        Route::Governed,
        "LLM provider keys read from the vault for operator-configured provider endpoints (S7)",
    ),
    (
        "app/src-tauri/src/commands/apps.rs",
        Route::Governed,
        "the fixed-host GitLab marketplace read: no credential, no redirect, bounded time and size (S17; Phase Three G-INV-6)",
    ),
    (
        "app/src-tauri/src/commands/chat_llm.rs",
        Route::Governed,
        "governed curl to the authorized Ollama address only (Final Gate item B, Architect decision F; CURL_SITES); the vault reads are the Phase Zero provider keys",
    ),
    (
        "app/src-tauri/src/commands/flash.rs",
        Route::Governed,
        "the fixed nvidia-smi metrics probe (Architect decision A: fixed name and arguments, read-only)",
    ),
    (
        "app/src-tauri/src/commands/trust_security.rs",
        Route::Governed,
        "the C5C notification: fixed program, the message only as data after --; its only caller is the emergency-stop shortcut",
    ),
    (
        "auth/src/config.rs",
        Route::Closed,
        "latent: the OIDC client-secret read; its only caller, OidcClient, has no production caller (needle OidcClient)",
    ),
    (
        "auth/src/error.rs",
        Route::Closed,
        "an error conversion only (no connection); its producer, OidcClient, has no production caller",
    ),
    (
        "auth/src/oidc.rs",
        Route::Closed,
        "latent: OidcClient has no production caller (needle OidcClient)",
    ),
    (
        "cli/src/lib.rs",
        Route::NonProduction,
        "only the withdrawn nexus binary depends on nexus-cli",
    ),
    (
        "connectors/core/src/http_connector.rs",
        Route::Closed,
        "latent: no caller outside connectors/core (other crates use only its rate limiter)",
    ),
    (
        "connectors/core/src/validation.rs",
        Route::Closed,
        "latent: only the withdrawn nexus-cli setup calls it",
    ),
    (
        "connectors/llm/src/model_hub.rs",
        Route::Governed,
        "model catalog downloads: fixed host, HTTPS-only governed curl, owned and reaped downloads (S8)",
    ),
    (
        "connectors/llm/src/nexus_link.rs",
        Route::Closed,
        "peer transfer closed (PeerTransfer); open nexus_link commands are state-only",
    ),
    (
        "connectors/llm/src/providers/claude.rs",
        Route::Governed,
        "operator-configured provider: fixed endpoint, vault key, bounded no-redirect client (S7)",
    ),
    (
        "connectors/llm/src/providers/mod.rs",
        Route::Governed,
        "Phase Zero governed curl and in-process client for operator-configured providers (S7, C5B)",
    ),
    (
        "connectors/llm/src/providers/ollama.rs",
        Route::Governed,
        "the authorized Ollama address only (decision F); governed curl, children reaped (S7)",
    ),
    (
        "connectors/messaging/src/discord.rs",
        Route::Closed,
        "the messaging gateway never sends or polls; the messaging commands are closed (GovernedRoute)",
    ),
    (
        "connectors/messaging/src/matrix.rs",
        Route::Closed,
        "never constructed in production",
    ),
    (
        "connectors/messaging/src/slack.rs",
        Route::Closed,
        "the messaging gateway never sends or polls; the messaging commands are closed (GovernedRoute)",
    ),
    (
        "connectors/messaging/src/telegram.rs",
        Route::Closed,
        "the messaging gateway never sends or polls; the messaging commands are closed (GovernedRoute)",
    ),
    (
        "connectors/messaging/src/webhook.rs",
        Route::Closed,
        "never constructed in production",
    ),
    (
        "connectors/messaging/src/whatsapp.rs",
        Route::Closed,
        "the messaging gateway never sends or polls; the messaging commands are closed (GovernedRoute)",
    ),
    (
        "connectors/web/src/reader.rs",
        Route::Closed,
        "latent: reached only from the social-poster manifest runner of the withdrawn CLI",
    ),
    (
        "connectors/web/src/search.rs",
        Route::Closed,
        "latent: reached only from the social-poster manifest runner of the withdrawn CLI",
    ),
    (
        "connectors/web/src/twitter.rs",
        Route::Closed,
        "posting only when not dry-run; the desktop's Herald runs drafts-only (P0-002C5C), the manifest runner only from the withdrawn CLI",
    ),
    (
        "crates/nexus-browser-agent/src/bridge.rs",
        Route::Closed,
        "the bridge is never started; the legacy browser commands are closed (GovernedRoute, G-INV-3)",
    ),
    (
        "crates/nexus-capability-measurement/src/evaluation/nim_client.rs",
        Route::Closed,
        "its validation runs are closed (AmbientResource)",
    ),
    (
        "crates/nexus-capability-measurement/src/evaluation/openrouter_client.rs",
        Route::Closed,
        "latent: no production constructor",
    ),
    (
        "crates/nexus-computer-control/src/engine.rs",
        Route::Closed,
        "cc_execute_action is closed (ProcessExecution)",
    ),
    (
        "crates/nexus-computer-use/src/agent/vision.rs",
        Route::Closed,
        "the computer-use agent loop has no production caller; its desktop routes are closed (ScreenObservation, GovernedRoute)",
    ),
    (
        "crates/nexus-computer-use/src/capability.rs",
        Route::Closed,
        "its only route, nx_computer_use_status, is closed (GovernedRoute, G-INV-2)",
    ),
    (
        "crates/nexus-computer-use/src/capture/backend.rs",
        Route::Closed,
        "the computer-use agent loop has no production caller; its desktop routes are closed (ScreenObservation, GovernedRoute)",
    ),
    (
        "crates/nexus-computer-use/src/governance/app_registry.rs",
        Route::Closed,
        "the computer-use agent loop has no production caller; its desktop routes are closed (ScreenObservation, GovernedRoute)",
    ),
    (
        "crates/nexus-computer-use/src/input/backend.rs",
        Route::Closed,
        "the computer-use agent loop has no production caller; its desktop routes are closed (ScreenObservation, GovernedRoute)",
    ),
    (
        "crates/nexus-computer-use/src/input/keyboard.rs",
        Route::Closed,
        "the computer-use agent loop has no production caller; its desktop routes are closed (ScreenObservation, GovernedRoute)",
    ),
    (
        "crates/nexus-computer-use/src/input/mouse.rs",
        Route::Closed,
        "the computer-use agent loop has no production caller; its desktop routes are closed (ScreenObservation, GovernedRoute)",
    ),
    (
        "crates/nexus-external-tools/src/adapter.rs",
        Route::Governed,
        "tools_execute reaches only the fixed-host web search (Phase Zero refusal set, C5B curl bounds, autonomy floor; PATH curl under decision A)",
    ),
    (
        "crates/nexus-flash-infer/src/downloader.rs",
        Route::Governed,
        "model downloads from the fixed Hugging Face host with identifier grammar and bounds (S8); df under decision A",
    ),
    (
        "crates/nexus-flash-infer/src/hardware.rs",
        Route::Governed,
        "fixed-name, fixed-argument hardware probes (Architect decision A, P0-LINUX-FINAL-R1)",
    ),
    (
        "crates/nexus-governed-control/src/broker.rs",
        Route::Phase3,
        "the credential broker: vault reads under executing commitments, header types",
    ),
    (
        "crates/nexus-governed-control/src/browser/proxy.rs",
        Route::Phase3,
        "the governed browser's session proxy (granted origins, egress address policy, pinned)",
    ),
    (
        "crates/nexus-governed-control/src/display/server.rs",
        Route::Phase3,
        "the agent display: backend-owned Xvfb, cookie-authorized X11 socket",
    ),
    (
        "crates/nexus-governed-control/src/egress/mod.rs",
        Route::Phase3,
        "governed egress (header types)",
    ),
    (
        "crates/nexus-governed-control/src/egress/transport.rs",
        Route::Phase3,
        "the pinned governed transport",
    ),
    (
        "crates/nexus-governed-control/src/launcher.rs",
        Route::Phase3,
        "the governed session launcher (own process group, parent-death signal, kill-group-and-reap)",
    ),
    (
        "crates/nexus-governed-control/src/tool/mod.rs",
        Route::Phase3,
        "governed tools through the kernel's sealed spawn",
    ),
    (
        "crates/nexus-mcp/src/client.rs",
        Route::Closed,
        "the MCP client commands are closed (ProcessExecution)",
    ),
    (
        "crates/nexus-mcp/src/tools.rs",
        Route::Closed,
        "mcp2_server_handle is closed (AmbientResource)",
    ),
    (
        "crates/nexus-memory/src/embedding.rs",
        Route::Closed,
        "latent: the embedder is never constructed in production",
    ),
    (
        "crates/nexus-perception/src/vision.rs",
        Route::Closed,
        "perception_init is closed (CredentialTransport)",
    ),
    (
        "crates/nexus-swarm/src/adapters/herald.rs",
        Route::Governed,
        "swarm Herald runs drafts-only (forced dry run, P0-002C5C); the credential lookup runs only when not dry-run, never from the desktop",
    ),
    (
        "crates/nexus-swarm/src/providers/anthropic.rs",
        Route::Governed,
        "operator-configured provider: fixed host, vault key, no redirects, capped reads (S7)",
    ),
    (
        "crates/nexus-swarm/src/providers/codex_cli.rs",
        Route::Closed,
        "external CLI agents are closed (ExternalCliAgent); not registered by the desktop",
    ),
    (
        "crates/nexus-swarm/src/providers/huggingface.rs",
        Route::Governed,
        "operator-configured provider: fixed host, vault token, no redirects (S7)",
    ),
    (
        "crates/nexus-swarm/src/providers/mod.rs",
        Route::Governed,
        "the bounded body readers of the governed providers (S7)",
    ),
    (
        "crates/nexus-swarm/src/providers/ollama.rs",
        Route::Governed,
        "the operator-configured Ollama address (decision F), no credential (S7)",
    ),
    (
        "crates/nexus-swarm/src/providers/openai.rs",
        Route::Governed,
        "operator-configured provider: fixed host, vault key, no redirects (S7)",
    ),
    (
        "crates/nexus-swarm/src/providers/openrouter.rs",
        Route::Governed,
        "operator-configured provider: fixed host, vault key, no redirects (S7)",
    ),
    (
        "crates/nexus-ui-repair/src/governance/xvfb_session.rs",
        Route::NonProduction,
        "developer QA scout (decision M): not in the desktop's dependency closure",
    ),
    (
        "crates/nexus-ui-repair/src/specialists/vision_judge.rs",
        Route::NonProduction,
        "developer QA scout (decision M): not in the desktop's dependency closure",
    ),
    (
        "crates/nexus-verifier-sandbox/src/launcher.rs",
        Route::Governed,
        "Phase Two verifier: native launch approval, verified packaged helper and toolchain",
    ),
    (
        "crates/nexus-verifier-sandbox/src/sys.rs",
        Route::Governed,
        "Phase Two verifier sandbox (seqpacket control socket; an interface-flags ioctl, no connection)",
    ),
    (
        "distributed/src/tcp_transport.rs",
        Route::Closed,
        "latent: TcpTransportManager has no production caller (needle)",
    ),
    (
        "factory/src/pipeline.rs",
        Route::Closed,
        "the factory build, test and pipeline commands are closed (ProcessExecution, FileSelection)",
    ),
    (
        "integrations/src/providers/discord.rs",
        Route::Closed,
        "latent (S13): the desktop's integration router has no provider configured and no send caller",
    ),
    (
        "integrations/src/providers/github.rs",
        Route::Closed,
        "latent (S13): never constructed",
    ),
    (
        "integrations/src/providers/gitlab.rs",
        Route::Closed,
        "latent (S13): never constructed",
    ),
    (
        "integrations/src/providers/jira.rs",
        Route::Closed,
        "latent (S13): never constructed",
    ),
    (
        "integrations/src/providers/servicenow.rs",
        Route::Closed,
        "latent (S13): never constructed",
    ),
    (
        "integrations/src/providers/slack.rs",
        Route::Closed,
        "latent (S13): the desktop's integration router has no provider configured and no send caller",
    ),
    (
        "integrations/src/providers/teams.rs",
        Route::Closed,
        "latent (S13): the desktop's integration router has no provider configured and no send caller",
    ),
    (
        "integrations/src/providers/telegram.rs",
        Route::Closed,
        "latent (S13): the desktop's integration router has no provider configured and no send caller",
    ),
    (
        "integrations/src/providers/webhook.rs",
        Route::Closed,
        "latent (S13): the desktop's integration router has no provider configured and no send caller",
    ),
    (
        "kernel/src/actuators/api.rs",
        Route::Closed,
        "ApiCall is governed egress in Phase Three and never reaches the registry; the Phase Zero fallback closes it (AgentExecution)",
    ),
    (
        "kernel/src/actuators/browser.rs",
        Route::Closed,
        "BrowserAutomate is a governed browser session in Phase Three (closed with a screenshot directory); the kernel actuator is never reached",
    ),
    (
        "kernel/src/actuators/code_exec.rs",
        Route::Closed,
        "CodeExecute stays closed",
    ),
    (
        "kernel/src/actuators/docker.rs",
        Route::Closed,
        "DockerCommand stays closed",
    ),
    (
        "kernel/src/actuators/image_gen.rs",
        Route::Closed,
        "ImageGenerate stays closed",
    ),
    (
        "kernel/src/actuators/shell.rs",
        Route::Closed,
        "ShellCommand stays closed",
    ),
    (
        "kernel/src/actuators/tts.rs",
        Route::Closed,
        "TextToSpeech runs as the governed speech.synthesize tool (or is closed); the kernel actuator is never reached",
    ),
    (
        "kernel/src/actuators/web.rs",
        Route::Governed,
        "fixed-host WebSearch (Phase Zero class A; governed curl, CURL_SITES); its WebFetch arm is Phase Three egress or closed",
    ),
    (
        "kernel/src/coding_run/local_model.rs",
        Route::Governed,
        "Phase One coding run: the authorized loopback Ollama, in-process, no proxy, no redirect, bounded",
    ),
    (
        "kernel/src/computer_control.rs",
        Route::Closed,
        "capture, vision and input commands are closed (ScreenObservation, OsInput); the engine is never enabled",
    ),
    (
        "kernel/src/hardware.rs",
        Route::Governed,
        "fixed-name, fixed-argument hardware probes (Architect decision A, read-only)",
    ),
    (
        "kernel/src/protocols/a2a_client.rs",
        Route::Closed,
        "every network A2A command is closed (NetworkDestination); A2aDelegation stays closed",
    ),
    (
        "kernel/src/resource_limiter.rs",
        Route::Governed,
        "the sealed-spawn specification (P0-002C4C2), used by the Builder lifecycle and the Phase Three tool launcher",
    ),
    (
        "kernel/src/resource_limiter/unix.rs",
        Route::Governed,
        "the sealed-spawn substrate: absolute program, cleared environment, own process group and limits, owner-held killpg",
    ),
    (
        "kernel/src/resource_limiter/windows.rs",
        Route::Governed,
        "the Windows sealed-spawn substrate (kill-on-close job); Windows makes no Phase Three claim",
    ),
    (
        "kernel/src/secrets/backend_keyring.rs",
        Route::Governed,
        "the kernel vault's OS keyring backend, reached only through the secrets facade",
    ),
    (
        "kernel/src/typed_tools.rs",
        Route::Closed,
        "execute_tool is closed (ProcessExecution, ApprovalRequired); needle execute_typed_tool",
    ),
    (
        "llama-bridge/src/model.rs",
        Route::Closed,
        "no open route loads a model (flash sessions closed, FileSelection)",
    ),
    (
        "nexus-code/src/bench/swe_bench.rs",
        Route::NonProduction,
        "benchmark code, unreferenced outside itself",
    ),
    (
        "nexus-code/src/commands/diff.rs",
        Route::NonProduction,
        "CLI-only slash command (the TUI and REPL entry points have no caller)",
    ),
    (
        "nexus-code/src/error.rs",
        Route::Closed,
        "an error type only, no effect site of its own",
    ),
    (
        "nexus-code/src/llm/provider.rs",
        Route::Closed,
        "nexus-code executor routes are closed in the desktop (nx_chat, nx_tool, nx_agent_run; AgentExecution, ProcessExecution)",
    ),
    (
        "nexus-code/src/llm/providers/anthropic.rs",
        Route::Closed,
        "nexus-code executor routes are closed in the desktop (nx_chat, nx_tool, nx_agent_run; AgentExecution, ProcessExecution)",
    ),
    (
        "nexus-code/src/llm/providers/claude_cli.rs",
        Route::Closed,
        "external CLI agents are closed (ExternalCliAgent); the desktop never constructs it",
    ),
    (
        "nexus-code/src/llm/providers/google.rs",
        Route::Closed,
        "nexus-code executor routes are closed in the desktop (nx_chat, nx_tool, nx_agent_run; AgentExecution, ProcessExecution)",
    ),
    (
        "nexus-code/src/llm/providers/openai_compat.rs",
        Route::Closed,
        "nexus-code executor routes are closed in the desktop (nx_chat, nx_tool, nx_agent_run; AgentExecution, ProcessExecution)",
    ),
    (
        "nexus-code/src/llm/router.rs",
        Route::Closed,
        "nexus-code executor routes are closed in the desktop (nx_chat, nx_tool, nx_agent_run; AgentExecution, ProcessExecution)",
    ),
    (
        "nexus-code/src/llm/streaming.rs",
        Route::Closed,
        "nexus-code executor routes are closed in the desktop (nx_chat, nx_tool, nx_agent_run; AgentExecution, ProcessExecution)",
    ),
    (
        "nexus-code/src/mcp/transport.rs",
        Route::Closed,
        "latent: no caller of the MCP connect paths",
    ),
    (
        "nexus-code/src/setup.rs",
        Route::Closed,
        "its process probes are CLI-only; the desktop diagnostic runs no program (Final Gate item I)",
    ),
    (
        "nexus-code/src/tools/bash.rs",
        Route::Closed,
        "nexus-code executor routes are closed in the desktop (nx_chat, nx_tool, nx_agent_run; AgentExecution, ProcessExecution)",
    ),
    (
        "nexus-code/src/tools/git.rs",
        Route::Closed,
        "nexus-code executor routes are closed in the desktop (nx_chat, nx_tool, nx_agent_run; AgentExecution, ProcessExecution)",
    ),
    (
        "nexus-code/src/tools/screen_analyze.rs",
        Route::Closed,
        "computer-use tools are not even registered in the desktop; executor routes closed",
    ),
    (
        "nexus-code/src/tools/screen_capture.rs",
        Route::Closed,
        "computer-use tools are not even registered in the desktop; executor routes closed",
    ),
    (
        "nexus-code/src/tools/screen_interact.rs",
        Route::Closed,
        "computer-use tools are not even registered in the desktop; executor routes closed",
    ),
    (
        "nexus-code/src/tools/search.rs",
        Route::Closed,
        "nexus-code executor routes are closed in the desktop (nx_chat, nx_tool, nx_agent_run; AgentExecution, ProcessExecution)",
    ),
    (
        "nexus-code/src/tools/test_runner.rs",
        Route::Closed,
        "nexus-code executor routes are closed in the desktop (nx_chat, nx_tool, nx_agent_run; AgentExecution, ProcessExecution)",
    ),
    (
        "nexus-code/src/tools/web_fetch.rs",
        Route::Closed,
        "nexus-code executor routes are closed in the desktop (nx_chat, nx_tool, nx_agent_run; AgentExecution, ProcessExecution)",
    ),
    (
        "protocols/src/mcp_client.rs",
        Route::Closed,
        "mcp_host_connect and mcp_host_call_tool are closed (NetworkDestination); the clients have no caller",
    ),
    (
        "protocols/src/server_runtime.rs",
        Route::Closed,
        "latent: the server binaries are withdrawn and not bundled",
    ),
    (
        "sdk/src/typed_tools.rs",
        Route::Closed,
        "latent: execute_typed_tool has no caller (needles)",
    ),
];

/// §22: every production route able to launch a process, reach the network,
/// drive a browser, capture the screen, move the mouse or press keys, use a
/// credential or run a connector effect is resolved structurally (aliases and
/// module indirection resolve to the real path) and pinned per file; every
/// file holding one has exactly one of the four classes, with its reason.
/// A new mechanism anywhere in the workspace fails here until it is
/// classified.
#[test]
fn p3_g6_08_every_real_world_mechanism_in_the_workspace_is_classified() {
    let measured = measured_effect_sites();
    let pinned: BTreeMap<(String, &str), usize> = EFFECT_SITES
        .iter()
        .map(|(file, kind, count)| ((file.to_string(), *kind), *count))
        .collect();
    assert_eq!(pinned.len(), EFFECT_SITES.len(), "a site is pinned twice");
    if measured != pinned {
        let rows: Vec<String> = measured
            .iter()
            .map(|((file, kind), count)| format!("SITE {file} {kind} {count}"))
            .collect();
        panic!(
            "the workspace's real-world mechanisms changed; classify them. Measured:\n{}",
            rows.join("\n")
        );
    }
    let holding: BTreeSet<&str> = EFFECT_SITES.iter().map(|(file, _, _)| *file).collect();
    let classified: BTreeSet<&str> = EFFECT_FILES.iter().map(|(file, _, _)| *file).collect();
    assert_eq!(
        classified.len(),
        EFFECT_FILES.len(),
        "a file is classified twice"
    );
    assert_eq!(classified, holding);
    for (file, route, reason) in EFFECT_FILES {
        // Phase Three is exactly its crate.
        assert_eq!(
            *route == Route::Phase3,
            file.starts_with(P3_CRATE),
            "{file}"
        );
        assert!(
            !reason.trim().is_empty() && reason.len() <= 160,
            "{file}: {reason}"
        );
    }
    // The crate's own mechanisms are the ones `p3_g6_04` confines.
    let own: BTreeSet<&str> = holding
        .iter()
        .filter_map(|file| file.strip_prefix(P3_CRATE))
        .collect();
    assert_eq!(
        own,
        BTreeSet::from([
            "broker.rs",
            "browser/proxy.rs",
            "display/server.rs",
            "egress/mod.rs",
            "egress/transport.rs",
            "launcher.rs",
            "tool/mod.rs",
        ])
    );
}

/// §22 (classification flag F4): the desktop embeds Nexus Code's `App`, whose
/// LLM router, tool registry, MCP manager and self-improvement engine reach
/// processes, the network and the agent's own prompt. Those effects run only
/// through the closed `nx_*` commands: the desktop names exactly these Nexus
/// Code items (resolved structurally, so an alias still counts), and touches
/// the App's effect-bearing fields only to list tools and configure a router
/// slot (whitespace-insensitively, so a call split across lines still counts).
#[test]
fn p3_g6_09_the_embedded_nexus_code_app_only_lists_and_configures() {
    use crate::phase0_surface::rust_paths::{starts_with, Analysis};
    let mut named: BTreeSet<(String, String)> = BTreeSet::new();
    let mut fields: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (file, text) in desktop_sources() {
        let analysis = Analysis::new(&workspace_file(&format!("{DESKTOP_SRC}{file}")), &["crate"]);
        for o in analysis.production() {
            for path in &o.resolved {
                if path.len() > 1 && starts_with(path, "nexus_code") {
                    named.insert((file.to_string(), path.join("::")));
                }
            }
        }
        let flat = without_whitespace(text);
        for field in [
            "router",
            "tool_registry",
            "mcp_manager",
            "self_improve",
            "envelope",
        ] {
            let dotted = format!(".{field}");
            for (at, _) in flat.match_indices(&dotted) {
                let rest = &flat[at + dotted.len()..];
                if rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                    continue; // a longer name
                }
                let method: String = rest
                    .strip_prefix('.')
                    .unwrap_or_default()
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                *fields
                    .entry((file.to_string(), format!("{field}.{method}")))
                    .or_insert(0) += 1;
            }
        }
    }
    let expected_named: BTreeSet<(String, String)> = NEXUS_CODE_NAMED
        .iter()
        .map(|(file, path)| (file.to_string(), path.to_string()))
        .collect();
    let expected_fields: BTreeMap<(String, String), usize> = NEXUS_CODE_APP_FIELDS
        .iter()
        .map(|(file, field, count)| ((file.to_string(), field.to_string()), *count))
        .collect();
    assert!(
        named == expected_named && fields == expected_fields,
        "the desktop's reach into Nexus Code changed:\n{named:#?}\n{fields:#?}"
    );
    // The one use of the registry's tool objects reads their names and
    // descriptions, nothing else.
    let commands = production_source("app/src-tauri/src/nx_bridge/commands.rs");
    assert_eq!(
        compact(one_fn(commands, "nx_tools").body_text(commands)),
        "letapp=state.app.lock().await;lettools:Vec<Value>=app.tool_registry.all().iter().map(|t|{json!({\"name\":t.name(),\"description\":t.description()})}).collect();Ok(tools)"
    );
}

/// (desktop file, resolved Nexus Code path) for every Nexus Code item the
/// desktop names in production.
const NEXUS_CODE_NAMED: &[(&str, &str)] = &[
    (
        "nx_bridge/commands.rs",
        "nexus_code::llm::router::ModelSlot::Execution",
    ),
    (
        "nx_bridge/commands.rs",
        "nexus_code::llm::router::SlotConfig",
    ),
    (
        "nx_bridge/commands.rs",
        "nexus_code::setup::diagnose_for_desktop",
    ),
    ("nx_bridge/mod.rs", "nexus_code::app::App"),
    ("nx_bridge/mod.rs", "nexus_code::app::App::new_for_desktop"),
    (
        "nx_bridge/mod.rs",
        "nexus_code::config::NxConfig::load_for_desktop",
    ),
    (
        "nx_bridge/mod.rs",
        "nexus_code::setup::diagnose_for_desktop",
    ),
];

/// (desktop file, "field.method", count) for every use of an effect-bearing
/// field of the embedded App (and of fields that share their names).
const NEXUS_CODE_APP_FIELDS: &[(&str, &str, usize)] = &[
    // The swarm's own router (`Arc::clone(&s.router)`), not the App's.
    ("commands/swarm.rs", "router.", 1),
    ("nx_bridge/commands.rs", "router.set_slot", 1),
    // `nx_tools`: names and descriptions only (its body is pinned).
    ("nx_bridge/commands.rs", "tool_registry.all", 1),
    ("nx_bridge/commands.rs", "tool_registry.list", 4),
    ("nx_bridge/mod.rs", "tool_registry.list", 1),
];
