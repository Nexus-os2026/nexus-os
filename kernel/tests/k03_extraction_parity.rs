//! K03 oracle registered before extraction. Only imports may follow the move.
//! The K01 JSON fixtures remain unchanged; independent Rust constructions bind
//! every planned variant to those bytes, its label and temporary capabilities.
//! K01 separately binds all six browser variants to their frozen bytes.
use nexus_kernel::action_types::{BrowserAction, PlannedAction};
use nexus_kernel::screen_region::ScreenRegion;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Deserialize)]
struct Row {
    variant: String,
    label: String,
    json: String,
}
fn rows() -> Vec<Row> {
    include_str!("retirement/actions.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn s(field: &str) -> String {
    format!("fixture-α\n{field}")
}
fn strings() -> Vec<String> {
    vec!["one".into(), "two".into()]
}

#[test]
fn every_planned_variant_keeps_independent_bytes_labels_and_capabilities() {
    use PlannedAction::*;
    let cases = [
        (
            LlmQuery {
                prompt: s("prompt"),
                context: strings(),
            },
            "llm.query",
        ),
        (FileRead { path: s("path") }, "fs.read"),
        (
            FileWrite {
                path: s("path"),
                content: s("content"),
            },
            "fs.write",
        ),
        (
            ShellCommand {
                command: s("command"),
                args: strings(),
            },
            "process.exec",
        ),
        (
            DockerCommand {
                subcommand: s("subcommand"),
                args: strings(),
            },
            "docker_run",
        ),
        (WebSearch { query: s("query") }, "web.search"),
        (
            WebFetch {
                url: "https://example.invalid/".into(),
            },
            "web.read",
        ),
        (
            ApiCall {
                method: "GET".into(),
                url: "https://example.invalid/".into(),
                body: None,
                headers: None,
            },
            "mcp.call",
        ),
        (
            ImageGenerate {
                prompt: s("prompt"),
                output_path: "".into(),
                provider: None,
                model: None,
                size: None,
            },
            "image.generate",
        ),
        (
            TextToSpeech {
                text: s("text"),
                output_path: "".into(),
                provider: None,
                voice: None,
                model: None,
            },
            "tts.generate",
        ),
        (
            KnowledgeGraphUpdate {
                entities: strings(),
                relationships: strings(),
            },
            "knowledge.graph",
        ),
        (KnowledgeGraphQuery { query: s("query") }, "knowledge.graph"),
        (
            BrowserAutomate {
                start_url: "https://example.invalid/".into(),
                actions: vec![BrowserAction::Navigate {
                    url: "https://example.invalid/".into(),
                }],
                screenshot_dir: None,
            },
            "browser.automate",
        ),
        (CaptureScreen { region: None }, "screen.capture"),
        (
            CaptureWindow {
                window_title: s("window_title"),
            },
            "screen.capture",
        ),
        (AnalyzeScreen { query: s("query") }, "screen.analyze"),
        (MouseMove { x: 7, y: 7 }, "input.mouse"),
        (
            MouseClick {
                x: 7,
                y: 7,
                button: "left".into(),
            },
            "input.mouse",
        ),
        (MouseDoubleClick { x: 7, y: 7 }, "input.mouse"),
        (
            MouseDrag {
                from_x: 7,
                from_y: 7,
                to_x: 7,
                to_y: 7,
            },
            "input.mouse",
        ),
        (KeyboardType { text: s("text") }, "input.keyboard"),
        (KeyboardPress { key: s("key") }, "input.keyboard"),
        (KeyboardShortcut { keys: strings() }, "input.keyboard"),
        (
            ScrollWheel {
                direction: "up".into(),
                amount: 7,
            },
            "input.mouse",
        ),
        (
            ComputerAction {
                description: s("description"),
                max_steps: 7,
            },
            "computer.use",
        ),
        (
            AgentMessage {
                target_agent: s("target_agent"),
                message: s("message"),
            },
            "agent.message",
        ),
        (
            HitlRequest {
                question: s("question"),
                options: strings(),
            },
            "",
        ),
        (
            MemoryStore {
                key: s("key"),
                value: s("value"),
                memory_type: s("memory_type"),
            },
            "",
        ),
        (
            MemoryRecall {
                query: s("query"),
                memory_type: None,
            },
            "",
        ),
        (
            SendNotification {
                title: s("title"),
                body: s("body"),
                level: s("level"),
            },
            "",
        ),
        (
            CodeExecute {
                language: s("language"),
                code: s("code"),
                timeout_secs: None,
            },
            "process.exec",
        ),
        (Noop, ""),
        (
            SelfModifyDescription {
                new_description: s("new_description"),
            },
            "self.modify",
        ),
        (
            SelfModifyStrategy {
                strategy_key: s("strategy_key"),
                new_strategy: s("new_strategy"),
            },
            "self.modify",
        ),
        (
            CreateSubAgent {
                manifest_json: s("manifest_json"),
            },
            "self.modify",
        ),
        (
            DestroySubAgent {
                agent_id: s("agent_id"),
            },
            "self.modify",
        ),
        (
            RunEvolutionTournament {
                variants: strings(),
                task: s("task"),
                rounds: 7,
            },
            "self.modify",
        ),
        (
            ModifyGovernancePolicy {
                policy_key: s("policy_key"),
                policy_value: s("policy_value"),
            },
            "self.modify",
        ),
        (
            AllocateEcosystemFuel {
                agent_id: s("agent_id"),
                amount: 1.5,
            },
            "self.modify",
        ),
        (
            ModifyCognitiveParams {
                param_key: s("param_key"),
                param_value: s("param_value"),
            },
            "self.modify",
        ),
        (
            SelectLlmProvider {
                phase: s("phase"),
                provider: s("provider"),
                model: s("model"),
            },
            "self.modify",
        ),
        (
            SelectAlgorithm {
                algorithm: s("algorithm"),
                config_json: s("config_json"),
            },
            "self.modify",
        ),
        (
            DesignAgentEcosystem {
                ecosystem_json: s("ecosystem_json"),
            },
            "self.modify",
        ),
        (
            RunCounterfactual {
                decision_id: s("decision_id"),
                alternatives: strings(),
            },
            "self.modify",
        ),
        (
            TemporalPlan {
                immediate: s("immediate"),
                short_term: s("short_term"),
                medium_term: s("medium_term"),
                long_term: s("long_term"),
            },
            "self.modify",
        ),
        (
            A2aDelegation {
                agent_url: s("agent_url"),
                message: s("message"),
            },
            "a2a.delegate",
        ),
    ];
    let rows = rows();
    assert_eq!(cases.len(), 46);
    assert_eq!(rows.len(), cases.len());
    let mut seen = BTreeSet::new();
    for ((expected, capability), row) in cases.into_iter().zip(rows) {
        assert!(seen.insert(row.variant.clone()));
        assert_eq!(
            serde_json::to_vec(&expected).unwrap(),
            row.json.as_bytes(),
            "{}",
            row.variant
        );
        let decoded: PlannedAction = serde_json::from_str(&row.json).unwrap();
        // Debug is an existing derive, deliberately unchanged by extraction.
        // This also compares every decoded field to its independent construction.
        assert_eq!(format!("{decoded:?}"), format!("{expected:?}"));
        assert_eq!(expected.action_type(), row.label);
        assert_eq!(decoded.action_type(), row.label);
        let capabilities: Vec<&str> = if capability.is_empty() {
            vec![]
        } else {
            vec![capability]
        };
        assert_eq!(expected.required_capabilities(), capabilities);
        assert_eq!(decoded.required_capabilities(), capabilities);
        assert_eq!(
            serde_json::to_vec(&expected.clone()).unwrap(),
            row.json.as_bytes()
        );
    }
}

#[test]
fn screen_region_preserves_u32_bytes_and_refuses_every_out_of_range_field() {
    let region = ScreenRegion {
        x: u32::MAX,
        y: 65536,
        width: 0,
        height: u32::MAX,
    };
    let bytes = br#"{"x":4294967295,"y":65536,"width":0,"height":4294967295}"#;
    assert_eq!(serde_json::to_vec(&region).unwrap(), bytes);
    assert_eq!(
        serde_json::from_slice::<ScreenRegion>(bytes).unwrap(),
        region
    );
    assert_eq!(region.clone(), region);
    assert_eq!(
        format!("{region:?}"),
        "ScreenRegion { x: 4294967295, y: 65536, width: 0, height: 4294967295 }"
    );
    let action = PlannedAction::CaptureScreen {
        region: Some(region),
    };
    assert_eq!(
        serde_json::to_string(&action).unwrap(),
        r#"{"type":"CaptureScreen","region":{"x":4294967295,"y":65536,"width":0,"height":4294967295}}"#
    );
    for field in ["x", "y", "width", "height"] {
        for bad in [
            json!(4294967296u64),
            json!(-1),
            json!(1.5),
            json!(null),
            json!("1"),
        ] {
            let mut value = json!({"x":0,"y":0,"width":1,"height":1});
            value[field] = bad;
            assert!(
                serde_json::from_value::<ScreenRegion>(value.clone()).is_err(),
                "{value}"
            );
            assert!(serde_json::from_value::<PlannedAction>(
                json!({"type":"CaptureScreen","region":value})
            )
            .is_err());
        }
        let mut value = json!({"x":0,"y":0,"width":1,"height":1});
        value.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<ScreenRegion>(value).is_err());
    }
}

#[test]
fn string_or_vec_keeps_all_six_consumers_permissive_and_other_vectors_strict() {
    for (kind, field) in [
        ("LlmQuery", "context"),
        ("ShellCommand", "args"),
        ("DockerCommand", "args"),
        ("KnowledgeGraphUpdate", "entities"),
        ("KnowledgeGraphUpdate", "relationships"),
        ("HitlRequest", "options"),
    ] {
        let base: Value =
            serde_json::from_str(&rows().into_iter().find(|r| r.variant == kind).unwrap().json)
                .unwrap();
        for (input, expected) in [
            (json!(null), json!([])),
            (json!(""), json!([])),
            (json!("α\n"), json!(["α\n"])),
            (json!(-1.5), json!(["-1.5"])),
            (json!(true), json!(["true"])),
            (json!({"a":1}), json!(["{\"a\":1}"])),
            (
                json!(["",null,[1],{"a":1},false]),
                json!(["", "[1]", "{\"a\":1}", "false"]),
            ),
        ] {
            let mut value = base.clone();
            value[field] = input;
            let action: PlannedAction = serde_json::from_value(value).unwrap();
            assert_eq!(
                serde_json::to_value(action).unwrap()[field],
                expected,
                "{kind}.{field}"
            );
        }
        let mut value = base;
        value.as_object_mut().unwrap().remove(field);
        let action: PlannedAction = serde_json::from_value(value).unwrap();
        assert_eq!(serde_json::to_value(action).unwrap()[field], json!([]));
    }
    for (kind, field) in [
        ("KeyboardShortcut", "keys"),
        ("RunEvolutionTournament", "variants"),
        ("RunCounterfactual", "alternatives"),
    ] {
        let base: Value =
            serde_json::from_str(&rows().into_iter().find(|r| r.variant == kind).unwrap().json)
                .unwrap();
        for bad in [json!(null), json!("one"), json!([1])] {
            let mut value = base.clone();
            value[field] = bad;
            assert!(
                serde_json::from_value::<PlannedAction>(value).is_err(),
                "{kind}.{field}"
            );
        }
    }
}

#[test]
fn codecs_preserve_defaults_optionals_and_refuse_invalid_actions() {
    for json in [
        r#"{"type":"ApiCall","method":"GET","url":"u"}"#,
        r#"{"type":"CodeExecute","language":"bash","code":"x"}"#,
        r#"{"type":"CaptureScreen"}"#,
    ] {
        let action: PlannedAction = serde_json::from_str(json).unwrap();
        let value = serde_json::to_value(action).unwrap();
        for field in match value["type"].as_str().unwrap() {
            "ApiCall" => &["body", "headers"][..],
            "CodeExecute" => &["timeout_secs"][..],
            "CaptureScreen" => &["region"][..],
            _ => unreachable!(),
        } {
            assert_eq!(value[*field], Value::Null);
        }
    }
    for bad in [
        "null",
        "{}",
        r#"{"type":"noop"}"#,
        r#"{"type":"FileRead"}"#,
        r#"{"type":"LlmQuery","prompt":null}"#,
        r#"{"type":"CodeExecute","language":"x","code":"x","timeout_secs":4294967296}"#,
    ] {
        assert!(serde_json::from_str::<PlannedAction>(bad).is_err(), "{bad}");
    }
    for bad in [
        r#"{"kind":"WaitFor"}"#,
        r#"{"kind":"click"}"#,
        r#"{"kind":"wait_for","timeout_ms":18446744073709551616}"#,
        r#"{"kind":"wait_for","timeout_ms":-1}"#,
    ] {
        assert!(serde_json::from_str::<BrowserAction>(bad).is_err(), "{bad}");
    }
    let wait: BrowserAction = serde_json::from_str(r#"{"kind":"wait_for"}"#).unwrap();
    assert_eq!(
        serde_json::to_string(&wait).unwrap(),
        r#"{"kind":"wait_for","selector":null,"timeout_ms":null}"#
    );
    let api: PlannedAction = serde_json::from_str(
        r#"{"type":"ApiCall","method":"POST","url":"u","body":"α","headers":{"X-Test":"value"}}"#,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_string(&api).unwrap(),
        r#"{"type":"ApiCall","method":"POST","url":"u","body":"α","headers":{"X-Test":"value"}}"#
    );
}

/// Delete old-path checks with cognitive at K11 and computer_control at K13;
/// K22 forbids remaining old-path imports. No conversion or duplicate type.
#[test]
fn transitional_exports_are_the_same_types() {
    use nexus_kernel::cognitive::types::{
        BrowserAction as OldBrowser, PlannedAction as OldPlanned,
    };
    use nexus_kernel::cognitive::PlannedAction as CognitivePlanned;
    use nexus_kernel::computer_control::ScreenRegion as OldRegion;
    use std::any::TypeId;
    assert_eq!(TypeId::of::<BrowserAction>(), TypeId::of::<OldBrowser>());
    assert_eq!(TypeId::of::<PlannedAction>(), TypeId::of::<OldPlanned>());
    assert_eq!(
        TypeId::of::<PlannedAction>(),
        TypeId::of::<CognitivePlanned>()
    );
    assert_eq!(TypeId::of::<ScreenRegion>(), TypeId::of::<OldRegion>());
    let browser: OldBrowser = BrowserAction::Click {
        selector: "#x".into(),
    };
    let action: OldPlanned = PlannedAction::BrowserAutomate {
        start_url: "u".into(),
        actions: vec![browser],
        screenshot_dir: None,
    };
    let _: CognitivePlanned = action;
    let old: OldRegion = ScreenRegion {
        x: 0,
        y: 1,
        width: 2,
        height: 3,
    };
    let _: ScreenRegion = old;
}
