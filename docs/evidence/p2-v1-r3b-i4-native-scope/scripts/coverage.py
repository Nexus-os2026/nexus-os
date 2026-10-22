#!/usr/bin/env python3
"""P2-V1-R3B-I4 test/control coverage matrix, generated from the candidate
runs: the mission's required regressions (I4-01..I4-30) and its completion
standard (section 25), each mapped to the unit tests that cover it (with
their result in the validation's unit-test run), the behavioural controls
whose intended test it is (with their results), and the API/type and
source guards that carry it.

Usage: coverage.py <candidate runs directory> <scripts directory> <output .md> <output .json>
"""
import importlib.util
import json
import pathlib
import re
import sys

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
REQUIREMENTS = [
    ("I4-01", "Start confirmed, helper placed, every proof succeeds: Proven Scope",
     [f"{S}i4_01_a_confirmed_start_with_every_proof_is_proven",
      f"{E}i4_18_no_launch_message_reaches_the_helper_before_the_scope_is_proven",
      f"{E}i4_19_finalization_ends_a_proven_scope_as_before"]),
    ("I4-02", "no effect, reply lost: absence proven, nothing retained",
     [f"{S}i4_02_a_start_without_effect_whose_reply_is_lost_is_proven_absent",
      f"{E}i4_29_the_classification_of_settled_executions_is_unchanged"]),
    ("I4-03", "effect, reply lost: discovered and proven, or retained",
     [f"{S}i4_03_a_start_with_effect_whose_reply_is_lost_is_discovered_and_proven",
      f"{S}i4_03_a_start_whose_effect_cannot_be_confirmed_gone_is_returned_owned"]),
    ("I4-04", "placed, property proof times out: no launch, candidate retained",
     [f"{E}i4_04_a_property_timeout_after_placement_never_launches_and_keeps_the_candidate"]),
    ("I4-05", "unit created, helper never enters: cleanup confirmed before the failure",
     [f"{E}i4_05_a_unit_the_helper_never_entered_is_confirmed_gone_before_the_failure"]),
    ("I4-06", "same, StopUnit outcome uncertain: PendingScope retained",
     [f"{E}i4_06_a_failed_stop_of_a_unit_the_helper_never_entered_retains_the_operation"]),
    ("I4-07", "StopUnit success, cgroup populated: not confirmed",
     [f"{E}i4_07_a_delivered_stop_reply_with_the_scope_still_populated_confirms_nothing"]),
    ("I4-08", "StopUnit timeout, cgroup empty or removed: may confirm",
     [f"{E}i4_08_a_timed_out_stop_whose_target_is_gone_may_confirm"]),
    ("I4-09", "StopUnit timeout, target remains: retained",
     [f"{E}i4_09_a_timed_out_stop_whose_target_remains_retains_the_operation"]),
    ("I4-10", "query uncertain after descriptor: descriptor retained",
     [f"{E}i4_10_an_uncertain_query_after_the_candidate_keeps_its_descriptor"]),
    ("I4-11", "manager absent, child in candidate: refuse/retain",
     [f"{E}i4_11_manager_absence_with_the_helper_in_the_candidate_is_refused_and_retained"]),
    ("I4-12", "limits mismatch: never launch; retain/clean",
     [f"{E}i4_12_mismatched_limits_never_launch",
      f"{S}i4_start_failed_settles_with_the_live_helper_and_reports_the_proof_failure"]),
    ("I4-13", "RuntimeMaxUSec mismatch: never launch",
     [f"{E}i4_13_a_runtime_backstop_mismatch_never_launches"]),
    ("I4-14", "OOMPolicy mismatch: never launch",
     [f"{E}i4_14_an_out_of_memory_policy_mismatch_never_launches"]),
    ("I4-15", "helper identity/membership mismatch: never launch",
     [f"{E}i4_15_a_membership_mismatch_never_launches"]),
    ("I4-16", "Pending never Proven by supplying the unit name",
     [f"{E}i4_16_a_pending_operation_is_never_proven_by_its_unit_name"]),
    ("I4-17", "name collision fails closed; another unit never claimed",
     [f"{E}i4_17_a_name_collision_fails_closed_and_never_touches_the_other_unit"]),
    ("I4-18", "no launch message before complete scope proof",
     [f"{E}i4_18_no_launch_message_reaches_the_helper_before_the_scope_is_proven"]),
    ("I4-19", "finalization cleans a Proven scope as before",
     [f"{E}i4_19_finalization_ends_a_proven_scope_as_before"]),
    ("I4-20", "finalization handles Pending",
     [f"{E}i4_20_finalization_settles_a_pending_operation"]),
    ("I4-21", "failed pending cleanup: CleanupFailed",
     [f"{E}i4_21_an_unconfirmed_operation_is_a_cleanup_failure_never_an_unavailable_sandbox"]),
    ("I4-22", "retry resolves Pending",
     [f"{E}i4_22_a_retry_resolves_a_pending_operation"]),
    ("I4-23", "retry works after the ScopeManager is dropped",
     [f"{E}i4_23_a_retained_boundary_is_retried_without_the_scope_manager",
      f"{S}i4_owners_are_self_contained_values"]),
    ("I4-24", "drop of an unresolved pending: best effort, no confirmation",
     [f"{E}i4_24_dropping_an_unresolved_operation_is_best_effort_and_confirms_nothing"]),
    ("I4-25", "panic after Start may have taken effect retains ownership",
     [f"{E}i4_25_a_panic_after_the_start_request_keeps_its_owner"]),
    ("I4-26", "panic after candidate descriptor acquisition retains it",
     [f"{E}i4_26_a_panic_after_the_candidate_is_retained_keeps_it"]),
    ("I4-27", "panic during StopUnit/reconciliation retains uncertainty",
     [f"{E}i4_27_a_panic_while_stopping_or_reconciling_retains_the_operation"]),
    ("I4-28", "helper-before-launch and scope-before-launch ordering remains",
     [f"{E}i4_28_the_helper_and_the_proven_scope_precede_any_launch"]),
    ("I4-29", "clean execution classification unchanged",
     [f"{E}i4_29_the_classification_of_settled_executions_is_unchanged"] + CLASSIFICATION),
    ("I4-30", "no PID-only, unit-name-only or cgroup-path-only cleanup",
     [f"{E}i4_30_the_helper_s_death_is_never_the_operation_s_cleanup",
      f"{E}i4_30_no_cgroup_of_the_unit_s_name_is_ever_taken_by_its_path",
      f"{S}i4_a_pending_operation_is_bound_to_its_own_helper"]),
]
COMPLETION = [
    ("a potentially effective StartTransientUnit request never escapes without retained ownership",
     ["I4-03", "I4-06", "I4-21", "I4-25"], ["NC-I4-START-LOST-OWNER", "NC-I4-PANIC-OWNER-LOSS"], []),
    ("Start uncertainty is reconciled", ["I4-02", "I4-03"], ["NC-I4-START-TIMEOUT-ABSENT"], []),
    ("Stop uncertainty is reconciled", ["I4-06", "I4-07", "I4-08", "I4-09"],
     ["NC-I4-STOP-OK-CONFIRMS", "NC-I4-STOP-ERR-NO-EFFECT", "NC-I4-STOP-TIMEOUT-DROPS"], []),
    ("helper-to-scope identity is proven natively", ["I4-15", "I4-16", "I4-30"],
     ["NC-I4-MEMBERSHIP-NAME-ONLY", "NC-I4-START-NAME-AUTH", "NC-I4-X-ANY-HELPER"], ["P-I4-HELPER-SERIAL"]),
    ("candidate cgroup descriptor ownership survives later proof failures", ["I4-04", "I4-10", "I4-26"],
     ["NC-I4-NO-EARLY-DIR", "NC-I4-PROPERTY-ERROR-DROPS", "NC-I4-X-NO-CANDIDATE-KILL"], ["P-I4-PENDING-CANDIDATE"]),
    ("no launch before full scope proof", ["I4-18", "I4-28"], ["NC-I4-LAUNCH-PENDING"], ["P-I4-BOUNDARY-TYPE", "P-I4-SCOPE-FORGE"]),
    ("Pending state survives panic", ["I4-25", "I4-26", "I4-27"], ["NC-I4-PANIC-OWNER-LOSS"], []),
    ("Pending state survives return to the caller as CleanupFailed", ["I4-06", "I4-20", "I4-21"],
     ["NC-I4-PENDING-NOT-RETAINED", "NC-I4-START-LOST-OWNER"], ["P-I4-RETAINED-SCOPE"]),
    ("RetainedBoundary retry cleans Pending state", ["I4-22"], [], []),
    ("retry remains possible after the original ScopeManager is dropped", ["I4-23"],
     ["NC-I4-RETRY-NEEDS-MANAGER-BORROW"], []),
    ("RPC success is never cleanup proof", ["I4-07"], ["NC-I4-STOP-OK-CONFIRMS"], []),
    ("RPC failure/timeout is never proof of no effect", ["I4-03", "I4-06", "I4-09"],
     ["NC-I4-START-TIMEOUT-ABSENT", "NC-I4-STOP-ERR-NO-EFFECT", "NC-I4-STOP-TIMEOUT-DROPS"], []),
    ("unit name / PID / path strings remain locators only", ["I4-11", "I4-16", "I4-17", "I4-24", "I4-30"],
     ["NC-I4-START-NAME-AUTH", "NC-I4-MANAGER-ABSENCE-ONLY", "NC-I4-PID-CLEANUP", "NC-I4-STRING-SWEEP",
      "NC-I4-X-COLLISION-CLAIMED", "NC-I4-X-DROP-REAPS"],
     ["P-I4-PENDING-NEW", "P-I4-PENDING-UNIT", "P-I4-SCOPE-FORGE"]),
    ("no unit-name sweep", ["I4-30"], ["NC-I4-STRING-SWEEP"], ["P-I4-PENDING-DESERIALIZE"]),
    ("no real host mutation in ordinary tests", [], [], []),
    ("existing clean execution behaviour and cleanup semantics remain green", ["I4-19", "I4-28", "I4-29"], [], []),
]
SOURCE_GUARDS_FOR = {
    "a potentially effective StartTransientUnit request never escapes without retained ownership": ["SG-I4-ONE-WAY"],
    "unit name / PID / path strings remain locators only": ["SG-I4-NO-PID-SIGNAL", "SG-I4-ONE-WAY"],
    "no unit-name sweep": ["SG-I4-NO-SWEEP", "SG-I4-NO-SERIALIZE"],
    "no real host mutation in ordinary tests": ["SG-I4-TEST-SEAMS", "SG-I4-BUS", "SG-I4-DESTINATION"],
    "retry remains possible after the original ScopeManager is dropped": ["SG-I4-NO-BACKGROUND"],
}


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
    controls = {c["id"]: c for c in json.loads(
        (runs / "controls" / "scope" / "summary.json").read_text())["counted"]}
    guards = {g["id"]: g for g in json.loads(
        (runs / "controls" / "api-guards" / "summary.json").read_text())["guards"]}
    sources = json.loads((runs / "controls" / "source-guards.json").read_text())["guards"]
    no_live = (runs / "no-live" / "verdict.txt").read_text().strip().endswith("RESULT: PASS")
    definitions = load(scripts / "scope_controls.py").COUNTED
    by_test = {}
    for c in definitions:
        by_test.setdefault(c["test"], []).append(c["id"])
    rows, missing = [], []
    for rid, what, tests in REQUIREMENTS:
        for test in tests:
            if test not in listed:
                missing.append(test)
        row = dict(
            id=rid, what=what,
            tests=[dict(test=t, result=results.get(t, "MISSING")) for t in tests],
            controls=[dict(id=cid, ok=controls.get(cid, {}).get("ok"))
                      for t in tests for cid in by_test.get(t, [])],
        )
        row["covered"] = all(t["result"] == "ok" for t in row["tests"])
        rows.append(row)
    completion = []
    req = {r["id"]: r for r in rows}
    for what, rids, cids, gids in COMPLETION:
        item = dict(
            standard=what,
            requirements={rid: req[rid]["covered"] for rid in rids},
            controls={cid: controls.get(cid, {}).get("ok") for cid in cids},
            api_guards={gid: guards.get(gid, {}).get("ok") for gid in gids},
            source_guards={sid: sources.get(sid, {}).get("ok") for sid in SOURCE_GUARDS_FOR.get(what, [])},
            traced={"no-live trace (validation/no-live/verdict.txt)": no_live}
            if what == "no real host mutation in ordinary tests" else {},
        )
        values = (list(item["requirements"].values()) + list(item["controls"].values())
                  + list(item["api_guards"].values()) + list(item["source_guards"].values())
                  + list(item["traced"].values()))
        item["met"] = all(v is True for v in values) and bool(values)
        completion.append(item)
    unmapped_controls = sorted(cid for cid in controls
                               if not any(cid in [c["id"] for c in r["controls"]] for r in rows))
    lines = ["# P2-V1-R3B-I4 test / control coverage", "",
             "Generated by `scripts/coverage.py` from the candidate runs (`validation/candidate/`, "
             "`controls/`). A test's result is its line in the validation's unit-test run; a "
             "control's result is its behavioural-control record; a guard's, its guard record.", "",
             "## Required regressions (mission section 16)", "",
             "| Requirement | Tests (result) | Behavioural controls on these tests | Covered |", "|---|---|---|---|"]
    for r in rows:
        tests = "<br>".join(f"`{t['test']}` ({t['result']})" for t in r["tests"])
        ctl = "<br>".join(f"`{c['id']}` ({'ok' if c['ok'] else 'NOT OK'})" for c in r["controls"]) or "—"
        lines.append(f"| {r['id']}: {r['what']} | {tests} | {ctl} | {'yes' if r['covered'] else 'NO'} |")
    lines += ["", "## Completion standard (mission section 25)", "",
              "| Standard | Requirements | Behavioural controls | API/type guards | Source guards and traces | Met |",
              "|---|---|---|---|---|---|"]
    for item in completion:
        fmt = lambda d: "<br>".join(f"{k} ({'ok' if v else 'NOT OK'})" for k, v in d.items()) or "—"
        lines.append(f"| {item['standard']} | {fmt(item['requirements'])} | {fmt(item['controls'])} | "
                     f"{fmt(item['api_guards'])} | {fmt({**item['source_guards'], **item['traced']})} | "
                     f"{'yes' if item['met'] else 'NO'} |")
    lines += ["", "The standard \"no real host mutation in ordinary tests\" is carried by the source "
              "guards (the simulation is test-only; the production bus and destination are fixed), by "
              "the traced unit-test run (`validation/no-live/`: no connect() at all, no systemd tool, "
              "no cgroup file but the standard library's read-only `cpu.max`), and by construction: "
              "every scope test runs over `scope/tests.rs`'s model, and the live harness is built and "
              "never run.", "",
              f"Unit tests listed: {len(listed)}; results recorded: {len(results)}; "
              f"tests named above but not listed: {missing or 'none'}.",
              f"Behavioural controls not mapped to a required regression: {unmapped_controls or 'none'}."]
    out_md.write_text("\n".join(lines) + "\n")
    out_json.write_text(json.dumps(dict(requirements=rows, completion=completion,
                                        missing_tests=missing,
                                        unmapped_controls=unmapped_controls), indent=2) + "\n")
    ok = not missing and all(r["covered"] for r in rows) and all(i["met"] for i in completion)
    print(json.dumps(dict(requirements=len(rows), covered=sum(r["covered"] for r in rows),
                          completion_met=sum(i["met"] for i in completion), completion=len(completion),
                          missing_tests=missing, unmapped_controls=unmapped_controls, ok=ok)))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
