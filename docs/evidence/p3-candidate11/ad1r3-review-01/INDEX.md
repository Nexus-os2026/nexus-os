# AD1R3 review packet — P3-C11-DR01-D2A1-AD1R3-READINESS-01

**Status: review materials only.** AD-1-R3 is a DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED. Nothing here is a
mutation kill, D2A-1 acceptance, adoption readiness or Phase Three completion.

Prepared by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort high) for the Sole Architect's review.

## Identities

| Item | Identity |
|---|---|
| Engineering source | `42dd1bd1cf21ffdea5968b256d2983f6575fab5b` (tree `7ac6bb21430d15b17e3723b217b20a49dccf8282`); `protocol.rs` blob `a2df2841b99cad5d452008a6d4150e9369d07e66`; `model_tests.rs` blob `5ad26ddc727d8be30f4fb8d7478bc69e1c974933` |
| Parent of this packet's commit | evidence-transport commit `8693e3a742ee4d3917cbfdb809b377bcd919d9f4` (tree `1e9568ddb5a40336820d266652643d6067cd33f6`); not an engineering baseline |
| AD-1-R2 (history) | blob `8505fb80867f787e29a20d8f4f3b121cee77213b`, sha256 `53cdb3d6ea8683a11ccaf90a6671a37b42dd5db178b599cfeb421b898a089ebc`, at `docs/evidence/p3-candidate11/ad1r2-review-packet-01/ad1r2/` (unchanged) |
| AD-1-R3 (this packet) | `ad1r3/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R3-DRAFT.md`, sha256 `485d9256932220644d7368d64b7b9b4d8df8a3a5c65aba4a1c5fcdf390ae286e`, 106,365 bytes |
| Contract | sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c` (in the earlier packet; unchanged) |
| Binding reviews | `6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce`, `1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35` (in the earlier packet) |
| controls_defs.py | original `/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-campaign-01/tools/stage5-prepared-NOT-EXECUTED/controls_defs.py`, sha256 `ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f`, sealed in CAMPAIGN-01's SHA256SUMS; carried here as `inert/controls_defs.py.txt` (byte-identical) |

## Where to start

1. `ad1r3/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R3-DRAFT.md`, sections 0A.1–0A.2, 1 (S-1, S-2, S-6, S-14, S-19 to S-21),
   F.1, G.3, K.1, K.2, O.4, R and the open items; `ad1r3/AD1-R2-to-AD1-R3.diff` for every change.
2. `preflight/C9-C10-C24-C4-ATTRIBUTION-PREFLIGHT.md`: C10 and C24 have sealed, unapproved definitions; C9 and C4 have
   none. Predicted first failures: C10 E4, C24 P8, C9 E4 (named in each case); C4 P4 only, so "P4, P8" needs the
   Architect's reading.
3. `receipts/04-final-gate.txt` and `checks/`.

## Verify

From this directory: `sha256sum -c PACKET-SHA256SUMS` (covers every file but itself; INDEX.md included).
The tools reproduce the draft and the checks from the authenticated inputs named in `receipts/03-freeze.txt`; they
read the contract and reviews from the earlier packet's byte copies or the sealed originals.

## Files

| Path | Bytes | sha256 | What |
|---|---|---|---|
| `ad1r3/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R3-DRAFT.md` | 106,365 | `485d9256932220644d7368d64b7b9b4d8df8a3a5c65aba4a1c5fcdf390ae286e` | AD-1-R3 DRAFT. NOT ACCEPTED, NOT ADOPTED, NOT APPLIED. |
| `ad1r3/AD1-R2-to-AD1-R3.diff` | 50,759 | `58a193eb927bf80c46958bac4cf8d3ffebe7de6feaa7c88765bfbf2ab2ed5dd0` | unified diff, AD-1-R2 (53cdb3d6…) to AD-1-R3 |
| `preflight/C9-C10-C24-C4-ATTRIBUTION-PREFLIGHT.md` | 22,159 | `a80339e15a253d4df199e0a9d8731f0a30ff513b26391b29a634d3ca95a3910b` | read-only attribution preflight; predictions from source, nothing executed |
| `inert/controls_defs.py.txt` | 7,402 | `ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f` | INERT byte copy of CAMPAIGN-01's sealed controls_defs.py; never imported or executed |
| `checks/01-ad1r3-mechanical-checks.txt` | 33,207 | `e2cc564423211b51915a23d452d49f2b0326519ef8faf3129e6d64b27cd7380a` | final gate G3: AD-1-R3 checker output |
| `checks/02-r2-to-r3-diff-review.txt` | 11,524 | `f28b0935005f287ca383e27fb8582439cc2156dd4b821357560905737635e0a3` | executor's R2->R3 diff review; 30 scan items accounted for |
| `checks/03-defs-anchor-check.txt` | 1,549 | `1f292847578d243b1bc0275c55f6c56725799ffd1406433f2bbd197591db7599` | final gate G6: sealed definitions C9/C10/C24/C4 vs protocol.rs |
| `checks/04-preflight-checks.txt` | 7,491 | `0b6fa6dba7816e2d1f449806a89ab46b82525eef558aa25f95852dc5bb73fd86` | final gate G7: every preflight source citation checked |
| `checks/05-classification-cases.txt` | 3,451 | `c00ef8889f6fb38bdfd32713322be0ead011042433f09c1f00fabb1a927d38cc` | final gate G5: K.2 classification cases (synthetic) |
| `checks/06-checker-negative-controls.txt` | 2,756 | `47f256457fa5d66e6630dc91c8ca93d17a72b0563d6be9351c1a2634c8a39f29` | final gate G4: checker negative controls |
| `checks/07-r2-r3-obligation-scan.txt` | 4,529 | `a3754fd5be7e64e01027b58df5dc71978a0fd4485c702567989bb8949e8b152d` | final gate G8: R2 sentences not verbatim in R3 |
| `receipts/00-STAGE1-AUTHENTICATION.txt` | 4,354 | `5ff744b573e5140bf405ef0388a7ad0499ce1954e3cf9a1c5d53dab080a0aca1` | provenance and sealed-input authentication |
| `receipts/01-worktree-authentication.txt` | 1,155 | `4bea542df5344e48de4c0f54d3f1fbe801628da34596b8bb92319386d85f66a9` | mission worktree authentication |
| `receipts/02-development-log.txt` | 14,854 | `2ee88ed41d56a9db45a805d65d4541eea87707d32d4ee07d395ea2c373e332c5` | every development run, failure and checker revision |
| `receipts/03-freeze.txt` | 2,427 | `ea37eac9e50b32e407cedf694044a6d1b228dbfe274463f5cccbd1ee9d10e1a2` | freeze record: frozen inputs and the declared gate |
| `receipts/04-final-gate.txt` | 7,229 | `cac0917df7cb9069459f4de100dd970c625220ee0e4dc58747ad8ae348e1ae22` | the final mechanical gate: commands and exit codes |
| `receipts/github-refs-before.txt` | 13,691 | `1b5b1a4095fd5f476c2c126a27296394efb6119d770e34c5fb904a2c902c9a73` | git ls-remote github before any change |
| `tools/ad1r3-template.md` | 98,843 | `59c7c0095943b1716faf40af11febfd68aabcd5f551f04ebbf69fe79671c5f00` | the draft's template (placeholders for verbatim lines) |
| `tools/fill-ad1r3.py` | 2,710 | `3b52b9a8cfde0081966e0f77d632db659d1ed2f426b05003627333f3304e08bf` | fills verbatim blocks from the authenticated bytes |
| `tools/check-ad1r3.py` | 39,145 | `7afda8b3930992c1c3b74e41247434873f861ca5915ab8fbf6510c32b7ebfb01` | independent mechanical checker (v3, frozen) |
| `tools/checker-negative-controls.py` | 4,621 | `53b92d5fcb232f20252a04f4b8733968f83cac0851cb8801f2ac66a69ccc809e` | seeded corruptions the checker must catch (v2, frozen) |
| `tools/classify-cases.py` | 12,820 | `9f73bab9464ced4de45b51181ca040029f6bcd29740facb3c5e299f4a786f5af` | K.2 rules on synthetic receipt descriptors |
| `tools/defs-anchor-check.py` | 3,323 | `2814e63f9b40054682d385ae77b4b38b859eb4ccc5a7fcafbfe55b39ee2e5f21` | reads controls_defs.py with ast only; counts anchors |
| `tools/check-preflight.py` | 11,460 | `55271a845059573c06aa3453890262be59226f6b01100eecfbdd229c0a47f969` | preflight citation checker (v2, frozen) |
| `tools/r2-r3-obligation-scan.py` | 1,080 | `5f93652666ee4a26c4637091a9c2bae07d0b68c1a8d8db85f2ce8ba21c617c7c` | obligation-sentence scan (AD1R2's logic, unchanged) |
| `tools/final-gate.sh` | 5,288 | `7338a186e027e993ff334ab7307b655ea12055785f8004b8d722a82785a46bc4` | the declared final gate |
| `tools/stage1-auth.sh` | 6,456 | `0d423d21058916cb7271a08ae992d646683a2e6479393027698166a8688b2fa7` | stage 1 authentication script |
| `tools/build-packet.py` | 10,565 | `dedc21eff81ba899e13904ff325fcba4fa5a3459ba1c25f928940097b8134df6` | this builder |
| `tools/revisions/check-ad1r3.v1.py` | 37,581 | `18fc4bcee3920c906cd8de7d057a3368add4ba95d99bae25b296dd1657086473` | checker revision history: v1 |
| `tools/revisions/check-ad1r3.v2.py` | 38,711 | `50062c3a40466ecd2aa894cc6e4142d0abb589e9e7e7a233fbde48c1158cd89c` | checker revision history: v2 (reconstructed; hash equals the logged one) |
| `tools/revisions/checker-negative-controls.v1.py` | 4,412 | `255939156e3fe7918e4a680fd4c72102171f21bbb21752acfb67b71fd83ae721` | negative controls v1 |
| `tools/revisions/check-preflight.v1.py` | 11,460 | `659302851eb680fe4623cb12fa2fa60c3dcb1b9830901926e5a80c17ef25ab6e` | preflight checker v1 |
| `tools/revisions/build-packet.v1.py` | 10,303 | `7a9263ef3e105e4de83adc5a27638ccdcf000e36a7fd2d62ea854b9d6f257af2` | builder v1 (crashed at its credential regex; receipts/05) |
| `receipts/05-packet-build-1-failed.txt` | 1,511 | `3484223c5850ca5faf3a2426e90efa75b18a20f1ed68386986a31c5c3e2894fe` | the failed first build (builder v1) |
