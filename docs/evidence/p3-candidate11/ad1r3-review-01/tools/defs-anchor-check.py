#!/usr/bin/env python3
"""Reads CAMPAIGN-01's sealed controls_defs.py as TEXT, parses it with the standard `ast` module (no import, no
execution, no evaluation of names), and for each named control reports its sealed definition and how often each anchor
and each replacement occurs in protocol.rs at the source commit (read from git objects). Applies nothing. Writes
nothing.
    defs-anchor-check.py <controls_defs.py> <repo> <source-commit> <control>..."""
import ast, hashlib, subprocess, sys

defs_path, repo, commit, *wanted = sys.argv[1:]
EXPECT_DEFS = "ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f"
raw = open(defs_path, "rb").read()
sha = hashlib.sha256(raw).hexdigest()
print(f"controls_defs.py {defs_path}\n  sha256 {sha} ({'= expected' if sha == EXPECT_DEFS else 'MISMATCH'})")
if sha != EXPECT_DEFS:
    sys.exit(2)
tree = ast.parse(raw.decode("utf-8"))
names = {}
controls = None
for node in tree.body:
    if isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name):
        tgt = node.targets[0].id
        if isinstance(node.value, ast.Constant) and isinstance(node.value.value, str):
            names[tgt] = node.value.value
        if tgt == "CONTROLS" and isinstance(node.value, ast.Dict):
            controls = node.value


def lit(n):
    """Literal value of a node; a Name is resolved only to a module-level string constant; '+' only on strings."""
    if isinstance(n, ast.Constant):
        return n.value
    if isinstance(n, ast.Name):
        return names[n.id]
    if isinstance(n, ast.BinOp) and isinstance(n.op, ast.Add):
        return lit(n.left) + lit(n.right)
    if isinstance(n, (ast.Tuple, ast.List)):
        return [lit(e) for e in n.elts]
    raise ValueError(f"not a literal: {ast.dump(n)[:80]}")


defs = {lit(k): lit(v) for k, v in zip(controls.keys, controls.values)}
print(f"  controls defined: {', '.join(defs)}")
CORE = names["CORE"]
src = subprocess.run(["git", "-C", repo, "show", f"{commit}:{CORE}"], check=True, capture_output=True).stdout
blob = subprocess.run(["git", "-C", repo, "rev-parse", f"{commit}:{CORE}"], check=True, capture_output=True,
                      text=True).stdout.strip()
text = src.decode("utf-8")
print(f"  {CORE} at {commit[:8]}: blob {blob}, sha256 {hashlib.sha256(src).hexdigest()}")
bad = 0
for c in wanted:
    if c not in defs:
        print(f"\n{c}: NO SEALED DEFINITION in controls_defs.py")
        continue
    meaning, path, edits, kind, target, props = defs[c]
    print(f"\n{c}: {meaning}\n  file {path}; kind {kind}; target {target}; properties {props}")
    for i, (anchor, repl) in enumerate(edits, 1):
        n_a, n_r = text.count(anchor), text.count(repl)
        start = text[: text.find(anchor)].count("\n") + 1 if n_a else 0
        end = start + anchor.count("\n")
        print(f"  edit {i}: anchor occurs {n_a} time(s) (lines {start}-{end}); replacement occurs {n_r} time(s)")
        print("    anchor sha256 " + hashlib.sha256(anchor.encode()).hexdigest() + "; replacement sha256 "
              + hashlib.sha256(repl.encode()).hexdigest())
        bad += n_a != 1 or n_r != 0
print(f"\nANCHOR CHECK: {'every anchor occurs exactly once and no replacement occurs' if not bad else f'{bad} edit(s) not unique or already present'}")
sys.exit(1 if bad else 0)
