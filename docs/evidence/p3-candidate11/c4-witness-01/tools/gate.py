#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-C4-WITNESS-01: the declared gate. Run once. It refuses to run twice.

  gate.py <A-commit>

The authorised execution, and nothing else, in this order; the first failure stops it (no retry, no alternative):
  1. verify the freeze (FREEZE-SHA256SUMS) and that the engineering worktree is clean at A;
  2. one optimised healthy test build of A (supervise.py build; at most 2 compiler jobs, 16 GiB, 15 min, no swap);
  3. one healthy invocation of only the C4 witness on that pinned executable (16 GiB, 180 s, no swap);
     HEALTHY GATE: build BUILD OK and invocation PASS under classify.py;
  4. the one disposable mutant worktree: A plus exactly the frozen C4 patch (c4-mutant.py apply);
  5. one optimised build of it; 6. one invocation of only the same witness test on that pinned mutant executable;
     MUTANT GATE: build BUILD OK and invocation POSITIVE (P4 and P8 each OBSERVED at its frozen locus, exit 101);
  7. the mutant restored and verified against A by blob (always, once applied).
"""
import json, os, subprocess, sys, time

EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c4-witness-01"
WT = "/home/nexus/NEXUS/nexus-os-p3-c11-c4-witness-01"
MWT = "/home/nexus/NEXUS/nexus-os-p3-c11-c4-witness-01-mutant"
R = os.path.join(EV, "receipts")
T = os.path.join(EV, "tools")
MODEL = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
log = []


def note(text):
    line = f"{time.strftime('%H:%M:%S')} {text}"
    log.append(line)
    print(line, flush=True)


def call(args, receipt=None):
    r = subprocess.run(args, capture_output=True, text=True)
    if receipt:
        open(receipt, "w").write(r.stdout + r.stderr + f"exit {r.returncode}\n")
    return r


def classify(desc, pins, receipt):
    pins_path = os.path.join(os.path.dirname(desc), "pins.json")
    json.dump(pins, open(pins_path, "w"), indent=1)
    r = call(["python3", os.path.join(T, "classify.py"), desc, pins_path], receipt)
    note(f"  classify {os.path.relpath(desc, EV)}: {r.stdout.splitlines()[0] if r.stdout else r.stderr.strip()}")
    return r.returncode == 0, r.stdout


def finish(status):
    note(f"GATE RESULT: {status}")
    open(os.path.join(R, "20-GATE-SUMMARY.txt"), "w").write("\n".join(log) + "\n")
    sys.exit(0 if status.startswith("BOTH GATES PASSED") else 1)


def main():
    a = sys.argv[1]
    if os.path.exists(os.path.join(R, "20-GATE-SUMMARY.txt")) or os.path.exists(os.path.join(R, "08-build-healthy")):
        raise SystemExit("REFUSED: the gate has already been started")
    note(f"P3-C11-DR01-D2A1-C4-WITNESS-01 DECLARED GATE — A {a}")
    r = subprocess.run(["sha256sum", "--quiet", "-c", "frozen/FREEZE-SHA256SUMS"], cwd=EV, capture_output=True, text=True)
    note(f"  freeze verification (frozen/FREEZE-SHA256SUMS): exit {r.returncode} {r.stdout.strip()} {r.stderr.strip()}")
    if r.returncode != 0:
        finish("STOPPED: the freeze does not verify")
    g = lambda cwd, *x: subprocess.run(["git", "-C", cwd, *x], capture_output=True, text=True, check=True).stdout.strip()
    head, tree, por = g(WT, "rev-parse", "HEAD"), g(WT, "rev-parse", "HEAD^{tree}"), g(WT, "status", "--porcelain")
    pins = {"source_commit": a, "source_tree": g(WT, "rev-parse", f"{a}^{{tree}}"),
            "model_tests_blob": g(WT, "rev-parse", f"{a}:{MODEL}"),
            "baseline_protocol_blob": "a2df2841b99cad5d452008a6d4150e9369d07e66",
            "mutant_protocol_blob": "f3793c2ff61f8adbf43fe17d6b5237b49bc507b0"}
    note(f"  engineering worktree: HEAD {head} tree {tree} porcelain {por!r}; pins {pins}")
    if head != a or por:
        finish("STOPPED: the engineering worktree is not clean at A")

    note("STEP 2: healthy build")
    r = call(["python3", os.path.join(T, "supervise.py"), "build", os.path.join(R, "08-build-healthy"), WT, "baseline"])
    note(f"  supervise build exit {r.returncode}")
    ok, _ = classify(os.path.join(R, "08-build-healthy/descriptor.json"), pins, os.path.join(R, "08-build-healthy/CLASSIFICATION.txt"))
    if r.returncode != 0 or not ok:
        finish("STOPPED: the healthy build failed or is not admissible")
    d = json.load(open(os.path.join(R, "08-build-healthy/descriptor.json")))
    exe = d["recorded_executables"][0]
    pins["baseline_executable_sha256"] = exe["sha256"]
    note(f"  pinned baseline executable {exe['path']} sha256 {exe['sha256']} size {exe['size']}")

    note("STEP 3: healthy invocation")
    r = call(["python3", os.path.join(T, "supervise.py"), "run", os.path.join(R, "09-run-healthy"), WT, "baseline",
              exe["path"], exe["sha256"]])
    note(f"  supervise run exit {r.returncode}")
    ok, _ = classify(os.path.join(R, "09-run-healthy/descriptor.json"), pins, os.path.join(R, "09-run-healthy/CLASSIFICATION.txt"))
    if r.returncode != 0 or not ok:
        finish("STOPPED: HEALTHY GATE FAILED")
    note("HEALTHY GATE PASSED")

    note("STEP 4: the disposable mutant")
    r = call(["python3", os.path.join(T, "c4-mutant.py"), "apply", a, os.path.join(R, "10-mutant-apply.txt")])
    note(f"  c4-mutant apply exit {r.returncode}")
    if r.returncode != 0:
        if os.path.isdir(MWT):
            rr = call(["python3", os.path.join(T, "c4-mutant.py"), "restore", a, os.path.join(R, "13-mutant-restore.txt")])
            note(f"  c4-mutant restore exit {rr.returncode}")
        finish("STOPPED: the mutant could not be applied exactly")
    status = None
    note("STEP 5: mutant build")
    r = call(["python3", os.path.join(T, "supervise.py"), "build", os.path.join(R, "11-build-mutant"), MWT, "mutant"])
    note(f"  supervise build exit {r.returncode}")
    ok, _ = classify(os.path.join(R, "11-build-mutant/descriptor.json"), pins, os.path.join(R, "11-build-mutant/CLASSIFICATION.txt"))
    if r.returncode != 0 or not ok:
        status = "STOPPED: the mutant build failed or is not admissible"
    else:
        d = json.load(open(os.path.join(R, "11-build-mutant/descriptor.json")))
        mexe = d["recorded_executables"][0]
        pins["mutant_executable_sha256"] = mexe["sha256"]
        note(f"  pinned mutant executable {mexe['path']} sha256 {mexe['sha256']} size {mexe['size']}")
        note("STEP 6: mutant invocation")
        r = call(["python3", os.path.join(T, "supervise.py"), "run", os.path.join(R, "12-run-mutant"), MWT, "mutant",
                  mexe["path"], mexe["sha256"]])
        note(f"  supervise run exit {r.returncode}")
        ok, _ = classify(os.path.join(R, "12-run-mutant/descriptor.json"), pins, os.path.join(R, "12-run-mutant/CLASSIFICATION.txt"))
        status = None if (r.returncode == 0 and ok) else "STOPPED: MUTANT GATE FAILED"
        if status is None:
            note("MUTANT GATE PASSED")
    note("STEP 7: restore the mutant")
    rr = call(["python3", os.path.join(T, "c4-mutant.py"), "restore", a, os.path.join(R, "13-mutant-restore.txt")])
    note(f"  c4-mutant restore exit {rr.returncode}")
    json.dump(pins, open(os.path.join(R, "pins-final.json"), "w"), indent=1)
    if rr.returncode != 0:
        finish("STOPPED: the mutant worktree did not restore to A" + (f"; also {status}" if status else ""))
    finish(status or "BOTH GATES PASSED (healthy PASS; mutant P4 and P8 OBSERVED); mutant restored to A")


if __name__ == "__main__":
    main()
