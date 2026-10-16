#!/usr/bin/env python3
"""Self-test of final.py (Stage 12) and stagegate.py on SYNTHETIC evidence trees in a scratch directory (never the real evidence,
never the Nexus worktrees, never cargo). Written under P3-C11-DR01-D2A1-C7-TOOL-REPAIR-RESUME-03.

  final.py selftest <new-scratch-dir>

A generator simulates the 13.3 stage sequence the way cmd/run-stage.sh does (GATE and END lines in SEQUENCE.log, receipts with
their own UTC times) and produces the receipt layout of c7sup, c7wt, c7bwc, c7class and c7link. The receipts that need cargo or the
real worktrees (builds, listings, runs, worktree creation, patch, restoration) are synthetic files in the tools' own formats, with
fake executables in scratch worktrees; everything else is produced by the real tools: c7bwc.stage3, c7bwc.check, c7class.classify
and c7link.link. The authorisation-gate receipts are copies of the real passing EXEC-02 dry-run gate receipt. One positive tree must
classify as KILL_EVIDENCE_CANDIDATE; every negative tree must be rejected, with the check that names the defect among the failures.
"""
import contextlib
import io
import json
import os
import re
import shutil
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from c7lib import *  # noqa
import c7bwc as B
import c7class as K
import c7class_selftest as CS
import c7link as L
import c7sup
import final as F
import stagegate as SG

RESULTS = []
HERE = os.path.dirname(os.path.abspath(__file__))
DRYRUN_GATE = os.path.join(EV_DIR, "preflight", "gate-dryrun2.txt")   # the real passing EXEC-02 gate receipt (read only)
OKV = [{"id": "WT-X", "ok": True, "detail": "synthetic"}]
_last = [0]


def expect(name, cond, detail=""):
    RESULTS.append(bool(cond))
    print("[%s] %s %s" % ("OK" if cond else "FAILED", name, detail), flush=True)


def tick():
    t = time.time_ns()
    if t <= _last[0]:
        t = _last[0] + 1000
    _last[0] = t
    return time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(t // 10**9)) + ".%09dZ" % (t % 10**9)


def quiet(fn, *a, **k):
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        r = fn(*a, **k)
    return r, buf.getvalue()


def wr(path, data, mode=0o644):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data if isinstance(data, bytes) else data.encode("utf-8"))
    os.chmod(path, mode)
    return path


def wj(path, obj):
    return wr(path, json.dumps(obj, indent=1))


def sup(t0, t1, limit):
    return {"memory_max": "17179869184", "swap_max": "0", "memory_high": "max", "memory_peak_bytes": "2000000000", "swap_peak_bytes": "0",
            "oom": 0, "oom_kill": 0, "swap_events": {"high": 0, "max": 0, "fail": 0}, "completed": True, "timed_out": False, "wall_seconds": 12.5,
            "time_limit_seconds": limit, "scope_result": "gone", "scope_show": {}, "launcher_elapsed_seconds": 12.6, "max_rss_kib_largest_process": 1000,
            "started_utc": t0, "finished_utc": t1, "disk_floor_breach": False,
            "disk": {"free_before_bytes": 30 * 2**30, "min_free_bytes": 29 * 2**30, "free_after_bytes": 29 * 2**30, "floor_bytes": DISK_FLOOR_BYTES,
                     "peak_growth_bytes": 2**30, "net_growth_bytes": 2**30, "breach": False}}


def proc(code):
    return {"exit_status": code, "signal": None, "supervisor_rc": code, "wrapper_recorded_status": str(code)}


def side_files(d):
    for n in ("RECEIPT.txt", "scope-record.txt", "time-v.txt", "cgroup-samples.txt"):
        if not os.path.exists(os.path.join(d, n)):
            wr(os.path.join(d, n), "synthetic %s\n" % n)


class Fx:
    """One synthetic evidence tree produced by simulating the 13.3 stage sequence."""

    def __init__(self, base, name):
        self.root = os.path.join(base, name)
        self.ev = os.path.join(self.root, "ev")
        self.wt = {"H": os.path.join(self.root, "wt-healthy"), "M": os.path.join(self.root, "wt-mutant")}
        self.exe = {"E-W": os.path.join(self.wt["H"], "target/release/deps/nexus_governed_control-1111111111111111"),
                    "E-W7": os.path.join(self.wt["M"], "target/release/deps/nexus_governed_control-7777777777777777")}
        self.cfgb = B.default_cfg()
        self.cfgb["wt"] = {"E-W": self.wt["H"], "E-W7": self.wt["M"]}
        os.makedirs(os.path.join(self.ev, "receipts"))
        os.makedirs(os.path.join(self.ev, "gates"))
        shutil.copyfile(os.path.join(EV_DIR, "MISSION.txt"), os.path.join(self.ev, "MISSION.txt"))
        self.seq = os.path.join(self.ev, "SEQUENCE.log")
        self.rp = lambda *a: os.path.join(self.ev, "receipts", *a)  # noqa

    def cfg(self, manifest, live_ok=True):
        return {"bwc": self.cfgb, "wt": dict(self.wt), "tools_root": os.path.dirname(HERE), "tools_manifest": manifest,
                "live_verify": lambda wt, role: [("WT-STUB", live_ok, "synthetic live verification")]}

    def gate(self, stage):
        with open(self.seq, "a") as f:
            f.write("%s GATE %s PASS free=%d memavail_kib=%d\n" % (tick(), stage, 29 * 2**30, 50 * 2**20))
        wr(os.path.join(self.ev, "gates", stage + ".txt"), "# STAGE GATE before %s\n[PASS] T-1: synthetic\nSTAGE GATE %s: PASS\n" % (stage, stage))

    def end(self, stage, rc=0):
        with open(self.seq, "a") as f:
            f.write("%s END %s rc=%d\n" % (tick(), stage, rc))

    # ---- stages
    def s_auth(self, tag):
        t = open(DRYRUN_GATE, encoding="utf-8").read()
        t = re.sub(r"^# utc \S+$", "# utc " + tick(), t, count=1, flags=re.M)
        g = os.path.join(self.ev, "gates", "AUTH-%s.txt" % tag)
        wr(g, t)
        shutil.copyfile(DRYRUN_GATE + ".EC7.json", g + ".EC7.json")
        if tag == "S02":
            wr(os.path.join(self.ev, "gates/SUP-S02/SELFTEST.txt"), "SELF-TEST RESULT: ALL OK\n")
        tick()

    def s_create(self, role):
        t0 = tick()
        wtp = self.wt[role]
        wr(os.path.join(wtp, PROTOCOL_PATH), blob_at(C_BASE, PROTOCOL_PATH))
        wr(os.path.join(wtp, MODEL_TESTS_PATH), blob_at(W_COMMIT, MODEL_TESTS_PATH))
        os.makedirs(os.path.join(wtp, "target/release/deps"))
        wj(self.rp("00-worktrees", "creation-%s.json" % role), {"mission": MISSION, "role": role, "path": wtp, "started_utc": t0, "finished_utc": tick()})

    def s_build(self, role):
        R = B.ROLES[role]
        wtp = self.wt["H" if role == "E-W" else "M"]
        exe = self.exe[role]
        t0 = tick()
        wr(exe, ("#!synthetic %s executable\n" % role).encode(), 0o755)
        rel = os.path.relpath(exe, wtp)
        bdir = self.rp(R["build"])
        stderr = ("   Compiling nexus-governed-control v0.1.0 (%s/crates/nexus-governed-control)\n    Finished `release` profile [optimized] target(s) in 300.00s\n"
                  "  Executable unittests src/lib.rs (%s)\n" % (wtp, rel))
        wr(os.path.join(bdir, "build.stderr"), stderr)
        wr(os.path.join(bdir, "build.stdout"), "")
        ec = B.load_ec(B.default_cfg())
        wj(os.path.join(bdir, "toolchain-versions.json"), {"rustc -vV": "\n".join(ec["rustc_lines"]) + "\n", "cargo -vV": "\n".join(ec["cargo_lines"]) + "\n",
                                                           "rustup show active-toolchain": "1.94.0-x86_64-unknown-linux-gnu\n"})
        side_files(bdir)
        t1 = tick()
        wj(os.path.join(bdir, "descriptor.json"), {
            "id": R["build"], "type": "build", "role": role, "mission": MISSION, "worktree": wtp, "argv": c7sup.BUILD_ARGV, "jobs": 2,
            "env": {"HOME": "/home/nexus", "PATH": "/home/nexus/.cargo/bin:/usr/local/bin:/usr/bin:/bin", "CARGO_NET_OFFLINE": "true",
                    "CARGO_TARGET_DIR": os.path.join(wtp, "target")},
            "process": proc(0), "supervisor": sup(t0, t1, c7sup.BUILD_LIMIT), "stderr_path": "build.stderr", "stdout_path": "build.stdout",
            "build_stderr_sha256": sha256_bytes(stderr.encode()), "compiler_messages": {"warnings": 0, "errors": 0, "warning_lines": [], "error_lines": []},
            "compiling": ["Compiling nexus-governed-control v0.1.0 (%s/crates/nexus-governed-control)" % wtp],
            "finished": ["Finished `release` profile [optimized] target(s) in 300.00s"], "executable_lines": [rel], "executables_found": [exe],
            "pinned_executables": [{"path": exe, "sha256": sha256_file(exe), "size": os.path.getsize(exe)}], "inventory": [],
            "source_verification_before": OKV, "source_verification_after": OKV, "toolchain": {}, "finished_at": now()})

    def s_pin(self, role):
        R = B.ROLES[role]
        exe, bdir = self.exe[role], self.rp(R["build"])
        s_ = os.lstat(exe)
        wj(self.rp("12-pin", role + ".json"), {
            "role": role, "mission": MISSION, "pinned_at": now(), "worktree": self.cfgb["wt"][role], "source": {"commit": W_COMMIT, "verification": OKV},
            "executable": {"path": exe, "sha256": sha256_file(exe), "size": s_.st_size, "mode": oct(s_.st_mode & 0o7777), "mtime_ns": s_.st_mtime_ns, "inode": s_.st_ino},
            "build": {"receipt_dir": "receipts/" + R["build"], "descriptor_sha256": sha256_file(os.path.join(bdir, "descriptor.json")),
                      "receipt_sha256": sha256_file(os.path.join(bdir, "RECEIPT.txt")), "stderr_sha256": sha256_file(os.path.join(bdir, "build.stderr")),
                      "finished": [], "warnings": 0, "errors": 0, "wall_seconds": 12.5, "memory_peak_bytes": "2000000000"},
            "artifact_inventory": {"files": 0, "inventory": []},
            "fingerprints": [{"path": "target/release/.fingerprint/nexus-governed-control-0/test-lib-nexus_governed_control.json", "sha256": "0" * 64,
                              "rustflags": [], "profile": 1}], "statement": "synthetic"})

    def listing_raw(self, key):
        ec = B.load_ec(B.default_cfg())
        one = lambda n: ("%s: test\n\n1 test, 0 benchmarks\n" % n).encode()  # noqa
        return {"L1-list-all": B.render_listing(sorted(B.parse_listing(ec["all_raw"])["tests"] + [TEST_WITNESS])), "L2-list-ignored": ec["ign_raw"],
                "L3-exact-witness": one(TEST_WITNESS), "L4-exact-full_fpt": one(TEST_FULL_FPT), "L5-exact-full_f3": one(TEST_FULL_F3),
                "L6-exact-ignored-witness": b"0 tests, 0 benchmarks\n", "L7-exact-ignored-full_fpt": one(TEST_FULL_FPT),
                "L8-exact-ignored-full_f3": one(TEST_FULL_F3)}[key]

    def invocation(self, rdir, wtp, exe, args, raw, code, limit=2700, label="x"):
        t0 = tick()
        wr(os.path.join(rdir, "raw-output.txt"), raw)
        side_files(rdir)
        wj(os.path.join(rdir, "descriptor.json"), {
            "id": os.path.basename(rdir), "type": "invocation", "label": label, "mission": MISSION, "worktree": wtp,
            "cwd": os.path.join(wtp, c7sup.CRATE_DIR), "argv": [exe] + list(args), "env": dict(c7sup.RUN_ENV), "limit_seconds": limit,
            "executable": {"path": exe, "sha256_before": sha256_file(exe), "sha256_after": sha256_file(exe), "size": os.path.getsize(exe)},
            "process": proc(code), "supervisor": sup(t0, tick(), limit), "output_path": "raw-output.txt",
            "raw_output_sha256": sha256_bytes(raw if isinstance(raw, bytes) else raw.encode()),
            "source_verification_before": OKV, "source_verification_after": OKV, "finished_at": now()})

    def s_lists(self, role):
        R = B.ROLES[role]
        wtp = self.cfgb["wt"][role]
        for spec in B.LIST_SPECS:
            self.invocation(self.rp(R["list"], spec[0]), wtp, self.exe[role], B.list_args(spec, self.cfgb), self.listing_raw(spec[0]), 0, B.LIST_LIMIT)

    def s_apply(self):
        t0 = tick()
        pb = blob_at(K1_COMMITS[1], K1_PATCH_PATH)
        p = wr(self.rp("00-worktrees", "patch", "C7.patch"), pb)
        wr(self.rp("00-worktrees", "apply-C7.txt"), "APPLY: EXACTLY_W7 (synthetic)\n")
        wj(self.rp("00-worktrees", "apply-C7.json"), {
            "mission": MISSION, "worktree": self.wt["M"], "started_utc": t0, "finished_utc": tick(), "patch_file": p, "patch_sha256": sha256_bytes(pb),
            "patch_git_blob": git_blob_id(pb), "catalog_patch_sha256": PATCH_SHA, "apply_returncode": 0,
            "mutant_protocol": {"path": PROTOCOL_PATH, "git_blob": MUTANT_BLOB, "sha256": MUTANT_SHA, "bytes": 0},
            "expected_mutant": {"git_blob": MUTANT_BLOB, "sha256": MUTANT_SHA}, "verification_EXACTLY_W7": OKV, "ok": True})

    def run_raw(self, kind, f3=("FAIL", "E11")):
        if kind in ("C", "B"):
            return CS.assemble(CS.gen_witness(kind)), (0 if kind == "C" else 101)
        if kind == "A":
            return CS.gen_explorer("A", "FAIL", prop="E11")
        return CS.gen_explorer("F3", f3[0], prop=f3[1])

    def s_run(self, kind, d, text=None, code=None):
        role = "E-W" if kind == "C" else "E-W7"
        if text is None:
            text, code = self.run_raw(kind)
        self.invocation(self.rp(d), self.cfgb["wt"][role], self.exe[role], K.expected_args(kind), text, code, K.LIMIT, kind)
        self.reclassify(kind, d)

    def reclassify(self, kind, d):
        for n in ("CLASSIFICATION.txt", "CLASSIFICATION.json"):
            if os.path.exists(self.rp(d, n)):
                os.remove(self.rp(d, n))
        K.WT_HEALTHY, K.WT_MUTANT = self.wt["H"], self.wt["M"]
        return quiet(K.classify, kind, self.rp(d), self.rp("12-pin"))[0]

    def s_restore(self):
        t0 = tick()
        o = self.rp("50-restoration")
        pre = os.path.join(o, "pre-restore")
        for n in F.PRESERVE_FILES:
            wr(os.path.join(pre, n), "synthetic\n")
        wr(os.path.join(pre, "mutant-protocol.identity.txt"), "git blob id %s\nsha256 %s\nbytes 123\n" % (MUTANT_BLOB, MUTANT_SHA))
        rs = ["[PASS] R-%d: synthetic" % i for i in range(1, 9)]
        wr(os.path.join(o, "restoration.txt"), "\n".join(
            ["RESTORATION (frozen inverse patch) %s (UTC %s)" % (now(), t0)] + rs + ["7.3A authentication R-1..R-8: OK", "git apply -R --check rc=0 ",
             "git apply -R (one attempt, 60 s ceiling) rc=0 ", "[PASS] WT-HEAD: synthetic", "EXACTLY_W after restoration: OK", "RESTORATION COMPLETE: EXACTLY_W",
             "RESTORE: EXACTLY_W; finished UTC %s" % tick()]) + "\n")
        sha = sha256_file(self.exe["E-W7"])
        wr(os.path.join(o, "E-W7-sha256-after-restoration.txt"), "E-W7 %s sha256 after restoration: %s (pinned %s): MATCH\n" % (self.exe["E-W7"], sha, sha))
        wr(os.path.join(o, "healthy-EXACTLY_W-after.txt"), "[PASS] WT-HEAD: synthetic\nEXACTLY_W: OK\n")

    def generate(self, f3=("FAIL", "E11")):
        S = F.STAGES
        steps = [lambda: self.s_auth("S01"), lambda: self.s_auth("S02"), lambda: self.s_create("H"), lambda: self.s_build("E-W"),
                 lambda: self.s_pin("E-W"), lambda: self.s_lists("E-W"), lambda: self.must(quiet(B.stage3, self.ev, self.cfgb)[0], "stage3"),
                 lambda: self.s_create("M"), self.s_apply, lambda: self.s_build("E-W7"), lambda: self.s_pin("E-W7"), lambda: self.s_lists("E-W7"),
                 lambda: self.must(quiet(B.check, self.ev, self.cfgb)[0], "B-W/C"), lambda: self.s_run("C", F.RUNS[0][1]), lambda: self.s_run("A", F.RUNS[1][1]),
                 lambda: self.s_run("B", F.RUNS[2][1]), lambda: self.s_run("F3", F.RUNS[3][1], *self.run_raw("F3", f3)), self.s_restore,
                 lambda: self.must(quiet(L.link, self.rp("60-link"), self.wt["H"], self.wt["M"], self.rp("30-bwc", "B-W-C.json"), self.rp("12-pin"),
                                         os.path.join(self.ev, "MISSION.txt"))[0][0], "link")]
        for stage, fn in zip(S, steps):
            self.gate(stage)
            fn()
            self.end(stage)
        self.gate(F.FINAL_STAGE)
        return self

    @staticmethod
    def must(rc, what):
        if rc != 0:
            raise RuntimeError("fixture generation: %s returned %s" % (what, rc))

    # ---- edits used by the negative cases
    def edit_json(self, rel, fn):
        p = os.path.join(self.ev, rel)
        j = json.load(open(p))
        fn(j)
        json.dump(j, open(p, "w"), indent=1)

    def set_raw(self, d, text, code=None, reclass=False):
        wr(self.rp(d, "raw-output.txt"), text)
        self.edit_json("receipts/%s/descriptor.json" % d, lambda j: j.update(raw_output_sha256=sha256_bytes(text.encode())))
        if code is not None:
            self.edit_json("receipts/%s/descriptor.json" % d, lambda j: j.update(process=proc(code)))
        if reclass:
            self.reclassify([k for k, dd, _ in F.RUNS if dd == d][0], d)


def make_manifest(path, tamper=None):
    root = os.path.dirname(HERE)
    rows = []
    for f in sorted(os.listdir(HERE)):
        if f.endswith((".py", ".sh")):
            rows.append("%s  c7tools/%s" % (sha256_file(os.path.join(HERE, f)), f))
    if tamper:
        rows[0] = "%s  %s" % ("0" * 64, rows[0].split("  ", 1)[1])
    wr(path, "\n".join(rows) + "\n")
    return path, root


def main(base):
    base = os.path.abspath(base)
    os.makedirs(base)
    man, _ = make_manifest(os.path.join(base, "TOOLS.sha256"))
    bad_man, _ = make_manifest(os.path.join(base, "TOOLS-tampered.sha256"), tamper=True)
    n = [0]

    def fixture(f3=("FAIL", "E11")):
        n[0] += 1
        return Fx(base, "fx%02d" % n[0]).generate(f3)

    def final(fx, cfg=None):
        (rc, out, rec), _ = quiet(F.classify, fx.ev, cfg or fx.cfg(man))
        return rc, out

    def neg(name, mutate, want, cfg_fn=None):
        fx = fixture()
        mutate(fx)
        try:
            rc, out = final(fx, cfg_fn(fx) if cfg_fn else None)
        except Exception as e:  # noqa: BLE001
            expect("NEGATIVE %s: rejected without an exception" % name, False, repr(e))
            return
        failed = out["failed_checks"]
        ok = rc != 0 and out["result"] != F.CANDIDATE and out["kill_evidence_candidate"] is False and set(want) <= set(failed)
        expect("NEGATIVE %s -> %s" % (name, out["result"]), ok, "failed=%s want⊇%s" % (failed, want))

    # ---------------------------------------------------------------- consistency of the frozen lists with the real tools
    t = open(DRYRUN_GATE, encoding="utf-8").read()
    ids = {m.group(1) for m in re.finditer(r"^\[(?:PASS|FAIL)\] (\S+?): ", t, re.M)}
    expect("GATE_IDS equals the set of check ids of the real passing gate receipt (gate-dryrun2)", set(F.GATE_IDS) == ids and len(F.GATE_IDS) == len(set(F.GATE_IDS)),
           "list %d, receipt %d" % (len(F.GATE_IDS), len(ids)))
    expect("the real failing dry-run gate receipt (gate-dryrun1) would fail AUTH-1 (it has [FAIL] lines)",
           "[FAIL]" in open(os.path.join(EV_DIR, "preflight/gate-dryrun1.txt"), encoding="utf-8").read())

    # ---------------------------------------------------------------- positive
    fx = fixture()
    rc, out = final(fx)
    expect("POSITIVE: the complete synthetic 13.3 evidence tree is KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING", rc == 0 and out["result"] == F.CANDIDATE
           and out["failed_checks"] == [] and out["architect_review_items"] == [], "result=%s failed=%s" % (out["result"], out["failed_checks"]))
    expect("POSITIVE: FINAL-CLASSIFICATION.txt/json written; raw observations kept apart from admissibility; no acceptance written",
           os.path.isfile(fx.rp("70-final", "FINAL-CLASSIFICATION.txt")) and set(out["raw_observations"]) >= {"C", "A", "B", "F3", "builds", "listings"}
           and set(out["admissibility"]) == {"C", "A", "B", "F3"} and "ACCEPTED" not in open(fx.rp("70-final", "FINAL-CLASSIFICATION.txt")).read())
    expect("POSITIVE: (A) recorded as E11, never P6; P6 only from (B)", out["raw_observations"]["A"]["fail"][0] == "E11"
           and out["raw_observations"]["B"]["records"] == [K.E11_REC, K.P6_REC] and out["attribution"]["primary"].startswith("(A) E11"))
    try:
        quiet(F.classify, fx.ev, fx.cfg(man))
        expect("Stage 12 runs once: a second classification is refused", False)
    except SystemExit as e:
        expect("Stage 12 runs once: a second classification is refused", "REFUSED" in str(e))

    # ---------------------------------------------------------------- the mission's twelve required negative cases
    neg("1 missing mandatory receipt: (A)'s descriptor deleted", lambda fx: os.remove(fx.rp(F.RUNS[1][1], "descriptor.json")), ["IN-1"])
    neg("2 altered receipt sha256: one byte appended to (C)'s raw output", lambda fx: open(fx.rp(F.RUNS[0][1], "raw-output.txt"), "a").write("x\n"), ["RUN-C-2", "RUN-C-1"])

    def mutant_build_early(fx):
        l8 = json.load(open(fx.rp("20-list-E-W", "L8-exact-ignored-full_f3", "descriptor.json")))["supervisor"]
        fx.edit_json("receipts/11-build-E-W7/descriptor.json", lambda j: j["supervisor"].update(started_utc=l8["started_utc"], finished_utc=l8["finished_utc"]))
        fx.edit_json("receipts/12-pin/E-W7.json", lambda j: j["build"].update(descriptor_sha256=sha256_file(fx.rp("11-build-E-W7", "descriptor.json"))))
    neg("3 mutant build before the healthy listings completed (E-W7 build inside the E-W listing sequence)", mutant_build_early, ["BWC-1", "SEQ-3"])
    neg("4 missing one of the eight listings (E-W L5 removed)", lambda fx: shutil.rmtree(fx.rp("20-list-E-W", "L5-exact-full_f3")), ["IN-1", "BWC-1"])

    def wrong_inventory(fx):
        bad = B.render_listing(sorted(B.parse_listing(fx.listing_raw("L1-list-all"))["tests"] + ["ledger::zz_extra_test"]))
        for ld in ("20-list-E-W", "21-list-E-W7"):
            wr(fx.rp(ld, "L1-list-all", "raw-output.txt"), bad)
            fx.edit_json("receipts/%s/L1-list-all/descriptor.json" % ld, lambda j: j.update(raw_output_sha256=sha256_bytes(bad)))
    neg("5a incorrect test inventory: 350 tests in both all-listings", wrong_inventory, ["BWC-1"])
    neg("5b incorrect recorded four-way agreement (350 in the stored B-W/C record)",
        lambda fx: fx.edit_json("receipts/30-bwc/B-W-C.json", lambda j: j["four_way"].update(expected=[350, 35])), ["BWC-4"])
    neg("6a incorrect E11/P6 attribution: (A) printed P6 while its stored classification says FAIL_E11_ADMISSIBLE",
        lambda fx: fx.set_raw(F.RUNS[1][1], *CS.gen_explorer("A", "FAIL", prop="P6")), ["RUN-A-1", "RUN-A-3", "ATT-1"])

    def swap_loci(fx):
        p = CS.gen_witness("B")
        p["blocks"][12] = [l for l in p["blocks"][12] if l != K.E11_REC]
        p["blocks"][22].append(K.E11_REC.replace("step 13 transition", "step 23 transition"))
        fx.set_raw(F.RUNS[2][1], CS.assemble(p), 101, reclass=True)
    neg("6b incorrect attribution: (B) records E11 at step 23 instead of step 13 (re-classified, stored equal)", swap_loci, ["RUN-B-3", "ATT-2"])

    def oracle(fx):
        p = CS.gen_witness("B")
        p["blocks"][7][0] = p["blocks"][7][0].replace("offered [0, 0, 0, 0]", "offered [0, 0, 0, 1]")
        fx.set_raw(F.RUNS[2][1], CS.assemble(p), 101, reclass=True)
    neg("7a incorrect Oracle accounting: (B) step 08 offered stream differs from (C)'s (G.9 rule 1)", oracle, ["RUN-B-3", "G9X-1"])

    def oracle2(fx):
        p = CS.gen_witness("B")
        p["blocks"][22][0] = p["blocks"][22][0].replace("consumed [(0, 4), (0, 4), (0, 4), (0, 3)] unused []", "consumed [(0, 4), (0, 4), (0, 3)] unused [0]")
        fx.set_raw(F.RUNS[2][1], CS.assemble(p), 101, reclass=True)
    neg("7b incorrect Oracle accounting: (B) step 23 consumed like the healthy end while showing the changed branch", oracle2, ["RUN-B-3"])
    neg("8a failed restoration: `RESTORATION FAILED` / `RESTORE: NOT EXACTLY_W`",
        lambda fx: wr(fx.rp("50-restoration", "restoration.txt"), open(fx.rp("50-restoration", "restoration.txt")).read()
                      .replace("RESTORATION COMPLETE: EXACTLY_W", "RESTORATION FAILED").replace("RESTORE: EXACTLY_W;", "RESTORE: NOT EXACTLY_W;")), ["RST-1", "LNK-2"])
    neg("8b absent restoration record", lambda fx: os.remove(fx.rp("50-restoration", "restoration.txt")), ["IN-1", "RST-0", "LNK-2"])
    neg("8c restoration not authenticated: R-5 failed", lambda fx: wr(fx.rp("50-restoration", "restoration.txt"),
                                                                    open(fx.rp("50-restoration", "restoration.txt")).read().replace("[PASS] R-5:", "[FAIL] R-5:")), ["RST-1"])
    neg("8d live worktree not EXACTLY_W after restoration", lambda fx: None, ["OBJ-1"], cfg_fn=lambda fx: fx.cfg(man, live_ok=False))
    neg("9a incorrect linked baseline identity: 44-full-F-PT raw sha256 differs",
        lambda fx: fx.edit_json("receipts/60-link/LINK.json", lambda j: j["receipts"]["44-full-F-PT"].update(raw_sha256="0" * 64)), ["LNK-3"])
    neg("9b incorrect linked baseline identity: E-C executable differs",
        lambda fx: fx.edit_json("receipts/60-link/LINK.json", lambda j: j["receipts"]["43-full-F-3"].update(executable_sha256="1" * 64)), ["LNK-3"])

    def res_breach(fx):
        fx.edit_json("receipts/%s/descriptor.json" % F.RUNS[1][1], lambda j: j["supervisor"].update(swap_peak_bytes="4096"))
        fx.reclassify("A", F.RUNS[1][1])
    neg("10a resource-limit breach: (A) used swap (re-classified, stored equal)", res_breach, ["RES-1", "RUN-A-3"])
    neg("10b resource-limit breach: the E-W build's memory peak reached the 16 GiB cap",
        lambda fx: fx.edit_json("receipts/10-build-E-W/descriptor.json", lambda j: j["supervisor"].update(memory_peak_bytes="17179869184")), ["RES-1", "BWC-1"])
    neg("10c resource-limit breach: a listing breached the disk floor",
        lambda fx: fx.edit_json("receipts/21-list-E-W7/L3-exact-witness/descriptor.json", lambda j: j["supervisor"].update(disk_floor_breach=True)), ["RES-1", "BWC-1"])

    def wrong_owner(fx):
        g = os.path.join(fx.ev, "gates/AUTH-S01.txt")
        wr(g, open(g).read().replace("K4 %s/%s/%s" % (K4_COMMIT, K4_BLOB, K4_SHA), "K4 %s/%s/%s" % (HIST_K4_COMMIT, HIST_K4_BLOB, HIST_K4_SHA)))
    neg("11a wrong Owner approval identity: the gate authenticated the historical K4 record", wrong_owner, ["AUTH-3.S01"])
    neg("11b wrong Owner approval identity: the mission text differs from the one the gates authenticated",
        lambda fx: open(os.path.join(fx.ev, "MISSION.txt"), "a").write("\nAPPENDED\n"), ["AUTH-5.S01", "AUTH-5.S02", "LNK-4"])

    def drop_last(fx):
        lines = open(fx.seq).read().splitlines(True)
        open(fx.seq, "w").write("".join(lines[:-1]))
    neg("12a incomplete Stage 12 input: the S12 gate line is missing", drop_last, ["SEQ-1"])
    neg("12b incomplete Stage 12 input: (B)'s classification was never written", lambda fx: os.remove(fx.rp(F.RUNS[2][1], "CLASSIFICATION.json")), ["IN-1", "RUN-B-0"])
    neg("12c incomplete Stage 12 input: no B-W/C record", lambda fx: shutil.rmtree(fx.rp("30-bwc")), ["IN-1", "BWC-0"])

    # ---------------------------------------------------------------- further fail-closed cases
    def runs_swapped(fx):
        fx.edit_json("receipts/%s/descriptor.json" % F.RUNS[1][1], lambda j: j["supervisor"].update(
            started_utc=json.load(open(fx.rp(F.RUNS[0][1], "descriptor.json")))["supervisor"]["started_utc"]))
    neg("13 run order: (A) started before (C) finished", runs_swapped, ["SEQ-3", "SEQ-4"])
    def c_violation(fx):
        p = CS.gen_witness("C")
        p["blocks"][12].append(K.E11_REC)
        fx.set_raw(F.RUNS[0][1], CS.assemble(p), 0, reclass=True)
    neg("14 the healthy witness was not PASS: a violation in (C) (re-classified, stored equal)", c_violation, ["RUN-C-3", "ATT-3"])
    neg("15 (A) INCOMPLETE is never a PASS or a kill (re-classified, stored equal)",
        lambda fx: fx.set_raw(F.RUNS[1][1], *CS.gen_explorer("A", "INCOMPLETE", incomplete="the state cap of 30000000 was reached"), reclass=True), ["RUN-A-3", "ATT-1"])
    neg("16 (A) printed another property (E4) (re-classified, stored equal)",
        lambda fx: fx.set_raw(F.RUNS[1][1], *CS.gen_explorer("A", "FAIL", prop="E4"), reclass=True), ["RUN-A-3", "ATT-1"])
    neg("17 E-W7 changed after the pin (live executable differs)", lambda fx: open(fx.exe["E-W7"], "ab").write(b"\0"), ["EXE-3"])
    neg("18 (B) ran on a different executable path than (A)",
        lambda fx: fx.edit_json("receipts/%s/descriptor.json" % F.RUNS[2][1], lambda j: j["executable"].update(path="/other/exe")), ["EXE-1"])
    neg("19 Stage-3 gate taken after the mutant worktree existed",
        lambda fx: fx.edit_json("receipts/25-stage3-gate/STAGE3-E-W.json", lambda j: j.update(mutant_worktree_existed=True)), ["BWC-5", "BWC-1"])
    neg("20 the stored B-W/C record is not satisfied", lambda fx: fx.edit_json("receipts/30-bwc/B-W-C.json", lambda j: j.update(satisfied=False)), ["BWC-2"])
    neg("21 a gate receipt with a failing check", lambda fx: wr(os.path.join(fx.ev, "gates/AUTH-S02.txt"),
                                                              open(os.path.join(fx.ev, "gates/AUTH-S02.txt")).read().replace("[PASS] RES-2:", "[FAIL] RES-2:")),
        ["AUTH-1.S02", "AUTH-2.S02"])
    neg("22 a stage gate record that failed", lambda fx: wr(os.path.join(fx.ev, "gates/S07-run-A.txt"), "STAGE GATE S07-run-A: FAIL (R-2)\n"), ["SEQ-2"])
    neg("23 the frozen tool manifest does not match", lambda fx: None, ["TOOL-1"], cfg_fn=lambda fx: fx.cfg(bad_man))
    neg("24 the stored link is not usable", lambda fx: fx.edit_json("receipts/60-link/LINK.json", lambda j: j.update(usable=False)), ["LNK-1"])
    neg("25 restoration began from a protocol.rs that was not the authenticated mutant",
        lambda fx: wr(fx.rp("50-restoration", "pre-restore", "mutant-protocol.identity.txt"), "git blob id %s\nsha256 %s\nbytes 1\n" % (PROTOCOL_C_BLOB, PROTOCOL_C_SHA)), ["RST-2"])
    neg("26 E-W7 after restoration did not match its pin", lambda fx: wr(fx.rp("50-restoration", "E-W7-sha256-after-restoration.txt"),
                                                                       "E-W7 x sha256 after restoration: %s (pinned %s): MISMATCH\n" % ("1" * 64, "2" * 64)), ["RST-3"])
    neg("27 a SEQUENCE.log with a stage out of order (S07 before S06)", lambda fx: open(fx.seq, "w").write(
        "".join(sorted(open(fx.seq).read().splitlines(True), key=lambda l: (0 if " S07-run-A " in l else 1, l)))), ["SEQ-1"])

    # ---------------------------------------------------------------- section 10 (F-3)
    fxp = fixture(f3=("PASS", None))
    rc, out = final(fxp)
    expect("F-3 PASS (completed, not E11): still a candidate, listed for the Architect's review (section 10)",
           rc == 0 and out["result"] == F.CANDIDATE and len(out["architect_review_items"]) == 1 and "INFORMATIONAL F-3: PASS" in out["architect_review_items"][0],
           "%s %s" % (out["result"], out["architect_review_items"]))
    fxn = fixture()
    fxn.edit_json("receipts/%s/descriptor.json" % F.RUNS[3][1], lambda j: j["process"].update(signal=9, exit_status=None, supervisor_rc=-9, wrapper_recorded_status=None))
    fxn.reclassify("F3", F.RUNS[3][1])
    rc, out = final(fxn)
    expect("F-3 NOT COMPLETED (killed by a signal): MUTATION_CONTROL_BLOCKED — INFORMATIONAL F-3 NOT COMPLETED, never a candidate",
           rc != 0 and out["result"] == F.F3_BLOCKED, "%s %s" % (out["result"], out["failed_checks"]))
    fxt = fixture()
    fxt.edit_json("receipts/%s/descriptor.json" % F.RUNS[3][1], lambda j: j["supervisor"].update(timed_out=True))
    fxt.reclassify("F3", F.RUNS[3][1])
    rc, out = final(fxt)
    expect("F-3 NOT COMPLETED (supervisor timeout): MUTATION_CONTROL_BLOCKED — INFORMATIONAL F-3 NOT COMPLETED, never a candidate",
           rc != 0 and out["result"] == F.F3_BLOCKED and "RES-1" in out["failed_checks"], "%s %s" % (out["result"], out["failed_checks"]))
    fxu = fixture()
    fxu.edit_json("receipts/%s/descriptor.json" % F.RUNS[3][1], lambda j: j["supervisor"].update(timed_out=True))
    fxu.reclassify("F3", F.RUNS[3][1])
    fxu.edit_json("receipts/%s/descriptor.json" % F.RUNS[1][1], lambda j: j["supervisor"].update(oom_kill=1))
    rc, out = final(fxu)
    expect("F-3 NOT COMPLETED together with another defect ((A) OOM): NOT A KILL EVIDENCE CANDIDATE (the F-3 label is not used to hide it)",
           rc != 0 and out["result"] == F.NOT_CANDIDATE, "%s %s" % (out["result"], out["failed_checks"]))

    # ---------------------------------------------------------------- precondition (the stage runner's sequence gate)
    pre = lambda ev, s: quiet(F.precondition, ev, s)[0]  # noqa
    e0 = os.path.join(base, "pre-empty")
    os.makedirs(e0)
    expect("precondition: S01 allowed on an empty evidence directory; S02 refused", pre(e0, "S01-authorisation-gate") == 0 and pre(e0, "S02-preflight-repeat") == 1)
    fxq = fixture()
    expect("precondition: after the whole sequence, S12 (already started) and S11 are refused", pre(fxq.ev, "S12-final") == 1 and pre(fxq.ev, "S11-link") == 1)
    lines = open(fxq.seq).read().splitlines(True)

    def upto(stage, extra=""):
        k = [i for i, l in enumerate(lines) if (" END %s " % stage) in l][0]
        open(fxq.seq, "w").write("".join(lines[:k + 1]) + extra)
    upto("S05-bwc")
    expect("precondition: after S05 the next stage S06 is allowed; S07 and S10 are refused",
           pre(fxq.ev, "S06-run-C") == 0 and pre(fxq.ev, "S07-run-A") == 1 and pre(fxq.ev, "S10-restore") == 1)
    k6 = [l for l in lines if " GATE S06-run-C " in l][0]
    upto("S05-bwc", k6 + "%s END S06-run-C rc=1\n" % tick())
    expect("precondition: a stop at S06 (rc 1): S07 refused, S10-restore allowed on the 7.3A stop path, S11 refused",
           pre(fxq.ev, "S07-run-A") == 1 and pre(fxq.ev, "S10-restore") == 0 and pre(fxq.ev, "S11-link") == 1)
    upto("S06-run-C", "%s GATE S07-run-A FAIL free=1 memavail_kib=1\n" % tick())
    expect("precondition: a failed stage gate at S07: S10-restore allowed (stop path), S08 refused", pre(fxq.ev, "S10-restore") == 0 and pre(fxq.ev, "S08-run-B") == 1)
    upto("S03c-pin-E-W", [l for l in lines if " GATE S03d-list-E-W " in l][0] + "%s END S03d-list-E-W rc=1\n" % tick())
    os.rename(fxq.rp("00-worktrees", "apply-C7.json"), fxq.rp("00-worktrees", "apply-C7.json.moved"))
    expect("precondition: a stop before the mutant patch (S03d): S10-restore refused (nothing to restore), S03e refused",
           pre(fxq.ev, "S10-restore") == 1 and pre(fxq.ev, "S03e-stage3-gate") == 1)
    open(fxq.seq, "w").write("".join(lines[:4]) + "garbage line\n")
    expect("precondition: a malformed SEQUENCE.log refuses every stage", pre(fxq.ev, "S03a-create-H") == 1)
    open(fxq.seq, "w").write("".join(lines[:4]))
    expect("precondition: the clean prefix S01..S02 allows S03a; an unknown stage is refused", pre(fxq.ev, "S03a-create-H") == 0 and pre(fxq.ev, "S99-bogus") == 1)

    # ---------------------------------------------------------------- stagegate.py (scratch evidence directory and scratch tool copy)
    sg = os.path.join(base, "stagegate")
    tools = os.path.join(sg, "tools")
    os.makedirs(os.path.join(tools, "c7tools"))
    os.makedirs(os.path.join(sg, "ev"))
    for f in os.listdir(HERE):
        if f.endswith((".py", ".sh")):
            shutil.copy2(os.path.join(HERE, f), os.path.join(tools, "c7tools", f))
    rows = ["%s  c7tools/%s" % (sha256_file(os.path.join(tools, "c7tools", f)), f) for f in sorted(os.listdir(os.path.join(tools, "c7tools")))]
    wr(os.path.join(tools, "TOOLS.sha256"), "\n".join(rows) + "\n")
    old = (SG.EV_DIR, SG.HERE)
    SG.EV_DIR, SG.HERE = os.path.join(sg, "ev"), os.path.join(tools, "c7tools")
    try:
        rc, txt = quiet(SG.main, "S01-authorisation-gate", 0)
        ents, probs = F.parse_sequence(os.path.join(sg, "ev"))
        expect("stagegate: PASS with zero projected growth; its SEQUENCE.log line parses as one GATE PASS", rc == 0 and not probs and len(ents) == 1
               and ents[0]["kind"] == "GATE" and ents[0]["ok"], "rc=%s %s %s" % (rc, probs, [l for l in txt.splitlines() if l.startswith("[FAIL]")]))
        try:
            quiet(SG.main, "S01-authorisation-gate", 0)
            expect("stagegate: a second gate for the same stage is refused", False)
        except SystemExit as e:
            expect("stagegate: a second gate for the same stage is refused", "REFUSED" in str(e))
        rc, txt = quiet(SG.main, "S02-preflight-repeat", 10**15)
        expect("stagegate: projected growth beyond the 16 GiB floor -> FAIL R-2", rc == 1 and "[FAIL] R-2" in txt)
        open(os.path.join(tools, "c7tools", "final.py"), "a").write("# tampered\n")
        rc, txt = quiet(SG.main, "S03a-create-H", 0)
        expect("stagegate: a changed frozen tool -> FAIL T-1", rc == 1 and "[FAIL] T-1" in txt)
        wr(os.path.join(tools, "c7tools", "extra.py"), "pass\n")
        rc, txt = quiet(SG.main, "S03b-build-E-W", 0)
        expect("stagegate: an unlisted tool file -> FAIL T-2", rc == 1 and "[FAIL] T-2" in txt)
        ents, probs = F.parse_sequence(os.path.join(sg, "ev"))
        expect("stagegate: FAIL gate lines parse as GATE FAIL", not probs and [e["ok"] for e in ents] == [True, False, False, False])
    finally:
        SG.EV_DIR, SG.HERE = old
    expect("the self-test never wrote the mutant blob to the object store", git("cat-file", "-e", MUTANT_BLOB, check=False).returncode != 0)
    print("\nSELF-TEST (final + stagegate) %s: %d/%d" % ("ALL OK" if all(RESULTS) else "FAILED", sum(RESULTS), len(RESULTS)))
    return 0 if all(RESULTS) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
