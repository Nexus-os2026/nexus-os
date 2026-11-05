//! Today's compact JSON bytes are frozen, not reconstructed as sorted Values.
//! SHA256 here witnesses the exact fixture preimages; it introduces no runtime
//! hash domain. UUID raw bytes additionally pin WorkspaceBinding::from_bytes.
#[allow(dead_code)]
mod retirement;
use nexus_kernel::audit::AuditError;
use nexus_kernel::backup::BackupScheduleConfig;
use nexus_kernel::cognitive::types::{BrowserAction, PlannedAction};
use nexus_kernel::computer_control::ScreenRegion;
use nexus_kernel::errors::{on_error, AgentError, ErrorStrategy};
use nexus_kernel::fuel_hardening::FuelViolation;
use nexus_kernel::lifecycle::AgentState;
use nexus_kernel::manifest::FsPermissionLevel;
use nexus_kernel::rate_limit::{ApiHardeningConfig, RateLimitConfig};
use nexus_kernel::supervisor::AgentId;
use nexus_kernel::workspace_authority::WorkspaceBinding;
use retirement::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Deserialize)]
struct ActionFixture {
    variant: String,
    label: String,
    json: String,
    sha256: String,
}
#[derive(Deserialize)]
struct ByteFixture {
    name: String,
    preimage: String,
    sha256: String,
}
fn bytes(name: &str) -> String {
    let rows: Vec<ByteFixture> = include_str!("retirement/bytes.jsonl")
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let row = rows.iter().find(|r| r.name == name).unwrap();
    let raw = if name == "agent_bytes" {
        hex::decode(&row.preimage).unwrap()
    } else {
        row.preimage.as_bytes().to_vec()
    };
    assert_eq!(digest(&raw), row.sha256);
    row.preimage.clone()
}
fn roundtrip<T: DeserializeOwned + Serialize>(json: &str) -> T {
    let value: T = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_vec(&value).unwrap(), json.as_bytes());
    value
}

// Exhaustive compiler-checked inventory independent of action_type() and fixtures.
fn variant(a: &PlannedAction) -> &'static str {
    use PlannedAction::*;
    match a {
        LlmQuery { .. } => "LlmQuery",
        FileRead { .. } => "FileRead",
        FileWrite { .. } => "FileWrite",
        ShellCommand { .. } => "ShellCommand",
        DockerCommand { .. } => "DockerCommand",
        WebSearch { .. } => "WebSearch",
        WebFetch { .. } => "WebFetch",
        ApiCall { .. } => "ApiCall",
        ImageGenerate { .. } => "ImageGenerate",
        TextToSpeech { .. } => "TextToSpeech",
        KnowledgeGraphUpdate { .. } => "KnowledgeGraphUpdate",
        KnowledgeGraphQuery { .. } => "KnowledgeGraphQuery",
        BrowserAutomate { .. } => "BrowserAutomate",
        CaptureScreen { .. } => "CaptureScreen",
        CaptureWindow { .. } => "CaptureWindow",
        AnalyzeScreen { .. } => "AnalyzeScreen",
        MouseMove { .. } => "MouseMove",
        MouseClick { .. } => "MouseClick",
        MouseDoubleClick { .. } => "MouseDoubleClick",
        MouseDrag { .. } => "MouseDrag",
        KeyboardType { .. } => "KeyboardType",
        KeyboardPress { .. } => "KeyboardPress",
        KeyboardShortcut { .. } => "KeyboardShortcut",
        ScrollWheel { .. } => "ScrollWheel",
        ComputerAction { .. } => "ComputerAction",
        AgentMessage { .. } => "AgentMessage",
        HitlRequest { .. } => "HitlRequest",
        MemoryStore { .. } => "MemoryStore",
        MemoryRecall { .. } => "MemoryRecall",
        SendNotification { .. } => "SendNotification",
        CodeExecute { .. } => "CodeExecute",
        Noop => "Noop",
        SelfModifyDescription { .. } => "SelfModifyDescription",
        SelfModifyStrategy { .. } => "SelfModifyStrategy",
        CreateSubAgent { .. } => "CreateSubAgent",
        DestroySubAgent { .. } => "DestroySubAgent",
        RunEvolutionTournament { .. } => "RunEvolutionTournament",
        ModifyGovernancePolicy { .. } => "ModifyGovernancePolicy",
        AllocateEcosystemFuel { .. } => "AllocateEcosystemFuel",
        ModifyCognitiveParams { .. } => "ModifyCognitiveParams",
        SelectLlmProvider { .. } => "SelectLlmProvider",
        SelectAlgorithm { .. } => "SelectAlgorithm",
        DesignAgentEcosystem { .. } => "DesignAgentEcosystem",
        RunCounterfactual { .. } => "RunCounterfactual",
        TemporalPlan { .. } => "TemporalPlan",
        A2aDelegation { .. } => "A2aDelegation",
    }
}

#[test]
fn all_46_actions_keep_json_labels_and_hash_input_bytes() {
    let mut seen = BTreeSet::new();
    for line in include_str!("retirement/actions.jsonl").lines() {
        let row: ActionFixture = serde_json::from_str(line).unwrap();
        let action: PlannedAction = roundtrip(&row.json);
        assert_eq!(variant(&action), row.variant);
        assert_eq!(action.action_type(), row.label);
        assert!(seen.insert(row.variant));
        assert_eq!(digest(&serde_json::to_vec(&action).unwrap()), row.sha256);
    }
    assert_eq!(seen.len(), 46);
    // Mutations must fail the same byte oracle, including a preimage-only change.
    let row: ActionFixture = serde_json::from_str(
        include_str!("retirement/actions.jsonl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_ne!(digest(format!("{}\n", row.json).as_bytes()), row.sha256);
    assert_ne!(row.json.replace("fixture-α", "mutant"), row.json);
    assert_ne!(
        digest(row.json.replace("fixture-α", "mutant").as_bytes()),
        row.sha256
    );
}

#[test]
fn browser_actions_keep_all_tags_fields_and_u64_timeout() {
    let mut kinds = BTreeSet::new();
    for json in include_str!("retirement/browser.jsonl").lines() {
        let _: BrowserAction = roundtrip(json);
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        kinds.insert(v["kind"].as_str().unwrap().to_owned());
    }
    assert_eq!(
        kinds,
        BTreeSet::from(
            [
                "navigate",
                "click",
                "fill",
                "press",
                "wait_for",
                "extract_text"
            ]
            .map(str::to_owned)
        )
    );
    assert!(serde_json::from_str::<BrowserAction>(r#"{"kind":"WaitFor"}"#).is_err());
}

#[test]
fn uuid_permission_region_and_binding_identity_are_unchanged() {
    let agent: AgentId = roundtrip(&bytes("agent_id"));
    let raw: [u8; 16] = hex::decode(bytes("agent_bytes"))
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(agent.as_bytes(), &raw);
    let run = [0x42; 16];
    let binding = WorkspaceBinding::from_bytes(raw, run);
    assert_eq!(binding.agent_id, agent);
    assert_eq!(binding.run_id.as_bytes(), &run);
    assert_ne!(binding, WorkspaceBinding::from_bytes(run, raw));
    for (value, json) in [
        (FsPermissionLevel::ReadOnly, "\"ReadOnly\""),
        (FsPermissionLevel::ReadWrite, "\"ReadWrite\""),
        (FsPermissionLevel::Deny, "\"Deny\""),
    ] {
        assert_eq!(roundtrip::<FsPermissionLevel>(json), value);
    }
    let region: ScreenRegion = roundtrip(&bytes("region"));
    assert_eq!(region.x, u32::MAX);
    assert_eq!(region.y, 65536);
    assert!(
        serde_json::from_str::<ScreenRegion>(r#"{"x":4294967296,"y":0,"width":1,"height":1}"#)
            .is_err()
    );
    assert!(serde_json::from_str::<FsPermissionLevel>("\"read_only\"").is_err());
}

#[test]
fn permissive_vector_codec_keeps_malformed_null_scalar_and_missing_cases() {
    for (kind, field) in [
        ("LlmQuery", "context"),
        ("ShellCommand", "args"),
        ("DockerCommand", "args"),
        ("KnowledgeGraphUpdate", "entities"),
        ("KnowledgeGraphUpdate", "relationships"),
        ("HitlRequest", "options"),
    ] {
        let row: ActionFixture = include_str!("retirement/actions.jsonl")
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .find(|r: &ActionFixture| r.variant == kind)
            .unwrap();
        for (input, expected) in [
            ("null", "[]"),
            ("\"\"", "[]"),
            ("\"one\"", "[\"one\"]"),
            ("123", "[\"123\"]"),
            ("true", "[\"true\"]"),
            ("{}", "[\"{}\"]"),
            ("[\"a\",null,2,false,{}]", "[\"a\",\"2\",\"false\",\"{}\"]"),
        ] {
            let mut v: serde_json::Value = serde_json::from_str(&row.json).unwrap();
            v[field] = serde_json::from_str(input).unwrap();
            let a: PlannedAction = serde_json::from_value(v).unwrap();
            let out = serde_json::to_value(a).unwrap();
            assert_eq!(
                out[field],
                serde_json::from_str::<serde_json::Value>(expected).unwrap()
            );
        }
        let mut v: serde_json::Value = serde_json::from_str(&row.json).unwrap();
        v.as_object_mut().unwrap().remove(field);
        let a: PlannedAction = serde_json::from_value(v).unwrap();
        assert_eq!(
            serde_json::to_value(a).unwrap()[field],
            serde_json::json!([])
        );
    }
    for bad in [
        "null",
        "{}",
        r#"{"type":"Unknown"}"#,
        r#"{"type":"LlmQuery","prompt":null}"#,
        r#"{"type":"LlmQuery","context":[}"#,
    ] {
        assert!(serde_json::from_str::<PlannedAction>(bad).is_err(), "{bad}");
    }
}

#[test]
fn config_defaults_keep_bytes_and_serde_missing_field_defaults() {
    for (actual, expected) in [
        (
            serde_json::to_string(&RateLimitConfig::default()).unwrap(),
            bytes("rate"),
        ),
        (
            serde_json::to_string(&ApiHardeningConfig::default()).unwrap(),
            bytes("api"),
        ),
    ] {
        assert_eq!(actual, expected);
        assert_ne!(
            digest(format!("{actual} ").as_bytes()),
            digest(expected.as_bytes())
        );
    }
    assert_eq!(
        serde_json::to_string(&serde_json::from_str::<RateLimitConfig>("{}").unwrap()).unwrap(),
        bytes("rate")
    );
    assert_eq!(
        serde_json::to_string(&serde_json::from_str::<ApiHardeningConfig>("{}").unwrap()).unwrap(),
        bytes("api")
    );
}

#[test]
fn backup_default_home_child() {
    let Ok(case) = std::env::var("K01_BACKUP_HOME_CASE") else {
        return;
    };
    let expected_dir = if case == "valid" {
        std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
            .join(".local/share/nexus-os/backups")
            .to_string_lossy()
            .into_owned()
    } else {
        String::new()
    };
    let expected = bytes("backup").replace(
        "\"<HOME>/.local/share/nexus-os/backups\"",
        &serde_json::to_string(&expected_dir).unwrap(),
    );
    assert_eq!(
        serde_json::to_string(&BackupScheduleConfig::default()).unwrap(),
        expected
    );
    assert_eq!(
        serde_json::to_string(&serde_json::from_str::<BackupScheduleConfig>("{}").unwrap())
            .unwrap(),
        expected
    );
}

#[test]
fn backup_defaults_use_validated_home_without_process_global_env_races() {
    let home = tempfile::tempdir().unwrap();
    for (case, value) in [
        ("valid", home.path().as_os_str()),
        ("empty", std::ffi::OsStr::new("")),
        ("relative", std::ffi::OsStr::new("relative")),
    ] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "backup_default_home_child", "--nocapture"])
            .env("K01_BACKUP_HOME_CASE", case)
            .env("HOME", value)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{case}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    }
}

#[test]
fn compatibility_states_violations_and_every_error_display_stay_exact() {
    use AgentState::*;
    for (state, text) in [
        (Created, "Created"),
        (Starting, "Starting"),
        (Running, "Running"),
        (Paused, "Paused"),
        (Stopping, "Stopping"),
        (Stopped, "Stopped"),
        (Destroyed, "Destroyed"),
    ] {
        assert_eq!(state.to_string(), text);
        assert_eq!(roundtrip::<AgentState>(&format!("\"{text}\"")), state);
    }
    for (violation, text) in [
        (FuelViolation::OverMonthlyCap, "OverMonthlyCap"),
        (FuelViolation::AnomalyDetected, "AnomalyDetected"),
        (
            FuelViolation::ProviderTokenMismatch,
            "ProviderTokenMismatch",
        ),
    ] {
        assert_eq!(
            roundtrip::<FuelViolation>(&format!("\"{text}\"")),
            violation
        );
        let e = AgentError::FuelViolation {
            violation,
            reason: "fixture".into(),
        };
        assert_eq!(e.to_string(), format!("fuel violation '{text}': fixture"));
        assert_eq!(on_error(&e), ErrorStrategy::Escalate);
    }
    for (e, text, strategy) in [
        (AgentError::FuelExhausted, "fuel budget exhausted", ErrorStrategy::Escalate),
        (AgentError::InvalidTransition { from: Created, to: Paused }, "invalid state transition from 'Created' to 'Paused'", ErrorStrategy::Skip),
        (AgentError::ManifestError("fixture".into()), "manifest error: fixture", ErrorStrategy::Skip),
        (AgentError::CapabilityDenied("fixture".into()), "capability denied: 'fixture' is not allowed", ErrorStrategy::Escalate),
        (AgentError::ApprovalRequired { request_id: "fixture".into() }, "approval required: request_id='fixture'", ErrorStrategy::Escalate),
        (AgentError::SupervisorError("fixture".into()), "supervisor error: fixture", ErrorStrategy::Retry { max_attempts: 3 }),
        (AgentError::KeyDestroyed("fixture".into()), "key 'fixture' has been destroyed", ErrorStrategy::Escalate),
        (AgentError::AdversarialBlock("fixture".into()), "adversarial challenge failed: fixture", ErrorStrategy::Escalate),
        (AgentError::AuditFailure(AuditError::BatcherPoisoned), "audit failure: audit batcher mutex poisoned — audit integrity compromised", ErrorStrategy::Escalate),
        (AgentError::AuditFailure(AuditError::SerializationFailed), "audit failure: audit event serialization failed — fail-closed, no hash without payload", ErrorStrategy::Escalate),
    ] {
        assert_eq!(e.to_string(), text);
        assert_eq!(on_error(&e), strategy);
    }
}

#[test]
fn every_persisted_source_kind_name_stays_exact() {
    use nexus_kernel::hm::SourceKind;
    use SourceKind::*;
    // Exhaustive match forces a conscious golden update if the enum grows.
    let expected = |kind| match kind {
        MemoryStoreAction => "memory_store_action",
        LoopEpisodic => "loop_episodic",
        EvolutionProcedural => "evolution_procedural",
        StrategyScore => "strategy_score",
        CrossAgentLearning => "cross_agent_learning",
        GoalRecord => "goal_record",
        OptimizedPrompt => "optimized_prompt",
        PlannerStrategyInjection => "planner_strategy_injection",
        CognitiveParam => "cognitive_param",
        ModelMapping => "model_mapping",
        DbMemoryStore => "db_memory_store",
        KnowledgeGraphReplace => "knowledge_graph_replace",
    };
    for kind in [
        MemoryStoreAction,
        LoopEpisodic,
        EvolutionProcedural,
        StrategyScore,
        CrossAgentLearning,
        GoalRecord,
        OptimizedPrompt,
        PlannerStrategyInjection,
        CognitiveParam,
        ModelMapping,
        DbMemoryStore,
        KnowledgeGraphReplace,
    ] {
        assert_eq!(kind.name(), expected(kind), "{kind:?}");
    }
}
