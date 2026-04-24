//! Swarm entry point for `coder-agent`.
//!
//! Implements [`SwarmAgentEntry`] so the swarm coordinator's Artisan
//! adapter can dispatch into this crate via a single
//! `CoderEntry::new().execute(input, ctx)` call. All LLM traffic flows
//! through `ctx.provider.invoke(...)`; tokens are recorded into
//! `ctx.budget`; cancellation is honoured between phases.
//!
//! # Input schema
//!
//! ```json
//! {
//!   "task": "Generate a Rust function that adds two integers",
//!   "files": [{ "path": "src/main.rs", "description": "entry point" }],
//!   "style": { "language": "rust", "conventions": ["clippy-clean"] },
//!   "output_dir": "/tmp/artisan-out",
//!   "max_tokens": 4096
//! }
//! ```
//!
//! `task` is required. Everything else is optional. When `output_dir` is
//! present, generated files are written to disk and their absolute paths
//! returned. When absent, file contents are returned inline.
//!
//! # Output schema
//!
//! ```json
//! {
//!   "files": [{ "path": "src/main.rs", "size_bytes": 142, "content": "..." }],
//!   "summary": "Generated 1 file from coder model gemma4:e2b.",
//!   "tokens_used": 152
//! }
//! ```
//!
//! # Phase events
//!
//! Six emissions per execution. Names are semantic, not generic:
//!
//! 1. `parsing_input`     — `{ task_chars }`
//! 2. `planning`          — `{ tool: "coder", language?, conventions? }`
//! 3. `generating`        — `{ model_id, max_tokens }` (just before the provider call)
//! 4. `parsing_response`  — `{ response_chars }` (just after the provider call)
//! 5. `writing_files`     — `{ count, output_dir? }` (only when files written)
//! 6. `complete`          — `{ files_count, tokens_used }`

use crate::llm_codegen::parse_multi_file_response;
use async_trait::async_trait;
use nexus_swarm_core::{AgentError, AgentExecutionContext, InvokeRequest, SwarmAgentEntry};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

/// Hint about an existing file the coder should consider when generating.
/// Currently advisory — the prompt mentions the path; Phase 4c may load
/// file contents into the prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRef {
    pub path: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodeStyle {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub conventions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoderInput {
    pub task: String,
    #[serde(default)]
    pub files: Option<Vec<FileRef>>,
    #[serde(default)]
    pub style: Option<CodeStyle>,
    #[serde(default)]
    pub output_dir: Option<PathBuf>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedFile {
    pub path: String,
    pub size_bytes: u64,
    /// Inline file contents. Always populated; redundant when
    /// `output_dir` was provided but useful for callers that prefer not
    /// to re-read from disk.
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoderOutput {
    pub files: Vec<GeneratedFile>,
    pub summary: String,
    pub tokens_used: u64,
}

const DEFAULT_MAX_TOKENS: u32 = 4096;

#[derive(Debug, Default, Clone)]
pub struct CoderEntry;

impl CoderEntry {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SwarmAgentEntry for CoderEntry {
    async fn execute(
        &self,
        input: Value,
        ctx: &AgentExecutionContext,
    ) -> Result<Value, AgentError> {
        // Phase 1: parse input.
        let parsed: CoderInput = serde_json::from_value(extract_node_inputs(input))
            .map_err(|e| AgentError::InvalidInput(format!("CoderInput parse: {e}")))?;
        if parsed.task.trim().is_empty() {
            return Err(AgentError::InvalidInput("task must not be empty".into()));
        }
        let task_chars = parsed.task.chars().count();
        ctx.emit
            .emit_phase("parsing_input", json!({ "task_chars": task_chars }))
            .await;
        if ctx.cancelled() {
            return Err(AgentError::Cancelled);
        }

        // Phase 2: planning. Surface the user-supplied style hints so
        // an observer can confirm the coder is honouring them.
        let language = parsed
            .style
            .as_ref()
            .and_then(|s| s.language.as_deref())
            .unwrap_or("rust");
        let conventions: Vec<String> = parsed
            .style
            .as_ref()
            .and_then(|s| s.conventions.clone())
            .unwrap_or_default();
        ctx.emit
            .emit_phase(
                "planning",
                json!({
                    "tool": "coder",
                    "language": language,
                    "conventions": conventions,
                    "file_hints": parsed.files.as_ref().map(|v| v.len()).unwrap_or(0),
                }),
            )
            .await;
        if ctx.cancelled() {
            return Err(AgentError::Cancelled);
        }

        let prompt = build_prompt(&parsed, language);
        let max_tokens = parsed.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);

        // Phase 3: generating. Emitted before the provider call so an
        // observer sees activity even if the call hangs.
        ctx.emit
            .emit_phase(
                "generating",
                json!({ "model_id": ctx.model_id, "max_tokens": max_tokens }),
            )
            .await;

        let resp = ctx
            .provider
            .invoke(InvokeRequest {
                model_id: ctx.model_id.clone(),
                prompt,
                max_tokens,
                temperature: Some(0.2),
                metadata: Value::Null,
            })
            .await?;

        if ctx.cancelled() {
            return Err(AgentError::Cancelled);
        }

        // Record per-node tokens immediately; the budget update fires
        // before the response-parsing phase so the UI reflects spend
        // even if downstream parsing fails.
        let tokens_used = u64::from(resp.tokens_in).saturating_add(u64::from(resp.tokens_out));
        let delta = {
            let mut guard = ctx.budget.lock().await;
            guard.record(tokens_used, resp.cost_cents);
            *guard
        };
        ctx.emit.emit_budget_update(delta).await;

        // Phase 4: parsing_response.
        let response_chars = resp.text.chars().count();
        ctx.emit
            .emit_phase(
                "parsing_response",
                json!({ "response_chars": response_chars }),
            )
            .await;

        let parsed_files = parse_multi_file_response(&resp.text);

        // Phase 5: writing_files. Only emitted when `output_dir` is
        // present — when it isn't, files are returned inline and we
        // skip filesystem side-effects entirely.
        let mut generated: Vec<GeneratedFile> = Vec::with_capacity(parsed_files.len());
        if let Some(out_dir) = parsed.output_dir.as_ref() {
            std::fs::create_dir_all(out_dir).map_err(|e| {
                AgentError::Internal(format!("create_dir_all {}: {e}", out_dir.display()))
            })?;
            for (filename, content) in &parsed_files {
                if filename.is_empty() || content.is_empty() {
                    continue;
                }
                let path = out_dir.join(filename);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        AgentError::Internal(format!("create dir for {filename}: {e}"))
                    })?;
                }
                std::fs::write(&path, content)
                    .map_err(|e| AgentError::Internal(format!("write {filename}: {e}")))?;
                generated.push(GeneratedFile {
                    path: path.to_string_lossy().into_owned(),
                    size_bytes: content.len() as u64,
                    content: content.clone(),
                });
            }
            ctx.emit
                .emit_phase(
                    "writing_files",
                    json!({
                        "count": generated.len(),
                        "output_dir": out_dir.to_string_lossy(),
                    }),
                )
                .await;
        } else {
            for (filename, content) in &parsed_files {
                if filename.is_empty() || content.is_empty() {
                    continue;
                }
                generated.push(GeneratedFile {
                    path: filename.clone(),
                    size_bytes: content.len() as u64,
                    content: content.clone(),
                });
            }
        }

        let summary = format!(
            "Generated {} file{} from model {}.",
            generated.len(),
            if generated.len() == 1 { "" } else { "s" },
            ctx.model_id,
        );

        // Phase 6: complete.
        ctx.emit
            .emit_phase(
                "complete",
                json!({
                    "files_count": generated.len(),
                    "tokens_used": tokens_used,
                }),
            )
            .await;

        let output = CoderOutput {
            files: generated,
            summary,
            tokens_used,
        };
        serde_json::to_value(output)
            .map_err(|e| AgentError::Internal(format!("serialize CoderOutput: {e}")))
    }
}

/// Inputs from the swarm coordinator are wrapped under `node_inputs` /
/// `route` keys (see `crates/nexus-swarm/src/coordinator.rs`); peel that
/// off so `CoderInput` deserializes cleanly. When `node_inputs` is
/// absent, treat the whole value as the input — that path is used by
/// direct unit tests that hand a raw `CoderInput` JSON.
fn extract_node_inputs(input: Value) -> Value {
    if let Value::Object(mut map) = input {
        if let Some(inner) = map.remove("node_inputs") {
            return inner;
        }
        return Value::Object(map);
    }
    input
}

fn build_prompt(input: &CoderInput, language: &str) -> String {
    let conventions = input
        .style
        .as_ref()
        .and_then(|s| s.conventions.as_ref())
        .map(|c| c.join(", "))
        .unwrap_or_else(|| "idiomatic, error-handled, commented".to_string());

    let file_hints = input
        .files
        .as_ref()
        .map(|fs| {
            fs.iter()
                .map(|f| match &f.description {
                    Some(d) => format!("- {} — {}", f.path, d),
                    None => format!("- {}", f.path),
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let file_hints_block = if file_hints.is_empty() {
        String::new()
    } else {
        format!("\nExisting files:\n{file_hints}\n")
    };

    format!(
        "You are an expert software engineer. Generate clean, production-ready code for the \
         task described. Return each file as a code block with the filename in the info string: \
         ```typescript:src/auth.ts or ```rust:src/main.rs. Include ALL necessary files. \
         Add error handling. Add comments.\n\n\
         Language: {language}\n\
         Conventions: {conventions}\n\
         {file_hints_block}\n\
         Task: {task}",
        task = input.task,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use nexus_swarm_core::emitter::recording::{Recorded, RecordingEmitter};
    use nexus_swarm_core::{
        CancelToken, CostClass, ModelDescriptor, NodeBudget, PrivacyClass, Provider,
        ProviderCapabilities, ProviderError, ProviderHealth, ProviderHealthStatus, ReasoningTier,
    };
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    struct FakeProvider {
        text: String,
        tokens_in: u32,
        tokens_out: u32,
    }

    impl Default for FakeProvider {
        fn default() -> Self {
            Self {
                text: "```rust:src/main.rs\nfn main() { println!(\"hi\"); }\n```\n".into(),
                tokens_in: 7,
                tokens_out: 5,
            }
        }
    }

    #[async_trait]
    impl Provider for FakeProvider {
        fn id(&self) -> &str {
            "fake"
        }
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                models: vec![ModelDescriptor {
                    id: "fake-small".into(),
                    param_count_b: Some(1),
                    tier: ReasoningTier::Light,
                    context_window: 4096,
                }],
                supports_tool_use: false,
                supports_streaming: false,
                max_context: 4096,
                cost_class: CostClass::Free,
                privacy_class: PrivacyClass::Public,
            }
        }
        async fn health_check(&self) -> ProviderHealth {
            ProviderHealth {
                provider_id: "fake".into(),
                status: ProviderHealthStatus::Ok,
                latency_ms: Some(1),
                models: vec!["fake-small".into()],
                notes: String::new(),
                checked_at_secs: 0,
            }
        }
        async fn invoke(
            &self,
            _req: InvokeRequest,
        ) -> Result<nexus_swarm_core::InvokeResponse, ProviderError> {
            Ok(nexus_swarm_core::InvokeResponse {
                text: self.text.clone(),
                tokens_in: self.tokens_in,
                tokens_out: self.tokens_out,
                cost_cents: 0,
                latency_ms: 0,
                model_id: "fake-small".into(),
            })
        }
    }

    fn mk_ctx(emitter: Arc<RecordingEmitter>, cancel: CancelToken) -> AgentExecutionContext {
        AgentExecutionContext {
            ticket_nonce: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
            node_id: "n-coder".into(),
            capability_id: "coder".into(),
            provider: Arc::new(FakeProvider::default()),
            model_id: "fake-small".into(),
            emit: emitter,
            cancel,
            budget: Arc::new(Mutex::new(NodeBudget::new())),
        }
    }

    #[tokio::test]
    async fn happy_path_returns_one_generated_file_with_inline_content() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec.clone(), CancelToken::new());
        let input = json!({
            "task": "write a hello world",
            "style": { "language": "rust" }
        });
        let out = CoderEntry::new().execute(input, &ctx).await.expect("ok");
        let parsed: CoderOutput = serde_json::from_value(out).expect("deserialize output");
        assert_eq!(parsed.files.len(), 1);
        assert_eq!(parsed.files[0].path, "src/main.rs");
        assert!(parsed.files[0].content.contains("println!"));
        assert_eq!(parsed.tokens_used, 12);
        assert!(parsed.summary.contains("fake-small"));
    }

    #[tokio::test]
    async fn invalid_input_returns_invalid_input_error() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new());
        // Missing the required `task` field.
        let input = json!({ "style": { "language": "rust" } });
        let err = CoderEntry::new()
            .execute(input, &ctx)
            .await
            .expect_err("expected InvalidInput");
        match err {
            AgentError::InvalidInput(msg) => assert!(msg.contains("CoderInput parse")),
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn empty_task_returns_invalid_input() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new());
        let input = json!({ "task": "   " });
        let err = CoderEntry::new()
            .execute(input, &ctx)
            .await
            .expect_err("expected InvalidInput on empty task");
        assert!(matches!(err, AgentError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn cancellation_before_provider_call_returns_cancelled() {
        let rec = Arc::new(RecordingEmitter::new());
        let cancel = CancelToken::new();
        let ctx = mk_ctx(rec.clone(), cancel.clone());
        cancel.cancel();
        let input = json!({ "task": "write hello world" });
        let err = CoderEntry::new()
            .execute(input, &ctx)
            .await
            .expect_err("expected Cancelled");
        assert!(matches!(err, AgentError::Cancelled));
    }

    #[tokio::test]
    async fn emits_six_phases_in_order_when_no_output_dir() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec.clone(), CancelToken::new());
        let input = json!({ "task": "write hello world" });
        CoderEntry::new().execute(input, &ctx).await.expect("ok");
        let log = rec.snapshot().await;
        let phases: Vec<&str> = log
            .iter()
            .filter_map(|r| match r {
                Recorded::Phase { phase, .. } => Some(phase.as_str()),
                _ => None,
            })
            .collect();
        // No output_dir → no `writing_files` phase. 5 phases total.
        assert_eq!(
            phases,
            vec![
                "parsing_input",
                "planning",
                "generating",
                "parsing_response",
                "complete"
            ],
            "phases out of order or missing: {phases:?}"
        );
        // Exactly one budget emission.
        let budgets: Vec<&NodeBudget> = log
            .iter()
            .filter_map(|r| match r {
                Recorded::Budget { delta } => Some(delta),
                _ => None,
            })
            .collect();
        assert_eq!(budgets.len(), 1);
        assert_eq!(budgets[0].tokens_consumed, 12);
    }
}
