#!/usr/bin/env python3
"""Per-stage gate for P3-C11-DR01-D2A1-C7-COMPLIANT-EXEC-02 (new in EXEC-02; modelled on EXEC-01's r3/cmd/00-gates.sh).

  stagegate.py <stage-name> <projected-bytes>

Before every stage: the frozen tools still hash to TOOLS.sha256; no conflicting process by EXECUTABLE IDENTITY (never pgrep -f);
no active campaign scope; MemAvailable >= 18 GiB; free disk minus the stage's projected growth >= the 16 GiB floor; the primary
checkout is unchanged; the destination ref is absent locally. Writes gates/<stage>.txt (exclusive) and appends one line to
SEQUENCE.log. Exit 0 = PASS. Nothing is stopped or killed.
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from c7lib import *  # noqa
import gate as G

HERE = os.path.dirname(os.path.abspath(__file__))


def main(stage, projected):
    out = os.path.join(EV_DIR, "gates", stage + ".txt")
    if os.path.exists(out):
        raise SystemExit("REFUSED: %s exists" % out)
    rec = Rec("STAGE GATE before %s" % stage)
    rec.log("# utc %s" % utcns())
    sums = os.path.join(os.path.dirname(HERE), "TOOLS.sha256")
    r = subprocess.run(["sha256sum", "-c", "--strict", "TOOLS.sha256"], cwd=os.path.dirname(HERE), capture_output=True, text=True)
    rec.check("T-1", "every frozen tool file hashes to tools/TOOLS.sha256 (sha256 %s)" % (sha256_file(sums)[:16] if os.path.exists(sums) else None),
              r.returncode == 0 and os.path.exists(sums), (r.stdout + r.stderr).strip().replace("\n", " | ")[:300])
    listed = sorted(l.split("  ", 1)[1] for l in open(sums).read().splitlines()) if os.path.exists(sums) else []
    present = sorted("c7tools/" + f for f in os.listdir(HERE) if f.endswith((".py", ".sh")))
    rec.check("T-2", "no unlisted tool file exists beside the frozen ones", present == [p for p in listed if p.startswith("c7tools/")], "present=%d listed=%d" % (len(present), len(listed)))
    rows = G.proc_scan()
    me = os.getpid()
    mine = set()
    p = me
    while p > 1:  # this gate's own ancestry (the stage script and its shell) is never a conflict
        mine.add(p)
        try:
            p = int(open("/proc/%d/stat" % p).read().rsplit(")", 1)[1].split()[1])
        except OSError:
            break
    conf = [r for r in rows if r[0] not in mine and ((r[1] and (G.CONFLICT_EXE.search(r[1]) or G.DEPS_EXE.search(r[1])))
                                                    or (not r[1] and r[2] and (G.CONFLICT_EXE.search(r[2][0]) or G.DEPS_EXE.search(r[2][0]))))]
    rec.check("P-1", "no cargo/rustc/clippy/test-executable/Actions-runner process by executable identity (%d processes scanned)" % len(rows), not conf,
              "; ".join("pid %d exe %s argv %s" % (r[0], r[1], r[2]) for r in conf)[:500])
    runsh = [r for r in rows if any(("actions-runner" in a or a.endswith("/run.sh") or a.endswith("/runsvc.sh")) for a in r[2])]
    rec.check("P-2", "no Actions runner script in any process argv", not runsh, str(runsh)[:300])
    units = subprocess.run(["systemctl", "--user", "list-units", "--type=scope", "--all", "--no-legend", "--plain"], capture_output=True, text=True).stdout.splitlines()
    active = [l for l in units if re.match(r"^(p3c11|c4w01|fbk|c7w)", l) and (" active " in l or " activating " in l)]
    rec.check("P-3", "no active campaign scope unit (p3c11/c4w01/fbk/c7w)", not active, "; ".join(active))
    mi = {l.split(":")[0]: int(l.split()[1]) for l in open("/proc/meminfo")}
    rec.check("R-1", "MemAvailable >= 18 GiB", mi["MemAvailable"] >= 18 * 1024 * 1024, "%d KiB" % mi["MemAvailable"])
    free = disk_free()
    rec.check("R-2", "free disk %d B minus projected growth %d B >= floor %d B" % (free, projected, DISK_FLOOR_BYTES), free - projected >= DISK_FLOOR_BYTES, "")
    head = git_out("rev-parse", "HEAD")
    st = git("status", "--porcelain=v1").stdout.decode().strip()
    rec.check("G-1", "primary checkout unchanged (f727f5c3, clean)", head == PROTECTED_REBUILD and st == "", "%s %r" % (head, st[:100]))
    rec.check("G-2", "destination ref absent locally", git("rev-parse", "--verify", "-q", DEST_REF, check=False).returncode != 0
              and git("rev-parse", "--verify", "-q", "refs/remotes/github/" + DEST_REF[len("refs/heads/"):], check=False).returncode != 0)
    rec.check("G-3", "mutant protocol.rs blob 0d4ab992 absent from the object store", git("cat-file", "-e", MUTANT_BLOB, check=False).returncode != 0)
    ok = not rec.fails
    rec.log("STAGE GATE %s: %s" % (stage, "PASS" if ok else "FAIL (%s)" % ", ".join(rec.fails)))
    os.makedirs(os.path.dirname(out), exist_ok=True)
    rec.write(out)
    with open(os.path.join(EV_DIR, "SEQUENCE.log"), "a") as f:
        f.write("%s GATE %s %s free=%d memavail_kib=%d\n" % (utcns(), stage, "PASS" if ok else "FAIL", free, mi["MemAvailable"]))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], int(sys.argv[2])))
