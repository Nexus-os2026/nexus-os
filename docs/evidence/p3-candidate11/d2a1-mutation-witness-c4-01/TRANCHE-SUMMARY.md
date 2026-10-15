# C4 witness-bound mutation summary — P3-C11-DR01-D2A1-MUTATION-WITNESS-C4-01

Derived status: **KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING** (from the receipts).

- Authorized (mission sections 1, 4-14): C4 alone, once, under AD-1-R4 G.5 and G.6.
- Attempted: C4.
- New executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`): **1 of 1** (C4). None is accepted here: acceptance belongs to the Sole Architect.
- Stopped (first unsuccessful control; nothing later was attempted): none.
- Authorized but not attempted: none.

## Campaign status before Architect review

- Architect-accepted kills: **33 of 35** (Tranche 1: C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b; Tranche 2: C23, C10, C16, C26, C3a; Tranche 2 continuation: C5, A3, A6, C8, C15; Tranche 3: C2; Tranche 3 continuation: C24, C3b, C3c, C6; Tranche 3 tail: A1, A2, C20, A5; witness-bound: C1, C25; record `documents/ARCHITECT-RULING-C1-C25-WITNESS-ACCEPTANCE.md`), unchanged by this mission.
- Executor candidates pending Architect review from this mission: 1 (C4): 33 accepted + 1 pending Architect review. Only the Architect may rule a higher accepted count (not claimed).
- MUTATION_CONTROL_BLOCKED (Architect ruling on `c10681b3c65dd377b5a13d08140427e8b34441d0`): C7 — E11 observed, P6 not established; not executed here.
- Reserved: none. A4: SATISFIED — ARCHITECT ACCEPTED (separately; not rerun). M01–M183: not executed. The accepted healthy witness and the unchanged-core configurations: linked, never rerun.
- Build rule (mission section 10): one fresh build with zero warnings and zero errors under the unchanged general classifier; no warning exception; no second build.
- AD-1-R4 G.5 and G.6 (C4 only): the type-(a) primary's first property is recorded as printed and never relabelled (P4 or P8 admissible; P4 the prediction, not a receipt; neither inferred from the other); P4 AND P8 are established only by the type-(c) mutant witness on the same pinned mutant executable, each at its frozen locus, with G.6 verified from the raw output at every step of both scenarios; the accepted healthy type-(e3) witness and the accepted F-8 type-(e1) receipt are linked after restoration. INCOMPLETE kills nothing.
- Not claimed: no accepted control, no D2A-1 acceptance, no Phase Three acceptance, no integration.

## Controls (mission order)

### C4 — `receipts/81-C4`

- result: **CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**
- gate: GATE: OPEN
- receipt types: (a) original-explorer mutant FAIL, the G.5(A) primary (recorded as printed) + (c) named-property mutant witness, G.5(B) and G.6, on the same pinned mutant executable; restored (e1), G.5(D) + healthy witness (e3), G.5(C); configuration F-8 (`ledger::protocol::model_tests::full_f8`, original explorer); named properties P4, P8 (both attributed only by the witness); admissible first property of the primary P4 or P8 (the prediction P4 is not a receipt); ceiling 30,000,000 states
- frozen patch sha256 `24ac7add3716a030dd710539e6082e197fdc5bd624c26a595b378b3971e0838d`; mutated file `crates/nexus-governed-control/src/ledger/protocol.rs`; mutant blob `f3793c2ff61f8adbf43fe17d6b5237b49bc507b0` (sha256 `4037de6e33a901f933f118dde5a395b70b1e5cd7e58e54dcf0078d4a40f335b2`; computed in-process, never written to any object store)
- apply: APPLY: EXACTLY_C_PLUS_PATCH
- build: BUILD: PINNED — mutant build succeeded; one library-test executable pinned; exit 0, signal None; Finished [['release', 'optimized', '2m 46s']]
- build resources: wall 166.97 s / 900 s; scope peak 2494.1 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 1964876 KiB
- mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mwc4-C4/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `38be24f405dbb56fe4f3e26baad35a6b892d6fc47b5200ddae1e388e02da6db8` size 8,988,320
- primary (type (a)), exact selection: `ledger::protocol::model_tests::full_f8 --exact --ignored --nocapture --test-threads=1`
- primary raw outcome (as printed): outcome FAIL; FAIL P4: U is occupied together with C or X;   shortest trace (8 steps):; exit 101, signal None
- primary first printed property (recorded as printed; never relabelled; it alone establishes neither P4 nor P8): P4
- primary raw state counts: 51,078 discovered (expanded 28,579) of 30,000,000
- primary classification: CLASSIFICATION: G.5(A) PRIMARY — ADMISSIBLE TYPE-(a) FAIL, RECORDED AS PRINTED (A PART ONLY; NOT A KILL BY ITSELF) — FAIL — admissible type-(a) original-explorer mutant FAIL; first failure P4 recorded as printed (AD-1-R4 G.5(A) primary of C4); the primary does not establish P4 and P8: both are attributed only by the type-(c) witness (G.5(B))
- primary resources: wall 0.19 s / 2700 s; scope peak 8.0 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 11516 KiB
- mutant witness (type (c)): `ledger::protocol::model_tests::c4_witness_p4_and_p8_on_f8 --exact --nocapture --test-threads=1` on the same pinned mutant executable: `/home/nexus/NEXUS/nexus-os-p3-c11-mwc4-C4/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `38be24f405dbb56fe4f3e26baad35a6b892d6fc47b5200ddae1e388e02da6db8` size 8,988,320 (before and after)
- mutant witness raw lines (as printed): `C4 WITNESS F-8: 2 scenarios, every step enabled and consuming its choices as planned; 2 violations`; `NAMED PROPERTY P4: OBSERVED on the world after step 8 of scenario P4 (append_acknowledgement: NotApplied(Uncertain); U holds the acknowledgement while X holds F1's explanation)`; `OTHER PROPERTIES VIOLATED in scenario P4, each reported separately and not counted as P4: []`; `NAMED PROPERTY P8: OBSERVED on the world after step 12 of scenario P8 (append_acknowledgement: Committed; G1's written attempt, not told Committed, is neither in U nor corrected in C or in the ledger)`; `OTHER PROPERTIES VIOLATED in scenario P8, each reported separately and not counted as P8: []`
- mutant witness process: exit 101, signal None; harness ['test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s']
- mutant witness classification: CLASSIFICATION: G.5(B) MUTANT WITNESS — ADMISSIBLE TYPE-(c) RECEIPT: P4 AND P8 AT THEIR FROZEN LOCI; G.6 HOLDS (A PART ONLY) — FAIL — admissible type-(c) named-property mutant-witness receipt (G.5(B)): both 'NAMED PROPERTY P4: OBSERVED on the world after step 8 of scenario P4 (append_acknowledgement: NotApplied(Uncertain); U holds the acknowledgement while X holds F1's explanation)' and 'NAMED PROPERTY P8: OBSERVED on the world after step 12 of scenario P8 (append_acknowledgement: Committed; G1's written attempt, not told Committed, is neither in U nor corrected in C or in the ledger)'; C4 WITNESS F-8: 2 scenarios, every step enabled and consuming its choices as planned; 2 violations; G.6 verified from the raw output in both scenarios
- mutant witness resources: wall 0.0 s / 2700 s; scope peak 3.6 MiB / 16 GiB; swap peak 0 B; oom_kill 0; largest process max RSS 5556 KiB
- G.6 at each changed append (from the raw output): scenario P4 step 8/8 Ack(0): offered [0, 0, 0, 1, 0, 0], consumed [(0, 4), (0, 4), (0, 4), (1, 3), (0, 2), (0, 3)], unused [], faults left 1 [fault]; branches ['append_acknowledgement: NotApplied(Uncertain)']; violations [('after', 'P4')]; scenario P8 step 12/12 Ack(0): offered [0, 0, 0, 0], consumed [(0, 4), (0, 4), (0, 4), (0, 3)], unused [], faults left 1; branches ['append_acknowledgement: Committed']; violations [('after', 'P8')]; G.6 problems []
- restoration: RESTORE: EXACTLY_C; post-restoration authentication: RESTORE AUTH: OK
- healthy witness (type (e3), linked): HEALTHY WITNESS LINK: VERIFIED — control C4: healthy-witness receipt docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/52-witness-C4 at 89326b859792a442c1853c3d35d30a48301c009e (accepted healthy-witness type-(e3) PASS on the pinned unchanged-baseline executable cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c)
- unchanged-core receipt (type (e1), linked): LINK: VERIFIED — control C4: restored-side receipt docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/64-full-F-8 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-8 unchanged-core original-explorer PASS, type (e1))
- G.5: G.5: (A) (B) (C) (D) HOLD; (E) IDENTITY AND RESTORATION HOLD; (E) ARCHITECT REVIEW PENDING
- (E) the Sole Architect's independent review: PENDING (never claimed by the executor)
- (F) recording, PROPOSED FOR ARCHITECT REVIEW (not an acceptance): 'C4: P4, P8 (first failure P4; witness)'

  RESULT facts:

  - apply: APPLY: EXACTLY_C_PLUS_PATCH
  - build: BUILD: PINNED
  - pinned mutant executable: /home/nexus/NEXUS/nexus-os-p3-c11-mwc4-C4/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 38be24f405dbb56fe4f3e26baad35a6b892d6fc47b5200ddae1e388e02da6db8 size 8988320
  - primary (type (a), G.5(A)): CLASSIFICATION: G.5(A) PRIMARY — ADMISSIBLE TYPE-(a) FAIL, RECORDED AS PRINTED (A PART ONLY; NOT A KILL BY ITSELF); outer class (AD-1-R4 K.2): FAIL — admissible type-(a) original-explorer mutant FAIL; first failure P4 recorded as printed (AD-1-R4 G.5(A) primary of C4); the primary does not establish P4 and P8: both are attributed only by the type-(c) witness (G.5(B))
  - mutant witness (type (c), G.5(B), G.6): CLASSIFICATION: G.5(B) MUTANT WITNESS — ADMISSIBLE TYPE-(c) RECEIPT: P4 AND P8 AT THEIR FROZEN LOCI; G.6 HOLDS (A PART ONLY); outer class (AD-1-R4 K.2): FAIL — admissible type-(c) named-property mutant-witness receipt (G.5(B)): both 'NAMED PROPERTY P4: OBSERVED on the world after step 8 of scenario P4 (append_acknowledgement: NotApplied(Uncertain); U holds the acknowledgement while X holds F1's explanation)' and 'NAMED PROPERTY P8: OBSERVED on the world after step 12 of scenario P8 (append_acknowledgement: Committed; G1's written attempt, not told Committed, is neither in U nor corrected in C or in the ledger)'; C4 WITNESS F-8: 2 scenarios, every step enabled and consuming its choices as planned; 2 violations; G.6 verified from the raw output in both scenarios
  - restoration: RESTORE: EXACTLY_C
  - post-restoration authentication: RESTORE AUTH: OK
  - linked healthy witness (type (e3), G.5(C), G.6): HEALTHY WITNESS LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/52-witness-C4 at 89326b859792a442c1853c3d35d30a48301c009e (accepted healthy-witness type-(e3) PASS on the pinned unchanged-baseline executable cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c)
  - linked restored-side receipt (type (e1), G.5(D)): LINK: VERIFIED; docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/64-full-F-8 at 89326b859792a442c1853c3d35d30a48301c009e (accepted exact-final F-8 unchanged-core original-explorer PASS, type (e1))
  - G.5: G.5: (A) (B) (C) (D) HOLD; (E) IDENTITY AND RESTORATION HOLD; (E) ARCHITECT REVIEW PENDING
  - G.5(F) recording, proposed for Architect review (not an acceptance): 'C4: P4, P8 (first failure P4; witness)'
  - G.5(E): the Sole Architect's independent review is PENDING (never claimed by the executor)
