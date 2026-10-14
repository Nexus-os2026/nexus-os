#!/usr/bin/env python3
"""The twenty-configuration accounting table, read from the authenticated
receipts' git objects (mission section 19). Nothing is run.

  accounting.py TEMPLATE.md OUT.md OUT.json DISPOSITION-SUMMARY.json

The template's line `<!-- ACCOUNTING TABLE -->` is replaced by the table,
`<!-- SOURCE TABLE -->` by the receipts' identities, and the F-R branch
counts by those of branch_disposition.py's summary.
"""
import hashlib
import json
import re
import subprocess
import sys

REPO = "/home/nexus/NEXUS/nexus-os"
S1 = "f023cc3d031a2a89e5ecbd4086e15f2e50869acd"
S2 = "89326b859792a442c1853c3d35d30a48301c009e"
P1 = "docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/"
P2 = "docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/"
SUITE = (S1, P1 + "30-suite/raw-output.txt", "d6f58742ea483dbb95452b186dcf72f729538e672ae60741d978b0eddc6ca3b3")

ROWS = [
    ("X-1", "default", "x1_two_facts_two_writers_one_admission", SUITE, None),
    ("X-2", "default", "x2_one_fact_two_gating_callers", SUITE, None),
    ("X-3", "default", "x3_one_conflicting_fact_with_env", SUITE, None),
    ("X-4", "default", "x4_one_conflicting_fact_one_acknowledgement", SUITE, None),
    ("X-5", "default", "x5_one_clean_batch_of_two", SUITE, None),
    ("F-1", "original", "full_f1", (S1, P1 + "45-full-F-1/raw-output.txt", None), P1 + "45-full-F-1/CLASSIFICATION.txt"),
    ("F-2", "original", "full_f2", (S1, P1 + "41-full-F-2/raw-output.txt", "985a3976713bc77bdf358744ec770e6220310f2ce2ed72b3e9ae10b3fb314ab5"), P1 + "41-full-F-2/CLASSIFICATION.txt"),
    ("F-3", "composed quotient (replacing run, AD-1-R4 E.1)", "gc_symmetry_f3", (S1, P1 + "43-full-F-3/raw-output.txt", None), P1 + "43-full-F-3/CLASSIFICATION.txt"),
    ("F-4", "original", "full_f4", (S2, P2 + "62-full-F-4/raw-output.txt", None), P2 + "62-full-F-4/CLASSIFICATION.txt"),
    ("F-5", "original", "full_f5", (S2, P2 + "65-full-F-5/raw-output.txt", None), P2 + "65-full-F-5/CLASSIFICATION.txt"),
    ("F-6", "original", "full_f6", (S2, P2 + "66-full-F-6/raw-output.txt", None), P2 + "66-full-F-6/CLASSIFICATION.txt"),
    ("F-7", "original", "full_f7", (S2, P2 + "67-full-F-7/raw-output.txt", None), P2 + "67-full-F-7/CLASSIFICATION.txt"),
    ("F-8", "original", "full_f8", (S2, P2 + "64-full-F-8/raw-output.txt", None), P2 + "64-full-F-8/CLASSIFICATION.txt"),
    ("F-PT", "composed quotient (replacing run, AD-1-R4 E.1)", "gc_symmetry_fpt", (S1, P1 + "44-full-F-PT/raw-output.txt", None), P1 + "44-full-F-PT/CLASSIFICATION.txt"),
    ("F-B", "composed quotient (replacing run, AD-1-R4 E.1)", "gc_symmetry_fb", (S1, P1 + "42-full-F-B/raw-output.txt", None), P1 + "42-full-F-B/CLASSIFICATION.txt"),
    ("F-B2", "original", "full_fb2", (S2, P2 + "61-full-F-B2/raw-output.txt", None), P2 + "61-full-F-B2/CLASSIFICATION.txt"),
    ("F-G", "original", "full_fg", (S2, P2 + "60-full-F-G/raw-output.txt", None), P2 + "60-full-F-G/CLASSIFICATION.txt"),
    ("F-W", "original", "full_fw", (S2, P2 + "63-full-F-W/raw-output.txt", None), P2 + "63-full-F-W/CLASSIFICATION.txt"),
    ("F-D", "original", "full_fd", (S2, P2 + "68-full-F-D/raw-output.txt", None), P2 + "68-full-F-D/CLASSIFICATION.txt"),
    ("F-R", "original", "full_fr", (S2, P2 + "69-full-F-R/raw-output.txt", "78e44c335b6ce65f8df6aea18b71e488613403ba8c0471c5cf1c33057ab58ccd"), P2 + "69-full-F-R/CLASSIFICATION.txt"),
]


def show(commit, path):
    return subprocess.run(["git", "-C", REPO, "show", f"{commit}:{path}"], check=True, capture_output=True).stdout


def report_for(raw, test):
    lines = raw.split("\n")
    for k, line in enumerate(lines):
        if line.startswith(f"test ledger::protocol::model_tests::{test} ... ") and "EXPLORATION" in line:
            out = [line[line.index("EXPLORATION"):]]
            for l2 in lines[k + 1:]:
                out.append(l2)
                if l2.startswith("  state key fields:"):
                    break
            return out
    return None


def main(template, out_md, out_json, disp_json):
    rows = []
    problems = []
    for cfg, explorer, test, (commit, path, want), clsp in ROWS:
        data = show(commit, path)
        h = hashlib.sha256(data).hexdigest()
        if want and h != want:
            problems.append(f"{cfg}: raw sha256 {h} != {want}")
        rep = report_for(data.decode(), test)
        if rep is None:
            problems.append(f"{cfg}: no report of {test}")
            continue
        text = "\n".join(rep)
        if not re.match(rf"EXPLORATION {re.escape(cfg)}( |$)", rep[0]):
            problems.append(f"{cfg}: header {rep[0][:40]}")
        outcome = re.search(r"^  outcome: (\w+)", text, re.M).group(1)
        st = re.search(r"^  states (\d+) \(expanded (\d+)\), transitions (\d+) \(with a fault (\d+)\), states with no enabled step (\d+)", text, re.M)
        props = dict(re.findall(r"^  (P\d+|D1|L1|E\d+|KEY): (.*)$", text, re.M))
        holds = sum(1 for v in props.values() if v.startswith("HOLDS"))
        kf = re.search(r"(\d+) expanded keys are their own mirror", text)
        adm = re.search(r"\((\d+) evaluations, (\d+) admitted\)", text)
        cls = show(commit, clsp).decode().split("\n")[1] if clsp else "PASS (Stage-1 suite classification: PASS, 48 of 48; default exploration inside the suite)"
        cls = cls.replace("outer class (AD-1-R4 K.2): ", "")
        r = {"config": cfg, "explorer": explorer, "test": test, "commit": commit, "raw": path, "raw_sha256": h,
             "outcome": outcome, "states": int(st.group(1)), "expanded": int(st.group(2)), "transitions": int(st.group(3)),
             "fault_edges": int(st.group(4)), "no_step": int(st.group(5)), "holds": holds,
             "L1": props.get("L1", "-"), "E10": props.get("E10", "-"), "D1": props.get("D1", "-"), "KEY": props.get("KEY", "-"),
             "kf": int(kf.group(1)) if kf else None, "adm": (int(adm.group(1)), int(adm.group(2))) if adm else None,
             "outer_class": cls}
        if r["kf"] is not None:
            r["derived_original_states"] = 3 * (2 * r["states"] - r["kf"])
        rows.append(r)
    completed = [r for r in rows if r["outcome"] == "PASS"]
    if len(completed) != 19 or [r["config"] for r in rows if r["outcome"] != "PASS"] != ["F-R"]:
        problems.append(f"expected nineteen PASS and F-R INCOMPLETE; got {[(r['config'], r['outcome']) for r in rows]}")

    def n(x):
        return f"{x:,}"
    table = ["| # | Configuration | Explorer / test | Raw outcome (as printed) | States or keys (expanded) | Transitions (fault) | No-step | Property lines | L1 line | Outer class (receipt) | Contract status |",
             "|---|---|---|---|---|---|---|---|---|---|---|"]
    for k, r in enumerate(rows, 1):
        if r["config"] == "F-R":
            status = "**PROOF-DISCHARGED — proposed, pending R5 acceptance and adoption**; raw verdict INCOMPLETE stays"
            states = f"{n(r['states'])} discovered ({n(r['expanded'])} expanded) at the cap — **not a total**"
            trans = f"{n(r['transitions'])} ({n(r['fault_edges'])}) in the prefix — **not a total**"
            props = "every line NOT ESTABLISHED (incomplete)"
            l1 = "NOT ESTABLISHED (incomplete)"
            nostep = f"{n(r['no_step'])} in the prefix — **not a total**"
        else:
            nostep = n(r["no_step"])
            unit = "keys" if r["kf"] is not None else "states"
            states = f"{n(r['states'])} {unit} ({n(r['expanded'])})"
            if r["kf"] is not None:
                states += f"; Kf {n(r['kf'])}; DERIVED original states {n(r['derived_original_states'])} (conditional on B(1); original graph not explored)"
            trans = f"{n(r['transitions'])} ({n(r['fault_edges'])})"
            e10 = "E10 HOLDS" if r["E10"].startswith("HOLDS") else "E10 not applicable"
            props = f"{r['holds']} HOLD; {e10}" + ("; L1 not claimed" if r["L1"].startswith("not claimed") else "")
            l1 = r["L1"]
            status = "MEASURED PASS (exact-final; accepted by the Architect's ruling)" if r["config"].startswith("F-") else "MEASURED PASS (exact-final default exploration, 8.2)"
        table.append(f"| {k} | {r['config']} | {r['explorer']}, `{r['test']}` | {r['outcome']} | {states} | {trans} | {nostep} | {props} | {l1} | {r['outer_class']} | {status} |")
    md = open(template, encoding="utf-8").read()
    if "<!-- ACCOUNTING TABLE -->" not in md:
        problems.append("template has no table marker")
    md = md.replace("<!-- ACCOUNTING TABLE -->", "\n".join(table))
    src_rows = ["| Configuration | Raw output (git path at the evidence commit) | Commit | sha256 |", "|---|---|---|---|"]
    for r in rows:
        src_rows.append(f"| {r['config']} | `{r['raw']}` | `{r['commit'][:12]}` | `{r['raw_sha256']}` |")
    md = md.replace("<!-- SOURCE TABLE -->", "\n".join(src_rows))
    disp = json.load(open(disp_json))
    c = disp["fr_status_by_proof"]
    if disp["problems"] or sum(c.values()) != 193:
        problems.append("the branch disposition is not complete")
    md = md.replace("<!-- FR TAKEN -->", str(c["TAKEN"])).replace("<!-- FR NOT TAKEN -->", str(c["NOT TAKEN"])).replace("<!-- FR NA -->", str(c["NOT APPLICABLE"]))
    open(out_md, "w", encoding="utf-8").write(md)
    json.dump({"rows": rows, "problems": problems}, open(out_json, "w"), indent=1, sort_keys=True)
    print(f"ACCOUNTING: {len(rows)} configurations; {len(completed)} PASS; " +
          ", ".join(f"{r['config']} {r['outcome']}" for r in rows if r["outcome"] != "PASS"))
    for p in problems:
        print("  PROBLEM:", p)
    return 0 if not problems else 1


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:5]))
