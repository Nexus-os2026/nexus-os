#!/usr/bin/python3 -I
"""SB-3E A1: the route packages' build graph and the snapshot's Cargo configuration.

Design section 5 row A1 (U4b item 1, ruling SB3E-R28.2). Stdlib only, Linux
only. GB3 installs it in the Gate Box bundle as the route-cells gate
`route_a1`, run before A7. It prints a JSON verdict and exits nonzero on any
finding.

SB3E GB3 (R48.1 B1): the repository's lock, graph and declared pins are read
from the checked tree's own `tools/route_pins.json` (`--pins`, required, no
default). GB3-1: a bad pins file, an inherited toolchain variable or any
configuration finding ends A1 before any Cargo process.

Checks, across the default and the all-features resolve, every target table
and normal, build and dev edges:
- route_package: each route package (ROUTE_PACKAGES, by name and manifest)
  exists, has no build script, no `[features]` entry (an implicit feature of
  an optional dependency counts), no `links`, and declares exactly the
  reviewed dependencies (the pins file's `declared`, every field Cargo reports).
- graph_differs: the resolved graph reachable from the route packages
  (packages by name, version and normalized source; edges by extern name,
  kind and target; enabled features) equals its reviewed hash, per resolve.
- config: `.cargo/config` is absent everywhere, `.cargo/config.toml` exists
  only at the root and is byte-equal to its pin (Cargo prefers the legacy name
  when both exist); no `.cargo` is a symbolic link. What a refused file sets is
  reported by key.
- lock: Cargo.lock is byte-equal to the pins file's lock digest.
- pins: the pins file is exactly <repo>/tools/route_pins.json, not a symbolic
  link, a regular file within PINS_MAX bytes, with exactly the pins keys.
Ancestor and Cargo-home configuration is outside the snapshot and is the
runner's (SB3E-R13.2); A2 validates what an installation really ran.
"""
import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tomllib

ROUTE_PACKAGES = {
    "nexus-governed-control": "crates/nexus-governed-control/Cargo.toml",
    "nexus-mother": "mother/Cargo.toml",
    "nexus-mother-host": "mother/host/Cargo.toml",
    "nexus-mother-desktop": "mother/desktop/Cargo.toml",
}
CARGO_CONFIG_PIN = "379a150667985722540bd30f737cfb363dc5bc2dc8a5edae3db3725e3cd0b234"
# Reviewed graph hashes are sha256 of graph_lines(), LF-joined; declared dependencies are
# name|rename|kind|target|optional|default-features|features|req|source. Both are in the pins file.
PINS = os.path.join("tools", "route_pins.json")
PINS_MAX = 65536
HEX = re.compile("[0-9a-f]{64}")
REFUSED_EXACT = ("MAKEFLAGS", "MFLAGS")
REFUSED_PREFIX = ("RUSTUP_", "RUSTC", "RUSTDOC", "RUSTFLAGS", "CARGO_")


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def sha256_file(path):
    with open(path, "rb") as handle:
        return sha256_bytes(handle.read())


def f(kind, **detail):
    return {"kind": kind, **detail}


def launch_block(toolchain):
    out = [f("toolchain_refused", reason=f"inherited {name}") for name in sorted(os.environ)
           if name in REFUSED_EXACT or name.startswith(REFUSED_PREFIX)]
    block = dict(os.environ)
    block["RUSTC"] = os.path.join(toolchain, "bin", "rustc")
    block["PATH"] = os.path.join(toolchain, "bin") + os.pathsep + os.environ.get("PATH", "/usr/bin:/bin")
    return block, out


def unique(pairs):
    if len({k for k, _ in pairs}) != len(pairs):
        raise ValueError("duplicate key")
    return dict(pairs)


def load_pins(repo, path):
    """([findings], pins): the checked tree's own pins file, opened without following a link (A1 and A7)."""
    refused = lambda reason: ([f("pins", reason=reason)], None)
    if path is None or os.path.abspath(path) != os.path.join(repo, PINS):
        return refused(f"--pins must be the checked tree's own {PINS}")
    if os.path.islink(os.path.join(repo, "tools")):
        return refused("tools is a symbolic link")
    try:
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    except OSError as error:
        return refused(f"missing or a symbolic link: {error.strerror}")
    if not stat.S_ISREG(os.fstat(fd).st_mode):
        os.close(fd)
        return refused("not a regular file")
    with os.fdopen(fd, "rb") as handle:
        data = handle.read(PINS_MAX + 1)
    if len(data) > PINS_MAX:
        return refused(f"over {PINS_MAX} bytes")
    try:
        pins = json.loads(data, object_pairs_hook=unique)
    except ValueError:
        return refused("not JSON")
    digest = lambda value: isinstance(value, str) and bool(HEX.fullmatch(value))
    lines = lambda value: isinstance(value, list) and all(isinstance(x, str) for x in value)
    if (not isinstance(pins, dict) or set(pins) != {"lock_sha256", "graph", "declared"} or not digest(pins["lock_sha256"])
            or not isinstance(pins["graph"], dict) or set(pins["graph"]) != {"default", "all-features"}
            or not all(map(digest, pins["graph"].values())) or not isinstance(pins["declared"], dict)
            or set(pins["declared"]) != set(ROUTE_PACKAGES) or not all(map(lines, pins["declared"].values()))):
        return refused("keys or values differ from the pins schema")
    return [], pins


def keys(value, prefix=""):
    """Dotted keys of a TOML table, for reporting what a configuration sets."""
    out = []
    for key, item in sorted(value.items()):
        name = f"{prefix}.{key}" if prefix else key
        out += keys(item, name) if isinstance(item, dict) else [name]
    return out


def config_findings(repo):
    out = []
    if not os.path.lexists(os.path.join(repo, ".cargo", "config.toml")):
        out.append(f("config", path=os.path.join(".cargo", "config.toml"), reason="missing (it is pinned)"))
    for directory, dirs, names in os.walk(repo):
        dirs[:] = sorted(x for x in dirs if x != ".git")
        for name in sorted(names + dirs):
            path = os.path.join(directory, name)
            rel = os.path.relpath(path, repo)
            if name == ".cargo" and os.path.islink(path):
                out.append(f("config", path=rel, reason="a symbolic link"))
            if os.path.basename(directory) != ".cargo" or name not in ("config", "config.toml"):
                continue
            if rel == os.path.join(".cargo", "config.toml") and not os.path.islink(path) \
                    and os.path.isfile(path) and sha256_file(path) == CARGO_CONFIG_PIN:
                continue
            try:
                with open(path, "rb") as handle:
                    sets = keys(tomllib.load(handle))
            except (OSError, tomllib.TOMLDecodeError) as error:
                sets = [f"unreadable: {type(error).__name__}"]
            reason = "differs from its pin" if rel == os.path.join(".cargo", "config.toml") else "not allowed"
            out.append(f("config", path=rel, reason=reason, sets=sets))
    return out


def metadata(repo, toolchain, block, all_features):
    argv = [os.path.join(toolchain, "bin", "cargo"), "metadata", "--locked", "--offline", "--format-version", "1"]
    run = subprocess.run(argv + (["--all-features"] if all_features else []), cwd=repo, env=block,
                         capture_output=True, text=True)
    if run.returncode != 0:
        return None, run.stderr.strip().splitlines()[-1:] or ["no output"]
    return json.loads(run.stdout), None


def source_of(package, repo):
    if package["source"] is None:
        return "local:" + os.path.relpath(os.path.dirname(package["manifest_path"]), repo)
    return package["source"]


def declared(package, repo):
    lines = []
    for dep in package["dependencies"]:
        source = dep.get("source") or "local:" + os.path.relpath(dep["path"], repo)
        lines.append("|".join([dep["name"], dep.get("rename") or "", dep.get("kind") or "normal",
                               dep.get("target") or "", str(dep["optional"]), str(dep["uses_default_features"]),
                               ",".join(sorted(dep["features"])), dep["req"], source]))
    return sorted(lines)


def graph_lines(meta, repo, roots):
    """The resolved graph reachable from `roots` over every edge kind."""
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    key = lambda i: f"{packages[i]['name']} {packages[i]['version']} {source_of(packages[i], repo)}"
    seen, pending, lines = set(), list(roots), []
    while pending:
        node = pending.pop()
        if node in seen:
            continue
        seen.add(node)
        lines.append(f"node {key(node)} features={','.join(sorted(nodes[node]['features']))}")
        for dep in nodes[node]["deps"]:
            for kind in dep["dep_kinds"]:
                lines.append(f"edge {key(node)} -> {key(dep['pkg'])} as {dep['name']} "
                             f"{kind['kind'] or 'normal'} {kind['target'] or ''}")
            pending.append(dep["pkg"])
    return sorted(lines)


def package_findings(meta, repo, label, wanted):
    out, roots = [], []
    for name, manifest in sorted(ROUTE_PACKAGES.items()):
        found = [p for p in meta["packages"] if p["name"] == name and p["source"] is None
                 and os.path.relpath(p["manifest_path"], repo) == manifest]
        if len(found) != 1:
            out.append(f("route_package", package=name, resolve=label, reason="not found once at its manifest"))
            continue
        package = found[0]
        roots.append(package["id"])
        for target in package["targets"]:
            if "custom-build" in target["kind"]:
                out.append(f("route_package", package=name, resolve=label, reason="build script",
                             path=os.path.relpath(target["src_path"], repo)))
        for feature in sorted(package["features"]):
            out.append(f("route_package", package=name, resolve=label, reason="feature", feature=feature))
        if package.get("links"):
            out.append(f("route_package", package=name, resolve=label, reason="links"))
        have, want = declared(package, repo), wanted[name]
        for line in sorted(set(have) ^ set(want)):
            out.append(f("route_package", package=name, resolve=label,
                         reason="undeclared dependency" if line in have else "missing dependency", dependency=line))
    return out, roots


def preflight(repo, toolchain, path):
    """GB3-1: (block, refusals, pins) from checks that start no process; any refusal ends A1 before Cargo."""
    block, out = launch_block(toolchain)
    if out:
        return block, out, None
    out, pins = load_pins(repo, path)
    return block, out + config_findings(repo), pins


def check(repo, toolchain, path):
    block, out, pins = preflight(repo, toolchain, path)
    lock = os.path.join(repo, "Cargo.lock")
    if pins and (not os.path.isfile(lock) or os.path.islink(lock) or sha256_file(lock) != pins["lock_sha256"]):
        out.append(f("lock", reason="Cargo.lock differs from its pin"))
    if any(x["kind"] != "lock" for x in out):
        return out
    for label, all_features in (("default", False), ("all-features", True)):
        meta, error = metadata(repo, toolchain, block, all_features)
        if meta is None:
            out.append(f("metadata_failed", resolve=label, error=error))
            continue
        found, roots = package_findings(meta, repo, label, pins["declared"])
        out += found
        lines = graph_lines(meta, repo, roots)
        digest = sha256_bytes("\n".join(lines).encode())
        if digest != pins["graph"][label]:
            out.append(f("graph_differs", resolve=label, sha256=digest, lines=len(lines)))
    return out


def main():
    parser = argparse.ArgumentParser()
    for name in ("--repo", "--toolchain", "--pins"):
        parser.add_argument(name, required=True)
    parser.add_argument("--json")
    parser.add_argument("--print-pins", action="store_true", help="review aid: the pins file this tree would have")
    args = parser.parse_args()
    repo, toolchain = os.path.abspath(args.repo), os.path.abspath(args.toolchain)
    if args.print_pins:
        block, refused, _ = preflight(repo, toolchain, args.pins)
        if refused:
            sys.exit(json.dumps(refused))
        pins = {"lock_sha256": sha256_file(os.path.join(repo, "Cargo.lock")), "graph": {}}
        for label, all_features in (("default", False), ("all-features", True)):
            meta, error = metadata(repo, toolchain, block, all_features)
            if meta is None:
                sys.exit(f"route_a1: metadata failed: {error}")
            _, roots = package_findings(meta, repo, label, {name: [] for name in ROUTE_PACKAGES})
            pins["graph"][label] = sha256_bytes("\n".join(graph_lines(meta, repo, roots)).encode())
            pins["declared"] = {p["name"]: declared(p, repo) for p in meta["packages"]
                                if p["name"] in ROUTE_PACKAGES and p["source"] is None}
        print(json.dumps(pins, indent=1, sort_keys=True))
        return 0
    findings = check(repo, toolchain, args.pins)
    result = {"verdict": "fail" if findings else "pass", "findings": findings}
    if args.json:
        with open(args.json, "w", encoding="utf-8") as handle:
            json.dump(result, handle, indent=1, sort_keys=True)
            handle.write("\n")
    print(json.dumps(result, sort_keys=True))
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
