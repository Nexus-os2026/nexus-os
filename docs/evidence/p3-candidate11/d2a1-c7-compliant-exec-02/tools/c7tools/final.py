#!/usr/bin/env python3
"""Stage 12 (13.3 step 13): independent mechanical FINAL classification for P3-C11-DR01-D2A1-C7-COMPLIANT-EXEC-02.
New in EXEC-02, where it was left incomplete (a truncated file that does not parse; preserved read-only under
resume-03/snapshot-initial/). Completed under P3-C11-DR01-D2A1-C7-TOOL-REPAIR-RESUME-03 as a small deterministic ORCHESTRATOR
of the existing tested checkers (c7class, c7bwc, c7link, c7wt); it reimplements none of them.

  final.py classify <evdir>               Stage 12: writes receipts/70-final/FINAL-CLASSIFICATION.{txt,json} (exclusive); exit 0
                                          only for KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING
  final.py precondition <evdir> <stage>   read-only, used by cmd/run-stage.sh before every stage: exit 0 only when <stage> may
                                          start now (the exact 13.3 stage sequence; the 7.3A stop path for S10-restore only)
  final.py selftest <new-scratch-dir>     synthetic evidence trees only (final_selftest.py); never the real evidence

Every section fails closed: a missing, unreadable, contradictory or unexpected input is a FAIL, never a skip, and a section that
raises is a FAIL. Raw observations are kept apart from admissibility. Nothing is inferred from an exit status alone; nothing is
rerun; E11 is never converted to P6; an incomplete exploration is never a PASS.
  IN    every mandatory receipt exists (the complete Stage 12 input)
  SEQ   SEQUENCE.log is exactly the 13.3 stage sequence S01..S11 completed with rc 0, then S12's gate; every receipt's own UTC
        times lie inside its stage's GATE..END window; B-W/C < (C) < (A) < (B) < F-3 < restoration < link
  AUTH  both Stage 1-2 authorisation gate receipts PASS with every mandatory check; K1..K5/EC7 identities as pinned; the mission
        text unchanged since the gates; K1, K2, K4, W-SRC and C-BASE re-read from git objects now
  BWC   B-W/C re-judged in memory by c7bwc.check (NI-6 build-time clause, NI-7, NI-8, 6A four-way agreement, ORD-1..3) and equal
        to the stored record; the Stage-3 hard order gate record
  RUN   the four runs re-classified in memory by c7class from raw output (CL-1..CL-12, C7-1..C7-21, F3-1..F3-4) and equal to the
        stored classifications; raw output still matches its descriptor
  EXE   one E-W7 for (A), (B) and F-3, E-W only for (C), the 0A inequalities, both executables unchanged since the pin (7.4)
  G9X   G.9 rules 1 to 4 compared ACROSS (C) and (B)
  ATT   (A) prints E11 and is recorded as E11; P6 only from (B), with E11 recorded separately; (C) records nothing
  RES   every supervised build, listing and run within 16 GiB, zero swap, no OOM, no timeout, disk floor kept (c7bwc.proc_checks)
  RST   restoration EXACTLY_W after R-1..R-8, one inverse-patch attempt, evidence preserved first, E-W7 unchanged (I-C7.6, 7.3A)
  LNK   the (D) link re-made in memory by c7link and equal to the stored one; the frozen Candidate C identities
  F3    section 10 recording (informational; never a kill; NOT COMPLETED blocks the candidate)
  OBJ   live state: both worktrees EXACTLY_W, mutant blob absent, primary checkout, destination ref, historical evidence
  TOOL  the frozen tools still hash to TOOLS.sha256
The maximum result is KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING. This tool never writes an acceptance.
"""
import contextlib
import io
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from c7lib import *  # noqa
import c7bwc as B
import c7class as K
import c7link as L
import c7sup
import c7wt as W

HERE = os.path.dirname(os.path.abspath(__file__))

STAGES = ["S01-authorisation-gate", "S02-preflight-repeat",
          "S03a-create-H", "S03b-build-E-W", "S03c-pin-E-W", "S03d-list-E-W", "S03e-stage3-gate",
          "S04a-create-M", "S04b-apply-C7", "S04c-build-E-W7", "S04d-pin-E-W7", "S04e-list-E-W7",
          "S05-bwc", "S06-run-C", "S07-run-A", "S08-run-B", "S09-run-F3", "S10-restore", "S11-link", "S12-final"]
RESTORE_STAGE, FIRST_MUTANT_STAGE, LAST_RUN_STAGE, FINAL_STAGE = "S10-restore", "S04b-apply-C7", "S09-run-F3", "S12-final"
RUNS = [("C", "40-run-C-healthy-witness", "S06-run-C"), ("A", "41-run-A-full_fpt", "S07-run-A"),
        ("B", "42-run-B-mutant-witness", "S08-run-B"), ("F3", "43-run-F3-informational", "S09-run-F3")]
GOOD = {"C": "PASS_ADMISSIBLE", "A": "FAIL_E11_ADMISSIBLE", "B": "ADMISSIBLE_TWO_CANONICAL"}
AUTH = [("S01-authorisation-gate", "gates/AUTH-S01.txt"), ("S02-preflight-repeat", "gates/AUTH-S02.txt")]
# Every check id that gate.py records; each must appear as [PASS] in both gate receipts (final_selftest proves this list equals
# the ids of a real passing gate receipt).
GATE_IDS = ["MIS-1", "GP-01", "GP-02", "GP-02b", "GP-03", "GP-03L", "GP-04", "GP-04L", "K1", "K1-C7", "K1-base", "K1-C7b",
            "K2-commit", "K2", "K2-reach", "K2-k4same", "K3-X1", "K3-X2", "K3-X3", "K3", "OVL-1", "P-2", "P-1set", "P-1add",
            "P-1append", "P-1p6", "P-5", "P-5t", "P-6", "CE-1", "2A.4-5", "EC7-fields", "AU-4a", "AU-4b", "AU-4c", "AU-4d", "AU-4e",
            "AU-5r", "AU-5k3", "AU-3a", "AU-3b", "AU-3c", "AU-3d", "AU-3e", "AU-1a", "AU-1b", "AU-1c", "AU-1d", "AU-5x", "AU-2",
            "BWA", "BWB", "HIST-K4", "HIST-CHAIN", "K5", "K5b", "K5c", "W-1", "W-2", "NI-2", "W-3", "NI-1a", "NI-1b", "W-4", "W-5",
            "W-6", "NI-3a", "NI-3b", "NI-5f", "NI-5f2", "NI-5b", "NI-5c", "NI-5d", "NI-5c2", "NI-5b2", "NI-5a1", "NI-5a2", "NI-5a3",
            "NI-5g1", "NI-5g2", "NI-5g3", "NI-5g4", "NI-5h", "NI-5-uses2", "NI-S-counts", "SR-1m", "SR-1i", "SR-1s", "SR-2m", "SR-3m",
            "SR-4m", "SR-5m", "SR-6m", "NI-4", "CNT", "NI-6", "NI-6p", "NI-6u", "NI-6t2", "W-7", "W-8", "W-9", "PAT-1", "PAT-2",
            "PAT-3", "PAT-4", "PAT-5", "PAT-6", "PAT-7", "REC-0", "REC-1", "REC-2", "REC-3", "ENV-1", "ENV-2", "ENV-3", "DEST",
            "DEST2", "DEST3", "DEST-EV", "OWN", "GIT-LOCK", "PROC-1", "PROC-2", "PROC-3", "SCOPE-1", "RUNNER-1", "SCOPE-3", "SCOPE-4",
            "RES-1", "RES-2", "HIST-1", "HIST-2", "HIST-3", "HIST-4", "HIST-5"]
PRESERVE_FILES = ["status-porcelain-ignored.txt", "head.txt", "worktree-list-entry.json", "diff.txt", "diff-index-cached.txt",
                  "mutant-protocol.identity.txt", "ownership.json"]
FOUR_WAY = {"source_counts": [349, 35], "E-W": [349, 35], "E-W7": [349, 35], "expected": [349, 35]}
LINKED = {"44-full-F-PT": ("ledger::protocol::model_tests::gc_symmetry_fpt", "0863794ef7500c25382f55307213c65557f2d02b7bc4bb433511a71e082119a8"),
          "43-full-F-3": ("ledger::protocol::model_tests::gc_symmetry_f3", "6dfc796a0b5144184e3e50c9b1b873c21bb31bf26405c572fe858fb694245562")}
HIST_C7_WT = "/home/nexus/NEXUS/nexus-os-p3-c11-mt3c-C7"
CANDIDATE = "KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"
F3_BLOCKED = "MUTATION_CONTROL_BLOCKED — INFORMATIONAL F-3 NOT COMPLETED"
NOT_CANDIDATE = "NOT A KILL EVIDENCE CANDIDATE — STAGE 12 FAILED"
UTC_RE = re.compile(r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{9}Z$")
RE_GATE = re.compile(r"^(\S+) GATE (\S+) (PASS|FAIL) free=(\d+) memavail_kib=(\d+)$")
RE_END = re.compile(r"^(\S+) END (\S+) rc=(-?\d+)$")


def jl(p):
    with open(p, encoding="utf-8") as f:
        return json.load(f)


def quiet(fn, *a, **k):
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        r = fn(*a, **k)
    return r, buf.getvalue()


def default_cfg():
    return {"bwc": B.default_cfg(), "wt": {"H": WT_HEALTHY, "M": WT_MUTANT}, "live_verify": W.verify,
            "tools_root": os.path.dirname(HERE), "tools_manifest": os.path.join(os.path.dirname(HERE), "TOOLS.sha256")}


# ----------------------------------------------------------------------------- the 13.3 stage sequence
def parse_sequence(evdir):
    """SEQUENCE.log -> (entries, problems). stagegate.py writes the GATE lines, cmd/run-stage.sh the END lines."""
    p = os.path.join(evdir, "SEQUENCE.log")
    if not os.path.exists(p):
        return [], []
    try:
        text = open(p, "rb").read().decode("utf-8")
    except (OSError, UnicodeDecodeError) as e:
        return [], ["SEQUENCE.log unreadable: %r" % (e,)]
    ents, probs = [], []
    if text and not text.endswith("\n"):
        probs.append("SEQUENCE.log does not end with a newline")
    for i, line in enumerate(text.split("\n")[:-1] if text else []):
        m = RE_GATE.match(line)
        if m:
            e = {"t": m.group(1), "kind": "GATE", "stage": m.group(2), "ok": m.group(3) == "PASS"}
        else:
            m = RE_END.match(line)
            if not m:
                probs.append("line %d is neither a GATE nor an END line: %r" % (i + 1, line[:160]))
                continue
            e = {"t": m.group(1), "kind": "END", "stage": m.group(2), "ok": m.group(3) == "0", "rc": int(m.group(3))}
        if not UTC_RE.match(e["t"]):
            probs.append("line %d: timestamp %r is not UTC with nanoseconds" % (i + 1, e["t"]))
        ents.append(e)
    for a, b in zip(ents, ents[1:]):
        if not a["t"] < b["t"]:
            probs.append("timestamps not strictly increasing at %s %s" % (b["kind"], b["stage"]))
    return ents, probs


def clean_prefix(ents, n):
    """ents is exactly GATE PASS + END rc=0 for STAGES[:n], in that order."""
    want = []
    for s in STAGES[:n]:
        want += [("GATE", s, True), ("END", s, True)]
    got = [(e["kind"], e["stage"], e["ok"]) for e in ents]
    return got == want, got


def precondition(evdir, stage):
    ents, probs = parse_sequence(evdir)

    def no(why):
        print("PRECONDITION %s: REFUSED (%s)" % (stage, why))
        return 1
    if stage not in STAGES:
        return no("unknown stage")
    if probs:
        return no("; ".join(probs))
    if any(e["stage"] == stage for e in ents):
        return no("the stage has already started once; nothing is retried")
    i = STAGES.index(stage)
    ok, got = clean_prefix(ents, i)
    if ok:
        print("PRECONDITION %s: ALLOWED (every earlier stage of the 13.3 sequence completed with rc 0, in order)" % stage)
        return 0
    if stage == RESTORE_STAGE:
        # AD-C7-W 7.3A: a stop after the mutant patch was applied (13.3 step 4) and before step 11; restoration only
        if not os.path.exists(os.path.join(evdir, "receipts/00-worktrees/apply-C7.json")):
            return no("no stop-path restoration: the mutant patch was never applied (apply-C7.json absent)")
        last = ents[-1] if ents else None
        k = STAGES.index(last["stage"]) if last and last["stage"] in STAGES else -1
        if last is None or last["ok"] or not STAGES.index(FIRST_MUTANT_STAGE) <= k <= STAGES.index(LAST_RUN_STAGE):
            return no("not a stop between %s and %s: last entry %s" % (FIRST_MUTANT_STAGE, LAST_RUN_STAGE, last))
        if last["kind"] == "END":
            if len(ents) < 2 or (ents[-2]["kind"], ents[-2]["stage"], ents[-2]["ok"]) != ("GATE", last["stage"], True):
                return no("the failed END line is not preceded by its own passing GATE line")
            head = ents[:-2]
        else:
            head = ents[:-1]
        if not clean_prefix(head, k)[0]:
            return no("the entries before the stop are not the clean 13.3 sequence")
        print("PRECONDITION %s: ALLOWED ON THE 7.3A STOP PATH (stop at %s); restoration only, no run or build" % (stage, last["stage"]))
        return 0
    return no("the sequence so far is not exactly the completed stages before %s (last entries %s)" % (stage, got[-3:]))


# ----------------------------------------------------------------------------- Stage 12 inputs
def required_files():
    req = ["MISSION.txt", "SEQUENCE.log", "gates/SUP-S02/SELFTEST.txt"]
    for _, g in AUTH:
        req += [g, g + ".EC7.json"]
    req += ["gates/%s.txt" % s for s in STAGES]
    req += ["receipts/00-worktrees/" + f for f in ("creation-H.json", "creation-M.json", "apply-C7.json", "apply-C7.txt", "patch/C7.patch")]
    for b in ("10-build-E-W", "11-build-E-W7"):
        req += ["receipts/%s/%s" % (b, f) for f in ("descriptor.json", "RECEIPT.txt", "build.stderr", "build.stdout", "toolchain-versions.json",
                                                     "scope-record.txt", "time-v.txt", "cgroup-samples.txt")]
    req += ["receipts/12-pin/E-W.json", "receipts/12-pin/E-W7.json"]
    for ld in ("20-list-E-W", "21-list-E-W7"):
        for sp in B.LIST_SPECS:
            req += ["receipts/%s/%s/%s" % (ld, sp[0], f) for f in ("descriptor.json", "raw-output.txt", "RECEIPT.txt", "scope-record.txt", "time-v.txt")]
    req += ["receipts/25-stage3-gate/STAGE3-E-W.txt", "receipts/25-stage3-gate/STAGE3-E-W.json", "receipts/30-bwc/B-W-C.txt", "receipts/30-bwc/B-W-C.json"]
    for _, d, _ in RUNS:
        req += ["receipts/%s/%s" % (d, f) for f in ("descriptor.json", "raw-output.txt", "RECEIPT.txt", "CLASSIFICATION.txt", "CLASSIFICATION.json",
                                                    "scope-record.txt", "time-v.txt")]
    req += ["receipts/50-restoration/" + f for f in ("restoration.txt", "E-W7-sha256-after-restoration.txt", "healthy-EXACTLY_W-after.txt")]
    req += ["receipts/50-restoration/pre-restore/" + f for f in PRESERVE_FILES]
    req += ["receipts/60-link/LINK.txt", "receipts/60-link/LINK.json"]
    return req


def witness_steps(kind, rdir):
    """c7class's own region and REPLAY parsers (no reimplementation). Returns the 24 parsed steps or None."""
    def go():
        ck = K.Ck("G9X parse %s" % kind)
        d = jl(os.path.join(rdir, "descriptor.json"))
        lines = K.read_raw(ck, os.path.join(rdir, d["output_path"]))
        if lines is None:
            return None
        reg = K.split_witness(ck, kind, lines)
        if reg is None:
            return None
        parsed = K.parse_replay(ck, kind, reg["replay"])
        return parsed[0] if parsed else None
    return quiet(go)[0]


# ----------------------------------------------------------------------------- Stage 12
def classify(evdir, cfg=None, write=True):
    cfg = cfg or default_cfg()
    evdir = os.path.abspath(evdir)
    rp = lambda *a: os.path.join(evdir, "receipts", *a)  # noqa
    outdir = rp("70-final")
    if write and os.path.lexists(outdir):
        raise SystemExit("REFUSED: %s exists (Stage 12 runs once)" % outdir)
    rec = Rec("STAGE 12 FINAL INDEPENDENT CLASSIFICATION (AD-C7-W 13.3 step 13; 8.2; G.7)")
    rec.log("# utc %s" % utcns())
    rec.log("# evidence directory %s" % evdir)
    raw_obs, adm, flags, resources = {}, {}, [], []
    st = {"pins": None, "facts": {}, "verdicts": {}, "seq": [], "res_bad": []}

    def section(cid, fn):
        try:
            fn()
        except Exception as e:  # noqa: BLE001 — fail closed: a section that cannot be evaluated is a FAIL
            rec.check(cid, "section evaluated without error (fail closed)", False, "%s: %s" % (type(e).__name__, str(e)[:300]))

    # ---- IN
    rec.log("\n## IN. the complete Stage 12 input")
    req = required_files()
    missing = [f for f in req if not os.path.isfile(os.path.join(evdir, f))]
    rec.check("IN-1", "every mandatory receipt file exists (%d required)" % len(req), not missing, "missing %d: %s" % (len(missing), missing[:12]))

    def pins():
        st["pins"] = {r: jl(rp("12-pin", r + ".json"))["executable"] for r in ("E-W", "E-W7")}
    section("IN-2", pins)
    pw, p7 = (st["pins"] or {}).get("E-W"), (st["pins"] or {}).get("E-W7")

    # ---- SEQ
    rec.log("\n## SEQ. the exact 13.3 stage sequence and every receipt inside its stage window")

    def seq():
        ents, probs = parse_sequence(evdir)
        st["seq"] = ents
        ok, got = clean_prefix(ents[:-1], STAGES.index(FINAL_STAGE)) if ents else (False, [])
        last = ents[-1] if ents else None
        rec.check("SEQ-1", "SEQUENCE.log is exactly S01..S11 (each GATE PASS then END rc=0, in order, strictly increasing UTC) then the S12 GATE PASS",
                  not probs and ok and last is not None and (last["kind"], last["stage"], last["ok"]) == ("GATE", FINAL_STAGE, True),
                  "; ".join(probs)[:300] or "got %s" % got[-4:])
        bad = []
        for s in STAGES:
            p = os.path.join(evdir, "gates", s + ".txt")
            txt = open(p, encoding="utf-8").read() if os.path.exists(p) else ""
            if ("STAGE GATE %s: PASS" % s) not in txt or "[FAIL]" in txt:
                bad.append(s)
        rec.check("SEQ-2", "every stage gate record says `STAGE GATE <stage>: PASS` with no failing check", not bad, "bad: %s" % bad)
        win = {}
        for e in ents:
            win.setdefault(e["stage"], {})[e["kind"]] = e["t"]
        outside = []

        def inside(label, stage, t0, t1):
            w = win.get(stage, {})
            g, en = w.get("GATE"), w.get("END")
            if not (t0 and t1 and UTC_RE.match(t0) and UTC_RE.match(t1) and g and en and g < t0 <= t1 < en):
                outside.append("%s %s..%s not inside %s %s..%s" % (label, t0, t1, stage, g, en))
        for s, g in AUTH:
            m = re.search(r"^# utc (\S+)$", open(os.path.join(evdir, g), encoding="utf-8").read(), re.M)
            t = m.group(1) if m else None
            inside(g, s, t, t)
        for role, s in (("H", "S03a-create-H"), ("M", "S04a-create-M")):
            j = jl(rp("00-worktrees", "creation-%s.json" % role))
            inside("creation-" + role, s, j.get("started_utc"), j.get("finished_utc"))
        j = jl(rp("00-worktrees", "apply-C7.json"))
        inside("apply-C7", "S04b-apply-C7", j.get("started_utc"), j.get("finished_utc"))
        for d, s in (("10-build-E-W", "S03b-build-E-W"), ("11-build-E-W7", "S04c-build-E-W7")):
            sup = jl(rp(d, "descriptor.json"))["supervisor"]
            inside(d, s, sup.get("started_utc"), sup.get("finished_utc"))
        for ld, s in (("20-list-E-W", "S03d-list-E-W"), ("21-list-E-W7", "S04e-list-E-W7")):
            for sp in B.LIST_SPECS:
                sup = jl(rp(ld, sp[0], "descriptor.json"))["supervisor"]
                inside("%s/%s" % (ld, sp[0]), s, sup.get("started_utc"), sup.get("finished_utc"))
        j = jl(rp("25-stage3-gate", "STAGE3-E-W.json"))
        inside("stage3", "S03e-stage3-gate", j.get("started_utc"), j.get("finished_utc"))
        times = {}
        for kind, d, s in RUNS:
            sup = jl(rp(d, "descriptor.json"))["supervisor"]
            times[kind] = (sup.get("started_utc"), sup.get("finished_utc"))
            inside(d, s, *times[kind])
        rt = open(rp("50-restoration", "restoration.txt"), encoding="utf-8").read()
        m0 = re.search(r"^RESTORATION \(frozen inverse patch\) .* \(UTC (\S+)\)$", rt, re.M)
        m1 = re.search(r"^RESTORE: \S+; finished UTC (\S+)$", rt, re.M)
        times["R"] = (m0.group(1) if m0 else None, m1.group(1) if m1 else None)
        inside("restoration", RESTORE_STAGE, *times["R"])
        rec.check("SEQ-3", "every receipt's own UTC times lie inside its stage's GATE..END window", not outside, "; ".join(outside)[:600])
        order = [("B-W/C END", win.get("S05-bwc", {}).get("END"))] + [("(%s)" % k, times[k][0]) for k in ("C", "A", "B", "F3")] + \
                [("restoration", times["R"][0]), ("link GATE", win.get("S11-link", {}).get("GATE"))]
        okord = all(t for _, t in order) and all(times[k][1] for k in ("C", "A", "B", "F3", "R")) \
            and all(order[i][1] < order[i + 1][1] for i in range(len(order) - 1)) \
            and times["C"][1] < times["A"][0] and times["A"][1] < times["B"][0] and times["B"][1] < times["F3"][0] \
            and times["F3"][1] < times["R"][0] and times["R"][1] < order[-1][1]
        rec.check("SEQ-4", "13.3 run order from receipt times: B-W/C < (C) < (A) < (B) < F-3 < restoration < link, each finished before the next began",
                  okord, json.dumps(order))
    section("SEQ-0", seq)

    # ---- AUTH
    rec.log("\n## AUTH. Stage 1-2 authorisation gates; identities; mission text; git objects re-read now")

    def auth():
        ec7 = []
        msha = sha256_file(os.path.join(evdir, "MISSION.txt"))
        for s, g in AUTH:
            t = open(os.path.join(evdir, g), encoding="utf-8").read()
            lines = t.splitlines()
            tag = s.split("-")[0]
            rec.check("AUTH-1.%s" % tag, "%s: `GATE RESULT: PASS (every check passed)` and no failing check" % g,
                      bool(lines) and lines[-1] == "GATE RESULT: PASS (every check passed)" and not any(l.startswith("[FAIL]") for l in lines),
                      lines[-1] if lines else "")
            passed = {m.group(1) for m in re.finditer(r"^\[PASS\] (\S+?): ", t, re.M)}
            miss = [i for i in GATE_IDS if i not in passed]
            rec.check("AUTH-2.%s" % tag, "%s: every one of the %d mandatory gate checks PASSed (AU-1..AU-6, B-W/A, B-W/B, NI-1..NI-6, SR-1..SR-6, K1..K5, W-SRC, "
                      "patch, E-C, environment, resources, history)" % (g, len(GATE_IDS)), not miss, "missing: %s" % miss[:12])
            m = re.search(r"^\[INFO\] 2A\.4-6: K1 (\S+)/(\S+); K2 (\S+)/(\S+); K3 (\S+); K4 (\S+)/(\S+)/(\S+); K5 (\S+)/(\S+); EC7 ([0-9a-f]{64})$", t, re.M)
            want = (K1_BLOB, K1_SHA, K2_BLOB, K2_SHA, K3_SHA, K4_COMMIT, K4_BLOB, K4_SHA, C_BASE, C_TREE)
            rec.check("AUTH-3.%s" % tag, "%s: the recorded K1..K5 identities equal the pinned ones (K4 = the NEW Owner record %s, blob %s, sha256 %s)"
                      % (g, K4_COMMIT[:8], K4_BLOB[:8], K4_SHA[:8]), m is not None and m.groups()[:10] == want, str(m.groups() if m else None)[:400])
            e = os.path.join(evdir, g + ".EC7.json")
            m2 = re.search(r"^\[INFO\] EC7: EC7 canonical bytes: (\d+); sha256 ([0-9a-f]{64})$", t, re.M)
            esha = sha256_file(e)
            rec.check("AUTH-4.%s" % tag, "%s: EC7 file sha256 and size equal the gate's EC7 record and its 2A.4-6 line" % g,
                      m is not None and m2 is not None and esha == m2.group(2) == m.group(11) and os.path.getsize(e) == int(m2.group(1)), esha)
            ec7.append(esha)
            m3 = re.search(r"^\[INFO\] MIS: mission text \S+ sha256 ([0-9a-f]{64}) ", t, re.M)
            rec.check("AUTH-5.%s" % tag, "%s: the mission text has the sha256 the gate authenticated" % g, m3 is not None and m3.group(1) == msha, msha)
        rec.check("AUTH-6", "both gates derived the same EC7", len(ec7) == 2 and ec7[0] == ec7[1], str(ec7))
        k4, k2, k1 = blob_at(K4_COMMIT, K4_PATH), blob_at(K2_COMMIT, K2_PATH), blob_at(K1_COMMITS[1], K1_PATH)
        wb = blob_at(W_COMMIT, MODEL_TESTS_PATH)
        rec.check("AUTH-7", "re-read now from git objects: K1, K2 and K4 blobs/sha256, W-COMMIT tree and W-BLOB, C-BASE tree",
                  git_blob_id(k4) == K4_BLOB and sha256_bytes(k4) == K4_SHA and git_blob_id(k2) == K2_BLOB and sha256_bytes(k2) == K2_SHA
                  and git_blob_id(k1) == K1_BLOB and sha256_bytes(k1) == K1_SHA and git_out("rev-parse", W_COMMIT + "^{tree}") == W_TREE
                  and git_blob_id(wb) == W_BLOB and sha256_bytes(wb) == W_BLOB_SHA and git_out("rev-parse", C_BASE + "^{tree}") == C_TREE)
    section("AUTH-0", auth)

    # ---- BWC
    rec.log("\n## BWC. B-W/C re-judged in memory by c7bwc.check (this mission's own builds and listings)")

    def bwc():
        rc, txt = quiet(B.check, evdir, cfg["bwc"], False)
        fl = sorted({l.split(":")[0][7:] for l in txt.splitlines() if l.startswith("[FAIL]")})
        rec.check("BWC-1", "B-W/C re-judged now: MECHANICALLY SATISFIED (NI-6 build-time clause, NI-7, NI-8, 6A, ORD-1..3)", rc == 0 and not fl, "rc=%d failed=%s" % (rc, fl))
        s = jl(rp("30-bwc", "B-W-C.json"))
        rec.check("BWC-2", "the stored B-W/C record is MECHANICALLY SATISFIED with no failed check", s.get("satisfied") is True and s.get("failed_checks") == []
                  and str(s.get("verdict", "")).startswith("B-W/C: MECHANICALLY SATISFIED"), str(s.get("verdict"))[:120])
        rec.check("BWC-3", "the stored B-W/C pins equal the 12-pin records (path, sha256, size)",
                  all(s.get("pins", {}).get(r) == {"path": st["pins"][r]["path"], "sha256": st["pins"][r]["sha256"], "size": st["pins"][r]["size"]} for r in ("E-W", "E-W7")))
        rec.check("BWC-4", "the stored four-way agreement is the frozen expectation (349 total, 35 ignored in W-TREE, E-W, E-W7 and 6A) and 348 filtered out",
                  s.get("four_way") == FOUR_WAY and s.get("filtered_out_expected") == 348 and s.get("identity") == cfg["bwc"]["identity"], json.dumps(s.get("four_way")))
        g3 = jl(rp("25-stage3-gate", "STAGE3-E-W.json"))
        rec.check("BWC-5", "the Stage-3 hard order gate judged E-W admissible before the mutant worktree existed, for the pinned E-W",
                  g3.get("admissible") is True and g3.get("mutant_worktree_existed") is False and g3.get("failed_checks") == []
                  and (g3.get("pin") or {}).get("sha256") == st["pins"]["E-W"]["sha256"] and (g3.get("pin") or {}).get("path") == st["pins"]["E-W"]["path"])
        raw_obs["listings"] = {"all_listing_sha256": s.get("all_listing_sha256"), "ignored_listing_sha256": s.get("ignored_listing_sha256"), "four_way": s.get("four_way")}
        b = {r: jl(rp(d, "descriptor.json")) for r, d in (("E-W", "10-build-E-W"), ("E-W7", "11-build-E-W7"))}
        raw_obs["builds"] = {r: {"warnings": v["compiler_messages"]["warnings"], "errors": v["compiler_messages"]["errors"], "finished": v["finished"],
                                 "wall_seconds": v["supervisor"]["wall_seconds"], "memory_peak_bytes": v["supervisor"]["memory_peak_bytes"],
                                 "started_utc": v["supervisor"]["started_utc"], "finished_utc": v["supervisor"]["finished_utc"],
                                 "executable": st["pins"][r]} for r, v in b.items()}
    section("BWC-0", bwc)

    # ---- RUN
    rec.log("\n## RUN. the four runs re-classified in memory from raw output (never from a stored verdict or an exit status alone)")
    for kind, d, _s in RUNS:
        def run(kind=kind, d=d):
            stored = jl(rp(d, "CLASSIFICATION.json"))
            fn = K.classify_witness if kind in ("C", "B") else K.classify_explorer
            (ck, verdict), _ = quiet(fn, kind, rp(d), K.load_pins(rp("12-pin")))
            st["facts"][kind], st["verdicts"][kind] = ck.facts, verdict
            rec.check("RUN-%s-1" % kind, "%s: the in-memory re-classification equals the stored one (%s)" % (d, stored.get("verdict")),
                      verdict == stored.get("verdict") and sorted(ck.fails) == sorted(stored.get("failed_checks", [])) and stored.get("label") == K.label(kind, verdict),
                      "now %s %s" % (verdict, ck.fails[:8]))
            desc = jl(rp(d, "descriptor.json"))
            rec.check("RUN-%s-2" % kind, "%s: raw output still matches its descriptor sha256" % d, sha256_file(rp(d, "raw-output.txt")) == desc["raw_output_sha256"])
            if kind in GOOD:
                rec.check("RUN-%s-3" % kind, "%s: verdict %s" % (d, GOOD[kind]), verdict == GOOD[kind], verdict)
            else:
                rec.check("RUN-F3-3", "%s: informational F-3 is a COMPLETED receipt (any completed outcome; F3-1..F3-4)" % d, verdict.startswith("COMPLETED"), verdict)
            f = ck.facts
            raw_obs[kind] = {"exit_status": desc["process"]["exit_status"], "signal": desc["process"]["signal"], "wall_seconds": desc["supervisor"]["wall_seconds"],
                             "memory_peak_bytes": desc["supervisor"]["memory_peak_bytes"], "swap_peak_bytes": desc["supervisor"]["swap_peak_bytes"],
                             "oom_kill": desc["supervisor"]["oom_kill"], "disk_min_free_bytes": (desc["supervisor"].get("disk") or {}).get("min_free_bytes"),
                             "started_utc": desc["supervisor"]["started_utc"], "finished_utc": desc["supervisor"]["finished_utc"],
                             "raw_output_sha256": desc["raw_output_sha256"], "executable_path": desc["executable"]["path"],
                             "executable_sha256": desc["executable"]["sha256_before"]}
            for key in ("outcome", "states", "elapsed", "fail", "incomplete", "records", "step23_end", "kept", "lost", "f3_text", "bounds_line", "actors_line"):
                if key in f:
                    raw_obs[kind][key] = f[key]
            adm[kind] = {"verdict": verdict, "label": K.label(kind, verdict), "failed_checks": list(ck.fails)}
        section("RUN-%s-0" % kind, run)

    # ---- EXE
    rec.log("\n## EXE. executable identities (CL-10, C7-4, C7-14, C7-16, 0A, 7.4)")

    def exe():
        bad = []
        for kind, d, _s in RUNS:
            e = jl(rp(d, "descriptor.json"))["executable"]
            p = pw if kind == "C" else p7
            if not (e["path"] == p["path"] and e["sha256_before"] == e["sha256_after"] == p["sha256"] and e["size"] == p["size"]):
                bad.append(kind)
        rec.check("EXE-1", "(A), (B) and F-3 ran the one pinned E-W7 (same path, sha256 equal before and after each); (C) ran the pinned E-W", not bad, "bad: %s" % bad)
        rec.check("EXE-2", "0A: E-W != E-W7, E-W != E-C, E-W7 != E-C, E-W7 != the historical C7 executable b320977a (they only differ; nothing more is claimed)",
                  pw["sha256"] != p7["sha256"] and pw["path"] != p7["path"] and E_C_SHA not in (pw["sha256"], p7["sha256"]) and p7["sha256"] != HIST_C7_SHA)
        live = []
        for role, p in (("E-W", pw), ("E-W7", p7)):
            s_ = os.stat(p["path"])
            if not (sha256_file(p["path"]) == p["sha256"] and s_.st_size == p["size"] and s_.st_ino == p["inode"] and s_.st_mtime_ns == p["mtime_ns"]):
                live.append(role)
        rec.check("EXE-3", "7.4: both executables now still have their pinned sha256, size, inode and mtime (nothing rebuilt after the pin)", not live, "changed: %s" % live)
    section("EXE-0", exe)

    # ---- G9X
    rec.log("\n## G9X. G.9 across the two cores (c7class also checks each receipt against Table A)")

    def g9x():
        c, b = witness_steps("C", rp(RUNS[0][1])), witness_steps("B", rp(RUNS[2][1]))
        if not rec.check("G9X-0", "both witness receipts parse into 24 steps with c7class's own parsers", c is not None and b is not None):
            return
        r1 = [n for n in range(1, 25) if (c[n]["actor"], c[n]["offered"]) != (b[n]["actor"], b[n]["offered"])]
        rec.check("G9X-1", "rule 1: the same actor and the same offered stream at every one of the 24 steps on both cores", not r1, "differ at %s" % r1)
        keys = ("consumed", "unused", "faults", "branches", "fault_flag")
        r2 = [n for n in list(range(1, 23)) + [24] if any(c[n][k] != b[n][k] for k in keys)]
        rec.check("G9X-2", "rules 2 and 4: steps 1-22 and 24 consume, leave unused, keep faults and show branches identically on both cores", not r2, "differ at %s" % r2)
        rec.check("G9X-3", "rule 3: step 23 healthy end in (C) (consumed [(0,4),(0,4),(0,3)], unused [0]) and changed end in (B) (consumed [(0,4),(0,4),(0,4),(0,3)], unused [])",
                  c[23]["consumed"] == [(0, 4), (0, 4), (0, 3)] and c[23]["unused"] == [0] and b[23]["consumed"] == [(0, 4), (0, 4), (0, 4), (0, 3)] and b[23]["unused"] == [])
    section("G9X-9", g9x)

    # ---- ATT
    rec.log("\n## ATT. attribution: E11 stays E11; P6 only from (B)")

    def att():
        fa, fb, fc = st["facts"]["A"], st["facts"]["B"], st["facts"]["C"]
        rec.check("ATT-1", "(A) printed FAIL with property E11 and is recorded as E11, never as P6 (G.7(A), S-C7-6)",
                  fa.get("outcome") == "FAIL" and (fa.get("fail") or [None])[0] == "E11" and not fa.get("incomplete"), str(fa.get("fail")))
        rec.check("ATT-2", "(B) has exactly the two canonical records: E11 at the step-13 transition, then P6 after step 24 (G.7(B), CL-4/5)",
                  fb.get("records") == [K.E11_REC, K.P6_REC], json.dumps(fb.get("records"))[:300])
        rec.check("ATT-3", "(C) has no violation record, the healthy step-23 end and the kept predicate (G.7(C))",
                  fc.get("records") == [] and fc.get("step23_end") == "healthy" and fc.get("kept") is True)
    section("ATT-0", att)

    # ---- RES
    rec.log("\n## RES. limits of every supervised build, listing and run (7.1, 7.2, 7.3; the disk floor)")

    def res():
        items = [("10-build-E-W", c7sup.BUILD_LIMIT), ("11-build-E-W7", c7sup.BUILD_LIMIT)]
        items += [("%s/%s" % (ld, sp[0]), B.LIST_LIMIT) for ld in ("20-list-E-W", "21-list-E-W7") for sp in B.LIST_SPECS]
        items += [(d, K.LIMIT) for _, d, _ in RUNS]
        bad = []
        for rel, limit in items:
            d = jl(rp(rel, "descriptor.json"))
            # PROC-1 (exit 0) is not applied to the four runs: (A), (B) and a failing F-3 exit 101 normally; c7class CL-1 judges their exit.
            fails = [c[0] for c in B.proc_checks(d, rel, limit) if not c[2] and not (c[0] == "PROC-1" and rel.startswith("4"))]
            s = d["supervisor"]
            resources.append({"receipt": rel, "wall_seconds": s.get("wall_seconds"), "limit_seconds": limit, "memory_peak_bytes": s.get("memory_peak_bytes"),
                              "swap_peak_bytes": s.get("swap_peak_bytes"), "oom_kill": s.get("oom_kill"), "timed_out": s.get("timed_out"),
                              "disk_min_free_bytes": (s.get("disk") or {}).get("min_free_bytes"), "disk_peak_growth_bytes": (s.get("disk") or {}).get("peak_growth_bytes"),
                              "failed": fails})
            if fails:
                bad.append("%s %s" % (rel, fails))
                st["res_bad"].append(rel)
        rec.check("RES-1", "all %d supervised processes: 16 GiB cap, zero swap, no OOM, complete, within the time ceiling, disk floor kept" % len(items), not bad, "; ".join(bad)[:500])
    section("RES-0", res)

    # ---- RST
    rec.log("\n## RST. restoration EXACTLY_W (I-C7.6, 7.3A, 7.4)")

    def rst():
        t = open(rp("50-restoration", "restoration.txt"), encoding="utf-8").read()
        lines = t.splitlines()
        rec.check("RST-1", "restoration record: R-1..R-8 PASS, one inverse-patch attempt rc 0, EXACTLY_W after it, `RESTORE: EXACTLY_W`, never EXACTLY_C",
                  all(("[PASS] R-%d:" % i) in t for i in range(1, 9)) and "7.3A authentication R-1..R-8: OK" in lines
                  and t.count("git apply -R (one attempt, 60 s ceiling)") == 1 and "git apply -R (one attempt, 60 s ceiling) rc=0 " in t
                  and "EXACTLY_W after restoration: OK" in lines and "RESTORATION COMPLETE: EXACTLY_W" in lines
                  and any(l.startswith("RESTORE: EXACTLY_W; finished UTC ") for l in lines) and not any(l.startswith("[FAIL]") for l in lines)
                  and "EXACTLY_C" not in t, lines[-1] if lines else "")
        ident = open(rp("50-restoration", "pre-restore", "mutant-protocol.identity.txt"), encoding="utf-8").read()
        rec.check("RST-2", "evidence was preserved first, and it shows the restoration began from the authenticated mutant protocol.rs (0d4ab992, 6d2f2d49)",
                  ("git blob id %s\n" % MUTANT_BLOB) in ident and ("sha256 %s\n" % MUTANT_SHA) in ident, ident[:200])
        m = re.search(r"sha256 after restoration: ([0-9a-f]{64}) \(pinned ([0-9a-f]{64})\): MATCH$",
                      open(rp("50-restoration", "E-W7-sha256-after-restoration.txt"), encoding="utf-8").read().strip())
        rec.check("RST-3", "E-W7's sha256 after restoration equals its pin (7.4)", m is not None and m.group(1) == m.group(2) == p7["sha256"])
        h = open(rp("50-restoration", "healthy-EXACTLY_W-after.txt"), encoding="utf-8").read().splitlines()
        rec.check("RST-4", "the healthy worktree verified EXACTLY_W after the run sequence", h[-1:] == ["EXACTLY_W: OK"] and not any(l.startswith("[FAIL]") for l in h))
        a = jl(rp("00-worktrees", "apply-C7.json"))
        rec.check("RST-5", "W7-SRC was exact: the frozen patch (b5d8bc6c) applied once, protocol.rs became 0d4ab992/6d2f2d49 only",
                  a.get("ok") is True and a.get("patch_sha256") == PATCH_SHA and a.get("mutant_protocol", {}).get("git_blob") == MUTANT_BLOB
                  and a.get("mutant_protocol", {}).get("sha256") == MUTANT_SHA and a.get("apply_returncode") == 0)
    section("RST-0", rst)

    # ---- LNK
    rec.log("\n## LNK. the (D) link (I-C7.7; CR-B with B-W/A, B-W/B and B-W/C)")

    def lnk():
        s = jl(rp("60-link", "LINK.json"))
        rec.check("LNK-1", "the stored link is USABLE with no failed check", s.get("usable") is True and s.get("failed_checks") == []
                  and str(s.get("verdict", "")).startswith("(D) LINK USABLE"), str(s.get("verdict"))[:120])
        (rc, lrec), _ = quiet(L.link, rp("60-link"), cfg["wt"]["H"], cfg["wt"]["M"], rp("30-bwc", "B-W-C.json"), rp("12-pin"), os.path.join(evdir, "MISSION.txt"), write=False)
        rec.check("LNK-2", "c7link re-made the link in memory now: USABLE (LK-0a..LK-11)", rc == 0 and not lrec.fails, "failed: %s" % sorted(set(lrec.fails)))
        bad = []
        for rid, (test, rsha) in LINKED.items():
            r = s.get("receipts", {}).get(rid, {})
            if not (r.get("test") == test and r.get("raw_sha256") == rsha and r.get("executable_sha256") == E_C_SHA and r.get("classification") == "CLASSIFICATION: PASS"):
                bad.append(rid)
        rec.check("LNK-3", "linked identities are Candidate C's accepted receipts at f023cc3d on E-C cde9f9d2 (44-full-F-PT raw 0863794e; 43-full-F-3 raw 6dfc796a), never rerun",
                  not bad and s.get("evidence_commit") == EC_EVID and s.get("e_c", {}).get("sha256") == E_C_SHA, "bad: %s" % bad)
        rec.check("LNK-4", "the link names this mission's B-W/C record and mission text",
                  os.path.realpath(s.get("b_w_c_json", "")) == rp("30-bwc", "B-W-C.json") and s.get("b_w_c_satisfied") is True
                  and s.get("mission_sha256") == sha256_file(os.path.join(evdir, "MISSION.txt")))
        t = open(rp("60-link", "LINK.txt"), encoding="utf-8").read()
        rec.check("LNK-5", "the link states it is C-BASE PASS on E-C and NOT a W-SRC PASS", "They are C-BASE PASS on E-C. They are NOT W-SRC PASS." in t)
    section("LNK-0", lnk)

    # ---- F3
    rec.log("\n## F3. informational F-3 (section 10; never a kill, never a G.7 receipt)")
    f3v = st["verdicts"].get("F3", "")
    f3text = st["facts"].get("F3", {}).get("f3_text")
    f3_completed = f3v.startswith("COMPLETED")
    if f3_completed and f3text != "INFORMATIONAL F-3: FAIL E11 (as expected)":
        flags.append("F-3 outcome differs from the expected FAIL E11: %s (section 10: listed for the Architect's review)" % f3text)
    rec.info("F3", f3text or "INFORMATIONAL F-3: NOT COMPLETED — %s" % (f3v or "no receipt"))

    # ---- OBJ
    rec.log("\n## OBJ. live state now")

    def obj():
        bad = {r: [c[0] for c in cfg["live_verify"](cfg["wt"][r], "W") if not c[1]] for r in ("H", "M")}
        rec.check("OBJ-1", "both worktrees are EXACTLY_W now (c7wt.verify)", not bad["H"] and not bad["M"], json.dumps(bad))
        rec.check("OBJ-2", "the mutant protocol.rs blob 0d4ab992 is absent from the object store", git("cat-file", "-e", MUTANT_BLOB, check=False).returncode != 0)
        head, br = git_out("rev-parse", "HEAD"), git_out("rev-parse", "--abbrev-ref", "HEAD")
        stt = git("status", "--porcelain=v1").stdout.decode().strip()
        rec.check("OBJ-3", "primary checkout unchanged: f727f5c3 on rebuild/phase0-trust-boundary, clean",
                  head == PROTECTED_REBUILD and br == "rebuild/phase0-trust-boundary" and stt == "", "%s %s %r" % (head, br, stt[:80]))
        rec.check("OBJ-4", "destination review ref absent locally (no publication before classification)",
                  git("rev-parse", "--verify", "-q", DEST_REF, check=False).returncode != 0
                  and git("rev-parse", "--verify", "-q", "refs/remotes/%s/%s" % (REMOTE, DEST_REF[len("refs/heads/"):]), check=False).returncode != 0)
        ent = [b for b in git("worktree", "list", "--porcelain").stdout.decode().strip().split("\n\n") if b.startswith("worktree %s\n" % HIST_C7_WT)]
        r = subprocess.run(["sha256sum", "-c", "--quiet", "--strict", "SHA256SUMS.final"], cwd=HIST_EXEC01_EVDIR, capture_output=True, text=True)
        rec.check("OBJ-5", "historical evidence preserved: the C7 worktree mt3c-C7 still registered, locked, detached at C-BASE; EXEC-01 evidence verifies against SHA256SUMS.final",
                  len(ent) == 1 and "\nlocked" in ent[0] and ("HEAD " + C_BASE) in ent[0] and r.returncode == 0, (r.stdout + r.stderr)[:200])
    section("OBJ-0", obj)

    # ---- TOOL
    rec.log("\n## TOOL. frozen tools")

    def tool():
        root, man = cfg["tools_root"], cfg["tools_manifest"]
        rows = [l.split("  ", 1) for l in open(man, encoding="utf-8").read().splitlines()]
        bad = [p for h, p in rows if not os.path.isfile(os.path.join(root, p)) or sha256_file(os.path.join(root, p)) != h]
        listed = sorted(p for _, p in rows if p.startswith("c7tools/"))
        present = sorted("c7tools/" + f for f in os.listdir(os.path.join(root, "c7tools")) if f.endswith((".py", ".sh")))
        rec.check("TOOL-1", "every frozen tool file still hashes to the manifest (sha256 %s), and no unlisted tool exists" % sha256_file(man)[:16],
                  bool(rows) and not bad and listed == present, "mismatch %s; unlisted %s" % (bad, sorted(set(present) - set(listed))))
    section("TOOL-0", tool)

    # ---- result
    fails = sorted(set(rec.fails))
    if not fails:
        result = CANDIDATE
    elif not f3_completed and set(fails) <= {"RUN-F3-1", "RUN-F3-3"} | ({"RES-1"} if st["res_bad"] == [RUNS[3][1]] else set()):
        result = F3_BLOCKED  # section 10: only the informational F-3 is not a completed receipt (e.g. killed, OOM, timed out)
    else:
        result = NOT_CANDIDATE
    rec.log("\n## RESULT")
    for fl in flags:
        rec.log("ARCHITECT REVIEW ITEM: %s" % fl)
    rec.log("FAILED CHECKS: %s" % (", ".join(fails) or "none"))
    rec.log("STAGE 12 RESULT: %s" % result)
    rec.log("Executor maximum; not an acceptance. G.7(E)'s independent Architect review and any G.7(F) recording are ARCHITECT DECISION PENDING.")
    out = {"mission": MISSION, "stage": "13.3 step 13 (Stage 12)", "result": result, "kill_evidence_candidate": result == CANDIDATE,
           "failed_checks": fails, "architect_review_items": flags,
           "raw_observations": raw_obs, "admissibility": adm,
           "attribution": {"primary": "(A) %s, recorded exactly as printed; attributes nothing to P6" % ((st["facts"].get("A") or {}).get("fail") or ["?"])[0],
                           "named_property": "P6 only from the (B) mutant witness; E11 recorded separately and never counted as P6",
                           "recording": "'C7: P6 (first failure E11; witness)' only under G.7(F), by the Architect; never by this tool"},
           "informational_f3": f3text or ("INFORMATIONAL F-3: NOT COMPLETED — %s" % (f3v or "no receipt")),
           "resources": resources, "sequence": st["seq"],
           "architect_decision_pending": ["G.7(E): the Sole Architect's independent review", "G.7(F): recording", "C7 acceptance and the campaign count"],
           "finished_utc": utcns()}
    if write:
        os.makedirs(outdir)
        rec.write(os.path.join(outdir, "FINAL-CLASSIFICATION.txt"))
        with open(os.path.join(outdir, "FINAL-CLASSIFICATION.json"), "x", encoding="utf-8") as f:
            json.dump(out, f, indent=1, ensure_ascii=False)
    return (0 if result == CANDIDATE else 1), out, rec


def main(argv):
    if len(argv) == 3 and argv[1] == "classify":
        return classify(argv[2])[0]
    if len(argv) == 4 and argv[1] == "precondition":
        return precondition(os.path.abspath(argv[2]), argv[3])
    if len(argv) == 3 and argv[1] == "selftest":
        import final_selftest
        return final_selftest.main(argv[2])
    raise SystemExit(__doc__)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
