#!/usr/bin/env python3
"""Stage 2 — section 8.4 record: 20 configurations, the 193-row branch accounting, the ignored-test inventory.
Read-only. Every value is recomputed from raw receipt bytes (Git objects and the local original receipt roots);
nothing is copied from an executor summary. argv[1] = output directory.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *  # noqa

OUT = sys.argv[1]
KR = f"{EVB}/final-baseline-known-runs-01/receipts"
RC = f"{EVB}/final-baseline-remaining-configs-01/receipts"
LKR = f"{AUDITS}/p3-c11-dr01-d2a1-final-baseline-known-runs-01/receipts"
LRC = f"{AUDITS}/p3-c11-dr01-d2a1-final-baseline-remaining-configs-01/receipts"
V02 = f"{AUDITS}/p3-c11-dr01-d2a1-exact-source-validation-02/receipts"
VOCAB = f"{EVB}/final-baseline-remaining-configs-01/vocabulary/VOCABULARY.tsv"
FRDISP = f"{EVB}/fr-stutter-r5-01/FR-BRANCH-DISPOSITION.tsv"

checks = []


def check(label, ok, detail=""):
    checks.append((label, bool(ok), str(detail)))
    return ok


# Contract 8.2 / 8.3 bounds, read by the executor from the contract (lines 819-851), encoded as the explorer prints
# them. A dash means "as the base configuration". None means not stated by the contract (not checked).
BASE = dict(ENV="false", batch="false", gating_conflict="false", ack=0, attempts=0, malformed="false", cell="false",
            unwind="false", DROP="false", REPLAY=0, A4="false")
EXPECT = {
    "X-1": dict(facts=2, conflicting="[false, false]", faults=2, admissions=1, gating=0, cap=250000),
    "X-2": dict(facts=1, faults=1, admissions=1, gating=2, cap=250000),
    "X-3": dict(facts=2, conflicting="[true, false]", ENV="true", faults=1, cap=250000),
    "X-4": dict(facts=1, conflicting="[true, false]", ack=1, attempts=None, admissions=1, faults=1, cap=250000),
    "X-5": dict(facts=2, conflicting="[false, false]", batch="true", faults=2, cap=250000),
    "F-1": dict(facts=2, conflicting="[false, false]", faults=3, admissions=2, gating=2, ack=0, cap=60000000),
    "F-2": dict(facts=2, conflicting="[false, true]", faults=2, admissions=2, gating=2, ack=0, cap=30000000),
    "F-3": dict(facts=2, conflicting="[true, true]", faults=2, admissions=2, gating=2, ack=0, ENV="true", cap=30000000),
    "F-4": dict(facts=2, conflicting="[false, true]", faults=2, admissions=2, gating=2, ack=0, cell="true", cap=30000000),
    "F-5": dict(facts=2, conflicting="[true, true]", faults=2, admissions=2, gating=0, ack=1, attempts=2, cap=30000000),
    "F-6": dict(facts=2, conflicting="[true, true]", faults=2, admissions=2, gating=0, ack=1, attempts=2,
                malformed="true", cap=30000000),
    "F-7": dict(facts=2, conflicting="[true, true]", faults=1, admissions=1, gating=0, ack=2, attempts=1, cap=30000000),
    "F-8": dict(facts=2, conflicting="[true, false]", faults=2, admissions=2, gating=1, ack=1, attempts=None, cap=30000000),
    "F-PT": dict(facts=2, conflicting="[true, true]", faults=2, admissions=2, gating=2, ack=0, ENV="true", A4="true",
                 cap=30000000),
    "F-B": dict(facts=2, conflicting="[true, true]", faults=2, admissions=2, gating=2, ack=0, ENV="true", batch="true",
                cap=30000000),
    "F-B2": dict(facts=2, conflicting="[false, false]", faults=3, admissions=1, gating=0, ack=0, batch="true",
                 cap=30000000),
    "F-G": dict(facts=1, faults=2, admissions=2, gating=2, ack=0, gating_conflict="true", cap=30000000),
    "F-W": dict(facts=2, conflicting="[false, true]", faults=2, admissions=2, gating=2, ack=0, unwind="true",
                cap=30000000),
    "F-D": dict(facts=2, conflicting="[false, false]", faults=2, admissions=2, gating=2, ack=0, DROP="true",
                cap=30000000),
    "F-R": dict(facts=2, conflicting="[false, true]", faults=2, admissions=2, gating=2, ack=0, REPLAY=3, cap=30000000),
}
TEST = {"X-1": "x1_two_facts_two_writers_one_admission", "X-2": "x2_one_fact_two_gating_callers",
        "X-3": "x3_one_conflicting_fact_with_env", "X-4": "x4_one_conflicting_fact_one_acknowledgement",
        "X-5": "x5_one_clean_batch_of_two",
        "F-1": "full_f1", "F-2": "full_f2", "F-3": "gc_symmetry_f3", "F-4": "full_f4", "F-5": "full_f5",
        "F-6": "full_f6", "F-7": "full_f7", "F-8": "full_f8", "F-PT": "gc_symmetry_fpt", "F-B": "gc_symmetry_fb",
        "F-B2": "full_fb2", "F-G": "full_fg", "F-W": "full_fw", "F-D": "full_fd", "F-R": "full_fr"}
DIRS = {"F-2": (STAGE1, KR, LKR, "41-full-F-2"), "F-B": (STAGE1, KR, LKR, "42-full-F-B"),
        "F-3": (STAGE1, KR, LKR, "43-full-F-3"), "F-PT": (STAGE1, KR, LKR, "44-full-F-PT"),
        "F-1": (STAGE1, KR, LKR, "45-full-F-1"), "F-G": (STAGE2, RC, LRC, "60-full-F-G"),
        "F-B2": (STAGE2, RC, LRC, "61-full-F-B2"), "F-4": (STAGE2, RC, LRC, "62-full-F-4"),
        "F-W": (STAGE2, RC, LRC, "63-full-F-W"), "F-8": (STAGE2, RC, LRC, "64-full-F-8"),
        "F-5": (STAGE2, RC, LRC, "65-full-F-5"), "F-6": (STAGE2, RC, LRC, "66-full-F-6"),
        "F-7": (STAGE2, RC, LRC, "67-full-F-7"), "F-D": (STAGE2, RC, LRC, "68-full-F-D"),
        "F-R": (STAGE2, RC, LRC, "69-full-F-R")}
XDIRS = {"X-1": "41-stage-C-X-1", "X-2": "42-stage-C-X-2", "X-3": "43-stage-C-X-3", "X-4": "44-stage-C-X-4",
         "X-5": "45-stage-C-X-5"}
QUOTIENT = {"F-3", "F-PT", "F-B"}
ORDER = ["X-1", "X-2", "X-3", "X-4", "X-5", "F-1", "F-2", "F-3", "F-4", "F-5", "F-6", "F-7", "F-8", "F-PT", "F-B",
         "F-B2", "F-G", "F-W", "F-D", "F-R"]


def parse_bounds(b):
    g = lambda pat: (re.search(pat, b).group(1) if re.search(pat, b) else None)
    return dict(facts=int(g(r"facts (\d+)")), conflicting=g(r"conflicting (\[[^\]]*\])"), ENV=g(r"ENV (\w+)"),
                batch=g(r"batch (\w+)"), faults=int(g(r"faults (\d+)")), admissions=int(g(r"admissions (\d+)")),
                gating=int(g(r"gating callers (\d+)")), gating_conflict=g(r"gating conflict (\w+)"),
                ack=int(g(r"acknowledgement actors (\d+)")), attempts=int(g(r"attempts each (\d+)")),
                malformed=g(r"malformed snapshots (\w+)"), cell=g(r"cell constraint (\w+)"),
                unwind=g(r"worker verbs unwind (\w+)"), DROP=g(r"DROP (\w+)"), REPLAY=int(g(r"REPLAY (\d+)")),
                A4=g(r"A4 (\w+)"), ticket=int(g(r"ticket limit (\d+)")), refusal=int(g(r"refusal limit (\d+)")),
                cap=int(g(r"caps (\d+) states")), time_cap=g(r"states, (\d+s)"))


def expect_for(cfg):
    e = dict(BASE)
    e.update(EXPECT[cfg])
    return e


configs = {}
for cfg in ORDER:
    R = {"config": cfg, "test": f"ledger::protocol::model_tests::{TEST[cfg]}"}
    if cfg.startswith("X-"):
        d = f"{V02}/{XDIRS[cfg]}"
        raw = open(f"{d}/raw-output.txt", "rb").read()
        desc = json.load(open(f"{d}/descriptor.json"))
        cls = open(f"{d}/CLASSIFICATION.txt").read().splitlines()[0]
        R.update(receipt=f"VALIDATION-02 receipts/{XDIRS[cfg]}/raw-output.txt (local original; not in Git)",
                 evidence_root="p3-c11-dr01-d2a1-exact-source-validation-02 (MANIFEST 36f10bf9...b822)",
                 git_copy="none (VALIDATION-02 is unpublished)")
    else:
        rev, gdir, ldir, name = DIRS[cfg]
        raw = show(rev, f"{gdir}/{name}/raw-output.txt")
        desc = jshow(rev, f"{gdir}/{name}/descriptor.json")
        cls = show(rev, f"{gdir}/{name}/CLASSIFICATION.txt").decode().splitlines()[0]
        R.update(receipt=f"{gdir.replace(EVB + '/', '')}/{name}/raw-output.txt at {rev[:12]}",
                 evidence_root=ldir.replace(AUDITS + "/", "").replace("/receipts", ""))
        check(f"{cfg}: the Git receipt is unchanged at the evidence head", blob_id(rev, f"{gdir}/{name}/raw-output.txt")
              == blob_id(HEAD_EV, f"{gdir}/{name}/raw-output.txt"))
        lraw = f"{ldir}/{name}/raw-output.txt"
        if os.path.exists(lraw):
            check(f"{cfg}: Git copy byte-equal to the local original receipt", file_sha(lraw) == sha(raw), lraw)
            R["git_copy"] = "byte-equal to local original"
        else:
            check(f"{cfg}: local original receipt present", False, lraw)
    R["raw_sha256"] = sha(raw)
    R["classification_first_line"] = cls
    reps = parse_reports(raw.decode(errors="replace"))
    distinct = {json.dumps({k: v for k, v in r.items() if k not in ("line_index", "end_index", "header")},
                           sort_keys=True) for r in reps}
    check(f"{cfg}: one exploration report (repeats byte-identical)", len(distinct) == 1, len(reps))
    rep = reps[0]
    R["header"] = rep["header"]
    check(f"{cfg}: header names the configuration", rep["config"] == cfg, rep["config"])
    q = cfg in QUOTIENT
    R["explorer"] = ("composed quotient (AQ o SQ), explore_symmetric; original graph NOT explored" if q
                     else "original (explore)" if cfg.startswith("F") else "original (explore), default suite")
    check(f"{cfg}: explorer kind as required (quotient only for F-3/F-PT/F-B)",
          (rep["quotient"] is not None and rep["symmetry"] is not None) == q, rep["header"][:100])
    b = parse_bounds(rep["bounds"])
    R["bounds"] = rep["bounds"]
    R["actors"] = rep.get("actors")
    e = expect_for(cfg)
    mism = {k: (b.get(k), v) for k, v in e.items() if v is not None and b.get(k) != v}
    check(f"{cfg}: printed bounds equal the contract's reviewed bounds (8.2/8.3; executor encoding)", not mism, mism)
    check(f"{cfg}: ticket limit 8 and refusal limit 4", b["ticket"] == 8 and b["refusal"] == 4)
    for k in ("outcome", "states", "expanded", "transitions", "fault_edges", "no_step", "elapsed", "fail", "incomplete"):
        R[k] = rep.get(k)
    ps = props_summary(rep)
    R["property_lines"] = rep["props"]
    R["holds"] = ps["holds"]
    R["not_applicable"] = ps["not_applicable"]
    R["not_claimed"] = ps["not_claimed"]
    R["not_established"] = ps["not_established"]
    R["taken"] = sorted(set(rep["taken"]) | set(rep["unlisted"]))
    R["unlisted_taken"] = rep["unlisted"]
    R["listed_not_taken"] = rep["not_taken"]
    R["quotient_line"] = rep["quotient"]
    R["symmetry_line"] = rep["symmetry"]
    if q:
        kf = re.search(r"(\d+) expanded keys are their own mirror", rep["symmetry"])
        R["Kf"] = int(kf.group(1))
        R["derived_original_states"] = (2 + 1) * (2 * rep["states"] - R["Kf"])
        R["derived_label"] = "DERIVED (AD-1-R4 B; conditional on B(1)); original graph not explored"
    # process / resources
    pr = desc.get("process", {})
    sv = desc.get("supervisor", {})
    R.update(argv=desc.get("argv"), exit_status=pr.get("exit_status"), signal=pr.get("signal"),
             memory_peak_bytes=sv.get("memory_peak_bytes"), memory_max=sv.get("memory_max"),
             swap_peak=sv.get("swap_peak_bytes"), oom_kill=sv.get("oom_kill"), wall_seconds=sv.get("wall_seconds"),
             time_limit=sv.get("time_limit_seconds"), timed_out=sv.get("timed_out"),
             exe_sha_before=(desc.get("executable") or {}).get("sha256_before") or
             ((desc.get("executable") or {}).get("before") or {}).get("sha256"),
             exe_sha_after=(desc.get("executable") or {}).get("sha256_after") or
             ((desc.get("executable") or {}).get("after") or {}).get("sha256"),
             source=desc.get("source_before", {}).get("verdict"), head=desc.get("source_before", {}).get("head"),
             tree=desc.get("source_before", {}).get("tree"))
    check(f"{cfg}: source exactly Candidate C before the run", R["head"] == C and R["tree"] == C_TREE
          and R["source"] == "EXACTLY_C", (R["head"], R["source"]))
    check(f"{cfg}: pinned baseline executable E-C, equal before and after", R["exe_sha_before"] == R["exe_sha_after"] == E_C,
          R["exe_sha_before"])
    check(f"{cfg}: argv selects exactly the configuration's test with --exact",
          R["argv"][1] == R["test"] and "--exact" in R["argv"] and (("--ignored" in R["argv"]) == cfg.startswith("F")),
          R["argv"])
    check(f"{cfg}: 16 GiB, zero swap, no OOM kill, not timed out",
          R["memory_max"] == "17179869184" and R["swap_peak"] in ("0", 0) and R["oom_kill"] in (0, "0")
          and not R["timed_out"])
    if cfg == "F-R":
        check("F-R: raw outcome INCOMPLETE at the 30,000,000-state cap: 30,000,001 discovered, 26,539,923 expanded",
              R["outcome"] == "INCOMPLETE" and R["states"] == 30000001 and R["expanded"] == 26539923)
        check("F-R: every property line NOT ESTABLISHED (incomplete); no HOLDS", R["holds"] == 0 and len(R["not_established"]) == 25)
        check("F-R: classification NOT PASS / INCOMPLETE", cls == "CLASSIFICATION: NOT PASS")
        check("F-R: harness exit 101 (assertion), no signal", R["exit_status"] == 101 and R["signal"] is None)
        R["admissibility"] = "INCOMPLETE — STATE CEILING REACHED (AD-1-R4 K.1); establishes no property"
        R["discharge"] = ("F-R — UNCHANGED-CORE OBLIGATIONS DISCHARGED BY ACCEPTED STUTTERING-REFINEMENT PROOF "
                          "(AD-1-R5 B, C, D; Owner adoption 2026-10-14); NOT an explorer PASS")
    else:
        want_holds = 23 if cfg == "F-D" else 25 if cfg == "F-W" else 24
        check(f"{cfg}: outcome PASS; {want_holds} lines HOLD; no FAIL/INCOMPLETE", R["outcome"] == "PASS" and
              R["holds"] == want_holds and not R["fail"] and not R["incomplete"] and not ps["other"]
              and not R["not_established"], (R["outcome"], R["holds"], ps["other"]))
        check(f"{cfg}: E10 {'HOLDS' if cfg == 'F-W' else 'not applicable'}; L1 {'not claimed' if cfg == 'F-D' else 'HOLDS'}",
              (rep["props"].get("E10", "").startswith("HOLDS") if cfg == "F-W" else R["not_applicable"] == ["E10"])
              and (R["not_claimed"] == ["L1"] if cfg == "F-D" else rep["props"].get("L1", "").startswith("HOLDS")))
        check(f"{cfg}: D1 HOLDS", rep["props"].get("D1", "").startswith("HOLDS"))
        check(f"{cfg}: complete: every discovered key expanded; discovered <= cap", R["states"] == R["expanded"]
              and R["states"] <= e["cap"], (R["states"], R["expanded"], e["cap"]))
        check(f"{cfg}: harness exit 0 by a normal exit", R["exit_status"] == 0 and R["signal"] is None)
        check(f"{cfg}: classification PASS", cls.startswith("CLASSIFICATION: PASS"), cls)
        R["admissibility"] = ("PASS — admissible unchanged-core composed-quotient PASS (AD-1-R4 B-E, N)" if q else
                              "PASS — admissible unchanged-core original-explorer PASS")
        R["discharge"] = ({"F-3": "REPLACED_BY gc_symmetry_f3 (AD-1-R4 E.1/E.3 row 3)",
                           "F-PT": "REPLACED_BY gc_symmetry_fpt (AD-1-R4 E.1/E.3 row 9); also A4's (f) comparison",
                           "F-B": "REPLACED_BY gc_symmetry_fb (AD-1-R4 E.1/E.3 row 10)"}.get(cfg) or
                          ("8.2 default exploration (also run unoptimised inside cargo test --workspace)" if cfg.startswith("X")
                           else "REQUIRED_FINAL_RUN, original explorer (AD-1-R4 E.3)"))
    configs[cfg] = R

# X: the Stage-1 suite (Git) and Stage B (unoptimised workspace test) blocks, anchored on each test's harness line
suite = show(STAGE1, f"{KR}/30-suite/raw-output.txt").decode(errors="replace")
stageb = open(f"{V02}/30-stage-B-workspace-test/raw-output.txt", "rb").read().decode(errors="replace")
gcbin = None
blocks = re.split(r"(?m)^\s*Running ", stageb)
for blk in blocks:
    if blk.startswith("unittests src/lib.rs (target/debug/deps/nexus_governed_control-"):
        m = re.search(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; \d+ filtered out; finished in ([\d.]+)s", blk)
        gcbin = m.groups() if m else None
check("Stage B: nexus-governed-control lib test binary result line found", gcbin is not None, gcbin)
for cfg in ORDER[:5]:
    t = configs[cfg]["test"]
    oks = re.findall(rf"(?m)^test {re.escape(t)} \.\.\. (\S+)$", stageb)
    check(f"{cfg}: VALIDATION-02 Stage B (unoptimised cargo test --workspace): harness line '... ok' exactly once "
          f"(output captured: no report printed)", oks == ["ok"], oks)
    configs[cfg].setdefault("corroborating", []).append(
        "VALIDATION-02 Stage B (cargo test --workspace --locked, unoptimised): '... ok'; report not printed (captured); "
        f"governed-control lib binary {gcbin[0] if gcbin else '?'} passed / {gcbin[1] if gcbin else '?'} failed / "
        f"{gcbin[2] if gcbin else '?'} ignored in {gcbin[3] if gcbin else '?'} s, which bounds the five X runs together "
        "(8.2's 180 s ceiling); individual unoptimised X times NOT MEASURED")
    for name, text in (("Stage-1 suite (pinned E-C, f023cc3d 30-suite)", suite),):
        reps = [r for r in parse_reports(text) if text.splitlines()[r["line_index"]].startswith(f"test {t} ... ")]
        ok = len(reps) == 1 and reps[0].get("outcome") == "PASS"
        same = ok and reps[0]["states"] == configs[cfg]["states"] and reps[0]["transitions"] == configs[cfg]["transitions"] \
            and sorted(set(reps[0]["taken"]) | set(reps[0]["unlisted"])) == configs[cfg]["taken"]
        check(f"{cfg}: {name}: exactly one block on its own harness line, PASS, same counts and branches as Stage C",
              ok and same, (len(reps), reps[0].get("outcome") if reps else None))
        configs[cfg].setdefault("corroborating", []).append(f"{name}: PASS, {reps[0]['states'] if reps else '?'} states")
    ok_line = re.search(rf"(?m)^test {re.escape(t)} \.\.\. .*$", stageb)
check("Stage B raw output sha256 (VALIDATION-02 receipts/30-stage-B-workspace-test/raw-output.txt)", True,
      file_sha(f"{V02}/30-stage-B-workspace-test/raw-output.txt"))
check("Stage-1 suite raw sha256 d6f58742...a3b3", sha(suite.encode()) ==
      "d6f58742ea483dbb95452b186dcf72f729538e672ae60741d978b0eddc6ca3b3" or
      sha(show(STAGE1, f"{KR}/30-suite/raw-output.txt")) == "d6f58742ea483dbb95452b186dcf72f729538e672ae60741d978b0eddc6ca3b3")

# ------------------------------------------------------------------------------------------- 193-row vocabulary
vt = show(HEAD_EV, VOCAB).decode().splitlines()
hdr = vt[0].split("\t")
vocab = [dict(zip(hdr, l.split("\t"))) for l in vt[1:]]
check("vocabulary: 193 rows, unique ids and unique labels", len(vocab) == 193 and len({v["id"] for v in vocab}) == 193
      and len({v["label"] for v in vocab}) == 193, len(vocab))
labels = {v["label"] for v in vocab}
frd = show(HEAD_EV, FRDISP).decode().splitlines()
fh = frd[0].split("\t")
frrows = {r["label"]: r for r in (dict(zip(fh, l.split("\t"))) for l in frd[1:])}
check("FR-BRANCH-DISPOSITION.tsv: 193 rows on the same labels", set(frrows) == labels, len(frrows))
complete = [c for c in ORDER if c != "F-R"]
for c in ORDER:
    out = sorted(set(configs[c]["taken"]) - labels)
    check(f"{c}: every taken label is a vocabulary row", not out, out)
rows = []
counts = {"MEASURED_COMPLETE": 0, "MEASURED_IN_INCOMPLETE_PREFIX": 0, "UNOBSERVED": 0}
derived_taken = 0
for v in vocab:
    lab = v["label"]
    by = [c for c in complete if lab in configs[c]["taken"]]
    by_q = [c for c in by if c in QUOTIENT]
    prefix = lab in configs["F-R"]["taken"]
    fr = frrows.get(lab, {})
    frs = fr.get("F-R status by proof", "")
    if by:
        cls = "MEASURED_COMPLETE"
    elif prefix:
        cls = "MEASURED_IN_INCOMPLETE_PREFIX"
    else:
        cls = "UNOBSERVED"
    counts[cls] += 1
    src = []
    if by:
        src.append("MEASURED_COMPLETE: " + ",".join(c for c in by if c not in QUOTIENT))
        if by_q:
            src.append("MEASURED_COMPLETE_QUOTIENT (taken set equal to the original's only under B(1)): " + ",".join(by_q))
    if prefix:
        src.append("MEASURED_IN_INCOMPLETE_PREFIX: F-R (existence on a reachable transition; not coverage)")
    if frs == "TAKEN":
        derived_taken += 1
        src.append("DERIVED_BY_ACCEPTED_PROOF: F-R TAKEN (AD-1-R5 D; FR-BRANCH-DISPOSITION.tsv)")
    elif frs:
        src.append(f"DERIVED_BY_ACCEPTED_PROOF: F-R {frs}")
    rows.append({"id": v["id"], "operation": v["operation"], "label": lab, "in_static_BRANCHES": v["in_static_BRANCHES"],
                 "classification": cls, "complete_runs_taking": ";".join(by) or "-", "fr_prefix": "TAKEN" if prefix else "not printed",
                 "fr_status_by_proof": frs, "fr_proof_provenance": fr.get("provenance", ""), "sources": " | ".join(src) or
                 "none: no run took it; it is UNOBSERVED (never covered)"})
check("branch accounting: rows sum to 193", sum(counts.values()) == 193, counts)
check("branch accounting: proof never upgrades an unobserved row (every proof-TAKEN row is measured somewhere)",
      all(r["classification"] != "UNOBSERVED" for r in rows if r["fr_status_by_proof"] == "TAKEN"))
check("branch accounting: every F-R-prefix-only row is proof-TAKEN", all(r["fr_status_by_proof"] == "TAKEN" for r in rows
                                                                         if r["classification"] == "MEASURED_IN_INCOMPLETE_PREFIX"))
check("branch accounting: F-2 measured set equals the proof's F-2 column", all(
    (r["label"] in configs["F-2"]["taken"]) == (frrows[r["label"]].get("F-2 complete (measured)") == "TAKEN") for r in rows))
check("branch accounting: F-R prefix set equals the disposition's prefix column", all(
    (r["label"] in configs["F-R"]["taken"]) == (frrows[r["label"]].get("F-R prefix (incomplete)") == "TAKEN") for r in rows))

# ------------------------------------------------------------------------------------------- ignored tests
mt = show(C, "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs").decode()
ut = show(C, "crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs").decode()


def ignored(src):
    out = []
    ls = src.splitlines()
    for i, l in enumerate(ls):
        if re.match(r"\s*#\[ignore", l):
            for j in range(i + 1, min(i + 8, len(ls))):
                m = re.match(r"\s*(?:pub(?:\(\w+\))?\s+)?fn (\w+)", ls[j])
                if m:
                    out.append((m.group(1), j + 1))
                    break
    return out


ig_m, ig_u = ignored(mt), ignored(ut)
check("model_tests.rs at C: 34 #[ignore] tests", len(ig_m) == 34, len(ig_m))
check("unit_tests.rs at C: no #[ignore]", len(ig_u) == 0, ig_u)
# Stage B: every ignored test in the workspace run, grouped by test binary
ign_b, cur = [], None
for l in stageb.splitlines():
    m = re.match(r"\s*Running (\S+) \((\S+)\)", l) or re.match(r"\s*Doc-tests (\S+)", l)
    if m:
        cur = m.group(1)
    m = re.match(r"test (.+?) \.\.\. ignored(?:$|, (.*)$| (\(.*\))$)", l)
    if m:
        ign_b.append((cur, m.group(1), m.group(2) or m.group(3) or ""))
gc = [n for b, n, _ in ign_b if n.startswith("ledger::protocol::model_tests::")]
check("Stage B: 79 '... ignored' entries (78 in libtest result sums + the custom-harness webview_boundary_live)",
      len(ign_b) == 79 and sum(int(x) for x in re.findall(r"test result: \w+\. \d+ passed; \d+ failed; (\d+) ignored", stageb)) == 78,
      len(ign_b))
check("Stage B: the 34 model_tests ignored tests are exactly the source's 34",
      sorted(n.split("::")[-1] for n in gc) == sorted(n for n, _ in ig_m), len(gc))

json.dump({"configs": configs, "branch_rows": rows, "branch_counts": counts, "derived_taken_rows": derived_taken,
           "ignored_model_tests": ig_m, "ignored_unit_tests": ig_u, "stageB_ignored": ign_b},
          open(os.path.join(OUT, "s2-derivation.json"), "w"), indent=1)
fails = [c for c in checks if not c[1]]
with open(os.path.join(OUT, "s2-checks.txt"), "w") as f:
    for label, ok, d in checks:
        f.write(f"{'OK  ' if ok else 'FAIL'} {label}" + (f" :: {d}" if d else "") + "\n")
    f.write(f"\nRESULT: {len(checks) - len(fails)} of {len(checks)} checks hold; {len(fails)} fail\n")
print(f"{len(checks) - len(fails)} of {len(checks)} hold; branch counts {counts}; proof-TAKEN rows {derived_taken}; "
      f"Stage B ignored {len(ign_b)}")
for label, ok, d in fails:
    print("FAIL", label, "::", d[:300])
sys.exit(0 if not fails else 1)
