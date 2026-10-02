#!/usr/bin/env python3
"""Check that scripts/scripts.adaptation.diff, as committed (normalized),
turns R2's scripts into this directory's R3 copies byte for byte.

R2's scripts are copied into a temporary directory. GNU `patch -p1` applies
the diff there (its file labels are `a/<name>` and `b/<name>`). Each result
is then compared with the R3 script of the same name. compare_i2r1.py is
R2's, unchanged: the diff has no hunk for it, and the copy must equal it.

Usage: adaptation_check.py <repository> <out file>
"""
import pathlib
import shutil
import subprocess
import sys
import tempfile

R2 = "docs/evidence/p2-v1-r3b-i3-i1-r2/scripts"
R3 = "docs/evidence/p2-v1-r3b-i3-i1-r3/scripts"
NAMES = ["api_probes.py", "candidate_runs.sh", "checks.sh", "control_mapping.py", "matrices.py",
         "normalize.py", "preflight.py", "scope.py", "store_controls.py", "validate.sh", "compare_i2r1.py"]


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    root, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    lines = []
    with tempfile.TemporaryDirectory() as tmp:
        work = pathlib.Path(tmp)
        for name in NAMES:
            shutil.copy(root / R2 / name, work / name)
        run = subprocess.run(["patch", "-p1", "--no-backup-if-mismatch", "-d", tmp, "-i",
                              str((root / R3 / "scripts.adaptation.diff").resolve())],
                             capture_output=True, text=True)
        lines.append(f"$ patch -p1 -d <temporary copy of {R2}> -i {R3}/scripts.adaptation.diff")
        lines.append(f"exit {run.returncode}")
        lines += [line for line in run.stdout.splitlines() if line.strip()]
        ok = run.returncode == 0
        for name in NAMES:
            same = (work / name).read_bytes() == (root / R3 / name).read_bytes()
            ok &= same
            lines.append(f"{'same' if same else 'DIFFERENT'} {R3}/{name}")
    lines.append(f"RESULT: {'PASS' if ok else 'FAIL'}")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
