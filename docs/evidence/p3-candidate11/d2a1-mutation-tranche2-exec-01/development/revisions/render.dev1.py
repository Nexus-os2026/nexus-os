#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 19: render INDEX.md and TRANCHE-SUMMARY.md from the
receipts (read-only; deterministic; a missing receipt is rendered as not attempted, never invented). Revised by
-RESUME-01 and -CONTINUE-02. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (sections 16 and 19): the Tranche-2
summary (every attempted control's identities, outcome, classification, resources, restoration and restored-side link;
the campaign status after this tranche), CARRIED-FORWARD.md (every immutable file this mission authenticates, links,
gates on or runs unchanged, by commit, path, blob and sha256, never republished) and this mission's index. The status
is derived from the receipts: READY_FOR_MUTATION_TRANCHE2_ARCHITECT_REVIEW, MUTATION_CONTROL_BLOCKED or BLOCKED.

    render.py OUTDIR       writes OUTDIR/INDEX.md, OUTDIR/TRANCHE-SUMMARY.md and OUTDIR/CARRIED-FORWARD.md; prints the
                           derived status
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

R = os.path.join(K.EV, "receipts")
RESULT_OK = "CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"
STATUSES = ("READY_FOR_MUTATION_TRANCHE2_ARCHITECT_REVIEW", "MUTATION_CONTROL_BLOCKED", "BLOCKED")


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
    return (f"wall {sup.get('wall_seconds')} s / {sup.get('time_limit_seconds')} s; scope peak {mib:.1f} MiB / 16 GiB; "
            f"swap peak {sup.get('swap_peak_bytes')} B; oom_kill {sup.get('oom_kill')}; largest process max RSS "
            f"{sup.get('max_rss_kib_largest_process')} KiB" if mib is not None else "—")


def status(exp):
    pre = (lines(os.path.join(K.EV, K.RULING))[:1] == [K.RULING_FIRST]
           and lines(os.path.join(K.EV, K.CAMPAIGN))[:1] == [K.CAMPAIGN_FIRST]
           and lines(os.path.join(R, "06-PRE-EXECUTION-GATE.txt"))[:1] == ["PRE-EXECUTION GATE: ALL CHECKS HOLD"])
    results = [lines(os.path.join(K.EV, exp["controls"][c]["dir"], "RESULT.txt"))[:1] for c in K.ORDER]
    if pre and all(r == [RESULT_OK] for r in results):
        return "READY_FOR_MUTATION_TRANCHE2_ARCHITECT_REVIEW", results
    if pre and any(r and r[0].startswith("CONTROL RESULT: ") and r != [RESULT_OK] for r in results):
        return "MUTATION_CONTROL_BLOCKED", results
    return "BLOCKED", results


def ref(commit, path):
    """(git blob at commit, sha256) of one immutable file."""
    b = K.git("rev-parse", f"{commit}:{path}").decode().strip()
    return b, K.sha256_bytes(K.git("cat-file", "blob", b))


def control_row(exp, cid):
    c = exp["controls"][cid]
    d = os.path.join(K.EV, c["dir"])
    if not os.path.exists(d):
        return None
    res_first = lines(os.path.join(d, "RESULT.txt"))[:1]
    build = jload(os.path.join(d, "03-build/PARSED-RECEIPT.json")) or {}
    bcls = outer(os.path.join(d, "03-build/CLASSIFICATION.txt"))
    run = jload(os.path.join(d, "04-run/PARSED-RECEIPT.json"))
    rcls = outer(os.path.join(d, "04-run/CLASSIFICATION.txt")) if run else None
    pin = jload(os.path.join(d, "03-build/PIN.json"))
    restore = lines(os.path.join(d, "05-restore/RESTORE.txt"))
    link = lines(os.path.join(d, "06-link/LINK.txt"))
    errs = (build.get("process") or {})
    row = {"id": cid, "dir": c["dir"], "result": res_first[0] if res_first else "NOT COMPLETED",
           "facts": lines(os.path.join(d, "RESULT.txt"))[3:], "gate": (lines(os.path.join(d, "00-GATE.txt")) or ["—"])[0],
           "apply": (lines(os.path.join(d, "02-apply/APPLY.txt")) or ["not reached"])[0],
           "type": c["receipt_type"], "patch_sha256": c["patch"]["sha256"], "mutant_blob": c["mutant_blob"],
           "mutant_sha256": c["mutant_sha256"], "file": c["source_file"],
           "build": f"{bcls[0]} — {bcls[1]}" if bcls[0] != "NOT PRODUCED" else "not reached",
           "build_raw": (f"exit {errs.get('exit_status')}, signal {errs.get('signal')}; Finished {build.get('finished')}"
                         if build else "—"),
           "build_resources": res(build.get("supervisor")),
           "executable": f"`{pin['path']}` sha256 `{pin['sha256']}` size {pin['size']:,}" if pin else "not produced",
           "selection": " ".join(c["argv_tail"]), "config": f"{c['configuration']} (`{c['test']}`, original explorer)",
           "restore": restore[0] if restore else "NOT PRODUCED", "link": (link[0] + " — " + link[1]) if link else "—",
           "named": c["named_property"], "ceiling": f"{c['ceiling_states']:,}"}
    if run:
        rep = run.get("report") or {}
        row["raw"] = f"outcome {rep.get('outcome')}; FAIL {rep.get('fail')}; {rep.get('trace_steps') or ''}".strip()
        row["printed"] = (rep.get("fail") or "").split(": ", 1)[0] if rep.get("fail") else "none"
        row["count"] = (f"{rep.get('states'):,} discovered (expanded {rep.get('expanded'):,}) of {c['ceiling_states']:,}"
                        if rep.get("states") is not None else "not printed")
        row["outer"] = rcls[1]
        row["run_first"] = rcls[0]
        row["exit"] = f"exit {run['process'].get('exit_status')}, signal {run['process'].get('signal')}"
        row["run_resources"] = res(run.get("supervisor"))
    else:
        row.update(raw="not run", printed="—", count="—", outer="not run", run_first="not run", exit="—",
                   run_resources="—")
    return row


def main(outdir):
    exp = json.load(open(os.path.join(R, "02-EXPECTATIONS.json")))
    st, results = status(exp)
    rows = [r for r in (control_row(exp, c) for c in K.ORDER) if r]
    attempted = [r["id"] for r in rows]
    candidates = [r["id"] for r in rows if r["result"] == RESULT_OK]
    blocked = [r["id"] for r in rows if r["result"] != RESULT_OK]
    not_attempted = [c for c in K.ORDER if c not in attempted]
    s = [f"# Tranche-2 summary — {K.MISSION}", "",
         f"Derived status: **{st}** (from the receipts).", "",
         f"- Authorized (mission section 2), in order: {', '.join(K.ORDER)}.",
         f"- Attempted: {', '.join(attempted) or 'none'}.",
         f"- New executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`): **{len(candidates)} of 10** "
         f"({', '.join(candidates) or 'none'}). None is accepted here: acceptance belongs to the Sole Architect.",
         f"- Stopped (first unsuccessful control; nothing later was attempted): {', '.join(blocked) or 'none'}.",
         f"- Authorized but not attempted: {', '.join(not_attempted) or 'none'}.",
         "", "## Campaign status after this tranche", "",
         f"- Architect-accepted kills: **12 of {K.KILL_REQUIRED}** ({', '.join(K.ACCEPTED)}; record `{K.RULING}`), "
         "unchanged by this mission.",
         f"- Executor candidates pending Architect review from this tranche: {len(candidates)} "
         f"({', '.join(candidates) or 'none'}).",
         f"- Not authorized here (never executed): {', '.join(K.NOT_AUTHORIZED)}. A4: SATISFIED — ARCHITECT ACCEPTED "
         "(separately; not rerun). M01–M183: not executed. Unchanged-core configurations: linked, never rerun.",
         "- Not claimed: no complete campaign, no D2A-1 acceptance, no integration, no Phase Three completion.", "",
         "## Controls (mission order)", ""]
    for r in rows:
        s += [f"### {r['id']} — `{r['dir']}`", "", f"- result: **{r['result']}**", f"- gate: {r['gate']}",
              f"- receipt type: {r['type']}; configuration {r['config']}; named property {r['named']}; ceiling "
              f"{r['ceiling']} states",
              f"- frozen patch sha256 `{r['patch_sha256']}`; mutated file `{r['file']}`; mutant blob `{r['mutant_blob']}` "
              f"(sha256 `{r['mutant_sha256']}`; computed in-process, never written to any object store)",
              f"- apply: {r['apply']}", f"- build: {r['build']}; {r['build_raw']}",
              f"- build resources: {r['build_resources']}", f"- mutant executable: {r['executable']}",
              f"- exact selection: `{r['selection']}`", f"- raw outcome (as printed): {r['raw']}; {r['exit']}",
              f"- printed property: {r['printed']}", f"- discovered count: {r['count']}",
              f"- outer classification: {r['run_first']} — {r['outer']}", f"- run resources: {r['run_resources']}",
              f"- restoration: {r['restore']}", f"- restored-side link: {r['link']}", "", "  RESULT facts:", ""]
        s += [f"  - {x}" for x in r["facts"]] + [""]
    open(os.path.join(outdir, "TRANCHE-SUMMARY.md"), "w").write("\n".join(s) + "\n")
    # CARRIED-FORWARD.md: every immutable file this mission authenticates, links, gates on or runs unchanged.
    cf = [f"# Referenced, not republished — {K.MISSION}", "",
          "Every file below is immutable evidence at the named commit. This mission authenticated it (receipts/"
          "00-authentication.txt), linked it, gated on it or ran it unchanged; it is referenced here by commit, path, git "
          "blob and sha256 and not copied into this packet.", "",
          "## Tools run unchanged (carried byte for byte from the accepted Tranche-1 tooling)", "",
          "| Tool | Packet path | Commit | Blob | sha256 |", "|---|---|---|---|---|"]
    c2tools = {l.split("  ", 1)[1][6:] for l in K.git("show", f"{K.T1}:{K.CONTINUE02_PACKET}/SHA256SUMS").decode()
               .splitlines() if l.split("  ", 1)[1].startswith("tools/")}
    for t in sorted(os.listdir(os.path.join(K.EV, "tools"))):
        o = os.path.join(K.EV, "original-tools", t)
        if os.path.exists(o) and K.sha256_file(o) == K.sha256_file(os.path.join(K.EV, "tools", t)):
            path = f"{K.CONTINUE02_PACKET if t in c2tools else K.RESUME01_PACKET}/tools/{t}"
            b, h = ref(K.T1, path)
            cf.append(f"| `{t}` | `{path}` | `{K.T1}` | `{b}` | `{h}` |")
    cf += ["", "## The old versions of the changed tools (the base of receipts/04-tool-diff)", "",
           "| Tool | Packet path | Commit | Blob | sha256 |", "|---|---|---|---|---|"]
    for t in sorted(os.listdir(os.path.join(K.EV, "original-tools"))):
        n = os.path.join(K.EV, "tools", t)
        if os.path.exists(n) and K.sha256_file(n) != K.sha256_file(os.path.join(K.EV, "original-tools", t)):
            path = f"{K.CONTINUE02_PACKET if t in c2tools else K.RESUME01_PACKET}/tools/{t}"
            b, h = ref(K.T1, path)
            cf.append(f"| `{t}` | `{path}` | `{K.T1}` | `{b}` | `{h}` |")
    cf += ["", "## Inputs: catalog, patches, contract, addenda, adoption records, proof", "",
           "| Item | Path | Commit | Blob | sha256 |", "|---|---|---|---|---|"]
    items = [("frozen catalog", K.R2, K.CATALOG_PATH), ("definitions record", K.R2, K.DEFS_PATH)]
    items += [(f"{c} frozen patch", K.R2, f"{K.R2_DIR}catalog/patches/{c}.patch") for c in K.ORDER]
    items += [("contract", K.CONTRACT_COMMIT, K.CONTRACT_PATH), ("AD-1-R4", K.C, K.AD1R4),
              ("AD-1-R4 Owner adoption record", K.T1, K.AD1R4_ADOPTION), ("AD-1-R5", K.R5_COMMIT, K.AD1R5),
              ("AD-1-R5 Owner adoption record", K.AD1R5_ADOPTION_COMMIT, K.AD1R5_ADOPTION),
              ("accepted F-R stuttering-refinement proof", K.R5_COMMIT, K.FR_PROOF)]
    for label, commit, path in items:
        b, h = ref(commit, path)
        cf.append(f"| {label} | `{path}` | `{commit}` | `{b}` | `{h}` |")
    cf += ["", "## Unchanged-core receipts linked on the restored side (never rerun)", "",
           "| For | Receipt file | Commit | Blob | sha256 |", "|---|---|---|---|---|"]
    linked = {}
    for cid in K.ORDER:
        L = exp["controls"][cid]["linked"]
        linked.setdefault((L["commit"], L["directory"]), []).append(cid)
        if L["type"] == "fr-discharge":
            linked.setdefault((K.STAGE2, L["fr_receipt"]), []).append(cid + " (F-R INCOMPLETE, beside the record)")
    for (commit, d), cids in sorted(linked.items(), key=lambda x: x[0][1]):
        for f in ("CLASSIFICATION.txt", "descriptor.json", "raw-output.txt"):
            b, h = ref(commit, f"{d}/{f}")
            cf.append(f"| {', '.join(cids)} | `{d}/{f}` | `{commit}` | `{b}` | `{h}` |")
    cf += ["", "## The accepted Tranche-1 evidence", "",
           "| Item | Path | Commit | Blob | sha256 |", "|---|---|---|---|---|"]
    for name, packet in K.T1_PACKETS:
        b, h = ref(K.T1, f"{packet}/SHA256SUMS")
        cf.append(f"| {name} packet manifest | `{packet}/SHA256SUMS` | `{K.T1}` | `{b}` | `{h}` |")
    for cid in K.ACCEPTED:
        packet, d = K.T1_RECEIPTS[cid]
        b, h = ref(K.T1, f"{packet}/{d}/RESULT.txt")
        cf.append(f"| {cid} RESULT (historical label; Architect-accepted) | `{packet}/{d}/RESULT.txt` | `{K.T1}` | `{b}` | "
                  f"`{h}` |")
    open(os.path.join(outdir, "CARRIED-FORWARD.md"), "w").write("\n".join(cf) + "\n")
    fr = jload(os.path.join(R, "05-FREEZE.json")) or {}
    frs = K.sha256_file(os.path.join(R, "05-FREEZE.json")) if os.path.exists(os.path.join(R, "05-FREEZE.json")) else "none"
    i = [f"# {K.MISSION} — evidence packet", "",
         "Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. Evidence only: no source, catalog, "
         "patch, contract, amendment or manifest is changed; no mutant source, executable, target directory or source "
         "copy is committed; the previous packets are referenced, not modified or duplicated.", "",
         f"Derived status: **{st}**. Tranche 2 attempted {len(attempted)} of 10 controls; new executor candidates "
         f"{len(candidates)} ({', '.join(candidates) or 'none'}), each **KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW "
         f"PENDING**; stopped at {', '.join(blocked) or 'none'}; not attempted {', '.join(not_attempted) or 'none'}. "
         f"Architect-accepted before this tranche: 12 of {K.KILL_REQUIRED} (unchanged here).", "",
         "## Identities", "",
         f"- Candidate C `{K.C}`, tree `{K.C_TREE}`, parent `{K.C_PARENT}`; protocol.rs `{K.BLOBS[K.PROTOCOL]}`, "
         f"model_tests.rs `{K.BLOBS[K.MODEL]}`, unit_tests.rs `{K.BLOBS[K.UNIT]}`",
         f"- frozen catalog `{K.CATALOG_PATH}` blob `{K.CATALOG_BLOB}` sha256 `{K.CATALOG_SHA}` at `{K.R2}`",
         f"- accepted unchanged-core evidence: Stage 1 `{K.STAGE1}`, Stage 2 `{K.STAGE2}`; AD-1-R5 `{K.AD1R5_SHA}` "
         f"(Owner adoption `{K.AD1R5_ADOPTION_SHA}`); accepted F-R proof `{K.FR_PROOF_SHA}`",
         f"- accepted Tranche-1 evidence: commit `{K.T1}`, tree `{K.T1_TREE}`, parent `{K.T1_PARENT}`, ref `{K.T1_REF}`",
         f"- pinned unchanged-baseline executable (the linked receipts') `{K.EXE}` sha256 `{K.EXE_SHA256}`",
         f"- this evidence commit's parent: `{K.T1}`; review ref `{K.DEST}`", "",
         "## Contents", "", "| Path | What |", "|---|---|",
         "| `INDEX.md` | this file |",
         "| `TRANCHE-SUMMARY.md` | every attempted control: identities, build, executable, selection, raw and outer "
         "outcome, printed property, discovered count, resources, restoration, restored-side link; the campaign status |",
         "| `CARRIED-FORWARD.md` | every immutable file referenced, linked, gated on or run unchanged (commit, path, blob, "
         "sha256; not republished) |",
         "| `documents/ARCHITECT-RULING-TRANCHE1-FINAL-ACCEPTANCE.md` | record of the Architect's Tranche-1 acceptance "
         "(12 of 35; A4 satisfied) |",
         "| `documents/CAMPAIGN-STATUS-BEFORE-TRANCHE2.md` | the campaign status before this tranche (12 of 35) |",
         "| `documents/DUPLICATE-AND-ZERO-LENGTH-MAP.tsv` (+ `.md`) | every packet file byte-equal to a previous-packet "
         "file and every empty file, each with its exact path, class, role, descriptor and prior occurrences |",
         "| `receipts/00-authentication.txt` | provenance of every input |",
         "| `receipts/01-supervision-selftest/` | the unchanged enforcement chain on non-Nexus processes |",
         "| `receipts/02-EXPECTATIONS.json` (+ console) | every expectation, derived from the frozen catalog |",
         "| `receipts/03-t2-controls/` | tooling controls on synthetic and historical evidence |",
         "| `receipts/04-tool-diff/` | the old-to-new tool diff, its authorization map and mechanical check |",
         "| `receipts/04b-rehearsals/` | render.py and build_packet.py on synthetic evidence roots |",
         "| `receipts/05-FREEZE.json`, `05-FREEZE-RECORD.txt` | the freeze |",
         "| `receipts/06-PRE-EXECUTION-GATE.txt` (+ `06-gate-authentication.txt`) | the static pre-execution gate |",
         "| `receipts/51-C23/` … `60-C15/` | one directory per attempted control: gate, worktree, apply, build, run, "
         "restore, link, RESULT |",
         "| `receipts/80-STOP-RECORD.md` | present only when the tranche stopped |",
         "| `receipts/90-final-gate.txt` (+ `90-gate-authentication.txt`) | the final gate |",
         "| `tools/` | the tools this mission changed or added (unchanged ones referenced in `CARRIED-FORWARD.md`) |",
         "| `development/` | the development log, every failing development output, every revision and its diff |",
         "| `SHA256SUMS` | every packet file but itself |", "",
         "## Freeze", "",
         f"Frozen files: {len(fr.get('files', {}))}; recorded {fr.get('recorded')}; 05-FREEZE.json sha256 `{frs}`. Every "
         "supervised step re-checks the freeze.", "",
         "## Limitations", "",
         "- Every classification here is an executor candidate for Architect review; no kill is accepted by the executor.",
         "- Results are the explored model's results on Candidate C's mutants, under AD-1-R4 J's storage assumptions; "
         "nothing here is a filesystem, network or OS-containment claim.",
         "- Reproducible builds are not established (AD-1-R4 I); every identity is the sha256 of the executable actually "
         "built and run.",
         "- The restored sides are linked, never rerun: the accepted exact-final Stage-1 and Stage-2 receipts; for C8 the "
         "Stage-1 composed-quotient F-B PASS (restored side only, AD-1-R4 F); for C15 the AD-1-R5 F refinement discharge "
         "record, which is not a run and never PASS (the F-R receipt stays INCOMPLETE).", ""]
    open(os.path.join(outdir, "INDEX.md"), "w").write("\n".join(i) + "\n")
    print(f"STATUS: {st}; new candidates {len(candidates)} of 10; attempted {attempted}; stopped {blocked}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
