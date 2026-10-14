#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01: human-readable renderings of the frozen vocabulary.

  render_vocab.py        writes vocabulary/VOCABULARY.tsv and vocabulary/STATIC-OMISSIONS.txt from VOCABULARY.json

Mechanical only: every value comes from VOCABULARY.json.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common  # noqa: E402


def main():
    vdir = os.path.join(common.EV, "vocabulary")
    path = os.path.join(vdir, "VOCABULARY.json")
    v = json.load(open(path))
    vsha = common.sha256_file(path)
    with open(os.path.join(vdir, "VOCABULARY.tsv"), "w") as f:
        f.write("id\toperation\tlabel\tin_static_BRANCHES\tstatic_index\tsites\tsource_items\tsource_expressions\t"
                "derivation\tnote\n")
        for r in v["rows"]:
            sites = "; ".join(f"{s['site']} {s['function']}:{s['line']}" for s in r["sites"])
            f.write("\t".join([r["id"], r["operation"], r["label"], "yes" if r["in_static_branches"] else "NO",
                               "" if r["static_index"] is None else str(r["static_index"]), sites,
                               ", ".join(r["source_items"]), " || ".join(r["source_expressions"]),
                               " || ".join(r["derivation"]), r["note"]]) + "\n")
    miss = [r for r in v["rows"] if not r["in_static_branches"]]
    lines = [f"SOURCE-EMITTABLE LABELS MISSING FROM THE STATIC BRANCHES CONSTANT (model_tests.rs at {common.C})",
             f"vocabulary sha256 {vsha}; {len(v['rows'])} rows; static BRANCHES {len(v['static_branches'])} labels, every "
             f"one a vocabulary row ({'yes' if not v['static_missing_from_vocabulary'] else 'NO: ' + str(v['static_missing_from_vocabulary'])}); "
             f"rows missing from BRANCHES: {len(miss)}",
             "The explorer prints a taken label of this list as '[taken    ] <label> (not in the listed set)' and does not "
             "print it when not taken.", ""]
    cur = None
    for r in miss:
        if r["operation"] != cur:
            cur = r["operation"]
            n = sum(1 for x in miss if x["operation"] == cur)
            lines.append(f"{cur} ({n}):")
        lines.append(f"  {r['id']}  {r['label']}" + (f"   [{r['note']}]" if r["note"] else ""))
    lines.append("")
    lines.append(f"TOTAL: {len(miss)} source-emittable labels are missing from the static BRANCHES constant.")
    open(os.path.join(vdir, "STATIC-OMISSIONS.txt"), "w").write("\n".join(lines) + "\n")
    print(f"rendered VOCABULARY.tsv ({len(v['rows'])} rows) and STATIC-OMISSIONS.txt ({len(miss)} labels)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
