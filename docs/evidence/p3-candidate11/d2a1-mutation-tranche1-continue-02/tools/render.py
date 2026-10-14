#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 19: render INDEX.md and TRANCHE-SUMMARY.md from the
receipts (read-only; deterministic; a missing receipt is rendered as not attempted, never invented). Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01 (section 19): the combined Tranche-1 summary (C22 Architect-accepted from
the ruling record; the eleven resume controls from this mission's receipts), the carried-forward receipts referenced
by previous commit, path, blob and sha256 (CARRIED-FORWARD.md), and this mission's index.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02 (section 18): the combined Tranche-1 status (C22 C14 C19
C21 C27 Architect-accepted, from the ruling record; C28 from its existing-build admission, pin, run, restoration and
link; C12 C9 C11 C17 C18a C18b from this mission's receipts), CARRIED-FORWARD.md (the EXEC-01 carried-forward copies,
and every RESUME-01 file this mission authenticates, gates or links on, by commit, path, blob and sha256; the unchanged
tools referenced, not republished), and this mission's index. The status is derived from the receipts:
READY_FOR_MUTATION_TRANCHE1_FINAL_ARCHITECT_REVIEW, C28_PRESERVED_ARTIFACT_BLOCKED, MUTATION_CONTROL_BLOCKED or BLOCKED.

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
C28_ARTIFACT_BLOCKED = "CONTROL RESULT: C28_PRESERVED_ARTIFACT_BLOCKED"
ACCEPTED_DIRS = {"C14": "32-C14", "C19": "33-C19", "C21": "34-C21", "C27": "35-C27"}


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
                  for b in exp["baselines"] if b["dir"] in K.CARRIED)
    ruling_ok = (lines(os.path.join(K.EV, K.RULING))[:1] == [K.RULING_FIRST]
                 and lines(os.path.join(K.EV, K.C28_RULING))[:1] == [K.C28_RULING_FIRST])
    results = [lines(os.path.join(K.EV, exp["controls"][c]["dir"], "RESULT.txt"))[:1] for c in K.CONTINUE_ORDER]
    if not (base_ok and ruling_ok):
        return "BLOCKED", base_ok, ruling_ok, results
    if all(r == [RESULT_OK] for r in results):
        return "READY_FOR_MUTATION_TRANCHE1_FINAL_ARCHITECT_REVIEW", base_ok, ruling_ok, results
    if results[0] == [C28_ARTIFACT_BLOCKED]:
        return "C28_PRESERVED_ARTIFACT_BLOCKED", base_ok, ruling_ok, results
    if any(r and r[0].startswith("CONTROL RESULT: ") and r != [RESULT_OK] for r in results):
        return "MUTATION_CONTROL_BLOCKED", base_ok, ruling_ok, results
    return "BLOCKED", base_ok, ruling_ok, results


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


def resume_ref(rel):
    """(path in the RESUME-01 packet, git blob at the RESUME-01 evidence commit, sha256) of one of its files."""
    path = f"{K.RESUME_PACKET}/{rel}"
    b = K.git("rev-parse", f"{K.RESUME}:{path}").decode().strip()
    return path, b, K.sha256_bytes(K.git("cat-file", "blob", b))


def control_row_c28(exp):
    """C28: the existing RESUME-01 build (admission, pin), one run, the read-only restoration, the link."""
    c = exp["controls"]["C28"]
    d = os.path.join(K.EV, c["dir"])
    if not os.path.exists(d):
        return None
    res_first = lines(os.path.join(d, "RESULT.txt"))[:1]
    adm = lines(os.path.join(d, "01-admission/ADMISSION.txt"))
    pin = jload(os.path.join(d, "02-pin/PIN.json"))
    pin_l = lines(os.path.join(d, "02-pin/PIN.txt"))
    run = jload(os.path.join(d, "04-run/PARSED-RECEIPT.json"))
    rcls = outer(os.path.join(d, "04-run/CLASSIFICATION.txt")) if run else None
    restore = lines(os.path.join(d, "05-restore/RESTORE.txt"))
    link = lines(os.path.join(d, "06-link/LINK.txt"))
    admj = jload(os.path.join(d, "01-admission/ADMISSION.json")) or {}
    return {"id": "C28", "dir": c["dir"], "result": res_first[0] if res_first else "NOT COMPLETED",
            "type": c["receipt_type"], "patch_sha256": c["patch"]["sha256"], "mutant_blob": c["mutant_blob"],
            "mutant_sha256": c["mutant_sha256"],
            "build": (f"NOT REPEATED — the existing RESUME-01 build (`{K.RESUME_PACKET}/receipts/36-C28/03-build/` at "
                      f"`{K.RESUME}`; executor history `BUILD: NOT OK` unchanged); admission: "
                      f"{adm[0] if adm else 'NOT PRODUCED'}; pin: {pin_l[0] if pin_l else 'NOT PRODUCED'}"),
            "build_resources": res(admj.get("build_supervisor")) + " (the RESUME-01 build's record)",
            "executable": (f"{pin['path']} sha256 {pin['sha256']} size {pin['size']} (preserved, not rebuilt)" if pin
                           else "not pinned"),
            "selection": " ".join(c["argv_tail"]), "named": f"{c['named_test_id']} ({c['test']})",
            "raw": (f"panic at {run.get('panic_location')}: {' | '.join(run.get('panic_message') or [])}" if run
                    else "not run"),
            "count": "not applicable" if run else "—", "outer": rcls[1] if rcls else "not run",
            "run_resources": res(run.get("supervisor")) if run else "—",
            "restore": restore[0] if restore else "NOT PRODUCED", "link": (link[0] + " — " + link[1]) if link else "—"}


def main(outdir):
    exp = json.load(open(os.path.join(R, "02-EXPECTATIONS.json")))
    st, base_ok, ruling_ok, results = status(exp)
    rows = [r for r in [control_row_c28(exp)] + [control_row(exp, c) for c in K.CONTINUE_ORDER[1:]] if r]
    attempted = [r["id"] for r in rows]
    candidates = [r["id"] for r in rows if r["result"] == RESULT_OK]
    s = [f"# Tranche-1 summary — {K.MISSION}", "",
         f"Derived status: **{st}** (from the receipts; publication and the final report follow).", "",
         "Combined Tranche 1 (12 controls):", "",
         "- **C22, C14, C19, C21, C27 — MUTATION CONTROL KILL ACCEPTED** by the Sole Architect (ruling recorded in "
         "`ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md`; C22 by the earlier ruling). Not executed again here. Their "
         "original executor classifications and raw receipts remain unchanged in the previous packets.",
         "- **C28** — its existing build is ARCHITECT-ADMISSIBLE under the narrow C28 ruling "
         "(`ARCHITECT-RULING-C28-BUILD-ADMISSIBILITY.md`); its control kill is pending the Architect's review of this "
         "mission's candidate evidence.",
         f"- new **KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING** results of this mission: **{len(candidates)}** "
         f"of the 7 remaining controls ({', '.join(candidates) or 'none'}). No control is called killed or accepted "
         "here by the executor: acceptance belongs to the Sole Architect, receipt by receipt.",
         f"- Tranche-1 count: Architect-accepted 5 of 12; executor candidates awaiting review {len(candidates)}; "
         f"attempted by this mission: {', '.join(attempted) or 'none'}; not attempted: "
         f"{', '.join(c for c in K.CONTINUE_ORDER if c not in attempted) or 'none'}", "",
         "## Architect-accepted controls (previous packets, not rerun)", "",
         "| Control | Receipt | Previous packet path | Blob | sha256 | First line (executor history) |",
         "|---|---|---|---|---|---|"]
    for cid, d in ACCEPTED_DIRS.items():
        for f in ("RESULT.txt", "04-run/CLASSIFICATION.txt", "05-restore/RESTORE.txt", "06-link/LINK.txt"):
            path, b, h = resume_ref(f"receipts/{d}/{f}")
            s.append(f"| {cid} | {f} | `{path}` | `{b}` | `{h}` | `{K.git('cat-file', 'blob', b).decode().splitlines()[0]}` |")
    for f in ("receipts/31-C22/RESULT.txt", "receipts/31-C22/03-build/CLASSIFICATION.txt"):
        path, b, h = prev_ref(f)
        s.append(f"| C22 | {f.split('/', 2)[2]} | `{path}` | `{b}` | `{h}` | "
                 f"`{K.git('cat-file', 'blob', b).decode().splitlines()[0]}` |")
    path, b, h = resume_ref("ARCHITECT-RULING-C22-ACCEPTANCE.md")
    s += [f"| C22 | the earlier ruling record | `{path}` | `{b}` | `{h}` | `C22 — MUTATION CONTROL KILL ACCEPTED` |", "",
          "## The C28 build evidence the admission authenticates (RESUME-01 packet, not rebuilt)", "",
          "| Receipt | Path | Blob | sha256 |", "|---|---|---|---|"]
    for f in ("03-build/CLASSIFICATION.txt", "03-build/build.stderr", "03-build/descriptor.json", "02-apply/APPLY.json",
              "05-restore/RESTORE.json", "RESULT.txt"):
        path, b, h = resume_ref(f"receipts/36-C28/{f}")
        s.append(f"| {f} | `{path}` | `{b}` | `{h}` |")
    s += ["", "## Baseline receipts linked by this mission (EXEC-01 packet, not rerun)", "",
          "| Baseline | Type | For | Previous packet CLASSIFICATION.txt | Blob | sha256 | Classification |",
          "|---|---|---|---|---|---|---|"]
    for b in [b for b in exp["baselines"] if b["dir"] in K.CARRIED]:
        path, bb, h = prev_ref(b["dir"] + "/CLASSIFICATION.txt")
        cl = outer(os.path.join(K.EV, b["dir"], "CLASSIFICATION.txt"))
        s.append(f"| {b['key']} | ({b['type']}) | {b['control']} | `{path}` | `{bb}` | `{h}` | {cl[0]} |")
    for cfg, d in (("F-B2", "61-full-F-B2"), ("F-G", "60-full-F-G")):
        rel = f"{K.STAGE2_PACKET}/receipts/{d}/CLASSIFICATION.txt"
        bb = K.git("rev-parse", f"{K.STAGE2}:{rel}").decode().strip()
        s.append(f"| {cfg} | (e1) | {'C9 C11 C17' if cfg == 'F-B2' else 'C18a C18b'} | `{rel}` (Stage 2, "
                 f"`{K.STAGE2[:8]}`) | `{bb}` | `{K.sha256_bytes(K.git('cat-file', 'blob', bb))}` | CLASSIFICATION: PASS |")
    s += ["", "## The seven controls of this mission (continue order)", ""]
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
    not_attempted = [c for c in K.CONTINUE_ORDER if c not in attempted]
    s += ["## Not attempted", "",
          f"- authorized but not attempted (a stop ends the tranche): {', '.join(not_attempted) or 'none'}",
          f"- not authorized (never executed): {', '.join(K.NOT_AUTHORIZED)}; C22 C14 C19 C21 C27 not rebuilt or rerun; "
          "A4 not rerun; M01–M183 not executed; no unchanged-core baseline rerun", ""]
    open(os.path.join(outdir, "TRANCHE-SUMMARY.md"), "w").write("\n".join(s) + "\n")
    cf = [f"# Carried-forward and referenced files — {K.MISSION}", "",
          f"Byte copies of EXEC-01 packet files (`{K.PREV_PACKET}/` at `{K.PREV}`), kept under this mission's local "
          "evidence root for linking and gating, frozen, authenticated against that packet's SHA256SUMS, and NOT "
          "republished here; then the RESUME-01 files this mission authenticates or gates on (read from git objects at "
          f"`{K.RESUME}`, never copied); then the RESUME-01 tools this mission ran unchanged (not republished: "
          "minimal reproducible tools are this packet's changed and added tools plus these references).", "",
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
    cf += ["", "## RESUME-01 files authenticated or gated on (referenced, not copied)", "",
           "| RESUME-01 packet path | Blob | sha256 |", "|---|---|---|"]
    import c28_admit
    for a in c28_admit.evidence()["auth"]:
        cf.append(f"| `{a['path']}` | `{a['blob']}` | `{a['sha256']}` |")
    for rel in ("SHA256SUMS", "ARCHITECT-RULING-C22-ACCEPTANCE.md", "receipts/05-FREEZE.json",
                "receipts/05-FREEZE-RECORD.txt", "receipts/04-tool-repair/TOOL-DIFF-CHECK.txt",
                "receipts/90-final-gate.txt", "receipts/80-STOP-RECORD.md"):
        path, b, h = resume_ref(rel)
        cf.append(f"| `{path}` | `{b}` | `{h}` |")
    cf += ["", "## RESUME-01 tools run unchanged by this mission (referenced, not republished)", "",
           "| Tool | RESUME-01 packet path | Blob | sha256 |", "|---|---|---|---|"]
    for t in sorted(os.listdir(os.path.join(K.EV, "original-tools"))):
        if os.path.exists(os.path.join(K.EV, "tools", t)) and K.sha256_file(os.path.join(K.EV, "tools", t)) == \
                K.sha256_file(os.path.join(K.EV, "original-tools", t)):
            path, b, h = resume_ref("tools/" + t)
            cf.append(f"| `{t}` | `{path}` | `{b}` | `{h}` |")
    open(os.path.join(outdir, "CARRIED-FORWARD.md"), "w").write("\n".join(cf) + "\n")
    fr = jload(os.path.join(R, "05-FREEZE.json")) or {}
    i = [f"# {K.MISSION} — evidence packet", "",
         "Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. Evidence only: no source, catalog, "
         "patch, contract, amendment or manifest is changed; no mutant source, executable or target directory is "
         "committed; the previous packets are not modified or duplicated.", "",
         f"Derived status: **{st}**. Architect-accepted (ruling record): C22, C14, C19, C21, C27 (5 of 35 kill-required "
         f"controls). C28: build ARCHITECT-ADMISSIBLE (narrow ruling); kill pending. New candidate kill evidence: "
         f"{len(candidates)} of 7 ({', '.join(candidates) or 'none'}), each **KILL_EVIDENCE_CANDIDATE — ARCHITECT "
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
         f"- previous execution evidence ({K.RESUME_MISSION}): commit `{K.RESUME}`, tree `{K.RESUME_TREE}`, parent "
         f"`{K.RESUME_PARENT}`, packet `{K.RESUME_PACKET}/`, ref `{K.RESUME_REF}`",
         f"- pinned unchanged-baseline executable `{K.EXE}` sha256 `{K.EXE_SHA256}` size {K.EXE_SIZE:,}",
         f"- evidence commit parent: `{K.RESUME}`; review ref `{K.DEST}`", "",
         "## Contents", "",
         "| Path | What |", "|---|---|",
         "| `INDEX.md` | this file |",
         "| `TRANCHE-SUMMARY.md` | the combined Tranche-1 status: the five Architect-accepted controls (references), the C28 "
         "build evidence, the linked baselines, and every attempted control with identities, outcomes, classifications, "
         "resources, restoration and links |",
         "| `ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md` | the record of the Architect's acceptance of C22, C14, C19, "
         "C21, C27 and of the C28 build admissibility (evidence of the ruling; a string is not authority) |",
         "| `ARCHITECT-RULING-C28-BUILD-ADMISSIBILITY.md` | the record of the narrow C28 build-admissibility ruling |",
         "| `CARRIED-FORWARD.md` | every previous-packet file this mission links, gates on or runs unchanged, by commit, "
         "path, blob and sha256 (not republished) |",
         "| `receipts/00-authentication.txt` | provenance: every input, both previous packets and their SHA256SUMS, every "
         "C14/C19/C21/C27 result, the C28 history, the previous tool-repair and freeze receipts |",
         "| `receipts/01-supervision-selftest/` | the unchanged enforcement chain proven again on non-Nexus processes |",
         "| `receipts/03-c28-admission-controls/` | the section-14 tool controls of the C28 admission checker and pin |",
         "| `receipts/04-tool-diff/` | the complete RESUME-01-to-this-mission tool diff, its authorization map, and the "
         "mechanical check that only authorized differences exist |",
         "| `receipts/04b-render-rehearsal/` | render.py on synthetic receipts |",
         "| `receipts/05-FREEZE.json`, `05-FREEZE-RECORD.txt` | the freeze, before the C28 invocation |",
         "| `receipts/06-PRE-EXECUTION-GATE.txt` (+ `06-gate-authentication.txt`) | the static pre-execution gate |",
         "| `receipts/36-C28/` | C28: gate, existing-build admission, pin, run, restoration re-authentication, link, RESULT "
         "(no build: not repeated) |",
         "| `receipts/37-C12/` … `42-C18b/` | one directory per attempted later control: gate, worktree, apply, build, "
         "run, restore, link, RESULT |",
         "| `receipts/90-final-gate.txt` (+ `90-gate-authentication.txt`) | the final gate |",
         "| `tools/` | the tools this mission changed or added (the unchanged RESUME-01 tools are referenced in "
         "`CARRIED-FORWARD.md`) |",
         "| `development/` | the development log, every failing development output, every revision and its diff |",
         "| `SHA256SUMS` | every packet file but itself |", "",
         "## Freeze", "",
         f"Frozen files: {len(fr.get('files', {}))}; recorded {fr.get('recorded')}; freeze record sha256 "
         f"`{K.sha256_file(os.path.join(R, '05-FREEZE.json')) if os.path.exists(os.path.join(R, '05-FREEZE.json')) else 'none'}`. "
         "No frozen file was changed after the freeze; every supervised step re-checks it.", "",
         "## Limitations", "",
         "- Classifications are executor candidates for Architect review; no kill is accepted here by the executor. The "
         "acceptances of C22, C14, C19, C21 and C27 and the C28 build admissibility are the Sole Architect's rulings, "
         "recorded, not executor findings.",
         "- C28's executable is the RESUME-01 build, admitted under a narrow C28-only ruling; it was not rebuilt. Its run "
         "was on the restored worktree (the ledger module reads no file at run time); its mutant identity is carried by "
         "the admitted build evidence (source C plus the frozen patch before and after the build) and the pinned "
         "executable identity.",
         "- Results are the explored model's and the named tests' results on Candidate C and its mutants, under "
         "AD-1-R4 J's storage assumptions; nothing here is a filesystem, network or OS-containment claim.",
         "- Reproducible builds are not established (AD-1-R4 I); every identity is the sha256 of the executable "
         "actually built and run.",
         "- The restored-side receipts are linked, not rerun: the standalone X-1 (e1) and T-RES (e2) baselines of the "
         "EXEC-01 packet and the accepted Stage-2 F-B2 and F-G receipts; each link is checked at its control's own "
         "verified restoration.", ""]
    open(os.path.join(outdir, "INDEX.md"), "w").write("\n".join(i) + "\n")
    print(f"STATUS: {st}; new candidates {len(candidates)} of 7; attempted {attempted}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
