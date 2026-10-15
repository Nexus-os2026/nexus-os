# C1/C25 witness-bound mutation summary — P3-C11-DR01-D2A1-MUTATION-WITNESS-C1-C25-01

Derived status: **KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING** (from the receipts).

- Authorized (mission sections 3-5), in order (C25 only after a C1 candidate): C1, C25.
- Attempted: C1, C25.
- New executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`): **2 of 2** (C1, C25). None is accepted here: acceptance belongs to the Sole Architect.
- Stopped (first unsuccessful control; nothing later was attempted): none.
- Authorized but not attempted: none.

## Campaign status before Architect review

- Architect-accepted kills: **31 of 35** (Tranche 1: C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b; Tranche 2: C23, C10, C16, C26, C3a; Tranche 2 continuation: C5, A3, A6, C8, C15; Tranche 3: C2; Tranche 3 continuation: C24, C3b, C3c, C6; Tranche 3 tail: A1, A2, C20, A5; record `documents/ARCHITECT-RULING-TRANCHE3-TAIL-ACCEPTANCE.md`), unchanged by this mission.
- Executor candidates pending Architect review from this mission: 2 (C1, C25): 31 accepted + 2 pending Architect review. Only the Architect may rule a higher accepted count (not claimed).
- MUTATION_CONTROL_BLOCKED (Architect ruling on `c10681b3c65dd377b5a13d08140427e8b34441d0`): C7 — E11 observed, P6 not established; not executed here.
- Reserved (never executed here): C4. A4: SATISFIED — ARCHITECT ACCEPTED (separately; not rerun). M01–M183: not executed. The accepted healthy witnesses and unchanged-core configurations: linked, never rerun.
- Build rule (mission section 6): every control one fresh build with zero warnings and zero errors under the unchanged general classifier; no warning exception.
- AD-1-R4 G.3 (C1 and C25 only): the type-(a) primary's first failure is recorded as printed (predicted E3 for C1 and E4 for C25; E3 is never counted as P1, nor E4 as P7); the named property is established only by the type-(c) mutant witness on the same pinned mutant executable; the accepted healthy type-(e3) witness and the accepted F-1 type-(e1) receipt are linked after restoration. INCOMPLETE kills nothing.
- Not claimed: no accepted control, no D2A-1 acceptance, no Phase Three acceptance, no integration.

## Controls (mission order)

### C1 — `receipts/81-C1`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt types: (a) original-explorer mutant FAIL, the G.3(a) primary (recorded as printed) + (c) named-property mutant witness, G.3(b), on the same pinned mutant executable; restored (e1) + healthy witness (e3), G.3(d); configuration F-1 (`ledger::protocol::model_tests::full_f1`, original explorer); named property P1 (attributed only by the witness); predicted first failure E3; ceiling 60,000,000 states
- frozen patch sha256 `5ecd716a8f123d61034b0d274238898e13a116a9cc464c7645a9736934970252`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `7d95dd425fcdf17b12dda2fdce461f97ba7b9a66` (sha256 `eb3bf0adfcab9711be1bfc5f0d608a58ae97dcb65a7ca1ca1d66023347522658`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 48s']]
- build resources: wall 168.27 s / 900 s; scope peak 2495.8 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1966732 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mtw-C1/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `f89bf7ba397c73b37e43bc6aae8851fd2ff88e5928a8ecdd69e967956d21f23c` size 8,987,200
- primary (type (a)), exact selection: `ledger::protocol::model_tests::full_f1 --exact --ignored --nocapture --test-threads=1`
- primary raw outcome (as printed): outcome FAIL; FAIL E3: live 2, unapplied 1; armed 0, counted 2, dropped 0;   shortest trace (4 steps):; exit 101, signal None
- primary first printed property (recorded as printed; not counted as P1): E3
- primary raw state counts: 5,794 discovered (expanded 1,553) of 60,000,000
- primary classification: CLASSIFICATION: G.3(a) PRIMARY — ADMISSIBLE TYPE-(a) FAIL, RECORDED AS PRINTED (A PART ONLY; NOT A KILL BY ITSELF) — FAIL — admissible type-(a) original-explorer mutant FAIL; first failure E3 recorded as printed (AD-1-R4 G.3(a) primary of C1); E3 is not counted as P1; P1's attribution comes only from the type-(c) witness (G.3(b))
- primary resources: wall 0.02 s / 2700 s; scope peak 4.1 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 6904 KiB
- mutant witness (type (c)): `ledger::protocol::model_tests::c1_witness_p1_at_a_successful_admission --exact --nocapture --test-threads=1` on the same pinned mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mtw-C1/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `f89bf7ba397c73b37e43bc6aae8851fd2ff88e5928a8ecdd69e967956d21f23c` size 8,987,200 (before and after)
- mutant witness raw lines (as printed): `C1 WITNESS F-1: 11 steps, each enabled and taking its choices; 9 violations`; `NAMED PROPERTY P1: OBSERVED at step 11 (admission: admitted)`; `OTHER PROPERTIES VIOLATED, each reported separately and not counted as P1: ["E3"]`
- mutant witness process: exit 101, signal None; harness ['test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s']
- mutant witness classification: CLASSIFICATION: G.3(b) MUTANT WITNESS — ADMISSIBLE TYPE-(c) RECEIPT (A PART ONLY) — FAIL — admissible type-(c) named-property mutant-witness receipt (G.3(b)): 'NAMED PROPERTY P1: OBSERVED at step 11 (admission: admitted)'; C1 WITNESS F-1: 11 steps, each enabled and taking its choices; 9 violations; E3 reported separately, not counted as P1
- mutant witness resources: wall 0.0 s / 2700 s; scope peak 3.8 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 5692 KiB
- restoration: RESTORE: EXACTLY_C
- healthy witness (type (e3), linked): HEALTHY WITNESS LINK: VERIFIED — control C1: healthy-witness receipt docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/50-witness-C1 at 89326b859792a442c1853c3d35d30a48301c009e (accepted healthy-witness type-(e3) PASS on the pinned unchanged-baseline executable cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c)
- unchanged-core receipt (type (e1), linked): LINK: VERIFIED — control C1: restored-side receipt docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-1 unchanged-core original-explorer PASS, type (e1))
- G.3: G.3: ALL OBLIGATIONS HOLD
- (e) recording, PROPOSED FOR ARCHITECT REVIEW (not an acceptance): 'C1: P1 (first failure E3; witness)'

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mtw-C1/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 f89bf7ba397c73b37e43bc6aae8851fd2ff88e5928a8ecdd69e967956d21f23c size 8987200
  - primary (type (a), G.3(a)): CLASSIFICATION: G.3(a) PRIMARY — ADMISSIBLE TYPE-(a) FAIL, RECORDED AS PRINTED (A PART ONLY; NOT A KILL BY ITSELF); outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL; first failure E3 recorded as printed (AD-1-R4 G.3(a) primary of C1); E3 is not counted as P1; P1's attribution comes only from the type-(c) witness (G.3(b))
  - mutant witness (type (c), G.3(b)): CLASSIFICATION: G.3(b) MUTANT WITNESS — ADMISSIBLE TYPE-(c) RECEIPT (A PART ONLY); outer class (AD-1-R4 K.2): FAIL — admissible type-(c) named-property mutant-witness receipt (G.3(b)): 'NAMED PROPERTY P1: OBSERVED at step 11 (admission: admitted)'; C1 WITNESS F-1: 11 steps, each enabled and taking its choices; 9 violations; E3 reported separately, not counted as P1
  - restoration: RESTORE: EXACTLY_C
  - linked healthy witness (type (e3), G.3(d)): HEALTHY WITNESS LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/50-witness-C1 at 89326b859792a442c1853c3d35d30a48301c009e (accepted healthy-witness type-(e3) PASS on the pinned unchanged-baseline executable cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c)
  - linked restored-side receipt (type (e1)): LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-1 unchanged-core original-explorer PASS, type (e1))
  - G.3: G.3: ALL OBLIGATIONS HOLD
  - G.3(e) recording, proposed for Architect review (not an acceptance): 'C1: P1 (first failure E3; witness)'

### C25 — `receipts/82-C25`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt types: (a) original-explorer mutant FAIL, the G.3(a) primary (recorded as printed) + (c) named-property mutant witness, G.3(b), on the same pinned mutant executable; restored (e1) + healthy witness (e3), G.3(d); configuration F-1 (`ledger::protocol::model_tests::full_f1`, original explorer); named property P7 (attributed only by the witness); predicted first failure E4; ceiling 60,000,000 states
- frozen patch sha256 `29a2c1e571b4f7fd278d106fe3d40a8cf912119dbe44fbbbbd92855431ecd732`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `0456a0c1c1fe40f71593edc6ba5938a3b0c29900` (sha256 `27c1b9f30f1e753675dc36417b23d4efcbb9c0309f89204ea21326d3601d9dec`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 47s']]
- build resources: wall 167.13 s / 900 s; scope peak 2492.0 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1969468 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mtw-C25/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `a0d6fc9d870b09d8bf4a17f1867d4a7bbae64ce21961d6133526c4e84d0ec315` size 8,984,464
- primary (type (a)), exact selection: `ledger::protocol::model_tests::full_f1 --exact --ignored --nocapture --test-threads=1`
- primary raw outcome (as printed): outcome FAIL; FAIL E4: append_gating said Committed but the record is absent;   shortest trace (1 steps):; exit 101, signal None
- primary first printed property (recorded as printed; not counted as P7): E4
- primary raw state counts: 4 discovered (expanded 0) of 60,000,000
- primary classification: CLASSIFICATION: G.3(a) PRIMARY — ADMISSIBLE TYPE-(a) FAIL, RECORDED AS PRINTED (A PART ONLY; NOT A KILL BY ITSELF) — FAIL — admissible type-(a) original-explorer mutant FAIL; first failure E4 recorded as printed (AD-1-R4 G.3(a) primary of C25); E4 is not counted as P7; P7's attribution comes only from the type-(c) witness (G.3(b))
- primary resources: wall 0.0 s / 2700 s; scope peak 3.3 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 6040 KiB
- mutant witness (type (c)): `ledger::protocol::model_tests::c25_witness_p7_on_the_resulting_world --exact --nocapture --test-threads=1` on the same pinned mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mtw-C25/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `a0d6fc9d870b09d8bf4a17f1867d4a7bbae64ce21961d6133526c4e84d0ec315` size 8,984,464 (before and after)
- mutant witness raw lines (as printed): `C25 WITNESS F-1: 1 steps, each enabled and taking its choices; 2 violations`; `NAMED PROPERTY P7: OBSERVED on the world after step 1 (append_gating: Committed (inserted))`; `OTHER PROPERTIES VIOLATED, each reported separately and not counted as P7: ["E4"]`
- mutant witness process: exit 101, signal None; harness ['test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s']
- mutant witness classification: CLASSIFICATION: G.3(b) MUTANT WITNESS — ADMISSIBLE TYPE-(c) RECEIPT (A PART ONLY) — FAIL — admissible type-(c) named-property mutant-witness receipt (G.3(b)): 'NAMED PROPERTY P7: OBSERVED on the world after step 1 (append_gating: Committed (inserted))'; C25 WITNESS F-1: 1 steps, each enabled and taking its choices; 2 violations; E4 reported separately, not counted as P7
- mutant witness resources: wall 0.0 s / 2700 s; scope peak 3.0 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 5384 KiB
- restoration: RESTORE: EXACTLY_C
- healthy witness (type (e3), linked): HEALTHY WITNESS LINK: VERIFIED — control C25: healthy-witness receipt docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/51-witness-C25 at 89326b859792a442c1853c3d35d30a48301c009e (accepted healthy-witness type-(e3) PASS on the pinned unchanged-baseline executable cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c)
- unchanged-core receipt (type (e1), linked): LINK: VERIFIED — control C25: restored-side receipt docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-1 unchanged-core original-explorer PASS, type (e1))
- G.3: G.3: ALL OBLIGATIONS HOLD
- (e) recording, PROPOSED FOR ARCHITECT REVIEW (not an acceptance): 'C25: P7 (first failure E4; witness)'

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mtw-C25/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 a0d6fc9d870b09d8bf4a17f1867d4a7bbae64ce21961d6133526c4e84d0ec315 size 8984464
  - primary (type (a), G.3(a)): CLASSIFICATION: G.3(a) PRIMARY — ADMISSIBLE TYPE-(a) FAIL, RECORDED AS PRINTED (A PART ONLY; NOT A KILL BY ITSELF); outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL; first failure E4 recorded as printed (AD-1-R4 G.3(a) primary of C25); E4 is not counted as P7; P7's attribution comes only from the type-(c) witness (G.3(b))
  - mutant witness (type (c), G.3(b)): CLASSIFICATION: G.3(b) MUTANT WITNESS — ADMISSIBLE TYPE-(c) RECEIPT (A PART ONLY); outer class (AD-1-R4 K.2): FAIL — admissible type-(c) named-property mutant-witness receipt (G.3(b)): 'NAMED PROPERTY P7: OBSERVED on the world after step 1 (append_gating: Committed (inserted))'; C25 WITNESS F-1: 1 steps, each enabled and taking its choices; 2 violations; E4 reported separately, not counted as P7
  - restoration: RESTORE: EXACTLY_C
  - linked healthy witness (type (e3), G.3(d)): HEALTHY WITNESS LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/51-witness-C25 at 89326b859792a442c1853c3d35d30a48301c009e (accepted healthy-witness type-(e3) PASS on the pinned unchanged-baseline executable cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c)
  - linked restored-side receipt (type (e1)): LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd (accepted exact-final F-1 unchanged-core original-explorer PASS, type (e1))
  - G.3: G.3: ALL OBLIGATIONS HOLD
  - G.3(e) recording, proposed for Architect review (not an acceptance): 'C25: P7 (first failure E4; witness)'
