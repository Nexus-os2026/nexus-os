# Tranche summary — P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01

Derived tranche status: **MUTATION_CONTROL_BLOCKED** (from the receipts; publication and the final report follow).

KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING: **0** of 12 authorized controls (none). No control is called killed or accepted: acceptance belongs to the Sole Architect, receipt by receipt.

## Baseline standalone receipts (mission section 7) and the healthy build (section 8)

| Step | Type | Receipt | Classification | Facts | Resources |
|---|---|---|---|---|---|
| X-1 (for C12) | (e1) | `receipts/10-baseline-X-1` | CLASSIFICATION: PASS — PASS — admissible unchanged-core original-explorer X-1 PASS (type (e1)) | states 59,104 of 250,000; branch accounting NORMALIZED | wall 0.18 s / 180 s; scope peak 8.5 MiB / 16 GiB; swap peak 0 B; oom_kill 0 |
| T-BIND (for C14) | (e2) | `receipts/11-baseline-T-BIND` | CLASSIFICATION: PASS — PASS — admissible restored named-unit-test T-BIND PASS (type (e2)) | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s | wall 0.0 s / 180 s; scope peak 3.3 MiB / 16 GiB; swap peak 0 B; oom_kill 0 |
| T-CELL (for C19) | (e2) | `receipts/12-baseline-T-CELL` | CLASSIFICATION: PASS — PASS — admissible restored named-unit-test T-CELL PASS (type (e2)) | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s | wall 0.0 s / 180 s; scope peak 3.8 MiB / 16 GiB; swap peak 0 B; oom_kill 0 |
| W6 (for C21) | (e2) | `receipts/13-baseline-W6` | CLASSIFICATION: PASS — PASS — admissible restored named-unit-test W6 PASS (type (e2)) | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s | wall 0.0 s / 180 s; scope peak 3.2 MiB / 16 GiB; swap peak 0 B; oom_kill 0 |
| T-PREC (for C27) | (e2) | `receipts/14-baseline-T-PREC` | CLASSIFICATION: PASS — PASS — admissible restored named-unit-test T-PREC PASS (type (e2)) | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s | wall 0.0 s / 180 s; scope peak 3.1 MiB / 16 GiB; swap peak 0 B; oom_kill 0 |
| T-RES (for C28) | (e2) | `receipts/15-baseline-T-RES` | CLASSIFICATION: PASS — PASS — admissible restored named-unit-test T-RES PASS (type (e2)) | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s | wall 0.0 s / 180 s; scope peak 3.7 MiB / 16 GiB; swap peak 0 B; oom_kill 0 |
| healthy build (for C22) | (e4) | `receipts/20-healthy-build` | CLASSIFICATION: PASS — PASS — admissible healthy-build type-(e4) success (exact command, Candidate C) | executable /home/nexus/NEXUS/nexus-os-p3-c11-mt1x-healthy/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 1f96de589533c6a1c46a2343af97347f1e6220c6896a04487d8172aaa8839d75 size 8984464 (recorded, never run); 2552 target files recorded | wall 167.82 s / 900 s; scope peak 2498.3 MiB / 16 GiB; swap peak 0 B; oom_kill 0 |

## Controls (mission section 9 order)

### C22 — `receipts/31-C22`

- result: **CONTROL RESULT: MUTATION_CONTROL_BLOCKED**
- receipt type: (d) intentional mutant compile failure; restored (e4) healthy build
- frozen patch sha256: `e017ced648c6d4329ab8c744441e8fc03e8207b47e9cb3387202cc29ce7e95c6`
- mutant source blob: `694b744de3b70a3a092c633b934262bd510d41aa` (sha256 `0f13a8e5512439aa6385bbe65c20ceb6a77fe6c819216ef82564f7b8880425e5`; computed in-process, never written to any object store)
- build: CLASSIFICATION: NOT A KILL EVIDENCE CANDIDATE — NOT ADMISSIBLE (source EXACTLY_C_PLUS_PATCH before and after the build)
- build resources: wall 124.73 s / 900 s; scope peak 2493.9 MiB / 16 GiB; swap peak 0 B; oom_kill 0
- mutant executable: NOT PRODUCED BY DESIGN
- exact selection: `the build command (no test selection by design)`
- raw outcome: build exit 101; ['error[E0283]: type annotations needed', 'error: could not compile `nexus-governed-control` (lib test) due to 1 previous error']
- outer classification: NOT ADMISSIBLE (source EXACTLY_C_PLUS_PATCH before and after the build)
- named property / test / diagnostic: E0283 at crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:1089 (T-OWN)
- discovered count: not applicable
- run resources: NOT PRODUCED BY DESIGN
- restoration: RESTORE: FAILED
- linked restored-side receipt: —

## Not attempted

- authorized but not attempted (a stop ends the tranche): C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b
- not authorized by this mission (never executed): C1, C2, C3a, C3b, C3c, C4, C5, C6, C7, C8, C10, C15, C16, C20, C23, C24, C25, C26, A1, A2, A3, A5, A6; A4 not rerun; M01–M183 not executed; no unchanged-core full configuration run except the standalone X-1; F-B2 and F-G not rerun on unchanged source

