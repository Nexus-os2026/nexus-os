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
"""
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

# PUBLISH-03 (mission sections 1 and 6): the Architect's duplicate-link-receipt ruling. The seven exact current paths
# (packet-relative; the CONTINUE-02 receipt directories, not renamed), each with its control, its linked unchanged-core
# configuration and its one prior canonical file in the EXEC-01 packet (commit K.PREV). No other duplicate is permitted.
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
MAP = ("/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-mutation-tranche1-publish-03/documents/"
       "DUPLICATE-CONTENT-MAP.tsv")
MAP_COLUMNS = ["current_path", "control", "current_sha256", "current_git_blob", "size", "prior_commit", "prior_path",
               "prior_git_blob", "prior_sha256", "byte_equal", "linked_configuration", "linked_baseline_commit",
               "linked_baseline_path", "restoration_receipt", "restoration_sha256", "control_result",
               "control_result_sha256", "classification", "classification_sha256"]

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


def dup_policy(dest, allf, map_path=MAP):
    """The duplicate-content policy (PUBLISH-03) over the staged packet dest (allf: its packet-relative files).
    Returns (accepted, problems): the ruled paths accepted, and every problem as (code, path, detail). Rejected by
    default: every nonempty staged file whose git blob id or sha256 equals any file of either previous packet."""
    problems, accepted, verified = [], [], set()

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
    dups, empty = {}, 0
    for f in allf:
        if unsafe(f):
            bad("STAGED_" + unsafe(f), f)
            continue
        data = read(f)
        if not data:
            empty += 1
            continue
        hits = by_blob.get(K.git_blob_id(data), set()) | by_sha.get(K.sha256_bytes(data), set())
        if hits:
            dups[f] = hits

    # The map: exactly the header, one row per ruled path, nothing else.
    try:
        lines = open(map_path, encoding="utf-8").read().split("\n")
    except (OSError, UnicodeDecodeError) as e:
        lines = []
        bad("MAP_UNREADABLE", map_path, e)
    if lines and lines[-1] == "":
        lines.pop()
    rows = {}
    if lines and lines[0].split("\t") != MAP_COLUMNS:
        bad("MAP_HEADER", map_path, lines[0][:120])
    for n, line in enumerate(lines[1:] if lines and lines[0].split("\t") == MAP_COLUMNS else [], 2):
        v = line.split("\t")
        if len(v) != len(MAP_COLUMNS):
            bad("MAP_ROW_SHAPE", f"map line {n}", line[:120])
        elif v[0] in rows:
            bad("MAP_ROW_REPEATED", v[0])
        else:
            rows[v[0]] = dict(zip(MAP_COLUMNS, v))
    for p in sorted(set(rows) - set(RULED)):
        bad("MAP_ROW_NOT_RULED", p)
    for p in sorted(set(RULED) - set(rows)):
        bad("MAP_ROW_MISSING", p)

    exp = json.load(open(os.path.join(K.EV, "receipts/02-EXPECTATIONS.json")))
    for p in sorted(set(rows) & set(RULED)):
        r, (cid, cfg, prior_rel) = rows[p], RULED[p]
        before = len(problems)
        d = exp["controls"][cid]["dir"]
        if p.split("/06-link/")[0] != d or d.split("/")[1].split("-", 1)[1] != cid or r["control"] != cid:
            bad("CONTROL_IDENTITY", p, f"map control {r['control']}; expected directory {d}")
        why = unsafe(p)
        if why:
            bad("CURRENT_" + why, p)
            continue
        cur = read(p)
        if (K.sha256_bytes(cur), K.git_blob_id(cur), str(len(cur))) != (r["current_sha256"], r["current_git_blob"],
                                                                          r["size"]):
            bad("CURRENT_BYTES_CHANGED", p, f"{K.sha256_bytes(cur)} {K.git_blob_id(cur)} {len(cur)}")
        prior_path = f"{K.PREV_PACKET}/{prior_rel}"
        if r["prior_commit"] != K.PREV:
            bad("PRIOR_COMMIT_MISMATCH", p, r["prior_commit"])
        if r["prior_path"] != prior_path:
            bad("PRIOR_PATH_MISMATCH", p, r["prior_path"])
        pb = K.git("rev-parse", f"{K.PREV}:{prior_path}").decode().strip()
        prior = K.git("cat-file", "blob", pb)
        if r["prior_git_blob"] != pb:
            bad("PRIOR_BLOB_MISMATCH", p, r["prior_git_blob"])
        if r["prior_sha256"] != K.sha256_bytes(prior):
            bad("PRIOR_SHA256_MISMATCH", p, r["prior_sha256"])
        if prior != cur or r["byte_equal"] != "yes":
            bad("NOT_BYTE_EQUAL", p, f"map byte_equal {r['byte_equal']}")
        if dups.get(p) != {(K.PREV, prior_path)}:
            bad("PRIOR_SOURCE_MISMATCH", p, sorted(x[1] for x in dups.get(p, set())))
        # The linked configuration and baseline, from the link's own content and the control's expectations.
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
                labels = [l[5:].split(": ", 1)[0] if l[5:].startswith("exact configuration") else l[5:] for l in t
                          if l.startswith("OK   ")]
                linked = t[0] == "LINK: VERIFIED" and not [l for l in t if l.startswith("FAIL")]
        except (ValueError, KeyError, IndexError, TypeError) as e:
            ref, labels, linked = None, [], False
            bad("LINK_UNPARSED", p, e)
        if ref is None and p.endswith(".txt"):
            bad("CONTROL_IDENTITY", p, f"the link text does not name control {cid}")
        if ref != reference:
            bad("BASELINE_MISMATCH", p, f"link reference {ref}")
        if cfg_label not in labels:
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
        if len(problems) == before:
            verified.add(p)

    for f in sorted(dups):
        if f in verified:
            accepted.append(f)
        else:
            bad("UNAUTHORIZED_DUPLICATE", f, sorted(x[1] for x in dups[f])[:3])
    for p in sorted(set(RULED) - set(dups)):
        bad("RULED_PATH_NOT_A_DUPLICATE", p)
    print(f"     duplicate-content policy: {len(dups)} nonempty staged file(s) byte-equal to a previous-packet file; "
          f"{len(accepted)} ruled link receipt(s) accepted; {empty} zero-length file(s) not compared")
    return accepted, problems


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
    accepted, problems = dup_policy(dest, allf)
    check("no nonempty staged file byte-equal to any file of either previous packet (EXEC-01, RESUME-01), whatever its "
          "size, but the seven Architect-ruled link receipts, each verified against every field of its "
          "DUPLICATE-CONTENT-MAP.tsv row (PUBLISH-03)", not problems, problems[:24])
    print(f"     ruled link receipts accepted ({len(accepted)} of {len(RULED)}): {accepted}")
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
