C22 C14 C19 C21 C27 — MUTATION CONTROL KILL ACCEPTED; C28 BUILD ARCHITECT-ADMISSIBLE (NARROW); C28 KILL PENDING

# Architect ruling record — Tranche 1 partial acceptance

Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission
P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02, from sections 1, 2 and 6 of that mission as issued by the Sole
Nexus OS Architect (ChatGPT GPT-6) via the Owner.

**This file is evidence of the Architect's ruling. A string is not authority.** The authority is the Sole
Architect's ruling itself. Claude Code does not independently accept any control and makes no acceptance claim of
its own. The executor's original classifications and raw receipts remain unchanged as executor history.

## The rulings

> C14 — MUTATION CONTROL KILL ACCEPTED
>
> C19 — MUTATION CONTROL KILL ACCEPTED
>
> C21 — MUTATION CONTROL KILL ACCEPTED
>
> C27 — MUTATION CONTROL KILL ACCEPTED

C22 remains accepted from the earlier Architect ruling (recorded in `ARCHITECT-RULING-C22-ACCEPTANCE.md` of the
resume packet `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-resume-01/` at `3695a817a6190781dc66769a72a487b442d3e6da`).

Current accepted mutation-control kills: C22, C14, C19, C21, C27. Campaign count stated by the ruling: **5 of 35**
kill-required controls accepted. A4 remains separately SATISFIED — ARCHITECT ACCEPTED.

Directions that bind this mission: do not rebuild or rerun C14, C19, C21 or C27; do not recreate their mutant
worktrees; do not regenerate their evidence. Their historical executor classifications and raw receipts remain
unchanged.

## C28 build admissibility (mission section 2)

The existing C28 mutant build is **ARCHITECT-ADMISSIBLE** under a narrow, C28-specific ruling recorded in
`ARCHITECT-RULING-C28-BUILD-ADMISSIBILITY.md`. It creates no general warning exception. The historical executor
output `BUILD: NOT OK` (`receipts/36-C28/03-build/CLASSIFICATION.txt` of the resume packet) remains unchanged as
executor history. **The C28 mutation-control kill remains pending**: this mission may only produce candidate
evidence for it.

## Evidence of the accepted controls (resume packet, not rerun)

Resume evidence commit `3695a817a6190781dc66769a72a487b442d3e6da` (tree `a5fce69d5f60b0b86f3982aa655d5a33f4ef9ea4`, parent `19eda8cc7f559054933fed4a93bdb0a18cc725d4`), review ref
`refs/heads/review/p3-c11-d2a1-mutation-tranche1-resume-01`, packet
`docs/evidence/p3-candidate11/d2a1-mutation-tranche1-resume-01/`. C22's evidence is the EXEC-01 packet `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/` at `19eda8cc7f559054933fed4a93bdb0a18cc725d4` (`receipts/31-C22/`), carried by its ruling.

### C14 — `receipts/32-C14/` (named test T-BIND)

- frozen patch sha256 `2c7f17dafd7347cc2ceb92228f48584a6c0a08292024943551e952879fa2c1e4`; mutant protocol.rs blob `72556e3f03a53d7a14cb89515a286f42718b6ac8`, sha256 `203d788cdfad48db63577de831cd4391d524503e99a0cc149b68a44774740d17`
- pinned mutant executable `/home/nexus/NEXUS/nexus-os-p3-c11-mt1x-C14/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `bfb7b864ef21ec8db377907c8fbbd4910ff15b4e58f59b9a26e332c215bbd77d` size 8,988,256
- panic at `crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:1119:5`; outer class: FAIL — admissible type-(b) named-test FAIL of T-BIND at its own check crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:1119

| Receipt | Blob | sha256 | First line (executor history, unchanged) |
|---|---|---|---|
| `RESULT.txt` | `40c73e6335b0e221c34e0d079b049ac815c9862b` | `225b706e06d472dace8557ac165f20f58ea7eff6ee0f6646617ff10dd745cd1f` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| `02-apply/APPLY.txt` | `8e5cf5ea83cea559e6bc58cb9b76dd6633f749d4` | `22e8394a09e9f8f8e509e4e4237694f9f531e5a56bc434145871b94ba33865f2` | `APPLY: EXACTLY_C_PLUS_PATCH` |
| `03-build/CLASSIFICATION.txt` | `f1ae8ff650df8c5b6d27ae34de4d909c07735af1` | `46094cef1b62a8ef013bfafa968d5c68646f91203f475a6feda2ecf2ef0ba2c7` | `BUILD: PINNED` |
| `03-build/PIN.json` | `00c5dfd33deca68e9f710e5cbb0a954a63f50b47` | `a8e3e16bffc4e96802fa85fbdd0f8226f94971e98866d60e4a5960fc4e8c62cf` | — |
| `04-run/CLASSIFICATION.txt` | `0fd445a2dd63989be7fdfac34d9bd757208e47f5` | `5fe02f11588859e4a00a12c9fec0d4dc2ad66f9335d85a2cacd8b8a01eff572a` | `CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| `04-run/raw-output.txt` | `2d8e33fa911d23c3ff3dd5e8b0e8c3e706ac36c5` | `b795ef1d0808f7a17c8db8fcc118c2ab5fe598d7a122d3b4d0aa549ad673970b` | — |
| `05-restore/RESTORE.txt` | `4102356b17245d16ec23d511ad0fe09e9245f48a` | `f81eeb1614f2fe6126194cdaa8bafeadd4df0fe3c154bb8e365f4f74b798b550` | `RESTORE: EXACTLY_C` |
| `06-link/LINK.txt` | `0740352727b9db2004399e0c7a1176f25d275186` | `2272e4816315613e5a6023df0647008fe3c91b81bf8616b51d9858d9ed1eb1a2` | `LINK: VERIFIED` |

### C19 — `receipts/33-C19/` (named test T-CELL)

- frozen patch sha256 `066cc479a89ea2779a2b4ff29ff15b966d877f78b252fbe07493c064766da48f`; mutant protocol.rs blob `899878323428eb34021f8dcf3cefc5ab041f98a2`, sha256 `a7bb602edbcd7894923a19c8e1e940a7e8ab59c31e9c8c8011eb2dc0a0750113`
- pinned mutant executable `/home/nexus/NEXUS/nexus-os-p3-c11-mt1x-C19/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `f564e177e8edcdcef6c5304ced8371d3bee88177bd72645977e6b61c382e89c5` size 8,984,720
- panic at `crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:1654:5`; outer class: FAIL — admissible type-(b) named-test FAIL of T-CELL at its own check crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:1654

| Receipt | Blob | sha256 | First line (executor history, unchanged) |
|---|---|---|---|
| `RESULT.txt` | `9c2999e128f86ab92cf1a1b3ec5427682c65c089` | `6ca7d192c0647ab6f8af2cf70dca350470a78e2aade94d0eeb12e43327f868e8` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| `02-apply/APPLY.txt` | `36090ad3bb17714b103b3b5273f069299b7a9300` | `fb4918756a4e2102c7e5e5e1ba5634104d5df6cf137101e84bad45738c49bf13` | `APPLY: EXACTLY_C_PLUS_PATCH` |
| `03-build/CLASSIFICATION.txt` | `ffc04c73dda9c24cdf5886468a0d5f5409015572` | `1e7c7cc12e18a739a89ebb2a71cbcfabee6dc93e8dd5461da7a9da68cad74c01` | `BUILD: PINNED` |
| `03-build/PIN.json` | `18ce39a9a2fc6a7ed5875ce27a8f7dae7b610e06` | `b7ad3c7a4e9fc153b15c7626f56602b3214671176fe5f95d7f4c9d8d5d9a64bd` | — |
| `04-run/CLASSIFICATION.txt` | `38a4c79ecd4f937796dfa1d9c3021a6538a6905e` | `8c07d30af555fcafab0450b0f258c2728a2739d08f45caee902145bb67f425b5` | `CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| `04-run/raw-output.txt` | `24cd2d314855471917192cc26523011d4154c27e` | `bca85833e56e4abd37453b1aa851ba1a00972a8117d66e392a14398c1035a306` | — |
| `05-restore/RESTORE.txt` | `a9d2559fb44f17bb339c11456fd265334023d584` | `94abddd3c4e83cdde72a30d5281719515a2fffff57a7433ba7008eaee49c72b6` | `RESTORE: EXACTLY_C` |
| `06-link/LINK.txt` | `ac8d950082556bb4546d8483d9779017b3a6bddc` | `4470574e56dd713397d51551390568964d84bcb6e030d636750bd0494a543067` | `LINK: VERIFIED` |

### C21 — `receipts/34-C21/` (named test W6)

- frozen patch sha256 `65029c7b396057e3bfd9c38893e36f77e110311392157a68439065a6be42f7d8`; mutant protocol.rs blob `80662d4c7f8b0c25a0d8a038a9e8b28a0e469826`, sha256 `db9b9d6e98c2d5eff8d38e9b62b44c4badd1248c82ba5d422f47612700bccd3b`
- pinned mutant executable `/home/nexus/NEXUS/nexus-os-p3-c11-mt1x-C21/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `b4c2d0a60231bffdcb8b466446e171aa193dea0b34a5a249255cd8d24cf1eeb8` size 8,957,304
- panic at `crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:2600:5`; outer class: FAIL — admissible type-(b) named-test FAIL of W6 at its own check crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:2600

| Receipt | Blob | sha256 | First line (executor history, unchanged) |
|---|---|---|---|
| `RESULT.txt` | `0daa04f2044d48575dd0d0363158a42e4dbbd6b2` | `6eda983f969ccfbe9618b0fd2ab133fab1b1719d98ebc47db5a25bb4a065523f` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| `02-apply/APPLY.txt` | `113140e487133d55988c123ae61eaab460976fc9` | `b277861d2b65134616219eb772278539210d85f73eaf64b8096e4101c6c28839` | `APPLY: EXACTLY_C_PLUS_PATCH` |
| `03-build/CLASSIFICATION.txt` | `e4262238fb5be05498770baaaad62d2feaecf18b` | `5ed675c2a48092990beb1dbd2442e3446ce77e2c45cbf5dbe9cc886eac790834` | `BUILD: PINNED` |
| `03-build/PIN.json` | `0de65de4460d546ef127024a4b744db131c0615d` | `c864abc8e3a7e95c667fe1803e8b5541db2885e72b09adb09f335bd7575ecb73` | — |
| `04-run/CLASSIFICATION.txt` | `a424c2f2c945c4cfd404d9cbdb9199c7d85e1279` | `7eef2d13214dd4ff4e7812d5b2cb8f03c48c5d0b4f7cf57a0bc51e3570590dcb` | `CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| `04-run/raw-output.txt` | `68369651cfe0c9dbd6d93e19f7f82061f15a1ef5` | `bc1322e08ac476e22e9384932ee34f61d52f03e141e4f58544d469856a6fbf43` | — |
| `05-restore/RESTORE.txt` | `0c83f1e32404d641faa9a693b77e48994b4c04f0` | `ff8498a3feb8754bae69bd8104e4748f0220102cc84a9a33a7f533246123a8fc` | `RESTORE: EXACTLY_C` |
| `06-link/LINK.txt` | `9dff562bd9c29e93f29b49d71a5776a83a4389a6` | `93fd8c35a3c934291d12611c7ea1d3727419105a7990f3369303ac4684bc3306` | `LINK: VERIFIED` |

### C27 — `receipts/35-C27/` (named test T-PREC)

- frozen patch sha256 `bb88f0446b1e56e83a13c1d969e5572a310b0b62ec1f3b2424572f433efefd3f`; mutant protocol.rs blob `bbcaaaf1dcf92d194eb6d9804daaa9901a68cadf`, sha256 `bc3fe6359decaf6210e43c4285432cd153f6550c30644ef356e35258122df5d7`
- pinned mutant executable `/home/nexus/NEXUS/nexus-os-p3-c11-mt1x-C27/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `8711617881e34ca65305f29e156773484797bb002b44d4ab0601b70146ec181f` size 8,986,736
- panic at `crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:799:9`; outer class: FAIL — admissible type-(b) named-test FAIL of T-PREC at its own check crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:799

| Receipt | Blob | sha256 | First line (executor history, unchanged) |
|---|---|---|---|
| `RESULT.txt` | `00145cd131aa2aeffd70e916a272c02aac55e916` | `4b8b64b5a6288733cf70c5d11fa1aae338f3fb2159450354eecfe0df000f6a3c` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| `02-apply/APPLY.txt` | `7122e572a753bcad97c4d98f0e993d43af4fac3f` | `549f0dac47d506b2982a65b5269ed0e4b9332fd77a348f15cf13d8b0d0eb3288` | `APPLY: EXACTLY_C_PLUS_PATCH` |
| `03-build/CLASSIFICATION.txt` | `27876151150a9c872dc171e034c5b994563cd4a1` | `4f9ff1fe22e78abaaf9ee1350914b7f4986f2e9bd5d17f2b82a4fc2e7f52d87f` | `BUILD: PINNED` |
| `03-build/PIN.json` | `df7b2603c3d13c184556a2665319e4b8377eb71b` | `263f530dfdbd24be2ad07bc1754bbb20a93b2437482aafc82ad97d8379bf7994` | — |
| `04-run/CLASSIFICATION.txt` | `7782fbf10975965ad4362ce8401b711e4eb311ee` | `85b754f5fc84e0f768a926352764ef7d883473b88e5e97105bbad076dfd26f3a` | `CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| `04-run/raw-output.txt` | `b920dd46432434514818b9b2142624f67d9d784f` | `79b4d2e25fc32ff3ff4081a4fdf041d9cedaabf3b91d8cee737c27f0ffad6622` | — |
| `05-restore/RESTORE.txt` | `eb9db4424c2cd2c6393c38e9b9fc18e82f318052` | `e9a5639938ec4d2d86b3d74b63627708c6b322f7865101fdf3cb9c15f347104b` | `RESTORE: EXACTLY_C` |
| `06-link/LINK.txt` | `19a192eac0a65ac9b53de9042049f915b7b0ba6c` | `fad81ccc445d4a0a277749ece7e53debd9ad476b36355f637b8c8219a41b7150` | `LINK: VERIFIED` |

### C22 (earlier ruling)

| Receipt | Blob | sha256 |
|---|---|---|
| resume packet `ARCHITECT-RULING-C22-ACCEPTANCE.md` | `b852dc55adfb3b2dbff05a48f811143e5b12fc37` | `8bfb7fdac4141bc00f0ae4a1f994c1f38dd157b9e452abc40f52a3ebdb290af3` |
| EXEC-01 packet `receipts/31-C22/RESULT.txt` | `b9717ad766e11dacf4d342db0022ab56886ae486` | `106c45675b7f1d0e164d10125b7258ffe71244f908573d63321371771dda0aec` |
| EXEC-01 packet `receipts/31-C22/03-build/CLASSIFICATION.txt` | `1c24be48ff59b0966795cb35f83674815ce449b6` | `5799f690ce64a4d7b0dc33f06147eeb03331e6db04b4976df06b2b5dd8bb893f` |
| EXEC-01 packet `receipts/31-C22/03-build/build.stderr` | `2064a39176fbb386ad5e4624aa873bd059c4e8e5` | `5cbb36bd2d8af4c66bf12fa30eb68133689a181e87d0e15e6d65f471afb633b2` |

## The Architect decision

**C22, C14, C19, C21, C27 — MUTATION CONTROL KILL ACCEPTED** (5 of 35). **C28 — build ARCHITECT-ADMISSIBLE under
the narrow C28 ruling; C28 control kill PENDING.** Decisions of the Sole Architect. Claude Code did not
independently accept any control.
