#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 19: stage the evidence packet (text only). Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01 (section 19): the C22 ruling record and CARRIED-FORWARD.md are staged; the
carried-forward copies of the previous packet's files are NOT staged (they are referenced by previous commit, path,
blob and sha256), and the staged packet is checked to republish none of them.

    build_packet.py DEST        DEST: the packet directory (inside the evidence worktree, or a rehearsal directory);
                                must not exist. Copies INDEX.md, TRANCHE-SUMMARY.md and CARRIED-FORWARD.md (rendered by
                                render.py into a fresh directory), the C22 ruling record, receipts/ (without the
                                carried-forward copies), tools/ (no caches) and development/; writes a self-excluding
                                SHA256SUMS; then checks the staged packet.

Checks (exit 1 if any fails): at most 32 MiB; every file text (UTF-8, no NUL byte); no Rust source, no ELF; no file
equal to any of the twelve mutant blobs, to any crate source blob of C, or to the pinned executables; no carried-forward
path and no file (of 1 KiB or more) of the previous packet's carried-forward set republished; every staged file
byte-equal to its source;
SHA256SUMS lists every file but itself and verifies. Nothing is committed here.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02 (section 18): both Architect ruling records are staged; only
the tools this mission changed or added are staged (the unchanged RESUME-01 tools are referenced in CARRIED-FORWARD.md,
not republished); and no staged file of 1 KiB or more may equal ANY file of either previous packet (EXEC-01 or
RESUME-01), so nothing is duplicated.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-03 (sections 6 and 7; publication recovery only): the
duplicate-content policy, and nothing else. Every nonempty staged file byte-equal (git blob id or sha256) to any file of
either previous packet is rejected, whatever its size (no size threshold), unless it is one of the seven exact current
link receipts of the Architect's duplicate-link-receipt ruling (RULED) and every field of its DUPLICATE-CONTENT-MAP.tsv
row verifies: the exact current path (normal, regular, no symlink on its path), its sha256, git blob and size; the
exact prior commit, path, blob and sha256, byte equality, and that this prior file is the only previous-packet file with
this content; the control (directory, expectations, CONTROL-RESULT.json, LINK.txt); the linked configuration and
Stage-2 baseline receipt (from the link's own content); the control's restoration receipt and its control result and
run classification. No directory, basename, 06-link, file-type, size or determinism exception exists. A staged symlink
is rejected. Zero-length files carry no content and are not compared (as before).
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04 (sections 6, 7 and 8; publication recovery only): the
duplicate-content and zero-length policy, and nothing else (Architect ruling R2). Every nonempty staged file byte-equal
to any file of either previous packet, whatever its size, is rejected unless it is one of exactly 21 Architect-ruled
current files (NONEMPTY: the seven link receipts, ten sampler records, four self-test outputs), each verified against
every field of its NONEMPTY-DUPLICATE-MAP.tsv row: class, exact path, control or self-test, role, bytes and literal
content, the exact canonical prior (commit, path, size, sha256, blob, the number of previous-packet files with these
bytes), the frozen generator (path, blob, sha256, function and literal) and the binding descriptor; link receipts also
every PUBLISH-03 check and every field of the R1 map. Every zero-length staged file is rejected unless it is one of
exactly 8 Architect-ruled artifacts (ZERO: two self-test outputs, six mutant-build stdouts), each verified against
every field of its ZERO-LENGTH-ARTIFACT-MAP.tsv row (regular, no symlink, size 0, the empty sha256 and blob, role,
control or self-test, expected command, descriptor, stderr and result evidence, generator); equality with a previous
empty file is not evidence. Either map that is a symlink, or holds any other row count, is rejected. No threshold,
wildcard, basename, directory, 06-link, cgroup-samples, build.stdout, determinism or empty-file exception exists.
"""
import ast
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

# PUBLISH-03 (mission sections 1 and 6): the Architect's duplicate-link-receipt ruling. The seven exact current paths
# (packet-relative; the CONTINUE-02 receipt directories, not renamed), each with its control, its linked unchanged-core
# configuration and its one prior canonical file in the EXEC-01 packet (commit K.PREV). Carried unchanged into R2.
RULED = {
    "receipts/38-C9/06-link/LINK.json": ("C9", "F-B2", "receipts/04-rehearsal/links/link-C9-stage2/LINK.json"),
    "receipts/38-C9/06-link/LINK.txt": ("C9", "F-B2", "receipts/04-rehearsal/links/link-C9-stage2/LINK.txt"),
    "receipts/39-C11/06-link/LINK.json": ("C11", "F-B2", "receipts/04-rehearsal/links/link-C9-stage2/LINK.json"),
    "receipts/40-C17/06-link/LINK.json": ("C17", "F-B2", "receipts/04-rehearsal/links/link-C9-stage2/LINK.json"),
    "receipts/41-C18a/06-link/LINK.json": ("C18a", "F-G", "receipts/04-rehearsal/links/link-C18a-stage2/LINK.json"),
    "receipts/41-C18a/06-link/LINK.txt": ("C18a", "F-G", "receipts/04-rehearsal/links/link-C18a-stage2/LINK.txt"),
    "receipts/42-C18b/06-link/LINK.json": ("C18b", "F-G", "receipts/04-rehearsal/links/link-C18a-stage2/LINK.json"),
}
# The accepted Stage-2 unchanged-core receipt each linked configuration names (K.STAGE2).
STAGE2_RECEIPT = {"F-B2": K.STAGE2_PACKET + "/receipts/61-full-F-B2", "F-G": K.STAGE2_PACKET + "/receipts/60-full-F-G"}

# PUBLISH-04 (mission sections 1, 6, 7 and 8): the Architect's duplicate-policy ruling R2. Exactly 21 nonempty
# duplicates (the seven link receipts above and fourteen deterministic constant records) and exactly 8 zero-length
# artifacts, each an exact current path with its exact class, identity, role, canonical prior (nonempty) or expected
# command (zero-length), generator and binding descriptor. No threshold, wildcard, basename, directory or generic
# exception exists; every other nonempty duplicate and every other zero-length file is rejected.
P4_ROOT = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-mutation-tranche1-publish-04"
MAP = P4_ROOT + "/documents/NONEMPTY-DUPLICATE-MAP.tsv"
ZERO_MAP = P4_ROOT + "/documents/ZERO-LENGTH-ARTIFACT-MAP.tsv"
R1_MAP = ("/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-mutation-tranche1-publish-03/documents/"
          "DUPLICATE-CONTENT-MAP.tsv")
R1_MAP_SHA = "8842a5e5a5e082ae0676adef5b83ac3743154bef7ace254f15cf8f15df579dd5"
R1_COLUMNS = ["current_path", "control", "current_sha256", "current_git_blob", "size", "prior_commit", "prior_path",
              "prior_git_blob", "prior_sha256", "byte_equal", "linked_configuration", "linked_baseline_commit",
              "linked_baseline_path", "restoration_receipt", "restoration_sha256", "control_result",
              "control_result_sha256", "classification", "classification_sha256"]
LINK_COLUMNS = R1_COLUMNS[10:]
MAP_COLUMNS = ["map_class", "current_path", "control_id", "selftest_id", "current_role", "size", "current_sha256",
               "current_git_blob", "literal_utf8", "prior_commit", "prior_path", "prior_role", "prior_size",
               "prior_sha256", "prior_git_blob", "prior_equal_files", "byte_equal", "generator_path",
               "generator_git_blob", "generator_sha256", "generator_item", "descriptor_path", "descriptor_sha256"
               ] + LINK_COLUMNS
ZERO_COLUMNS = ["current_path", "role", "control_id", "selftest_id", "expected_command", "file_type", "symlink", "size",
                "sha256", "git_blob", "descriptor_path", "descriptor_sha256", "stderr_evidence",
                "stderr_evidence_sha256", "result_evidence", "result_evidence_sha256", "generator_path",
                "generator_git_blob", "generator_sha256", "generator_item"]
EMPTY_SHA = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
EMPTY_BLOB = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
LITERAL_MAX = 256
MERGED = "MERGED_INTO_OUT_TXT"
SELFTEST = "receipts/01-supervision-selftest"
RUN_DIRS = {"C28": "receipts/36-C28", "C12": "receipts/37-C12", "C9": "receipts/38-C9", "C11": "receipts/39-C11",
            "C17": "receipts/40-C17", "C18a": "receipts/41-C18a", "C18b": "receipts/42-C18b"}
BUILT = ["C12", "C9", "C11", "C17", "C18a", "C18b"]
SUPERVISE, CONTROL = "tools/supervise.py", "tools/control.py"
SAMPLER = "supervise.sampler() (its cgroup-not-found record), started by supervise.supervised()"
# The self-test cases whose output files are mapped: the inner command after `nice -n 10` (SELFTEST.txt) and the
# literal of supervise.selftest() it comes from.
CASE_ARGV = {1: ["/usr/bin/python3", "-c", "b = b'x' * (10 << 20); print('allocated', len(b))"],
             3: ["/bin/sleep", "30"],
             5: ["/usr/bin/python3", "-c", "import sys; sys.exit(101)"],
             6: ["/usr/bin/python3", "-c", "import os; os.abort()"],
             7: ["/usr/bin/python3", "-c", "import os, sys; print(sorted(os.environ)); print('to stderr', file=sys.stderr)"]}
STREAMS = {"out.txt": "stdout and stderr merged (out.txt)", "out.txt+err.txt": "stdout (out.txt; stderr separate)",
           "err.txt": "stderr (err.txt; stdout separate)"}


def _nonempty():
    t = {}
    for p, (cid, cfg, prior_rel) in RULED.items():
        name = p.rsplit("/", 1)[1]
        reh = {"F-B2": "C9", "F-G": "C18a"}[cfg]
        t[p] = {"class": "LINK_RECEIPT", "control": cid, "selftest": "-",
                "role": f"control {cid} 06-link/{name}: the link of its restored side to the accepted {cfg} "
                        f"unchanged-core receipt",
                "prior": (K.PREV, prior_rel), "prior_role": f"EXEC-01 pre-freeze link rehearsal of {reh} ({cfg}), {name}",
                "generator": CONTROL, "item": "control.link()", "function": "link", "literal": name,
                "descriptor": f"{RUN_DIRS[cid]}/CONTROL-RESULT.json"}
    for n, commit in ((1, K.RESUME), (5, K.RESUME), (7, K.PREV)):
        p = f"{SELFTEST}/{n}/cgroup-samples.txt"
        t[p] = {"class": "CGROUP_CONSTANT", "control": "-", "selftest": str(n),
                "role": f"supervision self-test {n}: sampler record (cgroup-samples.txt)",
                "prior": (commit, p), "prior_role": f"{'EXEC-01' if commit == K.PREV else 'RESUME-01'} supervision "
                                                    f"self-test {n}: sampler record (cgroup-samples.txt)",
                "generator": SUPERVISE, "item": f"{SAMPLER}, from supervise.selftest() case {n}",
                "function": "sampler", "literal": None, "descriptor": f"{SELFTEST}/SELFTEST.txt"}
    for cid, d in RUN_DIRS.items():
        p = f"{d}/04-run/cgroup-samples.txt"
        t[p] = {"class": "CGROUP_CONSTANT", "control": cid, "selftest": "-",
                "role": f"control {cid} supervised mutant run (04-run): sampler record (cgroup-samples.txt)",
                "prior": (K.RESUME, "receipts/32-C14/04-run/cgroup-samples.txt"),
                "prior_role": "RESUME-01 control C14 supervised mutant run (04-run): sampler record (cgroup-samples.txt; "
                              "C14 Architect-accepted)",
                "generator": SUPERVISE, "item": f"{SAMPLER}, from supervise.invoke() for control {cid} (control.py)",
                "function": "sampler", "literal": None, "descriptor": f"{d}/04-run/descriptor.json"}
    for n, name, stream in ((1, "out.txt", "out.txt"), (6, "out.txt", "out.txt"), (7, "out.txt", "out.txt+err.txt"),
                            (7, "err.txt", "err.txt")):
        p = f"{SELFTEST}/{n}/{name}"
        t[p] = {"class": "SELFTEST_CONSTANT", "control": "-", "selftest": str(n),
                "role": f"supervision self-test {n}: {STREAMS[stream]}", "stream": stream,
                "prior": (K.PREV, p), "prior_role": f"EXEC-01 supervision self-test {n}: {STREAMS[stream]}",
                "generator": SUPERVISE, "item": f"supervise.selftest() case {n}, through supervise.supervised()",
                "function": "selftest", "literal": CASE_ARGV[n][-1], "descriptor": f"{SELFTEST}/SELFTEST.txt"}
    return t


def _zero():
    t = {}
    for n, why in ((3, "a command killed at its 3 s time limit before writing anything"),
                   (5, "a command that exits 101 without writing anything")):
        t[f"{SELFTEST}/{n}/out.txt"] = {
            "role": f"supervision self-test {n}: {STREAMS['out.txt']} of {why}", "control": "-", "selftest": str(n),
            "command": shlex.join(CASE_ARGV[n]), "descriptor": f"{SELFTEST}/SELFTEST.txt", "stderr": MERGED,
            "result": f"{SELFTEST}/{n}/time-v.txt", "generator": SUPERVISE,
            "item": f"supervise.selftest() case {n}, through supervise.supervised() (no stderr_path: stderr merged "
                    f"into out.txt)", "function": "selftest", "literal": CASE_ARGV[n][-1] if n == 5 else "/bin/sleep"}
    for cid in BUILT:
        d = RUN_DIRS[cid]
        t[f"{d}/03-build/build.stdout"] = {
            "role": f"control {cid} supervised mutant build (03-build): cargo's stdout (build.stdout; cargo writes its "
                    f"diagnostics to build.stderr)", "control": cid, "selftest": "-",
            "command": shlex.join(K.BUILD_ARGV), "descriptor": f"{d}/03-build/descriptor.json",
            "stderr": f"{d}/03-build/build.stderr", "result": f"{d}/03-build/CLASSIFICATION.txt",
            "generator": SUPERVISE, "item": "supervise.build(), through supervise.supervised(stdout_path=build.stdout, "
                                            "stderr_path=build.stderr)", "function": "build", "literal": "build.stdout"}
    return t


NONEMPTY = _nonempty()
ZERO = _zero()


SOURCES = [(K.RULING, "ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md"),
           (K.C28_RULING, "ARCHITECT-RULING-C28-BUILD-ADMISSIBILITY.md"),
           ("receipts", "receipts"), ("tools", "tools"), ("development", "development")]


def unchanged_tool(rel):
    """Is EV-relative path rel a tool this mission runs byte-identical to RESUME-01's (referenced, not republished)?"""
    if not rel.startswith("tools/") or rel.count("/") != 1:
        return False
    o = os.path.join(K.EV, "original-tools", rel[6:])
    return os.path.exists(o) and K.sha256_file(o) == K.sha256_file(os.path.join(K.EV, rel))


def carried(rel):
    """Is EV-relative path rel a carried-forward copy (never republished)?"""
    return any(rel == c or rel.startswith(c + "/") for c in K.CARRIED)


def files_under(root):
    out = []
    for d, dirs, fs in os.walk(root):
        dirs[:] = sorted(x for x in dirs if x != "__pycache__")
        out += [os.path.join(d, f) for f in sorted(fs) if not f.endswith(".pyc")]
    return out


def dup_policy(dest, allf, map_path=MAP, zero_map_path=ZERO_MAP):
    """The duplicate-content and zero-length policy (PUBLISH-04, Architect ruling R2) over the staged packet dest
    (allf: its packet-relative files). Returns (accepted, accepted_zero, problems): the mapped nonempty duplicates
    accepted, the mapped zero-length artifacts accepted, and every problem as (code, path, detail). Rejected by default:
    every nonempty staged file whose git blob id or sha256 equals any file of either previous packet, whatever its size,
    and every zero-length staged file. Accepted: exactly the 21 NONEMPTY and the 8 ZERO paths, each only when every
    field of its map row verifies against the staged bytes, the immutable previous commits and the control's or
    self-test's own receipts."""
    problems, verified, zverified = [], set(), set()

    def bad(code, path, detail=""):
        problems.append((code, path, str(detail)[:300]))

    real = os.path.realpath(dest)

    def unsafe(rel):
        """None if rel names a regular file under dest by a normal relative path with no symlink on it; else why."""
        parts = rel.split("/")
        if not rel or os.path.isabs(rel) or "\\" in rel or os.path.normpath(rel) != rel or ".." in parts or "." in parts:
            return "PATH_UNSAFE"
        cur = dest
        for part in parts:
            cur = os.path.join(cur, part)
            if os.path.islink(cur):
                return "SYMLINK"
            if not os.path.lexists(cur):
                return "ABSENT"
        if not stat.S_ISREG(os.lstat(cur).st_mode):
            return "NOT_REGULAR"
        return None if os.path.realpath(cur) == os.path.join(real, rel) else "PATH_UNSAFE"

    def read(rel):
        with open(os.path.join(dest, rel), "rb") as f:
            return f.read()

    def literal(data):
        """The UTF-8 text of a record, escaped for one TSV field (backslash, tab, carriage return, newline)."""
        return data.decode("utf-8").replace("\\", "\\\\").replace("\t", "\\t").replace("\r", "\\r").replace("\n", "\\n")

    # Every file of both previous packets (immutable commits), by git blob id (ls-tree) and by sha256 (their SHA256SUMS).
    by_blob, by_sha = {}, {}
    for commit, packet in ((K.PREV, K.PREV_PACKET), (K.RESUME, K.RESUME_PACKET)):
        for e in K.git("ls-tree", "-r", "-z", commit, packet + "/").split(b"\0"):
            if e:
                meta, path = e.split(b"\t", 1)
                by_blob.setdefault(meta.decode().split()[2], set()).add((commit, path.decode()))
        for line in K.git("show", f"{commit}:{packet}/SHA256SUMS").decode().splitlines():
            h, rel = line.split("  ", 1)
            by_sha.setdefault(h, set()).add((commit, f"{packet}/{rel}"))
    dups, empties = {}, []
    for f in allf:
        if unsafe(f):
            bad("STAGED_" + unsafe(f), f)
            continue
        data = read(f)
        if not data:
            empties.append(f)
            continue
        hits = by_blob.get(K.git_blob_id(data), set()) | by_sha.get(K.sha256_bytes(data), set())
        if hits:
            dups[f] = hits

    def load(path, columns, tag, table):
        """A map: a regular file (no symlink), exactly the header and one row per ruled path, nothing else."""
        label = {"MAP": "NONEMPTY-DUPLICATE-MAP.tsv", "ZMAP": "ZERO-LENGTH-ARTIFACT-MAP.tsv",
                 "R1MAP": "DUPLICATE-CONTENT-MAP.tsv (R1)"}[tag]
        rows = {}
        if os.path.islink(path) or not os.path.isfile(path):
            bad(tag + ("_SYMLINK" if os.path.islink(path) else "_UNREADABLE"), label, path)
            return rows
        try:
            lines = open(path, encoding="utf-8").read().split("\n")
        except (OSError, UnicodeDecodeError) as e:
            bad(tag + "_UNREADABLE", label, e)
            return rows
        if lines and lines[-1] == "":
            lines.pop()
        if not lines or lines[0].split("\t") != columns:
            bad(tag + "_HEADER", label, lines[0][:120] if lines else "")
            return rows
        key = columns.index("current_path")
        for n, line in enumerate(lines[1:], 2):
            v = line.split("\t")
            if len(v) != len(columns):
                bad(tag + "_ROW_SHAPE", f"{label} line {n}", line[:120])
            elif v[key] in rows:
                bad(tag + "_ROW_REPEATED", v[key])
            else:
                rows[v[key]] = dict(zip(columns, v))
        if len(lines) - 1 != len(table):
            bad(tag + "_ROW_COUNT", label, f"{len(lines) - 1} rows; ruled exactly {len(table)}")
        for p in sorted(set(rows) - set(table)):
            bad(tag + "_ROW_NOT_RULED", p)
        for p in sorted(set(table) - set(rows)):
            bad(tag + "_ROW_MISSING", p)
        return rows

    rows = load(map_path, MAP_COLUMNS, "MAP", NONEMPTY)
    zrows = load(zero_map_path, ZERO_COLUMNS, "ZMAP", ZERO)
    r1 = load(R1_MAP, R1_COLUMNS, "R1MAP", RULED)
    if r1 and K.sha256_file(R1_MAP) != R1_MAP_SHA:
        bad("R1MAP_MISMATCH", "DUPLICATE-CONTENT-MAP.tsv (R1)", K.sha256_file(R1_MAP))
    exp = json.load(open(os.path.join(K.EV, "receipts/02-EXPECTATIONS.json")))
    frozen = json.load(open(os.path.join(K.EV, "receipts/05-FREEZE.json")))["files"]
    st_rel = f"{SELFTEST}/SELFTEST.txt"
    st_text = read(st_rel).decode("utf-8", "replace") if not unsafe(st_rel) else None
    st_all_ok = st_text is not None and st_text.rstrip("\n").splitlines()[-1:] == ["SELF-TEST RESULT: ALL OK"]
    trees = {}

    def gen_check(p, r, spec, lit):
        """The generator: the frozen CONTINUE-02 tool (its freeze record), published (staged here or in the RESUME-01
        packet), and the named function holding the literal that ties this file to it."""
        rel = spec["generator"]
        src = os.path.join(K.EV, rel)
        if r["generator_path"] != rel or r["generator_item"] != spec["item"]:
            bad("GENERATOR_MISMATCH", p, f"map {r['generator_path']} {r['generator_item']}")
        if os.path.islink(src) or not os.path.isfile(src):
            bad("GENERATOR_ABSENT", p, rel)
            return
        data = open(src, "rb").read()
        if (K.sha256_bytes(data), K.git_blob_id(data)) != (r["generator_sha256"], r["generator_git_blob"]):
            bad("GENERATOR_MISMATCH", p, f"{K.sha256_bytes(data)} {K.git_blob_id(data)}")
        if frozen.get(rel) != K.sha256_bytes(data):
            bad("GENERATOR_NOT_FROZEN", p, rel)
        published = K.git("rev-parse", "-q", "--verify", f"{K.RESUME}:{K.RESUME_PACKET}/{rel}", check=False)
        here = K.git_blob_id(read(rel)) if not unsafe(rel) else None
        if K.git_blob_id(data) not in (here, published.decode().strip()):
            bad("GENERATOR_NOT_PUBLISHED", p, rel)
        if rel not in trees:
            trees[rel] = ast.parse(data.decode("utf-8"))
        fn = [n for n in trees[rel].body if isinstance(n, ast.FunctionDef) and n.name == spec["function"]]
        consts = {n.value for n in ast.walk(fn[0]) if isinstance(n, ast.Constant) and isinstance(n.value, str)} if fn else ()
        if lit not in consts:
            bad("GENERATOR_ITEM_ABSENT", p, f"{spec['function']}() holding {lit!r}")

    def evidence(p, rel, want, code):
        """A binding receipt: present (regular, no symlink), nonempty, and its sha256 the map's."""
        if unsafe(rel):
            bad(code + "_ABSENT", p, f"{rel}: {unsafe(rel)}")
            return None
        data = read(rel)
        if not data or K.sha256_bytes(data) != want:
            bad(code + "_MISMATCH", p, f"{rel} sha256 {K.sha256_bytes(data)}")
        return data

    def selftest_block(n):
        """Self-test n's passing block of SELFTEST.txt, its command line, and the inner command after `nice -n 10`."""
        if st_text is None:
            return None, [], None
        ls = st_text.splitlines()
        start = [i for i, l in enumerate(ls) if l.startswith(f"{n}. ")]
        if len(start) != 1:
            return None, [], None
        block = []
        for l in ls[start[0]:]:
            if not l:
                break
            block.append(l)
        cmd = [l[len("command: "):] for l in block if l.startswith("command: ")]
        argv = shlex.split(cmd[0]) if len(cmd) == 1 else []
        i = next((j for j in range(len(argv) - 2) if argv[j:j + 3] == ["nice", "-n", "10"]), None)
        return block, argv, argv[i + 3:] if i is not None else None

    def selftest_bound(p, n, code):
        """The self-test receipt binds this file: self-test n passed in this mission's root, recording into its own
        directory, with its scope record and time record present. Returns (block, inner command)."""
        block, argv, inner = selftest_block(n)
        if block is None or "  RESULT OK" not in block or not st_all_ok:
            bad(code, p, f"SELFTEST.txt has no passing self-test {n}")
        if f"{K.EV}/{SELFTEST}/{n}/scope-record.txt" not in argv:
            bad(code, p, f"self-test {n}'s command does not record into {SELFTEST}/{n}/ of {K.EV}")
        for sib in ("scope-record.txt", "time-v.txt"):
            if unsafe(f"{SELFTEST}/{n}/{sib}"):
                bad("SUPERVISOR_RECEIPT_ABSENT" if code == "DESCRIPTOR_MISMATCH" else "ZERO_RESULT_ABSENT", p,
                    f"{SELFTEST}/{n}/{sib}")
        return block or [], inner

    def link_checks(p, r, cur):
        """PUBLISH-03's link-receipt checks (unchanged in substance), and every R1 map field again."""
        cid, cfg, prior_rel = RULED[p]
        d = exp["controls"][cid]["dir"]
        if p.split("/06-link/")[0] != d or d.split("/")[1].split("-", 1)[1] != cid:
            bad("CONTROL_IDENTITY", p, f"expected directory {d}")
        r1r = r1.get(p)
        diff = [c for c in R1_COLUMNS if r1r is None or r1r[c] != r["control_id" if c == "control" else c]]
        if diff:
            bad("R1_FIELD_MISMATCH", p, diff)
        reference = f"{STAGE2_RECEIPT[cfg]} at {K.STAGE2} (accepted exact-final {cfg} unchanged-core PASS, type (e1))"
        if r["linked_configuration"] != cfg or K.EXPLORE_CFG.get(cid) != cfg or exp["controls"][cid].get(
                "configuration") != cfg:
            bad("CONFIG_MISMATCH", p, f"map {r['linked_configuration']}; ruled {cfg}")
        if (r["linked_baseline_commit"], r["linked_baseline_path"]) != (K.STAGE2, STAGE2_RECEIPT[cfg]):
            bad("BASELINE_MISMATCH", p, f"map {r['linked_baseline_commit']} {r['linked_baseline_path']}")
        if not K.git("ls-tree", K.STAGE2, STAGE2_RECEIPT[cfg] + "/CLASSIFICATION.txt").strip():
            bad("BASELINE_ABSENT", p, STAGE2_RECEIPT[cfg])
        cfg_label = f"exact configuration: the same test and flags as this control's run ({cfg})"
        try:
            if p.endswith(".json"):
                j = json.loads(cur)
                ref, labels = j["reference"], [c[0] for c in j["checks"]]
                linked = all(c[1] is True for c in j["checks"]) and len(j["checks"]) == 7
            else:
                t = cur.decode("utf-8").splitlines()
                ref = t[1].split(": restored-side receipt ", 1)[1] if t[1].startswith(f"control {cid}: ") else None
                labels = [l[5:] for l in t if l.startswith("OK   ")]
                linked = t[0] == "LINK: VERIFIED" and not [l for l in t if l.startswith("FAIL")]
        except (ValueError, KeyError, IndexError, TypeError) as e:
            ref, labels, linked = None, [], False
            bad("LINK_UNPARSED", p, e)
        if ref is None and p.endswith(".txt"):
            bad("CONTROL_IDENTITY", p, f"the link text does not name control {cid}")
        if ref != reference:
            bad("BASELINE_MISMATCH", p, f"link reference {ref}")
        if not [x for x in labels if x == cfg_label or x.startswith(cfg_label + ": ")]:
            bad("CONFIG_MISMATCH", p, f"the link's configuration check is not {cfg}")
        if not linked:
            bad("LINK_NOT_VERIFIED", p)
        sib = f"{d}/06-link/LINK.txt"
        st = read(sib).decode("utf-8", "replace").splitlines() if not unsafe(sib) else []
        if st[:2] != ["LINK: VERIFIED", f"control {cid}: restored-side receipt {reference}"]:
            bad("CONTROL_IDENTITY", p, f"{sib} does not name control {cid} and this reference")
        # The control's own restoration, result and classification receipts.
        for col, sha, rel, code in (
                ("restoration_receipt", "restoration_sha256", f"{d}/05-restore/RESTORE.txt", "RESTORATION"),
                ("control_result", "control_result_sha256", f"{d}/CONTROL-RESULT.json", "RESULT"),
                ("classification", "classification_sha256", f"{d}/04-run/CLASSIFICATION.txt", "RESULT")):
            if r[col] != rel:
                bad(code + "_PATH_MISMATCH", p, f"{col} {r[col]}")
            if unsafe(rel):
                bad(code + "_ABSENT", p, f"{rel}: {unsafe(rel)}")
            elif K.sha256_bytes(read(rel)) != r[sha]:
                bad(code + "_MISMATCH", p, f"{rel} sha256 {K.sha256_bytes(read(rel))}")
        for rel, code in ((f"{d}/05-restore/RESTORE.json", "RESTORATION"), (f"{d}/RESULT.txt", "RESULT")):
            if unsafe(rel):
                bad(code + "_ABSENT", p, f"{rel}: {unsafe(rel)}")
        rs = f"{d}/05-restore/RESTORE.txt"
        if not unsafe(rs) and read(rs).decode("utf-8", "replace").splitlines()[:2][0:1] != ["RESTORE: EXACTLY_C"]:
            bad("RESTORATION_MISMATCH", p, f"{rs} is not RESTORE: EXACTLY_C")
        if not unsafe(rs) and not read(rs).decode("utf-8", "replace").splitlines()[1].startswith(f"control {cid}: "):
            bad("RESTORATION_MISMATCH", p, f"{rs} does not name control {cid}")
        cr = f"{d}/CONTROL-RESULT.json"
        if not unsafe(cr):
            try:
                j = json.loads(read(cr))
                ok = j["control"] == cid and f"linked restored-side receipt: LINK: VERIFIED; {reference}" in j["facts"]
            except (ValueError, KeyError, TypeError):
                ok = False
            if not ok:
                bad("RESULT_MISMATCH", p, f"{cr} does not name control {cid} and this link")
        cl = f"{d}/04-run/CLASSIFICATION.txt"
        if not unsafe(cl) and not read(cl).decode("utf-8", "replace").startswith("CLASSIFICATION: "):
            bad("RESULT_MISMATCH", p, f"{cl} is not a classification")

    for p in sorted(set(rows) & set(NONEMPTY)):
        r, spec = rows[p], NONEMPTY[p]
        before = len(problems)
        cls, cid, n = spec["class"], spec["control"], spec["selftest"]
        if r["map_class"] != cls:
            bad("CLASS_MISMATCH", p, f"map {r['map_class']}; ruled {cls}")
        if r["control_id"] != cid or (cid != "-" and (exp["controls"][cid]["dir"] != RUN_DIRS[cid]
                                                      or not p.startswith(RUN_DIRS[cid] + "/"))):
            bad("CONTROL_IDENTITY", p, f"map control {r['control_id']}; ruled {cid}")
        if r["selftest_id"] != n or (n != "-" and not p.startswith(f"{SELFTEST}/{n}/")):
            bad("SELFTEST_IDENTITY", p, f"map self-test {r['selftest_id']}; ruled {n}")
        if (r["current_role"], r["prior_role"]) != (spec["role"], spec["prior_role"]):
            bad("ROLE_MISMATCH", p, f"map {r['current_role']} / {r['prior_role']}")
        if cls != "LINK_RECEIPT" and [c for c in LINK_COLUMNS if r[c] != "-"]:
            bad("MAP_FIELD_NOT_APPLICABLE", p, [c for c in LINK_COLUMNS if r[c] != "-"])
        why = unsafe(p)
        if why:
            bad("CURRENT_" + why, p)
            continue
        cur = read(p)
        if (K.sha256_bytes(cur), K.git_blob_id(cur), str(len(cur))) != (r["current_sha256"], r["current_git_blob"],
                                                                          r["size"]):
            bad("CURRENT_BYTES_CHANGED", p, f"{K.sha256_bytes(cur)} {K.git_blob_id(cur)} {len(cur)}")
        try:
            lit = literal(cur) if len(cur) <= LITERAL_MAX else "-"
        except UnicodeDecodeError:
            lit = None
        if r["literal_utf8"] != lit:
            bad("LITERAL_MISMATCH", p, f"map {r['literal_utf8'][:80]!r}")
        # The canonical prior: exactly the ruled immutable file (never inferred from equal bytes alone).
        pc, prel = spec["prior"]
        prior_path = f"{K.PREV_PACKET if pc == K.PREV else K.RESUME_PACKET}/{prel}"
        if r["prior_commit"] != pc:
            bad("PRIOR_COMMIT_MISMATCH", p, r["prior_commit"])
        if r["prior_path"] != prior_path:
            bad("PRIOR_PATH_MISMATCH", p, r["prior_path"])
        pb = K.git("rev-parse", f"{pc}:{prior_path}").decode().strip()
        prior = K.git("cat-file", "blob", pb)
        if r["prior_git_blob"] != pb:
            bad("PRIOR_BLOB_MISMATCH", p, r["prior_git_blob"])
        if r["prior_sha256"] != K.sha256_bytes(prior):
            bad("PRIOR_SHA256_MISMATCH", p, r["prior_sha256"])
        if r["prior_size"] != str(len(prior)):
            bad("PRIOR_SIZE_MISMATCH", p, r["prior_size"])
        if prior != cur or r["byte_equal"] != "yes":
            bad("NOT_BYTE_EQUAL", p, f"map byte_equal {r['byte_equal']}")
        hits = dups.get(p, set())
        if (pc, prior_path) not in hits or r["prior_equal_files"] != str(len(hits)):
            bad("PRIOR_SOURCE_MISMATCH", p, f"{len(hits)} previous-packet files equal; map {r['prior_equal_files']}")
        try:
            content = cur.decode("utf-8")
        except UnicodeDecodeError:
            content = None
        gen_check(p, r, spec, spec["literal"] if spec["literal"] is not None else content)
        desc = spec["descriptor"]
        if r["descriptor_path"] != desc:
            bad("DESCRIPTOR_MISMATCH", p, f"map {r['descriptor_path']}")
        evidence(p, desc, r["descriptor_sha256"], "DESCRIPTOR")
        if cls == "LINK_RECEIPT":
            link_checks(p, r, cur)
        elif cid != "-":
            d = RUN_DIRS[cid]
            if p != f"{d}/04-run/cgroup-samples.txt":
                bad("CONTROL_IDENTITY", p, f"not control {cid}'s run sampler record")
            if not unsafe(desc):
                try:
                    j = json.loads(read(desc))
                    ok = (j["id"] == "04-run" and j["control"] == cid and isinstance(j["supervisor"], dict)
                          and j["worktree"] == f"{K.NEXUS}/nexus-os-p3-c11-mt1x-{cid}")
                except (ValueError, KeyError, TypeError):
                    ok = False
                if not ok:
                    bad("DESCRIPTOR_MISMATCH", p, f"{desc} does not describe control {cid}'s supervised run")
            for sib in ("scope-record.txt", "time-v.txt", "RECEIPT.txt"):
                if unsafe(f"{d}/04-run/{sib}"):
                    bad("SUPERVISOR_RECEIPT_ABSENT", p, f"{d}/04-run/{sib}")
            rc = f"{d}/04-run/RECEIPT.txt"
            if not unsafe(rc) and not read(rc).decode("utf-8", "replace").startswith(
                    f"SUPERVISED MUTANT RUN — control {cid}: "):
                bad("DESCRIPTOR_MISMATCH", p, f"{rc} does not name control {cid}'s run")
        else:
            block, inner = selftest_bound(p, int(n), "DESCRIPTOR_MISMATCH")
            if cls == "SELFTEST_CONSTANT":
                if inner != CASE_ARGV[int(n)]:
                    bad("DESCRIPTOR_MISMATCH", p, f"self-test {n}'s inner command is {inner}")
                out, err = f"{SELFTEST}/{n}/out.txt", f"{SELFTEST}/{n}/err.txt"
                if p != (err if spec["stream"] == "err.txt" else out):
                    bad("ROLE_MISMATCH", p, f"not the {spec['stream']} record of self-test {n}")
                if spec["stream"] == "out.txt" and os.path.lexists(os.path.join(dest, err)):
                    bad("ROLE_MISMATCH", p, f"{err} exists: stderr is not merged into out.txt")
                if spec["stream"] != "out.txt":
                    if unsafe(out) or unsafe(err):
                        bad("ROLE_MISMATCH", p, "stdout and stderr are not both recorded separately")
                    elif (f"  stdout {read(out).decode('utf-8', 'replace').strip()!r}; stderr "
                          f"{read(err).decode('utf-8', 'replace').strip()!r}; ") not in "\n".join(block):
                        bad("DESCRIPTOR_MISMATCH", p, f"SELFTEST.txt does not record self-test {n}'s two streams")
            elif p != f"{SELFTEST}/{n}/cgroup-samples.txt":
                bad("SELFTEST_IDENTITY", p, f"not self-test {n}'s sampler record")
        if len(problems) == before:
            verified.add(p)

    for p in sorted(set(zrows) & set(ZERO)):
        r, spec = zrows[p], ZERO[p]
        before = len(problems)
        cid, n = spec["control"], spec["selftest"]
        if r["role"] != spec["role"]:
            bad("ZERO_ROLE_MISMATCH", p, f"map {r['role']}")
        if r["control_id"] != cid or (cid != "-" and (exp["controls"][cid]["dir"] != RUN_DIRS[cid]
                                                      or p != f"{RUN_DIRS[cid]}/03-build/build.stdout")):
            bad("ZERO_CONTROL_IDENTITY", p, f"map control {r['control_id']}; ruled {cid}")
        if r["selftest_id"] != n or (n != "-" and p != f"{SELFTEST}/{n}/out.txt"):
            bad("ZERO_SELFTEST_IDENTITY", p, f"map self-test {r['selftest_id']}; ruled {n}")
        if [r[c] for c in ("file_type", "symlink", "size", "sha256", "git_blob")] != ["regular", "no", "0", EMPTY_SHA,
                                                                                       EMPTY_BLOB]:
            bad("ZERO_MAP_FIELD", p, [r[c] for c in ("file_type", "symlink", "size", "sha256", "git_blob")])
        if r["expected_command"] != spec["command"]:
            bad("ZERO_COMMAND_MISMATCH", p, f"map {r['expected_command']}")
        if (r["descriptor_path"], r["stderr_evidence"], r["result_evidence"]) != (spec["descriptor"], spec["stderr"],
                                                                                 spec["result"]):
            bad("ZERO_EVIDENCE_PATH_MISMATCH", p, f"map {r['descriptor_path']} {r['stderr_evidence']} "
                                                   f"{r['result_evidence']}")
        why = unsafe(p)
        if why:
            bad("ZERO_CURRENT_" + why, p)
            continue
        data = read(p)
        if data or (K.sha256_bytes(data), K.git_blob_id(data)) != (EMPTY_SHA, EMPTY_BLOB):
            bad("ZERO_NOT_EMPTY", p, f"{len(data)} bytes")
        gen_check(p, r, spec, spec["literal"])
        dsc = evidence(p, spec["descriptor"], r["descriptor_sha256"], "ZERO_DESCRIPTOR")
        evidence(p, spec["result"], r["result_evidence_sha256"], "ZERO_RESULT")
        if cid != "-":
            se = evidence(p, spec["stderr"], r["stderr_evidence_sha256"], "ZERO_STDERR")
            try:
                j = json.loads(dsc)
                ok = (j["id"] == "03-build" and j["type"] == "build" and j["control"] == cid
                      and j["worktree"] == f"{K.NEXUS}/nexus-os-p3-c11-mt1x-{cid}"
                      and (j["stdout_path"], j["stdout_sha256"], j["stderr_path"]) == ("build.stdout", EMPTY_SHA,
                                                                                      "build.stderr")
                      and j["process"]["exit_status"] == 0 and se is not None
                      and j["stderr_sha256"] == K.sha256_bytes(se))
                cmd = j["argv"]
            except (ValueError, KeyError, TypeError):
                ok, cmd = False, None
            if not ok:
                bad("ZERO_DESCRIPTOR_MISMATCH", p, f"{spec['descriptor']} does not describe control {cid}'s build into "
                                                   f"this build.stdout and its build.stderr")
            if cmd != K.BUILD_ARGV or shlex.join(cmd) != spec["command"]:
                bad("ZERO_COMMAND_MISMATCH", p, f"descriptor argv {cmd}")
            res = spec["result"]
            if not unsafe(res) and not read(res).decode("utf-8", "replace").startswith("BUILD: PINNED"):
                bad("ZERO_RESULT_MISMATCH", p, f"{res} is not BUILD: PINNED")
        else:
            _, inner = selftest_bound(p, int(n), "ZERO_DESCRIPTOR_MISMATCH")
            if inner is None or shlex.join(inner) != spec["command"]:
                bad("ZERO_COMMAND_MISMATCH", p, f"self-test {n}'s inner command is {inner}")
            if (r["stderr_evidence"], r["stderr_evidence_sha256"]) != (MERGED, "-") or os.path.lexists(
                    os.path.join(dest, f"{SELFTEST}/{n}/err.txt")):
                bad("ZERO_STDERR_MISMATCH", p, f"self-test {n}'s stderr is not merged into out.txt")
        if len(problems) == before:
            zverified.add(p)

    accepted = sorted(f for f in dups if f in verified)
    for f in sorted(dups):
        if f not in verified:
            bad("UNAUTHORIZED_DUPLICATE", f, sorted(x[1] for x in dups[f])[:3])
    for p in sorted(set(NONEMPTY) - set(dups)):
        bad("MAPPED_PATH_NOT_A_DUPLICATE", p)
    if len(dups) != len(NONEMPTY):
        bad("DUPLICATE_COUNT", "staged packet", f"{len(dups)} nonempty duplicates; ruled exactly {len(NONEMPTY)}")
    accepted_zero = sorted(f for f in empties if f in zverified)
    for f in sorted(empties):
        if f not in zverified:
            bad("ZERO_LENGTH_UNMAPPED", f)
    for p in sorted(set(ZERO) - set(empties)):
        bad("ZERO_MAPPED_PATH_NOT_STAGED_EMPTY", p)
    if len(empties) != len(ZERO):
        bad("ZERO_LENGTH_COUNT", "staged packet", f"{len(empties)} zero-length files; ruled exactly {len(ZERO)}")
    print(f"     duplicate and zero-length policy (R2): {len(dups)} nonempty staged file(s) byte-equal to a "
          f"previous-packet file, {len(accepted)} mapped and accepted; {len(empties)} zero-length staged file(s), "
          f"{len(accepted_zero)} mapped and accepted")
    return accepted, accepted_zero, problems


def main(dest):
    if os.path.exists(dest):
        raise SystemExit("REFUSED: destination exists")
    os.makedirs(dest)
    rendered = tempfile.mkdtemp(prefix="mt1c-render-")
    r = subprocess.run([sys.executable, os.path.join(K.EV, "tools/render.py"), rendered], capture_output=True, text=True)
    print(r.stdout.strip(), r.stderr.strip())
    pairs = [(os.path.join(rendered, "INDEX.md"), "INDEX.md"),
             (os.path.join(rendered, "TRANCHE-SUMMARY.md"), "TRANCHE-SUMMARY.md"),
             (os.path.join(rendered, "CARRIED-FORWARD.md"), "CARRIED-FORWARD.md")]
    skipped = []
    for src, rel in SOURCES:
        p = os.path.join(K.EV, src)
        if os.path.isdir(p):
            for f in files_under(p):
                if carried(os.path.relpath(f, K.EV)) or unchanged_tool(os.path.relpath(f, K.EV)):
                    skipped.append(os.path.relpath(f, K.EV))
                else:
                    pairs.append((f, os.path.join(rel, os.path.relpath(f, p))))
        else:
            pairs.append((p, rel))
    print(f"carried-forward copies and unchanged RESUME-01 tools not staged (referenced in CARRIED-FORWARD.md): "
          f"{len(skipped)}")
    for src, rel in pairs:
        os.makedirs(os.path.dirname(os.path.join(dest, rel)), exist_ok=True)
        shutil.copyfile(src, os.path.join(dest, rel))
    names = sorted(rel for _, rel in pairs)
    with open(os.path.join(dest, "SHA256SUMS"), "w") as f:
        for rel in names:
            f.write(f"{K.sha256_file(os.path.join(dest, rel))}  {rel}\n")
    fails = []

    def check(label, ok, detail=""):
        print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
        if not ok:
            fails.append(label)
    allf = [os.path.relpath(f, dest) for f in files_under(dest)]
    total = sum(os.path.getsize(os.path.join(dest, f)) for f in allf)
    check("packet size at most 32 MiB", total <= K.PACKET_LIMIT, f"{total} bytes, {len(allf)} files")
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
    exp = __import__("json").load(open(os.path.join(K.EV, "receipts/02-EXPECTATIONS.json")))
    mutants = {c["mutant_blob"] for c in exp["controls"].values()}
    pinned = {K.EXE_SHA256}
    for d, _, fs in os.walk(os.path.join(K.EV, "receipts")):
        for n in fs:
            if n == "PIN.json":
                pinned.add(__import__("json").load(open(os.path.join(d, n)))["sha256"])
    check("no file equal to the pinned baseline executable or any pinned mutant executable",
          not [f for f in allf if K.sha256_file(os.path.join(dest, f)) in pinned], len(pinned))
    crate = set()
    for e in K.git("ls-tree", "-r", "-z", K.C, "crates/").split(b"\0"):
        if e:
            crate.add(e.split(b"\t", 1)[0].decode().split()[2])
    blobs = {f: K.git_blob_id(open(os.path.join(dest, f), "rb").read()) for f in allf}
    check("no file equal to a mutant source blob", not [f for f, b in blobs.items() if b in mutants])
    empty = K.git_blob_id(b"")
    check("no file equal to a crate source blob of C (the empty blob, present in C's crates/, copies no content and is "
          "not counted)", not [f for f, b in blobs.items() if b in crate and b != empty],
          f"{sum(b == empty for b in blobs.values())} empty file(s)")
    check("no carried-forward path staged", not [f for f in allf if carried(f)], [f for f in allf if carried(f)][:3])
    staged_tools = sorted(f[6:] for f in allf if f.startswith("tools/"))
    check("only changed or added tools staged (every unchanged RESUME-01 tool referenced, not republished)",
          staged_tools and not [t for t in staged_tools if os.path.exists(os.path.join(K.EV, "original-tools", t))
                                and K.sha256_file(os.path.join(K.EV, "original-tools", t)) ==
                                K.sha256_file(os.path.join(dest, "tools", t))], staged_tools)
    accepted, accepted_zero, problems = dup_policy(dest, allf)
    check("no nonempty staged file byte-equal to any file of either previous packet (EXEC-01, RESUME-01), whatever its "
          "size, but exactly the 21 Architect-ruled files of NONEMPTY-DUPLICATE-MAP.tsv, and no zero-length staged file "
          "but exactly the 8 Architect-ruled artifacts of ZERO-LENGTH-ARTIFACT-MAP.tsv, each verified against every "
          "field of its row (PUBLISH-04, ruling R2)", not problems, problems[:24])
    print(f"     mapped nonempty duplicates accepted ({len(accepted)} of {len(NONEMPTY)}): {accepted}")
    print(f"     mapped zero-length artifacts accepted ({len(accepted_zero)} of {len(ZERO)}): {accepted_zero}")
    check("every staged file byte-equal to its source", all(K.sha256_file(s) == K.sha256_file(os.path.join(dest, r))
                                                           for s, r in pairs))
    listed = [l.split("  ", 1) for l in open(os.path.join(dest, "SHA256SUMS")).read().splitlines()]
    check("SHA256SUMS lists every file but itself, and verifies",
          sorted(p for _, p in listed) == sorted(f for f in allf if f != "SHA256SUMS")
          and all(K.sha256_file(os.path.join(dest, p)) == h for h, p in listed), f"{len(listed)} listed")
    shutil.rmtree(rendered)
    print("PACKET: " + ("STAGED — ALL CHECKS HOLD" if not fails else "FAILED: " + "; ".join(fails)))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1])))
