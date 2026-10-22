use super::super::manifest::{Manifest, ManifestEntry};
use super::super::scope::RelPath;
use super::super::structural::{StructuralOutcome, StructuralProfile};
use super::*;

fn manifest_hash(content: &[u8]) -> ManifestHash {
    let mut manifest = Manifest::default();
    manifest.insert(
        RelPath::parse("src/lib.rs").expect("path"),
        ManifestEntry::of(content),
    );
    manifest.hash()
}

fn verification() -> StructuralVerification {
    StructuralVerification {
        run_id: RunId::generate(),
        base_manifest_hash: manifest_hash(b"base"),
        candidate_manifest_hash: manifest_hash(b"candidate"),
        profile_hash: StructuralProfile::V1.hash(),
        outcome: StructuralOutcome::Passed,
    }
}

fn inputs() -> VerifierInputs {
    VerifierInputs {
        profile_hash: [1; 32],
        toolchain_digest: [2; 32],
        toolchain_generation: 3,
        sandbox_policy_hash: [4; 32],
        resource_policy_hash: [5; 32],
    }
}

fn stream(fill: u8) -> StreamSummary {
    StreamSummary {
        bytes: 10,
        sha256: [fill; 32],
        truncated: false,
    }
}

fn result(binding: &VerifierLaunchBinding) -> VerificationResult {
    VerificationResult {
        run_id: binding.run_id,
        candidate_manifest_hash: binding.candidate_manifest_hash,
        structural_binding_hash: binding.structural_binding_hash,
        inputs: binding.inputs,
        generation: ExecutionGeneration::FIRST,
        outcome: VerifierOutcome {
            exit: VerifierExit::Passed,
            duration_ms: 1234,
            stdout: stream(7),
            stderr: stream(8),
            cleanup: VerifierCleanup::Confirmed,
        },
    }
}

#[test]
fn p2b_launch_binding_derives_from_the_exact_structural_verification() {
    let verification = verification();
    let binding = VerifierLaunchBinding::new(&verification, inputs());
    assert_eq!(binding.run_id, verification.run_id);
    assert_eq!(
        binding.candidate_manifest_hash,
        verification.candidate_manifest_hash
    );
    assert_eq!(binding.structural_binding_hash, verification.binding_hash());
    assert_eq!(binding.inputs, inputs());
}

#[test]
fn p2b_inputs_hash_covers_every_input() {
    let base = inputs();
    assert_eq!(base.hash(), inputs().hash());
    for changed in [
        VerifierInputs {
            profile_hash: [9; 32],
            ..base
        },
        VerifierInputs {
            toolchain_digest: [9; 32],
            ..base
        },
        VerifierInputs {
            toolchain_generation: 4,
            ..base
        },
        VerifierInputs {
            sandbox_policy_hash: [9; 32],
            ..base
        },
        VerifierInputs {
            resource_policy_hash: [9; 32],
            ..base
        },
    ] {
        assert_ne!(changed.hash(), base.hash(), "{changed:?}");
    }
}

#[test]
fn p2b_launch_binding_hash_covers_run_candidate_structure_and_inputs() {
    let binding = VerifierLaunchBinding::new(&verification(), inputs());
    assert_eq!(binding.hash(), binding.hash());
    let other = verification();
    for changed in [
        VerifierLaunchBinding {
            run_id: other.run_id,
            ..binding
        },
        VerifierLaunchBinding {
            candidate_manifest_hash: manifest_hash(b"other candidate"),
            ..binding
        },
        VerifierLaunchBinding {
            structural_binding_hash: [9; 32],
            ..binding
        },
        VerifierLaunchBinding {
            inputs: VerifierInputs {
                toolchain_generation: 99,
                ..binding.inputs
            },
            ..binding
        },
    ] {
        assert_ne!(changed.hash(), binding.hash(), "{changed:?}");
    }
}

#[test]
fn p2b_result_binding_covers_every_field_under_its_own_domain() {
    let binding = VerifierLaunchBinding::new(&verification(), inputs());
    let base = result(&binding);
    assert_eq!(base.binding_hash(), result(&binding).binding_hash());
    assert_ne!(base.binding_hash(), binding.hash(), "domain separated");
    let outcome = base.outcome;
    let other = verification();
    let variants = [
        VerificationResult {
            run_id: other.run_id,
            ..base
        },
        VerificationResult {
            candidate_manifest_hash: manifest_hash(b"x"),
            ..base
        },
        VerificationResult {
            structural_binding_hash: [9; 32],
            ..base
        },
        VerificationResult {
            inputs: VerifierInputs {
                profile_hash: [9; 32],
                ..base.inputs
            },
            ..base
        },
        VerificationResult {
            generation: ExecutionGeneration::FIRST.next().unwrap(),
            ..base
        },
        VerificationResult {
            outcome: VerifierOutcome {
                exit: VerifierExit::Failed { exit_code: 101 },
                ..outcome
            },
            ..base
        },
        VerificationResult {
            outcome: VerifierOutcome {
                duration_ms: 1,
                ..outcome
            },
            ..base
        },
        VerificationResult {
            outcome: VerifierOutcome {
                stdout: StreamSummary {
                    bytes: 11,
                    ..outcome.stdout
                },
                ..outcome
            },
            ..base
        },
        VerificationResult {
            outcome: VerifierOutcome {
                stdout: stream(9),
                ..outcome
            },
            ..base
        },
        VerificationResult {
            outcome: VerifierOutcome {
                stderr: StreamSummary {
                    truncated: true,
                    ..outcome.stderr
                },
                ..outcome
            },
            ..base
        },
        VerificationResult {
            outcome: VerifierOutcome {
                cleanup: VerifierCleanup::Failed,
                ..outcome
            },
            ..base
        },
    ];
    for variant in variants {
        assert_ne!(variant.binding_hash(), base.binding_hash(), "{variant:?}");
    }
    // Swapping the streams is a different result.
    let swapped = VerificationResult {
        outcome: VerifierOutcome {
            stdout: outcome.stderr,
            stderr: outcome.stdout,
            ..outcome
        },
        ..base
    };
    assert_ne!(swapped.binding_hash(), base.binding_hash());
}

#[test]
fn p2b_every_exit_class_is_distinct_and_only_passed_passes() {
    let classes = [
        VerifierExit::Passed,
        VerifierExit::Failed { exit_code: 101 },
        VerifierExit::Failed { exit_code: 1 },
        VerifierExit::TimedOut,
        VerifierExit::OutputLimitExceeded,
        VerifierExit::OomKilled,
        VerifierExit::ProcessLimit,
        VerifierExit::Signalled { signal: 9 },
        VerifierExit::Signalled { signal: 11 },
        VerifierExit::SandboxUnavailable,
        VerifierExit::SandboxSetupFailed,
        VerifierExit::SandboxFailed,
        VerifierExit::ToolchainUnavailable,
        VerifierExit::CandidateChanged,
        VerifierExit::CleanupFailed,
    ];
    let binding = VerifierLaunchBinding::new(&verification(), inputs());
    let base = result(&binding);
    let mut hashes: Vec<[u8; 32]> = classes
        .iter()
        .map(|exit| {
            VerificationResult {
                outcome: VerifierOutcome {
                    exit: *exit,
                    ..base.outcome
                },
                ..base
            }
            .binding_hash()
        })
        .collect();
    hashes.sort_unstable();
    hashes.dedup();
    assert_eq!(hashes.len(), classes.len(), "every class hashes distinctly");
    for exit in classes {
        assert_eq!(exit.passed(), exit == VerifierExit::Passed, "{exit:?}");
    }
    // A zero exit status reported as a failure is still not a pass.
    assert!(!VerifierExit::Failed { exit_code: 0 }.passed());
}

#[test]
fn p2b_a_result_binds_only_its_own_launch() {
    let verification = verification();
    let binding = VerifierLaunchBinding::new(&verification, inputs());
    let own = result(&binding);
    assert!(own.is_for(&binding));
    let other_candidate = VerifierLaunchBinding {
        candidate_manifest_hash: manifest_hash(b"another candidate"),
        ..binding
    };
    assert!(!own.is_for(&other_candidate));
    let other_inputs = VerifierLaunchBinding {
        inputs: VerifierInputs {
            sandbox_policy_hash: [9; 32],
            ..binding.inputs
        },
        ..binding
    };
    assert!(!own.is_for(&other_inputs));
    // A fresh verification belongs to a different run.
    let other_run = VerifierLaunchBinding::new(&verification, inputs());
    let other_run = VerifierLaunchBinding {
        run_id: RunId::generate(),
        ..other_run
    };
    assert!(!own.is_for(&other_run));
}

#[test]
fn p2b_marker_distinguishes_no_result_from_every_result() {
    let hash = |marker: VerifierMarker| {
        let mut hasher = Sha256::new();
        marker.put(&mut hasher);
        <[u8; 32]>::from(hasher.finalize())
    };
    assert_ne!(
        hash(VerifierMarker::NoResult),
        hash(VerifierMarker::Result([0; 32]))
    );
    assert_ne!(
        hash(VerifierMarker::Result([0; 32])),
        hash(VerifierMarker::Result([1; 32]))
    );
}

#[test]
fn p2b_generations_only_increase() {
    let first = ExecutionGeneration::FIRST;
    assert_eq!(first.get(), 1);
    let second = first.next().unwrap();
    assert!(second > first);
    assert_eq!(second.get(), 2);
    assert_eq!(ExecutionGeneration(u64::MAX).next(), None);
}

#[test]
fn p2b_the_launch_prompt_is_bounded_and_display_safe() {
    let binding = VerifierLaunchBinding::new(&verification(), inputs());
    let facts = VerifierLaunchFacts {
        profile_display: format!("Rust\u{202E}tests\n{}", "x".repeat(500)),
        wall_timeout_secs: 600,
        memory_max_bytes: 4 * 1024 * 1024 * 1024,
        cpus: 4,
        processes: 256,
    };
    let request = VerifierLaunchRequest::new(&binding, &facts);
    assert!(request.profile.chars().count() <= MAX_PROFILE_DISPLAY);
    assert!(!request.profile.contains('\u{202E}'));
    assert!(!request.profile.contains('\n'));
    assert_eq!(request.candidate_short.len(), 12);
    assert_eq!(request.memory_mib, 4096);
    let message = request.message();
    assert!(message.contains("no network"));
    assert!(message.contains("600 s"));
    assert!(message.contains("advisory"));
    assert!(!message.contains('/'), "no path in the prompt");
}

#[test]
fn p2b_a_confirmed_approval_names_exactly_its_binding() {
    let binding = VerifierLaunchBinding::new(&verification(), inputs());
    let approval = VerifierLaunchApproval::confirmed(binding);
    assert_eq!(approval.binding(), &binding);
    assert_eq!(approval.binding().hash(), binding.hash());
}
