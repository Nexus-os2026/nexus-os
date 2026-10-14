#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01, section 10: the static final pre-execution gate, run once after the
freeze and before C14. Read-only (it writes only its own receipts). Nothing is built or run.

    pre_exec_gate.py        writes receipts/06-PRE-EXECUTION-GATE.txt (first line `PRE-EXECUTION GATE: ALL CHECKS HOLD`
                            or `PRE-EXECUTION GATE: FAILED`) and receipts/06-gate-authentication.txt; refuses if the
                            gate receipt exists

E1  authenticate.py --gate: C, the R2 catalog and the twelve patches, contract, AD-1-R4/R5, Stage-1/Stage-2 evidence,
    the complete previous packet and its SHA256SUMS, the C22 ruling's facts, the carried-forward copies, the pinned
    baseline executable, refs and destination absence
E2  the freeze is recorded and holds (every frozen file equals its recorded sha256), and it covers the repaired tools,
    every unchanged carried-forward tool, the EXEC-01 tools, the inputs, the expectations, the carried-forward receipts
    and the C22 ruling record
E3  every pre-freeze receipt records success (authentication, supervision self-test, verifier controls, tool diff,
    render rehearsal)
E4  only authorized tool differences exist (tool_diff.py --check, again, against the frozen tools)
E5  no receipt directory and no worktree exists for C14 or any later control; nothing outside the tranche exists
E6  no conflicting mission process: no governed-control test executable, no cargo/rustc in a mission worktree, no
    p3c11-* scope, no other control.py
E7  the development and preserved C22 worktrees are exactly C (read-only restored check)
E8  the control order, the accepted C22 and the commands/environments are the frozen ones
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02 (section 14), run once after the freeze and before the C28
invocation: E1 also authenticates the complete RESUME-01 packet; E2 covers both Architect ruling records, the
RESUME-01 tools and the dedicated C28 tools; E3 the C28 admission/pin tool controls; E5 no receipt directory for C28 or
any later control, no worktree for C12 or later, the preserved C28 worktree present; E7 the preserved C28 worktree
exactly C (read-only); E8 the continue order and the five Architect-accepted controls; E9 every identity the freeze
records (both previous packets, the C28 prior build evidence, the C28 expected executable, the carried-forward
baselines, C and the catalog) is unchanged; E10 the ruling records state the rulings.
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
    r = subprocess.run([sys.executable, os.path.join(K.EV, "tools/authenticate.py"), "--gate"], capture_output=True,
                       text=True)
    open(os.path.join(K.EV, "receipts/06-gate-authentication.txt"), "w").write(r.stdout + r.stderr)
    check("E1 authenticate.py --gate: ALL CHECKS HOLD (receipts/06-gate-authentication.txt)",
          r.returncode == 0 and r.stdout.rstrip().endswith("RESULT: ALL CHECKS HOLD"),
          f"{sum(1 for l in r.stdout.splitlines() if l.startswith('    OK '))} checks OK")
    rec = []
    fz = supervise.freeze_check(rec)
    check("E2 the freeze holds", fz is None, rec[0] if rec else fz)
    fr = json.load(open(supervise.FREEZE)) if os.path.exists(supervise.FREEZE) else {"files": {}}
    tools = sorted(f for f in os.listdir(os.path.join(K.EV, "tools")) if f.endswith((".py", ".sh")))
    need = ([f"tools/{t}" for t in tools] + [f"original-tools/{t}" for t in os.listdir(os.path.join(K.EV,
                                                                                               "original-tools"))]
            + ["receipts/02-EXPECTATIONS.json", K.RULING, K.C28_RULING, "vocabulary/VOCABULARY.json",
               "inputs/catalog.json", "inputs/linked/F-B2/CLASSIFICATION.txt", "inputs/linked/F-G/CLASSIFICATION.txt"]
            + [f"inputs/patches/{c}.patch" for c in K.ORDER])
    for c in K.CARRIED:
        p = os.path.join(K.EV, c)
        if os.path.isdir(p):
            need += [os.path.relpath(os.path.join(a, f), K.EV) for a, _, fs in os.walk(p) for f in fs]
    missing = [n for n in need if n not in fr["files"]]
    check(f"E2 the freeze covers every tool ({len(tools)}), the RESUME-01 tools, the expectations, the catalog and the "
          f"twelve patches, the linked Stage-2 receipts, the carried-forward receipts and both Architect ruling records "
          f"({len(need)} required paths)", not missing, missing[:5])
    check("E2 the freeze record states the continue order and the five Architect-accepted controls",
          fr.get("policy", {}).get("control_order") == K.CONTINUE_ORDER
          and fr.get("policy", {}).get("architect_accepted_not_executed") == K.CONTINUE_ACCEPTED)
    for rel, want in (("receipts/00-authentication.txt", "RESULT: ALL CHECKS HOLD"),
                      ("receipts/01-supervision-selftest/SELFTEST.txt", "SELF-TEST RESULT: ALL OK"),
                      ("receipts/03-c28-admission-controls/SUMMARY.txt", "C28 ADMISSION CONTROLS: ALL AS EXPECTED"),
                      ("receipts/04-tool-diff/TOOL-DIFF-CHECK.txt", "TOOL DIFF: ALL CHECKS HOLD"),
                      ("receipts/04b-render-rehearsal/SUMMARY.txt", "RENDER REHEARSAL: ALL AS EXPECTED")):
        check(f"E3 {rel}: {want}", last(rel) == want and fr.get("pre_freeze_receipts", {}).get(rel) ==
              K.sha256_file(os.path.join(K.EV, rel)), last(rel))
    r = subprocess.run([sys.executable, os.path.join(K.EV, "tools/tool_diff.py"), "--check"], capture_output=True,
                       text=True)
    check("E4 only authorized tool differences (tool_diff.py --check against the frozen tools)",
          r.returncode == 0 and r.stdout.rstrip().endswith("TOOL DIFF: ALL CHECKS HOLD"),
          r.stdout.rstrip().splitlines()[-1] if r.stdout else r.stderr[-200:])
    exp = supervise.expectations()
    later = [c for c in K.CONTINUE_ORDER if os.path.lexists(os.path.join(K.EV, exp["controls"][c]["dir"]))
             or (c != "C28" and os.path.lexists(exp["controls"][c]["worktree"]))]
    check("E5 no receipt directory for C28 or any later control; no worktree for C12 or any later control", not later,
          later)
    check("E5 the preserved C28 worktree is present (it is not recreated)", os.path.isdir(K.control_wt("C28")))
    accepted = [c for c in K.CONTINUE_ACCEPTED if os.path.lexists(os.path.join(K.EV, exp["controls"][c]["dir"]))]
    check("E5 no receipt directory for an Architect-accepted control (C22 C14 C19 C21 C27 are not executed)",
          not accepted, accepted)
    outside = [c for c in K.NOT_AUTHORIZED if os.path.lexists(K.control_wt(c))]
    check("E5 no worktree for any control outside the tranche", not outside, outside)
    exes, builds, scopes = supervise.conflicts(K.NEXUS + "/nexus-os-p3-c11-mt1x-")
    others = [l for l in subprocess.run(["pgrep", "-af", "tools/control.py"], capture_output=True,
                                        text=True).stdout.splitlines() if str(os.getpid()) not in l.split()[0]]
    check("E6 no conflicting mission process (governed-control test executable, cargo/rustc in a mission worktree, "
          "p3c11-* scope, control.py)", not exes and not builds and not scopes and not others,
          f"{exes} {builds} {scopes} {others}")
    os.environ["GIT_OPTIONAL_LOCKS"] = "0"
    v = control.restored_check(K.control_wt("C28"), "pre-execution gate")
    check("E7 the preserved C28 worktree is exactly C (read-only restored check)", v["verdict"] == "EXACTLY_C",
          f"{v['verdict']}; {v['status_policy']['target_entries']} target entries")
    del os.environ["GIT_OPTIONAL_LOCKS"]
    check("E8 control order C28 C12 C9 C11 C17 C18a C18b; C22 C14 C19 C21 C27 Architect-accepted, not executed",
          K.CONTINUE_ORDER == ["C28", "C12", "C9", "C11", "C17", "C18a", "C18b"]
          and K.CONTINUE_ACCEPTED == ["C22", "C14", "C19", "C21", "C27"] and exp["order"] == K.ORDER
          and all(exp["controls"][c]["dir"].endswith("-" + c) for c in K.CONTINUE_ORDER))
    check("E8 build command and environment template, run environment and limits are the expectations' (frozen)",
          exp["build"]["argv"] == K.BUILD_ARGV and exp["build"]["env_template"] == [list(x) for x in K.build_env(
              "<worktree>/target")] and exp["run_env"] == dict(K.RUN_ENV) and exp["build"]["limit_seconds"] == 900
          and exp["build"]["memory_max"] == K.MEMORY_MAX and exp["build"]["swap_max"] == "0")
    import record_freeze
    now = record_freeze.identities()
    rec_ids = fr.get("identities", {})
    check("E9 every identity the freeze records is unchanged now (both previous packets, the C28 prior build evidence, "
          "the C28 expected executable, the carried-forward baselines, C, the catalog)",
          rec_ids and json.loads(json.dumps(now)) == rec_ids, sorted(k for k in now if json.loads(json.dumps(now[k]))
                                                                     != rec_ids.get(k)))
    check("E10 the Architect ruling records state the rulings (first lines), frozen",
          supervise.first_line(os.path.join(K.EV, K.RULING)) == K.RULING_FIRST
          and supervise.first_line(os.path.join(K.EV, K.C28_RULING)) == K.C28_RULING_FIRST
          and K.RULING in fr["files"] and K.C28_RULING in fr["files"])
    lines.append(f"finished: {supervise.now()}")
    head = "PRE-EXECUTION GATE: " + ("ALL CHECKS HOLD" if not fails else "FAILED: " + "; ".join(fails))
    open(OUT, "w").write(head + "\n" + "\n".join(lines) + "\n")
    print(head)
    print("\n".join(lines))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main())
