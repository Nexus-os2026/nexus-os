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
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-TAIL-01 (sections 9, 11-13): this mission's summary (every attempted
control: identities, build, executable, selection, raw and outer outcome, printed property, raw state counts, the
tranche-3 rules, L1's complete-search evidence for C20 and A5, resources, restoration and restored-side link; the
campaign status: 27 accepted, C7 blocked, this mission's candidates pending), its CARRIED-FORWARD.md (the continuation's
base tooling and evidence, the accepted controls' and C7's receipts, the inputs and linked receipts) and index. The
derived status is one of the mission's section-12 labels, taken from the receipts: KILL_EVIDENCE_CANDIDATE — ARCHITECT
REVIEW PENDING (every control a candidate), MUTATION_CONTROL_BLOCKED or INCOMPLETE — ARCHITECT REVIEW REQUIRED (the
stopping control's own RESULT), or STOPPED BEFORE EXECUTION (no control executed).

Revised by P3-C11-DR01-D2A1-MUTATION-WITNESS-C1-C25-01 (sections 8, 9, 11): this mission's summary (every attempted
control: identities, build, executable, the type-(a) primary with its first property as printed and its raw state
counts, the type-(c) mutant witness with its exact positive line, header and line of other properties on the same pinned
executable, resources, restoration, the linked healthy type-(e3) and unchanged-core type-(e1) receipts, the G.3 record
and its (e) recording proposed for Architect review; the campaign status: 31 accepted, C7 blocked, C4 reserved, this
mission's candidates pending), its CARRIED-FORWARD.md (the tail's base tooling and evidence, the accepted controls' and
C7's receipts, the inputs, the linked type-(e1) and type-(e3) receipts) and index. The derived status is one of the
mission's section-10 labels, taken from the receipts, as before.

Revised by P3-C11-DR01-D2A1-MUTATION-WITNESS-C4-01 (sections 12, 13, 16): this mission's summary for C4 (identities,
build, executable; the type-(a) primary with its first property as printed, never relabelled, and its raw state counts;
the type-(c) mutant witness with both exact positive lines, the header and the two lines of other properties on the same
pinned executable, and the G.6 record of each changed append (offered, consumed with option counts, unused suffix,
faults left); resources; restoration and its authentication; the linked healthy type-(e3) and F-8 type-(e1) receipts;
the G.5 record with (E)'s Architect review pending and (F)'s recording proposed; the campaign status: 33 accepted, C7
blocked, C4 pending), its CARRIED-FORWARD.md (the C1/C25 mission's base tooling and evidence, the accepted controls' and
C7's receipts, the inputs, the linked receipts, both witness sources) and index. The derived status is one of the
mission's section-15 labels, taken from the receipts, as before.

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
STATUSES = ("KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING", "MUTATION_CONTROL_BLOCKED",
            "INCOMPLETE — ARCHITECT REVIEW REQUIRED", "STOPPED BEFORE EXECUTION")


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
        return STATUSES[0], results
    done = [r[0] for r in results if r]
    stop = [x for x in done if x != RESULT_OK]
    if pre and stop and stop[0].startswith("CONTROL RESULT: ") and stop[0][len("CONTROL RESULT: "):] in STATUSES[1:]:
        return stop[0][len("CONTROL RESULT: "):], results  # the stopping control's own RESULT label
    return STATUSES[3], results


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
    wit = jload(os.path.join(d, "04c-witness/PARSED-RECEIPT.json"))
    wcls = outer(os.path.join(d, "04c-witness/CLASSIFICATION.txt")) if wit else None
    healthy = lines(os.path.join(d, "06a-healthy-witness/HEALTHY.txt"))
    g5rec = lines(os.path.join(d, "07-g5/G5.txt"))
    rauth = lines(os.path.join(d, "05b-restore-auth/RESTORE-AUTH.txt"))
    link = lines(os.path.join(d, "06b-link/LINK.txt"))
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
           "executable": (f"`{pin['path']}` sha256 `{pin['sha256']}` size {pin['size']:,}" if pin and pin.get("sha256")
                          and isinstance(pin.get("size"), int) else "not produced"),
           "selection": " ".join(c["argv_tail"]),
           "config": f"{c['configuration']} (`{c['test']}`, original explorer)",
           "restore": restore[0] if restore else "NOT PRODUCED", "link": (link[0] + " — " + link[1]) if link else "—",
           "named": c["named_property"], "ceiling": f"{c['ceiling_states']:,}", "predicted": c["g5"]["predicted"],
           "permitted": " or ".join(c["g5"]["permitted_primary"]),
           "witness_test": c["g5"]["witness"]["test"],
           "positive": [s["positive"] for s in c["g5"]["witness"]["scenarios"]],
           "healthy": (healthy[0] + " — " + healthy[1]) if healthy else "—",
           "restore_auth": rauth[0] if rauth else "not reached",
           "g5": g5rec[0] if g5rec else "not reached",
           "review": ([l for l in g5rec if l.startswith("(E) ")] or ["(E) the Architect's review: not reached"])[0],
           "recording": ([l for l in g5rec if l.startswith("(F) ")] or ["(F) none"])[0]}
    if wit:
        msg = wit.get("panic_message") or []
        row["witness"] = f"{wcls[0]} — {wcls[1]}"
        row["witness_lines"] = msg[:5]
        g6 = []
        for s in c["g5"]["witness"]["scenarios"]:
            st = ((wit.get("transcript") or {}).get("scenarios", {}).get(s["named"]) or {}).get("steps") or []
            if len(st) == s["total"]:
                x = st[-1]
                g6.append(f"scenario {s['named']} step {x['n']}/{x['total']} {x['actor']}: offered {x['offered']}, "
                          f"consumed {[tuple(y) for y in x['consumed']]}, unused {x['unused']}, faults left "
                          f"{x['faults']}{' [fault]' if x['marker'] else ''}; branches {x['branches']}; violations "
                          f"{[(v[0], v[1]) for v in x['violations']]}")
        row["g6"] = g6
        row["g6_problems"] = wit.get("g6_problems")
        row["witness_exit"] = f"exit {wit['process'].get('exit_status')}, signal {wit['process'].get('signal')}; harness " \
                              f"{wit.get('harness_result')}"
        row["witness_exe"] = (lambda e: f"`{e['path']}` sha256 `{e['sha256']}` size {e['size']:,} (before and after)"
                              if e.get("sha256") == (wit["executable"].get("after") or {}).get("sha256") else "CHANGED")(
            wit["executable"].get("before") or {})
        row["witness_resources"] = res(wit.get("supervisor"))
    else:
        row.update(witness="not run", witness_lines=[], witness_exit="—", witness_exe="—", witness_resources="—", g6=[],
                   g6_problems=None)
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
    n_acc = len(K.ACCEPTED)
    s = [f"# C4 witness-bound mutation summary — {K.MISSION}", "",
         f"Derived status: **{st}** (from the receipts).", "",
         f"- Authorized (mission sections 1, 4-14): {', '.join(K.ORDER)} alone, once, under AD-1-R4 G.5 and G.6.",
         f"- Attempted: {', '.join(attempted) or 'none'}.",
         f"- New executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`): **{len(candidates)} of "
         f"{len(K.ORDER)}** ({', '.join(candidates) or 'none'}). None is accepted here: acceptance belongs to the Sole "
         "Architect.",
         f"- Stopped (first unsuccessful control; nothing later was attempted): {', '.join(blocked) or 'none'}.",
         f"- Authorized but not attempted: {', '.join(not_attempted) or 'none'}.",
         "", "## Campaign status before Architect review", "",
         f"- Architect-accepted kills: **{n_acc} of {K.KILL_REQUIRED}** (Tranche 1: {', '.join(K.ACCEPTED_T1)}; "
         f"Tranche 2: {', '.join(K.ACCEPTED_T2)}; Tranche 2 continuation: {', '.join(K.ACCEPTED_T2C)}; Tranche 3: "
         f"{', '.join(K.ACCEPTED_T3)}; Tranche 3 continuation: {', '.join(K.ACCEPTED_T3C)}; Tranche 3 tail: "
         f"{', '.join(K.ACCEPTED_T3T)}; witness-bound: {', '.join(K.ACCEPTED_W1)}; record `{K.RULING}`), unchanged by "
         "this mission.",
         f"- Executor candidates pending Architect review from this mission: {len(candidates)} "
         f"({', '.join(candidates) or 'none'}): {n_acc} accepted + {len(candidates)} pending Architect review. Only the "
         "Architect may rule a higher accepted count (not claimed).",
         f"- MUTATION_CONTROL_BLOCKED (Architect ruling on `{K.T3C}`): {', '.join(K.BLOCKED)} — E11 observed, P6 not "
         "established; not executed here.",
         "- Reserved: none. A4: SATISFIED — ARCHITECT ACCEPTED (separately; not rerun). M01–M183: not executed. The "
         "accepted healthy witness and the unchanged-core configurations: linked, never rerun.",
         "- Build rule (mission section 10): one fresh build with zero warnings and zero errors under the unchanged "
         "general classifier; no warning exception; no second build.",
         "- AD-1-R4 G.5 and G.6 (C4 only): the type-(a) primary's first property is recorded as printed and never "
         "relabelled (P4 or P8 admissible; P4 the prediction, not a receipt; neither inferred from the other); P4 AND "
         "P8 are established only by the type-(c) mutant witness on the same pinned mutant executable, each at its "
         "frozen locus, with G.6 verified from the raw output at every step of both scenarios; the accepted healthy "
         "type-(e3) witness and the accepted F-8 type-(e1) receipt are linked after restoration. INCOMPLETE kills "
         "nothing.",
         "- Not claimed: no accepted control, no D2A-1 acceptance, no Phase Three acceptance, no integration.", "",
         "## Controls (mission order)", ""]
    for r in rows:
        s += [f"### {r['id']} — `{r['dir']}`", "", f"- result: **{r['result']}**", f"- gate: {r['gate']}",
              f"- receipt types: {r['type']}; configuration {r['config']}; named properties {r['named']} (both "
              f"attributed only by the witness); admissible first property of the primary {r['permitted']} (the "
              f"prediction {r['predicted']} is not a receipt); ceiling {r['ceiling']} states",
              f"- frozen patch sha256 `{r['patch_sha256']}`; mutated file `{r['file']}`; mutant blob `{r['mutant_blob']}` "
              f"(sha256 `{r['mutant_sha256']}`; computed in-process, never written to any object store)",
              f"- apply: {r['apply']}", f"- build: {r['build']}; {r['build_raw']}",
              f"- build resources: {r['build_resources']}", f"- mutant executable: {r['executable']}",
              f"- primary (type (a)), exact selection: `{r['selection']}`",
              f"- primary raw outcome (as printed): {r['raw']}; {r['exit']}",
              f"- primary first printed property (recorded as printed; never relabelled; it alone establishes neither "
              f"P4 nor P8): {r['printed']}",
              f"- primary raw state counts: {r['count']}",
              f"- primary classification: {r['run_first']} — {r['outer']}",
              f"- primary resources: {r['run_resources']}",
              f"- mutant witness (type (c)): `{r['witness_test']} --exact --nocapture --test-threads=1` on the same "
              f"pinned mutant executable: {r['witness_exe']}",
              f"- mutant witness raw lines (as printed): " + ("; ".join(f"`{x}`" for x in r["witness_lines"]) or "—"),
              f"- mutant witness process: {r['witness_exit']}",
              f"- mutant witness classification: {r['witness']}",
              f"- mutant witness resources: {r['witness_resources']}",
              "- G.6 at each changed append (from the raw output): " + ("; ".join(r["g6"]) or "—")
              + f"; G.6 problems {r['g6_problems']}",
              f"- restoration: {r['restore']}; post-restoration authentication: {r['restore_auth']}",
              f"- healthy witness (type (e3), linked): {r['healthy']}",
              f"- unchanged-core receipt (type (e1), linked): {r['link']}",
              f"- G.5: {r['g5']}", f"- {r['review']}", f"- {r['recording']}", "", "  RESULT facts:", ""]
        s += [f"  - {x}" for x in r["facts"]] + [""]
    open(os.path.join(outdir, "TRANCHE-SUMMARY.md"), "w").write("\n".join(s).rstrip("\n") + "\n")
    # CARRIED-FORWARD.md: every immutable file this mission authenticates, links, gates on or runs unchanged.
    cf = [f"# Referenced, not republished — {K.MISSION}", "",
          "Every file below is immutable evidence at the named commit. This mission authenticated it (receipts/"
          "00-authentication.txt), linked it, gated on it or ran it unchanged; it is referenced here by commit, path, git "
          "blob and sha256 and not copied into this packet.", ""]
    w1tools = {l.split("  ", 1)[1][6:] for l in K.git("show", f"{K.W1}:{K.W1_PACKET}/SHA256SUMS").decode().splitlines()
               if l.split("  ", 1)[1].startswith("tools/")}
    carried = {}
    for line in K.git("show", f"{K.W1}:{K.W1_PACKET}/CARRIED-FORWARD.md").decode().split(
            "## The old versions of the changed tools")[0].splitlines():
        cells = [x.strip().strip("`") for x in line.split("|")[1:-1]]
        if len(cells) == 5 and cells[0] not in ("Tool", "---"):
            carried[cells[0]] = (cells[2], cells[1])

    def tool_ref(t):
        if t in w1tools:
            return K.W1, f"{K.W1_PACKET}/tools/{t}"
        return carried[t]
    for title, same in (("Tools run unchanged (carried byte for byte from the C1/C25 mission's tooling)", True),
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
    cf += ["", "## Accepted healthy type-(e3) witness receipt linked (AD-1-R4 G.5(C); never rerun)", "",
           "| For | Receipt file | Commit | Blob | sha256 |", "|---|---|---|---|---|"]
    for cid in K.ORDER:
        hw = exp["controls"][cid]["g5"]["healthy"]
        for f in sorted(os.listdir(os.path.join(K.EV, hw["input_dir"]))):
            b, h = ref(hw["commit"], f"{hw['directory']}/{f}")
            cf.append(f"| {cid} | `{hw['directory']}/{f}` | `{hw['commit']}` | `{b}` | `{h}` |")
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
    b, h = ref(K.T3C, f"{K.T3C_PACKET}/SHA256SUMS")
    cf.append(f"| TRANCHE3-CONTINUE-02 packet manifest | `{K.T3C_PACKET}/SHA256SUMS` | `{K.T3C}` | `{b}` | `{h}` |")
    for cid in K.ACCEPTED_T3C:
        path = f"{K.T3C_PACKET}/{K.T3C_RECEIPTS[cid]}/RESULT.txt"
        b, h = ref(K.T3C, path)
        cf.append(f"| {cid} RESULT (historical label; Architect-accepted) | `{path}` | `{K.T3C}` | `{b}` | `{h}` |")
    b, h = ref(K.T3T, f"{K.T3T_PACKET}/SHA256SUMS")
    cf.append(f"| TRANCHE3-TAIL-01 packet manifest | `{K.T3T_PACKET}/SHA256SUMS` | `{K.T3T}` | `{b}` | `{h}` |")
    for cid in K.ACCEPTED_T3T:
        path = f"{K.T3T_PACKET}/{K.T3T_RECEIPTS[cid]}/RESULT.txt"
        b, h = ref(K.T3T, path)
        cf.append(f"| {cid} RESULT (historical label; Architect-accepted) | `{path}` | `{K.T3T}` | `{b}` | `{h}` |")
    b, h = ref(K.W1, f"{K.W1_PACKET}/SHA256SUMS")
    cf.append(f"| WITNESS-C1-C25-01 packet manifest | `{K.W1_PACKET}/SHA256SUMS` | `{K.W1}` | `{b}` | `{h}` |")
    for cid in K.ACCEPTED_W1:
        for f in ("RESULT.txt", "04-run/CLASSIFICATION.txt", "04c-witness/CLASSIFICATION.txt", "07-g3/G3.txt"):
            path = f"{K.W1_PACKET}/{K.W1_RECEIPTS[cid]}/{f}"
            b, h = ref(K.W1, path)
            cf.append(f"| {cid} {f} (historical label; Architect-accepted) | `{path}` | `{K.W1}` | `{b}` | `{h}` |")
    cf += ["", "## C7 — MUTATION_CONTROL_BLOCKED (Architect ruling; preserved as it stands; not a kill candidate)", "",
           "| File | Path | Commit | Blob | sha256 |", "|---|---|---|---|---|"]
    for f in ("RESULT.txt", "04-run/CLASSIFICATION.txt", "04-run/PARSED-RECEIPT.json", "04-run/raw-output.txt",
              "04a-rules/RULES.txt", "05-restore/RESTORE.txt"):
        path = f"{K.T3C_PACKET}/{K.T3C_RECEIPTS['C7']}/{f}"
        b, h = ref(K.T3C, path)
        cf.append(f"| C7 {f} | `{path}` | `{K.T3C}` | `{b}` | `{h}` |")
    cf += ["", "## The C1/C25 mission's records", "", "| Record | Path | Commit | Blob | sha256 |", "|---|---|---|---|---|"]
    for rel in (K.RULING_W1, K.CAMPAIGN_W1, "INDEX.md", "TRANCHE-SUMMARY.md"):
        b, h = ref(K.W1, f"{K.W1_PACKET}/{rel}")
        cf.append(f"| {rel} | `{K.W1_PACKET}/{rel}` | `{K.W1}` | `{b}` | `{h}` |")
    cf += ["", "## The reviewed witness source (AD-1-R4 G.1; held by Candidate C byte for byte)", "",
           "| Item | Path | Commit | Blob | sha256 |", "|---|---|---|---|---|"]
    b, h = ref("42dd1bd1cf21ffdea5968b256d2983f6575fab5b", K.MODEL)
    cf.append(f"| witness source commit (model_tests.rs, +277/-0) | `{K.MODEL}` | "
              f"`42dd1bd1cf21ffdea5968b256d2983f6575fab5b` | `{b}` | `{h}` |")
    a_full = K.git("rev-parse", "04e42a7d").decode().strip()
    b, h = ref(a_full, K.MODEL)
    cf.append(f"| C4 witness source commit A (model_tests.rs, +678/-0; blob equal to Candidate C's) | `{K.MODEL}` | "
              f"`{a_full}` | `{b}` | `{h}` |")
    open(os.path.join(outdir, "CARRIED-FORWARD.md"), "w").write("\n".join(cf).rstrip("\n") + "\n")
    fr = jload(os.path.join(R, "05-FREEZE.json")) or {}
    frs = K.sha256_file(os.path.join(R, "05-FREEZE.json")) if os.path.exists(os.path.join(R, "05-FREEZE.json")) else "none"
    first_dir, last_dir = exp["controls"][K.ORDER[0]]["dir"], exp["controls"][K.ORDER[-1]]["dir"]
    i = [f"# {K.MISSION} — evidence packet", "",
         "Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. Evidence only: no source, catalog, "
         "patch, contract or manifest is changed; no mutant source, executable, target directory or source copy is "
         "committed; the previous packets are referenced, not modified or duplicated.", "",
         f"Derived status: **{st}**. This mission attempted {len(attempted)} of {len(K.ORDER)} controls; new executor "
         f"candidates {len(candidates)} ({', '.join(candidates) or 'none'}), each **KILL_EVIDENCE_CANDIDATE — ARCHITECT "
         f"REVIEW PENDING**; stopped at {', '.join(blocked) or 'none'}; not attempted {', '.join(not_attempted) or 'none'}. "
         f"Architect-accepted before this mission: {n_acc} of {K.KILL_REQUIRED} (unchanged here; a C4 candidate is "
         f"pending Architect review, never accepted here). C7: MUTATION_CONTROL_BLOCKED (Architect ruling): not "
         "executed.", "",
         "## Identities", "",
         f"- Candidate C `{K.C}`, tree `{K.C_TREE}`, parent `{K.C_PARENT}`; protocol.rs `{K.BLOBS[K.PROTOCOL]}`, "
         f"model_tests.rs `{K.BLOBS[K.MODEL]}`, unit_tests.rs `{K.BLOBS[K.UNIT]}`",
         f"- frozen catalog `{K.CATALOG_PATH}` blob `{K.CATALOG_BLOB}` sha256 `{K.CATALOG_SHA}` at `{K.R2}`",
         f"- accepted unchanged-core evidence: Stage 1 `{K.STAGE1}`, Stage 2 `{K.STAGE2}`",
         f"- accepted Tranche-1 evidence: commit `{K.T1}`, tree `{K.T1_TREE}`, ref `{K.T1_REF}`",
         f"- Tranche-2 stopped evidence: commit `{K.T2}`, tree `{K.T2_TREE}`, ref `{K.T2_REF}`",
         f"- accepted Tranche-2 continuation evidence: commit `{K.T2C}`, tree `{K.T2C_TREE}`, parent `{K.T2C_PARENT}`, "
         f"ref `{K.T2C_REF}`",
         f"- Tranche-3 stopped evidence (C2 Architect-accepted): commit `{K.T3X}`, tree `{K.T3X_TREE}`, parent "
         f"`{K.T3X_PARENT}`, ref `{K.T3X_REF}`",
         f"- Tranche-3 continuation evidence (C24 C3b C3c C6 Architect-accepted; C7 MUTATION_CONTROL_BLOCKED): commit "
         f"`{K.T3C}`, tree `{K.T3C_TREE}`, parent `{K.T3C_PARENT}`, ref `{K.T3C_REF}`",
         f"- Tranche-3 tail evidence (A1 A2 C20 A5 Architect-accepted): commit `{K.T3T}`, tree `{K.T3T_TREE}`, parent "
         f"`{K.T3T_PARENT}`, ref `{K.T3T_REF}`",
         f"- C1/C25 witness evidence (C1 C25 Architect-accepted): commit `{K.W1}`, tree `{K.W1_TREE}`, parent "
         f"`{K.W1_PARENT}`, ref `{K.W1_REF}`",
         f"- accepted healthy type-(e3) witness receipt (Stage 2 `{K.STAGE2}`): "
         + ", ".join(f"`{exp['controls'][c]['g5']['healthy']['directory']}`" for c in K.ORDER)
         + f"; accepted F-8 type-(e1) receipt: "
         + ", ".join(f"`{exp['controls'][c]['linked']['directory']}`" for c in K.ORDER),
         f"- pinned unchanged-baseline executable (the linked receipts') `{K.EXE}` sha256 `{K.EXE_SHA256}`",
         f"- this evidence commit's parent: `{K.PARENT}`; review ref `{K.DEST}`", "",
         "## Contents", "", "| Path | What |", "|---|---|",
         "| `INDEX.md` | this file |",
         "| `TRANCHE-SUMMARY.md` | C4: identities, build, executable, the type-(a) primary (first property as printed, "
         "raw state counts), the type-(c) mutant witness (both exact positive lines, header, the two lines of other "
         "properties; the same pinned executable; G.6 at each changed append), resources, restoration and its "
         "authentication, the linked type-(e3) and type-(e1) receipts, the G.5 record; the campaign status before "
         "Architect review |",
         "| `CARRIED-FORWARD.md` | every immutable file referenced, linked, gated on or run unchanged (commit, path, blob, "
         "sha256; not republished), including the accepted controls' and C7's blocked receipts and the healthy "
         "witness receipts |",
         f"| `{K.RULING}` | record of the Architect's ruling: C1 and C25 accepted (33 of 35); C7 "
         "MUTATION_CONTROL_BLOCKED; this mission's authorization (C4, under G.5 and G.6) |",
         f"| `{K.CAMPAIGN}` | the campaign status before this mission (33 of 35) |",
         "| `documents/DUPLICATE-AND-ZERO-LENGTH-MAP.tsv` (+ `.md`) | every packet file byte-equal to a previous-packet "
         "file and every empty file, each with its exact path, class, role, descriptor and prior occurrences |",
         "| `receipts/00-authentication.txt` | provenance of every input, including every previous packet |",
         "| `receipts/01-supervision-selftest/` | the unchanged enforcement chain on non-Nexus processes |",
         "| `receipts/02-EXPECTATIONS.json` (+ console) | every expectation, derived from the frozen catalog |",
         "| `receipts/03-t3-controls/` | tooling controls on synthetic, historical and authenticated evidence (every "
         "required positive and negative control of mission section 11, G.5 and G.6 included) |",
         "| `receipts/04-tool-diff/` | the old-to-new tool diff, its authorization map and mechanical check |",
         "| `receipts/04b-rehearsals/` | render.py and build_packet.py on synthetic evidence roots, and the stopped-first-"
         "control path |",
         "| `receipts/05-FREEZE.json`, `05-FREEZE-RECORD.txt` | the freeze |",
         "| `receipts/06-PRE-EXECUTION-GATE.txt` (+ `06-gate-authentication.txt`) | the static pre-execution gate |",
         f"| `{first_dir}/` | C4: gate, worktree, apply, build, 04-run (primary), 04c-witness (mutant witness), "
         "05-restore, 05b-restore-auth, 06a-healthy-witness (e3), 06b-link (e1), 07-g5, RESULT |",
         "| `receipts/80-STOP-RECORD.md` | present only when the mission stopped |",
         "| `receipts/90-final-gate.txt` (+ `90-gate-authentication.txt`) | the final gate |",
         "| `tools/` | the tools this mission changed (unchanged ones referenced in `CARRIED-FORWARD.md`) |",
         "| `development/` | the development log, every failing development output, every revision and its diff |",
         "| `publication/10-build-packet.txt` | the packet build's console |",
         "| `SHA256SUMS` | every packet file but itself |", "",
         "## Freeze", "",
         f"Frozen files: {len(fr.get('files', {}))}; recorded {fr.get('recorded')}; 05-FREEZE.json sha256 `{frs}`. Every "
         "supervised step re-checks the freeze.", "",
         "## Limitations", "",
         "- Every classification here is an executor result for Architect review; no kill is accepted by the executor.",
         "- Results are the explored model's results on Candidate C's mutants, under AD-1-R4 J's storage assumptions; "
         "nothing here is a filesystem, network or OS-containment claim.",
         "- Reproducible builds are not established (AD-1-R4 I); every identity is the sha256 of the executable actually "
         "built and run.",
         "- The restored sides are linked, never rerun: the accepted exact-final Stage-2 F-8 receipt (type (e1)) and "
         "the accepted Stage-2 C4 healthy witness (type (e3)). The primary is the original explorer; no quotient, "
         "symmetry reduction, changed predicate, changed Oracle, changed schedule or instrumentation is used on the "
         "mutant.",
         "- AD-1-R4 G.5 and G.6 apply to C4 only: the primary's first property is recorded as printed and never "
         "relabelled; P4 and P8 are established only by the witness on the same pinned executable, both at their "
         "frozen loci; G.5(E)'s Architect review is pending; the (F) recording is proposed for Architect review, not "
         "made.",
         "- C7 (MUTATION_CONTROL_BLOCKED) and the 33 accepted controls were not executed; C7's receipts are cited as "
         "they stand.",
         ""]
    open(os.path.join(outdir, "INDEX.md"), "w").write("\n".join(i).rstrip("\n") + "\n")
    # The mission is named on the status line: a rehearsal's console must never repeat an earlier mission's bytes.
    print(f"STATUS: {st}; {K.MISSION}; new candidates {len(candidates)} of {len(K.ORDER)}; attempted {attempted}; "
          f"stopped {blocked}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
