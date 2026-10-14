#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-03, mission section 8: static controls of the repaired duplicate-content
policy, before any packet is created in the repository. Nothing is built, run or executed but Python and read-only git.

    repair_controls.py OUTDIR      OUTDIR must not exist; writes OUTDIR/CONTROLS.txt, OUTDIR/CONTROLS.json, every map
                                   variant (maps/*.tsv), and the builder run of control 3 (P3-builder-run.txt)

Control 3 runs the repaired builder exactly as published (its CLI) on the CONTINUE-02 evidence into a scratch directory.
Every other control applies the builder's own dup_policy() to a scratch copy of that staged packet, after one synthetic
change (the CONTINUE-02 evidence root is never touched). A negative control holds when its expected problem codes appear
among the problems that the change adds to the unchanged copy's problems (differential), and again when the staged set
excludes every file the unchanged copy already rejects (absolute). Control 11 holds when an over-broad "every 06-link
duplicate" policy (implemented here, never in the builder) wrongly accepts an unauthorized duplicate that dup_policy
rejects. D1 is a disclosed diagnostic, not a mission control.
"""
import json
import os
import re
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import build_packet as B  # noqa: E402
import pubcommon as Q  # noqa: E402

RES = Q.RESUME01_PACKET + "/receipts/32-C14/"
results = []


def staged(dest):
    return [os.path.relpath(f, dest) for f in B.files_under(dest)]


def load_map():
    ls = open(B.MAP).read().splitlines()
    return ls[0], [dict(zip(B.MAP_COLUMNS, l.split("\t"))) for l in ls[1:]]


def write_map(path, header, rows):
    with open(path, "w") as f:
        f.write(header + "\n")
        for r in rows:
            f.write("\t".join(r[c] for c in B.MAP_COLUMNS) + "\n")


def keys(problems):
    return {(c, p) for c, p, _ in problems}


def run_policy(dest, map_path, allf=None):
    import contextlib
    import io
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        acc, probs = B.dup_policy(dest, staged(dest) if allf is None else allf, map_path)
    return acc, probs, buf.getvalue().strip()


def prior_bytes(path):
    return Q.git("cat-file", "blob", Q.git("rev-parse", f"{Q.RESUME01 if path.startswith(Q.RESUME01_PACKET) else Q.EXEC01}:{path}").decode().strip())


def record(cid, title, ok, detail):
    results.append({"control": cid, "title": title, "as_expected": bool(ok), "detail": detail})
    print(f"{'AS EXPECTED    ' if ok else 'NOT AS EXPECTED'} {cid} {title}")
    for k, v in detail.items():
        print(f"       {k}: {v}")


def main(out):
    if os.path.exists(out):
        raise SystemExit("REFUSED: output exists")
    os.makedirs(os.path.join(out, "maps"))
    scratch = os.path.join(Q.P, "scratch", "controls-" + os.path.basename(out))
    if os.path.exists(scratch):
        raise SystemExit("REFUSED: scratch exists")
    os.makedirs(scratch)
    header, rows = load_map()
    print(f"map {B.MAP} sha256 {Q.sha256_file(B.MAP)}; builder {B.__file__} sha256 {Q.sha256_file(B.__file__)}")

    # Control 3 first: it stages the base copy every other control starts from.
    base = os.path.join(scratch, "base")
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    r = subprocess.run([sys.executable, "-B", os.path.join(Q.P, "tools/build_packet.py"), base], capture_output=True,
                       text=True, env=env)
    open(os.path.join(out, "P3-builder-run.txt"), "w").write(r.stdout + r.stderr + f"\nexit {r.returncode}\n")
    last = r.stdout.strip().splitlines()[-1] if r.stdout.strip() else ""
    acc0, p0, _ = run_policy(base, B.MAP)
    k0 = keys(p0)
    rejected0 = sorted({p for _, p in k0})
    clean = [f for f in staged(base) if f not in rejected0]
    sizes0 = {p: os.path.getsize(os.path.join(base, p)) for p in rejected0 if os.path.isfile(os.path.join(base, p))}

    def variant(name, mutate=None, map_rows=None):
        d = os.path.join(scratch, name)
        shutil.copytree(base, d, symlinks=True)
        if mutate:
            mutate(d)
        mp = B.MAP
        if map_rows is not None:
            mp = os.path.join(out, "maps", name + ".tsv")
            write_map(mp, header, map_rows)
        acc, probs, info = run_policy(d, mp)
        accc, probsc, _ = run_policy(d, mp, [f for f in staged(d) if f not in rejected0])
        return acc, probs, accc, probsc, info

    def negative(cid, title, name, expect, mutate=None, map_rows=None):
        acc, probs, accc, probsc, info = variant(name, mutate, map_rows)
        new = sorted(keys(probs) - k0)
        codes = {c for c, _ in new}
        ok = set(expect) <= set(new) and bool(probsc) and set(expect) <= keys(probsc)
        record(cid, title, ok, {"variant": name, "expected (code, path)": sorted(expect),
                                "added problems (differential)": new,
                                "absolute (staged set without the base's rejections)": sorted(keys(probsc)),
                                "verdict": "REJECTED" if probs else "ACCEPTED", "codes": sorted(codes)})
        return acc, probs

    def put(rel, data):
        def f(d):
            p = os.path.join(d, rel)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            if os.path.lexists(p):
                os.remove(p)
            open(p, "wb").write(data)
        return f

    def rows_with(path, **chg):
        out_rows = []
        for r0 in rows:
            r1 = dict(r0)
            if r1["current_path"] == path:
                r1.update(chg)
            out_rows.append(r1)
        return out_rows

    c9j, c9t, c11j, c17j = ("receipts/38-C9/06-link/LINK.json", "receipts/38-C9/06-link/LINK.txt",
                            "receipts/39-C11/06-link/LINK.json", "receipts/40-C17/06-link/LINK.json")
    c18aj, c18at, c18bj = ("receipts/41-C18a/06-link/LINK.json", "receipts/41-C18a/06-link/LINK.txt",
                           "receipts/42-C18b/06-link/LINK.json")
    fb2 = open(os.path.join(base, c9j), "rb").read()
    fg = open(os.path.join(base, c18aj), "rb").read()

    print("[positive controls]")
    record("P1", "all seven exact mapped duplicates accepted", acc0 == sorted(B.RULED) and not [
        x for x in k0 if x[1] in B.RULED] and run_policy(base, B.MAP, clean)[1] == [],
           {"accepted": acc0, "problems on a ruled path": sorted(x for x in k0 if x[1] in B.RULED),
            "absolute (staged set without the base's rejections): problems": run_policy(base, B.MAP, clean)[1]})
    sums = [l.split("  ", 1) for l in open(os.path.join(base, "SHA256SUMS")).read().splitlines()]
    listed = {}
    for h, p in sums:
        listed.setdefault(h, []).append(p)
    groups = {h: sorted(p for p in ps if p in B.RULED) for h, ps in listed.items() if set(ps) & set(B.RULED)}
    per_path = {p: sum(1 for _, q in sums if q == p) for p in B.RULED}
    record("P2", "each current control path is distinct in the packet manifest despite equal content",
           all(v == 1 for v in per_path.values()) and sorted(len(v) for v in groups.values()) == [1, 1, 2, 3]
           and all(h == r0["current_sha256"] for r0 in rows for h, ps in groups.items() if r0["current_path"] in ps),
           {"SHA256SUMS lines per ruled path": per_path, "ruled paths grouped by sha256": groups})
    record("P3", "the existing CONTINUE-02 evidence builds successfully with the seven-path allowlist (repaired "
           "builder CLI)", r.returncode == 0 and last == "PACKET: STAGED — ALL CHECKS HOLD",
           {"exit": r.returncode, "last line": last, "receipt": "P3-builder-run.txt",
            "problems (code, path, bytes)": [(c, p, sizes0.get(p)) for c, p in sorted(k0)]})

    print("[negative controls]")
    negative("N4", "remove one mapping (C11 LINK.json row)", "N4-remove-mapping",
             [("MAP_ROW_MISSING", c11j), ("UNAUTHORIZED_DUPLICATE", c11j)],
             map_rows=[r0 for r0 in rows if r0["current_path"] != c11j])
    eighth = "receipts/37-C12/06-link/LINK-F-B2.json"
    negative("N5a", "add an eighth duplicate (an F-B2 link copy added under C12)", "N5a-eighth-duplicate",
             [("UNAUTHORIZED_DUPLICATE", eighth)], mutate=put(eighth, fb2))
    r8 = dict(rows[0], current_path=eighth, control="C12")
    negative("N5b", "add an eighth duplicate and an eighth map row for it", "N5b-eighth-duplicate-and-row",
             [("MAP_ROW_NOT_RULED", eighth), ("UNAUTHORIZED_DUPLICATE", eighth)], mutate=put(eighth, fb2),
             map_rows=rows + [r8])
    moved = "receipts/37-C12/06-link/LINK.json"

    def move(d):
        os.replace(os.path.join(d, c9j), os.path.join(d, moved))
    negative("N6a", "move an allowed duplicate (C9 LINK.json) into another control's directory (C12)",
             "N6a-move", [("CURRENT_ABSENT", c9j), ("UNAUTHORIZED_DUPLICATE", moved),
                          ("RULED_PATH_NOT_A_DUPLICATE", c9j)], mutate=move)
    negative("N6b", "move it and point its map row at the new path", "N6b-move-and-row",
             [("MAP_ROW_NOT_RULED", moved), ("MAP_ROW_MISSING", c9j), ("UNAUTHORIZED_DUPLICATE", moved)],
             mutate=move, map_rows=rows_with(c9j, current_path=moved))
    flipped = fb2.replace(b"cde9f9d20f32", b"cde9f9d20f33", 1)
    negative("N7a", "change one byte in a mapped file (C17 LINK.json), map unchanged", "N7a-byte",
             [("CURRENT_BYTES_CHANGED", c17j), ("NOT_BYTE_EQUAL", c17j), ("PRIOR_SOURCE_MISMATCH", c17j)],
             mutate=put(c17j, flipped))
    negative("N7b", "change one byte and make the map row agree with the changed bytes (similar, not byte-equal)",
             "N7b-similar", [("NOT_BYTE_EQUAL", c17j), ("RULED_PATH_NOT_A_DUPLICATE", c17j)],
             mutate=put(c17j, flipped), map_rows=rows_with(
                 c17j, current_sha256=Q.sha256_bytes(flipped), current_git_blob=Q.git_blob_id(flipped)))
    wrong = Q.EXEC01_PACKET + "/receipts/04-rehearsal/links/link-C18a-stage2/LINK.json"
    negative("N8", "point C9 LINK.json's row at the wrong prior path (the F-G rehearsal link)", "N8-prior-path",
             [("PRIOR_PATH_MISMATCH", c9j)], map_rows=rows_with(c9j, prior_path=wrong))
    good = rows[0]["prior_sha256"]
    negative("N9", "correct prior path with a wrong prior sha256 (C9 LINK.json row)", "N9-prior-sha256",
             [("PRIOR_SHA256_MISMATCH", c9j)],
             map_rows=rows_with(c9j, prior_sha256=good[:-1] + ("0" if good[-1] != "0" else "1")))
    for sub, rel, src in (("a", "receipts/37-C12/03-build/RECEIPT.txt", RES + "03-build/RECEIPT.txt"),
                          ("b", "receipts/37-C12/04-run/RECEIPT.txt", RES + "04-run/RECEIPT.txt"),
                          ("c", "receipts/37-C12/04-run/CLASSIFICATION.txt", RES + "04-run/CLASSIFICATION.txt"),
                          ("d", "receipts/37-C12/05-restore/RESTORE.txt", RES + "05-restore/RESTORE.txt")):
        data = prior_bytes(src)
        negative(f"N10{sub}", f"duplicate a {['build', 'run', 'classification', 'restoration'][ord(sub) - 97]} "
                 f"receipt: C12's {rel.split('/', 2)[2]} replaced by RESUME-01 C14's ({len(data)} bytes)",
                 f"N10{sub}", [("UNAUTHORIZED_DUPLICATE", rel)], mutate=put(rel, data))

    print("[control 11: an over-broad policy would wrongly accept]")

    def overbroad(dest):
        """NEGATIVE-CONTROL IMPLEMENTATION ONLY (never in the builder): accept every duplicate under any 06-link/."""
        acc, probs, _ = run_policy(dest, B.MAP)
        return sorted(p for c, p, _ in probs if c == "UNAUTHORIZED_DUPLICATE" and re.fullmatch(
            r"receipts/[^/]+/06-link/[^/]+", p)), probs
    for name, mut, unauthorized in (("N11a-overbroad-eighth", put(eighth, fb2), eighth),
                                    ("N11b-overbroad-moved", move, moved)):
        d = os.path.join(scratch, name)
        shutil.copytree(base, d, symlinks=True)
        mut(d)
        wrongly, probs = overbroad(d)
        record(name.split("-")[0], f"a generic 06-link exception would accept the unauthorized {unauthorized}; the "
               "repaired policy rejects it", unauthorized in wrongly and ("UNAUTHORIZED_DUPLICATE", unauthorized) in
               keys(probs), {"over-broad policy would accept": wrongly,
                             "repaired policy rejects": ("UNAUTHORIZED_DUPLICATE", unauthorized) in keys(probs)})

    print("[negative controls, continued]")

    def rm(rel):
        return lambda d: os.remove(os.path.join(d, rel))
    negative("N12a", "missing current restoration receipt (C9 05-restore/RESTORE.txt)", "N12a",
             [("RESTORATION_ABSENT", c9j), ("RESTORATION_ABSENT", c9t)], mutate=rm("receipts/38-C9/05-restore/RESTORE.txt"))
    negative("N12b", "missing current restoration record (C18b 05-restore/RESTORE.json)", "N12b",
             [("RESTORATION_ABSENT", c18bj)], mutate=rm("receipts/42-C18b/05-restore/RESTORE.json"))
    negative("N13a", "missing current control result (C11 CONTROL-RESULT.json)", "N13a",
             [("RESULT_ABSENT", c11j)], mutate=rm("receipts/39-C11/CONTROL-RESULT.json"))
    negative("N13b", "missing current run classification (C17 04-run/CLASSIFICATION.txt)", "N13b",
             [("RESULT_ABSENT", c17j)], mutate=rm("receipts/40-C17/04-run/CLASSIFICATION.txt"))
    negative("N13c", "missing current RESULT.txt (C18a)", "N13c",
             [("RESULT_ABSENT", c18aj), ("RESULT_ABSENT", c18at)], mutate=rm("receipts/41-C18a/RESULT.txt"))
    fg_prior = Q.EXEC01_PACKET + "/receipts/04-rehearsal/links/link-C18a-stage2/LINK.json"
    r18 = [r0 for r0 in rows if r0["current_path"] == c18aj][0]
    negative("N14a", "C9 LINK.json replaced by the F-G link (a different configuration), the map row made to agree "
             "with the bytes and the F-G prior", "N14a",
             [("CONFIG_MISMATCH", c9j), ("BASELINE_MISMATCH", c9j), ("PRIOR_PATH_MISMATCH", c9j)],
             mutate=put(c9j, fg), map_rows=rows_with(
                 c9j, current_sha256=r18["current_sha256"], current_git_blob=r18["current_git_blob"], size=r18["size"],
                 prior_path=fg_prior, prior_git_blob=r18["prior_git_blob"], prior_sha256=r18["prior_sha256"],
                 linked_configuration="F-G", linked_baseline_path=r18["linked_baseline_path"]))
    negative("N14b", "the map row claims a different linked configuration (C11 F-G)", "N14b",
             [("CONFIG_MISMATCH", c11j)], map_rows=rows_with(c11j, linked_configuration="F-G"))
    relabel = fb2.replace(b"this control's run (F-B2)", b"this control's run (F-G)", 1)
    negative("N14c", "C17 LINK.json's configuration check relabelled F-G (map current fields agree)", "N14c",
             [("CONFIG_MISMATCH", c17j), ("NOT_BYTE_EQUAL", c17j)], mutate=put(c17j, relabel), map_rows=rows_with(
                 c17j, current_sha256=Q.sha256_bytes(relabel), current_git_blob=Q.git_blob_id(relabel),
                 size=str(len(relabel))))
    trav = "receipts/38-C9/06-link/../06-link/LINK.json"
    negative("N15a", "path traversal in the map (C9 LINK.json row as .../06-link/../06-link/LINK.json)", "N15a",
             [("MAP_ROW_NOT_RULED", trav), ("MAP_ROW_MISSING", c9j), ("UNAUTHORIZED_DUPLICATE", c9j)],
             map_rows=rows_with(c9j, current_path=trav))

    def link_to(rel, target):
        def f(d):
            os.remove(os.path.join(d, rel))
            os.symlink(target, os.path.join(d, rel))
        return f
    negative("N15b", "symlink substitution: C9 LINK.json replaced by a symlink to C11's (same bytes)", "N15b",
             [("CURRENT_SYMLINK", c9j), ("STAGED_SYMLINK", c9j)], mutate=link_to(c9j, "../../39-C11/06-link/LINK.json"))

    def dir_link(d):
        shutil.rmtree(os.path.join(d, "receipts/40-C17/05-restore"))
        os.symlink("../39-C11/05-restore", os.path.join(d, "receipts/40-C17/05-restore"))
    negative("N15c", "symlink substitution of a control's restoration directory (C17 05-restore -> C11's)", "N15c",
             [("RESTORATION_ABSENT", c17j)], mutate=dir_link)
    outside = os.path.join(scratch, "outside-LINK.txt")
    open(outside, "wb").write(open(os.path.join(base, c18at), "rb").read())
    negative("N15d", "symlink substitution to a byte-equal file outside the packet (C18a LINK.txt)", "N15d",
             [("CURRENT_SYMLINK", c18at), ("STAGED_SYMLINK", c18at)], mutate=link_to(c18at, outside))

    print("[D1: disclosed diagnostic, not a mission control: the frozen builder's 1 KiB floor, if it were retained]")
    floor = [x for x in p0 if not (x[0] == "UNAUTHORIZED_DUPLICATE" and sizes0.get(x[1], 0) < 1024)]
    cl = "receipts/37-C12/04-run/CLASSIFICATION.txt"
    d = os.path.join(scratch, "D1-floor-classification")
    shutil.copytree(base, d, symlinks=True)
    put(cl, prior_bytes(RES + "04-run/CLASSIFICATION.txt"))(d)
    _, pd, _ = run_policy(d, B.MAP)
    floor_d = [x for x in pd if not (x[0] == "UNAUTHORIZED_DUPLICATE" and os.path.getsize(os.path.join(d, x[1])) < 1024)]
    print("       with a 1 KiB floor, the existing evidence would have no problem:", floor == [], f"({len(p0)} -> {len(floor)})")
    print(f"       with a 1 KiB floor, C12's run classification replaced by RESUME-01 C14's ({os.path.getsize(os.path.join(d, cl))} "
          f"bytes) would be rejected: {bool(floor_d)} (the repaired, floor-free policy rejects it: "
          f"{('UNAUTHORIZED_DUPLICATE', cl) in keys(pd)})")
    results.append({"control": "D1", "title": "diagnostic: the frozen 1 KiB floor", "as_expected": None,
                    "detail": {"existing evidence problems with floor": floor,
                               "sub-1-KiB classification duplicate rejected with floor": bool(floor_d),
                               "rejected without floor": ("UNAUTHORIZED_DUPLICATE", cl) in keys(pd)}})

    print("[S0: the base staging against the CONTINUE-02 failed staging (old worktree, read-only)]")
    old = os.path.join(Q.OLD_WORKTREE, Q.PACKET)
    o, n = set(staged(old)), set(staged(base))
    diff = sorted(f for f in o & n if Q.sha256_file(os.path.join(old, f)) != Q.sha256_file(os.path.join(base, f)))
    print(f"       old {len(o)} files, new {len(n)}; only new {sorted(n - o)}; only old {sorted(o - n)}; differing {diff}")
    results.append({"control": "S0", "title": "base staging vs the CONTINUE-02 failed staging", "as_expected": None,
                    "detail": {"only_new": sorted(n - o), "only_old": sorted(o - n), "differing": diff}})

    mission = [x for x in results if x["as_expected"] is not None]
    bad = [x["control"] for x in mission if not x["as_expected"]]
    json.dump(results, open(os.path.join(out, "CONTROLS.json"), "w"), indent=1, default=list)
    print(f"controls {len(mission)}; as expected {len(mission) - len(bad)}; not as expected {bad}")
    print("REPAIR CONTROLS: " + ("ALL AS EXPECTED" if not bad else "NOT ALL AS EXPECTED: " + ", ".join(bad)))
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1])))
