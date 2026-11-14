#!/usr/bin/python3 -I
"""SB-3E A1: the route packages' build graph and the snapshot's Cargo configuration.

Design section 5 row A1 (U4b item 1, ruling SB3E-R28.2). Stdlib only, Linux
only. Not wired into install.sh, the policy or the bundle: it joins the
installed bundle in GB3. It prints a JSON verdict and exits nonzero on any
finding.

Checks, across the default and the all-features resolve, every target table
and normal, build and dev edges:
- route_package: each route package (ROUTE_PACKAGES, by name and manifest)
  exists, has no build script, no `[features]` entry (an implicit feature of
  an optional dependency counts), no `links`, and declares exactly the
  reviewed dependencies (DECLARED, every field Cargo reports).
- graph_differs: the resolved graph reachable from the route packages
  (packages by name, version and normalized source; edges by extern name,
  kind and target; enabled features) equals its reviewed hash, per resolve.
- config: `.cargo/config` is absent everywhere, `.cargo/config.toml` exists
  only at the root and is byte-equal to its pin (Cargo prefers the legacy name
  when both exist); no `.cargo` is a symbolic link. What a refused file sets is
  reported by key.
- lock: Cargo.lock is byte-equal to its pin.
Ancestor and Cargo-home configuration is outside the snapshot and is the
runner's (SB3E-R13.2); A2 validates what an installation really ran.
"""
import argparse
import hashlib
import json
import os
import subprocess
import sys
import tomllib

ROUTE_PACKAGES = {
    "nexus-governed-control": "crates/nexus-governed-control/Cargo.toml",
    "nexus-mother": "mother/Cargo.toml",
    "nexus-mother-host": "mother/host/Cargo.toml",
    "nexus-mother-desktop": "mother/desktop/Cargo.toml",
}
LOCK_PIN = "a42d2d167fe3c01f4c61b4365dcdb0c64f2548ed3c52d16193b702f1d30b000e"
CARGO_CONFIG_PIN = "379a150667985722540bd30f737cfb363dc5bc2dc8a5edae3db3725e3cd0b234"
# Reviewed graph hashes: sha256 of graph_lines(), LF-joined (see graph_lines).
GRAPH_PINS = {
    "default": "71ec30cb55fa73058c55d27d8395d89c0ec67694b2f17aea7990512cb7ee2229",
    "all-features": "044b850fa2953eab514b6d1d11fa75a9ce935ce1d2fb8ac6cacc8d71b18f2fa5",
}
# Each route package's declared dependencies:
# name|rename|kind|target|optional|default-features|features|req|source.
DECLARED = {
    "nexus-governed-control": [
        "base64||normal||False|True||^0.22|registry+https://github.com/rust-lang/crates.io-index",
        "getrandom||normal||False|True||^0.2|registry+https://github.com/rust-lang/crates.io-index",
        "hex||normal||False|True||^0.4|registry+https://github.com/rust-lang/crates.io-index",
        "libc||normal|cfg(target_os = \"linux\")|False|True||^0.2|registry+https://github.com/rust-lang/crates.io-index",
        "nexus-kernel||normal||False|True||*|local:kernel",
        "png||normal||False|True||^0.18|registry+https://github.com/rust-lang/crates.io-index",
        "reqwest||normal||False|False|rustls-tls|^0.12|registry+https://github.com/rust-lang/crates.io-index",
        "rusqlite||normal||False|True|bundled|^0.41|registry+https://github.com/rust-lang/crates.io-index",
        "serde_json||dev||False|True||^1.0|registry+https://github.com/rust-lang/crates.io-index",
        "serde_json||normal||False|True||^1.0|registry+https://github.com/rust-lang/crates.io-index",
        "serde||normal||False|True|derive|^1.0|registry+https://github.com/rust-lang/crates.io-index",
        "sha2||normal||False|True||^0.10|registry+https://github.com/rust-lang/crates.io-index",
        "tokio||normal||False|True|macros,rt,time|^1|registry+https://github.com/rust-lang/crates.io-index",
        "unicode-segmentation||normal||False|True||^1.12|registry+https://github.com/rust-lang/crates.io-index",
        "url||normal||False|True||^2|registry+https://github.com/rust-lang/crates.io-index",
        "x11rb||normal|cfg(target_os = \"linux\")|False|True|xtest|^0.13|registry+https://github.com/rust-lang/crates.io-index",
        "zeroize||normal||False|True||^1|registry+https://github.com/rust-lang/crates.io-index",
    ],
    "nexus-mother": [
        "hex||normal||False|True||^0.4|registry+https://github.com/rust-lang/crates.io-index",
        "nexus-governed-control||normal||False|True||*|local:crates/nexus-governed-control",
        "nix||normal||False|False|fs|^0.29|registry+https://github.com/rust-lang/crates.io-index",
        "serde_json||normal||False|True||^1.0|registry+https://github.com/rust-lang/crates.io-index",
        "serde||normal||False|True|derive|^1.0|registry+https://github.com/rust-lang/crates.io-index",
        "sha2||normal||False|True||^0.10|registry+https://github.com/rust-lang/crates.io-index",
    ],
    "nexus-mother-desktop": [
        "gtk4||normal||False|True||=0.11.5|registry+https://github.com/rust-lang/crates.io-index",
        "nexus-governed-control||normal||False|True||*|local:crates/nexus-governed-control",
        "nexus-kernel||normal||False|True||*|local:kernel",
        "nexus-mother-host||normal||False|True||*|local:mother/host",
        "nix||dev||False|False|process,signal|=0.29.0|registry+https://github.com/rust-lang/crates.io-index",
    ],
    "nexus-mother-host": [
        "nexus-governed-control||normal||False|True||*|local:crates/nexus-governed-control",
        "nexus-mother||normal||False|True||*|local:mother",
        "nix||dev||False|True|fs,poll,term|^0.29|registry+https://github.com/rust-lang/crates.io-index",
        "nix||normal|cfg(target_os = \"linux\")|False|True|poll,term|^0.29|registry+https://github.com/rust-lang/crates.io-index",
        "sha2||normal||False|True||^0.10|registry+https://github.com/rust-lang/crates.io-index",
        "uuid||normal||False|True|v4|^1.22|registry+https://github.com/rust-lang/crates.io-index",
    ],
}
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


def package_findings(meta, repo, label):
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
        have, want = declared(package, repo), DECLARED.get(name, [])
        for line in sorted(set(have) ^ set(want)):
            out.append(f("route_package", package=name, resolve=label,
                         reason="undeclared dependency" if line in have else "missing dependency", dependency=line))
    return out, roots


def check(repo, toolchain):
    block, out = launch_block(toolchain)
    if out:
        return out
    out += config_findings(repo)
    lock = os.path.join(repo, "Cargo.lock")
    if not os.path.isfile(lock) or os.path.islink(lock) or sha256_file(lock) != LOCK_PIN:
        out.append(f("lock", reason="Cargo.lock differs from its pin"))
    for label, all_features in (("default", False), ("all-features", True)):
        meta, error = metadata(repo, toolchain, block, all_features)
        if meta is None:
            out.append(f("metadata_failed", resolve=label, error=error))
            continue
        found, roots = package_findings(meta, repo, label)
        out += found
        lines = graph_lines(meta, repo, roots)
        digest = sha256_bytes("\n".join(lines).encode())
        if digest != GRAPH_PINS[label]:
            out.append(f("graph_differs", resolve=label, sha256=digest, lines=len(lines)))
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--toolchain", required=True)
    parser.add_argument("--json")
    parser.add_argument("--print-pins", action="store_true", help="review aid: the values this tree would pin")
    args = parser.parse_args()
    repo, toolchain = os.path.abspath(args.repo), os.path.abspath(args.toolchain)
    if args.print_pins:
        block, refused = launch_block(toolchain)
        if refused:
            sys.exit(json.dumps(refused))
        pins = {}
        for label, all_features in (("default", False), ("all-features", True)):
            meta, error = metadata(repo, toolchain, block, all_features)
            if meta is None:
                sys.exit(f"route_a1: metadata failed: {error}")
            _, roots = package_findings(meta, repo, label)
            pins[label] = sha256_bytes("\n".join(graph_lines(meta, repo, roots)).encode())
            pins["declared"] = {p["name"]: declared(p, repo) for p in meta["packages"]
                                if p["name"] in ROUTE_PACKAGES and p["source"] is None}
        print(json.dumps(pins, indent=1, sort_keys=True))
        return 0
    findings = check(repo, toolchain)
    result = {"verdict": "fail" if findings else "pass", "findings": findings}
    if args.json:
        with open(args.json, "w", encoding="utf-8") as handle:
            json.dump(result, handle, indent=1, sort_keys=True)
            handle.write("\n")
    print(json.dumps(result, sort_keys=True))
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
