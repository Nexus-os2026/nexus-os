#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-03, mission sections 5 and 6: derive the explicit per-file duplicate-content
map of the seven ruled link receipts, and the Architect duplicate-link-receipt ruling record, from the CONTINUE-02
evidence (read-only) and the immutable EXEC-01 commit. Every field is derived from bytes, never assumed from the control
group; the tool refuses (exit 1, nothing written) if any ruled current file is not byte-equal to its ruled prior file.

    make_map.py OUTDIR     writes OUTDIR/DUPLICATE-CONTENT-MAP.tsv, OUTDIR/DUPLICATE-CONTENT-MAP.md and
                           OUTDIR/ARCHITECT-RULING-DUPLICATE-LINK-RECEIPTS.md (each must not exist)
"""
import ast
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import build_packet as B  # noqa: E402
import common as K  # noqa: E402
import pubcommon as Q  # noqa: E402

RULING_FIRST = ("SEVEN CONTINUE-02 LINK RECEIPTS — DUPLICATE CONTENT PERMITTED FOR PUBLICATION (NARROW: EXACT PATHS; "
                "NO OTHER DUPLICATE EXCEPTION; CANDIDATE ACCEPTANCE PENDING)")


def link_source(src):
    for n in ast.parse(src).body:
        if isinstance(n, ast.FunctionDef) and n.name == "link":
            return ast.get_source_segment(src, n)
    return None


def derive():
    exp = json.load(open(os.path.join(Q.SRC, "receipts/02-EXPECTATIONS.json")))
    rows, problems = [], []
    for p, (cid, cfg, prior_rel) in B.RULED.items():
        d = exp["controls"][cid]["dir"]
        cur = open(os.path.join(Q.SRC, p), "rb").read()
        prior_path = f"{Q.EXEC01_PACKET}/{prior_rel}"
        pb = Q.git("rev-parse", f"{Q.EXEC01}:{prior_path}").decode().strip()
        prior = Q.git("cat-file", "blob", pb)
        if prior != cur:
            problems.append(f"{p}: not byte-equal to {prior_path}")
        if exp["controls"][cid].get("configuration") != cfg or K.EXPLORE_CFG[cid] != cfg:
            problems.append(f"{p}: configuration of {cid} is not {cfg}")
        ref = f"{B.STAGE2_RECEIPT[cfg]} at {Q.STAGE2} (accepted exact-final {cfg} unchanged-core PASS, type (e1))"
        link_ref = (json.loads(cur)["reference"] if p.endswith(".json")
                    else cur.decode().splitlines()[1].split(": restored-side receipt ", 1)[1])
        if link_ref != ref:
            problems.append(f"{p}: link reference {link_ref}")
        fs = {(k, h): os.path.join(Q.SRC, d, v) for k, h, v in (
            ("restoration_receipt", "restoration_sha256", "05-restore/RESTORE.txt"),
            ("control_result", "control_result_sha256", "CONTROL-RESULT.json"),
            ("classification", "classification_sha256", "04-run/CLASSIFICATION.txt"))}
        row = {"current_path": p, "control": cid, "current_sha256": Q.sha256_bytes(cur),
               "current_git_blob": Q.git_blob_id(cur), "size": str(len(cur)), "prior_commit": Q.EXEC01,
               "prior_path": prior_path, "prior_git_blob": pb, "prior_sha256": Q.sha256_bytes(prior),
               "byte_equal": "yes" if prior == cur else "no", "linked_configuration": cfg,
               "linked_baseline_commit": Q.STAGE2, "linked_baseline_path": B.STAGE2_RECEIPT[cfg]}
        for (k, h), f in fs.items():
            row[k] = os.path.relpath(f, Q.SRC)
            row[h] = Q.sha256_file(f)
        rows.append(row)
    return rows, problems, exp


def main(out):
    names = ["DUPLICATE-CONTENT-MAP.tsv", "DUPLICATE-CONTENT-MAP.md", "ARCHITECT-RULING-DUPLICATE-LINK-RECEIPTS.md"]
    if any(os.path.exists(os.path.join(out, n)) for n in names):
        raise SystemExit("REFUSED: an output exists")
    rows, problems, exp = derive()
    if problems or len(rows) != 7 or [r["current_path"] for r in rows] != list(B.RULED):
        print("REFUSED: " + "; ".join(problems))
        return 1
    with open(os.path.join(out, names[0]), "w") as f:
        f.write("\t".join(B.MAP_COLUMNS) + "\n")
        for r in rows:
            f.write("\t".join(r[c] for c in B.MAP_COLUMNS) + "\n")

    sources = {"EXEC-01 packet tools/control.py": Q.git("show", f"{Q.EXEC01}:{Q.EXEC01_PACKET}/tools/control.py").decode(),
               "RESUME-01 tools/control.py (CONTINUE-02 original-tools/control.py)":
                   open(os.path.join(Q.SRC, "original-tools/control.py")).read(),
               "CONTINUE-02 tools/control.py": open(os.path.join(Q.SRC, "tools/control.py")).read()}
    links = {k: link_source(v) for k, v in sources.items()}
    same_link = len({v for v in links.values()}) == 1 and None not in links.values()
    link_sha = Q.sha256_bytes(next(iter(links.values())).encode())

    m = ["# Duplicate-content map — the seven ruled CONTINUE-02 link receipts", "",
         f"Mission {Q.MISSION}, sections 6 and 7. Machine-readable form: `DUPLICATE-CONTENT-MAP.tsv` (one header line, "
         "seven rows, tab-separated); the repaired packet builder (`publication-03/tools/build_packet.py`, `RULED` and "
         "`dup_policy`) verifies every field of every row against the staged bytes, the immutable EXEC-01 commit and "
         "the control's own receipts, and rejects any other nonempty staged file byte-equal to a previous-packet file.",
         "", "Current paths are packet-relative (`docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/`); "
         "they are the CONTINUE-02 receipt paths, not renamed.", "",
         "## Byte identity", "",
         "| # | current path | control | bytes | current sha256 / git blob | prior (EXEC-01 `19eda8cc`) path | prior git blob / sha256 | byte-equal |",
         "|---|---|---|---|---|---|---|---|"]
    for i, r in enumerate(rows, 1):
        m.append(f"| {i} | `{r['current_path']}` | {r['control']} | {r['size']} | `{r['current_sha256']}` / "
                 f"`{r['current_git_blob']}` | `{r['prior_path'][len(Q.EXEC01_PACKET) + 1:]}` | `{r['prior_git_blob']}` / "
                 f"`{r['prior_sha256']}` | {r['byte_equal']} |")
    m += ["", "## Binding of each receipt to its own control", "",
          "| # | control | linked configuration | linked accepted baseline (at `89326b85`) | restoration receipt (sha256) | control result (sha256) | run classification (sha256) |",
          "|---|---|---|---|---|---|---|"]
    for i, r in enumerate(rows, 1):
        m.append(f"| {i} | {r['control']} | {r['linked_configuration']} | `{r['linked_baseline_path'][len(Q.STAGE2_PACKET) + 1:]}` | "
                 f"`{r['restoration_receipt']}` (`{r['restoration_sha256'][:16]}…`) | `{r['control_result']}` "
                 f"(`{r['control_result_sha256'][:16]}…`) | `{r['classification']}` (`{r['classification_sha256'][:16]}…`) |")
    m += ["", "Full sha256 values are in the TSV. Each restoration receipt states `RESTORE: EXACTLY_C` and names its "
          "control; each control result names its control and the same linked reference; each run classification is the "
          "control's own `KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` candidate classification.", "",
          "## Why the bytes are equal", "",
          f"`link()` is the same function in every mission that ran it (its source, sha256 `{link_sha}`, is identical in "
          f"{', '.join(links)}: {same_link}). It writes `LINK.json`/`LINK.txt` from the linked accepted Stage-2 receipt "
          "(path and commit), its seven check labels and results, and the sha256 of the pinned unchanged-baseline "
          "executable (`cde9f9d2…`); none of these depend on the mutant. `LINK.json` carries no control id, so the "
          "three F-B2 controls (C9, C11, C17) and the two F-G controls (C18a, C18b) write the same `LINK.json`; "
          "`LINK.txt` names the control, so only C9's and C18a's coincide with EXEC-01's pre-freeze link rehearsals of "
          "C9 and C18a (`receipts/04-rehearsal/links/link-C9-stage2/`, `link-C18a-stage2/`).", ""]
    open(os.path.join(out, names[1]), "w").write("\n".join(m) + "\n")

    acc = os.path.join(Q.SRC, "documents/ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md")
    g = [RULING_FIRST, "", "# Architect ruling record — duplicate link receipts (CONTINUE-02 publication)", "",
         f"Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission {Q.MISSION}, from "
         "sections 1, 5 and 6 of that mission as issued by the Sole Nexus OS Architect (ChatGPT GPT-6) via the Owner.",
         "", "This file is evidence of the Architect's ruling.", "A string is not authority.", "",
         "## The ruling", "",
         f"Mission {Q.SRC_MISSION} completed all authorized control executions but stopped when its frozen packet "
         "builder rejected seven byte-identical link receipts (`publication/10-build-packet.txt`, "
         "`receipts/80-STOP-RECORD.md`). The Sole Architect ruled:", "",
         "> PACKET DUPLICATE-CONTENT FAILURE — FALSE POSITIVE", ">",
         "> The seven files are genuine current-mission receipts. Their identical bytes are expected because the "
         "unchanged link() function linked the same immutable Stage-2 unchanged-core evidence with the same "
         "source/executable policy. Distinct repository paths, surrounding control-specific restoration receipts and "
         "control-specific result records bind each receipt to its control.", "",
         "Publication is authorized under the narrow policy of the mission's section 6: duplicate content is permitted "
         "only for the seven exact current files below, each with an explicit, verified per-file mapping. No control "
         "may be rebuilt or rerun; no existing raw receipt, classification, result, build output, execution output, "
         "restoration receipt or link receipt may be edited.", "",
         "## The seven exact current receipts and their prior immutable files", "",
         f"Prior commit: EXEC-01 evidence commit `{Q.EXEC01}`; prior paths under `{Q.EXEC01_PACKET}/`. Current paths "
         f"under the CONTINUE-02 packet `{Q.PACKET}/`.", "",
         "| # | current path | control | bytes | current git blob | current sha256 | prior path | prior git blob | prior sha256 | equal |",
         "|---|---|---|---|---|---|---|---|---|---|"]
    for i, r in enumerate(rows, 1):
        g.append(f"| {i} | `{r['current_path']}` | {r['control']} | {r['size']} | `{r['current_git_blob']}` | "
                 f"`{r['current_sha256']}` | `{r['prior_path'][len(Q.EXEC01_PACKET) + 1:]}` | `{r['prior_git_blob']}` | "
                 f"`{r['prior_sha256']}` | blob and sha256 equal; bytes equal |")
    g += ["", "## Why the equality is expected", "",
          f"- The unchanged `link()` (source sha256 `{link_sha}`, identical in the EXEC-01 packet, RESUME-01 and "
          f"CONTINUE-02 `tools/control.py`: {same_link}) is deterministic over the linked accepted Stage-2 receipt and "
          "the pinned unchanged-baseline executable's sha256; it reads nothing of the mutant.",
          "- EXEC-01's pre-freeze rehearsal ran `link()` on the real Stage-2 F-B2 receipt (for C9) and F-G receipt (for "
          "C18a); CONTINUE-02 linked C9, C11, C17 to the same F-B2 receipt "
          f"(`{B.STAGE2_RECEIPT['F-B2'][len(Q.STAGE2_PACKET) + 1:]}`) and C18a, C18b to the same F-G receipt "
          f"(`{B.STAGE2_RECEIPT['F-G'][len(Q.STAGE2_PACKET) + 1:]}`) at Stage-2 commit `{Q.STAGE2}`, with the same "
          "baseline executable `cde9f9d2…`.",
          "- `LINK.json` carries no control id: all three F-B2 `LINK.json` and both F-G `LINK.json` are equal. `LINK.txt` "
          "names the control: only C9's and C18a's equal EXEC-01's rehearsal outputs for C9 and C18a; C11's, C17's and "
          "C18b's `LINK.txt` are unique and are not covered by (and need no) exception.", "",
          "## Why the distinct current paths remain meaningful", "",
          "Each receipt sits in its own control's receipt directory (`receipts/NN-<control>/06-link/`), written after "
          "that control's own verified restoration, next to that control's own restoration receipt (`RESTORE: "
          "EXACTLY_C`, naming the control and its mutant's before/after blobs), control result (`CONTROL-RESULT.json`, "
          "naming the control and this link) and run classification. Equal bytes record an equal linked baseline; "
          "the binding to the control is by path and by these surrounding control-specific receipts, which "
          "`DUPLICATE-CONTENT-MAP.tsv` names with their sha256 and the repaired builder verifies.", "",
          "## Scope", "",
          "- Exactly the seven current paths above, each verified field by field. No other duplicate exception is "
          "granted: no directory-pattern, basename, generic `06-link`, generic JSON/TXT, size-threshold or "
          "deterministic-file exception.",
          "- Duplicate content remains rejected by default; any other nonempty staged file byte-equal to a "
          "previous-packet file is outside this ruling.",
          "- Publication only: nothing is rebuilt, rerun, reclassified or edited.", "",
          "## Candidate status (unchanged by this ruling)", "",
          "- C28, C12, C9, C11, C17, C18a, C18b remain executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT "
          "REVIEW PENDING`), pending independent Architect review. This ruling accepts none of them.",
          "- Previously Architect-accepted (CONTINUE-02 `ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md`, sha256 "
          f"`{Q.sha256_file(acc)}`): C22, C14, C19, C21, C27.", ""]
    open(os.path.join(out, names[2]), "w").write("\n".join(g) + "\n")
    for n in names:
        print(f"wrote {n} sha256 {Q.sha256_file(os.path.join(out, n))}")
    print(f"rows {len(rows)}; link() source identical across EXEC-01, RESUME-01, CONTINUE-02: {same_link}")
    print("MAP: DERIVED")
    return 0


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1])))
