# Duplicate-content map — the seven ruled CONTINUE-02 link receipts

Mission P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-03, sections 6 and 7. Machine-readable form: `DUPLICATE-CONTENT-MAP.tsv` (one header line, seven rows, tab-separated); the repaired packet builder (`publication-03/tools/build_packet.py`, `RULED` and `dup_policy`) verifies every field of every row against the staged bytes, the immutable EXEC-01 commit and the control's own receipts, and rejects any other nonempty staged file byte-equal to a previous-packet file.

Current paths are packet-relative (`docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02/`); they are the CONTINUE-02 receipt paths, not renamed.

## Byte identity

| # | current path | control | bytes | current sha256 / git blob | prior (EXEC-01 `19eda8cc`) path | prior git blob / sha256 | byte-equal |
|---|---|---|---|---|---|---|---|
| 1 | `receipts/38-C9/06-link/LINK.json` | C9 | 1395 | `4a69c8e7d4ea9574b4350378de65bdeb584e019fa62002c35c61d08f112a98f6` / `6ff1b4d66395956d20a45623802bd455022eb037` | `receipts/04-rehearsal/links/link-C9-stage2/LINK.json` | `6ff1b4d66395956d20a45623802bd455022eb037` / `4a69c8e7d4ea9574b4350378de65bdeb584e019fa62002c35c61d08f112a98f6` | yes |
| 2 | `receipts/38-C9/06-link/LINK.txt` | C9 | 1364 | `32439bccad6c9e7cf7152cad990f8c2b84dda9867afa3095a7401676365f7176` / `fa93af3bda6ec9589e015708a5402c6dd876f1cd` | `receipts/04-rehearsal/links/link-C9-stage2/LINK.txt` | `fa93af3bda6ec9589e015708a5402c6dd876f1cd` / `32439bccad6c9e7cf7152cad990f8c2b84dda9867afa3095a7401676365f7176` | yes |
| 3 | `receipts/39-C11/06-link/LINK.json` | C11 | 1395 | `4a69c8e7d4ea9574b4350378de65bdeb584e019fa62002c35c61d08f112a98f6` / `6ff1b4d66395956d20a45623802bd455022eb037` | `receipts/04-rehearsal/links/link-C9-stage2/LINK.json` | `6ff1b4d66395956d20a45623802bd455022eb037` / `4a69c8e7d4ea9574b4350378de65bdeb584e019fa62002c35c61d08f112a98f6` | yes |
| 4 | `receipts/40-C17/06-link/LINK.json` | C17 | 1395 | `4a69c8e7d4ea9574b4350378de65bdeb584e019fa62002c35c61d08f112a98f6` / `6ff1b4d66395956d20a45623802bd455022eb037` | `receipts/04-rehearsal/links/link-C9-stage2/LINK.json` | `6ff1b4d66395956d20a45623802bd455022eb037` / `4a69c8e7d4ea9574b4350378de65bdeb584e019fa62002c35c61d08f112a98f6` | yes |
| 5 | `receipts/41-C18a/06-link/LINK.json` | C18a | 1391 | `86b80a7962f697690fd17604d4328fb5e3fc2792e4168f36a4c01cac4bb55369` / `ae664a2c66abc29ff2393805c74602e70e79f074` | `receipts/04-rehearsal/links/link-C18a-stage2/LINK.json` | `ae664a2c66abc29ff2393805c74602e70e79f074` / `86b80a7962f697690fd17604d4328fb5e3fc2792e4168f36a4c01cac4bb55369` | yes |
| 6 | `receipts/41-C18a/06-link/LINK.txt` | C18a | 1362 | `dabbaf663bcf828b471c2f53912a2640766f435f43e87ecc2e79e2640de6ecb5` / `6319e3fe28f40379a87d650ae9e9a55957134390` | `receipts/04-rehearsal/links/link-C18a-stage2/LINK.txt` | `6319e3fe28f40379a87d650ae9e9a55957134390` / `dabbaf663bcf828b471c2f53912a2640766f435f43e87ecc2e79e2640de6ecb5` | yes |
| 7 | `receipts/42-C18b/06-link/LINK.json` | C18b | 1391 | `86b80a7962f697690fd17604d4328fb5e3fc2792e4168f36a4c01cac4bb55369` / `ae664a2c66abc29ff2393805c74602e70e79f074` | `receipts/04-rehearsal/links/link-C18a-stage2/LINK.json` | `ae664a2c66abc29ff2393805c74602e70e79f074` / `86b80a7962f697690fd17604d4328fb5e3fc2792e4168f36a4c01cac4bb55369` | yes |

## Binding of each receipt to its own control

| # | control | linked configuration | linked accepted baseline (at `89326b85`) | restoration receipt (sha256) | control result (sha256) | run classification (sha256) |
|---|---|---|---|---|---|---|
| 1 | C9 | F-B2 | `receipts/61-full-F-B2` | `receipts/38-C9/05-restore/RESTORE.txt` (`19ee70c2d1243f94…`) | `receipts/38-C9/CONTROL-RESULT.json` (`9b567c7407c2711c…`) | `receipts/38-C9/04-run/CLASSIFICATION.txt` (`604137105a1ead93…`) |
| 2 | C9 | F-B2 | `receipts/61-full-F-B2` | `receipts/38-C9/05-restore/RESTORE.txt` (`19ee70c2d1243f94…`) | `receipts/38-C9/CONTROL-RESULT.json` (`9b567c7407c2711c…`) | `receipts/38-C9/04-run/CLASSIFICATION.txt` (`604137105a1ead93…`) |
| 3 | C11 | F-B2 | `receipts/61-full-F-B2` | `receipts/39-C11/05-restore/RESTORE.txt` (`7d656542da5cb83d…`) | `receipts/39-C11/CONTROL-RESULT.json` (`6c4189cc3a41d369…`) | `receipts/39-C11/04-run/CLASSIFICATION.txt` (`fe29919bacb81509…`) |
| 4 | C17 | F-B2 | `receipts/61-full-F-B2` | `receipts/40-C17/05-restore/RESTORE.txt` (`63ad2ccf2db7b8ca…`) | `receipts/40-C17/CONTROL-RESULT.json` (`50ed70fb7c75a30a…`) | `receipts/40-C17/04-run/CLASSIFICATION.txt` (`29a5207fc65d42f3…`) |
| 5 | C18a | F-G | `receipts/60-full-F-G` | `receipts/41-C18a/05-restore/RESTORE.txt` (`7aed51680d506905…`) | `receipts/41-C18a/CONTROL-RESULT.json` (`cfbd6822c8511b95…`) | `receipts/41-C18a/04-run/CLASSIFICATION.txt` (`8ab8fc83aa588161…`) |
| 6 | C18a | F-G | `receipts/60-full-F-G` | `receipts/41-C18a/05-restore/RESTORE.txt` (`7aed51680d506905…`) | `receipts/41-C18a/CONTROL-RESULT.json` (`cfbd6822c8511b95…`) | `receipts/41-C18a/04-run/CLASSIFICATION.txt` (`8ab8fc83aa588161…`) |
| 7 | C18b | F-G | `receipts/60-full-F-G` | `receipts/42-C18b/05-restore/RESTORE.txt` (`16422d7996117fac…`) | `receipts/42-C18b/CONTROL-RESULT.json` (`420b3ccc6ab9874c…`) | `receipts/42-C18b/04-run/CLASSIFICATION.txt` (`225eb014b151def8…`) |

Full sha256 values are in the TSV. Each restoration receipt states `RESTORE: EXACTLY_C` and names its control; each control result names its control and the same linked reference; each run classification is the control's own `KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` candidate classification.

## Why the bytes are equal

`link()` is the same function in every mission that ran it (its source, sha256 `c5576ffbd0aa62dbefe6898683e23c2fda32d02962f9882eca80a98880886c4b`, is identical in EXEC-01 packet tools/control.py, RESUME-01 tools/control.py (CONTINUE-02 original-tools/control.py), CONTINUE-02 tools/control.py: True). It writes `LINK.json`/`LINK.txt` from the linked accepted Stage-2 receipt (path and commit), its seven check labels and results, and the sha256 of the pinned unchanged-baseline executable (`cde9f9d2…`); none of these depend on the mutant. `LINK.json` carries no control id, so the three F-B2 controls (C9, C11, C17) and the two F-G controls (C18a, C18b) write the same `LINK.json`; `LINK.txt` names the control, so only C9's and C18a's coincide with EXEC-01's pre-freeze link rehearsals of C9 and C18a (`receipts/04-rehearsal/links/link-C9-stage2/`, `link-C18a-stage2/`).

