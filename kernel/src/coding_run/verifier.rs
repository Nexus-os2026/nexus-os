//! Governed verification execution: the run-bound types (Phase Two, P2B).
//!
//! Data only. Nothing in this module starts anything. It defines what a
//! verifier launch binds to, the owner's native launch approval, the
//! execution generation, and the result a finalized execution produces.
//!
//! A launch binds the run, the exact candidate (its manifest hash and its
//! structural verification binding) and the verifier inputs: the verifier
//! profile, the verified toolchain digest and generation, the sandbox policy
//! and the resource policy. The owner's approval
//! ([`VerifierLaunchApproval`]) exists only after the backend's native
//! confirmation of exactly that binding; it cannot be constructed,
//! deserialized or copied outside this crate, and a launch consumes it.
//!
//! A [`VerificationResult`] is bound under its own domain to the same values
//! plus the execution generation and the outcome. It is advisory evidence:
//! it grants nothing, and output bytes are never part of any binding.
//!
//! P2H: a run's sandboxed verification has its own lifecycle
//! ([`VerificationPhase`]), separate from the run's state: prepared (the
//! launch binding recorded), materialized, approved (the owner's native
//! confirmation of exactly that binding), starting, running, finalizing,
//! then idle again or `CleanupFailed`. One execution at a time; generations
//! only increase; the latest finalized result is bound into the owner's
//! review, and Apply is refused while an execution is starting, running or
//! finalizing, or its cleanup is unconfirmed. Every transition that matters
//! is recorded in the coding-run ledger first (hashes, sizes and classes
//! only); a transition that cannot be recorded does not happen.

use std::collections::BTreeSet;

use serde_json::json;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::ledger::{record, EventKind};
use super::manifest::{put_bytes, Manifest, ManifestEntry, ManifestHash};
use super::review::{display_safe, ReviewBinding};
use super::scope::RelPath;
use super::{CodingRun, RunError, RunId, StructuralVerification};

const INPUTS_DOMAIN: &[u8] = b"nexus.coding_run.verifier_inputs.v1";
const LAUNCH_DOMAIN: &[u8] = b"nexus.coding_run.verifier_launch.v1";
const RESULT_DOMAIN: &[u8] = b"nexus.verification.result.v1";
const MAX_PROFILE_DISPLAY: usize = 64;

/// Everything outside the candidate that a verification depends on. The
/// backend fills it from its own verified objects; each value is an
/// identity, not a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifierInputs {
    pub profile_hash: [u8; 32],
    pub toolchain_digest: [u8; 32],
    pub toolchain_generation: u64,
    pub sandbox_policy_hash: [u8; 32],
    pub resource_policy_hash: [u8; 32],
}

impl VerifierInputs {
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        put_bytes(&mut hasher, INPUTS_DOMAIN);
        self.put(&mut hasher);
        hasher.finalize().into()
    }

    fn put(&self, hasher: &mut Sha256) {
        hasher.update(self.profile_hash);
        hasher.update(self.toolchain_digest);
        hasher.update(self.toolchain_generation.to_be_bytes());
        hasher.update(self.sandbox_policy_hash);
        hasher.update(self.resource_policy_hash);
    }
}

/// Exactly what a verifier launch approval binds to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifierLaunchBinding {
    pub run_id: RunId,
    pub candidate_manifest_hash: ManifestHash,
    pub structural_binding_hash: [u8; 32],
    pub inputs: VerifierInputs,
}

impl VerifierLaunchBinding {
    /// The binding for launching `inputs` against a structurally verified
    /// candidate.
    pub(crate) fn new(verification: &StructuralVerification, inputs: VerifierInputs) -> Self {
        Self {
            run_id: verification.run_id,
            candidate_manifest_hash: verification.candidate_manifest_hash,
            structural_binding_hash: verification.binding_hash(),
            inputs,
        }
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        put_bytes(&mut hasher, LAUNCH_DOMAIN);
        hasher.update(self.run_id.0.as_bytes());
        hasher.update(self.candidate_manifest_hash.bytes());
        hasher.update(self.structural_binding_hash);
        self.inputs.put(&mut hasher);
        hasher.finalize().into()
    }
}

/// A verifier execution's generation within its run. Generations only
/// increase; the backend assigns the next one when a launch begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExecutionGeneration(u64);

impl ExecutionGeneration {
    pub(crate) const FIRST: Self = Self(1);

    /// The following generation, or `None` if none remains.
    pub(crate) fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

/// How a verifier execution ended. Every class is distinct: an unknown or
/// infrastructure failure is never reported as passed or as an ordinary
/// failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifierExit {
    /// The profile's executable exited with its passing status.
    Passed,
    /// The profile's executable exited with another status.
    Failed { exit_code: i32 },
    /// The wall-clock deadline ended the execution.
    TimedOut,
    /// An output stream exceeded its ceiling and the execution was ended.
    OutputLimitExceeded,
    /// The memory limit was reached and the kernel killed a process.
    OomKilled,
    /// The process limit was reached.
    ProcessLimit,
    /// The executable was ended by a signal it was not sent by the backend.
    Signalled { signal: i32 },
    /// A required isolation layer is not available on this host.
    SandboxUnavailable,
    /// The sandbox could not be established; nothing untrusted ran.
    SandboxSetupFailed,
    /// The executable ran, but the sandbox lost it or its accounting before
    /// a final report: the result is unknown.
    SandboxFailed,
    /// The verified toolchain is missing or failed verification.
    ToolchainUnavailable,
    /// The materialized candidate no longer matched the verified candidate.
    CandidateChanged,
    /// The execution's cleanup could not be confirmed.
    CleanupFailed,
}

impl VerifierExit {
    pub fn passed(self) -> bool {
        self == Self::Passed
    }

    fn code(self) -> (u64, i64) {
        match self {
            Self::Passed => (1, 0),
            Self::Failed { exit_code } => (2, i64::from(exit_code)),
            Self::TimedOut => (3, 0),
            Self::OutputLimitExceeded => (4, 0),
            Self::OomKilled => (5, 0),
            Self::ProcessLimit => (6, 0),
            Self::Signalled { signal } => (7, i64::from(signal)),
            Self::SandboxUnavailable => (8, 0),
            Self::SandboxSetupFailed => (9, 0),
            Self::ToolchainUnavailable => (10, 0),
            Self::CandidateChanged => (11, 0),
            Self::CleanupFailed => (12, 0),
            Self::SandboxFailed => (13, 0),
        }
    }
}

/// Byte count, SHA-256 and truncation of one output stream. The bytes are
/// untrusted data and never part of a binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamSummary {
    pub bytes: u64,
    pub sha256: [u8; 32],
    pub truncated: bool,
}

/// Whether the execution's cleanup was confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifierCleanup {
    Confirmed,
    Failed,
}

/// What the backend observed about one finished execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifierOutcome {
    pub exit: VerifierExit,
    pub duration_ms: u64,
    pub stdout: StreamSummary,
    pub stderr: StreamSummary,
    pub cleanup: VerifierCleanup,
}

/// A finalized verification, bound to its run, the exact candidate, the
/// verifier inputs, the execution generation and the outcome. Advisory; it
/// grants nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationResult {
    pub run_id: RunId,
    pub candidate_manifest_hash: ManifestHash,
    pub structural_binding_hash: [u8; 32],
    pub inputs: VerifierInputs,
    pub generation: ExecutionGeneration,
    pub outcome: VerifierOutcome,
}

impl VerificationResult {
    /// Whether this result belongs to exactly this launch.
    pub fn is_for(&self, binding: &VerifierLaunchBinding) -> bool {
        self.run_id == binding.run_id
            && self.candidate_manifest_hash == binding.candidate_manifest_hash
            && self.structural_binding_hash == binding.structural_binding_hash
            && self.inputs == binding.inputs
    }

    /// Canonical hash of every field, under `nexus.verification.result.v1`.
    pub fn binding_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        put_bytes(&mut hasher, RESULT_DOMAIN);
        hasher.update(self.run_id.0.as_bytes());
        hasher.update(self.candidate_manifest_hash.bytes());
        hasher.update(self.structural_binding_hash);
        self.inputs.put(&mut hasher);
        hasher.update(self.generation.0.to_be_bytes());
        let (class, detail) = self.outcome.exit.code();
        hasher.update(class.to_be_bytes());
        hasher.update(detail.to_be_bytes());
        hasher.update(self.outcome.duration_ms.to_be_bytes());
        for stream in [&self.outcome.stdout, &self.outcome.stderr] {
            hasher.update(stream.bytes.to_be_bytes());
            hasher.update(stream.sha256);
            hasher.update([u8::from(stream.truncated)]);
        }
        hasher.update([match self.outcome.cleanup {
            VerifierCleanup::Confirmed => 1u8,
            VerifierCleanup::Failed => 2u8,
        }]);
        hasher.finalize().into()
    }
}

/// What a review binding records about verification: the latest finalized
/// result, or explicitly none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifierMarker {
    NoResult,
    Result([u8; 32]),
}

impl VerifierMarker {
    pub(crate) fn put(&self, hasher: &mut Sha256) {
        match self {
            Self::NoResult => hasher.update([0u8]),
            Self::Result(hash) => {
                hasher.update([1u8]);
                hasher.update(hash);
            }
        }
    }
}

/// The backend's native confirmation of a verifier launch. Only the
/// desktop's native dialog adapter implements it.
pub trait VerifierLaunchConfirmer {
    /// Show the request natively; `true` only if the owner confirmed.
    fn confirm_launch(&self, request: &VerifierLaunchRequest) -> bool;
}

/// Display facts the backend supplies with the inputs, taken from the same
/// compiled-in profile the inputs identify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifierLaunchFacts {
    pub profile_display: String,
    pub wall_timeout_secs: u64,
    pub memory_max_bytes: u64,
    pub cpus: u64,
    pub processes: u64,
}

/// Backend-computed facts shown to the owner before a launch. Bounded; it
/// holds no path, environment value or secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifierLaunchRequest {
    pub run_id: RunId,
    pub profile: String,
    pub candidate_short: String,
    pub wall_timeout_secs: u64,
    pub memory_mib: u64,
    pub cpus: u64,
    pub processes: u64,
}

impl VerifierLaunchRequest {
    pub(crate) fn new(binding: &VerifierLaunchBinding, facts: &VerifierLaunchFacts) -> Self {
        Self {
            run_id: binding.run_id,
            profile: display_safe(&facts.profile_display, false)
                .chars()
                .take(MAX_PROFILE_DISPLAY)
                .collect(),
            candidate_short: binding.candidate_manifest_hash.to_hex()[..12].to_string(),
            wall_timeout_secs: facts.wall_timeout_secs,
            memory_mib: facts.memory_max_bytes / (1024 * 1024),
            cpus: facts.cpus,
            processes: facts.processes,
        }
    }

    pub fn title(&self) -> &'static str {
        "Run this candidate's tests in the verifier sandbox?"
    }

    /// The text of the native confirmation.
    pub fn message(&self) -> String {
        format!(
            "Nexus will run the verification \"{}\" on this machine, in the \
             verifier sandbox, with no network access.\n\nRun: {}\nCandidate: {}\n\
             Limits: {} s, {} MiB memory, {} CPUs, {} processes\n\n\
             The candidate's own test code runs; the result is advisory.",
            self.profile,
            self.run_id,
            self.candidate_short,
            self.wall_timeout_secs,
            self.memory_mib,
            self.cpus,
            self.processes
        )
    }
}

/// The owner's native approval of exactly one verifier launch binding. It
/// cannot be constructed, deserialized or cloned outside this crate, and a
/// launch consumes it.
#[derive(Debug)]
pub struct VerifierLaunchApproval {
    binding: VerifierLaunchBinding,
}

impl VerifierLaunchApproval {
    /// Only after the owner confirmed `binding` natively.
    pub(crate) fn confirmed(binding: VerifierLaunchBinding) -> Self {
        Self { binding }
    }

    pub fn binding(&self) -> &VerifierLaunchBinding {
        &self.binding
    }
}

/// Where a run's sandboxed verification stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationPhase {
    /// Nothing is prepared or running: none has run, or the latest is
    /// finalized.
    Idle,
    /// A launch binding was prepared and recorded.
    Prepared,
    /// The candidate was materialized for the prepared launch.
    Materialized,
    /// The owner approved the prepared launch natively.
    Approved,
    /// The approved launch was taken; the sandbox is being set up.
    Starting,
    /// The verifier is running.
    Running,
    /// The execution ended; its cleanup and input check are being finalized.
    Finalizing,
    /// The last execution's cleanup could not be confirmed.
    CleanupFailed,
}

impl VerificationPhase {
    /// Whether this phase refuses Apply.
    pub fn blocks_apply(self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Running | Self::Finalizing | Self::CleanupFailed
        )
    }
}

/// Why a verification step was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum VerificationError {
    #[error("verification is not available in this state")]
    InvalidState,
    #[error("the verified candidate is no longer available")]
    CandidateUnavailable,
    #[error("the launch no longer matches what was approved")]
    Stale,
    #[error("the owner declined the verification")]
    Declined,
    #[error("the verification could not be recorded")]
    Unrecorded,
    #[error("no execution generation remains")]
    Exhausted,
}

/// A run's verification lifecycle state.
#[derive(Debug)]
pub(crate) struct VerifierState {
    phase: VerificationPhase,
    /// The binding being prepared, approved or executed.
    binding: Option<VerifierLaunchBinding>,
    /// The generation of the active or last execution.
    current: Option<ExecutionGeneration>,
    /// The next generation to assign, if any remains.
    next: Option<ExecutionGeneration>,
    /// The latest finalized result.
    latest: Option<VerificationResult>,
}

impl Default for VerifierState {
    fn default() -> Self {
        Self {
            phase: VerificationPhase::Idle,
            binding: None,
            current: None,
            next: Some(ExecutionGeneration::FIRST),
            latest: None,
        }
    }
}

impl VerifierState {
    /// The materialization step of a prepared launch (an idle run may also
    /// materialize, outside any launch).
    pub(crate) fn may_materialize(&self) -> bool {
        matches!(
            self.phase,
            VerificationPhase::Idle | VerificationPhase::Prepared
        )
    }

    pub(crate) fn materialized(&mut self) {
        if self.phase == VerificationPhase::Prepared {
            self.phase = VerificationPhase::Materialized;
        }
    }

    fn reset(&mut self) {
        self.phase = VerificationPhase::Idle;
        self.binding = None;
    }
}

fn exit_name(exit: VerifierExit) -> &'static str {
    match exit {
        VerifierExit::Passed => "passed",
        VerifierExit::Failed { .. } => "failed",
        VerifierExit::TimedOut => "timed_out",
        VerifierExit::OutputLimitExceeded => "output_limit_exceeded",
        VerifierExit::OomKilled => "oom_killed",
        VerifierExit::ProcessLimit => "process_limit",
        VerifierExit::Signalled { .. } => "signalled",
        VerifierExit::SandboxUnavailable => "sandbox_unavailable",
        VerifierExit::SandboxSetupFailed => "sandbox_setup_failed",
        VerifierExit::SandboxFailed => "sandbox_failed",
        VerifierExit::ToolchainUnavailable => "toolchain_unavailable",
        VerifierExit::CandidateChanged => "candidate_changed",
        VerifierExit::CleanupFailed => "cleanup_failed",
    }
}

impl VerifierExit {
    /// A stable, bounded name for display and the ledger.
    pub fn name(self) -> &'static str {
        exit_name(self)
    }
}

/// The verified candidate as a verifier profile's applicability check sees
/// it: its file paths, and reads of small files checked against the
/// verified manifest.
pub struct VerificationCandidate<'a> {
    run: &'a CodingRun,
    manifest: Manifest,
}

impl VerificationCandidate<'_> {
    pub fn paths(&self) -> BTreeSet<String> {
        self.manifest
            .entries()
            .keys()
            .map(RelPath::as_string)
            .collect()
    }

    /// One candidate file of at most `max_bytes`, only if it still hashes
    /// to its verified entry.
    pub fn read(&self, path: &str, max_bytes: u64) -> Option<Vec<u8>> {
        let path = RelPath::parse(path).ok()?;
        let entry = self.manifest.get(&path)?;
        if entry.size > max_bytes {
            return None;
        }
        let bytes = self.run.read_staged_file(&path, max_bytes).ok()?;
        (ManifestEntry::of(&bytes) == *entry).then_some(bytes)
    }
}

impl CodingRun {
    /// Where this run's sandboxed verification stands.
    pub fn verification_phase(&self) -> VerificationPhase {
        self.verifier.phase
    }

    /// The latest finalized sandboxed verification of this run.
    pub fn latest_verification(&self) -> Option<&VerificationResult> {
        self.verifier.latest.as_ref()
    }

    /// The generation of the active or last execution.
    pub fn verification_generation(&self) -> Option<ExecutionGeneration> {
        self.verifier.current
    }

    /// The verified candidate, re-scanned, for an applicability check.
    pub fn verification_candidate(&self) -> Result<VerificationCandidate<'_>, RunError> {
        let verification = self.unapplied_verification("verification_candidate")?;
        let manifest = self.staged_candidate(&verification)?;
        Ok(VerificationCandidate {
            run: self,
            manifest,
        })
    }

    /// The review binding of the current candidate, with the latest
    /// finalized verification (which must be this candidate's) or none.
    pub(crate) fn review_binding(
        &self,
        verification: &StructuralVerification,
    ) -> Result<ReviewBinding, RunError> {
        let marker = match &self.verifier.latest {
            None => VerifierMarker::NoResult,
            Some(result)
                if result.run_id == self.id
                    && result.candidate_manifest_hash == verification.candidate_manifest_hash
                    && result.structural_binding_hash == verification.binding_hash() =>
            {
                VerifierMarker::Result(result.binding_hash())
            }
            Some(_) => return Err(RunError::CandidateChanged),
        };
        Ok(ReviewBinding {
            run_id: self.id,
            base_manifest_hash: verification.base_manifest_hash,
            candidate_manifest_hash: verification.candidate_manifest_hash,
            profile_hash: verification.profile_hash,
            verification: marker,
        })
    }

    fn current_structural(
        &self,
        operation: &'static str,
    ) -> Result<StructuralVerification, VerificationError> {
        self.unapplied_verification(operation)
            .map_err(|error| match error {
                RunError::InvalidState { .. } => VerificationError::InvalidState,
                _ => VerificationError::CandidateUnavailable,
            })
    }

    /// Prepare a launch of `inputs` against the verified candidate and record
    /// its binding. Refused while an execution is active or its cleanup is
    /// unconfirmed.
    pub fn prepare_verification(
        &mut self,
        inputs: VerifierInputs,
    ) -> Result<VerifierLaunchBinding, VerificationError> {
        if self.verifier.phase.blocks_apply() {
            return Err(VerificationError::InvalidState);
        }
        let verification = self.current_structural("prepare_verification")?;
        let binding = VerifierLaunchBinding::new(&verification, inputs);
        record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::VerificationPrepared,
            &json!({
                "binding": hex::encode(binding.hash()),
                "candidate_manifest": binding.candidate_manifest_hash.to_hex(),
                "structural_binding": hex::encode(binding.structural_binding_hash),
                "profile": hex::encode(inputs.profile_hash),
                "toolchain": hex::encode(inputs.toolchain_digest),
                "toolchain_generation": inputs.toolchain_generation,
                "sandbox_policy": hex::encode(inputs.sandbox_policy_hash),
                "resource_policy": hex::encode(inputs.resource_policy_hash),
            }),
        )
        .map_err(|_| VerificationError::Unrecorded)?;
        self.verifier.phase = VerificationPhase::Prepared;
        self.verifier.binding = Some(binding);
        Ok(binding)
    }

    /// Give up a prepared or approved launch that has not started.
    pub fn abandon_verification(&mut self) -> Result<(), VerificationError> {
        match self.verifier.phase {
            VerificationPhase::Prepared
            | VerificationPhase::Materialized
            | VerificationPhase::Approved => {
                self.verifier.reset();
                Ok(())
            }
            _ => Err(VerificationError::InvalidState),
        }
    }

    /// Ask the owner, through the backend's native confirmation, to approve
    /// exactly the prepared, materialized launch. The decision is recorded;
    /// only a recorded confirmation yields the single-use approval.
    pub fn request_verification_approval(
        &mut self,
        facts: &VerifierLaunchFacts,
        confirmer: &dyn VerifierLaunchConfirmer,
    ) -> Result<VerifierLaunchApproval, VerificationError> {
        let binding = match (self.verifier.phase, self.verifier.binding) {
            (VerificationPhase::Materialized, Some(binding)) => binding,
            _ => return Err(VerificationError::InvalidState),
        };
        match self.current_structural("request_verification_approval") {
            Ok(verification)
                if VerifierLaunchBinding::new(&verification, binding.inputs) == binding => {}
            Ok(_) => {
                self.verifier.reset();
                return Err(VerificationError::Stale);
            }
            Err(error) => {
                self.verifier.reset();
                return Err(error);
            }
        }
        let request = VerifierLaunchRequest::new(&binding, facts);
        let confirmed = confirmer.confirm_launch(&request);
        let event = if confirmed {
            EventKind::VerificationApproved
        } else {
            EventKind::VerificationDeclined
        };
        let recorded = record(
            self.ledger.as_ref(),
            self.id.0,
            event,
            &json!({ "binding": hex::encode(binding.hash()) }),
        );
        if recorded.is_err() {
            self.verifier.reset();
            return Err(VerificationError::Unrecorded);
        }
        if !confirmed {
            self.verifier.reset();
            return Err(VerificationError::Declined);
        }
        self.verifier.phase = VerificationPhase::Approved;
        Ok(VerifierLaunchApproval::confirmed(binding))
    }

    /// Take the approval for a launch of `inputs`, computed again by the
    /// backend immediately before the launch: they, the candidate and the
    /// approval must all still be exactly what was approved. Assigns and
    /// records the execution generation.
    pub fn begin_verification(
        &mut self,
        approval: VerifierLaunchApproval,
        inputs: VerifierInputs,
    ) -> Result<ExecutionGeneration, VerificationError> {
        let approved = match (self.verifier.phase, self.verifier.binding) {
            (VerificationPhase::Approved, Some(binding)) => binding,
            _ => return Err(VerificationError::InvalidState),
        };
        let verification = match self.current_structural("begin_verification") {
            Ok(verification) => verification,
            Err(error) => {
                self.verifier.reset();
                return Err(error);
            }
        };
        let current = VerifierLaunchBinding::new(&verification, inputs);
        if approval.binding != approved || approval.binding != current {
            self.verifier.reset();
            return Err(VerificationError::Stale);
        }
        let Some(generation) = self.verifier.next else {
            self.verifier.reset();
            return Err(VerificationError::Exhausted);
        };
        if record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::VerificationLaunched,
            &json!({
                "binding": hex::encode(current.hash()),
                "generation": generation.get(),
            }),
        )
        .is_err()
        {
            self.verifier.reset();
            return Err(VerificationError::Unrecorded);
        }
        self.verifier.next = generation.next();
        self.verifier.current = Some(generation);
        self.verifier.phase = VerificationPhase::Starting;
        Ok(generation)
    }

    fn active_generation(
        &self,
        generation: ExecutionGeneration,
        allowed: &[VerificationPhase],
    ) -> Result<(), VerificationError> {
        if allowed.contains(&self.verifier.phase) && self.verifier.current == Some(generation) {
            Ok(())
        } else {
            Err(VerificationError::InvalidState)
        }
    }

    /// The verifier of `generation` is running.
    pub fn verification_running(
        &mut self,
        generation: ExecutionGeneration,
    ) -> Result<(), VerificationError> {
        self.active_generation(generation, &[VerificationPhase::Starting])?;
        self.verifier.phase = VerificationPhase::Running;
        Ok(())
    }

    /// The execution of `generation` ended; it is being finalized.
    pub fn verification_finalizing(
        &mut self,
        generation: ExecutionGeneration,
    ) -> Result<(), VerificationError> {
        self.active_generation(
            generation,
            &[VerificationPhase::Starting, VerificationPhase::Running],
        )?;
        self.verifier.phase = VerificationPhase::Finalizing;
        Ok(())
    }

    /// Finalize the execution of `generation` with what the backend
    /// observed. The result is bound to the launch that was approved, never
    /// to anything the caller names, and is recorded before it can be
    /// reviewed. An unconfirmed cleanup leaves the run `CleanupFailed`.
    pub fn finish_verification(
        &mut self,
        generation: ExecutionGeneration,
        outcome: VerifierOutcome,
    ) -> Result<VerificationResult, VerificationError> {
        self.active_generation(
            generation,
            &[
                VerificationPhase::Starting,
                VerificationPhase::Running,
                VerificationPhase::Finalizing,
            ],
        )?;
        let binding = self
            .verifier
            .binding
            .ok_or(VerificationError::InvalidState)?;
        let result = VerificationResult {
            run_id: binding.run_id,
            candidate_manifest_hash: binding.candidate_manifest_hash,
            structural_binding_hash: binding.structural_binding_hash,
            inputs: binding.inputs,
            generation,
            outcome,
        };
        let (_, detail) = outcome.exit.code();
        let stream = |s: &StreamSummary| {
            json!({
                "bytes": s.bytes,
                "sha256": hex::encode(s.sha256),
                "truncated": s.truncated,
            })
        };
        let recorded = record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::VerificationFinished,
            &json!({
                "result": hex::encode(result.binding_hash()),
                "generation": generation.get(),
                "exit": outcome.exit.name(),
                "detail": detail,
                "duration_ms": outcome.duration_ms,
                "stdout": stream(&outcome.stdout),
                "stderr": stream(&outcome.stderr),
                "cleanup": match outcome.cleanup {
                    VerifierCleanup::Confirmed => "confirmed",
                    VerifierCleanup::Failed => "failed",
                },
            }),
        );
        self.verifier.binding = None;
        self.verifier.phase = match outcome.cleanup {
            VerifierCleanup::Confirmed => VerificationPhase::Idle,
            VerifierCleanup::Failed => VerificationPhase::CleanupFailed,
        };
        // An unrecorded result is never review evidence.
        recorded.map_err(|_| VerificationError::Unrecorded)?;
        self.verifier.latest = Some(result);
        Ok(result)
    }

    /// The retained boundary of `generation` is now confirmed gone.
    pub fn confirm_verification_cleanup(
        &mut self,
        generation: ExecutionGeneration,
    ) -> Result<(), VerificationError> {
        self.active_generation(generation, &[VerificationPhase::CleanupFailed])?;
        record(
            self.ledger.as_ref(),
            self.id.0,
            EventKind::VerificationCleanup,
            &json!({ "generation": generation.get(), "cleanup": "confirmed" }),
        )
        .map_err(|_| VerificationError::Unrecorded)?;
        self.verifier.phase = VerificationPhase::Idle;
        Ok(())
    }
}

#[cfg(test)]
#[path = "p2b_verifier_tests.rs"]
mod tests;
