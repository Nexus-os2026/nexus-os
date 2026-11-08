# Exact schedule dispositions (review 1 F11)

`coverage.rs` independently generates 153,000 keys. It joins every key to
`combination-dispositions.tsv`, rejects omissions, duplicates and foreign keys,
and requires real successful receipts plus a named, written structural reason
for each equivalence or exclusion. A receipt must also witness that particular
sink in `combination-boundaries.tsv`. Empty/nonempty manifest columns cannot
satisfy the join. The original orphan regression is retained unchanged.

The matrix is **not 153,000 executions**. `execution` means the exact named
combination was run. `equivalence` explicitly composes real boundary executions
and ingress/outcome executions under the frozen admission/ownership contract.
`exclusion` identifies an absent primitive or unreachable lifecycle. Each row
names its reason and receipts. `combination-rules.tsv` defines those reasons;
`combination-boundaries.tsv` identifies all 75 final boundaries, their executed
real-route oracles, and the existing per-route bypass-plant receipts. Neither
model traces nor blanket denial supply successful controls.

The distinction is important for a serial primitive: input acceptance, scope
capture, pre-validation, reservation and waiting for its final hold all precede
the same final admission using the same original immutable scope. These do not
create six new authorities. The before/compare/owning-hold controls exercise the
actual final boundary; the separate input controls exercise the CR/LF/CRLF,
EOF/Ctrl-D/master-close/read-error, Quit and fatal restriction publication. The
late side of that boundary uses R-1: an entered native call can finish its
internal finalization, while each successor needs its own admission. Poison,
unwind and unknown recording retain the reserved owner. A native return error
never substitutes for an exact physical or durable fact.

The seven private filesystem APIs now have **112 real close-restriction
schedules**: two private-content kinds × seven APIs × before/after ×
control/EOF/Quit/fatal. The 42 matrix tuples for BeforeHold or PhysicalEffect
with EOF/Quit/fatal and no additional fault each cite both content-kind receipts
(84 executions); the 28 successful controls remain in the receipt file. The
old 112 private emergency schedules remain intact. In particular,
`R37:private_write:BeforeHold:EOF:none` is an execution, not an exclusion.
Native coverage additionally executes 32 native record/control schedules and
three retry-cleanup record/control schedules; restore and staging-discard faults
have their own F08 and F01 receipts. These counts are separate from model counts.

A normal Quit's fixed finalizer can run **after input quiescence and before
seal**. R29 uses the `close` equivalence, with the real held/failed-finalizer and
producer-accounting receipts. It is not excluded as an ordinary closed work
request. Sealed/end-record states cannot admit that finalizer again; the retained
close-request oracle supplies that exclusion's bypass check.

Receipt rows retain the exact successful output, archive filename/line and
SHA-256 of the complete log. `coverage-native.log` (91 parent tests) and
`coverage-mother.log` (21 parent tests) were executed in this fix round after the
production repairs. `green-G-restored.log` and `green-E-exact.log` are this
round's real terminal/finalizer runs. The `final-dd6cc3c9-*` rows are explicitly
historical passing receipts for unchanged terminal ingress and underlying
primary-evidence/legacy primitives, already part of the reviewed evidence.
They do not claim execution at the new HEAD. The mandatory final workspace,
host/crash and live gates re-execute the suites at the final candidate SHA.

`python3 mother/host/tests/session/generate_combinations.py` regenerates the
manifest and reasons from checked-in receipts and the frozen sink catalogue.
It refuses a missing referenced execution; it never synthesizes an observed
success. The checked-in expanded manifest is intentional: the independently
generated required set must match an explicit reviewable disposition for every
key, including negative controls and structurally absent combinations.

## SB-3C R32 supplement (spec 8.2-8.4, SB3C-R6)

The 153,000 keys above are the **historical retained matrix**: their sink
catalogue, dispositions and receipts are unchanged, and `session_oracles.rs`
still asserts exactly that count. The two R32 rows in `sinks.tsv` and
`exclusions.tsv` are relabelled residual/unlisted routes and narrowed to
anything outside the 35 listed sinks; they stay `D`, so the matrix is untouched.

`model-bridge-sinks.tsv` is a separate, frozen supplement: 35 rows, one per
SB-R0..R9 sink, each with its owning boundary, primitive evidence, oracle and
effective plant. `mother/host/tests/session_model_bridge.rs` verifies it from
the TSV alone and never calls a production classifier. A separate guard test
fails if the lifecycle list and the code's `SessionState` diverge.

A supplement cell is (sink, dimension, value) over 9 lifecycle states, the 17
ingress classes, the 6 retained failures and 2 schedule sides: 1,190 cells.
That is the B1e side x value form; the full cross product is not authored and
not claimed. Each cell cites a retained test by exact name (`c:`, primitive
evidence and never a claim of new-route execution), names a C2-new oracle
(`x:`), closes with a reason from the closed N/A list, or is OPEN with an owner.
Every cell is bound to the verifier's literal audited table per (sink, dimension,
value), row metadata and boundary text included (SB3C-R8); the table records the
audit (Oracle doc 14.6), it does not prove the audit right. A citation stands
only where the test executes that sink's own boundary on that value; a mock of
the boundary or a neighbouring injection point is OPEN. After the R8 to R11
audits: 239 cited, 2 executed, 240 N/A, 709 OPEN (D1 94, D2 614, F1 1). No
historical receipt counts as a test, and which tests ran is proven by the Gate
Box receipt, not here (Oracle doc 14.10).

The `SB-3C` block of `model-bridge-cases.tsv` lists, for each ingress class, the
session-level primitive it reduces to and the retained evidence for that
reduction. `stop_all` has no retained session-route evidence and is OPEN; six
reductions no session-level test performs (including real partial Ctrl-D input)
are OPEN at the session level (Oracle doc 14.7).
