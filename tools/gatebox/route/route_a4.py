#!/usr/bin/python3 -I
"""SB-3E A4 (design section 5 row A4; SB3E-R30.4, R33.3): the installed artifact.

Runs beside `mother/desktop/install.sh`, never inside it (Step 0 section 9).
The binary and its installing script are pinned (SHIPPED: nexus-mother via
mother/desktop/install.sh); route_seams.json must name exactly that, or A4
refuses to run.

  run --repo R --toolchain T --work DIR
      `bash mother/desktop/install.sh`, the script's exact command, with a
      fresh HOME and CARGO_TARGET_DIR under DIR and the pinned toolchain (as
      CI's A2 step). It must install and print its lines; the installed file's
      sha256 must be the one printed; A2's log from that same invocation must
      pass the script's own a2_check again (extracted verbatim) at the private
      target path the build used; then the installed binary is scanned by
      route_a3 against route_seams.json. Each seam is listed by name as
      `reachable` or `unwired`: only `reachable` seams are claimed absent;
      `unwired` proves nothing and rests on A5/A6. Any marker or unknown stem
      in the artifact fails.
  controls --repo R --toolchain T --work DIR --wrapper W
      Positive control: in a scratch copy, a real stripped release of the
      installed binary (the shipped profile: `-C strip=symbols`, ThinLTO,
      checked in its -v log and by the absence of `.symtab`) with the cfg set
      by the route lane's wrapper (its log shows exactly the three route
      units); every `reachable` seam's marker must survive and be reported
      (route_a3 census). Broken-scanner control: a scratch copy of
      route_a3.py with its marker match deliberately broken
      must fail that same positive control. Planted run: `run` on a copy
      carrying the data's first un-gate-and-call control for the installed
      binary must install it (A2 does not see source) and report its seams.

Exit 0 on pass; one JSON verdict on stdout.
"""
import argparse
import hashlib
import json
import os
import shutil
import struct
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
# The shipped binary and its installer, pinned here, not taken from data (SB3E-R37).
SHIPPED = ("nexus-mother", "mother/desktop/install.sh")


def shipped(repo):
    """The pinned shipped binary and script, asserted against route_seams.json."""
    with open(os.path.join(repo, "tools/gatebox/route/route_seams.json"), encoding="utf-8") as handle:
        binaries = json.load(handle)["binaries"]
    found = [(n, b.get("shipped")) for n, b in binaries.items() if b.get("shipped") is not None]
    if found != [SHIPPED] or any("shipped" not in b for b in binaries.values()):
        sys.exit(f"route_a4: route_seams.json must ship exactly {SHIPPED}, with a shipped key on every binary")
    return SHIPPED
# The deliberate break: a match tested one byte past each stem.
BREAK = ("blob.startswith(s[\"marker\"].encode(), at)", "blob.startswith(s[\"marker\"].encode(), at + 1)")


def sha256_file(path):
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def env_block(toolchain, extra=None):
    bin_dir = os.path.join(toolchain, "bin")
    real = os.environ["HOME"]
    block = {"HOME": real, "PATH": bin_dir + ":/usr/bin:/bin", "LANG": "C.UTF-8",
             "RUSTC": os.path.join(bin_dir, "rustc"), "CARGO_HOME": os.environ.get("CARGO_HOME", f"{real}/.cargo"),
             "RUSTUP_HOME": os.environ.get("RUSTUP_HOME", f"{real}/.rustup")}
    block.update(extra or {})
    return block


def a2_function(repo):
    lines = open(os.path.join(repo, shipped(repo)[1]), encoding="utf-8").read().split("\n")
    start = lines.index("a2_check() {")
    return "\n".join(lines[start:lines.index("}", start) + 1]) + "\n"


def section_names(path):
    """ELF64 little-endian section names."""
    with open(path, "rb") as handle:
        blob = handle.read()
    if blob[:6] != b"\x7fELF\x02\x01":
        return None
    shoff, = struct.unpack_from("<Q", blob, 0x28)
    entsize, count, strndx = struct.unpack_from("<HHH", blob, 0x3A)
    table = struct.unpack_from("<QQ", blob, shoff + strndx * entsize + 0x18)
    names = []
    for i in range(count):
        at, = struct.unpack_from("<I", blob, shoff + i * entsize)
        end = blob.index(b"\0", table[0] + at)
        names.append(blob[table[0] + at:end].decode("ascii", "replace"))
    return names


def cfg_set(wrapper_log):
    """The wrapper's own log: it added the cfg to exactly the three route library units."""
    try:
        rows = [json.loads(x) for x in open(wrapper_log, encoding="utf-8")]
    except (OSError, json.JSONDecodeError):
        return False
    return sorted(r["crate"] for r in rows if r["added"]) == ["nexus_governed_control", "nexus_mother",
                                                             "nexus_mother_host"]


def scan(tool, repo, path):
    run = subprocess.run(["/usr/bin/python3", "-I", tool, "scan", "--repo", repo, "--binary", f"{shipped(repo)[0]}={path}"],
                         capture_output=True, text=True)
    return json.loads(run.stdout)


def run_a4(repo, toolchain, work):
    home, parent = os.path.join(work, "home"), os.path.join(work, "parent")
    os.makedirs(home)
    env = env_block(toolchain, {"HOME": home, "CARGO_TARGET_DIR": parent})
    binary, script = shipped(repo)
    run = subprocess.run(["bash", os.path.join(repo, script)], cwd=work, env=env,
                         stdin=subprocess.DEVNULL, capture_output=True, text=True)
    printed = dict(x.split(":", 1) for x in run.stdout.splitlines() if ":" in x)
    printed = {k.strip(): v.strip() for k, v in printed.items()}
    installed, log = os.path.join(home, ".local/bin", binary), os.path.join(parent, "nexus-mother-install.log")
    findings, out = [], {"exit": run.returncode, "printed": printed}
    if run.returncode != 0 or printed.get("installed") != installed or printed.get("a2 log") != log:
        findings.append({"kind": "install_failed", "stderr_tail": run.stderr.strip().splitlines()[-5:]})
        return findings, out
    out["sha256"] = sha256_file(installed)
    if out["sha256"] != printed.get("sha256"):
        findings.append({"kind": "installed_sha256_differs"})
    sysroot = subprocess.run(["rustc", "--print", "sysroot"], cwd=repo, env=env, capture_output=True, text=True)
    rustc = os.path.join(sysroot.stdout.strip(), "bin", "rustc")
    script = "set -uo pipefail\nexport LC_ALL=C\n" + a2_function(repo) + 'a2_check "$1" "$2" "$3"\n'
    check = subprocess.run(["bash", "-c", script, "a4", rustc, os.path.join(parent, "nexus-mother-install"), log],
                           capture_output=True, text=True)
    out["a2_log"] = {"path": log, "sha256": sha256_file(log), "recheck_exit": check.returncode}
    if check.returncode != 0:
        findings.append({"kind": "a2_log_refused", "refusals": check.stdout.splitlines()[:5]})
    verdict = scan(os.path.join(HERE, "route_a3.py"), repo, installed)
    out["seams"] = verdict["results"][0]["seams"] if verdict["results"] else []
    findings += verdict["findings"]
    return findings, out


def scratch_copy(repo, work):
    """The tracked files, copied; returns the copy and its git tree hash (its own git directory)."""
    tree = os.path.join(work, "repo")
    run = subprocess.run(["git", "-C", repo, "ls-files", "-z"], capture_output=True, check=True)
    for rel in (x.decode() for x in run.stdout.split(b"\0") if x):
        path = os.path.join(repo, rel)
        if os.path.isfile(path) and not os.path.islink(path):
            os.makedirs(os.path.dirname(os.path.join(tree, rel)), exist_ok=True)
            shutil.copy2(path, os.path.join(tree, rel))
    git = ["git", f"--git-dir={os.path.join(work, 'tree.git')}", f"--work-tree={tree}"]
    subprocess.run(["git", "init", "-q", "--bare", os.path.join(work, "tree.git")], check=True)
    subprocess.run(git + ["add", "-A", "-f", "."], cwd=tree, check=True)
    return tree, subprocess.run(git + ["write-tree"], capture_output=True, text=True, check=True).stdout.strip()


def planted_run(repo, toolchain, work):
    """`run` must fail on a copy carrying the data's first control for this binary (ungate and call)."""
    os.makedirs(work)
    tree, tree_id = scratch_copy(repo, work)
    data = json.load(open(os.path.join(tree, "tools/gatebox/route/route_seams.json"), encoding="utf-8"))
    spec = next(c for c in data["controls"] if c["binary"] == shipped(tree)[0] and c["expect"])
    for e in [e for g in spec["ungate"] for e in data["ungates"][g]] + spec["edits"]:
        path = os.path.join(tree, e["file"])
        text = open(path, encoding="utf-8").read()
        if text.count(e["old"]) != 1:
            return {"tree": tree_id, "control": spec["name"], "ok": False, "error": f"edit not unique: {e['file']}"}
        open(path, "w", encoding="utf-8").write(text.replace(e["old"], e["new"]))
    findings, out = run_a4(tree, toolchain, os.path.join(work, "run"))
    got = sorted({f["seam"] for f in findings if f["kind"] == "marker_present"})
    shutil.rmtree(tree, ignore_errors=True)
    return {"tree": tree_id, "control": spec["name"], "installed": out.get("sha256") is not None,
            "reported": got, "ok": bool(findings) and set(spec["expect"]) <= set(got)}


def build_positive(repo, toolchain, work, wrapper):
    tree, tree_id = scratch_copy(repo, work)
    target = os.path.join(work, "target")
    census = subprocess.run(["/usr/bin/python3", "-I", os.path.join(HERE, "route_a3.py"), "build", "--repo", tree,
                             "--toolchain", toolchain, "--target", target, "--wrapper", wrapper,
                             "--only", shipped(tree)[0]],
                            env=env_block(toolchain), capture_output=True, text=True)
    return tree, tree_id, target, json.loads(census.stdout)


def controls(repo, toolchain, work, wrapper):
    os.makedirs(work)
    tree, tree_id, target, census = build_positive(repo, toolchain, work, wrapper)
    name = shipped(tree)[0]
    own = os.path.join(target, name)
    binary = os.path.join(own, "release", name)
    findings, out = [], {"tree": tree_id}
    log = open(f"{own}.a3.log", encoding="utf-8").read() if os.path.isfile(binary) else ""
    crate = name.replace("-", "_")
    bin_line = [x for x in log.splitlines() if f" --crate-name {crate} " in x and " --crate-type bin " in x]
    stripped = section_names(binary) if os.path.isfile(binary) else None
    out["release"] = {"bin_lines": len(bin_line), "thin_lto": bool(bin_line) and " -C lto=thin " in bin_line[0],
                      "strip_symbols": bool(bin_line) and " -C strip=symbols " in bin_line[0],
                      "symtab": stripped is None or ".symtab" in stripped,
                      "cfg_set": cfg_set(f"{own}.route_rustc.log")}
    real = scan(os.path.join(HERE, "route_a3.py"), tree, binary) if os.path.isfile(binary) else {"results": []}
    rows = real["results"][0]["seams"] if real["results"] else []
    reachable = [r["seam"] for r in rows if r["disposition"] == "reachable"]
    reported = [r["seam"] for r in rows if r["present"]]
    release_ok = (out["release"]["bin_lines"] == 1 and out["release"]["thin_lto"] and out["release"]["strip_symbols"]
                  and not out["release"]["symtab"] and out["release"]["cfg_set"])
    positive_ok = census["verdict"] == "pass" and release_ok and bool(reachable) and set(reachable) <= set(reported)
    out["positive"] = {"census": census["verdict"], "reachable": reachable, "reported": reported, "ok": positive_ok}
    if not positive_ok:
        findings.append({"kind": "positive_control_failed", "census_findings": census["findings"]})
    broken = os.path.join(work, "broken", "route_a3.py")
    os.makedirs(os.path.dirname(broken))
    text = open(os.path.join(HERE, "route_a3.py"), encoding="utf-8").read()
    if text.count(BREAK[0]) != 1:
        findings.append({"kind": "broken_scanner_plant_missing"})
    else:
        open(broken, "w", encoding="utf-8").write(text.replace(BREAK[0], BREAK[1]))
        rows = scan(broken, tree, binary)["results"][0]["seams"] if os.path.isfile(binary) else []
        missed = [r["seam"] for r in rows if r["disposition"] == "reachable" and not r["present"]]
        out["broken_scanner"] = {"sha256": sha256_file(broken), "missed": missed, "fails_its_control": bool(missed)}
        if not missed:
            findings.append({"kind": "broken_scanner_passed_the_positive_control"})
    shutil.rmtree(target, ignore_errors=True)
    shutil.rmtree(tree, ignore_errors=True)
    out["planted_run"] = planted_run(repo, toolchain, os.path.join(work, "planted"))
    if not out["planted_run"]["ok"]:
        findings.append({"kind": "planted_run_not_reported"})
    return findings, out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("run", "controls"))
    for name in ("--repo", "--toolchain", "--work"):
        parser.add_argument(name, required=True)
    parser.add_argument("--wrapper")
    args = parser.parse_args()
    repo, toolchain, work = map(os.path.abspath, (args.repo, args.toolchain, args.work))
    if os.path.lexists(work):
        sys.exit(f"route_a4: the work directory must be fresh: {work}")
    if args.command == "run":
        os.makedirs(work)
        findings, out = run_a4(repo, toolchain, work)
    else:
        if not args.wrapper:
            sys.exit("route_a4: controls need --wrapper (the route lane's wrapper)")
        findings, out = controls(repo, toolchain, work, os.path.abspath(args.wrapper))
    print(json.dumps(dict(out, verdict="fail" if findings else "pass", findings=findings), sort_keys=True))
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
