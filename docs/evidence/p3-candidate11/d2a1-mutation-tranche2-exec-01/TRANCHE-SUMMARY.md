# Tranche-2 summary — P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01

Derived status: **MUTATION_CONTROL_BLOCKED** (from the receipts).

- Authorized (mission section 2), in order: C23, C10, C16, C26, C3a, C5, A3, A6, C8, C15.
- Attempted: C23, C10, C16, C26, C3a, C5.
- New executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`): **5 of 10** (C23, C10, C16, C26, C3a). None is accepted here: acceptance belongs to the Sole Architect.
- Stopped (first unsuccessful control; nothing later was attempted): C5.
- Authorized but not attempted: A3, A6, C8, C15.

## Campaign status after this tranche

- Architect-accepted kills: **12 of 35** (C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b; record `documents/ARCHITECT-RULING-TRANCHE1-FINAL-ACCEPTANCE.md`), unchanged by this mission.
- Executor candidates pending Architect review from this tranche: 5 (C23, C10, C16, C26, C3a).
- Not authorized here (never executed): C1, C2, C3b, C3c, C4, C6, C7, C20, C24, C25, A1, A2, A5. A4: SATISFIED — ARCHITECT ACCEPTED (separately; not rerun). M01–M183: not executed. Unchanged-core configurations: linked, never rerun.
- Not claimed: no complete campaign, no D2A-1 acceptance, no integration, no Phase Three completion.

## Controls (mission order)

### C23 — `receipts/51-C23`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-2 (`ledger::protocol::model_tests::full_f2`, original explorer); named property E1; ceiling 30,000,000 states
- frozen patch sha256 `731ffbedf845790c681b1fe7a2dc20d53174c8ce7378de04e5963865f0bc04aa`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `cd50b40ce632d4edcb95fff89389246b0d75c51e` (sha256 `b866fa5aefa95dba779ae7d567d304232251d9ae6e4145abfc850553bea49afa`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 49s']]
- build resources: wall 169.96 s / 900 s; scope peak 2481.9 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1968824 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt2x-C23/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `3549e576ab943bad1cb9ee0a2d96d073924b5225dc0b24abc399b5d6db2b98b9` size 8,985,352
- exact selection: `ledger::protocol::model_tests::full_f2 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL E1: word bits 0x4, core 0x84;   shortest trace (2 steps):; exit 101, signal None
- printed property: E1
- discovered count: 671 discovered (expanded 94) of 30,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property E1
- run resources: wall 0.0 s / 2700 s; scope peak 3.5 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 6312 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control C23: restored-side receipt docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/41-full-F-2 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-2 unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt2x-C23/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 3549e576ab943bad1cb9ee0a2d96d073924b5225dc0b24abc399b5d6db2b98b9 size 8985352
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property E1
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/41-full-F-2 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-2 unchanged-core original-explorer PASS, type (e1))

### C10 — `receipts/52-C10`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-1 (`ledger::protocol::model_tests::full_f1`, original explorer); named property E4; ceiling 60,000,000 states
- frozen patch sha256 `5e73745b2f8a6d02585519bac4990513e3ae6997dec9ad4e478fb92a2464a16c`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `329a830718b81a065e3a03f4958e19e8fa3c8d9c` (sha256 `a04658fa091c2e4dcbcb116b1ec3c87828fb4cc6a358684d6b0f1c744510041d`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 46s']]
- build resources: wall 166.87 s / 900 s; scope peak 2492.2 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1970248 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt2x-C10/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `5c8ef9f07b0ef37b430161c9da088bcbe7253ea17a6863ce2677d5f28645e867` size 8,984,744
- exact selection: `ledger::protocol::model_tests::full_f1 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL E4: append_facts said Not written but the ledger changed: Open:Ok Facts:Ok Insert:Ok Commit:Unknown Rollback:Ok;   shortest trace (2 steps):; exit 101, signal None
- printed property: E4
- discovered count: 42 discovered (expanded 1) of 60,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property E4
- run resources: wall 0.0 s / 2700 s; scope peak 3.9 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 6296 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control C10: restored-side receipt docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-1 unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt2x-C10/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 5c8ef9f07b0ef37b430161c9da088bcbe7253ea17a6863ce2677d5f28645e867 size 8984744
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property E4
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-1 unchanged-core original-explorer PASS, type (e1))

### C16 — `receipts/53-C16`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-D (`ledger::protocol::model_tests::full_fd`, original explorer); named property E3; ceiling 30,000,000 states
- frozen patch sha256 `e48a988cfc85488141d1e9eb033f897135ad9a4509edf16a3df85b55f8ff5e97`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `33f0d257df3e126c589becd0b1c5760c57085be8` (sha256 `c8c2b3ef54f2a2d13dc69aa0922cd67166021f9705ba32782bcfbf1e4db8c9d4`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 47s']]
- build resources: wall 167.11 s / 900 s; scope peak 2497.1 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1964792 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt2x-C16/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `d53f4f0c7c96364c55c654b98ec6966066e68e30d590da964eb5488a9069ce8c` size 8,984,872
- exact selection: `ledger::protocol::model_tests::full_fd --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL E3: live 1, unapplied 0; armed 0, counted 0, dropped 1;   shortest trace (3 steps):; exit 101, signal None
- printed property: E3
- discovered count: 1,513 discovered (expanded 317) of 30,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property E3
- run resources: wall 0.01 s / 2700 s; scope peak 3.3 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 6144 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control C16: restored-side receipt docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/68-full-F-D at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-D unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt2x-C16/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 d53f4f0c7c96364c55c654b98ec6966066e68e30d590da964eb5488a9069ce8c size 8984872
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property E3
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/68-full-F-D at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-D unchanged-core original-explorer PASS, type (e1))

### C26 — `receipts/54-C26`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-4 (`ledger::protocol::model_tests::full_f4`, original explorer); named property E9; ceiling 30,000,000 states
- frozen patch sha256 `1e26422d79a19553d925585aeb580f8a0c684bcc78d8a922ab5ab9a3756e8b4d`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `88f1783305f6418129671e77769fd4caf53e2a88` (sha256 `c9cccd06367aced8a183493cde03e4c9a62ece34c171fb7a95ed094173ae7b49`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 47s']]
- build resources: wall 167.71 s / 900 s; scope peak 2495.4 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1969616 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt2x-C26/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `336c30483abf4a0b0672caefdd87e8c7fc504094b36d1129cce4fb90b0663f97` size 8,984,936
- exact selection: `ledger::protocol::model_tests::full_f4 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL E9: a latch or a handover fault was cleared;   shortest trace (4 steps):; exit 101, signal None
- printed property: E9
- discovered count: 3,478 discovered (expanded 987) of 30,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property E9
- run resources: wall 0.01 s / 2700 s; scope peak 3.6 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 6396 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control C26: restored-side receipt docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/62-full-F-4 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-4 unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt2x-C26/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 336c30483abf4a0b0672caefdd87e8c7fc504094b36d1129cce4fb90b0663f97 size 8984936
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property E9
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/62-full-F-4 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-4 unchanged-core original-explorer PASS, type (e1))

### C3a — `receipts/55-C3a`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-7 (`ledger::protocol::model_tests::full_f7`, original explorer); named property P13 or P15; ceiling 30,000,000 states
- frozen patch sha256 `2442f63d9383dc0d36dd934e2a6b8fb0b3d6936f215e00ba3d964ede73fb6298`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `5c74cfbd8124a78c616526ae1d461895b1a16b82` (sha256 `abeb1664c20aa079fcbc869ed818f876c84e3af679dc6ea6f4fec72d805ad7a6`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 47s']]
- build resources: wall 168.05 s / 900 s; scope peak 2493.6 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1965720 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt2x-C3a/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `c71e66debb2f3d42527668f48818c74f5ca48f03aceda67c9b8285b6f8e5a0a6` size 8,984,320
- exact selection: `ledger::protocol::model_tests::full_f7 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL P13: A 0 qualified 6 durable 5 head 8;   shortest trace (22 steps):; exit 101, signal None
- printed property: P13
- discovered count: 746,152 discovered (expanded 609,616) of 30,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P13 or P15: P13 (permitted: P13 or P15)
- run resources: wall 4.36 s / 2700 s; scope peak 111.4 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 116912 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control C3a: restored-side receipt docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/67-full-F-7 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-7 unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt2x-C3a/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 c71e66debb2f3d42527668f48818c74f5ca48f03aceda67c9b8285b6f8e5a0a6 size 8984320
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P13 or P15: P13 (permitted: P13 or P15)
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/67-full-F-7 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-7 unchanged-core original-explorer PASS, type (e1))

### C5 — `receipts/56-C5`

- result: **CONTROL RESULT: MUTATION_CONTROL_BLOCKED**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-5 (`ledger::protocol::model_tests::full_f5`, original explorer); named property P13; ceiling 30,000,000 states
- frozen patch sha256 `a4776885b669bd912fd8710d5a3283f4c1863ab9213e8474d26707b5a6e5f3d8`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `e1a49f5ba952bc81cdac9f8f8ace9eeb2aa93495` (sha256 `31ca02cfaeb6058879af75091e2b1bdb21addf846d59ee3e5b4f5806fe30dedd`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: NOT OK — BUILD NOT OK (no error or warning diagnostic): kills nothing; the mission stops; exit 0, signal None; Finished [['release', 'optimized', '2m 48s']]
- build resources: wall 168.31 s / 900 s; scope peak 2494.7 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1972704 KiB
- mutant executable: not produced
- exact selection: `ledger::protocol::model_tests::full_f5 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): not run; —
- printed property: —
- discovered count: —
- outer classification: not run — not run
- run resources: —
- restoration: RESTORE: EXACTLY_C
- restored-side link: —

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: NOT OK
  - restoration: RESTORE: EXACTLY_C
  - STOP: the mutant build did not succeed (kills nothing; no rebuild). No later control is executed.
