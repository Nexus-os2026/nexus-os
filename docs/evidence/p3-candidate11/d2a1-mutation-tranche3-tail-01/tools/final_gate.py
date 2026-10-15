#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission sections 17-22: the final gate over the mission's local state,
run once before the packet is staged. Revised by -RESUME-01, -CONTINUE-02, P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01
and -TRANCHE2-CONTINUE-02. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01 (sections 10-12, 15): this mission's
order and receipt chain (gate, worktree, apply, build, run, tranche-3 rules, C7's informational run, restore, link), its
worktrees, the 22 accepted controls' worktrees (read-only). Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02
(sections 9, 17, 18, 20): this mission's order; C24's own receipt chain (gate, admission, preserved worktree, pin, run,
rules, reauthentication, link; no apply, build or restore), its preserved worktree (exactly C, locked with the Tranche-3
reason) and executable (unchanged since its build); C3b's and C3c's build admission (a clean build or the one predicted
warning group) and pin record; the 23 accepted controls' worktrees (read-only). Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE3-TAIL-01 (section 11): this mission's order and one receipt chain for every control
(gate, worktree, apply, build, run, tranche-3 rules, restore, link; no admission, pin-only, reauthentication or
informational step); RESULT agreeing with its sub-receipts, including INCOMPLETE — ARCHITECT REVIEW REQUIRED exactly
when the run is classified INCOMPLETE and STOPPED BEFORE EXECUTION exactly when the gate refused; the mt3t worktrees; the
27 accepted controls' worktrees and C7's (read-only); no C7 receipt of any kind. Read-only but for its two receipts.

    final_gate.py            writes receipts/90-final-gate.txt and receipts/90-gate-authentication.txt; prints the gate;
                             exit 0 only if every check holds

G1  authenticate.py --gate: every input identity again (C, the catalog and patches, contract, AD-1-R4/R5 and adoption
    records, the proof, Stage-1/Stage-2 evidence and the linked receipts, every previous evidence commit, the baseline
    executable, refs, destination absence)
G2  the freeze: every frozen file equals its recorded sha256
G3  the receipt chain: the attempted controls are a prefix of this mission's order; only the last may be
    unsuccessful; each RESULT agrees with its own sub-receipts (gate, apply, build, run, rules, C7's informational run,
    restore, link); C7's informational run never carries a kill label and is never the RESULT's basis; a stop record
    exists exactly when the mission stopped; no receipt directory for any other control
G4  every attempted control's worktree is exactly C now (verify_tree.py and the path-aware restored check) and locked
    with this mission's reason; no mt3x worktree for any other control; the 22 accepted controls' worktrees exactly C
    (read-only; not touched)
G5  no catalog mutant blob in the repository object store; nothing staged in the main worktree
G6  every pinned mutant executable and the baseline executable keep their recorded sha256 and size
G7  no mutant source copy anywhere under the evidence root (every file's blob compared with every catalog mutant blob)
G8  no cargo, rustc or governed-control test process running now (by executable name, never by argument text)
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402
import control  # noqa: E402
import supervise  # noqa: E402
import t3  # noqa: E402
import worktrees  # noqa: E402

OUT = os.path.join(K.EV, "receipts/90-final-gate.txt")
CAND = "CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"
fails, out = [], []


def check(label, ok, detail=""):
    out.append(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def main():
    if os.path.exists(OUT):
        raise SystemExit("REFUSED: the final gate receipt exists (run once)")
    out.append(f"FINAL GATE — {K.MISSION}; started {supervise.now()}")
    r = subprocess.run([sys.executable, "-B", os.path.join(K.EV, "tools/authenticate.py"), "--gate"],
                       capture_output=True, text=True)
    open(os.path.join(K.EV, "receipts/90-gate-authentication.txt"), "w").write(r.stdout + r.stderr)
    check("G1 authenticate.py --gate: ALL CHECKS HOLD (receipts/90-gate-authentication.txt)",
          r.returncode == 0 and r.stdout.rstrip().endswith("RESULT: ALL CHECKS HOLD"),
          r.stdout.rstrip().splitlines()[-1] if r.stdout else r.stderr[-200:])
    rec = []
    fz = supervise.freeze_check(rec)
    check("G2 the freeze holds", fz is None, rec[0] if rec else fz)
    exp = supervise.expectations()
    attempted = [c for c in K.ORDER if os.path.exists(os.path.join(K.EV, exp["controls"][c]["dir"]))]
    check("G3 attempted controls are a prefix of this mission's order", attempted == K.ORDER[:len(attempted)], attempted)
    results = {c: supervise.first_line(os.path.join(K.EV, exp["controls"][c]["dir"], "RESULT.txt")) for c in attempted}
    check("G3 only the last attempted control may be unsuccessful (a stop ends the mission)",
          all(results[c] == control.RESULT_OK for c in attempted[:-1]), results)
    stopped = bool(attempted) and results[attempted[-1]] != control.RESULT_OK
    stop_rec = os.path.exists(os.path.join(K.EV, "receipts/80-STOP-RECORD.md"))
    check("G3 a stop record exists exactly when the mission stopped (or no control could start)",
          stop_rec == (stopped or not attempted), f"stopped {stopped}; stop record {stop_rec}")
    for c in attempted:
        d = os.path.join(K.EV, exp["controls"][c]["dir"])
        sub = {k: supervise.first_line(os.path.join(d, p)) for k, p in (
            ("gate", "00-GATE.txt"), ("apply", "02-apply/APPLY.txt"), ("build", "03-build/CLASSIFICATION.txt"),
            ("run", "04-run/CLASSIFICATION.txt"), ("rules", "04a-rules/RULES.txt"),
            ("restore", "05-restore/RESTORE.txt"), ("link", "06-link/LINK.txt"))}
        outer2 = (open(os.path.join(d, "04-run/CLASSIFICATION.txt")).read().splitlines()[1:2]
                  if sub["run"] is not None else [])
        run_incomplete = bool(outer2) and outer2[0].startswith(control.INCOMPLETE_OUTER)
        want_ok = (sub["gate"] == "GATE: OPEN" and sub["apply"] == control.APPLY_OK and sub["build"] == "BUILD: PINNED"
                   and sub["run"] == CAND and sub["rules"] == t3.RULES_OK and sub["restore"] == control.RESTORE_OK
                   and sub["link"] == control.LINK_OK)
        want = (control.RESULT_OK if want_ok else control.RESULT_STOPPED if sub["gate"] != "GATE: OPEN" else
                control.RESULT_INCOMPLETE if run_incomplete and sub["run"] != CAND and sub["restore"] ==
                control.RESTORE_OK else control.RESULT_BLOCKED)
        check(f"G3 {c}: RESULT agrees with its sub-receipts (candidate only with every step; INCOMPLETE exactly when the "
              "run is classified INCOMPLETE; STOPPED BEFORE EXECUTION exactly when the gate refused)",
              results[c] == want, f"{results[c]}; {sub}")
        check(f"G3 {c}: no build-admission, pin-only, reauthentication or informational record (none exists in this "
              "mission)", not [x for x in ("01-admission", "02-worktree", "03-pin", "03a-admission", "04b-info-F3",
                                           "05-reauth") if os.path.lexists(os.path.join(d, x))])
        if sub["apply"] is not None:
            check(f"G3 {c}: once the patch was applied, the restoration receipt exists and the worktree was restored",
                  sub["restore"] == control.RESTORE_OK, sub["restore"])
        if sub["link"] is not None:
            check(f"G3 {c}: a link exists only after a candidate run, its rules and its own restoration",
                  sub["run"] == CAND and sub["rules"] == t3.RULES_OK and sub["restore"] == control.RESTORE_OK, sub)
        wt = exp["controls"][c]["worktree"]
        if os.path.exists(wt):
            v = control.restored_check(wt, "final gate")
            check(f"G4 {c}: worktree exactly C now (verify_tree.py and the path-aware status policy)",
                  v["verdict"] == "EXACTLY_C", f"{v['verdict']}; {v['status_policy']['target_entries']} target entries")
            entry = worktrees.worktree_entry(wt) or []
            check(f"G4 {c}: worktree locked with this mission's reason",
                  any(l.startswith("locked " + K.MISSION + " ") for l in entry), entry[-1:] if entry else entry)
        pin = os.path.join(d, "03-build/PIN.json")
        if os.path.exists(pin):
            p = json.load(open(pin))
            check(f"G6 {c}: pinned mutant executable preserved (03-build/PIN.json)", os.path.exists(p["path"])
                  and K.sha256_file(p["path"]) == p["sha256"] and os.path.getsize(p["path"]) == p["size"],
                  f"{p['path']} {p['sha256'][:16]}…")
    for c in K.ORDER:
        if c not in attempted:
            check(f"G3 {c}: not attempted: no receipt directory and no worktree created by this mission",
                  not os.path.lexists(os.path.join(K.EV, exp["controls"][c]["dir"]))
                  and not os.path.lexists(exp["controls"][c]["worktree"]))
    check("G3 C7 (MUTATION_CONTROL_BLOCKED): no expectation, no receipt directory and no mt3t worktree; never executed",
          "C7" not in exp["controls"] and not [x for x in os.listdir(os.path.join(K.EV, "receipts")) if "C7" in x]
          and not os.path.lexists(K.NEXUS + "/nexus-os-p3-c11-mt3t-C7"))
    others = [c for c in K.ACCEPTED + K.BLOCKED + K.NOT_AUTHORIZED if os.path.lexists(K.NEXUS + "/nexus-os-p3-c11-mt3t-" + c)]
    check("G4 no mt3t worktree for any accepted, blocked or reserved control", not others, others)
    os.environ["GIT_OPTIONAL_LOCKS"] = "0"  # read-only checks of worktrees this mission does not own
    for c in K.ACCEPTED:
        v = control.restored_check(K.ACCEPTED_WT[c], "final gate")
        check(f"G4 {c} (Architect-accepted; its worktree {K.ACCEPTED_WT[c]}, read-only): exactly C, not touched",
              v["verdict"] == "EXACTLY_C", v["verdict"])
    for c, p in K.BLOCKED_WT.items():
        v = control.restored_check(p, "final gate")
        check(f"G4 {c} (MUTATION_CONTROL_BLOCKED; its worktree {p}, read-only): exactly C, not touched",
              v["verdict"] == "EXACTLY_C", v["verdict"])
    del os.environ["GIT_OPTIONAL_LOCKS"]
    check("G6 the pinned baseline executable unchanged", K.sha256_file(K.EXE) == K.EXE_SHA256
          and os.path.getsize(K.EXE) == K.EXE_SIZE)
    cat = json.loads(open(K.inputs("catalog.json"), "rb").read())
    mut = {c["mutant_blob"] for c in cat["controls"] if c.get("mutant_blob")}
    present = [b for b in sorted(mut) if subprocess.run(["git", "-C", K.REPO, "cat-file", "-e", b],
                                                         capture_output=True).returncode == 0]
    check("G5 no catalog mutant blob in the repository object store", not present, present)
    staged = subprocess.run(["git", "-C", K.REPO, "diff", "--cached", "--name-only"], capture_output=True,
                            text=True).stdout.split()
    check("G5 nothing staged in the main worktree", not staged, staged)
    copies, n = [], 0
    for d, dirs, fs in os.walk(K.EV):
        dirs[:] = [x for x in dirs if x != "__pycache__"]
        for f in fs:
            p = os.path.join(d, f)
            if os.path.islink(p) or not os.path.isfile(p):
                continue
            n += 1
            if K.git_blob_id(open(p, "rb").read()) in mut:
                copies.append(p)
    check(f"G7 no mutant source copy anywhere under the evidence root ({n} files compared)", not copies, copies)
    ps = subprocess.run(["ps", "-eo", "pid=,comm="], capture_output=True, text=True).stdout.splitlines()
    busy = [l.strip() for l in ps if l.split(None, 1)[-1] in ("cargo", "rustc")
            or l.split(None, 1)[-1].startswith("nexus_governed")]
    check("G8 no cargo, rustc or governed-control test process running now (by executable name)", not busy, busy[:3])
    out.append(f"finished {supervise.now()}")
    out.append("FINAL GATE: " + ("ALL CHECKS HOLD" if not fails else "FAILED: " + "; ".join(fails)))
    open(OUT, "w").write("\n".join(out) + "\n")
    print("\n".join(out))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main())
