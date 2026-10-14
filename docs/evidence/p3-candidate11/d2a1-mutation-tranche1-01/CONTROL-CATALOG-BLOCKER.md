# D2A-1 control catalog — CONTROL_CATALOG_BLOCKED

Mission P3-C11-DR01-D2A1-MUTATION-TRANCHE1-01. Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. For the Sole Architect's review. **Evidence only.**

Catalog `catalog/catalog.json`, version `d2a1-controls-v1`, sha256 `a0cdef7976bbb1715a7da25ad2900c978a32c6b3d9029b46f16872eda66b4747`: **CONTROL_CATALOG_BLOCKED**, **not frozen**. Kill-required controls: 35; A4 recorded separately as the accepted comparison. Definitions READY: 14. Definitions BLOCKED: 21.

## Why the catalog is blocked

Mission section 7: "Stop if any of the 35 definitions remains ambiguous. Return CONTROL_CATALOG_BLOCKED rather than inventing a definition." Section 8: no Rust build is authorised before the static catalog gate passes, and the gate freezes all 35 patches.

The authorised definition sources were each authenticated (`receipts/00-authentication.txt`). Between them they give an exact change — an anchor and its replacement, or a patch — for 14 of the 35 controls only:

| Source | Exact definitions it gives |
|---|---|
| Contract P3-C11-D2A1-CONTRACT-02-R1, 7.5 (`cac65351…3b3c`) | none: one line of prose per control, its named property and its configuration |
| AD-1-R4 G.2 (`00ced6d5…b693`), with the documentary patches it cites | C1, C25 (narrow), C4, C9 (C9-b); C10 and C24 retained verbatim from the sealed file |
| AD-1-R5 (`4e1ed8c9…1bb91473`) | none: C15's explorer, property, ceiling and restored side, not its text |
| Sealed historical `controls_defs.py` (`ef0420a1…a108f`) | C1, C2, C10, C24, C12, C14, C19, C21, C22, C27, C28; its compound C25 is superseded by G.2's narrow C25. Its own sealed README says: "controls_defs.py holds only a partial set of definitions" |

The other 21 controls — C3a, C3b, C3c, C5, C6, C7, C8, C11, C15, C16, C17, C18a, C18b, C20, C23, C26, A1, A2, A3, A5, A6 — have no exact definition in any authorised source. Writing one would be the executor authoring a kill-required control's mutation, which the mission forbids. Four of them are tranche-1 controls: C11, C17, C18a, C18b.

Because the gate cannot pass, nothing was built or run: no baseline exact receipt (mission 10), no mutant worktree, no mutant build, no mutant test. This includes the eight tranche-1 controls whose definitions are READY (C9, C12, C14, C19, C21, C22, C27, C28): mission 8 makes the gate a precondition of any build, and mission 19 says that if the catalog is blocked no baseline test and no mutant is executed.

## The 21 blocked controls

Each row: the contract's change, its named property or test and its configuration, and the executor's observations — where the change would land in Candidate C, and what an exact definition must settle. The observations are **not definitions**, carry no patch, and are not Architect-reviewed.

### C3a — acknowledgement test 2 removed

- Contract 7.5 line 701; fails **P13 or P15**; on **F-7** (`ledger::protocol::model_tests::full_f7`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 1664 (1 occurrence(s)): `let refusal = if set.frontier != now.frontier {`
- Open:
  - Publish::acknowledgement carries source markers only for tests 0 and 1 (`// Test 0.`, `// Test 1.`); nothing in the source binds contract 4.8 test 2 to a code clause. The chain after `recheck` has four clauses (Stale, Empty, Future, Set) in the contract's order, but the binding is an executor reading.
  - Test 2 is the chain's head clause; removing it means rewriting `let refusal = if ...` so that the chain starts at test 3. No authorised source gives that text.

### C3b — acknowledgement test 4 removed

- Contract 7.5 line 702; fails **P13 or P12**; on **F-6** (`ledger::protocol::model_tests::full_f6`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 1668 (1 occurrence(s)): `} else if set.top > set.head || set.head > now.head {`
- Open:
  - As C3a, the binding of contract test 4 to this clause is not marked in the source.
  - Contract test 4 is two comparisons (N at most the snapshot's head; that head at most the head now). Whether 'test 4 removed' removes the whole clause or one comparison is not stated in an authorised text.

### C3c — acknowledgement test 5 removed

- Contract 7.5 line 703; fails **P12**; on **F-6** (`ledger::protocol::model_tests::full_f6`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 1670 (1 occurrence(s)): `} else if now.count != set.count || now.digest != set.digest {`
- Open:
  - As C3a, the binding of contract test 5 to this clause is not marked in the source.
  - No authorised source gives the resulting text.

### C5 — resolve, for an acknowledgement in U, raises the frontier with neither the presence read nor the frontier check

- Contract 7.5 line 705; fails **P13**; on **F-5** (`ledger::protocol::model_tests::full_f5`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 1818 (1 occurrence(s)): `fn resolve_ack(&mut self, set: &SetFacts, nonce: &[u8; 16], io: &mut impl Reads) -> Resolve {`
  - `protocol.rs` line(s) 1825 (1 occurrence(s)): `Read::Got(Some(_)) => match io.frontier() {`
- Open:
  - 'Raises the frontier with neither the presence read nor the frontier check': whether the verbs `acknowledgement` and `frontier` are still issued (the explorer counts verbs, E2 and E5) or removed.
  - What resolve then answers (Acknowledged with N, or another variant), and whether the Absent, Failed and Corrupt arms remain reachable.
  - Several encodings with different observable verb sequences fit the one-line text.

### C6 — classification row b ignores the refusal table, **with** A4

- Contract 7.5 line 706; fails **P6**; on **F-PT** (`ledger::protocol::model_tests::full_fpt`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 1162 (1 occurrence(s)): `if let Some(cause) = self.refusals.get(key) {`
  - `protocol.rs` line(s) 1197 (1 occurrence(s)): `if let Some(known) = self.known(&member.key, facts.explained) {`
- Open:
  - Classification row b is evaluated through `Core::known`, which also serves lookup (4.6) and is read by the model itself (`step_gc` calls `w.core.known`). Whether the change is confined to classification (the `classify` call site) or made in `known` for every caller is not stated.
  - 'Ignores the refusal table' leaves X's explanation and the ledger's explanation in row b; the exact text is not given by an authorised source.

### C7 — *Explained* removes every table entry, **with** A4

- Contract 7.5 line 707; fails **P6**; on **F-PT** (`ledger::protocol::model_tests::full_fpt`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 1559 (1 occurrence(s)): `Read::Got(Some(seq)) => {`
- Open:
  - 'Explained removes every table entry': whether only explain's Explained path changes, or also the other removal in `settle` (X's own append committing).
  - The form (clear the whole table, or remove every entry but keep the call) is not given.

### C8 — members are classified and inserted one at a time

- Contract 7.5 line 708; fails **E5**; on **F-B** (`ledger::protocol::model_tests::full_fb`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 1348 (1 occurrence(s)): `// Step 3: every member classified before any insert.`
  - `protocol.rs` line(s) 1383 (1 occurrence(s)): `// Step 5.`
- Open:
  - 'Members are classified and inserted one at a time' needs steps 3 to 5 of `Publish::facts` merged into one loop. Step 4's no-insert path, the rollback and persistence when a later member is refused after an earlier insert, and the `issued` / `conflict` bookkeeping are each unspecified.
  - This is a restructuring, not a token change; any text would be executor-authored.

### C11 — a constraint failure during the inserts gives Not written whatever the rollback said. **This is the amendment's rule, which the review rejected.**

- Contract 7.5 line 711; fails **E4**; on **F-B2** (`ledger::protocol::model_tests::full_fb2`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 1391 (1 occurrence(s)): `Insert::Constraint => {`
  - `protocol.rs` line(s) 1479 (1 occurrence(s)): `Insert::Constraint => {`
  - `protocol.rs` line(s) 1683, 1909 (2 occurrence(s)): `if failed == Insert::Constraint {`
- Open:
  - Scope: the fact append's step 5 only (the first site), or every append with an insert constraint path: the gating append runs steps 2 to 6 of 4.4 for one member (the second site, the same code at another indentation), and the acknowledgement append and the cell appends have their own `Insert::Constraint` tests (the third site). The contract's 'during the inserts' does not settle it.
  - Whether the one rollback is still issued (E5 counts it) and only its answer is ignored.
  - No authorised source gives the resulting text.

### C15 — a cell operation ignores the occupancy number

- Contract 7.5 line 713; fails **E8**; on **F-R** (`ledger::protocol::model_tests::full_fr`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 1115 (1 occurrence(s)): `&& self.occupancy(cell) == Some(token.occupancy)`
- Open:
  - 'Ignores the occupancy number': whether the cell must still be occupied (`self.occupancy(cell).is_some()`) or the clause is dropped entirely; the two differ for a token naming an empty cell.
  - `Core::names` serves resolve, the cell appends and empty; whether every cell operation is changed (the contract says 'a cell operation').
  - AD-1-R5 F fixes C15's configuration, explorer, ceiling and restored side, not its text.

### C16 — the drop of a Counted ticket lowers the count

- Contract 7.5 line 714; fails **E3**; on **F-D** (`ledger::protocol::model_tests::full_fd`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 397 (1 occurrence(s)): `TicketState::Counted(_) => {`
- Open:
  - 'The drop of a Counted ticket lowers the count': which count (`unapplied`, `live`, or both), and whether the handover-fault bit is still set.
  - No authorised source gives the resulting text.

### C17 — the guard reports Not written after an insert was issued

- Contract 7.5 line 715; fails **E4**; on **F-B2** (`ledger::protocol::model_tests::full_fb2`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 1972 (1 occurrence(s)): `let persistence = if self.issued && self.conflict.is_none() {`
- Open:
  - 'The guard reports Not written after an insert was issued': whether only this persistence changes, and whether `self.keep = self.issued;` (which keeps a gating or acknowledgement occupant in U on unwind) is left unchanged.
  - No authorised source gives the resulting text.

### C18a — a refused gating append leaves U occupied

- Contract 7.5 line 716; fails **E7**; on **F-G** (`ledger::protocol::model_tests::full_fg`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 1985 (1 occurrence(s)): `if !self.keep {`
  - `protocol.rs` line(s) 1413 (1 occurrence(s)): `fn gating(&mut self, record: &GatingRecord, io: &mut impl Txn) -> GatingOutcome {`
- Open:
  - 'A refused gating append leaves U occupied': every refusal after U is placed (open failure, read failure, classification refusal, insert failure with Not written), or only the refusal E7 names (a conflict with a new cause).
  - Site: the gating operation's return paths (setting `keep`), or the guard's emptying of U.
  - Which exit mark the kept occupant carries (the guard writes ReturnedUncertain when `keep` is set).

### C18b — a gating conflict makes a table entry

- Contract 7.5 line 717; fails **E7**; on **F-G** (`ledger::protocol::model_tests::full_fg`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 1211 (1 occurrence(s)): `if gating {`
- Open:
  - 'A gating conflict makes a table entry': a direct insert into the table, or falling through to row d's fact path (which also applies the table-full check and its latch).
  - Whether X's explanation is still built.
  - No authorised source gives the resulting text.

### C20 — an explanation found already present is not marked committed

- Contract 7.5 line 718; fails **L1**; on **F-2** (`ledger::protocol::model_tests::full_f2`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 1893 (1 occurrence(s)): `Read::Got(Some(seq)) => {`
- Open:
  - 'An explanation found already present is not marked committed': whether only X's mark is withheld (C's present path unchanged), whether `settle`'s table-entry removal still happens, and what the operation answers.
  - No authorised source gives the resulting text.

### C23 — the publication omits the X bit

- Contract 7.5 line 719; fails **E1**; on **F-2** (`ledger::protocol::model_tests::full_f2`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 1085 (1 occurrence(s)): `| bit(self.x.is_some(), X_OCCUPIED)`
- Open:
  - A single evident site, but no authorised source gives the resulting text (removing the line leaves the expression ending at C's bit); the executor may not supply it.

### C26 — a passed barrier clears the latch

- Contract 7.5 line 722; fails **E9**; on **F-4** (`ledger::protocol::model_tests::full_f4`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `protocol.rs` line(s) 1720 (1 occurrence(s)): `if !io.run() {`
- Open:
  - 'A passed barrier clears the latch': where the clearing is inserted (before or after the phase assignment) and its exact text. No authorised source gives it.

### A1 — the writer confirms before it records

- Contract 7.5 line 739; fails **P1**; on **F-1** (`ledger::protocol::model_tests::full_f1`, ceiling 60,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `model_tests.rs` line(s) 2563 (1 occurrence(s)): `FwPos::Got(held) => {`
  - `model_tests.rs` line(s) 2605 (1 occurrence(s)): `FwPos::Recorded => {`
- Open:
  - The first writer records its classes in the `FwPos::Got` step and confirms in `FwPos::Recorded`. 'Confirms before it records' needs the two steps reordered, including the unwind choices ('unwinds before recording', 'unwinds before confirming') and the DROP choice, none of which an authorised source specifies.
  - Whether settlement's own record-then-confirm (SE) is also 'the writer' is not stated.

### A2 — admission is split into two steps under two separate holds of the test's lock. Step one takes a `view` and keeps only its low 16 bits: phase, session, latch, cells, handover fault. Any actor may step in between. Step two takes a second `view`, keeps only `unapplied`, reads the debt count, H and A, and decides from the kept bits of step one and the values of step two.

- Contract 7.5 line 740; fails **P1**; on **F-1** (`ledger::protocol::model_tests::full_f1`, ceiling 60,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `model_tests.rs` line(s) 3250 (1 occurrence(s)): `fn step_adm(config: &Config, w: &mut World, obs: &mut Obs) -> bool {`
  - `model_tests.rs` line(s) 1057 (1 occurrence(s)): `/// Actor change A2 only: the bits kept by the first step.`
- Open:
  - The contract states A2's semantics (two steps, two holds, the kept low 16 bits). The world already has a key field for the kept bits (`adm_kept`), but the two-step actor code is given by no authorised source; writing it is authoring a new actor program.

### A3 — the worker empties X before it raises H

- Contract 7.5 line 741; fails **P12**; on **F-5** (`ledger::protocol::model_tests::full_f5`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `model_tests.rs` line(s) 2989 (1 occurrence(s)): `if let Action::EmptyX(_, seq) = &action {`
  - `model_tests.rs` line(s) 995 (1 occurrence(s)): `/// Under actor change A3 only: H raised after X was emptied.`
- Open:
  - The world already has `WkPos::RaiseH` ('Under actor change A3 only'), but the transitions into it (stop raising H in `Seen`, go to `RaiseH` after `Act(EmptyX)`) are given by no authorised source.

### A5 — settlement claims a Conflict obligation while the store is not Healthy

- Contract 7.5 line 743; fails **L1**; on **F-2** (`ledger::protocol::model_tests::full_f2`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `model_tests.rs` line(s) 2648 (1 occurrence(s)): `Class::Conflict if view.phase() == Phase::Healthy && !view.x_occupied() => {`
- Open:
  - 'Settlement claims a Conflict obligation while the store is not Healthy': whether only the Healthy conjunct is dropped from the guard, and the exact text. No authorised source gives it.

### A6 — "recovery required" is a flag, not a comparison of H with A

- Contract 7.5 line 744; fails **P12 or P15**; on **F-5** (`ledger::protocol::model_tests::full_f5`, ceiling 30,000,000 states).
- Candidate sites at C (location only, not anchors):
  - `model_tests.rs` line(s) 3256 (1 occurrence(s)): `let admitted = view.admits() && w.table.debt == 0 && w.table.h <= w.table.a;`
- Open:
  - 'Recovery required is a flag, not a comparison of H with A' needs a new world field (part of the state key, 7.1) and rules for setting and clearing it. None is given by an authorised source.

## The 14 controls whose definitions are READY (not frozen, not applied)

Each passed every static check of `receipts/20-static-gate.txt`: anchor once in C's blob; the patch applies with GNU patch and with `git apply` to a disposable byte copy, gives the anchor replacement, changes only the anchor's span of `protocol.rs`, and reverses to blob `a2df2841…`; patches and mutant blobs pairwise distinct; named test present at C and in the pinned executable's listing; restored-evidence links resolve.

| Id | Origin | Anchor line at C | Patch sha256 | Mutant blob | Tranche 1 |
|---|---|---|---|---|---|
| C1 | AD-1-R4 G.2 | 175 | `5ecd716a8f123d61…` | `7d95dd425fcd…` | — |
| C2 | controls_defs.py entry 'C2' | 1429 | `f02c3e721aa2c96b…` | `1c10adbc93c2…` | — |
| C4 | AD-1-R4 G.2 | 1627 | `24ac7add3716a030…` | `f3793c2ff61f…` | — |
| C9 | AD-1-R4 G.2 | 1292 | `cda8e8d473a1a465…` | `7ba4a8247ba4…` | 8 |
| C10 | AD-1-R4 G.2 | 1404 | `5e73745b2f8a6d02…` | `329a830718b8…` | — |
| C12 | controls_defs.py entry 'C12' | 1997 | `d484b4d7f5699eb2…` | `d1e46ea63bb3…` | 7 |
| C24 | AD-1-R4 G.2 | 1774 | `39439c3c17a3b90d…` | `3e1bfbe74685…` | — |
| C25 | AD-1-R4 G.2 | 1476 | `29a2c1e571b4f7fd…` | `0456a0c1c1fe…` | — |
| C14 | controls_defs.py entry 'C14' | 1316 | `2c7f17dafd7347cc…` | `72556e3f03a5…` | 2 |
| C19 | controls_defs.py entry 'C19' | 1957 | `066cc479a89ea277…` | `899878323428…` | 3 |
| C21 | controls_defs.py entry 'C21' | 1335 | `65029c7b396057e3…` | `80662d4c7f8b…` | 4 |
| C22 | controls_defs.py entry 'C22' | 334 | `e017ced648c6d432…` | `694b744de3b7…` | 1 |
| C27 | controls_defs.py entry 'C27' | 973 | `bb88f0446b1e56e8…` | `bbcaaaf1dcf9…` | 5 |
| C28 | controls_defs.py entry 'C28' | 1826 | `519378de0e6b6a05…` | `446d0365e147…` | 6 |

These patches are documentary copies under `catalog/patches/`. None was applied to any worktree.

## Observations for the Architect (no action taken)

1. **The definition gap is structural.** CAMPAIGN-01's `controls_defs.py` was a partial preparation (its README); AD-1-R4 G.2 ruled on C1, C25, C4, C9, C10 and C24 only. No later document supplied the rest.
2. **Dormant model scaffolding.** `model_tests.rs` at C already carries state for two actor controls — `World.adm_kept` ("Actor change A2 only") and `WkPos::RaiseH` ("Under actor change A3 only") — but not the code that would use them. A definition for A2 and A3 still has to be written and ruled on.
3. **C22's healthy-build link.** Mission 10 links Stage-1's healthy build as C22's type (e4) evidence. That build ran `cargo test -p nexus-governed-control --locked --release --lib --no-run` (no `-j`; the repository's `[build] jobs = 8`). Mission 12's mutant command adds `-j 2`. AD-1-R4 F.1(d) asks for "a type (e4) receipt of the same command succeeding on the unmutated baseline". The job count enters no artifact or fingerprint, but the command lines differ; whether the link satisfies F.1(d) is the Architect's reading.
4. **Baseline receipts of mission 10** (X-1 alone for C12; T-BIND, T-CELL, W6, T-PREC and T-RES alone) were not produced. They remain required before C12 and the unit-test controls can be attempted.
5. **C1's two encodings agree.** The documentary `C1-mutation.patch` and `controls_defs.py`'s C1 anchor give byte-identical mutants (blob `7d95dd42…`).
