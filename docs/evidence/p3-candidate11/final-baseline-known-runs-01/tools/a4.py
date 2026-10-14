#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01, mission section 13: the A4 comparison record (AD-1-R4 F.1 (f)).

  a4.py      reads receipts/43-full-F-3 and receipts/44-full-F-PT; writes receipts/50-A4/A4-COMPARISON.txt and A4.json

F-3 and F-PT are not required to be equal in counts or branch sets; every difference is recorded. A4 is satisfied
only if F-3 PASSes, F-PT PASSes, no applicable property fails in F-PT, and both receipts are admissible (their
classification is PASS under AD-1-R4 K.2). This comparison is not a mutation kill.
"""
import json
import os
import sys

EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-final-baseline-known-runs-01"
OUT = os.path.join(EV, "receipts/50-A4")
RUNS = {"F-3": "receipts/43-full-F-3", "F-PT": "receipts/44-full-F-PT"}


def load(d):
    p = json.load(open(os.path.join(EV, d, "PARSED-RECEIPT.json")))
    desc = json.load(open(os.path.join(EV, d, "descriptor.json")))
    cls = open(os.path.join(EV, d, "CLASSIFICATION.txt")).read().splitlines()
    return p, desc, cls


def main():
    if os.path.exists(OUT):
        raise SystemExit("REFUSED: the A4 record exists")
    a, b = (load(RUNS[k]) for k in ("F-3", "F-PT"))
    (p3, d3, c3), (pt, dt, ct) = a, b
    r3, rt = p3["report"], pt["report"]
    lines, rows, diffs = [], [], []

    def row(label, v3, vt, note=""):
        same = v3 == vt
        rows.append({"item": label, "F-3": v3, "F-PT": vt, "equal": same, "note": note})
        if not same:
            diffs.append(label)
        lines.append(f"{label:58} | {str(v3):>44} | {str(vt):>44} | {'=' if same else 'DIFFERS'}{(' — ' + note) if note else ''}")

    lines.append("A4 COMPARISON — F-3 and F-PT, each by the admitted composed quotient, on the pinned baseline executable")
    lines.append("Not a mutation kill. Counts and branch sets are not required to be equal; every difference is listed.")
    lines.append("")
    lines.append(f"{'item':58} | {'F-3':>44} | {'F-PT':>44} | comparison")
    lines.append("-" * 170)
    # Identity.
    row("source commit (worktree HEAD, before)", d3["source_before"]["head"], dt["source_before"]["head"])
    row("source tree (before)", d3["source_before"]["tree"], dt["source_before"]["tree"])
    row("source verdict before / after", (d3["source_before"]["verdict"], d3["source_after"]["verdict"]),
        (dt["source_before"]["verdict"], dt["source_after"]["verdict"]))
    row("executable sha256 before", d3["executable"]["sha256_before"], dt["executable"]["sha256_before"])
    row("executable sha256 after", d3["executable"]["sha256_after"], dt["executable"]["sha256_after"])
    # Configuration.
    row("test selected (--exact --ignored)", p3["test"], pt["test"], "the configuration's own test")
    row("header", r3["header"], rt["header"])
    row("bounds", r3["bounds"], rt["bounds"], "A4 is the only difference: blanket_uncertain")
    row("actors", r3["actors"], rt["actors"])
    row("caps (keys, seconds)", (r3["cap"], r3["time_cap_s"]), (rt["cap"], rt["time_cap_s"]))
    # Verdicts.
    row("explorer outcome (raw)", r3["outcome"], rt["outcome"])
    row("harness result (raw)", p3["harness_result"], pt["harness_result"], "finish times differ")
    row("outer classification (K.2)", c3[1], ct[1])
    # Counts (MEASURED unless marked).
    row("MEASURED quotient keys", r3["states"], rt["states"])
    row("MEASURED expanded keys", r3["expanded"], rt["expanded"])
    row("MEASURED unexpanded keys", r3["states"] - r3["expanded"], rt["states"] - rt["expanded"])
    row("MEASURED transitions", r3["transitions"], rt["transitions"])
    row("MEASURED fault edges", r3["fault_edges"], rt["fault_edges"])
    row("MEASURED non-fault edges (L1's line)", r3["nonfault_edges"], rt["nonfault_edges"])
    row("MEASURED no-step keys", r3["no_step"], rt["no_step"])
    row("MEASURED admission evaluations", r3["evaluations"], rt["evaluations"])
    row("MEASURED admitted", r3["admitted"], rt["admitted"])
    row("MEASURED self-mirrored keys (Kf)", r3["self_mirrored"], rt["self_mirrored"])
    row("DERIVED original states (N+1)(2K-Kf), conditional on B(1)", r3["derived_original_states"],
        rt["derived_original_states"])
    row("explorer elapsed (s, as printed)", r3["elapsed_s"], rt["elapsed_s"])
    row("explorer tables (MiB, as printed)", r3["tables_mib"], rt["tables_mib"])
    row("process wall (s, /usr/bin/time)", d3["supervisor"]["wall_seconds"], dt["supervisor"]["wall_seconds"])
    row("scope memory.peak (bytes)", d3["supervisor"]["memory_peak_bytes"], dt["supervisor"]["memory_peak_bytes"])
    row("largest process max RSS (KiB)", d3["supervisor"]["max_rss_kib_largest_process"],
        dt["supervisor"]["max_rss_kib_largest_process"])
    row("scope memory.swap.peak (bytes)", d3["supervisor"]["swap_peak_bytes"], dt["supervisor"]["swap_peak_bytes"])
    row("cgroup oom_kill", d3["supervisor"]["oom_kill"], dt["supervisor"]["oom_kill"])
    lines.append("")
    lines.append("PROPERTY LINES (each as printed)")
    pv3, pvt = dict(r3["properties"]), dict(rt["properties"])
    for k in [k for k, _ in r3["properties"]]:
        row(f"property {k}", pv3.get(k), pvt.get(k))
    lines.append("")
    lines.append("BRANCH SETS (taken / NOT TAKEN for every listed label; extra labels as printed)")
    b3 = {lab: m.strip() for m, lab in r3["branches"]}
    bt = {lab: m.strip() for m, lab in rt["branches"]}
    labels = [lab for _, lab in r3["branches"]]
    taken3 = sorted(l for l in labels if b3[l] == "taken")
    takent = sorted(l for l in labels if bt.get(l) == "taken")
    lines.append(f"listed labels: F-3 {len(b3)}, F-PT {len(bt)}; same label list: {list(b3) == list(bt)}")
    lines.append(f"taken: F-3 {len(taken3)}, F-PT {len(takent)}")
    only3 = sorted(set(taken3) - set(takent))
    onlyt = sorted(set(takent) - set(taken3))
    lines.append(f"taken in F-3 only: {only3 or 'none'}")
    lines.append(f"taken in F-PT only: {onlyt or 'none'}")
    lines.append(f"extra labels F-3: {r3['extra_branches']}")
    lines.append(f"extra labels F-PT: {rt['extra_branches']}")
    for lab in labels:
        lines.append(f"  [{b3[lab]:9}] [{bt.get(lab, 'ABSENT'):9}] {lab}")
    if only3 or onlyt or r3["extra_branches"] != rt["extra_branches"]:
        diffs.append("branch sets")
    # The A4 criterion.
    pass3 = c3[0] == "CLASSIFICATION: PASS" and r3["outcome"] == "PASS"
    passt = ct[0] == "CLASSIFICATION: PASS" and rt["outcome"] == "PASS"
    applicable_fail = [k for k, v in rt["properties"] if k != "E10" and not (v == "HOLDS" or v.startswith("HOLDS:"))]
    satisfied = pass3 and passt and not applicable_fail
    lines.append("")
    lines.append("A4 CRITERION (mission section 13)")
    lines.append(f"  F-3 PASSes (admissible, K.2): {pass3}")
    lines.append(f"  F-PT PASSes (admissible, K.2): {passt}")
    lines.append(f"  no applicable property fails in F-PT: {not applicable_fail} {applicable_fail or ''}")
    lines.append(f"  both receipts admissible under AD-1-R4: {pass3 and passt}")
    lines.append(f"differences recorded: {diffs}")
    lines.append(f"A4: {'SATISFIED' if satisfied else 'NOT SATISFIED'} (comparison record; not a mutation kill)")
    os.makedirs(OUT)
    open(os.path.join(OUT, "A4-COMPARISON.txt"), "w").write("\n".join(lines) + "\n")
    json.dump({"rows": rows, "differences": diffs, "taken_only_in_F-3": only3, "taken_only_in_F-PT": onlyt,
               "satisfied": satisfied, "F-3_pass": pass3, "F-PT_pass": passt,
               "F-PT_applicable_property_failures": applicable_fail}, open(os.path.join(OUT, "A4.json"), "w"), indent=1)
    print("\n".join(lines[:4] + [l for l in lines if l.startswith(("A4", "  F-", "  no", "  both", "differences"))]))
    return 0 if satisfied else 1


if __name__ == "__main__":
    sys.exit(main())
