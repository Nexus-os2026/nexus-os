#!/usr/bin/python3 -I
"""SB-3E A3 (design section 5 row A3; SB3E-R28.1, R33.3): seam markers in real binaries.

Every seam, binary and disposition comes from `tools/gatebox/route/route_seams.json`
in the repository given; nothing here names a seam, so adding one is a data
change. Subcommands:

  check-data --repo R --toolchain T
      The data against the source: every marker begins with the stem and none
      is a substring of another (so matching is unambiguous); each marker is
      its seam's `const M: &str` line in the named source, exactly once; every
      stem token in route source is a listed marker; every seam names every
      binary as `reachable` or `unwired`; the binaries are exactly the route
      packages' bin targets (cargo metadata); exactly one binary's `shipped`
      names an existing file (the installing script), every other is null.
  scan --repo R --binary NAME=PATH ...
      The marker scan of real binaries. Matching is delimiter-safe: at every
      occurrence of the stem, at most one marker can begin (they are
      substring-free), and only a whole marker beginning there counts; a stem
      that begins no marker is reported as unknown, never as a near seam.
  build --repo R --toolchain T --target DIR [--wrapper W] [--only NAME]
      Each binary built from clean by its own plain recipe (`cargo build
      --release --locked -p PACKAGE --bin NAME`, the shipped release profile:
      strip=symbols, ThinLTO) in its own fresh target DIR/NAME (DIR must not
      exist), then scanned. Without
      --wrapper (the shipped configuration) any marker or unknown stem fails.
      With --wrapper (the route lane's wrapper, which sets the cfg on the
      route units) it is the census: the markers present must be exactly the
      binary's `reachable` seams.

Exit 0 on pass, 1 on a finding; one JSON verdict on stdout.
"""
import argparse
import json
import os
import re
import subprocess
import sys

DATA = "tools/gatebox/route/route_seams.json"
DISPOSITIONS = ("reachable", "unwired")


def load(repo):
    with open(os.path.join(repo, DATA), encoding="utf-8") as handle:
        return json.load(handle)


def env_block(toolchain, extra=None):
    bin_dir = os.path.join(toolchain, "bin")
    block = {"HOME": os.environ["HOME"], "PATH": bin_dir + ":/usr/bin:/bin", "LANG": "C.UTF-8",
             "RUSTC": os.path.join(bin_dir, "rustc")}
    for name in ("CARGO_HOME", "RUSTUP_HOME"):
        if name in os.environ:
            block[name] = os.environ[name]
    block.update(extra or {})
    return block


def bin_targets(repo, toolchain, packages):
    run = subprocess.run([os.path.join(toolchain, "bin", "cargo"), "metadata", "--no-deps", "--locked", "--offline",
                          "--format-version", "1"], cwd=repo, env=env_block(toolchain), capture_output=True, text=True)
    if run.returncode != 0:
        return None
    return {(t["name"], p["name"]) for p in json.loads(run.stdout)["packages"] if p["name"] in packages
            for t in p["targets"] if "bin" in t["kind"]}


def check_data(repo, toolchain):
    data, out = load(repo), []
    stem, seams, binaries = data["stem"], data["seams"], data["binaries"]
    names = [s["name"] for s in seams]
    if len(set(names)) != len(names):
        out.append({"kind": "data", "reason": "duplicate seam name"})
    markers = {s["name"]: s["marker"] for s in seams}
    for name, marker in markers.items():
        if not marker.startswith(stem) or not re.fullmatch(re.escape(stem) + r"[A-Za-z0-9-]+", marker):
            out.append({"kind": "data", "seam": name, "reason": "marker is not the stem and a name"})
        for other, text in markers.items():
            if other != name and marker in text:
                out.append({"kind": "data", "seam": name, "reason": f"marker is a substring of {other}'s"})
    for seam in seams:
        if sorted(seam["binaries"]) != sorted(binaries) or any(v not in DISPOSITIONS for v in seam["binaries"].values()):
            out.append({"kind": "data", "seam": seam["name"], "reason": "not every binary named reachable or unwired"})
        try:
            with open(os.path.join(repo, seam["source"]), encoding="utf-8") as handle:
                lines = [x.strip() for x in handle]
        except OSError:
            lines = []
        if lines.count(f'const M: &str = "{seam["marker"]}";') != 1:
            out.append({"kind": "source", "seam": seam["name"], "reason": "marker not defined once in its source"})
    token = re.compile(re.escape(stem) + r"[A-Za-z0-9_-]*")
    for root in data["route_sources"]:
        for directory, dirs, files in os.walk(os.path.join(repo, root)):
            dirs.sort()
            for name in sorted(f for f in files if f.endswith(".rs")):
                path = os.path.join(directory, name)
                with open(path, encoding="utf-8") as handle:
                    found = set(token.findall(handle.read()))
                for unlisted in sorted(found - set(markers.values())):
                    out.append({"kind": "source", "path": os.path.relpath(path, repo), "token": unlisted,
                                "reason": "a stem token that is no listed marker"})
    shipped = [b["shipped"] for b in binaries.values() if b["shipped"] is not None]
    if len(shipped) != 1 or not os.path.isfile(os.path.join(repo, shipped[0])):
        out.append({"kind": "data", "reason": "not exactly one binary shipped by an existing script"})
    have = bin_targets(repo, toolchain, {b["package"] for b in binaries.values()} | set(data["route_packages"]))
    want = {(name, b["package"]) for name, b in binaries.items()}
    if have is None:
        out.append({"kind": "metadata_failed"})
    elif have != want:
        out.append({"kind": "binaries", "reason": "not the route packages' bin targets",
                    "unlisted": sorted(map(list, have - want)), "missing": sorted(map(list, want - have))})
    return out


def find_markers(blob, stem, seams):
    """Delimiter-safe: count each whole marker beginning at an occurrence of the stem."""
    present, unknown, at = {}, [], blob.find(stem)
    while at >= 0:
        begun = [s["name"] for s in seams if blob.startswith(s["marker"].encode(), at)]
        if len(begun) == 1:
            present[begun[0]] = present.get(begun[0], 0) + 1
        else:
            tail = blob[at + len(stem):at + len(stem) + 24]
            unknown.append({"offset": at, "after_stem": "".join(chr(b) if 32 < b < 127 else "." for b in tail),
                            "begins": begun})
        at = blob.find(stem, at + 1)
    return present, unknown


def scan(data, name, path):
    with open(path, "rb") as handle:
        blob = handle.read()
    seams = data["seams"]
    present, unknown = find_markers(blob, data["stem"].encode(), seams)
    rows = [{"seam": s["name"], "disposition": s["binaries"][name], "present": present.get(s["name"], 0)}
            for s in seams]
    return {"binary": name, "elf": blob[:4] == b"\x7fELF", "size": len(blob), "seams": rows, "unknown": unknown}


def absence_findings(result):
    out = [] if result["elf"] else [{"kind": "not_elf", "binary": result["binary"]}]
    out += [{"kind": "marker_present", "binary": result["binary"], "seam": r["seam"], "disposition": r["disposition"]}
            for r in result["seams"] if r["present"]]
    out += [dict(u, kind="unknown_marker", binary=result["binary"]) for u in result["unknown"]]
    return out


def census_findings(result):
    out = [] if result["elf"] else [{"kind": "not_elf", "binary": result["binary"]}]
    for r in result["seams"]:
        if bool(r["present"]) != (r["disposition"] == "reachable"):
            out.append({"kind": "census_differs", "binary": result["binary"], "seam": r["seam"],
                        "disposition": r["disposition"], "present": r["present"]})
    out += [dict(u, kind="unknown_marker", binary=result["binary"]) for u in result["unknown"]]
    return out


def build(repo, toolchain, target, wrapper, only):
    data = load(repo)
    if os.path.lexists(target):
        return [{"kind": "target_not_fresh", "target": target}], []
    os.makedirs(target)
    findings, results = [], []
    for name, spec in sorted(data["binaries"].items()):
        if only and name not in only:
            continue
        own = os.path.join(target, name)
        extra = {"CARGO_TARGET_DIR": own}
        if wrapper:
            extra["RUSTC_WORKSPACE_WRAPPER"] = wrapper
        log = f"{own}.a3.log"
        with open(log, "w", encoding="utf-8") as handle:
            run = subprocess.run([os.path.join(toolchain, "bin", "cargo"), "build", "--release", "--locked", "-v",
                                  "--color", "never", "-p", spec["package"], "--bin", name], cwd=repo,
                                 env=env_block(toolchain, extra), stdout=handle, stderr=subprocess.STDOUT)
        if run.returncode != 0:
            findings.append({"kind": "build_failed", "binary": name, "log": log})
            continue
        result = scan(data, name, os.path.join(own, "release", name))
        results.append(dict(result, log=log))
        findings += census_findings(result) if wrapper else absence_findings(result)
    return findings, results


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("check-data", "scan", "build"))
    parser.add_argument("--repo", required=True)
    parser.add_argument("--toolchain")
    parser.add_argument("--binary", action="append", default=[], help="NAME=PATH (scan)")
    parser.add_argument("--target")
    parser.add_argument("--wrapper")
    parser.add_argument("--only", action="append")
    args = parser.parse_args()
    repo = os.path.abspath(args.repo)
    results = []
    if args.command == "check-data":
        findings = check_data(repo, os.path.abspath(args.toolchain))
    elif args.command == "scan":
        data, findings = load(repo), []
        for item in args.binary:
            name, path = item.split("=", 1)
            results.append(scan(data, name, path))
            findings += absence_findings(results[-1])
    else:
        wrapper = os.path.abspath(args.wrapper) if args.wrapper else None
        findings, results = build(repo, os.path.abspath(args.toolchain), os.path.abspath(args.target), wrapper,
                                  args.only)
    print(json.dumps({"verdict": "fail" if findings else "pass", "findings": findings, "results": results},
                     sort_keys=True))
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
