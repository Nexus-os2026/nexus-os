#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01, section 10: the static pre-execution gate. Revised by -CONTINUE-02.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (section 7): run once after the freeze and before C23.
Read-only (it writes only its own receipts). Nothing is built or run.

    pre_exec_gate.py        writes receipts/06-PRE-EXECUTION-GATE.txt (first line `PRE-EXECUTION GATE: ALL CHECKS HOLD`
                            or `PRE-EXECUTION GATE: FAILED: ...`) and receipts/06-gate-authentication.txt; refuses if
                            the gate receipt exists

E1  authenticate.py --gate: Candidate C, the catalog and the ten patches, the contract, AD-1-R4/R5 and the adoption
    records, the accepted proof, Stage-1/Stage-2 evidence and every linked receipt, the complete Tranche-1 evidence,
    the baseline executable, refs and destination absence
E2  the freeze holds, and it covers every tool, both original tool sets, every input, the expectations and both records
E3  every pre-freeze receipt records success and is the one the freeze recorded
E4  only authorized tool differences (tool_diff.py --check against the frozen tools)
E5  no receipt directory and no worktree exists for any Tranche-2 control; no mt2x worktree for any other control
E6  no conflicting process: no governed-control test executable, no cargo/rustc in a mission worktree, no p3c11-* scope,
    no other control.py
E7  the development worktree of receipts/03-t2-controls is exactly C (read-only) and locked with its role
E8  the order, the accepted and not-authorized sets, the commands, environments, limits and ceilings are the frozen
    expectations'
E9  every identity the freeze records is unchanged now
E10 the ruling and campaign records state the ruling and the 12-of-35 status; both frozen
E11 resources now: MemAvailable at least 18 GiB; at least 15 GiB of free disk for ten fresh worktree-local targets
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402
import control  # noqa: E402
import supervise  # noqa: E402
import worktrees  # noqa: E402

OUT = os.path.join(K.EV, "receipts/06-PRE-EXECUTION-GATE.txt")
DEV_WT = K.NEXUS + "/nexus-os-p3-c11-mt2x-dev-rehearsal"
lines, fails = [], []


def check(label, ok, detail=""):
    lines.append(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def last(rel):
    p = os.path.join(K.EV, rel)
    return open(p).read().rstrip().splitlines()[-1] if os.path.exists(p) else None


def main():
    if os.path.exists(OUT):
        raise SystemExit("REFUSED: the pre-execution gate receipt exists (run once)")
    lines.append(f"started: {supervise.now()}; mission {K.MISSION}")
    r = subprocess.run([sys.executable, "-B", os.path.join(K.EV, "tools/authenticate.py"), "--gate"],
                       capture_output=True, text=True)
    open(os.path.join(K.EV, "receipts/06-gate-authentication.txt"), "w").write(r.stdout + r.stderr)
    check("E1 authenticate.py --gate: ALL CHECKS HOLD (receipts/06-gate-authentication.txt)",
          r.returncode == 0 and r.stdout.rstrip().endswith("RESULT: ALL CHECKS HOLD"),
          f"{sum(1 for l in r.stdout.splitlines() if l.startswith('    OK '))} checks OK")
    rec = []
    fz = supervise.freeze_check(rec)
    check("E2 the freeze holds", fz is None, rec[0] if rec else fz)
    fr = json.load(open(supervise.FREEZE)) if os.path.exists(supervise.FREEZE) else {"files": {}}
    need = []
    for sub in ("tools", "original-tools", "original-publication-tools", "inputs"):
        for a, _, fs in os.walk(os.path.join(K.EV, sub)):
            need += [os.path.relpath(os.path.join(a, f), K.EV) for f in fs]
    need += ["receipts/02-EXPECTATIONS.json", K.RULING, K.CAMPAIGN]
    missing = [n for n in need if n not in fr["files"]]
    check(f"E2 the freeze covers every tool, both original tool sets, every input (catalog, the ten patches, linked "
          f"receipts, addenda, adoption records, proof), the expectations and both records ({len(need)} paths)",
          not missing and len(fr["files"]) == len(need), missing[:5])
    check("E2 the freeze record states the Tranche-2 order, the accepted and the not-authorized controls",
          fr.get("policy", {}).get("control_order") == K.ORDER
          and fr.get("policy", {}).get("architect_accepted_not_executed") == K.ACCEPTED
          and fr.get("policy", {}).get("not_authorized") == K.NOT_AUTHORIZED)
    import record_freeze
    for rel, want in record_freeze.PRE_FREEZE.items():
        check(f"E3 {rel}: {want}", last(rel) == want and fr.get("pre_freeze_receipts", {}).get(rel) ==
              K.sha256_file(os.path.join(K.EV, rel)), last(rel))
    r = subprocess.run([sys.executable, "-B", os.path.join(K.EV, "tools/tool_diff.py"), "--check"], capture_output=True,
                       text=True)
    check("E4 only authorized tool differences (tool_diff.py --check against the frozen tools)",
          r.returncode == 0 and r.stdout.rstrip().endswith("TOOL DIFF: ALL CHECKS HOLD"),
          r.stdout.rstrip().splitlines()[-1] if r.stdout else r.stderr[-200:])
    exp = supervise.expectations()
    present = [c for c in K.ORDER if os.path.lexists(os.path.join(K.EV, exp["controls"][c]["dir"]))
               or os.path.lexists(exp["controls"][c]["worktree"])]
    check("E5 no receipt directory and no worktree for any Tranche-2 control", not present, present)
    others = [c for c in K.ACCEPTED + K.NOT_AUTHORIZED if os.path.lexists(K.control_wt(c))]
    check("E5 no worktree of this mission's namespace for any accepted or not-authorized control", not others, others)
    exes, builds, scopes = supervise.conflicts(K.NEXUS + "/nexus-os-p3-c11-mt2x-")
    procs = [l for l in subprocess.run(["ps", "-eo", "pid=,args="], capture_output=True, text=True).stdout.splitlines()
             if "tools/control.py" in l and l.split(None, 1)[1].split()[0].endswith("python3")]
    check("E6 no conflicting mission process (governed-control test executable, cargo/rustc in a mission worktree, "
          "p3c11-* scope, control.py)", not exes and not builds and not scopes and not procs,
          f"{exes} {builds} {scopes} {procs}")
    os.environ["GIT_OPTIONAL_LOCKS"] = "0"
    v = control.restored_check(DEV_WT, "pre-execution gate")
    entry = worktrees.worktree_entry(DEV_WT) or []
    del os.environ["GIT_OPTIONAL_LOCKS"]
    check("E7 the development worktree of receipts/03-t2-controls is exactly C (read-only) and locked with its role",
          v["verdict"] == "EXACTLY_C" and any(l.startswith(f"locked {K.MISSION} development") for l in entry),
          f"{v['verdict']}; {entry[-1:] if entry else entry}")
    order_ok = (exp["order"] == K.ORDER and exp["accepted_not_executed"] == K.ACCEPTED
                and exp["not_authorized"] == K.NOT_AUTHORIZED
                and [exp["controls"][c]["dir"] for c in K.ORDER] == [f"receipts/{51 + i}-{c}" for i, c in
                                                                      enumerate(K.ORDER)])
    check("E8 order C23 C10 C16 C26 C3a C5 A3 A6 C8 C15; twelve accepted; thirteen not authorized; receipt directories "
          "51-60", order_ok)
    check("E8 build command, environment template, run environment, build limit, memory and swap are the frozen "
          "expectations'", exp["build"]["argv"] == K.BUILD_ARGV and exp["build"]["env_template"] == [
              list(x) for x in K.build_env("<worktree>/target")] and exp["run_env"] == dict(K.RUN_ENV)
          and exp["build"]["limit_seconds"] == 900 and exp["build"]["memory_max"] == K.MEMORY_MAX
          and exp["build"]["swap_max"] == "0")
    check("E8 every control: the original explorer, its catalog test with --exact --ignored --nocapture "
          "--test-threads=1, 2700 s, its ceiling (C10 60,000,000; the rest 30,000,000) and its permitted properties",
          all(exp["controls"][c]["argv_tail"] == [exp["controls"][c]["test"], "--exact", "--ignored", "--nocapture",
                                                    "--test-threads=1"] and exp["controls"][c]["limit_seconds"] == 2700
              and exp["controls"][c]["ceiling_states"] == K.MATRIX[c][2] and exp["controls"][c]["runner"] ==
              "full_exploration" and exp["controls"][c].get("permitted_properties", [exp["controls"][c]["named_property"]])
              == K.MATRIX[c][1] for c in K.ORDER))
    now = record_freeze.identities()
    rec_ids = fr.get("identities", {})
    check("E9 every identity the freeze records is unchanged now", rec_ids and json.loads(json.dumps(now)) == rec_ids,
          sorted(k for k in now if json.loads(json.dumps(now[k])) != rec_ids.get(k)))
    check("E10 the ruling and campaign records state the ruling and the 12-of-35 status (first lines), frozen",
          supervise.first_line(os.path.join(K.EV, K.RULING)) == K.RULING_FIRST
          and supervise.first_line(os.path.join(K.EV, K.CAMPAIGN)) == K.CAMPAIGN_FIRST
          and K.RULING in fr["files"] and K.CAMPAIGN in fr["files"])
    avail = supervise.meminfo("MemAvailable")
    st = os.statvfs(K.NEXUS)
    free = st.f_bavail * st.f_frsize >> 30
    check("E11 resources now: MemAvailable at least 18 GiB; at least 15 GiB free disk for ten fresh targets",
          avail >= K.NEED_KIB and free >= 15, f"MemAvailable {avail} KiB; disk free {free} GiB")
    lines.append(f"finished: {supervise.now()}")
    head = "PRE-EXECUTION GATE: " + ("ALL CHECKS HOLD" if not fails else "FAILED: " + "; ".join(fails))
    open(OUT, "w").write(head + "\n" + "\n".join(lines) + "\n")
    print(head)
    print("\n".join(lines))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main())
