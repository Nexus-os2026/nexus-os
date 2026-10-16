#!/usr/bin/env python3
"""Stage 12 (13.3 step 13): independent mechanical FINAL classification for P3-C11-DR01-D2A1-C7-COMPLIANT-EXEC-02 (new in EXEC-02).

  final.py <evdir>

It re-derives everything from this mission's receipts and from git objects, never from a previous verdict alone:
  1. re-classifies the four raw outputs in memory with c7class (CL-1..CL-12, C7-1..C7-21, F3-1..F3-4) and requires agreement
     with the stored CLASSIFICATION.json of each receipt;
  2. re-judges B-W/C in memory with c7bwc (NI-6 build-time clause, NI-7, NI-8, four-way agreement, 13.3 order);
  3. cross-checks executable identities (one E-W7 for (A), (B) and F-3; E-W only for (C); stale-artifact inequalities);
  4. cross-checks G.9 between (C) and (B): the per-step lines and branch lists are identical except at step 23;
  5. checks the run order (C) < (A) < (B) < F-3 < restoration < link, all after B-W/C, from UTC receipt times;
  6. checks restoration (EXACTLY_W, E-W7 unchanged), both worktrees EXACTLY_W now, the (D) link, and the object store;
  7. writes receipts/70-final/FINAL-CLASSIFICATION.{txt,json}, keeping RAW OBSERVATIONS apart from ADMISSIBILITY.
The maximum result is KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING. This tool never writes an acceptance.
"""
import contextlib
import io
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from c7lib import *  # noqa
import c7bwc as B
import c7class as K
import c7link as L
import c7wt as W

RUNS = [("C", "40-run-C-healthy-witness"), ("A", "41-run-A-full_fpt"), ("B", "42-run-B-mutant-witness"), ("F3", "43-run-F3-informational")]
GOOD = {"C": "PASS_ADMISSIBLE", "A": "FAIL_E11_ADMISSIBLE", "B": "ADMISSIBLE_TWO_CANONICAL"}


def jl(p):
    with open(p, encoding="utf-8") as f:
        return json.load(f)


def quiet(fn, *a, **k):
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        r = fn(*a, **k)
    return r, buf.getvalue()


def step_lines(raw_path):
    lines = open(raw_path, encoding="utf-8").read().split("\n")
    out = {}
    for l in lines:
        m = re.search(r"C7 WITNESS F-PT step (\d\d)/24 .*$", l)
        if m:
            out.setdefault(int(m.group(1)), []).append(m.group(0))
    br = {}
    for l in lines:
        m = re.match(r"^    branches (\[.*\]); word after", l)
        if m:
            br.setdefault(len(br) + 1, m.group(1))
    return out, br


def main(evdir):
    rec = Rec("STAGE 12 FINAL INDEPENDENT CLASSIFICATION (13.3 step 13)")
    rec.log("# utc %s" % utcns())
    rp = lambda *a: os.path.join(evdir, "receipts", *a)  # noqa
    pins = {r: jl(rp("12-pin", r + ".json")) for r in ("E-W", "E-W7")}
    pw, p7 = pins["E-W"]["executable"], pins["E-W7"]["executable"]
    raw_obs, adm = {}, {}

    rec.log("\n## 1. the four runs, re-classified in memory from raw output (never from the stored verdict alone)")
    present = {k: os.path.isdir(rp(d)) for k, d in RUNS}
    for kind, d in RUNS:
        if not present[kind]:
            rec.check("RUN-%s" % kind, "receipt %s exists" % d, False, "missing")
            continue
        stored = jl(rp(d, "CLASSIFICATION.json"))
        pins_dir = rp("12-pin")
        fn = K.classify_witness if kind in ("C", "B") else K.classify_explorer
        (ck, verdict), _ = quiet(fn, kind, rp(d), K.load_pins(pins_dir))
        rec.check("RUN-%s-1" % kind, "%s: the in-memory re-classification equals the stored one (%s)" % (d, stored.get("verdict")), verdict == stored.get("verdict") and
                  sorted(ck.fails) == sorted(stored.get("failed_checks", [])), "now %s %s" % (verdict, ck.fails))
        desc = jl(rp(d, "descriptor.json"))
        rec.check("RUN-%s-2" % kind, "%s: raw output still matches the descriptor sha256" % d, sha256_file(rp(d, "raw-output.txt")) == desc["raw_output_sha256"])
        if kind in GOOD:
            rec.check("RUN-%s-3" % kind, "%s: verdict %s" % (d, GOOD[kind]), verdict == GOOD[kind], verdict)
        else:
            rec.check("RUN-F3-3", "%s: informational F-3 is a COMPLETED, admissible receipt (any completed outcome; F3-1..F3-4)" % d, verdict.startswith("COMPLETED"), verdict)
        f = ck.facts
        raw_obs[kind] = {"exit_status": desc["process"]["exit_status"], "signal": desc["process"]["signal"], "wall_seconds": desc["supervisor"]["wall_seconds"],
                         "memory_peak_bytes": desc["supervisor"]["memory_peak_bytes"], "swap_peak_bytes": desc["supervisor"]["swap_peak_bytes"],
                         "oom_kill": desc["supervisor"]["oom_kill"], "disk_min_free_bytes": desc["supervisor"]["disk"]["min_free_bytes"],
                         "started_utc": desc["supervisor"]["started_utc"], "finished_utc": desc["supervisor"]["finished_utc"],
                         "raw_output_sha256": desc["raw_output_sha256"], "executable_sha256": desc["executable"]["sha256_before"]}
        for key in ("outcome", "states", "elapsed", "fail", "incomplete", "records", "step23_end", "kept", "lost", "f3_text", "bounds_line"):
            if key in f:
                raw_obs[kind][key] = f[key]
        adm[kind] = verdict

    rec.log("\n## 2. B-W/C re-judged in memory (this mission's own builds and listings)")
    rc, txt = quiet(B.check, evdir, None, False)
    stored = jl(rp("30-bwc", "B-W-C.json"))
    rec.check("BWC-1", "B-W/C re-judged now: MECHANICALLY SATISFIED, equal to the stored record", rc == 0 and stored.get("satisfied") is True, "rc=%d stored=%s" % (rc, stored.get("satisfied")))
    s3 = jl(rp("25-stage3-gate", "STAGE3-E-W.json"))
    rec.check("BWC-2", "the Stage-3 hard order gate judged E-W admissible before the mutant worktree existed", s3.get("admissible") is True and s3.get("mutant_worktree_existed") is False)

    rec.log("\n## 3. executable identities (CL-10, C7-14, C7-16, 0A)")
    ex = {k: jl(rp(d, "descriptor.json"))["executable"] for k, d in RUNS if present[k]}
    if len(ex) ==