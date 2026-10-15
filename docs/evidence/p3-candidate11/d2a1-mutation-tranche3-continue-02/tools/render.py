#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 19: render INDEX.md and TRANCHE-SUMMARY.md from the
receipts (read-only; deterministic; a missing receipt is rendered as not attempted, never invented). Revised by
-RESUME-01 and -CONTINUE-02. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (sections 16 and 19): the Tranche-2
summary (every attempted control's identities, outcome, classification, resources, restoration and restored-side link;
the campaign status after this tranche), CARRIED-FORWARD.md (every immutable file this mission authenticates, links,
gates on or runs unchanged, by commit, path, blob and sha256, never republished) and this mission's index. The status
is derived from the receipts: READY_FOR_MUTATION_TRANCHE2_ARCHITECT_REVIEW, MUTATION_CONTROL_BLOCKED or BLOCKED.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02 (sections 19 and 21): this mission's summary (C5's admission,
pin, run, reauthentication and link; A3, A6, C8, C15 as before), its CARRIED-FORWARD.md (the Tranche-2 base tooling, the
Tranche-2 stopped evidence, C5's preserved build receipt) and index. The status is derived from the receipts:
READY_FOR_MUTATION_TRANCHE2_FINAL_ARCHITECT_REVIEW, C5_PRESERVED_ARTIFACT_BLOCKED, MUTATION_CONTROL_BLOCKED or BLOCKED.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01 (sections 14 and 16): this mission's summary (every attempted
control: identities, build, executable, selection, raw and outer outcome, the tranche-3 rules, C7's informational run
without A4 on F-3, L1's complete-search evidence for C20/A5, resources, restoration and restored-side link; the campaign
status before Architect review), its CARRIED-FORWARD.md (the continuation's base tooling, the inputs, the linked
receipts, every previous evidence commit) and index. The status is derived from the receipts:
READY_FOR_MUTATION_TRANCHE3_ARCHITECT_REVIEW, MUTATION_CONTROL_BLOCKED or BLOCKED.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02 (sections 20 and 21): this mission's summary (C24's admission,
preserved worktree, pin, run, rules, reauthentication and link; C3b's and C3c's build admission under AMENDMENT-01; the
other controls as before), its CARRIED-FORWARD.md (the Tranche-3 base tooling and evidence, C24's preserved build
receipt) and index. The status is derived from the receipts: READY_FOR_MUTATION_TRANCHE3_FINAL_ARCHITECT_REVIEW,
C24_PRESERVED_ARTIFACT_BLOCKED, MUTATION_CONTROL_BLOCKED or BLOCKED.

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
RESULT_ARTIFACT = "CONTROL RESULT: C24_PRESERVED_ARTIFACT_BLOCKED"
STATUSES = ("READY_FOR_MUTATION_TRANCHE3_FINAL_ARCHITECT_REVIEW", "C24_PRESERVED_ARTIFACT_BLOCKED",
            "MUTATION_CONTROL_BLOCKED", "BLOCKED")


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
    pre = (all(lines(os.path.join(K.EV, r))[:1] == [K.RECORD_FIRST[r]] for r in K.RECORDS)
           and lines(os.path.join(R, "06-PRE-EXECUTION-GATE.txt"))[:1] == ["PRE-EXECUTION GATE: ALL CHECKS HOLD"])
    results = [lines(os.path.join(K.EV, exp["controls"][c]["dir"], "RESULT.txt"))[:1] for c in K.ORDER]
    if pre and all(r == [RESULT_OK] for r in results):
        return "READY_FOR_MUTATION_TRANCHE3_FINAL_ARCHITECT_REVIEW", results
    if pre and results[0] == [RESULT_ARTIFACT]:
        return "C24_PRESERVED_ARTIFACT_BLOCKED", results
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
    # TRANCHE3-CONTINUE-02: C24's pin is its preserved-executable pin record; C3b's/C3c's the admission's when it wrote one.
    pin = (jload(os.path.join(d, "03-pin/PIN.json")) or jload(os.path.join(d, "03a-admission/PIN.json"))
           or jload(os.path.join(d, "03-build/PIN.json")))
    restore = lines(os.path.join(d, "05-restore/RESTORE.txt")) or lines(os.path.join(d, "05-reauth/REAUTH.txt"))
    admission = lines(os.path.join(d, "01-admission/ADMISSION.txt")) or lines(os.path.join(d, "03a-admission/ADMISSION.txt"))
    rules = lines(os.path.join(d, "04a-rules/RULES.txt"))
    rj = jload(os.path.join(d, "04a-rules/RULES.json")) or {}
    link = lines(os.path.join(d, "06-link/LINK.txt"))
    errs = (build.get("process") or {})
    row = {"id": cid, "dir": c["dir"], "result": res_first[0] if res_first else "NOT COMPLETED",
           "facts": lines(os.path.join(d, "RESULT.txt"))[3:], "gate": (lines(os.path.join(d, "00-GATE.txt")) or ["—"])[0],
           "apply": (lines(os.path.join(d, "02-apply/APPLY.txt")) or [
               "no patch applied by this mission (C24: the preserved Tranche-3 build; its worktree restored to C there)"
               if cid == "C24" else "not reached"])[0],
           "type": c["receipt_type"], "patch_sha256": c["patch"]["sha256"], "mutant_blob": c["mutant_blob"],
           "mutant_sha256": c["mutant_sha256"], "file": c["source_file"],
           "build": (f"{bcls[0]} — {bcls[1]}" if bcls[0] != "NOT PRODUCED" else
                     "not rebuilt (the preserved Tranche-3 build; see the admission)" if cid == "C24" else "not reached"),
           "admission": admission[0] if admission else ("—" if cid not in ("C24", "C3b", "C3c") else "not reached"),
           "build_raw": (f"exit {errs.get('exit_status')}, signal {errs.get('signal')}; Finished {build.get('finished')}"
                         if build else "—"),
           "build_resources": res(build.get("supervisor")),
           "executable": (f"`{pin['path']}` sha256 `{pin['sha256']}` size {pin['size']:,}" if pin and pin.get("sha256")
                          and isinstance(pin.get("size"), int) else "not produced"),
           "selection": " ".join(c["argv_tail"]),
           "config": f"{c['configuration']} (`{c['test']}`, original explorer"
                     + (", A4 ON" if cid in K.A4_ON else "") + ")",
           "restore": restore[0] if restore else "NOT PRODUCED", "link": (link[0] + " — " + link[1]) if link else "—",
           "named": c.get("permitted_properties") and " or ".join(c["permitted_properties"]) or c["named_property"],
           "ceiling": f"{c['ceiling_states']:,}", "rules": rules[0] if rules else "not reached",
           "rules_failed": [l for l in rules if l.startswith("FAIL")]}
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
        if cid in K.L1_CONTROLS:
            ok = rj.get("holds") is True
            row["complete"] = (f"{'yes' if ok else 'NOT ESTABLISHED'}: outcome {rep.get('outcome')}; INCOMPLETE line "
                               f"{rep.get('incomplete')}; discovered {rep.get('states')} = expanded {rep.get('expanded')}"
                               f" (every discovered state expanded) of {c['ceiling_states']:,}; elapsed "
                               f"{rep.get('elapsed_s')} s of {c['limit_seconds']} s; {row['rules']}")
    else:
        row.update(raw="not run", printed="—", count="—", outer="not run", run_first="not run", exit="—",
                   run_resources="—")
    if cid == "C7":
        inf = jload(os.path.join(d, c["informational"]["dir"], "PARSED-RECEIPT.json"))
        icls = outer(os.path.join(d, c["informational"]["dir"], "CLASSIFICATION.txt")) if inf else None
        if inf:
            irep = inf.get("report") or {}
            row["info"] = (f"{icls[0]} — raw outcome {irep.get('outcome')}; FAIL {irep.get('fail')}; INCOMPLETE "
                           f"{irep.get('incomplete')}; {irep.get('states'):,} discovered (expanded "
                           f"{irep.get('expanded'):,}) of {c['informational']['ceiling_states']:,}; exit "
                           f"{inf['process'].get('exit_status')}, signal {inf['process'].get('signal')}; "
                           f"{res(inf.get('supervisor'))}" if irep.get("states") is not None else
                           f"{icls[0]} — {icls[1]}")
        else:
            row["info"] = "not reached (it follows only an admissible F-PT/P6 candidate, before restoration)"
    return row


def main(outdir):
    exp = json.load(open(os.path.join(R, "02-EXPECTATIONS.json")))
    st, results = status(exp)
    rows = [r for r in (control_row(exp, c) for c in K.ORDER) if r]
    attempted = [r["id"] for r in rows]
    candidates = [r["id"] for r in rows if r["result"] == RESULT_OK]
    blocked = [r["id"] for r in rows if r["result"] != RESULT_OK]
    not_attempted = [c for c in K.ORDER if c not in attempted]
    n_acc = len(K.ACCEPTED)
    s = [f"# Tranche-3 continuation summary — {K.MISSION}", "",
         f"Derived status: **{st}** (from the receipts).", "",
         f"- Authorized (mission section 6), in order: {', '.join(K.ORDER)}.",
         f"- Attempted: {', '.join(attempted) or 'none'}.",
         f"- New executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`): **{len(candidates)} of "
         f"{len(K.ORDER)}** ({', '.join(candidates) or 'none'}). None is accepted here: acceptance belongs to the Sole "
         "Architect.",
         f"- Stopped (first unsuccessful control; nothing later was attempted): {', '.join(blocked) or 'none'}.",
         f"- Authorized but not attempted: {', '.join(not_attempted) or 'none'}.",
         "", "## Campaign status before Architect review", "",
         f"- Architect-accepted kills: **{n_acc} of {K.KILL_REQUIRED}** (Tranche 1: {', '.join(K.ACCEPTED_T1)}; "
         f"Tranche 2: {', '.join(K.ACCEPTED_T2)}; Tranche 2 continuation: {', '.join(K.ACCEPTED_T2C)}; Tranche 3: "
         f"{', '.join(K.ACCEPTED_T3)}; record `{K.RULING}`), unchanged by this mission.",
         f"- Executor candidates pending Architect review from this mission: {len(candidates)} "
         f"({', '.join(candidates) or 'none'}); if every one were accepted the count would be {n_acc + len(candidates)} "
         f"of {K.KILL_REQUIRED} (not claimed).",
         f"- Reserved for the final witness-bound tranche (never executed here): {', '.join(K.NOT_AUTHORIZED)}. A4: "
         "SATISFIED — ARCHITECT ACCEPTED (separately; not rerun). M01–M183: not executed. Unchanged-core "
         "configurations: linked, never rerun.",
         "- C7's run without A4 on F-3 (mission section 12) is informational: it is not C7's kill receipt and counts "
         "for nothing in the campaign.",
         f"- Build rulings (`{K.W_RULING}`, `{K.AMENDMENT}`): C24 is not rebuilt (its preserved Tranche-3 build is "
         "admitted under the narrow C24 ruling); C3b and C3c admit a clean build or exactly their one predicted warning "
         "group (AMENDMENT-01: permitted, not required); C6, C7, A1, A2, C20 and A5 require zero warnings and zero "
         "errors. The general build classifier is unchanged.",
         "- Not claimed: no complete campaign, no D2A-1 acceptance, no integration, no Phase Three completion.", "",
         "## Controls (mission order)", ""]
    for r in rows:
        s += [f"### {r['id']} — `{r['dir']}`", "", f"- result: **{r['result']}**", f"- gate: {r['gate']}",
              f"- receipt type: {r['type']}; configuration {r['config']}; permitted property {r['named']}; ceiling "
              f"{r['ceiling']} states",
              f"- frozen patch sha256 `{r['patch_sha256']}`; mutated file `{r['file']}`; mutant blob `{r['mutant_blob']}` "
              f"(sha256 `{r['mutant_sha256']}`; computed in-process, never written to any object store)",
              f"- apply: {r['apply']}", f"- build: {r['build']}; {r['build_raw']}",
              f"- build admission (C24: the narrow C24 ruling; C3b/C3c: AMENDMENT-01; every other control: none, the "
              f"general classifier alone): {r['admission']}",
              f"- build resources: {r['build_resources']}", f"- mutant executable: {r['executable']}",
              f"- exact selection: `{r['selection']}`", f"- raw outcome (as printed): {r['raw']}; {r['exit']}",
              f"- printed property: {r['printed']}", f"- discovered count: {r['count']}",
              f"- outer classification: {r['run_first']} — {r['outer']}",
              f"- tranche-3 rules: {r['rules']}" + (f" (failed: {'; '.join(r['rules_failed'])[:400]})"
                                                    if r["rules_failed"] else "")]
        if "complete" in r:
            s.append(f"- L1 complete search (AD-1-R4 Q): {r['complete']}")
        if "info" in r:
            s.append(f"- C7 informational run without A4 on F-3 (not the kill receipt): {r['info']}")
        s += [f"- run resources: {r['run_resources']}", f"- restoration: {r['restore']}",
              f"- restored-side link: {r['link']}", "", "  RESULT facts:", ""]
        s += [f"  - {x}" for x in r["facts"]] + [""]
    open(os.path.join(outdir, "TRANCHE-SUMMARY.md"), "w").write("\n".join(s).rstrip("\n") + "\n")
    # CARRIED-FORWARD.md: every immutable file this mission authenticates, links, gates on or runs unchanged.
    cf = [f"# Referenced, not republished — {K.MISSION}", "",
          "Every file below is immutable evidence at the named commit. This mission authenticated it (receipts/"
          "00-authentication.txt), linked it, gated on it or ran it unchanged; it is referenced here by commit, path, git "
          "blob and sha256 and not copied into this packet.", ""]
    t3xtools = {l.split("  ", 1)[1][6:] for l in K.git("show", f"{K.T3X}:{K.T3X_PACKET}/SHA256SUMS").decode().splitlines()
                if l.split("  ", 1)[1].startswith("tools/")}
    carried = {}
    for line in K.git("show", f"{K.T3X}:{K.T3X_PACKET}/CARRIED-FORWARD.md").decode().split(
            "## The old versions of the changed tools")[0].splitlines():
        cells = [x.strip().strip("`") for x in line.split("|")[1:-1]]
        if len(cells) == 5 and cells[0] not in ("Tool", "---"):
            carried[cells[0]] = (cells[2], cells[1])

    def tool_ref(t):
        if t in t3xtools:
            return K.T3X, f"{K.T3X_PACKET}/tools/{t}"
        return carried[t]
    for title, same in (("Tools run unchanged (carried byte for byte from the Tranche-3 tooling)", True),
                        ("The old versions of the changed tools (the base of receipts/04-tool-diff)", False),):
        cf += [f"## {title}", "", "| Tool | Packet path | Commit | Blob | sha256 |", "|---|---|---|---|---|"]
        for t in sorted(os.listdir(os.path.join(K.EV, "original-tools"))):
            n = os.path.join(K.EV, "tools", t)
            eq = os.path.exists(n) and K.sha256_file(n) == K.sha256_file(os.path.join(K.EV, "original-tools", t))
            if eq == same and (same or os.path.exists(n)):
                commit, path = tool_ref(t)
                b, h = ref(commit, path)
                cf.append(f"| `{t}` | `{path}` | `{commit}` | `{b}` | `{h}` |")
        cf.append("")
    cf += ["## Base tools not carried into this mission", "",
           "| Tool | Packet path | Commit | Blob | sha256 |", "|---|---|---|---|---|"]
    for t in sorted(os.listdir(os.path.join(K.EV, "original-tools"))):
        if not os.path.exists(os.path.join(K.EV, "tools", t)):
            commit, path = tool_ref(t)
            b, h = ref(commit, path)
            cf.append(f"| `{t}` | `{path}` | `{commit}` | `{b}` | `{h}` |")
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
    for (commit, d), cids in sorted(linked.items(), key=lambda x: x[0][1]):
        for f in ("CLASSIFICATION.txt", "descriptor.json", "raw-output.txt"):
            b, h = ref(commit, f"{d}/{f}")
            cf.append(f"| {', '.join(cids)} | `{d}/{f}` | `{commit}` | `{b}` | `{h}` |")
    cf += ["", "## The accepted evidence of earlier tranches", "",
           "| Item | Path | Commit | Blob | sha256 |", "|---|---|---|---|---|"]
    for name, packet in K.T1_PACKETS:
        b, h = ref(K.T1, f"{packet}/SHA256SUMS")
        cf.append(f"| {name} packet manifest | `{packet}/SHA256SUMS` | `{K.T1}` | `{b}` | `{h}` |")
    for cid in K.ACCEPTED_T1:
        packet, d = K.T1_RECEIPTS[cid]
        b, h = ref(K.T1, f"{packet}/{d}/RESULT.txt")
        cf.append(f"| {cid} RESULT (historical label; Architect-accepted) | `{packet}/{d}/RESULT.txt` | `{K.T1}` | `{b}` | "
                  f"`{h}` |")
    b, h = ref(K.T2, f"{K.T2_PACKET}/SHA256SUMS")
    cf.append(f"| TRANCHE2-EXEC-01 packet manifest | `{K.T2_PACKET}/SHA256SUMS` | `{K.T2}` | `{b}` | `{h}` |")
    for cid in K.ACCEPTED_T2:
        path = f"{K.T2_PACKET}/{K.T2_RECEIPTS[cid]}/RESULT.txt"
        b, h = ref(K.T2, path)
        cf.append(f"| {cid} RESULT (historical label; Architect-accepted) | `{path}` | `{K.T2}` | `{b}` | `{h}` |")
    b, h = ref(K.T2C, f"{K.T2C_PACKET}/SHA256SUMS")
    cf.append(f"| TRANCHE2-CONTINUE-02 packet manifest | `{K.T2C_PACKET}/SHA256SUMS` | `{K.T2C}` | `{b}` | `{h}` |")
    for cid in K.ACCEPTED_T2C:
        path = f"{K.T2C_PACKET}/{K.T2C_RECEIPTS[cid]}/RESULT.txt"
        b, h = ref(K.T2C, path)
        cf.append(f"| {cid} RESULT (historical label; Architect-accepted) | `{path}` | `{K.T2C}` | `{b}` | `{h}` |")
    b, h = ref(K.T3X, f"{K.T3X_PACKET}/SHA256SUMS")
    cf.append(f"| TRANCHE3-EXEC-01 packet manifest | `{K.T3X_PACKET}/SHA256SUMS` | `{K.T3X}` | `{b}` | `{h}` |")
    for cid in K.ACCEPTED_T3:
        path = f"{K.T3X_PACKET}/{K.T3X_RECEIPTS[cid]}/RESULT.txt"
        b, h = ref(K.T3X, path)
        cf.append(f"| {cid} RESULT (historical label; Architect-accepted) | `{path}` | `{K.T3X}` | `{b}` | `{h}` |")
    cf += ["", "## C24's preserved Tranche-3 build (admitted under the narrow C24 ruling; byte copies authenticated under "
           "inputs/c24/, not republished)", "", "| File | Path | Commit | Blob | sha256 |", "|---|---|---|---|---|"]
    for f in ["02-apply/APPLY.json", "02-apply/APPLY.txt"] + [f"03-build/{x}" for x in (
            "CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json", "RECEIPT.txt", "build.stderr", "build.stdout",
            "cgroup-samples.txt", "descriptor.json", "scope-record.txt", "time-v.txt")] + ["RESULT.txt"]:
        path = f"{K.T3X_PACKET}/{K.T3X_RECEIPTS['C24']}/{f}"
        b, h = ref(K.T3X, path)
        cf.append(f"| C24 {f} | `{path}` | `{K.T3X}` | `{b}` | `{h}` |")
    open(os.path.join(outdir, "CARRIED-FORWARD.md"), "w").write("\n".join(cf).rstrip("\n") + "\n")
    fr = jload(os.path.join(R, "05-FREEZE.json")) or {}
    frs = K.sha256_file(os.path.join(R, "05-FREEZE.json")) if os.path.exists(os.path.join(R, "05-FREEZE.json")) else "none"
    first_dir, last_dir = exp["controls"][K.ORDER[0]]["dir"], exp["controls"][K.ORDER[-1]]["dir"]
    i = [f"# {K.MISSION} — evidence packet", "",
         "Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. Evidence only: no source, catalog, "
         "patch, contract, amendment or manifest is changed; no mutant source, executable, target directory or source "
         "copy is committed; the previous packets are referenced, not modified or duplicated.", "",
         f"Derived status: **{st}**. This mission attempted {len(attempted)} of {len(K.ORDER)} controls; new executor "
         f"candidates {len(candidates)} ({', '.join(candidates) or 'none'}), each **KILL_EVIDENCE_CANDIDATE — ARCHITECT "
         f"REVIEW PENDING**; stopped at {', '.join(blocked) or 'none'}; not attempted {', '.join(not_attempted) or 'none'}. "
         f"Architect-accepted before this mission: {n_acc} of {K.KILL_REQUIRED} (unchanged here).", "",
         "## Identities", "",
         f"- Candidate C `{K.C}`, tree `{K.C_TREE}`, parent `{K.C_PARENT}`; protocol.rs `{K.BLOBS[K.PROTOCOL]}`, "
         f"model_tests.rs `{K.BLOBS[K.MODEL]}`, unit_tests.rs `{K.BLOBS[K.UNIT]}`",
         f"- frozen catalog `{K.CATALOG_PATH}` blob `{K.CATALOG_BLOB}` sha256 `{K.CATALOG_SHA}` at `{K.R2}`",
         f"- accepted unchanged-core evidence: Stage 1 `{K.STAGE1}`, Stage 2 `{K.STAGE2}`",
         f"- accepted Tranche-1 evidence: commit `{K.T1}`, tree `{K.T1_TREE}`, ref `{K.T1_REF}`",
         f"- Tranche-2 stopped evidence: commit `{K.T2}`, tree `{K.T2_TREE}`, ref `{K.T2_REF}`",
         f"- accepted Tranche-2 continuation evidence: commit `{K.T2C}`, tree `{K.T2C_TREE}`, parent `{K.T2C_PARENT}`, "
         f"ref `{K.T2C_REF}`",
         f"- Tranche-3 stopped evidence (C2 Architect-accepted; C24 stopped at its build): commit `{K.T3X}`, tree "
         f"`{K.T3X_TREE}`, parent `{K.T3X_PARENT}`, ref `{K.T3X_REF}`",
         f"- C24's preserved executable (not rebuilt) `{K.C24_EXE}` sha256 `{K.C24_EXE_SHA256}` size {K.C24_EXE_SIZE:,}; "
         f"its preserved build receipt `{K.C24_BUILD_DIR}/` at `{K.T3X}`",
         f"- pinned unchanged-baseline executable (the linked receipts') `{K.EXE}` sha256 `{K.EXE_SHA256}`",
         f"- this evidence commit's parent: `{K.PARENT}`; review ref `{K.DEST}`", "",
         "## Contents", "", "| Path | What |", "|---|---|",
         "| `INDEX.md` | this file |",
         "| `TRANCHE-SUMMARY.md` | every attempted control: identities, build, executable, selection, raw and outer "
         "outcome, printed property, discovered count, the tranche-3 rules, L1's complete-search evidence (C20, A5), "
         "C7's informational run, resources, restoration, restored-side link; the campaign status before Architect "
         "review |",
         "| `CARRIED-FORWARD.md` | every immutable file referenced, linked, gated on or run unchanged (commit, path, blob, "
         "sha256; not republished) |",
         f"| `{K.RULING}` | record of the Architect's acceptance of C2 (23 of 35) with the exact evidence identities, and "
         "this mission's authorization |",
         f"| `{K.W_RULING}` | record of the Architect's build rulings: C24 (narrow, preserved build), C3b and C3c "
         "(conditional), no general warning exception |",
         f"| `{K.AMENDMENT}` | record of the Architect's AMENDMENT-01 (C3b/C3c: a clean build or exactly the one predicted "
         "warning group; permitted, not required), issued before any build, and how it arose |",
         f"| `{K.CAMPAIGN}` | the campaign status before this mission (23 of 35) |",
         "| `documents/DUPLICATE-AND-ZERO-LENGTH-MAP.tsv` (+ `.md`) | every packet file byte-equal to a previous-packet "
         "file and every empty file, each with its exact path, class, role, descriptor and prior occurrences |",
         "| `receipts/00-authentication.txt` | provenance of every input, including every previous packet |",
         "| `receipts/01-supervision-selftest/` | the unchanged enforcement chain on non-Nexus processes |",
         "| `receipts/02-EXPECTATIONS.json` (+ console) | every expectation, derived from the frozen catalog |",
         "| `receipts/03-t3-controls/` | tooling controls on synthetic and historical evidence (every required negative "
         "control) |",
         "| `receipts/04-tool-diff/` | the old-to-new tool diff, its authorization map and mechanical check |",
         "| `receipts/04b-rehearsals/` | render.py and build_packet.py on synthetic evidence roots |",
         "| `receipts/05-FREEZE.json`, `05-FREEZE-RECORD.txt` | the freeze |",
         "| `receipts/06-PRE-EXECUTION-GATE.txt` (+ `06-gate-authentication.txt`) | the static pre-execution gate |",
         f"| `{first_dir}/` … `{last_dir}/` | one directory per attempted control. C24: gate, admission of the preserved "
         "build, preserved worktree, pin, run, tranche-3 rules, reauthentication, link, RESULT. Every other control: "
         "gate, worktree, apply, build, (C3b/C3c) build admission, run, tranche-3 rules, (C7) informational F-3 run, "
         "restore, link, RESULT |",
         "| `receipts/80-STOP-RECORD.md` | present only when the mission stopped |",
         "| `receipts/90-final-gate.txt` (+ `90-gate-authentication.txt`) | the final gate |",
         "| `tools/` | the tools this mission changed or added (unchanged ones referenced in `CARRIED-FORWARD.md`) |",
         "| `development/` | the development log, every failing development output, every revision and its diff |",
         "| `publication/10-build-packet.txt` | the packet build's console |",
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
         "- The restored sides are linked, never rerun: the accepted exact-final Stage-1 and Stage-2 receipts; for C6 and "
         "C7 the Stage-1 composed-quotient F-PT PASS (restored side only, AD-1-R4 F). Every mutant run is the original "
         "explorer.",
         "- C24's executable is the preserved Tranche-3 build (not rebuilt); its mutant identity is bound through the "
         "admitted build record and the pinned executable, while its worktree is exactly C (restored in Tranche 3).",
         "- The build-warning rulings are control-specific (C24; C3b and C3c as amended); nothing here is a general "
         "warning exception, and the general build classifier is unchanged.",
         "- C7's run without A4 on F-3 is informational (mission section 12): it is not a kill receipt and establishes "
         "no kill.", ""]
    open(os.path.join(outdir, "INDEX.md"), "w").write("\n".join(i).rstrip("\n") + "\n")
    # The mission is named on the status line: a rehearsal's console must never repeat an earlier mission's bytes.
    print(f"STATUS: {st}; {K.MISSION}; new candidates {len(candidates)} of {len(K.ORDER)}; attempted {attempted}; "
          f"stopped {blocked}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
