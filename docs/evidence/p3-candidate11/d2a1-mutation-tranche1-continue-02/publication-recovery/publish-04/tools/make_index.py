#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission section 12: write the index of the publication recovery
(documents/INDEX-PUBLICATION-RECOVERY.md; placed at the packet root by assemble.py, beside CONTINUE-02's own INDEX.md).
Descriptive only: every result it names lives in the receipt it points to; it states no result of a step that has not
run when it is written (the freeze, the final gate and the publication).

    make_index.py OUT.md     OUT.md must not exist
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import assemble as A  # noqa: E402
import build_packet as B  # noqa: E402
import pub4common as Q  # noqa: E402

P3D, P4D = A.P3_DIR, A.P4_DIR


def main(out):
    Q.no_bytecode()
    if os.path.exists(out):
        raise SystemExit("REFUSED: output exists")
    acc = Q.sha256_file(os.path.join(Q.SRC, "documents/ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md"))
    c28 = Q.sha256_file(os.path.join(Q.SRC, "documents/ARCHITECT-RULING-C28-BUILD-ADMISSIBILITY.md"))
    r1 = Q.sha256_file(os.path.join(Q.P3, "documents/ARCHITECT-RULING-DUPLICATE-LINK-RECEIPTS.md"))
    r2 = Q.sha256_file(os.path.join(Q.P, "documents/ARCHITECT-RULING-DUPLICATE-POLICY-R2.md"))
    nm = Q.sha256_file(B.MAP)
    zm = Q.sha256_file(B.ZERO_MAP)
    i = [f"# {Q.SRC_MISSION} — evidence packet: publication recovery index", "",
         f"Published by {Q.MISSION} (publication recovery only). Executor: Claude Code, Claude Opus 5.5 "
         "(`claude-opus-5-5`), effort High; the mission named Claude Sonnet 5.5 (`claude-sonnet-5-5`), a deviation "
         "recorded here. No mutation control, baseline, build, test, explorer, witness or executable was run by "
         f"{Q.P3_MISSION} or by {Q.MISSION}: only Python publication tools, read-only git and the GitHub CLI.", "",
         "This file indexes how the already-produced CONTINUE-02 execution evidence came to be published. "
         "`INDEX.md` beside it is CONTINUE-02's own index, rendered by its frozen `render.py` from its receipts.", "",
         "## Status", "",
         "- C28, C12, C9, C11, C17, C18a, C18b: executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW "
         "PENDING`), pending independent Architect review. Nothing here accepts any of them.",
         f"- Architect-accepted before this packet (`ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md`, sha256 "
         f"`{acc}`): C22, C14, C19, C21, C27. C28's build: Architect-admissible under a narrow ruling "
         f"(`ARCHITECT-RULING-C28-BUILD-ADMISSIBILITY.md`, sha256 `{c28}`).", "",
         "## Layers", "",
         "| Path | Origin | What |", "|---|---|---|",
         "| `INDEX.md`, `TRANCHE-SUMMARY.md`, `CARRIED-FORWARD.md`, the two `ARCHITECT-RULING-*.md`, `receipts/`, "
         "`tools/`, `development/`, `SHA256SUMS` | CONTINUE-02 root, staged by the R2 builder | the complete CONTINUE-02 "
         "execution evidence, byte for byte; the same files the frozen CONTINUE-02 builder staged, plus its "
         "`receipts/80-STOP-RECORD.md` (`SHA256SUMS` rewritten over the whole packet) |",
         "| `publication/10-build-packet.txt` | CONTINUE-02 root | the original packet failure (the frozen builder's "
         "`PACKET: FAILED`, seven link receipts) |",
         f"| `{P3D}/` | PUBLISH-03 root (without `scratch/`) | ruling R1 (seven link receipts), its map, the first "
         "repaired builder and its diff, authentication, repair controls with the failed P3 result, the 14-duplicate "
         "and zero-length inventory, the source-evidence manifests, its stop record, development history and root "
         "SHA256SUMS |",
         f"| `{P4D}/` | this mission's root (without `scratch/`) | ruling R2, the 21-row and 8-row maps, the R2 builder "
         "and its three diffs, repair controls, manifests of both read-only roots, worktree and preflight receipts, "
         "freeze, final gate, development history |",
         "| `INDEX-PUBLICATION-RECOVERY.md` | this mission | this file |", "",
         "## Key records", "",
         "| Record | Path |", "|---|---|",
         f"| Architect ruling R1 — duplicate link receipts (sha256 `{r1[:16]}…`) | "
         f"`{P3D}/documents/ARCHITECT-RULING-DUPLICATE-LINK-RECEIPTS.md`, map `{P3D}/documents/DUPLICATE-CONTENT-MAP.tsv` |",
         f"| Architect ruling R2 — duplicate policy (sha256 `{r2[:16]}…`) | "
         f"`{P4D}/documents/ARCHITECT-RULING-DUPLICATE-POLICY-R2.md` |",
         f"| Nonempty duplicate map, exactly 21 rows (sha256 `{nm[:16]}…`) | "
         f"`{P4D}/documents/NONEMPTY-DUPLICATE-MAP.tsv` and `.md` |",
         f"| Zero-length artifact map, exactly 8 rows (sha256 `{zm[:16]}…`) | "
         f"`{P4D}/documents/ZERO-LENGTH-ARTIFACT-MAP.tsv` and `.md` |",
         "| The frozen failed builder (CONTINUE-02) | `tools/build_packet.py` |",
         f"| The first repaired builder (PUBLISH-03) and its diff | `{P3D}/tools/build_packet.py`, "
         f"`{P3D}/receipts/03-BUILDER-DIFF/BUILDER.diff` |",
         f"| The R2 builder (PUBLISH-04) and its three diffs (frozen → PUBLISH-03, PUBLISH-03 → PUBLISH-04, frozen → "
         f"PUBLISH-04), with the mechanical only-the-policy check | `{P4D}/tools/build_packet.py`, "
         f"`{P4D}/receipts/03-BUILDER-DIFF/`, `{P4D}/receipts/03-BUILDER-DIFF.console.txt` |",
         f"| Repair controls R2 (P1–P4, N1–N26) | `{P4D}/receipts/04-REPAIR-CONTROLS/CONTROLS.json`, "
         f"`{P4D}/receipts/04-REPAIR-CONTROLS.console.txt` |",
         f"| Repair controls R1 (29 of 30; P3 failed) | `{P3D}/receipts/04-REPAIR-CONTROLS/`, "
         f"`{P3D}/receipts/04-REPAIR-CONTROLS.console.txt` |",
         f"| Manifests of both read-only roots, before (and after: post-gate) | "
         f"`{P4D}/receipts/00-ROOT-MANIFESTS-BEFORE/`, `{P4D}/receipts/21-ROOT-MANIFESTS-AFTER/`; PUBLISH-03's "
         f"`{P3D}/receipts/00-SOURCE-MANIFEST-BEFORE.tsv`, `05-SOURCE-MANIFEST-AFTER.tsv` |",
         f"| Authentication of every input | `{P4D}/receipts/01-AUTHENTICATION.txt` |",
         f"| Worktree and publication preflight | `{P4D}/receipts/05-WORKTREE.txt`, "
         f"`{P4D}/receipts/06-PUBLICATION-PREFLIGHT.txt` |",
         f"| Packet assembly, freeze, final publication gate (post-gate records) | "
         f"`{P4D}/receipts/07-ASSEMBLE.console.txt`, `10-FREEZE.json`, `10-FREEZE-RECORD.txt`, `20-FINAL-GATE.txt` |", "",
         "## Preserved failures", "",
         "- CONTINUE-02: the first final-gate run's crash (`receipts/90-final-gate-attempt-1-CRASHED.txt`) and the "
         "successful unchanged re-run (`receipts/90-final-gate.txt`); the frozen builder (`tools/build_packet.py`) and "
         "its duplicate failure (`publication/10-build-packet.txt`); `receipts/80-STOP-RECORD.md`; "
         "`development/DEVELOPMENT-LOG.txt`.",
         f"- PUBLISH-03: control P3 failed (`{P3D}/receipts/04-REPAIR-CONTROLS/P3-builder-run.txt`), the exact "
         f"14-duplicate and eight-zero-length inventory (`{P3D}/receipts/07-DUPLICATE-INVENTORY.tsv`), its stop record "
         f"(`{P3D}/receipts/80-STOP-RECORD.md`), its development history and revisions, a superseded receipt pair "
         f"(`{P3D}/development/superseded-1-rc-misrecorded/`).",
         f"- PUBLISH-04: every development failure and revision (`{P4D}/development/`).", "",
         "## Exclusions (nothing else is left out)", "",
         "- `scratch/` of the PUBLISH-03 root and of this mission's root (control stagings; not evidence).",
         f"- `{P3D}/development/make-map-dev1-FAILED-sha-column-key.txt`: zero-length (the empty sha256). Ruling R2 "
         "permits exactly eight zero-length packet artifacts, so it is not staged; it remains in the PUBLISH-03 root, "
         f"listed in `{P3D}/receipts/90-ROOT-SHA256SUMS.txt` and in `{P4D}/receipts/00-ROOT-MANIFESTS-BEFORE/"
         "publish-03.tsv`. PUBLISH-03's development log, entry 6, describes the failure it recorded.",
         "- Previous packets (EXEC-01, RESUME-01) are referenced by commit, path, blob and sha256 "
         "(`CARRIED-FORWARD.md`, the maps), never copied.", "",
         "## Limitations", "",
         "- Every classification here is an executor candidate for Architect review, never an acceptance.",
         "- The duplicate and zero-length policy is a publication rule over files and their recorded provenance; it "
         "is not a claim about the mutants, the tests or any OS containment.",
         "- Publication receipts written after the commit (push, remote readback, Actions readback) cannot be inside "
         "the commit they describe; they are kept in this mission's root and reported to the Architect.", ""]
    open(out, "w").write("\n".join(i))
    print(f"wrote {out} sha256 {Q.sha256_file(out)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1])))
