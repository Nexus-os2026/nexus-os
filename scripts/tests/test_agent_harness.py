"""Synthetic fixtures only: no product mutations, Cargo builds or cloud calls."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("agent_harness", Path(__file__).resolve().parents[1] / "agent_harness.py")
h = importlib.util.module_from_spec(spec)
spec.loader.exec_module(h)


class ReceiptMappingTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="nexus-harness-test-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        (self.root / "stdout.txt").write_text("one native test result; no per-control stdout markers\n")
        (self.root / "stderr.txt").write_text("")
        self.ids = ["M131", "M132", "M160"]

    def receipt(self, rid, ids, rc=0):
        command = ["synthetic-fixture", "single-native-test"]
        return {"receipt_id": rid, "mission_id": "SYNTHETIC", "timestamp_start": "2026-01-01T12:00:00+00:00",
                "timestamp_end": "2026-01-01T12:00:01+00:00", "cwd": str(self.root),
                "candidate_sha": "a" * 40, "candidate_tree": "b" * 40, "command": command,
                "command_hash": h.command_hash(command), "exit_code": rc, "status": "EXITED",
                "stdout_ref": "stdout.txt", "stderr_ref": "stderr.txt",
                "raw_log_hash": {s: h.digest((self.root / (s + ".txt")).read_bytes()) for s in ("stdout", "stderr")},
                "environment": {"kind": "synthetic fixture; no actual execution"},
                "acceptance_criterion": "test coverage accounting", "control_ids": ids,
                "notes": "Synthetic input to the validator, not an execution claim."}

    def execution(self, rid="baseline", ids=None, kind="prerequisite", result="PASS", rc=0):
        ids = ids or self.ids
        e = {"command_execution_id": rid, "campaign_id": "single-campaign",
             "kind": kind, "result": result, "receipt": self.receipt(rid, ids, rc)}
        if kind == "mutation":
            e.update({"mutation_id": rid, "mutation_sha256": "c" * 64,
                      "mutated_control_ids": ids, "baseline_execution_id": "baseline",
                      "oracle_observed": True, "infrastructure_error": False,
                      "restoration": {"source": True, "execution": True}})
        return e

    def campaign(self, executions=None):
        return {"mission_id": "SYNTHETIC", "campaign_id": "single-campaign",
                "candidate_sha": "a" * 40, "candidate_tree": "b" * 40,
                "mission_status": "RUNNING", "required_controls": self.ids,
                "executions": executions if executions is not None else [self.execution()]}

    def valid_complete(self):
        return self.campaign([self.execution(), *[self.execution("mutation-" + i, [i], "mutation", "KILLED", 101) for i in self.ids]])

    def reject(self, data, message):
        with self.assertRaisesRegex(h.InvalidEvidence, message):
            h.validate_campaign(data, self.root)

    def test_shared_command_proves_three_baselines_without_markers(self):
        s = h.validate_campaign(self.campaign(), self.root)
        self.assertEqual(s["prerequisite_commands"], 1)
        self.assertEqual(s["prerequisite_logical_pass"], 3)
        self.assertEqual(s["mutation_commands"], 0)
        self.assertEqual(s["control_results"]["NOT_RUN"], 3)
        self.assertFalse(s["complete_single_campaign"])

    def test_117_prerequisites_cover_183_but_execute_zero_mutations(self):
        ids = [f"M{i:03}" for i in range(1, 184)]
        c = self.campaign([self.execution(f"pre-{i}", [ids[i]]) for i in range(116)] +
                          [self.execution("pre-116", ids[116:])])
        c["required_controls"] = ids
        s = h.validate_campaign(c, self.root)
        self.assertEqual((s["prerequisite_commands"], s["prerequisite_logical_pass"]), (117, 183))
        self.assertEqual(s["control_results"]["NOT_RUN"], 183)
        self.assertFalse(s["complete_single_campaign"])

    def test_baseline_cannot_claim_ready(self):
        c = self.campaign(); c["mission_status"] = "READY_FOR_REVIEW"
        self.reject(c, "complete single-campaign")

    def test_complete_single_campaign_requires_actual_kills(self):
        c = self.valid_complete(); c["mission_status"] = "READY_FOR_REVIEW"
        s = h.validate_campaign(c, self.root)
        self.assertTrue(s["complete_single_campaign"])
        self.assertEqual(s["control_results"]["KILLED"], 3)

    def test_one_mutation_can_map_multiple_actual_logical_properties(self):
        c = self.campaign([self.execution(), self.execution("one-mutant", self.ids, "mutation", "KILLED", 1)])
        s = h.validate_campaign(c, self.root)
        self.assertEqual(s["mutation_commands"], 1)
        self.assertEqual(s["control_results"]["KILLED"], 3)

    def test_shared_oracle_cannot_cover_unmutated_control(self):
        c = self.valid_complete(); c["executions"][1]["receipt"]["control_ids"] = self.ids
        self.reject(c, "mapped control was not mutated")

    def test_no_execution_is_explicitly_incomplete(self):
        s = h.validate_campaign(self.campaign([]), self.root)
        self.assertEqual(s["control_results"]["NOT_RUN"], 3)

    def test_survivor_is_not_complete(self):
        c = self.valid_complete(); e = c["executions"][1]
        e["result"] = "SURVIVED"; e["receipt"]["exit_code"] = 0
        s = h.validate_campaign(c, self.root)
        self.assertEqual(s["control_results"]["SURVIVED"], 1)
        self.assertFalse(s["complete_single_campaign"])

    def test_missing_baseline_rejected(self):
        c = self.valid_complete(); c["executions"].pop(0)
        self.reject(c, "passing mapped baseline")

    def test_failing_baseline_rejected(self):
        c = self.valid_complete(); e = c["executions"][0]
        e["result"] = "FAIL"; e["receipt"]["exit_code"] = 1
        self.reject(c, "passing mapped baseline")

    def test_invalid_and_harness_error_not_counted_as_kills(self):
        for status in ("INVALID", "HARNESS_ERROR"):
            c = self.valid_complete(); e = c["executions"][1]
            e["result"] = status; e["infrastructure_error"] = status == "HARNESS_ERROR"
            s = h.validate_campaign(c, self.root)
            self.assertFalse(s["complete_single_campaign"])
            self.assertEqual(s["control_results"][status], 1)

    def test_status_domains_not_interchangeable(self):
        for kind, result in [("prerequisite", "KILLED"), ("mutation", "PASS")]:
            c = self.campaign([self.execution(kind=kind, result=result)])
            self.reject(c, "invalid .* result")

    def test_mixed_historical_campaigns_rejected(self):
        c = self.valid_complete(); c["executions"][-1]["campaign_id"] = "isolated-rerun"
        self.reject(c, "cannot combine campaign")

    def test_other_candidate_or_tree_rejected(self):
        for key in ("candidate_sha", "candidate_tree"):
            c = self.valid_complete(); c["executions"][1]["receipt"][key] = "d" * 40
            self.reject(c, "candidate mismatch")

    def test_duplicate_execution_and_duplicate_coverage_rejected(self):
        c = self.valid_complete(); c["executions"].append(copy.deepcopy(c["executions"][1]))
        self.reject(c, "duplicate/missing execution")
        c = self.valid_complete(); c["executions"].append(self.execution("repeat", [self.ids[0]], "mutation", "KILLED", 1))
        self.reject(c, "duplicate mutation logical coverage")

    def test_duplicate_prerequisite_coverage_rejected(self):
        self.reject(self.campaign([self.execution(), self.execution("second")]), "duplicate prerequisite")

    def test_duplicate_or_unknown_inventory_controls_rejected(self):
        c = self.campaign(); c["required_controls"].append("M131")
        self.reject(c, "duplicate IDs")
        self.ids = ["M131", "M132", "M160"]
        c = self.campaign(); c["executions"][0]["receipt"]["control_ids"] = ["UNLISTED"]
        self.reject(c, "unknown control")

    def test_nonzero_exit_alone_does_not_prove_kill(self):
        c = self.valid_complete(); c["executions"][1]["oracle_observed"] = False
        self.reject(c, "semantic oracle")

    def test_infrastructure_error_never_killed(self):
        c = self.valid_complete(); c["executions"][1]["infrastructure_error"] = True
        self.reject(c, "infrastructure failure")

    def test_source_restore_alone_insufficient(self):
        c = self.valid_complete(); c["executions"][1]["restoration"]["execution"] = False
        self.reject(c, "source AND execution")

    def test_zero_exit_cannot_be_killed(self):
        c = self.valid_complete(); c["executions"][1]["receipt"]["exit_code"] = 0
        self.reject(c, "exit/result mismatch")

    def test_missing_mutation_identity_rejected(self):
        for key in ("mutation_id", "mutation_sha256"):
            c = self.valid_complete(); del c["executions"][1][key]
            self.reject(c, "mutation (case|definition)")

    def test_receipt_log_tampering_rejected(self):
        c = self.campaign(); (self.root / "stdout.txt").write_text("changed")
        self.reject(c, "stdout hash mismatch")

    def test_missing_command_and_command_tampering_rejected(self):
        c = self.campaign(); del c["executions"][0]["receipt"]["command"]
        self.reject(c, "receipt fields missing")
        c = self.campaign(); c["executions"][0]["receipt"]["command"] = ["different-command"]
        self.reject(c, "command hash mismatch")

    def test_receipt_not_run_has_no_fabricated_measurements(self):
        e = self.execution(result="NOT_RUN")
        e["receipt"].update(status="NOT_RUN", exit_code=None, timestamp_start=None,
                            timestamp_end=None, stdout_ref=None, stderr_ref=None, raw_log_hash={})
        s = h.validate_campaign(self.campaign([e]), self.root)
        self.assertEqual(s["prerequisite_commands"], 0)
        e["receipt"]["exit_code"] = 0
        self.reject(self.campaign([e]), "fabricated execution")

    def test_artifact_traversal_rejected_without_reading_outside(self):
        for ref in ("../secret", "/etc/passwd", "C:/secret", "..\\secret"):
            c = self.campaign(); c["executions"][0]["receipt"]["stdout_ref"] = ref
            self.reject(c, "unsafe artifact")

    def test_bad_timestamps_and_boolean_exit_rejected(self):
        c = self.campaign(); c["executions"][0]["receipt"]["timestamp_end"] = "2025-01-01T00:00:00+00:00"
        self.reject(c, "timestamp")
        c = self.campaign(); c["executions"][0]["receipt"]["exit_code"] = True
        self.reject(c, "exit code")

    def test_duplicate_and_empty_mission_fields_rejected(self):
        p = self.root / "MISSION.md"
        for text in ("## Mission ID\n\n", "## Mission ID\n\nx\n\n## Mission ID\n\ny\n"):
            p.write_text(text)
            with self.assertRaises(h.InvalidEvidence):
                h.fields(p, ["Mission ID"])


class IsolationTests(unittest.TestCase):
    """All Git writes are in a disposable synthetic repository, never Nexus refs."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="nexus-harness-isolation-test-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.env = {**os.environ, "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull}
        self.run_git("init", "--initial-branch=main")
        (self.repo / "tracked.txt").write_text("synthetic source\n")
        self.run_git("add", "tracked.txt")
        self.run_git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                     "-c", "commit.gpgsign=false", "commit", "-m", "synthetic baseline")
        self.evidence = self.root / "evidence"
        self.evidence.mkdir()
        (self.evidence / "artifact.txt").write_text("sealed synthetic evidence\n")
        self.before = {
            "refs": dict(line.split() for line in self.run_git(
                "for-each-ref", "--format=%(refname) %(objectname)").splitlines()),
            "candidate10_worktrees": [{"path": str(self.repo),
                "head": self.run_git("rev-parse", "HEAD"),
                "tree": self.run_git("rev-parse", "HEAD^{tree}"),
                "status": "", "branch": "main"}],
            "candidate10_evidence": {"path": str(self.evidence), **h.tree_hash(self.evidence)},
        }
        self.run_git("branch", "synthetic-mission")

    def run_git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.repo), *args],
                                       env=self.env, stderr=subprocess.PIPE, text=True).strip()

    def test_unchanged_refs_worktree_and_artifact_pass(self):
        result = h.verify_isolation(self.repo, self.before, "synthetic-mission")
        self.assertEqual(result["existing_refs_unchanged"], 1)

    def test_unexpected_ref_creation_rejected(self):
        self.run_git("branch", "unapproved")
        with self.assertRaisesRegex(h.InvalidEvidence, "unauthorized ref"):
            h.verify_isolation(self.repo, self.before, "synthetic-mission")

    def test_source_change_rejected(self):
        (self.repo / "tracked.txt").write_text("changed\n")
        with self.assertRaisesRegex(h.InvalidEvidence, "worktree changed"):
            h.verify_isolation(self.repo, self.before, "synthetic-mission")

    def test_evidence_change_rejected(self):
        (self.evidence / "artifact.txt").write_text("changed\n")
        with self.assertRaisesRegex(h.InvalidEvidence, "evidence changed"):
            h.verify_isolation(self.repo, self.before, "synthetic-mission")

    def test_protected_ref_advance_rejected(self):
        (self.repo / "tracked.txt").write_text("different commit\n")
        self.run_git("add", "tracked.txt")
        self.run_git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                     "-c", "commit.gpgsign=false", "commit", "-m", "synthetic forbidden advance")
        with self.assertRaisesRegex(h.InvalidEvidence, "pre-existing refs changed"):
            h.verify_isolation(self.repo, self.before, "synthetic-mission")


if __name__ == "__main__":
    unittest.main()
