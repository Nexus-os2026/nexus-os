//! Private, invocation-scoped authority for fresh Builder planning only.
//! Metadata and paths are never accepted as credentials. No authority lock is
//! held by this adapter across audit, provider calls, filesystem I/O or delivery.
use nexus_kernel::manifest::FsPermissionLevel;
use nexus_kernel::workspace::resolve_existing_relative;
use nexus_kernel::workspace_authority::{
    WorkspaceAuthorityRegistry, WorkspaceAuthoritySource, WorkspaceBinding, WorkspaceGrantId,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use uuid::Uuid;
use web_builder_agent::model_router::ModelSelection;
use web_builder_agent::plan::PlanResult;
use web_builder_agent::project::{create_project, transition, ProjectState, ProjectStatus};

// Audit payloads contain descriptive project IDs, never grants or private principals.
type Audit = Arc<dyn Fn(Value) + Send + Sync>;

#[derive(Debug)]
enum PlanningError {
    Authority(String),
    Persistence(String),
    Provider(String),
}
impl std::fmt::Display for PlanningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, detail) = match self {
            Self::Authority(s) => ("authority", s),
            Self::Persistence(s) => ("persistence", s),
            Self::Provider(s) => ("provider", s),
        };
        write!(f, "Builder planning {kind}: {detail}")
    }
}
type Result<T> = std::result::Result<T, PlanningError>;

pub(super) struct BuilderWorkspaceAuthority {
    registry: Arc<WorkspaceAuthorityRegistry>,
    root: PathBuf,
    allocator: Uuid,
    planner: Uuid,
}
impl BuilderWorkspaceAuthority {
    /// Trusted startup only. The existing identity-home policy rejects malformed
    /// HOME and uses the native Windows profile only when HOME is absent.
    pub(super) fn setup(
        registry: Arc<WorkspaceAuthorityRegistry>,
    ) -> std::result::Result<Arc<Self>, String> {
        let root = crate::oracle_runtime::default_identity_path_for("builds")
            .map_err(|e| format!("Builder storage setup: {e}"))?;
        Self::provision(registry, &root)
            .map(Arc::new)
            .map_err(|e| e.to_string())
    }

    fn provision(registry: Arc<WorkspaceAuthorityRegistry>, root: &Path) -> Result<Self> {
        if !root.is_absolute() {
            return Err(PlanningError::Authority("storage must be absolute".into()));
        }
        // The only recursive storage creation in this adapter is trusted setup.
        std::fs::create_dir_all(root)
            .map_err(|_| PlanningError::Persistence("storage provisioning failed".into()))?;
        let root = root
            .canonicalize()
            .map_err(|_| PlanningError::Authority("storage unavailable".into()))?;
        validate_root(&root)?;
        Ok(Self {
            registry,
            root,
            allocator: Uuid::new_v4(),
            planner: Uuid::new_v4(),
        })
    }

    fn begin(&self, audit: Audit) -> Result<PlanningExecution> {
        validate_root(&self.root)?;
        self.allocate(Uuid::new_v4(), Uuid::new_v4(), None, audit)
    }

    fn allocate(
        &self,
        project_id: Uuid,
        run_id: Uuid,
        expiry: Option<SystemTime>,
        audit: Audit,
    ) -> Result<PlanningExecution> {
        validate_root(&self.root)?;
        let allocator = WorkspaceBinding {
            agent_id: self.allocator,
            run_id,
        };
        let planner = WorkspaceBinding {
            agent_id: self.planner,
            run_id,
        };
        let allocation = self
            .registry
            .issue_trusted_root(
                &self.root,
                allocator,
                WorkspaceAuthoritySource::BackendAllocated,
                FsPermissionLevel::ReadWrite,
                expiry,
            )
            .map_err(|e| PlanningError::Authority(e.to_string()))?;
        // Arm before audit/allocation/narrowing so all subsequent exits revoke.
        let mut execution = PlanningExecution {
            registry: Arc::clone(&self.registry),
            project_id,
            allocator,
            planner,
            allocation,
            project: None,
            root: self.root.join(project_id.to_string()),
            audit,
            revoked: false,
        };
        execution.event("issue", "authorized");
        let allocated = (|| {
            let target = execution.authorize(
                Some(allocation),
                allocator,
                &self.root,
                Path::new(&project_id.to_string()),
                "allocate",
            )?;
            std::fs::create_dir(&target).map_err(|e| {
                PlanningError::Persistence(format!(
                    "exclusive project allocation failed: {}",
                    e.kind()
                ))
            })?;
            validate_root(&target)?;
            let canonical = target
                .canonicalize()
                .map_err(|_| PlanningError::Authority("project unavailable".into()))?;
            let child = self
                .registry
                .narrow(
                    allocation,
                    allocator,
                    self.planner,
                    &canonical,
                    FsPermissionLevel::ReadWrite,
                )
                .map_err(|e| PlanningError::Authority(e.to_string()))?;
            execution.root = canonical;
            execution.project = Some(child);
            execution.event("narrow", "authorized");
            Ok(())
        })();
        if let Err(error) = allocated {
            execution.revoke()?;
            return Err(error);
        }
        Ok(execution)
    }
}

fn validate_root(root: &Path) -> Result<()> {
    if !root.is_absolute() || !root.is_dir() || root.canonicalize().ok().as_deref() != Some(root) {
        return Err(PlanningError::Authority(
            "root missing or canonical identity changed".into(),
        ));
    }
    Ok(())
}

struct PlanningExecution {
    registry: Arc<WorkspaceAuthorityRegistry>,
    project_id: Uuid,
    allocator: WorkspaceBinding,
    planner: WorkspaceBinding,
    allocation: WorkspaceGrantId,
    project: Option<WorkspaceGrantId>,
    root: PathBuf,
    audit: Audit,
    revoked: bool,
}
impl PlanningExecution {
    fn event(&self, operation: &str, outcome: &str) {
        (self.audit)(json!({"operation": format!("builder.planning.{operation}"),
            "project_id": self.project_id.to_string(), "outcome": outcome}));
    }

    // Returned paths stay within the immediate mutation method, never a lease
    // passed to raw Builder persistence helpers or across provider execution.
    fn authorize(
        &self,
        id: Option<WorkspaceGrantId>,
        binding: WorkspaceBinding,
        root: &Path,
        relative: &Path,
        operation: &str,
    ) -> Result<PathBuf> {
        let checked = (|| {
            let id =
                id.ok_or_else(|| PlanningError::Authority("missing execution grant".into()))?;
            let grant = self
                .registry
                .resolve(id, binding)
                .map_err(|e| PlanningError::Authority(e.to_string()))?;
            if grant.permission() != &FsPermissionLevel::ReadWrite || grant.root() != root {
                return Err(PlanningError::Authority(
                    "mutation permission or root mismatch".into(),
                ));
            }
            validate_root(root)?;
            resolve_existing_relative(root, relative)
                .map_err(|_| PlanningError::Authority("relative target denied".into()))
        })();
        // Registry methods have returned; no registry/private state guard exists.
        self.event(
            operation,
            if checked.is_ok() {
                "authorized"
            } else {
                "denied"
            },
        );
        checked
    }

    fn project_target(&self, relative: &str, operation: &str) -> Result<PathBuf> {
        self.authorize(
            self.project,
            self.planner,
            &self.root,
            Path::new(relative),
            operation,
        )
    }

    fn provider_ready(&self) -> Result<()> {
        self.project_target("builder_state.json", "provider")?;
        Ok(())
    }

    fn create_artefacts(&self) -> Result<()> {
        let path = self.project_target("artefacts", "mkdir.artefacts")?;
        std::fs::create_dir(path).map_err(|e| {
            PlanningError::Persistence(format!("artefacts creation failed: {}", e.kind()))
        })
    }

    fn write(&self, relative: &str, bytes: &[u8]) -> Result<()> {
        let path = self.project_target(relative, relative)?;
        std::fs::write(path, bytes).map_err(|e| {
            PlanningError::Persistence(format!("{relative} write failed: {}", e.kind()))
        })
    }

    fn save_state(&self, state: &ProjectState) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(state)
            .map_err(|e| PlanningError::Persistence(e.to_string()))?;
        self.write("builder_state.json", &bytes)
    }

    fn persist_plan(&self, generated: &PlanResult, state: &mut ProjectState) -> Result<()> {
        let brief = serde_json::to_vec_pretty(&generated.plan.product_brief)
            .map_err(|e| PlanningError::Persistence(e.to_string()))?;
        let criteria = serde_json::to_vec_pretty(&generated.plan.acceptance_criteria)
            .map_err(|e| PlanningError::Persistence(e.to_string()))?;
        self.create_artefacts()?;
        self.write("artefacts/product_brief.json", &brief)?;
        self.write("artefacts/acceptance_criteria.json", &criteria)?;
        state.project_name = Some(generated.plan.product_brief.project_name.clone());
        state.plan_cost = generated.cost_usd;
        state.total_cost += generated.cost_usd;
        transition(state, ProjectStatus::Planned).map_err(PlanningError::Persistence)?;
        self.save_state(state)
    }

    fn revoke(&mut self) -> Result<()> {
        if self.revoked {
            return Ok(());
        }
        let result = self
            .registry
            .revoke(self.allocation, self.allocator)
            .map_err(|e| PlanningError::Authority(format!("cleanup failed: {e}")));
        if result.is_ok() {
            self.revoked = true;
        }
        self.event("revoke", if result.is_ok() { "revoked" } else { "denied" });
        result
    }
}
impl Drop for PlanningExecution {
    fn drop(&mut self) {
        if !self.revoked {
            // Audit may be a callback; never allow it to double-panic on unwind.
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.revoke()));
            if !matches!(outcome, Ok(Ok(()))) {
                eprintln!("Builder planning abnormal-exit revocation failed");
            }
        }
    }
}

struct GeneratedPlan {
    result: PlanResult,
    selection: ModelSelection,
}
struct CompletedPlan {
    project_id: String,
    project_dir: String,
    generated: GeneratedPlan,
}
impl CompletedPlan {
    fn into_json(self) -> Value {
        let r = self.generated.result;
        let m = self.generated.selection;
        json!({"project_id": self.project_id, "project_dir": self.project_dir,
            "plan": r.plan, "input_tokens": r.input_tokens, "output_tokens": r.output_tokens,
            "cost_usd": r.cost_usd, "elapsed_seconds": r.elapsed_seconds,
            "model": m.display_name, "model_id": m.model_id, "provider": m.provider.to_string(), "is_local": m.is_local})
    }
}

fn run_plan(
    authority: &BuilderWorkspaceAuthority,
    audit: Audit,
    prompt: &str,
    attempt: impl FnMut(bool) -> std::result::Result<GeneratedPlan, String>,
) -> Result<CompletedPlan> {
    let execution = authority.begin(Arc::clone(&audit)).inspect_err(|_| {
        audit(json!({"operation": "builder.planning.begin", "outcome": "denied"}));
    })?;
    finish_plan(execution, prompt, attempt)
}

fn finish_plan(
    mut execution: PlanningExecution,
    prompt: &str,
    mut attempt: impl FnMut(bool) -> std::result::Result<GeneratedPlan, String>,
) -> Result<CompletedPlan> {
    let result = (|| {
        let mut state = create_project(&execution.project_id.to_string(), prompt);
        execution.provider_ready()?;
        // Only these closures can produce provider errors. Authority and I/O
        // errors never enter this fallback branch.
        let generated = match attempt(false) {
            Ok(value) => Ok(value),
            Err(_) => {
                execution.provider_ready()?;
                attempt(true)
            }
        };
        match generated {
            Ok(generated) => {
                execution.persist_plan(&generated.result, &mut state)?;
                Ok(CompletedPlan {
                    project_id: execution.project_id.to_string(),
                    project_dir: execution.root.to_string_lossy().into_owned(),
                    generated,
                })
            }
            Err(error) => {
                state.error_message = Some(error.clone());
                transition(&mut state, ProjectStatus::PlanFailed)
                    .map_err(PlanningError::Persistence)?;
                execution.save_state(&state)?;
                Err(PlanningError::Provider(error))
            }
        }
    })();
    // Finalization precedes success, error delivery, and cost recording.
    execution.revoke()?;
    execution.event(
        "complete",
        if result.is_ok() {
            "succeeded"
        } else {
            "failed"
        },
    );
    result
}

pub(super) fn generate_plan(
    state: &crate::AppState,
    prompt: &str,
) -> std::result::Result<Value, String> {
    let authority = state.builder_workspace.as_ref().map_err(Clone::clone)?;
    let audit_state = state.clone();
    let audit: Audit = Arc::new(move |payload| {
        audit_state.log_event(
            Uuid::nil(),
            nexus_kernel::audit::EventType::UserAction,
            payload,
        );
    });
    let completed = run_plan(authority, audit, prompt, |fallback| {
        generate_with_provider(prompt, fallback)
    })
    .map_err(|e| e.to_string())?;
    // Existing global budget store is separate from project authority.
    web_builder_agent::plan::record_plan_cost(
        &completed.generated.result,
        &completed.generated.result.plan.product_brief.project_name,
    );
    Ok(completed.into_json())
}

fn generate_with_provider(
    prompt: &str,
    fallback: bool,
) -> std::result::Result<GeneratedPlan, String> {
    use nexus_connectors_llm::providers::{
        claude_code::ClaudeCodeProvider, codex_cli::CodexCliProvider, LlmProvider,
    };
    use web_builder_agent::model_router::*;
    let config = crate::load_config().map_err(|e| format!("config error: {e}"))?;
    let provider_config = crate::build_provider_config(&config);
    let (provider, selection): (Box<dyn LlmProvider>, ModelSelection) = if fallback {
        let selected = select_model(
            &BuilderTask::PlanGeneration,
            &RoutingBudget::from_budget_tracker(),
        );
        let provider: Box<dyn LlmProvider> = match selected.provider {
            ProviderType::Ollama => Box::new(crate::OllamaProvider::from_env()),
            ProviderType::Anthropic => {
                let (provider, _) = crate::provider_from_prefixed_model(
                    &format!("anthropic/{}", selected.model_id),
                    &provider_config,
                )?;
                provider
            }
            ProviderType::OpenAI => Box::new(crate::OpenAiProvider::new(
                provider_config.openai_api_key.clone(),
            )),
            ProviderType::CodexCli => Box::new(CodexCliProvider::new()),
            ProviderType::ClaudeCode => Box::new(ClaudeCodeProvider::new()),
        };
        (provider, selected)
    } else {
        let model = web_builder_agent::model_config::load_config().planning;
        let prefixed = web_builder_agent::model_config::to_prefixed_model(&model);
        let (provider, _) = crate::provider_from_prefixed_model(&prefixed, &provider_config)?;
        let selection = ModelSelection {
            provider: match model.provider.as_str() {
                "anthropic_api" | "anthropic" => ProviderType::Anthropic,
                "openai_api" | "openai" => ProviderType::OpenAI,
                "codex_cli" => ProviderType::CodexCli,
                "claude_cli" => ProviderType::ClaudeCode,
                _ => ProviderType::Ollama,
            },
            model_id: model.model_id,
            display_name: model.display_name,
            estimated_cost: 0.0,
            is_local: model.provider == "ollama",
        };
        (provider, selection)
    };
    let result = web_builder_agent::plan::generate_plan_with_model(
        provider.as_ref(),
        prompt,
        &selection.model_id,
    )?;
    Ok(GeneratedPlan { result, selection })
}

#[cfg(test)]
mod tests;
