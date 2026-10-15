# Tranche-3 tail summary — P3-C11-DR01-D2A1-MUTATION-TRANCHE3-TAIL-01

Derived status: **KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING** (from the receipts).

- Authorized (mission sections 1 and 6), in order: A1, A2, C20, A5.
- Attempted: A1, A2, C20, A5.
- New executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`): **4 of 4** (A1, A2, C20, A5). None is accepted here: acceptance belongs to the Sole Architect.
- Stopped (first unsuccessful control; nothing later was attempted): none.
- Authorized but not attempted: none.

## Campaign status before Architect review

- Architect-accepted kills: **27 of 35** (Tranche 1: C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b; Tranche 2: C23, C10, C16, C26, C3a; Tranche 2 continuation: C5, A3, A6, C8, C15; Tranche 3: C2; Tranche 3 continuation: C24, C3b, C3c, C6; record `documents/ARCHITECT-RULING-TRANCHE3-CONTINUATION-ACCEPTANCE.md`), unchanged by this mission.
- Executor candidates pending Architect review from this mission: 4 (A1, A2, C20, A5): 27 accepted + 4 pending Architect review. Only the Architect may rule a higher accepted count (not claimed).
- MUTATION_CONTROL_BLOCKED (Architect ruling on `c10681b3c65dd377b5a13d08140427e8b34441d0`): C7 — E11 observed, P6 unmet; expressly excluded from this mission (no rerun, rebuild, patch alteration, property substitution, informational comparison or contract amendment).
- Reserved for the final witness-bound tranche (never executed here): C1, C4, C25. A4: SATISFIED — ARCHITECT ACCEPTED (separately; not rerun). M01–M183: not executed. Unchanged-core configurations: linked, never rerun.
- Build rule (mission section 7): every control one fresh build with zero warnings and zero errors under the unchanged general classifier; no warning admission exception.
- L1 (C20, A5; mission sections 6 and 8): admissible only after a COMPLETE search (no prefix inference, no state-ceiling inference); INCOMPLETE kills nothing.
- Not claimed: no accepted component, no D2A-1 acceptance, no Phase Three acceptance, no integration.

## Controls (mission order)

### A1 — `receipts/81-A1`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-1 (`ledger::protocol::model_tests::full_f1`, original explorer); permitted property P1; ceiling 60,000,000 states
- frozen patch sha256 `e3a21c74babd86e1c40c58457319dc6901c7fac7a6e56d7c8f149e3389f72507`; mutated file `crates/nexus-governed-control/src/ledger/protocol/model_tests.rs`; mutant blob `26c09e7ff93633095de8d7a15ee81f5f46728336` (sha256 `ea3ac997cd4a66925613346707f305ef3d8b68c0871e581110a9c13e9e4dd632`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 47s']]
- build resources: wall 167.89 s / 900 s; scope peak 2499.8 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1972648 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt3t-A1/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `d58ccd31c2c42acbb065d3ef08a00f42a8966c04bd0f000f938f3b9b2bdf6b12` size 8,984,768
- exact selection: `ledger::protocol::model_tests::full_f1 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL P1: a result is unrecorded, or U, C or X is occupied, at a successful admission;   shortest trace (4 steps):; exit 101, signal None
- printed property: P1
- raw state counts: 1,962 discovered (expanded 430) of 60,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P1
- tranche-3 rules: TRANCHE-3 RULES: HOLD
- run resources: wall 0.01 s / 2700 s; scope peak 4.3 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 6180 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control A1: restored-side receipt docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-1 unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt3t-A1/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 d58ccd31c2c42acbb065d3ef08a00f42a8966c04bd0f000f938f3b9b2bdf6b12 size 8984768
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P1
  - tranche-3 rules: TRANCHE-3 RULES: HOLD
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-1 unchanged-core original-explorer PASS, type (e1))

### A2 — `receipts/82-A2`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-1 (`ledger::protocol::model_tests::full_f1`, original explorer); permitted property P1; ceiling 60,000,000 states
- frozen patch sha256 `5f1f30486da2457e2a50f0f72f1c96bfecf67e12692d47aa20d0ae3a1d677cf2`; mutated file `crates/nexus-governed-control/src/ledger/protocol/model_tests.rs`; mutant blob `c0ac428f58497d8cf42458d51f86db0f2e4b84ba` (sha256 `855a6ff6e419e6b3e09425047b629b42708d493507194a97516326bf2d8b2633`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 47s']]
- build resources: wall 167.92000000000002 s / 900 s; scope peak 2490.4 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1967848 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt3t-A2/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `257cb481e0de203654f7007d60ebe537e84b71dfd8c98b02b90288365249662d` size 8,984,784
- exact selection: `ledger::protocol::model_tests::full_f1 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL P1: a result is unrecorded, or U, C or X is occupied, at a successful admission;   shortest trace (3 steps):; exit 101, signal None
- printed property: P1
- raw state counts: 1,453 discovered (expanded 277) of 60,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P1
- tranche-3 rules: TRANCHE-3 RULES: HOLD
- run resources: wall 0.0 s / 2700 s; scope peak 3.9 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 6324 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control A2: restored-side receipt docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-1 unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt3t-A2/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 257cb481e0de203654f7007d60ebe537e84b71dfd8c98b02b90288365249662d size 8984784
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P1
  - tranche-3 rules: TRANCHE-3 RULES: HOLD
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-1 unchanged-core original-explorer PASS, type (e1))

### C20 — `receipts/83-C20`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-2 (`ledger::protocol::model_tests::full_f2`, original explorer); permitted property L1; ceiling 30,000,000 states
- frozen patch sha256 `5ea1889e67bc552d08c75074f3717d87a4509c56d13448d61bc574fc3d93f62f`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `7bb5aa2abbb52249efe76ab22dd921cb74dd337c` (sha256 `0e521df8ae9dfc69114085daafaa96f8b55f2119e6a3c51afa142af8acc5caaf`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 47s']]
- build resources: wall 167.28 s / 900 s; scope peak 2495.1 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1972160 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt3t-C20/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `5433a13ba47468ff457cf665ae1383e64fdd7de26086b4a95b9ffd7c4b121a02` size 8,988,112
- exact selection: `ledger::protocol::model_tests::full_f2 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL L1: 206694 states lie on or behind a cycle of non-fault steps; one is reached by;   shortest trace (8 steps):; exit 101, signal None
- printed property: L1
- raw state counts: 12,590,754 discovered (expanded 12,590,754) of 30,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property L1
- tranche-3 rules: TRANCHE-3 RULES: HOLD
- L1 complete search (AD-1-R4 Q): yes: outcome FAIL; INCOMPLETE line None; discovered 12590754 = expanded 12590754 (every discovered state expanded) of 30,000,000; elapsed 66.8 s of 2700 s; TRANCHE-3 RULES: HOLD
- run resources: wall 66.95 s / 2700 s; scope peak 1632.3 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1670240 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control C20: restored-side receipt docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/41-full-F-2 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-2 unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt3t-C20/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 5433a13ba47468ff457cf665ae1383e64fdd7de26086b4a95b9ffd7c4b121a02 size 8988112
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property L1
  - tranche-3 rules: TRANCHE-3 RULES: HOLD
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/41-full-F-2 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-2 unchanged-core original-explorer PASS, type (e1))

### A5 — `receipts/84-A5`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-2 (`ledger::protocol::model_tests::full_f2`, original explorer); permitted property L1; ceiling 30,000,000 states
- frozen patch sha256 `913eba6d31d7cbe6d65f4543b9fb2d0def6ca7e7586b26209b3ee5063a2f4d32`; mutated file `crates/nexus-governed-control/src/ledger/protocol/model_tests.rs`; mutant blob `cb08f5e8c62b3ce267f0688cc880261a28cc29a2` (sha256 `8d6f100133cced95076d584acc8a861c6da39eb234fc8a570775811b3e97ec5f`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 47s']]
- build resources: wall 167.97 s / 900 s; scope peak 2495.0 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1963408 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt3t-A5/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `22de28a2f3ff3a94d480f5247523ca2139600988139e9c4769d79be874041a85` size 8,984,464
- exact selection: `ledger::protocol::model_tests::full_f2 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL L1: 2162310 states lie on or behind a cycle of non-fault steps; one is reached by;   shortest trace (10 steps):; exit 101, signal None
- printed property: L1
- raw state counts: 12,684,666 discovered (expanded 12,684,666) of 30,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property L1
- tranche-3 rules: TRANCHE-3 RULES: HOLD
- L1 complete search (AD-1-R4 Q): yes: outcome FAIL; INCOMPLETE line None; discovered 12684666 = expanded 12684666 (every discovered state expanded) of 30,000,000; elapsed 66.9 s of 2700 s; TRANCHE-3 RULES: HOLD
- run resources: wall 67.08 s / 2700 s; scope peak 1644.6 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1682324 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control A5: restored-side receipt docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/41-full-F-2 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-2 unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt3t-A5/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 22de28a2f3ff3a94d480f5247523ca2139600988139e9c4769d79be874041a85 size 8984464
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property L1
  - tranche-3 rules: TRANCHE-3 RULES: HOLD
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/41-full-F-2 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-2 unchanged-core original-explorer PASS, type (e1))
