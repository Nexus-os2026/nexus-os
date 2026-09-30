//! Phase Two: governed sandboxed verification of a verified candidate
//! (Linux, x86_64).
//!
//! The owner starts it with only an opaque run id and a profile name; the
//! name selects a compiled-in profile and grants nothing. The backend then,
//! by itself: verifies the packaged toolchain and the helper installed with
//! Nexus (a development build has neither, so verification is unavailable
//! there); decides the profile's applicability from the verified candidate;
//! prepares and records the launch binding; materializes the candidate into
//! a fresh workspace; asks the owner natively to approve exactly that
//! launch; re-verifies and launches it in the verifier sandbox on a thread of
//! its own; and finalizes: the input is rescanned, the workspace removed and
//! the result bound into the owner's review. A cleanup that cannot be
//! confirmed retains the boundary for a retry and keeps Apply refused.
//!
//! The only execution route is `nexus_verifier_sandbox::execution::run`;
//! output is untrusted data, kept only as bounded, escaped tails.

use std::sync::Arc;
use std::time::Duration;

use nexus_kernel::coding_run::{
    display_safe, CodingRun, ExecutionGeneration, RunId, StreamSummary, VerifierCleanup,
    VerifierExit, VerifierInputs, VerifierLaunchConfirmer, VerifierLaunchFacts, VerifierOutcome,
};
use nexus_verifier_sandbox::applicability;
use nexus_verifier_sandbox::execution::{self, Cleanup, ExitClass, RetainedBoundary, StreamRecord};
use nexus_verifier_sandbox::launcher::HelperProgram;
use nexus_verifier_sandbox::policy::SandboxPolicy;
use nexus_verifier_sandbox::profile::{VerifierProfile, VerifierProfileId};
use nexus_verifier_sandbox::profile_launch::{launch_spec, LaunchSetupError};
use nexus_verifier_sandbox::scope::ScopeManager;
use nexus_verifier_sandbox::toolchain::VerifiedVerifierToolchain;
use nexus_verifier_sandbox::workspace::{Area, RetainedWorkspace, Workspace, WorkspaceRoot};

use super::linux::{claim, review_view, CodingFlow, RunSlot};
use super::{RunView, VerifierProfileView};

/// The largest manifest or lock file read for applicability.
const MAX_METADATA_BYTES: u64 = 1 << 20;
/// The displayed tail of each output stream.
const EXCERPT_CHARS: usize = 4096;

/// What remains of a verification whose cleanup is unconfirmed.
pub(crate) struct Retained {
    generation: ExecutionGeneration,
    boundary: Option<RetainedBoundary>,
    workspace: Option<Leftover>,
}

enum Leftover {
    /// Not removed yet: its execution's boundary had to be confirmed first.
    Pending(Workspace),
    Retained(RetainedWorkspace),
}

impl Leftover {
    fn remove(self) -> Option<Leftover> {
        match self {
            Self::Pending(workspace) => workspace.remove().err().map(Self::Retained),
            Self::Retained(retained) => retained.retry().err().map(Self::Retained),
        }
    }
}

/// Everything a started launch owns.
struct Launch {
    profile: &'static VerifierProfile,
    toolchain: VerifiedVerifierToolchain,
    workspace: Workspace,
    helper: HelperProgram,
}

fn unavailable(what: &str) -> String {
    format!("Sandboxed verification is unavailable: {what}.")
}

/// Everything outside the candidate a launch depends on, from the backend's
/// own verified objects.
fn inputs_for(profile: &VerifierProfile, toolchain: &VerifiedVerifierToolchain) -> VerifierInputs {
    VerifierInputs {
        profile_hash: profile.hash().bytes(),
        toolchain_digest: toolchain.digest(),
        toolchain_generation: toolchain.generation(),
        sandbox_policy_hash: SandboxPolicy::V1.hash().bytes(),
        resource_policy_hash: profile.resources.hash().bytes(),
    }
}

fn facts_for(profile: &VerifierProfile) -> VerifierLaunchFacts {
    VerifierLaunchFacts {
        profile_display: profile.display_name.to_string(),
        wall_timeout_secs: profile.resources.wall_timeout_secs,
        memory_max_bytes: profile.resources.memory_max_bytes,
        cpus: profile.resources.cpu_quota_us_per_sec / 1_000_000,
        processes: profile.resources.pids_max,
    }
}

/// Whether `profile` applies to the run's verified candidate.
fn applicable(profile: VerifierProfileId, run: &CodingRun) -> Result<(), String> {
    match profile {
        VerifierProfileId::RustCargoTestOfflineV1 => {
            let candidate = run
                .verification_candidate()
                .map_err(|error| format!("The candidate is not available: {error}"))?;
            applicability::check(&candidate.paths(), &|path| {
                candidate.read(path, MAX_METADATA_BYTES)
            })
            .map(|_| ())
            .map_err(|reason| format!("This profile does not apply to the candidate ({reason:?})."))
        }
    }
}

fn exit_of(class: ExitClass) -> VerifierExit {
    match class {
        ExitClass::Passed => VerifierExit::Passed,
        ExitClass::Failed { exit_code } => VerifierExit::Failed { exit_code },
        ExitClass::TimedOut => VerifierExit::TimedOut,
        ExitClass::OutputLimitExceeded => VerifierExit::OutputLimitExceeded,
        ExitClass::OomKilled => VerifierExit::OomKilled,
        ExitClass::ProcessLimit => VerifierExit::ProcessLimit,
        ExitClass::Signalled { signal } => VerifierExit::Signalled { signal },
        ExitClass::SandboxUnavailable => VerifierExit::SandboxUnavailable,
        ExitClass::SandboxSetupFailed => VerifierExit::SandboxSetupFailed,
        ExitClass::SandboxFailed => VerifierExit::SandboxFailed,
        ExitClass::CleanupFailed => VerifierExit::CleanupFailed,
    }
}

/// A launch that could not be set up ran nothing.
fn setup_exit(error: &LaunchSetupError) -> VerifierExit {
    match error {
        LaunchSetupError::ToolchainMismatch | LaunchSetupError::Toolchain(_) => {
            VerifierExit::ToolchainUnavailable
        }
        LaunchSetupError::Device => VerifierExit::SandboxUnavailable,
        LaunchSetupError::Workspace(_) | LaunchSetupError::Io(_) => {
            VerifierExit::SandboxSetupFailed
        }
    }
}

/// The recorded exit and whether cleanup is confirmed: an unconfirmed
/// cleanup outranks everything, then a changed input, then what the sandbox
/// reported.
fn settle(
    ran: VerifierExit,
    boundary_retained: bool,
    workspace_retained: bool,
    input_unchanged: bool,
) -> (VerifierExit, bool) {
    let cleanup_confirmed = !boundary_retained && !workspace_retained;
    let exit = if !cleanup_confirmed {
        VerifierExit::CleanupFailed
    } else if !input_unchanged {
        VerifierExit::CandidateChanged
    } else {
        ran
    };
    (exit, cleanup_confirmed)
}

fn summary(record: &StreamRecord) -> StreamSummary {
    StreamSummary {
        bytes: record.bytes,
        sha256: record.sha256,
        truncated: record.truncated,
    }
}

/// The tail of an output stream, escaped for display.
fn excerpt(record: &StreamRecord) -> Option<String> {
    if record.excerpt.is_empty() {
        return None;
    }
    let text = String::from_utf8_lossy(&record.excerpt);
    let tail: Vec<char> = text.chars().collect();
    let start = tail.len().saturating_sub(EXCERPT_CHARS);
    Some(display_safe(
        &tail[start..].iter().collect::<String>(),
        true,
    ))
}

fn message_for(exit: VerifierExit) -> String {
    match exit {
        VerifierExit::Passed => "The candidate's tests passed in the verifier sandbox.".into(),
        VerifierExit::Failed { exit_code } => format!(
            "The candidate's tests failed in the verifier sandbox (exit status {exit_code}). \
             The result is advisory."
        ),
        VerifierExit::CleanupFailed => "The verification's cleanup could not be confirmed; \
             Apply stays refused until a cleanup retry succeeds."
            .into(),
        other => format!(
            "The verification ended: {}. The result is advisory.",
            other.name().replace('_', " ")
        ),
    }
}

fn with_run<T>(slot: &RunSlot, f: impl FnOnce(&mut CodingRun) -> T) -> T {
    let mut run = slot.run.lock().unwrap_or_else(|p| p.into_inner());
    f(&mut run)
}

impl CodingFlow {
    /// The compiled-in profiles and whether each applies to the run's
    /// verified candidate.
    pub(crate) fn verification_profiles(
        &self,
        run_id: &str,
    ) -> Result<Vec<VerifierProfileView>, String> {
        let (_, slot) = self.slot(run_id)?;
        Ok(with_run(&slot, |run| {
            VerifierProfileId::PRODUCTION
                .iter()
                .map(|id| {
                    let profile = id.profile();
                    let decision = applicable(*id, run);
                    VerifierProfileView {
                        name: profile.name,
                        display_name: profile.display_name,
                        applicable: decision.is_ok(),
                        reason: decision.err(),
                    }
                })
                .collect()
        }))
    }

    /// Verify the run's candidate with the named profile in the verifier
    /// sandbox, after the owner's native approval.
    pub(crate) fn start_verification(
        self: &Arc<Self>,
        run_id: &str,
        profile: &str,
        confirmer: &dyn VerifierLaunchConfirmer,
    ) -> Result<RunView, String> {
        let (id, slot) = self.slot(run_id)?;
        let profile_id = VerifierProfileId::lookup(profile)
            .ok_or_else(|| "unknown verifier profile".to_string())?;
        let profile = profile_id.profile();
        let toolchain = VerifiedVerifierToolchain::installed()
            .map_err(|error| unavailable(&error.to_string()))?;
        let helper = HelperProgram::installed()
            .map_err(|_| unavailable("no helper installed with Nexus"))?;
        let root = WorkspaceRoot::derive()
            .map_err(|_| unavailable("no private user runtime directory"))?;
        let mut run = claim(&slot, &["review"], "verifying")?;
        let started = (|| -> Result<(Workspace, ExecutionGeneration), String> {
            applicable(profile_id, &run)?;
            let inputs = inputs_for(profile, &toolchain);
            run.prepare_verification(inputs)
                .map_err(|error| format!("The verification could not be prepared: {error}."))?;
            let workspace = Workspace::create(&root)
                .map_err(|_| "The verification workspace could not be created.".to_string())?;
            let prepared = workspace
                .directory(Area::Input)
                .map_err(|_| "The verification workspace is unavailable.".to_string())
                .and_then(|input| {
                    run.materialize_verification_input(input).map_err(|error| {
                        format!("The candidate could not be materialized: {error}.")
                    })
                })
                .and_then(|_| {
                    run.request_verification_approval(&facts_for(profile), confirmer)
                        .map_err(|error| format!("Not started: {error}."))
                })
                .and_then(|approval| {
                    run.begin_verification(approval, inputs_for(profile, &toolchain))
                        .map_err(|error| format!("Not started: {error}."))
                });
            match prepared {
                Ok(generation) => Ok((workspace, generation)),
                Err(message) => {
                    // Nothing ran; the workspace holds only the candidate copy.
                    let removed = workspace.remove().is_ok();
                    Err(if removed {
                        message
                    } else {
                        format!("{message} Its workspace could not be removed.")
                    })
                }
            }
        })();
        let (workspace, generation) = match started {
            Ok(started) => started,
            Err(message) => {
                let _ = run.abandon_verification();
                slot.refresh(&run, "review", Some(message));
                return Ok(slot.view(id));
            }
        };
        slot.display().excerpts = (None, None);
        slot.refresh(
            &run,
            "verifying",
            Some("Verifying in the sandbox; the result is advisory.".to_string()),
        );
        drop(run);
        let launch = Launch {
            profile,
            toolchain,
            workspace,
            helper,
        };
        let flow = Arc::clone(self);
        let worker = Arc::clone(&slot);
        // A launch the thread never received runs nothing; the Launch
        // (and its workspace) is dropped with the closure.
        let spawned = std::thread::Builder::new()
            .name("nexus-verifier".to_string())
            .spawn(move || execute(&flow, &worker, id, generation, launch));
        if spawned.is_err() {
            finish(
                self,
                &slot,
                id,
                generation,
                Finished {
                    exit: VerifierExit::SandboxSetupFailed,
                    duration: Duration::ZERO,
                    stdout: StreamRecord::default(),
                    stderr: StreamRecord::default(),
                    cleanup_confirmed: true,
                    retained: None,
                },
            );
        }
        Ok(slot.view(id))
    }

    /// Retry the cleanup of a verification whose cleanup was unconfirmed.
    pub(crate) fn retry_verification_cleanup(&self, run_id: &str) -> Result<RunView, String> {
        let (id, slot) = self.slot(run_id)?;
        let retained = self
            .retained
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&id);
        let Some(Retained {
            generation,
            boundary,
            workspace,
        }) = retained
        else {
            return Err("there is no verification cleanup to retry".to_string());
        };
        let mut run = claim(&slot, &["review"], "verifying")?;
        let boundary = boundary.and_then(|boundary| boundary.retry().err());
        // The workspace goes only once no verifier process can remain.
        let workspace = match boundary {
            Some(_) => workspace,
            None => workspace.and_then(Leftover::remove),
        };
        let message = if boundary.is_none() && workspace.is_none() {
            match run.confirm_verification_cleanup(generation) {
                Ok(()) => "The verification's cleanup is now confirmed.".to_string(),
                Err(error) => format!("The confirmed cleanup could not be recorded: {error}."),
            }
        } else {
            self.retained
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(
                    id,
                    Retained {
                        generation,
                        boundary,
                        workspace,
                    },
                );
            "The verification's cleanup is still unconfirmed; Apply stays refused.".to_string()
        };
        slot.refresh(&run, "review", Some(message));
        Ok(slot.view(id))
    }
}

/// What finalization observed.
struct Finished {
    exit: VerifierExit,
    duration: Duration,
    stdout: StreamRecord,
    stderr: StreamRecord,
    /// Whether the execution's boundary and workspace are proven gone.
    cleanup_confirmed: bool,
    /// What can still be retried, if cleanup is unconfirmed.
    retained: Option<Retained>,
}

/// The verification thread: set up, run and finalize one launch. A panic
/// here still finalizes the run, as an unknown result whose cleanup can
/// never be confirmed (nothing is left to retry, so Apply stays refused and
/// the owner may discard the run).
fn execute(
    flow: &Arc<CodingFlow>,
    slot: &RunSlot,
    id: RunId,
    generation: ExecutionGeneration,
    launch: Launch,
) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_launch(slot, generation, launch)
    }));
    let finished = outcome.unwrap_or(Finished {
        exit: VerifierExit::SandboxFailed,
        duration: Duration::ZERO,
        stdout: StreamRecord::default(),
        stderr: StreamRecord::default(),
        cleanup_confirmed: false,
        retained: None,
    });
    finish(flow, slot, id, generation, finished);
}

fn run_launch(slot: &RunSlot, generation: ExecutionGeneration, launch: Launch) -> Finished {
    let Launch {
        profile,
        toolchain,
        workspace,
        helper,
    } = launch;
    // Set-up failures ran nothing.
    let mut ran = None;
    let ran_exit = match launch_spec(profile, &toolchain, &workspace, generation.get()) {
        Err(error) => setup_exit(&error),
        Ok(spec) => match ScopeManager::connect() {
            Err(_) => VerifierExit::SandboxUnavailable,
            Ok(scopes) => {
                let _ = with_run(slot, |run| run.verification_running(generation));
                let report = execution::run(&scopes, &helper, spec, &profile.resources);
                let exit = exit_of(report.classify(profile.passing_exit_code));
                ran = Some(report);
                exit
            }
        },
    };
    let _ = with_run(slot, |run| run.verification_finalizing(generation));
    let (duration, stdout, stderr, boundary) = match ran {
        None => (
            Duration::ZERO,
            StreamRecord::default(),
            StreamRecord::default(),
            None,
        ),
        Some(report) => (
            report.duration,
            report.stdout,
            report.stderr,
            match report.cleanup {
                Cleanup::Confirmed => None,
                Cleanup::Failed(boundary) => Some(boundary),
            },
        ),
    };
    let (workspace, input_unchanged) = if boundary.is_some() {
        // A verifier process may remain: nothing is removed or trusted yet.
        (Some(Leftover::Pending(workspace)), false)
    } else {
        let unchanged = workspace
            .directory(Area::Input)
            .map(|input| with_run(slot, |run| run.check_verification_input(input).is_ok()))
            .unwrap_or(false);
        (workspace.remove().err().map(Leftover::Retained), unchanged)
    };
    let (exit, cleanup_confirmed) = settle(
        ran_exit,
        boundary.is_some(),
        workspace.is_some(),
        input_unchanged,
    );
    let retained = (!cleanup_confirmed).then_some(Retained {
        generation,
        boundary,
        workspace,
    });
    Finished {
        exit,
        duration,
        stdout,
        stderr,
        cleanup_confirmed,
        retained,
    }
}

/// Bind what was observed into the run and show it.
fn finish(
    flow: &CodingFlow,
    slot: &RunSlot,
    id: RunId,
    generation: ExecutionGeneration,
    finished: Finished,
) {
    let cleanup = if finished.cleanup_confirmed {
        VerifierCleanup::Confirmed
    } else {
        VerifierCleanup::Failed
    };
    let outcome = VerifierOutcome {
        exit: finished.exit,
        duration_ms: u64::try_from(finished.duration.as_millis()).unwrap_or(u64::MAX),
        stdout: summary(&finished.stdout),
        stderr: summary(&finished.stderr),
        cleanup,
    };
    if let Some(retained) = finished.retained {
        flow.retained
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id, retained);
    }
    let mut run = slot.run.lock().unwrap_or_else(|p| p.into_inner());
    slot.display().excerpts = (excerpt(&finished.stdout), excerpt(&finished.stderr));
    let message = match run.finish_verification(generation, outcome) {
        Ok(_) if !finished.cleanup_confirmed => message_for(VerifierExit::CleanupFailed),
        Ok(_) => message_for(finished.exit),
        Err(error) => format!("The verification result could not be recorded ({error}); it is not part of the review."),
    };
    if let Ok(review) = run.review() {
        slot.display().review = Some(review_view(&review));
    }
    slot.refresh(&run, "review", Some(message));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p2h_finalization_never_reports_more_than_it_proved() {
        let passed = VerifierExit::Passed;
        assert_eq!(settle(passed, false, false, true), (passed, true));
        // Unconfirmed cleanup outranks a changed input and any result.
        for (boundary, workspace) in [(true, false), (false, true), (true, true)] {
            for unchanged in [true, false] {
                assert_eq!(
                    settle(passed, boundary, workspace, unchanged),
                    (VerifierExit::CleanupFailed, false)
                );
            }
        }
        // A changed input invalidates even a pass.
        assert_eq!(
            settle(passed, false, false, false),
            (VerifierExit::CandidateChanged, true)
        );
        let failed = VerifierExit::Failed { exit_code: 101 };
        assert_eq!(settle(failed, false, false, true), (failed, true));
    }

    #[test]
    fn p2h_every_sandbox_class_maps_to_its_own_result_class() {
        let classes = [
            ExitClass::Passed,
            ExitClass::Failed { exit_code: 101 },
            ExitClass::TimedOut,
            ExitClass::OutputLimitExceeded,
            ExitClass::OomKilled,
            ExitClass::ProcessLimit,
            ExitClass::Signalled { signal: 9 },
            ExitClass::SandboxUnavailable,
            ExitClass::SandboxSetupFailed,
            ExitClass::SandboxFailed,
            ExitClass::CleanupFailed,
        ];
        let names: std::collections::BTreeSet<&str> =
            classes.iter().map(|class| exit_of(*class).name()).collect();
        assert_eq!(names.len(), classes.len());
        assert_eq!(exit_of(ExitClass::Passed), VerifierExit::Passed);
        assert!(classes[1..].iter().all(|class| !exit_of(*class).passed()));
        assert_eq!(
            setup_exit(&LaunchSetupError::ToolchainMismatch),
            VerifierExit::ToolchainUnavailable
        );
        assert_eq!(
            setup_exit(&LaunchSetupError::Device),
            VerifierExit::SandboxUnavailable
        );
    }

    #[test]
    fn p2h_output_is_shown_only_as_a_bounded_escaped_tail() {
        let mut record = StreamRecord::default();
        assert_eq!(excerpt(&record), None);
        record.excerpt = format!("{}\u{202e}evil\u{1b}[31m", "x".repeat(10_000)).into_bytes();
        let shown = excerpt(&record).unwrap();
        assert!(!shown.contains('\u{202e}') && !shown.contains('\u{1b}'));
        assert!(shown.contains("evil"));
        assert!(shown.chars().count() <= EXCERPT_CHARS * 12);
        assert!(!shown.starts_with(&"x".repeat(EXCERPT_CHARS)));
    }
}
