#!/usr/bin/env python3
"""P2-V1-R3B-I4 publication preflight and ref readback.

Adapted from docs/evidence/p2-v1-r3b-i3-i1-r3/scripts/preflight.py (which
stays unchanged): the I4 push target, the accepted custody-store branch
among the protected refs, and a phase argument. Before the push the target
branch must not exist on the remote (and locally it must be the
candidate); after it, both must be the candidate.

Reads, and changes nothing:
- every protected ref and the candidate branch, locally and on the remote
  (`git ls-remote`), against the expected SHA;
- the push triggers of every workflow under .github/workflows, and whether
  any could match the push target;
- the active local hooks (core.hooksPath and the worktree's git directory);
- the commit identity the repository configures.

Usage: preflight.py <repository> <remote> <candidate SHA> <phase: start|pre-push|post-push> <out file>
"""
import datetime
import fnmatch
import pathlib
import subprocess
import sys

TARGET = "review/p2-v1-native-scope-ownership"
PROTECTED = [
    ("main", "4b36f60694148b60029733bc7b3d26e5a215d460"),
    ("repair/p2-validation-workflow-bootstrap", "4b36f60694148b60029733bc7b3d26e5a215d460"),
    ("implement/phase2-governed-verification", "4d6763a8afe998bb5a54f0b9ecfd420cd4abfa8f"),
    ("rebuild/phase0-trust-boundary", "f727f5c39fab8d5c729a55eb28ad576d3d56ce47"),
    ("evidence/phase0-closure", "e33cf1ff1b8de0d0c6c8751e24d85ed98b3cf9b1"),
    ("rebuild/phase1-governed-coding", "14270a9a38770ac84456c1f812042d2967edec42"),
    ("evidence/phase1-closure", "c237937189b5977eaad01acad6a4c52bcce796cc"),
    ("review/p2-v1-cleanup-observers", "45898e05178a56efaadeb1f8f9ee7a0a521c72c9"),
    ("evidence/p2-v1-r3b-i2-r1", "8e2c46c00cd4071bbae41815d1d1cd0c4814655f"),
    ("review/p2-v1-custody-recorder-design", "7689e599ed8fc89ec7720d13869ef58cab767ecb"),
    ("review/p2-v1-custody-store-implementation", "3805204f941d9694b2c0c36549d32a5f4e6c157c"),
]


def git(root, *args, check=True):
    out = subprocess.run(["git", *args], cwd=root, capture_output=True, text=True)
    if check and out.returncode != 0:
        raise SystemExit(f"git {' '.join(args)}: {out.stderr.strip()}")
    return out.stdout.strip()


def push_trigger(text):
    """The lines of the `push:` block under `on:` (a plain reading of the
    YAML layout these workflows use), or None when there is none."""
    lines = text.splitlines()
    in_on, on_indent = False, 0
    for i, line in enumerate(lines):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(line) - len(line.lstrip())
        if not in_on:
            if stripped in ("on:", '"on":', "'on':"):
                in_on, on_indent = True, indent
            continue
        if indent <= on_indent:
            return None
        if stripped == "push:" or stripped.startswith("push:"):
            if stripped != "push:":
                return [stripped[len("push:"):].strip()]
            block, push_indent = [], indent
            for follow in lines[i + 1:]:
                if not follow.strip() or follow.strip().startswith("#"):
                    continue
                if len(follow) - len(follow.lstrip()) <= push_indent:
                    break
                block.append(follow.strip())
            return block
    return None


def main():
    if len(sys.argv) != 6 or sys.argv[4] not in ("start", "pre-push", "post-push"):
        raise SystemExit(__doc__)
    root, remote, candidate, phase = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
    out = pathlib.Path(sys.argv[5])
    now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    rows = PROTECTED + [(TARGET, candidate)]
    remote_refs = {}
    for line in git(root, "ls-remote", remote, *[f"refs/heads/{name}" for name, _ in rows]).splitlines():
        sha, ref = line.split("\t", 1)
        remote_refs[ref[len("refs/heads/"):]] = sha
    lines = [f"P2-V1-R3B-I4 publication preflight and ref readback ({phase}), {now}",
             f"remote {remote}: {git(root, 'remote', 'get-url', remote)}", "",
             f"{'ref':45} {'expected':41} {'local':41} {'remote':41} result"]
    ok = True
    for name, expected in rows:
        local = git(root, "rev-parse", "--verify", "--quiet", f"refs/heads/{name}", check=False) or "(absent)"
        far = remote_refs.get(name, "(absent)")
        if name == TARGET and phase == "start":
            # The branch exists locally at the base; nothing is published.
            good = far == "(absent)"
        elif name == TARGET and phase == "pre-push":
            good = local == expected and far == "(absent)"
        else:
            good = local == expected and far == expected
        ok &= good
        lines.append(f"{name:45} {expected:41} {local:41} {far:41} {'OK' if good else 'MISMATCH'}")
    lines += ["", f"worktree HEAD: {git(root, 'rev-parse', 'HEAD')} on {git(root, 'branch', '--show-current')}", "",
              f"Push target: refs/heads/{TARGET} on {remote}", "Workflow push triggers:"]
    matches = []
    for path in sorted(pathlib.Path(root, ".github", "workflows").glob("*.y*ml")):
        trigger = push_trigger(path.read_text())
        rel = path.relative_to(root)
        lines.append(f"  {rel}: push trigger: {' '.join(trigger) if trigger else 'none'}")
        if trigger is None:
            continue
        branches = []
        mode = None
        for item in trigger:
            if item.endswith(":"):
                mode = item[:-1]
                continue
            if item.startswith("- ") and mode == "branches":
                branches.append(item[2:].strip().strip('"').strip("'"))
        if "branches:" not in trigger and "tags:" not in trigger:
            matches.append(f"{rel} (push without a branch filter)")
        elif any(fnmatch.fnmatch(TARGET, pattern.replace("**", "*")) for pattern in branches):
            matches.append(str(rel))
    lines.append(f"Workflows whose push trigger could match {TARGET}: {matches or 'none'}")
    ok &= not matches
    hooks_path = git(root, "config", "--get", "core.hooksPath", check=False)
    git_hooks = pathlib.Path(git(root, "rev-parse", "--git-path", "hooks"))
    if not git_hooks.is_absolute():
        git_hooks = pathlib.Path(root, git_hooks)

    def active(directory):
        if not directory or not pathlib.Path(directory).is_dir():
            return []
        return sorted(p.name for p in pathlib.Path(directory).iterdir()
                      if p.is_file() and not p.name.endswith(".sample"))

    def listed(directory):
        names = active(directory)
        return f"{len(names)}" + (f" {names}" if names else "")

    lines += ["", f"Local hooks: core.hooksPath={hooks_path or '(unset)'}",
              f"  active (non-sample) hooks there: {listed(hooks_path)}",
              f"  worktree git dir hooks ({git_hooks}): {listed(git_hooks)}", "",
              f"Commit identity (repository config): {git(root, 'config', 'user.name')} <{git(root, 'config', 'user.email')}>",
              "", f"RESULT: {'PASS' if ok else 'FAIL'}"]
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
