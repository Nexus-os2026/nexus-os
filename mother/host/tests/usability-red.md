# U1–U5 preregistered RED receipt

Owner approval: “Make these 5 fixes don't keep pending.”
Production baseline: `08a407d8d461442ed0926b59763884bb147cc1b9`.
Command: `cargo test -p nexus-mother-host --locked --test usability -- --nocapture`
(using the required cargo-slot wrapper, TMPDIR and dedicated target).
Result: **2 passed, 10 failed, 0 ignored**, exit 101, 10.32 seconds.
Full receipt: scratchpad/ux/oracle-red.log (outside the repository).

- RED U1 visible echo: `expected "run"` before Enter.
- RED U1 editing: expected `MISSION done`, got failed protocol.
- RED U1 empty lines: expected `MISSION done`, got failed protocol.
- RED U1 Stop with blocked echo: missing `echo-hold` boundary because no echo exists.
- GREEN negative controls: nonempty wrong/space/overlong still fail closed;
  complete/partial/cleared typed-ahead input does not approve.
- RED U2: `MISSION done` count was 2, expected 1.
- RED U3: old recovery disclosure on a fresh home instead of fresh-start line.
- RED U4 normal, declined/protocol and native durability failure: no `RESULT:` line.
- RED U5: unknown command lacks the valid-command list.

No production code changed for this run. Tests and the echo-write/native-Stop
interposition fixture are registered before implementation. Old oracles are
amended in a separate `test(oracle-amend)` commit quoting Owner authority.
No existing oracle expected two `MISSION done` lines; the new exact-count
oracle covers that defect without removing another output assertion.

## U4 wording clarification before GREEN

Owner U4 requires “derived from the same facts the technical block reports
(never more optimistic: uncertain stays ‘uncertain’; unrecorded stays ‘not
yet recorded’).” The new normal-undo oracle originally said “RESULT: the
approved file was removed; restore completed and was recorded.” Its clarified
expectation is “RESULT: the approved file was removed; restore completed, but
other recording status is uncertain.” The existing broker observation records
the native restore separately and does not establish per-request primary
recording for undo (`host_session/outcome.rs`); the stronger blanket wording
would hide that distinction. The baseline RED never reached this assertion;
no GREEN result has been seen. All physical, grant, journal and safety
assertions remain unchanged. This follows the same Owner approval above.

The echo-hold fixture accepts any initial printable prefix of `mistake` as
its held write, rather than assuming the PTY reader consumes all seven bytes
in one read. This changes no expected behavior or assertion; the native Stop
recording receipt is still required while the real renderer write is held.

## Consequential provenance-seal amendment

Owner: “Make these 5 fixes don't keep pending.” U1's mandatory blocked-echo
PTY fixture and U3's approved old/new fresh-start assertion change two sealed
oracle files. The first full host gate rejected the stale digest (6 passed,
1 failed in the sealing target); the no-fail-fast workspace run confirmed it.
Both new digests below identify the already committed **pre-production**
oracles, not expectations derived from product results:

| File | Old SHA-256 | New SHA-256 / registered commit |
|---|---|---|
| `mother/host/tests/session_protocol.rs` | `c707a7359fc2122f01e80893f135e2b38ced6aa1eafb7f1066ff2b7f66c51d89` | `bbc0a15ace1bcd682f1daebeead1793c12b82bfd20c3e2a72bc342077e99b71b` / `1577232a` |
| `mother/host/tests/support/faults.c` | `eb49fa95f07b430062cc396c80a3c1677a42dff75bb652599eb83990e0078de5` | `68c4c3dc0e2dfc69f7584154788d71f008270fa6b4a1c1e8ecdfd0f5647a2c1b` / `2349a9e6` |

The explicit `test(oracle-amend)` changes only these two fingerprint values
and their authority comments. All guarded filenames, exact comparisons,
model/outcome digests, successful-native-report, torn-line, fake-receipt and
behavioral safety checks stay intact. Temporary one-file mutations must still
be rejected by the real sealing test, and restored sources must pass.

Observed seal controls after amendment: sealing suite **7 passed, 0 failed**;
protocol-file mutation **rejected, exit 101**, with that exact path named;
echo-fixture mutation **rejected, exit 101**, with that exact path named;
restored exact sealing test **1 passed, 0 failed**. Both source files were
restored byte-for-byte. Receipts: `scratchpad/ux/seal-plant-protocol.log`,
`seal-plant-echo-fixture.log`, `sealing-restored-green.log`, `seal-plants.json`.
The adapter boundary correction independently passes its unchanged guard
(**1 passed**) and the usability suite (**12 passed**).
