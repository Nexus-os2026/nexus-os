#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission sections 5, 6, 7, 9 and 13-15: derive every execution
expectation from the frozen R2 catalog (authenticated copy) and Candidate C's sources (git objects).
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (sections 3, 4, 5, 10-12): catalog-derived expectations for the
ten Tranche-2 controls, in the mission's order, each cross-checked against the mission's control matrix
(common.MATRIX). The helpers fn_span, attrs_above and verdict_assertion are unchanged.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02 (sections 4, 8-13): the same derivation for this mission's
five controls, in its order C5 A3 A6 C8 C15 (receipt directories 61-65), and, for C5 only, the preserved build's
identities (mission section 2), which the dedicated C5 admission checker (c5.py) requires; nothing else changed.

    expectations.py OUT.json        writes the expectations; exit 0 only if every derivation and cross-check holds

Nothing here builds or runs anything. The catalog is the execution authority: every patch, property, configuration,
receipt type, test selection, ceiling and restoration mapping is the catalog's. The source of C supplies only
locations the catalog states by reference (the verdict assertion of the explorer test, the test's own line) and is
cross-checked against the catalog's own numbers where it gives them.

Tranche-2 derivations that the catalog requires and Tranche 1 did not need:
  - every mutant's bytes, from the frozen patch applied with `git apply` to C's file in a temporary directory outside
    every repository (never an object store), must have the catalog's mutant blob and sha256;
  - A3 and A6 mutate model_tests.rs, which holds the explorer itself: their verdict-assertion location is read from
    the mutant bytes (the panic is reported at the mutant's line), and their PROPERTIES and BRANCHES constants, their
    test count, their F-5 configuration constant and their full_f5 test are checked equal to C's;
  - A6 (catalog R2, receipt_metadata_item KEY_FIELDS): the expected printed key fields are the mutant's KEY_FIELDS
    constant, which must name recovery_required;
  - C3a and A6 (catalog special_attribution_rules "AD-1-R4 F.1(a): either property the row names"): the permitted
    printed properties are the catalog's named_property list; every other control permits exactly its one property
    (the field is then absent and classify_a behaves exactly as in Tranche 1);
  - C8 runs full_fb on the ORIGINAL explorer (catalog test; configs.py maps F-B to the composed-quotient test, which is
    the restored side only): its bounds and actors are derived from the FB constant and cross-checked against the
    accepted Stage-1 F-B receipt's printed bounds;
  - the restored side of each control: the catalog's restored_receipt (Stage-1 or Stage-2 unchanged-core receipt; for
    C15 the AD-1-R5 F refinement discharge record).
"""
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402
import configs  # noqa: E402

problems = []
EITHER = "AD-1-R4 F.1(a): either property the row names"
C15_RULE = ("AD-1-R5 F: original explorer, E8, 30,000,000; FAIL beyond the cap is INCOMPLETE — BUDGET EXCEEDED; "
            "restored side may cite the F-R refinement discharge record only after verified restoration")


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


def mutant_bytes(patch, rel, base):
    """The frozen patch applied with `git apply` to a copy of C's file in a temporary directory outside every
    repository (GIT_CEILING_DIRECTORIES stops discovery): the resulting bytes, never written to any object store."""
    tmp = tempfile.mkdtemp(prefix="mt2x-exp-apply-")
    try:
        os.makedirs(os.path.join(tmp, "w", os.path.dirname(rel)))
        with open(os.path.join(tmp, "w", rel), "wb") as fh:
            fh.write(base)
        with open(os.path.join(tmp, "p.patch"), "wb") as fh:
            fh.write(patch)
        env = dict(os.environ, GIT_CEILING_DIRECTORIES=tmp)
        r = subprocess.run(["git", "apply", "--whitespace=nowarn", os.path.join(tmp, "p.patch")],
                           cwd=os.path.join(tmp, "w"), capture_output=True, env=env)
        if r.returncode != 0:
            return None, r.stderr.decode()
        files = [os.path.relpath(os.path.join(d, f), os.path.join(tmp, "w")) for d, _, fs in os.walk(
            os.path.join(tmp, "w")) for f in fs]
        if files != [rel]:
            return None, f"files after apply: {files}"
        return open(os.path.join(tmp, "w", rel), "rb").read(), ""
    finally:
        shutil.rmtree(tmp)


def key_fields(text):
    """The value of `const KEY_FIELDS: &str = "...";` as the program prints it: a backslash at a line's end continues
    the literal and drops the newline and the next line's leading whitespace (Rust string continuation)."""
    m = re.search(r'^const KEY_FIELDS: &str = "((?:[^"\\]|\\\n)*)";$', text, re.M)
    if not need(m is not None, "KEY_FIELDS: the constant is not one plain string literal"):
        return None
    return re.sub(r"\\\n[ \t]*", "", m.group(1))


def model_constants(text):
    props = re.search(r"const PROPERTIES: \[&str; 25\] = \[(.*?)\];", text, re.S).group(1)
    branches = re.search(r"const BRANCHES: &\[&str\] = &\[(.*?)\n\];", text, re.S).group(1)
    return re.findall(r'"([^"]+)"', props), re.findall(r'"((?:[^"\\]|\\.)*)"', branches)


def config_block(text, const):
    m = re.search(r"^const " + re.escape(const) + r": Config = (?:default_run\()?Config \{.*?^\}\)?;$", text, re.M | re.S)
    return m.group(0) if m else None


def original_fb(model, derived):
    """C8: full_fb on the original explorer (the catalog's test). Its body is exactly `full_exploration(&FB);`, it is
    ignored at C, and its bounds and actors are the FB constant's (configs.py's own formatting functions)."""
    m = re.search(r"#\[ignore[^\]]*\]\s*fn full_fb\(\) \{(.*?)\n\}", model, re.S)
    need(m is not None and configs.norm(m.group(1)) == "full_exploration(&FB);", "full_fb: body is full_exploration(&FB)")
    c = dict(derived["configs"]["FB"])
    need(c["name"] == "F-B", f"FB is configuration {c['name']}")
    return {"config": c, "const": "FB", "test": K.TEST_PREFIX + "full_fb", "runner": "full_exploration",
            "explorer": "original", "ignored": m is not None, "bounds": configs.bounds_line(c, derived["consts"]),
            "actors": configs.actors_line(c)}


def main(out):
    cat_bytes = open(K.inputs("catalog.json"), "rb").read()
    need(K.sha256_bytes(cat_bytes) == K.CATALOG_SHA, "authenticated catalog copy sha256")
    cat = json.loads(cat_bytes)
    by = {e["id"]: e for e in cat["controls"]}
    model, unit, protocol = K.source(K.MODEL), K.source(K.UNIT), K.source(K.PROTOCOL)
    need(K.git_blob_id(model.encode()) == K.BLOBS[K.MODEL] and K.git_blob_id(unit.encode()) == K.BLOBS[K.UNIT]
         and K.git_blob_id(protocol.encode()) == K.BLOBS[K.PROTOCOL], "C's sources")
    derived = configs.derive()
    need(not derived["problems"], f"configs.derive: {derived['problems']}")
    req = derived["required"]
    c_props, c_branches = model_constants(model)
    tests_at_c = model.count("#[test]") + unit.count("#[test]") + protocol.count("#[test]")
    exp = {"mission": K.MISSION, "catalog": {"path": K.CATALOG_PATH, "commit": K.R2, "blob": K.CATALOG_BLOB,
                                             "sha256": K.CATALOG_SHA, "version": cat["version"]},
           "candidate_c": {"commit": K.C, "tree": K.C_TREE, "blobs": K.BLOBS},
           "baseline_executable": {"path": K.EXE, "sha256": K.EXE_SHA256, "size": K.EXE_SIZE, "worktree": K.BASE_WT},
           "build": {"argv": K.BUILD_ARGV, "limit_seconds": K.BUILD_LIMIT, "memory_max": K.MEMORY_MAX, "swap_max": "0",
                     "env_template": K.build_env("<worktree>/target")},
           "run_env": dict(K.RUN_ENV), "order": K.ORDER, "listed_tests_at_C": K.LISTED,
           "accepted_not_executed": K.ACCEPTED, "not_authorized": K.NOT_AUTHORIZED, "controls": {}}
    need(sorted(K.ORDER + K.ACCEPTED + K.NOT_AUTHORIZED + ["A4"]) == sorted(by) and len(set(K.ORDER + K.ACCEPTED
                                                                                         + K.NOT_AUTHORIZED)) == 35,
         "Tranche 2 + accepted + not authorized + A4 = every catalog entry, 35 kill-required controls")
    # Stage-1 F-B (the restored side of C8): its printed bounds equal the FB constant's (the same configuration).
    s1fb = K.blob(K.STAGE1, K.STAGE1_PACKET + "/receipts/42-full-F-B/raw-output.txt").decode()
    s1fb_bounds = [l[len("  bounds: "):] for l in s1fb.splitlines() if l.startswith("  bounds: ")]
    fb = original_fb(model, derived)
    need(s1fb_bounds == [fb["bounds"]], f"C8: the accepted Stage-1 F-B receipt prints the FB bounds {s1fb_bounds}")
    for n, cid in enumerate(K.ORDER, start=61):
        e = by[cid]
        cfg, allowed, cap, src_file, link_key = K.MATRIX[cid]
        print(f"-- {cid}: {e['description']}")
        need(e["kind"] == "explore", f"{cid} kind explore")
        need(e["configuration"] == cfg, f"{cid} configuration {e['configuration']} (mission {cfg})")
        need(e["named_property"] == allowed, f"{cid} named property {e['named_property']} (mission {allowed})")
        need(e["named_property_text"] == " or ".join(allowed), f"{cid} named property text {e['named_property_text']}")
        need(e["expected_primary_failure"]["printed_property_must_be"] == (
            allowed[0] if len(allowed) == 1 else "either of " + " / ".join(allowed)), f"{cid} printed property rule")
        need(e["ceiling_states"] == cap == e["expected_primary_failure"]["discovered_at_most"],
             f"{cid} ceiling {e['ceiling_states']} (mission {cap})")
        need(e["time_limit_seconds"] == 2700, f"{cid} time limit 2700 s")
        need(e["explorer"] == "original (`explore`)", f"{cid} original explorer")
        need(e["receipt_type"] == "(a) original-explorer mutant FAIL; restored (e1)", f"{cid} receipt type")
        need(e["ignored_at_C"] is True and e["requires_mutant_executable"] is True, f"{cid} ignored, mutant executable")
        need(e["expected_changed_blob_count"] == 1, f"{cid} one changed blob")
        need(e["mutation_source_file"] == src_file, f"{cid} mutated file {e['mutation_source_file']} (mission {src_file})")
        need(e["baseline_blob"] == K.BLOBS[src_file], f"{cid} baseline blob")
        need(e["expected_primary_failure"]["beyond_ceiling"] == "INCOMPLETE — BUDGET EXCEEDED; kills nothing"
             and e["expected_primary_failure"]["incomplete"] == "kills nothing", f"{cid} INCOMPLETE kills nothing")
        rules = {"C3a": [EITHER], "A6": [EITHER], "C15": [C15_RULE]}.get(cid, [])
        need(e["special_attribution_rules"] == rules, f"{cid} special attribution rules {e['special_attribution_rules']}")
        # The mutant's bytes, from the frozen patch (never written to any object store).
        patch = open(K.inputs("patches", cid + ".patch"), "rb").read()
        need(K.sha256_bytes(patch) == e["patch_sha256"], f"{cid} authenticated patch copy sha256")
        base = K.blob(K.C, src_file)
        mut, err = mutant_bytes(patch, src_file, base)
        need(mut is not None and not err, f"{cid} patch applies to C's file: {err}")
        if mut is None:
            continue
        need(K.git_blob_id(mut) == e["mutant_blob"] and K.sha256_bytes(mut) == e["mutant_sha256"],
             f"{cid} mutant blob and sha256 are the catalog's")
        mut_text = mut.decode("utf-8")
        run_model = mut_text if src_file == K.MODEL else model
        mut_tests = (run_model.count("#[test]") + unit.count("#[test]")
                     + (mut_text if src_file == K.PROTOCOL else protocol).count("#[test]"))
        need(mut_tests == tests_at_c, f"{cid}: the mutant keeps every #[test] ({mut_tests} of {tests_at_c})")
        # The configuration and the test the mutant executable runs: the catalog's test on the original explorer.
        if cfg == "F-B":
            r = fb
            need(req["F-B"]["test"] == K.TEST_PREFIX + "gc_symmetry_fb" and req["F-B"]["explorer"] == "composed",
                 "C8: configs maps F-B to the composed quotient (the restored side only)")
        else:
            r = req[cfg]
        need(r["test"] == e["test"], f"{cid} test {e['test']} equals the configuration's {r['test']}")
        need(r["explorer"] == "original" and r["runner"] == "full_exploration", f"{cid} original explorer runner")
        need(r["ignored"] is True, f"{cid} the test is ignored at C")
        need(r["config"]["cap"] == cap and r["config"]["time"] == 2700, f"{cid} the test's cap and time")
        tname = e["test"].split("::")[-1]
        span = fn_span(run_model, tname)
        need(span is not None and fn_span(model, tname)[0] == e["test_line_at_C"],
             f"{cid} test line at C vs catalog {e['test_line_at_C']}")
        va = verdict_assertion(run_model, r["runner"])
        c = {"id": cid, "order": n - 60, "dir": f"receipts/{n}-{cid}", "worktree": K.control_wt(cid),
             "kind": e["kind"], "receipt_type": e["receipt_type"], "description": e["description"],
             "patch": {"input": f"inputs/patches/{cid}.patch", "catalog_file": "catalog/" + e["patch_file"],
                       "sha256": e["patch_sha256"]},
             "source_file": src_file, "baseline_blob": e["baseline_blob"], "baseline_sha256": K.sha256_bytes(base),
             "mutant_blob": e["mutant_blob"], "mutant_sha256": e["mutant_sha256"], "restoration": e["restoration"],
             "test": e["test"], "configuration": cfg, "ignored": True, "runner": r["runner"],
             "argv_tail": [e["test"], "--exact", "--ignored", "--nocapture", "--test-threads=1"],
             "limit_seconds": e["time_limit_seconds"], "ceiling_states": e["ceiling_states"],
             "named_property": e["named_property_text"], "bounds": r["bounds"], "actors": r["actors"],
             "verdict_assertion": va, "special_attribution_rules": e["special_attribution_rules"]}
        if len(allowed) > 1:
            c["permitted_properties"] = list(allowed)
        if src_file == K.MODEL:
            need(model_constants(mut_text) == (c_props, c_branches), f"{cid}: PROPERTIES and BRANCHES unchanged")
            need(config_block(mut_text, r["const"]) == config_block(model, r["const"]) is not None,
                 f"{cid}: the {r['const']} configuration constant unchanged")
            need("\n".join(mut_text.split("\n")[span[0] - 1:span[1]]) == "\n".join(
                model.split("\n")[fn_span(model, tname)[0] - 1:fn_span(model, tname)[1]]), f"{cid}: {tname} unchanged")
            c["verdict_assertion_at_C"] = verdict_assertion(model, r["runner"])
            c["verdict_assertion_source"] = "the mutant model_tests.rs (the mutated file holds the explorer)"
        else:
            c["verdict_assertion_source"] = "Candidate C's model_tests.rs (unchanged by the patch)"
        if cid == "A6":
            kf, kf_c = key_fields(mut_text), key_fields(model)
            # The parser, validated on C: C's KEY_FIELDS is exactly what the accepted Stage-2 F-5 receipt printed.
            printed = [l[len("  state key fields: "):] for l in open(K.inputs("linked", "S2-65-full-F-5", "raw-output.txt"),
                                                                     encoding="utf-8").read().splitlines()
                       if l.startswith("  state key fields: ")]
            need(printed == [kf_c], "KEY_FIELDS parser: C's constant equals the key fields printed by the accepted "
                                    "Stage-2 F-5 receipt")
            need(kf is not None and K.A6_KEY_TEXT in kf and K.A6_KEY_TEXT not in (kf_c or ""),
                 "A6: the mutant's KEY_FIELDS names recovery_required (C's does not)")
            need(e.get("receipt_metadata_item") == "KEY_FIELDS", "A6: catalog receipt metadata item KEY_FIELDS")
            c["key_fields"] = kf
        # The restored side (catalog restored_receipt; mission section 5).
        rr = e["restored_receipt"]
        if link_key == "FR-DISCHARGE":
            f2 = rr["f2_receipt"]
            need(rr["type"] == "AD-1-R5 F refinement discharge record (not a run, never PASS)"
                 and rr["ad1r5_sha256"] == K.AD1R5_SHA and rr["proof_sha256"] == K.FR_PROOF_SHA
                 and f2["commit"] == K.STAGE1 and f2["directory"] == K.STAGE1_PACKET + "/receipts/41-full-F-2"
                 and f2["descriptor_test"] == K.TEST_PREFIX + "full_f2", f"{cid} restored receipt: the F-R discharge")
            c["linked"] = {"type": "fr-discharge", "commit": K.STAGE1, "directory": f2["directory"],
                           "input_dir": "inputs/linked/S1-41-full-F-2", "classification_blob": f2["classification_blob"],
                           "raw_output_blob": f2["raw_output_blob"], "raw_output_sha256": f2["raw_output_sha256"],
                           "descriptor_test": f2["descriptor_test"], "outer_class": f2["outer_class"],
                           "proof_sha256": rr["proof_sha256"], "ad1r5_sha256": rr["ad1r5_sha256"],
                           "adoption_sha256": cat["sources"]["ad1r5"]["adoption_record_sha256"],
                           "fr_receipt": K.STAGE2_PACKET + "/receipts/69-full-F-R", "recording": K.FR_RECORDING}
            need(c["linked"]["adoption_sha256"] == K.AD1R5_ADOPTION_SHA, f"{cid} the R5 adoption record sha256")
        else:
            stage, d = link_key.split(":")
            commit, packet = (K.STAGE1, K.STAGE1_PACKET) if stage == "S1" else (K.STAGE2, K.STAGE2_PACKET)
            need(rr["type"] == "e1" and rr["commit"] == commit and rr["directory"] == f"{packet}/receipts/{d}",
                 f"{cid} restored receipt {rr['directory']} at {rr['commit']} (mission {link_key})")
            want_test = K.TEST_PREFIX + "gc_symmetry_fb" if cid == "C8" else e["test"]
            need(rr["descriptor_test"] == want_test and rr["descriptor_argv_executable"] == K.EXE
                 and rr["descriptor_argv_flags"] == ["--exact", "--ignored", "--nocapture", "--test-threads=1"],
                 f"{cid} restored receipt test and flags {rr['descriptor_test']}")
            need(rr["outer_class"] == "outer class (AD-1-R4 K.2): PASS — admissible unchanged-core " +
                 ("composed-quotient PASS" if cid == "C8" else "original-explorer PASS"), f"{cid} restored outer class")
            c["linked"] = {"type": "e1", "stage": stage, "commit": commit, "directory": rr["directory"],
                           "input_dir": f"inputs/linked/{stage}-{d}", "configuration": cfg,
                           "classification_blob": rr["classification_blob"], "raw_output_blob": rr["raw_output_blob"],
                           "raw_output_sha256": rr["raw_output_sha256"], "descriptor_test": rr["descriptor_test"],
                           "outer_class": rr["outer_class"], "quotient": cid == "C8"}
        if cid == "C5":
            # Mission sections 2, 6 and 7: the preserved build, admitted (not rebuilt), and its executable (pinned
            # read-only where cargo wrote it). The patch and mutant identities must be the catalog's.
            need(K.C5_PATCH_SHA256 == e["patch_sha256"] and K.C5_MUTANT_BLOB == e["mutant_blob"]
                 and K.C5_MUTANT_SHA256 == e["mutant_sha256"], "C5: the ruling's patch and mutant identities are the "
                                                               "catalog's")
            c["preserved"] = {"build_receipt": {"commit": K.T2, "directory": K.C5_BUILD_DIR,
                                                "input_dir": "inputs/c5/03-build"},
                              "build_stderr_sha256": K.C5_STDERR_SHA256, "worktree": K.C5_WT,
                              "lock_reason": K.C5_LOCK_REASON, "executable": {
                                  "path": K.C5_EXE, "sha256": K.C5_EXE_SHA256, "size": K.C5_EXE_SIZE},
                              "run_source_verdict": "EXACTLY_C"}
        exp["controls"][cid] = c
        print(f"   {c['dir']}; {c['test']} ({cfg}); permitted {allowed}; cap {cap:,}; verdict assertion "
              f"{va['file']}:{va['line']}:{va['col']} ({c['verdict_assertion_source']}); link {c['linked']['type']} "
              f"{c['linked']['directory']}")
    exp["problems"] = problems
    if not problems:
        json.dump(exp, open(out, "w"), indent=1)
    for p in problems:
        print("PROBLEM:", p)
    print("EXPECTATIONS: " + ("DERIVED — ALL CROSS-CHECKS HOLD" if not problems else "PROBLEMS — STOP"))
    return 0 if not problems else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
