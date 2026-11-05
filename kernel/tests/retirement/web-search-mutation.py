#!/usr/bin/env python3
"""Run the registered K11 F3 behavioral mutation control in a disposable copy.

This stages only the prescribed same-type PlannedAction import, not K11's
production deletion. No source worktree file is modified. Uses installed Cargo
and cached dependencies through the caller's cargo-slot wrapper; no installs.
"""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[3]
WEB = Path("kernel/src/actuators/web.rs")
GUARD = Path("kernel/tests/kernel_retirement_web_search.rs")
TEST = "f3_injected_backend_failure_preserves_the_original_error"
OLD = "self.backend.search(query).map_err(ActuatorError::IoError)?"
MUTATIONS = {
    "swallowed-error": "self.backend.search(query).unwrap_or_default()",
    "false-closed": (
        "self.backend.search(query)"
        '.map_err(|_| ActuatorError::Closed("web search is closed".into()))?'
    ),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo-slot", type=Path, required=True)
    args = parser.parse_args()
    for name in ("CARGO_TARGET_DIR", "TMPDIR"):
        assert os.environ.get(name), f"{name} must be set explicitly"
    slot = args.cargo_slot.resolve(strict=True)
    files = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).split(b"\0")
    originals = {path: (ROOT / path).read_bytes() for path in (WEB, GUARD)}
    snapshot = (ROOT / "kernel/tests/retirement/web-search-guard.rs.txt").read_bytes()
    assert originals[GUARD] == snapshot, "live guard must match frozen K01 source"

    with tempfile.TemporaryDirectory(prefix="k01-fix3-k11-") as directory:
        scratch = Path(directory)
        for entry in files:
            if not entry:
                continue
            rel = Path(os.fsdecode(entry))
            assert not rel.is_absolute() and ".." not in rel.parts
            source = ROOT / rel
            assert not source.is_symlink(), f"unexpected source symlink: {rel}"
            target = scratch / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
        # K03's registered same-type home, reused at K11. The independent guard
        # imports action_types, never the executor; other runtime stays intact.
        lib = scratch / "kernel/src/lib.rs"
        lib.write_bytes(lib.read_bytes() + b"\npub use cognitive::types as action_types;\n")
        staged_guard = snapshot.replace(
            b"nexus_kernel::cognitive::types::PlannedAction",
            b"nexus_kernel::action_types::PlannedAction",
        )
        assert staged_guard != snapshot
        (scratch / GUARD).write_bytes(staged_guard)
        assert b"use nexus_kernel::cognitive" not in staged_guard
        web = scratch / WEB
        original = web.read_bytes()
        assert original == originals[WEB]
        assert original.count(OLD.encode()) == 1

        def run(label, red=False):
            command = [
                str(slot), "cargo", "test", "-p", "nexus-kernel", "--locked",
                "--test", "kernel_retirement_web_search",
            ]
            if red:
                command += [TEST, "--", "--exact", "--nocapture"]
            result = subprocess.run(
                command, cwd=scratch, stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT, text=True, check=False,
            )
            print(f"\nK11 {label}: exit {result.returncode}", flush=True)
            print(result.stdout, flush=True)
            if red:
                assert result.returncode != 0, f"{label}: mutation survived"
                assert f"test {TEST} ... FAILED" in result.stdout
                assert "1 failed" in result.stdout
            else:
                assert result.returncode == 0, f"{label}: baseline failed"
                assert "4 passed; 0 failed" in result.stdout

        run("clean staged guard")
        try:
            for label, replacement in MUTATIONS.items():
                web.write_bytes(original.replace(OLD.encode(), replacement.encode()))
                assert web.read_bytes() != original
                run(label, red=True)
        finally:
            web.write_bytes(original)
            assert web.read_bytes() == original, "scratch source must restore byte-for-byte"
        run("byte-restored staged guard")
        assert web.read_bytes() == original
    for path, original in originals.items():
        assert (ROOT / path).read_bytes() == original, f"worktree changed: {path}"
    print("PASS: both K11 mutations rejected; scratch restored byte-for-byte; worktree untouched")


if __name__ == "__main__":
    main()
