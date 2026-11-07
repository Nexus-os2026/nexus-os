//! Deterministic, one-mission Hands host. Raw Owner handles never escape it.
use crate::session_home::SessionServices;
use crate::terminal::{Shared, Terminal};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::evidence::EvidencePhase;
use nexus_governed_control::authority::ids::CommitmentId;
use nexus_governed_control::files::{FileCreate, RecoveryFinding};
use nexus_governed_control::host_session::{
    close::{CloseSeal, Producer, ProducerKind},
    CommandKind, SessionRequest, SessionSeed, SessionState,
};
use nexus_mother::adapter::SessionPort;
use nexus_mother::contract::{Budget, EffectKind, MissionContract, PassCriterion};
use nexus_mother::coordinator::{
    Coordinator, CoordinatorError, FileSession, Lanes, PreparedSessionEnd,
};
use nexus_mother::effect::{EffectPort, EffectRequest};
use nexus_mother::journal::MissionId;
use nexus_mother::lane::{
    Lane, LaneIdentity, LaneInput, LaneOutput, LaneRole, ScriptedLane, Verdict,
};
use nexus_mother::state::MissionState;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[cfg(test)]
use crate::hands_tests;

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

// The coordinator performs actual lane admission. This test-only observer
// retains the original pause after a real scripted lane returns.
struct ObservedLane {
    route: Arc<crate::stop::StopRoute>,
    lane: ScriptedLane,
}
impl Lane for ObservedLane {
    fn identity(&self) -> LaneIdentity {
        self.lane.identity()
    }
    fn invoke(&self, input: &LaneInput) -> LaneOutput {
        let output = self.lane.invoke(input);
        #[cfg(test)]
        hands_tests::before_internal_dispatch(&self.route, self.lane.identity().role);
        let _session = &self.route;
        output
    }
}

struct Mission {
    session: FileSession<SessionPort>,
    port: Arc<SessionPort>,
    id: Option<MissionId>,
    last: Option<CommitmentId>,
}
struct Runtime {
    home: Arc<SessionServices>,
    config: Config,
    screen: Arc<Shared>,
    confirmer: Arc<crate::terminal::TerminalConfirmer>,
    used: bool,
    mission: Option<Mission>,
}
impl Runtime {
    fn run(
        &mut self,
        request: &SessionRequest,
        confirmation: Arc<crate::terminal::TerminalConfirmer>,
    ) {
        if self.used {
            self.screen.emit("REFUSED file_session_used\n");
            return;
        }
        self.used = true;
        let Some(workspace) = self.home.session.workspace() else {
            self.screen.emit("REFUSED workspace_unavailable\n");
            return;
        };
        let door = match self
            .home
            .session
            .open_confirmed_file_door(request, confirmation)
        {
            Ok(door) => door,
            Err(_) => {
                self.screen.emit("DOOR REFUSED request_unavailable\n");
                return;
            }
        };
        let context = door.context();
        let port = Arc::new(SessionPort::new(door, workspace.name.clone()));
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
                Arc::new(ObservedLane {
                    route: self.screen.stop.clone(),
                    lane,
                }) as Arc<dyn Lane>
            })
            .collect();
        let result = (|| {
            let journal = self.home.journal().map_err(|_| "journal_unavailable")?;
            let private = self
                .home
                .private(context.clone())
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
        let contract = MissionContract {
            model: None,
            notes: vec![],
            deliverable: None,
        page_facts: None,
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
            #[cfg(test)]
            hands_tests::review2_before_step(&self.screen, &self.home, "draft");
            let drafted = mission.session.draft(contract);
            #[cfg(test)]
            hands_tests::review2_after_step(&self.screen, &self.home, "draft", drafted.is_err());
            let id = drafted?;
            mission.id = Some(id);
            #[cfg(test)]
            hands_tests::review2_before_step(&self.screen, &self.home, "submit");
            let submitted = mission.session.submit(id);
            #[cfg(test)]
            hands_tests::review2_after_step(&self.screen, &self.home, "submit", submitted.is_err());
            submitted?;
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
            let Some(mission) = self.mission.as_ref() else {
                return;
            };
            let Some(id) = mission.id else {
                return;
            };
            #[cfg(test)]
            hands_tests::before_internal_admission(&self.screen, &self.home);
            let advanced = mission.session.advance(id);
            #[cfg(test)]
            hands_tests::after_internal_admission(&mission.port, &self.home, id);
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
                        // Done is emitted by observe, together with its notice.
                        // Preserve every other existing technical line.
                        if report.to != MissionState::Done {
                            self.screen
                                .emit(&format!("MISSION {}\n", report.to.as_str()));
                        }
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
        self.screen.stop.ingress.fail();
        self.screen.emit("REFUSED progress_bound\n");
    }
    fn approve(
        &mut self,
        request: &SessionRequest,
        confirmation: Arc<crate::terminal::TerminalConfirmer>,
    ) {
        let Some(mission) = self.mission.as_mut() else {
            self.screen.emit("REFUSED no_mission\n");
            return;
        };
        mission.last = mission
            .id
            .and_then(|id| mission.port.pending(id))
            .and_then(|p| CommitmentId::parse(&p.commitment));
        let result = self
            .home
            .session
            .approve_file_confirmed(request, &*confirmation);
        let affirmative = confirmation.answer() == crate::terminal::AnswerState::Affirmative;
        if result.is_ok() {
            let continuation = self.home.session.approval_continuation(request);
            if let Ok(continuation) = continuation {
                let old = self.mission.take().expect("installed mission");
                match old.session.continue_approval(continuation) {
                    Ok(session) => self.mission = Some(Mission { session, ..old }),
                    Err(e) => {
                        self.screen.stop.ingress.fail();
                        self.screen.emit(&format!(
                            "ACTION OUTCOME UNCERTAIN continuation={}\n",
                            mother_code(&e)
                        ));
                        return;
                    }
                }
            }
        }
        if let Ok(output) = &result {
            self.screen.emit(&format!(
                "ACTION FINISHED {}\n",
                output
                    .evidence
                    .as_ref()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "evidence unavailable".into())
            ));
        }
        // Advance/reconcile the real Mother outcome. There is no automatic deny
        // conversion on an authorization failure.
        self.progress();
        let consumed_decline = confirmation.answer() == crate::terminal::AnswerState::Declined
            && self
                .mission
                .as_ref()
                .and_then(|m| m.id.map(|id| (m, id)))
                .is_some_and(|(m, id)| {
                    matches!(m.session.state(id), Ok(Some(MissionState::Denied)))
                });
        match result {
            Ok(_) => {}
            Err(_) if consumed_decline => self.screen.emit("ACTION DENIED\n"),
            Err(e) => {
                use nexus_governed_control::authority::commitment::PublicationClass;
                let uncertain = self
                    .home
                    .session
                    .operation_observation(request)
                    .map(|facts| {
                        facts.actions.iter().any(|a| {
                            matches!(
                                a.publication,
                                PublicationClass::Pending | PublicationClass::Uncertain
                            )
                        })
                    })
                    .unwrap_or(true);
                let label = if uncertain {
                    "ACTION OUTCOME UNCERTAIN"
                } else {
                    "ACTION REFUSED"
                };
                self.screen.emit(&format!(
                    "{label} {} affirmative={affirmative}\n",
                    e.class()
                ));
            }
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
    fn command(&mut self, request: SessionRequest) {
        let confirmation = self.confirmer.for_request(&request);
        self.command_body(&request, confirmation.clone());
        self.operation_result(&request, &confirmation);
    }
    fn command_body(
        &mut self,
        request: &SessionRequest,
        confirmation: Arc<crate::terminal::TerminalConfirmer>,
    ) {
        match request.kind() {
            CommandKind::Run => self.run(request, confirmation.clone()),
            CommandKind::Approve => self.approve(request, confirmation.clone()),
            CommandKind::Undo => {
                let result = self.home.session.undo_confirmed(request, &*confirmation);
                #[cfg(test)]
                hands_tests::review4_returned(&self.screen, &self.home, "undo");
                let facts = request.restore_observation();
                use nexus_governed_control::host_session::outcome::RestoreOutcome;
                if confirmation.answer() != crate::terminal::AnswerState::Affirmative
                    && !facts.handed_off
                {
                    self.screen
                        .emit("Undo was not confirmed. No restore was handed off.\n");
                }
                if facts.interrupted {
                    self.screen.emit(match facts.outcome {
                        RestoreOutcome::NotEntered => "Stop received before undo handoff. The restore did not start.\n",
                        RestoreOutcome::Completed => "Stop received after undo handoff. The restore completed; remaining obligations are reported separately.\n",
                        RestoreOutcome::Refused => "Stop received after undo handoff. The restore was refused.\n",
                        RestoreOutcome::Pending | RestoreOutcome::Uncertain => "Stop received after undo handoff. The restore outcome is uncertain; recovery is required.\n",
                    });
                }
                self.screen.emit(&format!(
                    "UNDO FACTS request={} handoff={} physical={:?} grant={:?}\n",
                    facts.request, facts.handed_off, facts.outcome, facts.grant
                ));
                match result {
                    Ok(report) => {
                        show_undo(&self.screen, report);
                    }
                    Err(e) => self.screen.emit(&format!("UNDO REFUSED {}\n", e.class())),
                }
                #[cfg(test)]
                hands_tests::review4_reported(&self.screen, &self.home, "undo");
            }
            CommandKind::Cleanup => {
                let result = self.home.session.cleanup(request);
                #[cfg(test)]
                hands_tests::review4_returned(&self.screen, &self.home, "cleanup");
                match result {
                    Ok(report) => self.screen.emit(&format!(
                        "Cleanup requested.\nCLEANUP settled={} still_owed={}\n",
                        report.settled, report.still_owed
                    )),
                    Err(e) => self
                        .screen
                        .emit(&format!("CLEANUP REFUSED {}\n", e.class())),
                }
                self.inventory();
                #[cfg(test)]
                hands_tests::review4_reported(&self.screen, &self.home, "cleanup");
            }
            CommandKind::Evidence => {
                self.screen.emit("RECENT EVIDENCE bounded\n");
                for record in self.home.recent_evidence() {
                    let phase = if record.phase == EvidencePhase::EmergencyStop {
                        "emergency_stopped"
                    } else {
                        record.phase.as_str()
                    };
                    let reason = record
                        .detail
                        .iter()
                        .find(|(key, _)| key == "reason")
                        .map(|(_, value)| value.as_str())
                        .unwrap_or("none");
                    self.screen.emit(&format!(
                        "phase={phase} cancelled={} failure={} reason={}\n",
                        record.cancelled,
                        record.failure.unwrap_or("none"),
                        reason
                    ));
                }
                let health = self.home.evidence_health();
                self.screen.emit(&format!("latched={} owed={} recovery_required={} unsettled={} unclean_sessions={}\nEVIDENCE HEALTH\n", health.latched, health.owed, health.recovery_required, health.unsettled, health.unclean_sessions));
                self.inventory();
            }
            CommandKind::Detail => {
                if self.home.session.state() != SessionState::Normal {
                    self.screen.emit("REFUSED recovery_only\n");
                    return;
                }
                let mut view = OwnerScreen(self.screen.clone());
                if let Some(id) = self.mission.as_ref().and_then(|m| m.last) {
                    match self.home.session.file_working_detail(request, id) {
                        Ok(Some(detail)) => detail.show(&mut view),
                        Err(_) => self.screen.emit("DETAIL FAILED native_observation\n"),
                        _ => {}
                    }
                }
                if let Some((m, id)) = self.mission.as_ref().and_then(|m| m.id.map(|id| (m, id))) {
                    match m.session.working_detail(id) {
                        Ok(Some(detail)) => detail.show(&mut view),
                        Err(e) => self
                            .screen
                            .emit(&format!("DETAIL FAILED {}\n", mother_code(&e))),
                        _ => {}
                    }
                }
            }
            _ => self.screen.emit("REFUSED unavailable_command\n"),
        }
    }
    fn operation_result(
        &self,
        request: &SessionRequest,
        confirmation: &crate::terminal::TerminalConfirmer,
    ) {
        let answer = confirmation.answer();
        let [grant, door, mission, action, undo] = confirmation.answers();
        // One observation supplies both the Owner result and technical block.
        let observation = self.home.session.operation_observation(request);
        let terminal = self
            .mission
            .as_ref()
            .and_then(|m| m.id.map(|id| m.session.terminal_facts(id)));
        self.screen.emit(&format!(
            "RESULT: {}\n",
            owner_result(
                request.kind(),
                answer,
                observation.as_ref().ok(),
                terminal.as_ref().and_then(|t| t.as_ref().ok()),
                &self.config.name,
            )
        ));
        self.screen.emit(&format!(
            "OPERATION {:?} request={} target={:?} answer={answer:?}\n",
            request.kind(),
            request.sequence(),
            request.context()
        ));
        self.screen.emit(&format!("ANSWERS grant={grant:?} door={door:?} mission={mission:?} action={action:?} undo={undo:?}\n"));
        match observation {
            Ok(facts) => {
                self.screen.emit(&format!("ADMISSION steps={} apply_handoff={} restore_handoff={} generation_changes={} first_restriction={:?}\n",
                    facts.admitted_steps, facts.apply_handed_off, facts.restore.handed_off, facts.ingresses, facts.first_restriction));
                self.screen.emit(&format!("NATIVE FACTS restore={:?} grant={:?} kernel_recording={:?} primary_recording={:?}\n",
                    facts.restore.outcome, facts.restore.grant, facts.kernel, facts.primary));
                for call in facts.calls {
                    self.screen.emit(&format!(
                        "CALL {} handed_off={} returned={} failed={}\n",
                        call.call, call.entered, call.returned, call.failed
                    ));
                }
                self.screen.emit(&format!(
                    "PRIMARY HEALTH latched={} owed={} recovery_required={} unsettled={}\n",
                    facts.primary_health.latched,
                    facts.primary_health.owed,
                    facts.primary_health.recovery_required,
                    facts.primary_health.unsettled
                ));
                for action in facts.actions {
                    self.screen.emit(&format!("ACTION FACTS physical={:?} mother={:?} acknowledgement={:?} target={} run={} mission={:?} effect={:?} commitment={:?} mother_sequence={:?}\n",
                        action.publication, action.mother, action.acknowledgement, action.target, action.run, action.mission, action.effect, action.commitment, action.mother_sequence));
                }
                if facts.inventory.is_none() || facts.resources.is_none() {
                    self.screen.emit("RESOURCE OBSERVATION unknown\n");
                }
            }
            Err(_) => self.screen.emit("OPERATION FACTS unknown\n"),
        }
        if let Some(terminal) = terminal {
            match terminal {
                Ok(facts) => {
                    if facts.recording_unknown {
                        self.screen.stop.ingress.fail();
                    }
                    self.screen.emit(&format!("TERMINAL first_intent={} state={:?} settlement={:?} recorded_sequence={:?} recording_unknown={}\n",
                        facts.first_code(), facts.state, facts.settlement, facts.recorded_sequence, facts.recording_unknown));
                }
                Err(_) => self
                    .screen
                    .emit("TERMINAL first_intent=unknown recording_unknown=true\n"),
            }
        }
        self.inventory();
        self.screen.emit("OPERATION END\n");
    }
    fn inventory(&self) {
        match self.home.session.inventory() {
            Ok(inventory) => {
                let entries = &inventory.entries;
                self.screen.emit(&format!(
                    "INVENTORY entries={} in_flight={} uncertain={} unresolved={}\n",
                    entries.len(),
                    entries.iter().filter(|e| e.in_flight).count(),
                    entries.iter().filter(|e| e.uncertain).count(),
                    entries.iter().filter(|e| !e.discharged).count()
                ));
                self.screen.emit(&format!(
                    "CLEANUP EXCLUSIONS preparing={} current_undo={} retained={} excluded={}\n",
                    entries.iter().filter(|e| e.preparing).count(),
                    entries.iter().filter(|e| e.current_undo).count(),
                    entries.iter().filter(|e| e.intentionally_retained).count(),
                    entries
                        .iter()
                        .filter(|e| e.preparing
                            || e.in_flight
                            || e.current_undo
                            || e.uncertain
                            || e.intentionally_retained)
                        .count()
                ));
                self.screen.emit(&format!("RESOURCE FACTS observed_absent={} finalization_unverified={} grants_unclosed={} cleanup_record_unverified={}\n", entries.iter().filter(|e| e.staging_absent).count(), entries.iter().filter(|e| !e.private_finalization_established || !e.parent_synced).count(), entries.iter().filter(|e| !e.grants_closed).count(), entries.iter().filter(|e| !e.cleanup_recorded).count()));
                match self.home.session.resource_obligations() {
                    Ok(resources) => self.screen.emit(&format!("RESERVATIONS total={} unresolved={} uncertain={} private={} private_unresolved={}\n", resources.reserved, resources.unresolved, resources.uncertain, resources.private, resources.private_unresolved)),
                    Err(_) => self.screen.emit("RESERVATIONS unknown\n"),
                }
                for debt in self.screen.stop.delivery_debt() {
                    self.screen.emit(&format!(
                        "STOP DELIVERY event={} ingress_delivered={} outcome_delivered={}\n",
                        debt.event, debt.ingress_delivered, debt.outcome_delivered
                    ));
                }
                self.screen.emit("INVENTORY END\n");
            }
            Err(_) => self.screen.emit("INVENTORY unknown\n"),
        }
    }
    fn close(mut self, request: SessionRequest) -> (Option<CloseSeal>, String) {
        let result = (|| {
            let end = match self.mission.take() {
                Some(mission) => mission.session.prepare_session_end(),
                None => self
                    .home
                    .journal()
                    .map_err(CoordinatorError::Journal)
                    .and_then(|j| PreparedSessionEnd::idle(j, self.home.session.identity())),
            }
            .map_err(|_| "mother_settlement")?;
            self.home
                .session
                .settle_close_resources(&request)
                .map_err(|_| "resource_finalization")?;
            self.home
                .session
                .settle_close_authority(&request)
                .map_err(|_| "authority_finalization")?;
            let seal = self.home.session.seal_close(&request).map_err(|_| "seal")?;
            end.record(&seal).map_err(|_| "mother_end")?;
            self.home
                .session
                .observe_mother_end(&seal)
                .map_err(|_| "mother_end_observation")?;
            self.home
                .session
                .close_primary(&seal)
                .map_err(|_| "primary_end")?;
            Ok(seal)
        })();
        match result {
            Ok(seal) => (Some(seal), self.close_report("delivery_pending")),
            Err(stage) => (None, self.close_report(stage)),
        }
    }
    fn close_report(&self, stage: &str) -> String {
        // This runs only on the bounded close worker, never on the deadline
        // delivery path. A held observer yields an explicit unknown report.
        let mut report = format!("CLOSE FAILED stage={stage}\n");
        match self.home.session.resource_obligations() {
            Ok(r) => report.push_str(&format!("CLOSE OBLIGATIONS reservations_unresolved={} uncertain={} private={} private_unresolved={}\n", r.unresolved, r.uncertain, r.private, r.private_unresolved)),
            Err(_) => report.push_str(CLOSE_UNKNOWN),
        }
        match self.home.session.inventory() {
            Ok(inventory) => {
                for e in inventory.entries {
                    report.push_str(&format!("CLOSE RESOURCE request={} in_flight={} uncertain={} discharged={} staging_absent={} private_finalization_established={} parent_synced={} grants_closed={} cleanup_recorded={} intentionally_retained={}\n", e.request, e.in_flight, e.uncertain, e.discharged, e.staging_absent, e.private_finalization_established, e.parent_synced, e.grants_closed, e.cleanup_recorded, e.intentionally_retained));
                }
            }
            Err(_) => report.push_str(CLOSE_UNKNOWN),
        }
        match self.home.session.close_obligations() {
            Ok(o) => report.push_str(&format!("CLOSE AUTHORITY closed={} actions_unacknowledged={} producers_live={} producers_lost={} halt_pending={} halt_owed={} halt_uncertain={} halt_lost={}\n", o.authority_closed, o.actions_unacknowledged, o.producers_live, o.producers_lost, o.halt.pending, o.halt.owed, o.halt.uncertain, o.halt.lost)),
            Err(_) => report.push_str("CLOSE AUTHORITY unknown\n"),
        }
        match self.home.session.stop_obligations() {
            Ok(o) => report.push_str(&format!("CLOSE STOP received={} recorded={} pending={} in_flight={} owed={} uncertain={} failed={} overflowed={}\n", o.received, o.recorded, o.pending, o.in_flight, o.owed, o.uncertain, o.failed, o.overflowed)),
            Err(_) => report.push_str("CLOSE STOP unknown\n"),
        }
        let h = self.home.evidence_health();
        report.push_str(&format!("CLOSE PRIMARY latched={} owed={} recovery_required={} unsettled={} unclean_sessions={}\n", h.latched, h.owed, h.recovery_required, h.unsettled, h.unclean_sessions));
        report
    }
}

/// Plain-language projection only; no authority, reconciliation or new reads.
fn owner_result(
    kind: CommandKind,
    answer: crate::terminal::AnswerState,
    facts: Option<&nexus_governed_control::host_session::outcome::OperationObservation>,
    terminal: Option<&nexus_mother::coordinator::TerminalFacts>,
    name: &str,
) -> String {
    use crate::terminal::AnswerState;
    use nexus_governed_control::authority::commitment::PublicationClass;
    use nexus_governed_control::host_session::outcome::{
        Acknowledgement, GrantClosure, RecordingStatus, RestoreOutcome,
    };
    let Some(facts) = facts else {
        return "the operation outcome is uncertain; facts are unavailable.".into();
    };
    let healthy = !facts.primary_health.latched
        && facts.primary_health.owed == 0
        && !facts.primary_health.recovery_required
        && facts.primary_health.unsettled == 0
        && facts.inventory.is_some()
        && facts.resources.is_some();
    let terminal_recorded =
        terminal.is_some_and(|t| !t.recording_unknown && t.recorded_sequence.is_some());
    match kind {
        CommandKind::Run
            if healthy
                && terminal.is_some_and(|t| {
                    t.state == Some(MissionState::AwaitingApproval) && !t.recording_unknown
                }) =>
        {
            format!("waiting for your approval of {name} (type approve).")
        }
        CommandKind::Approve => {
            if !facts.apply_handed_off
                && facts.actions.iter().all(|a| {
                    matches!(
                        a.publication,
                        PublicationClass::ConfirmedNotPublished | PublicationClass::Pending
                    )
                })
            {
                return "refused - nothing was created.".into();
            }
            let published = facts.actions.len() == 1
                && matches!(facts.actions[0].publication, PublicationClass::Published { recovery_required: false, ref interrupted } if interrupted.is_empty());
            let done = terminal.is_some_and(|t| t.state == Some(MissionState::Done));
            if published {
                let action = &facts.actions[0];
                if done
                    && healthy
                    && facts.kernel == RecordingStatus::Recorded
                    && answer == AnswerState::Affirmative
                    && terminal_recorded
                    && facts.primary == RecordingStatus::Recorded
                    && action.mother == RecordingStatus::Recorded
                    && action.acknowledgement == Acknowledgement::Verified
                {
                    return format!("{name} was created, recorded and verified.");
                }
                return "the file was created, but is not yet fully recorded and acknowledged; verification is uncertain."
                    .into();
            }
            if facts
                .actions
                .iter()
                .all(|a| a.publication == PublicationClass::ConfirmedNotPublished)
                && !facts.actions.is_empty()
            {
                return "refused - nothing was created.".into();
            }
            "the file outcome is uncertain; recovery or recording obligations remain.".into()
        }
        CommandKind::Undo => match facts.restore.outcome {
            RestoreOutcome::Completed
                if facts.restore.grant == GrantClosure::Closed
                    && facts.kernel == RecordingStatus::Recorded
                    && healthy =>
            {
                if facts.primary == RecordingStatus::Recorded {
                    "the approved file was removed; restore completed and was recorded.".into()
                } else {
                    "the approved file was removed; restore completed, but other recording status is uncertain.".into()
                }
            }
            RestoreOutcome::Completed => {
                "the approved file was removed; grant closure or recording is uncertain.".into()
            }
            RestoreOutcome::NotEntered => "undo was refused; no restore was handed off.".into(),
            RestoreOutcome::Refused => {
                "the restore was refused; remaining obligations are reported below.".into()
            }
            RestoreOutcome::Pending | RestoreOutcome::Uncertain => {
                "the restore outcome is uncertain; recovery is required.".into()
            }
        },
        CommandKind::Cleanup => {
            "cleanup was requested; resource and recording obligations are reported below.".into()
        }
        CommandKind::Run if !facts.apply_handed_off => "refused - nothing was created.".into(),
        CommandKind::Evidence => {
            "the available evidence and its recording health are shown above.".into()
        }
        CommandKind::Detail => "the available working detail is shown above.".into(),
        _ => "the operation outcome is uncertain; see the facts below.".into(),
    }
}

const CLOSE_UNKNOWN: &str =
    "CLOSE OBLIGATIONS resource_snapshot=unknown in_flight_or_unknown=true\n";

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

fn show_recovery(home: &SessionServices, screen: &Shared) -> Result<(), &'static str> {
    let sources = home.session.startup_report();
    screen.emit(&format!(
        "SOURCES mother_verified={} resources_verified={} primary_latched={} primary_owed={} primary_recovery_required={} primary_unsettled={} primary_unclean_sessions={}\n",
        sources.mother.is_ok(),
        sources.resources.is_ok(),
        sources.primary.latched,
        sources.primary.owed,
        sources.primary.recovery_required,
        sources.primary.unsettled,
        sources.primary.unclean_sessions
    ));
    // Presentation only: never infer authority or readiness from this label.
    // Pre-open provenance is required: healthy post-open sources can still
    // contain primary-only history or a schema committed before session_opened.
    // Unproven freshness always retains both recovery disclosures.
    let fresh = home.initially_empty
        && sources.ready()
        && sources.mother.as_ref().is_ok_and(|m| m.records == 0)
        && sources.resources.as_ref().is_ok_and(Vec::is_empty);
    if fresh {
        screen.emit("Fresh start: no earlier session in this home; nothing to recover.\n");
    } else {
        screen.emit("Recovery report only. Mother has not removed files or staging, restored workspace permission, or restored undo. Some unfinished work may be known only from incomplete evidence.\n");
    }
    if sources.mother.is_err() {
        screen.emit("RECOVERY FAILED mother_history\n");
    }
    if sources.resources.is_err() {
        screen.emit("RECOVERY FAILED kernel_resources\n");
    }
    if let Ok(mother) = &sources.mother {
        for mission in &mother.missions {
            let (terminal, sequence, effect) = mission
                .terminal
                .map(|(name, seq, effect)| (name, seq.to_string(), effect))
                .unwrap_or(("unrecorded", "unavailable".into(), None));
            let prefix = format!(
                "RECORDED MOTHER mission={} state={} terminal={} terminal_seq={}",
                mission.mission, mission.state, terminal, sequence
            );
            if !mission
                .actions
                .iter()
                .any(|action| Some(action.effect) == effect)
            {
                screen.emit(&format!("{prefix} prior_acknowledgement=unavailable\n"));
            }
            for action in &mission.actions {
                let association = if Some(action.effect) == effect {
                    prefix.clone()
                } else {
                    format!("RECORDED ACTION mission={}", mission.mission)
                };
                screen.emit(&format!("{association} effect={} run={} commitment={} prior_acknowledgement=unavailable\n", action.effect, action.run, action.commitment));
            }
        }
    }
    if let Ok(resources) = &sources.resources {
        for r in resources {
            screen.emit(&format!("RECORDED RESOURCE run={} allocation_attempted={} allocated={} awaiting_approval={} discard_recorded={} publication_recorded={} restoration_recorded={} current_existence=unobserved\n", r.run, r.allocation_attempted, r.allocated, r.awaiting_approval, r.discard_recorded, r.publication_recorded, r.restoration_recorded));
        }
    }
    let report = sources.kernel.map_err(|_| {
        screen.emit("RECOVERY FAILED kernel_ledger\n");
        "recovery_report_unavailable"
    })?;
    if !fresh {
        screen.emit("RECOVERY REPORT only; no sweep or workspace restoration\n");
    }
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

// Every pre-finalization stage shares this earlier point in the original
// shutdown budget. A held producer cannot consume truthful timeout delivery.
fn producer_deadline(deadline: Instant) -> Instant {
    deadline
        .checked_sub(Duration::from_millis(250))
        .unwrap_or(deadline)
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

/// Run one process using the original seed created before terminal input.
pub fn run(args: impl Iterator<Item = String>) -> bool {
    let config = match parse(args) {
        Ok(c) => c,
        Err(code) => {
            eprintln!("{code}");
            return false;
        }
    };
    let seed = match SessionSeed::new() {
        Ok(seed) => seed,
        Err(_) => return false,
    };
    let ingress = seed.ingress();
    let mut terminal = match Terminal::open(ingress.clone()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}", e.code());
            return false;
        }
    };
    if !nexus_governed_control::files::make_non_dumpable() {
        terminal.shared.fail(crate::terminal::Failure::Input);
        return false;
    }
    let screen = terminal.shared.clone();
    let confirmer = terminal.confirmer.clone();
    let startup_debt = match ingress.producer(ProducerKind::Startup) {
        Ok(p) => p,
        Err(_) => {
            ingress.fail();
            return false;
        }
    };
    let startup_screen = screen.clone();
    let mut startup = std::thread::Builder::new()
        .name("hands-startup".into())
        .spawn(move || {
            #[cfg(test)]
            hands_tests::review2_startup("home");
            #[cfg(test)]
            let opened = hands_tests::open_home(seed, &config.home, confirmer.clone());
            #[cfg(not(test))]
            let opened = SessionServices::open(seed, &config.home, confirmer.clone());
            let home = Arc::new(opened.map_err(|error| match error.0.as_str() {
                "evidence_unavailable" => "evidence_unavailable",
                "session_unavailable" => "session_unavailable",
                "mother_history" => "mother_history",
                "absolute_path_required" => "absolute_path_required",
                _ => "home_unavailable",
            })?);
            if !startup_screen.stop.bind(home.session.clone()) {
                return Err("stop_binding_failed");
            }
            #[cfg(test)]
            hands_tests::review2_startup("recovery");
            let report = show_recovery(&home, &startup_screen);
            if report.is_err() {
                home.session.ingress().fail();
            }
            if home.session.state() == SessionState::Bootstrap
                && home.session.startup_report().ready()
            {
                let setup = home
                    .session
                    .bootstrap_hold(config.hold)
                    .map_err(|_| "hold_refused")
                    .and_then(|()| {
                        home.session
                            .bootstrap_workspace(&config.workspace, &[home.dir()])
                            .map(|_| ())
                            .map_err(|_| "workspace_refused")
                    })
                    .and_then(|()| {
                        home.session
                            .finish_bootstrap()
                            .map_err(|_| "readiness_refused")
                    });
                if let Err(code) = setup {
                    startup_screen.emit(&format!("STARTUP REFUSED {code}\n"));
                    if !home.session.ingress().emergency_seen()
                        && home.session.ingress().close_deadline().is_none()
                    {
                        home.session.ingress().fail();
                    }
                }
            }
            Ok::<_, &'static str>(Runtime {
                home,
                config,
                screen: startup_screen,
                confirmer,
                used: false,
                mission: None,
            })
        })
        .ok();
    if startup.is_none() {
        ingress.fail();
    }
    while startup.as_ref().is_some_and(|h| !h.is_finished()) {
        if screen.ended()
            || screen.input().quit.is_some()
            || ingress
                .close_deadline()
                .is_some_and(|d| Instant::now() >= producer_deadline(d))
        {
            break;
        }
        screen.wait();
    }
    let deadline = ingress.close_deadline().unwrap_or_else(Instant::now);
    let started = join(&mut startup, producer_deadline(deadline), &screen);
    if startup.is_none() {
        startup_debt.joined();
    } else {
        std::mem::forget(startup_debt);
    }
    let runtime = match started {
        Some(Ok(runtime)) => Arc::new(Mutex::new(runtime)),
        Some(Err(code)) => {
            screen.emit(&format!("STARTUP REFUSED {code}\n"));
            ingress.fail();
            return failed_startup(terminal, startup);
        }
        None => {
            ingress.fail();
            return failed_startup(terminal, startup);
        }
    };
    let session = runtime.lock().unwrap().home.session.clone();
    if !matches!(
        session.state(),
        SessionState::ClosingNormal | SessionState::ClosingRestricted
    ) {
        screen.emit(if session.state() == SessionState::Normal {
            "One File mission per fresh process. Emergency ends mission work.\nHANDS READY\n"
        } else if session.state() == SessionState::Emergency {
            "Report only; no work authority is available.\nHANDS READY\n"
        } else {
            "HANDS REPORT ONLY; sources or startup obligations prevent readiness\n"
        });
    }
    let mut work: Option<JoinHandle<()>> = None;
    let mut work_debt: Option<Producer> = None;
    loop {
        if work.as_ref().is_some_and(|h| h.is_finished()) {
            if join(&mut work, Instant::now(), &screen).is_none() {
                ingress.fail();
            }
            if let Some(debt) = work_debt.take() {
                debt.joined();
            }
        }
        if screen.ended()
            || screen.input().quit.is_some()
            || ingress
                .close_deadline()
                .is_some_and(|d| Instant::now() >= d)
        {
            break;
        }
        let command = screen.command_if_idle(work.is_none());
        if let Some(command) = command {
            if let Some(reason) = command.refusal {
                let subject = if command.name == "undo" { "UNDO " } else { "" };
                if matches!(
                    command.name.as_str(),
                    "run" | "approve" | "undo" | "cleanup"
                ) {
                    screen.emit("RESULT: request refused; no new operation was admitted.\n");
                }
                screen.emit(&format!("{subject}REFUSED {reason}\n"));
                if reason == "unknown_command" {
                    screen.emit("Valid commands: run, approve, status, evidence, detail, stop, undo, cleanup, quit\n");
                }
            } else if command.name == "status" {
                let current = session.stop_snapshot();
                let receipt = screen.stop.receipt();
                screen.emit(&format!("host_recovery_only={}\nsession={:?}\nkernel_stopped={} broker_stopped={} stop_number={} generation={}\nSTOP WORKERS outstanding={}\nSTATUS END\n", ingress.emergency_seen(), session.state(), current.kernel_stopped, current.broker_stopped, current.stop_number, current.policy_generation, receipt.outstanding));
            } else if let Some(request) = command.request {
                let kind = if request.kind() == CommandKind::Evidence {
                    ProducerKind::Observation
                } else {
                    ProducerKind::Work
                };
                match ingress.producer(kind) {
                    Ok(debt) => {
                        let runtime = runtime.clone();
                        match std::thread::Builder::new().name("hands-work".into()).spawn(
                            move || match runtime.lock() {
                                Ok(mut runtime) => runtime.command(request),
                                Err(p) => p.into_inner().screen.stop.ingress.fail(),
                            },
                        ) {
                            Ok(handle) => {
                                work = Some(handle);
                                work_debt = Some(debt);
                            }
                            Err(_) => {
                                ingress.fail();
                                drop(debt);
                            }
                        }
                    }
                    Err(_) => screen.emit("REFUSED session_restricted\n"),
                }
            }
        }
        screen.wait();
    }
    let request = screen.input().quit.take();
    if request.is_none() {
        ingress.fail();
    }
    let deadline = ingress
        .close_deadline()
        .expect("first shutdown trigger established deadline");
    let producers_until = producer_deadline(deadline);
    let reader_done = terminal.stop_reader(producers_until);
    let work_done = work.is_none() || join(&mut work, producers_until, &screen).is_some();
    if work.is_none() {
        if let Some(debt) = work_debt.take() {
            debt.joined();
        }
    }
    let stops_done = screen.stop.join_until(producers_until);
    let producers_done = terminal.quiesce(producers_until);
    let mut seal = None;
    let mut close_report = CLOSE_UNKNOWN.to_owned();
    if reader_done
        && work_done
        && stops_done
        && producers_done
        && !screen.ended()
        && !ingress.failed()
    {
        if let Some(request) = request {
            let owned = std::mem::ManuallyDrop::new(runtime);
            let mut close = std::thread::Builder::new()
                .name("hands-close".into())
                .spawn(move || {
                    let runtime = std::mem::ManuallyDrop::into_inner(owned);
                    Arc::try_unwrap(runtime)
                        .ok()
                        .and_then(|r| r.into_inner().ok())
                        .map(|r| r.close(request))
                })
                .ok();
            // An append shares the producer budget; it cannot borrow the
            // remaining delivery/mode-restoration time or refresh the clock.
            if let Some((result, report)) = join(&mut close, producers_until, &screen).flatten() {
                seal = result;
                close_report = report;
            }
            if close.is_some() {
                std::mem::forget(close);
            }
        } else {
            std::mem::forget(runtime);
        }
    } else {
        std::mem::forget(runtime);
    }
    let modes_ok = terminal.restore_modes(deadline);
    if let Some(seal) = seal {
        if modes_ok {
            // Both end records are durable. Delivery, renderer finalization
            // and the seal's final deadline check still have to succeed.
            let delivered = screen.delivered(
                "RESULT: closing; the final close status follows.\nSESSION CLOSED recorded\n",
                deadline,
            );
            let finished = terminal.finish(deadline);
            if delivered && finished {
                return seal.finish_delivery();
            }
        }
        drop(seal);
    }
    let records = session.close_records();
    let status = |value| match value {
        nexus_governed_control::host_session::close::FinalStatus::Pending => {
            "Unknown (not attempted)"
        }
        nexus_governed_control::host_session::close::FinalStatus::Unknown => "Unknown",
        nexus_governed_control::host_session::close::FinalStatus::Recorded => "Recorded",
        nexus_governed_control::host_session::close::FinalStatus::Failed => "Failed",
    };
    let _delivered = screen.delivered(
        &format!(
            "RESULT: close is uncertain; the full clean-close protocol did not complete.\n{}\nMOTHER END {}\nPRIMARY END {}\nFINAL DELIVERY {}\n{}PRODUCERS outstanding={}\n",
            crate::terminal::UNCLEAN_NOTICE,
            status(records.mother),
            status(records.primary),
            status(records.delivery),
            close_report,
            screen.stop.receipt().outstanding + usize::from(!work_done)
        ),
        deadline,
    );
    let _finished = terminal.finish(deadline);
    // Retained payloads and native owners cannot run final destructors after an
    // unverified bounded exit. No post-exit work or record is promised.
    std::mem::forget(work);
    std::mem::forget(work_debt);
    std::mem::forget(session);
    std::mem::forget(screen);
    false
}
fn failed_startup(
    mut terminal: Terminal,
    startup: Option<JoinHandle<Result<Runtime, &'static str>>>,
) -> bool {
    let screen = terminal.shared.clone();
    screen.stop.ingress.fail();
    let deadline = screen
        .stop
        .ingress
        .close_deadline()
        .expect("failure deadline");
    let producers_until = producer_deadline(deadline);
    let _input = terminal.stop_reader(producers_until);
    let _stops = screen.stop.join_until(producers_until);
    let _modes = terminal.restore_modes(deadline);
    let _delivered = screen.delivered(
        &format!(
            "RESULT: close is uncertain; the full clean-close protocol did not complete.\n{}\n",
            crate::terminal::UNCLEAN_NOTICE
        ),
        deadline,
    );
    let _terminal = terminal.finish(deadline);
    std::mem::forget(startup);
    std::mem::forget(screen);
    false
}

/// Keep successful removal and its possible grant interruption distinct.
pub(crate) fn show_undo(screen: &Shared, report: nexus_governed_control::files::UndoReport) {
    screen.emit(&format!(
        "UNDO REMOVED interrupted={}\n",
        report.interrupted.unwrap_or("none")
    ));
}
