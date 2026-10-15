# Tranche-3 summary — P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01

Derived status: **MUTATION_CONTROL_BLOCKED** (from the receipts).

- Authorized (mission section 2), in order: C2, C24, C3b, C3c, C6, C7, A1, A2, C20, A5.
- Attempted: C2, C24.
- New executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`): **1 of 10** (C2). None is accepted here: acceptance belongs to the Sole Architect.
- Stopped (first unsuccessful control; nothing later was attempted): C24.
- Authorized but not attempted: C3b, C3c, C6, C7, A1, A2, C20, A5.

## Campaign status before Architect review

- Architect-accepted kills: **22 of 35** (Tranche 1: C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b; Tranche 2: C23, C10, C16, C26, C3a; Tranche 2 continuation: C5, A3, A6, C8, C15; record `documents/ARCHITECT-RULING-TRANCHE2-CONTINUATION-ACCEPTANCE.md`), unchanged by this mission.
- Executor candidates pending Architect review from this mission: 1 (C2); if every one were accepted the count would be 23 of 35 (not claimed).
- Reserved for the final witness-bound tranche (never executed here): C1, C4, C25. A4: SATISFIED — ARCHITECT ACCEPTED (separately; not rerun). M01–M183: not executed. Unchanged-core configurations: linked, never rerun.
- C7's run without A4 on F-3 (mission section 5) is informational: it is not C7's kill receipt and counts for nothing in the campaign.
- Not claimed: no complete campaign, no D2A-1 acceptance, no integration, no Phase Three completion.

## Controls (mission order)

### C2 — `receipts/66-C2`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-1 (`ledger::protocol::model_tests::full_f1`, original explorer); permitted property P4; ceiling 60,000,000 states
- frozen patch sha256 `f02c3e721aa2c96bb7d15fe081acf3dc6e4c1958de096f4215bc55a7cafc218d`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `1c10adbc93c2fe0984d0ac677727a3bdc82b8e5d` (sha256 `4889d2ba4ac5c9a9c7e982d750957fa5cfd2af8c8be9cd0580ca7a859eac2403`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 47s']]
- build resources: wall 167.94 s / 900 s; scope peak 2497.1 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1970344 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mt3x-C2/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `8460515fe149a78637bcb1d797d9cb5c2ed5b89eb2361cd74e02e96cedaed0ed` size 8,984,096
- exact selection: `ledger::protocol::model_tests::full_f1 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): outcome FAIL; FAIL P4: U is occupied together with C or X;   shortest trace (8 steps):; exit 101, signal None
- printed property: P4
- discovered count: 361,534 discovered (expanded 185,130) of 60,000,000
- outer classification: CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING — FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P4
- tranche-3 rules: TRANCHE-3 RULES: HOLD
- run resources: wall 1.29 s / 2700 s; scope peak 37.7 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 42304 KiB
- restoration: RESTORE: EXACTLY_C
- restored-side link: LINK: VERIFIED — control C2: restored-side receipt docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-1 unchanged-core original-explorer PASS, type (e1))

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt3x-C2/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 8460515fe149a78637bcb1d797d9cb5c2ed5b89eb2361cd74e02e96cedaed0ed size 8984096
  - run (type (a)): CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P4
  - tranche-3 rules: TRANCHE-3 RULES: HOLD
  - restoration: RESTORE: EXACTLY_C
  - linked restored-side receipt: LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-1 unchanged-core original-explorer PASS, type (e1))

### C24 — `receipts/67-C24`

- result: **CONTROL RESULT: MUTATION_CONTROL_BLOCKED**
- gate: GATE: OPEN
- receipt type: (a) original-explorer mutant FAIL; restored (e1); configuration F-1 (`ledger::protocol::model_tests::full_f1`, original explorer); permitted property P8; ceiling 60,000,000 states
- frozen patch sha256 `39439c3c17a3b90d31ec3bec1f3b80e753ab3d042cc59ea9848c748872580e44`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `3e1bfbe746851b6c63811343cde9dd3b83ad0fa9` (sha256 `d4732d83f924d824752d62d59a28e12cc97a456669f7cbea0868d7e6e9742268`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: NOT OK — BUILD NOT OK (no error or warning diagnostic): kills nothing; the mission stops; exit 0, signal None; Finished [['release', 'optimized', '2m 47s']]
- build resources: wall 167.92000000000002 s / 900 s; scope peak 2483.2 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1964704 KiB
- mutant executable: not produced
- exact selection: `ledger::protocol::model_tests::full_f1 --exact --ignored --nocapture --test-threads=1`
- raw outcome (as printed): not run; —
- printed property: —
- discovered count: —
- outer classification: not run — not run
- tranche-3 rules: not reached
- run resources: —
- restoration: RESTORE: EXACTLY_C
- restored-side link: —

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: NOT OK
  - restoration: RESTORE: EXACTLY_C
  - STOP: the mutant build did not succeed (kills nothing; no rebuild). No later control is executed.
