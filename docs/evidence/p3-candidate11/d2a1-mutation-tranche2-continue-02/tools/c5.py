#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02, mission sections 2, 6, 7, 8 and 15: C5 only. Read-only.

The Sole Architect ruled C5's existing Tranche-2 mutant build ARCHITECT-ADMISSIBLE under a narrow C5-specific warning
ruling (mission section 2). The ruling does not transfer: every other control's build still requires zero warnings and
zero errors, and the general build classifier (classify.classify_build_success) is unchanged and still classifies the
historical C5 build `BUILD: NOT OK`. This module never rebuilds, never writes into the C5 worktree or its target, and
never relabels the historical result.

  admit(build_dir, ctl, synthetic=False)
                            the dedicated C5 build-admission checker (section 6), over the preserved Tranche-2 build
                            evidence (byte copies under inputs/c5/03-build, each authenticated against the Tranche-2
                            packet's SHA256SUMS and git blobs). Admissible only if: the control is C5 (refused for any
                            other control); A/B source EXACTLY_C_PLUS_PATCH before and after with the ruling's mutant blob
                            and sha256; C the ruling's patch identity; D the exact build command and environment; E the
                            exact toolchain and build inputs (those of the five accepted Tranche-2 builds; inputs C's
                            blobs); F normal exit 0; G 16 GiB, zero swap, within 900 s; H no OOM, no signal; I no `error`
                            diagnostic and no compiler-crash text; J exactly the five permitted warning groups (header,
                            primary location, every quoted source line equal to the mutant's or C's line, every marker at
                            the exact columns of the named items, labels and notes exactly the expected ones); K every
                            other stderr line is cargo progress, the five groups' own lines, cargo's summary of those
                            warnings, the Finished line or the Executable line (no other warning, lint or diagnostic); L
                            cargo named exactly one library-test executable; M its recorded sha256 and size equal the
                            ruling's; and the historical classification is BUILD: NOT OK with exactly one failed check,
                            the general classifier's `no error or warning diagnostic`. synthetic=True is for
                            c2_controls.py's tampered copies only: the preserved-receipt identity check is replaced by
                            a failing line, so a synthetic copy is never admissible, and every other check runs.
  pin(ctl, build_dir)       the preserved executable pin (section 7): a regular file, not a symlink, every path component
                            real, beneath the C5 worktree's own real target directory (not a symlink, not a mount point);
                            executable bit; sha256 and size exact; dep-info identity as recorded, naming the executable and
                            the worktree's mutated protocol.rs; release/deps holding exactly the two recorded
                            nexus_governed_control artifacts; no file or directory under the target modified (mtime or
                            ctime) after the build record finished.
  worktree_check(ctl, when, restored_check)
                            the preserved C5 worktree (sections 7 and 15): its filesystem identity and ownership, the
                            worktree administration (registered, detached at C, locked with the Tranche-2 reason), HEAD and
                            tree, and exactly C (verify_tree.py and the path-aware status policy: no tracked change,
                            ignored entries only beneath its own real target directory; restored_check is
                            control.restored_check, unchanged, passed in by control.main_c5).

Each returns (record, ok) and never raises on a missing or changed file: a missing artifact is a failed check.
"""
import json
import os
import re
import stat
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import classify  # noqa: E402
import common as K  # noqa: E402
import expectations  # noqa: E402
import worktrees  # noqa: E402

ADMIT_OK = "C5 BUILD ADMISSION: ARCHITECT-ADMISSIBLE BUILD AUTHENTICATED (NARROW C5-SPECIFIC WARNING RULING)"
ADMIT_FAIL = "C5 BUILD ADMISSION: REJECTED"
PIN_OK = "C5 PRESERVED EXECUTABLE: PINNED"
PIN_FAIL = "C5_PRESERVED_ARTIFACT_BLOCKED"
WT_OK = "C5 PRESERVED WORKTREE: AUTHENTICATED (EXACTLY C; LOCKED; NOT MODIFIED BY THIS MISSION)"
WT_FAIL = "C5 PRESERVED WORKTREE: NOT AUTHENTICATED"
BUILD_FILES = ["CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json", "RECEIPT.txt", "build.stderr", "build.stdout",
               "cgroup-samples.txt", "descriptor.json", "scope-record.txt", "time-v.txt"]
# The toolchain of every accepted Tranche-2 mutant build (and of this one): exact.
TOOLCHAIN = {"rustc": "rustc 1.94.0 (4a4ef493e 2026-03-02)", "cargo": "cargo 1.94.0 (85eff7c80 2026-01-15)",
             "active_toolchain": "1.94.0-x86_64-unknown-linux-gnu"}
SUMMARY = ("warning: `nexus-governed-control` (lib test) generated 5 warnings (run `cargo fix --lib -p "
           "nexus-governed-control --tests` to apply 2 suggestions)")
# Mission section 2: exactly these five mutation-induced warning groups, C5 only. Locations of 1 and 2 are the mission's;
# those of 3-5 are the named items' own lines in the mutant (identical to C's before line 1816) and in C's model_tests.rs.
# carets/dashes: (line, column, length, the source text they mark).
WARNINGS = [
    {"n": 1, "header": "warning: unused variable: `nonce`", "file": K.PROTOCOL, "line": 1818, "col": 47,
     "shown": [1818], "carets": [(1818, 47, 5, "nonce")], "dashes": [],
     "labels": ["help: if this is intentional, prefix it with an underscore: `_nonce`"],
     "notes": ["`#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default"]},
    {"n": 2, "header": "warning: unused variable: `io`", "file": K.PROTOCOL, "line": 1818, "col": 65,
     "shown": [1818], "carets": [(1818, 65, 2, "io")], "dashes": [],
     "labels": ["help: if this is intentional, prefix it with an underscore: `_io`"], "notes": []},
    {"n": 3, "header": "warning: methods `acknowledgement` and `frontier` are never used", "file": K.PROTOCOL,
     "line": 506, "col": 8, "shown": [503, 506, 507],
     "carets": [(506, 8, 15, "acknowledgement"), (507, 8, 8, "frontier")], "dashes": [(503, 18, 5, "Reads")],
     "labels": ["methods in this trait"], "notes": ["`#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default"]},
    {"n": 4, "header": "warning: variant `Inconsistent` is never constructed", "file": K.PROTOCOL, "line": 743,
     "col": 5, "shown": [737, 743], "carets": [(743, 5, 12, "Inconsistent")], "dashes": [(737, 17, 7, "Resolve")],
     "labels": ["variant in this enum"],
     "notes": ["`Resolve` has derived impls for the traits `Debug` and `Clone`, but these are intentionally ignored "
               "during dead code analysis"]},
    {"n": 5, "header": "warning: variants `Acknowledgement` and `Frontier` are never constructed", "file": K.MODEL,
     "line": 598, "col": 5, "shown": [590, 598, 599],
     "carets": [(598, 5, 15, "Acknowledgement"), (599, 5, 8, "Frontier")], "dashes": [(590, 6, 4, "Verb")],
     "labels": ["variants in this enum"],
     "notes": ["`Verb` has derived impls for the traits `Debug` and `Clone`, but these are intentionally ignored during "
               "dead code analysis"]},
]
HEAD_RE = re.compile(r"^\s*(error|warning)(\[[A-Za-z0-9_]+\])?(:|\b)")
LOC_RE = re.compile(r"^ +--> (\S+):(\d+):(\d+)$")
SRC_RE = re.compile(r"^(\d+) \| (.*)$")
GUTTER_RE = re.compile(r"^ +\|$")
MARK_RE = re.compile(r"^( +)\| ([ ^-]*[\^-])(?: (.*))?$")
NOTE_RE = re.compile(r"^ += note: (.*)$")
PROGRESS_RE = re.compile(r"^ +(Compiling \S+ v\S+(?: \(.*\))?|Running `.*`)$")
FINISHED_RE = re.compile(r"^ +Finished `release` profile \[optimized\] target\(s\) in .+$")
CRASH_RE = re.compile(r"internal compiler error|panicked|query stack|SIGSEGV|signal: ")


class Checks(classify.Checks):
    pass


def text_lines(rec, lines):
    return lines + [f"{'OK  ' if o else 'FAIL'} {l}{(': ' + d) if d else ''}" for l, o, d in rec]


def mutant_sources(ctl):
    """{path: lines} of the source the C5 executable was built from: the mutant protocol.rs (from the frozen patch,
    computed in a temporary directory outside every repository, never written to any object store) and C's
    model_tests.rs (unchanged by the patch)."""
    patch = open(K.inputs("patches", "C5.patch"), "rb").read()
    mut, err = expectations.mutant_bytes(patch, K.PROTOCOL, K.blob(K.C, K.PROTOCOL))
    if mut is None or K.git_blob_id(mut) != ctl["mutant_blob"]:
        return None, f"the mutant could not be reproduced from the frozen patch: {err}"
    return {K.PROTOCOL: mut.decode("utf-8").split("\n"), K.MODEL: K.source(K.MODEL).split("\n")}, ""


def parse_group(lines, i, src):
    """One diagnostic group starting at lines[i] (its header), through the blank line that ends it. Returns (group,
    next index, problems): the header, the location, the quoted source lines (each compared with src), the caret and dash
    runs (line, column, length, marked text), the labels and the notes; any other line is a problem."""
    g = {"header": lines[i], "loc": None, "shown": [], "carets": [], "dashes": [], "labels": [], "notes": [],
         "problems": []}
    probs = []
    j, last = i + 1, None
    m = LOC_RE.match(lines[j]) if j < len(lines) else None
    if not m:
        probs.append(f"line {j + 1}: no primary location after the header")
    else:
        g["loc"] = (m.group(1), int(m.group(2)), int(m.group(3)))
        j += 1
    while j < len(lines) and lines[j] != "":
        line = lines[j]
        ms, mm, mn = SRC_RE.match(line), MARK_RE.match(line), NOTE_RE.match(line)
        if ms:
            n, code = int(ms.group(1)), ms.group(2)
            f = g["loc"][0] if g["loc"] else None
            actual = src.get(f, [])[n - 1] if f in src and 0 < n <= len(src[f]) else None
            if actual != code:
                probs.append(f"line {j + 1}: quoted source {f}:{n} differs from the source the executable was built "
                             f"from: {code!r} vs {actual!r}")
            g["shown"].append(n)
            last = (n, len(ms.group(1)) + 3, actual or "")
        elif GUTTER_RE.match(line) or line == "...":
            pass
        elif mm and last is not None and len(mm.group(1)) + 2 == last[1]:
            marks = mm.group(2)
            for run in re.finditer(r"\^+|-+", marks):
                col, length = run.start() + 1, run.end() - run.start()
                key = "carets" if run.group(0)[0] == "^" else "dashes"
                g[key].append((last[0], col, length, last[2][col - 1:col - 1 + length]))
            if mm.group(3) is not None:
                g["labels"].append(mm.group(3))
        elif mn:
            g["notes"].append(mn.group(1))
        else:
            probs.append(f"line {j + 1}: not a line of a diagnostic group: {line[:120]!r}")
        j += 1
    if j >= len(lines):
        probs.append("the group is not ended by a blank line")
    g["problems"] = probs
    return g, j + 1, probs


def stderr_admission(ck, text, src, wanted):
    """J and K over the build's stderr: every line classified; exactly the wanted groups (and nothing else)."""
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines = lines[:-1]
    groups, unclassified, problems = [], [], []
    progress = finished = executable = summary = 0
    i = 0
    while i < len(lines):
        line = lines[i]
        if PROGRESS_RE.match(line):
            progress += 1
            i += 1
        elif line == SUMMARY:
            summary += 1
            i += 1
        elif HEAD_RE.match(line):
            g, i, probs = parse_group(lines, i, src)
            groups.append(g)
            problems += probs
        elif FINISHED_RE.match(line):
            finished += 1
            i += 1
        elif line == f"  Executable `{K.C5_EXE}`":
            executable += 1
            i += 1
        else:
            unclassified.append((i + 1, line[:160]))
            i += 1
    errs = [g["header"] for g in groups if g["header"].lstrip().startswith("error")]
    ck("I no `error` diagnostic of any kind (no line opening with `error`, `error[...]`)", not errs, errs[:3])
    ck("I no compiler crash text outside cargo's own Running lines (internal compiler error, panicked, query stack, "
       "SIGSEGV, signal)", not [l for l in lines if CRASH_RE.search(l) and not PROGRESS_RE.match(l)])
    heads = [g["header"] for g in groups]
    want_heads = [w["header"] for w in wanted]
    ck("J exactly the five permitted warning groups, each once, in rustc's order (mission section 2), and no other "
       "diagnostic group", heads == want_heads, heads)
    by = {g["header"]: g for g in groups}
    for w in wanted:
        g = by.get(w["header"])
        if g is None:
            ck(f"J warning {w['n']} present: {w['header']}", False, "absent")
            continue
        ck(f"J warning {w['n']} ({w['header'][len('warning: '):]}): primary location {w['file']}:{w['line']}:{w['col']}",
           g["loc"] == (w["file"], w["line"], w["col"]), g["loc"])
        ck(f"J warning {w['n']}: quoted source lines {w['shown']}, each equal to the source the executable was built from",
           g["shown"] == w["shown"] and not g["problems"], f"{g['shown']} {g['problems'][:2]}")
        ck(f"J warning {w['n']}: carets exactly under {[c[3] for c in w['carets']]} and dashes exactly under "
           f"{[c[3] for c in w['dashes']]} (line, column, length, marked text)",
           [tuple(c) for c in g["carets"]] == [tuple(c) for c in w["carets"]]
           and [tuple(c) for c in g["dashes"]] == [tuple(c) for c in w["dashes"]], f"{g['carets']} {g['dashes']}")
        ck(f"J warning {w['n']}: labels and notes exactly the expected ones", g["labels"] == w["labels"]
           and g["notes"] == w["notes"], f"{g['labels']} {g['notes']}")
    ck("K every line of every group is a line of rustc's group format (header, location, gutter, quoted source, marker, "
       "'...', note); each group ends with one blank line", not problems, problems[:4])
    ck("K cargo's summary of exactly those five warnings, once (the summary, not a sixth diagnostic)", summary == 1,
       summary)
    ck("K every other stderr line is cargo progress (Compiling, Running), the Finished line or the Executable line: no "
       "other warning, lint, note or diagnostic", not unclassified and finished == 1 and executable == 1 and progress > 0,
       f"unclassified {unclassified[:4]}; Finished {finished}; Executable {executable}; progress {progress}")
    return {"groups": groups, "summary_lines": summary,
            "progress_lines": progress, "finished_lines": finished, "executable_lines": executable,
            "unclassified": unclassified, "problems": problems}


def toolchain_ok(tc):
    """E: the build's toolchain record equals each accepted Tranche-2 mutant build's (rustc/cargo -vV, active toolchain,
    other Cargo configuration files), and its build inputs are Candidate C's blobs."""
    refs = []
    for cid in K.ACCEPTED_T2:
        d = json.loads(K.blob(K.T2, f"{K.T2_PACKET}/{K.T2_RECEIPTS[cid]}/03-build/descriptor.json"))["toolchain"]
        refs.append(all(tc.get(k) == d.get(k) for k in ("rustc", "cargo", "rustc_vV", "cargo_vV", "active_toolchain",
                                                         "other_cargo_configs", "build_inputs")))
    inputs = {f: K.git("rev-parse", f"{K.C}:{f}").decode().strip() for f in tc.get("build_inputs", {})}
    return (all(refs) and len(refs) == 5 and all(tc.get(k) == v for k, v in TOOLCHAIN.items())
            and tc.get("other_cargo_configs") == []
            and sorted(tc.get("build_inputs", {})) == sorted(["rust-toolchain.toml", "Cargo.lock", ".cargo/config.toml",
                                                               "Cargo.toml", K.CRATE + "/Cargo.toml"])
            and all(tc["build_inputs"][f]["git_blob"] == inputs[f] for f in inputs)), refs


def admit(build_dir, ctl, synthetic=False):
    """The dedicated C5 build-admission checker (mission section 6). Returns (record, ok)."""
    ck = Checks()
    rec = {"control": ctl.get("id"), "build_dir": build_dir}
    if not ck("the control is C5 (the warning ruling is C5-specific: it does not transfer to A3, A6, C8, C15 or any "
              "other control)", ctl.get("id") == "C5" and ctl.get("patch", {}).get("sha256") == K.C5_PATCH_SHA256
              and ctl.get("mutant_blob") == K.C5_MUTANT_BLOB, ctl.get("id")):
        rec["checks"] = ck.items
        return rec, False
    pre = ctl["preserved"]["build_receipt"]
    sums = {l.split("  ", 1)[1]: l.split("  ", 1)[0] for l in K.blob(K.T2, K.T2_PACKET + "/SHA256SUMS").decode()
            .splitlines() if l}
    rel = pre["directory"][len(K.T2_PACKET) + 1:]
    present = sorted(os.listdir(build_dir)) if os.path.isdir(build_dir) else []
    if synthetic:
        ck("SYNTHETIC CONTROL (c2_controls.py): a tampered copy, never admissible; the preserved-receipt identity check "
           "is not run", False, build_dir)
    else:
        bad = [f for f in BUILD_FILES if not os.path.isfile(os.path.join(build_dir, f))
               or sums.get(f"{rel}/{f}") != K.sha256_file(os.path.join(build_dir, f))
               or K.git("rev-parse", f"{K.T2}:{pre['directory']}/{f}").decode().strip()
               != K.git_blob_id(open(os.path.join(build_dir, f), "rb").read())]
        ck(f"the build evidence is the preserved Tranche-2 receipt: each of its {len(BUILD_FILES)} files equals "
           f"{pre['directory']} at {K.T2[:8]} (SHA256SUMS sha256 and git blob); nothing else in the directory",
           not bad and present == sorted(BUILD_FILES), f"bad {bad}; present {present}")
        if bad or present != sorted(BUILD_FILES):
            rec["checks"] = ck.items
            return rec, False
    desc = json.load(open(os.path.join(build_dir, "descriptor.json")))
    rec["descriptor_sha256"] = K.sha256_file(os.path.join(build_dir, "descriptor.json"))
    sb, sa = desc.get("source_before", {}), desc.get("source_after", {})

    def mutant_ok(s):
        m = s.get("mutant_file") or {}
        return (s.get("verdict") == "EXACTLY_C_PLUS_PATCH" and s.get("head") == K.C and s.get("tree") == K.C_TREE
                and s.get("detached") is True and m.get("path") == K.PROTOCOL and m.get("git_blob") == K.C5_MUTANT_BLOB
                and m.get("sha256") == K.C5_MUTANT_SHA256 and s.get("other_content_mismatches") == []
                and s.get("missing") == [] and s.get("mode_mismatches") == [])
    ck("A source before the build: EXACTLY_C_PLUS_PATCH, C's HEAD and tree, the mutant protocol.rs blob and sha256 of "
       "section 2, no other mismatch", mutant_ok(sb), f"{sb.get('verdict')}; {(sb.get('mutant_file') or {}).get('git_blob')}")
    ck("B source after the build: EXACTLY_C_PLUS_PATCH, the same mutant blob and sha256, no other mismatch",
       mutant_ok(sa), f"{sa.get('verdict')}; {(sa.get('mutant_file') or {}).get('git_blob')}")
    ap = json.load(open(K.inputs("c5", "02-apply", "APPLY.json")))
    ck("C the build's patch and mutant identities are section 2's and the catalog's (descriptor; and the apply receipt: "
       "the frozen patch's sha256, git apply rc 0, EXACTLY_C_PLUS_PATCH, only protocol.rs changed)",
       (desc.get("control"), desc.get("role"), desc.get("patch_sha256"), desc.get("mutant_blob"),
        desc.get("mutant_sha256")) == ("C5", "mutant build", K.C5_PATCH_SHA256, K.C5_MUTANT_BLOB, K.C5_MUTANT_SHA256)
       and (ap["patch_sha256"], ap["catalog_patch_sha256"], ap["apply"]["rc"], ap["verify_mutant"]["verdict"],
            ap["changed_files"]) == (K.C5_PATCH_SHA256, ctl["patch"]["sha256"], 0, "EXACTLY_C_PLUS_PATCH", [K.PROTOCOL])
       and K.sha256_file(K.inputs("patches", "C5.patch")) == K.C5_PATCH_SHA256, desc.get("patch_sha256"))
    target = os.path.join(K.C5_WT, "target")
    ck("D the exact build command in the C5 worktree: cargo test -p nexus-governed-control --locked --release --lib "
       "--no-run -j 2", desc.get("argv") == K.BUILD_ARGV and desc.get("cwd") == K.C5_WT == desc.get("worktree"),
       " ".join(desc.get("argv", [])))
    ck("D the exact environment (env -i): HOME, PATH, CARGO_TARGET_DIR=<C5 worktree>/target, CARGO_TERM_VERBOSE, "
       "CARGO_NET_OFFLINE=true; no RUSTFLAGS, CARGO_ENCODED_RUSTFLAGS, feature or cfg variable; a fresh target",
       desc.get("env") == dict(K.build_env(target)) and desc.get("target") == target
       and "target directory " + target + " existed before: False" in open(os.path.join(build_dir, "RECEIPT.txt")).read(),
       desc.get("env"))
    tok, refs = toolchain_ok(desc.get("toolchain", {}))
    ck("E the exact toolchain and build inputs: rustc 1.94.0, cargo 1.94.0, toolchain 1.94.0-x86_64-unknown-linux-gnu, "
       "no other Cargo configuration; equal to each of the five accepted Tranche-2 builds' records; the five build "
       "inputs are Candidate C's blobs", tok, f"equal to the accepted builds: {refs}")
    proc, sup = desc.get("process", {}), desc.get("supervisor", {})
    ck("F normal exit 0: exit status 0, no signal, supervisor rc 0, the wrapper's recorded status 0",
       (proc.get("exit_status"), proc.get("signal"), proc.get("supervisor_rc"), proc.get("wrapper_recorded_status"))
       == (0, None, 0, "0"), proc)
    sr = open(os.path.join(build_dir, "scope-record.txt")).read()
    tv = open(os.path.join(build_dir, "time-v.txt")).read()
    ck("G 16 GiB, zero swap, within 900 s: memory.max 17179869184 and memory.swap.max 0 enforced (descriptor and the "
       "scope's own record), the 900 s timeout in the timed command, wall below 900 s, not timed out, complete record",
       sup.get("memory_max") == K.MEMORY_MAX == "17179869184" and sup.get("swap_max") == "0"
       and "memory.max: 17179869184\n" in sr and "memory.swap.max: 0\n" in sr and sup.get("time_limit_seconds") == 900
       and sup.get("wall_seconds") is not None and sup["wall_seconds"] < 900 and sup.get("timed_out") is False
       and sup.get("completed") is True and sup.get("scope_result") in ("gone", "success")
       and '\tCommand being timed: "timeout -s KILL 900 nice -n 10 cargo test -p nexus-governed-control --locked '
           '--release --lib --no-run -j 2"' in tv.splitlines(),
       f"wall {sup.get('wall_seconds')} s; memory.peak {sup.get('memory_peak_bytes')} B")
    ck("H no OOM and no signal: oom 0, oom_kill 0, swap peak 0 (descriptor and the scope's memory.events); GNU time "
       "records exit status 0 and no terminating signal",
       sup.get("oom") == 0 and sup.get("oom_kill") == 0 and sup.get("swap_peak_bytes") == "0"
       and "memory.events: oom 0\n" in sr and "memory.events: oom_kill 0\n" in sr and "memory.swap.peak: 0\n" in sr
       and "command exit status: 0\n" in sr and "\tExit status: 0" in tv.splitlines()
       and "Command terminated by signal" not in tv, f"oom {sup.get('oom')} oom_kill {sup.get('oom_kill')}")
    stderr = open(os.path.join(build_dir, "build.stderr"), encoding="utf-8").read()
    ck("the stderr is the preserved one: sha256 equals section 2's and the build descriptor's; stdout empty and recorded",
       K.sha256_file(os.path.join(build_dir, "build.stderr")) == K.C5_STDERR_SHA256 == desc.get("stderr_sha256")
       and os.path.getsize(os.path.join(build_dir, "build.stdout")) == 0
       and desc.get("stdout_sha256") == K.sha256_bytes(b""), desc.get("stderr_sha256"))
    src, err = mutant_sources(ctl)
    ck("the source the executable was built from, reproduced for the comparison: the mutant protocol.rs from the frozen "
       "patch (blob of section 2; never written to any object store) and C's model_tests.rs", src is not None, err)
    rec["stderr"] = stderr_admission(ck, stderr, src or {}, WARNINGS)
    finished = re.findall(r"^\s*Finished `(\w+)` profile \[([^\]]+)\] target\(s\) in (.+)$", stderr, re.M)
    ck("cargo's Finished line: release profile, optimized, once", len(finished) == 1
       and finished[0][:2] == ("release", "optimized"), finished)
    _, _, rustc, flags = classify.build_facts(build_dir, desc)
    classify.check_flags(ck, rustc, flags, None)
    verbose = re.findall(r"^\s*Executable `([^`]+)`\s*$", stderr, re.M)
    plain = re.findall(r"^\s*Executable unittests src/lib\.rs \(([^)]+)\)\s*$", stderr, re.M)
    tests = [a for a in desc.get("deps_artifacts", []) if a.get("test_executable_name")]
    others = [a for a in desc.get("deps_artifacts", []) if not a.get("test_executable_name")]
    ck("L cargo named exactly one library-test executable (both line forms), the intended one in the C5 worktree's own "
       "target; release/deps recorded exactly it and its dep-info", verbose + plain == [K.C5_EXE]
       and [a["path"] for a in tests] == [K.C5_EXE] and [a["path"] for a in others] == [K.C5_EXE + ".d"]
       and tests[0]["executable_bit"] is True, f"verbose {verbose}; plain {plain}")
    ck("M the recorded executable sha256 and size equal section 2's (90a60883…, 8,986,720 bytes)",
       len(tests) == 1 and (tests[0]["sha256"], tests[0]["size"]) == (K.C5_EXE_SHA256, K.C5_EXE_SIZE)
       == (ctl["preserved"]["executable"]["sha256"], ctl["preserved"]["executable"]["size"]),
       [(a["sha256"], a["size"]) for a in tests])
    fps = classify.fingerprints(target, os.path.basename(K.C5_EXE))
    ck("the test unit's fingerprint in the preserved target: no rustflags, no features (read-only)",
       fps and all(f["rustflags"] == [] and f["features"] == "[]" for f in fps),
       [(f["file"], f["rustflags"], f["features"]) for f in fps])
    cls = open(os.path.join(build_dir, "CLASSIFICATION.txt")).read().splitlines()
    chks = open(os.path.join(build_dir, "CHECKS.txt")).read().splitlines()
    ck("the historical classification is unchanged: BUILD: NOT OK, 17 of 18 checks, the one failed check the general "
       "classifier's `no error or warning diagnostic` (this admission does not relabel it)",
       cls[:1] == ["BUILD: NOT OK"] and "checks: 17 of 18 hold" in cls
       and [l.split(":")[0] for l in cls if l.startswith("FAILED CHECK: ")] == ["FAILED CHECK"]
       and [l for l in cls if l.startswith("FAILED CHECK: ")][0].startswith("FAILED CHECK: no error or warning diagnostic: ")
       and [l[5:].split(":")[0] for l in chks if l.startswith("FAIL ")] == ["no error or warning diagnostic"],
       cls[:1])
    rec["checks"] = ck.items
    rec["executable"] = tests[0] if len(tests) == 1 else None
    rec["dep_info"] = others[0] if len(others) == 1 else None
    rec["build_finished"] = [l for l in open(os.path.join(build_dir, "RECEIPT.txt")).read().splitlines()
                             if l.startswith(("launched: ", "returned: ", "finished: "))]
    return rec, ck.ok()


def parse_ts(s):
    return time.mktime(time.strptime(s[:19], "%Y-%m-%dT%H:%M:%S")) if s else None


def pin(ctl, build_dir):
    """The preserved executable pin (mission section 7), read-only. Returns (record, ok)."""
    ck = Checks()
    pv = ctl["preserved"]["executable"]
    path, wt = pv["path"], ctl["preserved"]["worktree"]
    target = os.path.join(wt, "target")
    rec = {"path": path, "expected_sha256": pv["sha256"], "expected_size": pv["size"]}
    try:
        st = os.lstat(path)
    except OSError as e:
        ck("the preserved executable exists", False, repr(e))
        rec["checks"] = ck.items
        return rec, False
    ck("the path is the ruling's (mission section 2) and the C5 worktree's own", path == K.C5_EXE and wt == K.C5_WT
       and path.startswith(target + "/"), path)
    ck("a regular file, not a symlink", stat.S_ISREG(st.st_mode) and not os.path.islink(path), oct(st.st_mode))
    comps = [path[:i] for i in range(len(path)) if path[i] == "/" and i > 0] + [path]
    ck("every path component is real (no symlink anywhere on the path; the path is its own realpath)",
       not [c for c in comps if os.path.islink(c)] and os.path.realpath(path) == path, "")
    tst = os.lstat(target) if os.path.lexists(target) else None
    mounts = [l.split()[4] for l in open("/proc/self/mountinfo")]
    ck("beneath the C5 worktree's own real target directory: a real directory (not a symlink), its own realpath, not "
       "a mount point and holding none", tst is not None and stat.S_ISDIR(tst.st_mode) and not os.path.islink(target)
       and os.path.realpath(target) == target and os.path.realpath(path).startswith(target + "/")
       and not [m for m in mounts if m == target or m.startswith(target + "/")], target)
    ck("the executable bit is set (as the build recorded it: executable_bit true)", bool(st.st_mode & 0o100),
       oct(st.st_mode & 0o777))
    sha = K.sha256_file(path)
    rec.update({"sha256": sha, "size": st.st_size, "mode": oct(st.st_mode), "inode": st.st_ino, "dev": st.st_dev,
                "mtime_ns": st.st_mtime_ns, "ctime_ns": st.st_ctime_ns})
    ck("sha256 exact (mission section 2)", sha == pv["sha256"] == K.C5_EXE_SHA256, sha)
    ck("size exact (8,986,720 bytes)", st.st_size == pv["size"] == K.C5_EXE_SIZE, st.st_size)
    desc = json.load(open(os.path.join(build_dir, "descriptor.json")))
    rec_d = [a for a in desc["deps_artifacts"] if a["path"] == path + ".d"]
    dpath = path + ".d"
    dsha = K.sha256_file(dpath) if os.path.isfile(dpath) and not os.path.islink(dpath) else None
    dtext = open(dpath, errors="replace").read() if dsha else ""
    rule = [l for l in dtext.split("\n") if l.startswith(path + ": ")]
    ck("dep-info identity as the build recorded it: the .d file's sha256 and size; it has exactly one rule for the "
       "executable, whose inputs include the worktree's protocol.rs and model_tests.rs (paths relative to the C5 "
       "worktree, the build's cwd)",
       len(rec_d) == 1 and dsha == rec_d[0]["sha256"] and os.path.getsize(dpath) == rec_d[0]["size"]
       and len(rule) == 1 and K.PROTOCOL in rule[0].split(" ")[1:] and K.MODEL in rule[0].split(" ")[1:],
       f"{dsha} (recorded {rec_d[0]['sha256'] if rec_d else None}); rules for the executable {len(rule)}")
    deps = os.path.dirname(path)
    found = sorted((n, K.sha256_file(os.path.join(deps, n)), os.path.getsize(os.path.join(deps, n)))
                   for n in os.listdir(deps) if n.startswith("nexus_governed_control"))
    want = sorted((os.path.basename(a["path"]), a["sha256"], a["size"]) for a in desc["deps_artifacts"])
    ck("release/deps holds exactly the two nexus_governed_control artifacts the build recorded (names, sha256, sizes)",
       found == want, found)
    lines = open(os.path.join(build_dir, "RECEIPT.txt")).read().splitlines()
    launched = parse_ts(next((l[len("launched: "):] for l in lines if l.startswith("launched: ")), None))
    finished = parse_ts(next((l[len("finished: "):] for l in lines if l.startswith("finished: ")), None))
    newest, oldest, n = 0.0, None, 0
    for d, dirs, files in os.walk(target):
        for x in [d] + [os.path.join(d, f) for f in files] + [os.path.join(d, f) for f in dirs]:
            s = os.lstat(x)
            newest = max(newest, s.st_mtime, s.st_ctime)
            oldest = s.st_ctime if oldest is None else min(oldest, s.st_ctime)
            n += 1
    rec.update({"target_entries": n, "target_newest_mtime_or_ctime": newest, "build_launched": launched,
                "build_finished": finished})
    ck("no target or executable modification since the build record: no entry under the target (files and "
       "directories, mtime or ctime) is newer than the build receipt's finish (1 s resolution); every entry's ctime is "
       "after the build's launch (a fresh target)", finished is not None and newest < finished + 1
       and oldest is not None and launched is not None and oldest >= launched - 1,
       f"{n} entries; newest {time.strftime('%H:%M:%S', time.localtime(newest))}; build "
       f"{time.strftime('%H:%M:%S', time.localtime(launched or 0))}-{time.strftime('%H:%M:%S', time.localtime(finished or 0))}")
    rec["checks"] = ck.items
    return rec, ck.ok()


def worktree_check(ctl, when, restored_check):
    """The preserved C5 worktree, read-only (mission sections 7 and 15). Returns (record, ok)."""
    ck = Checks()
    wt = ctl["preserved"]["worktree"]
    rec = {"path": wt, "when": when}
    if not os.path.isdir(wt):
        ck("the preserved C5 worktree exists", False, wt)
        rec["checks"] = ck.items
        return rec, False
    st = os.lstat(wt)
    ck("filesystem identity: the path is its own realpath, a directory, not a symlink, owned by this user",
       os.path.realpath(wt) == wt and os.path.isdir(wt) and not os.path.islink(wt) and st.st_uid == os.getuid(),
       {"dev": st.st_dev, "ino": st.st_ino, "uid": st.st_uid})
    dotgit = os.path.join(wt, ".git")
    gitfile = open(dotgit).read().strip() if os.path.isfile(dotgit) else None
    admin = gitfile[len("gitdir: "):] if gitfile and gitfile.startswith("gitdir: ") else None
    back = open(os.path.join(admin, "gitdir")).read().strip() if admin and os.path.exists(
        os.path.join(admin, "gitdir")) else None
    locked = open(os.path.join(admin, "locked")).read().strip() if admin and os.path.exists(
        os.path.join(admin, "locked")) else None
    ck("registered: the .git file points into the repository's worktree administration, which points back",
       admin is not None and os.path.dirname(admin) == os.path.join(K.GIT_COMMON_DIR, "worktrees") and back == dotgit,
       gitfile)
    ck("locked with the Tranche-2 reason (its owner mission; this mission does not take it over)",
       locked == ctl["preserved"]["lock_reason"] == K.C5_LOCK_REASON, locked)
    entry = worktrees.worktree_entry(wt)
    ck("git worktree list: this path, HEAD C, detached, locked", entry is not None and f"HEAD {K.C}" in entry
       and "detached" in entry and any(l.startswith("locked") for l in entry), entry)
    env = dict(os.environ, GIT_OPTIONAL_LOCKS="0")
    import subprocess
    head = subprocess.run(["git", "-C", wt, "rev-parse", "HEAD", "HEAD^{tree}"], capture_output=True, text=True,
                          env=env).stdout.split()
    sym = subprocess.run(["git", "-C", wt, "symbolic-ref", "-q", "HEAD"], capture_output=True, env=env).returncode
    ck("HEAD is Candidate C and its tree, detached", head == [K.C, K.C_TREE] and sym != 0, head)
    os.environ["GIT_OPTIONAL_LOCKS"] = "0"
    try:
        v = restored_check(wt, when)
    finally:
        del os.environ["GIT_OPTIONAL_LOCKS"]
    rec["restored_check"] = v
    ck("source restored exactly to Candidate C: verify_tree.py EXACTLY_C (every tracked byte and mode) and the path-aware "
       "status policy (no tracked change; ignored entries only beneath its own real target directory)",
       v["verdict"] == "EXACTLY_C", v["line"])
    data = open(os.path.join(wt, K.PROTOCOL), "rb").read()
    ck("protocol.rs is C's blob again (computed in-process)", K.git_blob_id(data) == K.BLOBS[K.PROTOCOL],
       K.git_blob_id(data))
    rec["checks"] = ck.items
    return rec, ck.ok()


def write(outdir, name, first_ok, first_fail, title, rec, ok):
    os.makedirs(outdir, exist_ok=True)
    json.dump(rec, open(os.path.join(outdir, name + ".json"), "w"), indent=1, default=str)
    text = "\n".join(text_lines(rec.get("checks", []), [first_ok if ok else first_fail, title])) + "\n"
    open(os.path.join(outdir, name + ".txt"), "w").write(text)
    print(text, end="")
