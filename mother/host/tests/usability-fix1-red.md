# PR #98 fix round 1: preregistered review oracles

Owner mission: fix both HIGH findings from review 1, oracle-first, preserving
the frozen A6/A12 proof and every safety assertion. Base:
`1ff19ac97d03ac843cd5198ad78919d0bb9cdfa3`.

Command: `cargo test -p nexus-mother-host --locked --test usability_fix1 -- --nocapture`
through the designated cargo-slot wrapper, TMPDIR and fix1 target.
Qualified RED: **1 passed, 4 failed, 0 ignored**, exit 101.
Receipt: `scratchpad/ux/fix1-oracle-red-qualified.log`.

- Primary-only history: a separate process calls `Home::open/close_session`;
  independent primary-chain verification finds one opened/closed session,
  no Mother journal. Restart wrongly displays fresh-start wording.
- Schema-only crash: hold the successful post-checkpoint directory sync only
  after the main database header names the application identity. Kill the
  process; independent SQLite integrity, identity and chain verification
  establishes zero session records. Restart wrongly displays fresh-start wording.
- Unclassified prior state: an existing Home entry wrongly permits fresh wording.
- Late finalization: actual close-frame write and renderer join succeed; hold
  the join return for four seconds, beyond the original three-second deadline.
  Host exits unsuccessfully with both end records durable, yet the already
  delivered frame claims `RESULT: closed cleanly.`

Initial crash-fixture calibration selected SQLite's earlier WAL-setup parent
sync and failed independent store validation. The qualified fixture additionally
requires the checkpointed application identity; no assertion was removed or
weakened. No production edits preceded the qualified RED run or this registration.
The new interposer is separate from the frozen death harness; all existing
provenance fingerprints and their protected source files remain unchanged.

Explicit H2 oracle amendment, authorized by the current Owner mission:
`usability.rs` normal Quit expectation changes from
`RESULT: closed cleanly.\nSESSION CLOSED recorded\n` to
`RESULT: closing; the final close status follows.\nSESSION CLOSED recorded\n`.
Successful exit, durable-end, deadline, finalization and all other safety
assertions remain in force. The new late-finalization oracle rejects a clean
claim even when output delivery succeeded and both ends are durable.
