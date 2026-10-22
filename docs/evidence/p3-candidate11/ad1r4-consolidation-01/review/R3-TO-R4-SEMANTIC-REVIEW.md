# AD-1-R3 → AD-1-R4: semantic diff review

Prepared by the executor (Claude Code, Claude Opus 5.5) under P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01, for the Sole
Architect's independent final contract review. It is the executor's own review of its own consolidation, so it is not
independent review and accepts nothing. AD-1-R4 remains DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED.

**Scope.** Every section the AD-1-R3 → AD-1-R4 diff touches has its own heading below (`tools/check-review.py`
verifies that the headings match the touched sections exactly). Each change is classed as:

- **CONSOLIDATED** — the text of the C4-WITNESS-01 ruling, of the mission's directives, or of AD1R3-DISPOSITION-01
  (accepted as the repair basis), written into the operative section, with the sheet's "R3 n" pointers resolved;
- **CONSEQUENTIAL** — a cross-reference or scope phrase that the consolidated text makes necessary (for example "G.3
  for C1 and C25, G.5 for C4"), changing no obligation;
- **EXECUTOR WORDING** — wording the executor chose where the sources give the substance but not the words; listed so
  that the Architect can check that the words carry exactly the substance;
- **EXECUTOR ADDITION** — anything not stated by a source. Each is listed under "For the Architect's attention" at the
  end, with the reason. None adds an obligation on a control's kill, none widens a claim, and none authorises anything.

Every C4 witness line quoted in AD-1-R4 is checked by program against the authenticated raw outputs at evidence commit
B; every source quotation against `protocol.rs` at 42dd1bd1 or the sealed `controls_defs.py`; every contract or review
quotation against the authenticated bytes (`checks/01-check-ad1r4.txt`).

### header

- Title and status: AD-1-R4; DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED. NOT IN FORCE. CONSOLIDATED (mission 1).
- "the one canonical consolidated amendment candidate"; consolidation of AD-1-R3, the sheet and the C4-WITNESS-01
  ruling; the Owner keeps authority "over every contract amendment and over its adoption". CONSOLIDATED (mission 1, 10).
- "Self-contained" paragraph: a reader does not need AD-1 … AD1R3-DISPOSITION-01; the earlier files are immutable
  history. CONSOLIDATED (mission 10).
- Verbatim rule extended to two more sources: `VERBATIM PROTOCOL` (protocol.rs at 42dd1bd1) and `VERBATIM DEFS`
  (sealed `controls_defs.py`); fenced `rust` blocks are never quotations and are derived by the checker. EXECUTOR
  WORDING (a presentation device, so that C4's, C9's, C10's and C24's definitions can stand in the draft itself).
- "Nothing here … reads C4's "P4, P8" as a choice between them" added to the list. EXECUTOR ADDITION (a restatement
  of the ruling's "do not reinterpret the comma as 'or'"; see attention item 1).

### 0

- Rows added: the C4 witness prototype source (A, tree, parent, model_tests.rs blob, +678/−0), the C4 prototype
  evidence (B, review ref, patch, baseline and mutant `protocol.rs` blobs, both executables), the disposition sheet, and
  the sealed C10/C24 definitions. Each identity is the mission's, re-verified (stage 1; checker C6). Labels:
  PROVEN FROM SOURCE for what was read back; EXECUTOR-REPORTED for receipts; ARCHITECT-RULED for the C4-WITNESS-01
  ruling. CONSOLIDATED (mission 2, 11).

### 0A.1

- New governing table: the ruling of P3-C11-DR01-D2A1-C4-WITNESS-01, one row per ruling item of mission section 2
  (prototype review passed, prototype acceptance only; the sheet accepted as the repair basis; R3-01 and R3-02 ACCEPT;
  C4 both P4 AND P8, comma never "or", G.5 the route, the witness does not replace `full_f8`; C4's definition; the
  Oracle rule ACCEPT, Oracle unchanged; C9-b; C10 and C24 retained; no C9/C10/C24 execution; C1/C25 prior rulings
  unchanged, no final kill). CONSOLIDATED.
- The two earlier rulings (AD1R3-READINESS-01; witness audit) are carried unchanged. The closing sentence becomes
  "None of these rulings, and nothing in this draft, accepts D2A-1 …". CONSEQUENTIAL (three rulings now).

### 0A.2

- Retitled "Repairs consolidated here"; rows R3-01, R3-02, C4 and C9-C10-C24 added after AD-1-R3's four; R2-01's row
  now says "the witness invocations" (C4's witness included). "these four items" → "these items". CONSOLIDATED /
  CONSEQUENTIAL.

### 0A.5

- New: the earlier drafts and the disposition sheet are history, each with its sha256; none is changed; where they and
  AD-1-R4 differ, AD-1-R4 states the proposal. CONSOLIDATED (mission 10).

### 1

- Numbering note: S-19 to S-21 are AD-1-R3's; S-22 to S-25 new; S-5, S-6 and S-18 amended. CONSEQUENTIAL.
- S-5: SUPPLEMENTED with the restored-receipt types ((e1), (e2), (e4)). CONSOLIDATED (sheet section 2, I item 7).
- S-6: "REPLACED for C4 only: the kill criterion is G.5"; the witness-only rules (G.3(c), G.5, G.6) apply only to the
  witness invocations, types (c) and (e3). CONSOLIDATED (ruling: C4's route) / CONSEQUENTIAL.
- S-18: "every control is killed" reads per G.5 for C4. CONSEQUENTIAL.
- S-22 (line 704, C4): the comma is a conjunction, never "or"; recorded only under G.5 as "P4, P8 (first failure
  <exact property printed>; witness)"; the change has G.2's exact definition. CONSOLIDATED (ruling; mission 5).
- S-23 (line 709, C9), S-24 (line 710, C10), S-25 (line 720, C24): CLARIFIED, ARCHITECT-RULED definitions in G.2;
  named property and configuration unchanged. EXECUTOR ADDITION as rows (see attention item 2); the substance is the
  ruling's.
- Verbatim blocks for S-22 to S-25 (contract lines 697 with 704, 709, 710, 720). Checked byte for byte.
- "Not superseded": the 7.5 exception list names S-22 to S-25; a new item keeps section 4.8's six tests (G.2's C4
  definition removes Test 1 from the mutant only). The line-750 note says the documentary patches travel only in
  earlier packets (this packet carries none). CONSEQUENTIAL; the 4.8 item is an EXECUTOR ADDITION (attention item 3).

### F

- Kill sentence: "read under G.3 for C1 and C25 and under G.5 for C4". CONSEQUENTIAL.
- Restored receipts typed: (e1) for a configuration; (e2) for a named unit test; (e4) for C22. CONSOLIDATED (sheet
  section 2; R3-02).

### F.1

- One taxonomy, as mission 4 directs: (a) original-explorer mutant FAIL; (b) named non-explorer mutant-test FAIL;
  (c) named-property witness (mutant); (d) C22's intentional mutant compile failure; (e1) unchanged-core exploration
  PASS; (e2) restored named-unit-test PASS; (e3) healthy-witness PASS; (e4) healthy-build success; (f) the A4
  comparison. CONSOLIDATED.
- The summary table's "Role" and "Kill evidence" columns: EXECUTOR WORDING (each cell restates F, G.3 or G.5).
- One type per receipt: the sheet typed the healthy witness both as (c) (witness rules) and (e3) (PASS evidence).
  AD-1-R4 types it (e3) only and states the shared witness rules once, in F.2, for (c) and (e3). EXECUTOR WORDING that
  resolves the double typing, as mission 4's "single coherent taxonomy" requires; no rule is dropped (attention item 4).
- General rule added: "An artifact or field that does exist is recorded as it is: no NOT PRODUCED BY DESIGN marking
  ever erases it." CONSOLIDATED (mission 4).
- (a): the primary receipt of C1, C25 and C4; for C4 "the printed property is recorded as it is under G.5(A); C4's
  attribution of both P4 and P8 comes from G.5(B), never from this receipt alone" (replaces "not settled here (open
  item 10)"). CONSOLIDATED (ruling).
- (c): the mutant witness of C1/C25 and C4, its harness result, its markers, for C4 its consumption record (G.6); the
  witness-only rules confined to (c) and (e3). CONSOLIDATED (sheet sections 1, 3).
- (d): healthy evidence is an (e4) receipt; NOT PRODUCED BY DESIGN covers only the intentionally failing mutant build;
  any artifact it does leave is recorded as found; never extended to the healthy build. CONSOLIDATED (sheet section 2;
  mission 4).
- (e1): R3's (e) unchanged in substance, plus "K.2 points 5 to 7, as for every type" and "Only this type requires an
  explorer outcome, state or key counts, property lines and branch coverage". CONSOLIDATED (sheet section 2).
- (e2), (e3), (e4): as the sheet, with "K.2 points 5 to 7" where the sheet wrote "5–6" (attention item 5); (e3)
  gains a selection line (F.2's "exactly the one named test"). CONSOLIDATED / EXECUTOR WORDING.
- (f): "two admissible receipts of type (e1)". CONSOLIDATED.

### F.2

- New: witness artifact identity by role. Mutant witness on the same pinned mutant executable as its control's
  primary invocation, where that exists, binding the mutation source identity; healthy witness on the pinned
  unchanged-baseline executable; sha256 equal before and after; the common rejections (identity, selection, fixture,
  unrelated panic, compilation, incomplete output, K.2 points 5 to 7); role-specific outcomes in a table; a positive
  marker in a healthy receipt, or a healthy line in a mutant receipt, NOT ADMISSIBLE. CONSOLIDATED (R3-01; mission 3).
  No sentence anywhere says the healthy witness uses a mutant executable (contradiction scan X1).

### G

- Retitled for definitions and C1, C25 and C4. Preamble: G.3 for C1 and C25 only; G.5 and G.6 for C4 only; neither
  an exception for, nor extended to, any other control; "Every other control is killed only under F". CONSOLIDATED
  (sheet section 3: "G.3 stays for C1 and C25 only"; "not extended to any other control"); the last sentence is
  CONSEQUENTIAL (F already says so).

### G.1

- Rows added: the C4 witness source at A; the C4 prototype gate (historical supporting evidence only; discharges none
  of G.5(A) to (E); no final kill); C9, C10 and C24 (defined in G.2; not applied, built or run; no execution
  authorised). The first-failures row adds "P4 for C4" and "no `full_f8` run on the C4 mutant". CONSOLIDATED
  (ruling; mission 5, 8).

### G.2

- Retitled "Control definitions". The patches are inert copies in earlier packets; this packet carries none.
  CONSEQUENTIAL (mission 12 forbids mutation source in this packet).
- C4: the five-check removal exactly as the authenticated patch defines it (Test 0, `let core`, and everything from
  `let set` kept); patch sha256; lines 1627–1642 removed, none added; mutant blob. The contract's Test 1 row and the
  healthy text are quoted verbatim; the resulting text is a `rust` block that the checker derives, and the checker also
  verifies that removing exactly those lines gives the mutant blob and that the authenticated patch removes exactly
  them (C1r-b to C1r-d). CONSOLIDATED (ruling; mission 5).
- C9-b: in `Publish::rolled`, arm `Rollback::Unknown`, `self.failed();` kept and only `Persistence::Uncertain` (line
  1294) → `Persistence::NotWritten`; E4 on F-B2; before (verbatim) and after (derived). CONSOLIDATED (ruling; mission 8).
  "The other candidate … is not selected" and "No documentary patch of C9 exists yet" are EXECUTOR WORDING (factual).
- C10 and C24: the sealed `controls_defs.py` entries quoted verbatim (lines 37–47, 48–66), retained unchanged; the
  anchors' protocol.rs lines (1404–1408, 1774–1787, each occurring exactly once) are re-derived by the checker (C23).
  The one-line description of C10's replacement is EXECUTOR WORDING checked against the quoted entry. CONSOLIDATED
  (ruling; mission 8).
- "No execution of any of them is authorised by this draft or by the ruling it carries." CONSOLIDATED (ruling).

### G.3

- (b) names its type, (c). (c): applies to the witness invocations of (b) and (d), types (c) and (e3); the executable
  bullet is the role-specific one ("for (b), the pinned mutant executable of (a); for (d), the pinned
  unchanged-baseline executable"); a positive marker in (d) or a healthy line in (b) is rejected. (d): on the pinned
  unchanged-baseline executable, a type (e3) receipt. CONSOLIDATED (R3-01, the sheet's replacement text). Every C1/C25
  rule of mission 7 is otherwise unchanged (checker C24).

### G.4

- Text unchanged. The diff touches it only because G.5 and G.6 are inserted after its last paragraph (one blank line
  falls in G.4).

### G.5

- New: C4's kill criterion, for C4 only. (A) the primary `full_f8` receipt on the pinned C4 mutant executable under
  F-8's ceiling, type (a), admissible and within its cap, first property recorded unchanged and never relabelled, the
  prediction P4 not a receipt, neither P4 nor P8 → reported and the Architect rules; (B) the deterministic witness on
  the same mutant executable, two scenarios from the unchanged `initial(F8)` with at most 32 actor steps, both
  collected before the verdict, P4 at its frozen locus AND P8 at its frozen locus, only one insufficient; (C) the
  healthy replays on the baseline executable, type (e3), CellsBusy 0 of 6, NotHealthy 0 of 4, G1 kept in U; (D) the
  unchanged-core F-8 PASS on the exact final baseline, type (e1); (E) identities of I, restoration, independent review;
  (F) the recording form. The prototype results at A are historical supporting evidence only. The witness does not
  replace `full_f8`. CONSOLIDATED (ruling; sheet section 3; mission 5).
- The loci are given as the line prefixes the sheet gave ("… step 8 of scenario P4 (", "… step 12 of scenario P8 ("),
  and the full lines printed at A are quoted beside them. EXECUTOR WORDING (attention item 6).
- The header and "OTHER PROPERTIES" lines printed at A are recorded as informational, "not further conditions of (B)".
  An earlier development form required them by analogy with G.3(b); that was an executor over-reach and was removed
  before the freeze (`receipts/02-development-log.txt`).
- The rejection list for the C4 witness restates F.2's common rules, the role rule and G.6 for this invocation; the
  sentence "A PASS kills nothing. In each case 7.5's stop rule applies" restates line 693. CONSEQUENTIAL.
- "A witness carried to the final baseline that prints other text at these loci needs the Architect's review of that
  text before its receipt is used." EXECUTOR ADDITION (attention item 6).

### G.6

- New: the offered-versus-consumed Oracle rule, for C4's schedules only, as accepted: same actor sequence and offered
  stream; identical and frozen consumption, option counts, branch labels and remaining faults before the changed
  acknowledgement; the two planned ends (healthy: refuse before storage, zero storage choices, full suffix unused;
  mutant: exactly the frozen storage-choice stream, no unused suffix); every consumed choice equals the offered
  prefix; the five rejections; recorded and verified twice; Oracle and the replay helpers unchanged. CONSOLIDATED
  (sheet section 3; mission 6).
- Provenance for the reason: `Oracle::choose`, `model_tests.rs` line 552 at 42dd1bd1 (`map_or(0, …)`), checked by
  program (C2g). EXECUTOR WORDING (a source citation).
- The frozen changed steps at A (offered and consumed streams) are quoted; each is checked against the raw output at B
  (C2y). The test's name and "carrying it to the final baseline needs its own authorised mission". CONSOLIDATED.

### I

- Every C1, C25 and C4 witness receipt, mutant and healthy, is made on the exact final baseline or its mutant; the C4
  prototype receipts at A are informational, not 8.3 or 7.5 evidence. CONSOLIDATED (mission 5, 9).
- Builds: the mutant executable serves the mutant witness of C1, C25 and C4; every healthy witness runs on the pinned
  unchanged-baseline executable. CONSOLIDATED (R3-01: "I (R3 934): adds …").
- Item 4: C22's NOT PRODUCED BY DESIGN, with "any artifact it does leave is recorded as found". Item 7: the restored
  PASS receipt is of its control's type. CONSOLIDATED (R3-02; mission 4).

### K.2

- The seven points apply to every receipt type, with readings: (b), (c), (e2), (e3) without configuration bounds or a
  discovered count; (d) as before; (e4) as (d) but with the intended zero exit and every produced artifact recorded;
  points 3, 5 and 6 alike for every type. CONSOLIDATED (sheet section 2; mission 4).
- Case 1 typed (e1); case 6 says "no mutant executable" and points to case 12. CONSEQUENTIAL.
- Cases 10 (healthy witness, (e3)), 11 (either witness on the other role's executable, or the wrong marker), 12 ((e4)),
  13 ((e2)): CONSOLIDATED (sheet sections 1 and 2: "new cases are added"). Cases 8 (C4 mutant witness), 9 (only one of
  P4 and P8) and 14 (a G.6 breach): EXECUTOR ADDITION as table rows (attention item 7); each restates G.5 or G.6.
  `tools/classify-cases-r4.py` applies the rules to synthetic descriptors and checks that its classification of each
  case agrees with the draft's row.

### O

- O.3: the minimality of the C4 witness schedules is not claimed. O.6: C4's attribution also comes from the witness
  (G.5); C4's first-reported property is recorded as printed; P4 and P8 are never established by relabelling the
  explorer's report. CONSOLIDATED (ruling; mission 5).

### P

- P.1: C4's witness was added after every existing line at A. P.5: this packet's prefix and parent (B); earlier packets
  unchanged and not copied. CONSEQUENTIAL (mission 12, 14).

### open

- Item 2: the Architect's final contract review, of S-1 to S-25 and of the consolidation. Item 4: closed also as to the
  C4 prototype's source and evidence; no final kill. Item 5: adds the `full_f8` primary run on the C4 mutant and
  carrying the C4 witness to the final baseline, each by its own mission. Item 10: closed as to C4's reading (S-22,
  G.5); still open: preparing and running C9, C10 and C24 under G.2, each by its own mission; the preflight's orderings
  remain predictions. Item 12: names F.2 and the acceptance of R3-01 and R3-02 as the repair basis. CONSOLIDATED.

### changes

- A new table "Changes from AD-1-R3"; the two earlier tables kept as history, retitled as made by AD-1-R3 and AD-1-R2.
  Not operative.

## For the Architect's attention (executor additions and wording choices)

1. **Header list.** "Nothing here … reads C4's "P4, P8" as a choice between them" restates the ruling in the list of
   things the draft does not do. It changes nothing operative.
2. **S-23 to S-25.** The ruling selects C9-b and retains C10's and C24's sealed definitions; AD-1-R4 records each as a
   CLARIFIED supersession row on its contract row (709, 710, 720), as AD-1-R3 did for C1 and C25 (S-7, S-8). Each row
   keeps the named property and configuration. If the Architect prefers the definitions to stand in G.2 without rows,
   the rows can be dropped without changing any obligation.
3. **"Not superseded": section 4.8's six tests.** Added so that G.2's C4 definition is not read as removing Test 1 from
   the contract; it is removed from the disposable mutant only.
4. **One type per receipt.** The healthy witness is typed (e3) only; the witness rules the sheet attached to type (c)
   are stated once in F.2 and applied to both (c) and (e3). No rule is dropped (checks C10e, C10h3, C17).
5. **K.2 points "5 to 7" for (e2), (e3), (e4).** The sheet wrote "5–6". K.2 (carried) requires all seven points of
   every receipt, point 7 read without a discovered count (or, for a build, as "a build that finished"), and the
   sheet's own section 1 lists "K.2 points 5–7" for the witness roles. AD-1-R4 therefore writes "5 to 7"; this names
   completion, which K.2 already required, and adds no new evidence.
6. **C4's witness lines.** The binding requirement is the locus prefix the sheet gave; the full lines printed at A are
   quoted for reference and checked against the raw output. Because the witness must be carried to the final baseline
   by its own mission, AD-1-R4 adds that any other text printed at these loci needs the Architect's review before its
   receipt is used. This is fail-closed and adds no kill condition beyond the loci; the Architect may strike it.
7. **K.2 cases 8, 9 and 14.** Added so that the case table covers C4's witness (both properties, only one, a G.6
   breach). Each restates G.5 or G.6.

## Not changed

Sections A, B, C, D, E (E.1 to E.4), H (H.1 to H.6), J, K.1, L, M, N, Q and R, 0A.3 and 0A.4, and every contract and
review quotation carried from AD-1-R3, are byte-identical in AD-1-R4. The 331 obligation-bearing sentences of AD-1-R3
that the scan finds verbatim in AD-1-R4 stand in its operative text (check-matrix M2b); the other 44 are accounted for
one by one in the consolidation matrix (part C).
