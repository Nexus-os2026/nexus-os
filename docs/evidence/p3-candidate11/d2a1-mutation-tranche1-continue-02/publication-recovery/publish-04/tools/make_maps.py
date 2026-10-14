#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission sections 5, 6 and 7: derive the 21-row nonempty duplicate map,
the 8-row zero-length artifact map, and the Architect duplicate-policy R2 ruling record, from the CONTINUE-02 evidence
(read-only), the immutable EXEC-01 and RESUME-01 commits, the PUBLISH-03 R1 map (read-only) and the builder's ruled
tables (build_packet.NONEMPTY, build_packet.ZERO). Every field is derived from bytes; the tool refuses (exit 1, nothing
written) if any ruled current file is not byte-equal to its ruled prior, if any zero-length artifact is not a regular,
empty, non-symlink file, or if any link row differs from its R1 row.

    make_maps.py OUTDIR     writes OUTDIR/NONEMPTY-DUPLICATE-MAP.tsv and .md, OUTDIR/ZERO-LENGTH-ARTIFACT-MAP.tsv and
                            .md, and OUTDIR/ARCHITECT-RULING-DUPLICATE-POLICY-R2.md (each must not exist)
"""
import json
import os
import shlex
import stat
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import build_packet as B  # noqa: E402
import common as K  # noqa: E402
import pub4common as Q  # noqa: E402

RULING_FIRST = ("DUPLICATE POLICY R2 — 21 EXACT NONEMPTY DUPLICATES AND 8 EXACT ZERO-LENGTH ARTIFACTS PERMITTED FOR "
                "PUBLICATION (NO SIZE THRESHOLD; NO WILDCARD; CANDIDATE ACCEPTANCE PENDING)")
NAMES = ["NONEMPTY-DUPLICATE-MAP.tsv", "NONEMPTY-DUPLICATE-MAP.md", "ZERO-LENGTH-ARTIFACT-MAP.tsv",
         "ZERO-LENGTH-ARTIFACT-MAP.md", "ARCHITECT-RULING-DUPLICATE-POLICY-R2.md"]


def literal(data):
    return data.decode("utf-8").replace("\\", "\\\\").replace("\t", "\\t").replace("\r", "\\r").replace("\n", "\\n")


def prior_index():
    by_blob, by_sha = {}, {}
    for commit, packet in ((Q.EXEC01, Q.EXEC01_PACKET), (Q.RESUME01, Q.RESUME01_PACKET)):
        for e in Q.git("ls-tree", "-r", "-z", commit, packet + "/").split(b"\0"):
            if e:
                meta, path = e.split(b"\t", 1)
                by_blob.setdefault(meta.decode().split()[2], set()).add((commit, path.decode()))
        for line in Q.git("show", f"{commit}:{packet}/SHA256SUMS").decode().splitlines():
            h, rel = line.split("  ", 1)
            by_sha.setdefault(h, set()).add((commit, f"{packet}/{rel}"))
    return by_blob, by_sha


def src(rel):
    p = os.path.join(Q.SRC, rel)
    st = os.lstat(p)
    if not stat.S_ISREG(st.st_mode) or os.path.realpath(p) != p:
        raise SystemExit(f"REFUSED: {rel} is not a regular file reached without a symlink")
    return open(p, "rb").read()


def generator(rel):
    data = src(rel)
    return Q.sha256_bytes(data), Q.git_blob_id(data)


def derive():
    by_blob, by_sha = prior_index()
    r1 = {l.split("\t")[0]: dict(zip(B.R1_COLUMNS, l.split("\t"))) for l in
          open(B.R1_MAP).read().splitlines()[1:]}
    problems, rows, zrows = [], [], []
    for p, s in B.NONEMPTY.items():
        cur = src(p)
        pc, prel = s["prior"]
        prior_path = f"{Q.EXEC01_PACKET if pc == Q.EXEC01 else Q.RESUME01_PACKET}/{prel}"
        pb = Q.git("rev-parse", f"{pc}:{prior_path}").decode().strip()
        prior = Q.git("cat-file", "blob", pb)
        hits = by_blob.get(Q.git_blob_id(cur), set()) | by_sha.get(Q.sha256_bytes(cur), set())
        if prior != cur:
            problems.append(f"{p}: not byte-equal to {prior_path}")
        if (pc, prior_path) not in hits:
            problems.append(f"{p}: the ruled prior is not among its previous-packet equal files")
        gs, gb = generator(s["generator"])
        r = {"map_class": s["class"], "current_path": p, "control_id": s["control"], "selftest_id": s["selftest"],
             "current_role": s["role"], "size": str(len(cur)), "current_sha256": Q.sha256_bytes(cur),
             "current_git_blob": Q.git_blob_id(cur), "literal_utf8": literal(cur) if len(cur) <= B.LITERAL_MAX else "-",
             "prior_commit": pc, "prior_path": prior_path, "prior_role": s["prior_role"], "prior_size": str(len(prior)),
             "prior_sha256": Q.sha256_bytes(prior), "prior_git_blob": pb, "prior_equal_files": str(len(hits)),
             "byte_equal": "yes" if prior == cur else "no", "generator_path": s["generator"], "generator_git_blob": gb,
             "generator_sha256": gs, "generator_item": s["item"], "descriptor_path": s["descriptor"],
             "descriptor_sha256": Q.sha256_bytes(src(s["descriptor"]))}
        if s["class"] == "LINK_RECEIPT":
            cid, cfg, _ = B.RULED[p]
            d = B.RUN_DIRS[cid]
            r.update({"linked_configuration": cfg, "linked_baseline_commit": Q.STAGE2,
                      "linked_baseline_path": B.STAGE2_RECEIPT[cfg]})
            for k, h, v in (("restoration_receipt", "restoration_sha256", "05-restore/RESTORE.txt"),
                            ("control_result", "control_result_sha256", "CONTROL-RESULT.json"),
                            ("classification", "classification_sha256", "04-run/CLASSIFICATION.txt")):
                r[k] = f"{d}/{v}"
                r[h] = Q.sha256_bytes(src(f"{d}/{v}"))
            diff = [c for c in B.R1_COLUMNS if r1.get(p, {}).get(c) != r["control_id" if c == "control" else c]]
            if diff:
                problems.append(f"{p}: differs from its R1 row in {diff}")
        else:
            r.update({c: "-" for c in B.LINK_COLUMNS})
        rows.append(r)
    for p, s in B.ZERO.items():
        full = os.path.join(Q.SRC, p)
        st = os.lstat(full)
        if not stat.S_ISREG(st.st_mode) or os.path.realpath(full) != full or st.st_size != 0:
            problems.append(f"{p}: not a regular, empty file reached without a symlink")
            continue
        gs, gb = generator(s["generator"])
        z = {"current_path": p, "role": s["role"], "control_id": s["control"], "selftest_id": s["selftest"],
             "expected_command": s["command"], "file_type": "regular", "symlink": "no", "size": "0",
             "sha256": Q.sha256_bytes(src(p)), "git_blob": Q.git_blob_id(src(p)), "descriptor_path": s["descriptor"],
             "descriptor_sha256": Q.sha256_bytes(src(s["descriptor"])), "stderr_evidence": s["stderr"],
             "stderr_evidence_sha256": "-" if s["stderr"] == B.MERGED else Q.sha256_bytes(src(s["stderr"])),
             "result_evidence": s["result"], "result_evidence_sha256": Q.sha256_bytes(src(s["result"])),
             "generator_path": s["generator"], "generator_git_blob": gb, "generator_sha256": gs,
             "generator_item": s["item"]}
        if (z["sha256"], z["git_blob"]) != (Q.EMPTY_SHA, Q.EMPTY_BLOB):
            problems.append(f"{p}: not the empty blob")
        if s["control"] != "-":
            desc = json.loads(src(s["descriptor"]))
            if shlex.join(desc["argv"]) != s["command"]:
                problems.append(f"{p}: the descriptor's command is {desc['argv']}")
        zrows.append(z)
    return rows, zrows, problems


def tsv(path, columns, rows):
    with open(path, "w") as f:
        f.write("\t".join(columns) + "\n")
        for r in rows:
            vals = [r[c] for c in columns]
            if any("\t" in v or "\n" in v for v in vals):
                raise SystemExit(f"REFUSED: tab or newline in a field of {r['current_path']}")
            f.write("\t".join(vals) + "\n")


def short(path):
    for pre in (Q.EXEC01_PACKET, Q.RESUME01_PACKET):
        if path.startswith(pre + "/"):
            return ("EXEC-01" if pre == Q.EXEC01_PACKET else "RESUME-01") + " `" + path[len(pre) + 1:] + "`"
    return f"`{path}`"


def nonempty_md(rows):
    gen = {}
    for r in rows:
        gen[r["generator_path"]] = (r["generator_git_blob"], r["generator_sha256"])
    m = ["# Nonempty duplicate map — exactly 21 rows (Architect duplicate-policy ruling R2)", "",
         f"Mission {Q.MISSION}, section 6. Machine-readable form: `NONEMPTY-DUPLICATE-MAP.tsv` (one header line, 21 "
         "rows, tab-separated, 32 columns). The packet builder (`publish-04/tools/build_packet.py`: `NONEMPTY`, "
         "`dup_policy`) verifies every field of every row against the staged bytes, the immutable previous commits, "
         "the frozen generator and the binding receipts, and rejects every other nonempty staged file byte-equal to "
         "any previous-packet file, whatever its size.", "",
         "Current paths are packet-relative (`docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/`), the "
         "CONTINUE-02 receipt paths, not renamed. The canonical prior of each row is fixed by the ruling's table, "
         "never inferred from equal bytes: the same role in the earliest previous packet that holds it (link receipts: "
         "EXEC-01's link rehearsal of the same configuration; self-test records: the same self-test number and file; "
         "control-run sampler records: RESUME-01's C14 run, the first Architect-accepted control run).", "",
         "## Rows", "",
         "| # | class | current path | control / self-test | bytes | current sha256 / git blob | literal content (≤ 256 B) | canonical prior | prior blob | previous-packet files with these bytes |",
         "|---|---|---|---|---|---|---|---|---|---|"]
    for i, r in enumerate(rows, 1):
        who = r["control_id"] if r["control_id"] != "-" else "self-test " + r["selftest_id"]
        lit = f"`{r['literal_utf8']}`" if r["literal_utf8"] != "-" else "(link receipt, > 256 B)"
        m.append(f"| {i} | {r['map_class']} | `{r['current_path']}` | {who} | {r['size']} | `{r['current_sha256']}` / "
                 f"`{r['current_git_blob']}` | {lit} | {short(r['prior_path'])} | `{r['prior_git_blob']}` | "
                 f"{r['prior_equal_files']} |")
    m += ["", "## Roles, generators and binding receipts", "",
          "| # | current role | canonical prior role | generator item | binding descriptor (sha256) |",
          "|---|---|---|---|---|"]
    for i, r in enumerate(rows, 1):
        m.append(f"| {i} | {r['current_role']} | {r['prior_role']} | `{r['generator_path']}`: {r['generator_item']} | "
                 f"`{r['descriptor_path']}` (`{r['descriptor_sha256'][:16]}…`) |")
    m += ["", "Generators (CONTINUE-02 frozen tools; each sha256 equals the CONTINUE-02 freeze record):", ""]
    for g, (b, s) in sorted(gen.items()):
        where = ("staged in this packet (`tools/control.py`, changed by CONTINUE-02)" if g == "tools/control.py" else
                 "unchanged since RESUME-01, published at RESUME-01 `tools/supervise.py` (same blob)")
        m.append(f"- `{g}`: git blob `{b}`, sha256 `{s}`; {where}")
    m += ["", "## Why each class is byte-equal to earlier evidence", "",
          "- LINK_RECEIPT (7): the unchanged `control.link()` writes the same bytes whenever it links the same accepted "
          "Stage-2 receipt with the same baseline-executable hash (ruling R1, `publication-recovery/publish-03/"
          "documents/ARCHITECT-RULING-DUPLICATE-LINK-RECEIPTS.md`). Every R1 map field is carried and verified again.",
          "- CGROUP_CONSTANT (10): `supervise.sampler()` writes exactly the one line `sampler: the scope's cgroup was "
          "not found while it ran (a short run; scope-wrap.sh records it)` when the supervised process ends before the "
          "scope's cgroup can be sampled; the scope's enforced limits and peaks are recorded separately by "
          "`scope-wrap.sh` (`scope-record.txt`) and `/usr/bin/time -v` (`time-v.txt`), both present beside every row.",
          "- SELFTEST_CONSTANT (4): fixed outputs of fixed non-Nexus self-test commands (`supervise.selftest()`): case "
          "1 prints `allocated 10485760`; case 6's `os.abort()` makes GNU `timeout` print `timeout: the monitored "
          "command dumped core` on the merged stream; case 7 prints its exact environment names on stdout and `to "
          "stderr` on stderr. `SELFTEST.txt` of this mission records each case passing, its command line recording "
          "into the self-test's own directory, and case 7's two streams.", ""]
    return m


def zero_md(zrows):
    m = ["# Zero-length artifact map — exactly 8 rows (Architect duplicate-policy ruling R2)", "",
         f"Mission {Q.MISSION}, section 7. Machine-readable form: `ZERO-LENGTH-ARTIFACT-MAP.tsv` (one header line, 8 "
         "rows, tab-separated, 20 columns). The packet builder (`ZERO`, `dup_policy`) rejects every zero-length staged "
         "file that is not one of these exact paths, and accepts each of these only when every field of its row "
         "verifies: a regular file, not a symlink, size 0, sha256 "
         f"`{Q.EMPTY_SHA}`, git blob `{Q.EMPTY_BLOB}`, its exact role, control or self-test, expected command, "
         "binding descriptor, stderr and result evidence, and generator. Equality with an earlier empty file is not "
         "evidence: the provenance is the exact path, role and producing command or descriptor.", "",
         "| # | current path | role | control / self-test | expected command | descriptor (sha256) | stderr evidence (sha256) | result evidence (sha256) |",
         "|---|---|---|---|---|---|---|---|"]
    for i, z in enumerate(zrows, 1):
        who = z["control_id"] if z["control_id"] != "-" else "self-test " + z["selftest_id"]
        se = (f"`{z['stderr_evidence']}` (`{z['stderr_evidence_sha256'][:16]}…`)" if z["stderr_evidence"] != B.MERGED
              else "merged into this `out.txt` (no separate stream; `SELFTEST.txt`, `time-v.txt`)")
        m.append(f"| {i} | `{z['current_path']}` | {z['role']} | {who} | `{z['expected_command']}` | "
                 f"`{z['descriptor_path']}` (`{z['descriptor_sha256'][:16]}…`) | {se} | `{z['result_evidence']}` "
                 f"(`{z['result_evidence_sha256'][:16]}…`) |")
    m += ["", "Generator of every row: `tools/supervise.py` (git blob "
          f"`{zrows[0]['generator_git_blob']}`, sha256 `{zrows[0]['generator_sha256']}`), `supervise.supervised()` "
          "writing the command's stdout to the file: `supervise.build()` for the six mutant builds (cargo writes its "
          "diagnostics to `build.stderr`; each build descriptor records `stdout_sha256` = the empty sha256 and "
          "`process.exit_status` 0), `supervise.selftest()` cases 3 and 5 for the two self-test outputs (stderr merged "
          "into `out.txt`; case 3 is `/bin/sleep 30` killed by `timeout -s KILL 3`, case 5 exits 101 silently).", "",
          "Every other zero-length file is rejected. The one zero-length file of the PUBLISH-03 root outside its "
          "`scratch/` (`development/make-map-dev1-FAILED-sha-column-key.txt`) is therefore not staged; it remains in "
          "that root, recorded in its `receipts/90-ROOT-SHA256SUMS.txt` and in this mission's manifests of that root.",
          ""]
    return m


def ruling_md(rows, zrows):
    acc = Q.sha256_file(os.path.join(Q.SRC, "documents/ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md"))
    r1 = Q.sha256_file(os.path.join(Q.P3, "documents/ARCHITECT-RULING-DUPLICATE-LINK-RECEIPTS.md"))
    g = [RULING_FIRST, "", "# Architect ruling record — duplicate policy R2 (CONTINUE-02 publication)", "",
         f"Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission {Q.MISSION}, from "
         "sections 1, 6, 7 and 8 of that mission as issued by the Sole Nexus OS Architect (ChatGPT GPT-6) via the "
         "Owner.", "", "This file is evidence of the Architect's ruling.", "A string is not authority.", "",
         "## Background", "",
         f"- {Q.SRC_MISSION} completed every authorized control execution and stopped when its frozen packet builder "
         "rejected seven byte-identical link receipts (`publication/10-build-packet.txt`, `receipts/80-STOP-RECORD.md`).",
         f"- {Q.P3_MISSION} (ruling R1, sha256 `{r1}`) permitted exactly those seven link receipts. Its repaired "
         "builder removed the historical 1-KiB duplicate-content threshold, found fourteen further nonempty duplicate "
         "constant records, and correctly returned BLOCKED (its `receipts/80-STOP-RECORD.md`).", "",
         "## The ruling (mission section 1)", "",
         "> A. The old 1-KiB threshold is rejected. Every nonempty staged file must be checked for duplicate content, "
         "regardless of size.", ">",
         "> B. Exactly twenty-one nonempty duplicate files may be published: the seven previously approved link "
         "receipts; the fourteen deterministic constant records defined in section 6.", ">",
         "> C. Exactly eight expected zero-length artifacts may be published through the exact path-and-role policy in "
         "section 7. Zero-length artifacts are not accepted merely because their bytes equal another empty file. Their "
         "provenance comes from exact path, role and producing command/descriptor evidence.", ">",
         "> D. Every other duplicate or zero-length artifact remains fail-closed.", ">",
         "> E. No mutation control, build, test, explorer, witness or executable may be run.", ">",
         "> F. Candidate evidence remains pending independent Architect review: C28, C12, C9, C11, C17, C18a, C18b. Do "
         "not relabel any candidate as accepted.", "",
         "## 1. The seven link receipts (ruling R1, carried)", "",
         "| # | current path | control | canonical prior (EXEC-01 `19eda8cc`) |", "|---|---|---|---|"]
    for i, r in enumerate([r for r in rows if r["map_class"] == "LINK_RECEIPT"], 1):
        g.append(f"| {i} | `{r['current_path']}` | {r['control_id']} | {short(r['prior_path'])} |")
    g += ["", "Every R1 map field (`DUPLICATE-CONTENT-MAP.tsv`, 19 fields per row) is carried into the R2 map and "
          "verified again by the builder; the R1 map itself is read from the read-only PUBLISH-03 root and its sha256 "
          f"is pinned (`{B.R1_MAP_SHA}`).", "",
          "## 2. The fourteen deterministic constant records", "",
          "| # | class | current path | identity | bytes | literal content | canonical prior |",
          "|---|---|---|---|---|---|---|"]
    for i, r in enumerate([r for r in rows if r["map_class"] != "LINK_RECEIPT"], 1):
        who = f"control {r['control_id']}" if r["control_id"] != "-" else f"self-test {r['selftest_id']}"
        g.append(f"| {i} | {r['map_class']} | `{r['current_path']}` | {who} | {r['size']} | `{r['literal_utf8']}` | "
                 f"{short(r['prior_path'])} |")
    g += ["", "Each row's identity is its exact path in its own self-test or control-run directory, bound by that "
          "directory's descriptor (`SELFTEST.txt` for self-tests; `04-run/descriptor.json` naming the control, its "
          "worktree and its supervisor record for control runs) and its supervisor receipts (`scope-record.txt`, "
          "`time-v.txt`). No row is inferred from equal sha256 alone.", "",
          "## 3. The 1-KiB threshold is rejected", "",
          "The frozen CONTINUE-02 builder compared only files of 1 KiB or more. That threshold is rejected: every "
          "nonempty staged file is compared with every file of both previous packets (EXEC-01, RESUME-01), whatever "
          "its size. A duplicated run classification receipt (about 900 to 1000 bytes; RESUME-01 C14's is 937 bytes) "
          "is rejected; the repair controls show that the threshold, if reintroduced, would wrongly accept it.", "",
          "## 4. The eight zero-length artifacts", "",
          "| # | current path | role | expected command |", "|---|---|---|---|"]
    for i, z in enumerate(zrows, 1):
        g.append(f"| {i} | `{z['current_path']}` | {z['role']} | `{z['expected_command']}` |")
    g += ["", "Each is accepted only as that exact regular, non-symlink, empty file with its role, its producing "
          "command and its descriptor, stderr and result evidence (`ZERO-LENGTH-ARTIFACT-MAP.tsv`). Byte equality with "
          "an earlier empty file is not evidence.", "",
          "## 5. The exact policy (no wildcard)", "",
          "- Exactly the 21 nonempty paths and the 8 zero-length paths above; each map holds exactly that many rows, "
          "and a map that is a symlink is rejected.",
          "- No size threshold; no wildcard; no basename, directory-only, generic deterministic-file, generic `06-link`, "
          "generic `cgroup-samples`, generic `build.stdout` or generic empty-file exception.",
          "- Every other nonempty duplicate and every other zero-length file is rejected (fail closed).",
          "- Publication only: no existing raw receipt, classification, result, build output, execution output, "
          "restoration receipt or link receipt is edited.", "",
          "## Candidate status (unchanged by this ruling)", "",
          "- C28, C12, C9, C11, C17, C18a, C18b remain executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT "
          "REVIEW PENDING`), pending independent Architect review. This ruling accepts none of them.",
          "- Previously Architect-accepted (CONTINUE-02 `ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md`, sha256 "
          f"`{acc}`): C22, C14, C19, C21, C27.", "",
          "## No control was rerun", "",
          "No mutation control, baseline, build, test, explorer, witness or executable was run for this ruling or by "
          "this mission. Every receipt is CONTINUE-02's, byte for byte; this mission only reads, maps, checks and "
          "publishes it.", ""]
    return g


def main(out):
    Q.no_bytecode()
    if any(os.path.exists(os.path.join(out, n)) for n in NAMES):
        raise SystemExit("REFUSED: an output exists")
    rows, zrows, problems = derive()
    if (problems or len(rows) != 21 or len(zrows) != 8 or [r["current_path"] for r in rows] != list(B.NONEMPTY)
            or [z["current_path"] for z in zrows] != list(B.ZERO)):
        print("REFUSED: " + "; ".join(problems))
        return 1
    tsv(os.path.join(out, NAMES[0]), B.MAP_COLUMNS, rows)
    tsv(os.path.join(out, NAMES[2]), B.ZERO_COLUMNS, zrows)
    open(os.path.join(out, NAMES[1]), "w").write("\n".join(nonempty_md(rows)))
    open(os.path.join(out, NAMES[3]), "w").write("\n".join(zero_md(zrows)))
    open(os.path.join(out, NAMES[4]), "w").write("\n".join(ruling_md(rows, zrows)))
    for n in NAMES:
        print(f"wrote {n} sha256 {Q.sha256_file(os.path.join(out, n))}")
    cls = {}
    for r in rows:
        cls[r["map_class"]] = cls.get(r["map_class"], 0) + 1
    print(f"nonempty rows {len(rows)} {cls}; zero-length rows {len(zrows)}")
    print("MAPS: DERIVED")
    return 0


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1])))
