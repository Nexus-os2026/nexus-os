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

/// How long quitting waits for effects still executing to see their
/// cancellation and end (a browser session within one DevTools poll, a tool
/// within one 10 ms poll plus its reaping).
const SHUTDOWN_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// Phase Three in the desktop.
pub(crate) struct RealWorld {
    control: GovernedControl,
    recent: Arc<MemoryEvidence>,
    attachments: Attachments,
    agents: Mutex<Agents>,
}

/// What the desktop keeps about agents' use of Phase Three.
#[derive(Default)]
struct Agents {
    /// Each agent's runs (ended ones forgotten as new ones arrive), so that
    /// the owner's stop of an agent cancels what it left running or waiting.
    runs: HashMap<String, Vec<RunId>>,
    /// Counts the owner's stops of agents.
    epoch: u64,
    /// The stop (epoch) each agent was last stopped at: a loop begun before
    /// it can no longer act, even one that had not opened its run yet.
    stopped_at: HashMap<String, u64>,
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
            agents: Mutex::new(Agents::default()),
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
            agents: Mutex::new(Agents::default()),
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

    fn agents(&self) -> std::sync::MutexGuard<'_, Agents> {
        self.agents.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The owner stopped `agent`: its Phase Three runs are cancelled, so
    /// whatever of them still runs sees the cancellation and nothing it left
    /// waiting can still be approved; and no loop of it begun before this
    /// stop can act again (recorded under the lock its runs are opened
    /// under, so a run opened just then is not missed).
    pub(crate) fn cancel_agent(&self, agent: &str) {
        // One spelling per agent (see `AgentBridge::act`).
        let agent = &crate::commands::agents::canonical_agent_id(agent);
        let runs = {
            let mut agents = self.agents();
            agents.epoch += 1;
            let epoch = agents.epoch;
            agents.stopped_at.insert(agent.to_string(), epoch);
            agents.runs.remove(agent).unwrap_or_default()
        };
        for run in runs {
            let _ = self.control.cancel_run(run);
        }
    }

    /// The desktop is quitting: every run is cancelled (whatever still runs
    /// sees it and ends its processes) and the agent display is stopped
    /// gracefully, recorded, rather than left to die with the process.
    pub(crate) fn shutdown(&self) {
        let authority = self.control.authority();
        // No run opens any more, and every open one is cancelled.
        self.control.shut_down();
        // What still executes sees the cancellation at its next step, ends
        // its processes and removes its directories, and its end is
        // recorded: wait for that (within a bound) before the process
        // exits, then stop the display gracefully.
        let executing = || {
            authority.runs().views().iter().any(|run| {
                authority
                    .commitments()
                    .views_of_run(run.id)
                    .iter()
                    .any(|view| view.state == CommitmentState::Executing)
            })
        };
        let deadline = std::time::Instant::now() + SHUTDOWN_WAIT;
        while executing() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        self.control.stop_display();
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
    /// Whether the agent is running now: a paused or stopped agent acts no
    /// more, whichever way it was paused or stopped.
    running: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    /// The owner's stops counted when this loop began.
    began: u64,
}

/// The owner's Warden setting; unreadable, it counts as enabled.
fn warden_enabled() -> bool {
    nexus_kernel::config::load_config()
        .map_or(true, |config| config.governance.enable_warden_review)
}

impl AgentBridge {
    pub(crate) fn new(
        world: Arc<RealWorld>,
        running: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    ) -> Self {
        let began = world.agents().epoch;
        Self {
            world,
            run: Mutex::new(None),
            warden: warden_enabled,
            running,
            began,
        }
    }

    /// A bridge whose Warden setting and agent state are fixed (tests).
    #[cfg(test)]
    #[cfg(target_os = "linux")]
    pub(crate) fn for_tests(
        world: Arc<RealWorld>,
        warden: fn() -> bool,
        running: fn(&str) -> bool,
    ) -> Self {
        let mut bridge = Self::new(world, Arc::new(running));
        bridge.warden = warden;
        bridge
    }

    /// A bridge whose Warden setting is fixed and whose agent runs (tests).
    #[cfg(test)]
    #[cfg(target_os = "linux")]
    pub(crate) fn with_warden(world: Arc<RealWorld>, warden: fn() -> bool) -> Self {
        Self::for_tests(world, warden, |_| true)
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
        // One spelling per agent: its runs are kept, and cancelled, under
        // the canonical one, however its loop writes the id.
        let agent_id = &crate::commands::agents::canonical_agent_id(agent_id);
        if reviewed && (self.warden)() {
            return Err(format!(
                "Warden blocked action: {}",
                crate::commands::cognitive::WARDEN_REVIEW_UNAVAILABLE
            ));
        }
        if !(self.running)(agent_id) {
            return Err("governed control: the agent is not running".into());
        }
        let agent = AgentId::new(agent_id).ok_or("governed control: invalid agent identity")?;
        let run = {
            let mut current = self.run.lock().unwrap_or_else(|p| p.into_inner());
            let mut agents = self.world.agents();
            if agents
                .stopped_at
                .get(agent_id)
                .is_some_and(|stopped| *stopped > self.began)
            {
                return Err("governed control: the owner stopped this agent".into());
            }
            match current.as_ref() {
                Some((owner, run)) if *owner == agent => *run,
                Some(_) => return Err("governed control: one agent per loop".into()),
                None => {
                    let run = self
                        .world
                        .control
                        .open_run(agent.clone(), RunOrigin::AgentGoal)
                        .map_err(|e| e.to_string())?;
                    // Tracked under the lock a stop takes: a stop cannot
                    // come between opening the run and remembering it.
                    let registry = self.world.control.authority().runs();
                    for runs in agents.runs.values_mut() {
                        runs.retain(|r| {
                            registry
                                .view(*r)
                                .is_some_and(|view| !view.cancelled && !view.finished)
                        });
                    }
                    agents.runs.retain(|_, runs| !runs.is_empty());
                    agents
                        .runs
                        .entry(agent_id.to_string())
                        .or_default()
                        .push(run);
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

/// How long the answer of a confirmation stays unarmed after its window
/// appears: a click meant for something else cannot give it.
#[cfg(all(target_os = "linux", feature = "tauri-runtime"))]
const ARMING_DELAY: std::time::Duration = std::time::Duration::from_millis(1000);

/// When the owner may give the answer of a confirmation: once the arming
/// delay has passed and the end and the right edge of its text have been
/// reached (nothing it allows can be left unseen below or beside the view).
#[cfg(all(target_os = "linux", any(test, feature = "tauri-runtime")))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Arming {
    delay_passed: bool,
    end_reached: bool,
    edge_reached: bool,
}

#[cfg(all(target_os = "linux", any(test, feature = "tauri-runtime")))]
impl Arming {
    fn ready(self) -> bool {
        self.delay_passed && self.end_reached && self.edge_reached
    }
}

/// Whether a laid-out scrolled view shows its last part: its position plus
/// its page reaches its extent (a view that fits shows everything at once).
#[cfg(all(target_os = "linux", any(test, feature = "tauri-runtime")))]
fn reached(value: f64, page: f64, upper: f64) -> bool {
    page > 0.0 && value + page >= upper - 1.0
}

/// The owner's confirmations, in Nexus's own window: the only production
/// [`ControlConfirmer`]. They are shown by the backend; the webview holds
/// no dialog permission.
#[cfg(all(target_os = "linux", feature = "tauri-runtime"))]
pub(crate) struct ControlDialogs(pub(crate) tauri::AppHandle<tauri::Wry>);

#[cfg(all(target_os = "linux", feature = "tauri-runtime"))]
impl ControlDialogs {
    /// Show the window on the main thread and wait here (never on the main
    /// thread) for the owner's answer; no answer is a refusal.
    fn confirm(&self, title: &str, message: String, answer: &str) -> bool {
        use gtk::prelude::*;
        // One confirmation at a time: the next window opens (and its arming
        // delay starts) only once this one is answered, so a click meant
        // for one window can never land on another already armed.
        static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
        let (sender, receiver) = std::sync::mpsc::channel();
        let (title, answer) = (title.to_string(), answer.to_string());
        let shown = self.0.run_on_main_thread(move || {
            let (dialog, _) = owner_window(&title, &message, &answer);
            let sender = std::cell::RefCell::new(Some(sender));
            dialog.connect_response(move |dialog, response| {
                if let Some(sender) = sender.borrow_mut().take() {
                    let _ = sender.send(response == gtk::ResponseType::Accept);
                }
                dialog.close();
            });
            dialog.present();
        });
        shown.is_ok() && receiver.recv().unwrap_or(false)
    }
}

#[cfg(all(target_os = "linux", feature = "tauri-runtime"))]
impl ControlConfirmer for ControlDialogs {
    fn confirm_action(&self, request: &ActionConfirmation) -> bool {
        self.confirm(request.title(), request.message(), "Allow")
    }

    fn confirm_grant(&self, request: &GrantConfirmation) -> bool {
        self.confirm(request.title(), request.message(), "Grant")
    }

    fn confirm_resume(&self, request: &ResumeConfirmation) -> bool {
        self.confirm(request.title(), request.message(), "Resume")
    }
}

/// Nexus's own confirmation window. Its text is shown exactly as given, in
/// a monospace label that never wraps and scrolls both ways: every line
/// starts where the backend started it, with its marker, so nothing in it
/// can pass for the window's own text, and nothing is cut. Cancel is the
/// default answer (Enter and Escape cancel); the answer button arms only
/// after `ARMING_DELAY`, once the end and the right edge of the text have
/// been reached. Returns the window and its answer button.
#[cfg(all(target_os = "linux", feature = "tauri-runtime"))]
fn owner_window(title: &str, message: &str, answer: &str) -> (gtk::Dialog, gtk::Widget) {
    use gtk::prelude::*;
    use std::cell::Cell;
    use std::rc::Rc;
    let dialog = gtk::Dialog::new();
    dialog.set_title(title);
    dialog.set_modal(true);
    dialog.set_keep_above(true);
    dialog.set_default_size(900, 560);
    dialog.add_button("Cancel", gtk::ResponseType::Cancel);
    let allow = dialog.add_button(answer, gtk::ResponseType::Accept);
    allow.set_sensitive(false);
    dialog.set_default_response(gtk::ResponseType::Cancel);
    // A label, not a text view: its whole extent is known at its first
    // layout (a text view lays its lines out lazily, so its end would seem
    // reached before it is). Plain text, never markup; never wrapped.
    let text = gtk::Label::new(None);
    // Every line starts with a left-to-right mark, so each is laid out left
    // to right with its marker first, whatever script its content begins
    // with (a line led by a right-to-left letter would otherwise be drawn
    // flush right, its marker last).
    let lines: Vec<String> = message
        .split('\n')
        .map(|line| format!("\u{200E}{line}"))
        .collect();
    text.set_text(&lines.join("\n"));
    text.set_line_wrap(false);
    text.set_xalign(0.0);
    text.set_yalign(0.0);
    let monospace = gtk::pango::AttrList::new();
    monospace.insert(gtk::pango::AttrString::new_family("monospace"));
    text.set_attributes(Some(&monospace));
    let scroll = gtk::ScrolledWindow::builder().build();
    scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    scroll.add(&text);
    dialog.content_area().pack_start(&scroll, true, true, 0);
    let arming = Rc::new(Cell::new(Arming::default()));
    let update: Rc<dyn Fn()> = {
        let (arming, allow, scroll) = (arming.clone(), allow.clone(), scroll.clone());
        Rc::new(move || {
            let mut now = arming.get();
            let (down, across) = (scroll.vadjustment(), scroll.hadjustment());
            now.end_reached |= reached(down.value(), down.page_size(), down.upper());
            now.edge_reached |= reached(across.value(), across.page_size(), across.upper());
            arming.set(now);
            allow.set_sensitive(now.ready());
        })
    };
    for adjustment in [scroll.vadjustment(), scroll.hadjustment()] {
        let moved = update.clone();
        adjustment.connect_value_changed(move |_| moved());
        let resized = update.clone();
        adjustment.connect_changed(move |_| resized());
    }
    {
        let (arming, update) = (arming.clone(), update.clone());
        gtk::glib::timeout_add_local_once(ARMING_DELAY, move || {
            let mut now = arming.get();
            now.delay_passed = true;
            arming.set(now);
            update();
        });
    }
    dialog.show_all();
    (dialog, allow)
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

    /// Understanding a command can resolve a destination (blocking DNS):
    /// it runs on the blocking pool, never on the main thread.
    #[tauri::command]
    pub(crate) async fn p3_submit(
        state: tauri::State<'_, AppState>,
        envelope: CommandEnvelope,
    ) -> Result<Value, String> {
        let world = world(state.inner())?;
        blocking(move || world.submit(envelope)).await
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
