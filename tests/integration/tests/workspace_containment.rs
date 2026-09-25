//! Native agent filesystem adapters share the P0-002 workspace check.
use coding_agent::{CodingIoProxy, LocalCodingIo};
use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::{LlmProvider, LlmResponse};
use nexus_kernel::audit::AuditTrail;
use nexus_kernel::errors::AgentError;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tempfile::TempDir;
use uuid::Uuid;

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    outside: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("workspace");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "outside evidence").unwrap();
        Self {
            _temp: temp,
            root,
            outside,
        }
    }

    fn intact(&self) {
        assert_eq!(
            std::fs::read_to_string(self.outside.join("secret.txt")).unwrap(),
            "outside evidence"
        );
        assert_eq!(std::fs::read_dir(&self.outside).unwrap().count(), 1);
    }
}

struct FixedProvider {
    text: String,
    calls: Arc<AtomicUsize>,
}

#[cfg(unix)]
#[test]
fn coder_context_and_style_reads_reject_indexed_symlink_escape() {
    let f = Fixture::new();
    let indexed = f.root.join("lib.rs");
    std::fs::write(&indexed, "pub struct Fixture;").unwrap();
    let map = coder_agent::scanner::scan_project(&f.root).unwrap();
    assert_eq!(map.file_tree.len(), 1);
    std::fs::remove_file(&indexed).unwrap();
    std::os::unix::fs::symlink(f.outside.join("secret.txt"), &indexed).unwrap();
    let context = coder_agent::context::build_context(&map, "inspect lib");
    let style = coder_agent::writer::detect_style(&map);
    f.intact();
    assert!(matches!(context, Err(AgentError::CapabilityDenied(_))));
    assert!(matches!(style, Err(AgentError::CapabilityDenied(_))));
}

impl LlmProvider for FixedProvider {
    fn query(&self, _: &str, _: u32, model: &str) -> Result<LlmResponse, AgentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(LlmResponse {
            output_text: self.text.clone(),
            token_count: 10,
            model_name: model.into(),
            tool_calls: vec![],
            input_tokens: None,
        })
    }
    fn name(&self) -> &str {
        "mock-containment"
    }
    fn cost_per_token(&self) -> f64 {
        0.0
    }
}

#[test]
fn codegen_rejects_model_filename_escape_after_provider_call() {
    let f = Fixture::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = FixedProvider {
        text: "```text:../outside/secret.txt\nreplacement\n```".into(),
        calls: calls.clone(),
    };
    let mut gateway = GovernedLlmGateway::new(provider);
    let mut context = AgentRuntimeContext {
        agent_id: Uuid::new_v4(),
        capabilities: ["llm.query".into()].into_iter().collect(),
        fuel_remaining: 10000,
    };
    let result = coder_agent::llm_codegen::generate_code_with_llm(
        "write fixture",
        &f.root,
        &mut gateway,
        &mut context,
        "mock-test",
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    f.intact();
    assert!(
        matches!(result, Err(AgentError::CapabilityDenied(_))),
        "{result:?}"
    );
}

#[test]
fn conductor_design_and_general_outputs_cannot_escape() {
    use nexus_conductor::types::{AgentRole, PlannedTask};
    for role in [AgentRole::Designer, AgentRole::General] {
        let f = Fixture::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = FixedProvider {
            text: "```text:../outside/secret.txt\nreplacement\n```".into(),
            calls: calls.clone(),
        };
        let mut conductor = nexus_conductor::Conductor::new(provider, "mock-test");
        let task = PlannedTask {
            description: "write fixture".into(),
            role: role.clone(),
            capabilities_needed: vec!["llm.query".into()],
            estimated_fuel: 1000,
            depends_on: vec![],
            expected_outputs: vec![],
        };
        let mut audit = AuditTrail::new();
        let result = if role == AgentRole::Designer {
            conductor.execute_design_gen(&task, &f.root, &mut audit, Uuid::new_v4())
        } else {
            conductor.execute_general_task(&task, &f.root, &mut audit, Uuid::new_v4())
        };
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        f.intact();
        assert!(
            matches!(result, Err(AgentError::CapabilityDenied(_))),
            "{result:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn live_coding_io_denies_symlink_read_and_write() {
    let f = Fixture::new();
    std::os::unix::fs::symlink(&f.outside, f.root.join("escape")).unwrap();
    let mut io = LocalCodingIo::new(f.root.clone()).unwrap();
    let read = io.read_file("escape/secret.txt");
    let write = io.write_file("escape/secret.txt", "replacement");
    let create = io.write_file("escape/new/deep/file.txt", "replacement");
    f.intact();
    assert!(matches!(read, Err(AgentError::CapabilityDenied(_))));
    assert!(matches!(write, Err(AgentError::CapabilityDenied(_))));
    assert!(matches!(create, Err(AgentError::CapabilityDenied(_))));
    io.write_file("normal/deep/file.txt", "inside").unwrap();
    assert_eq!(io.read_file("normal/deep/file.txt").unwrap(), "inside");
}

#[cfg(unix)]
#[test]
fn preview_file_writer_denies_traversal_and_symlinks() {
    use web_builder_agent::dev_server::{DevServer, DevServerError};
    use web_builder_agent::react_gen::{ReactProject, ReactProjectFile};
    let f = Fixture::new();
    std::os::unix::fs::symlink(&f.outside, f.root.join("escape")).unwrap();
    let server = DevServer::new(f.root.clone());
    for path in [
        "../outside/secret.txt",
        "escape/secret.txt",
        "escape/new/file.txt",
    ] {
        let result = server.write_file(path, "replacement");
        f.intact();
        assert!(matches!(result, Err(DevServerError::GovernanceDenied(_))));
    }
    let project = ReactProject {
        files: vec![ReactProjectFile {
            path: "../outside/secret.txt".into(),
            content: "replacement".into(),
        }],
        project_name: "fixture".into(),
        template_id: "fixture".into(),
    };
    let result = DevServer::prepare(&project, &f.root);
    f.intact();
    assert!(matches!(result, Err(DevServerError::GovernanceDenied(_))));
    server.write_file("normal/deep/file.txt", "inside").unwrap();
    assert_eq!(
        std::fs::read_to_string(f.root.join("normal/deep/file.txt")).unwrap(),
        "inside"
    );
}
