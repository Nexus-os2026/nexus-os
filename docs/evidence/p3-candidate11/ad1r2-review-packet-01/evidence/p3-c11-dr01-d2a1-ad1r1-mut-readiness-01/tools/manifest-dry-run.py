#!/usr/bin/env python3
"""Read-only dry run of the current-tree manifest reconciliation (AD-1-R1 H) at one commit.
Reads docs/evidence/p3-candidate10/original-183/manifest.json and the source files at COMMIT through `git show`.
Writes nothing to the repository. For every (control, file) pair whose recorded source_sha256 differs from the
file at COMMIT: the recorded hash, the current sha256 and blob, and, for every edit of that control, the anchor's
occurrence count in the file at COMMIT against the count the manifest requires, and whether the replacement is
already present. Also lists every control whose anchors no longer match, stale or not."""
import hashlib, json, subprocess, sys
REPO = "/home/nexus/NEXUS/nexus-os"
COMMIT = sys.argv[1]
MAN = "docs/evidence/p3-candidate10/original-183/manifest.json"
def show(path):
    r = subprocess.run(["git", "show", f"{COMMIT}:{path}"], cwd=REPO, capture_output=True)
    return r.stdout if r.returncode == 0 else None
def blob(path):
    return subprocess.run(["git", "rev-parse", f"{COMMIT}:{path}"], cwd=REPO, capture_output=True, text=True).stdout.strip()
raw = show(MAN)
m = json.loads(raw)
print(f"DRY RUN at {COMMIT} (tree {subprocess.run(['git','rev-parse',COMMIT+'^{tree}'],cwd=REPO,capture_output=True,text=True).stdout.strip()})")
print(f"historical manifest {MAN}: sha256 {hashlib.sha256(raw).hexdigest()}, {len(m['controls'])} controls, version {m['version']!r}")
files = {}
stale = []
for c in m["controls"]:
    for f, rec in c.get("source_sha256", {}).items():
        if f not in files:
            data = show(f)
            files[f] = (hashlib.sha256(data).hexdigest() if data is not None else None, data)
        cur, _ = files[f]
        if cur != rec:
            stale.append((c["id"], f, rec, cur))
print(f"recorded (control, file) hashes: {sum(len(c.get('source_sha256', {})) for c in m['controls'])} over {len(files)} files; stale {len(stale)}")
by_id = {c["id"]: c for c in m["controls"]}
anchor_problems = []
for c in m["controls"]:
    for e in c["edits"]:
        data = files.get(e["file"], (None, show(e["file"])))[1]
        text = data.decode() if data is not None else ""
        n = text.count(e["anchor"])
        if n != e["count"]:
            anchor_problems.append((c["id"], e["file"], n, e["count"]))
print()
print("STALE PAIRS, with every edit of the control checked against the file at this commit:")
for cid, f, rec, cur in stale:
    c = by_id[cid]
    print(f"  {cid} {f}")
    print(f"     recorded {rec}")
    print(f"     current  {cur} (blob {blob(f)})")
    for k, e in enumerate(c["edits"]):
        text = files.get(e["file"], (None, show(e["file"])))[1].decode()
        n = text.count(e["anchor"])
        lines = [text.count("\n", 0, at) + 1 for at in range(len(text)) if text.startswith(e["anchor"], at)][:5]
        if e["replacement"] == "":
            rep = "replacement empty (a deletion)"
        else:
            rep = f"replacement text already occurs {text.count(e['replacement'])} time(s)"
        print(f"     edit {k+1} on {e['file']}: anchor occurs {n} (manifest count {e['count']}) "
              f"{'OK' if n == e['count'] else 'MISMATCH'} at line(s) {lines}; {rep}")
    print(f"     test command: {' '.join(c['command'])}; expected failing tests {c.get('expected_failure_tests')}")
print()
print(f"controls with any anchor count different from the manifest at this commit: {len(anchor_problems)}")
for a in anchor_problems:
    print(f"  {a[0]} {a[1]}: occurs {a[2]}, manifest count {a[3]}")
