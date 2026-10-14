#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01: pin the executable of the one build from that build's own, unedited
output. Builds nothing and runs nothing.

  pin.py            writes receipts/10-build/PIN.json (read-only) and receipts/11-pin/PIN-RECEIPT.txt

Why a separate step: supervise.py v3's build mode looked for cargo's non-verbose line `Executable unittests
src/lib.rs (<path>)`. Under CARGO_TERM_VERBOSE=true cargo prints `Executable `<path>`` instead, so the launcher
found no line, wrote no pin and reported BUILD NOT OK although the build exited 0. The build is not repeated (mission
section 5); this step reads the same receipt files and applies the same acceptance rule with both line forms.
"""
import hashlib
import json
import os
import re
import sys

EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-final-baseline-known-runs-01"
WT = "/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01"
TARGET = os.path.join(WT, "target")
BUILD = os.path.join(EV, "receipts/10-build")
PIN_FILE = os.path.join(BUILD, "PIN.json")
OUT = os.path.join(EV, "receipts/11-pin")
C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    if os.path.exists(PIN_FILE) or os.path.exists(OUT):
        raise SystemExit("REFUSED: a pin already exists")
    os.makedirs(OUT)
    rec = ["PIN — the executable of the one fresh optimised build, from the build's unedited output"]
    desc = json.load(open(os.path.join(BUILD, "descriptor.json")))
    stderr_path = os.path.join(BUILD, "build.stderr")
    stderr = open(stderr_path, errors="replace").read()
    rec.append(f"build receipt: receipts/10-build (build.stderr sha256 {sha256(stderr_path)}, "
               f"{len(stderr.splitlines())} lines; descriptor.json sha256 {sha256(os.path.join(BUILD, 'descriptor.json'))})")
    proc, sup = desc["process"], desc["supervisor"]
    checks = []

    def check(label, good, detail):
        checks.append(good)
        rec.append(f"{'OK  ' if good else 'FAIL'} {label}: {detail}")

    check("build process", proc["exit_status"] == 0 and proc["signal"] is None,
          f"exit status {proc['exit_status']}, signal {proc['signal']}, supervisor rc {proc['supervisor_rc']}")
    check("supervision", sup["completed"] and not sup["timed_out"] and sup["oom_kill"] == 0
          and sup["memory_max"] == "17179869184" and sup["swap_max"] == "0" and sup["swap_peak_bytes"] == "0",
          f"memory.max {sup['memory_max']}, memory.swap.max {sup['swap_max']}, memory.peak {sup['memory_peak_bytes']} B, "
          f"memory.swap.peak {sup['swap_peak_bytes']} B, oom_kill {sup['oom_kill']}, wall {sup['wall_seconds']} s of "
          f"{sup['time_limit_seconds']} s, completed {sup['completed']}")
    check("source before and after the build", desc["source_before"]["verdict"] == "EXACTLY_C"
          and desc["source_after"]["verdict"] == "EXACTLY_C",
          f"{desc['source_before']['verdict']} / {desc['source_after']['verdict']}")
    finished = re.findall(r"^\s*Finished `(\w+)` profile \[([^\]]+)\] target\(s\) in (.+)$", stderr, re.M)
    check("cargo's Finished line", finished == [("release", "optimized", finished[0][2])] if finished else False,
          f"{finished}")
    verbose = re.findall(r"^\s*Executable `([^`]+)`\s*$", stderr, re.M)
    plain = re.findall(r"^\s*Executable unittests src/lib\.rs \(([^)]+)\)\s*$", stderr, re.M)
    named = [p if os.path.isabs(p) else os.path.join(WT, p) for p in verbose + plain]
    check("executables cargo named (both line forms)", len(named) == 1,
          f"verbose form {verbose}; non-verbose form {plain}")
    deps = os.path.join(TARGET, "release/deps")
    on_disk = sorted(os.path.join(deps, n) for n in os.listdir(deps) if re.fullmatch(r"nexus_governed_control-[0-9a-f]{16}", n))
    check("library-test executables in the fresh target equal the one cargo named", on_disk == named, f"{on_disk}")
    others = sorted(n for n in os.listdir(deps) if n.startswith("nexus_governed_control") and not n.endswith(".d"))
    check("no other nexus_governed_control artifact in release/deps", others == [os.path.basename(p) for p in named],
          f"{others}")
    rustc = [l for l in stderr.splitlines() if "--crate-name nexus_governed_control " in l]
    check("one rustc invocation compiled nexus_governed_control, as a test", len(rustc) == 1 and " --test " in rustc[0],
          f"{len(rustc)} invocation(s)")
    flags = {}
    if rustc:
        line = rustc[0]
        flags = {"-C": re.findall(r" -C (\S+)", line), "--cfg": re.findall(r" --cfg (\S+)", line),
                 "--check-cfg": re.findall(r" --check-cfg ('[^']*'|\S+)", line), "--test": " --test " in line,
                 "--edition": re.findall(r" --edition=(\S+)", line)}
        rec.append(f"effective rustc flags of the test unit (from the build's own verbose message): {json.dumps(flags)}")
        check("profile flags", [f for f in flags["-C"] if not f.startswith(("metadata=", "extra-filename="))]
              == ["opt-level=3", "lto=thin", "codegen-units=16", "strip=symbols"],
              "opt-level=3, lto=thin, codegen-units=16, strip=symbols; no debuginfo, debug-assertions, overflow-checks or "
              "panic flag, so rustc's defaults hold: debuginfo 0, debug assertions off and overflow checks off at "
              "opt-level 3, panic unwind")
        check("features and cfg", flags["--cfg"] == [] and "'cfg(feature, values())'" in flags["--check-cfg"],
              "no --cfg flag (no feature enabled, no extra cfg); the declared feature set is empty")
    fp = os.path.join(TARGET, "release/.fingerprint", "nexus-governed-control-" + os.path.basename(named[0]).split("-")[-1]
                      if named else "")
    fpj = [n for n in os.listdir(fp) if n.endswith(".json")] if named and os.path.isdir(fp) else []
    for n in fpj:
        d = json.load(open(os.path.join(fp, n)))
        rec.append(f"fingerprint {n}: rustc {d.get('rustc')}, features {d.get('features')}, declared_features "
                   f"{d.get('declared_features')}, rustflags {d.get('rustflags')}, profile hash {d.get('profile')}, "
                   f"config {d.get('config')}, compile_kind {d.get('compile_kind')}")
        check(f"fingerprint {n}: no rustflags, no features", d.get("rustflags") == [] and d.get("features") == "[]",
              f"rustflags {d.get('rustflags')}, features {d.get('features')}")
    depinfo = os.path.join(deps, os.path.basename(named[0]) + ".d") if named else ""
    if os.path.exists(depinfo):
        srcs = sorted({p for p in re.findall(r"(\S+\.rs)", open(depinfo).read())})
        outside = [p for p in srcs if not p.startswith(os.path.join(WT, "crates/nexus-governed-control/src/"))]
        rec.append(f"dep-info {os.path.basename(depinfo)}: {len(srcs)} distinct .rs sources, all under "
                   f"crates/nexus-governed-control/src/: {not outside}; other inputs listed: "
                   f"{sorted({p for p in re.findall(r'(/[^ :]+)', open(depinfo).read()) if not p.endswith('.rs')})}")
        check("dep-info names no docs/ path", not any("/docs/" in p for p in re.findall(r"(\S+)", open(depinfo).read())),
              "no path under docs/ is an input of the test executable")
    if all(checks):
        exe = named[0]
        pin = {"executable": exe, "sha256": sha256(exe), "size": os.path.getsize(exe), "commit": C, "tree": C_TREE,
               "rustc": re.search(r"^compiler \(.*?\): (.+)$", open(os.path.join(BUILD, "RECEIPT.txt")).read(), re.M).group(1),
               "rustc_path": re.search(r"Running `(\S+/rustc) --crate-name nexus_governed_control ", stderr).group(1), "build_receipt": "receipts/10-build/RECEIPT.txt",
               "pinned_by": "tools/pin.py from receipts/10-build/build.stderr", "effective_rustc_flags": flags}
        json.dump(pin, open(PIN_FILE, "w"), indent=1)
        os.chmod(PIN_FILE, 0o444)
        rec.append(f"PINNED: executable {exe}")
        rec.append(f"        sha256 {pin['sha256']}")
        rec.append(f"        size {pin['size']} bytes")
    rec.append(f"OUTCOME: {'PINNED' if all(checks) else 'NOT PINNED'}")
    open(os.path.join(OUT, "PIN-RECEIPT.txt"), "w").write("\n".join(rec) + "\n")
    print("\n".join(rec))
    return 0 if all(checks) else 4


if __name__ == "__main__":
    sys.exit(main())
