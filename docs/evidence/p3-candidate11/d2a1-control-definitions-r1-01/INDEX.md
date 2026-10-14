# D2A-1 control definitions R1 — index

Mission P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01. Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. For the Sole Architect's review.

**Status: READY_FOR_CONTROL_DEFINITION_ARCHITECT_REVIEW.**

Static definition work only. No Cargo, no rustc, no build, no test, no explorer; no patch applied to any worktree; no mutant built or run. Candidate C is unchanged. The catalog is a **candidate**: every control's definition is CANDIDATE_READY, its Architect review pending, its execution NOT_ATTEMPTED. No kill is claimed. D2A-1 remains unaccepted; Candidate 11 is not integrated; Phase Three remains active.

The 21 definitions are the Sole Architect's (mission section 6), encoded by the executor: **ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING** (`ARCHITECT-CONTROL-DEFINITIONS-R1.md`). The other 14 are the blocked catalog's patches, carried forward byte for byte and re-authenticated (mission section 5).

## Identities

| Item | Value |
|---|---|
| Candidate C | `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`, single parent `71264aa81a40b86189783facc8437e0427887c16` |
| Blobs at C | `protocol.rs` `a2df2841b99cad5d452008a6d4150e9369d07e66`; `model_tests.rs` `28085ce232827301104b5d757af042ec933976f8`; `unit_tests.rs` `55dc66409eaccdaa93e8f0a56f2ccb14be81e114` |
| Contract | sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c` |
| AD-1-R4 | sha256 `00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693` |
| AD-1-R5 | sha256 `4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473`; Owner-adoption record `docs/evidence/p3-candidate11/ad1r5-adoption/OWNER-ADOPTION-2026-10-14.md`, blob `1492e185cb475bd32d95079a39b49078c0e23ddc`, sha256 `9990d2565a5ec96e0db03be832e96f64f774e662f6131cb69240c9a435757aa4` |
| Historical partial controls file | sha256 `ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f` |
| Blocked catalog (immutable review history) | `d2a1-controls-v1`, sha256 `a0cdef7976bbb1715a7da25ad2900c978a32c6b3d9029b46f16872eda66b4747`, evidence commit `59df21036f7c0183fa17d76b6e0886f8f5784685`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche1-01` |
| **Candidate catalog** | `docs/evidence/p3-candidate11/d2a1-control-definitions-r1-01/catalog/catalog.json`, version `d2a1-controls-v1-r1-candidate`, git blob `56d4424397be7740cefd8d89b3530aab4b25d3ed`, catalog sha256 `a73d5721ec59a5d14abf5c31db72d1da1d34962d1db7294347797dd677ae6479` |
| **Definitions record** | `docs/evidence/p3-candidate11/d2a1-control-definitions-r1-01/ARCHITECT-CONTROL-DEFINITIONS-R1.md`, git blob `b947621bb0d56e1e14780853e886c7e832d7f6a3`, definitions record sha256 `beb9f5e3129bbd559dfa375251bddeac17f78d3f1b79082a42767944d7d48bf0` |
| Evidence commit parent | `59df21036f7c0183fa17d76b6e0886f8f5784685` |

## Results

| Step | Result | Record |
|---|---|---|
| Input authentication (mission 4, 5) | ALL CHECKS OK (120 checks) | `receipts/00-authentication.txt` |
| Candidate catalog | 35 kill-required, all CANDIDATE_READY (14 carried forward, 21 Architect-defined), 0 blocked; A4 separate | `catalog/` |
| Checker A (bytes/text; GNU patch, git apply) | PASS, 800 checks | `receipts/20-checker-a.txt` |
| Checker B (independent: strict diff engine, tokenizer) | PASS, 630 checks | `receipts/21-checker-b.txt` |
| Checker agreement | identical per-id verdict maps (38 ids, all PASS) | `receipts/20`, `21`, `40` |
| Syntax (rustfmt --check, disposable copies) | all 21 SYNTAX_CHECKED_NOT_TYPECHECKED; C's files pass; an unparsable control fails | `receipts/30-syntax-check.txt` |
| Negative controls | 29 seeded defects, each detected by both checkers | `receipts/40-negative-controls.txt` |
| Freeze record (packet bytes; no catalog status) | every packet file hashed before the final gate | `receipts/50-FREEZE-RECORD.json` |
| Final gate | see its receipt | `receipts/60-final-gate.txt` |

## The 35 patch identities

| Id | Source | File | Patch sha256 | Mutant blob |
|---|---|---|---|---|
| C1 | CARRIED_FORWARD | `protocol.rs` | `5ecd716a8f123d61034b0d274238898e13a116a9cc464c7645a9736934970252` | `7d95dd425fcdf17b12dda2fdce461f97ba7b9a66` |
| C2 | CARRIED_FORWARD | `protocol.rs` | `f02c3e721aa2c96bb7d15fe081acf3dc6e4c1958de096f4215bc55a7cafc218d` | `1c10adbc93c2fe0984d0ac677727a3bdc82b8e5d` |
| C3a | ARCHITECT_DEFINED | `protocol.rs` | `2442f63d9383dc0d36dd934e2a6b8fb0b3d6936f215e00ba3d964ede73fb6298` | `5c74cfbd8124a78c616526ae1d461895b1a16b82` |
| C3b | ARCHITECT_DEFINED | `protocol.rs` | `614e53fd75518991c5e6518bc0fe61cf546cf092b955e3eda0e71cf25d5f6dd4` | `e6aff1884a4b9b7f880eee8784c371648fb2c073` |
| C3c | ARCHITECT_DEFINED | `protocol.rs` | `1e60d98d6d42c0aab581a5bde34162f9767949a495fad801b73d9c740d9e2e85` | `5cc9c09ac96062c5d348e86ace8cb0ac2225e83d` |
| C4 | CARRIED_FORWARD | `protocol.rs` | `24ac7add3716a030dd710539e6082e197fdc5bd624c26a595b378b3971e0838d` | `f3793c2ff61f8adbf43fe17d6b5237b49bc507b0` |
| C5 | ARCHITECT_DEFINED | `protocol.rs` | `a4776885b669bd912fd8710d5a3283f4c1863ab9213e8474d26707b5a6e5f3d8` | `e1a49f5ba952bc81cdac9f8f8ace9eeb2aa93495` |
| C6 | ARCHITECT_DEFINED | `protocol.rs` | `5c647500156a5e9c9b3faa27ce59ea2c09f0c45d1a50daa7a7d1d0a33c5a7e5e` | `6dde859cb243b9f4304cda61b0917b1fc555dde3` |
| C7 | ARCHITECT_DEFINED | `protocol.rs` | `b5d8bc6c9254e0ca2853fcf2f9f220f24782ac6cfa77efe7032365d76f0efbc4` | `0d4ab9929c0e15d910735a52b72a5f3855bef58a` |
| C8 | ARCHITECT_DEFINED | `protocol.rs` | `a7cdd1925bf34283880384b78dd1c918fc61e64afef10094ab6d947bdeb97927` | `fd6abbd8a80dc8a01620981c76427ca76a9e1f7d` |
| C9 | CARRIED_FORWARD | `protocol.rs` | `cda8e8d473a1a4650e98c875f1b9d81b184abfb02c2e55e30a97fd6f4ec2b76b` | `7ba4a8247ba483dea8799ae3b9a107e1c2b4ef5c` |
| C10 | CARRIED_FORWARD | `protocol.rs` | `5e73745b2f8a6d02585519bac4990513e3ae6997dec9ad4e478fb92a2464a16c` | `329a830718b81a065e3a03f4958e19e8fa3c8d9c` |
| C11 | ARCHITECT_DEFINED | `protocol.rs` | `913997fd20c0da21a39cb18b4d3fb6fd44cc0f1a22d42309fa25f2707111c768` | `6e039d9dad6415242884ceef97a54d3548658038` |
| C12 | CARRIED_FORWARD | `protocol.rs` | `d484b4d7f5699eb22bffed0a05ac685171c7960b30ab3973ad039e3a401a7803` | `d1e46ea63bb35c8e9b65d6fe1a2beb8b68a3e27b` |
| C15 | ARCHITECT_DEFINED | `protocol.rs` | `a9a5037d36b1a2a6fc5fddb5accea1198ea08d3823a3044bf45f1c4b6cea5810` | `abdd7518e6d1202a3a7c756af9716dc66183fd21` |
| C16 | ARCHITECT_DEFINED | `protocol.rs` | `e48a988cfc85488141d1e9eb033f897135ad9a4509edf16a3df85b55f8ff5e97` | `33f0d257df3e126c589becd0b1c5760c57085be8` |
| C17 | ARCHITECT_DEFINED | `protocol.rs` | `37b696d014ef6e251119e6b9b489e01d701b483153330d92592ec27e34613ac0` | `4dfed9e1600a82b38f3326d26a47c4c41f513ac3` |
| C18a | ARCHITECT_DEFINED | `protocol.rs` | `7ced1f36a56066c596913442cc3dd93abe23beb1affab728b914fa26f60065fa` | `92034563e479d1d622b7625e6ff4ad075a32a16a` |
| C18b | ARCHITECT_DEFINED | `protocol.rs` | `830f21c88f5ba82798baa9126d67d7f37fdcc4aaae71a52d4ed023c009713ec2` | `c7f2b5ea99bb1676e3844b6c612d3393f97d50da` |
| C20 | ARCHITECT_DEFINED | `protocol.rs` | `5ea1889e67bc552d08c75074f3717d87a4509c56d13448d61bc574fc3d93f62f` | `7bb5aa2abbb52249efe76ab22dd921cb74dd337c` |
| C23 | ARCHITECT_DEFINED | `protocol.rs` | `731ffbedf845790c681b1fe7a2dc20d53174c8ce7378de04e5963865f0bc04aa` | `cd50b40ce632d4edcb95fff89389246b0d75c51e` |
| C24 | CARRIED_FORWARD | `protocol.rs` | `39439c3c17a3b90d31ec3bec1f3b80e753ab3d042cc59ea9848c748872580e44` | `3e1bfbe746851b6c63811343cde9dd3b83ad0fa9` |
| C25 | CARRIED_FORWARD | `protocol.rs` | `29a2c1e571b4f7fd278d106fe3d40a8cf912119dbe44fbbbbd92855431ecd732` | `0456a0c1c1fe40f71593edc6ba5938a3b0c29900` |
| C26 | ARCHITECT_DEFINED | `protocol.rs` | `1e26422d79a19553d925585aeb580f8a0c684bcc78d8a922ab5ab9a3756e8b4d` | `88f1783305f6418129671e77769fd4caf53e2a88` |
| C14 | CARRIED_FORWARD | `protocol.rs` | `2c7f17dafd7347cc2ceb92228f48584a6c0a08292024943551e952879fa2c1e4` | `72556e3f03a53d7a14cb89515a286f42718b6ac8` |
| C19 | CARRIED_FORWARD | `protocol.rs` | `066cc479a89ea2779a2b4ff29ff15b966d877f78b252fbe07493c064766da48f` | `899878323428eb34021f8dcf3cefc5ab041f98a2` |
| C21 | CARRIED_FORWARD | `protocol.rs` | `65029c7b396057e3bfd9c38893e36f77e110311392157a68439065a6be42f7d8` | `80662d4c7f8b0c25a0d8a038a9e8b28a0e469826` |
| C22 | CARRIED_FORWARD | `protocol.rs` | `e017ced648c6d4329ab8c744441e8fc03e8207b47e9cb3387202cc29ce7e95c6` | `694b744de3b70a3a092c633b934262bd510d41aa` |
| C27 | CARRIED_FORWARD | `protocol.rs` | `bb88f0446b1e56e83a13c1d969e5572a310b0b62ec1f3b2424572f433efefd3f` | `bbcaaaf1dcf92d194eb6d9804daaa9901a68cadf` |
| C28 | CARRIED_FORWARD | `protocol.rs` | `519378de0e6b6a05019e6a2dc718441b2a03970ae0341d89d858f1168ee23868` | `446d0365e1472efc40321d259a6f1171d3fe716c` |
| A1 | ARCHITECT_DEFINED | `model_tests.rs` | `e3a21c74babd86e1c40c58457319dc6901c7fac7a6e56d7c8f149e3389f72507` | `26c09e7ff93633095de8d7a15ee81f5f46728336` |
| A2 | ARCHITECT_DEFINED | `model_tests.rs` | `5f1f30486da2457e2a50f0f72f1c96bfecf67e12692d47aa20d0ae3a1d677cf2` | `c0ac428f58497d8cf42458d51f86db0f2e4b84ba` |
| A3 | ARCHITECT_DEFINED | `model_tests.rs` | `ad0c64489e962756316b1a29b5d289a721517d1115525ee011e52db69cf2cc00` | `72bab79d71687a1c056afe18ab7a938edc20f4a9` |
| A5 | ARCHITECT_DEFINED | `model_tests.rs` | `913eba6d31d7cbe6d65f4543b9fb2d0def6ca7e7586b26209b3ee5063a2f4d32` | `cb08f5e8c62b3ce267f0688cc880261a28cc29a2` |
| A6 | ARCHITECT_DEFINED | `model_tests.rs` | `02e1fd1b60d4b1255079f5533072f5a213a92f8db9214984eb8b527975b76e86` | `85d262dbdc445569951ec43e914de4c192ea675d` |

## A4 and C22

- **A4**: SATISFIED — ARCHITECT ACCEPTED — the separate accepted comparison (Stage-1 `receipts/50-A4`), not a mutation, not counted among the 35, not rerun.
- **C22**: restored receipt **PENDING_EXACT_COMMAND_HEALTHY_BUILD** (mission section 7). The Stage-1 healthy build omitted `-j 2` and is not C22's type (e4) receipt. A future mission must build Candidate C by exactly `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2`, with command, toolchain, environment, target policy, features/cfg/RUSTFLAGS and supervision identical to C22's mutant build. Neither build ran here (`C22-RESTORED-BUILD-RULING.md`).

## Limitations

- Static only. `SYNTAX_CHECKED_NOT_TYPECHECKED` is a parse-and-format check by rustfmt, not a type check. Nothing was compiled: a type error (for example in A1's match guard, or A2's let-else) or a warning that a future build environment might deny (dead_code after C3a, C3b, C3c; unused `nonce` and `io` after C5) can only be found by building. At C, the crate denies `unsafe_code` and, outside test builds, five clippy lints; it does not deny `dead_code` or `unused_variables`, and `.cargo/config.toml` sets no RUSTFLAGS.
- No control is shown to be killed. Whether each mutant fails its named property within its ceiling, on the original explorer, is decided only by execution (each control's uncertainty is listed in the definitions record).
- Section 6 is recorded as received in the mission text; the mission text is not a repository artifact, so the executor cannot authenticate it beyond transcription.
- Encoding choices inside section 6 that the Architect should look at: C8 carries the healthy facts-read failure arm unchanged into the merged loop (after an earlier insert it reports Not written, as 6.7 rules for a refusal) and drops the two comments that describe the removed structure; A1 adds an `expect` message and trace notes, A2 a step-one trace note (trace text only); C6, C15, C16 and A5 use the layout rustfmt requires; A6 leaves the `KEY_FIELDS` receipt string unchanged.
- The two checkers are independent implementations written by the same executor; they are not an independent review.
- In the 14 carried-forward entries only non-definition text changed: the origin note, and six restored-receipt status sentences (C12, C14, C19, C21, C27, C28) that named the blocked mission's sections and said the catalog was blocked. Their patches, anchors and mutants are byte-identical.
- Still to be produced before execution: the baseline exact receipts (X-1 alone for C12; T-BIND, T-CELL, W6, T-PREC and T-RES alone) and C22's exact-command healthy build.

## Next blocker

Independent Sole Architect review of the 21 encodings and of the candidate catalog. Until the Architect accepts a catalog, no catalog is fixed and no build, baseline receipt or mutant run is authorised.

## Files

| Path | What |
|---|---|
| `INDEX.md` | this file |
| `ARCHITECT-CONTROL-DEFINITIONS-R1.md` | the 21 definitions: contract row, section-6 text, item, anchors, replacements, identities, property/test/configuration, receipt type, preserved behaviour, uncertainty |
| `C22-RESTORED-BUILD-RULING.md` | the C22 exact-command ruling as recorded in the catalog |
| `catalog/` | `catalog.json`, `CATALOG.md`, `PATCH-IDENTITIES.md`, `README.md`, `patches/` (35) |
| `receipts/` | authentication, checker A, checker B, syntax check, negative controls, freeze record, final gate |
| `development/` | `DEVELOPMENT-LOG.txt`, every failed development run as found, catalog revision diffs, the trial-encoding tool |
| `tools/` | every tool, the encoded definitions (`architect_definitions.py`) and section 6 as received (`section6.txt`) |
| `SHA256SUMS` | every file of this directory except itself |

## How to verify

From this directory: `sha256sum -c SHA256SUMS`. The tools re-run read-only against the repository's git objects given a root directory: `authenticate.py ROOT` writes the authenticated inputs to `ROOT/inputs/`; `derive_catalog.py ROOT` reproduces `catalog/` byte for byte; `checker_a.py ROOT`, `checker_b.py ROOT`, `syntax_check.py ROOT` and `negative_controls.py ROOT` reproduce `receipts/20`, `21`, `30` and `40`. Only `syntax_check.py` runs an external program other than git and patch: `rustfmt --check`, which never writes.
