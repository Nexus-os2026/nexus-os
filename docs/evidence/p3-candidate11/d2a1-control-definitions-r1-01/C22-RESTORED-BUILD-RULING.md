# C22 — restored-build ruling record (mission section 7)

Mission P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01. Executor: Claude Code, Claude Opus 5.5. A record of the Architect's ruling as applied to the candidate catalog. Nothing was built.

## The ruling (mission section 7, as received)

> The Stage-1 healthy build does NOT satisfy C22's final type-(e4) restored receipt because its command omitted `-j 2`.

## The Stage-1 healthy build, as recorded

- receipt: `docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/10-build/RECEIPT.txt` at Stage-1 evidence commit `f023cc3d031a2a89e5ecbd4086e15f2e50869acd` (blob `723f9eb852f53f17a60c11cec521b78b96c53f11`); pin `PIN.json` blob `688fa49b494ab1c4982422f63b93bd87b853674a`, pinned executable sha256 `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c`
- its recorded command line(s), read from that receipt now:

  - `command: systemd-run --user --scope --unit=p3c11-fbk01-build-1791979682 -p MemoryMax=16G -p MemorySwapMax=0 -p RuntimeMaxSec=910 --quiet /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-final-baseline-known-runs-01/tools/scope-wrap.sh /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-final-baseline-known-runs-01/receipts/10-build/scope-record.txt /usr/bin/env -i HOME=/home/nexus PATH=/home/nexus/.cargo/bin:/usr/local/bin:/usr/bin:/bin CARGO_TARGET_DIR=/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01/target CARGO_TERM_VERBOSE=true CARGO_NET_OFFLINE=true /usr/bin/time -v -o /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-final-baseline-known-runs-01/receipts/10-build/time-v.txt timeout -s KILL 900 nice -n 10 cargo test -p nexus-governed-control --locked --release --lib --no-run`

No `-j 2` appears in it (both static checkers re-read the receipt and check this).

## The candidate catalog's record

```json
{
 "identical_between_healthy_and_mutant": [
  "command",
  "toolchain",
  "environment",
  "target policy",
  "features/cfg/RUSTFLAGS",
  "supervision/resource limits"
 ],
 "intended_difference": "source identity only (Candidate C's protocol.rs versus the C22 mutant)",
 "not_run_in_this_mission": true,
 "required_future_healthy_command": "cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2",
 "required_future_mutant_command": "cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2",
 "restored_receipt_status": "PENDING_EXACT_COMMAND_HEALTHY_BUILD",
 "ruling": "mission section 7: the Stage-1 healthy build does NOT satisfy C22's final type (e4) restored receipt because its command omitted -j 2",
 "type": "e4"
}
```

The Stage-1 build stays in the catalog only under `historical_stage1_build_not_the_e4_receipt`; it is not C22's restored receipt.

## What a future mission must produce (not done here)

- a healthy build of Candidate C by exactly `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2`;
- C22's mutant build by exactly `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2`;
- identical between the two: command; toolchain; environment; target policy; features/cfg/RUSTFLAGS; supervision/resource limits;
- the only intended difference: source identity only (Candidate C's protocol.rs versus the C22 mutant).

C22's mutant failing receipt stays type (d): the intentional compile failure, `error[E0283]` at crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:1089 (contract 7.5 line 731; AD-1-R4 F.1(d)).

Status: **PENDING_EXACT_COMMAND_HEALTHY_BUILD**. Neither build ran in this mission.
