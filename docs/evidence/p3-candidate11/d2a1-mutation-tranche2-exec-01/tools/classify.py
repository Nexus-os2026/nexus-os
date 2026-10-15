#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission sections 7, 8, 11-16: classify one receipt directory.

Receipt types (AD-1-R4 F.1, read under K.1 and K.2, process-aware and fail-closed):

  e1    unchanged-core exploration PASS (the standalone X-1 baseline on the pinned baseline executable)
  e2    restored named-unit-test PASS (the five standalone named-test baselines)
  e4    healthy-build success (the exact-command healthy build of Candidate C)
  pin   a mutant build's success and its one library-test executable, pinned (not a receipt type of F.1: the build
        half of identity item 4 of AD-1-R4 I; a failure here kills nothing and stops the mission)
  d     C22's intentional mutant compile failure (E0283 at the T-OWN assertion)
  a     original-explorer mutant FAIL of the named property
  b     named non-explorer mutant-test FAIL at one of the named test's own checks

Every classifier keeps two facts apart: the raw outcome, exactly as printed, and the outer class. It writes
CHECKS.txt, PARSED-RECEIPT.json and CLASSIFICATION.txt into the receipt directory, and refuses to classify twice.
The first line of CLASSIFICATION.txt is the only line later steps read:

  e1, e2, e4:  `CLASSIFICATION: PASS` or `CLASSIFICATION: NOT PASS`
  pin:         `BUILD: PINNED` or `BUILD: NOT OK`
  a, b, d:     `CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` or
               `CLASSIFICATION: NOT A KILL EVIDENCE CANDIDATE`

No final kill label is ever written: acceptance belongs to the Sole Architect.

The functions take their expectations as arguments (the frozen EXPECTATIONS.json entry, the pinned executable, the
expected source verdict), so the same code is exercised on synthetic evidence by tool_controls.py before the freeze.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402
import report  # noqa: E402

PANIC = re.compile(r"^thread '([^']*)'(?: \(\d+\))? panicked at (\S+):(\d+):(\d+):$")
PANIC_ANY = re.compile(r"panicked at ")
L1_HOLDS = re.compile(r"^HOLDS: (\d+) non-fault edges, no cycle; (\d+) end states, each terminal \(D1\)$")
CANDIDATE = "CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"
NOT_CANDIDATE = "CLASSIFICATION: NOT A KILL EVIDENCE CANDIDATE"
E10_NA = "not applicable here (worker verbs do not unwind)"
PROFILE = ["opt-level=3", "lto=thin", "codegen-units=16", "strip=symbols"]


class Checks:
    def __init__(self):
        self.items = []

    def __call__(self, label, ok, detail=""):
        self.items.append((label, bool(ok), str(detail)))
        return bool(ok)

    def ok(self):
        return all(o for _, o, _ in self.items)

    def failed(self):
        return [l for l, o, _ in self.items if not o]


def refuse_twice(d):
    if os.path.exists(os.path.join(d, "CLASSIFICATION.txt")):
        raise SystemExit(f"REFUSED: {d} is already classified")


def write(d, first, outer, checks, parsed, extra_lines=()):
    parsed = dict(parsed, outer_classification=outer, checks_hold=sum(o for _, o, _ in checks.items),
                  checks_total=len(checks.items))
    json.dump(parsed, open(os.path.join(d, "PARSED-RECEIPT.json"), "w"), indent=1, default=str)
    with open(os.path.join(d, "CHECKS.txt"), "w") as f:
        for label, ok, detail in checks.items:
            f.write(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + detail) if detail else ''}\n")
    with open(os.path.join(d, "CLASSIFICATION.txt"), "w") as f:
        f.write(first + "\n")
        f.write(f"outer class (AD-1-R4 K.2): {outer}\n")
        for line in extra_lines:
            f.write(line + "\n")
        f.write(f"checks: {sum(o for _, o, _ in checks.items)} of {len(checks.items)} hold\n")
        for label, ok, detail in checks.items:
            if not ok:
                f.write(f"FAILED CHECK: {label}: {detail}\n")
    print(open(os.path.join(d, "CHECKS.txt")).read())
    print(open(os.path.join(d, "CLASSIFICATION.txt")).read())


def load(d):
    desc = json.load(open(os.path.join(d, "descriptor.json")))
    text = None
    if "not_started" not in desc and desc.get("output_path"):
        raw = open(os.path.join(d, desc["output_path"]), "rb").read()
        text = raw.decode("utf-8", errors="replace")
        desc["_raw_sha256"] = K.sha256_bytes(raw)
    return desc, text


def resources_line(sup, limit):
    return (f"resources: wall {sup.get('wall_seconds')} s of {limit} s; scope memory.peak {sup.get('memory_peak_bytes')} B "
            f"(memory.max {sup.get('memory_max')}); swap peak {sup.get('swap_peak_bytes')} B (swap.max "
            f"{sup.get('swap_max')}); oom {sup.get('oom')}, oom_kill {sup.get('oom_kill')}; largest process max RSS "
            f"{sup.get('max_rss_kib_largest_process')} KiB; scope {sup.get('scope_result')}")


def check_supervision(ck, sup, limit):
    return ck("launching command: 16 GiB, zero swap, enforced; no OOM, no kill, no timeout; complete record; within "
              f"{limit} s", sup["completed"] and sup["memory_max"] == K.MEMORY_MAX and sup["swap_max"] == "0"
              and sup["oom_kill"] == 0 and sup["oom"] == 0 and sup["swap_peak_bytes"] == "0" and not sup["timed_out"]
              and sup["time_limit_seconds"] == limit and sup["wall_seconds"] is not None and sup["wall_seconds"] < limit
              and sup["scope_result"] in ("gone", "success"),
              f"memory.peak {sup['memory_peak_bytes']} B, swap.peak {sup['swap_peak_bytes']} B, oom {sup['oom']}, "
              f"oom_kill {sup['oom_kill']}, wall {sup['wall_seconds']} s of {limit} s, timed out {sup['timed_out']}, "
              f"scope {sup['scope_result']}")


def check_identity(ck, desc, exe, source_verdict, mutant_blob=None):
    ex = desc["executable"]
    ids = [ex["before"], desc["selection"]["executable_after_listing"], ex["after"]]
    ck("executable is the pinned one (path, sha256, size) before, at selection and after",
       ex["path"] == exe["path"] and desc["argv"][0] == exe["path"]
       and all(i["sha256"] == exe["sha256"] and i["size"] == exe["size"] and not i["symlink"] and i["path"] == exe["path"]
               for i in ids),
       " / ".join(f"{i['sha256'][:16]}… {i['size']}" for i in ids) + f" (pinned {exe['sha256'][:16]}… {exe['size']})")
    sb, sa = desc["source_before"], desc["source_after"]
    good = sb["verdict"] == source_verdict and sa["verdict"] == source_verdict
    if mutant_blob is not None:
        good = good and all((s.get("mutant_file") or {}).get("git_blob") == mutant_blob for s in (sb, sa))
    ck(f"source {source_verdict} before and after" + (f" (mutant blob {mutant_blob})" if mutant_blob else ""), good,
       f"{sb['verdict']} / {sa['verdict']}")
    ck("raw output sha256 equals the launcher's record", desc["_raw_sha256"] == desc["raw_output_sha256"],
       desc["_raw_sha256"])


def check_selection(ck, desc, exe, test, argv_tail, ignored):
    ck("argv: the pinned executable, exactly the one test and flags " + " ".join(argv_tail[1:]),
       desc["argv"] == [exe["path"], *argv_tail], desc["argv"])
    sel = desc["selection"]
    ck("selection listing (--exact --list) shows exactly that one test",
       sel["listing"]["exit"] == 0 and sel["listing"]["listed"] == [test + ": test"], sel["listing"]["listed"])
    ck(f"ignored status from the executable's own listing (--exact --ignored --list): {'ignored' if ignored else 'not ignored'}"
       f"; --ignored passed iff ignored", sel["ignored_listing"]["exit"] == 0
       and sel["ignored_listing"]["listed"] == ([test + ": test"] if ignored else [])
       and (("--ignored" in argv_tail) == ignored), sel["ignored_listing"]["listed"])


def harness(text, test):
    starts = re.findall(r"^test (\S+) \.\.\. ", text, re.M)
    running = re.findall(r"^running (\d+) tests?$", text, re.M)
    result = re.findall(r"^test result: .*$", text, re.M)
    fails = re.findall(r"^failures:\n((?:    \S+\n)+)", text, re.M)
    failed_names = [n.strip() for block in fails for n in block.splitlines()]
    return starts, running, result, failed_names


def panics(text):
    """Every panic header; and every line mentioning 'panicked at' (so a malformed header cannot hide)."""
    lines = text.splitlines()
    heads = [(i, PANIC.match(l)) for i, l in enumerate(lines) if PANIC.match(l)]
    anyp = [l for l in lines if PANIC_ANY.search(l)]
    return lines, heads, anyp


def panic_message(lines, i):
    out = []
    for l in lines[i + 1:]:
        if l.startswith("note: run with `RUST_BACKTRACE=1`") or PANIC.match(l) or l.startswith("test result:"):
            break
        out.append(l)
    return out


# ---- e1 / e2: baselines on the pinned baseline executable ---------------------------------------------------------

def classify_e1(d, b, exe, source_verdict="EXACTLY_C", vocab_check=True):
    refuse_twice(d)
    ck = Checks()
    desc, text = load(d)
    started = ck("the run started (no pre-flight refusal)", "not_started" not in desc, desc.get("not_started", ""))
    if not started:
        return write(d, "CLASSIFICATION: NOT PASS", "NOT ADMISSIBLE (not started)", ck, {"baseline": b["key"]}) or 1
    proc, sup = desc["process"], desc["supervisor"]
    check_identity(ck, desc, exe, source_verdict)
    check_selection(ck, desc, exe, b["test"], b["argv_tail"], b["ignored"])
    starts, running, result, failed_names = harness(text, b["test"])
    ck("the harness ran exactly that one test", running == ["1"] and starts == [b["test"]],
       f"running {running}; started {starts}")
    want = f"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; {K.LISTED - 1} filtered out; finished in "
    ck(f"harness: 'ok' then '{want}…'", len(result) == 1 and result[0].startswith(want)
       and re.search(r"\n\nok\n\ntest result: ok\.", text), result)
    ck("normal exit 0, no signal", proc["exit_status"] == 0 and proc["signal"] is None,
       f"exit {proc['exit_status']}, signal {proc['signal']}, supervisor rc {proc['supervisor_rc']}")
    check_supervision(ck, sup, b["limit_seconds"])
    _, heads, anyp = panics(text)
    ck("no panic, no fixture failure, no 'FAILED' or 'failures:'", not heads and not anyp and "FAILED" not in text
       and "failures:" not in text and "WITNESS FIXTURE FAILURE" not in text, anyp[:2])
    ck("no quotient or symmetry refusal", "inapplicable" not in text)
    rep = report.parse_report(text)
    ck("exactly one exploration report", rep is not None and text.count("EXPLORATION ") == 1)
    table = None
    if rep is not None:
        ck("header 'EXPLORATION X-1' exactly (the original explorer)", rep["header"] == "EXPLORATION X-1"
           and "quotient_line" not in rep and "symmetry_line" not in rep, rep["header"])
        ck("outcome PASS; no INCOMPLETE line, no FAIL line", rep.get("outcome") == "PASS" and rep["incomplete"] is None
           and rep["fail"] is None, f"{rep.get('outcome')}; {rep['incomplete']}; {rep['fail']}")
        ck("printed bounds equal the source-derived configuration", rep.get("bounds") == b["bounds"], rep.get("bounds"))
        ck("printed actors equal actors() of the configuration", rep.get("actors") == b["actors"], rep.get("actors"))
        ck(f"caps printed: {b['ceiling_states']:,} states, {b['limit_seconds']} s (the default ceiling)",
           rep.get("cap") == b["ceiling_states"] and rep.get("time_cap_s") == b["limit_seconds"],
           f"{rep.get('cap')}, {rep.get('time_cap_s')}")
        have = all(k in rep for k in ("states", "expanded", "transitions", "fault_edges", "no_step"))
        ck("counts printed", have)
        if have:
            ck(f"discovered states <= {b['ceiling_states']:,} (K.1, inclusive)", rep["states"] <= b["ceiling_states"],
               f"{rep['states']:,}")
            ck("expanded states equal discovered states (0 unexpanded)", rep["expanded"] == rep["states"],
               f"states {rep['states']}, expanded {rep['expanded']}")
        names = [k for k, _ in rep["properties"]]
        props, branches = report.model_constants()
        ck("one line for each of the 25 PROPERTIES, in order", names == props, names)
        pv = dict(rep["properties"])
        bad = []
        for p, form in b["properties"].items():
            got = pv.get(p)
            if form == "L1-HOLDS":
                m = L1_HOLDS.match(got or "")
                if not m or (have and (int(m.group(1)) != rep["transitions"] - rep["fault_edges"]
                                       or int(m.group(2)) != rep["no_step"])):
                    bad.append((p, got))
            elif got != form:
                bad.append((p, got))
        ck("every property line has its source-derived form: 24 HOLD (L1 with consistent counts), E10 not applicable",
           not bad and sum(bool(v == "HOLDS" or (p == "L1" and L1_HOLDS.match(v))) for p, v in rep["properties"]) == 24
           and pv.get("E10") == E10_NA, bad)
        labels = [lab for _, lab in rep["branches"]]
        ck("branch list: every BRANCHES label once, in order, each taken or NOT TAKEN",
           labels == branches and all(m in ("taken    ", "NOT TAKEN") for m, _ in rep["branches"]),
           f"{len(labels)} of {len(branches)}")
        ck("state key fields line present", rep["key_fields"] is not None)
        if vocab_check:
            import normalize
            vocab, vsha = normalize.load_vocab()
            import configs
            derived = configs.derive()
            table, probs = normalize.normalize(text, "X-1", vocab, derived,
                                               f"this mission's raw output {os.path.join(d, 'raw-output.txt')}, sha256 "
                                               f"{desc['_raw_sha256']}")
            if table is not None:
                table["vocabulary_sha256"] = vsha
                normalize.write(table, d, "BRANCH-ACCOUNTING-X-1")
            ck("complete branch receipt: every raw label resolved to the frozen Stage-2 vocabulary (NORMALIZED)",
               table is not None and not probs, probs)
    good = ck.ok()
    outer = "PASS — admissible unchanged-core original-explorer X-1 PASS (type (e1))" if good else (
        "INCOMPLETE (killed from outside or abnormal termination; never PASS)"
        if proc["signal"] is not None or sup["timed_out"] or sup["oom_kill"]
        else "NOT ADMISSIBLE (" + "; ".join(ck.failed()) + ")")
    extra = []
    if rep:
        extra.append(f"raw outcome (as printed): outcome {rep.get('outcome')}; harness {result}")
        if all(k in rep for k in ("states", "expanded", "transitions", "fault_edges", "no_step")):
            extra.append(f"measured: states {rep['states']:,} (expanded {rep['expanded']:,}) of the {b['ceiling_states']:,} "
                         f"ceiling; transitions {rep['transitions']:,} (with a fault {rep['fault_edges']:,}); no "
                         f"enabled step {rep['no_step']:,}; elapsed {rep.get('elapsed_s')} s")
    if table:
        extra.append(f"branch accounting: {table['verdict']}; TAKEN {table['totals']['TAKEN']}, NOT TAKEN "
                     f"{table['totals']['NOT TAKEN']}, NOT APPLICABLE {table['totals']['NOT APPLICABLE']}")
    extra.append(resources_line(sup, b["limit_seconds"]))
    write(d, "CLASSIFICATION: PASS" if good else "CLASSIFICATION: NOT PASS", outer, ck,
          {"baseline": b["key"], "receipt_type": "AD-1-R4 F.1 (e1)", "test": b["test"], "configuration": "X-1",
           "report": {k: rep.get(k) for k in ("header", "outcome", "bounds", "actors", "states", "expanded",
                                               "transitions", "fault_edges", "no_step", "elapsed_s", "cap",
                                               "time_cap_s", "properties", "branches")} if rep else None,
           "harness_result": result, "process": proc, "supervisor": sup, "executable": desc["executable"],
           "selection": desc["selection"], "raw_output_sha256": desc["raw_output_sha256"],
           "branch_accounting": ({k: table[k] for k in ("verdict", "totals", "taken_outside_static")} if table else None)},
          extra)
    return 0 if good else 1


def classify_e2(d, b, exe, source_verdict="EXACTLY_C"):
    refuse_twice(d)
    ck = Checks()
    desc, text = load(d)
    if not ck("the run started (no pre-flight refusal)", "not_started" not in desc, desc.get("not_started", "")):
        return write(d, "CLASSIFICATION: NOT PASS", "NOT ADMISSIBLE (not started)", ck, {"baseline": b["key"]}) or 1
    proc, sup = desc["process"], desc["supervisor"]
    check_identity(ck, desc, exe, source_verdict)
    check_selection(ck, desc, exe, b["test"], b["argv_tail"], False)
    starts, running, result, failed_names = harness(text, b["test"])
    ck("the harness ran exactly that one test", running == ["1"] and starts == [b["test"]],
       f"running {running}; started {starts}")
    want = f"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; {K.LISTED - 1} filtered out; finished in "
    ck(f"harness: 'test {b['test']} ... ok' then '{want}…'", len(result) == 1 and result[0].startswith(want)
       and re.search(rf"^test {re.escape(b['test'])} \.\.\. ok$", text, re.M), result)
    ck("normal exit 0, no signal", proc["exit_status"] == 0 and proc["signal"] is None,
       f"exit {proc['exit_status']}, signal {proc['signal']}, supervisor rc {proc['supervisor_rc']}")
    check_supervision(ck, sup, b["limit_seconds"])
    _, heads, anyp = panics(text)
    ck("no panic, no fixture failure, no 'FAILED' or 'failures:'", not heads and not anyp and "FAILED" not in text
       and "failures:" not in text and "WITNESS FIXTURE FAILURE" not in text, anyp[:2])
    ck("no explorer report (NOT PRODUCED BY DESIGN for type (e2))", "EXPLORATION " not in text)
    good = ck.ok()
    outer = f"PASS — admissible restored named-unit-test {b['key']} PASS (type (e2))" if good else (
        "INCOMPLETE (killed from outside or abnormal termination; never PASS)"
        if proc["signal"] is not None or sup["timed_out"] or sup["oom_kill"]
        else "NOT ADMISSIBLE (" + "; ".join(ck.failed()) + ")")
    write(d, "CLASSIFICATION: PASS" if good else "CLASSIFICATION: NOT PASS", outer, ck,
          {"baseline": b["key"], "receipt_type": "AD-1-R4 F.1 (e2)", "test": b["test"], "harness_result": result,
           "process": proc, "supervisor": sup, "executable": desc["executable"], "selection": desc["selection"],
           "raw_output_sha256": desc["raw_output_sha256"],
           "not_produced_by_design": "the explorer report, counts, property lines and branch list (F.1 (e2))"},
          [f"raw outcome (as printed): harness {result}", resources_line(sup, b["limit_seconds"])])
    return 0 if good else 1


# ---- builds ---------------------------------------------------------------------------------------------------------

def build_facts(d, desc):
    stderr = open(os.path.join(d, desc["stderr_path"]), errors="replace").read()
    stdout = open(os.path.join(d, desc["stdout_path"]), errors="replace").read()
    rustc = [l for l in stderr.splitlines() if "--crate-name nexus_governed_control " in l
             and l.lstrip().startswith("Running `")]
    flags = {}
    if len(rustc) == 1:
        line = rustc[0]
        flags = {"-C": re.findall(r" -C (\S+)", line), "--cfg": re.findall(r" --cfg (\S+)", line),
                 "--check-cfg": re.findall(r" --check-cfg ('[^']*'|\S+)", line), "--test": " --test " in line,
                 "--edition": re.findall(r" --edition=(\S+)", line)}
    return stderr, stdout, rustc, flags


def check_build_common(ck, desc, source_verdict, mutant_blob=None):
    sb, sa = desc["source_before"], desc["source_after"]
    good = sb["verdict"] == source_verdict and sa["verdict"] == source_verdict
    if mutant_blob is not None:
        good = good and all((s.get("mutant_file") or {}).get("git_blob") == mutant_blob for s in (sb, sa))
    ck(f"source {source_verdict} before and after the build", good, f"{sb['verdict']} / {sa['verdict']}")
    ck("the exact build command, the worktree as cwd", desc["argv"] == K.BUILD_ARGV and desc["cwd"] == desc["worktree"],
       " ".join(desc["argv"]))
    want_env = dict(K.build_env(desc["target"]))
    ck("the exact environment (env -i): HOME, PATH, CARGO_TARGET_DIR=<worktree>/target, CARGO_TERM_VERBOSE, "
       "CARGO_NET_OFFLINE=true; no RUSTFLAGS/CARGO_ENCODED_RUSTFLAGS/feature/cfg variable",
       desc["env"] == want_env and desc["target"] == os.path.join(desc["worktree"], "target"), desc["env"])


def check_flags(ck, rustc, flags, fingerprints):
    ck("one rustc invocation compiled nexus_governed_control, as a test", len(rustc) == 1 and flags.get("--test"),
       f"{len(rustc)} invocation(s)")
    if flags:
        ck("profile flags: " + ", ".join(PROFILE) + " (no debuginfo/debug-assertions/overflow-checks/panic flag)",
           [f for f in flags["-C"] if not f.startswith(("metadata=", "extra-filename="))] == PROFILE, flags["-C"])
        ck("no --cfg flag (no feature, no extra cfg); declared feature set empty",
           flags["--cfg"] == [] and "'cfg(feature, values())'" in flags["--check-cfg"], flags["--cfg"])


def fingerprints(target, exe_name):
    out = []
    if not exe_name:
        return out
    fp = os.path.join(target, "release/.fingerprint", "nexus-governed-control-" + exe_name.split("-")[-1])
    if os.path.isdir(fp):
        for n in sorted(os.listdir(fp)):
            if n.endswith(".json"):
                dd = json.load(open(os.path.join(fp, n)))
                out.append({"file": n, "rustc": dd.get("rustc"), "features": dd.get("features"),
                            "declared_features": dd.get("declared_features"), "rustflags": dd.get("rustflags"),
                            "profile": dd.get("profile"), "config": dd.get("config")})
    return out


def classify_build_success(d, kind, source_verdict, mutant_blob=None, expect_target=None):
    """kind 'e4' (the healthy build) or 'pin' (a mutant build's success)."""
    refuse_twice(d)
    ck = Checks()
    desc = json.load(open(os.path.join(d, "descriptor.json")))
    succ, fail_first = (("CLASSIFICATION: PASS", "CLASSIFICATION: NOT PASS") if kind == "e4"
                        else ("BUILD: PINNED", "BUILD: NOT OK"))
    if not ck("the build started (no pre-flight refusal)", "not_started" not in desc, desc.get("not_started", "")):
        return write(d, fail_first, "NOT ADMISSIBLE (not started)", ck, {"type": kind}) or 1
    proc, sup = desc["process"], desc["supervisor"]
    check_build_common(ck, desc, source_verdict, mutant_blob)
    if expect_target:
        ck("the target directory is the worktree-local fresh one", desc["target"] == expect_target, desc["target"])
    ck("stdout and stderr sha256 equal the launcher's record",
       K.sha256_file(os.path.join(d, "build.stdout")) == desc["stdout_sha256"]
       and K.sha256_file(os.path.join(d, "build.stderr")) == desc["stderr_sha256"])
    ck("normal exit 0, no signal", proc["exit_status"] == 0 and proc["signal"] is None,
       f"exit {proc['exit_status']}, signal {proc['signal']}, supervisor rc {proc['supervisor_rc']}")
    check_supervision(ck, sup, K.BUILD_LIMIT)
    stderr, stdout, rustc, flags = build_facts(d, desc)
    finished = re.findall(r"^\s*Finished `(\w+)` profile \[([^\]]+)\] target\(s\) in (.+)$", stderr, re.M)
    ck("cargo's Finished line: release profile, optimized", len(finished) == 1 and finished[0][:2] == ("release",
                                                                                                         "optimized"),
       finished)
    errs = [l for l in stderr.splitlines() if re.match(r"^(error|warning)(\[E\d+\])?:", l)]
    ck("no error or warning diagnostic", not errs, errs[:3])
    ck("no compiler crash text", not re.search(r"internal compiler error|panicked|query stack|SIGSEGV|signal: ",
                                               stderr), "")
    verbose = re.findall(r"^\s*Executable `([^`]+)`\s*$", stderr, re.M)
    plain = re.findall(r"^\s*Executable unittests src/lib\.rs \(([^)]+)\)\s*$", stderr, re.M)
    named = [p if os.path.isabs(p) else os.path.join(desc["worktree"], p) for p in verbose + plain]
    ck("exactly one library-test executable named by cargo (both line forms)", len(named) == 1,
       f"verbose {verbose}; plain {plain}")
    tests = [a for a in desc["deps_artifacts"] if a["test_executable_name"]]
    ck("the library-test executables in the fresh target's release/deps equal the one cargo named",
       [a["path"] for a in tests] == named and all(a["executable_bit"] and a["sha256"] for a in tests),
       [(os.path.basename(a["path"]), a["sha256"], a["size"]) for a in tests])
    others = [os.path.basename(a["path"]) for a in desc["deps_artifacts"] if not a["test_executable_name"]]
    ck("no other nexus_governed_control artifact in release/deps but the executable's dep-info",
       all(o.endswith(".d") for o in others), others)
    check_flags(ck, rustc, flags, None)
    fps = fingerprints(desc["target"], os.path.basename(named[0]) if len(named) == 1 else None)
    ck("fingerprint of the test unit: no rustflags, no features", fps and all(f["rustflags"] == [] and
                                                                                  f["features"] == "[]" for f in fps),
       [(f["file"], f["rustflags"], f["features"]) for f in fps])
    if kind == "e4":
        tm = desc.get("target_manifest") or {}
        ck("every artifact the build produced is recorded by path, sha256 and size (TARGET-MANIFEST.tsv)",
           tm.get("files", 0) > 0 and K.sha256_file(os.path.join(d, tm.get("path", "TARGET-MANIFEST.tsv"))) ==
           tm.get("sha256"), f"{tm.get('files')} files, {tm.get('bytes')} bytes")
    good = ck.ok()
    exe = None
    if good:
        t = tests[0]
        exe = {"path": t["path"], "sha256": t["sha256"], "size": t["size"], "source_verdict": source_verdict,
               "mutant_blob": mutant_blob, "build_receipt": os.path.relpath(d, K.EV), "effective_rustc_flags": flags,
               "rustc": desc["toolchain"]["rustc"], "fingerprints": fps}
        json.dump(exe, open(os.path.join(d, "PIN.json" if kind == "pin" else "EXECUTABLES.json"), "w"), indent=1)
    if kind == "e4":
        outer = ("PASS — admissible healthy-build type-(e4) success (exact command, Candidate C)" if good else
                 "NOT ADMISSIBLE (" + "; ".join(ck.failed()) + ")")
    else:
        outer = ("mutant build succeeded; one library-test executable pinned" if good else
                 "BUILD NOT OK (" + "; ".join(ck.failed()) + "): kills nothing; the mission stops")
    extra = [f"raw outcome (as printed): exit {proc['exit_status']}, signal {proc['signal']}; Finished {finished}",
             resources_line(sup, K.BUILD_LIMIT)]
    if exe:
        extra.append(f"executable: {exe['path']} sha256 {exe['sha256']} size {exe['size']}")
    write(d, succ if good else fail_first, outer, ck,
          {"type": kind, "argv": desc["argv"], "env": desc["env"], "cwd": desc["cwd"], "process": proc,
           "supervisor": sup, "finished": finished, "named_executables": named, "deps_artifacts": desc["deps_artifacts"],
           "effective_rustc_flags": flags, "fingerprints": fps, "toolchain": desc["toolchain"],
           "stdout_sha256": desc["stdout_sha256"], "stderr_sha256": desc["stderr_sha256"],
           "target_manifest": desc.get("target_manifest"), "executable": exe}, extra)
    return 0 if good else 1


def classify_d(d, ctl, source_verdict="EXACTLY_C_PLUS_PATCH"):
    """C22: the mutant source's build ends by the compiler's intentional nonzero result, E0283 at the T-OWN line."""
    refuse_twice(d)
    ck = Checks()
    desc = json.load(open(os.path.join(d, "descriptor.json")))
    if not ck("the build started (no pre-flight refusal)", "not_started" not in desc, desc.get("not_started", "")):
        return write(d, NOT_CANDIDATE, "NOT ADMISSIBLE (not started)", ck, {"control": ctl["id"]}) or 1
    proc, sup = desc["process"], desc["supervisor"]
    check_build_common(ck, desc, source_verdict, ctl["mutant_blob"])
    ck("stdout and stderr sha256 equal the launcher's record",
       K.sha256_file(os.path.join(d, "build.stdout")) == desc["stdout_sha256"]
       and K.sha256_file(os.path.join(d, "build.stderr")) == desc["stderr_sha256"])
    ck("intentional nonzero termination by a normal exit (no signal)", proc["signal"] is None
       and proc["exit_status"] not in (None, 0), f"exit {proc['exit_status']}, signal {proc['signal']}, supervisor rc "
                                                 f"{proc['supervisor_rc']}")
    check_supervision(ck, sup, K.BUILD_LIMIT)
    stderr, stdout, rustc, flags = build_facts(d, desc)
    lines = stderr.splitlines()
    diags = [(i, m) for i, l in enumerate(lines) for m in [re.match(r"^(error|warning)(?:\[(E\d{4})\])?: (.*)$", l)] if m]
    coded = [(i, m) for i, m in diags if m.group(1) == "error" and m.group(2)]
    uncoded_err = [lines[i] for i, m in diags if m.group(1) == "error" and not m.group(2)]
    warnings = [lines[i] for i, m in diags if m.group(1) == "warning"]
    ck("exactly one coded error diagnostic, and it is E0283", [m.group(2) for _, m in coded] == [ctl["diagnostic_code"]],
       [lines[i] for i, _ in coded])
    loc = None
    if len(coded) == 1:
        i0 = coded[0][0]
        arrow = next((re.match(r"^\s*--> (\S+):(\d+):(\d+)$", l) for l in lines[i0 + 1:i0 + 4]
                      if re.match(r"^\s*--> ", l)), None)
        loc = (arrow.group(1), int(arrow.group(2)), int(arrow.group(3))) if arrow else None
        ck(f"E0283's primary location is the T-OWN assertion, {ctl['diagnostic_file']}:{ctl['diagnostic_line']}",
           loc is not None and loc[0] == ctl["diagnostic_file"] and loc[1] == ctl["diagnostic_line"], loc)
        span = "\n".join(lines[i0:i0 + 8])
        ck("the rendered source line is the assertion text",
           re.search(rf"^{ctl['diagnostic_line']} \|\s+{re.escape(ctl['diagnostic_source_line'])}$", span, re.M), "")
    ok_summary = [l for l in uncoded_err if re.fullmatch(
        r"error: could not compile `nexus-governed-control` \(lib test\) due to 1 previous error(?:; \d+ warnings? "
        r"emitted)?", l)]
    ck("no other error diagnostic: the only uncoded error line is cargo's one summary for nexus-governed-control "
       "(lib test) due to 1 previous error", len(uncoded_err) == 1 and ok_summary == uncoded_err, uncoded_err)
    ck("no compiler crash (no internal compiler error, panic or query stack), no signal",
       not re.search(r"internal compiler error|panicked|query stack|SIGSEGV|signal: \d", stderr), "")
    caused = re.findall(r"process didn't exit successfully: `(.*?)` \((.*?)\)", stderr)
    caused_ngc = [s for c, s in caused if "--crate-name nexus_governed_control " in c]
    ck("where cargo reports rustc's own status, it is 'exit status: 1' for nexus_governed_control only",
       all(s == "exit status: 1" for s in caused_ngc) and len(caused) == len(caused_ngc), caused_ngc or caused)
    compiling = re.findall(r"^\s*Compiling nexus-governed-control v", stderr, re.M)
    ck("nexus-governed-control was compiled once, by one rustc invocation as a test (a dependency failure would add an error line)",
       len(compiling) == 1 and len(rustc) == 1 and " --test " in rustc[0], f"{len(compiling)} Compiling, "
                                                                           f"{len(rustc)} rustc")
    tests = [a for a in desc["deps_artifacts"] if a["test_executable_name"]]
    ck("no mutant test executable exists (NOT PRODUCED BY DESIGN); any other artifact recorded as found",
       not tests, [(os.path.basename(a["path"]), a["sha256"], a["size"]) for a in desc["deps_artifacts"]])
    tm = desc.get("target_manifest") or {}
    ck("every artifact left in the target directory is recorded by path, sha256 and size (TARGET-MANIFEST.tsv)",
       tm.get("files", 0) > 0 and K.sha256_file(os.path.join(d, tm.get("path", "TARGET-MANIFEST.tsv"))) == tm.get("sha256"),
       f"{tm.get('files')} files, {tm.get('bytes')} bytes")
    good = ck.ok()
    outer = ("FAIL (build) — admissible type-(d) intentional mutant compile failure: E0283 at "
             f"{ctl['diagnostic_file']}:{ctl['diagnostic_line']}" if good else
             "INCOMPLETE (killed from outside or abnormal termination)"
             if proc["signal"] is not None or sup["timed_out"] or sup["oom_kill"]
             else "NOT ADMISSIBLE (" + "; ".join(ck.failed()) + ")")
    write(d, CANDIDATE if good else NOT_CANDIDATE, outer, ck,
          {"control": ctl["id"], "receipt_type": "AD-1-R4 F.1 (d)", "argv": desc["argv"], "env": desc["env"],
           "process": proc, "supervisor": sup, "diagnostics": [lines[i] for i, _ in diags], "primary_location": loc,
           "warnings": warnings, "rustc_status": caused, "deps_artifacts": desc["deps_artifacts"],
           "effective_rustc_flags": flags, "toolchain": desc["toolchain"], "stderr_sha256": desc["stderr_sha256"],
           "stdout_sha256": desc["stdout_sha256"], "target_manifest": desc.get("target_manifest"),
           "mutant_executable": "NOT PRODUCED BY DESIGN", "mutant_test_run": "NOT PRODUCED BY DESIGN"},
          [f"raw outcome (as printed): exit {proc['exit_status']}, signal {proc['signal']}; diagnostics "
           f"{[lines[i] for i, _ in diags]}; primary location {loc}",
           f"candidate: {'KILL_EVIDENCE_CANDIDATE — type (d) — ARCHITECT REVIEW PENDING' if good else 'none'}",
           "mutant executable: NOT PRODUCED BY DESIGN; mutant test run: NOT PRODUCED BY DESIGN",
           resources_line(sup, K.BUILD_LIMIT)])
    return 0 if good else 1


# ---- mutant runs: (a) and (b) ---------------------------------------------------------------------------------------

def classify_a(d, ctl, exe, source_verdict="EXACTLY_C_PLUS_PATCH"):
    refuse_twice(d)
    ck = Checks()
    desc, text = load(d)
    if not ck("the run started (no pre-flight refusal)", "not_started" not in desc, desc.get("not_started", "")):
        return write(d, NOT_CANDIDATE, "NOT ADMISSIBLE (not started)", ck, {"control": ctl["id"]}) or 1
    proc, sup = desc["process"], desc["supervisor"]
    P, cfg, cap, limit = ctl["named_property"], ctl["configuration"], ctl["ceiling_states"], ctl["limit_seconds"]
    # Tranche 2 (frozen catalog: C3a and A6 name two properties, special_attribution_rules "AD-1-R4 F.1(a): either
    # property the row names"): the permitted printed properties. Absent for every other control, which permits exactly
    # its one named property, with every check, label and output unchanged.
    allowed = ctl.get("permitted_properties", [P])
    check_identity(ck, desc, exe, source_verdict, ctl["mutant_blob"])
    check_selection(ck, desc, exe, ctl["test"], ctl["argv_tail"], ctl["ignored"])
    starts, running, result, failed_names = harness(text, ctl["test"])
    ck("the harness ran exactly that one test", running == ["1"] and starts == [ctl["test"]],
       f"running {running}; started {starts}")
    want = f"test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; {K.LISTED - 1} filtered out; finished in "
    ck(f"harness: '{want}…' and the failures list names exactly that test", len(result) == 1
       and result[0].startswith(want) and failed_names == [ctl["test"]], f"{result}; failures {failed_names}")
    ck("exit 101 by a normal exit, no signal", proc["exit_status"] == 101 and proc["signal"] is None,
       f"exit {proc['exit_status']}, signal {proc['signal']}, supervisor rc {proc['supervisor_rc']}")
    check_supervision(ck, sup, limit)
    lines, heads, anyp = panics(text)
    va = ctl["verdict_assertion"]
    msg = []
    ck("exactly one panic, of that test's thread", len(heads) == 1 and len(anyp) == 1
       and heads[0][1].group(1) == ctl["test"], [l for l in anyp][:3])
    if len(heads) == 1:
        h = heads[0][1]
        ck(f"the panic is the explorer's own verdict assertion ({ctl['runner']}, {va['file']}:{va['line']}:{va['col']})",
           (h.group(2), int(h.group(3)), int(h.group(4))) == (va["file"], va["line"], va["col"]),
           f"{h.group(2)}:{h.group(3)}:{h.group(4)}")
        msg = panic_message(lines, heads[0][0])
        ck(f"the assertion message carries failed: {{\"{P}\"}}" if len(allowed) == 1 else
           f"the assertion message carries failed: {{\"<p>\"}} for one permitted property p of {allowed}",
           msg[:1] in [[f'assertion `left == right` failed: failed: {{"{p}"}}'] for p in allowed], msg[:1])
        ck("the assertion's operands: left Fail, right Pass", msg[-2:] == ["  left: Fail", " right: Pass"], msg[-2:])
    ck("no quotient or symmetry refusal; no fixture failure", "inapplicable" not in text
       and "WITNESS FIXTURE FAILURE" not in text)
    rep = report.parse_report(text)
    ck("the report is printed, then repeated verbatim in the assertion message", rep is not None
       and text.count("EXPLORATION ") == 2 and len(heads) == 1 and "\n".join(msg[1:-3]) == "\n".join(
           [rep["header"]] + rep["lines"]), f"{text.count('EXPLORATION ')} EXPLORATION headers")
    if rep is not None:
        ck(f"header 'EXPLORATION {cfg}' exactly (the original explorer; no quotient or symmetry line)",
           rep["header"] == f"EXPLORATION {cfg}" and "quotient_line" not in rep and "symmetry_line" not in rep,
           rep["header"])
        ck("printed bounds equal the configuration's (source-derived)", rep.get("bounds") == ctl["bounds"],
           rep.get("bounds"))
        ck("printed actors equal the configuration's", rep.get("actors") == ctl["actors"], rep.get("actors"))
        ck(f"caps printed: {cap:,} states, {limit} s (the configuration's adopted ceiling)",
           rep.get("cap") == cap and rep.get("time_cap_s") == limit, f"{rep.get('cap')}, {rep.get('time_cap_s')}")
        ck("raw outcome FAIL; no INCOMPLETE line", rep.get("outcome") == "FAIL" and rep["incomplete"] is None,
           f"{rep.get('outcome')}; {rep['incomplete']}")
        prop = (rep["fail"] or "").split(": ", 1)[0] if rep["fail"] else None
        if len(allowed) == 1:
            ck(f"the printed property is the named property {P} (no attribution exception in the catalog)", prop == P,
               rep["fail"])
        else:
            ck(f"the printed property is one of the permitted properties {allowed} (catalog: AD-1-R4 F.1(a), either "
               f"property the row names)", prop in allowed, rep["fail"])
            ck("the assertion message names the printed property",
               msg[:1] == [f'assertion `left == right` failed: failed: {{"{prop}"}}'], msg[:1])
        have = all(k in rep for k in ("states", "expanded"))
        ck("'states <n> (expanded <m>)' printed", have)
        if have:
            ck(f"discovered count <= the ceiling {cap:,} (K.1, inclusive)", rep["states"] <= cap, f"{rep['states']:,}")
        ck("the trace is printed under 'shortest trace (<n> steps):'",
           re.fullmatch(r"  shortest trace \((\d+) steps\):", rep["trace_steps"] or "") is not None, rep["trace_steps"])
        ck("a FAIL prints only its failing property (no property lines)", rep["properties"] == [], rep["properties"][:3])
        props, branches = report.model_constants()
        ck("the failing property is one of PROPERTIES", prop in props, prop)
        labels = [lab for _, lab in rep["branches"]]
        ck("the report is complete: branch list (every BRANCHES label, in order) and the key-fields line",
           labels == branches and rep["key_fields"] is not None, f"{len(labels)} of {len(branches)}")
        if ctl.get("key_fields") is not None:
            # Tranche 2, A6 only (catalog R2, receipt_metadata_item KEY_FIELDS): the printed key fields are the
            # mutant's KEY_FIELDS, which name recovery_required.
            ck("the printed state key fields are the mutant's KEY_FIELDS and name recovery_required (catalog R2)",
               rep["key_fields"] == ctl["key_fields"] and "recovery_required" in ctl["key_fields"],
               (rep["key_fields"] or "")[-160:])
    good = ck.ok()
    prop = (rep["fail"] or "").split(": ", 1)[0] if rep and rep["fail"] else None
    if good:
        outer = f"FAIL — admissible type-(a) original-explorer mutant FAIL of the named property {P}" + (
            "" if len(allowed) == 1 else f": {prop} (permitted: {' or '.join(allowed)})")
    elif rep is not None and rep.get("outcome") == "FAIL" and rep.get("states", 0) > cap:
        outer = "INCOMPLETE — BUDGET EXCEEDED (a FAIL printed beyond the authorised cardinality): kills nothing"
    elif rep is not None and rep.get("outcome") == "INCOMPLETE":
        outer = f"INCOMPLETE ({rep.get('incomplete')}): kills nothing"
    elif "inapplicable" in text:
        outer = "INCOMPLETE — VERIFICATION INVALID: kills nothing"
    elif any(l.startswith(("exit 101", "launching command")) for l in ck.failed()) and (
            proc["signal"] is not None or sup["timed_out"] or sup["oom_kill"]):
        outer = "INCOMPLETE (killed from outside or abnormal termination): kills nothing"
    elif rep is not None and rep.get("outcome") == "PASS":
        outer = f"PASS on the mutant (the control survives {cfg}): kills nothing"
    elif rep is not None and rep.get("outcome") == "FAIL" and prop not in allowed:
        outer = (f"FAIL of {prop}, not the named property {P}: kills nothing (no attribution exception)"
                 if len(allowed) == 1 else f"FAIL of {prop}, not a permitted property ({' or '.join(allowed)}): kills "
                                           f"nothing")
    else:
        outer = "NOT ADMISSIBLE (" + "; ".join(ck.failed()) + "): kills nothing"
    extra = []
    if rep:
        extra.append(f"raw outcome (as printed): outcome {rep.get('outcome')}; FAIL {rep.get('fail')}; "
                     f"{rep.get('trace_steps', '').strip() if rep.get('trace_steps') else 'no trace'}; harness {result}")
        if "states" in rep:
            extra.append(f"measured: states {rep['states']:,} (expanded {rep['expanded']:,}) of the {cap:,} ceiling; "
                         f"transitions {rep.get('transitions')}; elapsed {rep.get('elapsed_s')} s")
    extra.append(f"named property: {P}; configuration {cfg}; test {ctl['test']}")
    extra.append(f"candidate: {'KILL_EVIDENCE_CANDIDATE — type (a) — ARCHITECT REVIEW PENDING' if good else 'none'}")
    extra.append(resources_line(sup, limit))
    write(d, CANDIDATE if good else NOT_CANDIDATE, outer, ck,
          {"control": ctl["id"], "receipt_type": "AD-1-R4 F.1 (a)", "test": ctl["test"], "configuration": cfg,
           "named_property": P, "ceiling_states": cap, "time_limit_seconds": limit,
           "report": {k: rep.get(k) for k in ("header", "outcome", "bounds", "actors", "states", "expanded",
                                               "transitions", "fault_edges", "no_step", "elapsed_s", "cap",
                                               "time_cap_s", "fail", "incomplete", "trace_steps")} if rep else None,
           "panic": {"header": lines[heads[0][0]] if len(heads) == 1 else [lines[i] for i, _ in heads],
                     "message_first_line": msg[:1]},
           "harness_result": result, "process": proc, "supervisor": sup, "executable": desc["executable"],
           "selection": desc["selection"], "raw_output_sha256": desc["raw_output_sha256"],
           "not_required": "any witness line, locus or verdict (F.1 (a))"}, extra)
    return 0 if good else 1


def classify_b(d, ctl, exe, source_verdict="EXACTLY_C_PLUS_PATCH"):
    refuse_twice(d)
    ck = Checks()
    desc, text = load(d)
    if not ck("the run started (no pre-flight refusal)", "not_started" not in desc, desc.get("not_started", "")):
        return write(d, NOT_CANDIDATE, "NOT ADMISSIBLE (not started)", ck, {"control": ctl["id"]}) or 1
    proc, sup = desc["process"], desc["supervisor"]
    limit, span = ctl["limit_seconds"], ctl["test_span"]
    check_identity(ck, desc, exe, source_verdict, ctl["mutant_blob"])
    check_selection(ck, desc, exe, ctl["test"], ctl["argv_tail"], False)
    starts, running, result, failed_names = harness(text, ctl["test"])
    ck("the harness ran exactly that one test", running == ["1"] and starts == [ctl["test"]],
       f"running {running}; started {starts}")
    want = f"test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; {K.LISTED - 1} filtered out; finished in "
    ck(f"harness: '{want}…' and the failures list names exactly that test", len(result) == 1
       and result[0].startswith(want) and failed_names == [ctl["test"]], f"{result}; failures {failed_names}")
    ck("exit 101 by a normal exit, no signal", proc["exit_status"] == 101 and proc["signal"] is None,
       f"exit {proc['exit_status']}, signal {proc['signal']}, supervisor rc {proc['supervisor_rc']}")
    check_supervision(ck, sup, limit)
    lines, heads, anyp = panics(text)
    msg, loc = [], None
    ck("exactly one panic, of that test's thread", len(heads) == 1 and len(anyp) == 1
       and heads[0][1].group(1) == ctl["test"], anyp[:3])
    if len(heads) == 1:
        h = heads[0][1]
        loc = (h.group(2), int(h.group(3)), int(h.group(4)))
        ck(f"the panic is at one of the named test's own checks: {ctl['test_file']}, inside fn "
           f"{ctl['test'].split('::')[-1]} (lines {span[0] + 1}-{span[1]}) — not in shared setup, a fixture or elsewhere",
           loc[0] == ctl["test_file"] and span[0] < loc[1] <= span[1], f"{loc[0]}:{loc[1]}:{loc[2]}")
        msg = panic_message(lines, heads[0][0])
        ck("the panic message is recorded (non-empty)", any(m.strip() for m in msg), msg[:1])
    ck("no fixture failure; no explorer report (NOT PRODUCED BY DESIGN for type (b))",
       "WITNESS FIXTURE FAILURE" not in text and "EXPLORATION " not in text)
    good = ck.ok()
    if good:
        outer = (f"FAIL — admissible type-(b) named-test FAIL of {ctl['named_test_id']} at its own check "
                 f"{loc[0]}:{loc[1]}")
    elif re.search(rf"^test {re.escape(ctl['test'])} \.\.\. ok$", text, re.M) and proc["exit_status"] == 0:
        outer = f"PASS on the mutant (the named test {ctl['named_test_id']} passed): kills nothing"
    elif proc["signal"] is not None or sup["timed_out"] or sup["oom_kill"]:
        outer = "INCOMPLETE (killed from outside or abnormal termination): kills nothing"
    elif loc is not None and not (loc[0] == ctl["test_file"] and span[0] < loc[1] <= span[1]):
        outer = (f"NOT ADMISSIBLE (a failure outside the named test's own checks, at {loc[0]}:{loc[1]}): kills nothing")
    else:
        outer = "NOT ADMISSIBLE (" + "; ".join(ck.failed()) + "): kills nothing"
    write(d, CANDIDATE if good else NOT_CANDIDATE, outer, ck,
          {"control": ctl["id"], "receipt_type": "AD-1-R4 F.1 (b)", "test": ctl["test"],
           "named_test_id": ctl["named_test_id"], "test_span_at_C": span, "panic_location": loc,
           "panic_header": [lines[i] for i, _ in heads], "panic_message": msg, "harness_result": result,
           "process": proc, "supervisor": sup, "executable": desc["executable"], "selection": desc["selection"],
           "raw_output_sha256": desc["raw_output_sha256"],
           "not_required": "an explorer outcome line, or any witness line (F.1 (b))"},
          [f"raw outcome (as printed): harness {result}; panic at {loc}",
           "panic message: " + " | ".join(msg),
           f"named test: {ctl['named_test_id']} ({ctl['test']})",
           f"candidate: {'KILL_EVIDENCE_CANDIDATE — type (b) — ARCHITECT REVIEW PENDING' if good else 'none'}",
           resources_line(sup, limit)])
    return 0 if good else 1


def expectations():
    return json.load(open(os.path.join(K.EV, "receipts/02-EXPECTATIONS.json")))


BASE_EXE = {"path": K.EXE, "sha256": K.EXE_SHA256, "size": K.EXE_SIZE}

if __name__ == "__main__":
    mode, key = sys.argv[1], sys.argv[2]
    exp = expectations()
    if mode == "baseline":
        b = next(x for x in exp["baselines"] if x["key"] == key)
        fn = classify_e1 if b["type"] == "e1" else classify_e2
        sys.exit(fn(os.path.join(K.EV, b["dir"]), b, BASE_EXE))
    raise SystemExit("usage: classify.py baseline <key> (builds and controls are classified by healthy.py and "
                     "control.py)")
