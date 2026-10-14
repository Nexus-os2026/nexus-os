# Tranche-1 summary — P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01

Derived status: **MUTATION_CONTROL_BLOCKED** (from the receipts; publication and the final report follow).

Combined Tranche 1 (12 controls):

- **C22 — MUTATION CONTROL KILL ACCEPTED** by the Sole Architect (ruling recorded in `ARCHITECT-RULING-C22-ACCEPTANCE.md`; type (d), restored side the exact-command healthy build (e4)). Not executed again here. Its original executor classifications remain as historical outputs of the defective frozen tool.
- new **KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING** results of this mission: **2** of the 11 remaining controls (C14, C12). No control is called killed or accepted here: acceptance belongs to the Sole Architect, receipt by receipt.
- attempted by this mission: C14, C12, C9; not attempted: C19, C21, C27, C28, C11, C17, C18a, C18b

## C22 (Architect-accepted; previous packet, not rerun)

| Receipt | Previous packet path | Blob at `19eda8cc` | sha256 |
|---|---|---|---|
| 31-C22/02-apply/APPLY.json | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/receipts/31-C22/02-apply/APPLY.json` | `2d2bc4da3f762ddb4b148298d2e5a0aa7a005fa5` | `01836ed995440b28251add2611bbb9b8d776371f8b7e4d76cf9f3985f93f5bbd` |
| 31-C22/03-build/build.stderr | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/receipts/31-C22/03-build/build.stderr` | `2064a39176fbb386ad5e4624aa873bd059c4e8e5` | `5cbb36bd2d8af4c66bf12fa30eb68133689a181e87d0e15e6d65f471afb633b2` |
| 31-C22/03-build/PARSED-RECEIPT.json | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/receipts/31-C22/03-build/PARSED-RECEIPT.json` | `8ea4b5ce17b302772885f3ce22e1a9de8a0efff7` | `669e639e9d7e7016f5937c3ac15abb6a8671aa1029a8ee6d1eb5c84843e5ad30` |
| 31-C22/03-build/CLASSIFICATION.txt | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/receipts/31-C22/03-build/CLASSIFICATION.txt` | `1c24be48ff59b0966795cb35f83674815ce449b6` | `5799f690ce64a4d7b0dc33f06147eeb03331e6db04b4976df06b2b5dd8bb893f` |
| 31-C22/05-restore/RESTORE.json | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/receipts/31-C22/05-restore/RESTORE.json` | `39cd3745e5e2032cbae95d6861bdfa8aa2059194` | `7d3648e6f7436f7c37d8a762414df472c6b4ebc6947d1ca3d2dff322d2a78946` |
| 31-C22/RESULT.txt | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/receipts/31-C22/RESULT.txt` | `b9717ad766e11dacf4d342db0022ab56886ae486` | `106c45675b7f1d0e164d10125b7258ffe71244f908573d63321371771dda0aec` |
| 20-healthy-build/CLASSIFICATION.txt | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/receipts/20-healthy-build/CLASSIFICATION.txt` | `604a6911710080ce5f5a01f4379017adb54e6704` | `f3ae4e225cdbfec34c31cd8704d86f5236223d6295e392d692af54d0002335f4` |
| 20-healthy-build/descriptor.json | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/receipts/20-healthy-build/descriptor.json` | `7ef2dfacbeffdd7f8c88e25486ffdf6a1989d01d` | `2ecdb28393c2c475ebe3acb2534b078f3b61b6d5bc99ce4b9c7408094ce4f78d` |

- frozen patch sha256 `e017ced648c6d4329ab8c744441e8fc03e8207b47e9cb3387202cc29ce7e95c6`; mutant protocol.rs blob `694b744de3b70a3a092c633b934262bd510d41aa`, sha256 `0f13a8e5512439aa6385bbe65c20ceb6a77fe6c819216ef82564f7b8880425e5`; E0283 at `crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:1089:5`, `<Ticket as NotClone<_>>::check();`; no mutant executable (by design)

## Baseline receipts linked by this mission (previous packet, not rerun)

| Baseline | Type | For | Previous packet CLASSIFICATION.txt | Blob | sha256 | Classification |
|---|---|---|---|---|---|---|
| X-1 | (e1) | C12 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/receipts/10-baseline-X-1/CLASSIFICATION.txt` | `722d29c2a12cede354b5942eff33f9a7dc12991e` | `23736f0fa04289cb24d9fe0b813ccc85299ebf4e10a8e67df1c9c6167e051f2b` | CLASSIFICATION: PASS |
| T-BIND | (e2) | C14 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/receipts/11-baseline-T-BIND/CLASSIFICATION.txt` | `76b09ff44bb87dcb895a3004d7133343fe072397` | `9c107104b9a3d600408343b1dc54b7c6a927876e2f601df975a5becfbed9846e` | CLASSIFICATION: PASS |
| T-CELL | (e2) | C19 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/receipts/12-baseline-T-CELL/CLASSIFICATION.txt` | `86184ad14f0c8443d6ae3470e22a64f99efbce31` | `846abb401950670a084f355824054a63ee7f70b9a7413742e6404c7bd9f813e9` | CLASSIFICATION: PASS |
| W6 | (e2) | C21 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/receipts/13-baseline-W6/CLASSIFICATION.txt` | `d4bc44fcfd46f42d91c54f379357e20dc451e8d8` | `48a9a0a2050da75c50444c6d8f62d01738b2a361cbf30686365b216621df07ec` | CLASSIFICATION: PASS |
| T-PREC | (e2) | C27 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/receipts/14-baseline-T-PREC/CLASSIFICATION.txt` | `ee5e643ef7ef7c93aef39328f8247abe30daa8bf` | `86b4d65217f3e2789bb5178ef90db8ae7e74aa8079eec412f0bfb17482a008d5` | CLASSIFICATION: PASS |
| T-RES | (e2) | C28 | `docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/receipts/15-baseline-T-RES/CLASSIFICATION.txt` | `ed8f90d09a6bc5850c423d920e174b294001abb4` | `fb124c13db93cbf231a3b4013e7ff52fbbd829800d9a484c491084de0580a8bb` | CLASSIFICATION: PASS |

## The eleven remaining controls (resume order)

### C14 — `receipts/32-C14`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- receipt type: (b) named non-explorer mutant-test FAIL; restored (e2) PASS
- frozen patch sha256: `2c7f17dafd7347cc2ceb92228f48584a6c0a08292024943551e952879fa2c1e4`
- mutant source blob: `72556e3f03a53d7a14cb89515a286f42718b6ac8` (sha256 `203d788cdfad48db63577de831cd4391d524503e99a0cc149b68a44774740d17`; computed in-process, never written to any object store)
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned
- build resources: wall 3.44 s / 900 s; scope peak 197.3 MiB / 16 GiB; swap peak 0 B; oom_kill 0
- mutant executable: /tmp/claude-1000/-home-nexus-NEXUS-nexus-os/8d6fb31a-3bfc-468c-8676-3e5d2773907e/scratchpad/synth-official/ws-healthy/target/release/deps/nexus_governed_control-801c2e64b68ac47d sha256 8cd1e45a72b281388b35bf3e46ebac6ae91fe7ac6914b344e5fdd26aa6cdb6a8 size 966208
- exact selection: `ledger::protocol::unit_tests::t_bind_every_bound_value_belongs_to_one_ledger --exact --nocapture --test-threads=1`
- raw outcome: panic at ['crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs', 1119, 5]: assertion `left == right` failed: a synthetic check |   left: Ok("no") |  right: Ok("ok")
- outer classification: FAIL — admissible type-(b) named-test FAIL of T-BIND at its own check crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:1119
- named property / test: T-BIND (ledger::protocol::unit_tests::t_bind_every_bound_value_belongs_to_one_ledger)
- discovered count: not applicable
- run resources: wall 0.0 s / 180 s; scope peak 3.4 MiB / 16 GiB; swap peak 0 B; oom_kill 0
- restoration: RESTORE: EXACTLY_C
- linked restored-side receipt: LINK: VERIFIED — control: synthetic link

### C12 — `receipts/37-C12`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- receipt type: (a) original-explorer mutant FAIL; restored (e1)
- frozen patch sha256: `d484b4d7f5699eb22bffed0a05ac685171c7960b30ab3973ad039e3a401a7803`
- mutant source blob: `d1e46ea63bb35c8e9b65d6fe1a2beb8b68a3e27b` (sha256 `4d67c182a6e411ee6f7ed6269281d4c84d0bc1ef2699253e39c01cd42aae6a04`; computed in-process, never written to any object store)
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned
- build resources: wall 3.44 s / 900 s; scope peak 197.3 MiB / 16 GiB; swap peak 0 B; oom_kill 0
- mutant executable: /tmp/claude-1000/-home-nexus-NEXUS-nexus-os/8d6fb31a-3bfc-468c-8676-3e5d2773907e/scratchpad/synth-official/ws-healthy/target/release/deps/nexus_governed_control-801c2e64b68ac47d sha256 8cd1e45a72b281388b35bf3e46ebac6ae91fe7ac6914b344e5fdd26aa6cdb6a8 size 966208
- exact selection: `ledger::protocol::model_tests::x1_two_facts_two_writers_one_admission --exact --nocapture --test-threads=1`
- raw outcome: outcome FAIL; FAIL E2: a synthetic violation (tool control)
- outer classification: FAIL — admissible type-(a) original-explorer mutant FAIL of the named property E2
- named property / test: E2
- discovered count: 59,104 of 250,000
- run resources: wall 0.0 s / 180 s; scope peak 3.3 MiB / 16 GiB; swap peak 0 B; oom_kill 0
- restoration: RESTORE: EXACTLY_C
- linked restored-side receipt: LINK: VERIFIED — control: synthetic link

### C9 — `receipts/38-C9`

- result: **CONTROL RESULT: MUTATION_CONTROL_BLOCKED**
- receipt type: (a) original-explorer mutant FAIL; restored (e1)
- frozen patch sha256: `cda8e8d473a1a4650e98c875f1b9d81b184abfb02c2e55e30a97fd6f4ec2b76b`
- mutant source blob: `7ba4a8247ba483dea8799ae3b9a107e1c2b4ef5c` (sha256 `afe5a170535241f2106ede6058b5e7f0ed98e01a875674e9fa70ee0032e7bd4e`; computed in-process, never written to any object store)
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned
- build resources: wall 3.44 s / 900 s; scope peak 197.3 MiB / 16 GiB; swap peak 0 B; oom_kill 0
- mutant executable: /tmp/claude-1000/-home-nexus-NEXUS-nexus-os/8d6fb31a-3bfc-468c-8676-3e5d2773907e/scratchpad/synth-official/ws-healthy/target/release/deps/nexus_governed_control-801c2e64b68ac47d sha256 8cd1e45a72b281388b35bf3e46ebac6ae91fe7ac6914b344e5fdd26aa6cdb6a8 size 966208
- exact selection: `ledger::protocol::model_tests::full_fb2 --exact --ignored --nocapture --test-threads=1`
- raw outcome: outcome FAIL; FAIL E4: a synthetic violation (tool control)
- outer classification: INCOMPLETE — BUDGET EXCEEDED (a FAIL printed beyond the authorised cardinality): kills nothing
- named property / test: E4
- discovered count: 30,000,005 of 30,000,000
- run resources: wall 0.0 s / 2700 s; scope peak 3.5 MiB / 16 GiB; swap peak 0 B; oom_kill 0
- restoration: RESTORE: EXACTLY_C
- linked restored-side receipt: —

## Not attempted

- authorized but not attempted (a stop ends the tranche): C19, C21, C27, C28, C11, C17, C18a, C18b
- not authorized (never executed): C1, C2, C3a, C3b, C3c, C4, C5, C6, C7, C8, C10, C15, C16, C20, C23, C24, C25, C26, A1, A2, A3, A5, A6; C22 not rerun; A4 not rerun; M01–M183 not executed; no baseline receipt rerun

