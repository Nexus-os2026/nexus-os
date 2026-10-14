#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01, mission section 12: write ROOT/docs-src/INDEX.md from the R2
catalog, the R1 catalog, the rendered definitions record and the receipts (deterministic; read-only for the
repository).

    render_index.py ROOT
"""
import hashlib
import json
import os
import re
import subprocess
import sys

PKT = "docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01"
R1_PKT = "docs/evidence/p3-candidate11/d2a1-control-definitions-r1-01"


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def blob(path):
    return subprocess.run(["git", "hash-object", path], capture_output=True, text=True, check=True).stdout.strip()


def count(path, rx):
    m = re.search(rx, open(path).read(), re.M)
    return m.group(1) if m else "?"


def main(root):
    rec = os.path.join(root, "receipts")
    catp = os.path.join(root, "out", "catalog", "catalog.json")
    defp = os.path.join(root, "out", "docs", "ARCHITECT-CONTROL-DEFINITIONS-R2.md")
    a6p = os.path.join(root, "out", "catalog", "patches", "A6.patch")
    cat = json.load(open(catp))
    r1 = {e["id"]: e for e in json.load(open(os.path.join(root, "inputs", "r1", "catalog.json")))["controls"]}
    kill = [e for e in cat["controls"] if e["id"] != "A4"]
    a4 = next(e for e in cat["controls"] if e["id"] == "A4")
    a6 = next(e for e in kill if e["id"] == "A6")
    rc = a6["r2_correction"]
    sup = cat["supersedes_as_candidate"]
    auth_ok = sum(1 for l in open(os.path.join(rec, "00-authentication.txt")) if l.startswith("    OK "))
    neg = os.path.join(rec, "40-negative-controls.txt")
    o = ["# D2A-1 control definitions R2 — index", "",
         "Mission P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01. Executor: Claude Code, Claude Opus 5.5 "
         "(`claude-opus-5-5`), effort High. For the Sole Architect's review.", "",
         "**Status: READY_FOR_CONTROL_DEFINITION_R2_ARCHITECT_REVIEW.**", "",
         "Static definition work only. No Cargo, no rustc, no build, no test, no explorer; no patch applied to any "
         "worktree; no mutant built or run. Candidate C is unchanged; the R1 packet is unchanged. The catalog is a "
         "**candidate**: every control's definition is CANDIDATE_READY, its Architect review pending, its execution "
         "NOT_ATTEMPTED. No kill is claimed. D2A-1 remains unaccepted; Candidate 11 is not integrated; Phase Three "
         "remains active.", "",
         "R2 repairs the one blocking defect the Architect's review of R1 found: A6 added `recovery_required` to the "
         "state key but left `KEY_FIELDS`, the field list every exploration report prints, unchanged. The R2 A6 patch "
         "is the R1 A6 patch with every semantic hunk kept byte for byte and exactly one receipt-metadata hunk added "
         "in `KEY_FIELDS`. The other 34 patches and their catalog entries are R1's, byte for byte. The definitions "
         "record carries R1's forward: **ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW "
         "PENDING** (`ARCHITECT-CONTROL-DEFINITIONS-R2.md`).", "",
         "## Identities", "",
         "| Item | Value |", "|---|---|",
         f"| Candidate C | `{cat['baseline']['commit']}`, tree `{cat['baseline']['tree']}`, single parent "
         "`71264aa81a40b86189783facc8437e0427887c16` |",
         "| Blobs at C | " + "; ".join(f"`{p.rsplit('/', 1)[1]}` `{b}`" for p, b in cat["baseline"]["blobs"].items()) +
         " |",
         f"| R1 evidence commit (this commit's parent) | `{sup['evidence_commit']}`, tree "
         "`234ca0990434485f59ebda0ceff3086d4e66d797`, parent `59df21036f7c0183fa17d76b6e0886f8f5784685`, ref "
         f"`{sup['review_ref']}` |",
         f"| R1 catalog (immutable review history) | `{R1_PKT}/catalog/catalog.json`, `{sup['version']}`, git blob "
         f"`{sup['git_blob']}`, sha256 `{sup['sha256']}` |",
         f"| R1 definitions record | `{R1_PKT}/ARCHITECT-CONTROL-DEFINITIONS-R1.md`, git blob "
         "`b947621bb0d56e1e14780853e886c7e832d7f6a3`, sha256 "
         "`beb9f5e3129bbd559dfa375251bddeac17f78d3f1b79082a42767944d7d48bf0` |",
         f"| **R2 candidate catalog** | `{PKT}/catalog/catalog.json`, version `{cat['version']}`, git blob "
         f"`{blob(catp)}`, catalog sha256 `{sha(catp)}` |",
         f"| **R2 definitions record** | `{PKT}/ARCHITECT-CONTROL-DEFINITIONS-R2.md`, git blob `{blob(defp)}`, "
         f"definitions record sha256 `{sha(defp)}` |",
         f"| **R2 A6 patch** | `{PKT}/catalog/patches/A6.patch`, git blob `{blob(a6p)}`, sha256 `{a6['patch_sha256']}`; "
         f"mutant blob `{a6['mutant_blob']}`, mutant sha256 `{a6['mutant_sha256']}` |",
         "| Contract | sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c` |",
         "| AD-1-R4 | sha256 `00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693` |",
         "| AD-1-R5 | sha256 `4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473` |", "",
         "## The A6 correction", "",
         f"- R1 A6: patch sha256 `{rc['r1_patch_sha256']}`, mutant blob `{rc['r1_mutant_blob']}`, "
         f"{rc['r1_hunks_preserved_byte_for_byte']} hunks, {rc['semantic_edits_unchanged_from_r1']} edits, lines "
         f"removed/added {rc['r1_patch_lines_removed_added'][0]}/{rc['r1_patch_lines_removed_added'][1]}.",
         f"- R2 A6: patch sha256 `{a6['patch_sha256']}`, mutant blob `{a6['mutant_blob']}`, "
         f"{rc['r1_hunks_preserved_byte_for_byte'] + rc['hunks_added']} hunks (R1's "
         f"{rc['r1_hunks_preserved_byte_for_byte']} byte for byte + 1), {len(a6['edits'])} edits (R1's "
         f"{rc['semantic_edits_unchanged_from_r1']} + 1), lines removed/added "
         f"{a6['patch_lines_removed_added'][0]}/{a6['patch_lines_removed_added'][1]}.",
         f"- The added hunk, in `KEY_FIELDS` (model_tests.rs line {rc['anchor_first_line_at_C']} at C): "
         f"`{rc['old_phrase']}` → `{rc['new_phrase']}`; nothing else. It is receipt metadata, not a second semantic "
         "mutation: the R1 mutant and the R2 mutant differ in that one line.",
         "- Exact differences: `comparison/A6-R1-TO-R2-PATCH.diff`, `comparison/A6-R1-TO-R2-MUTANT.diff`.", "",
         "## Results", "",
         "| Step | Result | Record |", "|---|---|---|",
         f"| Input authentication (Candidate C, the R1 packet, sources, refs) | ALL CHECKS OK ({auth_ok} checks) | "
         "`receipts/00-authentication.txt` |",
         f"| R2 candidate catalog | {cat['counts']['kill_required']} kill-required, all CANDIDATE_READY, "
         f"{cat['counts']['blocked']} blocked; A4 separate; changed from R1: "
         f"{', '.join(cat['r2']['changed_controls'])} only | `catalog/` |",
         "| R1 → R2 equality | 34 patches byte-identical and 34 entries equal; A6 the only difference | "
         "`comparison/R1-TO-R2-PATCH-COMPARISON.md`, `receipts/20`, `21` |",
         f"| Checker A (bytes/text; GNU patch, git apply; R1 from the authenticated copies) | "
         f"{count(os.path.join(rec, '20-checker-a.txt'), r'^CHECKER A: (\w+)')}, "
         f"{count(os.path.join(rec, '20-checker-a.txt'), r'^checks (\d+);')} checks | `receipts/20-checker-a.txt` |",
         f"| Checker B (independent: strict diff engine, tokenizer; R1 from the R1 commit's git objects) | "
         f"{count(os.path.join(rec, '21-checker-b.txt'), r'^CHECKER B: (\w+)')}, "
         f"{count(os.path.join(rec, '21-checker-b.txt'), r'^checks (\d+);')} checks | `receipts/21-checker-b.txt` |",
         "| Checker agreement | identical per-id verdict maps (38 ids, all PASS) | `receipts/20`, `21`, `40` |",
         "| Syntax (rustfmt --check, disposable copies) | all 21 SYNTAX_CHECKED_NOT_TYPECHECKED (A6 in its R2 form); "
         "C's files pass; an unparsable control fails | `receipts/30-syntax-check.txt` |",
         f"| Negative controls | {count(neg, r'^seeds (\d+);')} seeded defects (R1's 29 and R2's "
         f"{int(count(neg, r'^seeds (\d+);')) - 29}), each detected by both checkers | "
         "`receipts/40-negative-controls.txt` |",
         "| Freeze record (packet bytes; no catalog status) | every packet file hashed before the final gate | "
         "`receipts/50-FREEZE-RECORD.json` |",
         "| Final gate | see its receipt | `receipts/60-final-gate.txt` |", "",
         "## The 35 patch identities (R2; R1 equality)", "",
         "| Id | Source | File | Patch sha256 | Mutant blob | = R1 |", "|---|---|---|---|---|---|"]
    for e in kill:
        same = e == r1[e["id"]]
        o.append(f"| {e['id']} | {e['definition_source']} | `{e['mutation_source_file'].rsplit('/', 1)[1]}` | "
                 f"`{e['patch_sha256']}` | `{e['mutant_blob']}` | {'yes' if same else 'no: KEY_FIELDS hunk'} |")
    rr = next(e for e in kill if e["id"] == "C22")["restored_receipt"]
    o += ["", "## A4 and C22", "",
          f"- **A4**: {a4['status']} — the separate accepted comparison (Stage-1 `receipts/50-A4`), not a mutation, "
          "not counted among the 35, not rerun; its entry is R1's.",
          f"- **C22**: restored receipt **{rr['restored_receipt_status']}** (mission R1 section 7; R2 section 9.H). "
          "The Stage-1 healthy build omitted `-j 2` and is not C22's type (e4) receipt. A future mission must build "
          f"Candidate C by exactly `{rr['required_future_healthy_command']}`, with command, toolchain, environment, "
          "target policy, features/cfg/RUSTFLAGS and supervision identical to C22's mutant build. Neither build ran "
          "here (`C22-RESTORED-BUILD-RULING.md`).", "",
          "## Limitations", "",
          "- Static only. `SYNTAX_CHECKED_NOT_TYPECHECKED` is a parse-and-format check by rustfmt, not a type check. "
          "Nothing was compiled; R1's limitations on type errors and build-environment warnings stand unchanged.",
          "- The KEY_FIELDS line is now 128 characters. rustfmt (default `error_on_line_overflow = false`, "
          "`format_strings = false`) leaves string literals as written and `--check` passes; the repository sets no "
          "rustfmt configuration and C already has lines in this constant longer than 100 characters.",
          "- That a printed receipt shows the new table description is a static reading of the report source (the "
          "three `state key fields: {KEY_FIELDS}` sites); only execution prints a receipt.",
          "- No control is shown to be killed; whether each mutant fails its named property within its ceiling, on the "
          "original explorer, is decided only by execution.",
          "- The R2 mission text (sections 1 and 4, `tools/r2_section4.txt`) and R1's section 6 are recorded as "
          "received; the mission texts are not repository artifacts, so the executor cannot authenticate them beyond "
          "transcription.",
          "- The two checkers are independent implementations written by the same executor; they are not an "
          "independent review. Both extend R1's checkers, whose R1 checks are kept.",
          "- The 34 unchanged entries are R1's byte for byte, so their text still names R1's mission where R1 wrote "
          "it (the Architect-defined origin notes, and the restored-receipt status of C12, C14, C19, C21, C27 and C28, "
          "repeated in each one's restoration record, ending \"no execution is authorised in "
          "P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01\"). They were not rewritten: mission R2 "
          "section 6 requires equality, and no execution is authorised in R2 either.",
          "- Still to be produced before execution (unchanged from R1): the baseline exact receipts (X-1 alone for C12; "
          "T-BIND, T-CELL, W6, T-PREC and T-RES alone) and C22's exact-command healthy build.", "",
          "## Next blocker", "",
          "Independent Sole Architect review of the R2 candidate catalog (the A6 correction and the 34 unchanged "
          "controls). Until the Architect accepts a catalog, no catalog is fixed and no build, baseline receipt or "
          "mutant run is authorised.", "",
          "## Files", "",
          "| Path | What |", "|---|---|",
          "| `INDEX.md` | this file |",
          "| `ARCHITECT-CONTROL-DEFINITIONS-R2.md` | R1's 21 definitions carried forward, with the A6 correction "
          "recorded |",
          "| `C22-RESTORED-BUILD-RULING.md` | the C22 exact-command ruling as recorded in the catalog |",
          "| `catalog/` | `catalog.json`, `CATALOG.md`, `PATCH-IDENTITIES.md`, `README.md`, `patches/` (35) |",
          "| `comparison/` | `R1-TO-R2-PATCH-COMPARISON.md` (field by field, all 35), `A6-R1-TO-R2-PATCH.diff`, "
          "`A6-R1-TO-R2-MUTANT.diff` |",
          "| `receipts/` | authentication, checker A, checker B, syntax check, negative controls, freeze record, "
          "final gate |",
          "| `development/` | `DEVELOPMENT-LOG.txt`, every failed development run as found, catalog revision diff |",
          "| `tools/` | every tool, the A6 correction as data (`a6_correction.py`) and R2 sections 1 and 4 as "
          "received (`r2_section4.txt`) |",
          "| `SHA256SUMS` | every file of this directory except itself |", "",
          "## How to verify", "",
          "From this directory: `sha256sum -c SHA256SUMS`. The tools re-run read-only against the repository's git "
          "objects given a root directory: `authenticate.py ROOT` writes the authenticated inputs (Candidate C's "
          "sources, the R1 catalog, patches and section 6, the sources the checkers read) to `ROOT/inputs/`; "
          "`derive_catalog.py ROOT` reproduces `catalog/` and `comparison/` byte for byte; `checker_a.py ROOT`, "
          "`checker_b.py ROOT`, `syntax_check.py ROOT` and `negative_controls.py ROOT` reproduce `receipts/20`, `21`, "
          "`30` and `40`. Only `syntax_check.py` runs an external program other than git and patch: "
          "`rustfmt --check`, which never writes.", ""]
    os.makedirs(os.path.join(root, "docs-src"), exist_ok=True)
    with open(os.path.join(root, "docs-src", "INDEX.md"), "w") as fh:
        fh.write("\n".join(o))
    print(f"INDEX.md sha256 {sha(os.path.join(root, 'docs-src', 'INDEX.md'))}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
