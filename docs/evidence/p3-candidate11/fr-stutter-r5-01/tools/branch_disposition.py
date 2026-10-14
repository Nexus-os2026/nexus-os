#!/usr/bin/env python3
"""F-R branch accounting by proof, from authenticated inputs only (mission
section 14). No execution.

  branch_disposition.py OUTDIR

Inputs, all read from git objects (never a working tree):
  - the frozen 193-row vocabulary (Stage-2 commit 89326b85);
  - the complete F-2 PASS receipt (Stage-1 commit f023cc3d), parsed here,
    and Stage-2's normalized F-2 table, which must agree with that parse;
  - the incomplete F-R receipt (Stage-2), first report copy, with the second
    copy checked identical;
  - the Stage-1 nonignored-suite receipt (the stale-operation diagnostic).

Per row it records the F-R status that the proof derives and every source of
evidence, under the five provenance labels of mission section 14:
  MEASURED_IN_COMPLETE_F2, MEASURED_IN_FR_PREFIX_ONLY,
  MEASURED_IN_EXACT_FINAL_DIAGNOSTIC, DERIVED_BY_FR_PROOF, NOT_ESTABLISHED.
A prefix observation is never recorded as a complete F-R measurement.
"""
import hashlib
import json
import os
import re
import subprocess
import sys

REPO = "/home/nexus/NEXUS/nexus-os"
S1 = "f023cc3d031a2a89e5ecbd4086e15f2e50869acd"
S2 = "89326b859792a442c1853c3d35d30a48301c009e"
P1 = "docs/evidence/p3-candidate11/final-baseline-known-runs-01/"
P2 = "docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/"
EXPECT = {
    (S2, P2 + "vocabulary/VOCABULARY.json"): "db8734e0bd2743f22a27fb4001e8deb0f6381b393aecc33c781d710ee57fa932",
    (S1, P1 + "receipts/41-full-F-2/raw-output.txt"): "985a3976713bc77bdf358744ec770e6220310f2ce2ed72b3e9ae10b3fb314ab5",
    (S2, P2 + "receipts/69-full-F-R/raw-output.txt"): "78e44c335b6ce65f8df6aea18b71e488613403ba8c0471c5cf1c33057ab58ccd",
    (S1, P1 + "receipts/30-suite/raw-output.txt"): "d6f58742ea483dbb95452b186dcf72f729538e672ae60741d978b0eddc6ca3b3",
}
REPLAY_LABELS = {"resolve: Stale": "Resolve", "append_cell C: Stale": "AppendC",
                 "append_cell X: Stale": "AppendX", "empty: Stale": "EmptyX"}
DIAG = "test ledger::protocol::model_tests::e8_holds_for_stale_worker_operations ... ok"


def blob(commit, path):
    data = subprocess.run(["git", "-C", REPO, "show", f"{commit}:{path}"], check=True, capture_output=True).stdout
    want = EXPECT.get((commit, path))
    got = hashlib.sha256(data).hexdigest()
    if want and got != want:
        raise SystemExit(f"STOP: {path} at {commit[:8]}: sha256 {got} != {want}")
    return data.decode("utf-8")


def reports(raw):
    """The report copies in a raw output: lists of lines from an EXPLORATION
    header to the next header or the end."""
    copies, cur = [], None
    for line in raw.split("\n"):
        if re.search(r"EXPLORATION F-", line):
            cur = [line[line.index("EXPLORATION"):]]
            copies.append(cur)
        elif cur is not None:
            cur.append(line)
    return copies


def branches(copy):
    taken, not_taken = set(), set()
    for line in copy:
        m = re.match(r"\s+\[(taken    |NOT TAKEN)\] (.*?)( \(not in the listed set\))?$", line)
        if m:
            (taken if m.group(1).startswith("taken") else not_taken).add(m.group(2))
    return taken, not_taken


def main(out):
    problems = []
    vocab = json.loads(blob(S2, P2 + "vocabulary/VOCABULARY.json"))
    rows = vocab["rows"]
    labels = {r["label"] for r in rows}
    if len(rows) != 193 or len(labels) != 193:
        problems.append(f"vocabulary has {len(rows)} rows / {len(labels)} labels, not 193")
    f2raw = blob(S1, P1 + "receipts/41-full-F-2/raw-output.txt")
    f2c = reports(f2raw)
    if len(f2c) != 1 or "  outcome: PASS" not in f2c[0] or "  states 12602670 (expanded 12602670), transitions 54214081 (with a fault 3558228), states with no enabled step 3206" not in f2c[0]:
        problems.append("F-2 receipt is not the single complete PASS report of 12,602,670 states")
    for p in ("D1: HOLDS", "L1: HOLDS: 50655853 non-fault edges, no cycle; 3206 end states, each terminal (D1)", "KEY: HOLDS",
              "E2: HOLDS", "E4: HOLDS", "E8: HOLDS", "E9: HOLDS", "E11: HOLDS"):
        if "  " + p not in f2c[0]:
            problems.append(f"F-2 receipt lacks '{p}'")
    f2_taken, f2_not = branches(f2c[0])
    f2norm = json.loads(blob(S2, P2 + "normalized/stage1/F-2.json"))
    norm_status = {r["label"]: r["status"] for r in f2norm["rows"]}
    if {l for l, s in norm_status.items() if s == "TAKEN"} != f2_taken:
        problems.append("Stage-2's normalized F-2 TAKEN set differs from this parse of the F-2 receipt")
    if not f2_taken <= labels:
        problems.append(f"F-2 labels outside the vocabulary: {sorted(f2_taken - labels)}")
    frraw = blob(S2, P2 + "receipts/69-full-F-R/raw-output.txt")
    frc = reports(frraw)
    if len(frc) != 2:
        problems.append(f"F-R raw output has {len(frc)} report copies, expected 2 (the report and its echo in the panic)")
    def report_body(copy):
        # a report runs from its header to its "state key fields:" line
        end = next((k for k, l in enumerate(copy) if l.startswith("  state key fields: ")), None)
        return copy[:end + 1] if end is not None else None
    first, second = report_body(frc[0]), report_body(frc[1]) if len(frc) > 1 else None
    if first is None or second is None or first != second:
        problems.append("the two F-R report copies differ")
    head = "\n".join(frc[0])
    for p in ("  outcome: INCOMPLETE", "  states 30000001 (expanded 26539923), transitions 125538060 (with a fault 9877119), states with no enabled step 1920",
              "  INCOMPLETE: the state cap of 30000000 was reached. No property is established by this run.",
              "  actors: [Fw(0), Fw(1), Se, Wk, Gc(0), Gc(1), Adm, Replay]"):
        if p not in head:
            problems.append(f"F-R receipt lacks '{p.strip()}'")
    fr_taken, fr_not = branches(frc[0])
    if not fr_taken <= labels:
        problems.append(f"F-R prefix labels outside the vocabulary: {sorted(fr_taken - labels)}")
    extra = fr_taken - f2_taken
    if not extra <= set(REPLAY_LABELS):
        problems.append(f"F-R prefix took a non-replay label F-2 never took (contradicts FR-16): {sorted(extra - set(REPLAY_LABELS))}")
    if not f2_taken <= fr_taken:
        pass  # not required: a prefix need not reach every label
    suite = blob(S1, P1 + "receipts/30-suite/raw-output.txt")
    diag_ok = DIAG in suite and "test result: ok. 313 passed; 0 failed; 35 ignored; 0 measured; 0 filtered out" in suite
    if not diag_ok:
        problems.append("the stale-operation diagnostic is not an ok line of the passing Stage-1 suite")
    table = []
    for r in rows:
        lab = r["label"]
        st2 = norm_status[lab]
        prov = []
        prefix = "TAKEN" if lab in fr_taken else ("NOT TAKEN (prefix only)" if lab in fr_not else "not printed")
        if st2 == "NOT APPLICABLE":
            status = "NOT APPLICABLE"
            prov = ["MEASURED_IN_COMPLETE_F2 (operation absent by configuration)",
                    "DERIVED_BY_FR_PROOF (F-R's configuration equals F-2's but for replays; REPLAY adds labels only to resolve, append_cell and empty: FR-17)"]
        elif lab in REPLAY_LABELS and lab in fr_taken:
            status = "TAKEN"
            prov = ["MEASURED_IN_FR_PREFIX_ONLY (taken by a transition from an expanded, hence reachable, F-R state; the run is INCOMPLETE and is not complete coverage)",
                    f"MEASURED_IN_EXACT_FINAL_DIAGNOSTIC (e8_holds_for_stale_worker_operations exercised a stale {REPLAY_LABELS[lab]}: every label it emitted ends in Stale, no violation; forced core, not a reachable state)",
                    "DERIVED_BY_FR_PROOF (FR-16: F-2 never takes it, so only REPLAY can; FR-RS/FR-10: such a transition is a stutter that violates nothing)"]
        elif lab == "append_cell X: Stale":
            status = "NOT TAKEN"
            prov = ["DERIVED_BY_FR_PROOF (FR-17.X: whenever Wk.last is AppendX(t), X holds occupancy t, so REPLAY never runs an AppendX; FR-16: no ordinary F-R transition takes it, since F-2 does not)",
                    "MEASURED_IN_COMPLETE_F2 (NOT TAKEN by every ordinary transition)",
                    "MEASURED_IN_EXACT_FINAL_DIAGNOSTIC (a stale AppendX answers Stale with no violation on a forced core; this shows the operation's behaviour, not reachability)",
                    "MEASURED_IN_FR_PREFIX_ONLY: NOT TAKEN in the prefix (establishes nothing alone)"]
        elif lab == "append_cell U: Stale":
            status = "NOT TAKEN"
            prov = ["DERIVED_BY_FR_PROOF (INV-AC: AppendC and AppendX tokens name C and X, never U, so no operation labels 'append_cell U'; FR-16)",
                    "MEASURED_IN_COMPLETE_F2 (NOT TAKEN)"]
        elif st2 == "TAKEN":
            status = "TAKEN"
            prov = ["MEASURED_IN_COMPLETE_F2 (TAKEN)",
                    "DERIVED_BY_FR_PROOF (FR-8 lifts every F-2 path into F-R with the same ordinary transitions and labels; FR-16)"]
            if lab in fr_taken:
                prov.append("MEASURED_IN_FR_PREFIX_ONLY: also taken in the prefix (corroboration only)")
        else:
            status = "NOT TAKEN"
            prov = ["MEASURED_IN_COMPLETE_F2 (NOT TAKEN)",
                    "DERIVED_BY_FR_PROOF (FR-7/FR-16: every ordinary F-R label is an F-2 label; FR-17: REPLAY's labels are the four stale ones)"]
        table.append({"id": r["id"], "operation": r["operation"], "label": lab, "f2_complete": st2,
                      "fr_prefix": prefix, "fr_status_by_proof": status, "provenance": prov})
    counts = {}
    for t in table:
        counts[t["fr_status_by_proof"]] = counts.get(t["fr_status_by_proof"], 0) + 1
    derived_taken = {t["label"] for t in table if t["fr_status_by_proof"] == "TAKEN"}
    corroborates = derived_taken == fr_taken
    unresolved = [t for t in table if any(p.startswith("NOT_ESTABLISHED") for p in t["provenance"])]
    if unresolved:
        problems.append(f"{len(unresolved)} rows NOT_ESTABLISHED")
    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, "FR-BRANCH-DISPOSITION.tsv"), "w") as fh:
        fh.write("id\toperation\tlabel\tF-2 complete (measured)\tF-R prefix (incomplete)\tF-R status by proof\tprovenance\n")
        for t in table:
            fh.write("\t".join([t["id"], t["operation"], t["label"], t["f2_complete"], t["fr_prefix"],
                                t["fr_status_by_proof"], " | ".join(t["provenance"])]) + "\n")
    summary = {
        "rows": len(table), "fr_status_by_proof": counts, "f2_complete_taken": len(f2_taken),
        "fr_prefix_taken": len(fr_taken), "fr_prefix_taken_minus_f2": sorted(extra),
        "derived_taken_equals_prefix_taken": corroborates, "diagnostic_line_present": diag_ok,
        "problems": problems,
    }
    with open(os.path.join(out, "FR-BRANCH-DISPOSITION-SUMMARY.json"), "w") as fh:
        json.dump(summary, fh, indent=1, sort_keys=True)
    print(f"BRANCH DISPOSITION: {len(table)} rows; by proof {counts}; F-2 complete TAKEN {len(f2_taken)}; "
          f"F-R prefix TAKEN {len(fr_taken)} (extra over F-2: {sorted(extra)})")
    print(f"  derived TAKEN set equals the prefix's TAKEN set: {corroborates} (corroboration only, not a measurement of the complete graph)")
    for p in problems:
        print("  PROBLEM:", p)
    print("BRANCH DISPOSITION:", "COMPLETE — every row has a status and a provenance; none NOT_ESTABLISHED" if not problems else "INCOMPLETE")
    return 0 if not problems else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
