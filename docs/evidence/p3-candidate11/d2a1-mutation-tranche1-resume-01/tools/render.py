#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 19: render INDEX.md and TRANCHE-SUMMARY.md from the
receipts (read-only; deterministic; a missing receipt is rendered as not attempted, never invented). Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01 (section 19): the combined Tranche-1 summary (C22 Architect-accepted from
the ruling record; the eleven resume controls from this mission's receipts), the carried-forward receipts referenced
by previous commit, path, blob and sha256 (CARRIED-FORWARD.md), and this mission's index.

    render.py OUTDIR       writes OUTDIR/INDEX.md, OUTDIR/TRANCHE-SUMMARY.md and OUTDIR/CARRIED-FORWARD.md; prints the
                           derived status
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
    ruling_ok = lines(os.path.join(K.EV, K.C22_RULING))[:1] == [K.C22_RULING_FIRST]
    results = [lines(os.path.join(K.EV, exp["controls"][c]["dir"], "RESULT.txt"))[:1] for c in K.RESUME_ORDER]
    if not (base_ok and healthy_ok and ruling_ok):
        return "BLOCKED", base_ok, healthy_ok, results
    if all(r == [RESULT_OK] for r in results):
        return "READY_FOR_MUTATION_TRANCHE1_RESUME_ARCHITECT_REVIEW", base_ok, healthy_ok, results
    return "MUTATION_CONTROL_BLOCKED", base_ok, healthy_ok, results


def prev_ref(rel):
    """(path in the previous packet, git blob at the previous evidence commit, sha256) of one carried-forward file."""
    path = f"{K.PREV_PACKET}/{rel}"
    b = K.git("rev-parse", f"{K.PREV}:{path}").decode().strip()
    return path, b, K.sha256_bytes(K.git("cat-file", "blob", b))


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
    rows = [r for r in (control_row(exp, c) for c in K.RESUME_ORDER) if r]
    attempted = [r["id"] for r in rows]
    candidates = [r["id"] for r in rows if r["result"] == RESULT_OK]
    c22 = exp["controls"]["C22"]
    s = [f"# Tranche-1 summary — {K.MISSION}", "",
         f"Derived status: **{st}** (from the receipts; publication and the final report follow).", "",
         "Combined Tranche 1 (12 controls):", "",
         "- **C22 — MUTATION CONTROL KILL ACCEPTED** by the Sole Architect (ruling recorded in "
         "`ARCHITECT-RULING-C22-ACCEPTANCE.md`; type (d), restored side the exact-command healthy build (e4)). Not "
         "executed again here. Its original executor classifications remain as historical outputs of the defective "
         "frozen tool.",
         f"- new **KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING** results of this mission: **{len(candidates)}** "
         f"of the 11 remaining controls ({', '.join(candidates) or 'none'}). No control is called killed or accepted "
         "here: acceptance belongs to the Sole Architect, receipt by receipt.",
         f"- attempted by this mission: {', '.join(attempted) or 'none'}; not attempted: "
         f"{', '.join(c for c in K.RESUME_ORDER if c not in attempted) or 'none'}", "",
         "## C22 (Architect-accepted; previous packet, not rerun)", "",
         "| Receipt | Previous packet path | Blob at `" + K.PREV[:8] + "` | sha256 |", "|---|---|---|---|"]
    for rel in ("receipts/31-C22/02-apply/APPLY.json", "receipts/31-C22/03-build/build.stderr",
                "receipts/31-C22/03-build/PARSED-RECEIPT.json", "receipts/31-C22/03-build/CLASSIFICATION.txt",
                "receipts/31-C22/05-restore/RESTORE.json", "receipts/31-C22/RESULT.txt",
                "receipts/20-healthy-build/CLASSIFICATION.txt", "receipts/20-healthy-build/descriptor.json"):
        path, b, h = prev_ref(rel)
        s.append(f"| {rel.split('/', 1)[1]} | `{path}` | `{b}` | `{h}` |")
    s += ["", f"- frozen patch sha256 `{c22['patch']['sha256']}`; mutant protocol.rs blob `{c22['mutant_blob']}`, sha256 "
          f"`{c22['mutant_sha256']}`; E0283 at `{c22['diagnostic_file']}:{c22['diagnostic_line']}:5`, "
          f"`{c22['diagnostic_source_line']}`; no mutant executable (by design)", "",
          "## Baseline receipts linked by this mission (previous packet, not rerun)", "",
          "| Baseline | Type | For | Previous packet CLASSIFICATION.txt | Blob | sha256 | Classification |",
          "|---|---|---|---|---|---|---|"]
    for b in exp["baselines"]:
        path, bb, h = prev_ref(b["dir"] + "/CLASSIFICATION.txt")
        cl = outer(os.path.join(K.EV, b["dir"], "CLASSIFICATION.txt"))
        s.append(f"| {b['key']} | ({b['type']}) | {b['control']} | `{path}` | `{bb}` | `{h}` | {cl[0]} |")
    s += ["", "## The eleven remaining controls (resume order)", ""]
    for r in rows:
        s += [f"### {r['id']} — `{r['dir']}`", "", f"- result: **{r['result']}**", f"- receipt type: {r['type']}",
              f"- frozen patch sha256: `{r['patch_sha256']}`", f"- mutant source blob: `{r['mutant_blob']}` (sha256 "
              f"`{r['mutant_sha256']}`; computed in-process, never written to any object store)",
              f"- build: {r['build']}", f"- build resources: {r['build_resources']}",
              f"- mutant executable: {r['executable']}", f"- exact selection: `{r['selection']}`",
              f"- raw outcome: {r['raw']}", f"- outer classification: {r['outer']}",
              f"- named property / test: {r['named']}", f"- discovered count: {r['count']}",
              f"- run resources: {r['run_resources']}", f"- restoration: {r['restore']}",
              f"- linked restored-side receipt: {r['link']}", ""]
    not_attempted = [c for c in K.RESUME_ORDER if c not in attempted]
    s += ["## Not attempted", "",
          f"- authorized but not attempted (a stop ends the tranche): {', '.join(not_attempted) or 'none'}",
          f"- not authorized (never executed): {', '.join(K.NOT_AUTHORIZED)}; C22 not rerun; A4 not rerun; M01–M183 "
          "not executed; no baseline receipt rerun", ""]
    open(os.path.join(outdir, "TRANCHE-SUMMARY.md"), "w").write("\n".join(s) + "\n")
    cf = [f"# Carried-forward files — {K.MISSION}", "",
          f"Byte copies of files of the previous evidence packet (`{K.PREV_PACKET}/` at `{K.PREV}`), kept under this "
          "mission's local evidence root for linking and gating, frozen, authenticated against the previous "
          "SHA256SUMS, and NOT republished here. Each is identified by its previous path, git blob and sha256.", "",
          "| Local path | Previous packet path | Blob | sha256 |", "|---|---|---|---|"]
    for c in K.CARRIED:
        p = os.path.join(K.EV, c)
        rels = [c] if os.path.isfile(p) else sorted(os.path.relpath(os.path.join(a, f), K.EV)
                                                    for a, _, fs in os.walk(p) for f in fs)
        for rel in rels:
            path, b, h = prev_ref(rel)
            cf.append(f"| `{rel}` | `{path}` | `{b}` | `{h}` |")
    for dst, src in K.CARRIED_DOCS.items():
        path, b, h = prev_ref(src)
        cf.append(f"| `{dst}` | `{path}` | `{b}` | `{h}` |")
    open(os.path.join(outdir, "CARRIED-FORWARD.md"), "w").write("\n".join(cf) + "\n")
    fr = jload(os.path.join(R, "05-FREEZE.json")) or {}
    i = [f"# {K.MISSION} — evidence packet", "",
         "Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. Evidence only: no source, catalog, "
         "patch, contract, amendment or manifest is changed; no mutant source, executable or target directory is "
         "committed; the previous packet is not modified or duplicated.", "",
         f"Derived status: **{st}**. C22: Architect-accepted (ruling record). New candidate kill evidence: "
         f"{len(candidates)} of 11 ({', '.join(candidates) or 'none'}), each **KILL_EVIDENCE_CANDIDATE — ARCHITECT "
         "REVIEW PENDING**.", "",
         "## Identities", "",
         f"- Candidate C `{K.C}`, tree `{K.C_TREE}`, parent `{K.C_PARENT}`; protocol.rs `{K.BLOBS[K.PROTOCOL]}`, "
         f"model_tests.rs `{K.BLOBS[K.MODEL]}`, unit_tests.rs `{K.BLOBS[K.UNIT]}`",
         f"- frozen catalog `{K.CATALOG_PATH}` blob `{K.CATALOG_BLOB}` sha256 `{K.CATALOG_SHA}` at `{K.R2}` "
         f"({K.R2_REF}); definitions record blob `{K.DEFS_BLOB}` sha256 `{K.DEFS_SHA}`",
         f"- base contract sha256 `{K.CONTRACT_SHA}`; AD-1-R4 sha256 `{K.AD1R4_SHA}`; AD-1-R5 sha256 `{K.AD1R5_SHA}` "
         f"(R5 evidence commit `{K.R5_COMMIT}`)",
         f"- accepted unchanged-core evidence: Stage 1 `{K.STAGE1}`, Stage 2 `{K.STAGE2}`",
         f"- previous execution evidence ({K.PREV_MISSION}): commit `{K.PREV}`, tree `{K.PREV_TREE}`, parent "
         f"`{K.PREV_PARENT}`, packet `{K.PREV_PACKET}/`, ref `{K.PREV_REF}`",
         f"- pinned unchanged-baseline executable `{K.EXE}` sha256 `{K.EXE_SHA256}` size {K.EXE_SIZE:,}",
         f"- evidence commit parent: `{K.PREV}`; review ref `{K.DEST}`", "",
         "## Contents", "",
         "| Path | What |", "|---|---|",
         "| `INDEX.md` | this file |",
         "| `TRANCHE-SUMMARY.md` | the combined Tranche-1 summary: C22 Architect-accepted, the linked baselines, and every "
         "attempted remaining control with identities, outcomes, classifications, resources, restoration and links |",
         "| `ARCHITECT-RULING-C22-ACCEPTANCE.md` | the record of the Architect's C22 ruling (evidence of the ruling; a "
         "string is not authority) |",
         "| `CARRIED-FORWARD.md` | every previous-packet file this mission links or gates on, by previous commit, path, "
         "blob and sha256 (not republished) |",
         "| `receipts/00-authentication.txt` | provenance: every input, the complete previous packet and its "
         "SHA256SUMS, the facts of the C22 ruling, the carried-forward copies |",
         "| `receipts/01-supervision-selftest/` | the unchanged enforcement chain proven again on non-Nexus processes |",
         "| `receipts/03-verifier-controls/` | the repaired verifier's positive, negative, replay, sensitivity and "
         "randomized controls (no Cargo, no Nexus test) |",
         "| `receipts/04-tool-repair/` | the exact old-to-new tooling diff, its authorization map, and the mechanical check "
         "that only authorized differences exist |",
         "| `receipts/04b-render-rehearsal/` | render.py on synthetic receipts |",
         "| `receipts/05-FREEZE.json`, `05-FREEZE-RECORD.txt` | the freeze, before C14 |",
         "| `receipts/06-PRE-EXECUTION-GATE.txt` (+ `06-gate-authentication.txt`) | the static final pre-execution gate |",
         "| `receipts/32-C14/` … `42-C18b/` | one directory per attempted control: gate, worktree, apply, build, run, "
         "restore, link, RESULT |",
         "| `receipts/90-final-gate.txt` (+ `90-gate-authentication.txt`) | the final gate |",
         "| `tools/` | the tools this mission ran: the EXEC-01 tools, repaired only as authorized (see "
         "`receipts/04-tool-repair/`) |",
         "| `development/` | the development log, every failing development output, every revision and its diff |",
         "| `SHA256SUMS` | every packet file but itself |", "",
         "## Freeze", "",
         f"Frozen files: {len(fr.get('files', {}))}; recorded {fr.get('recorded')}; freeze record sha256 "
         f"`{K.sha256_file(os.path.join(R, '05-FREEZE.json')) if os.path.exists(os.path.join(R, '05-FREEZE.json')) else 'none'}`. "
         "No frozen file was changed after the freeze; every supervised step re-checks it.", "",
         "## Limitations", "",
         "- Classifications are executor candidates for Architect review; no kill is accepted here. C22's acceptance is "
         "the Sole Architect's ruling, recorded, not an executor finding.",
         "- Results are the explored model's and the named tests' results on Candidate C and its mutants, under "
         "AD-1-R4 J's storage assumptions; nothing here is a filesystem, network or OS-containment claim.",
         "- Reproducible builds are not established (AD-1-R4 I); every identity is the sha256 of the executable "
         "actually built and run.",
         "- The restored-side receipts are linked, not rerun: the standalone baselines (X-1 e1; T-BIND, T-CELL, W6, "
         "T-PREC, T-RES e2) of the previous packet and the accepted Stage-2 F-B2 and F-G receipts; each link is checked "
         "at its control's own verified restoration.", ""]
    open(os.path.join(outdir, "INDEX.md"), "w").write("\n".join(i) + "\n")
    print(f"STATUS: {st}; new candidates {len(candidates)} of 11; attempted {attempted}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
