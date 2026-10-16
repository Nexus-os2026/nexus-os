# EXEC-01-OPUS-R1 durable progress record
Mission: P3-C11-DR01-D2A1-C7-W-EVIDENCE-EXEC-01-OPUS-R1 (supersedes Sonnet executor; original EXEC-01 limits binding)
Session: f4afe1e2-e1f5-4b1a-9e7c-c71e0c63e031  (prior tooling session: 7b42496f-...)
OLD_SP=/tmp/claude-1000/-home-nexus-NEXUS-nexus-os/7b42496f-8d31-44a9-b5c7-079991353147/scratchpad

## Stage log
- [S0] Model gate: system id claude-opus-5-5; env CLAUDE_EFFORT=xhigh. No subagents (env CLAUDE_CODE_SUBAGENT_MODEL=opus-4-7 -> never use Agent).
- [S0] Main repo HEAD f727f5c39fab8d5c729a55eb28ad576d3d56ce47 on rebuild/phase0-trust-boundary, clean.
- [S1] OLD_SP authenticated: dirs 700 nexus, no symlinks/hardlinks/foreign/special files. 14/14 handoff sha256 OK.
  - DISCREPANCY: mission.txt is at OLD_SP/mission.txt, NOT OLD_SP/c7tools/mission.txt (absent). Hash matches beba3d57...
  - gate-run2.txt is gateout/gate-run2.txt; gate-run2.stdout byte-identical.
  - c7tools/__pycache__ exists -> run python with fresh PYTHONPYCACHEPREFIX to avoid stale pyc.
- [S2] Independent state check (00:2x): WT_HEALTHY, WT_MUTANT, EV_DIR absent; no cargo/rustc/test procs; no active p3c11/c4w01/fbk scope (2 old c4w01 scopes 'failed'); mutant blob 0d4ab992 absent; retained C7 worktrees present (witness-source-w1-01 @W, mt3c-C7 locked @C-BASE).
- [S3] Read all 9 helper sources + selftests. FROZEN: chmod a-w on c7tools/*.py, scope-wrap.sh, mission.txt, gate-run1/2; manifest MY/FROZEN-TOOLS.sha256.txt.
  - Run python as: PYTHONPYCACHEPREFIX=MY/pycache-empty PYTHONDONTWRITEBYTECODE=1 /usr/bin/python3 -B
  - c7sup preflight does NOT enforce 8 GiB disk floor -> check df avail >= 8 GiB myself before each build/major write.
  - Supervisor helper has 2 existing selftest entry points: selftest_sup.py (build/invoke; listed by hash) and `c7sup.py selftest` (enforcement; never run before). Decision: run each exactly once (fail-closed: extra checks can only block). Report transparently.
- [S4] K2 read: 6A, 7.1-7.5, 7.3A, 8.1/8.2, 10, 13.3, G.7.
- [S5] STOP (00:2x): gate-run3 invocation DENIED by Claude Code auto-mode classifier ("Code from External": helper scripts live in prior session's /tmp scratchpad). Nothing executed: no preflight dir, no gate-run3, no self-test, no worktree, no build, no evidence dir. Status: STOPPED BEFORE EXECUTION. Resume requires Owner to permit executing OLD_SP/c7tools/*.py (Bash permission rule), then a new start at [S5] = gate-run3 (still never run).
