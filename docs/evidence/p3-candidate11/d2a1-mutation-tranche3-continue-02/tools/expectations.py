#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission sections 5, 6, 7, 9 and 13-15: derive every execution
expectation from the frozen R2 catalog (authenticated copy) and Candidate C's sources (git objects).
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (sections 3, 4, 5, 10-12): catalog-derived expectations for the
ten Tranche-2 controls, in the mission's order, each cross-checked against the mission's control matrix
(common.MATRIX). The helpers fn_span, attrs_above and verdict_assertion are unchanged.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02 (sections 4, 8-13): the same derivation for this mission's
five controls, in its order C5 A3 A6 C8 C15 (receipt directories 61-65), and, for C5 only, the preserved build's
identities (mission section 2), which the dedicated C5 admission checker (c5.py) requires; nothing else changed.

Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01 (sections 3-9, 11): the same derivation for this mission's ten
controls, in its order C2 C24 C3b C3c C6 C7 A1 A2 C20 A5 (receipt directories 66-75), with: C3b's two permitted
properties (P13 or P12; catalog: either property the row names); C6 and C7 on full_fpt, the ORIGINAL explorer, with A4
ON by F-PT's definition (configs.py maps F-PT to the composed quotient, which is the restored side only), derived like
C8's full_fb (original_cfg); C7's informational F-3 run without A4 (mission section 5; catalog additional_comparison;
full_f3 on the original explorer, 30,000,000 states, 2700 s; never the kill receipt); C20 and A5's L1 rule (mission
section 8; AD-1-R4 Q: the explorer evaluates L1 only after a COMPLETE search, anchored in explore()'s source text and
unchanged in every mutant); the restored-side mappings of mission section 9. The helpers fn_span, attrs_above,
verdict_assertion, mutant_bytes, key_fields, model_constants, config_block and original_fb are unchanged.

Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02 (sections 2-15, AMENDMENT-01): the same derivation for this
mission's nine controls, in its order C24 C3b C3c C6 C7 A1 A2 C20 A5 (receipt directories 81-89); C2 is
Architect-accepted and no longer derived. New per control: the build rule (`build_rule`: C24 the preserved build under
the narrow C24 warning ruling, never rebuilt; C3b and C3c a clean build or exactly their one predicted warning group
(AMENDMENT-01); every other control zero warnings and zero errors under the unchanged general classifier); and, for C24
only, the preserved build's identities (mission section 2), which the dedicated C24 admission checker (c24.py)
requires. Every other derivation and every helper is unchanged.

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
WITH_A4 = "contract 7.5: 'with A4' — run on F-PT (A4 on), original explorer (AD-1-R4 F)"
C7_ADDED = "AD-1-R4 F (line 748): C7 without A4 on F-3 is an added run, not required to kill"
L1_RULE = "L1 is evaluated only after a complete search (AD-1-R4 Q)"
RULES = {"C3b": [EITHER], "C6": [WITH_A4], "C7": [WITH_A4, C7_ADDED], "C20": [L1_RULE], "A5": [L1_RULE]}
# explore()'s L1 evaluation (AD-1-R4 Q): L1 is computed only when the search ended with no failure and no INCOMPLETE,
# i.e. after every discovered state was expanded; a cycle is then reported as FAIL L1 with its own detail text.
L1_ANCHORS = ["if failure.is_none() && incomplete.is_none() && config.l1 {",
              '"{} states lie on or behind a cycle of non-fault steps; one is reached by",',
              "while (id as usize) < visited.len() {",
              "if visited.len() as u64 > config.cap {",
              'incomplete = Some(format!("the state cap of {} was reached", config.cap));']


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


def original_cfg(model, derived, test_name, const, name):
    """C6/C7 (full_fpt) and C7's informational run (full_f3): the catalog's test on the original explorer. Its body is
    exactly `full_exploration(&<const>);`, it is ignored at C, and its bounds and actors are the constant's (configs.py's
    own formatting functions); original_fb's derivation for another test."""
    m = re.search(r"#\[ignore[^\]]*\]\s*fn " + test_name + r"\(\) \{(.*?)\n\}", model, re.S)
    need(m is not None and configs.norm(m.group(1)) == f"full_exploration(&{const});",
         f"{test_name}: body is full_exploration(&{const})")
    c = dict(derived["configs"][const])
    need(c["name"] == name, f"{const} is configuration {c['name']}")
    return {"config": c, "const": const, "test": K.TEST_PREFIX + test_name, "runner": "full_exploration",
            "explorer": "original", "ignored": m is not None, "bounds": configs.bounds_line(c, derived["consts"]),
            "actors": configs.actors_line(c)}


def fn_text(text, name):
    """The text of the one item `[pub(super) ]fn name(` at column 0 through its closing `}` at column 0, or None."""
    lines = text.split("\n")
    starts = [i for i, l in enumerate(lines) if re.match(rf"^(?:pub\(super\) )?fn {re.escape(name)}\(", l)]
    if len(starts) != 1:
        return None
    e = next((j for j in range(starts[0] + 1, len(lines)) if lines[j] == "}"), None)
    return None if e is None else "\n".join(lines[starts[0]:e + 1])


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
         "Tranche 3 + accepted + not authorized + A4 = every catalog entry, 35 kill-required controls")
    # F-PT (C6, C7): the original explorer's full_fpt, A4 ON by the configuration's definition; the Stage-1 F-PT
    # composed-quotient receipt (the restored side only) prints the same bounds.
    fpt = original_cfg(model, derived, "full_fpt", "FPT", "F-PT")
    need(fpt["config"]["blanket_uncertain"] is True and " A4 true;" in fpt["bounds"], "F-PT: A4 on (blanket_uncertain)")
    need(req["F-PT"]["test"] == K.TEST_PREFIX + "gc_symmetry_fpt" and req["F-PT"]["explorer"] == "composed",
         "configs maps F-PT to the composed quotient (the restored side only)")
    s1pt = K.blob(K.STAGE1, K.STAGE1_PACKET + "/receipts/44-full-F-PT/raw-output.txt").decode()
    s1pt_bounds = [l[len("  bounds: "):] for l in s1pt.splitlines() if l.startswith("  bounds: ")]
    need(s1pt_bounds == [fpt["bounds"]], f"C6/C7: the accepted Stage-1 F-PT receipt prints the FPT bounds {s1pt_bounds}")
    # F-3 (C7's informational run): the original explorer's full_f3, A4 OFF.
    f3 = original_cfg(model, derived, "full_f3", "F3", "F-3")
    need(f3["config"]["blanket_uncertain"] is False and " A4 false;" in f3["bounds"], "F-3: A4 off")
    need(req["F-3"]["test"] == K.TEST_PREFIX + "gc_symmetry_f3" and req["F-3"]["explorer"] == "composed",
         "configs maps F-3 to the composed quotient (never used here: the informational run is the original explorer)")
    need({k: v for k, v in fpt["config"].items() if k not in ("name", "blanket_uncertain")}
         == {k: v for k, v in f3["config"].items() if k not in ("name", "blanket_uncertain")},
         "F-PT is F-3 with A4 on: every other field equal")
    # L1 (C20, A5): the explorer's complete-search rule, anchored in C's explore().
    explore_c = fn_text(model, "explore")
    need(explore_c is not None and all(configs.norm_tokens(a) in configs.norm_tokens(explore_c) for a in L1_ANCHORS),
         "explore() found at C, and it holds every L1 anchor")
    flat = configs.norm_tokens(model)
    need(all(configs.norm_tokens(a) in flat for a in L1_ANCHORS), "L1 anchors in explore() at C")
    need(derived["configs"]["F2"]["l1"] is True, "F-2 claims L1 (config.l1)")
    for n, cid in enumerate(K.ORDER, start=K.FIRST_DIR):
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
        rules = RULES.get(cid, [])
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
        if cfg == "F-PT":
            r = fpt
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
        c = {"id": cid, "order": n - K.FIRST_DIR + 1, "dir": f"receipts/{n}-{cid}", "worktree": K.control_wt(cid),
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
            need(fn_text(mut_text, "explore") == explore_c is not None
                 and fn_text(mut_text, "full_exploration") == fn_text(model, "full_exploration") is not None,
                 f"{cid}: explore() and full_exploration() unchanged (the explorer itself is not mutated)")
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
        if cid in K.A4_ON:
            need(r["config"]["blanket_uncertain"] is True and " A4 true;" in c["bounds"], f"{cid}: A4 ON (F-PT)")
            c["a4"] = {"required": True, "bounds_token": " A4 true;", "quotient_forbidden": True}
        if cid in K.L1_CONTROLS:
            need(r["config"]["l1"] is True and allowed == ["L1"], f"{cid}: L1 claimed on {cfg}; L1 the only property")
            c["l1_rule"] = {"complete_search_required": True, "anchors": L1_ANCHORS,
                            "detail_form": "<k> states lie on or behind a cycle of non-fault steps; one is reached by",
                            "rule": L1_RULE}
        # TRANCHE3-CONTINUE-02 (mission sections 2-5, 15; AMENDMENT-01): each control's build rule.
        if cid == "C24":
            need(K.C24_PATCH_SHA256 == e["patch_sha256"] and K.C24_MUTANT_BLOB == e["mutant_blob"]
                 and K.C24_MUTANT_SHA256 == e["mutant_sha256"], "C24: the ruling's patch and mutant identities are the "
                                                                 "catalog's")
            c["build_rule"] = {"kind": "preserved build, narrow C24 warning ruling (mission section 2); never rebuilt",
                               "permitted_warning": "warning: variant `Corrected` is never constructed",
                               "location": f"{K.PROTOCOL}:739:5"}
            c["preserved"] = {"build_receipt": {"commit": K.T3X, "directory": K.C24_BUILD_DIR,
                                                "input_dir": "inputs/c24/03-build"},
                              "apply_receipt": {"commit": K.T3X, "directory": K.C24_APPLY_DIR,
                                                "input_dir": "inputs/c24/02-apply"},
                              "build_stderr_sha256": K.C24_STDERR_SHA256, "worktree": K.C24_WT,
                              "lock_reason": K.C24_LOCK_REASON, "executable": {
                                  "path": K.C24_EXE, "sha256": K.C24_EXE_SHA256, "size": K.C24_EXE_SIZE},
                              "run_source_verdict": "EXACTLY_C"}
        elif cid in K.C3W:
            w = K.C3W[cid]
            need(w["patch_sha256"] == e["patch_sha256"] and w["mutant_blob"] == e["mutant_blob"]
                 and w["mutant_sha256"] == e["mutant_sha256"], f"{cid}: the build rule's patch and mutant identities are "
                                                                "the catalog's")
            mut_lines = mut_text.split("\n")
            need(mut_lines[K.C3W_ENUM[1] - 1] == "pub(super) enum AckRefusal {"
                 and mut_lines[w["line"] - 1] == f"    {w['variant']},"
                 and protocol.count(f"AckRefusal::{w['variant']}") == 1 and mut_text.count(f"AckRefusal::{w['variant']}")
                 == 0, f"{cid}: the mutant's AckRefusal enum at line {K.C3W_ENUM[1]}, {w['variant']} at line "
                       f"{w['line']}; protocol.rs names AckRefusal::{w['variant']} once at C and not at all in the mutant")
            c["build_rule"] = {"kind": "one fresh build: a clean build (BUILD: PINNED under the unchanged general "
                                       "classifier) or exactly the one predicted warning group (AMENDMENT-01); permitted, "
                                       "not required",
                               "permitted_warning": f"warning: variant `{w['variant']}` is never constructed",
                               "location": f"{K.PROTOCOL}:{w['line']}:{w['col']}", "enum_line": K.C3W_ENUM[1]}
        else:
            c["build_rule"] = {"kind": "one fresh build: zero warnings and zero errors under the unchanged general "
                                       "classifier (no warning exception)"}
        if cid == "C7":
            ac = e.get("additional_comparison") or {}
            need(ac.get("configuration") == "F-3" and ac.get("required_to_kill") is False
                 and ac.get("test") == f3["test"] and ac.get("patch") == "the same patches/C7.patch in both executions "
                                                                          "(mission 6.6)",
                 f"C7: catalog additional_comparison {ac}")
            need(f3["ignored"] is True and f3["config"]["cap"] == K.C7_INFO["ceiling_states"]
                 and f3["config"]["time"] == K.C7_INFO["limit_seconds"] and f3["test"] == K.TEST_PREFIX
                 + K.C7_INFO["test_name"], "C7 informational run: full_f3, ignored, 30,000,000 states, 2700 s")
            c["informational"] = {"dir": K.C7_INFO["dir"], "configuration": "F-3", "test": f3["test"], "ignored": True,
                                  "runner": "full_exploration", "explorer": "original", "a4": False,
                                  "argv_tail": [f3["test"], "--exact", "--ignored", "--nocapture", "--test-threads=1"],
                                  "ceiling_states": K.C7_INFO["ceiling_states"],
                                  "limit_seconds": K.C7_INFO["limit_seconds"], "bounds": f3["bounds"],
                                  "actors": f3["actors"], "verdict_assertion": va, "required_to_kill": False,
                                  "catalog_rule": ac.get("rule"),
                                  "outcomes": ["FAIL <property> (states at most the ceiling)",
                                               "PASS (a complete original search)",
                                               "INCOMPLETE at the explorer's own state or time ceiling"]}
        # The restored side (catalog restored_receipt; mission section 9).
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
            want_test = K.TEST_PREFIX + "gc_symmetry_fpt" if cid in K.A4_ON else e["test"]
            need(rr["descriptor_test"] == want_test and rr["descriptor_argv_executable"] == K.EXE
                 and rr["descriptor_argv_flags"] == ["--exact", "--ignored", "--nocapture", "--test-threads=1"],
                 f"{cid} restored receipt test and flags {rr['descriptor_test']}")
            need(rr["outer_class"] == "outer class (AD-1-R4 K.2): PASS — admissible unchanged-core " +
                 ("composed-quotient PASS" if cid in K.A4_ON else "original-explorer PASS"), f"{cid} restored outer class")
            c["linked"] = {"type": "e1", "stage": stage, "commit": commit, "directory": rr["directory"],
                           "input_dir": f"inputs/linked/{stage}-{d}", "configuration": cfg,
                           "classification_blob": rr["classification_blob"], "raw_output_blob": rr["raw_output_blob"],
                           "raw_output_sha256": rr["raw_output_sha256"], "descriptor_test": rr["descriptor_test"],
                           "outer_class": rr["outer_class"], "quotient": cid in K.A4_ON}
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
