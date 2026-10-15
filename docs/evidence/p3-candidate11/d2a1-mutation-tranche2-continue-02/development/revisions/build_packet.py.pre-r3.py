#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 19: stage the evidence packet (text only). Revised by
-RESUME-01, -CONTINUE-02, -PUBLISH-03 and -PUBLISH-04 (duplicate policy R2). Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (sections 16 and 17) for this packet. Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02 (sections 19 and 20): the previous packets are every file under
docs/evidence/p3-candidate11/ at this commit's parent (the Tranche-2 stopped evidence commit, which holds the Tranche-2
packet); C5 has no build of its own here (no 03-build records are enumerated for it), and its preserved executable is
among the pinned executables no staged file may equal. Nothing else changed.

    build_packet.py DEST        DEST: the packet directory (inside the evidence worktree, or a rehearsal directory);
                                must not exist. Copies INDEX.md, TRANCHE-SUMMARY.md and CARRIED-FORWARD.md (rendered by
                                render.py into a fresh directory), documents/, receipts/, the changed and added tools,
                                development/; writes documents/DUPLICATE-AND-ZERO-LENGTH-MAP.tsv (+ .md) and a
                                self-excluding SHA256SUMS; then checks the staged packet. Exit 1 if any check fails.

Not staged (referenced by commit, path, blob and sha256 in CARRIED-FORWARD.md): inputs/ (the catalog, patches, linked
unchanged-core receipts, contract, addenda, adoption records, proof), original-tools/, original-publication-tools/, the
tools carried byte for byte, dev-local/ (development scratch) and publication/ (receipts written after the commit).

Checks: at most 48 MiB; text only (UTF-8, no NUL byte, no ELF); regular files, no symlink; no Rust source; no file
equal to a pinned executable, to any catalog mutant blob, or to any blob of Candidate C outside docs/evidence/ (the
empty blob excepted: it copies nothing); only changed or added tools staged; every staged file byte-equal to its
source; SHA256SUMS complete and valid; every file this mission authored whitespace-clean (git's default rule), every
retained raw finding listed.

Duplicate and zero-length policy (modelled on the Architect's ruling R2 for the CONTINUE-02 packet: no size threshold,
no wildcard, exact paths): every nonempty staged file whose git blob equals that of ANY file under
docs/evidence/p3-candidate11/ at the parent commit (every previous packet) is rejected, unless its exact path is one of
the deterministic constant records enumerated below from the frozen order and self-test numbering, with exactly the
literal content the frozen generator (tools/supervise.py, unchanged) writes there, bound to its own descriptor (the
self-test's passing block of SELFTEST.txt; the control's 03-build or 04-run descriptor and receipt); or the self-test's
own GNU time record (receipts/01-supervision-selftest/<n>/time-v.txt), only when every equal previous-packet file is a
time record of the same self-test case and the record times exactly that case's command (a short fixed command's
counters can repeat by coincidence: the previous packets already hold such a pair). Every zero-length
staged file is rejected unless it is one of the enumerated zero-length artifacts (the two silent self-test commands'
merged out.txt; each control's 03-build/build.stdout, cargo writing its messages to build.stderr), bound the same way;
equality with an earlier empty file is not evidence. Every accepted file is listed in the map with its class, role,
literal, descriptor and every equal previous-packet file. Nothing else is permitted.
"""
import json
import os
import shlex
import shutil
import stat
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

SELFTEST = "receipts/01-supervision-selftest"
SAMPLER = "sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n"
EMPTY_SHA = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
EMPTY_BLOB = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
CASE_ARGV = {1: ["/usr/bin/python3", "-c", "b = b'x' * (10 << 20); print('allocated', len(b))"],
             3: ["/bin/sleep", "30"],
             5: ["/usr/bin/python3", "-c", "import sys; sys.exit(101)"],
             6: ["/usr/bin/python3", "-c", "import os; os.abort()"],
             7: ["/usr/bin/python3", "-c", "import os, sys; print(sorted(os.environ)); print('to stderr', file=sys.stderr)"]}
GENERATOR = "tools/supervise.py"


def control_dirs():
    exp = json.load(open(os.path.join(K.EV, "receipts/02-EXPECTATIONS.json")))
    return {cid: exp["controls"][cid]["dir"] for cid in K.ORDER}, exp


def allowlists():
    """The enumerated exact paths (from the frozen order and the self-test numbering): {path: spec}."""
    dirs, _ = control_dirs()
    nonempty, zero = {}, {}
    for n, lits in ((1, ["allocated 10485760\n"]), (6, ["timeout: the monitored command dumped core\n"]),
                    (7, ["['CARGO_NET_OFFLINE', 'LC_CTYPE', 'PATH']\n", "['CARGO_NET_OFFLINE', 'PATH']\n"])):
        nonempty[f"{SELFTEST}/{n}/out.txt"] = {"class": "SELFTEST_CONSTANT", "selftest": n, "control": "-",
                                                "literals": lits, "stream": "out.txt" if n != 7 else "out.txt+err.txt",
                                                "role": f"supervision self-test {n}: the command's "
                                                        + ("stdout and stderr, merged" if n != 7 else "stdout")}
    nonempty[f"{SELFTEST}/7/err.txt"] = {"class": "SELFTEST_CONSTANT", "selftest": 7, "control": "-",
                                          "literals": ["to stderr\n"], "stream": "err.txt",
                                          "role": "supervision self-test 7: the command's stderr"}
    for n in (1, 3, 5, 6, 7):
        nonempty[f"{SELFTEST}/{n}/time-v.txt"] = {
            "class": "SELFTEST_TIME_RECORD", "selftest": n, "control": "-", "literals": None,
            "role": f"supervision self-test {n}: /usr/bin/time -v's record of the case's command"}
        nonempty[f"{SELFTEST}/{n}/cgroup-samples.txt"] = {
            "class": "CGROUP_CONSTANT", "selftest": n, "control": "-", "literals": [SAMPLER],
            "role": f"supervision self-test {n}: the sampler's record of a run too short for its cgroup to be found"}
    for cid, d in dirs.items():
        for step in (("04-run",) if cid == "C5" else ("03-build", "04-run")):
            nonempty[f"{d}/{step}/cgroup-samples.txt"] = {
                "class": "CGROUP_CONSTANT", "selftest": "-", "control": cid, "step": step, "literals": [SAMPLER],
                "role": f"control {cid} {step}: the sampler's record of a run too short for its cgroup to be found"}
        if cid == "C5":
            continue
        zero[f"{d}/03-build/build.stdout"] = {"control": cid, "selftest": "-",
                                              "role": f"control {cid} 03-build: cargo's stdout (cargo writes its "
                                                      "messages to build.stderr)", "command": shlex.join(K.BUILD_ARGV)}
    for n, why in ((3, "killed at its 3 s limit before writing anything"), (5, "exits 101 without writing anything")):
        zero[f"{SELFTEST}/{n}/out.txt"] = {"control": "-", "selftest": n, "command": shlex.join(CASE_ARGV[n]),
                                           "role": f"supervision self-test {n}: stdout and stderr merged, of a command "
                                                   f"that {why}"}
    return nonempty, zero


def files_under(root):
    out = []
    for d, dirs, fs in os.walk(root):
        dirs[:] = sorted(x for x in dirs if x != "__pycache__")
        out += [os.path.join(d, f) for f in sorted(fs) if not f.endswith(".pyc")]
    return out


def whitespace(data):
    """git's default whitespace errors: blank-at-eol, space-before-tab in the indent, blank-at-eof. [(line, kind)]."""
    out = []
    lines = data.split(b"\n")
    body = lines[:-1] if data.endswith(b"\n") else lines
    for n, l in enumerate(body, 1):
        if l[-1:] in (b" ", b"\t", b"\r"):
            out.append((n, "trailing whitespace"))
        indent = l[:len(l) - len(l.lstrip(b" \t"))]
        if b" \t" in indent:
            out.append((n, "space before tab in indent"))
    blank = 0
    while blank < len(body) and not body[len(body) - 1 - blank].strip():
        blank += 1
    if data.endswith(b"\n") and blank:
        out.append((len(body) - blank + 1, "blank line at end of file"))
    return out


def authored(rel):
    """Files this mission wrote by hand or rendered (must be whitespace-clean); the rest are raw tool output, unified
    diffs or preserved development failures (byte-exact; findings listed, never edited)."""
    if rel in ("INDEX.md", "TRANCHE-SUMMARY.md", "CARRIED-FORWARD.md", "receipts/80-STOP-RECORD.md",
               "development/DEVELOPMENT-LOG.txt"):
        return True
    if rel.startswith(("documents/", "tools/", "development/revisions/")):
        return True
    return False


def prior_blobs():
    """{blob: [paths]} of every file under docs/evidence/p3-candidate11/ at the parent (every previous packet)."""
    out = {}
    for e in K.git("ls-tree", "-r", "-z", K.PARENT, "docs/evidence/p3-candidate11/").split(b"\0"):
        if e:
            meta, path = e.split(b"\t", 1)
            out.setdefault(meta.decode().split()[2], []).append(path.decode())
    return out


def selftest_block(text, n):
    ls = text.splitlines()
    start = [i for i, l in enumerate(ls) if l.startswith(f"{n}. ")]
    if len(start) != 1:
        return None, None
    block = []
    for l in ls[start[0]:]:
        if not l:
            break
        block.append(l)
    cmd = [l[len("command: "):] for l in block if l.startswith("command: ")]
    argv = shlex.split(cmd[0]) if len(cmd) == 1 else []
    i = next((j for j in range(len(argv) - 2) if argv[j:j + 3] == ["nice", "-n", "10"]), None)
    return block, (argv, argv[i + 3:] if i is not None else None)


def dup_policy(dest, allf):
    """Returns (rows, problems): one map row per accepted duplicate or zero-length file; every problem (code, path,
    detail). Fail closed: an unlisted duplicate or empty file is a problem."""
    nonempty, zero = allowlists()
    dirs, _ = control_dirs()
    by_blob = prior_blobs()
    rows, problems = [], []
    st_path = os.path.join(dest, SELFTEST, "SELFTEST.txt")
    st_text = open(st_path).read() if os.path.isfile(st_path) else ""
    st_ok = st_text.rstrip("\n").splitlines()[-1:] == ["SELF-TEST RESULT: ALL OK"]
    gen_sha = K.sha256_file(os.path.join(K.EV, GENERATOR))
    gen_orig = K.sha256_file(os.path.join(K.EV, "original-tools", "supervise.py"))

    def bad(code, p, detail=""):
        problems.append((code, p, str(detail)[:240]))

    def bound_selftest(p, n, spec, zero_file):
        block, (argv, inner) = selftest_block(st_text, n)
        if not st_ok or block is None or "  RESULT OK" not in block:
            bad("SELFTEST_NOT_PASSING", p, f"self-test {n}")
            return "-"
        if f"{K.EV}/{SELFTEST}/{n}/scope-record.txt" not in argv or inner != CASE_ARGV[n]:
            bad("SELFTEST_COMMAND", p, f"self-test {n}: {inner}")
        merged = not os.path.lexists(os.path.join(dest, SELFTEST, str(n), "err.txt"))
        if (zero_file or spec.get("stream") == "out.txt") and not merged:
            bad("SELFTEST_STREAMS", p, "stderr is not merged into out.txt")
        return f"{SELFTEST}/SELFTEST.txt"

    def bound_control(p, cid, step):
        d = dirs[cid]
        desc_rel = f"{d}/{step}/descriptor.json"
        try:
            j = json.load(open(os.path.join(dest, desc_rel)))
            ok = (j["id"] == step and j["control"] == cid and j["worktree"] == K.control_wt(cid)
                  and isinstance(j.get("supervisor"), dict))
        except (OSError, ValueError, KeyError, TypeError):
            j, ok = {}, False
        rc = os.path.join(dest, d, step, "RECEIPT.txt")
        title = ("SUPERVISED MUTANT BUILD — control " if step == "03-build" else "SUPERVISED MUTANT RUN — control ") + cid
        if not ok or not os.path.isfile(rc) or not open(rc).read().startswith(title):
            bad("DESCRIPTOR_MISMATCH", p, f"{desc_rel} / RECEIPT.txt do not describe control {cid}'s {step}")
        return desc_rel, j
    for f in allf:
        data = open(os.path.join(dest, f), "rb").read()
        if not data:
            spec = zero.get(f)
            if spec is None:
                bad("ZERO_LENGTH_UNLISTED", f)
                continue
            if spec["control"] != "-":
                desc_rel, j = bound_control(f, spec["control"], "03-build")
                se = os.path.join(dest, dirs[spec["control"]], "03-build/build.stderr")
                if not (j.get("type") == "build" and j.get("argv") == K.BUILD_ARGV and j.get("stdout_path") == "build.stdout"
                        and j.get("stdout_sha256") == EMPTY_SHA and j.get("stderr_path") == "build.stderr"
                        and os.path.isfile(se) and j.get("stderr_sha256") == K.sha256_file(se)):
                    bad("ZERO_DESCRIPTOR", f, f"{desc_rel} does not record this empty stdout and its stderr")
            else:
                desc_rel = bound_selftest(f, spec["selftest"], spec, True)
            rows.append(["ZERO_LENGTH", f, spec["control"], str(spec["selftest"]), spec["role"], "0", EMPTY_SHA, EMPTY_BLOB,
                         "", spec["command"], desc_rel, K.sha256_file(os.path.join(dest, desc_rel)) if os.path.isfile(
                             os.path.join(dest, desc_rel)) else "-", GENERATOR, gen_sha, "0", "-"])
            continue
        blob = K.git_blob_id(data)
        hits = by_blob.get(blob, [])
        if not hits:
            continue
        spec = nonempty.get(f)
        if spec is None:
            bad("UNAUTHORIZED_DUPLICATE", f, hits[:3])
            continue
        try:
            text = data.decode("utf-8")
        except UnicodeDecodeError:
            text = None
        if spec["literals"] is None:
            n = spec["selftest"]
            limit = "3" if n == 3 else "60"
            timed = f'\tCommand being timed: "timeout -s KILL {limit} nice -n 10 {" ".join(CASE_ARGV[n])}"'
            if not all(h.endswith(f"-supervision-selftest/{n}/time-v.txt") for h in hits) or timed not in (
                    text or "").splitlines():
                bad("TIME_RECORD_MISMATCH", f, f"equal previous files {hits[:3]}")
                continue
        elif text not in spec["literals"]:
            bad("LITERAL_MISMATCH", f, data[:80])
            continue
        if spec["control"] != "-":
            desc_rel, _ = bound_control(f, spec["control"], spec["step"])
        else:
            desc_rel = bound_selftest(f, spec["selftest"], spec, False)
        rows.append([spec["class"], f, spec["control"], str(spec["selftest"]), spec["role"], str(len(data)),
                     K.sha256_bytes(data), blob, text.replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n"),
                     "-", desc_rel, K.sha256_file(os.path.join(dest, desc_rel)) if os.path.isfile(
                         os.path.join(dest, desc_rel)) else "-", GENERATOR, gen_sha, str(len(hits)),
                     ";".join(f"{K.PARENT}:{h}" for h in sorted(hits))])
    if gen_sha != gen_orig:
        bad("GENERATOR_CHANGED", GENERATOR, "supervise.py is not the accepted Tranche-1 tool")
    return rows, problems


MAP_COLUMNS = ["class", "path", "control", "selftest", "role", "size", "sha256", "git_blob", "literal_utf8",
               "expected_command", "descriptor", "descriptor_sha256", "generator", "generator_sha256",
               "previous_packet_equal_files", "previous_packet_files"]


def main(dest):
    if os.path.exists(dest):
        raise SystemExit("REFUSED: destination exists")
    os.makedirs(dest)
    rendered = tempfile.mkdtemp(prefix="mt2c-render-")
    r = subprocess.run([sys.executable, "-B", os.path.join(K.EV, "tools/render.py"), rendered], capture_output=True,
                       text=True)
    print(r.stdout.strip() + (("\n" + r.stderr.strip()) if r.stderr.strip() else ""))
    pairs = [(os.path.join(rendered, n), n) for n in ("INDEX.md", "TRANCHE-SUMMARY.md", "CARRIED-FORWARD.md")]
    for sub in ("documents", "receipts", "development"):
        for f in files_under(os.path.join(K.EV, sub)):
            pairs.append((f, os.path.relpath(f, K.EV)))
    skipped = []
    for f in files_under(os.path.join(K.EV, "tools")):
        t = os.path.relpath(f, os.path.join(K.EV, "tools"))
        o = os.path.join(K.EV, "original-tools", t)
        if os.path.exists(o) and K.sha256_file(o) == K.sha256_file(f):
            skipped.append(t)
        else:
            pairs.append((f, "tools/" + t))
    print(f"tools carried byte for byte, not staged (referenced in CARRIED-FORWARD.md): {skipped}")
    for src, rel in pairs:
        os.makedirs(os.path.dirname(os.path.join(dest, rel)), exist_ok=True)
        shutil.copyfile(src, os.path.join(dest, rel))
    fails = []

    def check(label, ok, detail=""):
        print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
        if not ok:
            fails.append(label)
    allf = sorted(os.path.relpath(f, dest) for f in files_under(dest))
    rows, problems = dup_policy(dest, allf)
    os.makedirs(os.path.join(dest, "documents"), exist_ok=True)
    with open(os.path.join(dest, "documents/DUPLICATE-AND-ZERO-LENGTH-MAP.tsv"), "w") as f:
        f.write("\t".join(MAP_COLUMNS) + "\n")
        for row in rows:
            f.write("\t".join(row) + "\n")
    md = [f"# Duplicate and zero-length map — {K.MISSION}", "",
          "Every staged file whose git blob equals that of a file of a previous packet (any file under "
          f"`docs/evidence/p3-candidate11/` at `{K.PARENT}`), and every empty staged file. Each is a deterministic record "
          "the unchanged supervise.py writes at that exact path, bound to its own descriptor; the policy (tools/"
          "build_packet.py) enumerates the exact paths from the frozen order and the self-test numbering, with no size "
          "threshold and no wildcard, and rejects every other duplicate and every other empty file.", "",
          "| Class | Path | Bytes | Literal | Descriptor | Equal previous-packet files |", "|---|---|---|---|---|---|"]
    for row in rows:
        md.append(f"| {row[0]} | `{row[1]}` | {row[5]} | `{row[8] or '(empty)'}` | `{row[10]}` | {row[14]} |")
    md += ["", f"Rows: {len(rows)}. The TSV beside this file lists every equal previous-packet file by commit and path.",
           ""]
    open(os.path.join(dest, "documents/DUPLICATE-AND-ZERO-LENGTH-MAP.md"), "w").write("\n".join(md))
    allf = sorted(os.path.relpath(f, dest) for f in files_under(dest))
    with open(os.path.join(dest, "SHA256SUMS"), "w") as f:
        for rel in allf:
            f.write(f"{K.sha256_file(os.path.join(dest, rel))}  {rel}\n")
    allf = sorted(os.path.relpath(f, dest) for f in files_under(dest))
    total = sum(os.path.getsize(os.path.join(dest, f)) for f in allf)
    check("packet size at most 48 MiB", total <= K.PACKET_LIMIT, f"{total} bytes, {len(allf)} files")
    nonreg = [f for f in allf if os.path.islink(os.path.join(dest, f)) or not stat.S_ISREG(os.lstat(os.path.join(dest,
                                                                                                                  f)).st_mode)]
    check("regular files only (no symlink)", not nonreg, nonreg[:5])
    nontext = []
    for f in allf:
        b = open(os.path.join(dest, f), "rb").read()
        try:
            b.decode("utf-8")
        except UnicodeDecodeError:
            nontext.append(f)
        if b"\0" in b or b[:4] == b"\x7fELF":
            nontext.append(f)
    check("text only (UTF-8, no NUL byte, no ELF)", not nontext, nontext[:5])
    check("no Rust source file", not [f for f in allf if f.endswith(".rs")])
    pinned = {K.EXE_SHA256, K.C5_EXE_SHA256}
    for d, _, fs in os.walk(os.path.join(K.EV, "receipts")):
        for n in fs:
            if n == "PIN.json":
                pinned.add(json.load(open(os.path.join(d, n)))["sha256"])
    check("no file equal to the pinned baseline executable, the preserved C5 executable or any pinned mutant executable",
          not [f for f in allf if K.sha256_file(os.path.join(dest, f)) in pinned], f"{len(pinned)} pinned")
    cat = json.loads(open(K.inputs("catalog.json"), "rb").read())
    mutants = {c["mutant_blob"] for c in cat["controls"] if c.get("mutant_blob")}
    blobs = {f: K.git_blob_id(open(os.path.join(dest, f), "rb").read()) for f in allf}
    check("no file equal to any catalog mutant blob (no mutant source)", not [f for f, b in blobs.items() if b in mutants])
    src = set()
    for e in K.git("ls-tree", "-r", "-z", K.C).split(b"\0"):
        if e:
            meta, path = e.split(b"\t", 1)
            if not path.startswith(b"docs/evidence/"):
                src.add(meta.decode().split()[2])
    check("no source copy: no file equal to any blob of Candidate C outside docs/evidence/ (the empty blob copies nothing)",
          not [f for f, b in blobs.items() if b in src and b != EMPTY_BLOB])
    staged_tools = sorted(f[6:] for f in allf if f.startswith("tools/"))
    check("only changed or added tools staged (every carried tool referenced, not republished)",
          staged_tools and not [t for t in staged_tools if os.path.exists(os.path.join(K.EV, "original-tools", t))
                                and K.sha256_file(os.path.join(K.EV, "original-tools", t)) ==
                                K.sha256_file(os.path.join(dest, "tools", t))], staged_tools)
    check("nothing staged from inputs/, original-tools/, original-publication-tools/, dev-local/ or publication/",
          not [f for f in allf if f.startswith(("inputs/", "original-tools/", "original-publication-tools/",
                                                "dev-local/", "publication/"))])
    check("duplicate and zero-length policy: every previous-packet duplicate and every empty file is an enumerated "
          "deterministic record with its exact literal, bound to its descriptor; nothing else", not problems,
          problems[:12])
    print(f"     accepted: {len(rows)} ({sum(r[0] == 'ZERO_LENGTH' for r in rows)} zero-length, "
          f"{sum(r[0] != 'ZERO_LENGTH' for r in rows)} nonempty constants)")
    staged_src = dict((rel, s) for s, rel in pairs)
    check("every staged file byte-equal to its source", all(K.sha256_file(s) == K.sha256_file(os.path.join(dest, r))
                                                           for r, s in staged_src.items()))
    listed = [l.split("  ", 1) for l in open(os.path.join(dest, "SHA256SUMS")).read().splitlines()]
    check("SHA256SUMS lists every file but itself, and verifies",
          sorted(p for _, p in listed) == sorted(f for f in allf if f != "SHA256SUMS")
          and all(K.sha256_file(os.path.join(dest, p)) == h for h, p in listed), f"{len(listed)} listed")
    ws_auth, ws_raw = {}, {}
    for f in allf:
        w = whitespace(open(os.path.join(dest, f), "rb").read())
        if w:
            (ws_auth if authored(f) else ws_raw)[f] = w
    check("every file this mission authored is whitespace-clean (git's default rule)", not ws_auth,
          {k: v[:3] for k, v in list(ws_auth.items())[:5]})
    print(f"     retained raw whitespace findings (byte-exact tool output, unified diffs, preserved failures): "
          f"{sum(len(v) for v in ws_raw.values())} in {len(ws_raw)} files: {sorted(ws_raw)}")
    shutil.rmtree(rendered)
    print("PACKET: " + ("STAGED — ALL CHECKS HOLD" if not fails else "FAILED: " + "; ".join(fails)))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1])))
