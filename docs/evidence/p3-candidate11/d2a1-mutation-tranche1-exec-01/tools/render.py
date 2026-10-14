#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 19: render INDEX.md and TRANCHE-SUMMARY.md from the
receipts (read-only; deterministic; a missing receipt is rendered as not attempted, never invented).

    render.py OUTDIR       writes OUTDIR/INDEX.md and OUTDIR/TRANCHE-SUMMARY.md; prints the derived tranche status
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

R = os.path.join(K.EV, "receipts")
CANDIDATE = "CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"
RESULT_OK = "CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"


def lines(path):
    return open(path).read().splitlines() if os.path.exists(path) else []


def jload(path):
    return json.load(open(path)) if os.path.exists(path) else None


def outer(path):
    ls = lines(path)
    return (ls[0] if ls else "NOT PRODUCED", ls[1].replace("outer class (AD-1-R4 K.2): ", "") if len(ls) > 1 else "")


def res(sup):
    if not sup:
        return "—"
    mib = int(sup["memory_peak_bytes"]) / 2 ** 20 if sup.get("memory_peak_bytes") else None
    return (f"wall {sup.get('wall_seconds')} s / {sup.get('time_limit_seconds')} s; scope peak "
            f"{mib:.1f} MiB / 16 GiB; swap peak {sup.get('swap_peak_bytes')} B; oom_kill {sup.get('oom_kill')}"
            if mib is not None else "—")


def status(exp):
    base_ok = all(lines(os.path.join(K.EV, b["dir"], "CLASSIFICATION.txt"))[:1] == ["CLASSIFICATION: PASS"]
                  for b in exp["baselines"])
    healthy_ok = lines(os.path.join(R, "20-healthy-build/CLASSIFICATION.txt"))[:1] == ["CLASSIFICATION: PASS"]
    results = [lines(os.path.join(K.EV, exp["controls"][c]["dir"], "RESULT.txt"))[:1] for c in K.ORDER]
    if not (base_ok and healthy_ok):
        return "MUTATION_TRANCHE1_BLOCKED", base_ok, healthy_ok, results
    if all(r == [RESULT_OK] for r in results):
        return "READY_FOR_MUTATION_TRANCHE1_EXECUTION_REVIEW", base_ok, healthy_ok, results
    return "MUTATION_CONTROL_BLOCKED", base_ok, healthy_ok, results


def control_row(exp, cid):
    c = exp["controls"][cid]
    d = os.path.join(K.EV, c["dir"])
    res_first = lines(os.path.join(d, "RESULT.txt"))[:1]
    if not os.path.exists(d):
        return None
    build = jload(os.path.join(d, "03-build/PARSED-RECEIPT.json")) or {}
    bcls = outer(os.path.join(d, "03-build/CLASSIFICATION.txt"))
    run = jload(os.path.join(d, "04-run/PARSED-RECEIPT.json"))
    rcls = outer(os.path.join(d, "04-run/CLASSIFICATION.txt")) if run else None
    pin = jload(os.path.join(d, "03-build/PIN.json"))
    restore = lines(os.path.join(d, "05-restore/RESTORE.txt"))
    link = lines(os.path.join(d, "06-link/LINK.txt"))
    row = {"id": cid, "dir": c["dir"], "result": res_first[0] if res_first else "NOT COMPLETED",
           "type": c["receipt_type"], "patch_sha256": c["patch"]["sha256"], "mutant_blob": c["mutant_blob"],
           "mutant_sha256": c["mutant_sha256"],
           "build": f"{bcls[0]} — {bcls[1]}", "build_resources": res(build.get("supervisor")),
           "executable": (f"{pin['path']} sha256 {pin['sha256']} size {pin['size']}" if pin else
                          ("NOT PRODUCED BY DESIGN" if c["kind"] == "build" else "not produced")),
           "selection": " ".join(c["argv_tail"]) if c.get("argv_tail") else "the build command (no test selection by "
                                                                             "design)",
           "restore": restore[0] if restore else "NOT PRODUCED", "link": (link[0] + " — " + link[1]) if link else "—"}
    if c["kind"] == "build":
        row["raw"] = f"build exit {build.get('process', {}).get('exit_status')}; {build.get('diagnostics')}"
        row["named"] = f"E0283 at {c['diagnostic_file']}:{c['diagnostic_line']} (T-OWN)"
        row["count"] = "not applicable"
        row["outer"] = bcls[1]
        row["run_resources"] = "NOT PRODUCED BY DESIGN"
    else:
        row["named"] = c.get("named_property") or f"{c['named_test_id']} ({c['test']})"
        if run:
            rep = run.get("report") or {}
            row["raw"] = (f"outcome {rep.get('outcome')}; FAIL {rep.get('fail')}" if c["kind"] == "explore" else
                          f"panic at {run.get('panic_location')}: {' | '.join(run.get('panic_message') or [])}")
            row["count"] = (f"{rep.get('states'):,} of {c['ceiling_states']:,}" if rep.get("states") is not None
                            else "not applicable" if c["kind"] == "unit" else "not printed")
            row["outer"] = rcls[1]
            row["run_resources"] = res(run.get("supervisor"))
        else:
            row.update(raw="not run", count="—", outer=bcls[1] if bcls[0] != "BUILD: PINNED" else "not run",
                       run_resources="—")
    return row


def main(outdir):
    exp = json.load(open(os.path.join(R, "02-EXPECTATIONS.json")))
    st, base_ok, healthy_ok, results = status(exp)
    rows = [r for r in (control_row(exp, c) for c in K.ORDER) if r]
    attempted = [r["id"] for r in rows]
    candidates = [r["id"] for r in rows if r["result"] == RESULT_OK]
    s = [f"# Tranche summary — {K.MISSION}", "",
         f"Derived tranche status: **{st}** (from the receipts; publication and the final report follow).", "",
         f"KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING: **{len(candidates)}** of 12 authorized controls "
         f"({', '.join(candidates) or 'none'}). No control is called killed or accepted: acceptance belongs to the Sole "
         "Architect, receipt by receipt.", "",
         "## Baseline standalone receipts (mission section 7) and the healthy build (section 8)", "",
         "| Step | Type | Receipt | Classification | Facts | Resources |", "|---|---|---|---|---|---|"]
    for b in exp["baselines"]:
        d = os.path.join(K.EV, b["dir"])
        cl = outer(os.path.join(d, "CLASSIFICATION.txt"))
        p = jload(os.path.join(d, "PARSED-RECEIPT.json")) or {}
        fact = ""
        if b["type"] == "e1" and p.get("report"):
            fact = (f"states {p['report'].get('states'):,} of {b['ceiling_states']:,}; branch accounting "
                    f"{(p.get('branch_accounting') or {}).get('verdict')}")
        elif p.get("harness_result"):
            fact = p["harness_result"][0]
        s.append(f"| {b['key']} (for {b['control']}) | ({b['type']}) | `{b['dir']}` | {cl[0]} — {cl[1]} | {fact} | "
                 f"{res(p.get('supervisor'))} |")
    hb = jload(os.path.join(R, "20-healthy-build/PARSED-RECEIPT.json")) or {}
    hcl = outer(os.path.join(R, "20-healthy-build/CLASSIFICATION.txt"))
    exe = hb.get("executable") or {}
    s.append(f"| healthy build (for C22) | (e4) | `receipts/20-healthy-build` | {hcl[0]} — {hcl[1]} | executable "
             f"{exe.get('path')} sha256 {exe.get('sha256')} size {exe.get('size')} (recorded, never run); "
             f"{(hb.get('target_manifest') or {}).get('files')} target files recorded | {res(hb.get('supervisor'))} |")
    s += ["", "## Controls (mission section 9 order)", ""]
    for r in rows:
        s += [f"### {r['id']} — `{r['dir']}`", "", f"- result: **{r['result']}**", f"- receipt type: {r['type']}",
              f"- frozen patch sha256: `{r['patch_sha256']}`", f"- mutant source blob: `{r['mutant_blob']}` (sha256 "
              f"`{r['mutant_sha256']}`; computed in-process, never written to any object store)",
              f"- build: {r['build']}", f"- build resources: {r['build_resources']}",
              f"- mutant executable: {r['executable']}", f"- exact selection: `{r['selection']}`",
              f"- raw outcome: {r['raw']}", f"- outer classification: {r['outer']}",
              f"- named property / test / diagnostic: {r['named']}", f"- discovered count: {r['count']}",
              f"- run resources: {r['run_resources']}", f"- restoration: {r['restore']}",
              f"- linked restored-side receipt: {r['link']}", ""]
    not_attempted = [c for c in K.ORDER if c not in attempted]
    s += ["## Not attempted", "",
          f"- authorized but not attempted (a stop ends the tranche): {', '.join(not_attempted) or 'none'}",
          f"- not authorized by this mission (never executed): {', '.join(K.NOT_AUTHORIZED)}; A4 not rerun; M01–M183 "
          "not executed; no unchanged-core full configuration run except the standalone X-1; F-B2 and F-G not rerun on "
          "unchanged source", ""]
    open(os.path.join(outdir, "TRANCHE-SUMMARY.md"), "w").write("\n".join(s) + "\n")
    fr = jload(os.path.join(R, "05-FREEZE.json")) or {}
    i = [f"# {K.MISSION} — evidence packet", "",
         "Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. Evidence only: no source, catalog, "
         "patch, contract, amendment or manifest is changed; no mutant source, executable or target directory is "
         "committed.", "",
         f"Derived tranche status: **{st}**. Candidate kill evidence: {len(candidates)} of 12 "
         f"({', '.join(candidates) or 'none'}), each **KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**.", "",
         "## Identities", "",
         f"- Candidate C `{K.C}`, tree `{K.C_TREE}`, parent `{K.C_PARENT}`; protocol.rs `{K.BLOBS[K.PROTOCOL]}`, "
         f"model_tests.rs `{K.BLOBS[K.MODEL]}`, unit_tests.rs `{K.BLOBS[K.UNIT]}`",
         f"- frozen catalog `{K.CATALOG_PATH}` blob `{K.CATALOG_BLOB}` sha256 `{K.CATALOG_SHA}` at `{K.R2}` "
         f"({K.R2_REF}); definitions record blob `{K.DEFS_BLOB}` sha256 `{K.DEFS_SHA}`",
         f"- base contract sha256 `{K.CONTRACT_SHA}`; AD-1-R4 sha256 `{K.AD1R4_SHA}`; AD-1-R5 sha256 `{K.AD1R5_SHA}` "
         f"(R5 evidence commit `{K.R5_COMMIT}`)",
         f"- accepted unchanged-core evidence: Stage 1 `{K.STAGE1}`, Stage 2 `{K.STAGE2}`",
         f"- pinned unchanged-baseline executable `{K.EXE}` sha256 `{K.EXE_SHA256}` size {K.EXE_SIZE:,}",
         f"- evidence commit parent: `{K.R2}`; review ref `{K.DEST}`", "",
         "## Contents", "",
         "| Path | What |", "|---|---|",
         "| `INDEX.md` | this file |",
         "| `TRANCHE-SUMMARY.md` | every baseline, the healthy build and every attempted control, with identities, "
         "outcomes, classifications, resources, restoration and links |",
         "| `ARCHITECT-RULING-CATALOG-FREEZE.md` | the catalog-freeze Architect ruling, as received, and the frozen "
         "catalog's identity |",
         "| `receipts/00-authentication.txt` | full provenance: every input authenticated before any worktree or "
         "command |",
         "| `receipts/01-supervision-selftest/` | the enforcement chain proven on non-Nexus processes |",
         "| `receipts/02-EXPECTATIONS.json`, `02-expectations.txt` | every selection and expectation derived from the "
         "catalog and C's sources, cross-checked |",
         "| `receipts/03-tool-controls/` | classifiers and supervisor exercised on synthetic and historical evidence "
         "(positive and negative controls) |",
         "| `receipts/04-rehearsal/` | worktree, apply, verify, restore and link steps rehearsed with a synthetic patch "
         "in a development worktree (nothing built or run) |",
         "| `receipts/05-FREEZE.json`, `05-FREEZE-RECORD.txt` | the freeze: sha256 of every frozen tool, input and "
         "expectation, before the first baseline invocation |",
         "| `receipts/10-baseline-X-1/` … `15-baseline-T-RES/` | the standalone baseline receipts on the pinned "
         "executable: X-1 (e1), T-BIND, T-CELL, W6, T-PREC, T-RES (e2) |",
         "| `receipts/19-healthy-worktree/`, `20-healthy-build/` | the exact-command healthy build (e4) with its "
         "complete compiler output and target manifest |",
         "| `receipts/31-C22/` … `42-C18b/` | one directory per attempted control: gate, worktree, apply, build, run, "
         "restore, link, RESULT |",
         "| `receipts/90-final-gate.txt` | the final gate |",
         "| `tools/` | the minimal reproducible tooling (Stage-2's configs.py, normalize.py, report.py, verify_tree.py and "
         "scope-wrap.sh byte for byte; the frozen vocabulary is read from the Stage-2 evidence commit) |",
         "| `development/` | the development log, every defective tool version and its repair diff, failing "
         "development outputs |",
         "| `SHA256SUMS` | every packet file but itself |", "",
         "## Freeze", "",
         f"Frozen files: {len(fr.get('files', {}))}; recorded {fr.get('recorded')}; freeze record sha256 "
         f"`{K.sha256_file(os.path.join(R, '05-FREEZE.json')) if os.path.exists(os.path.join(R, '05-FREEZE.json')) else 'none'}`. "
         "No frozen file was changed after the freeze; every supervised step re-checks it.", "",
         "## Limitations", "",
         "- Classifications are executor candidates for Architect review; no kill is accepted here.",
         "- Results are the explored model's and the named tests' results on Candidate C and its mutants, under "
         "AD-1-R4 J's storage assumptions; nothing here is a filesystem, network or OS-containment claim.",
         "- Reproducible builds are not established (AD-1-R4 I); every identity is the sha256 of the executable "
         "actually built and run.",
         "- The F-B2 and F-G restored-side receipts are the accepted Stage-2 receipts, linked (not rerun) under "
         "AD-1-R4 I's proposed receipt-linking clause S-5 as the mission's section 15 directs; each link is checked at "
         "its control's own verified restoration.", ""]
    open(os.path.join(outdir, "INDEX.md"), "w").write("\n".join(i) + "\n")
    print(f"TRANCHE STATUS: {st}; candidates {len(candidates)}; attempted {attempted}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
