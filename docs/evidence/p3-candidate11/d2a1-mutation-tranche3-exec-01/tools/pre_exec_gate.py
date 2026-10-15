#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01, section 10: the static pre-execution gate. Revised by -CONTINUE-02.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (section 7) and -TRANCHE2-CONTINUE-02 (section 16). Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01 (section 11): run once after the freeze and before C2. Read-only (it writes
only its own receipts). Nothing is built or run.

    pre_exec_gate.py        writes receipts/06-PRE-EXECUTION-GATE.txt (first line `PRE-EXECUTION GATE: ALL CHECKS HOLD`
                            or `PRE-EXECUTION GATE: FAILED: ...`) and receipts/06-gate-authentication.txt; refuses if
                            the gate receipt exists

E1  authenticate.py --gate: Candidate C, the catalog and the ten patches, the contract, AD-1-R4/R5 and the adoption
    records, the accepted proof, Stage-1/Stage-2 evidence and every linked receipt, the complete Tranche-1 evidence,
    the Tranche-2 stopped evidence, the accepted continuation evidence and its five controls, the baseline executable,
    refs and destination absence
E2  the freeze holds, and it covers every tool, the base tool set, every input, the expectations and the two records
E3  every pre-freeze receipt records success and is the one the freeze recorded
E4  only authorized tool differences (tool_diff.py --check against the frozen tools)
E5  no receipt directory and no worktree for any of this mission's controls; none of this mission's namespace (mt3x)
    for any other control
E6  no conflicting process: no governed-control test executable, no cargo/rustc in a mission worktree (mt3x), no
    p3c11-* scope, no other control.py
E7  the 22 accepted controls' worktrees exactly C (read-only; not touched)
E8  the order, the accepted and reserved sets, receipt directories 66-75, the commands, environments, limits and
    ceilings are the frozen expectations'; C3b's two permitted properties; C6/C7 A4 ON on full_fpt; C20/A5 the L1
    complete-search rule; C7's informational F-3 run (full_f3, A4 off, 30,000,000 states, 2700 s, not required to kill)
E9  every identity the freeze records is unchanged now
E10 the ruling and campaign records state the ruling and the 22-of-35 status; both frozen
E11 resources now: MemAvailable at least 18 GiB; at least 12 GiB of free disk for ten fresh worktree-local targets
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402
import control  # noqa: E402
import supervise  # noqa: E402

OUT = os.path.join(K.EV, "receipts/06-PRE-EXECUTION-GATE.txt")
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
    for sub in ("tools", "original-tools", "inputs"):
        for a, _, fs in os.walk(os.path.join(K.EV, sub)):
            need += [os.path.relpath(os.path.join(a, f), K.EV) for f in fs]
    need += ["receipts/02-EXPECTATIONS.json", K.RULING, K.CAMPAIGN]
    missing = [n for n in need if n not in fr["files"]]
    check(f"E2 the freeze covers every tool, the continuation's base tool set, every input (catalog, the ten patches, "
          f"linked receipts, addenda, adoption records, proof), the expectations and the two records ({len(need)} paths)",
          not missing and len(fr["files"]) == len(need), missing[:5])
    check("E2 the freeze record states this mission's order, the accepted and the not-authorized controls",
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
    check("E5 no receipt directory and no worktree for any of this mission's controls", not present, present)
    others = [c for c in K.ACCEPTED + K.NOT_AUTHORIZED if os.path.lexists(K.NEXUS + "/nexus-os-p3-c11-mt3x-" + c)]
    check("E5 no worktree of this mission's namespace (mt3x) for any accepted or reserved control", not others, others)
    exes, builds, scopes = supervise.conflicts(K.NEXUS + "/nexus-os-p3-c11-mt3x")
    procs = [l for l in subprocess.run(["ps", "-eo", "pid=,args="], capture_output=True, text=True).stdout.splitlines()
             if "tools/control.py" in l and l.split(None, 1)[1].split()[0].endswith("python3")]
    check("E6 no conflicting mission process (governed-control test executable, cargo/rustc in a mission worktree, "
          "p3c11-* scope, control.py)", not exes and not builds and not scopes and not procs,
          f"{exes} {builds} {scopes} {procs}")
    os.environ["GIT_OPTIONAL_LOCKS"] = "0"  # read-only checks of worktrees this mission does not own
    for c in K.ACCEPTED:
        v = control.restored_check(K.ACCEPTED_WT[c], "pre-execution gate")
        check(f"E7 {c} (Architect-accepted; its worktree {K.ACCEPTED_WT[c]}, read-only): exactly C, not touched",
              v["verdict"] == "EXACTLY_C", v["verdict"])
    del os.environ["GIT_OPTIONAL_LOCKS"]
    order_ok = (exp["order"] == K.ORDER and exp["accepted_not_executed"] == K.ACCEPTED
                and exp["not_authorized"] == K.NOT_AUTHORIZED
                and [exp["controls"][c]["dir"] for c in K.ORDER] == [f"receipts/{K.FIRST_DIR + i}-{c}" for i, c in
                                                                      enumerate(K.ORDER)])
    check("E8 order C2 C24 C3b C3c C6 C7 A1 A2 C20 A5; 22 accepted; C1 C4 C25 reserved; receipt directories 66-75",
          order_ok)
    check("E8 build command, environment template, run environment, build limit, memory and swap are the frozen "
          "expectations'", exp["build"]["argv"] == K.BUILD_ARGV and exp["build"]["env_template"] == [
              list(x) for x in K.build_env("<worktree>/target")] and exp["run_env"] == dict(K.RUN_ENV)
          and exp["build"]["limit_seconds"] == 900 and exp["build"]["memory_max"] == K.MEMORY_MAX
          and exp["build"]["swap_max"] == "0")
    check("E8 every control: the original explorer, its catalog test with --exact --ignored --nocapture "
          "--test-threads=1, 2700 s, its ceiling (60,000,000 for F-1, else 30,000,000) and its permitted properties",
          all(exp["controls"][c]["argv_tail"] == [exp["controls"][c]["test"], "--exact", "--ignored", "--nocapture",
                                                    "--test-threads=1"] and exp["controls"][c]["limit_seconds"] == 2700
              and exp["controls"][c]["ceiling_states"] == K.MATRIX[c][2] and exp["controls"][c]["runner"] ==
              "full_exploration" and exp["controls"][c].get("permitted_properties", [exp["controls"][c]["named_property"]])
              == K.MATRIX[c][1] for c in K.ORDER))
    C_ = exp["controls"]
    check("E8 C3b permits P13 or P12; C3c P12 only; every other control its one property",
          C_["C3b"].get("permitted_properties") == ["P13", "P12"] and C_["C3c"]["named_property"] == "P12"
          and "permitted_properties" not in C_["C3c"]
          and all("permitted_properties" not in C_[c] for c in K.ORDER if c != "C3b"))
    check("E8 C6 and C7: full_fpt on the original explorer, A4 ON (bounds ' A4 true;'), the quotient forbidden; their "
          "restored side the Stage-1 F-PT composed-quotient PASS",
          all(C_[c]["test"] == K.TEST_PREFIX + "full_fpt" and C_[c]["configuration"] == "F-PT" and " A4 true;" in
              C_[c]["bounds"] and C_[c].get("a4", {}).get("required") is True and C_[c]["linked"]["quotient"] is True
              and C_[c]["linked"]["descriptor_test"] == K.TEST_PREFIX + "gc_symmetry_fpt" for c in K.A4_ON)
          and not [c for c in K.ORDER if c not in K.A4_ON and (C_[c].get("a4") or C_[c]["linked"]["quotient"])])
    check("E8 C20 and A5: F-2, L1 only, the complete-search rule present; no other control carries it",
          all(C_[c]["configuration"] == "F-2" and C_[c]["named_property"] == "L1" and C_[c].get("l1_rule", {}).get(
              "complete_search_required") is True for c in K.L1_CONTROLS)
          and not [c for c in K.ORDER if c not in K.L1_CONTROLS and C_[c].get("l1_rule")])
    inf = C_["C7"].get("informational") or {}
    check("E8 C7's informational run: full_f3, original explorer, A4 off, 30,000,000 states, 2700 s, not required to "
          "kill; no other control has one",
          inf.get("test") == K.TEST_PREFIX + "full_f3" and inf.get("explorer") == "original" and inf.get("a4") is False
          and " A4 false;" in inf.get("bounds", "") and inf.get("ceiling_states") == 30000000
          and inf.get("limit_seconds") == 2700 and inf.get("required_to_kill") is False
          and inf.get("argv_tail") == [K.TEST_PREFIX + "full_f3", "--exact", "--ignored", "--nocapture", "--test-threads=1"]
          and not [c for c in K.ORDER if c != "C7" and C_[c].get("informational")])
    now = record_freeze.identities()
    rec_ids = fr.get("identities", {})
    check("E9 every identity the freeze records is unchanged now", rec_ids and json.loads(json.dumps(now)) == rec_ids,
          sorted(k for k in now if json.loads(json.dumps(now[k])) != rec_ids.get(k)))
    check("E10 the ruling and campaign records state the ruling and the 22-of-35 status (first lines), frozen",
          supervise.first_line(os.path.join(K.EV, K.RULING)) == K.RULING_FIRST
          and supervise.first_line(os.path.join(K.EV, K.CAMPAIGN)) == K.CAMPAIGN_FIRST
          and K.RULING in fr["files"] and K.CAMPAIGN in fr["files"])
    avail = supervise.meminfo("MemAvailable")
    st = os.statvfs(K.NEXUS)
    free = st.f_bavail * st.f_frsize >> 30
    check("E11 resources now: MemAvailable at least 18 GiB; at least 12 GiB free disk for ten fresh targets",
          avail >= K.NEED_KIB and free >= 12, f"MemAvailable {avail} KiB; disk free {free} GiB")
    lines.append(f"finished: {supervise.now()}")
    head = "PRE-EXECUTION GATE: " + ("ALL CHECKS HOLD" if not fails else "FAILED: " + "; ".join(fails))
    open(OUT, "w").write(head + "\n" + "\n".join(lines) + "\n")
    print(head)
    print("\n".join(lines))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main())
