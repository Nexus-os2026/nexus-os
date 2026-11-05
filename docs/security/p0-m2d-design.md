# P0-M2d design: Hands session addendum

This repository addendum applies the frozen [M2d-D session specification](m2d-d-session-spec.md)
to the host-only portions of the historical `p0-m2d-design.md`. The original
architecture record remains historical and is not rewritten here. Kernel and
legacy broker behavior is unchanged.

- **§6.3 / D-O8 / H2′:** Hands has one original broker-issued session identity
  before input. Every operation retains its original request and target.
  Emergency denies new work, undo and cleanup; lift is unavailable in every
  Hands state. It does not retroactively cancel an entered native call.
- **§6.5 / D-O19:** Undo is a normal same-session operation before emergency.
  Confirmation and native watches precede the one restore handoff. Before
  handoff, emergency refuses; after handoff the existing restore can finish
  its internal finalization. Physical removal, grant closure and recording
  remain independent. No successor cleanup inherits the restore permission.
- **§6.6:** Inventory ownership survives failure and destruction. Normal cleanup
  selects returned disposable entries and preserves active, undo and recovery
  material. Restricted close cannot clean. A clean exit requires joined
  producers, exact debt discharge, atomic sealing, ordered durable Mother and
  primary end records, terminal finalization and output delivery within the
  original deadline. Restart only reports verified evidence and uncertainty.

The original kernel/legacy H2′ restore-watch controls remain frozen. Removing
emergency undo is not a new kernel observer and does not close the handoff
window. The [host oracle registration](m2d-d-host-oracles.md) and
[old/new assertions](m2d-d-session-oracle-amendments.md) specify changed host
expectations. Installed physical acceptance remains a separate Owner step;
this addendum declares no phase or capability acceptance.
