# Stage 4 — resource and process receipts

All new executions of this mission, in order. Limits were defined before each invocation and read back from the
kernel; they are measurements, not requests. Nothing was retried automatically.

## 1. Supervision chain

`tools/s4_launch.py` (frozen: `receipts/40-STAGE4-TOOL-FREEZE.txt`; revisions R1 and R2 recorded in
`receipts/40b-…` and `receipts/40c-…`, diffs in `development/`). It reuses VALIDATION-02's `scope-wrap.sh` and
`verify_tree.py` unchanged (`receipts/40-REUSED-TOOLS-PROVENANCE.txt`). Each expensive command ran in a transient
`systemd-run --user --scope` with `MemoryMax=16G`, `MemorySwapMax=0` and `RuntimeMaxSec=limit+10`, inside
`env -i`, `/usr/bin/time -v`, `timeout -s KILL <limit>` and `nice -n 10`; a 2 s memory sampler and a 2 s disk monitor
ran beside it. Disk rule: at 17 GiB available, a controlled stop of this mission's scope only (SIGTERM, SIGKILL after
15 s); below 16 GiB, SIGKILL at once. No other process could be signalled.

## 2. Receipts

| Receipt | What | Result | Key measurements |
|---|---|---|---|
| `41-supervision-selftest` | self-test v1, non-Nexus processes only | **FAILED (preserved)**: test 6's predicate expected a `/usr/bin/time` signal record, which cannot exist when the whole scope is SIGTERMed. The guard itself triggered on its first sample and stopped the 40 s sleep in about 1 s. | tests 1–5 OK |
| `40b-STAGE4-TOOL-REVISION-R1.txt` | predicate fix only | enforcement code unchanged | diff `development/s4_launch.v1-to-r1.diff` |
| `41b-supervision-selftest-r1` | self-test R1 | **PASS 6/6**: 16 GiB and zero swap read back inside the scope, exact `env -i` environment; a 64 MiB scope OOM-killed a 256 MiB writer; `timeout -s KILL` and the RuntimeMaxSec backstop enforced; exit 101 passed through; the disk guard stopped its own scope | — |
| `42-worktree` | `git worktree add --detach /home/nexus/NEXUS/nexus-os-p3-c11-d2a1-final-closure-01 d5964ee5…` and the workflow's verification commands | **PASS 24/24**; byte-exact C (5,364 blobs, 97,246,341 bytes) | path absent beforehand |
| `43-clippy-all-features` | first Clippy launch | **NOT_STARTED (preserved)**: the one-invocation gate counted the shell-redirect console file as an earlier receipt. No scope was launched; no cargo process ran; no raw output exists. | — |
| `40c-STAGE4-TOOL-REVISION-R2.txt` | gate fix only (directories only; an earlier directory admissible only if NOT_STARTED with no raw output) | — | diff `development/s4_launch.r1-to-r2.diff` |
| `43b-clippy-all-features-r2` | **the one** `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | **PASS 18/18** | wall **372.12 s** of 2,700 s; scope `memory.peak` **6,374,899,712 B** of 17,179,869,184 B; `memory.swap.peak` 0; memory.events max 0, oom 0, oom_kill 0; largest process max RSS 2,063,940 KiB; disk start 26,543,030,272 B, **minimum 24,145,653,760 B** (22.49 GiB, above the 17 GiB trigger), consumed 2,397,376,512 B; target 2,390,077,440 B; 186 disk samples; no controlled stop |

Read-only commands outside scopes (no build, no test): `git`, `sha256sum`, `du`, `df`, `free`, `ps`, `systemctl show`,
version queries of local tools, `cargo metadata --offline --locked` in the mission worktree (writes nothing to the
target), and this mission's Python checkers.

## 3. Host state

| Item | Before (Stage 0, 14:50Z) | After the Clippy run (15:18Z) |
|---|---|---|
| root filesystem available | 26,669,228,032 B (24.84 GiB) | 24,145,510,400 B (22.49 GiB) |
| 16 GiB floor preserved | yes | yes (no stop needed) |
| MemAvailable | 52,324,184 KiB | 52,152,456 KiB at the Clippy gate |
| host swap in use | 8.1 GiB (pre-existing, not this mission's) | unchanged by this mission; every mission scope had MemorySwapMax=0 and swap peak 0 |
| GitHub runner | inactive, disabled, MainPID 0 | unchanged |
| conflicting cargo/rustc/clippy/test processes | none | none |
| unrelated processes | two leftover PIDs (2978429 `tail -f`; 2306315 a wait loop) and another session's Node processes | left as found; nothing was signalled |

## 4. Retained

The mission worktree `/home/nexus/NEXUS/nexus-os-p3-c11-d2a1-final-closure-01` (detached at C, tracked bytes exact;
ignored `target/` of 2,390,077,440 B and the generated `app/src-tauri/permissions/`) is retained, as the mission
forbids destructive cleanup. Removing it later is an Owner decision.
