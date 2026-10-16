#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-C7-COMPLIANT-EXEC-02: independent classifier for the four test runs (adapted from the EXEC-01 classifier;
every change is listed in ../TOOLS-PROVENANCE.txt).

Reads RAW output only (never the witness's own verdict or the exit code alone) and applies AD-C7-W G.7/G.8/G.9, I-C7,
K.2-C7, CL-1..CL-12, C7-1..C7-21 and F3-1..F3-4.

  c7class.py classify <C|A|B|F3> <receipt-dir> <pin-dir>
  c7class.py selftest <new-scratch-dir>

Kinds: C = healthy witness on E-W; A = original F-PT explorer on E-W7; B = mutant witness on E-W7; F3 = informational F-3 on E-W7.
The maximum local result is a kill-evidence CANDIDATE. This tool never writes "C7 ACCEPTED".
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from c7lib import *  # noqa
import c7sup

LIMIT = 2700
CRATE_DIR = "crates/nexus-governed-control"
RUN_ENV = dict(c7sup.RUN_ENV)
PANIC_FILE_SUFFIX = "src/ledger/protocol/model_tests.rs"
PANIC_B = (9705, 9)       # the final panic! of `judge` in W-BLOB
PANIC_X = (4133, 5)       # the assert_eq! of `full_exploration`
FILTERED = "348 filtered out"
MAX_STATES = 30_000_000

PROPERTIES = ["P1", "P2", "P4", "P5", "P6", "P7", "P8", "P9", "P12", "P13", "P15", "D1", "L1",
              "E1", "E2", "E3", "E4", "E5", "E6", "E7", "E8", "E9", "E10", "E11", "KEY"]

DUP = "append_facts: NotApplied NotWritten Conflict(new or retained cause DuplicateRole)"
MISS = "append_facts: NotApplied NotWritten Conflict(new or retained cause MissingPrepared)"
CHG = "append_facts: Committed (inserted)"
FW1, FW0, WK, SE, ENV = "Fw(1)", "Fw(0)", "Wk", "Se", "Env"
Q = []


def _r(actor, offered=(), consumed=(), branches=(), faults=2):
    return dict(actor=actor, offered=list(offered), consumed=[tuple(c) for c in consumed], branches=list(branches), faults=faults)


TABLE_A = [
    _r(FW1), _r(FW1, [0, 0, 0, 0], [(0, 2), (0, 4), (0, 4), (0, 3)], [DUP]), _r(FW1, [0], [(0, 2)]),
    _r(FW0), _r(FW0, [0, 0, 0, 0], [(0, 2), (0, 4), (0, 4), (0, 3)], [MISS]),
    _r(WK), _r(WK), _r(WK, [0, 0, 0, 0], [(0, 3), (0, 3), (0, 2), (0, 2)], ["append_cell X: Committed (inserted)"]),
    _r(WK), _r(WK), _r(WK, [], [], ["empty: Done"]),
    _r(SE), _r(SE, [0], [(0, 4)], ["explain: Explained"]), _r(SE),
    _r(FW0, [1], [(1, 2)], [], 1), _r(FW0, faults=1),
    _r(SE, faults=1), _r(SE, [0], [(0, 4)], ["lookup: Absent"], 1), _r(SE, faults=1), _r(SE, faults=1), _r(SE, faults=1),
    _r(ENV, faults=1),
    _r(SE, [0, 0, 0, 0], [(0, 4), (0, 4), (0, 3)], [MISS], 1),   # step 23, healthy end
    _r(SE, faults=1),
]
CHANGED_23 = _r(SE, [0, 0, 0, 0], [(0, 4), (0, 4), (0, 4), (0, 3)], [CHG], 1)
FAULT_STEP = 15
assert len(TABLE_A) == 24

E11_REC = "    VIOLATION step 13 transition: E11: a table entry left in a step that did not see its explanation in the ledger"
P6_REC = "    VIOLATION after 24: P6: F1 was refused as a conflict and is Paid"
M2_OBS = ("NAMED PROPERTY P6: OBSERVED on the world after step 24 (append_facts: Committed (inserted) at step 23; F1, refused as a conflict at step 5, "
          "is Paid, with no refusal-table entry and no explanation of F1 in the ledger)")
M3_OBS = ("SEPARATE PROPERTY E11: OBSERVED at the transition of step 13 (explain: Explained of F2; F1's refusal-table entry left in a step "
          "that did not see F1's explanation in the ledger); never counted as P6")
M4_OBS = 'OTHER PROPERTIES VIOLATED, each reported separately and not counted as P6: ["E11"]'
H1 = ("C7 WITNESS F-PT: 24 steps, each enabled and consuming its choices as frozen; no violation; step 23 shows the healthy core's "
      "'append_facts: NotApplied NotWritten Conflict(new or retained cause MissingPrepared)', consuming 3 of its 4 offered choices, unused [0]; "
      "F1's refusal-table entry (MissingPrepared) kept after steps 13 and 24; step 24 records F1 as Conflict")
NOTE_LINE = "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace"

RE_STEP = re.compile(r"^C7 WITNESS F-PT step (\d\d)/24 (Fw\(1\)|Fw\(0\)|Wk|Se|Env) offered \[([0-9, ]*)\] consumed \[((?:\(\d+, \d+\)(?:, )?)*)\] "
                     r"unused \[([0-9, ]*)\] faults left (\d+): (.*?)(   \[fault\])?$")
RE_BR = re.compile(r"^    branches (\[.*\]); word after (0x[0-9a-f]+) \(live (\d+), unapplied (\d+)\); phase (\S.*), latch (\S.*)$")
RE_UCX = re.compile(r"^    U (.*?); C (.*?); X (.*)$")
RE_REF = re.compile(r"^    refusal table after (\d\d): \{(.*)\}$")
RE_CLS = re.compile(r"^    classes after (\d\d): F1 (\w+), F2 (\w+); debt (\d+); H (.+?); A (.+)$")
RE_CON = re.compile(r"^    conflicted after (\d\d): F1 (true|false), F2 (true|false)$")
RE_LED = re.compile(r"^    ledger after (\d\d): (\[.*\])$")
RE_VIO = re.compile(r"^    VIOLATION (initial|step (\d\d) in-step|step (\d\d) transition|after (\d\d)): ([A-Za-z0-9]+): (.+)$")
RE_FIX = re.compile(r"^    WITNESS FIXTURE FAILURE \(kills nothing\): (.+)$")
RE_PANIC = re.compile(r"^thread '([^']+)' \((\d+)\) panicked at ([^:]+):(\d+):(\d+):$")
RE_M1 = re.compile(r"^C7 WITNESS F-PT: 24 steps, every step enabled and consuming its choices as planned; (\d+) violations$")
RE_RESULT_OK = re.compile(r"^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 348 filtered out; finished in \d+\.\d\ds$")
RE_RESULT_FAIL = re.compile(r"^test result: FAILED\. 0 passed; 1 failed; 0 ignored; 0 measured; 348 filtered out; finished in \d+\.\d\ds$")
FRAGILE_TOKEN = lambda tok: re.compile(r"(?<![A-Za-z0-9_])" + re.escape(tok))  # noqa


def ints(s):
    return [int(x) for x in s.split(", ") if x != ""]


def pairs(s):
    return [(int(a), int(b)) for a, b in re.findall(r"\((\d+), (\d+)\)", s)]


def debug_strs(s):
    """A Rust Debug Vec<String> -> list of strings, or None if the text is not exactly that."""
    items = re.findall(r'"((?:[^"\\]|\\.)*)"', s)
    rebuilt = "[" + ", ".join('"%s"' % x for x in items) + "]"
    return items if rebuilt == s else None


class Ck:
    """Checks with the contract clause they implement; any FAIL makes the receipt NOT ADMISSIBLE (or a stop)."""

    def __init__(self, title):
        self.rec = Rec(title)
        self.facts = {}
        self.stop = None

    def __call__(self, cid, desc, ok, detail=""):
        return self.rec.check(cid, desc, bool(ok), detail)

    def info(self, cid, text):
        self.rec.info(cid, text)

    @property
    def fails(self):
        return self.rec.fails


# --------------------------------------------------------------------------------------------------------------------
# descriptor / process / identity (CL-1 process half, CL-10, C7-4, C7-14, C7-16, 7.3)
# --------------------------------------------------------------------------------------------------------------------
def load_pins(pin_dir):
    out = {}
    for role in ("E-W", "E-W7"):
        with open(os.path.join(pin_dir, role + ".json"), encoding="utf-8") as f:
            out[role] = json.load(f)
    return out


def expected_args(kind):
    if kind in ("C", "B"):
        return [TEST_WITNESS, "--exact", "--nocapture", "--test-threads=1"]
    return [TEST_FULL_FPT if kind == "A" else TEST_FULL_F3, "--exact", "--ignored", "--nocapture", "--test-threads=1"]


def descriptor_checks(ck, kind, rdir, d, pins):
    role = "E-W" if kind == "C" else "E-W7"
    other = "E-W7" if kind == "C" else "E-W"
    pin, pother = pins[role]["executable"], pins[other]["executable"]
    ex = d["executable"]
    wt = WT_HEALTHY if kind == "C" else WT_MUTANT
    ck("ID-1", "executable path equals the pinned %s path (CL-10)" % role, ex["path"] == pin["path"], "%s vs pinned %s" % (ex["path"], pin["path"]))
    ck("ID-2", "executable sha256 before and after equal the pinned %s sha256 (C7-16)" % role,
       ex["sha256_before"] == pin["sha256"] and ex["sha256_after"] == pin["sha256"] and ex["size"] == pin["size"],
       "before=%s after=%s pinned=%s size=%s/%s" % (ex["sha256_before"], ex["sha256_after"], pin["sha256"], ex["size"], pin["size"]))
    ck("ID-3", "not the other build, not E-C, not the historical C7 executable (C7-4, C7-14)",
       ex["path"] != pother["path"] and ex["sha256_before"] != pother["sha256"] and ex["sha256_before"] != E_C_SHA and ex["sha256_before"] != HIST_C7_SHA
       and "final-baseline-known-runs-01" not in ex["path"],
       "other=%s" % pother["path"])
    ck("ID-4", "argv is the exact frozen invocation", d["argv"] == [ex["path"]] + expected_args(kind), json.dumps(d["argv"]))
    ck("ID-5", "environment is exactly env -i PATH LANG RUST_BACKTRACE; ceiling 2700 s", d["env"] == RUN_ENV and d["limit_seconds"] == LIMIT, json.dumps(d["env"]))
    ck("ID-6", "worktree and working directory are the %s worktree's crate" % ("healthy" if kind == "C" else "mutant"),
       d["worktree"] == wt and d["cwd"] == os.path.join(wt, CRATE_DIR), "%s | %s" % (d["worktree"], d["cwd"]))
    raw = os.path.join(rdir, d["output_path"])
    ck("ID-7", "descriptor's raw-output sha256 equals the file", os.path.isfile(raw) and sha256_file(raw) == d["raw_output_sha256"])
    sv = d.get("source_verification_before", []) + d.get("source_verification_after", [])
    ck("ID-8", "source identity verified before and after (CL-11: %s)" % ("EXACTLY_W" if kind == "C" else "EXACTLY_W7"),
       len(d.get("source_verification_before", [])) > 0 and len(d.get("source_verification_after", [])) > 0 and all(x["ok"] for x in sv),
       "failed: %s" % [x["id"] for x in sv if not x["ok"]])


def process_checks(ck, kind, d):
    p, s = d["process"], d["supervisor"]
    code = p["exit_status"]
    ck("CL1-1", "process exited normally with status 0 or 101, no signal, supervisor and wrapper agree",
       code in (0, 101) and p["signal"] is None and p["supervisor_rc"] == code and str(p["wrapper_recorded_status"]) == str(code), json.dumps(p))
    ck("CL1-2", "scope record complete; no supervisor timeout (7.3)", s["completed"] is True and s["timed_out"] is False,
       "completed=%s timed_out=%s scope=%s" % (s["completed"], s["timed_out"], s["scope_result"]))
    ck("CL1-3", "oom 0 and oom_kill 0", s["oom"] == 0 and s["oom_kill"] == 0, "oom=%s oom_kill=%s" % (s["oom"], s["oom_kill"]))
    ck("CL1-4", "zero swap: memory.swap.peak 0, no swap events", s["swap_peak_bytes"] == "0" and all(v == 0 for v in s["swap_events"].values()),
       "peak=%s events=%s" % (s["swap_peak_bytes"], s["swap_events"]))
    ck("CL1-5", "cgroup limits memory.max 17179869184, memory.swap.max 0", s["memory_max"] == "17179869184" and s["swap_max"] == "0",
       "max=%s swap.max=%s" % (s["memory_max"], s["swap_max"]))
    ck("CL1-6", "memory.peak below the 16 GiB limit", s["memory_peak_bytes"] is not None and int(s["memory_peak_bytes"]) < 17179869184,
       "peak=%s" % s["memory_peak_bytes"])
    ck("CL1-7", "wall time below the 2700 s ceiling", s["wall_seconds"] is not None and s["wall_seconds"] < LIMIT and s["time_limit_seconds"] == LIMIT,
       "wall=%s limit=%s" % (s["wall_seconds"], s["time_limit_seconds"]))
    dk = s.get("disk") or {}
    ck("CL1-8", "disk floor preserved during the run: no floor breach (no supervisor stop), minimum free space >= 16 GiB",
       s.get("disk_floor_breach") is False and dk.get("min_free_bytes") is not None and dk["min_free_bytes"] >= DISK_FLOOR_BYTES,
       "breach=%s min_free=%s" % (s.get("disk_floor_breach"), dk.get("min_free_bytes")))
    return code


# --------------------------------------------------------------------------------------------------------------------
# raw text
# --------------------------------------------------------------------------------------------------------------------
def read_raw(ck, path):
    data = open(path, "rb").read()
    ck("RAW-1", "raw output is strict UTF-8 with LF only, no CR, no NUL, ends with a newline",
       b"\r" not in data and b"\x00" not in data and data.endswith(b"\n"), "%d bytes" % len(data))
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as e:
        ck("RAW-1b", "decodes as UTF-8", False, str(e))
        return None
    if b"\r" in data or b"\x00" in data or not data.endswith(b"\n"):
        return None
    lines = text.split("\n")[:-1]
    ck("RAW-2", "raw output has content", len(lines) > 0)
    ck.info("RAW-3", "%d lines, %d blank (blank lines are not structural)" % (len(lines), sum(1 for l in lines if l == "")))
    return lines


# --------------------------------------------------------------------------------------------------------------------
# witness runs: (C) healthy and (B) mutant
# --------------------------------------------------------------------------------------------------------------------
def split_witness(ck, kind, lines):
    """HEAD / REPLAY / MARKERS / SUMMARY / TAIL by the exact boundary lines (4.7A). Returns a dict or None."""
    nb = [l for l in lines if l != ""]
    prefix = "test %s ... " % TEST_WITNESS
    if not ck("REG-1", "HEAD: the first non-blank line is exactly `running 1 test`, and it occurs once",
              len(nb) >= 2 and nb[0] == "running 1 test" and nb.count("running 1 test") == 1):
        return None
    if not ck("REG-2", "REPLAY begins with the harness prefix `test <path> ... ` (removed once)", nb[1].startswith(prefix), repr(nb[1][:120])):
        return None
    first = nb[1][len(prefix):]
    rest = nb[2:]
    out = {"nb": nb}
    if kind == "C":
        oks = [i for i, l in enumerate(rest) if l == "ok"]
        if not ck("REG-3", "REPLAY ends at exactly one harness line `ok`", len(oks) == 1, "found %d" % len(oks)):
            return None
        i = oks[0]
        out["replay"] = [first] + rest[:i]
        tail = rest[i:]
        out["tail"] = tail
        ck("REG-4", "TAIL is `ok` and the result line only (nothing else follows)", len(tail) == 2 and tail[0] == "ok" and bool(RE_RESULT_OK.match(tail[1])), json.dumps(tail)[:300])
        out["markers"] = out["summary"] = None
        return out
    pidx = [i for i, l in enumerate(rest) if l.startswith("thread '")]
    if not ck("REG-3", "exactly one panic line `thread '<path>' (<tid>) panicked at <file>:<line>:<col>:`", len(pidx) == 1 and bool(RE_PANIC.match(rest[pidx[0]])) if pidx else False,
              "thread lines: %d" % len(pidx)):
        return None
    pi = pidx[0]
    m = RE_PANIC.match(rest[pi])
    out["panic"] = m.groups()
    out["replay"] = [first] + rest[:pi]
    ev = [i for i, l in enumerate(rest) if l == "every violation:"]
    ni = [i for i, l in enumerate(rest) if l == NOTE_LINE]
    if not ck("REG-5", "`every violation:` and the RUST_BACKTRACE note each occur exactly once, in that order, after the panic line",
              len(ev) == 1 and len(ni) == 1 and pi < ev[0] < ni[0], "every=%s note=%s panic=%s" % (ev, ni, pi)):
        return None
    out["markers"] = rest[pi + 1:ev[0]]
    out["summary"] = rest[ev[0] + 1:ni[0]]
    out["tail"] = rest[ni[0]:]
    t = out["tail"]
    ck("REG-4", "TAIL is the note, FAILED, failures:, failures:, the test path and the result line only",
       len(t) == 6 and t[0] == NOTE_LINE and t[1] == "FAILED" and t[2] == "failures:" and t[3] == "failures:" and t[4] == "    " + TEST_WITNESS
       and bool(RE_RESULT_FAIL.match(t[5])), json.dumps(t)[:400])
    return out


def parse_replay(ck, kind, replay):
    """Strict REPLAY schema, locus blocks and the 4.6 state lines. Returns (steps, records, h1_count) or None."""
    rep = list(replay)
    h1n = sum(1 for l in rep if l == H1)
    if kind == "C":
        ck("RP-1", "H1 appears exactly once in REPLAY, as its last line", h1n == 1 and rep[-1] == H1, "count=%d" % h1n)
        if h1n == 1 and rep[-1] == H1:
            rep = rep[:-1]
    else:
        ck("RP-1", "H1 does not appear in (B)", h1n == 0)
    sidx = [i for i, l in enumerate(rep) if RE_STEP.match(l)]
    if not ck("RP-2", "exactly 24 per-step lines, numbered 01..24 in order (CL-2)",
              len(sidx) == 24 and [int(RE_STEP.match(rep[i]).group(1)) for i in sidx] == list(range(1, 25)), "found %d" % len(sidx)):
        return None
    initial = rep[:sidx[0]]
    ck("RP-3", "the initial block is empty (no `initial` record is admissible)", initial == [], json.dumps(initial)[:200])
    bounds = sidx + [len(rep)]
    steps, records = {}, []
    ok_all = True
    for n in range(1, 25):
        blk = rep[bounds[n - 1]:bounds[n]]
        m = RE_STEP.match(blk[0])
        st = dict(actor=m.group(2), offered=ints(m.group(3)), consumed=pairs(m.group(4)), unused=ints(m.group(5)), faults=int(m.group(6)),
                  note=m.group(7), fault_flag=bool(m.group(8)), raw=blk[0])
        want = [RE_BR, RE_UCX, RE_REF, RE_CLS, RE_CON, RE_LED]
        names = ["branches", "U/C/X", "refusal table", "classes", "conflicted", "ledger"]
        got = []
        good = len(blk) >= 7
        for k, (rx, nm) in enumerate(zip(want, names)):
            mm = rx.match(blk[1 + k]) if good and 1 + k < len(blk) else None
            if not mm:
                good = False
                ck("RP-4.%02d" % n, "step %02d block: %s line present in order" % (n, nm), False, repr(blk[1 + k][:160]) if 1 + k < len(blk) else "missing")
                break
            got.append(mm)
        if good:
            nn_ok = all(got[i].group(1) == "%02d" % n for i in (2, 3, 4, 5))
            if not nn_ok:
                good = False
                ck("RP-4.%02d" % n, "step %02d block: the NN in its state lines equals %02d" % (n, n), False)
        if good:
            br = debug_strs(got[0].group(1))
            if br is None:
                good = False
                ck("RP-4.%02d" % n, "step %02d block: the branches value is a Debug list of strings" % n, False, got[0].group(1)[:160])
        if not good:
            ok_all = False
            continue
        st.update(branches=br, word=got[0].group(2), live=int(got[0].group(3)), unapplied=int(got[0].group(4)), phase=got[0].group(5), latch=got[0].group(6),
                  u=got[1].group(1), c=got[1].group(2), x=got[1].group(3), refusal=got[2].group(2),
                  cls=(got[3].group(2), got[3].group(3), got[3].group(4), got[3].group(5), got[3].group(6)),
                  conflicted=(got[4].group(2) == "true", got[4].group(3) == "true"), ledger=got[5].group(2))
        order = {"in-step": 0, "transition": 1, "after": 2}
        last = -1
        for l in blk[7:]:
            v = RE_VIO.match(l)
            if not v:
                ok_all = False
                ck("RP-5.%02d" % n, "step %02d block: every line after the state lines is a VIOLATION line" % n, False, repr(l[:200]))
                continue
            locus = v.group(1)
            stepno = v.group(2) or v.group(3) or v.group(4)
            kindk = "in-step" if v.group(2) else "transition" if v.group(3) else "after" if v.group(4) else "initial"
            if stepno != "%02d" % n or kindk == "initial" or order[kindk] < last:
                ok_all = False
                ck("RP-5.%02d" % n, "step %02d block: a record lies in its own locus block, in the order in-step, transition, after" % n, False, repr(l[:200]))
                continue
            last = order[kindk]
            records.append(dict(locus=locus, prop=v.group(5), detail=v.group(6), line=l, block=n))
        steps[n] = st
    ck("RP-6", "REPLAY follows the strict schema: per-step line, six state lines, then records, for each of the 24 steps", ok_all)
    return (steps, records, h1n) if ok_all and len(steps) == 24 else None


def g9_checks(ck, steps):
    """G.9 + Table A + CL-2 + CL-8. Returns 'healthy' | 'changed' | None for step 23's end."""
    ok2 = True
    for n in range(1, 25):
        st, row = steps[n], TABLE_A[n - 1]
        if n == 23:
            continue
        unused_calc = row["offered"][len(st["consumed"]):]
        good = (st["actor"] == row["actor"] and st["offered"] == row["offered"] and st["consumed"] == row["consumed"] and st["unused"] == []
                and unused_calc == [] and st["faults"] == row["faults"] and st["branches"] == row["branches"] and st["fault_flag"] == (n == FAULT_STEP))
        if not good:
            ok2 = False
            ck("G9-%02d" % n, "step %02d equals Table A (actor, offered, consumed with option counts, no unused suffix, branches, faults, fault flag)" % n, False,
               "got actor=%s offered=%s consumed=%s unused=%s faults=%s branches=%s flag=%s" % (st["actor"], st["offered"], st["consumed"], st["unused"],
                                                                                              st["faults"], st["branches"], st["fault_flag"]))
    ck("G9-1", "steps 1-22 and 24 equal Table A and G.9 rule 2/4 (identical on both cores)", ok2)
    s23 = steps[23]
    healthy, changed = TABLE_A[22], CHANGED_23
    end = None
    for name, row, unused in (("healthy", healthy, [0]), ("changed", changed, [])):
        if (s23["actor"] == row["actor"] and s23["offered"] == row["offered"] and s23["consumed"] == row["consumed"] and s23["unused"] == unused
                and s23["faults"] == row["faults"] and s23["branches"] == row["branches"] and not s23["fault_flag"]):
            end = name
    ck("G9-23", "step 23 is exactly one of its two planned ends (G.9 rule 3)", end is not None,
       "got offered=%s consumed=%s unused=%s faults=%s branches=%s" % (s23["offered"], s23["consumed"], s23["unused"], s23["faults"], s23["branches"]))
    s24 = steps[24]
    if end:
        paired = {"healthy": "Conflict", "changed": "Paid"}[end]
        ck("G9-24", "step 24 consumes nothing and pairs with step 23's %s end (F1 %s after 24)" % (end, paired), s24["consumed"] == [] and s24["cls"][0] == paired, "F1=%s" % s24["cls"][0])
    for n in range(1, 25):
        c = steps[n]
        for (choice, options), offered in zip(c["consumed"], c["offered"]):
            if choice != offered or choice >= options:
                ck("G9-5.%02d" % n, "step %02d: consumed choice equals the offered prefix and lies in range" % n, False)
    return end


def state_predicates(steps):
    s13, s24 = steps[13], steps[24]
    kept = (s13["refusal"] == "F1: MissingPrepared" and s24["refusal"] == "F1: MissingPrepared" and s24["cls"][0] == "Conflict"
            and not FRAGILE_TOKEN("Fact(0)").search(s24["ledger"]) and s24["x"] == "None" and s24["conflicted"][0])
    lost = (s13["refusal"] == "" and s24["refusal"] == "" and s24["cls"][0] == "Paid" and s24["conflicted"][0]
            and bool(FRAGILE_TOKEN("Fact(0)").search(s24["ledger"])) and not FRAGILE_TOKEN("Explanation(0)").search(s24["ledger"]))
    return kept, lost


def classify_witness(kind, rdir, pins):
    title = {"C": "(C) HEALTHY WITNESS on E-W", "B": "(B) MUTANT WITNESS on E-W7"}[kind]
    ck = Ck("C7 classification %s — %s" % (title, os.path.basename(rdir)))
    d = json.load(open(os.path.join(rdir, "descriptor.json"), encoding="utf-8"))
    descriptor_checks(ck, kind, rdir, d, pins)
    code = process_checks(ck, kind, d)
    verdict = "NOT_ADMISSIBLE"
    lines = read_raw(ck, os.path.join(rdir, d["output_path"]))
    facts = ck.facts
    facts.update(kind=kind, exit_status=code)
    if lines is None:
        return finish(ck, kind, rdir, verdict)
    reg = split_witness(ck, kind, lines)
    if reg is None:
        return finish(ck, kind, rdir, verdict)
    parsed = parse_replay(ck, kind, reg["replay"])
    if parsed is None:
        return finish(ck, kind, rdir, verdict)
    steps, records, h1n = parsed
    end = g9_checks(ck, steps)
    kept, lost = state_predicates(steps)
    facts.update(step23_end=end, kept=kept, lost=lost, records=[r["line"] for r in records])
    vio_raw = [i for i, l in enumerate(lines) if "VIOLATION " in l]
    if kind == "C":
        ck("C7-12", "(C): exit status 0 with `1 passed; 0 failed` (C7-13: a line-940 stop otherwise)", code == 0)
        ck("C7-13", "(C): zero VIOLATION lines anywhere in the raw output", len(vio_raw) == 0 and records == [], "raw lines containing VIOLATION: %d" % len(vio_raw))
        ck("CL8-C", "(C): step 23 ended healthy", end == "healthy")
        ck("CL9-C", "(C): kept predicate holds (table exactly {F1: MissingPrepared} after 13 and 24; after 24 F1 Conflict, conflicted, Fact(0) absent, X empty)", kept,
           "after13=%r after24=%r cls=%s x=%s" % (steps[13]["refusal"], steps[24]["refusal"], steps[24]["cls"], steps[24]["x"]))
        ck("M-C", "(C): none of M1-M4 or a fixture-failure or a panic marker appears anywhere",
           not any(("NAMED PROPERTY" in l or "SEPARATE PROPERTY" in l or "OTHER PROPERTIES" in l or "WITNESS FIXTURE" in l or "WITNESS: no violation" in l or "every violation" in l)
                   for l in lines))
        ck("RAW-C", "(C): all REPLAY lines are accounted for (24 blocks + H1)", True)
        ok = not [f for f in ck.fails]
        verdict = "PASS_ADMISSIBLE" if ok else ("STOP_C7-12_13" if any(f in ("C7-12", "C7-13", "CL8-C", "CL9-C") for f in ck.fails) else "NOT_ADMISSIBLE")
        return finish(ck, kind, rdir, verdict)

    # ---- (B) ----
    tid, pfile, pline, pcol = reg["panic"][1], reg["panic"][2], int(reg["panic"][3]), int(reg["panic"][4])
    ck("C7-11", "(B): the panic is the witness's own verdict panic (thread %s, location %s:%d:%d), not a fixture failure or an unrelated panic" % (reg["panic"][0], pfile, pline, pcol),
       reg["panic"][0] == TEST_WITNESS and pfile.endswith(PANIC_FILE_SUFFIX) and (pline, pcol) == PANIC_B, "expected %s:%d:%d" % (PANIC_FILE_SUFFIX, *PANIC_B))
    ck("C7-17b", "(B): exit status 101 and `1 failed` on `348 filtered out`", code == 101)
    ck("CL8-B", "(B): step 23 ended changed (CL-8)", end == "changed")
    # canonical records (C7-5, C7-8)
    lines_rec = [r["line"] for r in records]
    ck("C7-5", "(B): exactly the two canonical records, in order", lines_rec == [E11_REC, P6_REC], json.dumps(lines_rec)[:600])
    e11 = [r for r in records if r["prop"] == "E11"]
    p6 = [r for r in records if r["prop"] == "P6"]
    ck("C7-6", "(B): P6 observed together with E11 (not P6 alone)", not (p6 and not e11))
    ck("C7-7", "(B): E11 observed together with P6 (not E11 alone)", not (e11 and not p6))
    ck("C7-8", "(B): no third record; E11 and P6 each once, each at its own locus and in its own block",
       len(records) == 2 and len(e11) == 1 and len(p6) == 1 and e11 and e11[0]["locus"] == "step 13 transition" and e11[0]["block"] == 13
       and p6 and p6[0]["locus"] == "after 24" and p6[0]["block"] == 24, "records=%d" % len(records))
    ck("C7-8b", "(B): every property named by a record is in PROPERTIES (CL-3)", all(r["prop"] in PROPERTIES and r["detail"].strip() for r in records))
    summ = reg["summary"]
    ck("C7-9a", "(B): the SUMMARY is a byte-for-byte ordered copy of the canonical records (CL-5)", summ == lines_rec, json.dumps(summ)[:600])
    outside = [i for i in vio_raw if not (lines[i] in lines_rec or lines[i] in summ)]
    ck("C7-8c", "(B): `VIOLATION ` occurs only in the 2 canonical records and their 2 copies (4 raw lines)", len(vio_raw) == 4 and not outside,
       "raw lines containing VIOLATION: %d, outside REPLAY/SUMMARY: %d" % (len(vio_raw), len(outside)))
    # markers
    mk = reg["markers"]
    ck("C7-10a", "(B): MARKERS hold exactly four lines (M1..M4), once each, in order", len(mk) == 4, "got %d" % len(mk))
    if len(mk) == 4:
        m1 = RE_M1.match(mk[0])
        n_m1 = int(m1.group(1)) if m1 else None
        ck("C7-9b", "(B): M1 n equals the canonical records and the SUMMARY lines, and is 2", n_m1 == len(records) == len(summ) == 2, "n=%s records=%d summary=%d" % (n_m1, len(records), len(summ)))
        p6_rec, e11_rec = bool(p6 and p6[0]["locus"] == "after 24"), bool(e11 and e11[0]["locus"] == "step 13 transition")
        changed_end, lost_state = end == "changed", lost
        want2 = M2_OBS if (p6_rec and changed_end and lost_state) else \
            "NAMED PROPERTY P6: NOT OBSERVED on the world after step 24 (P6 record at that locus %s; step 23 took the changed end %s; lost state shown %s)" % (
                str(p6_rec).lower(), str(changed_end).lower(), str(lost_state).lower())
        want3 = M3_OBS if e11_rec else "SEPARATE PROPERTY E11: NOT OBSERVED at the transition of step 13 (E11 record at that locus %s)" % str(e11_rec).lower()
        others = sorted({r["prop"] for r in records if r["prop"] != "P6"})
        want4 = "OTHER PROPERTIES VIOLATED, each reported separately and not counted as P6: " + json.dumps(others)
        ck("C7-10b", "(B): M2 equals the text recomputed from the raw state (CL-7)", mk[1] == want2, "got %r" % mk[1][:260])
        ck("C7-10c", "(B): M3 equals the text recomputed from the raw records (CL-7)", mk[2] == want3, "got %r" % mk[2][:260])
        ck("C7-10d", "(B): M4 equals the recomputed list and is admissible only as [\"E11\"]", mk[3] == want4 and mk[3] == M4_OBS, "got %r" % mk[3][:200])
        ck("C7-5b", "(B): M1 is the admissible line; M2 and M3 say OBSERVED", bool(m1) and mk[1] == M2_OBS and mk[2] == M3_OBS)
    ck("CL9-B", "(B): lost predicate holds (table {} after 13 and 24; after 24 F1 Paid and conflicted, Fact(0) present, Explanation(0) absent)", lost,
       "after13=%r after24=%r cls=%s" % (steps[13]["refusal"], steps[24]["refusal"], steps[24]["cls"]))
    ck("C7-10e", "(B): none of H1, fixture-failure, `WITNESS: no violation` appears anywhere",
       not any(("no violation; step 23 shows" in l or "WITNESS FIXTURE" in l or "WITNESS: no violation" in l) for l in lines))
    admissible = not ck.fails
    if admissible:
        verdict = "ADMISSIBLE_TWO_CANONICAL"
    return finish(ck, kind, rdir, verdict)


# --------------------------------------------------------------------------------------------------------------------
# explorer runs: (A) F-PT and informational F-3
# --------------------------------------------------------------------------------------------------------------------
RE_OUT = re.compile(r"^  outcome: (PASS|FAIL|INCOMPLETE)$")

# Exact frozen configuration lines, read from git objects (raw sha256 verified before use): the accepted Candidate C receipts
# (E-C, unchanged core, same Config constants under NI-1) and, for F-PT, the historical 85-C7 receipt cited by AD-C7-W as
# G.7(A)'s prediction. Only their `bounds:` and `actors:` lines are used; nothing else in them is relied on.
FROZEN_CONFIG_REFS = {
    "A": [(EC_EVID, EC_RECEIPTS + "/44-full-F-PT/raw-output.txt", "0863794ef7500c25382f55307213c65557f2d02b7bc4bb433511a71e082119a8"),
          ("c10681b3c65dd377b5a13d08140427e8b34441d0", "docs/evidence/p3-candidate11/d2a1-mutation-tranche3-continue-02/receipts/85-C7/04-run/raw-output.txt",
           "db3bf1643cb5f1d367004400f5caa9d975ebeb2bf45aca2e8b64dae98691154d")],
    "F3": [(EC_EVID, EC_RECEIPTS + "/43-full-F-3/raw-output.txt", "6dfc796a0b5144184e3e50c9b1b873c21bb31bf26405c572fe858fb694245562")],
}


def frozen_config_lines(kind):
    """[(ref, bounds line, actors line)] from git objects; raises if a reference's sha256 differs."""
    out = []
    for commit, path, sha in FROZEN_CONFIG_REFS[kind]:
        b = blob_at(commit, path)
        if sha256_bytes(b) != sha:
            raise RuntimeError("frozen configuration reference %s:%s has sha256 %s, not %s" % (commit[:8], path, sha256_bytes(b), sha))
        ls = b.decode("utf-8").split("\n")
        bl = [l for l in ls if l.startswith("  bounds: ")]
        al = [l for l in ls if l.startswith("  actors: ")]
        # a PASS receipt prints one report; a FAIL receipt prints it twice (the panic copy): the copies must agree
        if len(bl) not in (1, 2) or len(al) != len(bl) or len(set(bl)) != 1 or len(set(al)) != 1:
            raise RuntimeError("reference %s has %d bounds and %d actors lines (not one agreeing value)" % (path, len(bl), len(al)))
        out.append(("%s:%s" % (commit[:8], path.split("/receipts/")[-1]), bl[0], al[0]))
    return out


def parse_instance(block):
    """One exploration report (EXPLORATION ... `  state key fields:`). Returns (info, problems)."""
    pr = []
    info = dict(text="\n".join(block))
    if len(block) < 9 or not block[0].startswith("EXPLORATION "):
        return info, ["not an exploration report"]
    info["name"] = block[0][len("EXPLORATION "):]
    outs = [l for l in block if l.startswith("  outcome:")]
    if len(outs) != 1 or not RE_OUT.match(outs[0]):
        pr.append("%d outcome lines" % len(outs))
        return info, pr
    info["outcome"] = RE_OUT.match(outs[0]).group(1)
    for i, pre in ((1, "  outcome: "), (2, "  bounds: "), (3, "  actors: "), (4, "  states "), (5, "  elapsed ")):
        if not block[i].startswith(pre):
            pr.append("line %d is not `%s...`" % (i + 1, pre.strip()))
    if pr:
        return info, pr
    b = block[2]
    ma = re.search(r" A4 (true|false);", b)
    info["a4"] = ma.group(1) if ma else None
    info["caps"] = "caps 30000000 states, 2700s" in b
    ms = re.match(r"^  states (\d+) \(expanded (\d+)\), transitions (\d+) \(with a fault (\d+)\), states with no enabled step (\d+)$", block[4])
    me = re.match(r"^  elapsed (\d+\.\d) s; explorer tables (\d+) MiB$", block[5])
    if not ms or not me:
        pr.append("states/elapsed lines malformed")
        return info, pr
    info["states"], info["elapsed"] = int(ms.group(1)), float(me.group(1))
    i = 6
    info["incomplete"] = None
    mi = re.match(r"^  INCOMPLETE: (.*)\. No property is established by this run\.$", block[i])
    if mi:
        info["incomplete"] = mi.group(1)
        i += 1
    info["fail"] = None
    if block[i].startswith("  FAIL: "):
        mf = re.match(r"^  FAIL: ([A-Za-z0-9]+): (.*)$", block[i])
        mt = re.match(r"^  shortest trace \((\d+) steps\):$", block[i + 1]) if i + 1 < len(block) else None
        if not mf or not mt:
            pr.append("FAIL / shortest trace lines malformed")
            return info, pr
        info["fail"] = (mf.group(1), mf.group(2))
        k = int(mt.group(1))
        j = i + 2
        nums = []
        while j < len(block) and re.match(r"^ {4}[ \d]{3}\. ", block[j]):
            nums.append(int(block[j][4:7]))
            j += 1
        if nums != list(range(1, k + 1)):
            pr.append("trace has %s numbered lines, header says %d" % (len(nums), k))
        info["trace_steps"] = k
        i = j
        if i < len(block) and block[i].startswith("    end: phase "):
            i += 1
        else:
            pr.append("the trace's `    end:` line is missing")
            return info, pr
    else:
        props = []
        while i < len(block) and block[i] != "  branches:":
            m = re.match(r"^  ([A-Za-z0-9]+): (.*)$", block[i])
            if not m:
                pr.append("unexpected line before `branches:`: %r" % block[i][:120])
                break
            props.append((m.group(1), m.group(2)))
            i += 1
        info["props"] = props
        if [p for p, _ in props] != PROPERTIES:
            pr.append("property list is not PROPERTIES in order")
    if i >= len(block) or block[i] != "  branches:":
        pr.append("`  branches:` line missing")
        return info, pr
    i += 1
    while i < len(block) - 1 and re.match(r"^    \[(taken    |NOT TAKEN)\] .+$", block[i]):
        i += 1
    if i != len(block) - 1 or not block[-1].startswith("  state key fields: "):
        pr.append("report does not end with branch lines then `  state key fields:`")
    # consistency of outcome with content
    o = info["outcome"]
    if o == "FAIL" and not info["fail"]:
        pr.append("outcome FAIL without a FAIL line")
    if o == "PASS" and (info["fail"] or info["incomplete"] or any(not (v.startswith("HOLDS") or v.startswith("not applicable here") or v.startswith("not claimed")) for _, v in info.get("props", []))):
        pr.append("outcome PASS but the report shows a failure, incomplete or an unproven property")
    if o == "INCOMPLETE" and (info["fail"] or not info["incomplete"] or any(v != "NOT ESTABLISHED (incomplete)" for _, v in info.get("props", []))):
        pr.append("outcome INCOMPLETE inconsistent with its report")
    return info, pr


def explorer_structure(ck, kind, lines, code):
    """PASS layout: one instance; FAIL/INCOMPLETE layout: two byte-equal instances. Returns (kind_of_run, info) or (None, reason)."""
    nb = [l for l in lines if l != ""]
    test = TEST_FULL_FPT if kind == "A" else TEST_FULL_F3
    prefix = "test %s ... " % test
    if len(nb) < 4 or nb[0] != "running 1 test" or nb.count("running 1 test") != 1:
        return None, "HEAD: `running 1 test` missing or repeated"
    if not nb[1].startswith(prefix):
        return None, "REPLAY does not begin with the harness prefix"
    body = [nb[1][len(prefix):]] + nb[2:]
    # instance 1
    starts = [i for i, l in enumerate(body) if l.startswith("EXPLORATION ")]
    ends = [i for i, l in enumerate(body) if l.startswith("  state key fields: ")]
    if not starts or starts[0] != 0 or not ends:
        return None, "first report instance not found at the start of the output"
    inst1 = body[:ends[0] + 1]
    rest = body[ends[0] + 1:]
    i1, p1 = parse_instance(inst1)
    if p1:
        return None, "instance 1: " + "; ".join(p1)
    out = {"inst1": i1}
    ck.info("EX-1", "instance 1: name=%s outcome=%s states=%s elapsed=%s fail=%s" % (i1["name"], i1["outcome"], i1["states"], i1["elapsed"], i1["fail"]))
    if rest[:1] == ["ok"]:
        # PASS layout
        if len(rest) != 2 or not RE_RESULT_OK.match(rest[1]):
            return None, "PASS layout: the tail is not `ok` + the result line only"
        if code != 0:
            return None, "PASS report but exit status %s (F3-4)" % code
        if i1["outcome"] != "PASS":
            return None, "`ok` tail with a %s report" % i1["outcome"]
        out["layout"] = "PASS"
        return "PASS", out
    if code != 101:
        return None, "FAIL/INCOMPLETE layout but exit status %s (F3-4)" % code
    if not rest or not RE_PANIC.match(rest[0]):
        return None, "FAIL/INCOMPLETE layout: no panic line after instance 1"
    pm = RE_PANIC.match(rest[0])
    out["panic"] = pm.groups()
    if pm.group(1) != test or not pm.group(3).endswith(PANIC_FILE_SUFFIX) or (int(pm.group(4)), int(pm.group(5))) != PANIC_X:
        return None, "panic at another location or thread: %s:%s:%s (thread %s)" % (pm.group(3), pm.group(4), pm.group(5), pm.group(1))
    am = re.match(r"^assertion `left == right` failed: failed: (\{.*\})$", rest[1]) if len(rest) > 1 else None
    if not am:
        return None, "panic message is not the full_exploration assertion"
    out["failed_set"] = am.group(1)
    r2 = rest[2:]
    e2 = [i for i, l in enumerate(r2) if l.startswith("  state key fields: ")]
    if not r2 or not r2[0].startswith("EXPLORATION ") or not e2:
        return None, "panic copy of the report (instance 2) not found"
    inst2 = r2[:e2[0] + 1]
    tail = r2[e2[0] + 1:]
    i2, p2 = parse_instance(inst2)
    if p2:
        return None, "instance 2: " + "; ".join(p2)
    if inst1 != inst2:
        return None, "the two report instances are not byte-equal"
    want_left = {"FAIL": "  left: Fail", "INCOMPLETE": "  left: Incomplete"}.get(i1["outcome"])
    exp_tail = [want_left, " right: Pass", NOTE_LINE, "FAILED", "failures:", "failures:", "    " + test]
    if want_left is None or tail[:7] != exp_tail or len(tail) != 8 or not RE_RESULT_FAIL.match(tail[7]):
        return None, "FAIL/INCOMPLETE tail is not the frozen one: %s" % json.dumps(tail)[:400]
    exp_set = '{"%s"}' % i1["fail"][0] if i1["fail"] else "{}"
    if am.group(1) != exp_set:
        return None, "failed set %s does not match the report (%s)" % (am.group(1), exp_set)
    out["layout"] = i1["outcome"]
    return i1["outcome"], out


def classify_explorer(kind, rdir, pins):
    ck = Ck("C7 classification %s — %s" % ({"A": "(A) ORIGINAL F-PT EXPLORER on E-W7", "F3": "INFORMATIONAL F-3 on E-W7"}[kind], os.path.basename(rdir)))
    d = json.load(open(os.path.join(rdir, "descriptor.json"), encoding="utf-8"))
    descriptor_checks(ck, kind, rdir, d, pins)
    code = process_checks(ck, kind, d)
    facts = ck.facts
    facts.update(kind=kind, exit_status=code)
    lines = read_raw(ck, os.path.join(rdir, d["output_path"]))
    verdict = "NOT_ADMISSIBLE" if kind == "A" else "NOT_COMPLETED"
    if lines is None:
        return finish(ck, kind, rdir, verdict, "unreadable raw output")
    layout, info = explorer_structure(ck, kind, lines, code)
    if layout is None:
        ck("STR", "the raw output has the frozen explorer layout (F3-4 / REPLAY schema)", False, info)
        return finish(ck, kind, rdir, verdict, info)
    ck("STR", "the raw output has the frozen explorer layout (%s: %s)" % (layout, "one instance" if layout == "PASS" else "two byte-equal instances"), True)
    i1 = info["inst1"]
    want_name, want_a4 = ("F-PT", "true") if kind == "A" else ("F-3", "false")
    ck("EX-2", "configuration is %s with A4 %s and caps 30000000 states, 2700s" % (want_name, want_a4), i1["name"] == want_name and i1["a4"] == want_a4 and i1["caps"],
       "name=%r a4=%r caps_ok=%s" % (i1["name"], i1["a4"], i1["caps"]))
    ck("EX-3", "no quotient or symmetry run: the header is exactly `EXPLORATION %s` and no `quotient:`/`symmetry:` line appears" % want_name,
       i1["text"].split("\n")[0] == "EXPLORATION " + want_name and "quotient" not in i1["text"].split("\n")[0] and "symmetry" not in i1["text"].split("\n")[0]
       and not any(l.startswith("  quotient:") or l.startswith("  symmetry:") for l in lines))
    blk = i1["text"].split("\n")
    try:
        refs = frozen_config_lines(kind)
        for ref, bl, al in refs:
            ck("EX-2b", "exact frozen configuration: the `bounds:` and `actors:` lines are byte-equal to those of %s" % ref, blk[2] == bl and blk[3] == al,
               "got %r / %r" % (blk[2][:200], blk[3][:120]))
        facts["frozen_config_refs"] = [r[0] for r in refs]
    except Exception as e:  # noqa: BLE001
        ck("EX-2b", "frozen configuration references readable from git objects with their frozen sha256", False, repr(e))
    facts["bounds_line"], facts["actors_line"] = blk[2], blk[3]
    ck("EX-4", "states within the 30,000,000 ceiling and elapsed within 2700 s", i1["states"] <= MAX_STATES and i1["elapsed"] <= LIMIT, "states=%s elapsed=%s" % (i1["states"], i1["elapsed"]))
    facts.update(outcome=layout, states=i1["states"], elapsed=i1["elapsed"], fail=i1["fail"], incomplete=i1["incomplete"])
    structural_ok = not ck.fails
    if kind == "F3":
        # F3-1..F3-4: informational, never a kill; any inconsistency is NOT COMPLETED
        if not structural_ok:
            return finish(ck, kind, rdir, "NOT_COMPLETED", "inconsistent receipt: " + ", ".join(ck.fails))
        if layout == "PASS":
            text = "INFORMATIONAL F-3: PASS"
        elif layout == "FAIL":
            p = i1["fail"][0]
            text = "INFORMATIONAL F-3: FAIL %s%s" % (p, " (as expected)" if p == "E11" else " (FLAGGED: not E11; never counted as P6)")
        else:
            text = "INFORMATIONAL F-3: INCOMPLETE (%s; no property established; F3-3: stays INCOMPLETE)" % i1["incomplete"]
        facts["f3_text"] = text
        return finish(ck, kind, rdir, "COMPLETED: " + text.split(": ", 1)[1], text)
    # (A)
    if not structural_ok:
        return finish(ck, kind, rdir, "NOT_ADMISSIBLE", "stop")
    if layout == "PASS":
        ck("C7-3", "(A): the original explorer FAILs (PASS is a STOP)", False, "outcome PASS")
        return finish(ck, kind, rdir, "STOP_C7-3", "PASS")
    if layout == "INCOMPLETE":
        ck("C7-3", "(A): the original explorer FAILs (INCOMPLETE is a STOP)", False, "INCOMPLETE: %s" % i1["incomplete"])
        return finish(ck, kind, rdir, "STOP_C7-3", "INCOMPLETE")
    prop = i1["fail"][0]
    if prop == "P6":
        ck("C7-2", "(A) printed P6: STOP for a ruling (E11 is never relabelled as P6, nor P6 as E11)", False, i1["fail"][1])
        return finish(ck, kind, rdir, "STOP_C7-2_P6", "P6")
    if prop != "E11":
        ck("C7-3", "(A) printed another property: STOP", False, "%s: %s" % i1["fail"])
        return finish(ck, kind, rdir, "STOP_C7-3", prop)
    ck("C7-1a", "(A): outcome FAIL with property E11, exit 101, `1 failed`, `348 filtered out`, failed set {\"E11\"}", code == 101 and info.get("failed_set") == '{"E11"}')
    ck("C7-1b", "(A): the FAIL is not accompanied by an incomplete reason (a cap hit is not a clean kill)", i1["incomplete"] is None, str(i1["incomplete"]))
    ck("C7-21", "(A): the executable is E-W7, never the historical `85-C7` build", True)
    return finish(ck, kind, rdir, "FAIL_E11_ADMISSIBLE" if not ck.fails else "NOT_ADMISSIBLE", "E11")


def finish(ck, kind, rdir, verdict, note=""):
    if ck.fails and verdict in ("PASS_ADMISSIBLE", "ADMISSIBLE_TWO_CANONICAL", "FAIL_E11_ADMISSIBLE"):
        verdict = "NOT_ADMISSIBLE"
    ck.rec.log("")
    ck.rec.log("KIND %s   VERDICT %s   %s" % (kind, verdict, note if isinstance(note, str) else ""))
    ck.rec.log("FAILED CHECKS: %s" % (", ".join(ck.fails) or "none"))
    ck.facts.update(verdict=verdict, failed_checks=list(ck.fails), note=note if isinstance(note, str) else "")
    return ck, verdict


def label(kind, verdict):
    if kind == "C":
        return {"PASS_ADMISSIBLE": "(C) HEALTHY WITNESS: PASS — ADMISSIBLE"}.get(verdict, "(C) HEALTHY WITNESS: %s" % verdict.replace("_", " "))
    if kind == "B":
        return {"ADMISSIBLE_TWO_CANONICAL": "(B) MUTANT WITNESS: ADMISSIBLE — exactly E11 (step 13 transition) and P6 (after 24)"}.get(verdict, "(B) MUTANT WITNESS: %s" % verdict.replace("_", " "))
    if kind == "A":
        return {"FAIL_E11_ADMISSIBLE": "(A) ORIGINAL F-PT: FAIL E11 — ADMISSIBLE (C7-1)"}.get(verdict, "(A) ORIGINAL F-PT: %s" % verdict.replace("_", " "))
    return "INFORMATIONAL — AD-1-R4 F (line 748) — NOT A KILL RECEIPT — NOT A G.7 RECEIPT: %s" % verdict.replace("_", " ")


def classify(kind, rdir, pin_dir):
    pins = load_pins(pin_dir)
    fn = classify_witness if kind in ("C", "B") else classify_explorer
    ck, verdict = fn(kind, rdir, pins)
    ck.rec.log(label(kind, verdict))
    ck.rec.write(os.path.join(rdir, "CLASSIFICATION.txt"))
    ck.facts["label"] = label(kind, verdict)
    json.dump(ck.facts, open(os.path.join(rdir, "CLASSIFICATION.json"), "x", encoding="utf-8"), indent=1)
    good = {"C": ("PASS_ADMISSIBLE",), "B": ("ADMISSIBLE_TWO_CANONICAL",), "A": ("FAIL_E11_ADMISSIBLE",)}.get(kind)
    return (0 if verdict in good else 1) if good else (0 if verdict.startswith("COMPLETED") else 1)


if __name__ == "__main__":
    if len(sys.argv) >= 2 and sys.argv[1] == "classify" and len(sys.argv) == 5 and sys.argv[2] in ("C", "A", "B", "F3"):
        sys.exit(classify(sys.argv[2], sys.argv[3], sys.argv[4]))
    if len(sys.argv) == 3 and sys.argv[1] == "selftest":
        import c7class_selftest
        sys.exit(c7class_selftest.main(sys.argv[2]))
    raise SystemExit(__doc__)
