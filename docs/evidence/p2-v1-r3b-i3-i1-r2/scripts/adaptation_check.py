#!/usr/bin/env python3
"""Check that scripts/scripts.adaptation.diff, as committed (normalized),
turns R1's scripts into this directory's R2 copies byte for byte.

R1's scripts are copied into a temporary directory under the R1 evidence
path. GNU `patch -p0` applies the diff there: it patches the existing,
R1-named copies. Each result is then compared with the R2 script of the
same name.

Usage: adaptation_check.py <repository> <out file>
"""
import pathlib
import shutil
import subprocess
import sys
import tempfile

R1 = "docs/evidence/p2-v1-r3b-i3-i1-r1/scripts"
R2 = "docs/evidence/p2-v1-r3b-i3-i1-r2/scripts"
NAMES = ["store_controls.py", "api_probes.py", "validate.sh", "candidate_runs.sh", "checks.sh",
         "scope.py", "control_mapping.py", "matrices.py", "normalize.py", "compare_i2r1.py"]


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    root, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    lines = []
    with tempfile.TemporaryDirectory() as tmp:
        work = pathlib.Path(tmp) / "p2-v1-r3b-i3-i1-r1" / "scripts"
        work.mkdir(parents=True)
        for name in NAMES:
            shutil.copy(root / R1 / name, work / name)
        run = subprocess.run(["patch", "-p0", "--no-backup-if-mismatch", "-d", tmp, "-i",
                              str((root / R2 / "scripts.adaptation.diff").resolve())],
                             capture_output=True, text=True)
        lines.append(f"$ patch -p0 -d <temporary> -i {R2}/scripts.adaptation.diff")
        lines.append(f"exit {run.returncode}")
        lines += [line for line in run.stdout.splitlines() if line.strip()]
        ok = run.returncode == 0
        for name in NAMES:
            same = (work / name).read_bytes() == (root / R2 / name).read_bytes()
            ok &= same
            lines.append(f"{'same' if same else 'DIFFERENT'} {R2}/{name}")
    lines.append(f"RESULT: {'PASS' if ok else 'FAIL'}")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
