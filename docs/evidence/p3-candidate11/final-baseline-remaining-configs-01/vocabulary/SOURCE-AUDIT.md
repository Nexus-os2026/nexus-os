# Branch-vocabulary source audit — Candidate C (manual inventory)

Mission P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01, section 5. Executor: Claude Code, Claude Opus 5.5.

Source read from git objects of frozen C `d5964ee5326c2733177307db6f507328023d361a`:
`model_tests.rs` blob `28085ce232827301104b5d757af042ec933976f8`, `protocol.rs` blob
`a2df2841b99cad5d452008a6d4150e9369d07e66`. Line numbers below are of those blobs. This is the manual inventory; the
machine form of the same inventory is `SITES` in `tools/vocab_generate.py`, and two independently implemented static
checks (`tools/vocab_check_sites.py`, `tools/vocab_check_eval.py`) verify it against the source.

## 1. Accounting unit

The unit is **(operation identity, exact emitted label)**. Every label is `<operation>: <body>`; the operation is the
text before the first `": "` and is fixed by the format expression of the emitting site (no body contains `": "`). No
label occurs under two operations (generator check; check B, B5).

## 2. How a label reaches a report

1. **Entry.** `Obs::branch(&mut self, label: String)` (model_tests.rs 2205–2207) runs `self.branches.push(label)`. It
   is the only write to any `branches` field in the crate (check A, A3: one write; every other mention reads, or copies
   an `Obs`'s or a record's `branches` into a record: `successors` 3836, `observe_admission` 4925, `expand` 4975,
   `items` 5469, `hold` 6430/6438, and the witness records 8345/8701).
2. **Tally.** Each explorer extends a `BTreeSet` `taken` from every successor's/item's/observation's `branches`:
   `explore` 3934; `explore_experimental` 5116, 5127; `explore_symmetric_kept` 6694, 6707 (check A, A4: no other
   extension). A step's labels are tallied even when the step also violates a property.
3. **Printing.** Each of the three printers (4089–4103, 5303–5317, 6881–6895) prints every label of the static
   `BRANCHES` constant (3701–3792), in order, as `[taken    ]` or `[NOT TAKEN]`, then **every other taken label once**,
   in `BTreeSet` order, as `[taken    ] <label> (not in the listed set)` (check A, A4).

Consequence used by the normalizer: a vocabulary row that is not a `BRANCHES` label and is not printed as taken was
**not taken** in that run. A printed label outside the vocabulary is a STOP.

## 3. The eleven emission sites

Every `.branch(` call of the crate (all 49 `.rs` files) was classified by receiver type (check A, A1; check B, B1):
eleven are `Obs::branch` (below); six are `Oracle::branch` — a choice among options that emits no label (692 `Io::undecided`,
735 `Io::do_commit`, 2713 `step_se`, 2977 `step_wk`, 3166 `step_ack`, 4271 `the_explorer_machinery_is_sound`).

| Site | Line | Function | Call | Operation(s) | Label expression | Reached from |
|---|---|---|---|---|---|---|
| S01 | 2489 | `fact_append` | `obs.branch(format!("append_facts: {label}"))` | append_facts | `let label = match &outcome` (2475–2488), 6 arms | `step_fw` FwPos::Armed, the wait not expiring (2551): FW actors; `step_se` SePos::Armed (2746): SE |
| S02 | 2548 | `step_fw` | `obs.branch("expire: Armed -> Counted".into())` | expire | literal | `step_fw` FwPos::Armed when `expires(o)` takes its fault option (2540): FW actors |
| S03 | 2600 | `step_fw` | `obs.branch("DROP: a Counted ticket dropped".into())` | DROP | literal | `step_fw` FwPos::Got, third option, offered only when `config.drop` and the ticket is Counted (2565–2569): FW actors |
| S04 | 2688 | `settlement_read` | `obs.branch(format!("{name}: {label}"))` | lookup, explain | `let (outcome, label, name) = if claim == Claim::Lookup {…} else {…}` (2666–2684); lookup match 5 arms, explain match 3 arms | `step_se` SePos::Claimed(f, Lookup or Explain) (2732): SE |
| S05 | 2939 | `worker_op` | `obs.branch(label.clone())` | barrier, resolve, append_cell C / X / U, empty | `let (label, stale_answer) = match action` (2870–2935), 5 arms | `step_wk` WkPos::Act (3003): WK; `step_replay` (3352): REPLAY (F-R); also the unit test `e8_holds_for_stale_worker_operations` (4802), whose `Obs` is never reported |
| S06 | 3055 | `step_gc` | `obs.branch(format!("append_gating: {label}"))` | append_gating | `let label = match &outcome` (3043–3054), 5 arms | `step` Actor::Gc (3365): G1, G2 |
| S07 | 3153 | `step_ack` | `obs.branch(format!("snapshot: {label}"))` | snapshot | `let label = match &outcome` (3148–3152), 3 arms | `step` Actor::Ack (3366), AckPos::Idle |
| S08 | 3180 | `step_ack` | `obs.branch(format!("malformed snapshot: {}", MALFORMATIONS[which]))` | malformed snapshot | `MALFORMATIONS` (3108) | AckPos::Holding, options 2–4, offered only when `config.malformed` (3161–3165) |
| S09 | 3216 | `step_ack` | `obs.branch(format!("append_acknowledgement: {label}"))` | append_acknowledgement | `let label = match &outcome` (3210–3215), 4 arms | AckPos::Confirmed |
| S10 | 3258 | `step_adm` | `obs.branch(format!("admission: {}", if admitted {…} else {…}))` | admission | `if admitted { "admitted" } else { "refused" }` | `step` Actor::Adm (3367); `step_with`/`observe_admission` with `adm = step_adm` (the composed runs); negative variants `adm_viewing_twice` … `adm_writing_the_table` (5829–5866) |
| S11 | 5798 | `admit_by` | `obs.branch(format!("admission: {label}"))` | admission | `let label = if admitted { "admitted" } else { "refused" }` | only `adm_ignoring_unapplied`, `adm_ignoring_recovery`, `adm_ignoring_the_latch` (5807–5826) and `adm_favouring_g2` (7673): negative-case tests; **not reachable from any required test** (check A, A5) |

## 4. Interpolated finite domains

Every enum whose rendering enters a label derives `Debug` (`#[derive(…, Debug, …)]`), and no hand-written `Debug`
exists anywhere in the crate (check A, A6). A derived `Debug` prints a unit variant's name and a tuple variant as
`Name(<payload's Debug>)`. Declarations in protocol.rs:

| Enum | Line | Variants (declaration order) | Where it enters a label |
|---|---|---|---|
| Persistence | 311 | NotWritten, Uncertain | append_facts `NotApplied {:?}` (counted.persistence); append_gating `Refused {persistence:?}`; AckOutcome::NotApplied |
| Reason | 556 | Entry(Entry), Conflict{member, known}, SessionClosed{member}, Corrected{seq}, TableFull{member}, Constraint, Storage | via `reason_label` (2210–2225), all 7 arms |
| Known | 550 | Cause(Cause), Explained(Seq) | `reason_label` Conflict arm (nested match, 2 arms); lookup's explicit arms |
| CauseKind | 420 | IdentityContent{…}, DuplicateRole{…}, MissingPrepared{…} | via `cause_label` (2227–2233), all 3 arms |
| Entry | 534 | NotOpen, Latched, NotHealthy, CellsBusy, Malformed, Expired | `Entry({entry:?})`; `Refused(…)` of Lookup, Explain, Resolve, CellAppend; `NotNow(…)` of SnapshotOutcome and AckRefusal |
| TicketFault | 544 | OtherLedger, NotArmed | append_facts `Rejected({fault:?})` |
| FactOutcome | 567 | Committed, NotApplied{counted, reason}, Rejected(TicketFault) | S01 match (all variants by explicit arms) |
| GatingOutcome | 574 | Committed(Seq), Refused{persistence, reason} | S06 match (all variants by explicit arms) |
| Lookup | 583 | Present(Seq), Absent, Conflict(Known), Refused(Entry), Failed | S04: Conflict and Present by explicit arms; `{other:?}` renders Absent, Refused(Entry), Failed |
| Explain | 592 | Explained(Seq), Pending, Busy, Unknown, Refused(Entry) | S04: Explained explicit; `{other:?}` renders the rest |
| SnapshotOutcome | 622 | Taken(Snapshot), Nothing, NotNow(Entry), Failed | S07: Taken explicit; `{other:?}` renders the rest |
| AckRefusal | 630 | OtherStore, NotNow(Entry), Stale, Empty, Future, Set | inside AckOutcome::Refused |
| AckOutcome | 640 | Committed(Seq), Refused(AckRefusal), NotApplied(Persistence) | S09: Committed explicit; `{other:?}` renders the rest |
| Cell | 655 | U, C, X | S05 `append_cell {cell:?}` (`let cell = token.cell`) |
| BarrierOutcome | 729 | Passed, Failed, NotNeeded, OtherLedger | S05 `{o:?}` |
| Resolve | 737 | Absent, Corrected(CellToken), Acknowledged(Seq), ConflictPlaced(CellToken), Retained, Inconsistent, Stale, Refused(Entry), OtherLedger | S05: Corrected, ConflictPlaced, Acknowledged explicit; `{other:?}` renders the rest |
| CellAppend | 750 | Committed(Seq), NotWritten, Uncertain, Stale, Refused(Entry), OtherLedger | S05: Committed by three guarded/unguarded explicit arms; `{other:?}` renders the rest |
| Emptying | 760 | Done, NotCommitted, Stale, OtherLedger | S05 `{o:?}` |

`MALFORMATIONS` (model_tests.rs 3108) is `["future", "set", "empty"]`; `which - 2` ranges over 0..3 exactly when
`config.malformed` (options = 2 + 3).

Every payload a `{…:?}` renders is a finite enum: the variants whose payload is not (Present(Seq), Explained(Seq),
Taken(Snapshot), Committed(Seq), Corrected(CellToken), …) are consumed by earlier explicit arms of the same match
(generator: anchored arms; check B: an unbounded value reaching a label is a failure, and none does).

### Type-domain rows

`append_cell {cell:?}` ranges over Cell's three declared variants. The core's `agenda()` (protocol.rs 954–966) builds
the C token with `Cell::C` and the X token with `Cell::X`, and `allowed()` (2803–2826) makes `AppendC`/`AppendX` from
exactly those tokens (U's token only feeds `Resolve`), so `append_cell U: …` is not expected to be emitted. The
fourteen `append_cell U` rows are nevertheless kept: the vocabulary may conservatively include a finite source-
emittable label and must not omit one, and this audit uses no reachability argument to remove a type-domain value.
Likewise every Persistence × Reason combination is kept for append_facts and append_gating.

## 5. Vocabulary size

193 rows: append_facts 36, expire 1, append_gating 34, lookup 12, explain 11, snapshot 10, append_acknowledgement 16,
barrier 5, resolve 15, append_cell C 14, append_cell X 14, append_cell U 14, empty 5, admission 2, DROP 1, malformed
snapshot 3. The static `BRANCHES` constant lists 90 labels, all of them vocabulary rows; **103 source-emittable labels
are missing from it** (`STATIC-OMISSIONS.txt`), among them the three the mission names — `append_cell C:
Refused(Latched)` (B148), `append_cell C: Refused(NotHealthy)` (B149), `append_cell X: Refused(NotHealthy)` (B163) —
and a fourth that a Stage-1 report already prints as taken: `resolve: Refused(NotHealthy)` (B135, F-1).

## 6. Collision analysis

- **Same label, two sites — resolved.** `admission: admitted` and `admission: refused` are emitted by S10 (`step_adm`)
  and S11 (`admit_by`). Both implement the `AdmStep` type ("An admission step: `step_adm`, or a deliberately defective
  one in a negative case", 4832–4834): each is ADM's one admission step, spends one admission, labels `admitted`
  exactly when that step admitted and `refused` otherwise, and runs `admission_properties` when admitted; they differ
  only in the admission predicate. They are therefore the same operation and semantic branch, and share one row per
  label. Attribution is not needed for any required receipt: no required test reaches S11 (check A, A5; control
  NC4b shows the check detects the opposite).
- **One site, several callers — not a collision.** S01 is reached from FW and from SE, and S05 from WK and REPLAY.
  The operation is the core call the label reports (`append_facts`; the worker verb), and the label is that call's
  outcome; the caller is not part of the operation's identity in the source (the static `BRANCHES` constant and the
  printers treat these labels as one each). These rows are single-site rows.
- **Same body, different operations — not a collision.** e.g. `lookup: Refused(Latched)` and
  `explain: Refused(Latched)`: the operation prefix differs, so the exact labels differ.
- **Explicit text against `{other:?}` text.** In every match an explicit arm's string differs from every rendering
  of the variants left to `{other:?}` (e.g. resolve's explicit `Corrected` and the rendered `Absent`, `Retained`, …),
  and no rendering equals `unwound` (no variant has that name).

No unresolved collision exists; `BRANCH_VOCABULARY_BLOCKED` does not apply.

## 7. Methods and their independence

- **Generator** (`vocab_generate.py`, method A): the inventory above in machine form; every call, block and arm is
  anchored verbatim in its function at C, every label match's arm count is compared with the source, helper matches
  must cover their enums, interpolated enums are expanded from declarations parsed from the source, and the number of
  `obs.branch(` calls must equal the inventory's eleven. `--compare` regenerates and compares row for row (the freeze
  check).
- **Check A** (`vocab_check_sites.py`): line/regex scanner of all 49 crate files with its own masking; resolves every
  `branch(` token's receiver type; site, sink and printer closure; reachability from the required tests; derived
  `Debug`; label literals.
- **Check B** (`vocab_check_eval.py`): its own lexer, parser and evaluator; derives each site's label set from the
  source's control flow (patterns, guards, correlated `let` bindings, `format!`, derived `Debug` over declared types),
  with no inventory, and compares site by site.
- **Controls** (`vocab_controls.py`): an omitted source site (two forms), an omitted enum variant (two forms), a
  changed exact label (two forms), an unresolved collision (two forms), a raw observed label outside the vocabulary
  (two forms), a vocabulary row with no source derivation (two forms), a NOT APPLICABLE operation taken, unsorted
  extras, a truncated list, two positive controls and two meta-controls.
