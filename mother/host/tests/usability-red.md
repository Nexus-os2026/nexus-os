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
