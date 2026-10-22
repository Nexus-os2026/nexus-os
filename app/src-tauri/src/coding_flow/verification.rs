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
//! A panic never drops what a verification owns. The sandbox's execution
//! itself never unwinds (it finalizes or retains its own boundary); here the
//! boundary it leaves unconfirmed and the workspace are held outside every
//! closure that can panic, and a panic in this thread is settled from them
//! like any other end: the workspace is removed only once no verifier
//! process can remain, and whatever is unconfirmed is retained for a retry.
//!
//! The only execution route is `nexus_verifier_sandbox::execution::run`;
//! output is untrusted data, kept only as bounded, escaped tails.

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nexus_kernel::coding_run::{
    display_safe, CodingRun, ExecutionGeneration, RunId, StreamSummary, VerifierCleanup,
    VerifierExit, VerifierInputs, VerifierLaunchConfirmer, VerifierLaunchFacts, VerifierOutcome,
};
use nexus_verifier_sandbox::applicability;
use nexus_verifier_sandbox::execution::{
    self, Cleanup, ExecutionReport, ExitClass, RetainedBoundary, StreamRecord,
};
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

/// An execution boundary whose cleanup is unconfirmed: the sandbox's
/// [`RetainedBoundary`]. Generic only so the panic paths below can be
/// tested without a live sandbox.
trait Boundary: Sized {
    /// End it again; `Err` keeps it retained.
    fn retry(self) -> Result<(), Self>;
}

impl Boundary for RetainedBoundary {
    fn retry(self) -> Result<(), Self> {
        RetainedBoundary::retry(self)
    }
}

/// What of a verification is still unconfirmed.
struct Unconfirmed<B = RetainedBoundary> {
    /// The execution's boundary: a verifier process may remain.
    boundary: Option<B>,
    workspace: Option<Leftover>,
}

/// A verification whose cleanup is unconfirmed, kept for its run until a
/// retry confirms it.
pub(crate) struct Retained {
    generation: ExecutionGeneration,
    unconfirmed: Unconfirmed,
}

enum Leftover {
    /// Not removed yet: its execution's boundary had to be confirmed first.
    Pending(Workspace),
    Retained(RetainedWorkspace),
}

impl Leftover {
    /// Try the removal (the sandbox's removal never unwinds); what remains
    /// stays retained.
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
        // The launch is handed over only once its thread exists. A launch no
        // thread received ran nothing; it is settled here, its workspace
        // removed or retained like any other.
        let (hand, receive) = std::sync::mpsc::sync_channel::<Launch>(1);
        let spawned = std::thread::Builder::new()
            .name("nexus-verifier".to_string())
            .spawn(move || {
                if let Ok(launch) = receive.recv() {
                    execute(&flow, &worker, id, generation, launch);
                }
            });
        let unreceived = match spawned {
            Ok(_) => hand.send(launch).err().map(|unsent| unsent.0),
            Err(_) => Some(launch),
        };
        if let Some(launch) = unreceived {
            let mut owned = Owned::<RetainedBoundary>::new(Some(launch.workspace));
            owned.release_workspace();
            let unconfirmed = owned.unconfirmed();
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
                    cleanup_confirmed: unconfirmed.is_none(),
                    unconfirmed,
                },
            );
        }
        Ok(slot.view(id))
    }

    /// Retry the cleanup of a verification whose cleanup was unconfirmed.
    pub(crate) fn retry_verification_cleanup(&self, run_id: &str) -> Result<RunView, String> {
        let (id, slot) = self.slot(run_id)?;
        // The run is claimed before its retained cleanup is taken, so a
        // refused claim can never drop it.
        let mut run = claim(&slot, &["review"], "verifying")?;
        let Some(Retained {
            generation,
            unconfirmed,
        }) = take_retained(&self.retained, id)
        else {
            slot.refresh(&run, "review", None);
            return Err("there is no verification cleanup to retry".to_string());
        };
        let message = match retry(unconfirmed) {
            None => match run.confirm_verification_cleanup(generation) {
                Ok(()) => "The verification's cleanup is now confirmed.".to_string(),
                Err(error) => format!("The confirmed cleanup could not be recorded: {error}."),
            },
            Some(unconfirmed) => {
                self.retained
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .insert(
                        id,
                        Retained {
                            generation,
                            unconfirmed,
                        },
                    );
                "The verification's cleanup is still unconfirmed; Apply stays refused.".to_string()
            }
        };
        slot.refresh(&run, "review", Some(message));
        Ok(slot.view(id))
    }
}

fn take_retained(retained: &Mutex<HashMap<RunId, Retained>>, id: RunId) -> Option<Retained> {
    retained
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&id)
}

/// Retry what is unconfirmed: the boundary first, and the workspace only
/// once no verifier process can remain. Returns what is still unconfirmed.
fn retry<B: Boundary>(unconfirmed: Unconfirmed<B>) -> Option<Unconfirmed<B>> {
    let boundary = unconfirmed
        .boundary
        .and_then(|boundary| boundary.retry().err());
    let workspace = match boundary {
        Some(_) => unconfirmed.workspace,
        None => unconfirmed.workspace.and_then(Leftover::remove),
    };
    (boundary.is_some() || workspace.is_some()).then_some(Unconfirmed {
        boundary,
        workspace,
    })
}

/// What finalization observed.
struct Finished<B = RetainedBoundary> {
    exit: VerifierExit,
    duration: Duration,
    stdout: StreamRecord,
    stderr: StreamRecord,
    /// Whether the execution's boundary and workspace are proven gone.
    cleanup_confirmed: bool,
    /// What can still be retried, if cleanup is unconfirmed.
    unconfirmed: Option<Unconfirmed<B>>,
}

/// What a verification owns while its thread works. It lives in
/// [`execute`]'s own frame and is only ever borrowed by the code that can
/// panic.
struct Owned<B> {
    /// The execution's unconfirmed boundary, taken from its report at once.
    boundary: Option<B>,
    /// The workspace, until it is removed or retained.
    workspace: Option<Workspace>,
    /// A workspace whose removal is unconfirmed.
    leftover: Option<Leftover>,
}

impl<B> Owned<B> {
    fn new(workspace: Option<Workspace>) -> Self {
        Self {
            boundary: None,
            workspace,
            leftover: None,
        }
    }

    /// Remove the workspace once no verifier process can remain: only
    /// without an unconfirmed boundary. A removal that is unconfirmed is
    /// kept for a retry.
    fn release_workspace(&mut self) {
        if self.boundary.is_none() {
            if let Some(workspace) = self.workspace.take() {
                self.leftover = workspace.remove().err().map(Leftover::Retained);
            }
        }
    }

    /// Everything still unconfirmed, handed on for a retry; `None` once
    /// nothing is.
    fn unconfirmed(&mut self) -> Option<Unconfirmed<B>> {
        let boundary = self.boundary.take();
        let workspace = self
            .workspace
            .take()
            .map(Leftover::Pending)
            .or_else(|| self.leftover.take());
        (boundary.is_some() || workspace.is_some()).then_some(Unconfirmed {
            boundary,
            workspace,
        })
    }
}

/// Do `work` over what the verification owns; if it panics, settle what
/// is still owned instead (see [`after_panic`]).
fn guarded<B>(
    owned: &mut Owned<B>,
    work: impl FnOnce(&mut Owned<B>) -> Finished<B>,
) -> Finished<B> {
    match catch_unwind(AssertUnwindSafe(|| work(owned))) {
        Ok(finished) => finished,
        Err(_) => after_panic(owned),
    }
}

/// A panic in the verification thread: nothing it observed is trusted (an
/// unknown result). What it owns is settled like any other end: the
/// workspace is removed only if no verifier process can remain, and
/// whatever is unconfirmed is retained for a retry, so Apply stays refused
/// until a retry confirms it.
fn after_panic<B>(owned: &mut Owned<B>) -> Finished<B> {
    owned.release_workspace();
    let unconfirmed = owned.unconfirmed();
    let cleanup_confirmed = unconfirmed.is_none();
    Finished {
        exit: if cleanup_confirmed {
            VerifierExit::SandboxFailed
        } else {
            VerifierExit::CleanupFailed
        },
        duration: Duration::ZERO,
        stdout: StreamRecord::default(),
        stderr: StreamRecord::default(),
        cleanup_confirmed,
        unconfirmed,
    }
}

/// The verification thread: set up, run and finalize one launch.
fn execute(
    flow: &Arc<CodingFlow>,
    slot: &RunSlot,
    id: RunId,
    generation: ExecutionGeneration,
    launch: Launch,
) {
    let Launch {
        profile,
        toolchain,
        workspace,
        helper,
    } = launch;
    let mut owned = Owned::new(Some(workspace));
    let finished = guarded(&mut owned, |owned| {
        run_launch(slot, generation, profile, &toolchain, &helper, owned)
    });
    finish(flow, slot, id, generation, finished);
}

fn run_launch(
    slot: &RunSlot,
    generation: ExecutionGeneration,
    profile: &'static VerifierProfile,
    toolchain: &VerifiedVerifierToolchain,
    helper: &HelperProgram,
    owned: &mut Owned<RetainedBoundary>,
) -> Finished {
    // Set-up failures ran nothing.
    let mut ran = (
        Duration::ZERO,
        StreamRecord::default(),
        StreamRecord::default(),
    );
    let spec = owned
        .workspace
        .as_ref()
        .map(|workspace| launch_spec(profile, toolchain, workspace, generation.get()));
    let ran_exit = match spec {
        // Never: the workspace is owned until the end is settled.
        None => VerifierExit::SandboxSetupFailed,
        Some(Err(error)) => setup_exit(&error),
        Some(Ok(spec)) => match ScopeManager::connect() {
            Err(_) => VerifierExit::SandboxUnavailable,
            Ok(scopes) => {
                let _ = with_run(slot, |run| run.verification_running(generation));
                let report = execution::run(&scopes, helper, spec, &profile.resources);
                let exit = exit_of(report.classify(profile.passing_exit_code));
                let ExecutionReport {
                    duration,
                    stdout,
                    stderr,
                    cleanup,
                    ..
                } = report;
                // An unconfirmed boundary leaves the report at once for what
                // `execute` owns: from here on no panic can drop it.
                if let Cleanup::Failed(boundary) = cleanup {
                    owned.boundary = Some(boundary);
                }
                ran = (duration, stdout, stderr);
                exit
            }
        },
    };
    let _ = with_run(slot, |run| run.verification_finalizing(generation));
    // The input is trusted, and the workspace removed, only once no
    // verifier process can remain.
    let input_unchanged = owned.boundary.is_none()
        && owned
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.directory(Area::Input).ok())
            .is_some_and(|input| with_run(slot, |run| run.check_verification_input(input).is_ok()));
    owned.release_workspace();
    let (exit, cleanup_confirmed) = settle(
        ran_exit,
        owned.boundary.is_some(),
        owned.workspace.is_some() || owned.leftover.is_some(),
        input_unchanged,
    );
    let (duration, stdout, stderr) = ran;
    Finished {
        exit,
        duration,
        stdout,
        stderr,
        cleanup_confirmed,
        unconfirmed: owned.unconfirmed(),
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
    if let Some(unconfirmed) = finished.unconfirmed {
        flow.retained
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                id,
                Retained {
                    generation,
                    unconfirmed,
                },
            );
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
    use std::sync::atomic::{AtomicBool, Ordering};

    /// A stand-in boundary that records being dropped.
    struct Fake {
        dropped: Arc<AtomicBool>,
        confirms: bool,
    }

    impl Boundary for Fake {
        fn retry(self) -> Result<(), Self> {
            if self.confirms {
                Ok(())
            } else {
                Err(self)
            }
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn p2r1_a_panic_after_the_execution_retains_its_unconfirmed_boundary() {
        let dropped = Arc::new(AtomicBool::new(false));
        let mut owned = Owned::<Fake>::new(None);
        let finished = guarded(&mut owned, |owned| {
            // The execution left its boundary unconfirmed; then a later step
            // of the verification thread panics.
            owned.boundary = Some(Fake {
                dropped: Arc::clone(&dropped),
                confirms: false,
            });
            panic!("a later step of the verification thread");
        });
        assert_eq!(finished.exit, VerifierExit::CleanupFailed);
        assert!(!finished.cleanup_confirmed);
        let unconfirmed = finished
            .unconfirmed
            .expect("retained for a retry, never dropped");
        assert!(unconfirmed.boundary.is_some());
        assert!(
            !dropped.load(Ordering::SeqCst),
            "the boundary is still owned"
        );
        // A retry that cannot confirm it keeps it; one that confirms it
        // releases it.
        let mut unconfirmed = retry(unconfirmed).expect("still unconfirmed");
        assert!(!dropped.load(Ordering::SeqCst));
        unconfirmed.boundary.as_mut().unwrap().confirms = true;
        assert!(retry(unconfirmed).is_none());
        assert!(dropped.load(Ordering::SeqCst));
    }

    #[test]
    fn p2r1_a_panic_with_nothing_unconfirmed_is_an_unknown_result() {
        let mut owned = Owned::<Fake>::new(None);
        let finished = guarded(&mut owned, |_| panic!("before any execution"));
        assert_eq!(finished.exit, VerifierExit::SandboxFailed);
        assert!(!finished.exit.passed());
        assert!(finished.cleanup_confirmed);
        assert!(finished.unconfirmed.is_none());
        // A verification that returns is taken as it is.
        let finished = guarded(&mut Owned::<Fake>::new(None), |_| Finished {
            exit: VerifierExit::Failed { exit_code: 101 },
            duration: Duration::from_millis(5),
            stdout: StreamRecord::default(),
            stderr: StreamRecord::default(),
            cleanup_confirmed: true,
            unconfirmed: None,
        });
        assert_eq!(finished.exit, VerifierExit::Failed { exit_code: 101 });
    }

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
