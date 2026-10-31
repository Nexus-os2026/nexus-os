# Mother Nexus M1: mission contract and coordinator core

Status: M1 candidate on `mother/m1-core`. Headless Rust library only: no
UI, no real research, no website building, no model calls. It is not
integrated and is not a passed gate. Every capability here is PLANNED
until its physical test passes on the installed build (`MOTHER.md` §1).

## What M1 is

Mother turns a human goal into a governed mission:

1. **MissionContract** (`contract.rs`): the Owner's words verbatim, Mother's
   restatement, the requested folder (a request to the broker, never path
   authority), a budget (time, lane calls, money = 0), forbidden effects,
   the missing-data questions with their answers, and pass criteria fixed
   before any work. A SHA-256 hash binds every field.
2. **Consent** (`consent.rs`): bound to the exact contract hash. A change to
   the contract voids it. A `Consent` cannot be built, cloned or
   deserialized outside the crate; the coordinator mints one only when a
   `MissionConfirmer` said yes to a confirmation the coordinator built, and
   only if the contract did not change meanwhile.
3. **State machine** (`state.rs`): Draft → AwaitingConsent → Approved →
   Investigating → Building → Verifying → Explaining → Done, plus Stopped,
   Denied, Failed and Blocked. One deterministic table, used live and on
   replay. Stop works from every state.
4. **EffectPort** (`effect.rs`, `adapter/`): the only way Mother reaches the
   world, sealed to `GovernedPort`, the adapter over
   `crates/nexus-governed-control`. Each mission acts as the agent `mother`
   in an agent run of its own, under the Owner's standing grants.
5. **Lanes** (`lane.rs`): Researcher A/B, Skeptic, Builder, Verifier,
   Sentinel. Every call is journaled with the lane's identity, an
   invocation id and the hashes of its input and output. M1 has only
   deterministic lanes.
6. **Journal** (`journal.rs`): append-only and hash-chained. A restarted
   coordinator resumes every mission in its state; a Stopped mission stays
   stopped; a tampered journal is refused.
7. **Walk example**: `cargo run -p nexus-mother --example walk`.

## Invariants and their oracles

The oracles were committed before the implementation (`MOTHER.md` §3.1).
Unit oracles are in `src/oracles/`; the real-broker, file-journal and
source-scan oracles are in `src/adapter/` (only the adapter module touches
the host, tests included).

| Invariant | Oracles |
|---|---|
| MN-01: zero file, network, process, model-tool or spend effects before consent | `oracles/mn01.rs`, `adapter/real_broker.rs` |
| Consent binds the exact hash; a changed contract voids it | `oracles/consent.rs` |
| Stop from every state: no new dispatch, durable across restart, never waits | `oracles/stop.rs`, `adapter/real_broker.rs` |
| Denial (the Owner's no, the broker's refusal) means zero effect | `oracles/deny.rs`, `adapter/real_broker.rs` |
| A planted "consent"/"approved" string grants nothing | `oracles/consent.rs` |
| Journal tampering is detected and refused | `oracles/journal.rs`, `adapter/file_tests.rs` |
| The state machine is the mission's table | `oracles/state.rs` |
| Lanes: identity, invocation id, input/output hashes, budgets | `oracles/lanes.rs` |
| Contract bounds; the hash binds every field; verbatim words | `oracles/contract.rs` |
| No host access outside the adapter; no kernel actuator or legacy stack | `adapter/scan.rs` |
| No production consent confirmer (Owner condition) | `adapter/scan.rs` |
| The port is sealed; the adapter calls only the agent side of the broker | `oracles/structure.rs` |

Coordinator rules the oracles pin:

- Nothing reaches the port, and no lane runs, before consent.
- Mother refuses an effect before the port when it is forbidden, when it is
  spend (the money budget is zero), or when it is a file request without a
  requested folder or not relative to it. Any refusal blocks the mission.
- The Owner's emergency stop is checked before every lane and every effect.
- No coordinator lock is held while the Owner is asked, a lane runs or an
  effect runs. A result that comes after a Stop is dropped and journaled
  as overtaken.
- Every lane call counts against the call budget. The time budget runs from
  consent, measured on both the wall and the monotonic clock.
- An effect dispatched but not settled when Mother restarts blocks its
  mission (`effect_outcome_unknown`): it may have happened. A mission that
  dispatched any effect in its current phase before a restart is blocked
  too (`phase_interrupted`): re-running the phase could repeat it.
- If a journal append fails, the mission ends (or stops) in memory, the
  port is cancelled, and the journal is poisoned durably (a
  `.poisoned` marker beside the file): it opens nothing until the Owner
  reconciles it, so a Stop the journal could not record is never undone
  by a restart.
- Replay accepts records only in the order the live coordinator writes
  them (lane answers after their calls, no transition but Stop while an
  effect is unsettled, mission ids in sequence).

## Non-claims

- **Consent is not yet the broker's.** The broker's native confirmer
  (`ControlConfirmer`) has no mission dialog, so `MissionConfirmer` is
  Mother's own trait, with no implementation in Mother's library (the walk
  example asks at the terminal; only test code fakes one). **M2 moves
  mission consent into the broker (`confirm_mission` in `ControlConfirmer`)
  when the desktop confirmer is rebuilt; that is a security-core change and
  needs Owner approval and a fresh review.**
- Consent is an extra gate, not authority: every effect still needs the
  Owner's broker grant, and an R2 effect needs the Owner's native approval.
  M1 does not wait for that approval: the mission ends Blocked
  (`awaiting_owner_approval`) and its broker run is cancelled, which
  withdraws the pending action, so the Owner can no longer approve it.
- The journal chain is unkeyed. It detects edits, removals, reordering,
  insertion and a torn last line. It does not detect someone with write
  access to Mother's home who rewrites the whole chain, or cuts entries off
  its end. Anchoring the head in the broker's evidence store is later work.
- The journal append is made under the coordinator lock: it is the
  transition's commit (a bounded local write), so the journal's order is the
  state machine's.
- The broker has no file or spend route: the adapter refuses both
  (`no_broker_route`). M1 writes no files for a mission.
- The journal file takes no exclusive lock: two coordinators on one file
  would interleave entries and the next open refuses the journal. A torn
  last line after a crash, or a poisoned journal, refuses the journal with
  no automatic recovery: both fail closed and need the Owner.
- After a restart, the time budget is measured from the wall clock at
  consent (a clock set earlier than the consent counts the budget as
  spent; a clock set back but still later than it is trusted).
- The source scans (`adapter/scan.rs`) are guards, not proofs: they match
  text (for instance, the confirmer check looks for `MissionConfirmer for`,
  `stdin` and a literal `true` body).
- `GovernedPort::cancel` cannot report a failed run cancellation to its
  caller. The adapter counts each one (`cancel_failures`).
- Lane calls are counted as model calls; no model is called in M1.
- Mother depends on `nexus-governed-control`, which itself depends on the
  kernel. Mother uses no kernel actuator and none of the legacy stack.
- Path validation, workspace grants and the broker's process-group
  ownership are not OS sandboxes. Mother adds none and claims none.
- Linux only. Windows and macOS are not claimed.

## Run the walk

```text
cargo run -p nexus-mother --example walk
```

Each time it asks, type the first 8 characters of the contract hash shown to
consent. In run 2, answer "no" when the Deny step asks. The walk opens a
fresh home under the system's temporary directory and prints its path.
