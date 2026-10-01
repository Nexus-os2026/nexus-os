#!/usr/bin/env python3
"""P2-V1-R3B-I2-R1 negative controls.

The 30 counted controls retained from I2A (the eight codec and integration
controls I2A-*, and the 22 custody controls: 14 R1, 7 R2, R2A-SPLIT; text
copied verbatim from i2a-negative-controls.py), plus the two new behavioural
request-receipt controls (NC-REQUEST-*), which run the new receipt
regression c17. Each
mutation must apply exactly once, compile, and fail its intended test (run
alone, --exact) with its marker inside the failing assertion's message. The
six edit-envelope files are verified byte-identical (sha256) before every
control and after every restoration (always performed, in `finally`); the
worktree must show exactly the expected status throughout.
"""
import hashlib
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path("/home/nexus/NEXUS/nexus-os-p2-v1-cleanup-observers")
TESTS = "crates/nexus-verifier-sandbox/tests"
CUSTODY = f"{TESTS}/support/custody"
FILES = [
    f"{CUSTODY}/codec.rs",
    f"{CUSTODY}/core.rs",
    f"{CUSTODY}/mod.rs",
    f"{CUSTODY}/model.rs",
    f"{TESTS}/phase2_custody_codec.rs",
    f"{TESTS}/phase2_custody_core.rs",
]
LOG = pathlib.Path("/home/nexus/.claude/jobs/4a4dc130/tmp/i2r1-negative-controls")
EXPECTED_STATUS = {
    f" M {CUSTODY}/core.rs",
    f" M {CUSTODY}/model.rs",
    f" M {TESTS}/phase2_custody_codec.rs",
}
BOUNDARY = "custody::core::commitment_boundary::"
DECIDED = BOUNDARY + "r2a_commitment_excludes_cancellation_between_its_decision_and_its_record"

RETAINED = [
    # The eight I1 controls, adapted to the corrected core.
    dict(
        id="I1-NC1",
        what="a failed cleanup becomes a boolean and its owner is discarded",
        anchor="        if finished {\n            self.finish(slot, now);\n        }\n        self.publish();\n",
        replacement=(
            "        if finished {\n            self.finish(slot, now);\n        } else {\n"
            "            drop(self.group.take(slot));\n        }\n        self.publish();\n"
        ),
        test="h01_same_owner_survives_failed_cleanup_and_exhausted_budgets",
        marker="[owner-retained]",
    ),
    dict(
        id="I1-NC2",
        what="a timeout becomes a confirmed no-effect",
        anchor="            NativeOutcome::Unknown => Ok(self.mark_unknown(now)),\n",
        replacement=(
            "            NativeOutcome::Unknown => "
            "Ok(self.resolve_without_owner(Settlement::NothingCreated, now)),\n"
        ),
        test="h12_unknown_outcome_blocks_launch_discard_and_fresh_admission",
        marker="[unknown-held]",
    ),
    dict(
        id="I1-NC3",
        what="a dependency is released before native completion",
        anchor=(
            "        let process_ended = self\n            .group\n            .process\n"
            "            .as_ref()\n            .is_none_or(Entry::natively_ended);\n"
        ),
        replacement="        let process_ended = true;\n",
        test="h11_dependencies_release_only_after_native_completion",
        marker="[dependency-order]",
    ),
    dict(
        id="I1-NC4",
        what="STATUS renews the lease",
        anchor="            let lease = lock(&self.shared.lease);\n            LeaseView {\n",
        replacement=(
            "            let mut lease = lock(&self.shared.lease);\n"
            "            if let LeaseState::Active { deadline } = lease.state {\n"
            "                lease.state = LeaseState::Active {\n"
            "                    deadline: deadline.plus(self.shared.lease_millis),\n"
            "                };\n"
            "            }\n"
            "            LeaseView {\n"
        ),
        test="h07_status_never_renews_the_lease",
        marker="[status-no-renew]",
    ),
    dict(
        id="I1-NC5a",
        what="an action is admitted without its start record acknowledged",
        anchor="        if !start_acked {\n",
        replacement="        if false && !start_acked {\n",
        test="h14_capacity_reservation_prevents_unrecordable_admission",
        marker="[start-acknowledged]",
    ),
    dict(
        id="I1-NC5b",
        what="an action is reserved without room for its records",
        anchor="        if !self.ledger.reserve(3) {\n",
        replacement="        if !self.ledger.reserve(0) {\n",
        test="h14_capacity_reservation_prevents_unrecordable_admission",
        marker="[capacity-reserved]",
    ),
    dict(
        id="I1-NC6",
        what="an evicted (stale) request executes again",
        anchor="            None if request.seq <= last => return RequestOutcome::Replayed,\n",
        replacement=(
            "            None if request.seq <= last => "
            "return self.execute(request, digest, cleanup, now),\n"
        ),
        test="h19_replayed_evicted_conflicting_and_foreign_requests_execute_nothing",
        marker="[replay-refused]",
    ),
    dict(
        id="I1-NC7",
        what="a successful recovery clears an unexpected failure",
        anchor=(
            "        if resolved && self.phase == RunPhase::RecoveryRequired {\n"
            "            self.become_candidate(now, Some(attempt));\n        }\n"
        ),
        replacement=(
            "        if resolved && self.phase == RunPhase::RecoveryRequired {\n"
            "            self.failures = FailureLog::new(self.config.failure_detail_limit);\n"
            "            self.cases_failed = 0;\n"
            "            self.become_candidate(now, Some(attempt));\n        }\n"
        ),
        test="h10_unexpected_failure_stays_failed_after_recovery",
        marker="[failure-retained]",
    ),
    # The six R1 controls restoring each rejected behavior.
    dict(
        id="R1-A1",
        what="a failure marks the verdict but leaves execution admission open",
        anchor="        self.shared.close(ClosureReason::Failed(class), now);\n",
        replacement="",
        test="r01_actual_failure_closes_admission_within_its_case",
        marker="[fail-stop]",
    ),
    dict(
        id="R1-A2",
        what="a failed case's cleanup permits a next case",
        anchor=(
            "    fn halted(&mut self, now: Tick) -> Option<Closure> {\n"
            "        self.observe_closure(now)\n    }\n"
        ),
        replacement=(
            "    fn halted(&mut self, now: Tick) -> Option<Closure> {\n"
            "        self.observe_closure(now)\n"
            "            .filter(|closure| !matches!(closure.reason, ClosureReason::Failed(_)))\n"
            "    }\n"
        ),
        test="r02_failed_case_cleanup_never_permits_a_next_case",
        marker="[no-next-case]",
    ),
    dict(
        id="R1-B",
        what="a recording failure permits normal custody closure",
        anchor="        if unresolved.is_empty() {\n            ShutdownDecision::Permitted\n",
        replacement=(
            "        if unresolved.is_empty() || self.ledger.failed.is_some() {\n"
            "            ShutdownDecision::Permitted\n"
        ),
        test="r03_failed_evidence_keeps_the_custody_open",
        marker="[evidence-close]",
    ),
    dict(
        id="R1-C1",
        what="a finalized verdict changes without matching terminal evidence",
        anchor="        if self.terminal.is_some() {\n            self.faults.push(Failure {\n",
        replacement="        if false {\n            self.faults.push(Failure {\n",
        test="r04_finalized_verdict_matches_acknowledged_terminal_evidence",
        marker="[finalized-verdict]",
    ),
    dict(
        id="R1-C2",
        what="late ownership bypasses its incident binding (held under the retired action)",
        anchor=(
            "            RecordKind::IncidentOpened {\n"
            "                incident,\n"
            "                action,\n"
            "                kind: declared,\n"
            "            },\n"
        ),
        replacement="            RecordKind::ActionFailed { action },\n",
        test="r05_late_owner_is_a_bound_incident_never_the_retired_action",
        marker="[late-incident]",
    ),
    dict(
        id="R1-D",
        what="detached output satisfies an ordinary clean pass",
        anchor="        if !declared {\n",
        replacement="        if false && !declared {\n",
        test="r06_detached_output_is_output_loss_never_a_clean_pass",
        marker="[output-lost]",
    ),
]

R2 = [
    dict(
        id="R2-A1",
        what="the commitment bypasses cancellation reconciliation (Candidate -> Passed restored)",
        anchor="        let closure = self.shared.commit(now);\n        self.note_cancellation(closure, now);\n",
        replacement="        self.shared.commit(now);\n",
        test="r07_cancellation_accepted_in_candidate_prevents_a_pass",
        marker="[cancel-before-commit]",
    ),
    dict(
        id="R2-A2",
        what="an observed cancellation of a candidate is not its failure until the commitment",
        anchor=(
            "        let closure = self.observe_closure(now);\n"
            "        self.note_cancellation(closure, now);\n        self.settle_idle(now);\n"
        ),
        replacement="        let closure = self.observe_closure(now);\n        self.settle_idle(now);\n",
        test="r07_cancellation_accepted_in_candidate_prevents_a_pass",
        marker="[cancel-observed]",
    ),
    dict(
        id="R2-A3",
        what="a cancellation after the commitment still closes admission and claims acceptance",
        anchor=(
            "        if let Some(commitment) = gate.committed {\n"
            "            return CancelReceipt::Late(commitment);\n        }\n"
        ),
        replacement="",
        test="r11_finalization_winning_makes_a_later_cancellation_late",
        marker="[late-cancel]",
    ),
    dict(
        id="R2-A4",
        what="the commitment decides against the execution owner's last observation, not the gate",
        anchor="        let closure = self.shared.commit(now);\n",
        replacement=(
            "        let closure = self.shared.commit(now).filter(|_| self.cancel_observed.is_some());\n"
        ),
        test="r10_cancellation_while_the_recorder_is_paused_precedes_the_commitment",
        marker="[cancel-in-flight]",
    ),
    dict(
        id="R2-B1",
        what="a failed first attempt is recognized only at the case's end (intervening admission)",
        anchor="        if unmet {\n",
        replacement="        if false && unmet {\n",
        test="r09_failed_first_attempt_of_a_retained_boundary_closes_admission_at_once",
        marker="[expectation-latched]",
    ),
    dict(
        id="R2-B2",
        what="the case's end records the latched expectation again",
        anchor="            Some(case) if !case.unmet => {\n",
        replacement="            Some(case) => {\n",
        test="r09_failed_first_attempt_of_a_retained_boundary_closes_admission_at_once",
        marker="[expectation-once]",
    ),
    dict(
        id="R2-C1",
        what="a shutdown request decides a completion candidate's outcome (cancels it)",
        anchor=(
            "        if self.phase == RunPhase::Running {\n            self.shared\n"
            "                .close(ClosureReason::Cancelled(CancelReason::Shutdown), now);\n"
        ),
        replacement=(
            "        if matches!(self.phase, RunPhase::Running | RunPhase::Candidate) {\n"
            "            self.shared\n"
            "                .close(ClosureReason::Cancelled(CancelReason::Shutdown), now);\n"
        ),
        test="r15_shutdown_request_is_no_second_finalization_rule",
        marker="[shutdown-candidate]",
    ),
]

PRIMITIVE = (
    "        let mut gate = lock(&self.gate);\n"
    "        let closure = gate.closure;\n"
    "        #[cfg(test)]\n"
    "        self.reach(CommitPoint::Decided);\n"
    "        gate.committed.get_or_insert(Commitment { at });\n"
    "        closure\n"
)

SPLIT = dict(
    id="R2A-SPLIT",
    what=(
        "the commitment primitive reads the deciding closure under one guard, releases it, "
        "and records the commitment under a second guard with that stale snapshot"
    ),
    anchor=PRIMITIVE,
    replacement=(
        "        let closure = lock(&self.gate).closure;\n"
        "        #[cfg(test)]\n"
        "        self.reach(CommitPoint::Decided);\n"
        "        lock(&self.gate).committed.get_or_insert(Commitment { at });\n"
        "        closure\n"
    ),
    test=DECIDED,
    marker="[commit-atomic]",
    require=[
        "gate.try_lock() -> Free",
        "cancel at the boundary -> Accepted(",
        "release missed: false",
        "terminal record Some((4, Passed))",
    ],
    forbid=["fixture failure", "release missed: true"],
)

for control in RETAINED + R2 + [SPLIT]:
    control["file"] = f"{CUSTODY}/core.rs"
    control["target"] = "phase2_custody_core"

DEBUG_RECORD = (
    "        let digest = {\n"
    "            use sha2::Digest as _;\n"
    "            let mut hasher = sha2::Sha256::new();\n"
    "            hasher.update(b\"nexus-phase2-custody-record\\0\");\n"
    "            hasher.update(id.generation.bytes());\n"
    "            hasher.update(id.seq.to_be_bytes());\n"
    "            hasher.update(at.0.to_be_bytes());\n"
    "            hasher.update(format!(\"{kind:?}\").as_bytes());\n"
    "            let digest: [u8; 32] = hasher.finalize().into();\n"
    "            digest\n"
    "        };\n"
)
DEBUG_REQUEST = (
    "        let digest = {\n"
    "            use sha2::Digest as _;\n"
    "            let mut hasher = sha2::Sha256::new();\n"
    "            hasher.update(b\"nexus-phase2-custody-request\\0\");\n"
    "            hasher.update(request.generation.bytes());\n"
    "            hasher.update(request.seq.to_be_bytes());\n"
    "            hasher.update(format!(\"{:?}\", request.op).as_bytes());\n"
    "            let digest: [u8; 32] = hasher.finalize().into();\n"
    "            digest\n"
    "        };\n"
)

CODEC = [
    dict(
        id="I2A-TRAIL",
        what="decoding ignores bytes after the frame",
        file=f"{CUSTODY}/codec.rs",
        anchor=(
            "        Ordering::Greater => {\n"
            "            return Err(CodecError::TrailingBytes {\n"
            "                extra: bytes.len().saturating_sub(frame_len),\n"
            "            })\n"
            "        }\n"
        ),
        replacement="        Ordering::Greater => {}\n",
        target="phase2_custody_codec",
        test="c04_decoding_takes_exactly_one_complete_frame",
        marker="[single-frame]",
    ),
    dict(
        id="I2A-ACTION-GENERATION",
        what="an embedded operation identity's generation is left out of the encoding and digest",
        file=f"{CUSTODY}/codec.rs",
        anchor="        self.generation(action.generation());\n        self.u64(action.seq());\n",
        replacement="        self.u64(action.seq());\n",
        target="phase2_custody_codec",
        test="c09_digests_bind_every_identity_instant_variant_and_field",
        marker="[identity-bound]",
        require=["the embedded action generation"],
    ),
    dict(
        id="I2A-RECORD-GENERATION",
        what="the record generation is left out of the encoding and digest",
        file=f"{CUSTODY}/codec.rs",
        anchor="    let mut out = Writer::new(Domain::Record);\n    out.generation(id.generation());\n",
        replacement="    let mut out = Writer::new(Domain::Record);\n",
        target="phase2_custody_codec",
        test="c09_digests_bind_every_identity_instant_variant_and_field",
        marker="[identity-bound]",
        require=["the record generation"],
    ),
    dict(
        id="I2A-REQUEST-FIELD",
        what="a request payload field (the retry epoch) is left out of the encoding and digest",
        file=f"{CUSTODY}/codec.rs",
        anchor="            out.u8(0x01);\n            out.u64(epoch);\n",
        replacement="            out.u8(0x01);\n            let _ = epoch;\n",
        target="phase2_custody_codec",
        test="c13_request_receipts_bind_the_full_payload",
        marker="[request-conflict]",
    ),
    dict(
        id="I2A-DEFAULT-TAG",
        what="an unknown slot-kind tag is accepted through a default",
        file=f"{CUSTODY}/codec.rs",
        anchor=(
            "        0x03 => Ok(SlotKind::Fixture),\n"
            "        tag => Err(CodecError::UnknownTag {\n"
            "            field: \"slot kind\",\n"
            "            tag,\n"
            "        }),\n"
        ),
        replacement="        0x03 => Ok(SlotKind::Fixture),\n        _ => Ok(SlotKind::Process),\n",
        target="phase2_custody_codec",
        test="c06_invalid_tags_booleans_and_presence_bytes_are_refused",
        marker="[unknown-tag]",
    ),
    dict(
        id="I2A-DIGEST-BYPASS",
        what="decoding does not check the digest trailer",
        file=f"{CUSTODY}/codec.rs",
        anchor="    if digest_of(covered) != digest {\n        return Err(CodecError::DigestMismatch);\n    }\n",
        replacement="",
        target="phase2_custody_codec",
        test="c07_digest_and_payload_mutations_are_detected",
        marker="[digest-bound]",
    ),
    dict(
        id="I2A-INTENT-BYPASS",
        what="encoding accepts an intent whose digest is not its fields'",
        file=f"{CUSTODY}/codec.rs",
        anchor=(
            "    if digest_of(&covered) != intent.digest {\n"
            "        return Err(CodecError::IntentDigestMismatch);\n    }\n"
        ),
        replacement="",
        target="phase2_custody_codec",
        test="c12_core_refuses_acknowledgements_for_altered_bytes",
        marker="[intent-digest]",
    ),
    dict(
        id="I2A-DEBUG-RECORD",
        what="the core's record digest path is restored to the Debug-text digest",
        file=f"{CUSTODY}/core.rs",
        anchor="        let digest = codec::record_digest(id, at, &kind);\n",
        replacement=DEBUG_RECORD,
        target="phase2_custody_codec",
        test="c11_core_issued_records_encode_decode_and_check",
        marker="[core-digest]",
    ),
]
ALTERED_REQUEST = (
    "        let mut digest = codec::request_digest(request);\n"
    "        digest[0] ^= 0x5a;\n"
)

NEW = [
    dict(
        id="NC-REQUEST-DEBUG",
        what="the core's real request digest path is restored to the Debug-text digest",
        file=f"{CUSTODY}/core.rs",
        anchor="        let digest = codec::request_digest(request);\n",
        replacement=DEBUG_REQUEST,
        target="phase2_custody_codec",
        test="c17_core_retains_the_canonical_request_digest_in_its_receipts",
        marker="[request-receipt]",
    ),
    dict(
        id="NC-REQUEST-ALTERED-DIGEST",
        what=(
            "the canonical request digest call stays, but its result is consistently "
            "altered before the core compares and stores it"
        ),
        file=f"{CUSTODY}/core.rs",
        anchor="        let digest = codec::request_digest(request);\n",
        replacement=ALTERED_REQUEST,
        target="phase2_custody_codec",
        test="c17_core_retains_the_canonical_request_digest_in_its_receipts",
        marker="[request-receipt]",
    ),
]

# Uncounted: what the older checks see under the two new mutations.
INFORMATIONAL = [
    (NEW[0], "c13_request_receipts_bind_the_full_payload", "pass"),
    (NEW[0], "c16_core_digest_paths_use_the_codec", "fail (source guard)"),
    (NEW[1], "c13_request_receipts_bind_the_full_payload", "pass"),
    (NEW[1], "c16_core_digest_paths_use_the_codec", "pass"),
]


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def state() -> dict:
    return {path: sha256(ROOT / path) for path in FILES}


def status() -> set:
    out = subprocess.run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout
    return {line for line in out.splitlines() if line}


def run_test(target: str, test: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["cargo", "test", "-p", "nexus-verifier-sandbox", "--locked",
         "--test", target, "--", test, "--exact"],
        cwd=ROOT, capture_output=True, text=True, timeout=1800,
    )


def message_of(output: str) -> str:
    lines = output.splitlines()
    at = next((i for i, line in enumerate(lines) if "panicked at" in line), None)
    if at is None:
        return ""
    tail = []
    for line in lines[at + 1:]:
        if line.startswith("note: run with") or line.startswith("failures:"):
            break
        tail.append(line)
    return "\n".join(tail).strip()


def run_one(control, test, originals, original_state):
    path = ROOT / control["file"]
    original = originals[control["file"]]
    text = original.decode()
    count = text.count(control["anchor"])
    if count != 1:
        raise SystemExit(f"{control['id']}: anchor occurs {count} times")
    mutated = text.replace(control["anchor"], control["replacement"], 1)
    if state() != original_state:
        raise SystemExit(f"{control['id']}: files changed before the control")
    try:
        path.write_bytes(mutated.encode())
        proc = run_test(control["target"], test)
    finally:
        path.write_bytes(original)
    restored = state() == original_state
    if status() != EXPECTED_STATUS:
        raise SystemExit(f"{control['id']}: unexpected worktree state after restoring: {status()}")
    output = proc.stdout + proc.stderr
    short = test.rsplit("::", 1)[-1]
    compiled = (
        "could not compile" not in output
        and "error[E" not in output
        and f"Running tests/{control['target']}.rs" in output
    )
    failed = (
        proc.returncode != 0
        and "test result: FAILED. 0 passed; 1 failed" in output
        and f"{short} ... FAILED" in output
    )
    passed = proc.returncode == 0 and "test result: ok. 1 passed" in output
    return output, compiled, failed, passed, restored, message_of(output), proc.returncode


def main() -> int:
    LOG.mkdir(parents=True, exist_ok=True)
    original_state = state()
    originals = {path: (ROOT / path).read_bytes() for path in FILES}
    if status() != EXPECTED_STATUS:
        raise SystemExit(f"unexpected worktree state before the controls: {status()}")
    counted = []
    for control in CODEC + RETAINED + R2 + [SPLIT] + NEW:
        output, compiled, failed, _, restored, message, code = run_one(
            control, control["test"], originals, original_state)
        (LOG / f"{control['id']}.log").write_text(output)
        marked = control["marker"] in message
        required = all(item in message for item in control.get("require", []))
        forbidden = any(item in message for item in control.get("forbid", []))
        result = dict(
            id=control["id"], what=control["what"], target=control["target"],
            test=control["test"], marker=control["marker"], compiled=compiled,
            failed_intended_test=failed, marker_in_assertion=marked,
            required_trace=required, forbidden_trace=forbidden,
            files_restored=restored, exit=code, assertion=message[:1500],
        )
        counted.append(result)
        print(json.dumps(result), flush=True)
        if not (compiled and failed and marked and required and not forbidden and restored):
            print(f"{control['id']}: control did not behave as required", file=sys.stderr)
            return 1
    informational = []
    for control, test, expect in INFORMATIONAL:
        output, compiled, failed, passed, restored, message, code = run_one(
            control, test, originals, original_state)
        label = f"{control['id']}-{test.split('_')[0]}"
        (LOG / f"{label}.info.log").write_text(output)
        result = dict(
            id=label, test=test, expect=expect, compiled=compiled, failed=failed,
            passed=passed, files_restored=restored, exit=code, assertion=message[:600],
        )
        informational.append(result)
        print(json.dumps(result), flush=True)
        if not (compiled and restored and (failed or passed)):
            print(f"{label}: informational run did not compile, run or restore", file=sys.stderr)
            return 1
    final_state = state()
    summary = dict(
        original=original_state,
        final=final_state,
        identical=final_state == original_state,
        counted=len(counted),
        counted_all_required=all(
            r["compiled"] and r["failed_intended_test"] and r["marker_in_assertion"]
            and r["required_trace"] and not r["forbidden_trace"] and r["files_restored"]
            for r in counted
        ),
        informational=len(informational),
    )
    print(json.dumps(summary), flush=True)
    (LOG / "summary.json").write_text(json.dumps(
        dict(summary=summary, counted=counted, informational=informational), indent=2))
    return 0 if summary["identical"] and summary["counted_all_required"] else 1


if __name__ == "__main__":
    sys.exit(main())
