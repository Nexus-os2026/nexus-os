#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01, mission section 13: write ROOT/docs-src/INDEX.md from the catalog,
the rendered definitions record and the receipts (deterministic; read-only for the repository).

    render_index.py ROOT
"""
import hashlib
import json
import os
import re
import subprocess
import sys

PKT = "docs/evidence/p3-candidate11/d2a1-control-definitions-r1-01"


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
    defp = os.path.join(root, "out", "docs", "ARCHITECT-CONTROL-DEFINITIONS-R1.md")
    cat = json.load(open(catp))
    kill = [e for e in cat["controls"] if e["id"] != "A4"]
    a4 = next(e for e in cat["controls"] if e["id"] == "A4")
    auth_ok = sum(1 for l in open(os.path.join(rec, "00-authentication.txt")) if l.startswith("    OK "))
    o = ["# D2A-1 control definitions R1 — index", "",
         "Mission P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01. Executor: Claude Code, Claude Opus 5.5 "
         "(`claude-opus-5-5`), effort High. For the Sole Architect's review.", "",
         "**Status: READY_FOR_CONTROL_DEFINITION_ARCHITECT_REVIEW.**", "",
         "Static definition work only. No Cargo, no rustc, no build, no test, no explorer; no patch applied to any "
         "worktree; no mutant built or run. Candidate C is unchanged. The catalog is a **candidate**: every control's "
         "definition is CANDIDATE_READY, its Architect review pending, its execution NOT_ATTEMPTED. No kill is "
         "claimed. D2A-1 remains unaccepted; Candidate 11 is not integrated; Phase Three remains active.", "",
         "The 21 definitions are the Sole Architect's (mission section 6), encoded by the executor: "
         "**ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING** "
         "(`ARCHITECT-CONTROL-DEFINITIONS-R1.md`). The other 14 are the blocked catalog's patches, carried forward "
         "byte for byte and re-authenticated (mission section 5).", "",
         "## Identities", "",
         "| Item | Value |", "|---|---|",
         f"| Candidate C | `{cat['baseline']['commit']}`, tree `{cat['baseline']['tree']}`, single parent "
         "`71264aa81a40b86189783facc8437e0427887c16` |",
         "| Blobs at C | " + "; ".join(f"`{p.rsplit('/', 1)[1]}` `{b}`" for p, b in cat["baseline"]["blobs"].items()) +
         " |",
         "| Contract | sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c` |",
         "| AD-1-R4 | sha256 `00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693` |",
         "| AD-1-R5 | sha256 `4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473`; Owner-adoption record "
         "`docs/evidence/p3-candidate11/ad1r5-adoption/OWNER-ADOPTION-2026-10-14.md`, blob "
         "`1492e185cb475bd32d95079a39b49078c0e23ddc`, sha256 "
         "`9990d2565a5ec96e0db03be832e96f64f774e662f6131cb69240c9a435757aa4` |",
         "| Historical partial controls file | sha256 `ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f` |",
         "| Blocked catalog (immutable review history) | `d2a1-controls-v1`, sha256 "
         "`a0cdef7976bbb1715a7da25ad2900c978a32c6b3d9029b46f16872eda66b4747`, evidence commit "
         "`59df21036f7c0183fa17d76b6e0886f8f5784685`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche1-01` |",
         f"| **Candidate catalog** | `{PKT}/catalog/catalog.json`, version `{cat['version']}`, git blob "
         f"`{blob(catp)}`, catalog sha256 `{sha(catp)}` |",
         f"| **Definitions record** | `{PKT}/ARCHITECT-CONTROL-DEFINITIONS-R1.md`, git blob `{blob(defp)}`, "
         f"definitions record sha256 `{sha(defp)}` |",
         "| Evidence commit parent | `59df21036f7c0183fa17d76b6e0886f8f5784685` |", "",
         "## Results", "",
         "| Step | Result | Record |", "|---|---|---|",
         f"| Input authentication (mission 4, 5) | ALL CHECKS OK ({auth_ok} checks) | `receipts/00-authentication.txt` |",
         f"| Candidate catalog | {cat['counts']['kill_required']} kill-required, all CANDIDATE_READY (14 carried "
         f"forward, 21 Architect-defined), 0 blocked; A4 separate | `catalog/` |",
         f"| Checker A (bytes/text; GNU patch, git apply) | "
         f"{count(os.path.join(rec, '20-checker-a.txt'), r'^CHECKER A: (\w+)')}, "
         f"{count(os.path.join(rec, '20-checker-a.txt'), r'^checks (\d+);')} checks | `receipts/20-checker-a.txt` |",
         f"| Checker B (independent: strict diff engine, tokenizer) | "
         f"{count(os.path.join(rec, '21-checker-b.txt'), r'^CHECKER B: (\w+)')}, "
         f"{count(os.path.join(rec, '21-checker-b.txt'), r'^checks (\d+);')} checks | `receipts/21-checker-b.txt` |",
         "| Checker agreement | identical per-id verdict maps (38 ids, all PASS) | `receipts/20`, `21`, `40` |",
         "| Syntax (rustfmt --check, disposable copies) | all 21 SYNTAX_CHECKED_NOT_TYPECHECKED; C's files pass; an "
         "unparsable control fails | `receipts/30-syntax-check.txt` |",
         f"| Negative controls | {count(os.path.join(rec, '40-negative-controls.txt'), r'^seeds (\d+);')} seeded "
         "defects, each detected by both checkers | `receipts/40-negative-controls.txt` |",
         "| Freeze record (packet bytes; no catalog status) | every packet file hashed before the final gate | "
         "`receipts/50-FREEZE-RECORD.json` |",
         "| Final gate | see its receipt | `receipts/60-final-gate.txt` |", "",
         "## The 35 patch identities", "",
         "| Id | Source | File | Patch sha256 | Mutant blob |", "|---|---|---|---|---|"]
    for e in kill:
        o.append(f"| {e['id']} | {e['definition_source']} | `{e['mutation_source_file'].rsplit('/', 1)[1]}` | "
                 f"`{e['patch_sha256']}` | `{e['mutant_blob']}` |")
    rr = next(e for e in kill if e["id"] == "C22")["restored_receipt"]
    o += ["", "## A4 and C22", "",
          f"- **A4**: {a4['status']} — the separate accepted comparison (Stage-1 `receipts/50-A4`), not a mutation, "
          "not counted among the 35, not rerun.",
          f"- **C22**: restored receipt **{rr['restored_receipt_status']}** (mission section 7). The Stage-1 healthy "
          "build omitted `-j 2` and is not C22's type (e4) receipt. A future mission must build Candidate C by exactly "
          f"`{rr['required_future_healthy_command']}`, with command, toolchain, environment, target policy, "
          "features/cfg/RUSTFLAGS and supervision identical to C22's mutant build. Neither build ran here "
          "(`C22-RESTORED-BUILD-RULING.md`).", "",
          "## Limitations", "",
          "- Static only. `SYNTAX_CHECKED_NOT_TYPECHECKED` is a parse-and-format check by rustfmt, not a type check. "
          "Nothing was compiled: a type error (for example in A1's match guard, or A2's let-else) or a warning that a "
          "future build environment might deny (dead_code after C3a, C3b, C3c; unused `nonce` and `io` after C5) can "
          "only be found by building. At C, the crate denies `unsafe_code` and, outside test builds, five clippy lints; "
          "it does not deny `dead_code` or `unused_variables`, and `.cargo/config.toml` sets no RUSTFLAGS.",
          "- No control is shown to be killed. Whether each mutant fails its named property within its ceiling, on the "
          "original explorer, is decided only by execution (each control's uncertainty is listed in the definitions "
          "record).",
          "- Section 6 is recorded as received in the mission text; the mission text is not a repository artifact, so "
          "the executor cannot authenticate it beyond transcription.",
          "- Encoding choices inside section 6 that the Architect should look at: C8 carries the healthy facts-read "
          "failure arm unchanged into the merged loop (after an earlier insert it reports Not written, as 6.7 rules "
          "for a refusal) and drops the two comments that describe the removed structure; A1 adds an `expect` message "
          "and trace notes, A2 a step-one trace note (trace text only); C6, C15, C16 and A5 use the layout rustfmt "
          "requires; A6 leaves the `KEY_FIELDS` receipt string unchanged.",
          "- The two checkers are independent implementations written by the same executor; they are not an "
          "independent review.",
          "- In the 14 carried-forward entries only non-definition text changed: the origin note, and six "
          "restored-receipt status sentences (C12, C14, C19, C21, C27, C28) that named the blocked mission's sections "
          "and said the catalog was blocked. Their patches, anchors and mutants are byte-identical.",
          "- Still to be produced before execution: the baseline exact receipts (X-1 alone for C12; T-BIND, T-CELL, "
          "W6, T-PREC and T-RES alone) and C22's exact-command healthy build.", "",
          "## Next blocker", "",
          "Independent Sole Architect review of the 21 encodings and of the candidate catalog. Until the Architect "
          "accepts a catalog, no catalog is fixed and no build, baseline receipt or mutant run is authorised.", "",
          "## Files", "",
          "| Path | What |", "|---|---|",
          "| `INDEX.md` | this file |",
          "| `ARCHITECT-CONTROL-DEFINITIONS-R1.md` | the 21 definitions: contract row, section-6 text, item, anchors, "
          "replacements, identities, property/test/configuration, receipt type, preserved behaviour, uncertainty |",
          "| `C22-RESTORED-BUILD-RULING.md` | the C22 exact-command ruling as recorded in the catalog |",
          "| `catalog/` | `catalog.json`, `CATALOG.md`, `PATCH-IDENTITIES.md`, `README.md`, `patches/` (35) |",
          "| `receipts/` | authentication, checker A, checker B, syntax check, negative controls, freeze record, "
          "final gate |",
          "| `development/` | `DEVELOPMENT-LOG.txt`, every failed development run as found, catalog revision diffs, "
          "the trial-encoding tool |",
          "| `tools/` | every tool, the encoded definitions (`architect_definitions.py`) and section 6 as received "
          "(`section6.txt`) |",
          "| `SHA256SUMS` | every file of this directory except itself |", "",
          "## How to verify", "",
          "From this directory: `sha256sum -c SHA256SUMS`. The tools re-run read-only against the repository's git "
          "objects given a root directory: `authenticate.py ROOT` writes the authenticated inputs to `ROOT/inputs/`; "
          "`derive_catalog.py ROOT` reproduces `catalog/` byte for byte; `checker_a.py ROOT`, `checker_b.py ROOT`, "
          "`syntax_check.py ROOT` and `negative_controls.py ROOT` reproduce `receipts/20`, `21`, `30` and `40`. "
          "Only `syntax_check.py` runs an external program other than git and patch: `rustfmt --check`, which never "
          "writes.", ""]
    os.makedirs(os.path.join(root, "docs-src"), exist_ok=True)
    with open(os.path.join(root, "docs-src", "INDEX.md"), "w") as fh:
        fh.write("\n".join(o))
    print(f"INDEX.md sha256 {sha(os.path.join(root, 'docs-src', 'INDEX.md'))}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
