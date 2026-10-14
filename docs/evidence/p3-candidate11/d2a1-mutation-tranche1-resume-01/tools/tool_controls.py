#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 6: test the frozen tooling with synthetic and historical
evidence only, before the freeze and before any Nexus build or run.

    tool_controls.py OUTDIR SCRATCH      OUTDIR: the receipt directory (must not exist); SCRATCH: a directory outside
                                         every repository for the synthetic workspace (must not exist)

What runs here is NOT Nexus. A synthetic cargo workspace (package name `nexus-governed-control`, no dependencies, the
same release profile and Cargo configuration shape) is generated in SCRATCH, with its files laid out so that its
panic and diagnostic locations coincide with the real expectations (the explorer verdict assertions at
model_tests.rs:4121:5 and :4133:5, the T-OWN assertion at unit_tests.rs:1089, T-BIND's own lines 1114-1249) and with
exactly 348 tests, as the pinned baseline executable lists. Its "explorer" prints a report read from a file: the
historical X-1 report of the Stage-1 evidence commit and the historical F-B2 and F-G reports of the Stage-2 evidence
commit (git objects), unchanged for PASS and transformed for FAIL, INCOMPLETE and overshoot cases.

The synthetic workspace is built with the real supervise.build (exact command, environment and limits) and its
executable is run with the real supervise.invoke; every receipt is classified by the real classify.py. Positive cases
must be classified as their success; negative cases (real runs and builds, and copies of positive receipts with one
field or line changed) must be rejected with the stated outer class. Nothing is retried; every receipt is kept.
"""
import json
import os
import re
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402
import classify  # noqa: E402
import supervise  # noqa: E402

SYN = "SYNTHETIC — tool_controls.py development fixture of P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01; not Nexus source"
TOTAL_TESTS = 348


def lay_out(lines_at, last):
    """A file whose line n (1-based) is lines_at[n], every other line a comment."""
    return "\n".join(lines_at.get(n, "//") for n in range(1, last + 1)) + "\n"


def block(lines, start):
    return {start + i: l for i, l in enumerate(lines)}


def make_workspace(ws, variant):
    """variant: healthy | c22 (Ticket derives Clone) | twice (Ticket and Snapshot derive Clone) | broken (a type
    error elsewhere)."""
    crate = os.path.join(ws, K.CRATE)
    os.makedirs(os.path.join(crate, "src/ledger/protocol"))
    os.makedirs(os.path.join(ws, ".cargo"))
    open(os.path.join(ws, "Cargo.toml"), "w").write(
        f"# {SYN}\n[workspace]\nmembers = [\"{K.CRATE}\"]\nresolver = \"2\"\n\n[profile.release]\nopt-level = 3\n"
        "lto = \"thin\"\ncodegen-units = 16\nstrip = \"symbols\"\n")
    open(os.path.join(ws, "rust-toolchain.toml"), "w").write(f"# {SYN}\n[toolchain]\nchannel = \"1.94.0\"\n")
    open(os.path.join(ws, ".cargo/config.toml"), "w").write("[build]\njobs = 8\n\n[env]\nCARGO_PROFILE_TEST_DEBUG = \"1\"\n")
    open(os.path.join(crate, "Cargo.toml"), "w").write(
        f"# {SYN}\n[package]\nname = \"nexus-governed-control\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")
    fillers = "\n".join(f"    #[test]\n    fn filler_{i:03}() {{}}" for i in range(TOTAL_TESTS - 5))
    open(os.path.join(crate, "src/lib.rs"), "w").write(
        f"//! {SYN}\n#![allow(dead_code)]\npub mod ledger;\n\n#[cfg(test)]\nmod fillers {{\n{fillers}\n}}\n")
    open(os.path.join(crate, "src/ledger.rs"), "w").write(f"//! {SYN}\npub mod protocol;\n")
    clone_t = "#[derive(Debug, Clone)]" if variant in ("c22", "twice") else "#[derive(Debug)]"
    clone_s = "#[derive(Debug, Clone)]" if variant == "twice" else "#[derive(Debug)]"
    broken = "pub(super) fn broken() -> u8 {\n    \"not a number\"\n}\n" if variant == "broken" else ""
    open(os.path.join(crate, "src/ledger/protocol.rs"), "w").write(
        f"//! {SYN}\n{clone_t}\npub(super) struct Ticket {{\n    pub(super) n: u8,\n}}\n{clone_s}\n"
        "pub(super) struct Snapshot {\n    pub(super) n: u8,\n}\n#[derive(Debug)]\npub(super) struct Worker {\n"
        "    pub(super) n: u8,\n}\n\npub(super) fn protocol_check() {\n"
        "    if std::env::var(\"SYNTH_PROTOCOL_PANIC\").is_ok() {\n        panic!(\"a synthetic failure elsewhere\");\n"
        f"    }}\n}}\n{broken}\n#[cfg(test)]\nmod model_tests;\n#[cfg(test)]\nmod unit_tests;\n")
    m = {1: f"//! {SYN}", 2: "use std::collections::BTreeSet;", 3: "use std::sync::Mutex;", 4: "",
         5: "#[derive(Debug, PartialEq, Eq)]", 6: "pub(super) enum Verdict {", 7: "    Pass,", 8: "    Fail,",
         9: "    Incomplete,", 10: "}", 11: "pub(super) struct Report {", 12: "    pub(super) verdict: Verdict,",
         13: "    pub(super) text: String,", 14: "    pub(super) failed: BTreeSet<String>,", 15: "}",
         16: "pub(super) struct Config {", 17: "    name: &'static str,", 18: "}",
         19: "const X1: Config = Config { name: \"X-1\" };", 20: "const FB2: Config = Config { name: \"F-B2\" };",
         21: "const FG: Config = Config { name: \"F-G\" };"}
    m.update(block([
        "fn explore(config: &Config) -> Report {",
        "    if std::env::var(\"SYNTH_ABORT\").is_ok() {",
        "        std::process::abort();",
        "    }",
        "    let var = format!(\"SYNTH_REPORT_{}\", config.name.replace('-', \"_\"));",
        "    let text = std::fs::read_to_string(std::env::var(var).expect(\"a synthetic report\")).expect(\"read\");",
        "    let outcome = text.lines().find_map(|l| l.strip_prefix(\"  outcome: \")).unwrap_or(\"\").to_string();",
        "    let verdict = match outcome.as_str() {",
        "        \"PASS\" => Verdict::Pass,",
        "        \"FAIL\" => Verdict::Fail,",
        "        _ => Verdict::Incomplete,",
        "    };",
        "    let mut failed = BTreeSet::new();",
        "    if let Some(p) = text.lines().find_map(|l| l.strip_prefix(\"  FAIL: \")) {",
        "        failed.insert(p.split(\": \").next().unwrap().to_string());",
        "    }",
        "    Report { verdict, text, failed }",
        "}"], 30))
    m.update(block([
        "static SERIAL: Mutex<()> = Mutex::new(());", "",
        "fn default_exploration(config: &Config) {",
        "    let _serial = SERIAL",
        "        .lock()",
        "        .unwrap_or_else(|poisoned| poisoned.into_inner());",
        "    let report = explore(config);",
        "    println!(\"{}\", report.text);",
        "    assert_eq!(",
        "        report.verdict,",
        "        Verdict::Pass,",
        "        \"failed: {:?}\\n{}\",",
        "        report.failed,",
        "        report.text",
        "    );",
        "}", "",
        "fn full_exploration(config: &Config) {",
        "    let report = explore(config);",
        "    println!(\"{}\", report.text);",
        "    assert_eq!(",
        "        report.verdict,",
        "        Verdict::Pass,",
        "        \"failed: {:?}\\n{}\",",
        "        report.failed,",
        "        report.text",
        "    );",
        "}", "",
        "#[test]",
        "fn x1_two_facts_two_writers_one_admission() {",
        "    default_exploration(&X1);",
        "}"], 4113))
    m.update(block(["#[test]", "#[ignore = \"synthetic\"]", "fn full_fb2() {", "    full_exploration(&FB2);", "}", "",
                    "#[test]", "#[ignore = \"synthetic\"]", "fn full_fg() {", "    full_exploration(&FG);", "}"], 4230))
    open(os.path.join(crate, "src/ledger/protocol/model_tests.rs"), "w").write(lay_out(m, 4245))
    u = {1: f"//! {SYN}", 2: "use super::*;"}
    u.update(block(["fn fixture() {", "    if std::env::var(\"SYNTH_FIXTURE_PANIC\").is_ok() {",
                    "        panic!(\"a synthetic fixture failure\");", "    }", "}"], 200))
    u.update(block(["trait NotClone<Marker> {", "    fn check() {}", "}", "", "impl<T: ?Sized> NotClone<()> for T {}",
                    "", "impl<T: Clone> NotClone<u8> for T {}", "", "", "", "#[test]",
                    "fn t_own_tickets_snapshots_and_the_worker_cannot_be_cloned() {",
                    "    <Ticket as NotClone<_>>::check();", "    <Snapshot as NotClone<_>>::check();",
                    "    <Worker as NotClone<_>>::check();", "}"], 1077))
    u.update(block(["#[test]", "fn t_bind_every_bound_value_belongs_to_one_ledger() {", "    fixture();",
                    "    protocol_check();", "    if std::env::var(\"SYNTH_ABORT\").is_ok() {",
                    "        std::process::abort();", "    }",
                    "    assert_eq!(std::env::var(\"SYNTH_BIND\").as_deref(), Ok(\"ok\"), \"a synthetic check\");"],
                   1112))
    u[1249] = "}"
    open(os.path.join(crate, "src/ledger/protocol/unit_tests.rs"), "w").write(lay_out(u, 1250))
    env = dict(K.build_env(os.path.join(ws, "target")))
    r = subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=ws, capture_output=True, text=True,
                       env={k: v for k, v in env.items() if k != "CARGO_TARGET_DIR"})
    return r.returncode, r.stderr


# ---- historical reports and their synthetic transformations --------------------------------------------------------

def x1_block():
    raw = K.blob(K.STAGE1, K.STAGE1_PACKET + "/receipts/30-suite/raw-output.txt").decode()
    lines = raw.splitlines()
    i = next(n for n, l in enumerate(lines) if l.startswith(
        "test ledger::protocol::model_tests::x1_two_facts_two_writers_one_admission ... EXPLORATION X-1"))
    j = next(n for n in range(i, len(lines)) if lines[n].startswith("  state key fields: "))
    out = [lines[i][lines[i].index("EXPLORATION "):]] + lines[i + 1:j + 1]
    return "\n".join(out) + "\n"


def stage2_block(cfg):
    raw = open(K.inputs(f"linked/{cfg}/raw-output.txt")).read()
    lines = raw.splitlines()
    i = next(n for n, l in enumerate(lines) if "EXPLORATION " + cfg in l)
    j = next(n for n in range(i, len(lines)) if lines[n].startswith("  state key fields: "))
    return "\n".join([lines[i][lines[i].index("EXPLORATION "):]] + lines[i + 1:j + 1]) + "\n"


def as_fail(text, prop, states=None):
    lines = text.splitlines()
    out = []
    for l in lines:
        if l == "  outcome: PASS":
            out.append("  outcome: FAIL")
        elif re.match(r"^  (P\d+|D1|L1|E\d+|KEY): ", l):
            continue
        elif l.startswith("  states ") and states is not None:
            out.append(re.sub(r"^  states \d+ \(expanded \d+\)", f"  states {states} (expanded {states - 9})", l))
        else:
            out.append(l)
        if l.startswith("  elapsed "):
            out += [f"  FAIL: {prop}: a synthetic violation (tool control)", "  shortest trace (2 steps):",
                    "      1. Fw(0): synthetic step", "      2. Se: synthetic step", "    end: phase synthetic"]
    return "\n".join(out) + "\n"


def as_incomplete(text):
    out = []
    for l in text.splitlines():
        if l == "  outcome: PASS":
            out.append("  outcome: INCOMPLETE")
        elif re.match(r"^  (P\d+|D1|L1|E\d+|KEY): ", l):
            out.append(l.split(": ")[0] + ": NOT ESTABLISHED (incomplete)")
        else:
            out.append(l)
        if l.startswith("  elapsed "):
            out.append("  INCOMPLETE: the state cap of 30000000 was reached. No property is established by this run.")
    return "\n".join(out) + "\n"


# ---- the harness -----------------------------------------------------------------------------------------------------

results = []


def expect(name, first_line_path, want, outer_must_contain=None):
    got = open(first_line_path).read().splitlines() if os.path.exists(first_line_path) else ["(missing)", ""]
    ok = got[0] == want and (outer_must_contain is None or outer_must_contain in got[1])
    results.append((name, ok, got[0], got[1] if len(got) > 1 else ""))
    print(f"{'CONTROL OK ' if ok else 'CONTROL BAD'} {name}: {got[0]} | {got[1] if len(got) > 1 else ''}")


def synthetic_source(mutant_blob=None):
    def ok(rec, when):
        rec.append(f"source ({when}): SYNTHETIC_SOURCE (synthetic workspace; no Nexus source)")
        d = {"verdict": "SYNTHETIC_SOURCE"}
        if mutant_blob:
            d["mutant_file"] = {"git_blob": mutant_blob}
        return d, True
    return ok


def mutate(src, dst, edit_desc=None, edit_raw=None):
    shutil.copytree(src, dst)
    for f in ("CLASSIFICATION.txt", "CHECKS.txt", "PARSED-RECEIPT.json"):
        if os.path.exists(os.path.join(dst, f)):
            os.remove(os.path.join(dst, f))
    for f in os.listdir(dst):
        if f.startswith("BRANCH-ACCOUNTING-"):
            os.remove(os.path.join(dst, f))
    if edit_desc:
        p = os.path.join(dst, "descriptor.json")
        d = json.load(open(p))
        edit_desc(d)
        json.dump(d, open(p, "w"), indent=1)
    if edit_raw:
        p = os.path.join(dst, "raw-output.txt")
        t = open(p).read()
        t2 = edit_raw(t)
        assert t2 != t, "a raw-output mutation must change the text"
        open(p, "w").write(t2)
        d = json.load(open(os.path.join(dst, "descriptor.json")))
        d["raw_output_sha256"] = K.sha256_file(p)  # identity kept consistent: the change itself must be caught
        json.dump(d, open(os.path.join(dst, "descriptor.json"), "w"), indent=1)


def main(outdir, scratch):
    if os.path.exists(outdir) or os.path.exists(scratch):
        raise SystemExit("REFUSED: output or scratch directory exists")
    os.makedirs(outdir)
    os.makedirs(scratch)
    exp = json.load(open(os.path.join(K.EV, "receipts/02-EXPECTATIONS.json")))
    C = exp["controls"]
    B = {b["key"]: b for b in exp["baselines"]}
    # 1. Synthetic workspaces and builds (real supervise.build: exact command, environment, limits).
    builds = {}
    for variant in ("healthy", "c22", "twice", "broken"):
        ws = os.path.join(scratch, "ws-" + variant)
        rc, err = make_workspace(ws, variant)
        print(f"workspace {variant}: generate-lockfile rc {rc} {err.strip()[:200]}")
        d = os.path.join(outdir, "build-" + variant)
        mb = C["C22"]["mutant_blob"] if variant != "healthy" else None
        supervise.build(d, f"SYNTHETIC BUILD {variant}", ws, synthetic_source(mb), [], {"synthetic": variant},
                        manifest=variant in ("healthy", "c22"), gate=False)
        builds[variant] = (ws, d)
    ws, d = builds["healthy"]
    e4 = d + "-as-e4"
    shutil.copytree(d, e4)
    classify.classify_build_success(e4, "e4", "SYNTHETIC_SOURCE", expect_target=os.path.join(ws, "target"))
    expect("healthy synthetic build classified (e4)", os.path.join(e4, "CLASSIFICATION.txt"), "CLASSIFICATION: PASS")
    pin = d + "-as-pin"
    shutil.copytree(d, pin)
    classify.classify_build_success(pin, "pin", "SYNTHETIC_SOURCE", C["C14"]["mutant_blob"])
    expect("healthy synthetic build as a mutant pin (mutant blob mismatch in the source verdict) is NOT OK",
           os.path.join(pin, "CLASSIFICATION.txt"), "BUILD: NOT OK")
    pin2 = d + "-as-pin-ok"
    mutate(d, pin2, edit_desc=lambda x: [s.update(mutant_file={"git_blob": "f" * 40}) for s in
                                          (x["source_before"], x["source_after"])])
    classify.classify_build_success(pin2, "pin", "SYNTHETIC_SOURCE", "f" * 40)
    expect("synthetic build pinned as a mutant build", os.path.join(pin2, "CLASSIFICATION.txt"), "BUILD: PINNED")
    for name, edit in (("signal", lambda x: x["process"].update(exit_status=None, signal=9)),
                       ("timeout", lambda x: x["supervisor"].update(timed_out=True)),
                       ("oom", lambda x: x["supervisor"].update(oom_kill=1)),
                       ("swap", lambda x: x["supervisor"].update(swap_peak_bytes="4096")),
                       ("source after", lambda x: x["source_after"].update(verdict="NOT_C")),
                       ("argv without -j 2", lambda x: x.update(argv=x["argv"][:-2])),
                       ("RUSTFLAGS in env", lambda x: x["env"].update(RUSTFLAGS="-C debuginfo=2"))):
        m = d + "-neg-" + name.replace(" ", "-")
        mutate(d, m, edit_desc=edit)
        classify.classify_build_success(m, "e4", "SYNTHETIC_SOURCE")
        expect(f"healthy build negative: {name}", os.path.join(m, "CLASSIFICATION.txt"), "CLASSIFICATION: NOT PASS")
    # C22-shaped builds: (d).
    ctl22 = dict(C["C22"])
    ws, d = builds["c22"]
    classify.classify_d(d, ctl22, "SYNTHETIC_SOURCE")
    expect("synthetic E0283 at unit_tests.rs:1089 classified (d)", os.path.join(d, "CLASSIFICATION.txt"),
           classify.CANDIDATE)
    for v, outer in (("twice", "NOT ADMISSIBLE"), ("broken", "NOT ADMISSIBLE")):
        classify.classify_d(builds[v][1], ctl22, "SYNTHETIC_SOURCE")
        expect(f"synthetic build '{v}' is not a (d) candidate", os.path.join(builds[v][1], "CLASSIFICATION.txt"),
               classify.NOT_CANDIDATE, outer)
    for name, edit, outer in (
            ("signal", lambda x: x["process"].update(exit_status=None, signal=9), "INCOMPLETE"),
            ("timeout", lambda x: x["supervisor"].update(timed_out=True), "INCOMPLETE"),
            ("other line", lambda x: None, "NOT ADMISSIBLE"),
            ("exit 0", lambda x: x["process"].update(exit_status=0), "NOT ADMISSIBLE"),
            ("a mutant executable present", lambda x: x["deps_artifacts"].append(
                {"path": "/x/nexus_governed_control-0123456789abcdef", "sha256": "0" * 64, "size": 1,
                 "executable_bit": True, "test_executable_name": True}), "NOT ADMISSIBLE")):
        m = d + "-neg-" + name.replace(" ", "-")
        mutate(d, m, edit_desc=edit)
        c = dict(ctl22, diagnostic_line=1090) if name == "other line" else ctl22
        classify.classify_d(m, c, "SYNTHETIC_SOURCE")
        expect(f"(d) negative: {name}", os.path.join(m, "CLASSIFICATION.txt"), classify.NOT_CANDIDATE, outer)
    m = d + "-neg-ice"
    mutate(d, m)
    with open(os.path.join(m, "build.stderr"), "a") as f:
        f.write("error: internal compiler error: synthetic\n")
    dd = json.load(open(os.path.join(m, "descriptor.json")))
    dd["stderr_sha256"] = K.sha256_file(os.path.join(m, "build.stderr"))
    json.dump(dd, open(os.path.join(m, "descriptor.json"), "w"), indent=1)
    classify.classify_d(m, ctl22, "SYNTHETIC_SOURCE")
    expect("(d) negative: an internal compiler error line", os.path.join(m, "CLASSIFICATION.txt"), classify.NOT_CANDIDATE)
    # 2. Runs of the synthetic executable (real supervise.invoke).
    pinj = json.load(open(os.path.join(pin2, "PIN.json")))
    exe = {"path": pinj["path"], "sha256": pinj["sha256"], "size": pinj["size"]}
    ws = builds["healthy"][0]
    cwd = os.path.join(ws, K.CRATE)
    reports = os.path.join(scratch, "reports")
    os.makedirs(reports)
    rep = {"x1-pass": x1_block(), "x1-fail-E2": as_fail(x1_block(), "E2"),
           "fb2-fail-E4": as_fail(stage2_block("F-B2"), "E4"), "fb2-fail-E7": as_fail(stage2_block("F-B2"), "E7"),
           "fb2-overshoot": as_fail(stage2_block("F-B2"), "E4", states=30_000_005),
           "fb2-incomplete": as_incomplete(stage2_block("F-B2")), "fb2-pass": stage2_block("F-B2"),
           "fg-fail-E7": as_fail(stage2_block("F-G"), "E7")}
    for k, t in rep.items():
        open(os.path.join(reports, k + ".txt"), "w").write(t)

    def run(name, test, argv_tail, limit, extra_env, ignored, mutant_blob=None):
        d = os.path.join(outdir, "run-" + name)
        env = list(K.RUN_ENV) + list(extra_env)
        supervise.invoke(d, f"SYNTHETIC RUN {name}", exe["path"], exe["sha256"], exe["size"], cwd, ws, test, ignored,
                         argv_tail, limit, synthetic_source(mutant_blob), "synthetic", [], {"synthetic": name},
                         gate=False, env=env)
        return d
    VAR = {"x1": "SYNTH_REPORT_X_1", "fb2": "SYNTH_REPORT_F_B2", "fg": "SYNTH_REPORT_F_G"}
    R = lambda k: [(VAR[k.split("-")[0]], os.path.join(reports, k + ".txt"))]
    b = B["X-1"]
    d = run("e1-x1-pass", b["test"], b["argv_tail"], 180, R("x1-pass"), False)
    classify.classify_e1(d, b, exe, "SYNTHETIC_SOURCE")
    expect("e1: historical X-1 PASS report, harness ok", os.path.join(d, "CLASSIFICATION.txt"), "CLASSIFICATION: PASS")
    e1_ok = d
    for name, edit_desc, edit_raw in (
            ("count above the default ceiling", None, lambda t: re.sub(r"  states 59104 \(expanded 59104\)",
                                                                       "  states 250001 (expanded 250001)", t)),
            ("a property line not HOLDS", None, lambda t: t.replace("  E7: HOLDS", "  E7: FAILED", 1)),
            ("a branch label missing", None, lambda t: t.replace("    [taken    ] expire: Armed -> Counted\n", "", 1)),
            ("executable changed after", lambda x: x["executable"]["after"].update(sha256="0" * 64), None),
            ("source not C before", lambda x: x["source_before"].update(verdict="NOT_C"), None),
            ("signal after PASS text", lambda x: x["process"].update(exit_status=None, signal=9), None),
            ("selection lists two tests", lambda x: x["selection"]["listing"]["listed"].append("other: test"), None),
            ("argv with --ignored", lambda x: x["argv"].insert(2, "--ignored"), None)):
        m = e1_ok + "-neg-" + re.sub(r"\W+", "-", name)
        mutate(e1_ok, m, edit_desc=edit_desc, edit_raw=edit_raw)
        classify.classify_e1(m, b, exe, "SYNTHETIC_SOURCE")
        expect(f"e1 negative: {name}", os.path.join(m, "CLASSIFICATION.txt"), "CLASSIFICATION: NOT PASS",
               "INCOMPLETE" if name.startswith("signal") else "NOT ADMISSIBLE")
    b = B["T-BIND"]
    d = run("e2-tbind-pass", b["test"], b["argv_tail"], 180, [("SYNTH_BIND", "ok")], False)
    classify.classify_e2(d, b, exe, "SYNTHETIC_SOURCE")
    expect("e2: named test passes", os.path.join(d, "CLASSIFICATION.txt"), "CLASSIFICATION: PASS")
    d2 = run("e2-tbind-fail", b["test"], b["argv_tail"], 180, [("SYNTH_BIND", "no")], False)
    classify.classify_e2(d2, b, exe, "SYNTHETIC_SOURCE")
    expect("e2 negative: named test fails", os.path.join(d2, "CLASSIFICATION.txt"), "CLASSIFICATION: NOT PASS",
           "NOT ADMISSIBLE")
    # (b)
    c = C["C14"]
    mb = c["mutant_blob"]
    d = run("b-c14-own-check", c["test"], c["argv_tail"], 180, [("SYNTH_BIND", "no")], False, mb)
    classify.classify_b(d, c, exe, "SYNTHETIC_SOURCE")
    expect("(b): failure at the named test's own check (unit_tests.rs:1119)", os.path.join(d, "CLASSIFICATION.txt"),
           classify.CANDIDATE)
    b_ok = d
    for name, env, outer in (("fixture before the test's checks", [("SYNTH_FIXTURE_PANIC", "1")], "outside the named"),
                             ("failure elsewhere (protocol.rs)", [("SYNTH_PROTOCOL_PANIC", "1")], "outside the named"),
                             ("the named test passes", [("SYNTH_BIND", "ok")], "PASS on the mutant"),
                             ("abort (signal 6)", [("SYNTH_ABORT", "1")], "INCOMPLETE")):
        d = run("b-neg-" + re.sub(r"\W+", "-", name), c["test"], c["argv_tail"], 180, env, False, mb)
        classify.classify_b(d, c, exe, "SYNTHETIC_SOURCE")
        expect(f"(b) negative: {name}", os.path.join(d, "CLASSIFICATION.txt"), classify.NOT_CANDIDATE, outer)
    for name, edit_desc, edit_raw, outer in (
            ("mutant blob differs", lambda x: x["source_after"]["mutant_file"].update(git_blob="0" * 40), None, None),
            ("second panic line", None, lambda t: t.replace("FAILED\n", "thread 'other' (1) panicked at "
                                                                       "x.rs:1:1:\nboom\nFAILED\n", 1), None),
            ("timeout", lambda x: x["supervisor"].update(timed_out=True), None, "INCOMPLETE"),
            ("oom", lambda x: x["supervisor"].update(oom_kill=1), None, "INCOMPLETE"),
            ("exit 1 not 101", lambda x: x["process"].update(exit_status=1), None, None),
            ("raw output edited after the run", lambda x: None, None, None),
            ("wrong executable", lambda x: x["executable"].update(path="/x"), None, None)):
        m = b_ok + "-neg-" + re.sub(r"\W+", "-", name)
        if name == "raw output edited after the run":
            shutil.copytree(b_ok, m)
            for f in ("CLASSIFICATION.txt", "CHECKS.txt", "PARSED-RECEIPT.json"):
                os.remove(os.path.join(m, f))
            open(os.path.join(m, "raw-output.txt"), "a").write("tampered\n")
        else:
            mutate(b_ok, m, edit_desc=edit_desc, edit_raw=edit_raw)
        classify.classify_b(m, c, exe, "SYNTHETIC_SOURCE")
        expect(f"(b) negative: {name}", os.path.join(m, "CLASSIFICATION.txt"), classify.NOT_CANDIDATE, outer)
    # (a)
    for cid, key, ok_case in (("C12", "x1-fail-E2", True), ("C9", "fb2-fail-E4", True), ("C18a", "fg-fail-E7", True)):
        c = C[cid]
        d = run(f"a-{cid}-{key}", c["test"], c["argv_tail"], c["limit_seconds"], R(key), c["ignored"], c["mutant_blob"])
        classify.classify_a(d, c, exe, "SYNTHETIC_SOURCE")
        expect(f"(a) {cid}: FAIL of the named property {c['named_property']} ({key})",
               os.path.join(d, "CLASSIFICATION.txt"), classify.CANDIDATE)
        if cid == "C9":
            a_ok = d
    c = C["C9"]
    for key, outer in (("fb2-fail-E7", "not the named property"), ("fb2-overshoot", "BUDGET EXCEEDED"),
                       ("fb2-incomplete", "INCOMPLETE ("), ("fb2-pass", "PASS on the mutant")):
        d = run(f"a-neg-{key}", c["test"], c["argv_tail"], c["limit_seconds"], R(key), c["ignored"], c["mutant_blob"])
        classify.classify_a(d, c, exe, "SYNTHETIC_SOURCE")
        expect(f"(a) negative C9: {key}", os.path.join(d, "CLASSIFICATION.txt"), classify.NOT_CANDIDATE, outer)
    d = run("a-neg-abort", c["test"], c["argv_tail"], c["limit_seconds"], R("fb2-fail-E4") + [("SYNTH_ABORT", "1")],
            c["ignored"], c["mutant_blob"])
    classify.classify_a(d, c, exe, "SYNTHETIC_SOURCE")
    expect("(a) negative C9: abort (signal 6)", os.path.join(d, "CLASSIFICATION.txt"), classify.NOT_CANDIDATE,
           "INCOMPLETE")
    d = run("a-neg-without-ignored", c["test"], [c["test"], "--exact", "--nocapture", "--test-threads=1"],
            c["limit_seconds"], R("fb2-fail-E4"), c["ignored"], c["mutant_blob"])
    classify.classify_a(d, c, exe, "SYNTHETIC_SOURCE")
    expect("(a) negative C9: ignored test run without --ignored", os.path.join(d, "CLASSIFICATION.txt"),
           classify.NOT_CANDIDATE)
    c18 = C["C18a"]
    d = run("a-neg-C18a-on-fb2-report", c18["test"], c18["argv_tail"], c18["limit_seconds"],
            [("SYNTH_REPORT_F_G", os.path.join(reports, "fb2-fail-E4.txt"))], c18["ignored"], c18["mutant_blob"])
    classify.classify_a(d, c18, exe, "SYNTHETIC_SOURCE")
    expect("(a) negative C18a: a report of another configuration (F-B2 bounds) under full_fg",
           os.path.join(d, "CLASSIFICATION.txt"), classify.NOT_CANDIDATE)
    for name, edit_desc, edit_raw, outer in (
            ("report copy in the message differs", None, lambda t: t.replace("  outcome: FAIL", "  outcome: FAIL ", 1),
             None),
            ("wrong panic location", None, lambda t: t.replace("model_tests.rs:4133:5:", "model_tests.rs:4134:5:", 1),
             None),
            ("wrong thread", None, lambda t: t.replace("thread 'ledger::protocol::model_tests::full_fb2'",
                                                       "thread 'main'", 1), None),
            ("harness result line missing", None, lambda t: re.sub(r"\ntest result: [^\n]*\n", "\n", t), None),
            ("cap printed differs", None, lambda t: t.replace("caps 30000000 states", "caps 60000000 states"), None),
            ("signal", lambda x: x["process"].update(exit_status=None, signal=9), None, "INCOMPLETE"),
            ("wall at the limit", lambda x: x["supervisor"].update(wall_seconds=2700.0), None, None)):
        m = a_ok + "-neg-" + re.sub(r"\W+", "-", name)
        mutate(a_ok, m, edit_desc=edit_desc, edit_raw=edit_raw)
        classify.classify_a(m, C["C9"], exe, "SYNTHETIC_SOURCE")
        expect(f"(a) negative C9: {name}", os.path.join(m, "CLASSIFICATION.txt"), classify.NOT_CANDIDATE, outer)
    # Refusal to classify twice.
    try:
        classify.classify_a(a_ok, C["C9"], exe, "SYNTHETIC_SOURCE")
        results.append(("refuses to classify twice", False, "classified twice", ""))
    except SystemExit as e:
        results.append(("refuses to classify twice", "already classified" in str(e), str(e), ""))
    print(f"{'CONTROL OK ' if results[-1][1] else 'CONTROL BAD'} refuses to classify twice: {results[-1][2]}")
    bad = [r for r in results if not r[1]]
    summary = [f"TOOL CONTROLS {K.MISSION}: {len(results)} controls; {len(results) - len(bad)} as expected"]
    summary += [f"{'OK ' if ok else 'BAD'} {n}: {a} | {o}" for n, ok, a, o in results]
    summary.append("TOOL CONTROLS: " + ("ALL AS EXPECTED" if not bad else "FAILED: " + "; ".join(r[0] for r in bad)))
    open(os.path.join(outdir, "SUMMARY.txt"), "w").write("\n".join(summary) + "\n")
    print("\n".join(summary))
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])))
