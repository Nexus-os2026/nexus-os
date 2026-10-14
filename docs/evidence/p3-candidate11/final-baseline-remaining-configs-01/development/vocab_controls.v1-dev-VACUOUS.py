#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01, mission section 5: negative (and positive) controls of the vocabulary
generator, the two static checks and the normalizer.

  vocab_controls.py VOCABULARY.json WORKDIR OUT.txt

WORKDIR (outside the evidence directory) receives the mutated inputs; no mutated source is kept in the evidence, only
each mutation's description and the mutated file's sha256. Every tool runs as a subprocess, exactly as frozen.
Each control names the tools that must detect it; a control passes only if every one of them fails (exit != 0, or
the normalizer's STOP, exit 2) and every positive control passes. Exit 0 only if every control behaves as required.

Controls (the six classes of mission section 5, and the normalizer's own STOP conditions):
  PC1 unmutated source and vocabulary              generator EQUAL, check A PASS, check B PASS
  PC2 unmutated Stage-1 F-2 report                 normalizer NORMALIZED
  NC1a an omitted source site: a new obs.branch call in source          generator, A, B
  NC1b an omitted source site: the vocabulary lacks site S08            generator, A, B
  NC2a an omitted enum variant: the vocabulary lacks one Debug row      generator, B
  NC2b an omitted enum variant: Entry gains a variant in source         generator, B
  NC3a a changed exact label in source                                  generator, B
  NC3b a changed exact label in the vocabulary                          generator, B
  NC4a an unresolved collision: a second site emits 'lookup: Absent',
       and the vocabulary lists it under the same row                   generator, A, B
  NC4b an unresolved collision: the defective admission site becomes
       reachable from a required test                                   A
  NC5a a raw observed label outside the vocabulary                      normalizer
  NC5b a listed raw label changed                                       normalizer
  NC6a a vocabulary row with no source derivation                       generator, B
  NC6b a vocabulary row with no site at all                             generator, A, B
  NC7  a TAKEN label of an operation NOT APPLICABLE by configuration    normalizer
  NC8  extra labels out of the printed (sorted) order                   normalizer
  NC9  a truncated branch list                                          normalizer
"""
import copy
import hashlib
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common  # noqa: E402

TOOLS = os.path.dirname(os.path.abspath(__file__))
F2_RAW = f"{common.STAGE1_PACKET}/receipts/41-full-F-2/raw-output.txt"


def sha(b):
    return hashlib.sha256(b).hexdigest()


def run(args, timeout=1800):
    r = subprocess.run([sys.executable, *args], capture_output=True, text=True, timeout=timeout)
    lines = (r.stdout + r.stderr).splitlines()
    key = [l for l in lines if l.startswith(("FAIL", "PROBLEM", "COMPARE", "CHECK", "vocab_generate", "STOP", "F-2",
                                             "Traceback", "SystemExit"))]
    return r.returncode, key


def mutate_source(work, name, edits):
    """edits: [(file key 'model'|'protocol', old, new)]: each old must occur exactly once."""
    d = os.path.join(work, name)
    os.makedirs(d, exist_ok=True)
    texts = {"model": common.source(common.MODEL), "protocol": common.source(common.PROTOCOL)}
    desc = []
    for which, old, new in edits:
        n = texts[which].count(old)
        if n != 1:
            raise SystemExit(f"control {name}: the snippet occurs {n} times in {which}")
        line = texts[which][:texts[which].index(old)].count("\n") + 1
        texts[which] = texts[which].replace(old, new)
        desc.append(f"{os.path.basename(common.MODEL if which == 'model' else common.PROTOCOL)} at line {line}: "
                    f"{old!r} -> {new!r}")
    out = {}
    for which, path in (("model", common.MODEL), ("protocol", common.PROTOCOL)):
        data = texts[which].encode()
        open(os.path.join(d, os.path.basename(path)), "wb").write(data)
        out[os.path.basename(path)] = sha(data)
    return d, desc, out


def write_json(work, name, obj):
    p = os.path.join(work, name + ".json")
    json.dump(obj, open(p, "w"), indent=1, ensure_ascii=False)
    return p, sha(open(p, "rb").read())


def main(vocab_path, work, out):
    os.makedirs(work, exist_ok=True)
    vocab = json.load(open(vocab_path))
    gen, a, b, nz = (os.path.join(TOOLS, f) for f in ("vocab_generate.py", "vocab_check_sites.py",
                                                      "vocab_check_eval.py", "normalize.py"))
    results, report = [], [f"VOCABULARY CONTROLS — vocabulary {vocab_path} sha256 {sha(open(vocab_path, 'rb').read())}",
                           f"work directory (not in the evidence): {work}", ""]

    def record(cid, what, desc, expect, outcomes, positive=False):
        ok = all((rc == 0) if positive else (rc != 0) for _, rc, _ in outcomes if _ in expect) and \
             all(t in [o[0] for o in outcomes] for t in expect)
        results.append((cid, ok))
        report.append(f"{cid} — {what}")
        for d in desc:
            report.append(f"    mutation: {d}")
        for tool, rc, key in outcomes:
            verdict = ("PASS" if rc == 0 else "FAIL") if positive else ("DETECTED" if rc != 0 else "NOT DETECTED")
            exp = "expected" if tool in expect else "not required"
            report.append(f"    {tool}: exit {rc} -> {verdict} ({exp})")
            for k in key[:6]:
                report.append(f"        | {k[:220]}")
        report.append(f"    CONTROL {'OK' if ok else 'FAILED'}")
        report.append("")

    def all_three(src_dir, vp):
        g = run([gen, "--compare", vp] + (["--source-dir", src_dir] if src_dir else []))
        ca = run([a, vp] + (["--source-dir", src_dir] if src_dir else []))
        cb = run([b, vp] + (["--source-dir", src_dir] if src_dir else []))
        return [("generator --compare", g[0], g[1]), ("check A", ca[0], ca[1]), ("check B", cb[0], cb[1])]

    # PC1.
    record("PC1", "unmutated source and the vocabulary", ["none"], ["generator --compare", "check A", "check B"],
           all_three(None, vocab_path), positive=True)
    # PC2 and the normalizer controls' base.
    f2 = common.blob(common.STAGE1, F2_RAW).decode()

    def norm_case(cid, what, text, desc, positive=False):
        p = os.path.join(work, cid + "-raw-output.txt")
        open(p, "w").write(text)
        rc, key = run([nz, "file", "F-2", p, os.path.join(work, cid + "-normalized"), "--vocab", vocab_path])
        record(cid, what, desc + [f"input sha256 {sha(text.encode())}"], ["normalizer"], [("normalizer", rc, key)],
               positive=positive)

    norm_case("PC2", "unmutated Stage-1 F-2 report", f2, [f"Stage-1 {F2_RAW}"], positive=True)

    # NC1a.
    d, desc, h = mutate_source(work, "NC1a", [(
        "model", "    w.rows.push(SimRow::Prepared(0));\n",
        "    w.rows.push(SimRow::Prepared(0)); obs.branch(\"expire: Phantom\".into());\n")])
    record("NC1a", "an omitted source site: a new obs.branch call (step_env)", desc + [f"mutated sha256 {h}"],
           ["generator --compare", "check A", "check B"], all_three(d, vocab_path))
    # NC1b.
    v = copy.deepcopy(vocab)
    v["sites"] = [s for s in v["sites"] if s["site"] != "S08"]
    v["rows"] = [r for r in v["rows"] if r["operation"] != "malformed snapshot"]
    p, hv = write_json(work, "NC1b-vocabulary", v)
    record("NC1b", "an omitted source site: the vocabulary lacks site S08 and its rows",
           [f"vocabulary without S08, sha256 {hv}"], ["generator --compare", "check A", "check B"], all_three(None, p))
    # NC2a.
    v = copy.deepcopy(vocab)
    v["rows"] = [r for r in v["rows"] if r["label"] != "lookup: Refused(Expired)"]
    p, hv = write_json(work, "NC2a-vocabulary", v)
    record("NC2a", "an omitted enum variant: the vocabulary lacks 'lookup: Refused(Expired)'",
           [f"vocabulary sha256 {hv}"], ["generator --compare", "check B"], all_three(None, p))
    # NC2b.
    d, desc, h = mutate_source(work, "NC2b", [("protocol", "    Malformed,\n    Expired,\n}",
                                               "    Malformed,\n    Expired, Bogus,\n}")])
    record("NC2b", "an omitted enum variant: Entry gains a variant in source", desc + [f"mutated sha256 {h}"],
           ["generator --compare", "check B"], all_three(d, vocab_path))
    # NC3a.
    d, desc, h = mutate_source(work, "NC3a", [(
        "model", 'Some(FactOutcome::Committed) => "Committed (already present)".to_string(),',
        'Some(FactOutcome::Committed) => "Committed (already there)".to_string(),')])
    record("NC3a", "a changed exact label in source (fact_append)", desc + [f"mutated sha256 {h}"],
           ["generator --compare", "check B"], all_three(d, vocab_path))
    # NC3b.
    v = copy.deepcopy(vocab)
    for r in v["rows"]:
        if r["label"] == "barrier: Passed":
            r["label"] = "barrier: passed"
    p, hv = write_json(work, "NC3b-vocabulary", v)
    record("NC3b", "a changed exact label in the vocabulary ('barrier: Passed' -> 'barrier: passed')",
           [f"vocabulary sha256 {hv}"], ["generator --compare", "check B"], all_three(None, p))
    # NC4a.
    model = common.source(common.MODEL)
    gc_stmt = "    w.gc[g as usize] = GcPos::Done;\n"
    line = model[:model.index(gc_stmt)].count("\n") + 1
    d, desc, h = mutate_source(work, "NC4a", [(
        "model", gc_stmt, "    w.gc[g as usize] = GcPos::Done; obs.branch(\"lookup: Absent\".into());\n")])
    v = copy.deepcopy(vocab)
    new_site = dict(site="S12", file=common.MODEL, function="step_gc", line=line,
                    call='obs.branch("lookup: Absent".into());')
    v["sites"].append(dict(site="S12", function="step_gc", line=line, call=new_site["call"], operations=["lookup"]))
    for r in v["rows"]:
        if r["label"] == "lookup: Absent":
            r["sites"].append(new_site)
    p, hv = write_json(work, "NC4a-vocabulary", v)
    record("NC4a", "an unresolved collision: step_gc also emits 'lookup: Absent'; the vocabulary lists the second site "
           "under the same row without a proof", desc + [f"mutated sha256 {h}", f"vocabulary sha256 {hv}"],
           ["generator --compare", "check A", "check B"], all_three(d, p))
    # NC4b.
    d, desc, h = mutate_source(work, "NC4b", [(
        "model", "fn full_fg() {\n    full_exploration(&FG);\n}",
        "fn full_fg() {\n    full_exploration(&FG); let _ = adm_ignoring_unapplied;\n}")])
    record("NC4b", "an unresolved collision: the defective admission site S11 becomes reachable from a required test",
           desc + [f"mutated sha256 {h}"], ["check A"], all_three(d, vocab_path))
    # NC5a.
    marker = "    [taken    ] append_cell C: Refused(Latched) (not in the listed set)\n"
    norm_case("NC5a", "a raw observed label outside the vocabulary (inserted in sorted position)",
              f2.replace(marker, "    [taken    ] append_cell C: Refused(Bogus) (not in the listed set)\n" + marker),
              ["F-2 report + '    [taken    ] append_cell C: Refused(Bogus) (not in the listed set)'"])
    # NC5b.
    norm_case("NC5b", "a listed raw label changed",
              f2.replace("    [NOT TAKEN] lookup: Conflict(cause)\n", "    [NOT TAKEN] lookup: Conflict(Cause)\n"),
              ["F-2 report: 'lookup: Conflict(cause)' -> 'lookup: Conflict(Cause)'"])
    # NC6a.
    v = copy.deepcopy(vocab)
    s04 = next(s for r in v["rows"] for s in r["sites"] if s["site"] == "S04")
    v["rows"].append(dict(id="B999", operation="lookup", label="lookup: Phantom", sites=[s04], source_items=[],
                          source_expressions=[], derivation=[], in_static_branches=False, static_index=None, note=""))
    p, hv = write_json(work, "NC6a-vocabulary", v)
    record("NC6a", "a vocabulary row with no source derivation ('lookup: Phantom', claiming site S04)",
           [f"vocabulary sha256 {hv}"], ["generator --compare", "check B"], all_three(None, p))
    # NC6b.
    v = copy.deepcopy(vocab)
    v["rows"].append(dict(id="B999", operation="lookup", label="lookup: Phantom", sites=[], source_items=[],
                          source_expressions=[], derivation=[], in_static_branches=False, static_index=None, note=""))
    p, hv = write_json(work, "NC6b-vocabulary", v)
    record("NC6b", "a vocabulary row with no site and no derivation", [f"vocabulary sha256 {hv}"],
           ["generator --compare", "check A", "check B"], all_three(None, p))
    # NC7.
    norm_case("NC7", "a TAKEN label of an operation NOT APPLICABLE by configuration (DROP in F-2)",
              f2.replace("    [NOT TAKEN] DROP: a Counted ticket dropped\n",
                         "    [taken    ] DROP: a Counted ticket dropped\n"),
              ["F-2 report: DROP marked taken"])
    # NC8.
    e1 = "    [taken    ] append_cell C: Refused(NotHealthy) (not in the listed set)\n"
    e2 = "    [taken    ] append_cell X: Refused(NotHealthy) (not in the listed set)\n"
    norm_case("NC8", "extra labels out of the printed (sorted) order", f2.replace(e1 + e2, e2 + e1),
              ["F-2 report: the last two extra lines exchanged"])
    # NC9.
    norm_case("NC9", "a truncated branch list (one listed line removed)",
              f2.replace("    [NOT TAKEN] explain: Busy\n", ""), ["F-2 report: 'explain: Busy' line removed"])

    allok = all(ok for _, ok in results)
    report.append(f"CONTROLS: {sum(ok for _, ok in results)} of {len(results)} behave as required — "
                  f"{'ALL OK' if allok else 'FAILED: ' + ', '.join(c for c, ok in results if not ok)}")
    open(out, "w").write("\n".join(report) + "\n")
    print("\n".join(report))
    return 0 if allok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2], sys.argv[3]))
