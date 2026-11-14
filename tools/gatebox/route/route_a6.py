#!/usr/bin/python3 -I
"""SB-3E A6 (design section 5 row A6, T1; mission U4b item 6): blank-source identity.

  run --repo R --toolchain T --work DIR
      In a scratch copy of the tracked files, `bash mother/desktop/install.sh`
      (the shipped recipe as A4 runs it: a fresh HOME and CARGO_TARGET_DIR
      under DIR and the pinned toolchain) installs the binary: sha256 X. Then
      every attribute naming the route cfg in the route sources
      (route_seams.json `route_sources`) is blanked with the item, statement,
      field or module declaration it governs (route_a5's parser), the outer
      attributes stacked directly above it, and the comments (doc comments
      too) between it and the previous token, so no doc comment or attribute
      is left documenting nothing: every
      character but a newline becomes a space, in place, so each line and
      column is kept. The private target, its log and the installed file are
      removed, and the same command, at the same path with the same HOME and
      CARGO_TARGET_DIR, installs again: sha256 Y. Pass iff both install, the
      blanked sources keep their line counts and name the cfg in no
      attribute, and X == Y.
  control --repo R --toolchain T --work DIR
      The same on a copy whose S1 gates (route_seams.json `ungates.S1`) are
      broadened to `cfg(any(target_os = "linux", mother_route_seams))`, so the
      seam is compiled into the shipped binary: blanking removes it, and X
      must differ from Y. Pass iff X != Y.

Exit 0 on pass; one JSON verdict on stdout.
"""
import argparse
import json
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import route_a4  # noqa: E402  (scratch_copy, env_block, shipped, sha256_file)
import route_a5  # noqa: E402  (the A5 parser)

BROADENED = '#[cfg(any(target_os = "linux", mother_route_seams))]'


def blank(tree, roots):
    """Blank each cfg-naming attribute with its governed span; {file: attributes blanked}."""
    blanked = {}
    for rel, text in route_a5.read_sources(tree, roots).items():
        src = route_a5.Source(rel, text)
        spans, ends = [], {a[2]: a for a in src.attrs if not a[0].startswith("#!")}
        for norm, first, end, start, stop in src.attrs:
            if route_a5.CFG in norm:
                while first in ends:  # the outer attributes stacked directly above it
                    first = ends[first][1]
                last = src.toks[max(end, stop) - 1]
                # back to the previous token: its doc comments (and any comment) go too
                gap = src.toks[first - 1][2] + len(src.toks[first - 1][0]) if first else 0
                spans.append((gap, last[2] + len(last[0])))
        if spans:
            chars = list(text)
            for a, z in spans:
                chars[a:z] = [c if c == "\n" else " " for c in chars[a:z]]
            with open(os.path.join(tree, rel), "w", encoding="utf-8") as handle:
                handle.write("".join(chars))
            blanked[rel] = len(spans)
    return blanked


def residue(tree, roots, before):
    """Files whose line count changed, that still name the cfg in an attribute, or that hold a form
    outside A5's closed subset (SB3E-R55.3: what A5 cannot see is not blanked, so it fails here)."""
    out = []
    for rel, text in route_a5.read_sources(tree, roots).items():
        src = route_a5.Source(rel, text)
        named = [a[0] for a in src.attrs if route_a5.CFG in a[0]]
        if named or src.refused or text.count("\n") != before[rel].count("\n"):
            out.append({"file": rel, "attributes": named, "refused": src.refused})
    return out


def install(tree, toolchain, home, parent):
    """The shipped recipe once; the installed sha256, or a failure."""
    binary, script = route_a4.shipped(tree)
    env = route_a4.env_block(toolchain, {"HOME": home, "CARGO_TARGET_DIR": parent})
    run = subprocess.run(["bash", os.path.join(tree, script)], cwd=os.path.dirname(parent), env=env,
                         stdin=subprocess.DEVNULL, capture_output=True, text=True)
    printed = {k.strip(): v.strip() for k, v in (x.split(":", 1) for x in run.stdout.splitlines() if ":" in x)}
    installed = os.path.join(home, ".local/bin", binary)
    out = {"exit": run.returncode, "printed_sha256": printed.get("sha256")}
    if run.returncode != 0 or printed.get("installed") != installed or not os.path.isfile(installed):
        out["stderr_tail"] = run.stderr.strip().splitlines()[-5:]
        return None, out
    out["sha256"] = route_a4.sha256_file(installed)
    return (out["sha256"] if out["sha256"] == printed.get("sha256") else None), out


def identity(repo, toolchain, work, broaden):
    os.makedirs(work)
    tree, tree_id = route_a4.scratch_copy(repo, work)
    seams = json.load(open(os.path.join(tree, route_a5.SEAMS), encoding="utf-8"))
    findings, out = [], {"tree": tree_id}
    if broaden:
        for edit in seams["ungates"]["S1"]:
            path = os.path.join(tree, edit["file"])
            text = open(path, encoding="utf-8").read()
            if text.count(edit["old"]) != 1 or route_a5.ROUTE not in edit["old"]:
                return [{"kind": "broadening_edit_not_unique", "file": edit["file"]}], out
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(text.replace(edit["old"], edit["old"].replace(route_a5.ROUTE, BROADENED, 1)))
        out["broadened"] = len(seams["ungates"]["S1"])
    home, parent = os.path.join(work, "home"), os.path.join(work, "parent")
    os.makedirs(home)
    x, out["shipped"] = install(tree, toolchain, home, parent)
    for path in (os.path.join(parent, "nexus-mother-install"), os.path.join(parent, "nexus-mother-install.log"),
                 os.path.join(home, ".local/bin", route_a4.shipped(tree)[0])):
        shutil.rmtree(path) if os.path.isdir(path) else os.remove(path) if os.path.lexists(path) else None
    before = route_a5.read_sources(tree, seams["route_sources"])
    out["blanked"] = blank(tree, seams["route_sources"])
    out["residue"] = residue(tree, seams["route_sources"], before)
    y, out["blank"] = install(tree, toolchain, home, parent)
    if x is None or y is None:
        findings.append({"kind": "install_failed", "shipped": x is not None, "blank": y is not None})
    if out["residue"] or not out["blanked"]:
        findings.append({"kind": "blanking_incomplete"})
    out["equal"] = x is not None and x == y
    shutil.rmtree(work, ignore_errors=True)
    return findings, out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("run", "control"))
    for name in ("--repo", "--toolchain", "--work"):
        parser.add_argument(name, required=True)
    args = parser.parse_args()
    repo, toolchain, work = map(os.path.abspath, (args.repo, args.toolchain, args.work))
    if os.path.lexists(work):
        sys.exit(f"route_a6: the work directory must be fresh: {work}")
    findings, out = identity(repo, toolchain, work, args.command == "control")
    if not findings and out["equal"] != (args.command == "run"):
        findings.append({"kind": "hash_differs" if args.command == "run" else "broadened_cfg_kept_the_hash"})
    print(json.dumps(dict(out, verdict="fail" if findings else "pass", findings=findings), sort_keys=True))
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
