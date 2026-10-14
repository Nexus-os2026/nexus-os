#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission sections 17-22: the final gate over the mission's local state,
run once before the packet is staged. Read-only (it writes only its own output).

    final_gate.py            prints the gate; exit 0 only if every check holds

G1  authenticate.py --gate: every input identity again (C, the R2 catalog and patches, contract, AD-1-R4/R5, Stage-1
    and Stage-2 evidence, the pinned baseline executable, refs, destination absence)
G2  the freeze: every frozen file equals its recorded sha256
G3  the receipt chain is consistent: each baseline and the healthy build classified once; each attempted control's
    RESULT agrees with its own sub-receipts; the controls attempted are a prefix of the frozen order and stop at the
    first non-candidate; no receipt or worktree exists for any control outside the tranche
G4  every attempted control's worktree is exactly C now (verify_tree.py), locked with this mission's reason
G5  no mutant blob is in the repository object store; nothing mutant is staged anywhere
G6  the pinned executables are preserved for review: the baseline executable, the healthy build's and every pinned
    mutant executable still have their recorded sha256 and size
G7  Candidate C and the R2 catalog are unchanged (from G1), and no file under the mission's evidence root is a source
    copy (no file equal to a mutant blob)
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402
import supervise  # noqa: E402
import worktrees  # noqa: E402

fails = []


def check(label, ok, detail=""):
    print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def main():
    print(f"FINAL GATE — {K.MISSION}")
    r = subprocess.run([sys.executable, os.path.join(K.EV, "tools/authenticate.py"), "--gate"], capture_output=True,
                       text=True)
    open(os.path.join(K.EV, "receipts/90-gate-authentication.txt"), "w").write(r.stdout + r.stderr)
    check("G1 authenticate.py --gate: ALL CHECKS HOLD (receipts/90-gate-authentication.txt)",
          r.returncode == 0 and r.stdout.rstrip().endswith("RESULT: ALL CHECKS HOLD"),
          r.stdout.rstrip().splitlines()[-1] if r.stdout else r.stderr[-200:])
    rec = []
    fz = supervise.freeze_check(rec)
    check("G2 the freeze holds", fz is None, rec[0] if rec else fz)
    exp = supervise.expectations()
    stopped = False
    for b in exp["baselines"]:
        cl = supervise.first_line(os.path.join(K.EV, b["dir"], "CLASSIFICATION.txt"))
        if stopped:
            check(f"G3 baseline {b['key']} not run after an earlier stop", not os.path.exists(os.path.join(K.EV, b["dir"])))
        else:
            check(f"G3 baseline {b['key']} run once and classified", cl in ("CLASSIFICATION: PASS",
                                                                             "CLASSIFICATION: NOT PASS"), cl)
            stopped = cl != "CLASSIFICATION: PASS"
    hcl = supervise.first_line(os.path.join(K.EV, "receipts/20-healthy-build/CLASSIFICATION.txt"))
    if stopped:
        check("G3 healthy build not run after a baseline stop",
              not os.path.exists(os.path.join(K.EV, "receipts/20-healthy-build")))
    else:
        print(f"     healthy build: {hcl}")
    stopped = stopped or hcl != "CLASSIFICATION: PASS"
    if stopped:
        check("G3 no control attempted after a baseline or healthy-build stop",
              not any(os.path.exists(os.path.join(K.EV, exp["controls"][c]["dir"])) for c in K.ORDER))
    attempted = [c for c in K.ORDER if os.path.exists(os.path.join(K.EV, exp["controls"][c]["dir"]))]
    check("G3 attempted controls are a prefix of the frozen order", attempted == K.ORDER[:len(attempted)], attempted)
    results = {c: supervise.first_line(os.path.join(K.EV, exp["controls"][c]["dir"], "RESULT.txt")) for c in attempted}
    ok_prefix = True
    for n, c in enumerate(attempted):
        if results[c] != "CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING" and n != len(attempted) - 1:
            ok_prefix = False
    check("G3 only the last attempted control may be non-candidate (a stop ends the tranche)", ok_prefix, results)
    for c in attempted:
        d = os.path.join(K.EV, exp["controls"][c]["dir"])
        res = results[c]
        sub = {k: supervise.first_line(os.path.join(d, p)) for k, p in (
            ("apply", "02-apply/APPLY.txt"), ("build", "03-build/CLASSIFICATION.txt"),
            ("run", "04-run/CLASSIFICATION.txt"), ("restore", "05-restore/RESTORE.txt"), ("link", "06-link/LINK.txt"))}
        want_ok = (sub["apply"] == "APPLY: EXACTLY_C_PLUS_PATCH" and sub["restore"] == "RESTORE: EXACTLY_C"
                   and sub["link"] == "LINK: VERIFIED"
                   and ((c == "C22" and sub["build"] == "CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"
                         and sub["run"] is None) or
                        (c != "C22" and sub["build"] == "BUILD: PINNED"
                         and sub["run"] == "CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING")))
        check(f"G3 {c}: RESULT agrees with its sub-receipts", (res == "CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — "
                                                               "ARCHITECT REVIEW PENDING") == want_ok, f"{res}; {sub}")
        wt = exp["controls"][c]["worktree"]
        if os.path.exists(wt):
            v = json.loads(subprocess.run([sys.executable, os.path.join(K.EV, "tools/verify_tree.py"), wt],
                                          capture_output=True, text=True).stdout)
            check(f"G4 {c}: worktree exactly C now", v["verdict"] == "EXACTLY_C", f"{v['verdict']} {v['status_entries']}")
            entry = worktrees.worktree_entry(wt) or []
            check(f"G4 {c}: worktree locked with this mission's reason",
                  any(l.startswith("locked " + K.MISSION) for l in entry), entry[-1:] if entry else entry)
        pin = os.path.join(d, "03-build/PIN.json")
        if os.path.exists(pin):
            p = json.load(open(pin))
            check(f"G6 {c}: pinned mutant executable preserved", os.path.exists(p["path"])
                  and K.sha256_file(p["path"]) == p["sha256"] and os.path.getsize(p["path"]) == p["size"],
                  f"{p['path']} {p['sha256'][:16]}…")
    for c in K.NOT_AUTHORIZED:
        check(f"G3 {c}: not executed (no worktree)", not os.path.lexists(K.control_wt(c)))
    if os.path.exists(K.HEALTHY_WT):
        v = json.loads(subprocess.run([sys.executable, os.path.join(K.EV, "tools/verify_tree.py"), K.HEALTHY_WT],
                                      capture_output=True, text=True).stdout)
        check("G4 healthy-build worktree exactly C now", v["verdict"] == "EXACTLY_C", v["verdict"])
    hx = os.path.join(K.EV, "receipts/20-healthy-build/EXECUTABLES.json")
    if os.path.exists(hx):
        p = json.load(open(hx))
        check("G6 the healthy build's executable preserved (never run)", K.sha256_file(p["path"]) == p["sha256"],
              f"{p['path']} {p['sha256'][:16]}…")
    check("G6 the pinned baseline executable unchanged", K.sha256_file(K.EXE) == K.EXE_SHA256
          and os.path.getsize(K.EXE) == K.EXE_SIZE)
    absent = [c["mutant_blob"] for c in exp["controls"].values()
              if subprocess.run(["git", "-C", K.REPO, "cat-file", "-e", c["mutant_blob"]], capture_output=True).returncode == 0]
    check("G5 no mutant blob in the repository object store", not absent, absent)
    staged = subprocess.run(["git", "-C", K.REPO, "diff", "--cached", "--name-only"], capture_output=True,
                            text=True).stdout.split()
    check("G5 nothing staged in the main worktree", not staged, staged)
    mut = {c["mutant_blob"] for c in exp["controls"].values()}
    copies = []
    for d, dirs, fs in os.walk(K.EV):
        dirs[:] = [x for x in dirs if x != "__pycache__"]
        for f in fs:
            p = os.path.join(d, f)
            if os.path.getsize(p) < 200_000 and K.git_blob_id(open(p, "rb").read()) in mut:
                copies.append(p)
    check("G7 no mutant source copy anywhere under the evidence root", not copies, copies)
    print("FINAL GATE: " + ("ALL CHECKS HOLD" if not fails else "FAILED: " + "; ".join(fails)))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main())
