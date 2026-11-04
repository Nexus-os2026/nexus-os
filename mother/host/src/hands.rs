//! Deterministic, one-mission Hands host. Raw Owner handles never escape it.
use crate::stop::{Gate, Route};
use crate::terminal::{Shared, Terminal};
use crate::Home;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::evidence::{EndStatus, EvidencePhase};
use nexus_governed_control::authority::ids::{CommitmentId, RunId};
use nexus_governed_control::files::{FileCreate, RecoveryFinding};
use nexus_governed_control::governed::GrantRequest;
use nexus_mother::adapter::{mother_agent, GovernedPort};
use nexus_mother::contract::{Budget, EffectKind, MissionContract, PassCriterion};
use nexus_mother::coordinator::{Coordinator, CoordinatorError, FileSession, Lanes};
use nexus_mother::effect::{EffectPort, EffectRequest};
use nexus_mother::journal::MissionId;
use nexus_mother::lane::{
    Lane, LaneIdentity, LaneInput, LaneOutput, LaneRole, ScriptedLane, Verdict,
};
use nexus_mother::state::MissionState;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[cfg(test)]
#[path = "hands_tests.rs"]
mod hands_tests;

fn mother_code(error: &CoordinatorError) -> &'static str {
    match error {
        CoordinatorError::UnknownMission => "unknown_mission",
        CoordinatorError::FileExclusive => "file_exclusive",
        CoordinatorError::FileSession(_) => "file_session",
        CoordinatorError::Poisoned => "poisoned",
        CoordinatorError::Recording => "recording",
        CoordinatorError::FileUnfinished => "file_unfinished",
        CoordinatorError::Contract(_) => "contract",
        CoordinatorError::Journal(_) => "journal",
        CoordinatorError::Illegal(_) => "illegal_transition",
        CoordinatorError::Lanes(_) => "lanes",
        CoordinatorError::Capacity => "capacity",
        CoordinatorError::NoConsent => "no_consent",
        CoordinatorError::ConsentStale => "consent_stale",
        CoordinatorError::ConsentMismatch => "consent_mismatch",
        CoordinatorError::Broker(_) => "broker",
        CoordinatorError::Withdrawal(_) => "withdrawal",
        CoordinatorError::Private(_) => "private_store",
        CoordinatorError::Denied => "denied",
        CoordinatorError::Stopped => "stopped",
        CoordinatorError::Busy => "busy",
        CoordinatorError::Declined => "declined",
        CoordinatorError::ResumeStale => "resume_stale",
        CoordinatorError::Held => "held",
        CoordinatorError::Closed => "closed",
    }
}

struct Config {
    home: PathBuf,
    workspace: PathBuf,
    name: String,
    text: String,
    hold: Duration,
}
fn parse(args: impl Iterator<Item = String>) -> Result<Config, &'static str> {
    let mut values = std::collections::BTreeMap::new();
    let mut args = args;
    while let Some(key) = args.next() {
        if !matches!(
            key.as_str(),
            "--home" | "--workspace" | "--name" | "--text" | "--hold-before-publish"
        ) || values.contains_key(&key)
        {
            return Err("invalid_arguments");
        }
        values.insert(key, args.next().ok_or("missing_value")?);
    }
    let home = PathBuf::from(values.remove("--home").ok_or("home_required")?);
    let workspace = PathBuf::from(values.remove("--workspace").ok_or("workspace_required")?);
    if !home.is_absolute() || !workspace.is_absolute() {
        return Err("absolute_path_required");
    }
    let name = values.remove("--name").ok_or("name_required")?;
    let text = values.remove("--text").ok_or("text_required")?;
    let hold = values
        .remove("--hold-before-publish")
        .unwrap_or_else(|| "0".into());
    if hold.is_empty() || !hold.bytes().all(|b| b.is_ascii_digit()) {
        return Err("invalid_hold");
    }
    let hold: u64 = hold.parse().map_err(|_| "invalid_hold")?;
    if hold > 30 {
        return Err("invalid_hold");
    }
    Ok(Config {
        home,
        workspace,
        name,
        text,
        hold: Duration::from_secs(hold),
    })
}

struct GatedLane {
    gate: Arc<Gate>,
    lane: ScriptedLane,
}
impl Lane for GatedLane {
    fn identity(&self) -> LaneIdentity {
        self.lane.identity()
    }
    fn invoke(&self, input: &LaneInput) -> LaneOutput {
        // Dispatch linearizes at this last memory-only admission check.
        if !self.gate.admit(Route::Lane) {
            return LaneOutput {
                text: String::new(),
                effects: vec![],
                verdict: Verdict::Halt {
                    reason: "recovery_only".into(),
                },
            };
        }
        let output = self.lane.invoke(input);
        #[cfg(test)]
        hands_tests::before_internal_dispatch(&self.gate, self.lane.identity().role);
        output
    }
}

struct Mission {
    session: FileSession<GovernedPort>,
    port: Arc<GovernedPort>,
    id: Option<MissionId>,
    last: Option<CommitmentId>,
}
struct Runtime {
    home: Arc<Home>,
    config: Config,
    screen: Arc<Shared>,
    confirmer: Arc<crate::terminal::TerminalConfirmer>,
    used: bool,
    mission: Option<Mission>,
}
impl Runtime {
    fn allowed(&self, route: Route) -> bool {
        if self.screen.stop.gate.admit(route) {
            true
        } else {
            self.screen.emit("REFUSED recovery_only\n");
            false
        }
    }
    fn run(&mut self) {
        if !self.allowed(Route::Mission) {
            return;
        }
        if self.used {
            self.screen.emit("REFUSED file_session_used\n");
            return;
        }
        self.used = true;
        let Some(workspace) = self.home.owner().workspace() else {
            self.screen.emit("REFUSED workspace_unavailable\n");
            return;
        };
        if !self.allowed(Route::Files) {
            return;
        }
        if let Err(e) = self.home.owner().request_grant(
            &GrantRequest::Files {
                workspace: workspace.id,
            },
            Duration::from_secs(300),
            &*self.confirmer,
        ) {
            self.screen.emit(&format!("GRANT REFUSED {}\n", e.class()));
            return;
        }
        if !self.allowed(Route::Door) {
            return;
        }
        let door = match self.home.door() {
            Ok(d) => d,
            Err(e) => {
                self.screen.emit(&format!("DOOR REFUSED {}\n", e.class()));
                return;
            }
        };
        if !self.allowed(Route::Run) {
            return;
        }
        let port = Arc::new(
            GovernedPort::new(door)
                .with_admission_gate(self.screen.stop.gate.port.clone())
                .with_workspace(workspace.name.clone()),
        );
        let lanes: Vec<Arc<dyn Lane>> = LaneRole::ALL
            .into_iter()
            .map(|role| {
                let mut lane = ScriptedLane::new(
                    role,
                    "Deterministic Hands demonstration; no model was called.",
                    if role == LaneRole::Verifier {
                        Verdict::Pass
                    } else {
                        Verdict::Continue
                    },
                );
                if role == LaneRole::Builder {
                    lane = lane.asking(vec![EffectRequest::CreateFile(FileCreate {
                        name: self.config.name.clone(),
                        content: self.config.text.clone(),
                    })]);
                }
                Arc::new(GatedLane {
                    gate: self.screen.stop.gate.clone(),
                    lane,
                }) as Arc<dyn Lane>
            })
            .collect();
        let result = (|| {
            let journal = self.home.journal().map_err(|_| "journal_unavailable")?;
            let private = self
                .home
                .private()
                .map_err(|_| "private_store_unavailable")?;
            let lanes = Lanes::new(lanes).map_err(|_| "lane_configuration")?;
            let coordinator = Coordinator::open(
                journal,
                port.clone(),
                lanes,
                Arc::new(SystemClock::default()),
                private,
            )
            .map_err(|_| "coordinator_unavailable")?;
            if !self.screen.stop.gate.admit(Route::Files) {
                return Err("recovery_only");
            }
            coordinator
                .into_file_session()
                .map_err(|_| "file_session_unavailable")
        })();
        let session = match result {
            Ok(s) => s,
            Err(code) => {
                self.screen.emit(&format!("REFUSED {code}\n"));
                return;
            }
        };
        self.mission = Some(Mission {
            session,
            port,
            id: None,
            last: None,
        });
        if !self.allowed(Route::Mission) {
            return;
        }
        let contract = MissionContract {
            owner_words: format!("Create {} with {} UTF-8 bytes, shown in full at the separate File approval.", self.config.name, self.config.text.len()),
            restatement: format!("Create one approved top-level text file named {}. One File mission per fresh process; no in-process replacement. Emergency ends mission work.", self.config.name),
            requested_folder: Some(workspace.name),
            budget: Budget { max_duration_ms: 300_000, max_model_calls: 20, max_money_minor: 0 },
            forbidden_effects: BTreeSet::from([EffectKind::Spend, EffectKind::Process, EffectKind::Network]),
            questions: vec![],
            pass_criteria: vec![PassCriterion { id: "exact_file".into(), text: "Create only the approved file with its exact approved content.".into() }],
        };
        let mission = self.mission.as_mut().expect("installed typed session");
        let result = (|| {
            let id = mission.session.draft(contract)?;
            mission.id = Some(id);
            mission.session.submit(id)?;
            mission.session.request_consent(id)?;
            Ok::<(), CoordinatorError>(())
        })();
        if let Err(e) = result {
            self.screen
                .emit(&format!("MISSION REFUSED {}\n", mother_code(&e)));
            return;
        }
        self.progress();
    }
    fn progress(&mut self) {
        for _ in 0..32 {
            if !self.allowed(Route::Mission) {
                self.observe();
                return;
            }
            let Some(mission) = self.mission.as_ref() else {
                return;
            };
            let Some(id) = mission.id else {
                return;
            };
            #[cfg(test)]
            hands_tests::before_internal_admission(self);
            let advanced = mission.session.advance(id);
            #[cfg(test)]
            hands_tests::after_internal_admission(self);
            match advanced {
                Ok(report) => {
                    for lane in &report.lanes {
                        self.screen.emit(&format!(
                            "LANE invoked {}\n│ {}\n",
                            lane.identity.role.as_str(),
                            lane.text
                        ));
                    }
                    for effect in &report.effects {
                        self.screen.emit(&format!(
                            "EFFECT {} {}\n",
                            effect.kind.as_str(),
                            effect.outcome
                        ));
                    }
                    if report.to == MissionState::AwaitingApproval {
                        self.screen.emit("FILE PENDING\n");
                        return;
                    }
                    if report.to.is_terminal() {
                        self.screen
                            .emit(&format!("MISSION {}\n", report.to.as_str()));
                        self.observe();
                        return;
                    }
                }
                Err(e) => {
                    self.screen
                        .emit(&format!("MISSION REFUSED {}\n", mother_code(&e)));
                    self.observe();
                    return;
                }
            }
        }
        self.screen.stop.gate.fail();
        self.screen.emit("REFUSED progress_bound\n");
    }
    fn approve(&mut self) {
        if !self.allowed(Route::Files) {
            return;
        }
        let Some(mission) = self.mission.as_mut() else {
            self.screen.emit("REFUSED no_mission\n");
            return;
        };
        let Some(pending) = mission.id.and_then(|id| mission.port.pending(id)) else {
            self.screen.emit("REFUSED no_pending_action\n");
            return;
        };
        let (Some(id), Some(run)) = (
            CommitmentId::parse(&pending.commitment),
            RunId::parse(&pending.run),
        ) else {
            self.screen.stop.gate.fail();
            return;
        };
        mission.last = Some(id);
        let owner = self.home.owner();
        if let Err(error) = owner.authorize(id, &mother_agent(), run, &*self.confirmer) {
            // Deny only this retained mission action. Revoked/stopped actions
            // remain ended; a failed deny is not reported as successful.
            let denied = owner.deny(id, &mother_agent(), run);
            if let Err(e) = denied {
                self.screen.emit(&format!(
                    "DENIAL RECORD {} original={}\n",
                    e.class(),
                    error.class()
                ));
            }
            if self.screen.stop.gate.admit(Route::Mission) {
                self.progress();
            } else {
                self.observe();
            }
            // This receipt promises Mother has consumed the denial. Quit may
            // close admission immediately after the Owner sees it.
            self.screen.emit("ACTION DENIED\n");
            return;
        }
        if !self.allowed(Route::Files) {
            self.observe();
            return;
        }
        match self.home.owner().execute(id, &mother_agent(), run) {
            Ok(output) => self.screen.emit(&format!(
                "ACTION FINISHED {}\n",
                output
                    .evidence
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "evidence unavailable".into())
            )),
            Err(e) => self
                .screen
                .emit(&format!("ACTION FINISHED refused={}\n", e.class())),
        }
        if self.screen.stop.gate.admit(Route::Mission) {
            self.progress();
        } else {
            self.observe();
        }
    }
    fn observe(&self) {
        if let Some((mission, id)) = self.mission.as_ref().and_then(|m| m.id.map(|id| (m, id))) {
            match mission.session.state(id) {
                Ok(Some(state)) => self.screen.emit(&format!("MISSION {}\n", state.as_str())),
                Ok(None) => self.screen.emit("MISSION absent\n"),
                Err(e) => self
                    .screen
                    .emit(&format!("MISSION RECORDING FAILED {}\n", mother_code(&e))),
            }
            match mission.session.notice(id) {
                Ok(Some(notice)) => self.screen.emit(&format!("{notice}\n")),
                Err(e) => self
                    .screen
                    .emit(&format!("MISSION NOTICE FAILED {}\n", mother_code(&e))),
                _ => {}
            }
        }
    }
    fn command(&mut self, command: &str) {
        match command {
            "run" => self.run(),
            "approve" => self.approve(),
            "lift-for-undo" => {
                if !self.screen.stop.gate.recovery() {
                    self.screen.emit("REFUSED recovery_only_lift\n");
                    return;
                }
                if self.screen.stop.receipt().outstanding != 0 {
                    self.screen.emit("REFUSED stop_workers_outstanding\n");
                    return;
                }
                match self.home.owner().resume(&*self.confirmer) {
                    Ok(()) => self
                        .screen
                        .emit("LIFTED recovery only; missions remain stopped\n"),
                    Err(e) => self.screen.emit(&format!(
                        "LIFT REFUSED {} (recovery acknowledgement unsupported)\n",
                        e.class()
                    )),
                }
            }
            "undo" => match self.home.owner().files_undo(&*self.confirmer) {
                Ok(report) => show_undo(&self.screen, report),
                Err(e) => self.screen.emit(&format!("UNDO REFUSED {}\n", e.class())),
            },
            "cleanup" => match self.home.owner().files_retry_cleanup() {
                Ok(report) => self.screen.emit(&format!(
                    "CLEANUP settled={} still_owed={}\n",
                    report.settled, report.still_owed
                )),
                Err(e) => self.screen.emit(&format!("CLEANUP FAILED {}\n", e.class())),
            },
            "evidence" => {
                self.screen.emit("RECENT EVIDENCE bounded\n");
                for record in self.home.recent_evidence() {
                    let phase = if record.phase == EvidencePhase::EmergencyStop {
                        "emergency_stopped"
                    } else {
                        record.phase.as_str()
                    };
                    self.screen
                        .emit(&format!("phase={phase} cancelled={}\n", record.cancelled));
                }
                let health = self.home.evidence_health();
                self.screen.emit(&format!("latched={} owed={} recovery_required={} unsettled={} unclean_sessions={}\nEVIDENCE HEALTH\n", health.latched, health.owed, health.recovery_required, health.unsettled, health.unclean_sessions));
            }
            "detail" => {
                let mut view = OwnerScreen(self.screen.clone());
                if let Some(mission) = &self.mission {
                    if let Some(id) = mission.id {
                        match mission.session.working_detail(id) {
                            Ok(Some(detail)) => detail.show(&mut view),
                            Err(e) => self
                                .screen
                                .emit(&format!("DETAIL FAILED {}\n", mother_code(&e))),
                            _ => {}
                        }
                    }
                    if let Some(detail) = mission
                        .last
                        .and_then(|id| self.home.owner().files_working_detail(id))
                    {
                        detail.show(&mut view);
                    }
                }
            }
            _ => self.screen.emit("REFUSED unknown_command\n"),
        }
    }
    fn close(
        self,
        owner: Arc<nexus_governed_control::governed::GovernedControl>,
        stop_body: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> bool {
        if self.screen.stop.gate.failed() {
            self.screen.emit("UNCLEAN host_failure\n");
            return false;
        }
        if let Some(mission) = &self.mission {
            if let Err(e) = mission.session.close() {
                self.screen
                    .emit(&format!("UNCLEAN file_session={}\n", mother_code(&e)));
                return false;
            }
        }
        self.home.owner().shut_down();
        match self.home.owner().files_retry_cleanup() {
            Ok(report) if report.still_owed == 0 => {}
            _ => {
                self.screen.emit("UNCLEAN cleanup_owed\n");
                return false;
            }
        }
        let health = self.home.evidence_health();
        if health.latched || health.owed != 0 || health.recovery_required {
            self.screen.emit("UNCLEAN evidence_obligations\n");
            return false;
        }
        // Backend destruction can perform defensive staging cleanup. Keep
        // it on this owned worker and before closing primary evidence.
        let Self {
            home,
            screen,
            mission,
            ..
        } = self;
        drop(mission);
        drop(stop_body);
        drop(owner);
        let Ok(home) = Arc::try_unwrap(home) else {
            screen.emit("UNCLEAN home_still_owned\n");
            return false;
        };
        let Home {
            control, evidence, ..
        } = home;
        drop(control);
        let health = evidence.health();
        if health.latched || health.owed != 0 || health.recovery_required {
            screen.emit("UNCLEAN evidence_obligations\n");
            return false;
        }
        match evidence.close_session() {
            EndStatus::Recorded { .. } => {
                screen.emit("SESSION CLOSED recorded\n");
                true
            }
            status => {
                screen.emit(&format!("UNCLEAN evidence_close={}\n", status.as_str()));
                false
            }
        }
    }
}
struct OwnerScreen(Arc<Shared>);
impl nexus_mother::evidence::OwnerView for OwnerScreen {
    fn line(&mut self, label: &str, value: &str) {
        self.0.emit(&format!("│ {label}: {value}\n"));
    }
}
impl nexus_governed_control::files::OwnerView for OwnerScreen {
    fn line(&mut self, label: &str, value: &str) {
        self.0.emit(&format!("│ {label}: {value}\n"));
    }
}

fn show_recovery(home: &Home, screen: &Shared) -> Result<(), &'static str> {
    let report = home
        .owner()
        .files_recovery_report()
        .map_err(|_| "recovery_report_unavailable")?;
    screen.emit("RECOVERY REPORT only; no sweep or workspace restoration\n");
    for entry in report {
        screen.emit(&format!("run={}\n", entry.run));
        for finding in entry.findings {
            match finding {
                RecoveryFinding::PreparedWithoutOutcome { files }
                | RecoveryFinding::RestorePreparedWithoutOutcome { files } => {
                    screen.emit("prepared; publication or temp status unknown\n");
                    for file in files {
                        screen.emit(&format!(
                            "path_sha256={} temp_id={}\n",
                            file.path_sha256.as_deref().unwrap_or("unknown"),
                            file.temp_id.as_deref().unwrap_or("unknown")
                        ));
                    }
                }
                RecoveryFinding::CleanupOwed { temps }
                | RecoveryFinding::OwnerVersionKept { versions: temps } => {
                    screen.emit("owned recovery material; not disposable\n");
                    for file in temps {
                        screen.emit(&format!(
                            "path_sha256={} temp_id={}\n",
                            file.path_sha256.as_deref().unwrap_or("unknown"),
                            file.temp_id.as_deref().unwrap_or("unknown")
                        ));
                    }
                }
                RecoveryFinding::RecoveryRequired { failed } => {
                    screen.emit(&format!("recovery_required={failed}\n"))
                }
                RecoveryFinding::StagingAwaitingApproval => {
                    screen.emit("staging_awaiting_approval; publication or temp status unknown\n")
                }
                RecoveryFinding::StagingNotDiscarded => screen.emit("staging_not_discarded\n"),
                RecoveryFinding::WriteGrantNotClosed => screen.emit("write_grant_not_closed\n"),
            }
        }
    }
    Ok(())
}

fn join<T>(handle: &mut Option<JoinHandle<T>>, deadline: Instant, screen: &Shared) -> Option<T> {
    while handle.as_ref().is_some_and(|h| !h.is_finished()) && Instant::now() < deadline {
        screen.wait();
    }
    if handle.as_ref().is_some_and(|h| h.is_finished()) {
        handle.take()?.join().ok()
    } else {
        None
    }
}

/// Run one Linux process session. The binary maps false to nonzero exit.
pub fn run(args: impl Iterator<Item = String>) -> bool {
    let config = match parse(args) {
        Ok(c) => c,
        Err(code) => {
            eprintln!("{code}");
            return false;
        }
    };
    let mut terminal = match Terminal::open() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}", e.code());
            return false;
        }
    };
    if !nexus_governed_control::files::make_non_dumpable() {
        terminal.shared.emit("process_non_dumpable_unavailable\n");
        return false;
    }
    let screen = terminal.shared.clone();
    let confirmer = terminal.confirmer.clone();
    let (tx, rx) = mpsc::channel();
    let startup_screen = screen.clone();
    let mut startup = match std::thread::Builder::new()
        .name("hands-startup".into())
        .spawn(move || {
            #[cfg(test)]
            let opened = hands_tests::open_home(&config.home, confirmer.clone());
            #[cfg(not(test))]
            let opened = Home::open(&config.home, confirmer.clone());
            let opened = opened.map(Arc::new).map_err(|e| {
                startup_screen.emit(&format!("Home unavailable: {}\n", e.0));
                "home_unavailable"
            });
            let runtime = opened.and_then(|home| {
                if !startup_screen.stop.bind(home.owner().clone()) {
                    return Err("stop_binding_failed");
                }
                show_recovery(&home, &startup_screen)?;
                if startup_screen.stop.gate.admit(Route::Files) {
                    home.owner()
                        .hold_files_before_publish(config.hold)
                        .map_err(|_| "hold_refused")?;
                    home.owner()
                        .register_workspace(&config.workspace, &[home.dir()])
                        .map_err(|_| "workspace_refused")?;
                }
                Ok(Runtime {
                    home,
                    config,
                    screen: startup_screen,
                    confirmer,
                    used: false,
                    mission: None,
                })
            });
            let _sent = tx.send(runtime);
        }) {
        Ok(h) => Some(h),
        Err(_) => {
            screen.stop.gate.fail();
            return false;
        }
    };
    let runtime = loop {
        match rx.try_recv() {
            Ok(Ok(runtime)) => break Some(Arc::new(Mutex::new(runtime))),
            Ok(Err(code)) => {
                screen.emit(&format!("STARTUP REFUSED {code}\n"));
                break None;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                screen.stop.gate.fail();
                break None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if screen.ended() || screen.stop.gate.failed() || screen.input().quit {
            break None;
        }
        screen.wait();
    };
    let Some(runtime) = runtime else {
        screen.stop.gate.close();
        let deadline = Instant::now() + Duration::from_secs(3);
        let started = join(&mut startup, deadline, &screen).is_some();
        let stopped = screen.stop.join_until(deadline);
        if started && stopped {
            // A late startup result and the Stop callback may own the final
            // backend reference. Their destruction can perform cleanup too.
            let owned = std::mem::ManuallyDrop::new((rx, screen.stop.take_body()));
            let mut dispose = std::thread::Builder::new()
                .name("hands-startup-close".into())
                .spawn(move || drop(std::mem::ManuallyDrop::into_inner(owned)))
                .ok();
            let _disposed = join(&mut dispose, deadline, &screen);
        } else {
            std::mem::forget(rx);
            std::mem::forget(screen.stop.clone());
        }
        let _terminal = terminal.finish(deadline);
        return false;
    };
    let _started = join(
        &mut startup,
        Instant::now() + Duration::from_secs(3),
        &screen,
    );
    let owner = runtime.lock().unwrap().home.owner().clone();
    screen.emit("One File mission per fresh process. Emergency ends mission work.\nHANDS READY\n");
    let mut work: Option<JoinHandle<()>> = None;
    let mut queued = std::collections::VecDeque::new();
    let mut last = screen.stop.receipt();
    let mut snapshot = owner.stop_snapshot();
    loop {
        screen.stop.reap();
        if work.as_ref().is_some_and(|w| w.is_finished())
            && join(&mut work, Instant::now(), &screen).is_none()
        {
            screen.stop.gate.fail();
        }
        let receipt = screen.stop.receipt();
        if receipt.epoch != last.epoch {
            screen.emit(&format!(
                "STOP REQUEST RECEIVED epoch={}\nRECOVERY ONLY\n",
                receipt.epoch
            ));
        }
        if receipt.coalesced != last.coalesced {
            screen.emit("STOP COALESCED\n");
        }
        let current = owner.stop_snapshot();
        if current != snapshot || receipt.epoch != last.epoch {
            screen.emit(&format!(
                "kernel_stopped={} broker_stopped={} stop_number={} generation={}\n",
                current.kernel_stopped,
                current.broker_stopped,
                current.stop_number,
                current.policy_generation
            ));
        }
        if receipt.finished != last.finished {
            screen.emit(&format!("STOP RECORDING COMPLETED count={} (durability requires evidence health)\nSTOP WORKERS outstanding={}\n", receipt.finished, receipt.outstanding));
        }
        last = receipt;
        snapshot = current;
        if screen.ended() || screen.stop.gate.failed() || screen.input().quit {
            break;
        }
        let command = if work.is_none() && !queued.is_empty() {
            queued.pop_front()
        } else {
            screen.command()
        };
        if let Some(command) = command {
            if command == "quit" {
                break;
            }
            if command == "status" {
                screen.emit(&format!("host_recovery_only={}\nkernel_stopped={} broker_stopped={} stop_number={} generation={}\nSTOP WORKERS outstanding={}\n", screen.stop.gate.recovery(), current.kernel_stopped, current.broker_stopped, current.stop_number, current.policy_generation, receipt.outstanding));
                screen.emit("Mother lifecycle is the last worker receipt; status does not wait for journal I/O\n");
                let [mission, door, run, files, lane, inference, model] =
                    screen.stop.gate.admitted();
                screen.emit(&format!("ADMITTED mission={mission} door={door} run={run} files={files} lane={lane} inference={inference} model={model}\n"));
                screen.emit("STATUS END\n");
            } else if command == "lift-for-undo" && receipt.outstanding > 0 {
                screen.emit("REFUSED stop_workers_outstanding\n");
            } else if matches!(command.as_str(), "run" | "approve") && screen.stop.gate.recovery() {
                screen.emit("REFUSED recovery_only\n");
            } else if work.is_some() {
                if queued.len() == 64 {
                    screen.stop.gate.fail();
                } else {
                    queued.push_back(command);
                }
            } else {
                let runtime = runtime.clone();
                match std::thread::Builder::new()
                    .name("hands-work".into())
                    .spawn(move || match runtime.lock() {
                        Ok(mut runtime) => runtime.command(&command),
                        Err(p) => p.into_inner().screen.stop.gate.fail(),
                    }) {
                    Ok(h) => work = Some(h),
                    Err(_) => screen.stop.gate.fail(),
                }
            }
        }
        screen.wait();
    }
    screen.stop.gate.close();
    if screen.ended() || screen.stop.gate.failed() {
        screen.stop.press();
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    let reader_done = terminal.stop_reader(deadline);
    let had_work = work.is_some();
    let work_done = join(&mut work, deadline, &screen).is_some() || !had_work;
    let stops_done = screen.stop.join_until(deadline);
    let mut clean = false;
    if reader_done && work_done && stops_done && !screen.ended() && !screen.stop.gate.failed() {
        let body = screen.stop.take_body();
        // If spawning fails, dropping the rejected closure must not run the
        // last backend destructor on the deadline thread.
        let owned = std::mem::ManuallyDrop::new((runtime, owner, body));
        let mut close = std::thread::Builder::new()
            .name("hands-close".into())
            .spawn(move || {
                let (runtime, owner, body) = std::mem::ManuallyDrop::into_inner(owned);
                match Arc::try_unwrap(runtime)
                    .ok()
                    .and_then(|r| r.into_inner().ok())
                {
                    Some(runtime) => runtime.close(owner, body),
                    None => false,
                }
            })
            .ok();
        clean = join(&mut close, deadline, &screen).unwrap_or(false);
        if close.is_some() {
            screen.emit("UNCLEAN outstanding=close\n");
        }
    } else {
        screen.emit(&format!(
            "UNCLEAN outstanding={}\n",
            screen.stop.receipt().outstanding + usize::from(!work_done)
        ));
        // Unclean bounded process exit: these references deliberately stay
        // owned for the process lifetime. Destructors must not turn a timeout
        // into an unbounded main-thread filesystem wait. No post-exit records
        // or cleanup are promised; the operating system reclaims the process.
        std::mem::forget(runtime);
        std::mem::forget(owner);
    }
    let terminal_done = terminal.finish(deadline);
    clean && terminal_done && !screen.stop.gate.failed()
}

/// Keep successful removal and its possible grant interruption distinct.
pub(crate) fn show_undo(screen: &Shared, report: nexus_governed_control::files::UndoReport) {
    screen.emit(&format!(
        "UNDO REMOVED interrupted={}\n",
        report.interrupted.unwrap_or("none")
    ));
}
