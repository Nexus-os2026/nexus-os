# D2A-1 control definitions R2 — index

Mission P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01. Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. For the Sole Architect's review.

**Status: READY_FOR_CONTROL_DEFINITION_R2_ARCHITECT_REVIEW.**

Static definition work only. No Cargo, no rustc, no build, no test, no explorer; no patch applied to any worktree; no mutant built or run. Candidate C is unchanged; the R1 packet is unchanged. The catalog is a **candidate**: every control's definition is CANDIDATE_READY, its Architect review pending, its execution NOT_ATTEMPTED. No kill is claimed. D2A-1 remains unaccepted; Candidate 11 is not integrated; Phase Three remains active.

R2 repairs the one blocking defect the Architect's review of R1 found: A6 added `recovery_required` to the state key but left `KEY_FIELDS`, the field list every exploration report prints, unchanged. The R2 A6 patch is the R1 A6 patch with every semantic hunk kept byte for byte and exactly one receipt-metadata hunk added in `KEY_FIELDS`. The other 34 patches and their catalog entries are R1's, byte for byte. The definitions record carries R1's forward: **ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING** (`ARCHITECT-CONTROL-DEFINITIONS-R2.md`).

## Identities

| Item | Value |
|---|---|
| Candidate C | `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`, single parent `71264aa81a40b86189783facc8437e0427887c16` |
| Blobs at C | `protocol.rs` `a2df2841b99cad5d452008a6d4150e9369d07e66`; `model_tests.rs` `28085ce232827301104b5d757af042ec933976f8`; `unit_tests.rs` `55dc66409eaccdaa93e8f0a56f2ccb14be81e114` |
| R1 evidence commit (this commit's parent) | `f81159046bf0b62666851bc1185992318302d31f`, tree `234ca0990434485f59ebda0ceff3086d4e66d797`, parent `59df21036f7c0183fa17d76b6e0886f8f5784685`, ref `refs/heads/review/p3-c11-d2a1-control-definitions-r1-01` |
| R1 catalog (immutable review history) | `docs/evidence/p3-candidate11/d2a1-control-definitions-r1-01/catalog/catalog.json`, `d2a1-controls-v1-r1-candidate`, git blob `56d4424397be7740cefd8d89b3530aab4b25d3ed`, sha256 `a73d5721ec59a5d14abf5c31db72d1da1d34962d1db7294347797dd677ae6479` |
| R1 definitions record | `docs/evidence/p3-candidate11/d2a1-control-definitions-r1-01/ARCHITECT-CONTROL-DEFINITIONS-R1.md`, git blob `b947621bb0d56e1e14780853e886c7e832d7f6a3`, sha256 `beb9f5e3129bbd559dfa375251bddeac17f78d3f1b79082a42767944d7d48bf0` |
| **R2 candidate catalog** | `docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/catalog/catalog.json`, version `d2a1-controls-v1-r2-candidate`, git blob `1918374263740d2f98640da00fc466737945975e`, catalog sha256 `d6c05a7cdc71b3ca1812b5199b8529cbe6db0363ca11bacc26d1f21c2cd11a83` |
| **R2 definitions record** | `docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/ARCHITECT-CONTROL-DEFINITIONS-R2.md`, git blob `c79b75b65b85e1fad53592e92ab2d8507d6c8799`, definitions record sha256 `1da1ae92f313496dade2bfea5c508af1e17bf2e39b9bd11cca3e448c35f0cb26` |
| **R2 A6 patch** | `docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/catalog/patches/A6.patch`, git blob `150c8d66d74bbfbe32d2708602af03ac4dc152c5`, sha256 `0700052eb3e9fb91f36ee6d535b02b923ff007f1061490ac7dd16dd98d0d0ee8`; mutant blob `51e54aed7cc33ebc8348a74fe6c881c057336ee7`, mutant sha256 `59c2f87666ad3272661497e30d7a4a841dcc5aafd90569307cc4fdbd69b5a4b4` |
| Contract | sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c` |
| AD-1-R4 | sha256 `00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693` |
| AD-1-R5 | sha256 `4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473` |

## The A6 correction

- R1 A6: patch sha256 `02e1fd1b60d4b1255079f5533072f5a213a92f8db9214984eb8b527975b76e86`, mutant blob `85d262dbdc445569951ec43e914de4c192ea675d`, 8 hunks, 8 edits, lines removed/added 5/21.
- R2 A6: patch sha256 `0700052eb3e9fb91f36ee6d535b02b923ff007f1061490ac7dd16dd98d0d0ee8`, mutant blob `51e54aed7cc33ebc8348a74fe6c881c057336ee7`, 9 hunks (R1's 8 byte for byte + 1), 9 edits (R1's 8 + 1), lines removed/added 6/22.
- The added hunk, in `KEY_FIELDS` (model_tests.rs line 1172 at C): `the test's table (classes, debt counter, H, A);` → `the test's table (classes, debt counter, H, A, recovery_required);`; nothing else. It is receipt metadata, not a second semantic mutation: the R1 mutant and the R2 mutant differ in that one line.
- Exact differences: `comparison/A6-R1-TO-R2-PATCH.diff`, `comparison/A6-R1-TO-R2-MUTANT.diff`.

## Results

| Step | Result | Record |
|---|---|---|
| Input authentication (Candidate C, the R1 packet, sources, refs) | ALL CHECKS OK (195 checks) | `receipts/00-authentication.txt` |
| R2 candidate catalog | 35 kill-required, all CANDIDATE_READY, 0 blocked; A4 separate; changed from R1: A6 only | `catalog/` |
| R1 → R2 equality | 34 patches byte-identical and 34 entries equal; A6 the only difference | `comparison/R1-TO-R2-PATCH-COMPARISON.md`, `receipts/20`, `21` |
| Checker A (bytes/text; GNU patch, git apply; R1 from the authenticated copies) | PASS, 923 checks | `receipts/20-checker-a.txt` |
| Checker B (independent: strict diff engine, tokenizer; R1 from the R1 commit's git objects) | PASS, 750 checks | `receipts/21-checker-b.txt` |
| Checker agreement | identical per-id verdict maps (38 ids, all PASS) | `receipts/20`, `21`, `40` |
| Syntax (rustfmt --check, disposable copies) | all 21 SYNTAX_CHECKED_NOT_TYPECHECKED (A6 in its R2 form); C's files pass; an unparsable control fails | `receipts/30-syntax-check.txt` |
| Negative controls | 42 seeded defects (R1's 29 and R2's 13), each detected by both checkers | `receipts/40-negative-controls.txt` |
| Freeze record (packet bytes; no catalog status) | every packet file hashed before the final gate | `receipts/50-FREEZE-RECORD.json` |
| Final gate | see its receipt | `receipts/60-final-gate.txt` |

## The 35 patch identities (R2; R1 equality)

| Id | Source | File | Patch sha256 | Mutant blob | = R1 |
|---|---|---|---|---|---|
| C1 | CARRIED_FORWARD | `protocol.rs` | `5ecd716a8f123d61034b0d274238898e13a116a9cc464c7645a9736934970252` | `7d95dd425fcdf17b12dda2fdce461f97ba7b9a66` | yes |
| C2 | CARRIED_FORWARD | `protocol.rs` | `f02c3e721aa2c96bb7d15fe081acf3dc6e4c1958de096f4215bc55a7cafc218d` | `1c10adbc93c2fe0984d0ac677727a3bdc82b8e5d` | yes |
| C3a | ARCHITECT_DEFINED | `protocol.rs` | `2442f63d9383dc0d36dd934e2a6b8fb0b3d6936f215e00ba3d964ede73fb6298` | `5c74cfbd8124a78c616526ae1d461895b1a16b82` | yes |
| C3b | ARCHITECT_DEFINED | `protocol.rs` | `614e53fd75518991c5e6518bc0fe61cf546cf092b955e3eda0e71cf25d5f6dd4` | `e6aff1884a4b9b7f880eee8784c371648fb2c073` | yes |
| C3c | ARCHITECT_DEFINED | `protocol.rs` | `1e60d98d6d42c0aab581a5bde34162f9767949a495fad801b73d9c740d9e2e85` | `5cc9c09ac96062c5d348e86ace8cb0ac2225e83d` | yes |
| C4 | CARRIED_FORWARD | `protocol.rs` | `24ac7add3716a030dd710539e6082e197fdc5bd624c26a595b378b3971e0838d` | `f3793c2ff61f8adbf43fe17d6b5237b49bc507b0` | yes |
| C5 | ARCHITECT_DEFINED | `protocol.rs` | `a4776885b669bd912fd8710d5a3283f4c1863ab9213e8474d26707b5a6e5f3d8` | `e1a49f5ba952bc81cdac9f8f8ace9eeb2aa93495` | yes |
| C6 | ARCHITECT_DEFINED | `protocol.rs` | `5c647500156a5e9c9b3faa27ce59ea2c09f0c45d1a50daa7a7d1d0a33c5a7e5e` | `6dde859cb243b9f4304cda61b0917b1fc555dde3` | yes |
| C7 | ARCHITECT_DEFINED | `protocol.rs` | `b5d8bc6c9254e0ca2853fcf2f9f220f24782ac6cfa77efe7032365d76f0efbc4` | `0d4ab9929c0e15d910735a52b72a5f3855bef58a` | yes |
| C8 | ARCHITECT_DEFINED | `protocol.rs` | `a7cdd1925bf34283880384b78dd1c918fc61e64afef10094ab6d947bdeb97927` | `fd6abbd8a80dc8a01620981c76427ca76a9e1f7d` | yes |
| C9 | CARRIED_FORWARD | `protocol.rs` | `cda8e8d473a1a4650e98c875f1b9d81b184abfb02c2e55e30a97fd6f4ec2b76b` | `7ba4a8247ba483dea8799ae3b9a107e1c2b4ef5c` | yes |
| C10 | CARRIED_FORWARD | `protocol.rs` | `5e73745b2f8a6d02585519bac4990513e3ae6997dec9ad4e478fb92a2464a16c` | `329a830718b81a065e3a03f4958e19e8fa3c8d9c` | yes |
| C11 | ARCHITECT_DEFINED | `protocol.rs` | `913997fd20c0da21a39cb18b4d3fb6fd44cc0f1a22d42309fa25f2707111c768` | `6e039d9dad6415242884ceef97a54d3548658038` | yes |
| C12 | CARRIED_FORWARD | `protocol.rs` | `d484b4d7f5699eb22bffed0a05ac685171c7960b30ab3973ad039e3a401a7803` | `d1e46ea63bb35c8e9b65d6fe1a2beb8b68a3e27b` | yes |
| C15 | ARCHITECT_DEFINED | `protocol.rs` | `a9a5037d36b1a2a6fc5fddb5accea1198ea08d3823a3044bf45f1c4b6cea5810` | `abdd7518e6d1202a3a7c756af9716dc66183fd21` | yes |
| C16 | ARCHITECT_DEFINED | `protocol.rs` | `e48a988cfc85488141d1e9eb033f897135ad9a4509edf16a3df85b55f8ff5e97` | `33f0d257df3e126c589becd0b1c5760c57085be8` | yes |
| C17 | ARCHITECT_DEFINED | `protocol.rs` | `37b696d014ef6e251119e6b9b489e01d701b483153330d92592ec27e34613ac0` | `4dfed9e1600a82b38f3326d26a47c4c41f513ac3` | yes |
| C18a | ARCHITECT_DEFINED | `protocol.rs` | `7ced1f36a56066c596913442cc3dd93abe23beb1affab728b914fa26f60065fa` | `92034563e479d1d622b7625e6ff4ad075a32a16a` | yes |
| C18b | ARCHITECT_DEFINED | `protocol.rs` | `830f21c88f5ba82798baa9126d67d7f37fdcc4aaae71a52d4ed023c009713ec2` | `c7f2b5ea99bb1676e3844b6c612d3393f97d50da` | yes |
| C20 | ARCHITECT_DEFINED | `protocol.rs` | `5ea1889e67bc552d08c75074f3717d87a4509c56d13448d61bc574fc3d93f62f` | `7bb5aa2abbb52249efe76ab22dd921cb74dd337c` | yes |
| C23 | ARCHITECT_DEFINED | `protocol.rs` | `731ffbedf845790c681b1fe7a2dc20d53174c8ce7378de04e5963865f0bc04aa` | `cd50b40ce632d4edcb95fff89389246b0d75c51e` | yes |
| C24 | CARRIED_FORWARD | `protocol.rs` | `39439c3c17a3b90d31ec3bec1f3b80e753ab3d042cc59ea9848c748872580e44` | `3e1bfbe746851b6c63811343cde9dd3b83ad0fa9` | yes |
| C25 | CARRIED_FORWARD | `protocol.rs` | `29a2c1e571b4f7fd278d106fe3d40a8cf912119dbe44fbbbbd92855431ecd732` | `0456a0c1c1fe40f71593edc6ba5938a3b0c29900` | yes |
| C26 | ARCHITECT_DEFINED | `protocol.rs` | `1e26422d79a19553d925585aeb580f8a0c684bcc78d8a922ab5ab9a3756e8b4d` | `88f1783305f6418129671e77769fd4caf53e2a88` | yes |
| C14 | CARRIED_FORWARD | `protocol.rs` | `2c7f17dafd7347cc2ceb92228f48584a6c0a08292024943551e952879fa2c1e4` | `72556e3f03a53d7a14cb89515a286f42718b6ac8` | yes |
| C19 | CARRIED_FORWARD | `protocol.rs` | `066cc479a89ea2779a2b4ff29ff15b966d877f78b252fbe07493c064766da48f` | `899878323428eb34021f8dcf3cefc5ab041f98a2` | yes |
| C21 | CARRIED_FORWARD | `protocol.rs` | `65029c7b396057e3bfd9c38893e36f77e110311392157a68439065a6be42f7d8` | `80662d4c7f8b0c25a0d8a038a9e8b28a0e469826` | yes |
| C22 | CARRIED_FORWARD | `protocol.rs` | `e017ced648c6d4329ab8c744441e8fc03e8207b47e9cb3387202cc29ce7e95c6` | `694b744de3b70a3a092c633b934262bd510d41aa` | yes |
| C27 | CARRIED_FORWARD | `protocol.rs` | `bb88f0446b1e56e83a13c1d969e5572a310b0b62ec1f3b2424572f433efefd3f` | `bbcaaaf1dcf92d194eb6d9804daaa9901a68cadf` | yes |
| C28 | CARRIED_FORWARD | `protocol.rs` | `519378de0e6b6a05019e6a2dc718441b2a03970ae0341d89d858f1168ee23868` | `446d0365e1472efc40321d259a6f1171d3fe716c` | yes |
| A1 | ARCHITECT_DEFINED | `model_tests.rs` | `e3a21c74babd86e1c40c58457319dc6901c7fac7a6e56d7c8f149e3389f72507` | `26c09e7ff93633095de8d7a15ee81f5f46728336` | yes |
| A2 | ARCHITECT_DEFINED | `model_tests.rs` | `5f1f30486da2457e2a50f0f72f1c96bfecf67e12692d47aa20d0ae3a1d677cf2` | `c0ac428f58497d8cf42458d51f86db0f2e4b84ba` | yes |
| A3 | ARCHITECT_DEFINED | `model_tests.rs` | `ad0c64489e962756316b1a29b5d289a721517d1115525ee011e52db69cf2cc00` | `72bab79d71687a1c056afe18ab7a938edc20f4a9` | yes |
| A5 | ARCHITECT_DEFINED | `model_tests.rs` | `913eba6d31d7cbe6d65f4543b9fb2d0def6ca7e7586b26209b3ee5063a2f4d32` | `cb08f5e8c62b3ce267f0688cc880261a28cc29a2` | yes |
| A6 | ARCHITECT_DEFINED | `model_tests.rs` | `0700052eb3e9fb91f36ee6d535b02b923ff007f1061490ac7dd16dd98d0d0ee8` | `51e54aed7cc33ebc8348a74fe6c881c057336ee7` | no: KEY_FIELDS hunk |

## A4 and C22

- **A4**: SATISFIED — ARCHITECT ACCEPTED — the separate accepted comparison (Stage-1 `receipts/50-A4`), not a mutation, not counted among the 35, not rerun; its entry is R1's.
- **C22**: restored receipt **PENDING_EXACT_COMMAND_HEALTHY_BUILD** (mission R1 section 7; R2 section 9.H). The Stage-1 healthy build omitted `-j 2` and is not C22's type (e4) receipt. A future mission must build Candidate C by exactly `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2`, with command, toolchain, environment, target policy, features/cfg/RUSTFLAGS and supervision identical to C22's mutant build. Neither build ran here (`C22-RESTORED-BUILD-RULING.md`).

## Limitations

- Static only. `SYNTAX_CHECKED_NOT_TYPECHECKED` is a parse-and-format check by rustfmt, not a type check. Nothing was compiled; R1's limitations on type errors and build-environment warnings stand unchanged.
- The KEY_FIELDS line is now 128 characters. rustfmt (default `error_on_line_overflow = false`, `format_strings = false`) leaves string literals as written and `--check` passes; the repository sets no rustfmt configuration and C already has lines in this constant longer than 100 characters.
- That a printed receipt shows the new table description is a static reading of the report source (the three `state key fields: {KEY_FIELDS}` sites); only execution prints a receipt.
- No control is shown to be killed; whether each mutant fails its named property within its ceiling, on the original explorer, is decided only by execution.
- The R2 mission text (sections 1 and 4, `tools/r2_section4.txt`) and R1's section 6 are recorded as received; the mission texts are not repository artifacts, so the executor cannot authenticate them beyond transcription.
- The two checkers are independent implementations written by the same executor; they are not an independent review. Both extend R1's checkers, whose R1 checks are kept.
- The 34 unchanged entries are R1's byte for byte, so their text still names R1's mission where R1 wrote it (the Architect-defined origin notes, and the restored-receipt status of C12, C14, C19, C21, C27 and C28, repeated in each one's restoration record, ending "no execution is authorised in P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01"). They were not rewritten: mission R2 section 6 requires equality, and no execution is authorised in R2 either.
- Still to be produced before execution (unchanged from R1): the baseline exact receipts (X-1 alone for C12; T-BIND, T-CELL, W6, T-PREC and T-RES alone) and C22's exact-command healthy build.

## Next blocker

Independent Sole Architect review of the R2 candidate catalog (the A6 correction and the 34 unchanged controls). Until the Architect accepts a catalog, no catalog is fixed and no build, baseline receipt or mutant run is authorised.

## Files

| Path | What |
|---|---|
| `INDEX.md` | this file |
| `ARCHITECT-CONTROL-DEFINITIONS-R2.md` | R1's 21 definitions carried forward, with the A6 correction recorded |
| `C22-RESTORED-BUILD-RULING.md` | the C22 exact-command ruling as recorded in the catalog |
| `catalog/` | `catalog.json`, `CATALOG.md`, `PATCH-IDENTITIES.md`, `README.md`, `patches/` (35) |
| `comparison/` | `R1-TO-R2-PATCH-COMPARISON.md` (field by field, all 35), `A6-R1-TO-R2-PATCH.diff`, `A6-R1-TO-R2-MUTANT.diff` |
| `receipts/` | authentication, checker A, checker B, syntax check, negative controls, freeze record, final gate |
| `development/` | `DEVELOPMENT-LOG.txt`, every failed development run as found, catalog revision diff |
| `tools/` | every tool, the A6 correction as data (`a6_correction.py`) and R2 sections 1 and 4 as received (`r2_section4.txt`) |
| `SHA256SUMS` | every file of this directory except itself |

## How to verify

From this directory: `sha256sum -c SHA256SUMS`. The tools re-run read-only against the repository's git objects given a root directory: `authenticate.py ROOT` writes the authenticated inputs (Candidate C's sources, the R1 catalog, patches and section 6, the sources the checkers read) to `ROOT/inputs/`; `derive_catalog.py ROOT` reproduces `catalog/` and `comparison/` byte for byte; `checker_a.py ROOT`, `checker_b.py ROOT`, `syntax_check.py ROOT` and `negative_controls.py ROOT` reproduce `receipts/20`, `21`, `30` and `40`. Only `syntax_check.py` runs an external program other than git and patch: `rustfmt --check`, which never writes.
