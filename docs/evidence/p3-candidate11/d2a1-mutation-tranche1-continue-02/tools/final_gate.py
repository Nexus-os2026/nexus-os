#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission sections 17-22: the final gate over the mission's local state,
run once before the packet is staged. Read-only (it writes only its own output). Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01 (sections 10, 15, 19): G3 reads the carried-forward baseline, healthy-build
and C22 receipts as frozen copies and the Architect's C22 ruling record, and checks the attempted controls against the
resume order; G4 also applies the path-aware restored check and re-checks the preserved C22 and development
worktrees read-only.

    final_gate.py            prints the gate; exit 0 only if every check holds

G1  authenticate.py --gate: every input identity again (C, the R2 catalog and patches, contract, AD-1-R4/R5, Stage-1
    and Stage-2 evidence, the pinned baseline executable, refs, destination absence)
G2  the freeze: every frozen file equals its recorded sha256
G3  the receipt chain is consistent: the carried-forward baselines and healthy build are PASS and C22 is recorded as
    Architect-accepted (frozen copies, equal to the previous packet: G1, G2); each attempted control's RESULT agrees
    with its own sub-receipts; the controls attempted are a prefix of the resume order and stop at the first
    non-candidate; no receipt or worktree exists for any control outside the tranche
G4  every attempted control's worktree is exactly C now (verify_tree.py and the path-aware restored check), locked
    with this mission's reason; the preserved C22 worktree is still exactly C (read-only) and the development worktree
    exactly C
G5  no mutant blob is in the repository object store; nothing mutant is staged anywhere
G6  the pinned executables are preserved for review: the baseline executable, the healthy build's and every pinned
    mutant executable still have their recorded sha256 and size
G7  Candidate C and the R2 catalog are unchanged (from G1), and no file under the mission's evidence root is a source
    copy (no file equal to a mutant blob)
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02 (sections 16, 18, 19): G3 reads the carried-forward X-1 and
T-RES baselines, both Architect ruling records, and the attempted controls against the continue order (C28's
sub-receipts: admission, pin, run, restoration, link; no build by design); G4 the preserved C28 worktree exactly C and
still locked with RESUME-01's reason, every newly created worktree exactly C and locked with this mission's reason;
G6 the C28 pin (02-pin/PIN.json) and every later pinned executable preserved; G8 no receipt for any Architect-accepted
control (C22 C14 C19 C21 C27) and their preserved worktrees untouched (still exactly C, read-only).
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import c28_admit  # noqa: E402
import common as K  # noqa: E402
import control  # noqa: E402
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
    for b in [b for b in exp["baselines"] if b["dir"] in K.CARRIED]:
        cl = supervise.first_line(os.path.join(K.EV, b["dir"], "CLASSIFICATION.txt"))
        check(f"G3 carried-forward baseline {b['key']} (EXEC-01 packet, not rerun) is PASS", cl == "CLASSIFICATION: PASS",
              cl)
    fr = json.load(open(supervise.FREEZE))
    for rel, first in ((K.RULING, K.RULING_FIRST), (K.C28_RULING, K.C28_RULING_FIRST)):
        check(f"G3 the Architect ruling record {rel} is frozen and states the ruling",
              supervise.first_line(os.path.join(K.EV, rel)) == first and rel in fr["files"]
              and K.sha256_file(os.path.join(K.EV, rel)) == fr["files"][rel])
    attempted = [c for c in K.CONTINUE_ORDER if os.path.exists(os.path.join(K.EV, exp["controls"][c]["dir"]))]
    check("G3 attempted controls are a prefix of the continue order", attempted == K.CONTINUE_ORDER[:len(attempted)],
          attempted)
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
            ("admission", "01-admission/ADMISSION.txt"), ("pin", "02-pin/PIN.txt"),
            ("run", "04-run/CLASSIFICATION.txt"), ("restore", "05-restore/RESTORE.txt"), ("link", "06-link/LINK.txt"))}
        cand = "CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"
        want_ok = (sub["restore"] == "RESTORE: EXACTLY_C" and sub["link"] == "LINK: VERIFIED" and sub["run"] == cand
                   and ((c == "C28" and sub["admission"] == c28_admit.ADMITTED and sub["pin"] == c28_admit.PINNED
                         and sub["apply"] is None and sub["build"] is None) or
                        (c != "C28" and sub["apply"] == "APPLY: EXACTLY_C_PLUS_PATCH" and sub["build"] == "BUILD: PINNED"
                         and sub["admission"] is None and sub["pin"] is None)))
        check(f"G3 {c}: RESULT agrees with its sub-receipts", (res == "CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — "
                                                               "ARCHITECT REVIEW PENDING") == want_ok, f"{res}; {sub}")
        wt = exp["controls"][c]["worktree"]
        if os.path.exists(wt):
            if c == "C28":
                os.environ["GIT_OPTIONAL_LOCKS"] = "0"  # RESUME-01's worktree: read, never written
            v = control.restored_check(wt, "final gate")
            os.environ.pop("GIT_OPTIONAL_LOCKS", None)
            check(f"G4 {c}: worktree exactly C now (verify_tree.py and the path-aware status policy)",
                  v["verdict"] == "EXACTLY_C", f"{v['verdict']} {v['verify_tree']['status_entries']}; "
                                              f"{v['status_policy']['target_entries']} target entries")
            entry = worktrees.worktree_entry(wt) or []
            owner = K.RESUME_MISSION if c == "C28" else K.MISSION
            check(f"G4 {c}: worktree locked with {'RESUME-01' if c == 'C28' else 'this mission'}'s reason",
                  any(l.startswith("locked " + owner + " ") for l in entry), entry[-1:] if entry else entry)
        pin = os.path.join(d, "02-pin/PIN.json" if c == "C28" else "03-build/PIN.json")
        if os.path.exists(pin):
            p = json.load(open(pin))
            check(f"G6 {c}: pinned mutant executable preserved", os.path.exists(p["path"])
                  and K.sha256_file(p["path"]) == p["sha256"] and os.path.getsize(p["path"]) == p["size"],
                  f"{p['path']} {p['sha256'][:16]}…")
    for c in K.NOT_AUTHORIZED:
        check(f"G3 {c}: not executed (no worktree)", not os.path.lexists(K.control_wt(c)))
    for c in K.CONTINUE_ACCEPTED:
        check(f"G8 {c} (Architect-accepted): no receipt directory in this mission (not rebuilt, not rerun)",
              not os.path.lexists(os.path.join(K.EV, exp["controls"][c]["dir"])))
    os.environ["GIT_OPTIONAL_LOCKS"] = "0"  # the previous missions' worktrees are read, never written
    for c in K.CONTINUE_ACCEPTED:
        v = control.restored_check(K.control_wt(c), "final gate")
        entry = worktrees.worktree_entry(K.control_wt(c)) or []
        owner = K.PREV_MISSION if c == "C22" else K.RESUME_MISSION
        check(f"G8 {c}'s preserved worktree untouched: exactly C now (read-only), still locked with its own mission's "
              f"reason", v["verdict"] == "EXACTLY_C" and any(l.startswith("locked " + owner + " ") for l in entry),
              f"{v['verdict']}; {entry[-1:] if entry else entry}")
    del os.environ["GIT_OPTIONAL_LOCKS"]
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
