#!/usr/bin/env python3
"""B-W/C: pin E-W and E-W7, run the 6A listings, and judge NI-6 (build-time clause), NI-7, NI-8 and the four-way agreement.
Mission P3-C11-DR01-D2A1-C7-W-EVIDENCE-EXEC-01 (AD-C7-W 6.2, 6.3 O-1..O-6, 6A, 7.1, 13.3 steps 3-6).

  c7bwc.py pin   <evdir> <E-W|E-W7>   write receipts/12-pin/<role>.json from the build receipt (re-hashes the executable)
  c7bwc.py list  <evdir> <E-W|E-W7>   the eight supervised listings of that executable
  c7bwc.py check <evdir>              judge everything; write receipts/30-bwc/B-W-C.{txt,json}; exit 0 = MECHANICALLY SATISFIED, 1 = BLOCKED
  c7bwc.py selftest <dir>             synthetic + scratch-crate tests of this module (never touches the Nexus repository)

Nothing here runs cargo. Every output file is created exclusively.
"""
import bisect
import json
import os
import re
import stat
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from c7lib import *  # noqa
import c7sup
import c7wt

ROLES = {"E-W": {"build": "10-build-E-W", "list": "20-list-E-W", "srole": "W", "wt": WT_HEALTHY},
         "E-W7": {"build": "11-build-E-W7", "list": "21-list-E-W7", "srole": "W7", "wt": WT_MUTANT}}

LIST_SPECS = [  # (directory, libtest arguments, kind, test-name key)
    ("L1-list-all", ["--list"], "all", None),
    ("L2-list-ignored", ["--list", "--ignored"], "ignored", None),
    ("L3-exact-witness", ["--exact", "--list", "{witness}"], "exact", "witness"),
    ("L4-exact-full_fpt", ["--exact", "--list", "{fpt}"], "exact", "fpt"),
    ("L5-exact-full_f3", ["--exact", "--list", "{f3}"], "exact", "f3"),
    ("L6-exact-ignored-witness", ["--exact", "--ignored", "--list", "{witness}"], "exact-ignored", "witness"),
    ("L7-exact-ignored-full_fpt", ["--exact", "--ignored", "--list", "{fpt}"], "exact-ignored", "fpt"),
    ("L8-exact-ignored-full_f3", ["--exact", "--ignored", "--list", "{f3}"], "exact-ignored", "f3"),
]
LIST_LIMIT = 2700
ALLOWED_BUILD_ENV = {"HOME", "PATH", "CARGO_NET_OFFLINE", "CARGO_TARGET_DIR"}


def default_cfg():
    return {
        "all_count": 349, "ign_count": 35,
        "witness": TEST_WITNESS, "fpt": TEST_FULL_FPT, "f3": TEST_FULL_F3,
        "build_argv": c7sup.BUILD_ARGV, "run_env": dict(c7sup.RUN_ENV), "crate_dir": c7sup.CRATE_DIR,
        "compiling_marker": "Compiling nexus-governed-control ",
        "verify_source": True,
        "ec": None,  # loaded lazily from git objects
        "src_counts": None,  # callable -> (tests, ignores); default: git grep at W_COMMIT
        "e_c_sha": E_C_SHA, "hist_sha": HIST_C7_SHA,
        "wt": {r: ROLES[r]["wt"] for r in ROLES},
        "ec_all_sha": EC_LIST_ALL_SHA, "ec_ign_sha": EC_LIST_IGN_SHA,
        "identity": {"W_COMMIT": W_COMMIT, "W_TREE": W_TREE, "W_BLOB": W_BLOB, "W_BLOB_SHA": W_BLOB_SHA, "W_BLOB_SIZE": W_BLOB_SIZE,
                     "W_PATCH_SHA": W_PATCH_SHA, "C7_PATCH_SHA": PATCH_SHA, "MUTANT_BLOB": MUTANT_BLOB, "MUTANT_SHA": MUTANT_SHA},
    }


# ----------------------------------------------------------------------------- libtest listing parser
def parse_listing(data):
    """Strict parser of libtest's `--list` output. Returns {tests, count, trailer, problems}."""
    problems = []
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return {"tests": [], "count": None, "trailer": None, "problems": ["not valid UTF-8"]}
    if "\r" in text:
        problems.append("contains CR")
    if text and not text.endswith("\n"):
        problems.append("does not end with a newline")
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    if not lines:
        return {"tests": [], "count": None, "trailer": None, "problems": problems + ["empty"]}
    m = re.fullmatch(r"(\d+) tests?, (\d+) benchmarks?", lines[-1])
    if not m:
        return {"tests": [], "count": None, "trailer": lines[-1], "problems": problems + ["last line is not the 'N tests, M benchmarks' trailer: %r" % lines[-1]]}
    n, nb = int(m.group(1)), int(m.group(2))
    word_ok = lines[-1] == "%d %s, %d %s" % (n, "test" if n == 1 else "tests", nb, "benchmark" if nb == 1 else "benchmarks")
    if not word_ok:
        problems.append("trailer pluralisation is not libtest's: %r" % lines[-1])
    body = lines[:-1]
    if n == 0:
        if body:
            problems.append("0 tests but %d line(s) precede the trailer" % len(body))
        tests = []
    else:
        if not body or body[-1] != "":
            problems.append("no blank line before the trailer")
            tests_lines = body
        else:
            tests_lines = body[:-1]
        tests = []
        for l in tests_lines:
            mm = re.fullmatch(r"([A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*): test", l)
            if not mm:
                problems.append("line is not '<path>: test': %r" % l)
            else:
                tests.append(mm.group(1))
        if len(tests) != n:
            problems.append("trailer says %d tests but %d test lines" % (n, len(tests)))
    if nb != 0:
        problems.append("%d benchmarks listed" % nb)
    if len(set(tests)) != len(tests):
        problems.append("duplicate test lines")
    return {"tests": tests, "count": n, "trailer": lines[-1], "problems": problems}


def render_listing(tests):
    n = len(tests)
    body = "".join("%s: test\n" % t for t in tests)
    return (body + ("\n" if n else "") + "%d %s, 0 benchmarks\n" % (n, "test" if n == 1 else "tests")).encode()


# ----------------------------------------------------------------------------- E-C baseline (git objects only)
def load_ec(cfg):
    if cfg["ec"] is not None:
        return cfg["ec"]
    base = EC_RECEIPTS
    allraw = blob_at(EC_EVID, base + "/20-list-all/raw-output.txt")
    ignraw = blob_at(EC_EVID, base + "/21-list-ignored/raw-output.txt")
    rcpt = blob_at(EC_EVID, base + "/10-build/RECEIPT.txt").decode()
    rustc = [l.split("  rustc -vV: ", 1)[1] for l in rcpt.splitlines() if l.startswith("  rustc -vV: ")]
    cargo = [l.split("  cargo -vV: ", 1)[1] for l in rcpt.splitlines() if l.startswith("  cargo -vV: ")]
    pin = json.loads(blob_at(EC_EVID, base + "/10-build/PIN.json"))
    cfg["ec"] = {"all_raw": allraw, "ign_raw": ignraw, "rustc_lines": rustc, "cargo_lines": cargo, "pin": pin,
                 "all_sha": sha256_bytes(allraw), "ign_sha": sha256_bytes(ignraw)}
    return cfg["ec"]


def default_src_counts():
    def count(pat):
        r = git("grep", "-E", "-c", pat, W_COMMIT, "--", "crates/nexus-governed-control/src", check=False).stdout.decode().strip().splitlines()
        return sum(int(x.rsplit(":", 1)[1]) for x in r if x)
    return count(r"#\[test\]"), count(r"#\[ignore")


# ----------------------------------------------------------------------------- descriptors
def jload(p):
    with open(p) as f:
        return json.load(f)


def proc_checks(d, who, limit):
    """Process / cgroup / timeout checks shared by builds and invocations (CL-1 process half, 7.3)."""
    p, s = d["process"], d["supervisor"]
    out = []

    def c(cid, desc, ok, det=""):
        out.append((cid, "%s: %s" % (who, desc), bool(ok), det))
    c("PROC-1", "normal exit status 0, no signal, supervisor rc 0", p["exit_status"] == 0 and p["signal"] is None and p["supervisor_rc"] == 0
      and p["wrapper_recorded_status"] == "0", str(p))
    c("PROC-2", "scope record complete; not timed out", s["completed"] is True and s["timed_out"] is False,
      "completed=%s timed_out=%s scope_result=%s" % (s["completed"], s["timed_out"], s["scope_result"]))
    c("PROC-3", "oom 0 and oom_kill 0", s["oom"] == 0 and s["oom_kill"] == 0, "oom=%s oom_kill=%s" % (s["oom"], s["oom_kill"]))
    c("PROC-4", "zero swap: memory.swap.peak 0 and no swap events", s["swap_peak_bytes"] == "0" and all(v == 0 for v in s["swap_events"].values()),
      "swap.peak=%s swap.events=%s" % (s["swap_peak_bytes"], s["swap_events"]))
    c("PROC-5", "cgroup limits memory.max 17179869184 and memory.swap.max 0", s["memory_max"] == "17179869184" and s["swap_max"] == "0",
      "memory.max=%s swap.max=%s" % (s["memory_max"], s["swap_max"]))
    c("PROC-6", "memory.peak below the limit", s["memory_peak_bytes"] is not None and int(s["memory_peak_bytes"]) < 17179869184, "peak=%s" % s["memory_peak_bytes"])
    c("PROC-7", "wall time below the ceiling %d s" % limit, s["wall_seconds"] is not None and s["wall_seconds"] < limit and s["time_limit_seconds"] == limit,
      "wall=%s limit=%s" % (s["wall_seconds"], s["time_limit_seconds"]))
    return out


def build_checks(d, role, cfg, bdir):
    wt = cfg["wt"][role]
    out = proc_checks(d, "%s build" % role, c7sup.BUILD_LIMIT)

    def c(cid, desc, ok, det=""):
        out.append((cid, "%s build: %s" % (role, desc), bool(ok), det))
    c("NI-6a", "build command is exactly the 7.1 command", d["argv"] == cfg["build_argv"], " ".join(d["argv"]))
    env = d["env"]
    c("NI-6b", "environment is exactly HOME, PATH, CARGO_NET_OFFLINE=true and the worktree-local CARGO_TARGET_DIR; nothing else",
      set(env) == ALLOWED_BUILD_ENV and env["CARGO_NET_OFFLINE"] == "true" and env["CARGO_TARGET_DIR"] == os.path.join(wt, "target"), json.dumps(env))
    c("NI-6c", "no RUSTFLAGS, CARGO_ENCODED_RUSTFLAGS, CARGO_PROFILE_*, CARGO_BUILD_*, RUSTC_WRAPPER or target override in the build environment",
      not any(re.match(r"^(RUSTFLAGS|CARGO_ENCODED_RUSTFLAGS|RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|RUSTDOCFLAGS|CARGO_PROFILE_|CARGO_BUILD_|CARGO_TARGET_(?!DIR$))", k) for k in env), "")
    c("BLD-1", "built in the recorded worktree with -j 2", d["worktree"] == wt and d["jobs"] == 2 and d["argv"][-2:] == ["-j", "2"], d["worktree"])
    ts = jload(os.path.join(bdir, "toolchain-versions.json"))
    ec = load_ec(cfg)
    c("NI-6d", "rustc -vV equals E-C's build record (%d lines)" % len(ec["rustc_lines"]),
      bool(ec["rustc_lines"]) and ts["rustc -vV"].splitlines() == ec["rustc_lines"], "\n".join(ts["rustc -vV"].splitlines()[:3]))
    c("NI-6e", "cargo -vV equals E-C's build record (%d lines)" % len(ec["cargo_lines"]),
      bool(ec["cargo_lines"]) and ts["cargo -vV"].splitlines() == ec["cargo_lines"], "\n".join(ts["cargo -vV"].splitlines()[:2]))
    cm = d["compiler_messages"]
    stderr_p = os.path.join(bdir, "build.stderr")
    text = open(stderr_p, errors="replace").read()
    lines = text.splitlines()
    nwarn = sum(1 for l in lines if re.match(r"^warning", l))
    nerr = sum(1 for l in lines if re.match(r"^error", l))
    c("NI-7a", "zero warnings and zero errors (recorded and recounted from build.stderr)",
      cm["warnings"] == 0 and cm["errors"] == 0 and nwarn == 0 and nerr == 0, "recorded %s/%s recount %d/%d" % (cm["warnings"], cm["errors"], nwarn, nerr))
    c("NI-7b", "build.stderr matches its recorded sha256", sha256_file(stderr_p) == d["build_stderr_sha256"], "")
    c("NI-7c", "exactly one `Finished` line, release profile", len(d["finished"]) == 1 and d["finished"][0].startswith("Finished `release` profile [optimized]"), str(d["finished"]))
    c("NI-7d", "the package under test was compiled", any(l.startswith(cfg["compiling_marker"]) for l in d["compiling"]), str(d["compiling"][-3:]))
    c("BLD-2", "exactly one pinned test executable, named by cargo and found by the deps scan",
      len(d["pinned_executables"]) == 1 and len(d["executable_lines"]) == 1 and d["executables_found"] == [d["pinned_executables"][0]["path"]], str(d["executables_found"]))
    if d["pinned_executables"]:
        pe = d["pinned_executables"][0]
        c("BLD-3", "the executable lies under the worktree-local target/release/deps", pe["path"].startswith(os.path.join(wt, "target/release/deps") + "/"), pe["path"])
    if cfg["verify_source"]:
        c("BLD-4", "source verification (%s) passed before and after the build" % ROLES[role]["srole"],
          all(x["ok"] for x in d["source_verification_before"]) and all(x["ok"] for x in d["source_verification_after"])
          and len(d["source_verification_before"]) > 0 and len(d["source_verification_after"]) > 0, "")
    return out


def fingerprint_of(wt):
    """The compile fingerprint of the package's test-lib unit (recorded; `rustflags` must be empty)."""
    fp = os.path.join(wt, "target/release/.fingerprint")
    found = []
    if os.path.isdir(fp):
        for n in sorted(os.listdir(fp)):
            if n.startswith("nexus-governed-control-") or n.startswith("listcrate-") or n.startswith("scratchcrate-"):
                for f in sorted(os.listdir(os.path.join(fp, n))):
                    if f.startswith("test-lib-") and f.endswith(".json"):
                        found.append(os.path.join(fp, n, f))
    return found


# ----------------------------------------------------------------------------- pin
def pin(evdir, role, cfg=None):
    cfg = cfg or default_cfg()
    R = ROLES[role]
    wt = cfg["wt"][role]
    bdir = os.path.join(evdir, "receipts", R["build"])
    d = jload(os.path.join(bdir, "descriptor.json"))
    checks = build_checks(d, role, cfg, bdir)
    bad = [c for c in checks if not c[2]]
    for cid, desc, ok, det in checks:
        print("[%s] %s: %s%s" % ("PASS" if ok else "FAIL", cid, desc, "" if ok else " -- " + det[:300]))
    if bad:
        print("PIN REFUSED: the build receipt is not clean (%d check(s) failed); nothing is pinned" % len(bad))
        return 1
    pe = d["pinned_executables"][0]
    exe = pe["path"]
    st = os.lstat(exe)
    sha = sha256_file(exe)
    ok = stat.S_ISREG(st.st_mode) and not stat.S_ISLNK(st.st_mode) and (st.st_mode & 0o111) and sha == pe["sha256"] and st.st_size == pe["size"] \
        and os.path.realpath(exe) == exe
    print("[%s] PIN-1: executable %s is a regular, non-symlink, executable file; sha256 %s equals the build receipt's; size %d" % ("PASS" if ok else "FAIL", exe, sha, st.st_size))
    if not ok:
        return 1
    vchecks = [(a, b, c) for a, b, c in c7wt.verify(wt, R["srole"])] if cfg["verify_source"] else []
    if not all(b for _, b, _ in vchecks):
        print("PIN REFUSED: the source verification (%s) failed at pin time" % R["srole"])
        return 1
    fps = fingerprint_of(wt)
    fpj = []
    for f in fps:
        j = jload(f)
        fpj.append({"path": os.path.relpath(f, wt), "sha256": sha256_file(f), "rustflags": j.get("rustflags"), "profile": j.get("profile"),
                    "config": j.get("config"), "target": j.get("target"), "rustc": j.get("rustc"), "features": j.get("features"),
                    "declared_features": j.get("declared_features"), "compile_kind": j.get("compile_kind"), "path_hash": j.get("path")})
    inv = d["inventory"]
    inv_exe = [x for x in inv if x["path"] == os.path.relpath(exe, os.path.join(wt, "target"))]
    pinrec = {
        "role": role, "mission": MISSION, "pinned_at": now(), "worktree": wt,
        "source": {"commit": W_COMMIT, "tree": W_TREE, "model_tests_blob": W_BLOB, "patch_sha256": W_PATCH_SHA,
                   "applied_mutation": "C7 (patch sha256 %s; mutant protocol.rs blob %s sha256 %s)" % (PATCH_SHA, MUTANT_BLOB, MUTANT_SHA) if role == "E-W7" else None,
                   "verification": [{"id": a, "ok": b, "detail": c} for a, b, c in vchecks]},
        "executable": {"path": exe, "sha256": sha, "size": st.st_size, "mode": oct(st.st_mode & 0o7777), "mtime_ns": st.st_mtime_ns, "inode": st.st_ino},
        "build": {"receipt_dir": "receipts/" + R["build"], "descriptor_sha256": sha256_file(os.path.join(bdir, "descriptor.json")),
                  "receipt_sha256": sha256_file(os.path.join(bdir, "RECEIPT.txt")), "stderr_sha256": d["build_stderr_sha256"],
                  "finished": d["finished"], "warnings": d["compiler_messages"]["warnings"], "errors": d["compiler_messages"]["errors"],
                  "wall_seconds": d["supervisor"]["wall_seconds"], "memory_peak_bytes": d["supervisor"]["memory_peak_bytes"]},
        "artifact_inventory": {"files": len(inv), "executable_entry": inv_exe, "sha256_of_inventory": sha256_bytes(json.dumps(inv, sort_keys=True).encode()), "inventory": inv},
        "fingerprints": fpj,
        "statement": "No cargo build or test command is run in this target after the pin. No reproducible-build equality is assumed.",
    }
    os.makedirs(os.path.join(evdir, "receipts/12-pin"), exist_ok=True)
    with open(os.path.join(evdir, "receipts/12-pin", role + ".json"), "x") as f:
        json.dump(pinrec, f, indent=1)
    print("PINNED %s: %s sha256 %s size %d" % (role, exe, sha, st.st_size))
    for j in fpj:
        print("  fingerprint %s rustflags=%s profile=%s" % (j["path"], j["rustflags"], j["profile"]))
    return 0


# ----------------------------------------------------------------------------- listings
def list_args(spec, cfg):
    return [a.format(witness=cfg["witness"], fpt=cfg["fpt"], f3=cfg["f3"]) for a in spec[1]]


def run_lists(evdir, role, cfg=None):
    cfg = cfg or default_cfg()
    R = ROLES[role]
    wt = cfg["wt"][role]
    pj = jload(os.path.join(evdir, "receipts/12-pin", role + ".json"))
    exe, sha = pj["executable"]["path"], pj["executable"]["sha256"]
    for spec in LIST_SPECS:
        out = os.path.join(evdir, "receipts", R["list"], spec[0])
        rc = c7sup.invoke(out, wt, "list-%s-%s" % (role, spec[0]), exe, sha, LIST_LIMIT, list_args(spec, cfg),
                          verify_role=R["srole"] if cfg["verify_source"] else None, cwd=os.path.join(wt, cfg["crate_dir"]), env=[(k, v) for k, v in cfg["run_env"].items()])
        d = jload(os.path.join(out, "descriptor.json")) if rc == 0 else None
        if rc != 0 or any(not c[2] for c in proc_checks(d, "x", LIST_LIMIT)):
            print("LISTING %s FAILED (rc %s); STOP BEFORE TEST EXECUTION" % (spec[0], rc))
            return 1
        print("listing %s ok" % spec[0])
    return 0


def listing_checks(evdir, role, cfg, ec):
    """NI-8 / 6A checks for one executable. Returns (checks, info) where info carries the parsed listings."""
    R = ROLES[role]
    wt = cfg["wt"][role]
    pj = jload(os.path.join(evdir, "receipts/12-pin", role + ".json"))
    exe, sha = pj["executable"]["path"], pj["executable"]["sha256"]
    out, info = [], {}

    def c(cid, desc, ok, det=""):
        out.append((cid, "%s: %s" % (role, desc), bool(ok), det))
    exe_now = sha256_file(exe)
    c("NI-8.0", "the executable still has the pinned sha256 after all listings", exe_now == sha, exe_now)
    raws, parsed = {}, {}
    for spec in LIST_SPECS:
        ld = os.path.join(evdir, "receipts", R["list"], spec[0])
        dp = os.path.join(ld, "descriptor.json")
        if not os.path.exists(dp):
            c("NI-8.%s" % spec[0], "listing receipt exists", False, ld)
            continue
        d = jload(dp)
        raw = open(os.path.join(ld, "raw-output.txt"), "rb").read()
        raws[spec[0]] = raw
        parsed[spec[0]] = parse_listing(raw)
        for cid, desc, ok, det in proc_checks(d, "%s %s" % (role, spec[0]), LIST_LIMIT):
            out.append((cid, desc, ok, det))
        args = list_args(spec, cfg)
        e = d["executable"]
        c("LST-1.%s" % spec[0], "%s: argv is exactly [pinned executable] + %s; env is exactly the 7.2 run environment; cwd is the crate directory" % (spec[0], args),
          d["argv"] == [exe, *args] and d["env"] == cfg["run_env"] and d["cwd"] == os.path.join(wt, cfg["crate_dir"]), "%s | %s | %s" % (d["argv"], d["env"], d["cwd"]))
        c("LST-2.%s" % spec[0], "%s: executable sha256 before and after equal the pin" % spec[0],
          e["path"] == exe and e["sha256_before"] == sha and e["sha256_after"] == sha, "%s %s" % (e["sha256_before"], e["sha256_after"]))
        c("LST-3.%s" % spec[0], "%s: raw output matches its recorded sha256" % spec[0], sha256_bytes(raw) == d["raw_output_sha256"], "")
        if cfg["verify_source"]:
            c("LST-4.%s" % spec[0], "%s: source verification passed before and after" % spec[0],
              d["source_verification_before"] and d["source_verification_after"] and all(x["ok"] for x in d["source_verification_before"] + d["source_verification_after"]), "")
        c("LST-5.%s" % spec[0], "%s: strictly formed libtest listing" % spec[0], not parsed[spec[0]]["problems"], "; ".join(parsed[spec[0]]["problems"])[:300])
    info["raws"], info["parsed"] = raws, parsed
    if len(parsed) != len(LIST_SPECS):
        return out, info
    allp, ignp = parsed["L1-list-all"], parsed["L2-list-ignored"]
    ec_all = parse_listing(ec["all_raw"])
    ec_ign = parse_listing(ec["ign_raw"])
    witness, fpt, f3 = cfg["witness"], cfg["fpt"], cfg["f3"]
    c("NI-8.1", "all-tests listing has %d tests with trailer '%d tests, 0 benchmarks'" % (cfg["all_count"], cfg["all_count"]),
      allp["count"] == cfg["all_count"] and len(allp["tests"]) == cfg["all_count"] and allp["trailer"] == "%d tests, 0 benchmarks" % cfg["all_count"], str(allp["trailer"]))
    c("NI-8.2", "the all-listing equals E-C's %d lines plus exactly the one new witness line (set comparison)" % ec_all["count"],
      set(allp["tests"]) - set(ec_all["tests"]) == {witness} and set(ec_all["tests"]) - set(allp["tests"]) == set() and allp["tests"].count(witness) == 1,
      "added=%s removed=%s" % (sorted(set(allp["tests"]) - set(ec_all["tests"])), sorted(set(ec_all["tests"]) - set(allp["tests"]))))
    ec_sorted = ec_all["tests"] == sorted(ec_all["tests"])
    if ec_sorted:
        exp = list(ec_all["tests"])
        bisect.insort(exp, witness)
        c("NI-8.3", "the all-listing is byte-identical to E-C's raw listing with the witness line inserted in sorted position and the trailer replaced",
          raws["L1-list-all"] == render_listing(exp), "sha256 %s" % sha256_bytes(raws["L1-list-all"]))
    else:
        c("NI-8.3", "E-C's listing is not sorted, so only the set comparison applies (recorded)", True, "")
    c("NI-8.4", "ignored listing is byte-identical to E-C's `21-list-ignored` raw output (sha256 %s) and has %d tests" % (cfg["ec_ign_sha"][:8] + "…" + cfg["ec_ign_sha"][-4:], cfg["ign_count"]),
      raws["L2-list-ignored"] == ec["ign_raw"] and sha256_bytes(raws["L2-list-ignored"]) == cfg["ec_ign_sha"] and ignp["count"] == cfg["ign_count"], sha256_bytes(raws["L2-list-ignored"]))
    c("NI-8.5", "the ignored tests are a subset of all tests; the witness is ordinary (not ignored); full_fpt and full_f3 are ignored",
      set(ignp["tests"]) <= set(allp["tests"]) and witness in allp["tests"] and witness not in ignp["tests"] and fpt in ignp["tests"] and f3 in ignp["tests"],
      "")
    one = lambda n: ("%s: test\n\n1 test, 0 benchmarks\n" % n).encode()  # noqa
    zero = b"0 tests, 0 benchmarks\n"
    for key, nm in (("L3-exact-witness", witness), ("L4-exact-full_fpt", fpt), ("L5-exact-full_f3", f3)):
        c("NI-8.6.%s" % key, "`--exact --list %s` resolves to exactly one test" % nm.split("::")[-1], raws[key] == one(nm), "")
    c("NI-8.7.L6", "`--exact --ignored --list <witness>` selects nothing (the witness is not ignored)", raws["L6-exact-ignored-witness"] == zero, "")
    for key, nm in (("L7-exact-ignored-full_fpt", fpt), ("L8-exact-ignored-full_f3", f3)):
        c("NI-8.7.%s" % key, "`--exact --ignored --list %s` resolves to exactly one test" % nm.split("::")[-1], raws[key] == one(nm), "")
    info["filtered_out_expected"] = allp["count"] - 1 if allp["count"] else None
    info["all_sha"] = sha256_bytes(raws["L1-list-all"])
    info["ign_sha"] = sha256_bytes(raws["L2-list-ignored"])
    return out, info


# ----------------------------------------------------------------------------- check = B-W/C
def check(evdir, cfg=None, selfwrite=True):
    cfg = cfg or default_cfg()
    ec = load_ec(cfg)
    rec = Rec("B-W/C: NI-6 (build-time clause), NI-7, NI-8 and the 6A four-way agreement")
    rec.check("EC-1", "E-C baseline listings read from git objects at %s have the frozen sha256 values" % EC_EVID[:8],
                      ec["all_sha"] == cfg["ec_all_sha"] and ec["ign_sha"] == cfg["ec_ign_sha"], "%s %s" % (ec["all_sha"][:12], ec["ign_sha"][:12]))
    ec_all, ec_ign = parse_listing(ec["all_raw"]), parse_listing(ec["ign_raw"])
    rec.check("EC-2", "E-C listings are well formed with %d all / %d ignored tests" % (cfg["all_count"] - 1, cfg["ign_count"]),
              not ec_all["problems"] and not ec_ign["problems"] and ec_all["count"] == cfg["all_count"] - 1 and ec_ign["count"] == cfg["ign_count"], "")
    info = {}
    pins = {}
    for role in ("E-W", "E-W7"):
        R = ROLES[role]
        rec.log("\n## %s" % role)
        pp = os.path.join(evdir, "receipts/12-pin", role + ".json")
        if not os.path.exists(pp):
            rec.check("PIN", "%s pin exists" % role, False, pp)
            continue
        pins[role] = jload(pp)
        bdir = os.path.join(evdir, "receipts", R["build"])
        d = jload(os.path.join(bdir, "descriptor.json"))
        for cid, desc, ok, det in build_checks(d, role, cfg, bdir):
            rec.check(cid, desc, ok, det if not ok else "")
        pj = pins[role]
        rec.check("PIN-2", "%s pin equals the build receipt's executable (path, sha256, size) and its descriptor/receipt hashes still match" % role,
                  pj["executable"]["path"] == d["pinned_executables"][0]["path"] and pj["executable"]["sha256"] == d["pinned_executables"][0]["sha256"]
                  and pj["executable"]["size"] == d["pinned_executables"][0]["size"] and pj["build"]["descriptor_sha256"] == sha256_file(os.path.join(bdir, "descriptor.json"))
                  and pj["build"]["receipt_sha256"] == sha256_file(os.path.join(bdir, "RECEIPT.txt")), "")
        rec.check("PIN-3", "%s: the compile fingerprint shows rustflags == [] (recorded: %s)" % (role, [(f["path"].split("/")[-2], f["profile"]) for f in pj["fingerprints"]]),
                  len(pj["fingerprints"]) >= 1 and all(f["rustflags"] == [] for f in pj["fingerprints"]), "")
        checks, linfo = listing_checks(evdir, role, cfg, ec)
        for cid, desc, ok, det in checks:
            rec.check(cid, desc, ok, det if not ok else "")
        info[role] = linfo
    rec.log("\n## cross checks")
    if len(pins) == 2:
        pw, p7 = pins["E-W"]["executable"], pins["E-W7"]["executable"]
        rec.check("0A-1", "E-W and E-W7 are different executables (different sha256 and path; proves difference only)", pw["sha256"] != p7["sha256"] and pw["path"] != p7["path"], "%s vs %s" % (pw["sha256"][:12], p7["sha256"][:12]))
        rec.check("0A-2", "E-W is not E-C (cde9f9d2…)", pw["sha256"] != cfg["e_c_sha"] and pw["path"] != ec["pin"]["executable"], pw["sha256"][:12])
        rec.check("0A-3", "E-W7 is not the historical C7 executable (b320977a…)", p7["sha256"] != cfg["hist_sha"], p7["sha256"][:12])
        rec.check("0A-4", "E-W7 is not E-C", p7["sha256"] != cfg["e_c_sha"], "")
    if len(info) == 2 and all("raws" in i and len(i["raws"]) == len(LIST_SPECS) for i in info.values()):
        for spec in LIST_SPECS:
            same = info["E-W"]["raws"][spec[0]] == info["E-W7"]["raws"][spec[0]]
            rec.check("AGREE-1.%s" % spec[0], "E-W's and E-W7's `%s` raw outputs are byte-identical" % spec[0], same, "")
        sc = (cfg["src_counts"] or default_src_counts)()
        tw = (info["E-W"]["parsed"]["L1-list-all"]["count"], info["E-W"]["parsed"]["L2-list-ignored"]["count"])
        t7 = (info["E-W7"]["parsed"]["L1-list-all"]["count"], info["E-W7"]["parsed"]["L2-list-ignored"]["count"])
        exp = (cfg["all_count"], cfg["ign_count"])
        rec.check("AGREE-2", "four-way agreement (total, ignored): W-TREE source %s; E-W listings %s; E-W7 listings %s; expectation %s" % (sc, tw, t7, exp),
                  tuple(sc) == tw == t7 == exp, "")
        rec.check("AGREE-3", "each single-test invocation will print `%d filtered out` (the listing's own all-count minus 1) on both executables" % (cfg["all_count"] - 1),
                  info["E-W"]["filtered_out_expected"] == info["E-W7"]["filtered_out_expected"] == cfg["all_count"] - 1, "")
        fourway = {"source_counts": list(sc), "E-W": list(tw), "E-W7": list(t7), "expected": list(exp)}
    else:
        rec.check("AGREE", "all sixteen listings are present for the four-way agreement", False, "")
        fourway = None
    ok = not rec.fails
    verdict = ("B-W/C: MECHANICALLY SATISFIED (NI-6 build-time clause, NI-7, NI-8; four-way agreement holds). SUBJECT TO INDEPENDENT ARCHITECT AUDIT. "
               "Not a final acceptance of C7.") if ok else "B-W/C: NOT ESTABLISHED — B-W/C VALIDATION BLOCKED (failed: %s). STOP BEFORE TEST EXECUTION." % ", ".join(sorted(set(rec.fails)))
    rec.log("\n" + verdict)
    # O-6 record
    o6 = ["", "## O-6 record (AD-C7-W 6.3)",
          "W-SRC identity: W-COMMIT %s; W-TREE %s; W-BLOB %s (sha256 %s, %d bytes); W-PATCH sha256 %s; sole parent C-BASE %s." % (W_COMMIT, W_TREE, W_BLOB, W_BLOB_SHA, W_BLOB_SIZE, W_PATCH_SHA, C_BASE),
          "W7-SRC = W-SRC plus the frozen C7 patch (sha256 %s), never committed or staged; protocol.rs blob %s sha256 %s." % (PATCH_SHA, MUTANT_BLOB, MUTANT_SHA),
          "B-W/B was conveyed by: the Architect (Primary / Sole Decision Architect) statement in the text of mission %s ('The Primary Architect has independently accepted B-W/B for this exact W-COMMIT only'). "
          "This executor did not review the source for B-W/B; it authenticated only the frozen identities." % MISSION]
    for role in ("E-W", "E-W7"):
        if role in pins:
            pj = pins[role]
            o6.append("%s: %s sha256 %s size %d; build receipt %s (descriptor sha256 %s, RECEIPT.txt sha256 %s): warnings %s errors %s; listings under receipts/%s (eight invocations)."
                      % (role, pj["executable"]["path"], pj["executable"]["sha256"], pj["executable"]["size"], pj["build"]["receipt_dir"], pj["build"]["descriptor_sha256"],
                         pj["build"]["receipt_sha256"], pj["build"]["warnings"], pj["build"]["errors"], ROLES[role]["list"]))
    o6.append("Listing inventories: all-tests listing sha256 %s; ignored listing sha256 %s (E-C's `21-list-ignored` raw sha256 %s)."
              % (info.get("E-W", {}).get("all_sha"), info.get("E-W", {}).get("ign_sha"), cfg["ec_ign_sha"]))
    o6.append("Four-way agreement: %s." % (json.dumps(fourway) if fourway else "NOT ESTABLISHED"))
    o6.append("Asserts nothing beyond NI-6 (build-time clause), NI-7, NI-8 and CR-B's conclusion for this one identity. It does not establish that W-SRC equals C-BASE, that any test passes, "
              "or anything about the witness. No cargo command was run after either executable was pinned.")
    o6.append("Limitation: the CARGO_TERM_VERBOSE=true variable of E-C's build environment (output only) is omitted from this mission's build environment, as 7.1 lists only CARGO_NET_OFFLINE and the target directory.")
    for l in o6:
        rec.log(l)
    if selfwrite:
        os.makedirs(os.path.join(evdir, "receipts/30-bwc"), exist_ok=True)
        rec.write(os.path.join(evdir, "receipts/30-bwc/B-W-C.txt"))
        with open(os.path.join(evdir, "receipts/30-bwc/B-W-C.json"), "x") as f:
            json.dump({"mission": MISSION, "verdict": verdict, "satisfied": ok, "failed_checks": sorted(set(rec.fails)), "identity": cfg["identity"],
                       "pins": {r: {"path": p["executable"]["path"], "sha256": p["executable"]["sha256"], "size": p["executable"]["size"]} for r, p in pins.items()},
                       "four_way": fourway, "filtered_out_expected": cfg["all_count"] - 1, "all_listing_sha256": info.get("E-W", {}).get("all_sha"),
                       "ignored_listing_sha256": info.get("E-W", {}).get("ign_sha"), "finished": now()}, f, indent=1)
    return 0 if ok else 1


# ----------------------------------------------------------------------------- self-test
def selftest(base):
    """Scratch-crate test: real c7sup builds/listings of a throwaway crate, then `check` with a synthetic configuration."""
    import shutil
    import subprocess
    here = os.path.dirname(os.path.abspath(__file__))
    src = os.path.abspath(os.path.join(here, "..", "listcrate"))
    results = []

    def expect(name, cond, det=""):
        results.append(bool(cond))
        print("[%s] %s %s" % ("OK" if cond else "FAILED", name, det), flush=True)
    # ---- parser
    fx = os.path.abspath(os.path.join(here, "..", "listfix"))
    rd = lambda n: open(os.path.join(fx, n), "rb").read()  # noqa
    p = parse_listing(rd("f1.txt"))
    expect("parser: real libtest --list (3 tests)", p["count"] == 3 and not p["problems"] and p["tests"][0] == "m::ignored_one", str(p["problems"]))
    p = parse_listing(rd("f2.txt"))
    expect("parser: real libtest --list --ignored (1 test, singular trailer)", p["count"] == 1 and not p["problems"])
    p = parse_listing(rd("f6.txt"))
    expect("parser: real libtest empty selection", p["count"] == 0 and p["tests"] == [] and not p["problems"])
    expect("render_listing reproduces real output byte for byte", all(render_listing(parse_listing(rd(n))["tests"]) == rd(n) for n in ("f1.txt", "f2.txt", "f3.txt", "f6.txt")))
    ec = load_ec(default_cfg())
    pa = parse_listing(ec["all_raw"])
    expect("parser: E-C 20-list-all (348 tests, sorted)", pa["count"] == 348 and not pa["problems"] and pa["tests"] == sorted(pa["tests"]), str(pa["problems"]))
    expect("render_listing reproduces E-C 20-list-all byte for byte", render_listing(pa["tests"]) == ec["all_raw"])
    pi = parse_listing(ec["ign_raw"])
    expect("parser: E-C 21-list-ignored (35 tests)", pi["count"] == 35 and not pi["problems"])
    expect("E-C listings hash to the frozen values", ec["all_sha"] == EC_LIST_ALL_SHA and ec["ign_sha"] == EC_LIST_IGN_SHA)
    bad = [b"", b"1 test, 0 benchmarks", b"a::b: test\n1 test, 0 benchmarks\n", b"a::b: test\n\n2 tests, 0 benchmarks\n", b"a::b: test\n\n1 tests, 0 benchmarks\n",
           b"a::b: test\na::b: test\n\n2 tests, 0 benchmarks\n", b"a::b: bench\n\n1 test, 0 benchmarks\n", b"junk\n0 tests, 0 benchmarks\n", b"a: test\n\n1 test, 1 benchmark\n"]
    expect("parser rejects 9 malformed listings", all(parse_listing(b)["problems"] for b in bad), str([bool(parse_listing(b)["problems"]) for b in bad]))
    # ---- scratch builds / listings / check
    os.makedirs(base)
    ev = os.path.join(base, "ev")
    os.makedirs(os.path.join(ev, "receipts"))
    wts = {}
    for role in ("E-W", "E-W7"):
        wt = os.path.join(base, "wt-" + role)
        shutil.copytree(src, wt, ignore=shutil.ignore_patterns("target"))
        os.makedirs(os.path.join(wt, "target"))
        body = "{ assert_eq!(std::hint::black_box(1u32) + 1, 2); }" if role == "E-W7" else "{}"
        open(os.path.join(wt, "src/lib.rs"), "w").write(
            "#[cfg(test)]\nmod m {\n    #[test]\n    fn ordinary_one() %s\n    #[test]\n    #[ignore]\n    fn ignored_one() {}\n"
            "    #[test]\n    #[ignore]\n    fn ignored_two() {}\n    mod inner {\n        #[test]\n        fn witness_like() {}\n    }\n}\n" % body)
        wts[role] = wt
    cfgb = default_cfg()
    cfgb.update(all_count=4, ign_count=2, witness="m::inner::witness_like", fpt="m::ignored_one", f3="m::ignored_two", verify_source=False,
                compiling_marker="Compiling listcrate ", crate_dir=".", wt=wts,
                build_argv=["cargo", "test", "--locked", "--release", "--lib", "--no-run", "-j", "2"], src_counts=lambda: (4, 2))
    cfgb["ec"] = None
    # synthetic E-C baseline: the 3 tests other than the witness (sorted), 2 ignored; the toolchain lines are the scratch toolchain's, like E-W's
    ecs = {"all_raw": render_listing(["m::ignored_one", "m::ignored_two", "m::ordinary_one"]), "ign_raw": render_listing(["m::ignored_one", "m::ignored_two"])}
    ecs["all_sha"], ecs["ign_sha"] = sha256_bytes(ecs["all_raw"]), sha256_bytes(ecs["ign_raw"])
    tv = subprocess.run(["env", "-i", "HOME=/home/nexus", "PATH=/home/nexus/.cargo/bin:/usr/local/bin:/usr/bin:/bin", "rustc", "-vV"], capture_output=True, text=True, cwd=src).stdout
    tc = subprocess.run(["env", "-i", "HOME=/home/nexus", "PATH=/home/nexus/.cargo/bin:/usr/local/bin:/usr/bin:/bin", "cargo", "-vV"], capture_output=True, text=True, cwd=src).stdout
    ecs.update(rustc_lines=tv.splitlines(), cargo_lines=tc.splitlines(), pin={"executable": "/nonexistent/ec"})
    cfgb["ec"] = ecs
    cfgb["ec_all_sha"], cfgb["ec_ign_sha"] = ecs["all_sha"], ecs["ign_sha"]
    cfgb["e_c_sha"] = "0" * 64
    cfgb["hist_sha"] = "1" * 64
    for role in ("E-W", "E-W7"):
        R = ROLES[role]
        wt = wts[role]
        bdir = os.path.join(ev, "receipts", R["build"])
        old = c7sup.BUILD_ARGV
        rc = c7sup.build(bdir, wt, role, argv=cfgb["build_argv"], verify_role=None, check_source=False, exe_prefix="listcrate")
        expect("scratch build %s" % role, rc == 0)
        expect("pin %s" % role, pin(ev, role, cfgb) == 0)
        expect("listings %s" % role, run_lists(ev, role, cfgb) == 0)
    # the scratch witness is `m::inner::witness_like`; both roles list the same 3 tests
    rc = check(ev, cfgb)
    expect("check: MECHANICALLY SATISFIED on the scratch data", rc == 0)
    expect("B-W-C record written", os.path.exists(os.path.join(ev, "receipts/30-bwc/B-W-C.json")))
    # ---- negative variants
    def variant(name, mutate, want_fail):
        d2 = os.path.join(base, "var-" + name)
        shutil.copytree(ev, d2, symlinks=True)
        shutil.rmtree(os.path.join(d2, "receipts/30-bwc"))
        mutate(d2)
        import io
        import contextlib
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            c = default_cfg()
            c.update(cfgb)
            rc2 = check(d2, c)
        out = buf.getvalue()
        failed = sorted({l.split(":")[0][7:] for l in out.splitlines() if l.startswith("[FAIL]")})
        expect("negative: %s -> BLOCKED; failed %s, wanted %s" % (name, failed, want_fail), rc2 == 1 and set(want_fail) <= set(failed) and set(failed) <= set(want_fail) | {"PIN-2"}, "")

    def edit_desc(dirrel, fn):
        def m(d2):
            p = os.path.join(d2, dirrel, "descriptor.json")
            j = jload(p)
            fn(j)
            json.dump(j, open(p, "w"), indent=1)
        return m

    def edit_raw(dirrel, newbytes):
        def m(d2):
            p = os.path.join(d2, dirrel, "raw-output.txt")
            open(p, "wb").write(newbytes)
        return m
    variant("a build warning", edit_desc("receipts/10-build-E-W", lambda j: j["compiler_messages"].update(warnings=1)), ["NI-7a", "PIN-2"])
    variant("a build error count", edit_desc("receipts/11-build-E-W7", lambda j: j["compiler_messages"].update(errors=2)), ["NI-7a", "PIN-2"])
    variant("a swap event", edit_desc("receipts/10-build-E-W", lambda j: j["supervisor"].update(swap_peak_bytes="4096")), ["PROC-4", "PIN-2"])
    variant("an OOM kill", edit_desc("receipts/11-build-E-W7", lambda j: j["supervisor"].update(oom_kill=1)), ["PROC-3", "PIN-2"])
    variant("a timed-out build", edit_desc("receipts/10-build-E-W", lambda j: j["supervisor"].update(timed_out=True)), ["PROC-2", "PIN-2"])
    variant("RUSTFLAGS in the build env", edit_desc("receipts/10-build-E-W", lambda j: j["env"].update(RUSTFLAGS="-Cdebuginfo=2")), ["NI-6b", "NI-6c", "PIN-2"])
    variant("a different rustc", lambda d2: open(os.path.join(d2, "receipts/10-build-E-W/toolchain-versions.json"), "w").write(
        json.dumps({"rustc -vV": "rustc 1.93.0\n", "cargo -vV": tc, "rustup show active-toolchain": ""})), ["NI-6d"])
    variant("a listing with an extra test", edit_raw("receipts/20-list-E-W/L1-list-all", render_listing(["m::ignored_one", "m::ignored_two", "m::inner::witness_like", "m::ordinary_one", "m::zz_extra"])),
            ["LST-3.L1-list-all", "NI-8.1", "NI-8.2", "AGREE-1.L1-list-all", "AGREE-2", "AGREE-3", "NI-8.3"])
    variant("the witness missing from the all-listing", edit_raw("receipts/21-list-E-W7/L1-list-all", render_listing(["m::ignored_one", "m::ignored_two", "m::ordinary_one"])), ["LST-3.L1-list-all", "NI-8.1", "NI-8.2", "NI-8.5", "AGREE-1.L1-list-all", "AGREE-2", "AGREE-3", "NI-8.3"])
    variant("an ignored listing that differs", edit_raw("receipts/20-list-E-W/L2-list-ignored", render_listing(["m::ignored_one", "m::ordinary_one"])), ["LST-3.L2-list-ignored", "NI-8.4", "NI-8.5", "AGREE-1.L2-list-ignored"])
    variant("an exact selection that resolves to nothing", edit_raw("receipts/20-list-E-W/L3-exact-witness", b"0 tests, 0 benchmarks\n"), ["LST-3.L3-exact-witness", "NI-8.6.L3-exact-witness", "AGREE-1.L3-exact-witness"])
    cfg_dis = dict(cfgb)
    d3 = os.path.join(base, "var-counts")
    shutil.copytree(ev, d3, symlinks=True)
    shutil.rmtree(os.path.join(d3, "receipts/30-bwc"))
    cfg_dis["src_counts"] = lambda: (5, 2)
    import io
    import contextlib
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        rc3 = check(d3, cfg_dis)
    expect("negative: source counts disagree -> BLOCKED with AGREE-2", rc3 == 1 and "[FAIL] AGREE-2" in buf.getvalue())
    d4 = os.path.join(base, "var-exe")
    shutil.copytree(ev, d4, symlinks=True)
    shutil.rmtree(os.path.join(d4, "receipts/30-bwc"))
    pj = jload(os.path.join(ev, "receipts/12-pin/E-W7.json"))
    with open(pj["executable"]["path"], "ab") as f:  # the scratch executable itself is changed after listing (the scratch copy only)
        f.write(b"\0")
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        rc4 = check(d4, cfgb)
    expect("negative: a changed executable after the listings -> BLOCKED with NI-8.0", rc4 == 1 and "[FAIL] NI-8.0" in buf.getvalue())
    print("\nSELF-TEST (c7bwc) %s: %d/%d" % ("ALL OK" if all(results) else "FAILED", sum(results), len(results)))
    return 0 if all(results) else 1


if __name__ == "__main__":
    mode = sys.argv[1]
    if mode == "selftest":
        sys.exit(selftest(os.path.abspath(sys.argv[2])))
    ev = os.path.abspath(sys.argv[2])
    if mode == "pin":
        sys.exit(pin(ev, sys.argv[3]))
    if mode == "list":
        sys.exit(run_lists(ev, sys.argv[3]))
    if mode == "check":
        sys.exit(check(ev))
    raise SystemExit("usage")
