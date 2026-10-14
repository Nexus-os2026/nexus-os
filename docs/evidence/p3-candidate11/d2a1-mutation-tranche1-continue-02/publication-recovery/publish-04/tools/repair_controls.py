#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission section 9: static controls of the R2 duplicate-content and
zero-length policy, before any packet is created in the repository. Adapted from PUBLISH-03 tools/repair_controls.py.
Nothing is built, run or executed but Python and read-only git.

    repair_controls.py OUTDIR      OUTDIR must not exist; writes OUTDIR/CONTROLS.json, every map variant
                                   (maps/*.tsv) and the builder run of control P4 (P4-builder-run.txt)

Control P4 runs the R2 builder exactly as published (its CLI) on the CONTINUE-02 evidence into a scratch directory
(the base). Every other control applies the builder's own dup_policy() to a scratch copy of the base after one
synthetic change (of the staged files, or of a copy of a map); the CONTINUE-02 and PUBLISH-03 roots are never touched.
A negative control holds when the policy rejects the change with every expected (code, path) problem, and every problem
it reports lies on a path the control names (its intended failure class, nothing collateral elsewhere). N20, N21 and
N22 hold when an over-broad policy (a 1-KiB threshold; every cgroup-samples.txt; every build.stdout), implemented here
and never in the builder, wrongly accepts a change that the R2 policy rejects.
"""
import json
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import build_packet as B  # noqa: E402
import pub4common as Q  # noqa: E402

NM, ZM, SP = "NONEMPTY-DUPLICATE-MAP.tsv", "ZERO-LENGTH-ARTIFACT-MAP.tsv", "staged packet"
RES = Q.RESUME01_PACKET + "/receipts/32-C14/"
ST = B.SELFTEST
results = []


def staged(dest):
    return [os.path.relpath(f, dest) for f in B.files_under(dest)]


def load(path, columns):
    ls = open(path).read().splitlines()
    return ls[0], [dict(zip(columns, l.split("\t"))) for l in ls[1:]]


def write_map(path, header, columns, rows):
    with open(path, "w") as f:
        f.write(header + "\n")
        for r in rows:
            f.write("\t".join(r[c] for c in columns) + "\n")


def keys(problems):
    return {(c, p) for c, p, _ in problems}


def run_policy(dest, map_path, zmap_path):
    import contextlib
    import io
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        acc, accz, probs = B.dup_policy(dest, staged(dest), map_path, zmap_path)
    return acc, accz, probs, buf.getvalue().strip()


def prior_bytes(path):
    c = Q.RESUME01 if path.startswith(Q.RESUME01_PACKET) else Q.EXEC01
    return Q.git("cat-file", "blob", Q.git("rev-parse", f"{c}:{path}").decode().strip())


def record(cid, title, ok, detail):
    results.append({"control": cid, "title": title, "as_expected": bool(ok), "detail": detail})
    print(f"{'AS EXPECTED    ' if ok else 'NOT AS EXPECTED'} {cid} {title}")
    for k, v in detail.items():
        print(f"       {k}: {v}")


def main(out):
    Q.no_bytecode()
    if os.path.exists(out):
        raise SystemExit("REFUSED: output exists")
    os.makedirs(os.path.join(out, "maps"))
    scratch = os.path.join(Q.P, "scratch", "controls-" + os.path.basename(out))
    if os.path.exists(scratch):
        raise SystemExit("REFUSED: scratch exists")
    os.makedirs(scratch)
    nh, rows = load(B.MAP, B.MAP_COLUMNS)
    zh, zrows = load(B.ZERO_MAP, B.ZERO_COLUMNS)
    print(f"map {B.MAP} sha256 {Q.sha256_file(B.MAP)}")
    print(f"zero-length map {B.ZERO_MAP} sha256 {Q.sha256_file(B.ZERO_MAP)}")
    print(f"builder {B.__file__} sha256 {Q.sha256_file(B.__file__)}")

    # Control P4 first: it stages the base copy every other control starts from.
    base = os.path.join(scratch, "base")
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    r = subprocess.run([sys.executable, "-B", os.path.join(Q.P, "tools/build_packet.py"), base], capture_output=True,
                       text=True, env=env)
    open(os.path.join(out, "P4-builder-run.txt"), "w").write(r.stdout + r.stderr + f"\nexit {r.returncode}\n")
    last = r.stdout.strip().splitlines()[-1] if r.stdout.strip() else ""
    acc0, accz0, p0, info0 = run_policy(base, B.MAP, B.ZERO_MAP)

    def variant(name, mutate=None, map_rows=None, zmap_rows=None):
        d = os.path.join(scratch, name)
        shutil.copytree(base, d, symlinks=True)
        if mutate:
            mutate(d)
        mp, zp = B.MAP, B.ZERO_MAP
        if map_rows is not None:
            mp = os.path.join(out, "maps", name + ".nonempty.tsv")
            write_map(mp, nh, B.MAP_COLUMNS, map_rows)
        if zmap_rows is not None:
            zp = os.path.join(out, "maps", name + ".zero.tsv")
            write_map(zp, zh, B.ZERO_COLUMNS, zmap_rows)
        return d, mp, zp

    def negative(cid, title, expect, mutate=None, map_rows=None, zmap_rows=None, maps=None):
        d, mp, zp = variant(cid, mutate, map_rows, zmap_rows)
        if maps:
            mp, zp = maps(d, mp, zp)
        acc, accz, probs, _ = run_policy(d, mp, zp)
        got = keys(probs)
        paths = {p for _, p in expect}
        stray = sorted(k for k in got if k[1] not in paths)
        ok = bool(probs) and set(expect) <= got and not stray
        record(cid, title, ok, {"expected (code, path), all required": sorted(expect),
                                "reported (code, path)": sorted(got), "reported outside the named paths": stray,
                                "verdict": "REJECTED" if probs else "ACCEPTED",
                                "accepted nonempty / zero-length": f"{len(acc)} / {len(accz)}"})
        return d, mp, zp, probs

    def put(rel, data):
        def f(d):
            p = os.path.join(d, rel)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            if os.path.lexists(p):
                os.remove(p)
            open(p, "wb").write(data)
        return f

    def rm(rel):
        return lambda d: os.remove(os.path.join(d, rel))

    def move(src, dst):
        def f(d):
            os.makedirs(os.path.dirname(os.path.join(d, dst)), exist_ok=True)
            os.replace(os.path.join(d, src), os.path.join(d, dst))
        return f

    def link_to(rel, target):
        def f(d):
            os.remove(os.path.join(d, rel))
            os.symlink(target, os.path.join(d, rel))
        return f

    def both(*fs):
        def f(d):
            for g in fs:
                g(d)
        return f

    def nrows(path=None, drop=False, add=None, **chg):
        o = []
        for r0 in rows:
            if r0["current_path"] == path and drop:
                continue
            r1 = dict(r0)
            if r1["current_path"] == path:
                r1.update(chg)
            o.append(r1)
        return o + ([add] if add else [])

    def zrows_(path=None, drop=False, add=None, **chg):
        o = []
        for r0 in zrows:
            if r0["current_path"] == path and drop:
                continue
            r1 = dict(r0)
            if r1["current_path"] == path:
                r1.update(chg)
            o.append(r1)
        return o + ([add] if add else [])

    def row(path):
        return [r0 for r0 in rows if r0["current_path"] == path][0]

    def flip(sha):
        return sha[:-1] + ("0" if sha[-1] != "0" else "1")

    c9j, c9t, c11j, c17j = ("receipts/38-C9/06-link/LINK.json", "receipts/38-C9/06-link/LINK.txt",
                            "receipts/39-C11/06-link/LINK.json", "receipts/40-C17/06-link/LINK.json")
    c18aj, c18at, c18bj = ("receipts/41-C18a/06-link/LINK.json", "receipts/41-C18a/06-link/LINK.txt",
                           "receipts/42-C18b/06-link/LINK.json")
    cg = {c: f"{d}/04-run/cgroup-samples.txt" for c, d in B.RUN_DIRS.items()}
    zb = {c: f"{B.RUN_DIRS[c]}/03-build/build.stdout" for c in B.BUILT}
    st = {k: f"{ST}/{k}" for k in ("1/cgroup-samples.txt", "5/cgroup-samples.txt", "7/cgroup-samples.txt", "1/out.txt",
                                    "6/out.txt", "7/out.txt", "7/err.txt", "3/out.txt", "5/out.txt")}
    st_rows = [p for p in B.NONEMPTY if p.startswith(ST + "/")]
    fb2 = open(os.path.join(base, c9j), "rb").read()
    fg = open(os.path.join(base, c18aj), "rb").read()
    const = open(os.path.join(base, cg["C12"]), "rb").read()

    print("[positive controls]")
    record("P1", "exactly all 21 mapped nonempty duplicates accepted",
           acc0 == sorted(B.NONEMPTY) and not p0 and len(acc0) == 21,
           {"accepted": len(acc0), "problems": p0, "policy": info0})
    sums = [l.split("  ", 1) for l in open(os.path.join(base, "SHA256SUMS")).read().splitlines()]
    per = {p: sum(1 for _, q in sums if q == p) for p in list(B.NONEMPTY) + list(B.ZERO)}
    groups = {}
    for h, p in sums:
        if p in B.NONEMPTY:
            groups.setdefault(h, []).append(p)
    distinct = len({os.stat(os.path.join(base, p)).st_ino for p in B.NONEMPTY}) == 21
    record("P2", "all 21 remain separate paths in the packet and in its manifest (SHA256SUMS), equal content or not",
           all(per[p] == 1 for p in B.NONEMPTY) and distinct
           and sorted(len(v) for v in groups.values()) == [1, 1, 1, 1, 1, 1, 2, 3, 10],
           {"SHA256SUMS lines per mapped path": sorted(set(per[p] for p in B.NONEMPTY)),
            "21 distinct files (inodes)": distinct,
            "mapped paths grouped by sha256": {h[:12]: len(v) for h, v in groups.items()}})
    record("P3", "exactly all 8 mapped zero-length artifacts accepted, each a separate path listed once with the empty "
           "sha256", accz0 == sorted(B.ZERO) and not p0 and all(per[p] == 1 for p in B.ZERO)
           and all(h == B.EMPTY_SHA for h, p in sums if p in B.ZERO),
           {"accepted": accz0, "SHA256SUMS lines per path": sorted(set(per[p] for p in B.ZERO))})
    record("P4", "the complete real CONTINUE-02 evidence packet stages successfully (R2 builder CLI)",
           r.returncode == 0 and last == "PACKET: STAGED — ALL CHECKS HOLD",
           {"exit": r.returncode, "last line": last, "receipt": "P4-builder-run.txt"})

    print("[negative controls]")
    negative("N1", "remove one link mapping (C11 LINK.json row)",
             [("MAP_ROW_MISSING", c11j), ("MAP_ROW_COUNT", NM), ("UNAUTHORIZED_DUPLICATE", c11j)],
             map_rows=nrows(c11j, drop=True))
    negative("N2", "remove one constant mapping (C12 run sampler record row)",
             [("MAP_ROW_MISSING", cg["C12"]), ("MAP_ROW_COUNT", NM), ("UNAUTHORIZED_DUPLICATE", cg["C12"])],
             map_rows=nrows(cg["C12"], drop=True))
    negative("N3", "remove one zero-length mapping (C9 build.stdout row)",
             [("ZMAP_ROW_MISSING", zb["C9"]), ("ZMAP_ROW_COUNT", ZM), ("ZERO_LENGTH_UNMAPPED", zb["C9"])],
             zmap_rows=zrows_(zb["C9"], drop=True))
    extra = "receipts/37-C12/06-link/LINK-F-B2.json"
    negative("N4a", "add a twenty-second nonempty duplicate (an F-B2 link copy under C12)",
             [("UNAUTHORIZED_DUPLICATE", extra), ("DUPLICATE_COUNT", SP)], mutate=put(extra, fb2))
    extra2 = f"{ST}/2/err.txt"
    negative("N4b", "add a twenty-second nonempty duplicate (self-test 7's err.txt bytes as self-test 2's err.txt)",
             [("UNAUTHORIZED_DUPLICATE", extra2), ("DUPLICATE_COUNT", SP)],
             mutate=put(extra2, open(os.path.join(base, st["7/err.txt"]), "rb").read()))
    ninth = "receipts/37-C12/04-run/stderr.txt"
    negative("N5", "add a ninth zero-length file (C12 04-run/stderr.txt)",
             [("ZERO_LENGTH_UNMAPPED", ninth), ("ZERO_LENGTH_COUNT", SP)], mutate=put(ninth, b""))
    moved = "receipts/37-C12/06-link/LINK.json"
    negative("N6a", "move an allowed duplicate (C9 LINK.json) into another control's directory (C12's 06-link)",
             [("CURRENT_ABSENT", c9j), ("UNAUTHORIZED_DUPLICATE", moved), ("MAPPED_PATH_NOT_A_DUPLICATE", c9j)],
             mutate=move(c9j, moved))
    zmoved = "receipts/36-C28/03-build/build.stdout"
    negative("N6b", "move an allowed zero-length artifact (C12 build.stdout) into another control's directory (C28)",
             [("ZERO_CURRENT_ABSENT", zb["C12"]), ("ZERO_LENGTH_UNMAPPED", zmoved),
              ("ZERO_MAPPED_PATH_NOT_STAGED_EMPTY", zb["C12"])], mutate=move(zb["C12"], zmoved))
    e6 = f"{ST}/6/err.txt"
    negative("N6c", "move an allowed constant (self-test 7 err.txt) into another self-test's directory (6)",
             [("CURRENT_ABSENT", st["7/err.txt"]), ("UNAUTHORIZED_DUPLICATE", e6),
              ("MAPPED_PATH_NOT_A_DUPLICATE", st["7/err.txt"]), ("ROLE_MISMATCH", st["7/out.txt"]),
              ("UNAUTHORIZED_DUPLICATE", st["7/out.txt"]), ("ROLE_MISMATCH", st["6/out.txt"]),
              ("UNAUTHORIZED_DUPLICATE", st["6/out.txt"])], mutate=move(st["7/err.txt"], e6))
    negative("N7a", "change one byte in a mapped link receipt (C17 LINK.json), map unchanged",
             [("CURRENT_BYTES_CHANGED", c17j), ("NOT_BYTE_EQUAL", c17j), ("PRIOR_SOURCE_MISMATCH", c17j),
              ("MAPPED_PATH_NOT_A_DUPLICATE", c17j), ("DUPLICATE_COUNT", SP)],
             mutate=put(c17j, fb2.replace(b"cde9f9d20f32", b"cde9f9d20f33", 1)))
    negative("N7b", "change one byte in a mapped constant record (C9 run sampler record), map unchanged",
             [("CURRENT_BYTES_CHANGED", cg["C9"]), ("LITERAL_MISMATCH", cg["C9"]), ("NOT_BYTE_EQUAL", cg["C9"]),
              ("PRIOR_SOURCE_MISMATCH", cg["C9"]), ("GENERATOR_ITEM_ABSENT", cg["C9"]),
              ("MAPPED_PATH_NOT_A_DUPLICATE", cg["C9"]), ("DUPLICATE_COUNT", SP)],
             mutate=put(cg["C9"], b"S" + const[1:]))
    negative("N8", "make a mapped zero-length artifact nonempty (C17 build.stdout = 'x\\n')",
             [("ZERO_NOT_EMPTY", zb["C17"]), ("ZERO_MAPPED_PATH_NOT_STAGED_EMPTY", zb["C17"]),
              ("ZERO_LENGTH_COUNT", SP)], mutate=put(zb["C17"], b"x\n"))
    negative("N9", "replace a mapped empty file with a symlink (C11 build.stdout -> C9's)",
             [("STAGED_SYMLINK", zb["C11"]), ("ZERO_CURRENT_SYMLINK", zb["C11"]),
              ("ZERO_MAPPED_PATH_NOT_STAGED_EMPTY", zb["C11"]), ("ZERO_LENGTH_COUNT", SP)],
             mutate=link_to(zb["C11"], "../../38-C9/03-build/build.stdout"))
    wrong_fg = Q.EXEC01_PACKET + "/receipts/04-rehearsal/links/link-C18a-stage2/LINK.json"
    negative("N10a", "wrong canonical prior path for a link receipt (C9 LINK.json -> the F-G rehearsal link)",
             [("PRIOR_PATH_MISMATCH", c9j), ("R1_FIELD_MISMATCH", c9j), ("UNAUTHORIZED_DUPLICATE", c9j)],
             map_rows=nrows(c9j, prior_path=wrong_fg))
    c19 = Q.RESUME01_PACKET + "/receipts/33-C19/04-run/cgroup-samples.txt"
    negative("N10b", "wrong canonical prior path, itself byte-equal (C12 run sampler record -> RESUME-01 C19's)",
             [("PRIOR_PATH_MISMATCH", cg["C12"]), ("UNAUTHORIZED_DUPLICATE", cg["C12"])],
             map_rows=nrows(cg["C12"], prior_path=c19))
    negative("N10c", "wrong canonical prior commit and path, byte-equal but another self-test (self-test 1 sampler "
             "record -> EXEC-01 self-test 7's)",
             [("PRIOR_COMMIT_MISMATCH", st["1/cgroup-samples.txt"]), ("PRIOR_PATH_MISMATCH", st["1/cgroup-samples.txt"]),
              ("UNAUTHORIZED_DUPLICATE", st["1/cgroup-samples.txt"])],
             map_rows=nrows(st["1/cgroup-samples.txt"], prior_commit=Q.EXEC01,
                            prior_path=f"{Q.EXEC01_PACKET}/{ST}/7/cgroup-samples.txt"))
    negative("N11a", "wrong prior sha256 (self-test 6 out.txt row)",
             [("PRIOR_SHA256_MISMATCH", st["6/out.txt"]), ("UNAUTHORIZED_DUPLICATE", st["6/out.txt"])],
             map_rows=nrows(st["6/out.txt"], prior_sha256=flip(row(st["6/out.txt"])["prior_sha256"])))
    negative("N11b", "wrong prior git blob (C18b run sampler record row)",
             [("PRIOR_BLOB_MISMATCH", cg["C18b"]), ("UNAUTHORIZED_DUPLICATE", cg["C18b"])],
             map_rows=nrows(cg["C18b"], prior_git_blob=flip(row(cg["C18b"])["prior_git_blob"])))
    negative("N11c", "wrong prior sha256 of a link receipt (C9 LINK.txt row)",
             [("PRIOR_SHA256_MISMATCH", c9t), ("R1_FIELD_MISMATCH", c9t), ("UNAUTHORIZED_DUPLICATE", c9t)],
             map_rows=nrows(c9t, prior_sha256=flip(row(c9t)["prior_sha256"])))
    negative("N12a", "missing current run descriptor (C12 04-run/descriptor.json)",
             [("DESCRIPTOR_ABSENT", cg["C12"]), ("UNAUTHORIZED_DUPLICATE", cg["C12"])],
             mutate=rm("receipts/37-C12/04-run/descriptor.json"))
    negative("N12b", "missing current build descriptor (C18a 03-build/descriptor.json)",
             [("ZERO_DESCRIPTOR_ABSENT", zb["C18a"]), ("ZERO_DESCRIPTOR_MISMATCH", zb["C18a"]),
              ("ZERO_COMMAND_MISMATCH", zb["C18a"]), ("ZERO_LENGTH_UNMAPPED", zb["C18a"])],
             mutate=rm("receipts/41-C18a/03-build/descriptor.json"))
    negative("N12c", "missing current self-test receipt (SELFTEST.txt): every self-test row, nonempty and empty",
             [("DESCRIPTOR_ABSENT", p) for p in st_rows] + [("UNAUTHORIZED_DUPLICATE", p) for p in st_rows]
             + [("ZERO_DESCRIPTOR_ABSENT", st["3/out.txt"]), ("ZERO_LENGTH_UNMAPPED", st["3/out.txt"]),
                ("ZERO_DESCRIPTOR_ABSENT", st["5/out.txt"]), ("ZERO_LENGTH_UNMAPPED", st["5/out.txt"])],
             mutate=rm(f"{ST}/SELFTEST.txt"))
    negative("N12d", "missing current restoration receipt (C9 05-restore/RESTORE.txt)",
             [("RESTORATION_ABSENT", c9j), ("RESTORATION_ABSENT", c9t), ("UNAUTHORIZED_DUPLICATE", c9j),
              ("UNAUTHORIZED_DUPLICATE", c9t)], mutate=rm("receipts/38-C9/05-restore/RESTORE.txt"))
    negative("N12e", "missing current control result (C11 CONTROL-RESULT.json)",
             [("RESULT_ABSENT", c11j), ("DESCRIPTOR_ABSENT", c11j), ("UNAUTHORIZED_DUPLICATE", c11j)],
             mutate=rm("receipts/39-C11/CONTROL-RESULT.json"))
    negative("N12f", "missing current build stderr (C17 03-build/build.stderr)",
             [("ZERO_STDERR_ABSENT", zb["C17"]), ("ZERO_DESCRIPTOR_MISMATCH", zb["C17"]),
              ("ZERO_LENGTH_UNMAPPED", zb["C17"])], mutate=rm("receipts/40-C17/03-build/build.stderr"))
    negative("N12g", "missing current supervisor receipt (C28 04-run/scope-record.txt)",
             [("SUPERVISOR_RECEIPT_ABSENT", cg["C28"]), ("UNAUTHORIZED_DUPLICATE", cg["C28"])],
             mutate=rm("receipts/36-C28/04-run/scope-record.txt"))
    negative("N12h", "missing current result record (self-test 5 time-v.txt): its sampler record and its empty out.txt",
             [("SUPERVISOR_RECEIPT_ABSENT", st["5/cgroup-samples.txt"]),
              ("UNAUTHORIZED_DUPLICATE", st["5/cgroup-samples.txt"]), ("ZERO_RESULT_ABSENT", st["5/out.txt"]),
              ("ZERO_LENGTH_UNMAPPED", st["5/out.txt"])], mutate=rm(f"{ST}/5/time-v.txt"))
    negative("N13a", "wrong current control id in the map (C12 run sampler record claims C9)",
             [("CONTROL_IDENTITY", cg["C12"]), ("UNAUTHORIZED_DUPLICATE", cg["C12"])],
             map_rows=nrows(cg["C12"], control_id="C9"))
    negative("N13b", "wrong current role in the map (self-test 7 err.txt claims the stdout role)",
             [("ROLE_MISMATCH", st["7/err.txt"]), ("UNAUTHORIZED_DUPLICATE", st["7/err.txt"])],
             map_rows=nrows(st["7/err.txt"], current_role=row(st["7/out.txt"])["current_role"]))
    negative("N13c", "wrong control id in the zero-length map (C11 build.stdout claims C17)",
             [("ZERO_CONTROL_IDENTITY", zb["C11"]), ("ZERO_LENGTH_UNMAPPED", zb["C11"])],
             zmap_rows=zrows_(zb["C11"], control_id="C17"))
    negative("N13d", "wrong role in the zero-length map (self-test 3 out.txt claims a build stdout role)",
             [("ZERO_ROLE_MISMATCH", st["3/out.txt"]), ("ZERO_LENGTH_UNMAPPED", st["3/out.txt"])],
             zmap_rows=zrows_(st["3/out.txt"], role=[z for z in zrows if z["current_path"] == zb["C12"]][0]["role"]))
    negative("N13e", "wrong self-test id in the map (self-test 6 out.txt claims self-test 1)",
             [("SELFTEST_IDENTITY", st["6/out.txt"]), ("UNAUTHORIZED_DUPLICATE", st["6/out.txt"])],
             map_rows=nrows(st["6/out.txt"], selftest_id="1"))
    negative("N13f", "wrong control id of a link receipt (C18a LINK.txt claims C18b)",
             [("CONTROL_IDENTITY", c18at), ("R1_FIELD_MISMATCH", c18at), ("UNAUTHORIZED_DUPLICATE", c18at)],
             map_rows=nrows(c18at, control_id="C18b"))
    negative("N13g", "the current run receipt names another control (C12 04-run/RECEIPT.txt replaced by C9's)",
             [("DESCRIPTOR_MISMATCH", cg["C12"]), ("UNAUTHORIZED_DUPLICATE", cg["C12"])],
             mutate=put("receipts/37-C12/04-run/RECEIPT.txt",
                        open(os.path.join(base, "receipts/38-C9/04-run/RECEIPT.txt"), "rb").read()))
    negative("N13h", "the current run descriptor describes another control (C12 04-run/descriptor.json replaced by "
             "C9's)", [("DESCRIPTOR_MISMATCH", cg["C12"]), ("UNAUTHORIZED_DUPLICATE", cg["C12"])],
             mutate=put("receipts/37-C12/04-run/descriptor.json",
                        open(os.path.join(base, "receipts/38-C9/04-run/descriptor.json"), "rb").read()))
    fg_prior = Q.EXEC01_PACKET + "/receipts/04-rehearsal/links/link-C18a-stage2/LINK.json"
    r18 = row(c18aj)
    negative("N14a", "C9 LINK.json replaced by the F-G link (another configuration), the map row made to agree with "
             "the bytes and the F-G prior",
             [("CONFIG_MISMATCH", c9j), ("BASELINE_MISMATCH", c9j), ("PRIOR_PATH_MISMATCH", c9j),
              ("R1_FIELD_MISMATCH", c9j), ("UNAUTHORIZED_DUPLICATE", c9j)],
             mutate=put(c9j, fg), map_rows=nrows(
                 c9j, current_sha256=r18["current_sha256"], current_git_blob=r18["current_git_blob"], size=r18["size"],
                 prior_path=fg_prior, prior_git_blob=r18["prior_git_blob"], prior_sha256=r18["prior_sha256"],
                 prior_size=r18["prior_size"], prior_equal_files=r18["prior_equal_files"], linked_configuration="F-G",
                 linked_baseline_path=r18["linked_baseline_path"]))
    negative("N14b", "the map row claims another linked configuration (C11 LINK.json: F-G)",
             [("CONFIG_MISMATCH", c11j), ("R1_FIELD_MISMATCH", c11j), ("UNAUTHORIZED_DUPLICATE", c11j)],
             map_rows=nrows(c11j, linked_configuration="F-G"))
    relabel = fb2.replace(b"this control's run (F-B2)", b"this control's run (F-G)", 1)
    negative("N14c", "C17 LINK.json's configuration check relabelled F-G (map current fields agree)",
             [("CONFIG_MISMATCH", c17j), ("NOT_BYTE_EQUAL", c17j), ("MAPPED_PATH_NOT_A_DUPLICATE", c17j),
              ("DUPLICATE_COUNT", SP)], mutate=put(c17j, relabel),
             map_rows=nrows(c17j, current_sha256=Q.sha256_bytes(relabel), current_git_blob=Q.git_blob_id(relabel),
                            size=str(len(relabel))))
    bcg = "receipts/37-C12/03-build/cgroup-samples.txt"
    _, _, _, p15 = negative("N15a", "the sampler constant in a build directory (C12 03-build/cgroup-samples.txt)",
                            [("UNAUTHORIZED_DUPLICATE", bcg), ("DUPLICATE_COUNT", SP)], mutate=put(bcg, const))
    negative("N15b", "the sampler constant in a build directory, with the C12 run row pointed at it",
             [("MAP_ROW_NOT_RULED", bcg), ("MAP_ROW_MISSING", cg["C12"]), ("UNAUTHORIZED_DUPLICATE", bcg),
              ("UNAUTHORIZED_DUPLICATE", cg["C12"]), ("DUPLICATE_COUNT", SP)],
             mutate=put(bcg, const), map_rows=nrows(cg["C12"], current_path=bcg))
    s3cg = f"{ST}/3/cgroup-samples.txt"
    negative("N15c", "the sampler constant in an unmapped self-test directory (self-test 3, a sampled run)",
             [("UNAUTHORIZED_DUPLICATE", s3cg), ("DUPLICATE_COUNT", SP)], mutate=put(s3cg, const))
    for cid, title, rel, src in (
            ("N16", "build", "receipts/37-C12/03-build/RECEIPT.txt", RES + "03-build/RECEIPT.txt"),
            ("N17", "run", "receipts/37-C12/04-run/RECEIPT.txt", RES + "04-run/RECEIPT.txt"),
            ("N18", "classification", "receipts/37-C12/04-run/CLASSIFICATION.txt", RES + "04-run/CLASSIFICATION.txt"),
            ("N19", "restoration", "receipts/37-C12/05-restore/RESTORE.txt", RES + "05-restore/RESTORE.txt")):
        data = prior_bytes(src)
        expect = [("UNAUTHORIZED_DUPLICATE", rel), ("DUPLICATE_COUNT", SP)]
        if cid == "N17":
            expect += [("DESCRIPTOR_MISMATCH", cg["C12"]), ("UNAUTHORIZED_DUPLICATE", cg["C12"])]
        d, mp, zp, probs = negative(cid, f"duplicate a {title} receipt: C12's {rel.split('/', 2)[2]} replaced by "
                                    f"RESUME-01 C14's ({len(data)} bytes)", expect, mutate=put(rel, data))
        if cid == "N18":
            n18 = (d, probs, rel, len(data))

    print("[over-broad policies (implemented here only, never in the builder) would wrongly accept]")
    d, probs, rel, size = n18
    floor = [x for x in probs if not ((x[0] == "UNAUTHORIZED_DUPLICATE" and os.path.getsize(os.path.join(d, x[1])) < 1024)
                                      or x[0] == "DUPLICATE_COUNT")]
    record("N20", f"reintroduce the 1-KiB threshold: it wrongly accepts N18 (C12's run classification replaced by "
           f"RESUME-01 C14's, {size} bytes); R2 rejects it",
           size < 1024 and floor == [] and ("UNAUTHORIZED_DUPLICATE", rel) in keys(probs),
           {"threshold policy problems": floor, "R2 problems": sorted(keys(probs))})
    broad = [x for x in p15 if not ((x[0] == "UNAUTHORIZED_DUPLICATE" and x[1].endswith("/cgroup-samples.txt"))
                                    or x[0] == "DUPLICATE_COUNT")]
    record("N21", f"permit every cgroup-samples.txt: it wrongly accepts the unauthorized {bcg} (N15a); R2 rejects it",
           broad == [] and ("UNAUTHORIZED_DUPLICATE", bcg) in keys(p15),
           {"over-broad policy problems": broad, "R2 problems": sorted(keys(p15))})
    c28bs = "receipts/36-C28/03-build/build.stdout"
    _, _, _, p22 = negative("N22a", "an unauthorized empty build.stdout (C28 has no build): R2 rejects it",
                            [("ZERO_LENGTH_UNMAPPED", c28bs), ("ZERO_LENGTH_COUNT", SP)], mutate=put(c28bs, b""))
    broad = [x for x in p22 if not ((x[0] == "ZERO_LENGTH_UNMAPPED" and x[1].endswith("/build.stdout"))
                                    or x[0] == "ZERO_LENGTH_COUNT")]
    record("N22b", f"permit every build.stdout: it wrongly accepts the unauthorized empty {c28bs}; R2 rejects it",
           broad == [] and ("ZERO_LENGTH_UNMAPPED", c28bs) in keys(p22),
           {"over-broad policy problems": broad, "R2 problems": sorted(keys(p22))})

    print("[negative controls, continued]")
    trav = "receipts/38-C9/06-link/../06-link/LINK.json"
    negative("N23a", "path traversal in the nonempty map (C9 LINK.json row as .../06-link/../06-link/LINK.json)",
             [("MAP_ROW_NOT_RULED", trav), ("MAP_ROW_MISSING", c9j), ("UNAUTHORIZED_DUPLICATE", c9j)],
             map_rows=nrows(c9j, current_path=trav))
    ztrav = "receipts/37-C12/03-build/../03-build/build.stdout"
    negative("N23b", "path traversal in the zero-length map (C12 build.stdout row as .../03-build/../03-build/...)",
             [("ZMAP_ROW_NOT_RULED", ztrav), ("ZMAP_ROW_MISSING", zb["C12"]), ("ZERO_LENGTH_UNMAPPED", zb["C12"])],
             zmap_rows=zrows_(zb["C12"], current_path=ztrav))
    negative("N24a", "symlink substitution of a mapped nonempty file (C11 run sampler record -> C9's, same bytes)",
             [("STAGED_SYMLINK", cg["C11"]), ("CURRENT_SYMLINK", cg["C11"]), ("MAPPED_PATH_NOT_A_DUPLICATE", cg["C11"]),
              ("DUPLICATE_COUNT", SP)], mutate=link_to(cg["C11"], "../../38-C9/04-run/cgroup-samples.txt"))
    outside = os.path.join(scratch, "outside-LINK.txt")
    open(outside, "wb").write(open(os.path.join(base, c18at), "rb").read())
    negative("N24b", "symlink substitution to a byte-equal file outside the packet (C18a LINK.txt; C18a's LINK.json, "
             "which binds on its sibling LINK.txt, is rejected with it)",
             [("STAGED_SYMLINK", c18at), ("CURRENT_SYMLINK", c18at), ("MAPPED_PATH_NOT_A_DUPLICATE", c18at),
              ("DUPLICATE_COUNT", SP), ("CONTROL_IDENTITY", c18aj), ("UNAUTHORIZED_DUPLICATE", c18aj)],
             mutate=link_to(c18at, outside))
    empty_out = os.path.join(scratch, "outside-empty")
    open(empty_out, "wb").close()
    negative("N24c", "symlink substitution of a mapped zero-length artifact to an empty file outside the packet "
             "(self-test 3 out.txt)",
             [("STAGED_SYMLINK", st["3/out.txt"]), ("ZERO_CURRENT_SYMLINK", st["3/out.txt"]),
              ("ZERO_MAPPED_PATH_NOT_STAGED_EMPTY", st["3/out.txt"]), ("ZERO_LENGTH_COUNT", SP)],
             mutate=link_to(st["3/out.txt"], empty_out))

    def map_link(which):
        def f(d, mp, zp):
            real = os.path.join(scratch, f"{os.path.basename(d)}-real-{which}.tsv")
            shutil.copyfile(B.MAP if which == "nonempty" else B.ZERO_MAP, real)
            ln = os.path.join(scratch, f"{os.path.basename(d)}-{which}-map-symlink.tsv")
            os.symlink(real, ln)
            return (ln, zp) if which == "nonempty" else (mp, ln)
        return f
    negative("N24d", "the nonempty map is a symlink (to a byte-identical copy)",
             [("MAP_SYMLINK", NM)] + [("UNAUTHORIZED_DUPLICATE", p) for p in B.NONEMPTY], maps=map_link("nonempty"))
    negative("N24e", "the zero-length map is a symlink (to a byte-identical copy)",
             [("ZMAP_SYMLINK", ZM)] + [("ZERO_LENGTH_UNMAPPED", p) for p in B.ZERO], maps=map_link("zero"))

    def dir_link(d):
        shutil.rmtree(os.path.join(d, "receipts/40-C17/05-restore"))
        os.symlink("../39-C11/05-restore", os.path.join(d, "receipts/40-C17/05-restore"))
    negative("N24f", "symlink substitution of a control's restoration directory (C17 05-restore -> C11's)",
             [("RESTORATION_ABSENT", c17j), ("UNAUTHORIZED_DUPLICATE", c17j)], mutate=dir_link)
    negative("N25a", "the nonempty map holds 22 rows (an added row for C12's build sampler record)",
             [("MAP_ROW_COUNT", NM), ("MAP_ROW_NOT_RULED", bcg)], map_rows=nrows(add=dict(row(cg["C12"]),
                                                                                       current_path=bcg)))
    negative("N25b", "the nonempty map holds 20 rows (C18b's run sampler record row removed)",
             [("MAP_ROW_COUNT", NM), ("MAP_ROW_MISSING", cg["C18b"]), ("UNAUTHORIZED_DUPLICATE", cg["C18b"])],
             map_rows=nrows(cg["C18b"], drop=True))
    negative("N25c", "the nonempty map holds 22 rows (one row repeated)",
             [("MAP_ROW_COUNT", NM), ("MAP_ROW_REPEATED", st["1/out.txt"])], map_rows=nrows(add=row(st["1/out.txt"])))
    negative("N26a", "the zero-length map holds 9 rows (an added row for C28 03-build/build.stdout)",
             [("ZMAP_ROW_COUNT", ZM), ("ZMAP_ROW_NOT_RULED", c28bs)],
             zmap_rows=zrows_(add=dict([z for z in zrows if z["current_path"] == zb["C12"]][0], current_path=c28bs,
                                       control_id="C28")))
    negative("N26b", "the zero-length map holds 7 rows (C18b build.stdout row removed)",
             [("ZMAP_ROW_COUNT", ZM), ("ZMAP_ROW_MISSING", zb["C18b"]), ("ZERO_LENGTH_UNMAPPED", zb["C18b"])],
             zmap_rows=zrows_(zb["C18b"], drop=True))

    print("[S0: disclosed comparison, not a mission control: the base staging against PUBLISH-03's base staging]")
    old = os.path.join(Q.P3, "scratch/controls-04-REPAIR-CONTROLS/base")
    o, n = set(staged(old)), set(staged(base))
    diff = sorted(f for f in o & n if Q.sha256_file(os.path.join(old, f)) != Q.sha256_file(os.path.join(base, f)))
    print(f"       PUBLISH-03 base {len(o)} files, this base {len(n)}; only here {sorted(n - o)}; only there "
          f"{sorted(o - n)}; differing {diff}")
    results.append({"control": "S0", "title": "base staging vs PUBLISH-03's base staging (read-only)", "as_expected": None,
                    "detail": {"only_here": sorted(n - o), "only_there": sorted(o - n), "differing": diff}})

    mission = [x for x in results if x["as_expected"] is not None]
    bad = [x["control"] for x in mission if not x["as_expected"]]
    json.dump(results, open(os.path.join(out, "CONTROLS.json"), "w"), indent=1, default=list)
    print(f"controls {len(mission)}; as expected {len(mission) - len(bad)}; not as expected {bad}")
    print("REPAIR CONTROLS: " + ("ALL AS EXPECTED" if not bad else "NOT ALL AS EXPECTED: " + ", ".join(bad)))
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1])))
