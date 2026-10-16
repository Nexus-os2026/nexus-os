#!/usr/bin/env python3
"""Read-only: every registry package in a Cargo.lock has a .crate file in ~/.cargo/registry/cache whose sha256
equals the lock's checksum; every git package is listed separately. Writes nothing."""
import hashlib, os, sys, tomllib, glob, json
lock = tomllib.load(open(sys.argv[1], "rb"))
caches = glob.glob(os.path.expanduser("~/.cargo/registry/cache/*/"))
reg = git = path = 0; missing = []; badsum = []; gits = []
for p in lock["package"]:
    src = p.get("source")
    if src is None: path += 1; continue
    if src.startswith("git+"): git += 1; gits.append(f"{p['name']} {p['version']} {src}"); continue
    reg += 1
    fn = f"{p['name']}-{p['version']}.crate"
    hits = [c + fn for c in caches if os.path.exists(c + fn)]
    if not hits: missing.append(fn); continue
    ok = any(hashlib.sha256(open(h, "rb").read()).hexdigest() == p.get("checksum") for h in hits)
    if not ok: badsum.append(fn)
print(json.dumps({"lock_version": lock.get("version"), "packages": len(lock["package"]), "path_packages": path,
                  "registry_packages": reg, "git_packages": git, "registry_missing_from_cache": missing,
                  "registry_checksum_mismatch": badsum, "git_sources": gits, "caches": caches}, indent=1))
sys.exit(0 if not missing and not badsum else 1)
