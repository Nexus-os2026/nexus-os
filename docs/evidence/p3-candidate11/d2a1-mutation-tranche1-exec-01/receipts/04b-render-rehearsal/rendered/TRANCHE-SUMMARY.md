# Tranche summary — P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01

Derived tranche status: **MUTATION_CONTROL_BLOCKED** (from the receipts; publication and the final report follow).

KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING: **3** of 12 authorized controls (C22, C14, C12). No control is called killed or accepted: acceptance belongs to the Sole Architect, receipt by receipt.

## Baseline standalone receipts (mission section 7) and the healthy build (section 8)

| Step | Type | Receipt | Classification | Facts | Resources |
|---|---|---|---|---|---|
| X-1 (for C12) | (e1) | `receipts/10-baseline-X-1` | CLASSIFICATION: PASS — PASS — admissible unchanged-core original-explorer X-1 PASS (type (e1)) | states 59,104 of 250,000; branch accounting NORMALIZED | wall 0.0 s / 180 s; scope peak 3.1 MiB / 16 GiB; swap peak 0 B; oom_kill 0 |
| T-BIND (for C14) | (e2) | `receipts/11-baseline-T-BIND` | CLASSIFICATION: PASS — PASS — admissible restored named-unit-test T-BIND PASS (type (e2)) | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s | wall 0.0 s / 180 s; scope peak 3.5 MiB / 16 GiB; swap peak 0 B; oom_kill 0 |
| T-CELL (for C19) | (e2) | `receipts/12-baseline-T-CELL` | CLASSIFICATION: PASS — PASS — admissible restored named-unit-test T-BIND PASS (type (e2)) | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s | wall 0.0 s / 180 s; scope peak 3.5 MiB / 16 GiB; swap peak 0 B; oom_kill 0 |
| W6 (for C21) | (e2) | `receipts/13-baseline-W6` | CLASSIFICATION: PASS — PASS — admissible restored named-unit-test T-BIND PASS (type (e2)) | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s | wall 0.0 s / 180 s; scope peak 3.5 MiB / 16 GiB; swap peak 0 B; oom_kill 0 |
| T-PREC (for C27) | (e2) | `receipts/14-baseline-T-PREC` | CLASSIFICATION: PASS — PASS — admissible restored named-unit-test T-BIND PASS (type (e2)) | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s | wall 0.0 s / 180 s; scope peak 3.5 MiB / 16 GiB; swap peak 0 B; oom_kill 0 |
| T-RES (for C28) | (e2) | `receipts/15-baseline-T-RES` | CLASSIFICATION: PASS — PASS — admissible restored named-unit-test T-BIND PASS (type (e2)) | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s | wall 0.0 s / 180 s; scope peak 3.5 MiB / 16 GiB; swap peak 0 B; oom_kill 0 |
| healthy build (for C22) | (e4) | `receipts/20-healthy-build` | CLASSIFICATION: PASS — PASS — admissible healthy-build type-(e4) success (exact command, Candidate C) | executable /tmp/claude-1000/-home-nexus-NEXUS-nexus-os/8d6fb31a-3bfc-468c-8676-3e5d2773907e/scratchpad/synth-official/ws-healthy/target/release/deps/nexus_governed_control-801c2e64b68ac47d sha256 8cd1e45a72b281388b35bf3e46ebac6ae91fe7ac6914b344e5fdd26aa6cdb6a8 size 966208 (recorded, never run); 9 target files recorded | wall 3.44 s / 900 s; scope peak 197.3 MiB / 16 GiB; swap peak 0 B; oom_kill 0 |

## Controls (mission section 9 order)

### C22 — `receipts/31-C22`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- receipt type: (d) intentional mutant compile failure; restored (e4) healthy build
- frozen patch sha256: `e017ced648c6d4329ab8c744441e8fc03e8207b47e9cb3387202cc29ce7e95c6`
- mutant source blob: `694b744de3b70a3a092c633b934262bd510d41aa` (sha256 `0f13a8e5512439aa6385bbe65c20ceb6a77fe6c819216ef82564f7b8880425e5`; computed in-process, never written to any object store)
- build: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL (build) — admissible type-(d) intentional mutant compile failure: E0283 at crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:1089
- build resources: wall 0.26 s / 900 s; scope peak 45.6 MiB / 16 GiB; swap peak 0 B; oom_kill 0
- mutant executable: NOT PRODUCED BY DESIGN
- exact selection: `the build command (no test selection by design)`
- raw outcome: build exit 101; ['error[E0283]: type annotations needed', 'error: could not compile `nexus-governed-control` (lib test) due to 1 previous error']
- outer classification: FAIL (build) — admissible type-(d) intentional mutant compile failure: E0283 at crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:1089
- named property / test / diagnostic: E0283 at crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:1089 (T-OWN)
- discovered count: not applicable
- run resources: NOT PRODUCED BY DESIGN
- restoration: RESTORE: EXACTLY_C
- linked restored-side receipt: LINK: VERIFIED — control: synthetic link

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
- named property / test / diagnostic: T-BIND (ledger::protocol::unit_tests::t_bind_every_bound_value_belongs_to_one_ledger)
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
- named property / test / diagnostic: E2
- discovered count: 59,104 of 250,000
- run resources: wall 0.0 s / 180 s; scope peak 3.3 MiB / 16 GiB; swap peak 0 B; oom_kill 0
- restoration: RESTORE: EXACTLY_C
- linked restored-side receipt: LINK: VERIFIED — control: synthetic link

## Not attempted

- authorized but not attempted (a stop ends the tranche): C19, C21, C27, C28, C9, C11, C17, C18a, C18b
- not authorized by this mission (never executed): C1, C2, C3a, C3b, C3c, C4, C5, C6, C7, C8, C10, C15, C16, C20, C23, C24, C25, C26, A1, A2, A3, A5, A6; A4 not rerun; M01–M183 not executed; no unchanged-core full configuration run except the standalone X-1; F-B2 and F-G not rerun on unchanged source

