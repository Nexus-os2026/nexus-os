TRANCHE 3 — C2 — MUTATION CONTROL KILL ACCEPTED (23 OF 35); TRANCHE 3 CONTINUATION AUTHORIZED: C24 C3b C3c C6 C7 A1 A2 C20 A5

# Architect ruling record — Mutation Tranche 3: C2 acceptance; Tranche 3 continuation authorized

Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02, from section 1 (ARCHITECT RULING), section 6 (OBJECTIVE / ORDER) and section 19 (RESERVED FINAL CONTROLS) of that mission as issued by the Sole Nexus OS Architect (ChatGPT GPT-6) via the Owner.

This file is evidence of the Architect's ruling.
A string is not authority.

Claude Code does not independently accept any control.

## The ruling, as issued (mission section 1)

> Independent review of:
>
> P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01
>
> accepts:
>
> C2 — MUTATION CONTROL KILL ACCEPTED
>
> Current accepted campaign count:
>
> 23 of 35 kill-required controls.
>
> A4 remains separately:
>
> SATISFIED — ARCHITECT ACCEPTED
>
> Evidence commit:
>
> 774f457a4fc27f6b26e132a0caf71b683b9af8cd
>
> Review ref:
>
> refs/heads/review/p3-c11-d2a1-mutation-tranche3-exec-01
>
> Do not rebuild, rerun or regenerate C2 evidence.

## The accepted C2 evidence (immutable; read from git objects at its evidence commit; not republished)

Evidence commit `774f457a4fc27f6b26e132a0caf71b683b9af8cd` (tree `f74660b98288c53e285bcb4d3b98fb9f55962b57`, single parent `b97205b2b9ea058ee864b6649dc5002d28b1edeb`), review ref `refs/heads/review/p3-c11-d2a1-mutation-tranche3-exec-01`, packet `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-exec-01/**`.

| Control | Change (catalog) | Configuration; printed property; discovered states | RESULT.txt (historical executor record) | Blob | sha256 | First line (unchanged) |
|---|---|---|---|---|---|---|
| C2 | the gating entry rule does not test C and X | F-1; P4; 361,534 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-exec-01/receipts/66-C2/RESULT.txt` | `57c789536982a03c357492bd7aec619e27ac7684` | `130210abc0b6b3c408219959458ccc8381e0fd5f612a43feb5c84855cc75d0f5` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |

| Control | Mutant (patch sha256; mutant blob) | Pinned mutant executable sha256 | Run classification (blob) | Raw output sha256 | Restored-side link (blob) |
|---|---|---|---|---|---|
| C2 | `f02c3e721aa2c96bb7d15fe081acf3dc6e4c1958de096f4215bc55a7cafc218d`; `1c10adbc93c2fe0984d0ac677727a3bdc82b8e5d` | `8460515fe149a78637bcb1d797d9cb5c2ed5b89eb2361cd74e02e96cedaed0ed` | `a0f3a04d1daf7e1fc25e17f5366c327602f6e674` | `758a6b98779fb27bd44ef1e99783ceb1982b2532e135d0d7de50e48963a1fb31` | `4f3e63c5849ce856c188f47517af7a98cc825608` |

The first line above is the executor's historical candidate label; it is not relabelled. Acceptance is this ruling's, not the receipts'.

## Campaign count

Architect-accepted mutation-control kills: **23 of 35** — Tranche 1: C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b; Tranche 2: C23, C10, C16, C26, C3a; Tranche 2 continuation: C5, A3, A6, C8, C15; Tranche 3: C2. A4: SATISFIED — ARCHITECT ACCEPTED (separately; not kill-required).

| Earlier record | Path | Blob | sha256 |
|---|---|---|---|
| Tranche-3 INDEX.md | `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-exec-01/INDEX.md` | `992166dabfdd5442685fea5ae0154f74cd742353` | `2183faeaa1d39e121c2bd3c1a0d5bdf62414ac28f2aeafa9d6896561c87abf13` |
| Tranche-3 TRANCHE-SUMMARY.md | `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-exec-01/TRANCHE-SUMMARY.md` | `d4c9c820e329600dc2a0df1736a5067e498feaac` | `60afe7cf788cd2afe2bd7c49fca7988072cb868ee9f84c7e7589ed1dbeac09ae` |
| Tranche-3 SHA256SUMS | `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-exec-01/SHA256SUMS` | `91b77394e0e5b20ab9606306a27745df62440cde` | `ba2d6a6bdcf527a8503ccc0e926155f6fec9d393c4e30131572631aef9fa2209` |
| Tranche-3 stop record | `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-exec-01/receipts/80-STOP-RECORD.md` | `9fcf5ff3a740abe928c5423b3dcea114b9b6b06a` | `ca87a5a07a716f08fdcfdca03b42253ba9a651d3606d774fef9e600d1b8da0f7` |
| Tranche-2 continuation acceptance (as recorded in Tranche 3) | `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-exec-01/documents/ARCHITECT-RULING-TRANCHE2-CONTINUATION-ACCEPTANCE.md` | `674ed583be5e88700426c6e05db978003d03e016` | `6013b68a1097d0bb1c3702164635185e664de51be2e15dd8421c4bfd21d6b7d8` |

## What this mission is authorized to do (mission sections 6 and 19)

- Continue Tranche 3 in exactly this order: C24, C3b, C3c, C6, C7, A1, A2, C20, A5; stop on the first unsuccessful required control.
- C24 uses its existing preserved executable (no rebuild); C3b and C3c are built once under their build rules (`documents/ARCHITECT-RULING-C24-C3B-C3C-BUILD-ADMISSIBILITY.md`, as amended by `documents/ARCHITECT-AMENDMENT-01-C3B-C3C-BUILD-RULE.md`); C6 onward use the ordinary zero-warning policy.
- Not execute C1, C4, C25 (reserved for the final witness-bound tranche), any of the 23 accepted controls, A4 or M01–M183.

## What this record does not do

- It does not accept any of C24, C3b, C3c, C6, C7, A1, A2, C20, A5: their results in this mission are executor candidates, pending Architect review.
- It does not accept D2A-1, complete the campaign, integrate anything or complete Phase Three.
- It changes no receipt, label, catalog entry, patch or source.
