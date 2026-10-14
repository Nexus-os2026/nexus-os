SEVEN CONTINUE-02 LINK RECEIPTS — DUPLICATE CONTENT PERMITTED FOR PUBLICATION (NARROW: EXACT PATHS; NO OTHER DUPLICATE EXCEPTION; CANDIDATE ACCEPTANCE PENDING)

# Architect ruling record — duplicate link receipts (CONTINUE-02 publication)

Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-03, from sections 1, 5 and 6 of that mission as issued by the Sole Nexus OS Architect (ChatGPT GPT-6) via the Owner.

This file is evidence of the Architect's ruling.
A string is not authority.

## The ruling

Mission P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02 completed all authorized control executions but stopped when its frozen packet builder rejected seven byte-identical link receipts (`publication/10-build-packet.txt`, `receipts/80-STOP-RECORD.md`). The Sole Architect ruled:

> PACKET DUPLICATE-CONTENT FAILURE — FALSE POSITIVE
>
> The seven files are genuine current-mission receipts. Their identical bytes are expected because the unchanged link() function linked the same immutable Stage-2 unchanged-core evidence with the same source/executable policy. Distinct repository paths, surrounding control-specific restoration receipts and control-specific result records bind each receipt to its control.

Publication is authorized under the narrow policy of the mission's section 6: duplicate content is permitted only for the seven exact current files below, each with an explicit, verified per-file mapping. No control may be rebuilt or rerun; no existing raw receipt, classification, result, build output, execution output, restoration receipt or link receipt may be edited.

## The seven exact current receipts and their prior immutable files

Prior commit: EXEC-01 evidence commit `19eda8cc7f559054933fed4a93bdb0a18cc725d4`; prior paths under `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/`. Current paths under the CONTINUE-02 packet `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/`.

| # | current path | control | bytes | current git blob | current sha256 | prior path | prior git blob | prior sha256 | equal |
|---|---|---|---|---|---|---|---|---|---|
| 1 | `receipts/38-C9/06-link/LINK.json` | C9 | 1395 | `6ff1b4d66395956d20a45623802bd455022eb037` | `4a69c8e7d4ea9574b4350378de65bdeb584e019fa62002c35c61d08f112a98f6` | `receipts/04-rehearsal/links/link-C9-stage2/LINK.json` | `6ff1b4d66395956d20a45623802bd455022eb037` | `4a69c8e7d4ea9574b4350378de65bdeb584e019fa62002c35c61d08f112a98f6` | blob and sha256 equal; bytes equal |
| 2 | `receipts/38-C9/06-link/LINK.txt` | C9 | 1364 | `fa93af3bda6ec9589e015708a5402c6dd876f1cd` | `32439bccad6c9e7cf7152cad990f8c2b84dda9867afa3095a7401676365f7176` | `receipts/04-rehearsal/links/link-C9-stage2/LINK.txt` | `fa93af3bda6ec9589e015708a5402c6dd876f1cd` | `32439bccad6c9e7cf7152cad990f8c2b84dda9867afa3095a7401676365f7176` | blob and sha256 equal; bytes equal |
| 3 | `receipts/39-C11/06-link/LINK.json` | C11 | 1395 | `6ff1b4d66395956d20a45623802bd455022eb037` | `4a69c8e7d4ea9574b4350378de65bdeb584e019fa62002c35c61d08f112a98f6` | `receipts/04-rehearsal/links/link-C9-stage2/LINK.json` | `6ff1b4d66395956d20a45623802bd455022eb037` | `4a69c8e7d4ea9574b4350378de65bdeb584e019fa62002c35c61d08f112a98f6` | blob and sha256 equal; bytes equal |
| 4 | `receipts/40-C17/06-link/LINK.json` | C17 | 1395 | `6ff1b4d66395956d20a45623802bd455022eb037` | `4a69c8e7d4ea9574b4350378de65bdeb584e019fa62002c35c61d08f112a98f6` | `receipts/04-rehearsal/links/link-C9-stage2/LINK.json` | `6ff1b4d66395956d20a45623802bd455022eb037` | `4a69c8e7d4ea9574b4350378de65bdeb584e019fa62002c35c61d08f112a98f6` | blob and sha256 equal; bytes equal |
| 5 | `receipts/41-C18a/06-link/LINK.json` | C18a | 1391 | `ae664a2c66abc29ff2393805c74602e70e79f074` | `86b80a7962f697690fd17604d4328fb5e3fc2792e4168f36a4c01cac4bb55369` | `receipts/04-rehearsal/links/link-C18a-stage2/LINK.json` | `ae664a2c66abc29ff2393805c74602e70e79f074` | `86b80a7962f697690fd17604d4328fb5e3fc2792e4168f36a4c01cac4bb55369` | blob and sha256 equal; bytes equal |
| 6 | `receipts/41-C18a/06-link/LINK.txt` | C18a | 1362 | `6319e3fe28f40379a87d650ae9e9a55957134390` | `dabbaf663bcf828b471c2f53912a2640766f435f43e87ecc2e79e2640de6ecb5` | `receipts/04-rehearsal/links/link-C18a-stage2/LINK.txt` | `6319e3fe28f40379a87d650ae9e9a55957134390` | `dabbaf663bcf828b471c2f53912a2640766f435f43e87ecc2e79e2640de6ecb5` | blob and sha256 equal; bytes equal |
| 7 | `receipts/42-C18b/06-link/LINK.json` | C18b | 1391 | `ae664a2c66abc29ff2393805c74602e70e79f074` | `86b80a7962f697690fd17604d4328fb5e3fc2792e4168f36a4c01cac4bb55369` | `receipts/04-rehearsal/links/link-C18a-stage2/LINK.json` | `ae664a2c66abc29ff2393805c74602e70e79f074` | `86b80a7962f697690fd17604d4328fb5e3fc2792e4168f36a4c01cac4bb55369` | blob and sha256 equal; bytes equal |

## Why the equality is expected

- The unchanged `link()` (source sha256 `c5576ffbd0aa62dbefe6898683e23c2fda32d02962f9882eca80a98880886c4b`, identical in the EXEC-01 packet, RESUME-01 and CONTINUE-02 `tools/control.py`: True) is deterministic over the linked accepted Stage-2 receipt and the pinned unchanged-baseline executable's sha256; it reads nothing of the mutant.
- EXEC-01's pre-freeze rehearsal ran `link()` on the real Stage-2 F-B2 receipt (for C9) and F-G receipt (for C18a); CONTINUE-02 linked C9, C11, C17 to the same F-B2 receipt (`receipts/61-full-F-B2`) and C18a, C18b to the same F-G receipt (`receipts/60-full-F-G`) at Stage-2 commit `89326b859792a442c1853c3d35d30a48301c009e`, with the same baseline executable `cde9f9d2…`.
- `LINK.json` carries no control id: all three F-B2 `LINK.json` and both F-G `LINK.json` are equal. `LINK.txt` names the control: only C9's and C18a's equal EXEC-01's rehearsal outputs for C9 and C18a; C11's, C17's and C18b's `LINK.txt` are unique and are not covered by (and need no) exception.

## Why the distinct current paths remain meaningful

Each receipt sits in its own control's receipt directory (`receipts/NN-<control>/06-link/`), written after that control's own verified restoration, next to that control's own restoration receipt (`RESTORE: EXACTLY_C`, naming the control and its mutant's before/after blobs), control result (`CONTROL-RESULT.json`, naming the control and this link) and run classification. Equal bytes record an equal linked baseline; the binding to the control is by path and by these surrounding control-specific receipts, which `DUPLICATE-CONTENT-MAP.tsv` names with their sha256 and the repaired builder verifies.

## Scope

- Exactly the seven current paths above, each verified field by field. No other duplicate exception is granted: no directory-pattern, basename, generic `06-link`, generic JSON/TXT, size-threshold or deterministic-file exception.
- Duplicate content remains rejected by default; any other nonempty staged file byte-equal to a previous-packet file is outside this ruling.
- Publication only: nothing is rebuilt, rerun, reclassified or edited.

## Candidate status (unchanged by this ruling)

- C28, C12, C9, C11, C17, C18a, C18b remain executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`), pending independent Architect review. This ruling accepts none of them.
- Previously Architect-accepted (CONTINUE-02 `ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md`, sha256 `cf9bf865e01c8f8b359916eae75f29147132ff67465f93b1c08473c0a8b0d776`): C22, C14, C19, C21, C27.

