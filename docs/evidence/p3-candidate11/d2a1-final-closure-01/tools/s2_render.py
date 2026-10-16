#!/usr/bin/env python3
"""Render 02-CONFIGURATIONS.tsv and 02-BRANCH-VOCABULARY.tsv from receipts/22-S2-DERIVATION.json. Read-only input."""
import json
import os
import sys

ROOT = sys.argv[1]
d = json.load(open(os.path.join(ROOT, "receipts/22-S2-DERIVATION.json")))
ORDER = ["X-1", "X-2", "X-3", "X-4", "X-5", "F-1", "F-2", "F-3", "F-4", "F-5", "F-6", "F-7", "F-8", "F-PT", "F-B",
         "F-B2", "F-G", "F-W", "F-D", "F-R"]
cols = ["config", "test", "explorer", "source_head", "source_tree", "executable_sha256", "argv", "bounds", "actors",
        "raw_outcome", "admissibility", "states_or_keys", "expanded", "transitions", "fault_edges", "no_step",
        "derived_original_states", "Kf", "property_lines_hold", "not_applicable", "not_claimed", "not_established",
        "D1", "L1", "branches_taken", "elapsed_printed", "wall_seconds", "time_limit_s", "memory_peak_bytes",
        "memory_max_bytes", "swap_peak_bytes", "oom_kill", "exit_status", "signal", "raw_sha256", "receipt",
        "discharge", "corroborating"]
out = ["\t".join(cols)]
for c in ORDER:
    r = d["configs"][c]
    inc = c == "F-R"
    val = {
        "config": c, "test": r["test"], "explorer": r["explorer"], "source_head": r["head"], "source_tree": r["tree"],
        "executable_sha256": r["exe_sha_before"], "argv": " ".join(r["argv"][1:]), "bounds": r["bounds"],
        "actors": r["actors"], "raw_outcome": r["outcome"], "admissibility": r["admissibility"],
        "states_or_keys": (f"{r['states']} discovered at the cap (incomplete prefix; NOT A TOTAL)" if inc else
                           f"{r['states']} {'quotient keys (MEASURED)' if 'Kf' in r else 'states (MEASURED)'}"),
        "expanded": f"{r['expanded']}{' (prefix; NOT A TOTAL)' if inc else ''}",
        "transitions": f"{r['transitions']}{' (prefix; NOT A TOTAL)' if inc else ''}",
        "fault_edges": f"{r['fault_edges']}{' (prefix; NOT A TOTAL)' if inc else ''}",
        "no_step": f"{r['no_step']}{' (prefix; NOT A TOTAL)' if inc else ''}",
        "derived_original_states": (f"{r['derived_original_states']} {r['derived_label']}" if "Kf" in r else
                                    "not stated (F-R: no total exists or is derived)" if inc else "n/a (original run)"),
        "Kf": r.get("Kf", ""), "property_lines_hold": r["holds"], "not_applicable": ",".join(r["not_applicable"]),
        "not_claimed": ",".join(r["not_claimed"]), "not_established": ",".join(r["not_established"]) or "-",
        "D1": r["property_lines"].get("D1", ""), "L1": r["property_lines"].get("L1", ""),
        "branches_taken": f"{len(r['taken'])}{' (prefix only)' if inc else ''}", "elapsed_printed": r["elapsed"],
        "wall_seconds": r["wall_seconds"], "time_limit_s": r["time_limit"], "memory_peak_bytes": r["memory_peak_bytes"],
        "memory_max_bytes": r["memory_max"], "swap_peak_bytes": r["swap_peak"], "oom_kill": r["oom_kill"],
        "exit_status": r["exit_status"], "signal": r["signal"], "raw_sha256": r["raw_sha256"], "receipt": r["receipt"],
        "discharge": r["discharge"], "corroborating": " || ".join(r.get("corroborating", [])) or "-"}
    out.append("\t".join(str(val[k]).replace("\t", " ").replace("\n", " ") for k in cols))
open(os.path.join(ROOT, "02-CONFIGURATIONS.tsv"), "x").write("\n".join(out) + "\n")
bcols = ["id", "operation", "label", "in_static_BRANCHES", "classification", "complete_runs_taking", "fr_prefix",
         "fr_status_by_proof", "sources", "fr_proof_provenance"]
bo = ["\t".join(bcols)] + ["\t".join(str(r[k]).replace("\t", " ") for k in bcols) for r in d["branch_rows"]]
open(os.path.join(ROOT, "02-BRANCH-VOCABULARY.tsv"), "x").write("\n".join(bo) + "\n")
print("rendered", len(out) - 1, "configurations and", len(bo) - 1, "vocabulary rows;", d["branch_counts"])
