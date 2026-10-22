#!/usr/bin/env python3
"""P2-V1-R3B-I3-I1-R3 control matrices, kept apart:

- behavioural controls: the counted store controls (by category) and the
  I2-R1 suite's counted controls and informational runs, from their as-run
  summaries;
- API/type guards: the compile-time probes, from api_probes.json.

No figure adds the two kinds, or two categories, together.

Usage: matrices.py <store summary.json> <i2r1 summary.json> <api_probes.json> <out dir>
"""
import json
import pathlib
import sys


def cell(text):
    return str(text).replace("|", "\\|").replace("\n", " ")


def main():
    if len(sys.argv) != 5:
        raise SystemExit(__doc__)
    store = json.loads(pathlib.Path(sys.argv[1]).read_text())
    i2r1 = json.loads(pathlib.Path(sys.argv[2]).read_text())
    probes = json.loads(pathlib.Path(sys.argv[3]).read_text())
    out = pathlib.Path(sys.argv[4])
    out.mkdir(parents=True, exist_ok=True)

    lines = ["# Behavioural controls (P2-V1-R3B-I3-I1-R3)", ""]
    s = store["summary"]
    lines.append(f"Store controls: {s['counted']} counted; all as required: {s['all_required']}; "
                 f"sources identical after the run: {s['identical']}; failures: {s['failures'] or 'none'}.")
    lines.append("")
    lines.append("By category (each counted separately):")
    lines.append("")
    for category, count in sorted(s["by_category"].items()):
        ok = sum(1 for r in store["counted"] if r["category"] == category and r["ok"])
        lines.append(f"- {category}: {ok} of {count} as required")
    for category in sorted(s["by_category"]):
        lines += ["", f"## {category}", "",
                  "| control | what it restores | files | intended test | marker | compiled | failed the test | marker present | restored | as required |",
                  "|---|---|---|---|---|---|---|---|---|---|"]
        for r in store["counted"]:
            if r["category"] != category:
                continue
            files = ", ".join(path.rsplit("/", 1)[-1] for path in r["files"])
            lines.append(f"| {r['id']} | {cell(r['what'])} | {files} | {r['test']} | {cell(r['marker'])} | "
                         f"{r['compiled']} | {r['failed_intended_test']} | {r['marker_in_assertion']} | "
                         f"{r['files_restored']} | {r['ok']} |")
    t = i2r1["summary"]
    lines += ["", "## I2-R1 suite (rerun with I3-I1's runner, unchanged)", "",
              f"Counted: {t['counted']}; all as required: {t['counted_all_required']}; informational runs: "
              f"{t['informational']}; files identical after the run: {t['identical']}.", "",
              "| control | intended test | as required |", "|---|---|---|"]
    for r in i2r1["counted"]:
        lines.append(f"| {r['id']} | {r.get('test', '-')} | {r.get('ok')} |")
    lines += ["", "| informational run | test | expected | compiled | failed | passed | restored |",
              "|---|---|---|---|---|---|---|"]
    for r in i2r1["informational"]:
        lines.append(f"| {r['id']} | {r.get('test', '-')} | {cell(r.get('expect', '-'))} | {r.get('compiled')} | "
                     f"{r.get('failed')} | {r.get('passed')} | {r.get('files_restored')} |")
    (out / "behavioural-controls.md").write_text("\n".join(lines) + "\n")

    lines = ["# API/type guards (P2-V1-R3B-I3-I1-R3)", "",
             "Compile-time probes from outside the store boundary. Each guard must fail to build with "
             "exactly its expected error and, once that one access is reopened in a scratch copy, build. "
             "They prove the safe API surface only; they are not behavioural detections and are not added "
             "to any behavioural figure.", ""]
    counts = {}
    for r in probes:
        counts.setdefault(r["category"], [0, 0])
        counts[r["category"]][0] += 1
        counts[r["category"]][1] += 1 if r["ok"] else 0
    for category, (total, ok) in sorted(counts.items()):
        lines.append(f"- {category}: {ok} of {total} as required")
    lines += ["", "| probe | category | boundary | base path it restates | attempt | expected error | guarded | self-check | as required |",
              "|---|---|---|---|---|---|---|---|---|"]
    for r in probes:
        lines.append(f"| {r['id']} | {r['category']} | {cell(r.get('boundary', '-'))} | {cell(r.get('base', '-'))} | "
                     f"{cell(r.get('attempt', '-'))} | {cell('; '.join(r.get('expect', r.get('errors', [])))) or '(none)'} | "
                     f"{r.get('guarded', '-')} | {r.get('self_check', '-')} | {r['ok']} |")
    (out / "api-type-guards.md").write_text("\n".join(lines) + "\n")
    print((out / "behavioural-controls.md").read_text().split("\n## ")[0])
    print("\n".join(l for l in lines if l.startswith("- ")))


if __name__ == "__main__":
    main()
