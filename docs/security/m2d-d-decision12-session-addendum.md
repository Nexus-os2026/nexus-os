# Decision 12: preserved history and session supersession

Decision 12's original history is retained in the external architecture record
and Git-era host registrations. This addendum neither rewrites that history
nor creates a new architecture decision. Decision 12 amendment A and the
[frozen M2d-D session specification](m2d-d-session-spec.md), including amendment
B, govern the current Hands candidate.

The earlier cleanup-on-restart promise is superseded. Restart verifies the
kernel ledger, Mother history and primary evidence/health and reports surviving
resources and uncertainty. It does not remove files/staging, restore workspace
permission or consent, reconstruct undo or acknowledgement capabilities, adopt
a run, or retry an uncertain operation. Unresolved or invalid sources prevent
work readiness. An empty apply summary is not an all-clear resource proof.

The host-only H2′ amendment removes emergency lift/undo/cleanup while retaining
normal pre-emergency undo and the entered-call handoff limit. Native kernel and
legacy broker contracts remain unchanged. See the [P0-M2d addendum](p0-m2d-design.md),
[H1 addendum](m2d-b2-h1-decision.md), [oracle amendments](m2d-d-session-oracle-amendments.md)
and [installed acceptance procedure](../../mother/README.md#install-and-record-the-exact-build-owner-procedure).

Architect Ruling R-1 accepts the disclosed oracle-provenance deviation at
`05239391` without rewriting history. Later fixes identify their oracle commits.
Process deviations are disclosed and repaired; they are not specification STOPs.
No local result substitutes for exact-SHA CI, independent implementation review,
security-core approval or Owner physical acceptance.
