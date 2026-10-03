#!/usr/bin/env python3
"""Compare two complete sets of P2-V1-R3B-I4-R1 candidate runs (the
pre-commit snapshot's and the post-commit checkout's): the same commands
with the same exit statuses, the same unit tests with the same results, the
same validated source bytes, and the same outcome for every behavioural
control, API/type guard, source guard and rerun of an accepted suite.
Timings, paths and assertion texts are not compared (they legitimately
differ between runs); the stability counts are reported.

Usage: compare_runs.py <runs A> <runs B>
"""
import json
import pathlib
import re
import sys


def exits(text):
    return dict(re.findall(r"^(\S+) exit=(\d+) ", text, re.M))


def tests(path):
    return dict(re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored)$", path.read_text(), re.M))


def records(path, key, fields):
    data = json.loads(path.read_text())
    rows = data[key] if key else data
    return {r["id"]: {f: r.get(f) for f in fields} for r in rows}


def main():
    a, b = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    report, differences = [], []

    def same(name, x, y):
        equal = x == y
        report.append(f"{'same' if equal else 'DIFFERENT'} {name}")
        if not equal:
            differences.append(name)
            report.append(f"    A: {x}\n    B: {y}")

    same("step exits", exits((a / "runs.txt").read_text()), exits((b / "runs.txt").read_text()))
    same("validation command exits", exits((a / "validation/commands.txt").read_text()),
         exits((b / "validation/commands.txt").read_text()))
    same("validated sources", (a / "validation/sources.SHA256SUMS").read_text(),
         (b / "validation/sources.SHA256SUMS").read_text())
    for name in ("lib-tests", "store-tests", "codec-tests", "core-tests", "cleanup-observation",
                 "package-layout"):
        same(f"test results: {name}", tests(a / f"validation/{name}.txt"), tests(b / f"validation/{name}.txt"))
    fields = ("category", "test", "marker", "mapping", "test_passes_unmutated", "compiled",
              "failed_intended_test", "marker_in_assertion", "files_restored", "ok")
    same("behavioural controls", records(a / "controls/scope/summary.json", "counted", fields),
         records(b / "controls/scope/summary.json", "counted", fields))
    gfields = ("kind", "family", "mapping", "expected", "errors", "exact", "self_check_builds",
               "sources_restored", "ok")
    same("API/type guards", records(a / "controls/api-guards/summary.json", "guards", gfields),
         records(b / "controls/api-guards/summary.json", "guards", gfields))
    nfields = ("kind", "expected", "errors", "exact", "self_check_builds", "ok")
    same("normal-build API guards", records(a / "controls/normal-api-probes/summary.json", "guards", nfields),
         records(b / "controls/normal-api-probes/summary.json", "guards", nfields))
    same("normal-build positive control",
         json.loads((a / "controls/normal-api-probes/summary.json").read_text())["positive"]["ok"],
         json.loads((b / "controls/normal-api-probes/summary.json").read_text())["positive"]["ok"])
    sa = json.loads((a / "controls/source-guards.json").read_text())
    sb = json.loads((b / "controls/source-guards.json").read_text())
    same("source guards", (sa["guards"], [t["detected"] for t in sa["self_tests"]]),
         (sb["guards"], [t["detected"] for t in sb["self_tests"]]))
    sfields = ("category", "test", "marker", "compiled", "failed_intended_test", "marker_in_assertion",
               "required_present", "forbidden_present", "files_restored", "ok")
    same("R3 store controls rerun", records(a / "controls/r3-store/summary.json", "counted", sfields),
         records(b / "controls/r3-store/summary.json", "counted", sfields))
    ifields = ("ok", "compiled", "failed", "passed", "files_restored", "expect", "marker_in_assertion")
    same("I2-R1 rerun", records(a / "controls/i2r1-rerun/summary.json", "counted", ifields),
         records(b / "controls/i2r1-rerun/summary.json", "counted", ifields))
    afields = ("category", "expect", "exit", "errors", "guarded", "reopened_exit", "reopened_errors", "ok")
    same("R3 API probes rerun", records(a / "controls/r3-api-probes/api_probes.json", None, afields),
         records(b / "controls/r3-api-probes/api_probes.json", None, afields))
    same("no-live verdict", (a / "no-live/verdict.txt").read_text().strip().splitlines()[-1],
         (b / "no-live/verdict.txt").read_text().strip().splitlines()[-1])
    for side, root in (("A", a), ("B", b)):
        last = (root / "stability/stability.txt").read_text().strip().splitlines()[-1]
        report.append(f"stability {side}: {last}")
    report.append("RESULT: " + ("IDENTICAL" if not differences else f"DIFFERENT {differences}"))
    print("\n".join(report))
    return 0 if not differences else 1


if __name__ == "__main__":
    sys.exit(main())
