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
