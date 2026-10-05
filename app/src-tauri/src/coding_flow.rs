//! Phase One governed coding flow (P1-08): the only desktop surface that
//! reaches the coding-run primitive.
//!
//! The frontend sends only opaque selectors and choices: a project id issued
//! here, relative scope folder names, the task text and a local model name
//! chosen from the models installed in the operator's loopback Ollama. It
//! never sends a path, a grant or an approval. The backend:
//! - selects projects through the native folder picker it invokes itself;
//! - issues a fresh read grant per run (the run revokes it as soon as it no
//!   longer needs project read authority) and pins the local model;
//! - runs the governed worker and structural verification off the IPC
//!   thread;
//! - shows the owner's native confirmation itself before any apply or
//!   restore, and applies or restores only the exact reviewed candidate.
//!
//! Everything returned to the frontend is display data. A run id names a run
//! in this process; it is never authority by itself. Nothing here runs a
//! process, uses git, or reaches a network destination other than the
//! loopback Ollama address.
//!
//! Phase Two adds governed sandboxed verification of a verified candidate
//! (`coding_flow/verification.rs`): the frontend names only the run and a
//! compiled-in profile; the only execution route is the verifier sandbox.

use serde::Serialize;

/// A project as the frontend sees it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ProjectView {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct StartView {
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct WorkerView {
    pub turns: u32,
    pub files_read: u32,
    pub accepted: Vec<String>,
    pub rejected: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct VerificationView {
    pub passed: bool,
    pub candidate_short: String,
    pub base_short: String,
    pub violations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ChangeView {
    pub path: String,
    pub kind: &'static str,
    pub old_sha256: Option<String>,
    pub new_sha256: Option<String>,
    pub old_size: Option<u64>,
    pub new_size: Option<u64>,
    pub diff: Option<String>,
    pub diff_note: Option<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ReviewView {
    pub binding_short: String,
    pub changes: Vec<ChangeView>,
    /// The sandboxed verification result this review binds (Phase Two).
    pub verification_short: Option<String>,
}

/// A compiled-in verifier profile and whether it applies to a run's
/// candidate.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct VerifierProfileView {
    pub name: &'static str,
    pub display_name: &'static str,
    pub applicable: bool,
    pub reason: Option<String>,
}

/// A finalized sandboxed verification, as bounded display data.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct SandboxResultView {
    pub generation: u64,
    pub profile: Option<&'static str>,
    pub exit: &'static str,
    pub passed: bool,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub duration_ms: u64,
    pub stdout_bytes: u64,
    pub stdout_truncated: bool,
    pub stderr_bytes: u64,
    pub stderr_truncated: bool,
    pub cleanup: &'static str,
    pub result_short: String,
    /// The tail of the output, escaped for display (untrusted data).
    pub stdout_excerpt: Option<String>,
    pub stderr_excerpt: Option<String>,
}

/// Where a run's sandboxed verification stands.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct SandboxVerificationView {
    pub phase: &'static str,
    pub result: Option<SandboxResultView>,
}

/// A run as the frontend sees it. Display data only.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct RunView {
    pub run_id: String,
    pub project_id: String,
    pub project_name: String,
    pub model: String,
    pub stage: &'static str,
    pub message: Option<String>,
    pub run_state: String,
    pub apply_state: String,
    pub worker: Option<WorkerView>,
    pub verification: Option<VerificationView>,
    pub review: Option<ReviewView>,
    pub sandbox_verification: Option<SandboxVerificationView>,
    pub can_apply: bool,
    pub can_restore: bool,
    pub can_discard: bool,
    pub can_verify: bool,
    pub can_retry_verification_cleanup: bool,
}

#[cfg(target_os = "linux")]
pub(crate) use linux::*;

#[cfg(all(test, target_os = "linux"))]
#[path = "coding_flow/worker_panic_tests.rs"]
mod worker_panic_tests;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "coding_flow/verification.rs"]
mod verification;

#[cfg(target_os = "linux")]
mod linux {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, OnceLock};

    use nexus_kernel::coding_run::{
        display_safe, loopback_endpoint, run_worker, ApplyError, ApplyState, ChangeKind,
        CleanupStatus, CodingRun, FolderPicker, LedgerStore, LocalEndpoint, LocalModel,
        LocalOllama, OwnerConfirmer, ProjectId, ProjectInfo, ProjectRegistry, RelPath, Review,
        RunError, RunId, RunScopes, RunState, ScopeEntry, ScopeSet, StagingParent,
        StructuralOutcome, StructuralVerification, TextDiff, VerificationPhase, VerificationResult,
        VerifierCleanup, VerifierExit, VerifierMarker, WorkerReport,
    };
    use nexus_kernel::workspace_authority::{WorkspaceAuthorityRegistry, WorkspaceBinding};
    use nexus_persistence::coding_run_ledger::CodingRunLedger;

    use super::*;

    /// The most scope folders of each kind a run may name.
    const MAX_SCOPE_ENTRIES: usize = 32;

    pub(super) struct Display {
        pub(super) stage: &'static str,
        pub(super) message: Option<String>,
        worker: Option<WorkerView>,
        verification: Option<VerificationView>,
        pub(super) review: Option<ReviewView>,
        run_state: String,
        apply_state: String,
        can_apply: bool,
        can_restore: bool,
        sandbox: Option<SandboxVerificationView>,
        /// The latest sandboxed execution's output tails, escaped.
        pub(super) excerpts: (Option<String>, Option<String>),
        can_verify: bool,
        can_retry_cleanup: bool,
    }

    pub(super) struct RunSlot {
        pub(super) run: Mutex<CodingRun>,
        binding: WorkspaceBinding,
        project: ProjectInfo,
        model: String,
        display: Mutex<Display>,
    }

    impl RunSlot {
        /// The slot of a newly created run, shown as preparing.
        pub(super) fn new(
            run: CodingRun,
            binding: WorkspaceBinding,
            project: ProjectInfo,
            model: String,
        ) -> Self {
            Self {
                run: Mutex::new(run),
                binding,
                project,
                model,
                display: Mutex::new(Display {
                    stage: "preparing",
                    message: None,
                    worker: None,
                    verification: None,
                    review: None,
                    run_state: "Created".to_string(),
                    apply_state: "NotApplied".to_string(),
                    can_apply: false,
                    can_restore: false,
                    sandbox: None,
                    excerpts: (None, None),
                    can_verify: false,
                    can_retry_cleanup: false,
                }),
            }
        }

        pub(super) fn display(&self) -> std::sync::MutexGuard<'_, Display> {
            self.display.lock().unwrap_or_else(|p| p.into_inner())
        }

        /// Refresh the display from the run after a transition.
        pub(super) fn refresh(
            &self,
            run: &CodingRun,
            stage: &'static str,
            message: Option<String>,
        ) {
            let mut display = self.display();
            display.stage = stage;
            display.message = message;
            display.run_state = format!("{:?}", run.state());
            display.apply_state = format!("{:?}", run.apply_state());
            let phase = run.verification_phase();
            let reviewable = run.state() == RunState::StructurallyVerified
                && run.apply_state() == ApplyState::NotApplied
                && run.verification().is_some();
            display.can_apply = reviewable && !phase.blocks_apply();
            display.can_verify =
                cfg!(target_arch = "x86_64") && reviewable && phase == VerificationPhase::Idle;
            display.can_retry_cleanup = phase == VerificationPhase::CleanupFailed;
            display.can_restore = run.apply_state() == ApplyState::Applied;
            if run.verification().is_none() && run.apply_state() == ApplyState::NotApplied {
                display.review = None;
            }
            display.sandbox = sandbox_view(run, &display.excerpts);
        }

        pub(super) fn view(&self, id: RunId) -> RunView {
            let display = self.display();
            let busy = matches!(
                display.stage,
                "preparing" | "working" | "applying" | "restoring" | "discarding" | "verifying"
            );
            RunView {
                run_id: id.to_string(),
                project_id: self.project.id.to_string(),
                project_name: self.project.name.clone(),
                model: self.model.clone(),
                stage: display.stage,
                message: display.message.clone(),
                run_state: display.run_state.clone(),
                apply_state: display.apply_state.clone(),
                worker: display.worker.clone(),
                verification: display.verification.clone(),
                review: display.review.clone(),
                sandbox_verification: display.sandbox.clone(),
                can_apply: !busy && display.can_apply,
                can_restore: !busy && display.can_restore,
                can_discard: !busy
                    && !display.can_restore
                    && !display.can_retry_cleanup
                    && display.stage != "discarded",
                can_verify: !busy && display.can_verify,
                can_retry_verification_cleanup: !busy && display.can_retry_cleanup,
            }
        }
    }

    fn phase_name(phase: VerificationPhase) -> &'static str {
        match phase {
            VerificationPhase::Idle => "idle",
            VerificationPhase::Prepared => "prepared",
            VerificationPhase::Materialized => "materialized",
            VerificationPhase::Approved => "approved",
            VerificationPhase::Starting => "starting",
            VerificationPhase::Running => "running",
            VerificationPhase::Finalizing => "finalizing",
            VerificationPhase::CleanupFailed => "cleanup_failed",
        }
    }

    /// The run's sandboxed verification as display data.
    fn sandbox_view(
        run: &CodingRun,
        excerpts: &(Option<String>, Option<String>),
    ) -> Option<SandboxVerificationView> {
        let phase = run.verification_phase();
        let result = run
            .latest_verification()
            .map(|result| result_view(result, excerpts));
        (phase != VerificationPhase::Idle || result.is_some()).then(|| SandboxVerificationView {
            phase: phase_name(phase),
            result,
        })
    }

    fn result_view(
        result: &VerificationResult,
        excerpts: &(Option<String>, Option<String>),
    ) -> SandboxResultView {
        use nexus_verifier_sandbox::profile::VerifierProfileId;
        let outcome = &result.outcome;
        SandboxResultView {
            generation: result.generation.get(),
            profile: VerifierProfileId::PRODUCTION
                .iter()
                .find(|id| id.profile().hash().bytes() == result.inputs.profile_hash)
                .map(|id| id.name()),
            exit: outcome.exit.name(),
            passed: outcome.exit.passed(),
            exit_code: match outcome.exit {
                VerifierExit::Failed { exit_code } => Some(exit_code),
                _ => None,
            },
            signal: match outcome.exit {
                VerifierExit::Signalled { signal } => Some(signal),
                _ => None,
            },
            duration_ms: outcome.duration_ms,
            stdout_bytes: outcome.stdout.bytes,
            stdout_truncated: outcome.stdout.truncated,
            stderr_bytes: outcome.stderr.bytes,
            stderr_truncated: outcome.stderr.truncated,
            cleanup: match outcome.cleanup {
                VerifierCleanup::Confirmed => "confirmed",
                VerifierCleanup::Failed => "failed",
            },
            result_short: hex::encode(result.binding_hash())[..12].to_string(),
            stdout_excerpt: excerpts.0.clone(),
            stderr_excerpt: excerpts.1.clone(),
        }
    }

    /// The desktop's coding-run session: owner-selected projects and the
    /// runs started in this process.
    pub(crate) struct CodingFlow {
        projects: ProjectRegistry,
        ledger: OnceLock<Result<Arc<CodingRunLedger>, String>>,
        runs: Mutex<HashMap<RunId, Arc<RunSlot>>>,
        /// Boundaries of sandboxed verifications whose cleanup is unconfirmed.
        #[cfg(target_arch = "x86_64")]
        pub(super) retained: Mutex<HashMap<RunId, super::verification::Retained>>,
    }

    impl std::fmt::Debug for CodingFlow {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("CodingFlow").finish_non_exhaustive()
        }
    }

    fn unavailable() -> String {
        "governed coding is unavailable: the Nexus state directory cannot be resolved".to_string()
    }

    impl CodingFlow {
        pub(crate) fn new(authority: Arc<WorkspaceAuthorityRegistry>) -> Result<Self, String> {
            let state_dir =
                nexus_kernel::identity_home::nexus_state_dir().map_err(|_| unavailable())?;
            // No project may contain or lie within the Nexus state directory
            // or, where sandboxed verification exists, the verifier's
            // workspaces (XA-L-01).
            #[cfg(target_arch = "x86_64")]
            let workspaces = nexus_verifier_sandbox::workspace::workspaces_path();
            #[cfg(target_arch = "x86_64")]
            let reserved = [state_dir.as_path(), workspaces.as_path()];
            #[cfg(not(target_arch = "x86_64"))]
            let reserved = [state_dir.as_path()];
            Ok(Self {
                projects: ProjectRegistry::reserving(authority, &reserved),
                ledger: OnceLock::new(),
                runs: Mutex::new(HashMap::new()),
                #[cfg(target_arch = "x86_64")]
                retained: Mutex::new(HashMap::new()),
            })
        }

        /// The durable coding-run ledger (opened once, fail closed).
        fn ledger(&self) -> Result<Arc<dyn LedgerStore>, String> {
            let ledger = self.ledger.get_or_init(|| {
                // Deriving the staging parent creates `.nexus` through
                // retained handles before the ledger file is opened in it.
                StagingParent::from_identity_home().map_err(|_| unavailable())?;
                let path = nexus_kernel::identity_home::nexus_state_path("coding-runs.db")
                    .map_err(|_| unavailable())?;
                CodingRunLedger::open(&path).map(Arc::new).map_err(|_| {
                    "governed coding is unavailable: the coding-run ledger cannot be opened"
                        .to_string()
                })
            });
            match ledger {
                Ok(ledger) => Ok(Arc::clone(ledger) as Arc<dyn LedgerStore>),
                Err(error) => Err(error.clone()),
            }
        }

        fn runs(&self) -> std::sync::MutexGuard<'_, HashMap<RunId, Arc<RunSlot>>> {
            self.runs.lock().unwrap_or_else(|p| p.into_inner())
        }

        pub(super) fn slot(&self, run_id: &str) -> Result<(RunId, Arc<RunSlot>), String> {
            let id = RunId::parse(run_id).ok_or_else(|| "unknown coding run".to_string())?;
            let slot = self
                .runs()
                .get(&id)
                .cloned()
                .ok_or_else(|| "unknown coding run".to_string())?;
            Ok((id, slot))
        }

        pub(crate) fn select_project(
            &self,
            picker: &dyn FolderPicker,
        ) -> Result<ProjectView, String> {
            self.projects
                .select(picker)
                .map(|info| project_view(&info))
                .map_err(|error| error.to_string())
        }

        pub(crate) fn list_projects(&self) -> Vec<ProjectView> {
            self.projects.list().iter().map(project_view).collect()
        }

        pub(crate) fn list_runs(&self) -> Vec<RunView> {
            let mut runs: Vec<RunView> = self
                .runs()
                .iter()
                .map(|(id, slot)| slot.view(*id))
                .collect();
            runs.sort_by(|a, b| a.run_id.cmp(&b.run_id));
            runs
        }

        pub(crate) fn status(&self, run_id: &str) -> Result<RunView, String> {
            let (id, slot) = self.slot(run_id)?;
            Ok(slot.view(id))
        }

        /// Create a run and start the worker off the IPC thread.
        pub(crate) fn start_run(
            self: &Arc<Self>,
            project_id: &str,
            write_scope: &[String],
            protected_scope: &[String],
            task: String,
            model: String,
        ) -> Result<StartView, String> {
            let project = ProjectId::parse(project_id)
                .and_then(|id| self.projects.info(id))
                .ok_or_else(|| "unknown project; select the folder again".to_string())?;
            let scopes = scopes(write_scope, protected_scope)?;
            let endpoint = local_endpoint()?;
            let ledger = self.ledger()?;
            let binding = WorkspaceBinding {
                agent_id: uuid::Uuid::new_v4(),
                run_id: uuid::Uuid::new_v4(),
            };
            let grant = self
                .projects
                .grant_for_run(project.id, binding)
                .map_err(|error| error.to_string())?;
            let run = CodingRun::create_for_project(ledger, grant, scopes)
                .map_err(|error| format!("the run could not be recorded: {error}"))?;
            let id = run.id();
            let slot = Arc::new(RunSlot::new(run, binding, project, model.clone()));
            self.runs().insert(id, Arc::clone(&slot));
            let worker = Arc::clone(&slot);
            if std::thread::Builder::new()
                .name("nexus-coding-run".to_string())
                .spawn(move || work(&worker, &endpoint, &model, &task))
                .is_err()
            {
                // The run never started: end it now, closing its read grant,
                // instead of leaving it preparing until the grant expires.
                let mut run = claim(&slot, &["preparing"], "failed")?;
                let (stage, message) =
                    end_failed(&mut run, "The coding run could not be started.".to_string());
                slot.refresh(&run, stage, Some(message));
                return Err("the coding run could not be started".to_string());
            }
            Ok(StartView {
                run_id: id.to_string(),
            })
        }

        /// Ask the owner natively, then apply the exact reviewed candidate.
        pub(crate) fn approve_apply(
            &self,
            run_id: &str,
            confirmer: &dyn OwnerConfirmer,
        ) -> Result<RunView, String> {
            let (id, slot) = self.slot(run_id)?;
            let mut run = claim(&slot, &["review"], "applying")?;
            let outcome = run
                .request_approval(&slot.project.name, confirmer)
                .and_then(|approval| {
                    let grant = self
                        .projects
                        .grant_for_apply(slot.project.id, slot.binding)
                        .map_err(|error| ApplyError::Refused(project_refusal(error)))?;
                    let storage = StagingParent::from_identity_home().map_err(|_| {
                        ApplyError::Refused(nexus_kernel::coding_run::Refusal::PreimageUnavailable)
                    })?;
                    run.apply(approval, grant, &storage)
                });
            let (stage, message) = match outcome {
                Ok(report) => (
                    "applied",
                    Some(format!(
                        "Applied {} file(s) to {}.",
                        report.files.len(),
                        slot.project.name
                    )),
                ),
                Err(ApplyError::RolledBack { failed }) => (
                    "rolled_back",
                    Some(format!(
                        "Apply failed at {failed}; every applied change was undone."
                    )),
                ),
                Err(ApplyError::RecoveryRequired { failed, unrestored }) => (
                    "recovery_required",
                    Some(format!(
                        "Apply failed at {failed}; these files need manual recovery: {}",
                        unrestored.join(", ")
                    )),
                ),
                Err(error) => ("review", Some(apply_message(&error))),
            };
            let stage = if run.apply_state() == ApplyState::RecoveryRequired {
                "recovery_required"
            } else if stage == "review" && run.verification().is_none() {
                "failed"
            } else {
                stage
            };
            slot.refresh(&run, stage, message);
            Ok(slot.view(id))
        }

        /// Ask the owner natively, then restore this run's apply.
        pub(crate) fn restore(
            &self,
            run_id: &str,
            confirmer: &dyn OwnerConfirmer,
        ) -> Result<RunView, String> {
            let (id, slot) = self.slot(run_id)?;
            let mut run = claim(&slot, &["applied"], "restoring")?;
            let outcome = run
                .request_restore(&slot.project.name, confirmer)
                .and_then(|approval| {
                    let grant = self
                        .projects
                        .grant_for_apply(slot.project.id, slot.binding)
                        .map_err(|error| ApplyError::Refused(project_refusal(error)))?;
                    run.restore(approval, grant)
                });
            let (stage, message) = match outcome {
                Ok(report) => (
                    "restored",
                    Some(format!("Restored {} file(s).", report.files.len())),
                ),
                Err(ApplyError::RecoveryRequired { unrestored, .. }) => (
                    "recovery_required",
                    Some(format!(
                        "Restore could not finish; these files need manual recovery: {}",
                        unrestored.join(", ")
                    )),
                ),
                Err(error) => ("applied", Some(apply_message(&error))),
            };
            let stage = if run.apply_state() == ApplyState::RecoveryRequired {
                "recovery_required"
            } else {
                stage
            };
            slot.refresh(&run, stage, message);
            Ok(slot.view(id))
        }

        /// Cancel a run that has not been applied and remove its staging.
        /// The run is claimed atomically, so it can never wait on a run an
        /// apply or restore holds while its native dialog is open.
        pub(crate) fn discard(&self, run_id: &str) -> Result<RunView, String> {
            let (id, slot) = self.slot(run_id)?;
            let mut run = claim(
                &slot,
                &[
                    "review",
                    "no_changes",
                    "failed",
                    "rolled_back",
                    "restored",
                    "recovery_required",
                ],
                "discarding",
            )?;
            if run.apply_state() == ApplyState::Applied {
                let refusal = "an applied run can be restored, not discarded".to_string();
                slot.refresh(&run, "applied", Some(refusal.clone()));
                return Err(refusal);
            }
            // A sandboxed verification whose cleanup is unconfirmed keeps its
            // run: its retained boundary is retried from the review, never
            // abandoned by a discard.
            if run.verification_phase().blocks_apply() {
                let refusal = "the sandboxed verification's cleanup is unconfirmed; retry it first"
                    .to_string();
                slot.refresh(&run, "review", Some(refusal.clone()));
                return Err(refusal);
            }
            let (stage, message) = discard_run(&mut run);
            slot.refresh(&run, stage, Some(message));
            Ok(slot.view(id))
        }
    }

    /// Cancel a run that is still live and remove its staging copy, and say
    /// what actually happened. The run is shown as discarded only if
    /// cancellation (when needed) and staging cleanup both succeeded and the
    /// run is not left requiring recovery. The run's own read grant is closed
    /// by the time it ends (a failed closure leaves it requiring recovery),
    /// and the discard retries that closure.
    pub(crate) fn discard_run(run: &mut CodingRun) -> (&'static str, String) {
        let cancel = if run.state().is_terminal() {
            Ok(())
        } else {
            run.cancel()
        };
        let cleanup = run.discard_staging();
        discard_outcome(cancel, cleanup, run.state())
    }

    /// The displayed outcome of a discard from its actual results.
    pub(crate) fn discard_outcome(
        cancel: Result<(), RunError>,
        cleanup: Result<CleanupStatus, RunError>,
        state: RunState,
    ) -> (&'static str, String) {
        if let Err(error) = cancel {
            return (
                "recovery_required",
                format!("The run could not be cancelled cleanly ({error}); it needs recovery."),
            );
        }
        match cleanup {
            Ok(CleanupStatus::Discarded | CleanupStatus::NotStarted) => {}
            Ok(CleanupStatus::DiscardFailed) => {
                return (
                    "recovery_required",
                    "The staging copy could not be removed; it needs recovery.".to_string(),
                )
            }
            Err(error) => {
                return (
                    "recovery_required",
                    format!("The staging copy could not be removed ({error}); it needs recovery."),
                )
            }
        }
        if let RunState::RecoveryRequired(reason) = state {
            return (
                "recovery_required",
                format!("The staging copy was removed, but the run needs recovery ({reason:?})."),
            );
        }
        ("discarded", "The run was discarded.".to_string())
    }

    /// End a run that failed before review: cancel it if it is still live,
    /// and report `recovery_required` rather than an ordinary failure if the
    /// run could not be closed cleanly.
    pub(crate) fn end_failed(run: &mut CodingRun, message: String) -> (&'static str, String) {
        let cancel = if run.state().is_terminal() {
            Ok(())
        } else {
            run.cancel()
        };
        failure_outcome(cancel, run.state(), message)
    }

    /// The displayed outcome of a failed run from its actual state.
    pub(crate) fn failure_outcome(
        cancel: Result<(), RunError>,
        state: RunState,
        message: String,
    ) -> (&'static str, String) {
        match (cancel, state) {
            (_, RunState::RecoveryRequired(reason)) => (
                "recovery_required",
                format!("{message} The run also needs recovery ({reason:?})."),
            ),
            (Err(error), _) => (
                "recovery_required",
                format!("{message} The run could not be closed cleanly ({error})."),
            ),
            (Ok(()), _) => ("failed", message),
        }
    }

    /// Take the run for an owner action, only from the expected stage.
    pub(super) fn claim<'a>(
        slot: &'a RunSlot,
        from: &[&'static str],
        to: &'static str,
    ) -> Result<std::sync::MutexGuard<'a, CodingRun>, String> {
        {
            let mut display = slot.display();
            if !from.contains(&display.stage) {
                return Err(format!(
                    "the run is not ready for this action ({})",
                    display.stage
                ));
            }
            display.stage = to;
            display.message = None;
        }
        Ok(slot.run.lock().unwrap_or_else(|p| p.into_inner()))
    }

    fn project_refusal(
        error: nexus_kernel::coding_run::ProjectError,
    ) -> nexus_kernel::coding_run::Refusal {
        use nexus_kernel::coding_run::{ProjectError, Refusal};
        match error {
            ProjectError::IdentityChanged => Refusal::IdentityChanged,
            ProjectError::UnknownProject => Refusal::WrongProject,
            _ => Refusal::AuthorityDenied,
        }
    }

    fn apply_message(error: &ApplyError) -> String {
        use nexus_kernel::coding_run::Refusal;
        match error {
            ApplyError::Refused(Refusal::Declined) => {
                "You declined; nothing was changed.".to_string()
            }
            ApplyError::Refused(Refusal::Stale(path)) => {
                format!("{path} changed since the run started; nothing was changed.")
            }
            ApplyError::Refused(Refusal::CandidateChanged) => {
                "The staged candidate changed after verification; it can no longer be applied."
                    .to_string()
            }
            ApplyError::Refused(Refusal::IdentityChanged) => {
                "The project folder was moved or replaced; nothing was changed.".to_string()
            }
            ApplyError::Refused(refusal) => format!("Refused before any change: {refusal:?}"),
            ApplyError::InvalidState => "This action is not available for the run.".to_string(),
            ApplyError::CompletionUnrecorded { operation } => format!(
                "The {operation} finished on disk, but it could not be recorded durably; the run needs recovery and its saved originals are kept."
            ),
            ApplyError::AuthorityNotClosed { after } => {
                let what = match after.as_ref() {
                    Ok(report) => format!("{} file(s) were written", report.files.len()),
                    Err(error) => apply_message(error),
                };
                format!(
                    "{what}, but the temporary write permission could not be confirmed closed; the run needs recovery."
                )
            }
            other => other.to_string(),
        }
    }

    fn project_view(info: &ProjectInfo) -> ProjectView {
        ProjectView {
            id: info.id.to_string(),
            name: info.name.clone(),
        }
    }

    /// Scopes from the owner's choices: read the whole project; write the
    /// named folders (or the whole project); protect the named folders.
    fn scopes(write: &[String], protected: &[String]) -> Result<RunScopes, String> {
        let parse = |entries: &[String]| -> Result<Vec<ScopeEntry>, String> {
            let entries: Vec<&str> = entries
                .iter()
                .map(|e| e.trim().trim_end_matches('/'))
                .filter(|e| !e.is_empty())
                .collect();
            if entries.len() > MAX_SCOPE_ENTRIES {
                return Err("too many scope folders".to_string());
            }
            entries
                .into_iter()
                .map(|e| {
                    RelPath::parse(e)
                        .map(ScopeEntry::Tree)
                        .map_err(|error| format!("scope folder \"{}\": {error}", bounded(e)))
                })
                .collect()
        };
        let mut write = parse(write)?;
        if write.is_empty() {
            write.push(ScopeEntry::WholeProject);
        }
        RunScopes::new(
            ScopeSet::new([ScopeEntry::WholeProject]),
            ScopeSet::new(write),
            ScopeSet::new(parse(protected)?),
        )
        .map_err(|error| error.to_string())
    }

    fn bounded(text: &str) -> String {
        text.chars().take(64).collect()
    }

    /// The operator-authorized Ollama address, required to be loopback.
    fn local_endpoint() -> Result<LocalEndpoint, String> {
        let authorized = crate::commands::chat_llm::authorized_ollama_base_url()?;
        loopback_endpoint(&authorized).map_err(|error| error.to_string())
    }

    pub(crate) fn list_local_models() -> Result<Vec<String>, String> {
        LocalOllama::installed_models(&local_endpoint()?).map_err(|error| error.to_string())
    }

    /// The worker thread: the governed work behind the worker panic
    /// boundary ([`guarded`]).
    fn work(slot: &RunSlot, endpoint: &LocalEndpoint, model: &str, task: &str) {
        guarded(slot, |run| governed_work(slot, run, endpoint, model, task));
    }

    /// What the display says after a caught worker panic: fixed and bounded,
    /// never the panic's payload.
    pub(super) const WORKER_STOPPED: &str =
        "The coding worker stopped unexpectedly; the run needs recovery.";

    /// The coding worker thread's panic boundary (P2-ENTRY-H1-R1).
    ///
    /// The run is locked once, outside the boundary, so a panic in `body`
    /// unwinds only `body`: the run guard is never dropped while unwinding
    /// (the run mutex is not poisoned) and recovery reuses it rather than
    /// locking again. The kernel closes the run's authority first and leaves
    /// it requiring recovery ([`CodingRun::guard_worker`]); only then does
    /// the display say so, without the payload, and the thread ends. Nothing
    /// is retried or continued.
    pub(super) fn guarded(slot: &RunSlot, body: impl FnOnce(&mut CodingRun)) {
        let mut run = slot.run.lock().unwrap_or_else(|p| p.into_inner());
        if run.guard_worker(body).is_err() {
            slot.refresh(&run, "recovery_required", Some(WORKER_STOPPED.to_string()));
        }
    }

    /// The worker's governed work on the locked run: stage, pin the model,
    /// run the worker, review.
    fn governed_work(
        slot: &RunSlot,
        run: &mut CodingRun,
        endpoint: &LocalEndpoint,
        model: &str,
        task: &str,
    ) {
        let prepared = StagingParent::from_identity_home()
            .map_err(|error| error.to_string())
            .and_then(|parent| run.grant(&parent).map_err(|e| e.to_string()))
            .and_then(|()| run.snapshot().map(|_| ()).map_err(|e| e.to_string()));
        let fail = |run: &mut CodingRun, message: String| {
            let (stage, message) = end_failed(run, message);
            slot.refresh(run, stage, Some(message));
        };
        if let Err(error) = prepared {
            fail(run, format!("The project could not be staged: {error}"));
            return;
        }
        let local = match LocalOllama::select(endpoint, model) {
            Ok(local) => local,
            Err(error) => {
                fail(run, format!("{error}; no other model is used."));
                return;
            }
        };
        if let Err(error) = run.pin_model(local.pin().clone()) {
            fail(run, format!("The model could not be pinned: {error}"));
            return;
        }
        slot.refresh(run, "working", None);
        match run_worker(run, &local, task) {
            Ok(report) => {
                slot.display().worker = Some(worker_view(&report));
                match &report.verification {
                    None => slot.refresh(
                        run,
                        "no_changes",
                        Some("The model finished without proposing changes.".to_string()),
                    ),
                    Some(verification) => {
                        slot.display().verification = Some(verification_view(verification));
                        if !verification.passed() {
                            fail(
                                run,
                                "Structural verification rejected the candidate; it cannot be applied.".to_string(),
                            );
                            return;
                        }
                        match run.review() {
                            Ok(review) => {
                                slot.display().review = Some(review_view(&review));
                                slot.refresh(run, "review", None);
                            }
                            Err(error) => {
                                fail(run, format!("The review could not be computed: {error}"))
                            }
                        }
                    }
                }
            }
            Err(error) => fail(run, error.to_string()),
        }
    }

    fn worker_view(report: &WorkerReport) -> WorkerView {
        WorkerView {
            turns: report.turns,
            files_read: report.files_read,
            accepted: report
                .accepted
                .iter()
                .map(|path| display_safe(&path.as_string(), false))
                .collect(),
            rejected: report.rejected,
        }
    }

    fn verification_view(verification: &StructuralVerification) -> VerificationView {
        VerificationView {
            passed: verification.passed(),
            candidate_short: verification.candidate_manifest_hash.to_hex()[..12].to_string(),
            base_short: verification.base_manifest_hash.to_hex()[..12].to_string(),
            violations: match &verification.outcome {
                StructuralOutcome::Passed => Vec::new(),
                StructuralOutcome::Rejected(violations) => violations
                    .iter()
                    .take(64)
                    .map(|v| format!("{v:?}"))
                    .collect(),
            },
        }
    }

    pub(crate) fn review_view(review: &Review) -> ReviewView {
        ReviewView {
            verification_short: match review.binding.verification {
                VerifierMarker::NoResult => None,
                VerifierMarker::Result(hash) => Some(hex::encode(hash)[..12].to_string()),
            },
            binding_short: review.binding.short(),
            changes: review
                .changes
                .iter()
                .map(|change| {
                    let (diff, diff_note, truncated) = match &change.diff {
                        TextDiff::Unified { text, truncated } => {
                            (Some(display_safe(text, true)), None, *truncated)
                        }
                        TextDiff::Omitted(reason) => (None, Some(format!("{reason:?}")), false),
                    };
                    ChangeView {
                        path: display_safe(&change.path.as_string(), false),
                        kind: match change.kind {
                            ChangeKind::Create => "create",
                            ChangeKind::Replace => "replace",
                            ChangeKind::Delete => "delete",
                        },
                        old_sha256: change.old.as_ref().map(|e| hex::encode(e.sha256)),
                        new_sha256: change.new.as_ref().map(|e| hex::encode(e.sha256)),
                        old_size: change.old.as_ref().map(|e| e.size),
                        new_size: change.new.as_ref().map(|e| e.size),
                        diff,
                        diff_note,
                        truncated,
                    }
                })
                .collect(),
        }
    }

    /// Text for the native message dialog. On Linux the dialog plugin's GTK
    /// backend passes the message to `gtk_message_dialog_format_secondary_text`
    /// as a printf format string, so every `%` (which a model-chosen file name
    /// or a folder name may contain) is doubled to print literally.
    pub(crate) fn native_dialog_text(message: &str) -> String {
        message.replace('%', "%%")
    }

    /// The desktop's native dialogs: the only [`FolderPicker`] and
    /// [`OwnerConfirmer`] implementations. They are invoked by the backend;
    /// the webview holds no dialog permission.
    #[cfg(feature = "tauri-runtime")]
    pub(crate) struct NativeDialogs(pub(crate) tauri::AppHandle<tauri::Wry>);

    #[cfg(feature = "tauri-runtime")]
    impl FolderPicker for NativeDialogs {
        fn pick_folder(&self) -> Option<PathBuf> {
            use tauri_plugin_dialog::DialogExt;
            self.0
                .dialog()
                .file()
                .set_title("Select a project folder for Nexus")
                .blocking_pick_folder()?
                .into_path()
                .ok()
        }
    }

    #[cfg(feature = "tauri-runtime")]
    impl nexus_kernel::coding_run::VerifierLaunchConfirmer for NativeDialogs {
        fn confirm_launch(
            &self,
            request: &nexus_kernel::coding_run::VerifierLaunchRequest,
        ) -> bool {
            use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
            self.0
                .dialog()
                .message(native_dialog_text(&request.message()))
                .title(request.title())
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "Run verification".to_string(),
                    "Cancel".to_string(),
                ))
                .blocking_show()
        }
    }

    #[cfg(feature = "tauri-runtime")]
    impl OwnerConfirmer for NativeDialogs {
        fn confirm(&self, request: &nexus_kernel::coding_run::ConfirmationRequest) -> bool {
            use nexus_kernel::coding_run::ConfirmationKind;
            use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
            let ok = match request.kind {
                ConfirmationKind::Apply => "Apply changes",
                ConfirmationKind::Restore => "Restore files",
            };
            self.0
                .dialog()
                .message(native_dialog_text(&request.message()))
                .title(request.title())
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    ok.to_string(),
                    "Cancel".to_string(),
                ))
                .blocking_show()
        }
    }
}

/// The IPC entry points (thin: every decision is made above or in the
/// kernel). Long or dialog-bound work runs on Tauri's blocking pool, never on
/// the IPC or main thread.
#[cfg(feature = "tauri-runtime")]
pub(crate) mod ipc {
    use super::*;

    type App = tauri::AppHandle<tauri::Wry>;

    #[cfg(not(target_os = "linux"))]
    const UNSUPPORTED: &str = "governed coding is available on Linux only";

    #[cfg(target_os = "linux")]
    async fn blocking<T: Send + 'static>(
        work: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        tauri::async_runtime::spawn_blocking(work)
            .await
            .map_err(|_| "governed coding: the operation did not complete".to_string())?
    }

    pub(crate) async fn select_project(
        app: App,
        state: crate::AppState,
    ) -> Result<ProjectView, String> {
        #[cfg(target_os = "linux")]
        {
            let flow = state.coding_flow()?;
            blocking(move || flow.select_project(&NativeDialogs(app))).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (app, state);
            Err(UNSUPPORTED.to_string())
        }
    }

    pub(crate) fn list_projects(state: &crate::AppState) -> Result<Vec<ProjectView>, String> {
        #[cfg(target_os = "linux")]
        {
            Ok(state.coding_flow()?.list_projects())
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = state;
            Err(UNSUPPORTED.to_string())
        }
    }

    pub(crate) async fn list_local_models() -> Result<Vec<String>, String> {
        #[cfg(target_os = "linux")]
        {
            blocking(super::list_local_models).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(UNSUPPORTED.to_string())
        }
    }

    pub(crate) fn start_run(
        state: &crate::AppState,
        project_id: &str,
        write_scope: &[String],
        protected_scope: &[String],
        task: String,
        model: String,
    ) -> Result<StartView, String> {
        #[cfg(target_os = "linux")]
        {
            state
                .coding_flow()?
                .start_run(project_id, write_scope, protected_scope, task, model)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (state, project_id, write_scope, protected_scope, task, model);
            Err(UNSUPPORTED.to_string())
        }
    }

    pub(crate) fn status(state: &crate::AppState, run_id: &str) -> Result<RunView, String> {
        #[cfg(target_os = "linux")]
        {
            state.coding_flow()?.status(run_id)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (state, run_id);
            Err(UNSUPPORTED.to_string())
        }
    }

    pub(crate) fn list_runs(state: &crate::AppState) -> Result<Vec<RunView>, String> {
        #[cfg(target_os = "linux")]
        {
            Ok(state.coding_flow()?.list_runs())
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = state;
            Err(UNSUPPORTED.to_string())
        }
    }

    pub(crate) async fn approve_apply(
        app: App,
        state: crate::AppState,
        run_id: String,
    ) -> Result<RunView, String> {
        #[cfg(target_os = "linux")]
        {
            let flow = state.coding_flow()?;
            blocking(move || flow.approve_apply(&run_id, &NativeDialogs(app))).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (app, state, run_id);
            Err(UNSUPPORTED.to_string())
        }
    }

    pub(crate) async fn restore(
        app: App,
        state: crate::AppState,
        run_id: String,
    ) -> Result<RunView, String> {
        #[cfg(target_os = "linux")]
        {
            let flow = state.coding_flow()?;
            blocking(move || flow.restore(&run_id, &NativeDialogs(app))).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (app, state, run_id);
            Err(UNSUPPORTED.to_string())
        }
    }

    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    const NO_VERIFICATION: &str = "sandboxed verification is available on Linux x86_64 only";

    pub(crate) async fn verification_profiles(
        state: crate::AppState,
        run_id: String,
    ) -> Result<Vec<VerifierProfileView>, String> {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            let flow = state.coding_flow()?;
            blocking(move || flow.verification_profiles(&run_id)).await
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            let _ = (state, run_id);
            Err(NO_VERIFICATION.to_string())
        }
    }

    pub(crate) async fn start_verification(
        app: App,
        state: crate::AppState,
        run_id: String,
        profile: String,
    ) -> Result<RunView, String> {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            let flow = state.coding_flow()?;
            blocking(move || flow.start_verification(&run_id, &profile, &NativeDialogs(app))).await
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            let _ = (app, state, run_id, profile);
            Err(NO_VERIFICATION.to_string())
        }
    }

    pub(crate) async fn retry_verification_cleanup(
        state: crate::AppState,
        run_id: String,
    ) -> Result<RunView, String> {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            let flow = state.coding_flow()?;
            blocking(move || flow.retry_verification_cleanup(&run_id)).await
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            let _ = (state, run_id);
            Err(NO_VERIFICATION.to_string())
        }
    }

    pub(crate) async fn discard(state: crate::AppState, run_id: String) -> Result<RunView, String> {
        #[cfg(target_os = "linux")]
        {
            let flow = state.coding_flow()?;
            blocking(move || flow.discard(&run_id)).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (state, run_id);
            Err(UNSUPPORTED.to_string())
        }
    }
}
