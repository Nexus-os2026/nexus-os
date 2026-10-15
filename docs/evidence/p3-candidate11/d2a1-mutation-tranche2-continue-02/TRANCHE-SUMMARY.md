# Tranche-2 continuation summary — P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02

Derived status: **READY_FOR_MUTATION_TRANCHE2_FINAL_ARCHITECT_REVIEW** (from the receipts).

- Authorized (mission sections 4 and 9), in order: C5, A3, A6, C8, C15.
- Attempted: C5, A3, A6, C8, C15.
- New executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`): **5 of 5** (C5, A3, A6, C8, C15). None is accepted here: acceptance belongs to the Sole Architect.
- Stopped (first unsuccessful control; nothing later was attempted): none.
- Authorized but not attempted: none.

## Campaign status before Architect review

- Architect-accepted kills: **17 of 35** (Tranche 1: C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b; Tranche 2: C23, C10, C16, C26, C3a; record `documents/ARCHITECT-RULING-TRANCHE2-PARTIAL-ACCEPTANCE.md`), unchanged by this mission.
- Executor candidates pending Architect review from this mission: 5 (C5, A3, A6, C8, C15); if every one were accepted the count would be 22 of 35 (not claimed).
- Not authorized here (never executed): C1, C2, C3b, C3c, C4, C6, C7, C20, C24, C25, A1, A2, A5. A4: SATISFIED — ARCHITECT ACCEPTED (separately; not rerun). M01–M183: not executed. Unchanged-core configurations: linked, never rerun.
- C5: the preserved Tranche-2 build is admitted under the narrow C5-specific warning ruling (record `documents/ARCHITECT-RULING-C5-BUILD-ADMISSIBILITY.md`); it was not rebuilt; its historical `BUILD: NOT OK` and `CONTROL RESULT: MUTATION_CONTROL_BLOCKED` are unchanged.
- Not claimed: no complete campaign, no D2A-1 acceptance, no integration, no Phase Three completion.

## Controls (mission order)

### C5 — `receipts/61-C5`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-5 (`ledger::protocol::model_tests::full_f5`, original explorer); named property P13; ceiling 30,000,000 states
- frozen patch sha256 `a4776885b669bd912fd8710d5a3283f4c1863ab9213e8474d26707b5a6e5f3d8`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `e1a49f5ba952bc81cdac9f8f8ace9eeb2aa93495` (sha256 `31ca02cfaeb6058879af75091e2b1bdb21addf846d59ee3e5b4f5806fe30dedd`; computed in-process, never written to any object store)
- apply: not applied by design (C5: the preserved Tranche-2 mutant build)
- build: C5 BUILD ADMISSION: ARCHITECT-ADMISSIBLE BUILD AUTHENTICATED (NARROW C5-SPECIFIC WARNING RULING) — the preserved Tranche-2 build `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/receipts/56-C5/03-build` at `2e06860f4b404d1a79a0d290116239b5545b931f` (historical classification BUILD: NOT OK, unchanged; not rebuilt); exit 0, signal None; Finished [['release', 'optimized', '2m 48s']]
- build resources: wall 168.31 s / 900 s; scope peak 2494.7 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1972704 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt2x-C5/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `90a608834ad0227632ba26f095de5e16914942da86d2f05fdaab5a0d745c15f9` size 8,986,720
- exact selection: `ledger::protocol::model_tests::full_f5 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL P13: A 0 qualified 5 durable 0 head 5;   shortest trace (17 steps):; exit 101, signal None
- printed property: P13
- discovered count: 418,422 discovered (expanded 312,900) of 30,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P13
- run resources: wall 2.18 s / 2700 s; scope peak 60.2 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 64336 KiB
- restoration: no patch applied here (restored in Tranche 2); before the run: C5 PRESERVED WORKTREE: AUTHENTICATED (EXACTLY C; LOCKED; NOT MODIFIED BY THIS MISSION); after the run: C5 REAUTHENTICATION AFTER THE RUN: HOLDS (WORKTREE EXACTLY C; EXECUTABLE AND TARGET UNCHANGED)
- restored-side link: LINK: VERIFIED — control C5: restored-side receipt docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/65-full-F-5 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-5 unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - build admission: C5 BUILD ADMISSION: ARCHITECT-ADMISSIBLE BUILD AUTHENTICATED (NARROW C5-SPECIFIC WARNING RULING)
  - preserved worktree before the run: C5 PRESERVED WORKTREE: AUTHENTICATED (EXACTLY C; LOCKED; NOT MODIFIED BY THIS MISSION)
  - preserved executable: C5 PRESERVED EXECUTABLE: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt2x-C5/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 90a608834ad0227632ba26f095de5e16914942da86d2f05fdaab5a0d745c15f9 size 8986720
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P13
  - after the run: C5 REAUTHENTICATION AFTER THE RUN: HOLDS (WORKTREE EXACTLY C; EXECUTABLE AND TARGET UNCHANGED)
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/65-full-F-5 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-5 unchanged-core original-explorer PASS, type (e1))

### A3 — `receipts/62-A3`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-5 (`ledger::protocol::model_tests::full_f5`, original explorer); named property P12; ceiling 30,000,000 states
- frozen patch sha256 `ad0c64489e962756316b1a29b5d289a721517d1115525ee011e52db69cf2cc00`; mutated file `crates/nexus-governed-control/src/ledger/protocol/model_tests.rs`; mutant blob `72bab79d71687a1c056afe18ab7a938edc20f4a9` (sha256 `5eff1f8c581c2de4ab54470a2670057b6139c83f6bdc56326ff96f6a3d8c9ab9`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 50s']]
- build resources: wall 170.16 s / 900 s; scope peak 2491.7 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1970040 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt2c-A3/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `4805365da56eb6534708df651442299570561b3937838366b819267f420c4b64` size 8,984,528
- exact selection: `ledger::protocol::model_tests::full_f5 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL P12: the explanation of original 0 is covered by no durable acknowledgement;   shortest trace (14 steps):; exit 101, signal None
- printed property: P12
- discovered count: 112,880 discovered (expanded 80,145) of 30,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P12
- run resources: wall 0.58 s / 2700 s; scope peak 18.6 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 21308 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control A3: restored-side receipt docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/65-full-F-5 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-5 unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt2c-A3/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 4805365da56eb6534708df651442299570561b3937838366b819267f420c4b64 size 8984528
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P12
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/65-full-F-5 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-5 unchanged-core original-explorer PASS, type (e1))

### A6 — `receipts/63-A6`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-5 (`ledger::protocol::model_tests::full_f5`, original explorer); named property P12 or P15; ceiling 30,000,000 states
- frozen patch sha256 `0700052eb3e9fb91f36ee6d535b02b923ff007f1061490ac7dd16dd98d0d0ee8`; mutated file `crates/nexus-governed-control/src/ledger/protocol/model_tests.rs`; mutant blob `51e54aed7cc33ebc8348a74fe6c881c057336ee7` (sha256 `59c2f87666ad3272661497e30d7a4a841dcc5aafd90569307cc4fdbd69b5a4b4`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 47s']]
- build resources: wall 167.89 s / 900 s; scope peak 2492.6 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1973648 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt2c-A6/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `53d72467fe1d5cd1c318cf413ef0550c2cf05ce0d62954816cf25499b9e0b01e` size 8,984,560
- exact selection: `ledger::protocol::model_tests::full_f5 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL P12: the explanation of original 1 is covered by no durable acknowledgement;   shortest trace (31 steps):; exit 101, signal None
- printed property: P12
- discovered count: 3,537,396 discovered (expanded 3,240,539) of 30,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P12 or P15: P12 (permitted: P12 or P15)
- run resources: wall 20.91 s / 2700 s; scope peak 489.5 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 503212 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control A6: restored-side receipt docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/65-full-F-5 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-5 unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt2c-A6/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 53d72467fe1d5cd1c318cf413ef0550c2cf05ce0d62954816cf25499b9e0b01e size 8984560
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P12 or P15: P12 (permitted: P12 or P15)
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/65-full-F-5 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-5 unchanged-core original-explorer PASS, type (e1))

### C8 — `receipts/64-C8`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-B (`ledger::protocol::model_tests::full_fb`, original explorer); named property E5; ceiling 30,000,000 states
- frozen patch sha256 `a7cdd1925bf34283880384b78dd1c918fc61e64afef10094ab6d947bdeb97927`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `fd6abbd8a80dc8a01620981c76427ca76a9e1f7d` (sha256 `8baa7d6daa9e2adf3ea0b382a04d81e0d340cf40f1a09ecccd24a87bb849c30a`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 49s']]
- build resources: wall 169.8 s / 900 s; scope peak 2493.8 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1965040 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt2c-C8/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `60cc14b41266ec603e6a8400bac791cb7c93e03acd73a01c3ddbc72f0c3f2109` size 8,973,616
- exact selection: `ledger::protocol::model_tests::full_fb --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL E5: append_facts: an insert before every member is classified: Open:Ok Facts:Ok Insert:Ok Facts:Ok Rollback:Ok;   shortest trace (3 steps):; exit 101, signal None
- printed property: E5
- discovered count: 547 discovered (expanded 84) of 30,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property E5
- run resources: wall 0.0 s / 2700 s; scope peak 2.6 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 6288 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control C8: restored-side receipt docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/42-full-F-B at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-B unchanged-core composed-quotient PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt2c-C8/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 60cc14b41266ec603e6a8400bac791cb7c93e03acd73a01c3ddbc72f0c3f2109 size 8973616
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property E5
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/42-full-F-B at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-B unchanged-core composed-quotient PASS, type (e1))

### C15 — `receipts/65-C15`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-R (`ledger::protocol::model_tests::full_fr`, original explorer); named property E8; ceiling 30,000,000 states
- frozen patch sha256 `a9a5037d36b1a2a6fc5fddb5accea1198ea08d3823a3044bf45f1c4b6cea5810`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `abdd7518e6d1202a3a7c756af9716dc66183fd21` (sha256 `271b13d4422d0ec8322951cb71c2979a80b98d61975c3ab9ee0e5c69bc62141b`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 48s']]
- build resources: wall 168.47 s / 900 s; scope peak 2499.7 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1973520 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt2c-C15/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `89809695e0b87c8be259927a622bf2f2326a0323bde357aaa2406facf1c7b9f3` size 8,986,544
- exact selection: `ledger::protocol::model_tests::full_fr --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL E8: a stale token: resolve: Refused(NotHealthy); the answer is not stale, or the core or word changed;   shortest trace (8 steps):; exit 101, signal None
- printed property: E8
- discovered count: 83,988 discovered (expanded 45,292) of 30,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property E8
- run resources: wall 0.28 s / 2700 s; scope peak 11.5 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 15016 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control C15: restored-side receipt the AD-1-R5 F refinement discharge record (not a run; no outcome line; never PASS): Candidate C d5964ee5326c2733177307db6f507328023d361a; the accepted exact-final F-2 receipt docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/41-full-F-2 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd; the accepted proof docs/evidence/p3-candidate11/fr-stutter-r5-01/FR-STUTTERING-REFINEMENT-PROOF-DRAFT.md (sha256 c97332a33e68f3fecae12dc0ed863b24fbc1f8bffdbf67430765fd3b5281606b); AD-1-R5 (sha256 4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473) and its Owner adoption docs/evidence/p3-candidate11/ad1r5-adoption/OWNER-ADOPTION-2026-10-14.md (sha256 9990d2565a5ec96e0db03be832e96f64f774e662f6131cb69240c9a435757aa4)

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt2c-C15/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 89809695e0b87c8be259927a622bf2f2326a0323bde357aaa2406facf1c7b9f3 size 8986544
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property E8
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; the AD-1-R5 F refinement discharge record (not a run; no outcome line; never PASS): Candidate C d5964ee5326c2733177307db6f507328023d361a; the accepted exact-final F-2 receipt docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/41-full-F-2 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd; the accepted proof docs/evidence/p3-candidate11/fr-stutter-r5-01/FR-STUTTERING-REFINEMENT-PROOF-DRAFT.md (sha256 c97332a33e68f3fecae12dc0ed863b24fbc1f8bffdbf67430765fd3b5281606b); AD-1-R5 (sha256 4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473) and its Owner adoption docs/evidence/p3-candidate11/ad1r5-adoption/OWNER-ADOPTION-2026-10-14.md (sha256 9990d2565a5ec96e0db03be832e96f64f774e662f6131cb69240c9a435757aa4)
