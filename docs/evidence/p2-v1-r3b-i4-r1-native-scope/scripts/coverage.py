#!/usr/bin/env python3
"""P2-V1-R3B-I4-R1 test/control coverage matrix, generated from the candidate
runs: the mission's required regressions (I4R1-01..I4R1-24, section 15), the
accepted I4 regressions (I4-01..I4-30, kept green), and the completion
standard (section 24), each mapped to the unit tests that cover it (with
their result in the validation's unit-test run), the behavioural controls
whose intended test it is (with their results), and the API/type guards,
source guards, desktop guards and traces that carry it.

Usage: coverage.py <candidate runs directory> <scripts directory> <output .md> <output .json>
"""
import importlib.util
import json
import pathlib
import re
import sys

sys.dont_write_bytecode = True

E = "execution::tests::"
S = "scope::tests::"
CLASSIFICATION = [
    f"{E}p2d_only_a_clean_passing_exit_is_passed",
    f"{E}p2d_a_lost_verifier_or_lost_counters_are_never_a_result",
    f"{E}p2d_limits_and_backend_endings_are_never_passed",
    f"{E}p2d_unconfirmed_cleanup_overrides_every_other_class",
    f"{E}p2d_setup_failures_distinguish_an_unavailable_host",
    f"{E}p2r1_an_interrupted_execution_is_never_passed",
]
PANIC_MATRIX = f"{E}i4r1_x_a_panic_at_any_ownership_point_leaves_confirmation_or_one_owner"
NORMAL_FORM = f"{E}i4r1_x_a_membership_path_not_in_normal_form_never_locates_a_candidate"
ONLY_RUN = f"{S}i4r1_x_a_normal_build_starts_a_scope_only_within_run"

R1 = [
    ("I4R1-01", "a panic after an uncertain Start with no candidate keeps ownership",
     [f"{E}i4r1_01_a_panic_after_an_uncertain_start_without_a_candidate_keeps_its_owner", PANIC_MATRIX,
      f"{E}i4_25_a_panic_after_the_start_request_keeps_its_owner"]),
    ("I4R1-02", "a direct/harness failure cannot return Pending without the Helper owner",
     [f"{E}i4r1_02_a_direct_placement_failure_returns_one_owner_of_the_operation_and_its_helper",
      f"{S}i4_03_a_start_whose_effect_cannot_be_confirmed_gone_stays_owned_with_its_helper"]),
    ("I4R1-03", "dropping/ending the owned failure is defense only, never confirmation",
     [f"{E}i4r1_03_dropping_a_direct_owner_is_defense_only_and_never_a_confirmation",
      f"{E}i4_24_dropping_an_unresolved_operation_is_best_effort_and_confirms_nothing"]),
    ("I4R1-04", "a normal build cannot name or use ScopeManager::connect_at",
     [f"{S}i4r1_04_a_normal_build_constructs_a_manager_only_by_connect"]),
    ("I4R1-05", "a normal build cannot construct or use PendingScope or StartFailed",
     [f"{S}i4r1_05_a_normal_build_holds_no_scope_operation_apart_from_its_helper", ONLY_RUN]),
    ("I4R1-06", "a kernel membership path in another slice with the same basename is not Proven",
     [f"{E}i4r1_06_a_cgroup_of_the_unit_s_name_in_another_place_is_never_proven", NORMAL_FORM]),
    ("I4R1-07", "a manager ControlGroup mismatch refuses",
     [f"{E}i4r1_07_a_control_group_that_is_not_exactly_the_kernel_s_is_refused"]),
    ("I4R1-08", "a manager ControlGroup unavailable or uncertain refuses",
     [f"{E}i4r1_08_an_unavailable_or_uncertain_control_group_is_refused"]),
    ("I4R1-09", "a manager Id mismatch refuses",
     [f"{E}i4r1_09_a_unit_id_that_is_not_exactly_the_generated_name_is_refused"]),
    ("I4R1-10", "exact Id + ControlGroup + descriptor + limits + properties proves",
     [f"{E}i4r1_10_the_exact_binding_of_the_manager_s_unit_to_the_kernel_s_cgroup_proves",
      f"{S}i4_01_a_confirmed_start_with_every_proof_is_proven"]),
    ("I4R1-11", "uncertain Start, no candidate, GetUnit NoSuchUnit: stays Pending",
     [f"{E}i4r1_11_an_uncertain_start_without_a_candidate_is_not_released_by_no_such_unit",
      f"{S}i4_02_a_start_without_effect_whose_reply_is_lost_is_never_proven_absent"]),
    ("I4R1-12", "uncertain Start, no candidate, StopUnit success reply: stays Pending",
     [f"{E}i4r1_12_an_uncertain_start_without_a_candidate_is_not_released_by_a_stop_reply"]),
    ("I4R1-13", "uncertain Start, no candidate, Stop timeout: stays Pending",
     [f"{E}i4r1_13_an_uncertain_start_without_a_candidate_is_not_released_by_a_timed_out_stop"]),
    ("I4R1-14", "an uncertain Start that later places the Helper: candidate acquired, may be Proven",
     [f"{E}i4r1_14_an_uncertain_start_that_places_the_helper_is_acquired_and_may_be_proven",
      f"{S}i4_03_a_start_with_effect_whose_reply_is_lost_is_discovered_and_proven"]),
    ("I4R1-15", "a UnitExists collision does not stop the foreign unit",
     [f"{E}i4r1_15_a_collision_never_stops_or_claims_the_foreign_unit",
      f"{E}i4_17_a_name_collision_fails_closed_and_never_touches_the_other_unit"]),
    ("I4R1-16", "a collision cannot become Proven by the foreign cgroup's basename",
     [f"{E}i4r1_16_a_collision_is_never_proven_by_the_foreign_cgroup_s_name"]),
    ("I4R1-17", "no launch while any identity/ControlGroup proof is unresolved",
     [f"{E}i4r1_17_no_launch_while_the_unit_binding_is_unresolved",
      f"{E}i4_18_no_launch_message_reaches_the_helper_before_the_scope_is_proven"]),
    ("I4R1-18", "the execution's Pending/Proven lifecycle stays green",
     [f"{E}i4r1_18_a_proven_execution_s_lifecycle_is_unchanged",
      f"{E}i4_19_finalization_ends_a_proven_scope_as_before",
      f"{E}i4_20_finalization_settles_a_pending_operation",
      f"{E}i4_28_the_helper_and_the_proven_scope_precede_any_launch",
      f"{E}i4_29_the_classification_of_settled_executions_is_unchanged"]),
    ("I4R1-19", "RetainedBoundary retry after the ScopeManager is dropped stays green",
     [f"{E}i4r1_19_a_binding_failure_s_candidate_is_retried_without_the_scope_manager",
      f"{E}i4_23_a_retained_boundary_is_retried_without_the_scope_manager",
      f"{S}i4_owners_are_self_contained_values"]),
    ("I4R1-20", "helper serial allocation refuses instead of wrapping",
     [f"{E}i4r1_20_helper_identities_are_refused_instead_of_wrapping"]),
    ("I4R1-21", "no child is spawned when helper identities are exhausted",
     [f"{E}i4r1_21_no_helper_is_spawned_once_identities_are_exhausted"]),
    ("I4R1-22", "the desktop reaches the sandbox only through execution::run",
     []),  # the desktop's own guards: see DESKTOP below
    ("I4R1-23", "the live harness's direct-scope coverage is equivalent or stronger",
     [f"{E}i4r1_23_the_harness_owner_lets_the_helper_hold_its_scope_until_released"]),
    ("I4R1-24", "no real user-manager contact in ordinary unit tests",
     [f"{E}i4r1_24_unit_tests_never_construct_the_real_manager_or_kernel"]),
]
DESKTOP = {
    "I4R1-22": ["phase2_tests::p2_g_01_the_desktop_reaches_project_code_only_through_the_verifier_sandbox",
                "phase2_tests::p2_g_06_production_cannot_construct_a_helper_from_an_arbitrary_path"],
    "I4R1-23": ["phase2_tests::p2_g_09_cleanup_is_observed_only_through_the_checked_observations"],
}
I4 = [
    ("I4-01", "Start confirmed, every proof succeeds: Proven",
     [f"{S}i4_01_a_confirmed_start_with_every_proof_is_proven",
      f"{E}i4_18_no_launch_message_reaches_the_helper_before_the_scope_is_proven",
      f"{E}i4_19_finalization_ends_a_proven_scope_as_before"]),
    ("I4-02", "no effect, reply lost (I4-R1: never proven absent; retained)",
     [f"{S}i4_02_a_start_without_effect_whose_reply_is_lost_is_never_proven_absent",
      f"{E}i4_29_the_classification_of_settled_executions_is_unchanged"]),
    ("I4-03", "effect, reply lost: discovered and proven, or retained with its helper",
     [f"{S}i4_03_a_start_with_effect_whose_reply_is_lost_is_discovered_and_proven",
      f"{S}i4_03_a_start_whose_effect_cannot_be_confirmed_gone_stays_owned_with_its_helper"]),
    ("I4-04", "placed, property proof times out: no launch, candidate retained",
     [f"{E}i4_04_a_property_timeout_after_placement_never_launches_and_keeps_the_candidate"]),
    ("I4-05", "unit created, helper never enters: cleanup confirmed before the failure",
     [f"{E}i4_05_a_unit_the_helper_never_entered_is_confirmed_gone_before_the_failure"]),
    ("I4-06", "same, StopUnit outcome uncertain: retained",
     [f"{E}i4_06_a_failed_stop_of_a_unit_the_helper_never_entered_retains_the_operation"]),
    ("I4-07", "StopUnit success, cgroup populated: not confirmed",
     [f"{E}i4_07_a_delivered_stop_reply_with_the_scope_still_populated_confirms_nothing"]),
    ("I4-08", "StopUnit timeout, target gone: may confirm",
     [f"{E}i4_08_a_timed_out_stop_whose_target_is_gone_may_confirm"]),
    ("I4-09", "StopUnit timeout, target remains: retained",
     [f"{E}i4_09_a_timed_out_stop_whose_target_remains_retains_the_operation"]),
    ("I4-10", "query uncertain after the descriptor: descriptor retained",
     [f"{E}i4_10_an_uncertain_query_after_the_candidate_keeps_its_descriptor"]),
    ("I4-11", "manager absent, helper in the candidate: refused, retained",
     [f"{E}i4_11_manager_absence_with_the_helper_in_the_candidate_is_refused_and_retained"]),
    ("I4-12", "limits mismatch: never launch",
     [f"{E}i4_12_mismatched_limits_never_launch",
      f"{S}i4_a_failed_placement_settles_with_its_helper_and_reports_the_proof_failure"]),
    ("I4-13", "RuntimeMaxUSec mismatch: never launch",
     [f"{E}i4_13_a_runtime_backstop_mismatch_never_launches"]),
    ("I4-14", "OOMPolicy mismatch: never launch",
     [f"{E}i4_14_an_out_of_memory_policy_mismatch_never_launches"]),
    ("I4-15", "membership mismatch: never launch",
     [f"{E}i4_15_a_membership_mismatch_never_launches"]),
    ("I4-16", "never Proven by the unit name",
     [f"{E}i4_16_a_pending_operation_is_never_proven_by_its_unit_name"]),
    ("I4-17", "a name collision fails closed",
     [f"{E}i4_17_a_name_collision_fails_closed_and_never_touches_the_other_unit"]),
    ("I4-18", "no launch before the complete proof",
     [f"{E}i4_18_no_launch_message_reaches_the_helper_before_the_scope_is_proven"]),
    ("I4-19", "finalization of a Proven scope as before",
     [f"{E}i4_19_finalization_ends_a_proven_scope_as_before"]),
    ("I4-20", "finalization handles Pending",
     [f"{E}i4_20_finalization_settles_a_pending_operation"]),
    ("I4-21", "unconfirmed: CleanupFailed",
     [f"{E}i4_21_an_unconfirmed_operation_is_a_cleanup_failure_never_an_unavailable_sandbox"]),
    ("I4-22", "retry resolves Pending",
     [f"{E}i4_22_a_retry_resolves_a_pending_operation"]),
    ("I4-23", "retry after the ScopeManager is dropped",
     [f"{E}i4_23_a_retained_boundary_is_retried_without_the_scope_manager"]),
    ("I4-24", "drop of an unresolved operation: no confirmation",
     [f"{E}i4_24_dropping_an_unresolved_operation_is_best_effort_and_confirms_nothing"]),
    ("I4-25", "panic after Start retains ownership",
     [f"{E}i4_25_a_panic_after_the_start_request_keeps_its_owner"]),
    ("I4-26", "panic after the candidate retains it",
     [f"{E}i4_26_a_panic_after_the_candidate_is_retained_keeps_it"]),
    ("I4-27", "panic while stopping or reconciling retains the operation",
     [f"{E}i4_27_a_panic_while_stopping_or_reconciling_retains_the_operation"]),
    ("I4-28", "helper and proven scope precede any launch",
     [f"{E}i4_28_the_helper_and_the_proven_scope_precede_any_launch"]),
    ("I4-29", "classification unchanged",
     [f"{E}i4_29_the_classification_of_settled_executions_is_unchanged"] + CLASSIFICATION),
    ("I4-30", "no PID-only, name-only or path-only cleanup",
     [f"{E}i4_30_the_helper_s_death_is_never_the_operation_s_cleanup",
      f"{E}i4_30_no_cgroup_of_the_unit_s_name_is_ever_taken_by_its_path",
      f"{S}i4_a_pending_operation_is_bound_to_its_own_helper"]),
]
# Section 24: (item, requirements, behavioural controls, harness-build API
# guards, normal-build API guards, source guards, other evidence).
COMPLETION = [
    ("1. production direct route is only ScopeManager::connect + execution::run",
     ["I4R1-04", "I4R1-05", "I4R1-22"], ["NC-I4R1-PUBLIC-START", "NC-I4R1-PUBLIC-CONNECT-AT"],
     ["P-I4R1-START-REMOVED"], ["N-I4R1-DIRECT-START", "N-I4R1-CONNECT-AT", "N-I4R1-PLACE"],
     ["SG-I4R1-NORMAL-SURFACE"], ["desktop"]),
    ("2. no normal public caller-selected manager socket",
     ["I4R1-04"], ["NC-I4R1-PUBLIC-CONNECT-AT"], [], ["N-I4R1-CONNECT-AT"],
     ["SG-I4R1-NORMAL-SURFACE", "SG-I4-BUS"], ["desktop"]),
    ("3. no safe public PendingScope lifecycle separated from Helper ownership",
     ["I4R1-02", "I4R1-05"], ["NC-I4R1-SPLIT-OWNER", "NC-I4R1-HARNESS-WEAK-OWNER"],
     ["P-I4-PENDING-NEW", "P-I4-PENDING-STATE", "P-I4-PENDING-RECONCILE", "P-I4-PREPARE",
      "P-I4R1-START-FAILED-REMOVED", "P-I4R1-SCOPED-TAKE-HELPER"],
     ["N-I4R1-PENDING-SCOPE", "N-I4R1-START-FAILED"], ["SG-I4R1-NORMAL-SURFACE"], []),
    ("4. no panic crosses the final owner of an uncertain remote effect",
     ["I4R1-01"], ["NC-I4R1-PANIC-DROPS-PENDING", "NC-I4-PANIC-OWNER-LOSS", "NC-I4R1-X-ACCEPTED-EARLY"],
     [], [], ["SG-I4R1-NO-RESUME-UNWIND"], []),
    ("5. harness direct ownership owns Helper + boundary together",
     ["I4R1-02", "I4R1-03", "I4R1-23"], ["NC-I4R1-SPLIT-OWNER", "NC-I4R1-HARNESS-WEAK-OWNER"],
     ["P-I4R1-SCOPED-FORGE", "P-I4R1-SCOPED-TAKE-HELPER", "P-I4R1-SCOPED-TAKE-SCOPE",
      "P-I4R1-SCOPED-CLONE", "P-I4R1-PLACEMENT-CLONE", "P-I4R1-SCOPED-DESERIALIZE"],
     ["N-I4R1-PLACE", "N-I4R1-SCOPED-HELPER", "N-I4R1-PLACEMENT-FAILED"],
     ["SG-I4-DESKTOP-REPLICA"], ["live-build"]),
    ("6. Pending -> Proven includes the exact ControlGroup <-> kernel membership binding",
     ["I4R1-06", "I4R1-07", "I4R1-08", "I4R1-10"],
     ["NC-I4R1-NO-CONTROLGROUP", "NC-I4R1-CONTROLGROUP-BASENAME", "NC-I4R1-CONTROLGROUP-MISMATCH",
      "NC-I4R1-X-CONTROLGROUP-UNCERTAIN", "NC-I4R1-X-MEMBERSHIP-NOT-NORMAL"], [], [],
     ["SG-I4R1-BINDING"], []),
    ("7. manager unit identity is exact",
     ["I4R1-09", "I4R1-10"], ["NC-I4R1-ID-MISMATCH"], [], [], ["SG-I4R1-BINDING"], []),
    ("8. the same basename in another cgroup is refused",
     ["I4R1-06", "I4R1-16"], ["NC-I4R1-CONTROLGROUP-BASENAME"], [], [], ["SG-I4R1-BINDING"], []),
    ("9. an uncertain Start with no candidate is not confirmed by GetUnit absence",
     ["I4R1-11"], ["NC-I4R1-ABSENCE-GETUNIT", "NC-I4R1-X-ACCEPTED-EARLY"], [], [],
     ["SG-I4R1-UNCERTAIN"], []),
    ("10. a StopUnit reply does not confirm that case",
     ["I4R1-12", "I4R1-13"], ["NC-I4R1-STOP-REPLY-ABSENCE", "NC-I4-STOP-OK-CONFIRMS",
                              "NC-I4-STOP-TIMEOUT-DROPS"], [], [], [], []),
    ("11. the Helper stays unreaped while that operation is unresolved",
     ["I4R1-13", "I4R1-03"], ["NC-I4R1-REAP-UNRESOLVED", "NC-I4-X-DROP-REAPS"], [], [], [], []),
    ("12. the candidate descriptor is retained before fallible later proofs",
     ["I4R1-14", "I4R1-19"], ["NC-I4-NO-EARLY-DIR", "NC-I4-PROPERTY-ERROR-DROPS"],
     ["P-I4-PENDING-CANDIDATE"], [], [], []),
    ("13. the existing proven-scope finalization stays green",
     ["I4R1-18"], [], [], [], [], []),
    ("14. RetainedBoundary retry stays self-contained after the ScopeManager is dropped",
     ["I4R1-19"], ["NC-I4-RETRY-NEEDS-MANAGER-BORROW"], [], [], ["SG-I4-NO-BACKGROUND"], []),
    ("15. the helper identity cannot wrap or be reused",
     ["I4R1-20"], ["NC-I4R1-SERIAL-WRAP"], ["P-I4-HELPER-SERIAL"], [], ["SG-I4R1-IDENTITY"], []),
    ("16. identity exhaustion prevents the spawn",
     ["I4R1-21"], ["NC-I4R1-SPAWN-BEFORE-IDENTITY"], [], [], ["SG-I4R1-IDENTITY"], []),
    ("17. no launch before all new proofs",
     ["I4R1-17"], ["NC-I4-LAUNCH-PENDING", "NC-I4R1-NO-CONTROLGROUP"], ["P-I4-BOUNDARY-TYPE"], [],
     ["SG-I4R1-BINDING"], []),
    ("18. no live user-manager contact in ordinary tests",
     ["I4R1-24"], [], [], [], ["SG-I4-TEST-SEAMS", "SG-I4-BUS", "SG-I4-DESTINATION"], ["no-live"]),
    ("19. the old accepted suites stay green",
     ["I4R1-18"], [], [], [], [], ["r3-store", "i2r1", "r3-api", "i4-controls", "i4-guards"]),
]


def load(path):
    spec = importlib.util.spec_from_file_location(path.stem, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main():
    runs, scripts, out_md, out_json = (pathlib.Path(a) for a in sys.argv[1:5])
    lib = (runs / "validation" / "lib-tests.txt").read_text()
    results = dict(re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored)$", lib, re.M))
    listed = set(re.findall(r"^(\S+): test$",
                            (runs / "validation" / "lib-test-list.txt").read_text(), re.M))
    desktop_text = (runs / "desktop-callers.out").read_text()
    desktop = dict(re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored)$", desktop_text, re.M))
    controls = {c["id"]: c for c in json.loads(
        (runs / "controls" / "scope" / "summary.json").read_text())["counted"]}
    guards = {g["id"]: g for g in json.loads(
        (runs / "controls" / "api-guards" / "summary.json").read_text())["guards"]}
    normal = {g["id"]: g for g in json.loads(
        (runs / "controls" / "normal-api-probes" / "summary.json").read_text())["guards"]}
    sources = json.loads((runs / "controls" / "source-guards.json").read_text())["guards"]
    no_live = (runs / "no-live" / "verdict.txt").read_text().strip().endswith("RESULT: PASS")
    commands = (runs / "validation" / "commands.txt").read_text()
    live_build = re.search(r"^live-harness-build-only exit=0 ", commands, re.M) is not None
    scope_summary = json.loads((runs / "controls" / "scope" / "summary.json").read_text())["summary"]
    guard_summary = json.loads((runs / "controls" / "api-guards" / "summary.json").read_text())["summary"]
    other = {
        "desktop": all(desktop.get(t) == "ok" for t in DESKTOP["I4R1-22"]),
        "live-build": live_build,
        "no-live": no_live,
        "r3-store": json.loads((runs / "controls" / "r3-store" / "summary.json").read_text())
        ["summary"].get("all_required") is True,
        "i2r1": json.loads((runs / "controls" / "i2r1-rerun" / "summary.json").read_text())
        ["summary"].get("counted_all_required") is True,
        "r3-api": all(p.get("ok") for p in json.loads(
            (runs / "controls" / "r3-api-probes" / "api_probes.json").read_text())),
        "i4-controls": all(c["ok"] for c in controls.values() if c["category"].startswith("i4-")),
        "i4-guards": all(g["ok"] for g in guards.values() if g.get("family") == "i4"),
    }
    definitions = load(scripts / "scope_controls.py")
    i4_defs, _ = definitions.i4_controls(scripts.parent.parent.parent.parent)
    by_test = {}
    for c in i4_defs + definitions.I4R1_REQUIRED + definitions.I4R1_ADDITIONAL:
        by_test.setdefault(c["test"], []).append(c["id"])
    missing = []

    def rows_of(requirements):
        rows = []
        for rid, what, tests in requirements:
            for test in tests:
                if test not in listed:
                    missing.append(test)
            row = dict(
                id=rid, what=what,
                tests=[dict(test=t, result=results.get(t, "MISSING")) for t in tests],
                desktop=[dict(test=t, result=desktop.get(t, "MISSING")) for t in DESKTOP.get(rid, [])],
                controls=[dict(id=cid, ok=controls.get(cid, {}).get("ok"))
                          for t in tests for cid in by_test.get(t, [])],
            )
            entries = row["tests"] + row["desktop"]
            row["covered"] = bool(entries) and all(e["result"] == "ok" for e in entries)
            rows.append(row)
        return rows

    r1_rows, i4_rows = rows_of(R1), rows_of(I4)
    req = {r["id"]: r for r in r1_rows + i4_rows}
    completion = []
    for what, rids, cids, gids, nids, sids, others in COMPLETION:
        item = dict(
            standard=what,
            requirements={rid: req[rid]["covered"] for rid in rids},
            controls={cid: controls.get(cid, {}).get("ok") for cid in cids},
            api_guards={gid: guards.get(gid, {}).get("ok") for gid in gids},
            normal_guards={nid: normal.get(nid, {}).get("ok") for nid in nids},
            source_guards={sid: sources.get(sid, {}).get("ok") for sid in sids},
            other={o: other[o] for o in others},
        )
        values = [v for key in ("requirements", "controls", "api_guards", "normal_guards",
                                "source_guards", "other") for v in item[key].values()]
        item["met"] = bool(values) and all(v is True for v in values)
        completion.append(item)
    mapped = {c["id"] for r in r1_rows + i4_rows for c in r["controls"]}
    unmapped_controls = sorted(set(controls) - mapped)

    def table(rows):
        lines = ["| Requirement | Tests (result) | Behavioural controls on these tests | Covered |",
                 "|---|---|---|---|"]
        for r in rows:
            entries = r["tests"] + r["desktop"]
            tests = "<br>".join(f"`{t['test']}` ({t['result']})" for t in entries)
            ctl = "<br>".join(f"`{c['id']}` ({'ok' if c['ok'] else 'NOT OK'})"
                              for c in r["controls"]) or "—"
            lines.append(f"| {r['id']}: {r['what']} | {tests} | {ctl} | "
                         f"{'yes' if r['covered'] else 'NO'} |")
        return lines

    fmt = lambda d: "<br>".join(f"{k} ({'ok' if v else 'NOT OK'})" for k, v in d.items()) or "—"
    lines = ["# P2-V1-R3B-I4-R1 test / control coverage", "",
             "Generated by `scripts/coverage.py` from the candidate runs (`validation/candidate/`, "
             "`controls/`, `validation/desktop-callers.txt`). A test's result is its line in the "
             "validation's unit-test run (desktop guards: the desktop's own run); a control's result "
             "is its behavioural-control record; a guard's, its guard record.", "",
             "## I4-R1 required regressions (mission section 15)", ""] + table(r1_rows)
    lines += ["", "## The accepted I4 regressions, kept green", ""] + table(i4_rows)
    lines += ["", "## Completion standard (mission section 24, items 1-19; item 20 is the "
              "publication itself, read back in the report)", "",
              "| Standard | Requirements | Behavioural controls | API/type guards (harness build) | "
              "API guards (normal build) | Source guards | Other evidence | Met |",
              "|---|---|---|---|---|---|---|---|"]
    for item in completion:
        lines.append(f"| {item['standard']} | {fmt(item['requirements'])} | {fmt(item['controls'])} | "
                     f"{fmt(item['api_guards'])} | {fmt(item['normal_guards'])} | "
                     f"{fmt(item['source_guards'])} | {fmt(item['other'])} | "
                     f"{'yes' if item['met'] else 'NO'} |")
    lines += ["", "Other evidence: `desktop` the desktop's Phase Two guards p2_g_01 and p2_g_06 "
              "(`validation/desktop-callers.txt`); `live-build` the live harness built with "
              "`--no-run`; `no-live` the traced unit-test run (`validation/no-live/`); `r3-store`, "
              "`i2r1`, `r3-api` the accepted suites' reruns (`controls/`); `i4-controls` the 21 "
              "accepted I4 behavioural controls; `i4-guards` the 19 accepted I4 API/type guards.", "",
              f"Behavioural controls: {scope_summary['counted']} counted ({scope_summary['by_category']}); "
              f"harness-build API/type guards: {guard_summary['guards']}; normal-build API guards: "
              f"{len(normal)}; source guards: {len(sources)}.",
              f"Unit tests listed: {len(listed)}; results recorded: {len(results)}; "
              f"tests named above but not listed: {missing or 'none'}.",
              f"Behavioural controls not mapped to a regression: {unmapped_controls or 'none'}."]
    out_md.write_text("\n".join(lines) + "\n")
    out_json.write_text(json.dumps(dict(i4r1=r1_rows, i4=i4_rows, completion=completion,
                                        other=other, missing_tests=missing,
                                        unmapped_controls=unmapped_controls), indent=2) + "\n")
    ok = (not missing and all(r["covered"] for r in r1_rows + i4_rows)
          and all(i["met"] for i in completion))
    print(json.dumps(dict(i4r1=len(r1_rows), i4r1_covered=sum(r["covered"] for r in r1_rows),
                          i4=len(i4_rows), i4_covered=sum(r["covered"] for r in i4_rows),
                          completion_met=sum(i["met"] for i in completion), completion=len(completion),
                          missing_tests=missing, unmapped_controls=unmapped_controls, ok=ok)))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
