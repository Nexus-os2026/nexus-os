#!/usr/bin/env python3
"""P2-V1-R3B-I3-I1 coverage matrix.

Links the R5 invariants (INV-1 to INV-21), the R5 design controls (the 67
negative controls of the R5 Python model, from its coverage.json), the Rust
tests and their markers, and the counted Rust mutation controls (from
store_controls.py and its as-run summary). Every design control is listed
exactly once with its Rust counterpart, or as a historical Python-model
control when no Rust mutation restores it. Categories are kept apart and
never summed into one figure.

Usage: coverage_matrix.py <R5 coverage.json> <store_controls.py> <summary.json> <test file> <out dir>
"""
import json
import pathlib
import re
import sys

# The invariants (design section 12.1) and the Rust tests that hold them,
# with the markers their assertions carry. The controls column is derived
# from the controls whose intended test is listed here and whose marker is
# one of the invariant's markers.
INVARIANTS = [
    ("INV-1", "Acknowledged implies durable",
     ["r01", "r03", "r10", "n05", "n08"], ["[ack-durable]"]),
    ("INV-2", "Write-ahead: every admitted action has a durable ActionStarted",
     ["r03", "k02", "k03"], ["[admission-fence]", "[ack-durable]", "[grammar-conformance]"]),
    ("INV-3", "No false resolution",
     ["c01", "f02", "k02", "k03", "k04", "r08"], ["[no-false-resolution]", "[late-uncertain]"]),
    ("INV-4", "Nothing beyond P; failure delivered once at an issued record",
     ["r01", "r02", "r03", "r04", "r05", "r06", "r07", "r08", "r09"],
     ["[fatal-total]", "[fatal-latched]", "[dup-bounded]", "[ack-durable]", "[admission-fence]"]),
    ("INV-5", "Totality and bounds",
     ["c03", "c04", "m07", "f01", "f02", "f03"], ["[arith-bounds]", "[aggregate-bounds]", "[exact-bytes]"]),
    ("INV-6", "Determinism", ["c02"], ["[archive-binding]"]),
    ("INV-7", "Binding sensitivity", ["f07", "m02", "m04"], ["[archive-binding]"]),
    ("INV-8", "Disposition authority", ["f05", "f09", "m02", "m03", "m09"],
     ["[disposition-exact]", "[admin-crash]"]),
    ("INV-9", "Seal meaning", ["f02", "k01", "r08"], ["[exact-bytes]", "[grammar-conformance]", "[fatal-total]"]),
    ("INV-10", "Exclusion", ["r07", "o03", "m01", "n04", "a01"],
     ["[lock-retained]", "[busy-before-sync]", "[session-verify]"]),
    ("INV-11", "Report honesty", ["c01", "s02", "o13", "j06", "k04"],
     ["[late-uncertain]", "[errseq-reopen]", "[F1-F2-sync]", "[errseq-limit]", "[no-false-resolution]"]),
    ("INV-12", "Administrative atomicity", ["m02", "m03", "m04", "m05", "m08", "m09", "o05", "s04"],
     ["[admin-crash]", "[archive-binding]", "[metadata-schedules]", "[safe-open]"]),
    ("INV-13", "Admission fence", ["r02", "r03", "r04", "a01"], ["[admission-fence]"]),
    ("INV-14", "Selection authority", ["o04", "o11", "m04", "a01"], ["[provision-selection]"]),
    ("INV-15", "Session authority", ["m01", "o03"], ["[session-verify]"]),
    ("INV-16", "Bounded aggregates", ["c03", "m07", "m10"], ["[aggregate-bounds]"]),
    ("INV-17", "Protocol parity", ["m08", "m09"], ["[step-parity]", "[metadata-schedules]"]),
    ("INV-18", "Durable activation",
     ["o02", "o11", "j01", "j02", "j03", "j04", "j05", "j07", "n06"],
     ["[durable-activation]", "[activation-proof]", "[journal-conformance]", "[stable-completion]"]),
    ("INV-19", "History across recovery", ["x01", "x02", "m06", "m09"], ["[composed-recovery]"]),
    ("INV-20", "Supported profile", ["o06", "o07", "o08"], ["[supported-profile]", "[safe-open]"]),
    ("INV-21", "Storage admission", ["o09", "o10", "o12", "x03", "f08"],
     ["[storage-qualification]", "[storage-composed]"]),
]

# Mission requirements that are not R5 invariants.
REQUIREMENTS = [
    ("REQ-ENTRY", "The real configured-store entry is closed before any I/O; no switch", ["o01"], ["[real-entry-closed]"]),
    ("REQ-NATIVE", "Native primitives beneath CARGO_TARGET_TMPDIR: safe open, links, FIFO, flock, write and sync wiring, identity",
     ["n01", "n02", "n03", "n04", "n05", "n06", "n07", "n08"],
     ["[safe-open]", "[lock-retained]", "[ack-durable]", "[durable-activation]"]),
    ("REQ-SIM", "The simulator's state model: F1 versus F2, errseq, reclaim, schedules, tears",
     ["s01", "s02", "s03", "s04", "s05"],
     ["[F1-volatile]", "[F1-F2-sync]", "[errseq-reopen]", "[metadata-schedules]", "[tear-containment]"]),
]

HISTORICAL = {
    "NC04": (
        "The first candidate's geometry (128-byte slots sharing pages, the seal in block 0) "
        "has no Rust form: format.rs fixes one record per 4096-byte block and the seal in "
        "block 1, and a mutation restoring shared slots would be another container format. "
        "The Rust tests keep the assertions (s05 containment in the simulator, c01 a torn "
        "record never benign); S-TEAR-SPANS is the related simulator control and counts "
        "only for the simulator's containment, not for the geometry decision."
    ),
}


def load_controls(path):
    namespace = {"__name__": "store_controls"}
    exec(compile(pathlib.Path(path).read_text(), path, "exec"), namespace)
    return namespace["COUNTED"]


def main():
    if len(sys.argv) != 6:
        raise SystemExit(__doc__)
    coverage = json.loads(pathlib.Path(sys.argv[1]).read_text())
    controls = load_controls(sys.argv[2])
    summary = json.loads(pathlib.Path(sys.argv[3]).read_text())
    tests_src = pathlib.Path(sys.argv[4]).read_text()
    out = pathlib.Path(sys.argv[5])
    out.mkdir(parents=True, exist_ok=True)
    results = {r["id"]: r for r in summary["counted"]}
    tests = {m.group(1).split("_")[0]: m.group(1)
             for m in re.finditer(r"\n#\[test\]\nfn ([a-z]\d\d_[a-z0-9_]+)\(", tests_src)}
    design = coverage["negative_controls"]
    design_ids = [c["id"] for c in design]
    assert len(design_ids) == 67 and len(set(design_ids)) == 67, "67 design controls"
    by_design = {}
    for c in controls:
        for d in c["design"]:
            if d not in design_ids:
                raise SystemExit(f"{c['id']} names an unknown design control {d}")
            by_design.setdefault(d, []).append(c["id"])
    rows = []
    for d in design:
        rust = by_design.get(d["id"], [])
        if rust:
            status = "rust-mutation"
        elif d["id"] in HISTORICAL:
            status = "historical-python-model"
        else:
            raise SystemExit(f"{d['id']} has neither a Rust control nor a historical note")
        rows.append(dict(
            design=d["id"], check=d["target"], restores=d["restores"],
            python_assertion=d["assertion"], python_result=d["result"],
            status=status, rust_controls=rust,
            rust_tests=sorted({c["test"] for c in controls if c["id"] in rust}),
            rust_markers=sorted({c["marker"] for c in controls if c["id"] in rust}),
            rust_categories=sorted({c["category"] for c in controls if c["id"] in rust}),
            rust_ok=[results.get(cid, {}).get("ok") for cid in rust],
            note=HISTORICAL.get(d["id"], ""),
        ))
    # Every counted control must have run, compiled, failed its intended
    # test at its marker, and restored the bytes.
    missing = [c["id"] for c in controls if c["id"] not in results]
    bad = [cid for cid, r in results.items() if not r["ok"]]
    if missing or bad:
        raise SystemExit(f"controls not run {missing} or not as required {bad}")
    inv_rows = []
    for inv, title, inv_tests, markers in INVARIANTS + REQUIREMENTS:
        names = [tests.get(t) for t in inv_tests]
        if None in names:
            raise SystemExit(f"{inv}: unknown test in {inv_tests}")
        for t in inv_tests:
            body = tests_src[tests_src.index(f"fn {tests[t]}("):]
            nxt = body.find("\n#[test]\n")
            body = body if nxt < 0 else body[:nxt]
            if not any(m in body for m in markers):
                raise SystemExit(f"{inv}: {t} carries none of {markers}")
        ctl = sorted(c["id"] for c in controls
                     if c["test"].split("_")[0] in inv_tests and c["marker"] in markers)
        inv_rows.append(dict(id=inv, title=title, tests=names, markers=markers, controls=ctl))
    categories = {}
    for c in controls:
        categories.setdefault(c["category"], []).append(c["id"])
    data = dict(
        design_controls=len(rows),
        design_with_rust_mutation=sum(r["status"] == "rust-mutation" for r in rows),
        design_with_api_surface_counterpart_only=sorted(
            r["design"] for r in rows
            if r["status"] == "rust-mutation" and r["rust_categories"] == ["authority-api-surface"]),
        design_historical=sum(r["status"] == "historical-python-model" for r in rows),
        rust_controls_by_category={k: len(v) for k, v in sorted(categories.items())},
        rust_controls_without_design_id=sorted(c["id"] for c in controls if not c["design"]),
        design=rows, invariants=inv_rows,
    )
    (out / "coverage-matrix.json").write_text(json.dumps(data, indent=2) + "\n")
    md = []
    md.append("# P2-V1-R3B-I3-I1 coverage matrix\n")
    md.append("Generated by `coverage_matrix.py` from the R5 model's `coverage.json`, "
              "`store_controls.py` and its as-run `summary.json`, and the store test target. "
              "Categories are counted separately and never added together.\n")
    md.append("## Counts\n")
    md.append(f"- R5 design controls: {len(rows)}; with a counted Rust mutation: "
              f"{data['design_with_rust_mutation']}; historical Python-model only: "
              f"{data['design_historical']}.")
    md.append("- Of the design controls with a Rust mutation, these have only an "
              "authority-api-surface counterpart (a source or type guard, not behaviour): "
              + ", ".join(data["design_with_api_surface_counterpart_only"]) + ".")
    for k, v in sorted(categories.items()):
        md.append(f"- Rust store controls, {k}: {len(v)}.")
    md.append(f"- Rust store controls with no R5 design control (Rust-specific): "
              f"{len(data['rust_controls_without_design_id'])}: "
              + ", ".join(data["rust_controls_without_design_id"]) + ".")
    md.append("- The I2-R1 custody and codec controls (32 counted, 4 informational) are a "
              "separate suite, rerun unchanged; see `controls/i2r1-rerun/`.\n")
    md.append("## R5 design controls\n")
    md.append("| Design | Check | Restores | Status | Rust controls | Rust test(s) | Marker(s) |")
    md.append("|---|---|---|---|---|---|---|")
    for r in rows:
        md.append("| {} | {} | {} | {} | {} | {} | {} |".format(
            r["design"], r["check"], r["restores"].replace("|", "/"),
            r["status"], ", ".join(r["rust_controls"]) or "none",
            ", ".join(t.split("_")[0] for t in r["rust_tests"]) or "-",
            ", ".join(r["rust_markers"]) or "-"))
    md.append("")
    for r in rows:
        if r["note"]:
            md.append(f"- **{r['design']}**: {r['note']}")
    md.append("\n## Invariants and requirements\n")
    md.append("| Id | Invariant | Rust tests | Markers | Counted Rust controls on those tests |")
    md.append("|---|---|---|---|---|")
    for r in inv_rows:
        md.append("| {} | {} | {} | {} | {} |".format(
            r["id"], r["title"], ", ".join(t.split("_")[0] for t in r["tests"]),
            ", ".join(r["markers"]), ", ".join(r["controls"]) or "none (see note)"))
    md.append("")
    md.append("A control appears under an invariant when its intended test is one of the "
              "invariant's tests and its marker one of the invariant's markers; a control may "
              "appear under more than one invariant. INV-2's write-ahead rule rests on the "
              "core's own admission check, whose control is the I2-R1 suite's I1-NC5a.")
    md.append("\n## Rust store controls\n")
    md.append("| Control | Category | Design | Restores | Files | Test | Marker | Result |")
    md.append("|---|---|---|---|---|---|---|---|")
    for c in controls:
        r = results[c["id"]]
        md.append("| {} | {} | {} | {} | {} | {} | {} | {} |".format(
            c["id"], c["category"], ", ".join(c["design"]) or "-", c["what"].replace("|", "/"),
            ", ".join(f.rsplit("/", 1)[-1] for f in r["files"]), c["test"].split("_")[0],
            c["marker"], "caught, restored" if r["ok"] else "NOT AS REQUIRED"))
    (out / "coverage-matrix.md").write_text("\n".join(md) + "\n")
    print(json.dumps({k: data[k] for k in ("design_controls", "design_with_rust_mutation",
                                            "design_historical", "rust_controls_by_category")}))


if __name__ == "__main__":
    main()
