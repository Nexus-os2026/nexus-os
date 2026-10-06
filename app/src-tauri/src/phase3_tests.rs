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
const EXECUTOR_BODY: &str = "use{classify,Disposition};letSome(bridge)=&self.governedelse{ifletSome(closure)=phase0_agent_action_closure(action){returnErr(closed(action.action_type(),closure));}returnself.inner.execute(agent_id,action,audit,hitl_approved);};matchclassify(action){Disposition::Inert=>{ifletSome(closure)=phase0_agent_action_closure(action){returnErr(closed(action.action_type(),closure));}self.inner.execute(agent_id,action,audit,hitl_approved)}Disposition::Governed(intent)=>bridge.act(agent_id,&intent,warden_reviews(action)),Disposition::Orchestrated{max_steps}=>Ok(orchestration_guidance(max_steps)),Disposition::Closed(_)=>Err(closed(action.action_type(),phase0_agent_action_closure(action).unwrap_or(Closure::AgentExecution)))}";

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
            assert!(
                !block.trait_name.starts_with('$'),
                "{file}: an impl of a macro fragment hides its trait"
            );
            if block.trait_name == trait_name {
                impls.push((file, block.self_type));
            }
        }
    }
    impls
}

/// Files renaming `word` with `as`. (A second trait of the same name needs
/// no check here: `trait_impls` matches traits by their last segment, so its
/// impls are counted with the real one's.)
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

/// Every mention of the word `name` in production code under `prefix`
/// (comments and literals left out), except its definitions (`fn name`):
/// a call of any form, a path, an import, an alias, a function pointer.
/// (file, enclosing function) per mention, sorted.
fn mentions(prefix: &str, name: &str) -> Vec<(&'static str, String)> {
    let mut found = Vec::new();
    for (file, text) in workspace_sources() {
        if !file.starts_with(prefix) || !text.contains(name) {
            continue;
        }
        let m = masked(text);
        let functions = fn_items(text);
        for at in words(&m, name) {
            if previous_word(&m, at) == "fn" {
                continue;
            }
            let function = enclosing(&functions, at).map_or_else(String::new, |f| f.path.clone());
            found.push((file.as_str(), function));
        }
    }
    found.sort();
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

/// If a `#[cfg(...)]` attribute starts at `i` and its predicate holds only
/// in test builds (`test`, an `all(..)` with such a member, or an `any(..)`
/// whose members all are), the end of the attribute. An item shipped in
/// some build (`any(test, target_os = "linux")`) is not test-only.
fn test_only_cfg(b: &[u8], i: usize) -> Option<usize> {
    if !b[i..].starts_with(b"#[cfg(") {
        return None;
    }
    let open = i + b"#[cfg(".len();
    let mut depth = 1usize;
    let mut j = open;
    while j < b.len() {
        match b[j] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            b'"' => j = literal_end(b, j)? - 1,
            _ => {}
        }
        j += 1;
    }
    if b.get(j + 1) != Some(&b']') {
        return None;
    }
    let predicate = std::str::from_utf8(&b[open..j]).ok()?;
    cfg_requires_test(predicate).then_some(j + 2)
}

/// Whether a `cfg` predicate holds only when `test` is set, or one of the
/// features only test builds enable (on dev-dependency edges, or by the
/// workspace's integration-test crate; `p3_g6_12` pins that).
fn cfg_requires_test(predicate: &str) -> bool {
    const TEST_ONLY_FEATURES: [&str; 5] = [
        "test-support",
        "test-utils",
        "testing",
        "live-sandbox-harness",
        "development-toolchain",
    ];
    let predicate = predicate.trim();
    if TEST_ONLY_FEATURES
        .iter()
        .any(|feature| predicate == format!("feature = \"{feature}\""))
    {
        return true;
    }
    let members = |inner: &str| -> Vec<String> {
        let mut out = Vec::new();
        let (mut depth, mut start, mut quoted) = (0i32, 0usize, false);
        for (at, c) in inner.char_indices() {
            match c {
                '"' => quoted = !quoted,
                '(' if !quoted => depth += 1,
                ')' if !quoted => depth -= 1,
                ',' if !quoted && depth == 0 => {
                    out.push(inner[start..at].trim().to_string());
                    start = at + 1;
                }
                _ => {}
            }
        }
        out.push(inner[start..].trim().to_string());
        out.retain(|member| !member.is_empty());
        out
    };
    if predicate == "test" {
        true
    } else if let Some(inner) = predicate
        .strip_prefix("all(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        members(inner)
            .iter()
            .any(|member| cfg_requires_test(member))
    } else if let Some(inner) = predicate
        .strip_prefix("any(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let members = members(inner);
        !members.is_empty() && members.iter().all(|member| cfg_requires_test(member))
    } else {
        false
    }
}

/// Production text of a Rust source: comments removed, and every item that
/// only a test build compiles removed (its `cfg` requires `test` or a
/// test-only feature; see `cfg_requires_test`). Literals are kept and are
/// never read as delimiters or attributes.
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
        } else if let Some(attribute) = test_only_cfg(b, i) {
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
        "fn test_only_cfg(b: &[u8], i: usize) -> Option<usize> {",
        "fn cfg_requires_test(predicate: &str) -> bool {",
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
        "#[cfg(any(test, target_os = \"linux\"))]\nfn shipped() {}\n",
        "#[cfg(all(test, unix))]\nfn only_in_tests() {}\n",
        "#[cfg(any(test, all(test, feature = \"x\")))]\nfn also_only_in_tests() {}\n",
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
    assert!(!text.contains("mod tests"), "{text}");
    // An item some build ships is production (it only also compiles in
    // tests); an item no build but a test build compiles is not.
    assert!(
        text.contains("pub fn helper()") && text.contains("fn shipped()"),
        "{text}"
    );
    assert!(
        !text.contains("only_in_tests") && !text.contains("also_only_in_tests"),
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
    }
    // The owner's confirmations are exactly Nexus's own window, pinned
    // whole: its answers and labels, the main-thread hand-off, and the
    // window that shows the text unwrapped and arms its answer only after
    // the delay, at the end and the right edge of the text.
    let front = production_source(P3_FRONT_DOOR);
    for (method, answer) in [
        ("confirm_action", "Allow"),
        ("confirm_grant", "Grant"),
        ("confirm_resume", "Resume"),
    ] {
        let dialog = one_fn(front, &format!("ControlDialogs::{method}"));
        assert_eq!(
            compact(dialog.body_text(front)),
            format!("self.confirm(request.title(),request.message(),\"{answer}\")"),
            "ControlDialogs::{method}"
        );
    }
    for (path, body) in CONFIRMATION_WINDOW {
        assert_eq!(
            compact(one_fn(front, path).body_text(front)),
            *body,
            "{path}"
        );
    }
    assert!(
        front.contains(
            "const ARMING_DELAY: std::time::Duration = std::time::Duration::from_millis(1000);"
        ),
        "the answer stays unarmed for a second"
    );
    for constant in [
        "const DIALOG_TURN_WAIT: std::time::Duration =\n    nexus_governed_control::authority::commitment::MAX_COMMITMENT_TTL;",
    ] {
        assert!(front.contains(constant), "{constant}");
    }
    // The trait is defined once and never renamed.
    assert_eq!(
        renaming("ControlConfirmer"),
        Vec::<&str>::new(),
        "ControlConfirmer is renamed somewhere"
    );
}

/// The owner's confirmation window, pinned whole (normalized text): the
/// one-at-a-time turn (a flag, never a lock held across the dialog), the
/// window and its arming.
const CONFIRMATION_WINDOW: [(&str, &str); 7] = [
    ("DialogTurn::take", "Self::take_within(DIALOG_TURN_WAIT)"),
    ("ControlDialogs::confirm", "use*;letSome(_turn)=DialogTurn::take()else{returnfalse;};let(sender,receiver)=channel();let(title,answer)=(title.to_string(),answer.to_string());letshown=self.0.run_on_main_thread(move||{let(dialog,_)=owner_window(&title,&message,&answer);letsender=RefCell::new(Some(sender));dialog.connect_response(move|dialog,response|{ifletSome(sender)=sender.borrow_mut().take(){let_=sender.send(response==ResponseType::Accept);}dialog.close();});dialog.present();});shown.is_ok()&&receiver.recv().unwrap_or(false)"),
    ("DialogTurn::take_within", "let(busy,freed)=&DIALOG_BUSY;letdeadline=Instant::now()+wait;letmuton_screen=busy.lock().unwrap_or_else(|p|p.into_inner());while*on_screen{letleft=deadline.saturating_duration_since(Instant::now());ifleft.is_zero(){returnNone;}on_screen=freed.wait_timeout(on_screen,left).unwrap_or_else(|p|p.into_inner()).0;}*on_screen=true;Some(DialogTurn)"),
    ("DialogTurn::drop", "let(busy,freed)=&DIALOG_BUSY;*busy.lock().unwrap_or_else(|p|p.into_inner())=false;freed.notify_one();"),
    ("owner_window", "use*;useCell;useRc;letdialog=Dialog::new();dialog.set_title(title);dialog.set_modal(true);dialog.set_keep_above(true);dialog.set_default_size(900,560);dialog.add_button(\"Cancel\",ResponseType::Cancel);letallow=dialog.add_button(answer,ResponseType::Accept);allow.set_sensitive(false);dialog.set_default_response(ResponseType::Cancel);lettext=Label::new(None);letlines:Vec<String>=message.split('\\n').map(|line|format!(\"\\u{200E}{line}\")).collect();text.set_text(&lines.join(\"\\n\"));text.set_line_wrap(false);text.set_xalign(0.0);text.set_yalign(0.0);letmonospace=AttrList::new();monospace.insert(AttrString::new_family(\"monospace\"));text.set_attributes(Some(&monospace));letscroll=ScrolledWindow::builder().build();scroll.set_policy(PolicyType::Automatic,PolicyType::Automatic);scroll.add(&text);dialog.content_area().pack_start(&scroll,true,true,0);letarming=Rc::new(Cell::new(Arming::default()));letupdate:Rc<dynFn()>={let(arming,allow,scroll)=(arming.clone(),allow.clone(),scroll.clone());Rc::new(move||{letmutnow=arming.get();let(down,across)=(scroll.vadjustment(),scroll.hadjustment());now.end_reached|=reached(down.value(),down.page_size(),down.upper());now.edge_reached|=reached(across.value(),across.page_size(),across.upper());arming.set(now);allow.set_sensitive(now.ready());})};foradjustmentin[scroll.vadjustment(),scroll.hadjustment()]{letmoved=update.clone();adjustment.connect_value_changed(move|_|moved());letresized=update.clone();adjustment.connect_changed(move|_|resized());}{let(arming,update)=(arming.clone(),update.clone());timeout_add_local_once(ARMING_DELAY,move||{letmutnow=arming.get();now.delay_passed=true;arming.set(now);update();});}dialog.show_all();(dialog,allow)"),
    ("Arming::ready", "self.delay_passed&&self.end_reached&&self.edge_reached"),
    ("reached", "page>0.0&&value+page>=upper-1.0"),
];

/// An R2 approval exists only after the owner's native answer to exactly
/// that commitment; it cannot be built, copied or deserialized elsewhere.
#[test]
fn p3_g6_02_an_r2_approval_is_minted_once_after_the_native_answer() {
    let commitment = production_source("crates/nexus-governed-control/src/authority/commitment.rs");
    // Every reference anywhere in the crate, `approval.rs` included.
    let mints: Vec<(&str, String)> = references(P3_CRATE, "confirmed");
    assert_eq!(
        mints,
        [(
            "crates/nexus-governed-control/src/authority/commitment.rs",
            "CommitmentRegistry::request_approval".to_string()
        )]
    );
    let request =
        compact(one_fn(commitment, "CommitmentRegistry::request_approval").body_text(commitment));
    // The dialog is shown with no lock held; afterwards the commitment must
    // be exactly what was shown, unreserved and live; the approval is
    // reserved, recorded with the lock released, and minted only once the
    // same reservation of a still live commitment completes.
    assert_in_order(
        &request,
        &[
            "self.flush(deferred);let(request,binding)=asked?;letconfirmed=confirmer.confirm_action(&request);",
            "Some(entry)ifentry.state!=CommitmentState::Prepared||entry.binding!=binding||entry.reserved.is_some()=>{Err(AuthorityError::NotPending)}",
            "self.live_check(id,entry,agent,run,now,&mutdeferred)",
            "if!confirmed{",
            "lettoken=self.reserve(entry,Transition::Approve);",
            "self.flush(deferred);let(token,record)=answered?;let_reserved=Reserved{registry:self,id,token};self.record(&record)?;self.complete(id,token,CommitmentState::Prepared,agent,run,|_,_|())?;",
            "Ok(R2Approval::confirmed(id,binding))",
        ],
        "request_approval",
    );
    assert_eq!(request.matches("confirm_action(").count(), 1);
    let approval = production_source("crates/nexus-governed-control/src/authority/approval.rs");
    let at = approval.find("pub struct R2Approval").expect("R2Approval");
    // Every attribute of the item: from the previous item's end.
    let head = &approval[approval[..at].rfind(['}', ';']).unwrap() + 1..at];
    assert_eq!(
        compact(head),
        "#[derive(Debug)]",
        "no Clone, Copy, Default or serde"
    );
    // Its only impl in the crate is the inherent one, with exactly these
    // functions: no `From`, `Default` or `Deserialize` builds one.
    let mut impls = Vec::new();
    for (file, text) in workspace_sources() {
        if !file.starts_with(P3_CRATE) || !text.contains("R2Approval") {
            continue;
        }
        for block in impl_blocks(&masked(text)) {
            if block.self_type == "R2Approval" {
                impls.push((file.as_str(), block.trait_name));
            }
        }
    }
    assert_eq!(
        impls,
        [(
            "crates/nexus-governed-control/src/authority/approval.rs",
            String::new()
        )]
    );
    let functions: Vec<String> = fn_items(approval)
        .into_iter()
        .filter(|item| item.path.starts_with("R2Approval::"))
        .map(|item| format!("{}fn {}{}", item.head, item.name, item.params))
        .collect();
    assert_eq!(
        functions,
        [
            "pub(crate)fn confirmed(commitment:CommitmentId,binding:Digest)",
            "pubfn commitment(&self)",
            "pubfn binding(&self)",
        ]
    );
    assert!(compact(approval)
        .contains("pub(crate)fnconfirmed(commitment:CommitmentId,binding:Digest)->Self"));
    // Authorizing consumes it and checks it names this commitment's binding
    // (under the lock, before reserving); the authorization is recorded
    // before it takes effect.
    let authorization =
        compact(one_fn(commitment, "CommitmentRegistry::authorization").body_text(commitment));
    assert_in_order(
        &authorization,
        &[
            "ifapproval.commitment()!=id||approval.binding()!=&entry.binding{returnErr(AuthorityError::ApprovalMismatch);}",
            "lettoken=self.reserve(entry,Transition::Authorize);",
        ],
        "CommitmentRegistry::authorization",
    );
    let authorize =
        compact(one_fn(commitment, "CommitmentRegistry::authorize").body_text(commitment));
    assert_in_order(
        &authorize,
        &[
            "self.authorization(id,entry,agent,run,approval,now,&mutdeferred)",
            "self.record(&record)?;self.complete(id,token,CommitmentState::Prepared,agent,run,|entry,_|{entry.approval=approved.or(entry.approval);entry.state=CommitmentState::Authorized;})",
        ],
        "CommitmentRegistry::authorize",
    );
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
    // No file names the trait under another name (an alias would escape the
    // implementation scan above).
    assert!(
        renaming("PendingEffect").is_empty(),
        "PendingEffect is renamed"
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
                if opens_network(path) || network_beyond_sockets(path) {
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
        // A name resolved by a method call (no path names it).
        let t = &analysis.tokens;
        for k in 1..t.len() {
            if !analysis.test[k]
                && (t[k - 1].is(".") || t[k - 1].is("::"))
                && RESOLVING_METHODS.iter().any(|method| t[k].is(method))
            {
                *found.entry((file.to_string(), "network")).or_insert(0) += 1;
            }
        }
    }
    let allowed: BTreeMap<(String, &str), usize> = [
        ("broker.rs", "network", 2), // header types for the released credential
        ("broker.rs", "vault", 5),   // the facade read and the refusal of environment secrets
        ("browser/proxy.rs", "network", 9),
        ("display/server.rs", "network", 1),
        ("display/server.rs", "x11", 32),
        ("egress/destination.rs", "network", 1), // name resolution, then the address policy
        ("egress/mod.rs", "network", 8),         // header types
        ("egress/transport.rs", "network", 19),
        ("launcher.rs", "ends", 2), // SIGTERM with a grace, then SIGKILL
        ("launcher.rs", "libc", 32),
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
    // The crate's items the desktop names, resolved (aliases included),
    // exactly: nothing else of the crate, such as a raw transport, a run's
    // cancellation token or a secret reader, can be named here.
    let mut items = BTreeSet::new();
    for file in naming {
        let own = module_of(file);
        let module: Vec<&str> = own.iter().map(String::as_str).collect();
        let analysis =
            crate::phase0_surface::rust_paths::Analysis::new(&workspace_file(file), &module);
        for o in analysis.production() {
            for path in o.resolved.iter().chain(as_written(&o.written)) {
                if path
                    .first()
                    .is_some_and(|root| root == "nexus_governed_control")
                {
                    items.insert(path.join("::"));
                }
            }
        }
    }
    let items: Vec<&str> = items.iter().map(String::as_str).collect();
    assert_eq!(items, DESKTOP_P3_ITEMS);
}

/// The paths of the governed-control crate the desktop names (see
/// `p3_g6_06`).
const DESKTOP_P3_ITEMS: &[&str] = &[
    "nexus_governed_control::authority::approval::ActionConfirmation",
    "nexus_governed_control::authority::approval::ControlConfirmer",
    "nexus_governed_control::authority::approval::GrantConfirmation",
    "nexus_governed_control::authority::approval::ResumeConfirmation",
    "nexus_governed_control::authority::clock::SystemClock",
    "nexus_governed_control::authority::clock::SystemClock::default",
    "nexus_governed_control::authority::commitment::CommitmentState",
    "nexus_governed_control::authority::commitment::CommitmentState::Executing",
    "nexus_governed_control::authority::commitment::CommitmentView",
    "nexus_governed_control::authority::commitment::MAX_COMMITMENT_TTL",
    "nexus_governed_control::authority::evidence::EvidenceRecord",
    "nexus_governed_control::authority::evidence::EvidenceRecord::to_json",
    "nexus_governed_control::authority::evidence::EvidenceSink",
    "nexus_governed_control::authority::evidence::EvidenceUnavailable",
    "nexus_governed_control::authority::evidence::MemoryEvidence",
    "nexus_governed_control::authority::evidence::MemoryEvidence::new",
    "nexus_governed_control::authority::evidence::TeeEvidence",
    "nexus_governed_control::authority::ids::AgentId",
    "nexus_governed_control::authority::ids::AgentId::new",
    "nexus_governed_control::authority::ids::AgentId::owner_session",
    "nexus_governed_control::authority::ids::CommitmentId",
    "nexus_governed_control::authority::ids::CommitmentId::parse",
    "nexus_governed_control::authority::ids::GrantId",
    "nexus_governed_control::authority::ids::GrantId::parse",
    "nexus_governed_control::authority::ids::RunId",
    "nexus_governed_control::authority::ids::RunId::parse",
    "nexus_governed_control::authority::run::RunOrigin",
    "nexus_governed_control::authority::run::RunOrigin::AgentGoal",
    "nexus_governed_control::authority::run::RunOrigin::Command",
    "nexus_governed_control::broker::Vault",
    "nexus_governed_control::broker::Vault::Kernel",
    "nexus_governed_control::control::EffectOutput",
    "nexus_governed_control::governed::AgentOutcome",
    "nexus_governed_control::governed::AgentOutcome::AwaitingApproval",
    "nexus_governed_control::governed::AgentOutcome::Done",
    "nexus_governed_control::governed::GovernedControl",
    "nexus_governed_control::governed::GovernedControl::new",
    "nexus_governed_control::governed::GrantRequest",
    "nexus_governed_control::governed::Intent",
    "nexus_governed_control::ingress::Attachments",
    "nexus_governed_control::ingress::Attachments::default",
    "nexus_governed_control::ingress::CommandEnvelope",
    "nexus_governed_control::ingress::MAX_ATTACHMENT",
    "nexus_governed_control::ingress::Understood",
    "nexus_governed_control::ingress::Understood::Intent",
    "nexus_governed_control::ingress::Understood::NotUnderstood",
    "nexus_governed_control::ingress::understand",
    "nexus_governed_control::planned::Disposition",
    "nexus_governed_control::planned::Disposition::Closed",
    "nexus_governed_control::planned::Disposition::Governed",
    "nexus_governed_control::planned::Disposition::Inert",
    "nexus_governed_control::planned::Disposition::Orchestrated",
    "nexus_governed_control::planned::classify",
];

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

/// What reaches the network beyond Phase Zero's socket list: name
/// resolution (std's `ToSocketAddrs`, libc's resolver functions, the DNS
/// crates) and protocol crates that open their own connections.
/// (`tokio::net::lookup_host` is under `tokio::net`, which `opens_network`
/// counts; `std::net::lookup_host` is listed in case a toolchain gains it.)
/// A resolver call made as a method is not a path: see `RESOLVING_METHODS`.
fn network_beyond_sockets(path: &[String]) -> bool {
    use crate::phase0_surface::rust_paths::starts_with;
    (path.len() > 1
        && [
            "std::net::ToSocketAddrs",
            "std::net::lookup_host",
            "hickory_resolver",
            "hickory_proto",
            "hickory_client",
            "trust_dns_resolver",
            "trust_dns_proto",
            "trust_dns_client",
            "dns_lookup",
            "hyper_util",
            "h2",
            "h3",
            "quinn",
            "quinn_proto",
            "async_net",
            "smol::net",
            // Network-capable crates already in production dependencies: the
            // Prometheus exporter (an HTTP listener, a push gateway) and the
            // Hugging Face hub client.
            "metrics_exporter_prometheus",
            "hf_hub",
        ]
        .iter()
        .any(|prefix| starts_with(path, prefix)))
        || path.windows(2).any(|pair| {
            pair[0] == "libc"
                && [
                    "getaddrinfo",
                    "gethostbyname",
                    "gethostbyname2",
                    "gethostbyname_r",
                    "gethostbyname2_r",
                    "gethostbyaddr",
                    "gethostbyaddr_r",
                    "getnameinfo",
                    "res_init",
                    "res_query",
                    "res_search",
                    "res_nquery",
                    "res_nsearch",
                ]
                .contains(&pair[1].as_str())
        })
}

/// Methods that resolve a name (`ToSocketAddrs::to_socket_addrs`,
/// `url::Url::socket_addrs`): a call is a network site wherever and however
/// it is made, as a method (`.`) or through any type or trait path (`::`).
const RESOLVING_METHODS: &[&str] = &["to_socket_addrs", "socket_addrs"];

/// The Prometheus exporter's builder methods that start its HTTP listener
/// or push gateway: a call is a network site in a file that names the
/// exporter (its in-process recorder, `install_recorder`, is not).
const EXPORTER_METHODS: &[&str] = &[
    "install",
    "build",
    "with_http_listener",
    "with_push_gateway",
    "with_http_uds_listener",
];

/// The kinds of real-world mechanism a resolved path names. Phase Zero's
/// process, termination and network predicates, extended with WebSocket
/// clients, nix and rustix sockets, rustix's fork, exec and kill calls and
/// sysinfo's signals; raw system calls (`syscall` can start or replace a process
/// outside every predicate); the X server; the desktop bus (D-Bus, and
/// AT-SPI, which reads and drives other applications); loading a shared
/// library; the credential vault (its global facade and the OS keyring);
/// sealed spawns; launching the OS's opener or browser; and direct screen,
/// input, clipboard, audio and browser-driver crates.
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
            "rustix::runtime",
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
    if ends_process(path)
        || any(&[
            "rustix::process::kill_process",
            "rustix::process::kill_process_group",
            "rustix::process::kill_current_process_group",
            "rustix::process::test_kill_process",
            "rustix::process::test_kill_process_group",
            "rustix::process::test_kill_current_process_group",
            "rustix::process::pidfd_send_signal",
            "sysinfo::Signal",
        ])
    {
        kinds.insert("ends");
    }
    if opens_network(path)
        || network_beyond_sockets(path)
        || any(&[
            "tokio_tungstenite",
            "tungstenite",
            "async_tungstenite",
            "nix::sys::socket",
            "rustix::net",
        ])
    {
        kinds.insert("network");
    }
    if any(&["libc::syscall", "nix::libc::syscall"]) {
        kinds.insert("syscall");
    }
    if starts_with(path, "x11rb") {
        kinds.insert("x11");
    }
    if any(&["zbus", "atspi", "dbus"]) {
        kinds.insert("bus");
    }
    if any(&["libloading", "dlopen2"])
        || path.windows(2).any(|pair| {
            pair[0] == "libc"
                && ["dlopen", "dlmopen", "dlsym", "dlvsym"].contains(&pair[1].as_str())
        })
    {
        kinds.insert("dynload");
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
    // The kernel's unsealed spawn: `ResourceLimiter::spawn` takes a
    // `ResourceSpawnSpec`, whose `ResourceProgram::Shell` runs `sh -lc` with
    // a PATH lookup and the inherited environment (also re-exported as
    // `nexus_sdk::resource_limiter`).
    if path
        .iter()
        .any(|segment| segment == "ResourceSpawnSpec" || segment == "ResourceProgram")
        || shown.ends_with("ResourceLimiter::spawn")
    {
        kinds.insert("process");
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

/// Production modules named like test files (`*tests.rs`, `*_test.rs`), which
/// the production scanner skips by name: every `mod` declaring them, up to
/// their crate root, carries no `test` cfg. Their mechanisms are measured
/// with the rest.
const NAME_SKIPPED_MODULES: &[&str] = &[
    "crates/nexus-computer-use/src/bin/agent_test.rs",
    "crates/nexus-computer-use/src/bin/governance_test.rs",
    "crates/nexus-computer-use/src/bin/input_test.rs",
    "crates/nexus-computer-use/src/bin/learn_test.rs",
    "crates/nexus-computer-use/src/bin/screen_test.rs",
    "kernel/src/autopilot/stress_test.rs",
];

/// Workspace sources whose names look like tests but which production
/// compiles (see `NAME_SKIPPED_MODULES`).
fn production_modules_named_like_tests() -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
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
            if in_src && (name.ends_with("tests.rs") || name.ends_with("_test.rs")) {
                out.push(relative);
            }
        }
    }
    let mut candidates = Vec::new();
    walk(&root, &root, &mut candidates);
    candidates
        .into_iter()
        .filter(|file| compiled_in_production(file, 0))
        .collect()
}

/// Whether `file` is a crate root, or is declared by a `mod` without a `test`
/// cfg in a parent module that is itself compiled in production.
fn compiled_in_production(file: &str, depth: usize) -> bool {
    let path = std::path::Path::new(file);
    let dir = path.parent().unwrap();
    let file_name = path.file_name().unwrap().to_str().unwrap();
    if matches!(file_name, "lib.rs" | "main.rs") || dir.ends_with("src/bin") {
        return true;
    }
    if depth > 16 {
        return false;
    }
    let (name, parent_dir) = if file_name == "mod.rs" {
        (
            dir.file_name().unwrap().to_str().unwrap(),
            dir.parent().unwrap(),
        )
    } else {
        (path.file_stem().unwrap().to_str().unwrap(), dir)
    };
    let mut parents: Vec<String> = ["mod.rs", "lib.rs", "main.rs"]
        .iter()
        .map(|candidate| parent_dir.join(candidate).to_string_lossy().into_owned())
        .collect();
    if let (Some(up), Some(dir_name)) = (parent_dir.parent(), parent_dir.file_name()) {
        parents.push(
            up.join(format!("{}.rs", dir_name.to_str().unwrap()))
                .to_string_lossy()
                .into_owned(),
        );
    }
    parents.into_iter().any(|parent| {
        let full = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(&parent);
        let Ok(text) = std::fs::read_to_string(full) else {
            return false;
        };
        let text = without_comments(&text);
        let declared = text.match_indices("mod ").any(|(at, _)| {
            let rest = text[at + 4..].trim_start();
            let Some(after) = rest.strip_prefix(name) else {
                return false;
            };
            if !after.trim_start().starts_with(';') {
                return false;
            }
            // The attributes of this item: after the previous item's end.
            let start = text[..at].rfind([';', '}', '{']).map_or(0, |end| end + 1);
            let attributes: String = text[start..at].split_whitespace().collect();
            !(attributes.contains("#[cfg(test)]") || attributes.contains("#[cfg(all(test"))
        });
        declared && compiled_in_production(&parent, depth + 1)
    })
}

/// `text` without `//` line comments and `/* */` block comments.
fn without_comments(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if let Some(end) = literal_end(b, i) {
            out.extend_from_slice(&b[i..end]);
            i = end;
        } else if let Some(end) = comment_end(b, i) {
            out.push(b' ');
            i = end;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("cuts fall on ASCII boundaries")
}

/// (file, kind) -> occurrences, over every workspace production source: each
/// production occurrence that resolves to a mechanism counts once per kind
/// (a private `use` only binds a name, so its uses count instead).
///
/// Fail closed on shadowing: a path that does not start at `crate`, `self`
/// or `super` is also measured exactly as written. The resolver lets any
/// binding or module of the same name win, including one declared inside a
/// test module (`mod reqwest {}`, `use super::probe as reqwest;`), and that
/// must not hide the file's real `reqwest::get`.
fn measured_effect_sites() -> BTreeMap<(String, &'static str), usize> {
    let mut sites = BTreeMap::new();
    let files = workspace_sources()
        .iter()
        .map(|(file, _)| file.clone())
        .chain(NAME_SKIPPED_MODULES.iter().map(|file| file.to_string()));
    for file in files {
        let src = workspace_file(&file);
        let own = module_of(&file);
        for (kind, count) in file_effect_sites(&file, &src, &own) {
            sites.insert((file.clone(), kind), count);
        }
    }
    sites
}

/// kind -> occurrences in one source, `file` (whose module is `own`); see
/// `measured_effect_sites`.
fn file_effect_sites(file: &str, src: &str, own: &[String]) -> BTreeMap<&'static str, usize> {
    use crate::phase0_surface::rust_paths::{Analysis, Declaration};
    let mut sites = BTreeMap::new();
    let module: Vec<&str> = own.iter().map(String::as_str).collect();
    let analysis = Analysis::new(src, &module);
    for o in analysis.production() {
        if o.declaration == Some(Declaration::Use) && !o.public && o.item.is_none() {
            continue;
        }
        let candidates: Vec<&Vec<String>> =
            o.resolved.iter().chain(as_written(&o.written)).collect();
        let mut kinds: BTreeSet<&str> = candidates.iter().flat_map(|p| effect_kinds(p)).collect();
        if candidates.iter().any(|p| reaches_latent(file, own, p)) {
            kinds.insert("latent");
        }
        for kind in kinds {
            *sites.entry(kind).or_insert(0) += 1;
        }
    }
    // A method call is not a path: a latent effect a value's method reaches
    // counts wherever that method is called, and so does a kill in a file
    // that lists processes with sysinfo (a kill by name or by pid).
    let t = &analysis.tokens;
    let lists_processes = (0..t.len()).any(|k| !analysis.test[k] && t[k].is("sysinfo"));
    let exports_metrics =
        (0..t.len()).any(|k| !analysis.test[k] && t[k].is("metrics_exporter_prometheus"));
    for k in 1..t.len() {
        if analysis.test[k] {
            continue;
        }
        if t[k - 1].is(".") && LATENT_METHODS.iter().any(|method| t[k].is(method)) {
            *sites.entry("latent").or_insert(0) += 1;
        }
        if (t[k - 1].is(".") || t[k - 1].is("::"))
            && RESOLVING_METHODS.iter().any(|method| t[k].is(method))
        {
            *sites.entry("network").or_insert(0) += 1;
        }
        if exports_metrics
            && t[k - 1].is(".")
            && EXPORTER_METHODS.iter().any(|method| t[k].is(method))
        {
            *sites.entry("network").or_insert(0) += 1;
        }
        if lists_processes && t[k - 1].is(".") && (t[k].is("kill") || t[k].is("kill_with")) {
            *sites.entry("ends").or_insert(0) += 1;
        }
        // Inline assembly reaches the kernel without any path.
        if ["asm", "global_asm", "naked_asm"]
            .iter()
            .any(|name| t[k - 1].is(name))
            && t[k].is("!")
        {
            *sites.entry("asm").or_insert(0) += 1;
        }
    }
    // Input device nodes, opened by path.
    for (literal, _, _) in analysis.literals() {
        if literal.contains("/dev/uinput") || literal.contains("/dev/input/") {
            *sites.entry("device").or_insert(0) += 1;
        }
    }
    sites
}

/// The latent closed modules of `EFFECT_FILES` and the paths that reach
/// their mechanisms: (crate directory, library name, module below the crate
/// root (empty: the root), the items that reach the mechanism (empty: the
/// whole module), the same items as the crate root re-exports them). A
/// resolved reference to one from any production source outside the
/// module, inside its crate (`crate::…`) or outside it, is a `latent` site,
/// so a new caller anywhere fails until it is classified. Data types the
/// modules share are not entries; what runs the mechanism is.
type Latent = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
);

const LATENT_ENTRIES: &[Latent] = &[
    (
        "agents/coder",
        "coder_agent",
        "context",
        &["build_context"],
        &[],
    ),
    (
        "agents/coder",
        "coder_agent",
        "fix_loop",
        &[
            "fix_until_pass",
            "fix_until_pass_with",
            "FrameworkTestExecutor",
        ],
        &[],
    ),
    ("agents/coder", "coder_agent", "git", &[], &[]),
    (
        "agents/coder",
        "coder_agent",
        "scanner",
        &["scan_project", "scan_project_with_config"],
        &[],
    ),
    ("agents/coder", "coder_agent", "terminal", &[], &[]),
    (
        "agents/coder",
        "coder_agent",
        "test_runner",
        &["run_tests"],
        &[],
    ),
    // The conductor runs the coder's scanner, context and tests.
    (
        "agents/conductor",
        "nexus_conductor",
        "",
        &["Conductor"],
        &[],
    ),
    // The social poster's real pipeline runs the web search and reader.
    (
        "agents/social-poster",
        "social_poster_agent",
        "",
        &[
            "PipelineDependencies::real",
            "RealReaderStep",
            "RealSearchStep",
            "SocialPosterAgent::new",
            "run_social_poster_from_manifest",
        ],
        &[],
    ),
    (
        "agents/web-builder",
        "web_builder_agent",
        "dev_server",
        &[],
        &[],
    ),
    (
        "auth",
        "nexus_auth",
        "config",
        &["AuthConfig::resolve_client_secret"],
        &["AuthConfig::resolve_client_secret"],
    ),
    (
        "auth",
        "nexus_auth",
        "oidc",
        &["OidcClient"],
        &["OidcClient"],
    ),
    (
        "connectors/core",
        "nexus_connectors_core",
        "github_connector",
        &["GitHubConnector"],
        &[],
    ),
    (
        "connectors/core",
        "nexus_connectors_core",
        "http_connector",
        &[],
        &[],
    ),
    (
        "connectors/core",
        "nexus_connectors_core",
        "validation",
        &[],
        &[],
    ),
    (
        "connectors/web",
        "nexus_connectors_web",
        "reader",
        &["WebReaderConnector"],
        &[],
    ),
    (
        "connectors/web",
        "nexus_connectors_web",
        "search",
        &["WebSearchConnector"],
        &[],
    ),
    (
        "crates/nexus-capability-measurement",
        "nexus_capability_measurement",
        "evaluation::openrouter_client",
        &[],
        &[],
    ),
    (
        "crates/nexus-memory",
        "nexus_memory",
        "embedding",
        &["OllamaEmbedder"],
        &["OllamaEmbedder"],
    ),
    (
        "distributed",
        "nexus_distributed",
        "tcp_transport",
        &[],
        &[],
    ),
    (
        "integrations",
        "nexus_integrations",
        "providers::discord",
        &[],
        &[],
    ),
    (
        "integrations",
        "nexus_integrations",
        "providers::github",
        &[],
        &[],
    ),
    (
        "integrations",
        "nexus_integrations",
        "providers::gitlab",
        &[],
        &[],
    ),
    (
        "integrations",
        "nexus_integrations",
        "providers::jira",
        &[],
        &[],
    ),
    (
        "integrations",
        "nexus_integrations",
        "providers::servicenow",
        &[],
        &[],
    ),
    (
        "integrations",
        "nexus_integrations",
        "providers::slack",
        &[],
        &[],
    ),
    (
        "integrations",
        "nexus_integrations",
        "providers::teams",
        &[],
        &[],
    ),
    (
        "integrations",
        "nexus_integrations",
        "providers::telegram",
        &[],
        &[],
    ),
    (
        "integrations",
        "nexus_integrations",
        "providers::webhook",
        &[],
        &[],
    ),
    // The router builds the providers its configuration names.
    (
        "integrations",
        "nexus_integrations",
        "router",
        &["IntegrationRouter::from_config"],
        &["IntegrationRouter::from_config"],
    ),
    ("nexus-code", "nexus_code", "mcp", &["McpManager"], &[]),
    (
        "nexus-code",
        "nexus_code",
        "mcp::transport",
        &["StdioTransport", "SseTransport"],
        &[],
    ),
    ("protocols", "nexus_protocols", "server_runtime", &[], &[]),
    (
        "sdk",
        "nexus_sdk",
        "typed_tools",
        &["execute_typed_tool", "build_command"],
        &[],
    ),
];

/// Methods of latent values whose effect no path names (see
/// `LATENT_ENTRIES`): the vault read of the OIDC secret and the MCP
/// manager's connections.
const LATENT_METHODS: &[&str] = &["resolve_client_secret", "connect", "connect_all"];

/// Whether `path`, resolved in `file` (whose module is `own`), names a latent
/// entry from outside its module.
fn reaches_latent(file: &str, own: &[String], path: &[String]) -> bool {
    /// Per entry: the crate's source directory, the module's own path from
    /// `crate` (none for the root), and the targets as written outside the
    /// crate and inside it.
    type Targets = (
        String,
        Option<Vec<String>>,
        Vec<Vec<String>>,
        Vec<Vec<String>>,
    );
    static TARGETS: OnceLock<Vec<Targets>> = OnceLock::new();
    let targets = TARGETS.get_or_init(|| {
        let segments = |parts: &[&str]| -> Vec<String> {
            parts
                .iter()
                .flat_map(|part| part.split("::"))
                .filter(|segment| !segment.is_empty())
                .map(str::to_string)
                .collect()
        };
        LATENT_ENTRIES
            .iter()
            .map(|(dir, library, module, items, reexported)| {
                let mut relative: Vec<Vec<String>> = if items.is_empty() {
                    vec![segments(&[module])]
                } else {
                    items.iter().map(|item| segments(&[module, item])).collect()
                };
                relative.extend(reexported.iter().map(|item| segments(&[item])));
                let outside = relative
                    .iter()
                    .map(|path| {
                        segments(&[library])
                            .into_iter()
                            .chain(path.clone())
                            .collect()
                    })
                    .collect();
                let inside = relative
                    .iter()
                    .map(|path| {
                        segments(&["crate"])
                            .into_iter()
                            .chain(path.clone())
                            .collect()
                    })
                    .collect();
                let own_module = (!module.is_empty()).then(|| segments(&["crate", module]));
                (format!("{dir}/src/"), own_module, outside, inside)
            })
            .collect()
    });
    let prefix = |path: &[String], target: &[String]| path.starts_with(target);
    targets.iter().any(|(dir, module, outside, inside)| {
        let in_crate = file.starts_with(dir.as_str());
        if in_crate
            && module
                .as_ref()
                .is_some_and(|module| own.starts_with(module))
        {
            return false;
        }
        outside.iter().any(|target| prefix(path, target))
            || (in_crate && inside.iter().any(|target| prefix(path, target)))
    })
}

/// The module path of a workspace source, from its crate root (`crate`, then
/// the directories and file below the crate's `src`), so that `self::` and
/// `super::` resolve where they are written. A target root (`lib.rs`,
/// `main.rs`, a `src/bin` file) is `crate` itself.
fn module_of(file: &str) -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = std::path::Path::new(file);
    let crate_dir = path
        .ancestors()
        .skip(1)
        .find(|dir| root.join(dir).join("Cargo.toml").is_file())
        .unwrap_or_else(|| panic!("{file}: no crate"));
    let mut module = vec!["crate".to_string()];
    let Ok(below) = path.strip_prefix(crate_dir.join("src")) else {
        return module;
    };
    let parts: Vec<&str> = below.iter().map(|part| part.to_str().unwrap()).collect();
    if parts.first() == Some(&"bin") {
        return module;
    }
    let (file_name, dirs) = parts.split_last().unwrap();
    module.extend(dirs.iter().map(|dir| dir.to_string()));
    let stem = file_name.strip_suffix(".rs").unwrap();
    if !(dirs.is_empty() && matches!(stem, "lib" | "main")) && stem != "mod" {
        module.push(stem.to_string());
    }
    module
}

/// The path exactly as written, unless it starts at `crate`, `self` or
/// `super` (which no binding can shadow).
fn as_written(written: &crate::phase0_surface::rust_paths::Written) -> Option<&Vec<String>> {
    let local = matches!(
        written.segs.first().map(String::as_str),
        Some("crate" | "self" | "super")
    );
    (!local).then_some(&written.segs)
}

/// Every production real-world mechanism in the workspace, resolved
/// structurally: (file, kind, count). A new occurrence anywhere fails.
const EFFECT_SITES: &[(&str, &str, usize)] = &[
    ("agents/coder/src/context.rs", "process", 1),
    ("agents/coder/src/fix_loop.rs", "latent", 1),
    ("agents/coder/src/git.rs", "process", 1),
    ("agents/coder/src/scanner.rs", "process", 1),
    ("agents/coder/src/terminal.rs", "process", 2),
    ("agents/coder/src/test_runner.rs", "process", 2),
    ("agents/coding-agent/src/lib.rs", "process", 2),
    ("agents/conductor/src/lib.rs", "latent", 5),
    ("agents/social-poster/src/lib.rs", "latent", 12),
    ("agents/web-builder/src/deploy/cloudflare.rs", "network", 7),
    ("agents/web-builder/src/deploy/mod.rs", "network", 6),
    ("agents/web-builder/src/deploy/netlify.rs", "network", 5),
    ("agents/web-builder/src/deploy/vercel.rs", "network", 4),
    ("agents/web-builder/src/dev_server.rs", "ends", 3),
    ("agents/web-builder/src/dev_server.rs", "latent", 1),
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
    ("app/src-tauri/src/lib.rs", "latent", 1),
    ("auth/src/config.rs", "vault", 1),
    ("auth/src/error.rs", "network", 2),
    ("auth/src/lib.rs", "latent", 1),
    ("auth/src/oidc.rs", "latent", 1),
    ("auth/src/oidc.rs", "network", 2),
    ("cli/src/lib.rs", "latent", 3),
    ("cli/src/lib.rs", "process", 1),
    ("cli/src/setup.rs", "latent", 3),
    ("connectors/core/src/github_connector.rs", "latent", 2),
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
        32,
    ),
    (
        "crates/nexus-governed-control/src/egress/destination.rs",
        "network",
        1,
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
    ("crates/nexus-governed-control/src/launcher.rs", "ends", 2),
    (
        "crates/nexus-governed-control/src/launcher.rs",
        "process",
        1,
    ),
    (
        "crates/nexus-governed-control/src/launcher.rs",
        "syscall",
        1,
    ),
    ("crates/nexus-governed-control/src/tool/mod.rs", "sealed", 1),
    ("crates/nexus-mcp/src/client.rs", "process", 1),
    ("crates/nexus-mcp/src/tools.rs", "process", 2),
    ("crates/nexus-memory/src/embedding.rs", "process", 1),
    ("crates/nexus-memory/src/lib.rs", "latent", 1),
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
    ("crates/nexus-ui-repair/src/bin/sg5_probe.rs", "bus", 5),
    (
        "crates/nexus-ui-repair/src/governance/xvfb_session.rs",
        "process",
        1,
    ),
    (
        "crates/nexus-ui-repair/src/specialists/live_enumerator.rs",
        "bus",
        44,
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
        "crates/nexus-verifier-sandbox/src/landlock_rules.rs",
        "syscall",
        1,
    ),
    (
        "crates/nexus-verifier-sandbox/src/launcher.rs",
        "process",
        1,
    ),
    (
        "crates/nexus-verifier-sandbox/src/scope/manager.rs",
        "bus",
        55,
    ),
    ("crates/nexus-verifier-sandbox/src/sys.rs", "network", 4),
    ("crates/nexus-verifier-sandbox/src/sys.rs", "process", 1),
    ("crates/nexus-verifier-sandbox/src/sys.rs", "syscall", 6),
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
    ("integrations/src/router.rs", "latent", 9),
    ("kernel/src/actuators/api.rs", "process", 1),
    ("kernel/src/actuators/browser.rs", "process", 1),
    ("kernel/src/actuators/code_exec.rs", "process", 1),
    ("kernel/src/actuators/docker.rs", "process", 1),
    ("kernel/src/actuators/execution_platform.rs", "process", 3),
    ("kernel/src/actuators/image_gen.rs", "process", 3),
    ("kernel/src/actuators/shell.rs", "process", 1),
    ("kernel/src/actuators/tts.rs", "process", 3),
    ("kernel/src/actuators/web.rs", "process", 2),
    ("kernel/src/coding_run/local_model.rs", "network", 6),
    ("kernel/src/computer_control.rs", "process", 5),
    ("kernel/src/hardware.rs", "process", 5),
    ("kernel/src/protocols/a2a_client.rs", "process", 2),
    ("kernel/src/resource_limiter.rs", "process", 3),
    ("kernel/src/resource_limiter.rs", "sealed", 1),
    ("kernel/src/resource_limiter/unix.rs", "ends", 1),
    ("kernel/src/resource_limiter/unix.rs", "process", 7),
    ("kernel/src/resource_limiter/unix.rs", "sealed", 1),
    ("kernel/src/resource_limiter/windows.rs", "ends", 1),
    ("kernel/src/resource_limiter/windows.rs", "process", 11),
    ("kernel/src/resource_limiter/windows.rs", "sealed", 1),
    ("kernel/src/secrets/backend_keyring.rs", "vault", 4),
    ("kernel/src/typed_tools.rs", "process", 1),
    ("llama-bridge/src/model.rs", "process", 1),
    ("nexus-code/src/app.rs", "latent", 2),
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
    ("nexus-code/src/mcp/mod.rs", "latent", 3),
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
    ("protocols/src/metrics.rs", "network", 2),
    ("protocols/src/server_runtime.rs", "network", 1),
    ("sdk/src/typed_tools.rs", "process", 13),
    ("sdk/src/wasmtime_host_functions.rs", "latent", 1),
    ("telemetry/src/nexus_metrics.rs", "network", 3),
];

/// The class of every file holding a mechanism, and why (no fifth class).
const EFFECT_FILES: &[(&str, Route, &str)] = &[
    (
        "agents/coder/src/context.rs",
        Route::Closed,
        "latent: the coder conductor is never constructed by the desktop (only the withdrawn CLI and tests)",
    ),
    (
        "agents/coder/src/fix_loop.rs",
        Route::Closed,
        "latent: the coder fix loop runs the coder's tests; only the conductor calls it (LATENT_ENTRIES)",
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
        "agents/coder/src/terminal.rs",
        Route::Closed,
        "latent: the coder terminal's unsealed sh -lc spawn; no production caller (LATENT_ENTRIES)",
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
        "agents/conductor/src/lib.rs",
        Route::Closed,
        "latent: the conductor runs the coder's scanner, context, tests and fix loop; only the withdrawn CLI builds one",
    ),
    (
        "agents/social-poster/src/lib.rs",
        Route::Closed,
        "latent: the real pipeline (web search and reader) runs only from the withdrawn CLI's manifest runner",
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
        "app/src-tauri/src/lib.rs",
        Route::Closed,
        "the integration router is built only from the default configuration, which names no provider (Phase Zero S13 pin)",
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
        "auth/src/lib.rs",
        Route::Closed,
        "re-exports the latent OidcClient; its callers are latent sites (LATENT_ENTRIES)",
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
        "cli/src/setup.rs",
        Route::NonProduction,
        "the withdrawn nexus-cli's setup; only the withdrawn nexus binary depends on nexus-cli",
    ),
    (
        "connectors/core/src/github_connector.rs",
        Route::Closed,
        "latent: wraps HttpConnector; GitHubConnector has no production caller (LATENT_ENTRIES)",
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
        "crates/nexus-governed-control/src/egress/destination.rs",
        Route::Phase3,
        "governed egress: names resolved once, every address checked by the address policy",
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
        "crates/nexus-memory/src/lib.rs",
        Route::Closed,
        "re-exports the latent OllamaEmbedder; its callers are latent sites (LATENT_ENTRIES)",
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
        "crates/nexus-ui-repair/src/bin/sg5_probe.rs",
        Route::NonProduction,
        "developer QA probe binary (decision M): AT-SPI over D-Bus; not shipped, not in the desktop's dependency closure",
    ),
    (
        "crates/nexus-ui-repair/src/governance/xvfb_session.rs",
        Route::NonProduction,
        "developer QA scout (decision M): not in the desktop's dependency closure",
    ),
    (
        "crates/nexus-ui-repair/src/specialists/live_enumerator.rs",
        Route::NonProduction,
        "developer QA scout (decision M): AT-SPI over D-Bus; not in the desktop's dependency closure",
    ),
    (
        "crates/nexus-ui-repair/src/specialists/vision_judge.rs",
        Route::NonProduction,
        "developer QA scout (decision M): not in the desktop's dependency closure",
    ),
    (
        "crates/nexus-verifier-sandbox/src/landlock_rules.rs",
        Route::Governed,
        "Phase Two verifier: the sandbox's Landlock ruleset system calls (no launch, no connection)",
    ),
    (
        "crates/nexus-verifier-sandbox/src/launcher.rs",
        Route::Governed,
        "Phase Two verifier: native launch approval, verified packaged helper and toolchain",
    ),
    (
        "crates/nexus-verifier-sandbox/src/scope/manager.rs",
        Route::Governed,
        "Phase Two verifier: the owner's systemd user manager over D-Bus, only for the verifier's own cgroup scope",
    ),
    (
        "crates/nexus-verifier-sandbox/src/sys.rs",
        Route::Governed,
        "Phase Two verifier sandbox: the helper's fork and execveat, close_range, Landlock and a seqpacket control socket, after native launch approval",
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
        "integrations/src/router.rs",
        Route::Closed,
        "builds only the providers its configuration names; the desktop's router names none (Phase Zero S13 pin)",
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
        "kernel/src/actuators/execution_platform.rs",
        Route::Closed,
        "the Windows backend of ShellCommand and CodeExecute, which stay closed; Windows makes no Phase Three claim",
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
        "WebSearch from fixed initial hosts, following https redirects to any host (Phase Zero class A, P0-002C5B, for the Architect); WebFetch: Phase Three or closed",
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
        "the kernel spawn specs: sealed (Builder lifecycle, Phase Three tools) and unsealed (callers closed or latent only)",
    ),
    (
        "kernel/src/resource_limiter/unix.rs",
        Route::Governed,
        "sealed spawn (absolute program, cleared env, own group, limits, owner killpg); unsealed sh -lc only for closed or latent callers",
    ),
    (
        "kernel/src/resource_limiter/windows.rs",
        Route::Governed,
        "the Windows spawn substrate (kill-on-close job), sealed and unsealed; Windows makes no Phase Three claim",
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
        "nexus-code/src/app.rs",
        Route::Closed,
        "App holds an unconnected McpManager; nothing calls connect or connect_all; the desktop only lists tools (p3_g6_09)",
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
        "nexus-code/src/mcp/mod.rs",
        Route::Closed,
        "McpManager::connect spawns or dials a configured MCP server; nothing calls connect or connect_all (LATENT_METHODS)",
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
        "protocols/src/metrics.rs",
        Route::Closed,
        "the Prometheus exporter only as an in-process recorder (install_recorder): its HTTP listener and push gateway are never started",
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
    (
        "sdk/src/wasmtime_host_functions.rs",
        Route::Closed,
        "nexus_exec_tool only builds a typed command to validate it and returns it as data; nothing spawns it (pinned)",
    ),
    (
        "telemetry/src/nexus_metrics.rs",
        Route::Closed,
        "the Prometheus exporter only as an in-process recorder (install_recorder): its HTTP listener and push gateway are never started",
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
    assert_eq!(production_modules_named_like_tests(), NAME_SKIPPED_MODULES);
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
            "egress/destination.rs",
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

/// The production files that declare foreign functions (`extern` blocks).
const FFI_FILES: [&str; 3] = [
    "app/src-tauri/src/commands/flash.rs",
    "kernel/src/resource_limiter/darwin_group.rs",
    "llama-bridge/src/ffi.rs",
];

/// The foreign functions each of those files declares, exactly.
const FFI_DECLARED: [(&str, &[&str]); 3] = [
    ("app/src-tauri/src/commands/flash.rs", &["malloc_trim"]),
    (
        "kernel/src/resource_limiter/darwin_group.rs",
        &[
            "nexus_darwin_group_pids",
            "nexus_darwin_proc_decode",
            "nexus_darwin_proc_record_size",
        ],
    ),
    (
        "llama-bridge/src/ffi.rs",
        &[
            "llama_backend_free",
            "llama_backend_init",
            "llama_batch_free",
            "llama_batch_init",
            "llama_chat_apply_template",
            "llama_decode",
            "llama_free",
            "llama_get_memory",
            "llama_memory_clear",
            "llama_model_chat_template",
            "llama_model_free",
            "llama_model_get_vocab",
            "llama_model_meta_val_str",
            "llama_model_n_ctx_train",
            "llama_model_n_params",
            "llama_model_size",
            "llama_n_ctx",
            "llama_perf_context",
            "llama_perf_context_reset",
            "llama_sampler_chain_add",
            "llama_sampler_chain_default_params",
            "llama_sampler_chain_init",
            "llama_sampler_free",
            "llama_sampler_init_dist",
            "llama_sampler_init_greedy",
            "llama_sampler_init_min_p",
            "llama_sampler_init_penalties",
            "llama_sampler_init_temp",
            "llama_sampler_init_top_k",
            "llama_sampler_init_top_p",
            "llama_sampler_reset",
            "llama_sampler_sample",
            "llama_token_bos",
            "llama_token_eos",
            "llama_token_to_piece",
            "llama_tokenize",
            "llama_vocab_is_eog",
            "llama_vocab_n_tokens",
            "malloc_trim",
            "nexus_ctx_params_create",
            "nexus_ctx_params_free",
            "nexus_ctx_params_set_flash_attn",
            "nexus_ctx_params_set_n_batch",
            "nexus_ctx_params_set_n_ctx",
            "nexus_ctx_params_set_n_threads",
            "nexus_ctx_params_set_n_threads_batch",
            "nexus_ctx_params_set_n_ubatch",
            "nexus_ctx_params_set_no_perf",
            "nexus_ctx_params_set_type_k",
            "nexus_ctx_params_set_type_v",
            "nexus_init_from_model",
            "nexus_model_load_from_file",
            "nexus_model_params_create",
            "nexus_model_params_free",
            "nexus_model_params_set_n_gpu_layers",
            "nexus_model_params_set_use_mlock",
            "nexus_model_params_set_use_mmap",
            "nexus_sizeof_context_params",
            "nexus_sizeof_model_params",
        ],
    ),
];

/// C names of process, network, signal, raw-syscall and loader functions: a
/// local `extern` declaration of one would reach the system without any path
/// the mechanism predicates resolve.
const FFI_DENIED: &[&str] = &[
    "fork",
    "vfork",
    "clone",
    "clone3",
    "execv",
    "execve",
    "execvp",
    "execvpe",
    "execveat",
    "fexecve",
    "execl",
    "execle",
    "execlp",
    "posix_spawn",
    "posix_spawnp",
    "system",
    "popen",
    "socket",
    "socketpair",
    "connect",
    "bind",
    "listen",
    "accept",
    "accept4",
    "sendto",
    "sendmsg",
    "kill",
    "killpg",
    "tgkill",
    "tkill",
    "pidfd_send_signal",
    "ptrace",
    "syscall",
    "dlopen",
    "dlmopen",
    "dlsym",
    "dlvsym",
    "recvfrom",
    "recvmsg",
    "getaddrinfo",
    "getaddrinfo_a",
    "gai_suspend",
    "gai_cancel",
    "gai_error",
    "getnameinfo",
    "gethostbyname",
    "gethostbyname2",
    "gethostbyname_r",
    "gethostbyname2_r",
    "gethostbyaddr",
    "gethostbyaddr_r",
    "res_init",
    "res_ninit",
    "res_query",
    "res_nquery",
    "res_search",
    "res_nsearch",
    "res_send",
    "res_nsend",
    "res_querydomain",
    "res_nquerydomain",
    "res_mkquery",
    "res_nmkquery",
    "__res_init",
    "__res_ninit",
    "__res_query",
    "__res_nquery",
    "__res_search",
    "__res_nsearch",
    "__res_send",
    "__res_nsend",
    "__res_querydomain",
    "__res_mkquery",
    "sigqueue",
    "CreateProcessW",
    "CreateProcessA",
    "CreateProcessAsUserW",
    "CreateProcessAsUserA",
    "CreateProcessWithLogonW",
    "CreateProcessWithTokenW",
    "ShellExecuteW",
    "ShellExecuteA",
    "ShellExecuteExW",
    "ShellExecuteExA",
    "WinExec",
    "TerminateProcess",
    "OpenProcess",
    "LoadLibraryW",
    "LoadLibraryA",
    "LoadLibraryExW",
    "LoadLibraryExA",
    "GetProcAddress",
];

/// §22 and §34 (alternate imports): a foreign declaration escapes the
/// structural resolver, so the files allowed to declare one are pinned and
/// none may declare a process, network, signal, raw-syscall or loader
/// function.
#[test]
fn p3_g6_10_no_foreign_declaration_escapes_the_mechanism_predicates() {
    let sources = workspace_sources()
        .iter()
        .map(|(file, text)| (file.clone(), text.clone()))
        .chain(
            NAME_SKIPPED_MODULES
                .iter()
                .map(|file| (file.to_string(), production_text(&workspace_file(file)))),
        );
    let mut declaring = BTreeSet::new();
    let mut declared: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (file, text) in sources {
        // Literals are blanked, so neither an ABI string's content nor a
        // literal that spells `extern "C" {` can mislead the scan.
        let m = masked(&text);
        let flat = String::from_utf8(m.clone()).expect("masking keeps UTF-8");
        // `#[link_name]` binds a declaration to any symbol: no name below
        // would show what it calls.
        assert!(words(&m, "link_name").is_empty(), "{file}: link_name");
        for at in words(&m, "extern") {
            // `extern {` or `extern "ABI" {`; `extern crate`, `extern "C" fn`
            // items and types are not blocks.
            let mut open = skip_ws(&m, at + "extern".len());
            if m.get(open) == Some(&b'"') {
                let abi_end = open + 1 + flat[open + 1..].find('"').expect("a closed ABI string");
                open = skip_ws(&m, abi_end + 1);
            }
            if m.get(open) != Some(&b'{') {
                continue;
            }
            let close = close_of(&m, open);
            declaring.insert(file.clone());
            let block = &flat[open..close];
            for index in words(block.as_bytes(), "fn") {
                let name = ident_at(block.as_bytes(), skip_ws(block.as_bytes(), index + 2));
                assert!(
                    !FFI_DENIED.contains(&name),
                    "{file}: a foreign declaration of {name}"
                );
                declared
                    .entry(file.clone())
                    .or_default()
                    .insert(name.to_string());
            }
        }
    }
    let pinned: BTreeSet<String> = FFI_FILES.iter().map(|file| file.to_string()).collect();
    assert_eq!(declaring, pinned);
    // Every foreign function they declare is pinned by name: a new one fails
    // here whatever it is (FFI_DENIED names what one may never be).
    let pinned: BTreeMap<String, BTreeSet<String>> = FFI_DECLARED
        .iter()
        .map(|(file, names)| {
            (
                file.to_string(),
                names.iter().map(|name| name.to_string()).collect(),
            )
        })
        .collect();
    assert_eq!(declared, pinned);
    // The scan itself: an ABI-less block, odd spacing, and a test-named
    // module are all read.
    let fixture = masked("extern {\n    fn  execve(a: i32);\n}\n");
    let at = words(&fixture, "extern")[0];
    let open = skip_ws(&fixture, at + "extern".len());
    assert_eq!(fixture[open], b'{');
    assert!(NAME_SKIPPED_MODULES.contains(&"kernel/src/autopilot/stress_test.rs"));
}

/// The owner's stops reach Phase Three: the global emergency key stops it
/// with the legacy engines, and stopping an agent cancels its runs (what
/// they still run sees it; nothing they left waiting can be approved).
#[test]
fn p3_g6_11_the_owners_stops_reach_phase_three() {
    let lib = production_source("app/src-tauri/src/lib.rs");
    let stop = lib.find("activate_emergency_kill_switch();").unwrap();
    let upto = lib[stop..].find("log_event(").unwrap();
    let handler = without_whitespace(&lib[stop..stop + upto]);
    assert!(
        handler.contains("ifletOk(world)=state.real_world(){world.emergency_stop();}"),
        "{handler}"
    );
    // Every owner route that stops agents (Stop, the Admin "Stop all", bulk
    // Stop) is one routine, `stop_agents`: every agent stops at once (its
    // schedule under every spelling, its loop's cancel flag, Phase Three,
    // the supervisor) before anything waits; then a thread of its own
    // removes the loops, which may wait while any agent's cycle holds the
    // loop lock. Ending a goal cancels in Phase Three at once and removes
    // the loop the same way. Quitting closes the run registry, cancels every
    // run, waits (bounded) until nothing executes, and stops the display.
    // The routines are pinned whole.
    let cognitive = production_source("app/src-tauri/src/commands/cognitive.rs");
    for (file, path, body) in STOP_ROUTINES {
        let text = production_source(file);
        assert_eq!(
            compact(one_fn(text, path).body_text(text)),
            body,
            "{file} {path}"
        );
    }
    let enterprise = production_source("app/src-tauri/src/commands/enterprise.rs");
    // The agents a stop applies to are chosen by the supervisor's state.
    let stoppable = compact(one_fn(enterprise, "stoppable_agents").body_text(enterprise));
    assert!(
        stoppable.contains(
            "matches!(status.state,AgentState::Running|AgentState::Paused|AgentState::Starting)"
        ),
        "{stoppable}"
    );
    // The stop commands run on the IPC thread, at once: nothing they do
    // waits (no runtime worker is needed). The commands that wait on the
    // loops' lock run off it (`p3_g10_01`).
    for command in [
        "stop_agent",
        "stop_agent_goal",
        "admin_agent_stop_all",
        "admin_agent_bulk_update",
    ] {
        assert_eq!(one_fn(lib, command).head, "#[command]", "{command}");
    }
    // Every mention of the names that stop, pause, restart or unschedule an
    // agent, or wait for its loop, in any form (a call, a path, an import,
    // an alias, a function pointer), pinned: the routines above, Pause, and
    // the two that act before any loop exists (restoring agents at startup,
    // registering prebuilt ones); `run` registers the commands.
    let at = |file: &'static str, function: &str| (file, function.to_string());
    let agents_rs = "app/src-tauri/src/commands/agents.rs";
    let cognitive_rs = "app/src-tauri/src/commands/cognitive.rs";
    let enterprise_rs = "app/src-tauri/src/commands/enterprise.rs";
    let lib_rs = "app/src-tauri/src/lib.rs";
    for (name, expected) in [
        (
            "stop_agent",
            vec![
                at(agents_rs, "restore_persisted_agents"),
                at(agents_rs, "stop_agent"),
                at(agents_rs, "stop_agent_now"),
                at(
                    "app/src-tauri/src/commands/chat_llm.rs",
                    "AppState::load_prebuilt_agents",
                ),
                at(lib_rs, "run"),
            ],
        ),
        (
            "pause_agent",
            vec![
                at(agents_rs, "pause_agent"),
                at(agents_rs, "restore_persisted_agents"),
                at(lib_rs, "pause_agent"),
                at(lib_rs, "run"),
            ],
        ),
        (
            "stop_agent_loop",
            vec![
                at(cognitive_rs, "end_agent_loop"),
                at(cognitive_rs, "end_goal_loop"),
            ],
        ),
        (
            "stop_agents",
            vec![
                at(enterprise_rs, "admin_agent_bulk_update"),
                at(enterprise_rs, "admin_agent_stop_all"),
                at(lib_rs, "stop_agent"),
            ],
        ),
        ("stop_agent_now", vec![at(agents_rs, "stop_agents")]),
        (
            "end_agent_loop",
            vec![at(agents_rs, "stop_agents"), at(lib_rs, "stop_agent_goal")],
        ),
        (
            "end_goal_loop",
            vec![
                at(cognitive_rs, "ScheduledGoalExecutor::execute"),
                at(cognitive_rs, "execute_hivemind_subtask"),
            ],
        ),
        (
            "agent_stopped",
            vec![
                at(cognitive_rs, "execute_hivemind_subtask"),
                at(cognitive_rs, "execute_hivemind_subtask"),
                at(cognitive_rs, "spawn_cognitive_loop_with_bridge"),
            ],
        ),
        (
            "restart_agent",
            vec![
                at(agents_rs, "start_agent"),
                at(cognitive_rs, "ScheduledGoalExecutor::execute"),
            ],
        ),
        (
            "unregister_agent",
            vec![
                at(agents_rs, "stop_agent"),
                at(agents_rs, "stop_agent_now"),
                at(lib_rs, "stop_autonomous_loop"),
            ],
        ),
        ("stop_agent_goal", vec![at(lib_rs, "run")]),
        (
            "spellings",
            vec![
                at(agents_rs, "stop_agent_now"),
                at(agents_rs, "stop_agents"),
                at(lib_rs, "stop_agent_goal"),
            ],
        ),
        (
            "was_stopped",
            vec![at(cognitive_rs, "ScheduledGoalExecutor::execute")],
        ),
    ] {
        assert_eq!(mentions(DESKTOP_SRC, name), expected, "{name}");
    }
    // The waits happen only on the threads the routines spawn.
    for (file, path) in [
        ("app/src-tauri/src/commands/agents.rs", "stop_agents"),
        ("app/src-tauri/src/lib.rs", "stop_agent_goal"),
    ] {
        let text = production_source(file);
        let body = compact(one_fn(text, path).body_text(text));
        let spawn = body.find(".spawn(move||{").expect("a thread");
        assert!(!body[..spawn].contains("end_agent_loop("), "{path}");
        assert!(body[spawn..].contains("end_agent_loop("), "{path}");
    }
    // A scheduled tick that saw its schedule as it began, and finds it
    // removed (an owner's stop removes it first), does not bring the agent
    // back: checked under the supervisor's lock before any restart.
    let tick = compact(one_fn(cognitive, "ScheduledGoalExecutor::execute").body_text(cognitive));
    assert_in_order(
        &tick,
        &[
            "letwas_scheduled=scheduled();",
            "ifis_transcendent_agent(&self.state,agent_id){",
            "letowner_stopped=self.state.real_world().is_ok_and(|world|world.was_stopped(agent_id));if!scheduled()&&(was_scheduled||owner_stopped){returnErr(UNSCHEDULED.to_string());}supervisor.restart_agent(agent_uuid)",
            "letstopped=supervisor.get_agent(agent_uuid).is_none_or(|handle|{matches!(handle.state,AgentState::Stopping|AgentState::Stopped|AgentState::Destroyed)});letrefusal=ifstopped{Some(STOPPED)}elseifwas_scheduled&&!scheduled(){Some(UNSCHEDULED)}else{None};ifletSome(refusal)=refusal{drop(supervisor);end_goal_loop(&self.state,agent_id,&goal_id);persist_task_completion(&self.state,agent_id,&goal_id,\"failed\",refusal,false,0.0);returnErr(refusal.to_string());}",
            "spawn_cognitive_loop_with_bridge(",
            "drop(supervisor);Ok(())",
        ],
        "ScheduledGoalExecutor::execute",
    );
    assert_eq!(tick.matches("restart_agent(").count(), 1, "{tick}");
    // A stopped agent's loop runs no further cycle (its cancel flag may not
    // exist yet when the owner's stop comes), and a HiveMind session gives
    // it no sub-task; a session's own time limit ends only its goal.
    let driver =
        compact(one_fn(cognitive, "spawn_cognitive_loop_with_bridge").body_text(cognitive));
    assert_in_order(
        &driver,
        &[
            "'cycle_loop:for_cyclein0..max_cycles{ifagent_stopped(&state,&agent_id){",
            "\"Goal ended: the agent is stopped.\",false,0.0);return;}",
            "ifcancel_flag.load(Ordering::Relaxed){",
        ],
        "spawn_cognitive_loop_with_bridge",
    );
    let subtask = compact(one_fn(cognitive, "execute_hivemind_subtask").body_text(cognitive));
    assert!(
        subtask.starts_with("ifagent_stopped(state,agent_id){returnErr("),
        "{subtask}"
    );
    assert_in_order(
        &subtask,
        &[
            "ifstarted.elapsed()>=timeout{end_goal_loop(state,agent_id,&goal_id);returnErr(",
            "ifagent_stopped(state,agent_id){returnErr(",
        ],
        "execute_hivemind_subtask",
    );
    assert!(!subtask.contains("cancel_agent("), "{subtask}");
    // An agent that is not running, however it was stopped or paused, acts
    // no more: the one production bridge asks the supervisor, and `act`
    // refuses before anything else, then refuses a stop that came after
    // the loop began, under the lock that tracks the run it opens.
    let executor = compact(one_fn(cognitive, "phase0_agent_executor").body_text(cognitive));
    assert_eq!(
        executor.matches("AgentBridge::new(").count(),
        1,
        "{executor}"
    );
    assert!(
        executor.contains(".get_agent(id).is_some_and(|handle|handle.state==AgentState::Running)"),
        "{executor}"
    );
    let bridges: Vec<(&str, usize)> = desktop_sources()
        .into_iter()
        .map(|(file, text)| {
            let text = compact(text);
            (
                file,
                text.matches("AgentBridge::new(").count()
                    + text.matches("AgentBridge::for_tests(").count(),
            )
        })
        .filter(|(_, count)| *count > 0)
        .collect();
    assert_eq!(bridges, [("commands/cognitive.rs", 1)]);
    let world = production_source("app/src-tauri/src/governed_real_world.rs");
    let act = compact(one_fn(world, "AgentBridge::act").body_text(world));
    // One spelling per agent: its runs are kept, and cancelled, under the
    // canonical one.
    assert!(
        act.starts_with("letagent_id=&canonical_agent_id(agent_id);"),
        "{act}"
    );
    assert_in_order(
        &act,
        &[
            "if!(self.running)(agent_id){returnErr(",
            "letrun=matchself.loop_run(agent_id,&agent)?{",
            "None=>self.open_loop_run(agent_id,&agent)?}",
            ".agent_action(&agent,run,intent)",
        ],
        "AgentBridge::act",
    );
    let stopped = compact(one_fn(world, "AgentBridge::stopped_since_began").body_text(world));
    assert_eq!(
        stopped,
        "agents.stopped_at.get(agent_id).is_some_and(|stopped|*stopped>self.began)"
    );
    let loop_run = compact(one_fn(world, "AgentBridge::loop_run").body_text(world));
    assert_in_order(
        &loop_run,
        &[
            "letcurrent=self.run.lock()",
            "ifself.stopped_since_began(&self.world.agents(),agent_id){returnErr(",
            "Some((owner,run))ifowner==agent=>Ok(Some(*run)),",
        ],
        "AgentBridge::loop_run",
    );
    // The run is opened (and recorded) with no lock held, then kept under
    // the lock a stop takes, checked again there; one not kept ends.
    let open = compact(one_fn(world, "AgentBridge::open_loop_run").body_text(world));
    assert_in_order(
        &open,
        &[
            ".open_run(agent.clone(),RunOrigin::AgentGoal)",
            "letmutcurrent=self.run.lock()",
            "letmutagents=self.world.agents();",
            "ifself.stopped_since_began(&agents,agent_id){Kept::Stopped}",
            "agents.runs.entry(agent_id.to_string()).or_default().push(opened);",
            "Kept::Stopped=>{let_=self.world.control.cancel_run(opened);",
        ],
        "AgentBridge::open_loop_run",
    );
    assert!(
        !open[..open.find(".open_run(").unwrap()].contains(".lock()"),
        "{open}"
    );
    let cancel = compact(one_fn(world, "RealWorld::cancel_agent").body_text(world));
    assert_in_order(
        &cancel,
        &[
            "agents.epoch+=1;",
            "agents.stopped_at.insert(agent.to_string(),epoch);",
            "agents.runs.remove(agent)",
            "self.control.cancel_run(run);",
        ],
        "cancel_agent",
    );
    // Quitting cancels every open run, waits (within a bound) until
    // nothing executes, and stops the agent display.
    let exit = &lib[lib.find("RunEvent::Exit").unwrap() + "RunEvent::Exit".len()..];
    let arm = without_whitespace(&exit[..exit.find("RunEvent::").unwrap_or(exit.len())]);
    assert!(
        arm.contains("ifletOk(world)=app.state::<AppState>().real_world(){world.shutdown();}"),
        "{arm}"
    );
    let shutdown = compact(one_fn(world, "RealWorld::shutdown").body_text(world));
    assert_in_order(
        &shutdown,
        &[
            "self.control.shut_down();",
            ".any(|view|view.state==CommitmentState::Executing)",
            "letdeadline=Instant::now()+SHUTDOWN_WAIT;",
            "whileexecuting()&&Instant::now()<deadline{",
            "self.control.stop_display();",
        ],
        "RealWorld::shutdown",
    );
    assert!(world
        .contains("const SHUTDOWN_WAIT: std::time::Duration = std::time::Duration::from_secs(5);"));
}

/// The owner's stop routines and quitting, pinned whole (normalized text):
/// see `p3_g6_11`.
const STOP_ROUTINES: [(&str, &str, &str); 17] = [
    (
        "app/src-tauri/src/commands/cognitive.rs",
        "agent_stopped",
        "Uuid::parse_str(agent_id).is_ok_and(|id|{state.supervisor.lock().unwrap_or_else(|p|p.into_inner()).get_agent(id).is_some_and(|handle|{matches!(handle.state,AgentState::Stopping|AgentState::Stopped|AgentState::Destroyed)})})",
    ),
    (
        "app/src-tauri/src/commands/cognitive.rs",
        "end_goal_loop",
        "letours=state.cognitive_runtime.get_agent_status_fast(agent_id).and_then(|status|status.active_goal).is_some_and(|goal|goal.id==goal_id);ifours&&state.cognitive_runtime.stop_agent_loop(agent_id).is_ok(){state.wake_and_clear_blocked_consent_wait(agent_id);}",
    ),
    (
        "app/src-tauri/src/commands/cognitive.rs",
        "end_agent_loop",
        "state.cognitive_runtime.stop_agent_loop(agent_id).map_err(|e|e.to_string())?;state.wake_and_clear_blocked_consent_wait(agent_id);state.log_event(Uuid::parse_str(agent_id).unwrap_or_default(),EventType::UserAction,json!({\"action\":\"stop_agent_goal\",\"agent_id\":agent_id}));Ok(())",
    ),
    (
        "app/src-tauri/src/commands/agents.rs",
        "stop_agent_now",
        "forspellinginspellings(state,agent_id){state.agent_scheduler.unregister_agent(&spelling);ifletSome(flag)=state.cognitive_cancellations.lock().unwrap_or_else(|p|p.into_inner()).get(&spelling){flag.store(true,Ordering::Relaxed);}}ifletOk(world)=state.real_world(){world.cancel_agent(agent_id);}stop_agent(state,agent_id.to_string())",
    ),
    (
        "app/src-tauri/src/commands/agents.rs",
        "stop_agents",
        "letagents:Vec<String>=agents.iter().map(|id|canonical_agent_id(id)).collect();ifletOk(world)=state.real_world(){foragentin&agents{world.cancel_agent(agent);}}letloops:Vec<(String,Option<Arc<AtomicBool>>)>=agents.iter().flat_map(|agent|spellings(state,agent)).map(|spelling|{letflag=state.cognitive_cancellations.lock().unwrap_or_else(|p|p.into_inner()).get(&spelling).cloned();(spelling,flag)}).collect();letresults=agents.iter().map(|agent|stop_agent_now(state,agent)).collect();letremover=state.clone();letspawned=Builder::new().name(\"nexus-agent-stop\".into()).spawn(move||{for(agent,flag)in&loops{letnow=remover.cognitive_cancellations.lock().unwrap_or_else(|p|p.into_inner()).get(agent).cloned();letreplaced=match(flag,&now){(Some(then),Some(now))=>!Arc::ptr_eq(then,now),(None,Some(_))=>true,_=>false};if!replaced{let_=end_agent_loop(&remover,agent);}}});ifspawned.is_err(){foragentin&agents{state.log_event(Uuid::parse_str(agent).unwrap_or_default(),EventType::StateChange,json!({\"event\":\"stop_agent\",\"loop_removal\":\"not started\"}));}}results",
    ),
    (
        "app/src-tauri/src/commands/agents.rs",
        "spellings",
        "letuuid=Uuid::parse_str(agent_id).ok();letsame=|key:&str|key==agent_id||(uuid.is_some()&&Uuid::parse_str(key).ok()==uuid);letmutfound=vec![canonical_agent_id(agent_id)];letscheduled=state.agent_scheduler.list().into_iter().map(|s|s.agent_id);letlooping:Vec<String>=state.cognitive_cancellations.lock().unwrap_or_else(|p|p.into_inner()).keys().cloned().collect();forkeyinscheduled.chain(looping){ifsame(&key)&&!found.contains(&key){found.push(key);}}found",
    ),
    (
        "app/src-tauri/src/commands/agents.rs",
        "canonical_agent_id",
        "Uuid::parse_str(agent_id).map_or_else(|_|agent_id.to_string(),|id|id.to_string())",
    ),
    (
        "app/src-tauri/src/lib.rs",
        "stop_agent",
        "letagent_id=canonical_agent_id(&agent_id);letmutresults=stop_agents(state.inner(),from_ref(&agent_id));results.pop().unwrap_or(Ok(()))?;emit_agent_status(&window,state.inner(),&agent_id);Ok(())",
    ),
    (
        "app/src-tauri/src/lib.rs",
        "stop_agent_goal",
        "letagent_id=canonical_agent_id(&agent_id);ifletOk(world)=state.real_world(){world.cancel_agent(&agent_id);}letloops=spellings(state.inner(),&agent_id);letremover=state.inner().clone();letremoving=loops.clone();letspawned=Builder::new().name(\"nexus-goal-stop\".into()).spawn(move||{forspellingin&removing{let_=end_agent_loop(&remover,spelling);}});ifspawned.is_err(){forspellingin&loops{ifletSome(flag)=state.cognitive_cancellations.lock().unwrap_or_else(|p|p.into_inner()).get(spelling){flag.store(true,Ordering::Relaxed);}}state.log_event(Uuid::parse_str(&agent_id).unwrap_or_default(),EventType::StateChange,json!({\"event\":\"stop_agent_goal\",\"loop_removal\":\"not started\"}));}Ok(())",
    ),
    (
        "app/src-tauri/src/commands/enterprise.rs",
        "admin_agent_stop_all",
        "letmutstopped=0u32;let_=workspace_id;letagents=stoppable_agents(state);forresultinstop_agents(state,&agents){ifresult.is_ok(){stopped+=1;}}letmutaudit=state.audit.lock().unwrap_or_else(|p|p.into_inner());let_=audit.append_event(SYSTEM_UUID,EventType::UserAction,json!({\"action\":\"admin_agent_stop_all\",\"stopped\":stopped}));Ok(stopped)",
    ),
    (
        "app/src-tauri/src/commands/enterprise.rs",
        "admin_agent_bulk_update",
        "letcount=agent_dids.len();letaction=from_str::<Value>(&update).ok().and_then(|value|value[\"action\"].as_str().map(to_string)).unwrap_or_default();letmutsucceeded=0usize;ifaction==\"stop\"{letstoppable=stoppable_agents(state);letnamed:Vec<String>=agent_dids.iter().map(|did|{canonical_agent_id(did.strip_prefix(\"did:nexus:\").unwrap_or(did))}).filter(|agent|stoppable.iter().any(|known|known==agent)).collect();succeeded=stop_agents(state,&named).into_iter().filter(Result::is_ok).count();}letmutaudit=state.audit.lock().unwrap_or_else(|p|p.into_inner());let_=audit.append_event(SYSTEM_UUID,EventType::UserAction,json!({\"action\":\"admin_agent_bulk_update\",\"count\":count,\"update\":update,\"succeeded\":succeeded}));letresult=json!({\"succeeded\":succeeded,\"failed\":count-succeeded});to_string(&result).map_err(|e|format!(\"serialize: {e}\"))",
    ),
    (
        "app/src-tauri/src/governed_real_world.rs",
        "RealWorld::shutdown",
        "letauthority=self.control.authority();self.control.shut_down();letexecuting=||{self.control.is_starting_display()||authority.runs().views().iter().any(|run|{authority.commitments().views_of_run(run.id).iter().any(|view|view.state==CommitmentState::Executing)})};letdeadline=Instant::now()+SHUTDOWN_WAIT;whileexecuting()&&Instant::now()<deadline{sleep(Duration::from_millis(25));}self.control.stop_display();",
    ),
    (
        "app/src-tauri/src/governed_real_world.rs",
        "RealWorld::cancel_agent",
        "letagent=&canonical_agent_id(agent);letruns={letmutagents=self.agents();agents.epoch+=1;letepoch=agents.epoch;agents.stopped_at.insert(agent.to_string(),epoch);agents.runs.remove(agent).unwrap_or_default()};forruninruns{let_=self.control.cancel_run(run);}",
    ),
    (
        "crates/nexus-governed-control/src/governed.rs",
        "GovernedControl::stop_display",
        "ifletSome(status)=self.display.stop(){let_=self.authority().record_display(EvidencePhase::DisplayStopped,vec![(\"display\".into(),status.number.to_string())]);}",
    ),
    (
        "crates/nexus-governed-control/src/governed.rs",
        "GovernedControl::emergency_stop",
        "letcancelled=self.control.emergency_stop();self.stop_display();cancelled",
    ),
    (
        "crates/nexus-governed-control/src/governed.rs",
        "GovernedControl::shut_down",
        "letruns=self.authority().runs();runs.close();runs.views().into_iter().filter(|run|!run.cancelled&&!run.finished).filter(|run|self.cancel_run(run.id).is_ok()).count()",
    ),
    (
        "app/src-tauri/src/commands/agents.rs",
        "stop_agent",
        "letparsed=parse_agent_id(agent_id.as_str())?;state.agent_scheduler.unregister_agent(&agent_id);letmutsupervisor=matchstate.supervisor.lock(){Ok(guard)=>guard,Err(poisoned)=>poisoned.into_inner()};supervisor.stop_agent(parsed).map_err(agent_error)?;drop(supervisor);let_=state.db.update_agent_state(&agent_id,\"stopped\");persist_agent_fuel_ledger(state,&agent_id);update_last_action(state,parsed,\"stopped\");state.log_event(parsed,EventType::StateChange,json!({\"event\":\"stop_agent\",\"status\":\"ok\"}));Ok(())",
    ),
];

/// §22 (audit P8, P9): the inventory fails closed. A module or binding named
/// like a mechanism crate, even one declared in a test module, hides nothing;
/// the kernel's unsealed spawn is a process; a latent API's caller is a site
/// wherever it is and however it names the API (alias, turbofish, crate-root
/// re-export, method); and every latent class names its entries.
#[test]
fn p3_g6_12_the_inventory_fails_closed() {
    use crate::phase0_surface::rust_paths::Analysis;
    // Shadowing: the resolver alone lets a test module's names win.
    let shadowed = concat!(
        "fn fetch() {\n",
        "    let _ = reqwest::blocking::get(\"https://example.com\");\n",
        "    let _ = std::process::Command::new(\"true\");\n",
        "}\n",
        "#[cfg(test)]\n",
        "mod tests {\n",
        "    mod std {}\n",
        "    use super::fetch as reqwest;\n",
        "}\n",
    );
    let file = "app/src-tauri/src/fixture_shadowed.rs";
    let own = module_of(file);
    let module: Vec<&str> = own.iter().map(String::as_str).collect();
    let analysis = Analysis::new(shadowed, &module);
    for written in ["reqwest::blocking::get", "std::process::Command::new"] {
        let o = analysis
            .production()
            .find(|o| o.written.to_string() == written)
            .unwrap_or_else(|| panic!("{written}"));
        assert!(
            o.resolved.iter().all(|p| effect_kinds(p).is_empty()),
            "the resolver alone sees {written}: {:?}",
            o.resolved
        );
    }
    let sites = file_effect_sites(file, shadowed, &own);
    assert_eq!(
        sites,
        BTreeMap::from([("network", 1), ("process", 1)]),
        "shadowing hid a mechanism"
    );

    // The kernel's unsealed spawn, through the SDK's re-export.
    let unsealed = concat!(
        "use nexus_sdk::resource_limiter::{ResourceLimiter, ResourceProgram, ResourceSpawnSpec};\n",
        "pub fn run(spec: ResourceSpawnSpec) {\n",
        "    let _ = ResourceProgram::Shell(String::new());\n",
        "    let _ = ResourceLimiter::spawn(&ResourceLimiter::default(), &spec);\n",
        "}\n",
    );
    let file = "agents/coder/src/fixture_unsealed.rs";
    let sites = file_effect_sites(file, unsealed, &module_of(file));
    assert_eq!(sites.get("process"), Some(&3), "{sites:?}");

    // Latent callers, however they name the entry.
    let callers = concat!(
        "use nexus_auth::OidcClient as Client;\n",
        "use coder_agent::terminal;\n",
        "pub fn wire(config: nexus_auth::AuthConfig) {\n",
        "    let _ = Client::new(config.clone());\n",
        "    let _ = terminal::execute(\"true\", todo!());\n",
        "    let _ = nexus_conductor::Conductor::<()>::new(todo!());\n",
        "    let _ = config.resolve_client_secret();\n",
        "    let _ = nexus_auth::AuthConfig::resolve_client_secret(&config);\n",
        "}\n",
    );
    let file = "app/src-tauri/src/fixture_latent.rs";
    let sites = file_effect_sites(file, callers, &module_of(file));
    assert_eq!(sites.get("latent"), Some(&5), "{sites:?}");
    // Inside its crate a latent module's sibling is a caller; the module
    // itself is not.
    let inside = "pub fn f() { let _ = crate::terminal::execute(\"true\", todo!()); }";
    let sibling = "agents/coder/src/fixture_sibling.rs";
    assert_eq!(
        file_effect_sites(sibling, inside, &module_of(sibling)).get("latent"),
        Some(&1)
    );
    let itself = "agents/coder/src/terminal.rs";
    assert_eq!(
        file_effect_sites(itself, inside, &module_of(itself)).get("latent"),
        None
    );

    // Assembly, device nodes, and a kill beside a process listing.
    let other = concat!(
        "pub fn f(sys: &sysinfo::System) {\n",
        "    unsafe { core::arch::asm!(\"nop\") };\n",
        "    let _ = std::fs::File::open(\"/dev/uinput\");\n",
        "    for p in sys.processes().values() { p.kill(); }\n",
        "}\n",
    );
    let file = "app/src-tauri/src/fixture_other.rs";
    let sites = file_effect_sites(file, other, &module_of(file));
    for kind in ["asm", "device", "ends"] {
        assert_eq!(sites.get(kind), Some(&1), "{kind}: {sites:?}");
    }

    // Name resolution: a method, a trait path, libc and a DNS crate.
    let resolving = concat!(
        "use std::net::ToSocketAddrs;\n",
        "pub fn f() {\n",
        "    let _ = (\"example.com\", 443).to_socket_addrs();\n",
        "    let _ = ToSocketAddrs::to_socket_addrs(&(\"example.com\", 443));\n",
        "    let _ = unsafe { libc::getaddrinfo(todo!(), todo!(), todo!(), todo!()) };\n",
        "    let _ = hickory_resolver::Resolver::default();\n",
        "    let _ = str::to_socket_addrs(\"example.com:443\");\n",
        "    let _ = <(&str, u16)>::to_socket_addrs(&(\"example.com\", 443));\n",
        "    let _ = url::Url::parse(\"https://example.com\").unwrap().socket_addrs(|| None);\n",
        "    let _ = metrics_exporter_prometheus::PrometheusBuilder::new().with_push_gateway(\"x\", todo!(), None, None, false);\n",
        "}\n",
    );
    let file = "app/src-tauri/src/fixture_resolving.rs";
    let sites = file_effect_sites(file, resolving, &module_of(file));
    assert_eq!(sites.get("network"), Some(&10), "{sites:?}");

    // Every latent class has entries, so its callers are sites.
    for (file, _, reason) in EFFECT_FILES {
        if !reason.starts_with("latent") {
            continue;
        }
        let own = module_of(file);
        let covered = LATENT_ENTRIES.iter().any(|(dir, _, module, _, _)| {
            let module: Vec<String> = std::iter::once("crate")
                .chain(module.split("::").filter(|s| !s.is_empty()))
                .map(str::to_string)
                .collect();
            file.starts_with(&format!("{dir}/src/")) && own == module
        });
        assert!(covered, "{file} is latent but no LATENT_ENTRIES names it");
    }
}

/// §22 (audit P10): the guards' file set is the module tree. Every module a
/// production source declares, with or without `#[path]`, loads a source the
/// guards scan, and the only production `include!`s are the two toolchain
/// manifests that build scripts generate (constants), pinned exactly.
#[test]
fn p3_g6_13_every_production_module_is_a_scanned_source() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scanned: BTreeSet<String> = workspace_sources()
        .iter()
        .map(|(file, _)| file.clone())
        .chain(NAME_SKIPPED_MODULES.iter().map(|file| file.to_string()))
        .collect();
    // Every production crate's targets, from its manifest: what Cargo
    // infers (`src/lib.rs`, `src/main.rs`, `src/bin`) and every `[lib]` or
    // `[[bin]]` path, which must lie below `src`. Each is a scanned source,
    // and no crate is a proc-macro (code that no guard reads).
    let mut roots = BTreeSet::new();
    for manifest in workspace_manifests(true) {
        let crate_dir = manifest.strip_suffix("Cargo.toml").unwrap().to_string();
        let mut section = String::new();
        let mut package = false;
        for line in workspace_file(&manifest).lines() {
            let line = line.split('#').next().unwrap_or_default().trim();
            if line.starts_with('[') {
                section = line.to_string();
                package |= line == "[package]";
                continue;
            }
            let compact: String = line.split_whitespace().collect();
            assert!(
                compact != "proc-macro=true" && compact != "proc_macro=true",
                "{manifest}: a proc-macro crate"
            );
            if (section == "[lib]" || section == "[[bin]]") && compact.starts_with("path=") {
                let value = line.split('"').nth(1).unwrap_or_default();
                assert!(value.starts_with("src/"), "{manifest}: a target at {value}");
                roots.insert(format!("{crate_dir}{value}"));
            }
        }
        if !package {
            continue;
        }
        for default in ["src/lib.rs", "src/main.rs"] {
            if root.join(&crate_dir).join(default).is_file() {
                roots.insert(format!("{crate_dir}{default}"));
            }
        }
        if let Ok(entries) = std::fs::read_dir(root.join(&crate_dir).join("src/bin")) {
            for path in entries.flatten().map(|entry| entry.path()) {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                if path.is_file() && name.ends_with(".rs") {
                    roots.insert(format!("{crate_dir}src/bin/{name}"));
                } else if path.join("main.rs").is_file() {
                    roots.insert(format!("{crate_dir}src/bin/{name}/main.rs"));
                }
            }
        }
    }
    for target in &roots {
        assert!(
            scanned.contains(target),
            "{target}: a crate target no guard scans"
        );
    }
    // The module tree, from every crate root: (file, loaded through `#[path]`).
    let mut queue: Vec<(String, bool)> = roots.iter().map(|file| (file.clone(), false)).collect();
    assert!(queue.len() > 60, "{} crate roots", queue.len());
    let mut seen = BTreeSet::new();
    let mut includes = BTreeSet::new();
    let mut declared = 0;
    while let Some((file, path_loaded)) = queue.pop() {
        if !seen.insert(file.clone()) {
            continue;
        }
        let (found, included) = module_declarations(&workspace_file(&file));
        includes.extend(included.into_iter().map(|call| (file.clone(), call)));
        let path = std::path::Path::new(&file);
        let dir = path.parent().unwrap();
        // rustc treats every `#[path]` file as a `mod.rs`: its own modules
        // are its siblings.
        let mod_rs = path.file_name().unwrap() == "mod.rs" || roots.contains(&file) || path_loaded;
        let own_dir = if mod_rs {
            dir.to_path_buf()
        } else {
            dir.join(path.file_stem().unwrap())
        };
        for d in found {
            declared += 1;
            let inline: std::path::PathBuf = d.inline.iter().collect();
            let candidates = match &d.path {
                Some(value) if d.inline.is_empty() => vec![dir.join(value)],
                Some(value) => vec![own_dir.join(&inline).join(value)],
                None => {
                    let base = own_dir.join(&inline);
                    vec![
                        base.join(format!("{}.rs", d.name)),
                        base.join(&d.name).join("mod.rs"),
                    ]
                }
            };
            let loaded: Vec<String> = candidates
                .iter()
                .map(|candidate| lexically_normal(candidate))
                .filter(|candidate| root.join(candidate).is_file())
                .collect();
            assert_eq!(loaded.len(), 1, "{file}: mod {} loads {loaded:?}", d.name);
            assert!(
                scanned.contains(&loaded[0]),
                "{file}: mod {} loads {}, which no guard scans",
                d.name,
                loaded[0]
            );
            queue.push((loaded[0].clone(), d.path.is_some()));
        }
    }
    assert!(declared > 400, "{declared} declarations");
    assert!(seen.len() > 500, "{} production modules", seen.len());
    assert_eq!(
        includes,
        BTreeSet::from([
            (
                "app/src-tauri/src/builder_workspace/trusted_toolchain.rs".to_string(),
                "include!(concat!(env!(\"OUT_DIR\"),\"/builder_toolchain_manifest.rs\"))"
                    .to_string()
            ),
            (
                "crates/nexus-verifier-sandbox/src/toolchain.rs".to_string(),
                "include!(concat!(env!(\"OUT_DIR\"),\"/verifier_toolchain_manifest.rs\"))"
                    .to_string()
            ),
        ])
    );
    // The scan itself: a `#[path]` module inside an inline module, a test
    // module, and an `include!` are all seen.
    let (found, included) = module_declarations(concat!(
        "pub(crate) mod a;\n",
        "mod outer { #[cfg(unix)] #[path = \"../x.rs\"] pub mod b; }\n",
        "#[cfg(test)] mod tests;\n",
        "include!(\"gen.rs\");\n",
    ));
    let found: Vec<(String, Option<String>, Vec<String>)> = found
        .into_iter()
        .map(|d| (d.name, d.path, d.inline))
        .collect();
    assert_eq!(
        found,
        [
            ("a".to_string(), None, vec![]),
            (
                "b".to_string(),
                Some("../x.rs".to_string()),
                vec!["outer".to_string()]
            ),
        ]
    );
    assert_eq!(included, ["include!(\"gen.rs\")"]);
}

/// A production `mod name;` declaration: its `#[path]` value, and the
/// inline modules it sits in.
struct ModuleDeclaration {
    name: String,
    path: Option<String>,
    inline: Vec<String>,
}

/// The production module declarations and `include!` calls (rendered
/// without whitespace) of one source.
fn module_declarations(src: &str) -> (Vec<ModuleDeclaration>, Vec<String>) {
    use crate::phase0_surface::rust_paths::{Analysis, Tok, Token};
    let ident = |token: Option<&Token>| match token.map(|token| &token.tok) {
        Some(Tok::Ident(name)) => Some(name.clone()),
        _ => None,
    };
    let rendered = |tokens: &[Token]| -> String {
        tokens
            .iter()
            .map(|token| match &token.tok {
                Tok::Ident(text) | Tok::Num(text) | Tok::Punct(text) => text.clone(),
                Tok::Str(text) => format!("\"{text}\""),
                Tok::Atom => "'".to_string(),
            })
            .collect()
    };
    let analysis = Analysis::new(src, &["crate"]);
    let t = &analysis.tokens;
    let mut declarations = Vec::new();
    let mut includes = Vec::new();
    // Inline modules: (name, depth of their body).
    let mut inline: Vec<(String, usize)> = Vec::new();
    let mut depth = 0usize;
    for k in 0..t.len() {
        if t[k].is("{") {
            depth += 1;
            if k >= 2 && t[k - 2].is("mod") {
                if let Some(name) = ident(t.get(k - 1)) {
                    inline.push((name, depth));
                }
            }
            continue;
        }
        if t[k].is("}") {
            if inline.last().is_some_and(|(_, at)| *at == depth) {
                inline.pop();
            }
            depth = depth.saturating_sub(1);
            continue;
        }
        if analysis.test[k] {
            continue;
        }
        if t[k].is("include") && t.get(k + 1).is_some_and(|x| x.is("!")) {
            let mut end = k + 2;
            let mut open = 0;
            while end < t.len() {
                if t[end].is("(") {
                    open += 1;
                } else if t[end].is(")") {
                    open -= 1;
                    if open == 0 {
                        break;
                    }
                }
                end += 1;
            }
            includes.push(rendered(&t[k..=end.min(t.len() - 1)]));
            continue;
        }
        let qualified = k > 0 && (t[k - 1].is("::") || t[k - 1].is("."));
        let Some(name) = ident(t.get(k + 1)) else {
            continue;
        };
        if !t[k].is("mod") || qualified || !t.get(k + 2).is_some_and(|x| x.is(";")) {
            continue;
        }
        // The item's attributes, before its visibility.
        let mut j = k;
        if j >= 1 && t[j - 1].is("pub") {
            j -= 1;
        } else if j >= 1 && t[j - 1].is(")") {
            let open = (0..j - 1).rev().find(|&o| t[o].is("(")).unwrap();
            if open >= 1 && t[open - 1].is("pub") {
                j = open - 1;
            }
        }
        let mut path = None;
        while j >= 1 && t[j - 1].is("]") {
            let close = j - 1;
            let mut level = 0;
            let mut open = close;
            loop {
                if t[open].is("]") {
                    level += 1;
                } else if t[open].is("[") {
                    level -= 1;
                    if level == 0 {
                        break;
                    }
                }
                open -= 1;
            }
            if open == 0 || !t[open - 1].is("#") {
                break;
            }
            let body = &t[open + 1..close];
            assert!(
                !(body.first().is_some_and(|x| x.is("cfg_attr"))
                    && body.iter().any(|x| x.is("path"))),
                "mod {name} has a conditional #[path]"
            );
            if let [key, eq, value] = body {
                if let (true, true, Tok::Str(value)) = (key.is("path"), eq.is("="), &value.tok) {
                    path = Some(value.clone());
                }
            }
            j = open - 1;
        }
        declarations.push(ModuleDeclaration {
            name,
            path,
            inline: inline.iter().map(|(name, _)| name.clone()).collect(),
        });
    }
    (declarations, includes)
}

/// Every workspace `Cargo.toml` (workspace-relative, `/`-separated), build
/// output and dependencies aside; `production` also leaves out the
/// directories that hold no production source (`NOT_PRODUCTION_DIRS`).
fn workspace_manifests(production: bool) -> Vec<String> {
    fn walk(
        root: &std::path::Path,
        dir: &std::path::Path,
        production: bool,
        out: &mut Vec<String>,
    ) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                let skipped = name.starts_with('.')
                    || name == "target"
                    || name == "node_modules"
                    || (production && NOT_PRODUCTION_DIRS.contains(&name.as_str()));
                if !skipped {
                    walk(root, &path, production, out);
                }
            } else if name == "Cargo.toml" {
                out.push(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    walk(&root, &root, production, &mut out);
    assert!(out.len() > 40, "{out:?}");
    out
}

/// `path` with `.` and `..` resolved lexically, `/`-separated.
fn lexically_normal(path: &std::path::Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop();
            }
            other => parts.push(other.as_os_str().to_string_lossy().into_owned()),
        }
    }
    parts.join("/")
}

/// §22 (audit P25): a macro can build a path, a trait or a method name from
/// its fragments, and then neither the resolver nor the impl inventories see
/// it. The production `macro_rules!` are pinned, and none of them puts a
/// fragment in a path, as a trait, as a method or as a macro name.
#[test]
fn p3_g6_14_no_macro_assembles_a_path_trait_or_method() {
    let sources = workspace_sources()
        .iter()
        .map(|(file, text)| (file.clone(), text.clone()))
        .chain(
            NAME_SKIPPED_MODULES
                .iter()
                .map(|file| (file.to_string(), production_text(&workspace_file(file)))),
        );
    let mut defined = BTreeMap::new();
    for (file, text) in sources {
        let m = masked(&text);
        for at in words(&m, "macro_rules") {
            let open = at
                + m[at..]
                    .iter()
                    .position(|c| matches!(c, b'{' | b'(' | b'['))
                    .expect("a macro body");
            let body = String::from_utf8_lossy(&m[open..close_of(&m, open)]).into_owned();
            *defined.entry(file.clone()).or_insert(0) += 1;
            for (i, _) in body.match_indices('$') {
                let rest = &body[i + 1..];
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if name.is_empty() || name == "crate" {
                    continue; // a repetition, or the macro's own crate
                }
                let after = rest[name.len()..].trim_start();
                let before = body[..i].trim_end();
                let as_trait = after.starts_with("for")
                    && !after[3..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_');
                assert!(
                    !after.starts_with("::") && !before.ends_with("::"),
                    "{file}: ${name} in a path"
                );
                assert!(!before.ends_with('.'), "{file}: ${name} as a method");
                assert!(!after.starts_with('!'), "{file}: ${name} as a macro");
                assert!(!as_trait, "{file}: ${name} as a trait");
            }
        }
    }
    assert_eq!(
        defined,
        BTreeMap::from([
            (
                "crates/nexus-governed-control/src/authority/ids.rs".to_string(),
                2
            ),
            ("crates/nexus-verifier-sandbox/src/hash.rs".to_string(), 1),
        ])
    );
}

/// The crate's compile-fail doctests, each pinned whole with the error that
/// makes it fail (checked by rustdoc on nightly; on the pinned stable
/// toolchain the text pin keeps each one failing for its stated reason).
const COMPILE_FAIL_DOCTESTS: &[(&str, &str)] = &[
    (
        "E0451",
        "use nexus_governed_control::authority::approval::R2Approval;\nuse nexus_governed_control::authority::ids::{CommitmentId, Digest};\nfn forge(id: CommitmentId, binding: Digest) -> R2Approval {\n    R2Approval { commitment: id, binding } // the fields are private\n}",
    ),
    (
        "E0624",
        "use nexus_governed_control::authority::approval::R2Approval;\nuse nexus_governed_control::authority::ids::{CommitmentId, Digest};\nfn forge(id: CommitmentId, binding: Digest) -> R2Approval {\n    R2Approval::confirmed(id, binding) // crate-private\n}",
    ),
    (
        "E0599",
        "use nexus_governed_control::authority::approval::R2Approval;\nfn twice(approval: R2Approval) -> (R2Approval, R2Approval) {\n    (approval.clone(), approval) // not Clone\n}",
    ),
    (
        "E0277",
        "let _: nexus_governed_control::authority::approval::R2Approval =\n    serde_json::from_str(\"{}\").unwrap(); // no deserializer",
    ),
    (
        "E0624",
        "use nexus_governed_control::authority::ids::CommitmentId;\nlet _ = CommitmentId::fresh(); // only the registry creates identities",
    ),
    (
        "E0599",
        "use nexus_governed_control::authority::commitment::ExecutionGuard;\nfn twice(guard: ExecutionGuard) -> (ExecutionGuard, ExecutionGuard) {\n    (guard.clone(), guard) // one-shot: not Clone\n}",
    ),
    (
        "E0624",
        "use nexus_governed_control::authority::Authority;\nuse nexus_governed_control::authority::ids::{AgentId, CommitmentId, Digest, RunId};\nfn start(a: &Authority, id: CommitmentId, agent: &AgentId, run: RunId, d: &Digest) {\n    // Only the pipeline starts an effect: the lifecycle is crate-private.\n    let _ = a.commitments().begin(id, agent, run, d, d);\n}",
    ),
];

/// §24 (audit P27): the non-forgeability doctests stay, each whole and with
/// its error code, and keep running under `cargo test`.
#[test]
fn p3_g6_15_the_compile_fail_doctests_are_pinned_with_their_errors() {
    let lib = workspace_file("crates/nexus-governed-control/src/lib.rs");
    let docs: String = lib
        .lines()
        .filter_map(|line| line.strip_prefix("//!"))
        .map(|line| line.strip_prefix(' ').unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    let mut blocks = Vec::new();
    let mut rest = docs.as_str();
    while let Some(start) = rest.find("```") {
        let fence = &rest[start + 3..];
        let tag_end = fence.find('\n').expect("a fence line");
        let tag = &fence[..tag_end];
        let close = fence[tag_end + 1..].find("```").expect("a closed block");
        let code = fence[tag_end + 1..tag_end + 1 + close].trim_end();
        if tag.starts_with("compile_fail") {
            let code_tag = tag
                .strip_prefix("compile_fail,")
                .unwrap_or_else(|| panic!("a compile_fail doctest without an error code: {code}"));
            blocks.push((code_tag.to_string(), code.to_string()));
        }
        rest = &fence[tag_end + 1 + close + 3..];
    }
    let pinned: Vec<(String, String)> = COMPILE_FAIL_DOCTESTS
        .iter()
        .map(|(code, text)| (code.to_string(), text.to_string()))
        .collect();
    assert_eq!(blocks, pinned);
    let manifest = workspace_file("crates/nexus-governed-control/Cargo.toml");
    assert!(
        !manifest
            .lines()
            .any(|line| line.trim_start().starts_with("doctest")),
        "{manifest}"
    );
    assert!(normalized(production_source(
        "crates/nexus-governed-control/src/lib.rs"
    ))
    .contains("pubmodauthority;"));
}

/// Test-only features (see `cfg_requires_test`): every edge that enables one
/// is a dev-dependency, or belongs to the integration-test crate, and no
/// crate's own feature turns one on, so no shipped build compiles what the
/// production scanners leave out.
#[test]
fn p3_g6_16_test_only_features_are_enabled_only_by_tests() {
    const TEST_ONLY_FEATURES: [&str; 5] = [
        "test-support",
        "test-utils",
        "testing",
        "live-sandbox-harness",
        "development-toolchain",
    ];
    // These are exactly the features the production scanners treat as
    // test-only (`cfg_requires_test`, the same in both scanner copies).
    let scanner = item_source(
        &workspace_file(OWN_SOURCE),
        "fn cfg_requires_test(predicate: &str) -> bool {",
    );
    let start = scanner
        .find("constTEST_ONLY_FEATURES")
        .expect("the scanners' test-only features");
    let listed: Vec<&str> = scanner[start..start + scanner[start..].find("];").unwrap()]
        .split('"')
        .skip(1)
        .step_by(2)
        .collect();
    assert_eq!(listed, TEST_ONLY_FEATURES);
    // Every manifest, the integration-test crate's included.
    let manifests = workspace_manifests(false);
    let named = |text: &str| -> Vec<String> {
        text.split('"')
            .skip(1)
            .step_by(2)
            .map(str::to_string)
            .collect()
    };
    let mut enabling = BTreeSet::new();
    for manifest in &manifests {
        let text = workspace_file(manifest);
        let mut section = String::new();
        let mut pending = String::new();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or_default().trim();
            if line.starts_with('[') && pending.is_empty() {
                section = line.trim_matches(['[', ']']).to_string();
                continue;
            }
            pending.push_str(line);
            pending.push(' ');
            if pending.matches('[').count() > pending.matches(']').count() {
                continue; // a list continues on the next line
            }
            let statement = std::mem::take(&mut pending);
            let Some((key, value)) = statement.split_once('=') else {
                continue;
            };
            let key = key.trim();
            for feature in TEST_ONLY_FEATURES {
                let enables = if section == "features" {
                    key != feature
                        && named(value)
                            .iter()
                            .any(|item| item == feature || item.ends_with(&format!("/{feature}")))
                } else {
                    (key == "features" || value.contains("features"))
                        && named(value).iter().any(|item| item == feature)
                };
                if enables {
                    enabling.insert((manifest.clone(), section.clone(), key.to_string()));
                }
            }
        }
    }
    for (manifest, section, key) in &enabling {
        assert!(
            section.ends_with("dev-dependencies") || manifest == "tests/integration/Cargo.toml",
            "{manifest} [{section}] {key} enables a test-only feature"
        );
    }
    assert_eq!(enabling.len(), 6, "the known test-only edges: {enabling:?}");
}

/// §25 (audit P32): only the browser and the agent display launch a session
/// process, each with the executable it pinned.
#[test]
fn p3_g6_17_only_the_browser_and_display_launch_session_processes() {
    let mut launches = references(P3_CRATE, "launch");
    launches.sort();
    assert_eq!(
        launches,
        [
            (
                "crates/nexus-governed-control/src/browser/mod.rs",
                "run".to_string()
            ),
            (
                "crates/nexus-governed-control/src/display/server.rs",
                "AgentServer::start".to_string()
            ),
        ]
    );
    for (file, function, program) in [
        (
            "browser/mod.rs",
            "run",
            "program:session.executable.clone(),",
        ),
        (
            "display/server.rs",
            "AgentServer::start",
            "program:program.path.clone(),",
        ),
    ] {
        let text = production_source(&format!("{P3_CRATE}{file}"));
        let launching: Vec<String> = fn_items(text)
            .into_iter()
            .filter(|item| item.path == function)
            .map(|item| compact(item.body_text(text)))
            .filter(|body| body.contains("SessionProcess::launch("))
            .collect();
        assert_eq!(launching.len(), 1, "{file}");
        assert!(
            launching[0].contains(&format!("SessionProcess::launch(SessionSpec{{{program}")),
            "{file}: {}",
            launching[0]
        );
    }
}

/// G9 (audit Q15, Q17): a response body, which may echo a released
/// credential, lives only in zeroizing buffers: `exchange` collects it with
/// `append` into a `Zeroizing` vector and grows it no other way, and
/// `append` moves an outgrown buffer into a fresh one rather than letting
/// the vector reallocate. A peer the client cannot name is unpinned.
#[test]
fn p3_g9_01_a_response_body_lives_only_in_zeroizing_buffers() {
    let text = production_source(&format!("{P3_CRATE}egress/transport.rs"));
    // The exchange is pinned whole: the peer check, the zeroizing body that
    // grows only through `append`, and nothing else that writes into it.
    let exchange = compact(one_fn(text, "exchange").body_text(text));
    assert_eq!(exchange, EXCHANGE);
    // Redaction builds its output in a buffer sized for the worst case.
    let redact = compact(one_fn(text, "redact_one").body_text(text));
    assert!(
        redact.contains("letmutout=Vec::with_capacity(response.body.len()/needle.len()*MARK.len().saturating_sub(needle.len())+response.body.len());"),
        "{redact}"
    );
    // The header fields go through a buffer sized the same way.
    assert!(
        redact.contains("letredacted=replaced(text,secret);"),
        "{redact}"
    );
    let append = compact(one_fn(text, "append").body_text(text));
    assert_in_order(
        &append,
        &[
            "ifneeded>body.capacity(){",
            "letmutgrown=Zeroizing::new(Vec::with_capacity(",
            "grown.extend_from_slice(body);*body=grown;}",
            "body.extend_from_slice(chunk);",
        ],
        "append",
    );
}

/// The governed transport's exchange, pinned whole (normalized text): see
/// `p3_g9_01`.
const EXCHANGE: &str = "letPinnedRequest{method,url,domain,addresses,headers,body,secret,timeout,max_body}=request;ifaddresses.is_empty(){returnErr(TransportError::Unpinned);}letmutbuilder=Client::builder().no_proxy().redirect(Policy::none()).referer(false).https_only(url.scheme()==\"https\").connect_timeout(timeout.min(Duration::from_secs(10))).timeout(timeout).pool_max_idle_per_host(0).user_agent(USER_AGENT).use_rustls_tls();ifletSome(domain)=&domain{builder=builder.resolve_to_addrs(domain,&addresses);}letclient=builder.build().map_err(|_|TransportError::Unavailable)?;letmutheader_map=HeaderMap::new();for(name,value)inheaders{header_map.append(name,value);}letredact_with=secret.as_ref().map(needles);ifletSome(secret)=secret{letmutvalue=HeaderValue::from_str(&secret.value).map_err(|_|TransportError::Protocol)?;value.set_sensitive(true);header_map.insert(secret.name,value);}letmutrequest=client.request(method.to_reqwest(),url).headers(header_map);ifletSome(body)=body{request=request.body(body);}letwork=asyncmove{letmutresponse=request.send().await.map_err(classify)?;letpeer=response.remote_addr();if!peer.is_some_and(|peer|addresses.iter().any(|a|a.ip()==peer.ip())){returnErr(TransportError::Unpinned);}letstatus=response.status().as_u16();letcontent_type=header_text(response.headers().get(CONTENT_TYPE),128);letlocation=header_text(response.headers().get(LOCATION),MAX_URL);ifresponse.content_length().is_some_and(|length|length>max_bodyasu64){returnErr(TransportError::Bounds);}letmutbody=Zeroizing::new(Vec::with_capacity(response.content_length().map_or(INITIAL_BODY,|length|lengthasusize).min(max_body)));whileletSome(chunk)=response.chunk().await.map_err(classify)?{ifbody.len()+chunk.len()>max_body{returnErr(TransportError::Bounds);}append(&mutbody,&chunk,max_body);}Ok(HttpResponse{status,peer,content_type,location,body:take(&mut*body),redacted:false})};letmutresponse=select!{result=work=>result?,()=until_cancelled(&cancel)=>returnErr(TransportError::Cancelled)};ifletSome(secret)=redact_with{redact(&mutresponse,&secret);}Ok(response)";

/// G9 (audit Q29): the agent display's X connection is made in one place,
/// on the agent socket whose ownership was just checked, with the session's
/// cookie: `connect_to_stream_with_auth_info` in `display/server.rs`'s
/// `connect`. No constructor that reads `DISPLAY` or `XAUTHORITY`
/// (`x11rb::connect`, `RustConnection::connect`), opens its own stream, or
/// skips the cookie (`connect_to_stream`) is named in Phase Three; `p3_g6_04`
/// and the §22 inventory confine `x11rb` to that file.
#[test]
fn p3_g9_02_the_x_connection_is_made_only_on_the_agent_socket_with_its_cookie() {
    use crate::phase0_surface::rust_paths::Analysis;
    let mut constructors = BTreeSet::new();
    for (file, _) in p3_sources() {
        let src = workspace_file(&format!("{P3_CRATE}{file}"));
        let analysis = Analysis::new(&src, &["crate"]);
        for o in analysis.production() {
            for path in o.resolved.iter().chain(as_written(&o.written)) {
                let last = path.last().map(String::as_str).unwrap_or_default();
                if path.first().is_some_and(|root| root == "x11rb")
                    && (last.starts_with("connect")
                        || path
                            .iter()
                            .any(|segment| segment == "xcb_ffi" || segment == "XCBConnection"))
                {
                    constructors.insert((file.to_string(), path.join("::")));
                }
            }
        }
    }
    assert_eq!(
        constructors,
        BTreeSet::from([(
            "display/server.rs".to_string(),
            "x11rb::rust_connection::RustConnection::connect_to_stream_with_auth_info".to_string()
        )])
    );
    // Every path call of a `connect` in the crate, however written
    // (`<RustConnection>::connect` included): only these two.
    let mut paths = Vec::new();
    for (file, _) in p3_sources() {
        let text = production_source(&format!("{P3_CRATE}{file}"));
        let m = masked(text);
        for name in [
            "connect",
            "connect_to_stream",
            "connect_to_stream_with_auth_info",
        ] {
            for at in words(&m, name) {
                let before = m[..at]
                    .iter()
                    .rposition(|c| !c.is_ascii_whitespace())
                    .map_or(0, |i| i + 1);
                if before >= 2 && &m[before - 2..before] == b"::" {
                    let qualifier = String::from_utf8_lossy(&m[..before - 2]);
                    let qualifier = qualifier
                        .rsplit(|c: char| {
                            !(c.is_ascii_alphanumeric() || c == '_' || c == '>' || c == '<')
                        })
                        .next()
                        .unwrap_or_default()
                        .to_string();
                    paths.push((file.to_string(), qualifier, name));
                }
            }
        }
    }
    // Each counted, so a second one in the same file shows.
    paths.sort();
    assert_eq!(
        paths,
        [
            (
                "display/server.rs".to_string(),
                "RustConnection".to_string(),
                "connect_to_stream_with_auth_info"
            ),
            (
                "display/server.rs".to_string(),
                "UnixStream".to_string(),
                "connect"
            ),
        ]
    );
    let text = production_source(&format!("{P3_CRATE}display/server.rs"));
    // The socket is the agent display's, in the fixed socket directory.
    assert_eq!(
        compact(one_fn(text, "socket").body_text(text)),
        "PathBuf::from(format!(\"{SOCKET_DIR}{number}\"))"
    );
    assert!(text.contains("const SOCKET_DIR: &str = \"/tmp/.X11-unix/X\";"));
    let naming: Vec<String> = fn_items(text)
        .into_iter()
        .filter(|item| {
            item.body_text(text)
                .contains("connect_to_stream_with_auth_info")
        })
        .map(|item| item.path)
        .collect();
    assert_eq!(naming, ["connect"]);
    let connect = compact(one_fn(text, "connect").body_text(text));
    assert_in_order(
        &connect,
        &[
            "if!socket_is_ours(number,uid){returnNone;}",
            "letstream=UnixStream::connect(socket(number)).ok()?;",
            "connect_to_stream_with_auth_info(stream,0,COOKIE.to_vec(),cookie.to_vec())",
        ],
        "connect",
    );
}

/// G9 (audit Q6, Q7): a browser session goes out only while it may. Its
/// proxy is given the guard's liveness (the run not cancelled, the
/// commitment still covered), asks it before every step out of a
/// connection and stops when it ends; the session asks before each step
/// whether its grant is still live.
#[test]
fn p3_g9_03_the_browser_goes_out_only_while_its_session_may() {
    let browser = production_source(&format!("{P3_CRATE}browser/mod.rs"));
    let runs: Vec<String> = fn_items(browser)
        .into_iter()
        .filter(|item| item.path == "run")
        .map(|item| compact(item.body_text(browser)))
        .filter(|body| body.contains("BrowserProxy::start("))
        .collect();
    assert_eq!(runs.len(), 1);
    assert_in_order(
        &runs[0],
        &[
            "letproxy=BrowserProxy::start(OriginPolicy{",
            "live:guard.liveness()}",
            "ifletSome(reason)=guard.lapse(){returnErr(before((FailureClass::Refused,reason.into())));}letstart=page.navigate(&session.start).map_err(before)?;",
            "ifletSome(reason)=guard.lapse(){returnErr(at((FailureClass::Refused,reason.into())));}",
            "page.close_popups().map_err(at)?;",
            "page.step(step).map_err(at)?;",
            "page.close_popups().map_err(after)?;ifletSome(reason)=guard.lapse(){returnErr(after((FailureClass::Refused,reason.into())));}",
        ],
        "run",
    );
    // The crate names the proxy only there, outside its own file (an alias
    // or another start would show), and starts it once.
    let named: Vec<(&str, String)> = mentions(P3_CRATE, "BrowserProxy")
        .into_iter()
        .filter(|(file, _)| !file.ends_with("browser/proxy.rs"))
        .collect();
    assert_eq!(
        named,
        [
            (
                "crates/nexus-governed-control/src/browser/mod.rs",
                String::new()
            ),
            (
                "crates/nexus-governed-control/src/browser/mod.rs",
                "run".to_string()
            ),
        ]
    );
    let starts: usize = p3_sources()
        .into_iter()
        .map(|(file, _)| {
            compact(production_source(&format!("{P3_CRATE}{file}")))
                .matches("BrowserProxy::start(")
                .count()
        })
        .sum();
    assert_eq!(starts, 1);
    let proxy = production_source(&format!("{P3_CRATE}browser/proxy.rs"));
    // Nor does the proxy's own file start another through `Self`.
    assert!(!compact(proxy).contains("Self::start("));
    assert_eq!(
        compact(one_fn(proxy, "may_go_on").body_text(proxy)),
        "!stop.load(Ordering::SeqCst)&&(policy.live)()"
    );
    // When the session may no longer go out, the traffic stops and the port
    // stays the session's: new connections are closed unserved until the
    // session ends.
    assert_in_order(
        &compact(one_fn(proxy, "BrowserProxy::start").body_text(proxy)),
        &[
            "while!closed.load(Ordering::SeqCst){letlive=catch_unwind(AssertUnwindSafe(||{(policy.live)()})).unwrap_or(false);if!live{stop.store(true,Ordering::SeqCst);}",
            "ifstop.load(Ordering::SeqCst)||active.load(Ordering::SeqCst)>=MAX_CONNECTIONS{refused.fetch_add(1,Ordering::SeqCst);drop(client);continue;}",
            "ifspawned.is_err(){active.fetch_sub(1,Ordering::SeqCst);refused.fetch_add(1,Ordering::SeqCst);}",
        ],
        "BrowserProxy::start",
    );
    assert_eq!(
        compact(one_fn(proxy, "BrowserProxy::drop").body_text(proxy)),
        "self.stop.store(true,Ordering::SeqCst);self.closed.store(true,Ordering::SeqCst);"
    );
    assert_in_order(
        &compact(one_fn(proxy, "tunnel").body_text(proxy)),
        &[
            "copy(client_reader,upstream,||may_go_on(&up_policy,&up_stop))",
            "copy(upstream_reader,client,||may_go_on(policy,stop));",
        ],
        "tunnel",
    );
    assert_in_order(
        &compact(one_fn(proxy, "serve").body_text(proxy)),
        &[
            "letgo_on=||may_go_on(policy,stop);",
            "read_head(&mutclient,stop)else{returnrefuse(client);};if!go_on(){returnrefuse(client);}",
            "connect(&addresses,go_on)",
            "if!go_on(){returnrefuse(client);}ifclient.write_all(",
            "connect(&addresses,go_on)",
            "if!go_on(){returnrefuse(client);}ifupstream.write_all(forwarded.as_bytes())",
        ],
        "serve",
    );
    assert_in_order(
        &compact(one_fn(proxy, "connect").body_text(proxy)),
        &[
            "foraddressinaddresses{if!go_on(){returnNone;}",
            "connect_timeout(",
        ],
        "connect",
    );
    assert_in_order(
        &compact(one_fn(proxy, "copy").body_text(proxy)),
        &[
            "whilego_on(){",
            "iftotal>MAX_TUNNEL||!go_on()||to.write_all(&buffer[..n]).is_err(){break;}",
        ],
        "copy",
    );
}

/// G10, G11 (verification V3, G10-A N1 and N6): no command the interface
/// thread runs waits on the agent loops' lock, which a running cycle holds
/// for its whole length (so the page and every stop stay usable). The
/// kernel's loop runtime takes it in the methods found here (and those that
/// call them); every desktop function that reaches one, through any chain
/// of desktop functions called outside the threads and tasks they spawn, is
/// pinned; and no command the interface thread runs (`#[command]` that is
/// neither `#[command(async)]` nor an `async fn`), in any desktop file,
/// calls one of them or a locking method, outside a thread it spawns.
/// Names are matched as calls, so the set is conservative (a method of the
/// same name elsewhere counts too).
#[test]
fn p3_g10_01_no_interface_thread_command_waits_on_the_loops_lock() {
    // The methods that take the lock, and those that call one of them.
    let kernel = production_source("kernel/src/cognitive/loop_runtime.rs");
    let methods: Vec<(String, String)> = fn_items(kernel)
        .into_iter()
        .map(|item| (item.name.clone(), compact(item.body_text(kernel))))
        .collect();
    let mut locking: BTreeSet<String> = methods
        .iter()
        .filter(|(_, body)| body.contains("self.loops.lock()"))
        .map(|(name, _)| name.clone())
        .collect();
    loop {
        let more: Vec<String> = methods
            .iter()
            .filter(|(name, body)| {
                !locking.contains(name)
                    && locking
                        .iter()
                        .any(|method| body.contains(&format!("self.{method}(")))
            })
            .map(|(name, _)| name.clone())
            .collect();
        if more.is_empty() {
            break;
        }
        locking.extend(more);
    }
    for method in [
        "stop_agent_loop",
        "approve_blocked_steps",
        "approve_blocked_step",
        "deny_blocked_step",
        "assign_goal",
        "set_review_each_mode",
        "get_agent_status",
    ] {
        assert!(locking.contains(method), "{method}: {locking:?}");
    }
    // Every desktop function's code outside what it spawns, literals masked.
    let code = |text: &str, item: &FnItem| {
        let masked = String::from_utf8(masked(item.body_text(text))).expect("masked text");
        outside_spawned(&compact(&masked))
    };
    let mut bodies: Vec<(&'static str, FnItem, String)> = Vec::new();
    for (file, text) in workspace_sources() {
        if file.starts_with(DESKTOP_SRC) {
            for item in fn_items(text) {
                let body = code(text, &item);
                bodies.push((file.as_str(), item, body));
            }
        }
    }
    let takes_lock = |body: &str| {
        locking
            .iter()
            .any(|method| body.contains(&format!("cognitive_runtime.{method}(")))
    };
    let mut waiting: BTreeSet<String> = bodies
        .iter()
        .filter(|(_, _, body)| takes_lock(body))
        .map(|(_, item, _)| item.name.clone())
        .collect();
    loop {
        let more: Vec<String> = bodies
            .iter()
            .filter(|(_, item, body)| {
                !waiting.contains(&item.name) && waiting.iter().any(|name| uses(body, name))
            })
            .map(|(_, item, _)| item.name.clone())
            .collect();
        if more.is_empty() {
            break;
        }
        waiting.extend(more);
    }
    // Reviewed: the consent decisions and goal assignment (their commands
    // run off the interface thread), the loop's own cycle and its model
    // route, the end of a loop (on the stop routines' threads), the review
    // mode switch, HiveMind sessions, the scheduler's tick and its trigger,
    // the Phase Three approval (a blocking-pool command), and names that
    // match those calls (`approve`, `execute`, `execute_goal`).
    assert_eq!(
        waiting,
        BTreeSet::from(
            [
                "approve",
                "approve_consent_request",
                "assign_agent_goal",
                "batch_approve_consents",
                "batch_deny_consents",
                "deny_consent_request",
                "end_agent_loop",
                "end_goal_loop",
                "execute",
                "execute_agent_goal",
                "execute_goal",
                "execute_hivemind_subtask",
                "p3_approve",
                "persist_task_start",
                "resolve_agent_llm_route",
                "review_consent_batch",
                "run_cognitive_cycle",
                "scheduler_trigger_now",
                "set_agent_review_mode",
                "start_hivemind",
                "with_agent_llm_route",
            ]
            .map(str::to_string)
        )
    );
    // The commands the interface thread runs, in every desktop file.
    let mut checked = 0;
    for (file, item, body) in &bodies {
        if command_runs_off_thread(item) != Some(false) {
            continue;
        }
        checked += 1;
        assert!(
            !takes_lock(body),
            "{file} {} waits on the loops' lock",
            item.path
        );
        for name in &waiting {
            assert!(
                !uses(body, name),
                "{file} {} waits on the loops' lock through {name}",
                item.path
            );
        }
    }
    // The reading itself: a doc string spelling the async attribute, a
    // `cfg_attr` command, a function value and a joined thread are seen.
    let fixture = "/// x\n#[doc = \"#[tauri::command(async)]\"]\n#[tauri::command]\nfn f() {}\n#[cfg_attr(all(), tauri::command)]\nfn g() {}\n#[tauri::command]\nasync fn h() {}\nfn i() {}\n";
    let threads: Vec<Option<bool>> = fn_items(fixture)
        .iter()
        .map(command_runs_off_thread)
        .collect();
    assert_eq!(threads, [Some(false), Some(false), Some(true), None]);
    assert!(uses(
        "letf=execute_agent_goal;f(state)",
        "execute_agent_goal"
    ));
    assert!(!uses(
        "letf=execute_agent_goal_count;",
        "execute_agent_goal"
    ));
    assert!(
        outside_spawned("spawn(move||{assign_agent_goal(x)}).join()")
            .contains("assign_agent_goal(")
    );
    assert!(!outside_spawned("spawn(move||{assign_agent_goal(x)});").contains("assign_agent_goal("));
    assert!(checked > 500, "{checked} interface-thread commands checked");
    // Consent decisions, off the interface thread, are still made one at a
    // time: two answers to one request never both count.
    let lib = production_source("app/src-tauri/src/lib.rs");
    for command in [
        "approve_consent_request",
        "deny_consent_request",
        "batch_approve_consents",
        "review_consent_batch",
        "batch_deny_consents",
    ] {
        let item = one_fn(lib, command);
        assert_eq!(item.head, "#[command(async)]", "{command}");
        let body = compact(item.body_text(lib));
        assert!(
            body.starts_with("let_one=CONSENT_DECISIONS.lock().unwrap_or_else(|p|p.into_inner());"),
            "{command}"
        );
        // Held to the end: never released early, never another lock.
        assert!(
            !body.contains("_one)") && !body.contains("CONSENT_DECISIONS:"),
            "{command}"
        );
    }
    assert_eq!(compact(lib).matches("CONSENT_DECISIONS:").count(), 1);
    // A HiveMind session runs on a thread of its own, not on a runtime
    // worker: its sub-tasks' loops must be free to run while it waits.
    let hivemind = one_fn(lib, "start_hivemind");
    assert_eq!(hivemind.head, "#[command]async");
    assert_in_order(
        &compact(hivemind.body_text(lib)),
        &[
            ".name(\"nexus-hivemind\".into()).spawn(move||{let_=done.send(start_hivemind(&state,goal,agent_ids));})",
            "result.await",
        ],
        "start_hivemind",
    );
}

/// Whether a desktop function, if it is a command, runs off the interface
/// thread (`Some(true)`: `#[command(async)]` or an `async fn`) or on it
/// (`Some(false)`); `None` if it is not a command. Its attributes are read
/// with their literals blanked (a doc string cannot spell one), and a
/// `cfg_attr` that makes it a command counts.
fn command_runs_off_thread(item: &FnItem) -> Option<bool> {
    let head = String::from_utf8(masked(&item.head)).expect("masked text");
    let command =
        head.contains("#[command") || (head.contains("cfg_attr(") && head.contains("command"));
    command.then(|| head.contains("command(async)") || head.ends_with("async"))
}

/// `text` (normalized) without the bodies of the threads and tasks it
/// spawns (`spawn(move||{…})`, `spawn(asyncmove{…})`), unless it waits for
/// them: a thread joined (`.join()`), a scope (which joins its threads) or
/// `block_on` keeps every body, since what runs there runs synchronously.
fn outside_spawned(text: &str) -> String {
    if text.contains(".join()") || calls(text, "scope") || calls(text, "block_on") {
        return text.to_string();
    }
    let mut out = String::new();
    let mut rest = text;
    loop {
        let next = ["spawn(move||{", "spawn(asyncmove{"]
            .iter()
            .filter_map(|start| rest.find(start).map(|at| (at, at + start.len() - 1)))
            .min();
        let Some((at, open)) = next else {
            break;
        };
        out.push_str(&rest[..at]);
        let mut depth = 0;
        let mut end = rest.len();
        for (i, c) in rest[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// Whether normalized `text` calls `name` (`name(`, not part of a longer
/// identifier).
fn calls(text: &str, name: &str) -> bool {
    text.match_indices(&format!("{name}("))
        .any(|(at, _)| !text[..at].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_'))
}

/// Whether normalized `text` calls `name` or takes it as a value: a
/// function bound to a name (`=name;`) or passed as the first argument
/// (`(name,`, `(name)`) to be called later. (A list naming commands, such
/// as their registration, is not a use.)
fn uses(text: &str, name: &str) -> bool {
    calls(text, name)
        || text.match_indices(name).any(|(at, _)| {
            let before = text[..at].chars().next_back();
            let after = text[at + name.len()..].chars().next();
            matches!(before, Some('=' | '('))
                && !text[..at].ends_with("==")
                && matches!(after, Some(';' | ',' | ')'))
        })
}
