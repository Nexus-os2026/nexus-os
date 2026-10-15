TRANCHE 2 CONTINUATION — C5 A3 A6 C8 C15 — MUTATION CONTROL KILL ACCEPTED (22 OF 35); TRANCHE 3 AUTHORIZED: C2 C24 C3b C3c C6 C7 A1 A2 C20 A5

# Architect ruling record — Mutation Tranche 2 continuation acceptance

Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01, from section 1 (ARCHITECT RULING / CURRENT STATUS), section 2 (OBJECTIVE) and section 13 (NOT AUTHORIZED) of that mission as issued by the Sole Nexus OS Architect (ChatGPT GPT-6) via the Owner.

This file is evidence of the Architect's ruling.
A string is not authority.

Claude Code does not independently accept any control.

## The ruling, as issued (mission section 1)

> Independent Architect review accepts all five controls from: P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02
>
> Accepted: C5, A3, A6, C8, C15
>
> Previously accepted: C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b, C23, C10, C16, C26, C3a
>
> Current mutation-control count: 22 of 35 kill-required controls ACCEPTED.
>
> A4 remains separately: SATISFIED — ARCHITECT ACCEPTED
>
> Tranche-2 continuation evidence: commit b97205b2b9ea058ee864b6649dc5002d28b1edeb; review ref refs/heads/review/p3-c11-d2a1-mutation-tranche2-continue-02
>
> Do not rebuild, rerun or regenerate evidence for any accepted control.
>
> The remaining kill-required controls are: C1, C2, C3b, C3c, C4, C6, C7, C20, C24, C25, A1, A2, A5. This mission authorizes ten of them. C1, C4 and C25 are deliberately reserved for the final witness-bound mutation tranche.

## The accepted Tranche-2 continuation evidence (immutable; read from git objects at its evidence commit; not republished)

Evidence commit `b97205b2b9ea058ee864b6649dc5002d28b1edeb` (tree `6db2f923278f9f6ea7bacd97bd29444d4e8d6816`, single parent `2e06860f4b404d1a79a0d290116239b5545b931f`), review ref `refs/heads/review/p3-c11-d2a1-mutation-tranche2-continue-02`, packet `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-continue-02/**`.

| Control | Change (catalog) | Configuration; printed property; discovered states | RESULT.txt (historical executor record) | Blob | sha256 | First line (unchanged) |
|---|---|---|---|---|---|---|
| C5 | resolve, for an acknowledgement in U, raises the frontier with neither the presence read nor the frontier check | F-5; P13; 418,422 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-continue-02/receipts/61-C5/RESULT.txt` | `cc1ccc68b5cfc300a8f19204e32e9e421c8675ec` | `1d2eb2ece426c830745a3ac1091a96236a85c031b3306e90b0b34bca1a754ce7` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| A3 | the worker empties X before it raises H | F-5; P12; 112,880 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-continue-02/receipts/62-A3/RESULT.txt` | `8fe853c28225ffa6f2cba3fb85d1fdf448e8fd44` | `8f0f39305f65ca5d46738d1e40d36d1bc5c3649ab3e7ed2c7c213381297fa8f3` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| A6 | "recovery required" is a flag, not a comparison of H with A | F-5; P12; 3,537,396 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-continue-02/receipts/63-A6/RESULT.txt` | `0b676921013432ad4149ccbf48e537b128955107` | `8127246bcfca9c022724d62d0ad28f962df4d8311890f44668d687576ac6b581` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C8 | members are classified and inserted one at a time | F-B; E5; 547 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-continue-02/receipts/64-C8/RESULT.txt` | `f0bc22fbedbe4c86afa59dbff793ed7dd713e99d` | `d2c572022bf8836c7d0dc029630ea167b62626b5c0519d0f492e54608964924b` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C15 | a cell operation ignores the occupancy number | F-R; E8; 83,988 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-continue-02/receipts/65-C15/RESULT.txt` | `712e4ca3a4d02d9c41547c201f0c8ce8c681c54d` | `82a1bc4111654181d4bd7b77e51fca09919016fadc956c17660fa58456092d8a` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |

| Control | Mutant (patch sha256; mutant blob) | Pinned mutant executable sha256 | Run classification (blob) | Raw output sha256 | Restored-side link (blob) |
|---|---|---|---|---|---|
| C5 | `a4776885b669bd912fd8710d5a3283f4c1863ab9213e8474d26707b5a6e5f3d8`; `e1a49f5ba952bc81cdac9f8f8ace9eeb2aa93495` | `90a608834ad0227632ba26f095de5e16914942da86d2f05fdaab5a0d745c15f9` | `30564b81a2d71ee3e89cd3a34c3b748ba1dfd7c6` | `62ca176023a7f5066a474483fda926cd32e0f263c3c67835d9d5f6d1329ced19` | `0df4b80247545642ec0e723f38d9441a356dd5ac` |
| A3 | `ad0c64489e962756316b1a29b5d289a721517d1115525ee011e52db69cf2cc00`; `72bab79d71687a1c056afe18ab7a938edc20f4a9` | `4805365da56eb6534708df651442299570561b3937838366b819267f420c4b64` | `72851e74fac3d3d84d256200315585c03337aaa6` | `17132fdad65419ef1f26eca196f38c13cdb5322e337636d17c58423d24e9e1b9` | `96a67228e9b659064937eb7c967caba0fcdeb57e` |
| A6 | `0700052eb3e9fb91f36ee6d535b02b923ff007f1061490ac7dd16dd98d0d0ee8`; `51e54aed7cc33ebc8348a74fe6c881c057336ee7` | `53d72467fe1d5cd1c318cf413ef0550c2cf05ce0d62954816cf25499b9e0b01e` | `9ca455757e8fbe4b8cc6208f8aa765c65a630adb` | `52ea67c27c5187cc81ae1e11b333f19b9d1081c0846375177cf7dafebc56db2d` | `56c25fc66b3c2406cccb5c355d398c5673d3275d` |
| C8 | `a7cdd1925bf34283880384b78dd1c918fc61e64afef10094ab6d947bdeb97927`; `fd6abbd8a80dc8a01620981c76427ca76a9e1f7d` | `60cc14b41266ec603e6a8400bac791cb7c93e03acd73a01c3ddbc72f0c3f2109` | `f2ff96fbc6b76dd2b168893ce7850f66e7dca71e` | `b42d0eb5775ba8fb6c1279dcf390468c7fffcbc47e7f4e888fc2cd23e3dd90cc` | `bb3625dbac4da15285fd388eb1b30145300c62ff` |
| C15 | `a9a5037d36b1a2a6fc5fddb5accea1198ea08d3823a3044bf45f1c4b6cea5810`; `abdd7518e6d1202a3a7c756af9716dc66183fd21` | `89809695e0b87c8be259927a622bf2f2326a0323bde357aaa2406facf1c7b9f3` | `40386480443f4b4f3cc2cd116b7fc5d68218b227` | `75c2c6d76f3868e75b85e1a4770ab78ebb691fab87981187bcd070bea471a5ad` | `77e46b103ff14bf8fdcda94df18661756ceb7aa3` |

Every first line above is the executor's historical candidate label; none is relabelled. Acceptance is this ruling's, not the receipts'.

## Campaign count

Architect-accepted mutation-control kills: **22 of 35** — Tranche 1: C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b; Tranche 2: C23, C10, C16, C26, C3a; Tranche 2 continuation: C5, A3, A6, C8, C15. A4: SATISFIED — ARCHITECT ACCEPTED (separately; not kill-required).

| Earlier record | Path | Blob | sha256 |
|---|---|---|---|
| Tranche-2 continuation INDEX.md | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-continue-02/INDEX.md` | `5e927ecd202a2fcf66711d3dfc1814e29d8f8015` | `5673031b0c9bc5705e34fdc764ef9d54825ac1bccf3b7f865045667f1f43b89c` |
| Tranche-2 continuation TRANCHE-SUMMARY.md | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-continue-02/TRANCHE-SUMMARY.md` | `ca7bceb0c945bdfd82fc558a7c22889da1bbe211` | `15616d006cadf3c5fd18d45dca92694a7c2634c41ac5d0397e926e0ed26bd384` |
| Tranche-2 continuation SHA256SUMS | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-continue-02/SHA256SUMS` | `4181a3b7b2a93e686bd743e90c3893c27a4bb595` | `ed6ff1930baaa28c54428133e748d126493530557e9b5e15b29621dbaa8ab0b2` |
| Tranche-2 partial acceptance (as recorded in the continuation) | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-continue-02/documents/ARCHITECT-RULING-TRANCHE2-PARTIAL-ACCEPTANCE.md` | `54f618029d79a3eaf65c891a4b8eae5a1455f1d0` | `17cf49462a6ea81f4255b97952042772ed42e59fa80db2f139aeedd51b331a59` |
| C5 build-admissibility ruling (as recorded in the continuation) | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-continue-02/documents/ARCHITECT-RULING-C5-BUILD-ADMISSIBILITY.md` | `d85b1bd6b89e4d19b2bf71c52f705232d1793bfa` | `88fde93e8395d405795cbabc67db4b19d8729570d0bea71443cbba40b5c936c0` |

## What this mission is authorized to do (mission sections 2 and 13)

- Execute exactly C2, C24, C3b, C3c, C6, C7, A1, A2, C20, A5, in that order, one mutant at a time, stopping at the first unsuccessful required control.
- Not execute C1, C4, C25 (reserved for the final witness-bound tranche), any of the 22 accepted controls, A4, M01–M183 or any unchanged-core baseline configuration.

## What this record does not do

- It does not accept any of C2, C24, C3b, C3c, C6, C7, A1, A2, C20, A5: their results in this mission are executor candidates, pending Architect review.
- It does not accept D2A-1, complete the campaign, integrate anything or complete Phase Three.
- It changes no receipt, label, catalog entry, patch or source.
