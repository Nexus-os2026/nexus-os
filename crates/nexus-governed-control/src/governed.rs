//! The front door: one object the desktop holds for all governed
//! real-world control.
//!
//! Everything arrives here as data (a typed [`Intent`], a [`GrantRequest`]
//! from the owner's interface, a command envelope) and leaves only through
//! the control pipeline: a domain prepares, the authority commits, the
//! owner's grants and native approvals authorize, the pipeline executes
//! once. There is no other entry to any actuator.

use crate::authority::approval::ControlConfirmer;
use crate::authority::clock::Clock;
use crate::authority::commitment::CommitmentView;
use crate::authority::effect::EffectClass;
use crate::authority::evidence::{EvidencePhase, EvidenceSink};
use crate::authority::ids::{AgentId, CommitmentId, GrantId, RunId};
use crate::authority::policy::{Grant, GrantScope};
use crate::authority::run::RunOrigin;
use crate::authority::{Authority, AuthorityError};
use crate::broker::{CredentialBroker, SecretSource, Vault};
use crate::browser::{Browser, BrowserIntent};
use crate::connector::{ConnectorIntent, Connectors};
use crate::control::{Control, EffectOutput, Preparation};
use crate::display::{AgentDisplay, DisplayStatus, InputIntent, PerceptionIntent};
use crate::egress::{Egress, EgressIntent};
use crate::runtime_root::RuntimeRoot;
use crate::tool::{ToolIntent, Tools};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Every governed intent, as data.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "domain",
    content = "intent",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Intent {
    Request(EgressIntent),
    Tool(ToolIntent),
    Connector(ConnectorIntent),
    Browse(BrowserIntent),
    Observe(PerceptionIntent),
    Input(InputIntent),
}

/// What the owner may grant, as data. Each becomes a canonical scope built
/// by its domain (pinning identities where there are executables) and is
/// granted only after the owner's native confirmation of that scope.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GrantRequest {
    Egress {
        origin: String,
        methods: Vec<String>,
        #[serde(default)]
        allow_private: bool,
    },
    Tool {
        tool: String,
    },
    Connector {
        connector: String,
        account: String,
        operations: Vec<String>,
    },
    Browser {
        origins: Vec<String>,
        #[serde(default)]
        downloads: bool,
    },
    Perception,
    Input {
        max_steps: u32,
        #[serde(default)]
        session_r1: bool,
    },
}

/// What the interface shows about the control.
#[derive(Clone, Debug, Serialize)]
pub struct ControlStatus {
    pub platform_supported: bool,
    pub emergency_stopped: bool,
    pub policy_generation: u64,
    pub display: Option<(u64, u32, u16, u16)>,
    pub tools: Vec<(&'static str, &'static str, &'static str)>,
    pub connector_operations: Vec<(&'static str, &'static str, &'static str, &'static str)>,
}

/// The longest grant the interface may ask for.
pub const MAX_GRANT: Duration = Duration::from_secs(24 * 3600);

/// The governed control.
pub struct GovernedControl {
    control: Arc<Control>,
    egress: Arc<Egress>,
    connectors: Connectors,
    tools: Tools,
    display: AgentDisplay,
    browser: Browser,
    /// Runs whose agent or command has ended while something of theirs
    /// still waits: each finishes once nothing in it can start any more.
    detached: Mutex<HashSet<RunId>>,
}

impl GovernedControl {
    /// Build the production control: `root` is the owned runtime directory,
    /// `vault` where credentials come from, `evidence` the audit sink.
    pub fn new(
        root: &Path,
        vault: Vault,
        evidence: Arc<dyn EvidenceSink>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, AuthorityError> {
        Self::with_secrets(root, vault.source(), evidence, clock)
    }

    pub(crate) fn with_secrets(
        root: &Path,
        secrets: Arc<dyn SecretSource>,
        evidence: Arc<dyn EvidenceSink>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, AuthorityError> {
        let root = RuntimeRoot::open(root)?;
        let control = Arc::new(Control::new(Authority::new(evidence, clock)));
        let egress = Arc::new(Egress::system());
        let broker = CredentialBroker::new(control.authority(), secrets);
        Ok(Self {
            connectors: Connectors::production(egress.clone(), broker),
            tools: Tools::production(root.clone()),
            display: AgentDisplay::new(root.clone()),
            browser: Browser::new(root),
            control,
            egress,
            detached: Mutex::new(HashSet::new()),
        })
    }

    /// End whatever can no longer start (expired, stale, its run or a grant
    /// ended), recording each, and finish every detached run nothing of
    /// which can start any more.
    pub fn settle(&self) {
        self.control.prune();
        let detached: Vec<RunId> = self
            .detached
            .lock()
            .expect("detached")
            .iter()
            .copied()
            .collect();
        for run in detached {
            let live = self
                .authority()
                .commitments()
                .views_of_run(run)
                .iter()
                .any(|view| !view.state.is_final());
            if !live {
                self.finish_run(run);
                self.detached.lock().expect("detached").remove(&run);
            }
        }
    }

    /// The agent or command of `run` has ended: finish it now if nothing of
    /// it waits, or as soon as what waits is approved, denied or expires.
    pub fn finish_when_settled(&self, run: RunId) {
        self.detached.lock().expect("detached").insert(run);
        self.settle();
    }

    pub fn authority(&self) -> &Authority {
        self.control.authority()
    }

    pub fn open_run(&self, agent: AgentId, origin: RunOrigin) -> Result<RunId, AuthorityError> {
        self.settle();
        self.authority().open_run(agent, origin)
    }

    fn prepare(
        &self,
        run: RunId,
        agent: &AgentId,
        intent: &Intent,
    ) -> Result<Preparation, AuthorityError> {
        let authority = self.authority();
        match intent {
            Intent::Request(intent) => self.egress.prepare(authority, intent),
            Intent::Tool(intent) => self.tools.prepare(authority, intent),
            Intent::Connector(intent) => self.connectors.prepare(authority, agent, run, intent),
            Intent::Browse(intent) => self.browser.prepare(authority, intent),
            Intent::Observe(intent) => self.display.prepare_observation(authority, run, intent),
            Intent::Input(intent) => self.display.prepare_input(authority, run, intent),
        }
    }

    /// Prepare and commit to an intent for `agent` in `run`. Nothing
    /// happens yet.
    pub fn propose(
        &self,
        agent: &AgentId,
        run: RunId,
        intent: &Intent,
    ) -> Result<CommitmentView, AuthorityError> {
        let preparation = self.prepare(run, agent, intent)?;
        self.control.propose(agent, run, preparation)
    }

    /// Authorize (R2: the owner's native approval of exactly this
    /// commitment).
    pub fn authorize(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
        confirmer: &dyn ControlConfirmer,
    ) -> Result<(), AuthorityError> {
        self.control.authorize(id, agent, run, confirmer)
    }

    pub fn execute(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
    ) -> Result<EffectOutput, AuthorityError> {
        self.control.execute(id, agent, run)
    }

    pub fn deny(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
    ) -> Result<(), AuthorityError> {
        let denied = self.control.deny(id, agent, run);
        self.settle();
        denied
    }

    /// An agent's action under the owner's standing grants: R0 and R1 run
    /// now; an R2 action is committed and left for the owner's native
    /// approval from the interface. An agent's path never raises the
    /// owner's dialog itself.
    pub fn agent_action(
        &self,
        agent: &AgentId,
        run: RunId,
        intent: &Intent,
    ) -> Result<AgentOutcome, AuthorityError> {
        let view = self.propose(agent, run, intent)?;
        if view.class == EffectClass::R2 {
            return Ok(AgentOutcome::AwaitingApproval(view));
        }
        self.authorize(view.id, agent, run, &NeverAsk)?;
        self.execute(view.id, agent, run).map(AgentOutcome::Done)
    }

    pub fn cancel_run(&self, run: RunId) -> Result<(), AuthorityError> {
        self.display.forget_run(run);
        self.control.cancel_run(run)
    }

    pub fn finish_run(&self, run: RunId) {
        self.display.forget_run(run);
        self.control.finish_run(run);
    }

    /// The owner's emergency stop: every run cancelled, every unconsumed
    /// commitment ended, the policy generation moved, the agent display
    /// stopped; no new run until the owner resumes natively.
    pub fn emergency_stop(&self) -> usize {
        let cancelled = self.control.emergency_stop();
        // Recorded, as every display stop is.
        self.stop_display();
        cancelled
    }

    pub fn resume(&self, confirmer: &dyn ControlConfirmer) -> Result<(), AuthorityError> {
        self.authority().resume(confirmer)
    }

    /// The canonical scope for a grant request (identities pinned now).
    pub fn grant_scope(&self, request: &GrantRequest) -> Result<GrantScope, AuthorityError> {
        match request {
            GrantRequest::Egress {
                origin,
                methods,
                allow_private,
            } => Egress::grant_scope(origin, methods, *allow_private),
            GrantRequest::Tool { tool } => self.tools.grant_scope(tool),
            GrantRequest::Connector {
                connector,
                account,
                operations,
            } => self.connectors.grant_scope(connector, account, operations),
            GrantRequest::Browser { origins, downloads } => {
                self.browser.grant_scope(origins, *downloads)
            }
            GrantRequest::Perception => Ok(AgentDisplay::perception_scope()),
            GrantRequest::Input {
                max_steps,
                session_r1,
            } => AgentDisplay::input_scope(*max_steps, *session_r1),
        }
    }

    /// Ask the owner, natively, for a grant.
    pub fn request_grant(
        &self,
        request: &GrantRequest,
        ttl: Duration,
        confirmer: &dyn ControlConfirmer,
    ) -> Result<GrantId, AuthorityError> {
        let scope = self.grant_scope(request)?;
        self.authority()
            .grants()
            .request(scope, ttl.min(MAX_GRANT), confirmer)
    }

    pub fn revoke_grant(&self, id: GrantId) -> Result<(), AuthorityError> {
        let revoked = self.authority().grants().revoke(id);
        // What relied on it ends now, recorded, not when next touched.
        self.settle();
        revoked
    }

    pub fn grants(&self) -> Vec<Grant> {
        self.authority().grants().all()
    }

    /// Start the agent display: a process Phase Three owns, so it needs a
    /// live perception or input grant and no emergency stop, and its start
    /// and stop are recorded.
    pub fn start_display(&self) -> Result<DisplayStatus, AuthorityError> {
        if self.authority().runs().is_stopped() {
            return Err(AuthorityError::EmergencyStopped);
        }
        let grants = self.authority().grants();
        if grants
            .live_of(crate::authority::effect::CapabilityKind::Perception)
            .is_empty()
            && grants
                .live_of(crate::authority::effect::CapabilityKind::Input)
                .is_empty()
        {
            return Err(AuthorityError::NoCoveringGrant);
        }
        // Evidence first: an unrecorded display does not start. An
        // emergency stop that comes while it starts ends it unused.
        let runs = self.authority().runs();
        let recorded = std::cell::Cell::new(false);
        let started = self.display.start(
            1280,
            800,
            || {
                let record = self.authority().record_display(
                    EvidencePhase::DisplayStarted,
                    vec![("size".into(), "1280x800".into())],
                );
                recorded.set(record.is_ok());
                record
            },
            || runs.is_stopped(),
        );
        // A start recorded but not completed is recorded as ended, so the
        // evidence never shows a display that is not there.
        if started.is_err() && recorded.get() {
            let _ = self.authority().record_display(
                EvidencePhase::DisplayStopped,
                vec![("outcome".into(), "the start failed".into())],
            );
        }
        started
    }

    /// Stop the agent display (recorded).
    pub fn stop_display(&self) {
        if let Some(status) = self.display.status() {
            self.display.stop();
            let _ = self.authority().record_display(
                EvidencePhase::DisplayStopped,
                vec![("display".into(), status.number.to_string())],
            );
        }
    }

    pub fn status(&self) -> ControlStatus {
        ControlStatus {
            platform_supported: cfg!(target_os = "linux"),
            emergency_stopped: self.authority().runs().is_stopped(),
            policy_generation: self.authority().policy_generation(),
            display: self
                .display
                .status()
                .map(|s| (s.generation, s.number, s.width, s.height)),
            tools: self
                .tools
                .tools()
                .into_iter()
                .map(|(key, class, executable)| (key, class.as_str(), executable))
                .collect(),
            connector_operations: self
                .connectors
                .operations()
                .into_iter()
                .map(|(connector, op, class, method)| (connector, op, class.as_str(), method))
                .collect(),
        }
    }
}

/// Declines everything: R0 and R1 authorization never asks, and nothing on
/// an agent's path may ask the owner.
struct NeverAsk;

impl ControlConfirmer for NeverAsk {
    fn confirm_action(&self, _: &crate::authority::approval::ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &crate::authority::approval::GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &crate::authority::approval::ResumeConfirmation) -> bool {
        false
    }
}

/// What an agent's governed action came to.
#[derive(Debug)]
pub enum AgentOutcome {
    Done(EffectOutput),
    /// Committed; the owner approves (or not) from the interface.
    AwaitingApproval(CommitmentView),
}

#[cfg(test)]
mod tests;
