#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01: the final static gate, run after every execution and before the packet
is assembled. Read-only; runs and builds nothing.

  final_gate.py > receipts/60-final-gate.txt

Re-checks: authentication (authenticate.py: C, blobs, build inputs, AD-1-R4, proofs, refs, destination absent); the
worktree is still exactly C and the target directory and pinned executable are preserved; every invocation's
executable digest equals the pin; the suite and the five runs are classified PASS in the required order, each started
only after its predecessor; the A4 record is satisfied; the derived and measured counts are the mission's; and no
secret-like text is in the mission's records.
"""
import hashlib
import json
import os
import re
import subprocess
import sys

EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-final-baseline-known-runs-01"
WT = "/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01"
REPO = "/home/nexus/NEXUS/nexus-os"
PIN = json.load(open(os.path.join(EV, "receipts/10-build/PIN.json")))
ORDER = ["receipts/30-suite", "receipts/41-full-F-2", "receipts/42-full-F-B", "receipts/43-full-F-3",
         "receipts/44-full-F-PT", "receipts/45-full-F-1"]
SECRET = re.compile(r"(gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-ant-[A-Za-z0-9-]{10,}|"
                    r"AKIA[0-9A-Z]{16}|-----BEGIN [A-Z ]*PRIVATE KEY-----|xox[abp]-[A-Za-z0-9-]{10,}|"
                    r"(?i:authorization: *bearer +[A-Za-z0-9._-]{16,}))")
fails = []


def check(label, ok, detail=""):
    print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    print("FINAL GATE — P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01")
    a = subprocess.run([sys.executable, os.path.join(EV, "tools/authenticate.py"), REPO], capture_output=True, text=True)
    check("authentication re-run (authenticate.py)", a.returncode == 0 and "RESULT: ALL CHECKS HOLD" in a.stdout,
          a.stdout.strip().splitlines()[-1])
    t = json.loads(subprocess.run([sys.executable, os.path.join(EV, "tools/verify_tree.py"), WT], capture_output=True,
                                  text=True).stdout)
    check("mission worktree still exactly C (byte for byte); only the target directory untracked", t["verdict"] ==
          "EXACTLY_C" and t["status_entries"] == ["!! target/"], f"{t['verdict']} {t['status_entries']}")
    check("pinned executable preserved, digest and size unchanged", os.path.exists(PIN["executable"]) and
          sha256(PIN["executable"]) == PIN["sha256"] and os.path.getsize(PIN["executable"]) == PIN["size"],
          PIN["sha256"])
    deps = os.path.join(WT, "target/release/deps")
    exes = sorted(n for n in os.listdir(deps) if re.fullmatch(r"nexus_governed_control-[0-9a-f]{16}", n))
    check("still exactly one library-test executable in the mission target", exes == [os.path.basename(PIN["executable"])],
          exes)
    # Every invocation of the executable used the pin.
    for root, _, files in os.walk(os.path.join(EV, "receipts")):
        if "descriptor.json" in files and "executable" in json.load(open(os.path.join(root, "descriptor.json"))):
            d = json.load(open(os.path.join(root, "descriptor.json")))
            e = d["executable"]
            if isinstance(e, dict) and "sha256_before" in e:
                check(f"{os.path.relpath(root, EV)}: executable before/after equal the pin",
                      e["sha256_before"] == e.get("sha256_after") == PIN["sha256"] and e["path"] == PIN["executable"])
    # Classifications, in order.
    times = []
    for d in ORDER:
        cls = open(os.path.join(EV, d, "CLASSIFICATION.txt")).read().splitlines()
        rec = open(os.path.join(EV, d, "RECEIPT.txt")).read()
        launched = re.search(r"^launched: (\S+)$", rec, re.M).group(1)
        finished = re.search(r"^finished: (\S+)$", rec, re.M).group(1)
        times.append((launched, finished))
        check(f"{d}: CLASSIFICATION: PASS", cls[0] == "CLASSIFICATION: PASS", cls[1] if len(cls) > 1 else "")
    check("one at a time, in the mission's order: each launched after its predecessor finished",
          all(times[i][0] > times[i - 1][1] for i in range(1, len(times))), times)
    a4 = json.load(open(os.path.join(EV, "receipts/50-A4/A4.json")))
    check("A4 comparison satisfied", a4["satisfied"] is True)
    want = {"receipts/42-full-F-B": (3775373, 156463, 22182849), "receipts/43-full-F-3": (20431851, 880778, 119948772),
            "receipts/44-full-F-PT": (29751473, 1698028, 173414754)}
    for d, (k, kf, der) in want.items():
        r = json.load(open(os.path.join(EV, d, "PARSED-RECEIPT.json")))["report"]
        check(f"{d}: K {k:,}, Kf {kf:,}, derived {der:,}, 0 unexpanded",
              (r["states"], r["self_mirrored"], r["derived_original_states"], r["states"] - r["expanded"]) == (k, kf, der, 0))
    for d, n in (("receipts/41-full-F-2", 12602670), ("receipts/45-full-F-1", 46524336)):
        r = json.load(open(os.path.join(EV, d, "PARSED-RECEIPT.json")))["report"]
        check(f"{d}: original states {n:,}, expanded equal", (r["states"], r["expanded"]) == (n, n))
    # No secret-like text in any record to be published.
    hits = []
    for sub in ("receipts", "tools", "development"):
        for root, dirs, files in os.walk(os.path.join(EV, sub)):
            dirs[:] = [x for x in dirs if x != "__pycache__"]
            for f in files:
                for m in SECRET.finditer(open(os.path.join(root, f), errors="replace").read()):
                    hits.append((os.path.relpath(os.path.join(root, f), EV), m.group(0)[:12] + "…"))
    check("no secret-like text in receipts, tools or development records", not hits, hits[:5])
    print("GATE RESULT: " + ("ALL CHECKS HOLD" if not fails else "FAILED: " + "; ".join(fails)))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main())
