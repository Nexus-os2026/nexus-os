# Tranche-3 continuation summary — P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02

Derived status: **MUTATION_CONTROL_BLOCKED** (from the receipts).

- Authorized (mission section 6), in order: C24, C3b, C3c, C6, C7, A1, A2, C20, A5.
- Attempted: C24, C3b, C3c, C6, C7.
- New executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`): **4 of 9** (C24, C3b, C3c, C6). None is accepted here: acceptance belongs to the Sole Architect.
- Stopped (first unsuccessful control; nothing later was attempted): C7.
- Authorized but not attempted: A1, A2, C20, A5.

## Campaign status before Architect review

- Architect-accepted kills: **23 of 35** (Tranche 1: C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b; Tranche 2: C23, C10, C16, C26, C3a; Tranche 2 continuation: C5, A3, A6, C8, C15; Tranche 3: C2; record `documents/ARCHITECT-RULING-TRANCHE3-C2-ACCEPTANCE.md`), unchanged by this mission.
- Executor candidates pending Architect review from this mission: 4 (C24, C3b, C3c, C6); if every one were accepted the count would be 27 of 35 (not claimed).
- Reserved for the final witness-bound tranche (never executed here): C1, C4, C25. A4: SATISFIED — ARCHITECT ACCEPTED (separately; not rerun). M01–M183: not executed. Unchanged-core configurations: linked, never rerun.
- C7's run without A4 on F-3 (mission section 12) is informational: it is not C7's kill receipt and counts for nothing in the campaign.
- Build rulings (`documents/ARCHITECT-RULING-C24-C3B-C3C-BUILD-ADMISSIBILITY.md`, `documents/ARCHITECT-AMENDMENT-01-C3B-C3C-BUILD-RULE.md`): C24 is not rebuilt (its preserved Tranche-3 build is admitted under the narrow C24 ruling); C3b and C3c admit a clean build or exactly their one predicted warning group (AMENDMENT-01: permitted, not required); C6, C7, A1, A2, C20 and A5 require zero warnings and zero errors. The general build classifier is unchanged.
- Not claimed: no complete campaign, no D2A-1 acceptance, no integration, no Phase Three completion.

## Controls (mission order)

### C24 — `receipts/81-C24`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-1 (`ledger::protocol::model_tests::full_f1`, original explorer); permitted property P8; ceiling 60,000,000 states
- frozen patch sha256 `39439c3c17a3b90d31ec3bec1f3b80e753ab3d042cc59ea9848c748872580e44`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `3e1bfbe746851b6c63811343cde9dd3b83ad0fa9` (sha256 `d4732d83f924d824752d62d59a28e12cc97a456669f7cbea0868d7e6e9742268`; computed in-process, never written to any object store)
- apply: no patch applied by this mission (C24: the preserved Tranche-3 build; its worktree restored to C there)
- build: not rebuilt (the preserved Tranche-3 build; see the admission); —
- build admission (C24: the narrow C24 ruling; C3b/C3c: AMENDMENT-01; every other control: none, the general classifier alone): C24 BUILD ADMISSION: ARCHITECT-ADMISSIBLE BUILD AUTHENTICATED (NARROW C24-SPECIFIC WARNING RULING)
- build resources: —
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt3x-C24/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `8a81956b1105d4639a804cdbdc51555b7b3416bfcec86a1be223dec2c9fa7b1d` size 8,987,680
- exact selection: `ledger::protocol::model_tests::full_f1 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL P8: G1's attempt is written, uncorrected and not in U;   shortest trace (7 steps):; exit 101, signal None
- printed property: P8
- discovered count: 185,114 discovered (expanded 87,111) of 60,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P8
- tranche-3 rules: TRANCHE-3 RULES: HOLD
- run resources: wall 0.62 s / 2700 s; scope peak 20.1 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 23684 KiB
- restoration: C24 REAUTHENTICATION AFTER THE RUN: HOLDS (WORKTREE EXACTLY C; EXECUTABLE AND TARGET UNCHANGED)
- restored-side link: LINK: VERIFIED — control C24: restored-side receipt docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-1 unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - build admission: C24 BUILD ADMISSION: ARCHITECT-ADMISSIBLE BUILD AUTHENTICATED (NARROW C24-SPECIFIC WARNING RULING)
  - preserved worktree before the run: C24 PRESERVED WORKTREE: AUTHENTICATED (EXACTLY C; LOCKED; NOT MODIFIED BY THIS MISSION)
  - preserved executable: C24 PRESERVED EXECUTABLE: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt3x-C24/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 8a81956b1105d4639a804cdbdc51555b7b3416bfcec86a1be223dec2c9fa7b1d size 8987680
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P8
  - tranche-3 rules: TRANCHE-3 RULES: HOLD
  - after the run: C24 REAUTHENTICATION AFTER THE RUN: HOLDS (WORKTREE EXACTLY C; EXECUTABLE AND TARGET UNCHANGED)
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-1 unchanged-core original-explorer PASS, type (e1))

### C3b — `receipts/82-C3b`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-6 (`ledger::protocol::model_tests::full_f6`, original explorer); permitted property P13 or P12; ceiling 30,000,000 states
- frozen patch sha256 `614e53fd75518991c5e6518bc0fe61cf546cf092b955e3eda0e71cf25d5f6dd4`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `e6aff1884a4b9b7f880eee8784c371648fb2c073` (sha256 `84dc2a424ce80ce47b71871f111c4aaa37d8b776dc4851f16513d86312247273`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 50s']]
- build admission (C24: the narrow C24 ruling; C3b/C3c: AMENDMENT-01; every other control: none, the general classifier alone): C3b BUILD ADMISSION: ADMISSIBLE — CLEAN BUILD (BUILD: PINNED UNDER THE UNCHANGED GENERAL CLASSIFIER)
- build resources: wall 170.1 s / 900 s; scope peak 2502.7 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1971360 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt3c-C3b/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `8aeecde3c905a0f7069e28eec7f187db0cf2d9637e93293841bff92d8d31281b` size 8,983,840
- exact selection: `ledger::protocol::model_tests::full_f6 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL P13: A 0 qualified 7 durable 7 head 6;   shortest trace (11 steps):; exit 101, signal None
- printed property: P13
- discovered count: 67,871 discovered (expanded 44,338) of 30,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P13 or P12: P13 (permitted: P13 or P12)
- tranche-3 rules: TRANCHE-3 RULES: HOLD
- run resources: wall 0.31 s / 2700 s; scope peak 11.4 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 15056 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control C3b: restored-side receipt docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/66-full-F-6 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-6 unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - build admission (C3b/C3c rule, AMENDMENT-01): C3b BUILD ADMISSION: ADMISSIBLE — CLEAN BUILD (BUILD: PINNED UNDER THE UNCHANGED GENERAL CLASSIFIER)
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt3c-C3b/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 8aeecde3c905a0f7069e28eec7f187db0cf2d9637e93293841bff92d8d31281b size 8983840
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P13 or P12: P13 (permitted: P13 or P12)
  - tranche-3 rules: TRANCHE-3 RULES: HOLD
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/66-full-F-6 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-6 unchanged-core original-explorer PASS, type (e1))

### C3c — `receipts/83-C3c`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-6 (`ledger::protocol::model_tests::full_f6`, original explorer); permitted property P12; ceiling 30,000,000 states
- frozen patch sha256 `1e60d98d6d42c0aab581a5bde34162f9767949a495fad801b73d9c740d9e2e85`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `5cc9c09ac96062c5d348e86ace8cb0ac2225e83d` (sha256 `e72f8f619fa1c9b4a8379351138b354fa83fe470b1c56450b8b22f28c61710c2`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 48s']]
- build admission (C24: the narrow C24 ruling; C3b/C3c: AMENDMENT-01; every other control: none, the general classifier alone): C3c BUILD ADMISSION: ADMISSIBLE — CLEAN BUILD (BUILD: PINNED UNDER THE UNCHANGED GENERAL CLASSIFIER)
- build resources: wall 168.4 s / 900 s; scope peak 2484.2 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1965300 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt3c-C3c/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `e8cfbe4c98412d4ae78fbe3c38d9c6e93d2af16d445631971618822646c38e32` size 8,983,584
- exact selection: `ledger::protocol::model_tests::full_f6 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL P12: the explanation of original 0 is covered by no durable acknowledgement;   shortest trace (18 steps):; exit 101, signal None
- printed property: P12
- discovered count: 541,827 discovered (expanded 381,761) of 30,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P12
- tranche-3 rules: TRANCHE-3 RULES: HOLD
- run resources: wall 2.88 s / 2700 s; scope peak 75.9 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 80764 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control C3c: restored-side receipt docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/66-full-F-6 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-6 unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - build admission (C3b/C3c rule, AMENDMENT-01): C3c BUILD ADMISSION: ADMISSIBLE — CLEAN BUILD (BUILD: PINNED UNDER THE UNCHANGED GENERAL CLASSIFIER)
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt3c-C3c/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 e8cfbe4c98412d4ae78fbe3c38d9c6e93d2af16d445631971618822646c38e32 size 8983584
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P12
  - tranche-3 rules: TRANCHE-3 RULES: HOLD
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/66-full-F-6 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-6 unchanged-core original-explorer PASS, type (e1))

### C6 — `receipts/84-C6`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-PT (`ledger::protocol::model_tests::full_fpt`, original explorer, A4 ON); permitted property P6; ceiling 30,000,000 states
- frozen patch sha256 `5c647500156a5e9c9b3faa27ce59ea2c09f0c45d1a50daa7a7d1d0a33c5a7e5e`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `6dde859cb243b9f4304cda61b0917b1fc555dde3` (sha256 `649d95ba8232cefa81d71c7793d8413e956271efebc25bc291ddb4288084c2a3`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 48s']]
- build admission (C24: the narrow C24 ruling; C3b/C3c: AMENDMENT-01; every other control: none, the general classifier alone): —
- build resources: wall 168.64 s / 900 s; scope peak 2494.2 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1968404 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt3c-C6/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `5c88a5c7b0c2c9e9ab644c97bd93a4e97c0570e85807a5be7467466bdcdd832d` size 8,985,584
- exact selection: `ledger::protocol::model_tests::full_fpt --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL P6: F1 was refused as a conflict and is Paid;   shortest trace (14 steps):; exit 101, signal None
- printed property: P6
- discovered count: 2,652,070 discovered (expanded 1,807,301) of 30,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P6
- tranche-3 rules: TRANCHE-3 RULES: HOLD
- run resources: wall 12.96 s / 2700 s; scope peak 342.6 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 353320 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control C6: restored-side receipt docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/44-full-F-PT at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-PT unchanged-core composed-quotient PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt3c-C6/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 5c88a5c7b0c2c9e9ab644c97bd93a4e97c0570e85807a5be7467466bdcdd832d size 8985584
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P6
  - tranche-3 rules: TRANCHE-3 RULES: HOLD
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/44-full-F-PT at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-PT unchanged-core composed-quotient PASS, type (e1))

### C7 — `receipts/85-C7`

- result: **CONTROL RESULT: MUTATION_CONTROL_BLOCKED**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-PT (`ledger::protocol::model_tests::full_fpt`, original explorer, A4 ON); permitted property P6; ceiling 30,000,000 states
- frozen patch sha256 `b5d8bc6c9254e0ca2853fcf2f9f220f24782ac6cfa77efe7032365d76f0efbc4`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `0d4ab9929c0e15d910735a52b72a5f3855bef58a` (sha256 `6d2f2d49842cdf5cc155dd1cb0984e121ada154cfc69a412219c7ac12dc846aa`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 47s']]
- build admission (C24: the narrow C24 ruling; C3b/C3c: AMENDMENT-01; every other control: none, the general classifier alone): —
- build resources: wall 167.65 s / 900 s; scope peak 2496.7 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1966616 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt3c-C7/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `b320977a41d78ad031c74cb0f3fc9fe8c3f89051bbd599cc9b42deebb303a7e5` size 8,984,592
- exact selection: `ledger::protocol::model_tests::full_fpt --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL E11: a table entry left in a step that did not see its explanation in the ledger;   shortest trace (13 steps):; exit 101, signal None
- printed property: E11
- discovered count: 1,071,165 discovered (expanded 686,654) of 30,000,000
- outer classification: CLASSIFICATION: NOT A KILL EVIDENCE CANDIDATE — FAIL of E11, not the named property P6: kills nothing (no attribution exception)
- tranche-3 rules: TRANCHE-3 RULES: NOT SATISFIED (failed: FAIL the printed first property is permitted for C7: P6: E11: a table entry left in a step that did not see its explanation in the ledger)
- C7 informational run without A4 on F-3 (not the kill receipt): not reached (it follows only an admissible F-PT/P6 candidate, before restoration)
- run resources: wall 4.7 s / 2700 s; scope peak 131.7 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 136516 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: —

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt3c-C7/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 b320977a41d78ad031c74cb0f3fc9fe8c3f89051bbd599cc9b42deebb303a7e5 size 8984592
  - run (type (a)): CLASSIFICATION: NOT A KILL EVIDENCE CANDIDATE; outer class (AD-1-R4 K.2): FAIL of E11, not the named property P6: kills nothing (no attribution exception)
  - tranche-3 rules: TRANCHE-3 RULES: NOT SATISFIED
  - restoration: RESTORE: EXACTLY_C
  - STOP: the mutant run is not admissible kill evidence of the named property or test. No later control is executed.
