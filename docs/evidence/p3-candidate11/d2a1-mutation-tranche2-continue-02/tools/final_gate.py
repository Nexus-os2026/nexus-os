#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission sections 17-22: the final gate over the mission's local state,
run once before the packet is staged. Revised by -RESUME-01 and -CONTINUE-02. Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (sections 13, 14, 17). Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02 (sections 9, 15, 17, 20): this mission's order, C5's own receipt
chain (admission, preserved worktree, pin, run, reauthentication, link; no apply, build or restore), the preserved C5
worktree and executable. Read-only but for its two receipts.

    final_gate.py            writes receipts/90-final-gate.txt and receipts/90-gate-authentication.txt; prints the gate;
                             exit 0 only if every check holds

G1  authenticate.py --gate: every input identity again (C, the catalog and patches, contract, AD-1-R4/R5 and adoption
    records, the proof, Stage-1/Stage-2 evidence and the linked receipts, the complete Tranche-1 evidence, the baseline
    executable, refs, destination absence)
G2  the freeze: every frozen file equals its recorded sha256
G3  the receipt chain: the attempted controls are a prefix of this mission's order; only the last may be
    unsuccessful; each RESULT agrees with its own sub-receipts (C5: gate, admission, worktree, pin, run, reauth, link);
    a stop record exists exactly when the mission stopped; no receipt directory for any other control
G4  every attempted control's worktree is exactly C now (verify_tree.py and the path-aware restored check) and locked
    with its owner's reason (C5: the Tranche-2 reason, read-only; A3, A6, C8, C15: this mission's); no mt2c worktree for
    any other control; the seventeen accepted controls' worktrees exactly C (read-only; not touched)
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
import c5  # noqa: E402
import common as K  # noqa: E402
import control  # noqa: E402
import supervise  # noqa: E402
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
        if c == "C5":
            sub = {k: supervise.first_line(os.path.join(d, p)) for k, p in (
                ("gate", "00-GATE.txt"), ("admission", "01-admission/ADMISSION.txt"),
                ("worktree", "02-worktree/AUTHENTICATE-BEFORE.txt"), ("pin", "03-pin/PIN.txt"),
                ("run", "04-run/CLASSIFICATION.txt"), ("reauth", "05-reauth/REAUTH.txt"), ("link", "06-link/LINK.txt"))}
            want_ok = (sub["gate"] == "GATE: OPEN" and sub["admission"] == c5.ADMIT_OK and sub["worktree"] == c5.WT_OK
                       and sub["pin"] == c5.PIN_OK and sub["run"] == CAND and sub["reauth"] == control.REAUTH_OK
                       and sub["link"] == control.LINK_OK)
            check("G3 C5: RESULT agrees with its sub-receipts", (results[c] == control.RESULT_OK) == want_ok,
                  f"{results[c]}; {sub}")
            check("G3 C5: no apply, build or restoration receipt (no patch applied, nothing built, by design)",
                  not [x for x in ("02-apply", "03-build", "05-restore") if os.path.lexists(os.path.join(d, x))])
            wrec, wok = c5.worktree_check(exp["controls"][c], "final gate", control.restored_check)
            check("G4 C5: the preserved worktree exactly C now, registered, detached, locked with the Tranche-2 reason "
                  "(read-only)", wok, [l for l, o, _ in wrec["checks"] if not o])
            prec, pok = c5.pin(exp["controls"][c], os.path.join(K.EV, exp["controls"][c]["preserved"]["build_receipt"]
                                                                   ["input_dir"]))
            check("G6 C5: the preserved executable keeps its sha256 and size; its target unmodified since the build",
                  pok, [l for l, o, _ in prec["checks"] if not o])
            continue
        sub = {k: supervise.first_line(os.path.join(d, p)) for k, p in (
            ("gate", "00-GATE.txt"), ("apply", "02-apply/APPLY.txt"), ("build", "03-build/CLASSIFICATION.txt"),
            ("run", "04-run/CLASSIFICATION.txt"), ("restore", "05-restore/RESTORE.txt"), ("link", "06-link/LINK.txt"))}
        want_ok = (sub["gate"] == "GATE: OPEN" and sub["apply"] == control.APPLY_OK and sub["build"] == "BUILD: PINNED"
                   and sub["run"] == CAND and sub["restore"] == control.RESTORE_OK and sub["link"] == control.LINK_OK)
        check(f"G3 {c}: RESULT agrees with its sub-receipts", (results[c] == control.RESULT_OK) == want_ok,
              f"{results[c]}; {sub}")
        if sub["apply"] is not None:
            check(f"G3 {c}: once the patch was applied, the restoration receipt exists and the worktree was restored",
                  sub["restore"] == control.RESTORE_OK, sub["restore"])
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
            check(f"G6 {c}: pinned mutant executable preserved", os.path.exists(p["path"])
                  and K.sha256_file(p["path"]) == p["sha256"] and os.path.getsize(p["path"]) == p["size"],
                  f"{p['path']} {p['sha256'][:16]}…")
    for c in K.ORDER:
        if c not in attempted:
            check(f"G3 {c}: not attempted: no receipt directory and no worktree",
                  not os.path.lexists(os.path.join(K.EV, exp["controls"][c]["dir"]))
                  and not os.path.lexists(exp["controls"][c]["worktree"]))
    others = [c for c in K.ACCEPTED + K.NOT_AUTHORIZED if os.path.lexists(K.NEXUS + "/nexus-os-p3-c11-mt2c-" + c)]
    check("G4 no mt2c worktree for any accepted or not-authorized control", not others, others)
    os.environ["GIT_OPTIONAL_LOCKS"] = "0"  # read-only checks of worktrees this mission does not own
    for c in K.ACCEPTED:
        wt = K.NEXUS + ("/nexus-os-p3-c11-mt1x-" if c in K.ACCEPTED_T1 else "/nexus-os-p3-c11-mt2x-") + c
        v = control.restored_check(wt, "final gate")
        check(f"G4 {c} (Architect-accepted; {'Tranche 1' if c in K.ACCEPTED_T1 else 'Tranche 2'}'s worktree, "
              "read-only): exactly C, not touched", v["verdict"] == "EXACTLY_C", v["verdict"])
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
