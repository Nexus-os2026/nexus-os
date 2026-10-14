#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01, mission section 11: the unchanged-core coverage matrix of all twenty
explorations, produced ONLY if all ten remaining configurations are classified PASS.

  aggregate.py OUTDIR [--tables DIR]     (--tables: development input in place of the mission's receipts)

Inputs: the frozen vocabulary; the Stage-1 tables (normalized/stage1/<cfg>.json: X-1..X-5, F-1, F-2, F-3, F-PT,
F-B); this mission's tables (receipts/6n-full-<cfg>/BRANCH-ACCOUNTING-<cfg>.json), each with its run classified
PASS. For every vocabulary row: the configurations where it is TAKEN, NOT TAKEN and NOT APPLICABLE, and whether at
least one required run takes it. Nothing requires a row to be taken (mission section 11).

Cross-links: for a row that no required run takes, the suite's tests (model_tests.rs and unit_tests.rs at C) whose
body calls the row's protocol method and names every protocol outcome variant behind the row's label, with each test's result in the Stage-1 unfiltered
suite and whether it is one of AD-1-R4 D's 51 diagnostics. This is a TEXTUAL cross-reference: such a test exercises
the protocol outcome, not the explorer's branch label, and naming a variant is not proof of exercising it.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common  # noqa: E402
import normalize  # noqa: E402

ORDER = ["X-1", "X-2", "X-3", "X-4", "X-5", "F-1", "F-2", "F-3", "F-4", "F-5", "F-6", "F-7", "F-8", "F-PT", "F-B",
         "F-B2", "F-G", "F-W", "F-D", "F-R"]
STAGE1 = ["X-1", "X-2", "X-3", "X-4", "X-5", "F-1", "F-2", "F-3", "F-PT", "F-B"]
REMAINING = {"F-G": "60", "F-B2": "61", "F-4": "62", "F-W": "63", "F-8": "64", "F-5": "65", "F-6": "66", "F-7": "67",
             "F-D": "68", "F-R": "69"}
# The protocol method each operation's label reports (the call the model makes), required in a cross-referenced test.
METHOD = {"append_facts": r"\.append_facts\(", "append_gating": r"\.append_gating\(", "lookup": r"\.lookup\(",
          "explain": r"\.explain\(", "snapshot": r"\.snapshot\(", "append_acknowledgement": r"\.append_acknowledgement\(",
          "barrier": r"\.barrier\(", "resolve": r"\.resolve\(", "append_cell C": r"\.append_cell\(",
          "append_cell X": r"\.append_cell\(", "append_cell U": r"\.append_cell\(", "empty": r"\.empty\(",
          "expire": r"\.expire\("}
VARIANT = re.compile(r"\b(Persistence|Reason|Entry|Known|CauseKind|FactOutcome|TicketFault|GatingOutcome|Lookup|Explain|"
                     r"SnapshotOutcome|AckRefusal|AckOutcome|BarrierOutcome|Resolve|CellAppend|Emptying)::(\w+)")


def tests_of(path, prefix):
    text = common.source(path)
    out = {}
    for m in re.finditer(r"#\[test\]", text):
        rest = text[m.start():]
        fn = re.search(r"\bfn (\w+)\s*\(", rest)
        end = rest.find("\n}\n", fn.end())
        out[prefix + fn.group(1)] = rest[:end + 3]
    return out


def main(outdir, tables=None):
    vocab, vsha = normalize.load_vocab()
    status, sources = {}, {}
    problems = []
    for cfg in STAGE1:
        p = os.path.join(tables or os.path.join(common.EV, "normalized/stage1"), cfg + ".json")
        t = json.load(open(p))
        if t.get("verdict") != "NORMALIZED" or t.get("vocabulary_sha256") != vsha:
            problems.append(f"{cfg}: table not NORMALIZED over the frozen vocabulary")
        status[cfg] = {r["id"]: r for r in t["rows"]}
        sources[cfg] = f"{os.path.relpath(p, common.EV) if not tables else p} (Stage-1 report, {t['explorer']} explorer)"
    for cfg, n in REMAINING.items():
        d = os.path.join(tables, cfg) if tables else os.path.join(common.EV, f"receipts/{n}-full-{cfg}")
        cls = os.path.join(d, "CLASSIFICATION.txt")
        if not os.path.exists(cls) or open(cls).read().splitlines()[0] != "CLASSIFICATION: PASS":
            print(f"AGGREGATE NOT PRODUCED: {cfg} is not classified PASS (mission section 11)")
            return 3
        t = json.load(open(os.path.join(d, f"BRANCH-ACCOUNTING-{cfg}.json")))
        if t.get("verdict") != "NORMALIZED" or t.get("vocabulary_sha256") != vsha:
            problems.append(f"{cfg}: table not NORMALIZED over the frozen vocabulary")
        status[cfg] = {r["id"]: r for r in t["rows"]}
        sources[cfg] = f"{os.path.relpath(d, common.EV) if not tables else d} (this mission, original explorer)"
    if problems:
        print("AGGREGATE STOP:", problems)
        return 2
    # Cross-link material.
    tests = {}
    tests.update(tests_of(common.MODEL, common.TEST_PREFIX))
    tests.update(tests_of(common.UNIT, "ledger::protocol::unit_tests::"))
    inv = json.loads(common.blob(common.STAGE1, f"{common.STAGE1_PACKET}/receipts/30-suite/INVENTORY.json"))
    outcomes, ignored = inv["outcomes"], set(inv["ignored"])
    diag = {d["test"] for d in inv["diagnostics_51"]}
    rows = []
    for r in vocab["rows"]:
        taken = [c for c in ORDER if status[c][r["id"]]["status"] == "TAKEN"]
        nt = [c for c in ORDER if status[c][r["id"]]["status"] == "NOT TAKEN"]
        na = {c: status[c][r["id"]]["na_reason"] for c in ORDER if status[c][r["id"]]["status"] == "NOT APPLICABLE"}
        row = dict(id=r["id"], operation=r["operation"], label=r["label"], in_static_branches=r["in_static_branches"],
                   taken_in=taken, not_taken_in=nt, not_applicable_in=na, taken_by_a_required_run=bool(taken))
        if not taken:
            paths = sorted({f"{a}::{b}" for d in r["derivation"] for a, b in VARIANT.findall(d)})
            links = []
            method = METHOD.get(r["operation"])
            if paths and method:
                for name, body in sorted(tests.items()):
                    if re.search(method, body) and all(re.search(r"\b" + re.escape(pth) + r"\b", body) for pth in paths):
                        links.append(dict(test=name, stage1_suite_result=outcomes.get(name, "not executed"),
                                          ignored=name in ignored, one_of_the_51_diagnostics=name in diag))
            row["cross_link_variants"] = paths
            row["cross_link_method"] = method
            row["cross_links"] = links
        rows.append(row)
    ops = {}
    for row in rows:
        o = ops.setdefault(row["operation"], dict(rows=0, taken_by_a_required_run=0))
        o["rows"] += 1
        o["taken_by_a_required_run"] += row["taken_by_a_required_run"]
    agg = dict(tool="aggregate v2", vocabulary_sha256=vsha, configurations=ORDER, sources=sources, rows=rows,
               operations=ops, rows_taken_by_a_required_run=sum(r["taken_by_a_required_run"] for r in rows),
               rows_total=len(rows),
               static_omissions_taken=[r["label"] for r in rows if not r["in_static_branches"] and
                                       r["taken_by_a_required_run"]])
    os.makedirs(outdir, exist_ok=True)
    json.dump(agg, open(os.path.join(outdir, "AGGREGATE-20.json"), "w"), indent=1, ensure_ascii=False)
    code = lambda c, r: {"TAKEN": "T", "NOT TAKEN": ".", "NOT APPLICABLE": "na"}[status[c][r["id"]]["status"]]
    with open(os.path.join(outdir, "AGGREGATE-20.tsv"), "w") as f:
        f.write("id\toperation\tlabel\tstatic\t" + "\t".join(ORDER) + "\ttaken_by_a_required_run\n")
        for r in vocab["rows"]:
            row = next(x for x in rows if x["id"] == r["id"])
            f.write(f"{r['id']}\t{r['operation']}\t{r['label']}\t{'yes' if r['in_static_branches'] else 'NO'}\t" +
                    "\t".join(code(c, r) for c in ORDER) + f"\t{'yes' if row['taken_by_a_required_run'] else 'no'}\n")
    md = ["# Unchanged-core branch coverage — twenty explorations", "",
          f"Vocabulary: {len(rows)} source-bound rows (sha256 `{vsha}`). Columns: the twenty required unchanged-core "
          "explorations. `T` TAKEN, `.` NOT TAKEN, `na` NOT APPLICABLE (the whole operation absent by configuration).",
          "F-3, F-PT and F-B are the accepted composed-quotient reports of Stage 1 (their original graphs were not "
          "explored); all other columns are original-explorer reports.", "",
          f"Rows taken by at least one required run: **{agg['rows_taken_by_a_required_run']} of {len(rows)}**. "
          "Nothing requires every row to be taken; the obligation is complete and truthful accounting.", "",
          "## Per operation", "", "| operation | rows | taken by a required run |", "|---|---|---|"]
    md += [f"| {op} | {o['rows']} | {o['taken_by_a_required_run']} |" for op, o in ops.items()]
    md += ["", "## Matrix", "", "| id | label | static | " + " | ".join(ORDER) + " | any |",
           "|---|---|---|" + "---|" * len(ORDER) + "---|"]
    for r in vocab["rows"]:
        row = next(x for x in rows if x["id"] == r["id"])
        md.append(f"| {r['id']} | `{r['label']}` | {'yes' if r['in_static_branches'] else 'NO'} | " +
                  " | ".join(code(c, r) for c in ORDER) + f" | {'yes' if row['taken_by_a_required_run'] else 'no'} |")
    md += ["", "## Rows no required run takes, with textual cross-references", "",
           "A cross-reference is a suite test whose body calls the operation's protocol method and names every protocol "
           "outcome variant behind the label; it "
           "exercises the protocol outcome, not the explorer's label, and naming a variant does not prove exercising it.",
           ""]
    for row in rows:
        if row["taken_by_a_required_run"]:
            continue
        links = row.get("cross_links", [])
        md.append(f"- {row['id']} `{row['label']}` — variants {row.get('cross_link_variants') or 'none (a literal label)'};"
                  f" NOT APPLICABLE in {len(row['not_applicable_in'])} of 20; cross-references: "
                  + (", ".join(f"`{l['test'].split('::', 2)[-1]}` ({l['stage1_suite_result']}"
                               f"{'; one of the 51' if l['one_of_the_51_diagnostics'] else ''})" for l in links[:6])
                     + (f" and {len(links) - 6} more" if len(links) > 6 else "") if links else "none found"))
    md.append("")
    open(os.path.join(outdir, "AGGREGATE-20.md"), "w").write("\n".join(md))
    print(f"AGGREGATE PRODUCED: {agg['rows_taken_by_a_required_run']} of {len(rows)} rows taken by at least one of the "
          f"20 required runs; static omissions taken: {agg['static_omissions_taken']}")
    return 0


if __name__ == "__main__":
    a = sys.argv[1:]
    sys.exit(main(a[0], a[a.index("--tables") + 1] if "--tables" in a else None))
