#!/usr/bin/env python3
"""P2-V1-R3B-I3-I1-R1 old-to-new mapping of the store controls.

Loads the I3-I1 control list (docs/evidence/p2-v1-r3b-i3-i1/scripts/
store_controls.py, unchanged) and the R1 list (this directory's
store_controls.py) by path, and states for every I3-I1 control what became
of it in R1: unchanged (the same edits, test, marker and checks), adapted
(each changed field named), or remapped (not run as a behavioural control;
the API/type guards that carry its intention). Every R1 control that has no
I3-I1 counterpart is listed as added. With an as-run summary.json it also
gives each R1 control's result.

Usage: control_mapping.py <I3-I1 store_controls.py> <R1 store_controls.py> <out dir> [summary.json]
"""
import json
import pathlib
import sys

FIELDS = ["category", "design", "what", "edits", "test", "marker", "require", "forbid"]


def load(path):
    namespace = {"__name__": "store_controls"}
    exec(compile(pathlib.Path(path).read_text(), str(path), "exec"), namespace)
    return namespace


def main():
    if len(sys.argv) not in (4, 5):
        raise SystemExit(__doc__)
    old = load(sys.argv[1])
    new = load(sys.argv[2])
    out = pathlib.Path(sys.argv[3])
    out.mkdir(parents=True, exist_ok=True)
    results = {}
    if len(sys.argv) == 5:
        summary = json.loads(pathlib.Path(sys.argv[4]).read_text())
        results = {r["id"]: r["ok"] for r in summary["counted"]}
    new_by_id = {c["id"]: c for c in new["COUNTED"]}
    remapped = {c["id"]: c for c in new.get("REMAPPED", [])}
    remapped_to = new.get("REMAPPED_TO", {})
    rows = []
    for c in old["COUNTED"]:
        cid = c["id"]
        if cid in remapped:
            same = all(remapped[cid][f] == c[f] for f in FIELDS)
            rows.append(dict(old=cid, new=None, status="remapped to API/type guards",
                             changed=[] if same else [f for f in FIELDS if remapped[cid][f] != c[f]],
                             guards=remapped_to.get(cid, []), category=c["category"],
                             result=None))
            continue
        n = new_by_id.get(cid)
        if n is None:
            rows.append(dict(old=cid, new=None, status="MISSING", changed=[], guards=[],
                             category=c["category"], result=None))
            continue
        changed = [f for f in FIELDS if n[f] != c[f]]
        rows.append(dict(old=cid, new=cid, status="adapted" if changed else "unchanged",
                         changed=changed, guards=[], category=n["category"],
                         result=results.get(cid)))
    old_ids = {c["id"] for c in old["COUNTED"]}
    for c in new["COUNTED"]:
        if c["id"] not in old_ids:
            rows.append(dict(old=None, new=c["id"], status="added (R1)", changed=[], guards=[],
                             category=c["category"], result=results.get(c["id"])))
    counts = {}
    for r in rows:
        counts[r["status"]] = counts.get(r["status"], 0) + 1
    (out / "control_mapping.json").write_text(json.dumps(dict(rows=rows, counts=counts), indent=2) + "\n")
    lines = [
        "# Store controls: I3-I1 to R1 mapping",
        "",
        f"I3-I1 controls: {len(old['COUNTED'])}. R1 counted controls: {len(new['COUNTED'])}. "
        f"Remapped (not run, carried by API/type guards): {len(remapped)}.",
        "",
        "| I3-I1 control | R1 control | status | changed fields | category | R1 result |",
        "|---|---|---|---|---|---|",
    ]
    for r in rows:
        detail = ", ".join(r["changed"]) or "-"
        if r["guards"]:
            detail = "carried by: " + "; ".join(r["guards"])
        result = "-" if r["result"] is None else ("as required" if r["result"] else "NOT AS REQUIRED")
        lines.append(f"| {r['old'] or '-'} | {r['new'] or '-'} | {r['status']} | {detail} | "
                     f"{r['category']} | {result} |")
    lines.append("")
    for status, count in sorted(counts.items()):
        lines.append(f"- {status}: {count}")
    (out / "control_mapping.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines[-len(counts):]))
    missing = [r for r in rows if r["status"] == "MISSING"]
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
