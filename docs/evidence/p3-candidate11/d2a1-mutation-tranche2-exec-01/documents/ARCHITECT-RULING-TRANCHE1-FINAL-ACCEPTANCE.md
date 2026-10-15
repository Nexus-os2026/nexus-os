TRANCHE 1 — C22 C14 C19 C21 C27 C28 C12 C9 C11 C17 C18a C18b — MUTATION CONTROL KILL ACCEPTED (12 OF 35); A4 SATISFIED — ARCHITECT ACCEPTED

# Architect ruling record — Mutation Tranche 1 final acceptance

Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01, from section 1 (ARCHITECT RULING) of that mission as issued by the Sole Nexus OS Architect (ChatGPT GPT-6) via the Owner.

This file is evidence of the Architect's ruling.
A string is not authority.

## The ruling, as issued (mission section 1)

> Mutation Tranche 1 has passed independent Architect review.
>
> Accepted controls: C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b.
>
> Current accepted kill count: 12 of 35 kill-required controls.
>
> A4 remains separately: SATISFIED — ARCHITECT ACCEPTED
>
> The complete Tranche-1 evidence is published at: evidence commit `ebe42009ba1bd262cf31ae2f401e02cec101afa7`; review ref `refs/heads/review/p3-c11-d2a1-mutation-tranche1-continue-02`; packet `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/**`.
>
> Do not rebuild, rerun or regenerate any accepted Tranche-1 control or baseline receipt.
>
> Their historical executor candidate labels remain unchanged. This Architect ruling supplies acceptance authority.

## The accepted evidence (immutable; read from git objects at the Tranche-1 evidence commit; not republished)

Evidence commit `ebe42009ba1bd262cf31ae2f401e02cec101afa7` (tree `7d8ab59e4b3e61196d9c4cee3cb3a046fa8c4400`, single parent `3695a817a6190781dc66769a72a487b442d3e6da`), review ref `refs/heads/review/p3-c11-d2a1-mutation-tranche1-continue-02`.

| Control | Change (catalog) | RESULT.txt (historical executor record) | Blob | sha256 | First line (unchanged) |
|---|---|---|---|---|---|
| C22 | the ticket derives `Clone` | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/receipts/31-C22/RESULT.txt` | `b9717ad766e11dacf4d342db0022ab56886ae486` | `106c45675b7f1d0e164d10125b7258ffe71244f908573d63321371771dda0aec` | `CONTROL RESULT: MUTATION_CONTROL_BLOCKED` |
| C14 | the ticket check of step 0 is removed | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-resume-01/receipts/32-C14/RESULT.txt` | `40c73e6335b0e221c34e0d079b049ac815c9862b` | `225b706e06d472dace8557ac165f20f58ea7eff6ee0f6646617ff10dd745cd1f` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C19 | X can be emptied before it is marked committed | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-resume-01/receipts/33-C19/RESULT.txt` | `9c2999e128f86ab92cf1a1b3ec5427682c65c089` | `6ca7d192c0647ab6f8af2cf70dca350470a78e2aade94d0eeb12e43327f868e8` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C21 | the well-formed test of step 1 is removed | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-resume-01/receipts/34-C21/RESULT.txt` | `0daa04f2044d48575dd0d0363158a42e4dbbd6b2` | `6eda983f969ccfbe9618b0fd2ab133fab1b1719d98ebc47db5a25bb4a065523f` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C27 | the guard is created after the entry rule | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-resume-01/receipts/35-C27/RESULT.txt` | `00145cd131aa2aeffd70e916a272c02aac55e916` | `4b8b64b5a6288733cf70c5d11fa1aae338f3fb2159450354eecfe0df000f6a3c` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C28 | resolve's "present, frontier below N" check alone is removed | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/receipts/36-C28/RESULT.txt` | `21d0ba8e95903d7e66ad720e2176a6404c1eac63` | `d425668a12ee748e4e50a130ddd7fef337c0c6d427bde78cede700817f178d6d` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C12 | the publication is two exchanges: bits, then count | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/receipts/37-C12/RESULT.txt` | `9935630251c4c81adcb49fc9cd83c3a739cf59d2` | `915a93a761f97c117ba3ab4d0e19e274b6796928035375bd60253fdcc00237b7` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C9 | `Rollback::Unknown` is treated as `Undone` | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/receipts/38-C9/RESULT.txt` | `2eecc90b8176d6c6dd86ba7f6ba6be80e0d9f8f4` | `d8964dc693266739a697e9e49f3dac6a260176e640d2cda3b9fe53161ab73b40` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C11 | a constraint failure during the inserts gives Not written whatever the rollback said. **This is the amendment's rule, which the review rejected.** | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/receipts/39-C11/RESULT.txt` | `048a8cfa7932050d44018ef5cbf0be5c146c85f4` | `a01e5394d7007ac74551460c6a1c52b777fb47c90a30a6c8c731b409d08ed6f3` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C17 | the guard reports Not written after an insert was issued | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/receipts/40-C17/RESULT.txt` | `9ac14d9641e5141644dbde15272887d87d42d78d` | `a45c9679ce0849b8b4111b6f2f98043235879eff5c7279f90e59d205cd40eebb` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C18a | a refused gating append leaves U occupied | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/receipts/41-C18a/RESULT.txt` | `b3cb2b68b40581fd1b64c8b7c8dd7c3392f6420f` | `ba7a5459ca7ad3e9eb47e9b2d7fbf5297fcdc94cff073b7cdb946479bfa73f1b` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C18b | a gating conflict makes a table entry | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/receipts/42-C18b/RESULT.txt` | `8bfc35a4f6f64cf6cb26cee63beb518f87bed4f2` | `e2ebe8dbb30093eaaf677db401d5c1b7f76873f3c669f1d91cef3c1982e524b2` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |

C22's historical first line `CONTROL RESULT: MUTATION_CONTROL_BLOCKED` is the immutable output of the frozen tool that the earlier C22 ruling addressed; every other first line is the executor's candidate label. Neither is relabelled: acceptance is this ruling's, not the receipts'.

| Earlier record | Path | Blob | sha256 |
|---|---|---|---|
| C22 acceptance ruling | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-resume-01/ARCHITECT-RULING-C22-ACCEPTANCE.md` | `b852dc55adfb3b2dbff05a48f811143e5b12fc37` | `8bfb7fdac4141bc00f0ae4a1f994c1f38dd157b9e452abc40f52a3ebdb290af3` |
| partial acceptance (C22 C14 C19 C21 C27; C28 build admissible) | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md` | `59d7a24b19ee2294d68c0f3a375ba57380b4b377` | `cf9bf865e01c8f8b359916eae75f29147132ff67465f93b1c08473c0a8b0d776` |
| C28 build admissibility | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/ARCHITECT-RULING-C28-BUILD-ADMISSIBILITY.md` | `ecaa1239ea656bb8b3d48ccd23202d0ebc3478a8` | `870972bbc991b759d00ecaa379dd8f96fca17ff0ad7b3ff11425fc254cdd1d7b` |
| CONTINUE-02 INDEX.md | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/INDEX.md` | `8b177afd152dadf00fdcfbeec06de0b2fee216aa` | `c35d9128da0f4ee2f380d5cff40295a52c93d8386a3663acf0bdc7ebe4574cdc` |
| CONTINUE-02 publication recovery index | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/INDEX-PUBLICATION-RECOVERY.md` | `0cf7195a1d8f2413e40e024e37873c325af931a8` | `c454c31fa8af58a356d7e3314f0faea07025a22c1fb75af3434f8e135f0dd77a` |
| CONTINUE-02 SHA256SUMS | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/SHA256SUMS` | `fa845cac5a110290451e5136984dce14f150d0de` | `034672bf51ecdfe1939d61c0ec9f7052b1a28ae44592509883e6dc6ea9342835` |

## What this record does not do

- It does not accept any Tranche-2 control: those are this mission's executor candidates, pending review.
- It does not accept D2A-1, complete the campaign, integrate anything or complete Phase Three.
- It changes no receipt, label, catalog entry, patch or source.
