//! Phase Three: governed real-world control in the desktop.
//!
//! The desktop holds one `GovernedControl` (crate `nexus-governed-control`)
//! and adds only what the desktop owns: the owner's native dialogs (the
//! only production `ControlConfirmer`), the hash-chained audit trail as the
//! evidence sink (evidence first: an unrecorded effect does not start), the
//! vault as the credential source, the `p3_*` IPC commands, and the agent
//! executor's bridge. Every effect still goes through the crate's one
//! pipeline; nothing here performs one.

use crate::AppState;
use nexus_governed_control::authority::approval::{
    ActionConfirmation, ControlConfirmer, GrantConfirmation, ResumeConfirmation,
};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::commitment::{CommitmentState, CommitmentView};
use nexus_governed_control::authority::evidence::{
    EvidenceRecord, EvidenceSink, EvidenceUnavailable, MemoryEvidence, TeeEvidence,
};
use nexus_governed_control::authority::ids::{AgentId, CommitmentId, GrantId, RunId};
use nexus_governed_control::authority::run::RunOrigin;
use nexus_governed_control::broker::Vault;
use nexus_governed_control::control::EffectOutput;
use nexus_governed_control::governed::{AgentOutcome, GovernedControl, Intent};
use nexus_governed_control::ingress::{understand, Attachments, CommandEnvelope, Understood};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_persistence::NexusDatabase;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// The most output text one effect returns to the interface or an agent.
const MAX_RETURNED_TEXT: usize = 64 * 1024;

/// Phase Three in the desktop.
pub(crate) struct RealWorld {
    control: GovernedControl,
    recent: Arc<MemoryEvidence>,
    attachments: Attachments,
    /// Each agent's runs (ended ones forgotten as new ones arrive), so that
    /// the owner's stop of an agent cancels what it left running or waiting.
    agent_runs: Mutex<HashMap<String, Vec<RunId>>>,
}

/// Evidence into the shared hash-chained audit trail (and its persisted
/// table). A record that cannot be appended refuses the effect.
struct AuditEvidence {
    audit: Arc<Mutex<AuditTrail>>,
    db: Arc<NexusDatabase>,
}

impl EvidenceSink for AuditEvidence {
    fn record(&self, record: &EvidenceRecord) -> Result<(), EvidenceUnavailable> {
        crate::append_audit_event_checked(
            &self.audit,
            &self.db,
            uuid::Uuid::nil(),
            EventType::StateChange,
            record.to_json(),
        )
        .map_err(|_| EvidenceUnavailable)
    }
}

impl RealWorld {
    /// The production control: its runtime root under the Nexus state
    /// directory, the vault, the audit trail.
    pub(crate) fn setup(
        audit: Arc<Mutex<AuditTrail>>,
        db: Arc<NexusDatabase>,
    ) -> Result<Arc<Self>, String> {
        let root = nexus_kernel::identity_home::nexus_state_dir()
            .map_err(|e| format!("governed control: {e}"))?
            .join("p3-runtime");
        let recent = Arc::new(MemoryEvidence::new(512));
        let sink = Arc::new(TeeEvidence(vec![
            Arc::new(AuditEvidence { audit, db }),
            recent.clone(),
        ]));
        let control =
            GovernedControl::new(&root, Vault::Kernel, sink, Arc::new(SystemClock::default()))
                .map_err(|e| e.to_string())?;
        Ok(Arc::new(Self {
            control,
            recent,
            attachments: Attachments::default(),
            agent_runs: Mutex::new(HashMap::new()),
        }))
    }

    /// An isolated control for tests: a temporary root, the given (in
    /// memory) audit trail and database, no vault.
    #[cfg(test)]
    #[cfg(target_os = "linux")]
    pub(crate) fn for_tests(
        root: &std::path::Path,
        audit: Arc<Mutex<AuditTrail>>,
        db: Arc<NexusDatabase>,
    ) -> Arc<Self> {
        let recent = Arc::new(MemoryEvidence::new(512));
        let sink = Arc::new(TeeEvidence(vec![
            Arc::new(AuditEvidence { audit, db }),
            recent.clone(),
        ]));
        let control = GovernedControl::new(
            root,
            Vault::Disabled,
            sink,
            Arc::new(SystemClock::default()),
        )
        .expect("a test control");
        Arc::new(Self {
            control,
            recent,
            attachments: Attachments::default(),
            agent_runs: Mutex::new(HashMap::new()),
        })
    }

    /// The owner's command: understood strictly, committed, not yet run.
    pub(crate) fn submit(&self, envelope: CommandEnvelope) -> Result<Value, String> {
        let intent = match understand(&envelope, &self.attachments).map_err(|e| e.to_string())? {
            Understood::Intent(intent) => intent,
            Understood::NotUnderstood(why) => {
                return Ok(json!({ "understood": false, "reason": why }));
            }
        };
        let agent = AgentId::owner_session();
        let run = self
            .control
            .open_run(
                agent.clone(),
                RunOrigin::Command {
                    modalities: envelope.modalities(),
                },
            )
            .map_err(|e| e.to_string())?;
        match self.control.propose(&agent, run, &intent) {
            Ok(view) => {
                // The command's run ends with its commitment: approved and
                // run, denied, or expired if the owner never answers.
                self.control.finish_when_settled(run);
                Ok(json!({ "understood": true, "commitment": view_json(&view) }))
            }
            Err(error) => {
                self.control.finish_run(run);
                Err(error.to_string())
            }
        }
    }

    fn view(&self, commitment: &str) -> Result<CommitmentView, String> {
        let id = CommitmentId::parse(commitment).ok_or("governed control: unknown commitment")?;
        self.control
            .authority()
            .commitments()
            .view(id)
            .ok_or_else(|| "governed control: unknown commitment".to_string())
    }

    /// Authorize (R2: the owner's native approval of exactly this
    /// commitment) and run it once.
    pub(crate) fn approve(
        &self,
        commitment: &str,
        confirmer: &dyn ControlConfirmer,
    ) -> Result<Value, String> {
        let view = self.view(commitment)?;
        let result = self
            .control
            .authorize(view.id, &view.agent, view.run, confirmer)
            .and_then(|()| self.control.execute(view.id, &view.agent, view.run));
        self.settle(&view);
        result
            .map(|output| output_json(&output))
            .map_err(|e| e.to_string())
    }

    pub(crate) fn deny(&self, commitment: &str) -> Result<(), String> {
        let view = self.view(commitment)?;
        let result = self.control.deny(view.id, &view.agent, view.run);
        self.settle(&view);
        result.map_err(|e| e.to_string())
    }

    /// An owner command's run ends with its one commitment.
    fn settle(&self, view: &CommitmentView) {
        if view.agent == AgentId::owner_session() {
            self.control.finish_run(view.run);
        }
    }

    /// Remember `run` as one of `agent`'s (ended runs are forgotten here).
    fn track_agent_run(&self, agent: &str, run: RunId) {
        let registry = self.control.authority().runs();
        let mut map = self.agent_runs.lock().unwrap_or_else(|p| p.into_inner());
        for runs in map.values_mut() {
            runs.retain(|r| {
                registry
                    .view(*r)
                    .is_some_and(|view| !view.cancelled && !view.finished)
            });
        }
        map.retain(|_, runs| !runs.is_empty());
        map.entry(agent.to_string()).or_default().push(run);
    }

    /// The owner stopped `agent`: its Phase Three runs are cancelled, so
    /// whatever of them still runs sees the cancellation and nothing it left
    /// waiting can still be approved.
    pub(crate) fn cancel_agent(&self, agent: &str) {
        let runs = self
            .agent_runs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(agent)
            .unwrap_or_default();
        for run in runs {
            let _ = self.control.cancel_run(run);
        }
    }

    pub(crate) fn cancel_run(&self, run: &str) -> Result<(), String> {
        let run = RunId::parse(run).ok_or("governed control: unknown run")?;
        self.control.cancel_run(run).map_err(|e| e.to_string())
    }

    pub(crate) fn emergency_stop(&self) -> usize {
        self.control.emergency_stop()
    }

    pub(crate) fn resume(&self, confirmer: &dyn ControlConfirmer) -> Result<(), String> {
        self.control.resume(confirmer).map_err(|e| e.to_string())
    }

    pub(crate) fn request_grant(
        &self,
        request: &nexus_governed_control::governed::GrantRequest,
        ttl_secs: u64,
        confirmer: &dyn ControlConfirmer,
    ) -> Result<String, String> {
        self.control
            .request_grant(request, std::time::Duration::from_secs(ttl_secs), confirmer)
            .map(|id| id.to_string())
            .map_err(|e| e.to_string())
    }

    pub(crate) fn revoke_grant(&self, grant: &str) -> Result<(), String> {
        let id = GrantId::parse(grant).ok_or("governed control: unknown grant")?;
        self.control.revoke_grant(id).map_err(|e| e.to_string())
    }

    /// Everything the interface shows.
    pub(crate) fn status(&self) -> Value {
        let runs = self.control.authority().runs().views();
        let mut commitments: Vec<Value> = runs
            .iter()
            .rev()
            .take(32)
            .flat_map(|run| self.control.authority().commitments().views_of_run(run.id))
            .map(|view| view_json(&view))
            .collect();
        commitments.truncate(64);
        let grants: Vec<Value> = self
            .control
            .grants()
            .into_iter()
            .rev()
            .take(64)
            .map(|grant| {
                json!({
                    "id": grant.id.to_string(),
                    "kind": grant.scope.kind().as_str(),
                    "lines": grant.scope.describe(),
                    "live": self.control.authority().grants().live(grant.id).is_some(),
                })
            })
            .collect();
        json!({
            "status": self.control.status(),
            "commitments": commitments,
            "grants": grants,
            "runs": runs.iter().rev().take(32).map(|run| json!({
                "id": run.id.to_string(),
                "agent": run.agent.to_string(),
                "cancelled": run.cancelled,
                "finished": run.finished,
            })).collect::<Vec<_>>(),
        })
    }

    pub(crate) fn evidence(&self) -> Vec<Value> {
        self.recent
            .records()
            .iter()
            .rev()
            .take(200)
            .map(EvidenceRecord::to_json)
            .collect()
    }

    pub(crate) fn import_attachment(&self, name: &str, bytes: Vec<u8>) -> Result<Value, String> {
        let attachment = self
            .attachments
            .import(name, bytes)
            .map_err(|e| e.to_string())?;
        Ok(json!({
            "id": attachment.id,
            "name": attachment.name,
            "bytes": attachment.bytes.len(),
            "sha256": attachment.digest.to_hex(),
        }))
    }

    pub(crate) fn display_start(&self) -> Result<Value, String> {
        self.control
            .start_display()
            .map(|s| json!({ "generation": s.generation, "number": s.number, "width": s.width, "height": s.height }))
            .map_err(|e| e.to_string())
    }

    pub(crate) fn display_stop(&self) {
        self.control.stop_display();
    }
}

fn view_json(view: &CommitmentView) -> Value {
    json!({
        "id": view.id.to_string(),
        "agent": view.agent.to_string(),
        "run": view.run.to_string(),
        "kind": view.kind.as_str(),
        "class": view.class.as_str(),
        "operation": view.operation,
        "target": view.target,
        "summary": view.summary,
        "state": view.state.as_str(),
        "requires_approval": view.requires_approval,
        "binding": view.binding_short,
    })
}

/// An effect's output for the interface or an agent: bounded text, and only
/// the size and digest of bytes.
fn output_json(output: &EffectOutput) -> Value {
    let text = output.text.as_deref().map(|text| {
        let mut end = text.len().min(MAX_RETURNED_TEXT);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text[..end].to_string()
    });
    json!({
        "text": text,
        "bytes": output.bytes.as_ref().map(Vec::len),
        "meta": output.meta.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
    })
}

/// The agent executor's way into Phase Three: one run per agent loop,
/// opened on the first governed action. When the loop ends its run finishes
/// as soon as nothing of it waits (what waits ends approved, denied or
/// expired); the owner's stop of the agent cancels it at once.
pub(crate) struct AgentBridge {
    world: Arc<RealWorld>,
    run: Mutex<Option<(AgentId, RunId)>>,
    /// Whether the owner's Warden review is enabled (it then denies every
    /// action it reviews, as it does for the kernel registry).
    warden: fn() -> bool,
}

/// The owner's Warden setting; unreadable, it counts as enabled.
fn warden_enabled() -> bool {
    nexus_kernel::config::load_config()
        .map_or(true, |config| config.governance.enable_warden_review)
}

impl AgentBridge {
    pub(crate) fn new(world: Arc<RealWorld>) -> Self {
        Self {
            world,
            run: Mutex::new(None),
            warden: warden_enabled,
        }
    }

    /// A bridge whose Warden setting is fixed (tests).
    #[cfg(test)]
    #[cfg(target_os = "linux")]
    pub(crate) fn with_warden(world: Arc<RealWorld>, warden: fn() -> bool) -> Self {
        Self {
            world,
            run: Mutex::new(None),
            warden,
        }
    }

    /// Run a governed intent for `agent_id`: data back to the agent. A
    /// `reviewed` action is refused while the owner's Warden review is
    /// enabled.
    pub(crate) fn act(
        &self,
        agent_id: &str,
        intent: &Intent,
        reviewed: bool,
    ) -> Result<String, String> {
        if reviewed && (self.warden)() {
            return Err(format!(
                "Warden blocked action: {}",
                crate::commands::cognitive::WARDEN_REVIEW_UNAVAILABLE
            ));
        }
        let agent = AgentId::new(agent_id).ok_or("governed control: invalid agent identity")?;
        let run = {
            let mut current = self.run.lock().unwrap_or_else(|p| p.into_inner());
            match current.as_ref() {
                Some((owner, run)) if *owner == agent => *run,
                Some(_) => return Err("governed control: one agent per loop".into()),
                None => {
                    let run = self
                        .world
                        .control
                        .open_run(agent.clone(), RunOrigin::AgentGoal)
                        .map_err(|e| e.to_string())?;
                    self.world.track_agent_run(agent_id, run);
                    *current = Some((agent.clone(), run));
                    run
                }
            }
        };
        match self
            .world
            .control
            .agent_action(&agent, run, intent)
            .map_err(|e| e.to_string())?
        {
            AgentOutcome::Done(output) => Ok(output_json(&output).to_string()),
            AgentOutcome::AwaitingApproval(view) => Ok(json!({
                "awaiting_owner_approval": view.id.to_string(),
                "summary": view.summary,
            })
            .to_string()),
        }
    }
}

impl Drop for AgentBridge {
    fn drop(&mut self) {
        let current = self.run.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some((_, run)) = current {
            self.world.control.finish_when_settled(run);
        }
    }
}

/// Text for the native message dialog: the GTK backend reads it as a printf
/// format, so every `%` is doubled.
fn dialog_text(message: &str) -> String {
    message.replace('%', "%%")
}

/// The owner's native dialogs: the only production [`ControlConfirmer`].
/// They are shown by the backend; the webview holds no dialog permission.
#[cfg(all(target_os = "linux", feature = "tauri-runtime"))]
pub(crate) struct ControlDialogs(pub(crate) tauri::AppHandle<tauri::Wry>);

#[cfg(all(target_os = "linux", feature = "tauri-runtime"))]
impl ControlConfirmer for ControlDialogs {
    fn confirm_action(&self, request: &ActionConfirmation) -> bool {
        use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
        self.0
            .dialog()
            .message(dialog_text(&request.message()))
            .title(request.title())
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Allow".to_string(),
                "Cancel".to_string(),
            ))
            .blocking_show()
    }

    fn confirm_grant(&self, request: &GrantConfirmation) -> bool {
        use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
        self.0
            .dialog()
            .message(dialog_text(&request.message()))
            .title(request.title())
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Grant".to_string(),
                "Cancel".to_string(),
            ))
            .blocking_show()
    }

    fn confirm_resume(&self, request: &ResumeConfirmation) -> bool {
        use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
        self.0
            .dialog()
            .message(dialog_text(&request.message()))
            .title(request.title())
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Resume".to_string(),
                "Cancel".to_string(),
            ))
            .blocking_show()
    }
}

/// The IPC commands (thin: every decision is in the crate). Dialog-bound
/// work runs on the blocking pool, never on the IPC or main thread.
#[cfg(feature = "tauri-runtime")]
pub(crate) mod ipc {
    use super::RealWorld;
    use crate::AppState;
    use nexus_governed_control::governed::GrantRequest;
    use nexus_governed_control::ingress::CommandEnvelope;
    use serde_json::Value;
    use std::sync::Arc;

    type App = tauri::AppHandle<tauri::Wry>;

    fn world(state: &AppState) -> Result<Arc<RealWorld>, String> {
        state.real_world()
    }

    #[cfg(target_os = "linux")]
    async fn blocking<T: Send + 'static>(
        work: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        tauri::async_runtime::spawn_blocking(work)
            .await
            .map_err(|_| "governed control: the operation did not complete".to_string())?
    }

    #[cfg(not(target_os = "linux"))]
    async fn blocking<T: Send + 'static>(
        _work: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        Err("governed real-world control is available on Linux only".to_string())
    }

    #[tauri::command]
    pub(crate) fn p3_status(state: tauri::State<'_, AppState>) -> Result<Value, String> {
        Ok(world(state.inner())?.status())
    }

    #[tauri::command]
    pub(crate) fn p3_evidence(state: tauri::State<'_, AppState>) -> Result<Vec<Value>, String> {
        Ok(world(state.inner())?.evidence())
    }

    #[tauri::command]
    pub(crate) fn p3_submit(
        state: tauri::State<'_, AppState>,
        envelope: CommandEnvelope,
    ) -> Result<Value, String> {
        world(state.inner())?.submit(envelope)
    }

    #[tauri::command]
    pub(crate) async fn p3_approve(
        app: App,
        state: tauri::State<'_, AppState>,
        commitment: String,
    ) -> Result<Value, String> {
        let world = world(state.inner())?;
        blocking(move || {
            #[cfg(target_os = "linux")]
            {
                world.approve(&commitment, &super::ControlDialogs(app))
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (app, world, commitment);
                Err("governed real-world control is available on Linux only".to_string())
            }
        })
        .await
    }

    #[tauri::command]
    pub(crate) fn p3_deny(
        state: tauri::State<'_, AppState>,
        commitment: String,
    ) -> Result<(), String> {
        world(state.inner())?.deny(&commitment)
    }

    #[tauri::command]
    pub(crate) fn p3_cancel_run(
        state: tauri::State<'_, AppState>,
        run: String,
    ) -> Result<(), String> {
        world(state.inner())?.cancel_run(&run)
    }

    #[tauri::command]
    pub(crate) fn p3_emergency_stop(state: tauri::State<'_, AppState>) -> Result<usize, String> {
        Ok(world(state.inner())?.emergency_stop())
    }

    #[tauri::command]
    pub(crate) async fn p3_resume(
        app: App,
        state: tauri::State<'_, AppState>,
    ) -> Result<(), String> {
        let world = world(state.inner())?;
        blocking(move || {
            #[cfg(target_os = "linux")]
            {
                world.resume(&super::ControlDialogs(app))
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (app, world);
                Err("governed real-world control is available on Linux only".to_string())
            }
        })
        .await
    }

    #[tauri::command]
    pub(crate) async fn p3_request_grant(
        app: App,
        state: tauri::State<'_, AppState>,
        request: GrantRequest,
        ttl_secs: u64,
    ) -> Result<String, String> {
        let world = world(state.inner())?;
        blocking(move || {
            #[cfg(target_os = "linux")]
            {
                world.request_grant(&request, ttl_secs, &super::ControlDialogs(app))
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (app, world, request, ttl_secs);
                Err("governed real-world control is available on Linux only".to_string())
            }
        })
        .await
    }

    #[tauri::command]
    pub(crate) fn p3_revoke_grant(
        state: tauri::State<'_, AppState>,
        grant: String,
    ) -> Result<(), String> {
        world(state.inner())?.revoke_grant(&grant)
    }

    /// The owner picks a file natively; only its content is kept.
    #[tauri::command]
    pub(crate) async fn p3_import_attachment(
        app: App,
        state: tauri::State<'_, AppState>,
    ) -> Result<Option<Value>, String> {
        let world = world(state.inner())?;
        blocking(move || {
            #[cfg(target_os = "linux")]
            {
                use std::io::Read;
                use tauri_plugin_dialog::DialogExt;
                let Some(picked) = app
                    .dialog()
                    .file()
                    .set_title("Attach a file to a Nexus command")
                    .blocking_pick_file()
                else {
                    return Ok(None);
                };
                let path = picked
                    .into_path()
                    .map_err(|_| "governed control: the selection is not a file".to_string())?;
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let file = std::fs::File::open(&path)
                    .map_err(|_| "governed control: the file cannot be read".to_string())?;
                let mut bytes = Vec::new();
                file.take(nexus_governed_control::ingress::MAX_ATTACHMENT as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "governed control: the file cannot be read".to_string())?;
                world.import_attachment(&name, bytes).map(Some)
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (app, world);
                Err("governed real-world control is available on Linux only".to_string())
            }
        })
        .await
    }

    #[tauri::command]
    pub(crate) async fn p3_display_start(
        state: tauri::State<'_, AppState>,
    ) -> Result<Value, String> {
        let world = world(state.inner())?;
        blocking(move || world.display_start()).await
    }

    #[tauri::command]
    pub(crate) fn p3_display_stop(state: tauri::State<'_, AppState>) -> Result<(), String> {
        world(state.inner())?.display_stop();
        Ok(())
    }
}

// End to end on Linux only, where Phase Three is qualified: elsewhere the
// runtime root cannot be made private (Windows) and no program can be
// pinned (macOS), so these effects fail closed before they could run.
#[cfg(test)]
#[cfg(target_os = "linux")]
#[path = "governed_real_world/tests.rs"]
mod tests;
