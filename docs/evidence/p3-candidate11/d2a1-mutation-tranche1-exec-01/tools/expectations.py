#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission sections 5, 6, 7, 9 and 13-15: derive every execution
expectation from the frozen R2 catalog (authenticated copy) and Candidate C's sources (git objects).

    expectations.py OUT.json        writes the expectations; exit 0 only if every derivation and cross-check holds

Nothing here builds or runs anything. The catalog is the execution authority: every patch, property, configuration,
receipt type, test selection, ceiling and restoration mapping is the catalog's. The source of C supplies only
locations the catalog states by reference (the verdict assertion of the explorer test, the named test's own lines,
the T-OWN assertion line) and is cross-checked against the catalog's own numbers where it gives them.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402
import configs  # noqa: E402

problems = []


def need(ok, why):
    if not ok:
        problems.append(why)
    return ok


def fn_span(text, name):
    """(first line, last line) of `fn name(` at column 0 through its closing `}` at column 0 (1-based, inclusive)."""
    lines = text.split("\n")
    starts = [i for i, l in enumerate(lines) if re.match(rf"^fn {re.escape(name)}\(", l)]
    if not need(len(starts) == 1, f"fn {name}: {len(starts)} definitions at column 0"):
        return None
    s = starts[0]
    e = next((j for j in range(s + 1, len(lines)) if lines[j] == "}"), None)
    need(e is not None, f"fn {name}: no closing brace")
    return s + 1, e + 1


def attrs_above(text, first_line):
    """The attribute lines directly above line first_line (1-based)."""
    lines = text.split("\n")
    out, j = [], first_line - 2
    while j >= 0 and lines[j].startswith("#["):
        out.insert(0, lines[j])
        j -= 1
    return out


def verdict_assertion(text, runner):
    """file:line:col of the one `assert_eq!(` inside `fn <runner>(config: &Config) {` (the explorer's verdict
    assertion, AD-1-R4 F.1 (a)); the macro's location is what the panic reports."""
    span = fn_span(text, runner)
    lines = text.split("\n")
    hits = [(i + 1, l.index("assert_eq!(") + 1) for i in range(span[0] - 1, span[1]) for l in [lines[i]]
            if "assert_eq!(" in l]
    need(len(hits) == 1, f"{runner}: {len(hits)} assert_eq! calls")
    body = "\n".join(lines[span[0] - 1:span[1]])
    need('"failed: {:?}\\n{}",' in body and "report.verdict," in body and "Verdict::Pass," in body,
         f"{runner}: the verdict assertion's text")
    return {"file": K.MODEL, "line": hits[0][0], "col": hits[0][1], "span": list(span)}


def main(out):
    cat_bytes = open(K.inputs("catalog.json"), "rb").read()
    need(K.sha256_bytes(cat_bytes) == K.CATALOG_SHA, "authenticated catalog copy sha256")
    cat = json.loads(cat_bytes)
    by = {e["id"]: e for e in cat["controls"]}
    model, unit = K.source(K.MODEL), K.source(K.UNIT)
    need(K.git_blob_id(model.encode()) == K.BLOBS[K.MODEL] and K.git_blob_id(unit.encode()) == K.BLOBS[K.UNIT],
         "C's test sources")
    derived = configs.derive()
    need(not derived["problems"], f"configs.derive: {derived['problems']}")
    req = derived["required"]
    exp = {"mission": K.MISSION, "catalog": {"path": K.CATALOG_PATH, "commit": K.R2, "blob": K.CATALOG_BLOB,
                                             "sha256": K.CATALOG_SHA, "version": cat["version"]},
           "candidate_c": {"commit": K.C, "tree": K.C_TREE, "blobs": K.BLOBS},
           "baseline_executable": {"path": K.EXE, "sha256": K.EXE_SHA256, "size": K.EXE_SIZE, "worktree": K.BASE_WT},
           "build": {"argv": K.BUILD_ARGV, "limit_seconds": K.BUILD_LIMIT, "memory_max": K.MEMORY_MAX, "swap_max": "0",
                     "env_template": K.build_env("<worktree>/target")},
           "run_env": dict(K.RUN_ENV), "order": K.ORDER, "listed_tests_at_C": K.LISTED,
           "baselines": [], "controls": {}}
    # The T-OWN assertion (C22, F.1 (d)): derived again at the final baseline.
    town = [i + 1 for i, l in enumerate(unit.split("\n")) if l == "    <Ticket as NotClone<_>>::check();"]
    need(len(town) == 1, f"T-OWN assertion lines {town}")
    need(town == [int(by["C22"]["expected_primary_failure"]["location"].rsplit(":", 1)[1])],
         "T-OWN line equals the catalog's location")
    town_span = fn_span(unit, "t_own_tickets_snapshots_and_the_worker_cannot_be_cloned")
    need(town_span[0] < town[0] <= town_span[1], "T-OWN assertion inside the T-OWN test")
    # Baseline standalone receipts (mission section 7): X-1 (e1), then the five named tests (e2).
    x1 = req["X-1"]
    x1_span = fn_span(model, "x1_two_facts_two_writers_one_admission")
    need(not any("ignore" in a for a in attrs_above(model, x1_span[0])), "X-1 is not ignored at C")
    exp["baselines"].append({"key": "X-1", "type": "e1", "control": "C12", "dir": "receipts/10-baseline-X-1",
                             "test": x1["test"], "configuration": "X-1", "ignored": False,
                             "argv_tail": [x1["test"], "--exact", "--nocapture", "--test-threads=1"],
                             "limit_seconds": 180, "ceiling_states": 250000,
                             "bounds": x1["bounds"], "actors": x1["actors"], "properties": x1["properties"],
                             "verdict_assertion": verdict_assertion(model, "default_exploration")})
    for n, cid in enumerate(["C14", "C19", "C21", "C27", "C28"], start=11):
        e = by[cid]
        name = e["test"].split("::")[-1]
        span = fn_span(unit, name)
        need(not any("ignore" in a for a in attrs_above(unit, span[0])) and "#[test]" in attrs_above(unit, span[0]),
             f"{cid}: {name} is a #[test] and not ignored at C")
        need(e["ignored_at_C"] is False, f"{cid}: catalog ignored_at_C")
        exp["baselines"].append({"key": e["named_test_id"], "type": "e2", "control": cid,
                                 "dir": f"receipts/{n}-baseline-{e['named_test_id']}", "test": e["test"],
                                 "ignored": False, "argv_tail": [e["test"], "--exact", "--nocapture", "--test-threads=1"],
                                 "limit_seconds": 180, "test_span": list(span)})
    # The twelve controls (mission sections 9-15), in order.
    for n, cid in enumerate(K.ORDER, start=31):
        e = by[cid]
        c = {"id": cid, "order": n - 30, "dir": f"receipts/{n}-{cid}", "worktree": K.control_wt(cid), "kind": e["kind"],
             "receipt_type": e["receipt_type"], "description": e["description"],
             "patch": {"input": f"inputs/patches/{cid}.patch", "catalog_file": "catalog/" + e["patch_file"],
                       "sha256": e["patch_sha256"]},
             "source_file": e["mutation_source_file"], "baseline_blob": e["baseline_blob"],
             "baseline_sha256": K.sha256_bytes(K.blob(K.C, e["mutation_source_file"])),
             "mutant_blob": e["mutant_blob"], "mutant_sha256": e["mutant_sha256"],
             "restoration": e["restoration"]}
        need(e["baseline_blob"] == K.BLOBS[K.PROTOCOL], f"{cid} baseline blob")
        if e["kind"] == "build":
            c.update({"diagnostic_code": "E0283", "diagnostic_file": K.UNIT, "diagnostic_line": town[0],
                      "diagnostic_source_line": "<Ticket as NotClone<_>>::check();",
                      "linked": {"type": "e4", "dir": "receipts/20-healthy-build"}})
        elif e["kind"] == "unit":
            name = e["test"].split("::")[-1]
            span = fn_span(unit, name)
            need(e["named_test_candidates_at_C"] == [name], f"{cid} named test candidates")
            need(e["test"] == K.UNIT_PREFIX_TEST + name, f"{cid} test path")
            base = next(b for b in exp["baselines"] if b["control"] == cid)
            c.update({"test": e["test"], "named_test_id": e["named_test_id"], "ignored": False,
                      "argv_tail": [e["test"], "--exact", "--nocapture", "--test-threads=1"],
                      "limit_seconds": e["time_limit_seconds"], "test_file": K.UNIT, "test_span": list(span),
                      "linked": {"type": "e2", "dir": base["dir"]}})
            need(e["time_limit_seconds"] == 180, f"{cid} time limit 180 s (mission 13)")
        else:
            cfg = e["configuration"]
            r = req[cfg]
            need(r["test"] == e["test"], f"{cid} test equals the configuration's test")
            need(r["ignored"] == e["ignored_at_C"], f"{cid} ignored at C: source {r['ignored']}, catalog {e['ignored_at_C']}")
            need(r["config"]["cap"] == e["ceiling_states"], f"{cid} ceiling = the configuration's cap")
            need(r["config"]["time"] == e["time_limit_seconds"], f"{cid} time limit = the configuration's time cap")
            need(r["explorer"] == "original", f"{cid} original explorer")
            need(len(e["named_property"]) == 1 and e["named_property"][0] == e["expected_primary_failure"][
                "printed_property_must_be"], f"{cid} one named property")
            need(e["special_attribution_rules"] == [], f"{cid}: no attribution exception in the catalog")
            span = fn_span(model, e["test"].split("::")[-1])
            need(span[0] == e["test_line_at_C"], f"{cid} test line at C {span[0]} vs catalog {e['test_line_at_C']}")
            tail = [e["test"], "--exact"] + (["--ignored"] if r["ignored"] else []) + ["--nocapture", "--test-threads=1"]
            if cfg == "X-1":
                linked = {"type": "e1", "dir": "receipts/10-baseline-X-1"}
            else:
                linked = {"type": "e1", "stage2": True, "configuration": cfg, "input_dir": f"inputs/linked/{cfg}",
                          "commit": K.STAGE2, "directory": e["restored_receipt"]["directory"],
                          "classification_blob": e["restored_receipt"]["classification_blob"],
                          "raw_output_blob": e["restored_receipt"]["raw_output_blob"],
                          "raw_output_sha256": e["restored_receipt"]["raw_output_sha256"]}
            c.update({"test": e["test"], "configuration": cfg, "ignored": r["ignored"], "runner": r["runner"],
                      "argv_tail": tail, "limit_seconds": e["time_limit_seconds"], "ceiling_states": e["ceiling_states"],
                      "named_property": e["named_property"][0], "bounds": r["bounds"], "actors": r["actors"],
                      "verdict_assertion": verdict_assertion(model, r["runner"]), "linked": linked})
        exp["controls"][cid] = c
    exp["problems"] = problems
    if not problems:
        json.dump(exp, open(out, "w"), indent=1)
    print(json.dumps({"problems": problems, "controls": {k: {kk: v.get(kk) for kk in ("dir", "kind", "test",
                      "argv_tail", "limit_seconds", "ceiling_states", "named_property", "verdict_assertion", "test_span",
                      "diagnostic_line", "linked")} for k, v in exp["controls"].items()},
                      "baselines": exp["baselines"]}, indent=1))
    print("EXPECTATIONS: " + ("DERIVED — ALL CROSS-CHECKS HOLD" if not problems else "PROBLEMS — STOP"))
    return 0 if not problems else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
