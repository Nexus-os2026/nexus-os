# FG1 post-integration evidence

Status: evidence recorded for Architect review. FG1 is not declared complete.

## Integration

- `rebuild/phase0-trust-boundary`: `98fbb6a369daa9f3f3998a8555fec7cf67078664`
  -> `71c47acbf3f8ee8210109587b3229f8d89067b6b` by `git merge --ff-only`,
  pushed normally to GitHub at 2026-09-28T16:36:17Z.
- Local and GitHub authoritative head `71c47acb…`, tree
  `824669d4bea9df5a24e8065081fedb109f0fd66f`.
- `main`, FG1, CI, composition and repair branches unchanged; PR #15 unchanged
  (open, draft, unmerged, auto-merge off).
- The push triggered no workflow.

## The single post-integration dispatch

- Command: `gh workflow run ci.yml --repo Nexus-os2026/nexus-os --ref rebuild/phase0-trust-boundary --raw-field candidate_sha=71c47acbf3f8ee8210109587b3229f8d89067b6b`
- Sent once at 2026-09-28T16:38:33.967Z, exit 0. Before it: no `ci.yml` run
  existed for this branch and SHA.
- Run: [36452512718 (#108)](https://github.com/Nexus-os2026/nexus-os/actions/runs/36452512718),
  attempt 1, `workflow_dispatch`, head `71c47acb…`, created 16:38:35Z,
  completed 17:02:57Z (24m 22s), **success**.

## Identity (native logs, every job)

Preflight passed before checkout; `RAW_CANDIDATE_SHA`, validated candidate,
checked-out HEAD and tested commit are `71c47acb…`; workflow definition
`Nexus-os2026/nexus-os/.github/workflows/ci.yml@refs/heads/rebuild/phase0-trust-boundary at 71c47acb…`.
The tested commit's tree is `824669d4…`, the authoritative tree.

## Jobs (GitHub-hosted)

| Job | Job ID | Image | Result | Duration |
|---|---|---|---|---|
| test-linux | 109030528792 | ubuntu-24.04 20260920.314.1 | success | 21m 11s |
| test-windows | 109030528145 | windows-2025-vs2026 20260922.246.2 | success | 24m 17s |
| test-macos | 109030528716 | macos-26-arm64 20260907.0351.1 | success | 9m 35s |
| test-frontend | 109030528581 | ubuntu-24.04 | success | 1m 54s |
| test-python | 109030529154 | ubuntu-24.04 | success | 2m 11s |

## Measured results

| `cargo test --workspace --locked` | Executables | Doctests | Workspace total | Packaged gate (separate) |
|---|---|---|---|---|
| Linux | 217: 7,399 / 0 / 41 ignored | 68: 10 / 0 / 2 | 285: 7,409 passed, 0 failed, 43 ignored, 0 filtered | 13/0 (333 filtered) |
| Windows | 217: 7,353 / 0 / 39 | 68: 10 / 0 / 2 | 285: 7,363 / 0 / 41, 0 filtered | 13/0 (331 filtered) |
| macOS | 217: 7,396 / 0 / 38 | 68: 10 / 0 / 2 | 285: 7,406 / 0 / 40, 0 filtered | 13/0 (333 filtered) |

On all three: `binary_target_identity` 1/1, `phase0_withdrawal` 10/10,
`report_format` 6/6, R1 shared-state 8/8, C5C 47/47, surface 25/25; target
Builder assembly; trusted-entry JS 21/21; packaged assertion executed. Linux
fmt and full Clippy (`-D warnings`) clean. Whole-log searches: 0
`output filename collision`, 0 multiple-build-target, 0 linker errors.
Frontend: 103/103 files, 460/460 tests, build ok; npm audit reports 14
vulnerabilities (2 low, 6 moderate, 6 high), tracked under DEP.
Python: 18 voice tests OK; no CUDA mask in the hosted job.
Warnings: Node.js 20 action deprecation (all jobs); platform dead-code
warnings in Windows/macOS test builds (unchanged since #106).
