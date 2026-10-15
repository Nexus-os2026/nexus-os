TRANCHE 2 — C23 C10 C16 C26 C3a — MUTATION CONTROL KILL ACCEPTED (17 OF 35); C5 BUILD ARCHITECT-ADMISSIBLE (NARROW C5-SPECIFIC WARNING RULING); C5 KILL PENDING

# Architect ruling record — Mutation Tranche 2 partial acceptance

Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02, from section 1 (ARCHITECT RULING) and section 2 (C5 BUILD RULING) of that mission as issued by the Sole Nexus OS Architect (ChatGPT GPT-6) via the Owner.

This file is evidence of the Architect's ruling.
A string is not authority.

Claude Code does not independently accept any control.

## The ruling, as issued (mission sections 1 and 2)

> Independent review of P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 is complete.
>
> The Sole Architect ACCEPTS these mutation-control kills: C23, C10, C16, C26, C3a.
>
> Previously accepted controls remain: C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b.
>
> Current accepted mutation-control kill count: 17 of 35.
>
> A4 remains separately: SATISFIED — ARCHITECT ACCEPTED
>
> Do NOT rebuild, rerun or regenerate evidence for any accepted control.
>
> The historical executor candidate labels remain unchanged. Architect acceptance comes from this ruling.
>
> C5's existing mutant build is: ARCHITECT-ADMISSIBLE — NARROW C5-SPECIFIC WARNING RULING

## The accepted Tranche-2 evidence (immutable; read from git objects at the Tranche-2 evidence commit; not republished)

Evidence commit `2e06860f4b404d1a79a0d290116239b5545b931f` (tree `4c929aa877816cc0cb5c42e32ddbdd75b380bacd`, single parent `ebe42009ba1bd262cf31ae2f401e02cec101afa7`), review ref `refs/heads/review/p3-c11-d2a1-mutation-tranche2-exec-01`, packet `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/**`.

| Control | Change (catalog) | Configuration; printed property; discovered states | RESULT.txt (historical executor record) | Blob | sha256 | First line (unchanged) |
|---|---|---|---|---|---|---|
| C23 | the publication omits the X bit | F-2; E1; 671 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/receipts/51-C23/RESULT.txt` | `6e4bbaae5e1ce8b63157b9a3408e85b2e20c8cc4` | `a549c43ab79308755a55cc5185ddcb0e015b3ab7fab4cb8f902222fdf2792472` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C10 | `Commit::Unknown` followed by `Rollback::Undone` gives Not written | F-1; E4; 42 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/receipts/52-C10/RESULT.txt` | `c535a404e357e7dec2b306e3ffed262395abc860` | `93820cf47bec3f484a533ad31d3397573e693657ed27999d4472d302237cb0f4` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C16 | the drop of a Counted ticket lowers the count | F-D; E3; 1,513 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/receipts/53-C16/RESULT.txt` | `743cf3867a06af943057dc9ebf3e769d6065a33a` | `5e42903607a229828640cce7db92a0be6bcf7332dde6115e9b0093d7e61e45f3` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C26 | a passed barrier clears the latch | F-4; E9; 3,478 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/receipts/54-C26/RESULT.txt` | `f4505012b964e1f1da47efce8c2293bb85036749` | `f720a2e84d2b0ed74ced2a12b7b8ba0900e541f671a2734023d6563f055b3b70` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C3a | acknowledgement test 2 removed | F-7; P13; 746,152 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/receipts/55-C3a/RESULT.txt` | `d6a4c0645d564bb0be25bd71fed8f80b2e80155a` | `6b70930d1f8ef473697b4ae6517f0901f8ace276d8a99acca67783ad6e9e3bd2` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |

| Control | Mutant (patch sha256; mutant blob) | Pinned mutant executable sha256 | Run classification (blob) | Raw output sha256 | Restored-side link (blob) |
|---|---|---|---|---|---|
| C23 | `731ffbedf845790c681b1fe7a2dc20d53174c8ce7378de04e5963865f0bc04aa`; `cd50b40ce632d4edcb95fff89389246b0d75c51e` | `3549e576ab943bad1cb9ee0a2d96d073924b5225dc0b24abc399b5d6db2b98b9` | `d6f1dfadce972bf50d5cb8c02fe584b7a34453e0` | `eccccfb71cab27d3c9c880c64d07dd2e0db41c50f2abfcc6417638853ce209bd` | `90b40e304ad92764538c41835badb665ac701a04` |
| C10 | `5e73745b2f8a6d02585519bac4990513e3ae6997dec9ad4e478fb92a2464a16c`; `329a830718b81a065e3a03f4958e19e8fa3c8d9c` | `5c8ef9f07b0ef37b430161c9da088bcbe7253ea17a6863ce2677d5f28645e867` | `cd9cc8d439572f422535fe5758337ea0ffbe3c71` | `36c16109f0c1ad0fa54d7db0eb534b8f473c29c656505ea969ecf2a9cda1fd13` | `e9f40ccd3ed0fbdd01b6cb0298824bada87fcf01` |
| C16 | `e48a988cfc85488141d1e9eb033f897135ad9a4509edf16a3df85b55f8ff5e97`; `33f0d257df3e126c589becd0b1c5760c57085be8` | `d53f4f0c7c96364c55c654b98ec6966066e68e30d590da964eb5488a9069ce8c` | `93a23d98c83f084db57bfa5e154dc934b679d2bc` | `ee469a45a015b4ad23914c2ebb23c83a281e47a0136f37a4fcd11b801e1242ba` | `cb1f93f40ca4fbaee1704d7d1c46f190ed934f65` |
| C26 | `1e26422d79a19553d925585aeb580f8a0c684bcc78d8a922ab5ab9a3756e8b4d`; `88f1783305f6418129671e77769fd4caf53e2a88` | `336c30483abf4a0b0672caefdd87e8c7fc504094b36d1129cce4fb90b0663f97` | `2bb850c436b57352457c9cd405f5a48c06680bee` | `0313d7154ad42a4f69f3d8bb2e6b919df5b8e94ab15a45e9bc5998ba15f83faa` | `355d44f80eded7f2f6dfa2b521204da690236dc1` |
| C3a | `2442f63d9383dc0d36dd934e2a6b8fb0b3d6936f215e00ba3d964ede73fb6298`; `5c74cfbd8124a78c616526ae1d461895b1a16b82` | `c71e66debb2f3d42527668f48818c74f5ca48f03aceda67c9b8285b6f8e5a0a6` | `158d224057bd5bbc552c1e48501c1eebec66fdf3` | `b17f69a34ae04752b2d123493d6b8ec8d0b393fd5ca4f8b3de4d52f2d2339000` | `4654bb57a2746abd0ef13229eedb86129c4a20db` |

Every first line above is the executor's historical candidate label; none is relabelled. Acceptance is this ruling's, not the receipts'.

## C5: build Architect-admissible; kill pending

C5's preserved Tranche-2 build (`docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/receipts/56-C5/03-build` at `2e06860f4b404d1a79a0d290116239b5545b931f`) is ARCHITECT-ADMISSIBLE under the narrow C5-specific warning ruling (record `documents/ARCHITECT-RULING-C5-BUILD-ADMISSIBILITY.md`). The C5 kill is still pending: it requires this mission's C5 run and Architect review. The historical frozen result `BUILD: NOT OK` and C5's historical `CONTROL RESULT: MUTATION_CONTROL_BLOCKED` remain immutable executor history and are not edited.

| Identity | Value |
|---|---|
| patch sha256 | `a4776885b669bd912fd8710d5a3283f4c1863ab9213e8474d26707b5a6e5f3d8` |
| mutant protocol.rs blob | `e1a49f5ba952bc81cdac9f8f8ace9eeb2aa93495` |
| mutant protocol.rs sha256 | `31ca02cfaeb6058879af75091e2b1bdb21addf846d59ee3e5b4f5806fe30dedd` |
| executable | `/home/nexus/NEXUS/nexus-os-p3-c11-mt2x-C5/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` |
| executable sha256 | `90a608834ad0227632ba26f095de5e16914942da86d2f05fdaab5a0d745c15f9` |
| executable size | 8,986,720 bytes |
| build stderr sha256 | `4b7bdea581849cc8a980ab637228c2e19afd7a8f12704e8a44a2dc922663e8dc` |
| `CLASSIFICATION.txt` (blob; sha256) | `86e68a5edc72c381c0f3c57849c123fe36b868af`; `b69d2d68af6e61d710ac8fcfbdf1d4e71b1eb459567bbc712525a5a2e8751685` |
| `build.stderr` (blob; sha256) | `4a3810aa7144f1bba024caccd951e821f3a8cfef`; `4b7bdea581849cc8a980ab637228c2e19afd7a8f12704e8a44a2dc922663e8dc` |
| `descriptor.json` (blob; sha256) | `80d81f8252187c4f43355f9f20c0438c097a2ddb`; `b9028310bb865c84b349bb277dd5a362cbd43a8214ee728901a525a01e250d0a` |
| C5 `RESULT.txt` (blob; sha256; historical) | `c71d14c9e932dfa95da38c8c7782dd4363be30a2`; `9c8256f86e593223f7b9c8ce829deecebfeae87acaab8a62f8be63dafc036845` |

## Campaign count

Architect-accepted mutation-control kills: **17 of 35** — Tranche 1: C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b; Tranche 2: C23, C10, C16, C26, C3a. A4: SATISFIED — ARCHITECT ACCEPTED (separately; not kill-required).

| Earlier record | Path | Blob | sha256 |
|---|---|---|---|
| Tranche-1 final acceptance (as recorded in Tranche 2) | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/documents/ARCHITECT-RULING-TRANCHE1-FINAL-ACCEPTANCE.md` | `c82adabae323eaada06fc24ed5b6faeaedb41485` | `e8859092ad0de7a4a357486172a4c2704946e70fb8510a43c383f9384a9cf04b` |
| Tranche-2 stop record | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/receipts/80-STOP-RECORD.md` | `adbd3308bebdd5a08779825748c10c0aa65e8bab` | `bb9546d2a81c13175a1007308affda5ca743ba2e93cf31ea43c57dd2856c8fc5` |
| Tranche-2 INDEX.md | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/INDEX.md` | `64f10a1aee8d831e5bbc14ae458b1d9a9e56e090` | `ae5dc9753d53a510876a75334734c6787728cbf352498307dc637a709787fc11` |
| Tranche-2 SHA256SUMS | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/SHA256SUMS` | `767d0b547ffe10e80c5b5ddf64ce4e8afce439ee` | `01a713bdfb1404f7b503d8b0e0a2178e9388988be8dbc471119386b8ae120cd7` |

## What this record does not do

- It does not accept C5, A3, A6, C8 or C15: their results in this mission are executor candidates, pending Architect review.
- It does not accept D2A-1, complete the campaign, integrate anything or complete Phase Three.
- It changes no receipt, label, catalog entry, patch or source.
