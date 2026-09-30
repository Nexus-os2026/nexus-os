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

// P2B defines these types; P2H wires their crate-private constructors into
// `CodingRun` and removes this allowance.
#![cfg_attr(not(test), allow(dead_code))]

use sha2::{Digest, Sha256};

use super::manifest::{put_bytes, ManifestHash};
use super::review::display_safe;
use super::{RunId, StructuralVerification};

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

#[cfg(test)]
#[path = "p2b_verifier_tests.rs"]
mod tests;
