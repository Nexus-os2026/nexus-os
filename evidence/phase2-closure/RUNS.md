# Phase Two closure run ledger

Read from the GitHub Actions API for `Nexus-os2026/nexus-os` on 2026-10-04
(P2-CLOSE-F1 precheck). Every row is attempt 1, and none was re-run.
Failures and cancellations are listed, not hidden.

This ledger covers the qualification and closure path from the first live
dispatch onward. It is not a list of every repository run: earlier Phase Two
Fast Local runs on review and implementation branches are not listed here.

| Run | Workflow | Event | Attempt | Head SHA | Branch | Result | Role in closure |
|---|---|---|---|---|---|---|---|
| `37091121444` | NEXUS OS Phase Two Linux Sandbox | `workflow_dispatch` | 1 | `3d46b4dbe6b920dfcc2a1fa0c2d3b055f86c1823` | `review/p2-v1-native-scope-host-qualification` | **failure** | First live dispatch. It stopped at "Required host layers": the runner account (uid 1001) had no `/run/user/1001`, no user manager bus and no memory, pids or cpu delegation. This led to the runner's migration into the `github-runner` user manager (G-HOST). |
| `37163435032` | NEXUS OS Phase Two Linux Sandbox | `workflow_dispatch` | 1 | `d16d7ca30e9f6431c9399e6facd4bda26b4a1740` | `review/p2-v1-native-scope-host-qualification` | **failure** | Cases 01–17 passed. Case 18, `p2d_live_unmovable_process_fails_closed`, failed on the never-populated scope lifecycle, and cases 19–39 did not run. This led to the Q3 diagnosis and the Q3-R1 repair (`a7001311`). |
| `37172261810` | NEXUS OS Phase Two Linux Sandbox | `workflow_dispatch` | 1 | `a7001311221023d37d73e6355ec0c0f5020a32a8` | `review/p2-v1-native-scope-host-qualification` | success | Accepted live qualification of `a7001311`: 39/39 live cases, H1–H7, pre- and post-cleanup observations (G-LIVE). |
| `37191730167` | NEXUS OS CI | `workflow_dispatch` | 1 | `a7001311221023d37d73e6355ec0c0f5020a32a8` | `review/p2-v1-native-scope-host-qualification` | **failure** | Hosted closure CI of `a7001311`. `security-audit-linux` failed: the fresh RustSec database reported RUSTSEC-2026-0327 (wasmtime 43.0.2, critical, published 2026-10-02). `test-linux`, `test-frontend` and `test-python` succeeded. This led to the R1 security repair. |
| `37196418437` | NEXUS OS Fast Local CI | `push` | 1 | `dc52fadba078f2cddd6b51e26dd1a38db1ffedcc` | `repair/p2-security-wasmtime-36` | **cancelled** | Triggered automatically by the R1 push (`repair/**`). Cancelled by the executor before its Linux job started, because it would have run the live suite on the host without authorization. A disclosed process deviation; it gives no result. |
| `37198034561` | NEXUS OS CI | `workflow_dispatch` | 1 | `dc52fadba078f2cddd6b51e26dd1a38db1ffedcc` | `repair/p2-security-wasmtime-36` | success | **Exact-SHA hosted closure CI** of the security-repair checkpoint: `test-linux`, `test-frontend`, `test-python` and `security-audit-linux` all succeeded. |
| `37201338898` | NEXUS OS Phase Two Linux Sandbox | `workflow_dispatch` | 1 | `dc52fadba078f2cddd6b51e26dd1a38db1ffedcc` | `repair/p2-security-wasmtime-36` | success | **Exact-SHA live qualification** of the checkpoint on the supported host (runner 21): 39 expected and 39 observed, none missing or unexpected, case 18 passed, H1–H7 passed, pre- and post-cleanup observations clean, kernel 26 and desktop 32 controls passed. |
| `37203318840` | NEXUS OS Fast Local CI | `push` | 1 | `dc52fadba078f2cddd6b51e26dd1a38db1ffedcc` | `implement/phase2-governed-verification` | success | **Automatic post-integration run** after the P2-CLOSE-I1 fast-forward of the authoritative branch (`4d6763a8` → `dc52fadb`): `fast-linux`, `fast-python` and `fast-frontend` all succeeded. |

## Timing (UTC, 2026-10-04 unless noted)

| Run | Created | Completed |
|---|---|---|
| `37091121444` | 2026-10-03 02:49:21 | 2026-10-03 02:49:34 |
| `37163435032` | 2026-10-03 23:58:10 | 00:01:54 |
| `37172261810` | 02:50:13 | 02:57:41 |
| `37191730167` | 09:18:31 | 09:45:35 |
| `37196418437` | 10:45:45 | 10:46:52 |
| `37198034561` | 11:14:05 | 11:42:54 |
| `37201338898` | 12:12:59 | 12:20:04 |
| `37203318840` | 12:47:21 | 13:05:19 |

This directory records run identities, not run logs. The executor's
collected copies of the logs for `37198034561`, `37201338898` and
`37203318840` were checksummed in work directories outside the repository;
they are not part of this evidence. No run listed here was dispatched or
re-run for this record.
