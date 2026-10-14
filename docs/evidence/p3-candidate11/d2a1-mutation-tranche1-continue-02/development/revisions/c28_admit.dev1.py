#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02, sections 2, 8 and 14: the dedicated, read-only C28 existing-build
admission checker and the dedicated C28 pin step.

Nothing here runs Cargo or rustc, builds, rebuilds, writes to a worktree or a target directory, or classifies any
control other than C28. The general build classifier (classify.classify_build_success) is not used, not changed and
not bypassed for any other control: it classified the C28 build `BUILD: NOT OK` (RESUME-01 executor history, unchanged)
and still would. This tool applies only the Sole Architect's narrow C28 ruling (documents/ARCHITECT-RULING-C28-BUILD-
ADMISSIBILITY.md): exit 0; no signal, timeout or OOM; no error diagnostic; exactly one warning, of class
unreachable_patterns, at protocol.rs:1834:17, explaining that the earlier `Read::Got(_)` matches all relevant values;
no other warning or diagnostic; source exactly C plus the frozen C28 patch before and after the build; and the
expected executable present and byte-identical to the recorded artifact.

    (library) evidence()                   the RESUME-01 C28 receipts, read from git objects at 3695a817, each checked
                                           against that packet's SHA256SUMS (the previous packet is the authority; no
                                           local copy is trusted)
    (library) admit(ev, cid, ...)          the ruling's conditions as named checks; any cid but C28 is refused
    (library) exe_facts(path, wt, bound)   read-only filesystem facts of an executable beneath a worktree's target
    (library) pin_checks(facts, ...)       the pin's conditions on those facts
    c28_admit.py admit OUTDIR              writes OUTDIR/ADMISSION.txt and ADMISSION.json; first line
                                           `C28 BUILD: ARCHITECT-ADMISSIBLE — NARROW C28 RULING` or
                                           `C28 BUILD: NOT ADMISSIBLE`
    c28_admit.py pin OUTDIR                writes OUTDIR/PIN.txt and PIN.json; first line
                                           `PIN: PINNED — EXISTING C28 BUILD (NOT REBUILT)` or
                                           `PIN: C28_PRESERVED_ARTIFACT_BLOCKED`
Both refuse an existing OUTDIR; neither retries anything.
"""
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402
import verify_mutant  # noqa: E402

ADMITTED = "C28 BUILD: ARCHITECT-ADMISSIBLE — NARROW C28 RULING"
NOT_ADMITTED = "C28 BUILD: NOT ADMISSIBLE"
PINNED = "PIN: PINNED — EXISTING C28 BUILD (NOT REBUILT)"
PIN_BLOCKED = "PIN: C28_PRESERVED_ARTIFACT_BLOCKED"
BUILD_DIR = "receipts/36-C28"
WT = K.control_wt("C28")
# The ruling's identities, as the mission states them (section 8).
RULING = {"control": "C28", "worktree": WT,
          "executable": WT + "/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3",
          "exe_sha256": "116a6584ebe58c1fea17e2067c6f2af3f3c365cb64f595648040f5c797a5de36", "exe_size": 8983776,
          "mutant_blob": "446d0365e1472efc40321d259a6f1171d3fe716c",
          "mutant_sha256": "1b59477e6c974f79c9c698916aa2aad350c4c27f1130c86481a15277572c77d8",
          "patch_sha256": "519378de0e6b6a05019e6a2dc718441b2a03970ae0341d89d858f1168ee23868",
          "stderr_sha256": "3e8336d9b25baf56610aad813202e9a3bea60b268de8e0a489b69d6b2caf7436",
          "warning_message": "unreachable pattern", "lint": "unreachable_patterns",
          "location": (K.PROTOCOL, 1834, 17), "earlier_line": 1826,
          "lock_reason": f"{K.RESUME_MISSION} control C28 disposable mutant worktree: mission-owned; preserve until "
                         f"Architect review"}
FILES = ["00-GATE.txt", "02-apply/APPLY.json", "02-apply/APPLY.txt", "03-build/descriptor.json",
         "03-build/build.stderr", "03-build/build.stdout", "03-build/PARSED-RECEIPT.json", "03-build/CLASSIFICATION.txt",
         "03-build/CHECKS.txt", "03-build/RECEIPT.txt", "03-build/scope-record.txt", "03-build/time-v.txt",
         "05-restore/RESTORE.json", "05-restore/RESTORE.txt", "RESULT.txt"]
PROFILE = ["opt-level=3", "lto=thin", "codegen-units=16", "strip=symbols"]
SUMMARY = re.compile(r"^warning: `([^`]+)` \(([^)]+)\) generated (\d+) warnings?(?: \(.*\))?$")
DIAG = re.compile(r"^(error|warning)(?:\[([A-Za-z0-9_]+)\])?(?::|\b)")


class Checks:
    def __init__(self):
        self.items = []

    def __call__(self, label, ok, detail=""):
        self.items.append((label, bool(ok), str(detail)))
        return bool(ok)

    def ok(self):
        return bool(self.items) and all(o for _, o, _ in self.items)

    def failed(self):
        return [l for l, o, _ in self.items if not o]


def git(*args):
    return subprocess.run(["git", "-C", K.REPO, *args], capture_output=True, check=True).stdout


def evidence():
    """The C28 receipts of the RESUME-01 packet, from git objects, each verified against that packet's SHA256SUMS."""
    sums = {l.split("  ", 1)[1]: l.split("  ", 1)[0]
            for l in git("show", f"{K.RESUME}:{K.RESUME_PACKET}/SHA256SUMS").decode().splitlines() if l}
    raw, auth = {}, []
    for f in FILES:
        rel = f"{BUILD_DIR}/{f}"
        b = git("rev-parse", f"{K.RESUME}:{K.RESUME_PACKET}/{rel}").decode().strip()
        data = git("cat-file", "blob", b)
        raw[f] = data
        auth.append({"path": f"{K.RESUME_PACKET}/{rel}", "blob": b, "sha256": K.sha256_bytes(data),
                     "listed_sha256": sums.get(rel), "ok": sums.get(rel) == K.sha256_bytes(data)})
    j = lambda f: json.loads(raw[f])
    t = lambda f: raw[f].decode("utf-8")
    return {"commit": K.RESUME, "auth": auth, "apply": j("02-apply/APPLY.json"), "apply_txt": t("02-apply/APPLY.txt"),
            "desc": j("03-build/descriptor.json"), "stderr": t("03-build/build.stderr"),
            "stderr_bytes": raw["03-build/build.stderr"], "stdout_bytes": raw["03-build/build.stdout"],
            "parsed": j("03-build/PARSED-RECEIPT.json"), "cls": t("03-build/CLASSIFICATION.txt"),
            "checks": t("03-build/CHECKS.txt"), "receipt": t("03-build/RECEIPT.txt"),
            "scope_record": t("03-build/scope-record.txt"), "time_v": t("03-build/time-v.txt"),
            "restore": j("05-restore/RESTORE.json"), "restore_txt": t("05-restore/RESTORE.txt"),
            "result": t("RESULT.txt"), "gate": t("00-GATE.txt")}


def reference_toolchains():
    """The toolchain and build inputs of RESUME-01's C27 build (the control built just before C28, same mission) and of
    EXEC-01's healthy (e4) build: the exact toolchain the C28 build must share."""
    out = {}
    for name, commit, path in (("RESUME-01 C27 build", K.RESUME, f"{K.RESUME_PACKET}/receipts/35-C27/03-build/descriptor.json"),
                               ("EXEC-01 healthy build", K.PREV, f"{K.PREV_PACKET}/receipts/20-healthy-build/descriptor.json")):
        out[name] = json.loads(git("show", f"{commit}:{path}"))["toolchain"]
    return out


def catalog_entry(cid):
    cat = json.loads(git("show", f"{K.R2}:{K.CATALOG_PATH}"))
    return next((e for e in cat["controls"] if e["id"] == cid), None)


def mutant_bytes(entry):
    """The catalog's mutant of C's protocol.rs, computed in-process from the catalog edits (never written anywhere)."""
    text = git("cat-file", "blob", K.BLOBS[K.PROTOCOL]).decode()
    for ed in entry["edits"]:
        text = text.replace(ed["anchor"], ed["replacement"], 1)
    return text.encode()


def warning_blocks(lines):
    """Each diagnostic header line (column 0, `error`/`warning`) with the block that follows it up to a blank line."""
    out = []
    for i, l in enumerate(lines):
        if DIAG.match(l):
            j = i + 1
            while j < len(lines) and lines[j] != "":
                j += 1
            out.append((i, j))
    return out


def admit(ev, cid, entry=None, toolchains=None, c_protocol=None, mutant=None, ruling=RULING):
    """The narrow C28 ruling's conditions over the evidence ev (a dict as evidence() returns). Returns Checks."""
    ck = Checks()
    if not ck("the control is C28 (the ruling is C28-specific; it transfers to no other control)",
              cid == ruling["control"], cid):
        return ck
    entry = entry or catalog_entry(cid)
    toolchains = toolchains if toolchains is not None else reference_toolchains()
    c_protocol = c_protocol if c_protocol is not None else git("cat-file", "blob", K.BLOBS[K.PROTOCOL])
    mutant = mutant if mutant is not None else mutant_bytes(entry)
    desc, ap, stderr = ev["desc"], ev["apply"], ev["stderr"]
    ck("A1 every C28 evidence file is the RESUME-01 packet's (sha256 listed in its SHA256SUMS at 3695a817)",
       ev["auth"] and all(a["ok"] for a in ev["auth"]), [a["path"] for a in ev["auth"] if not a["ok"]])
    ck("A2 the frozen C28 patch: catalog patch sha256 = applied patch sha256 = the ruling's",
       entry["patch_sha256"] == ap["patch_sha256"] == ap["catalog_patch_sha256"] == ruling["patch_sha256"],
       ap["patch_sha256"])
    ck("A2 git apply --check and git apply rc 0; the only changed file is protocol.rs",
       ap["check"]["rc"] == 0 and ap["apply"]["rc"] == 0 and ap["changed_files"] == [K.PROTOCOL]
       and ev["apply_txt"].splitlines()[0] == "APPLY: EXACTLY_C_PLUS_PATCH", ap["changed_files"])
    ck("A2 the catalog's mutant: blob and sha256 equal the ruling's; the in-process mutant (catalog edits on C's "
       "protocol.rs) has that blob", entry["mutant_blob"] == ruling["mutant_blob"] == K.git_blob_id(mutant)
       and entry["mutant_sha256"] == ruling["mutant_sha256"] == K.sha256_bytes(mutant), K.git_blob_id(mutant))
    for when in ("source_before", "source_after"):
        s = desc[when]
        m = s.get("mutant_file") or {}
        pol = s.get("status_policy") or {}
        ck(f"A3 {when} the build: exactly C plus the frozen C28 patch (HEAD/tree C, detached, nothing staged, every other "
           "tracked byte and mode equal, mutant blob and sha256 exact, path-aware status policy holds)",
           s["verdict"] == "EXACTLY_C_PLUS_PATCH" and s["head"] == K.C and s["tree"] == K.C_TREE and s["detached"]
           and s["index_differs_from_head"] == [] and s["other_content_mismatches"] == [] and s["missing"] == []
           and s["mode_mismatches"] == [] and m.get("path") == K.PROTOCOL and m.get("git_blob") == ruling["mutant_blob"]
           and m.get("sha256") == ruling["mutant_sha256"] and pol.get("ok") is True
           and pol.get("modified_seen") == [K.PROTOCOL] and pol.get("rejected") == [],
           f"{s['verdict']}; {m.get('git_blob')}; status {s.get('status_entries')}")
    ck("A4 the exact build command, in the C28 worktree", desc["argv"] == K.BUILD_ARGV and desc["cwd"] ==
       desc["worktree"] == ruling["worktree"], " ".join(desc["argv"]))
    ck("A5 the exact environment (env -i): HOME, PATH, CARGO_TARGET_DIR=<C28 worktree>/target, CARGO_TERM_VERBOSE, "
       "CARGO_NET_OFFLINE=true; nothing else (no RUSTFLAGS, CARGO_ENCODED_RUSTFLAGS, feature or cfg variable)",
       desc["env"] == dict(K.build_env(ruling["worktree"] + "/target"))
       and desc["target"] == ruling["worktree"] + "/target", desc["env"])
    tc = desc["toolchain"]
    ck("A6 the exact toolchain: rustc -vV, cargo -vV and the active toolchain equal RESUME-01's C27 build's and "
       "EXEC-01's healthy build's", toolchains and all(all(tc[k] == r[k] for k in ("rustc_vV", "cargo_vV",
                                                                                    "active_toolchain"))
                                                       for r in toolchains.values()), tc["rustc"])
    inputs_c = {f: git("rev-parse", f"{K.C}:{f}").decode().strip() for f in tc["build_inputs"]}
    ck("A6 the exact build inputs: rust-toolchain.toml, Cargo.lock, .cargo/config.toml, Cargo.toml and the crate "
       "manifest have Candidate C's blobs, equal to the reference builds'",
       sorted(tc["build_inputs"]) == sorted(["rust-toolchain.toml", "Cargo.lock", ".cargo/config.toml", "Cargo.toml",
                                             K.CRATE + "/Cargo.toml"])
       and all(v["git_blob"] == inputs_c[f] for f, v in tc["build_inputs"].items())
       and all(r["build_inputs"] == tc["build_inputs"] for r in toolchains.values()), "")
    ck("A6 no other Cargo configuration file in effect", tc["other_cargo_configs"] == [], tc["other_cargo_configs"])
    proc, sup = desc["process"], desc["supervisor"]
    ck("A7 process exit 0 by a normal exit; no signal; supervisor and wrapper record exit 0",
       proc["exit_status"] == 0 and proc["signal"] is None and proc["supervisor_rc"] == 0
       and proc["wrapper_recorded_status"] == "0", proc)
    ck("A8 resources: 16 GiB, zero swap, enforced and complete; no OOM, no OOM kill, no swap; no timeout; within 900 s",
       sup["memory_max"] == K.MEMORY_MAX and sup["swap_max"] == "0" and sup["oom"] == 0 and sup["oom_kill"] == 0
       and sup["swap_peak_bytes"] == "0" and sup["completed"] is True and sup["timed_out"] is False
       and sup["time_limit_seconds"] == K.BUILD_LIMIT and sup["wall_seconds"] is not None
       and sup["wall_seconds"] < K.BUILD_LIMIT and sup["scope_result"] in ("gone", "success"),
       f"wall {sup['wall_seconds']} s; peak {sup['memory_peak_bytes']} B; swap {sup['swap_peak_bytes']}; oom_kill "
       f"{sup['oom_kill']}; timed out {sup['timed_out']}")
    sr = dict(l.split(": ", 1) for l in ev["scope_record"].splitlines() if ": " in l)
    ck("A8 the resource receipt (scope-record.txt, time-v.txt) agrees: memory.max, swap.max, peak, swap peak, exit 0",
       sr.get("memory.max") == sup["memory_max"] and sr.get("memory.swap.max") == "0"
       and sr.get("memory.peak") == sup["memory_peak_bytes"] and sr.get("memory.swap.peak") == "0"
       and sr.get("command exit status") == "0" and "Exit status: 0" in ev["time_v"]
       and "Command terminated by signal" not in ev["time_v"], {k: sr.get(k) for k in ("memory.peak", "command exit status")})
    ck("A9 build.stderr sha256 = the launcher's record = the ruling's; build.stdout sha256 = the launcher's record",
       K.sha256_bytes(ev["stderr_bytes"]) == desc["stderr_sha256"] == ruling["stderr_sha256"]
       and K.sha256_bytes(ev["stdout_bytes"]) == desc["stdout_sha256"], K.sha256_bytes(ev["stderr_bytes"]))
    lines = stderr.splitlines()
    heads = [(i, l) for i, l in enumerate(lines) if DIAG.match(l)]
    errors = [l for _, l in heads if l.startswith("error")]
    summaries = [(i, SUMMARY.match(l)) for i, l in heads if SUMMARY.match(l)]
    warns = [(i, l) for i, l in heads if l.startswith("warning") and not SUMMARY.match(l)]
    ck("A10 no error diagnostic (no stderr line begins `error`)", not errors, errors[:3])
    ck("A11 exactly one warning diagnostic", len(warns) == 1, [l for _, l in warns][:4])
    ck("A11 exactly one cargo summary, for nexus-governed-control (lib test), counting exactly 1 warning",
       len(summaries) == 1 and summaries[0][1].groups() == ("nexus-governed-control", "lib test", "1"),
       [lines[i] for i, _ in summaries])
    block, loc, note = [], None, None
    if len(warns) == 1:
        i0 = warns[0][0]
        j = i0 + 1
        while j < len(lines) and lines[j] != "":
            j += 1
        block = lines[i0:j]
        arrow = [re.match(r"^\s*--> (\S+):(\d+):(\d+)$", l) for l in block[1:2]]
        loc = (arrow[0].group(1), int(arrow[0].group(2)), int(arrow[0].group(3))) if arrow and arrow[0] else None
        notes = [re.match(r"^\s*= note: `#\[warn\(([a-z_]+)\)\]`", l) for l in block]
        notes = [n.group(1) for n in notes if n]
        note = notes[0] if len(notes) == 1 else notes
    ck("A12 the warning's class is unreachable_patterns (message `unreachable pattern`; the lint note names "
       "`#[warn(unreachable_patterns)]`; no code, no other lint)",
       len(warns) == 1 and warns[0][1] == f"warning: {ruling['warning_message']}" and note == ruling["lint"],
       f"{warns[0][1] if warns else None}; lint {note}")
    ck(f"A13 the warning's location is {ruling['location'][0]}:{ruling['location'][1]}:{ruling['location'][2]}",
       loc == tuple(ruling["location"]), loc)
    src = lambda n: re.compile(rf"^{n} \|(.*)$")
    earlier = [k for k, l in enumerate(block) if src(ruling["earlier_line"]).match(l)]
    later = [k for k, l in enumerate(block) if src(ruling["location"][1]).match(l)]
    ck(f"A14 the warning explains that the earlier `Read::Got(_)` (line {ruling['earlier_line']}) matches all the "
       f"relevant values and that no value can reach line {ruling['location'][1]}",
       len(earlier) == 1 and len(later) == 1 and earlier[0] < later[0]
       and src(ruling["earlier_line"]).match(block[earlier[0]]).group(1).strip() == "Read::Got(_) => {"
       and block[earlier[0] + 1].strip().endswith("matches all the relevant values")
       and src(ruling["location"][1]).match(block[later[0]]).group(1).strip() == "Read::Got(_) => {"
       and block[later[0] + 1].strip().endswith("no value can reach this"), block[:9])
    mlines, clines = mutant.decode().splitlines(), c_protocol.decode().splitlines()
    e_n, w_n, col = ruling["earlier_line"], ruling["location"][1], ruling["location"][2]
    ck("A15 the warning is the frozen mutation's direct consequence: at C, line "
       f"{e_n} reads `Read::Got(durable) if durable >= set.top => {{`; in the catalog mutant lines {e_n} and {w_n} both "
       f"read `Read::Got(_) => {{` from column {col}, and the warning renders exactly those mutant lines",
       len(mlines) == len(clines) and clines[e_n - 1][col - 1:] == "Read::Got(durable) if durable >= set.top => {"
       and mlines[e_n - 1][col - 1:] == "Read::Got(_) => {" == mlines[w_n - 1][col - 1:]
       and clines[w_n - 1] == mlines[w_n - 1]
       and [i for i, (a, b) in enumerate(zip(clines, mlines)) if a != b] == [e_n - 1]
       and len(earlier) == 1 and len(later) == 1
       and src(e_n).match(block[earlier[0]]).group(1)[1:] == mlines[e_n - 1]
       and src(w_n).match(block[later[0]]).group(1)[1:] == mlines[w_n - 1], "")
    known = set(range(*([warns[0][0], warns[0][0] + len(block) + 1] if block else [0, 0])))
    known |= {i for i, _ in summaries}
    residual = [l for i, l in enumerate(lines) if i not in known and not l.startswith("     Running `")
                and not re.match(r"^   Compiling \S+ v\S+( \(.*\))?$", l)
                and not re.match(r"^    Finished `release` profile \[optimized\] target\(s\) in .+$", l)
                and not re.match(r"^  Executable `[^`]+`$", l)]
    ck("A16 no other diagnostic or unexpected output: every stderr line is a cargo `Compiling`/`Running` line, the one "
       "warning block, the one summary, `Finished` or the `Executable` line", not residual, residual[:4])
    ck("A17 no compiler crash text (internal compiler error, panic, query stack, signal)",
       not re.search(r"internal compiler error|panicked|query stack|SIGSEGV|signal: ", stderr), "")
    fin = re.findall(r"^\s*Finished `(\w+)` profile \[([^\]]+)\] target\(s\) in (.+)$", stderr, re.M)
    rustc = [l for l in lines if "--crate-name nexus_governed_control " in l and l.lstrip().startswith("Running `")]
    flags = re.findall(r" -C (\S+)", rustc[0]) if len(rustc) == 1 else []
    ck("A18 cargo's Finished line (release, optimized); one rustc invocation compiled nexus_governed_control as a "
       "test; profile flags opt-level=3, lto=thin, codegen-units=16, strip=symbols; no --cfg",
       len(fin) == 1 and fin[0][:2] == ("release", "optimized") and len(rustc) == 1 and " --test " in rustc[0]
       and [f for f in flags if not f.startswith(("metadata=", "extra-filename="))] == PROFILE
       and not re.findall(r" --cfg (\S+)", rustc[0]), f"{fin}; {flags}")
    named = re.findall(r"^\s*Executable `([^`]+)`\s*$", stderr, re.M)
    tests = [a for a in desc["deps_artifacts"] if a["test_executable_name"]]
    others = [a for a in desc["deps_artifacts"] if not a["test_executable_name"]]
    ck("A19 the produced-artifact record: cargo named exactly one library-test executable, the expected one; release/deps "
       "held exactly that executable (sha256, size and executable bit as the ruling states) and its dep-info only",
       named == [ruling["executable"]] and len(tests) == 1 and tests[0]["path"] == ruling["executable"]
       and tests[0]["sha256"] == ruling["exe_sha256"] and tests[0]["size"] == ruling["exe_size"]
       and tests[0]["executable_bit"] and len(others) == 1 and others[0]["path"] == ruling["executable"] + ".d",
       [(os.path.basename(a["path"]), a["sha256"], a["size"]) for a in desc["deps_artifacts"]])
    cls, chks = ev["cls"].splitlines(), ev["checks"].splitlines()
    fails = [l for l in chks if l.startswith("FAIL")]
    ck("A20 executor history, unchanged: the general classifier's C28 output is `BUILD: NOT OK` with 17 of 18 checks "
       "holding; its only failed check is `no error or warning diagnostic`, naming exactly this warning and its summary",
       cls[0] == "BUILD: NOT OK" and "checks: 17 of 18 hold" in cls and len(chks) == 18 and len(fails) == 1
       and fails[0] == "FAIL no error or warning diagnostic: ['warning: unreachable pattern', 'warning: "
                       "`nexus-governed-control` (lib test) generated 1 warning']", fails)
    ck("A20 executor history, unchanged: RESULT `CONTROL RESULT: MUTATION_CONTROL_BLOCKED`; no PIN.json was written",
       ev["result"].splitlines()[0] == "CONTROL RESULT: MUTATION_CONTROL_BLOCKED" and ev["parsed"]["executable"] is None,
       ev["result"].splitlines()[0])
    rs = ev["restore"]
    ck("A21 the RESUME-01 restoration: git apply -R --check and git apply -R rc 0; protocol.rs from the mutant blob back to "
       "C's blob and sha256; verify_tree.py EXACTLY_C; the path-aware restored policy held",
       rs["applied_before_restoration"] is True and rs["check"]["rc"] == 0 and rs["reverse"]["rc"] == 0
       and rs["before"]["git_blob"] == ruling["mutant_blob"] and rs["after"]["git_blob"] == K.BLOBS[K.PROTOCOL]
       == entry["baseline_blob"] and rs["after"]["sha256"] == K.sha256_bytes(c_protocol)
       and rs["verify_tree"]["verdict"] == "EXACTLY_C" and rs["status_policy"]["ok"] is True
       and ev["restore_txt"].splitlines()[0] == "RESTORE: EXACTLY_C",
       f"{rs['before']['git_blob']} -> {rs['after']['git_blob']}")
    return ck


def build_bound(ev):
    """The end of the build's recorded run: the build receipt's `finished:` time (second resolution) plus one second."""
    m = re.search(r"^finished: (\S+)$", ev["receipt"], re.M)
    t = time.mktime(time.strptime(m.group(1)[:19], "%Y-%m-%dT%H:%M:%S")) if m else None
    return (t + 1.0) if t is not None else None, (m.group(1) if m else None)


def exe_facts(path, wt, bound, newer_limit=20):
    """Read-only facts of an executable that must lie beneath wt's own real target directory."""
    f = {"path": path, "worktree": wt, "path_is_normal_absolute": os.path.isabs(path) and os.path.normpath(path) == path,
         "worktree_realpath": os.path.realpath(wt), "exists": os.path.lexists(path)}
    tf = verify_mutant.target_facts(wt)
    f["target"] = {k: v for k, v in tf.items() if k != "path"}
    target = os.path.join(wt, "target")
    f["beneath_target"] = path.startswith(target + "/")
    if f["exists"]:
        st = os.lstat(path)
        f.update(is_regular=stat.S_ISREG(st.st_mode), is_symlink=stat.S_ISLNK(st.st_mode),
                 realpath=os.path.realpath(path), size=st.st_size, executable_bit=bool(st.st_mode & 0o100),
                 inode=st.st_ino, dev=st.st_dev, nlink=st.st_nlink, mtime_ns=st.st_mtime_ns, ctime_ns=st.st_ctime_ns,
                 uid=st.st_uid)
        f["sha256"] = K.sha256_file(path) if f["is_regular"] else None
        f["realpath_beneath_target"] = f["realpath"].startswith(os.path.join(f["worktree_realpath"], "target") + "/")
    newer, scanned = [], 0
    if bound is not None and tf.get("exists") and tf.get("ok"):
        for d, dirs, files in os.walk(target):
            dirs.sort()
            for n in sorted(dirs + files):
                p = os.path.join(d, n)
                st = os.lstat(p)
                scanned += 1
                if max(st.st_mtime, st.st_ctime) > bound and len(newer) < newer_limit:
                    newer.append((os.path.relpath(p, target), st.st_mtime, st.st_ctime))
        st = os.lstat(target)
        if max(st.st_mtime, st.st_ctime) > bound:
            newer.insert(0, (".", st.st_mtime, st.st_ctime))
    f["target_entries_scanned"] = scanned
    f["changed_after_bound"] = newer
    f["bound"] = bound
    return f


def pin_checks(f, ruling=RULING, dep_info=None, worktree_entry=None):
    """The pin's conditions on exe_facts(); dep_info: (path, sha256, size) recorded by the build; worktree_entry: the
    `git worktree list --porcelain` block of the C28 worktree and its lock reason (None for synthetic controls)."""
    ck = Checks()
    ck("P1 the executable path is the ruling's (the one the build recorded)", f["path"] == ruling["executable"]
       and f["worktree"] == ruling["worktree"] and f["path_is_normal_absolute"], f["path"])
    ck("P2 it exists, as a regular file, not a symlink, with no symlinked path component",
       f["exists"] and f.get("is_regular") and not f.get("is_symlink") and f.get("realpath") == f["path"],
       {k: f.get(k) for k in ("exists", "is_regular", "is_symlink", "realpath")})
    ck("P3 it lies beneath the C28 worktree's own target: the target is a real directory (not a symlink, not a mount, "
       "the worktree's own entry) and the executable's realpath is beneath it",
       f["worktree_realpath"] == f["worktree"] and f["target"].get("exists") and f["target"].get("ok")
       and f["beneath_target"] and f.get("realpath_beneath_target"), f["target"])
    ck("P4 byte-identical to the recorded artifact: sha256 and size equal the ruling's (= the build's deps record)",
       f.get("sha256") == ruling["exe_sha256"] and f.get("size") == ruling["exe_size"],
       f"{f.get('sha256')} {f.get('size')}")
    ck("P5 the executable bit is set", f.get("executable_bit"), f.get("executable_bit"))
    ck("P6 no target or executable mutation since the build: the executable's and every target entry's mtime and ctime "
       "are no later than the build's recorded end", f["bound"] is not None and f["exists"]
       and max(f.get("mtime_ns", 0), f.get("ctime_ns", 0)) / 1e9 <= f["bound"] and not f["changed_after_bound"]
       and f["target_entries_scanned"] > 0, f"{f['target_entries_scanned']} entries; later: {f['changed_after_bound'][:3]}")
    if dep_info is not None:
        p, sha, size = dep_info
        ck("P7 the executable's dep-info file is unchanged since the build (sha256 and size as recorded)",
           os.path.isfile(p) and not os.path.islink(p) and K.sha256_file(p) == sha and os.path.getsize(p) == size, p)
    if worktree_entry is not None:
        entry, reason = worktree_entry
        ck("P8 the preserved C28 worktree: registered, HEAD C, detached, locked with RESUME-01's C28 reason",
           entry is not None and f"HEAD {K.C}" in entry and "detached" in entry and reason == ruling["lock_reason"],
           f"{entry}; {reason}")
    return ck


def write(outdir, name, first, ck, extra_lines, record):
    os.makedirs(outdir)
    lines = [first] + list(extra_lines) + [f"checks: {sum(o for _, o, _ in ck.items)} of {len(ck.items)} hold"]
    lines += [f"{'OK  ' if o else 'FAIL'} {l}{(': ' + d) if d else ''}" for l, o, d in ck.items]
    open(os.path.join(outdir, name + ".txt"), "w").write("\n".join(lines) + "\n")
    json.dump(dict(record, checks=ck.items, first_line=first), open(os.path.join(outdir, name + ".json"), "w"),
              indent=1, default=str)
    print("\n".join(lines))


def admit_receipt(outdir):
    if os.path.exists(outdir):
        raise SystemExit(f"REFUSED: {outdir} exists (one admission; nothing is retried)")
    ev = evidence()
    ck = admit(ev, "C28")
    good = ck.ok()
    first = ADMITTED if good else NOT_ADMITTED
    extra = [f"control C28: the existing RESUME-01 build ({K.RESUME_PACKET}/{BUILD_DIR}/03-build at {K.RESUME}); "
             "not rebuilt; no Cargo, no rustc",
             f"ruling: {K.C28_RULING} (first line {K.C28_RULING_FIRST!r}); narrow and C28-specific; the general build "
             "classifier is unchanged and its historical C28 output `BUILD: NOT OK` stands as executor history",
             f"evidence files authenticated: {len(ev['auth'])} (from git objects, each against the RESUME-01 SHA256SUMS)"]
    write(outdir, "ADMISSION", first, ck, extra,
          {"control": "C28", "ruling": RULING, "evidence": ev["auth"], "admitted": good,
           "build_supervisor": ev["desc"]["supervisor"], "build_process": ev["desc"]["process"]})
    return 0 if good else 1


def pin_receipt(outdir, admission):
    if os.path.exists(outdir):
        raise SystemExit(f"REFUSED: {outdir} exists (one pin; nothing is retried)")
    first_adm = open(admission).readline().rstrip("\n") if os.path.exists(admission) else None
    ev = evidence()
    bound, fin = build_bound(ev)
    f = exe_facts(RULING["executable"], WT, bound)
    dep = next((a for a in ev["desc"]["deps_artifacts"] if not a["test_executable_name"]), None)
    lock = subprocess.run(["git", "-C", WT, "rev-parse", "--git-path", "locked"], capture_output=True,
                          text=True).stdout.strip() if os.path.isdir(WT) else ""
    reason = open(lock).read().strip() if lock and os.path.exists(lock) else None
    out = subprocess.run(["git", "-C", K.REPO, "worktree", "list", "--porcelain"], capture_output=True, text=True).stdout
    entry = next((b.splitlines() for b in out.strip().split("\n\n") if b.splitlines()[:1] == ["worktree " + WT]), None)
    ck = pin_checks(f, RULING, (dep["path"], dep["sha256"], dep["size"]) if dep else ("", "", -1), (entry, reason))
    ck("P0 the C28 build admission holds (prior step)", first_adm == ADMITTED, first_adm)
    good = ck.ok()
    first = PINNED if good else PIN_BLOCKED
    pin = {"path": RULING["executable"], "sha256": f.get("sha256"), "size": f.get("size"), "inode": f.get("inode"),
           "mtime_ns": f.get("mtime_ns"), "ctime_ns": f.get("ctime_ns"), "source_verdict_at_build": "EXACTLY_C_PLUS_PATCH",
           "mutant_blob": RULING["mutant_blob"], "mutant_sha256": RULING["mutant_sha256"],
           "patch_sha256": RULING["patch_sha256"],
           "build_receipt": f"{K.RESUME_PACKET}/{BUILD_DIR}/03-build at {K.RESUME}", "rebuilt": False,
           "pinned": good, "facts": f, "build_finished": fin}
    write(outdir, "PIN", first, ck,
          [f"executable: {RULING['executable']} sha256 {f.get('sha256')} size {f.get('size')} (ruling "
           f"{RULING['exe_sha256']} / {RULING['exe_size']})",
           f"read-only: hashed, sized and stat-ed; never written, moved or rebuilt; build end {fin} (bound {bound})",
           "C28_PRESERVED_ARTIFACT_BLOCKED: the executable is missing or differs; it is not rebuilt in this mission"
           if not good else "the run invokes exactly this path, re-checking sha256 and size before, at selection and "
                            "after"], pin)
    return 0 if good else 1


if __name__ == "__main__":
    mode, out = sys.argv[1], os.path.abspath(sys.argv[2])
    if mode == "admit":
        sys.exit(admit_receipt(out))
    if mode == "pin":
        sys.exit(pin_receipt(out, os.path.join(os.path.dirname(out), "01-admission/ADMISSION.txt")))
    raise SystemExit("usage: c28_admit.py admit|pin OUTDIR")
