//! Real Conductor mutation/fallback paths with disposable files and a scripted provider.
#![cfg(unix)]

use super::*;
use nexus_connectors_llm::providers::LlmResponse;
use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

struct Fixture {
    temp: PathBuf,
    workspace: PathBuf,
    outside: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = std::env::temp_dir().join(format!("nexus-p0-002a-{}", Uuid::new_v4()));
        std::fs::create_dir(&temp).unwrap();
        let workspace = temp.join("workspace");
        let outside = temp.join("outside");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(workspace.join("target.txt"), b"inside evidence").unwrap();
        std::fs::write(outside.join("target.txt"), b"outside evidence").unwrap();
        Self {
            temp,
            workspace,
            outside,
        }
    }

    fn outside_snapshot(&self) -> BTreeMap<OsString, (Option<PathBuf>, Vec<u8>)> {
        std::fs::read_dir(&self.outside)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                let path = entry.path();
                let link = std::fs::read_link(&path).ok();
                let bytes = if link.is_some() {
                    Vec::new()
                } else {
                    std::fs::read(&path).unwrap()
                };
                (entry.file_name(), (link, bytes))
            })
            .collect()
    }

    fn delete(&self, path: &str) -> Result<Vec<PathBuf>, AgentError> {
        Conductor::<ScriptedProvider>::apply_coder_changes(
            &self.workspace,
            &[CoderFileChange::Delete(path.into())],
            &mut AuditTrail::new(),
            Uuid::new_v4(),
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.temp);
    }
}

#[test]
fn p0_002a_conductor_denies_outside_entry_resolving_back_inside() {
    let f = Fixture::new();
    std::os::unix::fs::symlink(&f.outside, f.workspace.join("escape")).unwrap();
    std::os::unix::fs::symlink(f.workspace.join("target.txt"), f.outside.join("back")).unwrap();
    let before = f.outside_snapshot();
    let result = f.delete("escape/back");
    assert_eq!(f.outside_snapshot(), before);
    assert_eq!(
        std::fs::read(f.workspace.join("target.txt")).unwrap(),
        b"inside evidence"
    );
    assert!(
        matches!(result, Err(AgentError::CapabilityDenied(_))),
        "{result:?}"
    );
}

#[test]
fn p0_002a_conductor_denies_absolute_outside_leaf_link() {
    let f = Fixture::new();
    let back = f.outside.join("back");
    std::os::unix::fs::symlink(f.workspace.join("target.txt"), &back).unwrap();
    let before = f.outside_snapshot();
    let result = f.delete(back.to_str().unwrap());
    assert_eq!(f.outside_snapshot(), before);
    assert_eq!(
        std::fs::read(f.workspace.join("target.txt")).unwrap(),
        b"inside evidence"
    );
    assert!(
        matches!(result, Err(AgentError::CapabilityDenied(_))),
        "{result:?}"
    );
}

#[test]
fn p0_002a_conductor_unlinks_inside_leaf_without_following_it() {
    let f = Fixture::new();
    let before = f.outside_snapshot();
    for target in ["target.txt", "missing.txt"] {
        let link = f.workspace.join("link");
        std::os::unix::fs::symlink(f.outside.join(target), &link).unwrap();
        f.delete("link").unwrap();
        assert!(std::fs::symlink_metadata(link).is_err());
        assert_eq!(f.outside_snapshot(), before);
    }
}

#[test]
fn p0_002a_conductor_deletes_normal_inside_file() {
    let f = Fixture::new();
    let before = f.outside_snapshot();
    f.delete("target.txt").unwrap();
    assert!(!f.workspace.join("target.txt").exists());
    assert_eq!(f.outside_snapshot(), before);
}

struct ScriptedProvider {
    replies: Mutex<VecDeque<Result<String, AgentError>>>,
    calls: Arc<AtomicUsize>,
}

impl LlmProvider for ScriptedProvider {
    fn query(&self, _: &str, _: u32, model: &str) -> Result<LlmResponse, AgentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let output_text = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected provider/fallback invocation")?;
        Ok(LlmResponse {
            output_text,
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

fn conductor(
    replies: Vec<Result<String, AgentError>>,
) -> (Conductor<ScriptedProvider>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = ScriptedProvider {
        replies: Mutex::new(replies.into()),
        calls: calls.clone(),
    };
    (Conductor::new(provider, "mock-containment"), calls)
}

fn task() -> PlannedTask {
    PlannedTask {
        description: "write fixture".into(),
        role: AgentRole::Coder,
        capabilities_needed: vec!["llm.query".into()],
        estimated_fuel: 2000,
        depends_on: vec![],
        expected_outputs: vec![],
    }
}

fn escape_plan() -> String {
    r#"[{"filename":"../outside/target.txt","description":"fixture"}]"#.into()
}

#[test]
fn p0_002a_conductor_containment_denial_never_enters_fallback() {
    let f = Fixture::new();
    let before = f.outside_snapshot();
    let (mut conductor, calls) = conductor(vec![
        Ok(escape_plan()),
        Ok("replacement".into()),
        Ok("```text:fallback.txt\nfallback output\n```".into()),
    ]);
    let result = conductor.execute_code_gen(
        &task(),
        &f.workspace,
        &mut AuditTrail::new(),
        Uuid::new_v4(),
    );
    assert_eq!(f.outside_snapshot(), before);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "one plan and one content call, no retry"
    );
    assert_eq!(
        conductor.generation_fallbacks, 0,
        "neither provider nor template fallback may be entered"
    );
    assert_eq!(std::fs::read_dir(&f.workspace).unwrap().count(), 1);
    assert!(
        matches!(result, Err(AgentError::CapabilityDenied(_))),
        "{result:?}"
    );
}

#[test]
fn p0_002a_conductor_web_denial_never_enters_fallback() {
    let f = Fixture::new();
    std::os::unix::fs::symlink(f.outside.join("target.txt"), f.workspace.join("index.html"))
        .unwrap();
    let before = f.outside_snapshot();
    let (mut conductor, calls) = conductor(vec![Ok(
        "<!doctype html><html><body>fixture</body></html>".into(),
    )]);
    let result = conductor.execute_web_build(
        &task(),
        &f.workspace,
        &mut AuditTrail::new(),
        Uuid::new_v4(),
    );
    assert_eq!(f.outside_snapshot(), before);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(conductor.generation_fallbacks, 0);
    assert_eq!(std::fs::read_dir(&f.workspace).unwrap().count(), 2);
    assert!(
        matches!(result, Err(AgentError::CapabilityDenied(_))),
        "{result:?}"
    );
}

#[test]
fn p0_002a_conductor_streaming_denial_never_enters_fallback() {
    use nexus_connectors_llm::streaming::{
        new_usage_cell, StreamChunk, StreamingLlmProvider, StreamingResponse,
    };
    struct StreamProvider(AtomicUsize);
    impl StreamingLlmProvider for StreamProvider {
        fn stream_query(
            &self,
            _: &str,
            _: &str,
            _: u32,
            _: &str,
        ) -> Result<StreamingResponse, AgentError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(StreamingResponse::new(
                Box::new(std::iter::once(Ok(StreamChunk {
                    text: "<!doctype html><html><body>fixture</body></html>".into(),
                    token_count: Some(10),
                }))),
                new_usage_cell(),
            ))
        }
        fn streaming_provider_name(&self) -> &str {
            "mock-stream"
        }
    }
    let f = Fixture::new();
    std::os::unix::fs::symlink(f.outside.join("target.txt"), f.workspace.join("index.html"))
        .unwrap();
    let before = f.outside_snapshot();
    let stream = StreamProvider(AtomicUsize::new(0));
    let (mut conductor, calls) = conductor(vec![]);
    let result = conductor.execute_web_build_streaming(
        &task(),
        &f.workspace,
        &mut AuditTrail::new(),
        Uuid::new_v4(),
        &stream,
        &|_| {},
    );
    assert_eq!(f.outside_snapshot(), before);
    assert_eq!(stream.0.load(Ordering::SeqCst), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(conductor.generation_fallbacks, 0);
    assert_eq!(std::fs::read_dir(&f.workspace).unwrap().count(), 2);
    assert!(
        matches!(result, Err(AgentError::CapabilityDenied(_))),
        "{result:?}"
    );
}

#[test]
fn p0_002a_conductor_run_preserves_security_denial() {
    let f = Fixture::new();
    let before = f.outside_snapshot();
    let (mut conductor, calls) = conductor(vec![
        Ok(r#"[{"role":"coder","description":"write fixture","estimated_fuel":2000,"depends_on_indices":[]}]"#.into()),
        Ok(escape_plan()), Ok("replacement".into()),
    ]);
    let result = conductor.run(
        UserRequest::new("write fixture", f.workspace.to_str().unwrap()),
        &mut Supervisor::new(),
    );
    assert_eq!(f.outside_snapshot(), before);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(conductor.generation_fallbacks, 0);
    assert!(
        matches!(result, Err(AgentError::CapabilityDenied(_))),
        "{result:?}"
    );
}

#[test]
fn p0_002a_conductor_preserves_governance_denial_category() {
    for error in [
        AgentError::CapabilityDenied("fixture".into()),
        AgentError::ApprovalRequired {
            request_id: "fixture".into(),
        },
        AgentError::AdversarialBlock("fixture".into()),
    ] {
        let f = Fixture::new();
        let before = f.outside_snapshot();
        let (mut conductor, calls) = conductor(vec![Err(error.clone())]);
        let result = conductor.execute_code_gen(
            &task(),
            &f.workspace,
            &mut AuditTrail::new(),
            Uuid::new_v4(),
        );
        assert_eq!(result.unwrap_err(), error);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(conductor.generation_fallbacks, 0);
        assert_eq!(f.outside_snapshot(), before);
    }
}

#[test]
fn p0_002a_conductor_keeps_ordinary_provider_fallback() {
    let f = Fixture::new();
    let (mut conductor, calls) = conductor(vec![
        Err(AgentError::SupervisorError("provider unavailable".into())),
        Ok("```text:normal.txt\ninside output\n```".into()),
    ]);
    conductor
        .execute_code_gen(
            &task(),
            &f.workspace,
            &mut AuditTrail::new(),
            Uuid::new_v4(),
        )
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(conductor.generation_fallbacks, 1);
    assert_eq!(
        std::fs::read_to_string(f.workspace.join("normal.txt")).unwrap(),
        "inside output"
    );
}

#[test]
fn p0_002a_conductor_keeps_plan_parse_fallback() {
    let f = Fixture::new();
    let (mut conductor, calls) = conductor(vec![Ok("not JSON".into()), Ok("inside output".into())]);
    conductor
        .execute_code_gen(
            &task(),
            &f.workspace,
            &mut AuditTrail::new(),
            Uuid::new_v4(),
        )
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        std::fs::read_to_string(f.workspace.join("src/main.rs")).unwrap(),
        "inside output"
    );
}
