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

Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-TAIL-01 (sections 1, 2, 6-9): the same derivation for this mission's four
controls, in its order A1 A2 C20 A5 (receipt directories 81-84); C24, C3b, C3c and C6 are Architect-accepted and C7 is
MUTATION_CONTROL_BLOCKED: none of them is derived (no expectation exists for C7, so nothing can execute it), and the
catalog partition now counts C7 among the blocked controls. Each of the four keeps the zero-warning, zero-error build
rule; C20 and A5 the L1 complete-search rule. Every other derivation and every helper is unchanged.

Revised by P3-C11-DR01-D2A1-MUTATION-WITNESS-C1-C25-01 (sections 2-7): the same derivation for this mission's two
controls, in its order C1 C25 (receipt directories 81-82); A1, A2, C20 and A5 are Architect-accepted, C7 is blocked and
C4 is reserved: none of them is derived (no expectation exists for C4 or C7, so nothing can execute either). New per
control, the AD-1-R4 G.3 block (`g3`, g3_block): the type-(a) primary's predicted first failure (AD-1-R4 G.3(a)'s
prediction, recorded as printed, never counted as the named property); the type-(c) mutant witness (its test at C, an
ordinary #[test] whose body is exactly run_witness(&<const>); the Witness constant's schedule, locus and branches; the
location of run_witness's own verdict panic; the exact positive line, header and line of other properties, computed
from the source's format and found verbatim in AD-1-R4 G.3(b)); the accepted healthy type-(e3) receipt (the catalog's
healthy_witness, Stage 2) with its exact healthy line (verbatim in AD-1-R4 G.3(d)); C25's narrow patch preserving
`self.failed();`, `self.rolled(io.rollback());` and `self.keep = true;`. Every value is cross-checked against the
mission's statement (common.G3). Every other derivation and every helper is unchanged.

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
G3_RULE = ("AD-1-R4 G.3 (kill criterion for C1 and C25 only): type (a) primary on full_f1 is recorded as printed; "
           "attribution from the type (c) mutant witness; type (e3) healthy witness required")
RULES = {"C3b": [EITHER], "C6": [WITH_A4], "C7": [WITH_A4, C7_ADDED], "C20": [L1_RULE], "A5": [L1_RULE],
         "C1": [G3_RULE], "C25": [G3_RULE]}
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


def witness_const(model, name):
    """The Witness constant `name` of the source, parsed: (control, config const, steps, named, at, in_step, healthy,
    changed); steps as printed by run_witness ({actor:?}, {choices:?}) with their fixed branches or None."""
    m = re.search(r"^const " + re.escape(name) + r": Witness = Witness \{\n(.*?)\n\};$", model, re.M | re.S)
    if not need(m is not None, f"{name}: one Witness constant"):
        return None
    t = m.group(1)
    field = lambda k: (re.findall(rf"^    {k}: (.*),$", t, re.M) or [None])[0]
    # The list is written over several lines (C1) or on one line (C25).
    sm = re.search(r"^    steps: &\[\n(.*?)\n    \],$", t, re.M | re.S) or re.search(r"^    steps: &\[(.*)\],$", t, re.M)
    need(sm is not None, f"{name}: the steps list")
    flat = re.sub(r"\s+", " ", sm.group(1) if sm else "")
    steps = []
    for a, ch, fx in re.findall(r"\( ?Actor::(\w+(?:\(\d+\))?), &\[([\d, ]*)\], (None|Some\(&\[[^\]]*\]\)),? ?\)",
                                flat):
        nums = [int(x) for x in re.findall(r"\d+", ch)]
        fixed = None if fx == "None" else re.findall(r'"((?:[^"\\]|\\.)*)"', fx)
        steps.append([a, "[" + ", ".join(str(x) for x in nums) + "]", fixed])
    need(len(steps) == flat.count("Actor::"), f"{name}: every step parsed ({len(steps)} of {flat.count('Actor::')})")
    unq = lambda v: v[1:-1] if v and v.startswith('"') and v.endswith('"') else None
    return {"control": unq(field("control")), "config": field("config"), "steps": steps, "named": unq(field("named")),
            "at": int(field("at")) if field("at") and field("at").isdigit() else None,
            "in_step": {"true": True, "false": False}.get(field("in_step")), "healthy": unq(field("healthy")),
            "changed": unq(field("changed"))}


def g3_block(cid, e, model, mut_text, derived):
    """The AD-1-R4 G.3 expectations of C1 or C25 (mission sections 4 and 5), cross-checked against common.G3."""
    M = K.G3[cid]
    ad1r4 = open(K.inputs("AD1-R4.md"), encoding="utf-8").read()
    need(K.sha256_bytes(ad1r4.encode()) == K.AD1R4_SHA, f"{cid}: authenticated AD-1-R4 copy sha256")
    flat = re.sub(r"\s+", " ", ad1r4)
    # The catalog: the witness requirement and the healthy witness (the accepted Stage-2 type-(e3) receipt).
    need(e.get("witness_required") is True and e["special_attribution_rules"] == [G3_RULE], f"{cid}: catalog G.3 rule")
    wt = K.TEST_PREFIX + M["witness_test"]
    cw = e.get("witness") or {}
    need(cw.get("test") == wt and "same pinned mutant executable" in cw.get("mutant_witness", "")
         and "type (c)" in cw.get("mutant_witness", ""), f"{cid}: catalog witness {cw.get('test')}")
    need(e["patch_sha256"] == M["patch_sha256"] and e["mutant_blob"] == M["mutant_blob"],
         f"{cid}: the frozen patch sha256 and mutant blob are the mission's (sections 4, 5)")
    hw = cw.get("healthy_witness") or {}
    hdir = f"{K.STAGE2_PACKET}/{M['healthy_dir']}"
    need(hw.get("type") == "e3" and hw.get("commit") == K.STAGE2 and hw.get("directory") == hdir
         and hw.get("descriptor_test") == wt and hw.get("descriptor_argv_executable") == K.EXE
         and hw.get("descriptor_argv_flags") == ["--exact", "--nocapture", "--test-threads=1"]
         and hw.get("executable_before") == hw.get("executable_after") == hw.get("executable_pinned_sha256")
         == K.EXE_SHA256 and hw.get("outer_class") == "outer class (AD-1-R4 K.2): PASS — admissible healthy-witness "
                                                    "type-(e3) PASS" and hw.get("files_equal_at_introducing_commit")
         is True, f"{cid}: catalog healthy witness {hw.get('directory')} at {hw.get('commit')}")
    # AD-1-R4 G.3: the prediction, the attribution rule, the exact lines.
    pm = re.search(r"Predicted: (\w+) in (\d+) steps? for C1; (\w+) in (\d+) steps? for C25\.", flat)
    need(pm is not None, "AD-1-R4 G.3(a): the prediction sentence")
    pred = {"C1": (pm.group(1), int(pm.group(2))), "C25": (pm.group(3), int(pm.group(4)))}[cid] if pm else (None, None)
    need(pred[0] == M["predicted"], f"{cid}: AD-1-R4 G.3(a) predicts {pred[0]}; the mission states {M['predicted']}")
    need("E3 is never counted as P1, and E4 never as P7." in flat, "AD-1-R4 G.3(a): E3 never P1, E4 never P7")
    need(e["named_property"] == [M["named"]], f"{cid}: the catalog's named property {e['named_property']}")
    # The source at C (model_tests.rs; both patches change protocol.rs only, so the witness is C's in the mutant too).
    need(mut_text is not None and e["mutation_source_file"] == K.PROTOCOL, f"{cid}: the patch changes protocol.rs only")
    span = fn_span(model, M["witness_test"])
    if span is None:
        return None
    lines = model.split("\n")
    need(attrs_above(model, span[0]) == ["#[test]"], f"{cid}: {M['witness_test']} is an ordinary #[test] (not ignored)")
    need([l.strip() for l in lines[span[0]:span[1] - 1]] == [f"run_witness(&{M['witness_const']});"],
         f"{cid}: {M['witness_test']}'s body is exactly run_witness(&{M['witness_const']});")
    wc = witness_const(model, M["witness_const"])
    if wc is None:
        return None
    cfgs = {k: v for k, v in derived["configs"].items()}
    cname = (cfgs.get(wc["config"]) or {}).get("name")
    need(wc["control"] == cid and cname == "F-1" and wc["named"] == M["named"] and len(wc["steps"]) == M["steps"]
         and wc["at"] is not None and 1 <= wc["at"] <= len(wc["steps"]) and wc["healthy"] and wc["changed"]
         and [s[2] is None for s in wc["steps"]] == [i + 1 == wc["at"] for i in range(len(wc["steps"]))],
         f"{cid}: the {M['witness_const']} constant (control, F-1, named, steps, locus, the one changed step)")
    rw = fn_span(model, "run_witness")
    body = lines[rw[0] - 1:rw[1]] if rw else []
    pan = [(rw[0] + i, l.index("panic!(") + 1) for i, l in enumerate(body) if "panic!(" in l]
    need(len(pan) == 1, f"run_witness: {len(pan)} panic! calls")
    rwt = "\n".join(body)
    for anchor in ('"{} WITNESS {}: {total} steps, each enabled and taking its choices; {} violations\\n\\',
                   "NAMED PROPERTY {}: {seen} {locus} step {} ({})\\n\\",
                   "OTHER PROPERTIES VIOLATED, each reported separately and not counted as {}: {others:?}\\n\\",
                   "every violation:\\n  {}\",", '"OBSERVED"', '"NOT OBSERVED"', '"at"', '"on the world after"',
                   ".filter(|property| *property != witness.named)"):
        need(anchor in rwt, f"run_witness: format anchor {anchor[:50]!r}")
    total = len(wc["steps"])
    positive = (f"NAMED PROPERTY {wc['named']}: OBSERVED {'at' if wc['in_step'] else 'on the world after'} step "
                f"{wc['at']} ({wc['changed']})")
    healthy_line = (f"{cid} WITNESS {cname}: {total} steps, each enabled and taking its choices; no violation; step "
                    f"{wc['at']} shows the healthy core's '{wc['healthy']}'")
    header_prefix = f"{cid} WITNESS {cname}: {total} steps, each enabled and taking its choices; "
    others_prefix = f"OTHER PROPERTIES VIOLATED, each reported separately and not counted as {wc['named']}: "
    need(positive == M["positive"] and f"{cid}: `{positive}`" in flat,
         f"{cid}: the positive line from the source is the mission's and AD-1-R4 G.3(b)'s: {positive}")
    need(healthy_line == M["healthy_line"] and f"{cid}: `{healthy_line}`" in flat,
         f"{cid}: the healthy line from the source is the mission's and AD-1-R4 G.3(d)'s: {healthy_line}")
    need(f"`{header_prefix}<n> violations`" in flat, f"{cid}: AD-1-R4 G.3(b) header form {header_prefix}")
    need(f"not counted as P1: [...]` (or `… as P7: [...]`)" in flat and others_prefix.startswith(
        "OTHER PROPERTIES VIOLATED, each reported separately and not counted as "), f"{cid}: G.3(b) other-properties form")
    if cid == "C25":
        ml = mut_text.split("\n")
        bl = K.source(K.PROTOCOL).split("\n")
        j = [k for k, l in enumerate(bl) if l.strip() == "return refused(Persistence::Uncertain, Reason::Storage);"]
        # C already holds one `return GatingOutcome::Committed(seq);` (the Durable arm's neighbour, line 1460): the
        # mutant's changed line is found at the index of C's one Uncertain return, and every other line is C's.
        need(len(j) == 1 and len(ml) == len(bl) and ml[j[0]].strip() == "return GatingOutcome::Committed(seq);"
             and [l.strip() for l in ml[j[0] - 3:j[0]]] == M["preserved_lines"] and ml[:j[0]] == bl[:j[0]]
             and ml[j[0] + 1:] == bl[j[0] + 1:],
             "C25: the narrow patch replaces only the Uncertain return; self.failed(), the rollback and self.keep = true "
             "are preserved (mission section 5)")
        need(e["patch_lines_removed_added"] == [1, 1], "C25: one line removed, one added")
    return {"rule": G3_RULE, "predicted": pred[0], "predicted_steps": pred[1], "named": wc["named"],
            "recording": M["recording"],
            "witness": {"test": wt, "argv_tail": [wt, "--exact", "--nocapture", "--test-threads=1"], "ignored": False,
                        "limit_seconds": K.WITNESS_LIMIT, "panic": {"file": K.MODEL, "line": pan[0][0],
                                                                    "col": pan[0][1]} if pan else None,
                        "control": cid, "config_name": cname, "const": M["witness_const"], "steps": wc["steps"],
                        "total": total, "at": wc["at"], "in_step": wc["in_step"], "healthy": wc["healthy"],
                        "changed": wc["changed"], "positive": positive, "header_prefix": header_prefix,
                        "others_prefix": others_prefix, "healthy_line": healthy_line,
                        "test_line_at_C": span[0]},
            "healthy": {"type": "e3", "commit": K.STAGE2, "directory": hdir,
                        "input_dir": f"inputs/linked/S2-{M['healthy_dir'].split('/')[-1]}",
                        "classification_blob": hw["classification_blob"], "raw_output_blob": hw["raw_output_blob"],
                        "raw_output_sha256": hw["raw_output_sha256"], "outer_class": hw["outer_class"],
                        "descriptor_test": hw["descriptor_test"], "descriptor_argv_flags": hw["descriptor_argv_flags"],
                        "executable_sha256": hw["executable_pinned_sha256"]}}


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
           "accepted_not_executed": K.ACCEPTED, "blocked_not_executed": K.BLOCKED, "not_authorized": K.NOT_AUTHORIZED,
           "controls": {}}
    need(sorted(K.ORDER + K.ACCEPTED + K.BLOCKED + K.NOT_AUTHORIZED + ["A4"]) == sorted(by)
         and len(set(K.ORDER + K.ACCEPTED + K.BLOCKED + K.NOT_AUTHORIZED)) == 35,
         "this mission + accepted + blocked (C7) + not authorized + A4 = every catalog entry, 35 kill-required controls")
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
        need(e["expected_primary_failure"]["printed_property_must_be"] == K.G3[cid]["named"],
             f"{cid}: the catalog's printed-property field names {K.G3[cid]['named']}, read under its own special "
             "attribution rule (AD-1-R4 G.3): the primary is recorded as printed")
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
        # WITNESS-C1-C25-01 (mission sections 4 and 5; AD-1-R4 G.3): the G.3 block; the RESULT names both receipt
        # types of each role.
        if cid in K.G3:
            c["g3"] = g3_block(cid, e, model, mut_text, derived)
            c["receipt_type"] = ("(a) original-explorer mutant FAIL, the G.3(a) primary (recorded as printed) + (c) "
                                 "named-property mutant witness, G.3(b), on the same pinned mutant executable; restored "
                                 "(e1) + healthy witness (e3), G.3(d)")
        exp["controls"][cid] = c
        print(f"   {c['dir']}; {c['test']} ({cfg}); permitted {allowed}; cap {cap:,}; verdict assertion "
              f"{va['file']}:{va['line']}:{va['col']} ({c['verdict_assertion_source']}); link {c['linked']['type']} "
              f"{c['linked']['directory']}")
    # TRANCHE3-TAIL-01 (mission sections 1 and 2): exactly this mission's four controls are derived; never C7.
    # WITNESS-C1-C25-01 (mission sections 1, 3, 10): exactly C1 and C25, each with its G.3 block; never C4 or C7.
    need(sorted(exp["controls"]) == sorted(K.ORDER) and not set(K.BLOCKED + K.ACCEPTED + K.NOT_AUTHORIZED)
         & set(exp["controls"]) and K.BLOCKED == ["C7"] and K.NOT_AUTHORIZED == ["C4"]
         and all(exp["controls"][c].get("g3") for c in K.ORDER),
         f"expectations exist for exactly {K.ORDER} (each with its G.3 block), none for C4, C7 or an accepted control")
    exp["problems"] = problems
    if not problems:
        json.dump(exp, open(out, "w"), indent=1)
    for p in problems:
        print("PROBLEM:", p)
    print("EXPECTATIONS: " + ("DERIVED — ALL CROSS-CHECKS HOLD" if not problems else "PROBLEMS — STOP"))
    return 0 if not problems else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
