# EXEC-01-OPUS-R3 progress (executor: Claude Opus 5.5, session 882e437e-0db3-409e-8b8f-72205c69d44d)

Mission text: r3/MISSION-R3.txt. Original EXEC-01: preflight/sonnet-7b42496f/mission.txt (sha256 beba3d57...).
Frozen helpers are run ONLY from their authenticated originals:
  /tmp/claude-1000/-home-nexus-NEXUS-nexus-os/7b42496f-8d31-44a9-b5c7-079991353147/scratchpad/c7tools
  (hash-checked against preflight-r2/TOOLS.sha256 by r3/cmd/00-gates.sh before every stage). Copies under preflight/ are archive only.
Every stage = one file r3/cmd/NN-*.sh, executed verbatim with `bash`; console in r3/console/, gate record in r3/gates/.

## Log
- 00:50 State: main repo f727f5c3 clean; destinations absent; no cargo/rustc/test process; no active p3c11 scope; disk avail 14.09e9 B.
- 00:5x 4A-C: TOOLS+FIXTURES 26/26 OK; R2 MANIFEST 34/35 (the 35th is its self-entry = sha256 of its first 7 lines, unverifiable by construction);
  R1 frozen manifest 34/34; expected.txt 14/14; modes 12/12 same; all frozen git identities OK; mutant blob absent; protected refs unchanged.
- 00:52 4D: EV dir created, mode 700, ino 54395292. 4E-G: three scratchpads copied, byte-identical (r3/01-PRESERVATION.txt), sealed a-w; originals intact.
- 00:56 10-create-H: v1 gate FAILED CLOSED (pgrep -f matched stale Oct-13 bash wait-loop PID 2306315, not a test process; not ours; not touched). Nothing started.
  Gate v2 (process-name match) proven against decoys; v1 preserved as 00-gates.v1.sh.
- 00:58 10b-create-H: healthy worktree created; EXACTLY_W OK (13/13).
- 00:58 11-build-E-W: started (background).
- 01:01 11-build-E-W: exit 0, 2m49s, peak 2616541184 B, swap 0, oom 0, 0 warnings/errors. Disk after 13137137664 B.
- 01:01 12-pin-E-W: PINNED .../nexus-os-p3-c11-c7w-healthy/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 042b11e69964df40487a93f307900499a8c0253e24abe4ea598b11b53871c825 size 8995112 (21/21 PASS).
- 01:01 13-create-M: mutant worktree created; EXACTLY_W OK.
- 01:02 14-apply-C7: C7.patch (b5d8bc6c...) applied once; EXACTLY_W7 OK (protocol.rs 0d4ab992 / 6d2f2d49; unstaged; mutant blob absent). FROM HERE: any stop -> r3/cmd/24-restore.sh (7.3A).
- 01:02 15-build-E-W7: started (background).
- 01:04 15-build-E-W7: exit 0, 2m48s, peak 2611060736 B, swap 0, oom 0, 0 warnings/errors. Disk after 12252008448 B.
- 01:05 16-pin-E-W7: PINNED .../nexus-os-p3-c11-c7w-mutant/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3 sha256 aa0c0577ebbd651dbcc6a7a27849cb6a2d1f7004f87732a59f3a123fda9b1223 size 8995176 (21/21 PASS).
- 01:0x 17/18 listings: 8+8 supervised listings ok.
- 01:0x 19-bwc-check: B-W/C MECHANICALLY SATISFIED (276 PASS / 0 FAIL); receipts/30-bwc/B-W-C.{txt,json}.
- next: 20-run-C (background).
- 01:0x 20-run-C: (C) PASS_ADMISSIBLE (34/0); exit 0; 0 VIOLATION; healthy step-23 end; kept predicate holds. next: 21-run-A.
- 01:06 21-run-A: (A) FAIL_E11_ADMISSIBLE (24/0); exit 101; states 1071165; 4.65 s; two byte-equal instances. next: 22-run-B.
- 01:07 22-run-B: (B) ADMISSIBLE_TWO_CANONICAL (47/0); exit 101; 4 VIOLATION raw lines = E11@13-transition + P6@after-24 + 2 summary copies; G.9 cross-check C vs B identical except step 23. next: 23-run-F3.
- 01:08 23-run-F3: INFORMATIONAL F-3 COMPLETED: FAIL E11 (as expected) (21/0); exit 101; states 930702; 4.11 s. next: 24-restore.
- 01:08 24-restore: preserved; R-1..R-8 OK; git apply -R rc 0; RESTORATION COMPLETE: EXACTLY_W; E-W7 sha MATCH; healthy EXACTLY_W OK. next: 25-link.
- 01:1x local result KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING recorded (r3/90-LOCAL-CLASSIFICATION.txt). next: publication (30 stage, 31 commit, 32 precheck, 33 push, 34 readback).
