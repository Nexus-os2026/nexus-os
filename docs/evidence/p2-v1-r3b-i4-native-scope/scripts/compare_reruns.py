#!/usr/bin/env python3
"""Compare an I4 rerun of an accepted R3 suite with R3's as-run results,
entry by entry (the accepted custody store is not part of this mission;
its suites are rerun unchanged to show the candidate leaves them as they
were accepted).

- store: R3's store controls (docs/evidence/p2-v1-r3b-i3-i1-r3/controls/
  store/summary.json): the same counted controls with the same outcome
  fields, and the same source hashes (`original`) of every file the suite
  pins.
- api: R3's compile-time API probes (.../controls/api-probes/
  api_probes.json): the same probes with the same results, error lines and
  self-checks.

Usage: compare_reruns.py store|api <R3 result> <rerun result>
"""
import json
import sys

STORE_FIELDS = ("category", "test", "marker", "compiled", "failed_intended_test",
                "marker_in_assertion", "required_present", "forbidden_present",
                "files_restored", "ok")
API_FIELDS = ("category", "expect", "exit", "errors", "guarded", "reopened_exit",
              "reopened_errors", "variant_exit", "variant_errors", "ok")


def main():
    if len(sys.argv) != 4 or sys.argv[1] not in ("store", "api"):
        raise SystemExit(__doc__)
    kind, reference_path, rerun_path = sys.argv[1:]
    reference = json.load(open(reference_path))
    rerun = json.load(open(rerun_path))
    differences = []
    if kind == "store":
        fields = STORE_FIELDS
        ref_rows = {r["id"]: r for r in reference["counted"]}
        new_rows = {r["id"]: r for r in rerun["counted"]}
        same_sources = reference["summary"]["original"] == rerun["summary"]["original"]
        print(f"pinned sources identical to R3's: {'yes' if same_sources else 'NO'} "
              f"({len(reference['summary']['original'])} files)")
        if not same_sources:
            differences.append("pinned sources")
        for key in ("identical", "all_required", "counted", "by_category", "failures"):
            same = reference["summary"].get(key) == rerun["summary"].get(key)
            print(f"summary {key}: R3 {reference['summary'].get(key)} rerun {rerun['summary'].get(key)}"
                  f" {'same' if same else 'DIFFERENT'}")
            if not same:
                differences.append(f"summary {key}")
    else:
        fields = API_FIELDS
        ref_rows = {r["id"]: r for r in reference}
        new_rows = {r["id"]: r for r in rerun}
    print(f"reference: {reference_path} ({len(ref_rows)} entries)")
    print(f"rerun:     {rerun_path} ({len(new_rows)} entries)")
    for missing in sorted(set(ref_rows) ^ set(new_rows)):
        differences.append(f"only one side: {missing}")
    for cid in sorted(set(ref_rows) & set(new_rows)):
        a = {f: ref_rows[cid].get(f) for f in fields}
        b = {f: new_rows[cid].get(f) for f in fields}
        same = a == b
        print(f"  {'same' if same else 'DIFFERENT'} {cid}")
        if not same:
            differences.append(cid)
    print("RESULT:", "IDENTICAL" if not differences else f"DIFFERENT {differences}")
    return 0 if not differences else 1


if __name__ == "__main__":
    sys.exit(main())
