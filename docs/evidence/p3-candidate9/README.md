# Phase Three Candidate 9: mutation controls

This directory holds the mutation controls for the repairs of Candidate 9
(`repair/p3-candidate9`, a bounded repair of Candidate 8 `7d1eaced`) and
the script that runs them. It holds no results: a run's results are
reported with the exact commit it ran on.

These are the Candidate 9 controls only. They are **not** the original
Phase Three mutation harness (183 controls, expected by the Architect),
which is not in this repository, and they do not stand in for it.

## What a control is

Each control (`scripts/c9_controls.py`, `CONTROLS`) restores one wrong
behaviour that a Candidate 9 repair removed, or, for a repaired source
guard, adds the one input the guard must refuse. It names the repair it
covers (`item`), the mission's required control it answers (`required`,
when it is one), one intended test, and a marker the test's failure must
show. A control counts as killed only if:

- every edit applies: its anchor occurs exactly once in its file (an edit
  with no anchor appends to the file);
- the control's crate builds its library's unit-test target;
- the intended test, run alone (`--exact`), fails, and its output carries
  the marker (it failed at the intended assertion, not by accident);
- the intended test passed unmutated (the baseline, run first for every
  intended test).

No control edits a test file: the tests and guards are the fixed judge.
After each control the mutated files are written back byte for byte; each
is verified by SHA-256 against its starting content, and the checkout's
Git status must equal its status at the start, or the run stops. Before
the baseline every tracked file's modification time is set to now
(content untouched), so a warm target directory cannot serve an artifact
built from other content.

## Running them

```sh
docs/evidence/p3-candidate9/scripts/c9_controls.py <checkout> --check-anchors
docs/evidence/p3-candidate9/scripts/run_controls.sh <checkout> <out> <target-dir>
```

`<checkout>` is an isolated Git checkout of the candidate, used by nothing
else during the run (never a worktree under review). The run needs the
Linux toolchain the repository pins, Xvfb and `xvfb-run` (the display and
confirmation-window tests use them), and runs with `CI=1` and no `DISPLAY`
(the wrapper sets both), so that a live test fails rather than skips when
its program is missing. `<out>/runs.txt` records the commit, its tree, the
script's SHA-256 and the checkout's status before and after;
`<out>/c9/summary.json` the baseline and every control. The exit status is
0 only for a complete run in which every intended test passed unmutated,
every control was killed at its marker, every required control has at
least one control, and every file was restored.

## Coverage

The mission's required controls and the controls that answer them:

| Required control | Repair | Controls |
|---|---|---|
| R2 wide-space/fake-header spoof | P-1 | `NC-C9-P1-WIDE-SPACE-SHOWN` |
| scroll-away of genuine target | P-1 | `NC-C9-P1-HEADER-SCROLLS-AWAY` (live, under Xvfb) |
| same-X-client drag relaxation | C8-1 | `NC-C9-C81-SAME-CLIENT` (live X) |
| audit/evidence call while authority lock is held | P-2 | `NC-C9-P2-START-RECORDED-UNDER-LOCK` |
| stale evidence reservation becoming executable | P-2 | `NC-C9-P2-STALE-RESERVATION-COMPLETES` |
| Chrome managed-policy direct mode | P-3 | `NC-C9-P3-POLICY-FILES-IGNORED` |
| local-interface/on-link global address acceptance | P-4 | `NC-C9-P4-LOCAL-BOUNDARY-SKIPPED` |
| connector destination metadata mismatch | P-5 | `NC-C9-P5-DESTINATION-NOT-RECHECKED` |
| gtk/glib spawn | P-6 | `NC-C9-P6-GLIB-SPAWN` |
| gio subprocess/opener | P-6 | `NC-C9-P6-GIO-SUBPROCESS`, `NC-C9-P6-GIO-OPENER` |
| lock call by type path | P-6b | `NC-C9-P6B-TYPE-PATH` |
| lock call through receiver alias | P-6b | `NC-C9-P6B-RECEIVER-ALIAS` |
| missing-agent fail-open | C8-3 | `NC-C9-C83-MISSING-AGENT-RUNS` |
| clear_all_agents orphaning a loop | C8-3 | `NC-C9-C83-CLEAR-ORPHANS` |
| end_goal_loop TOCTOU | C8-4 | `NC-C9-C84-COMPARED-BEFORE-THE-GUARD` |
| unlimited HiveMind starts | C8-5 | `NC-C9-C85-UNLIMITED-STARTS` |
| owner-session string capacity escalation | P-7a | `NC-C9-P7A-NAME-GETS-OWNER-PLACES` |
| private-browser-policy flip | P-7k | `NC-C9-P7K-PRIVATE-FLIP` |
| proxy stop-flag removal | P-7l | `NC-C9-P7L-STOP-FLAG-IGNORED` |

The additional controls cover the rest of each repair: the drag's bare
corner and keyboard grab (C8-2), grants, leases and the display start
recorded under a lock (P-2), the policy checks at preparation and launch
(P-3, through `p3_g9_03`), an unreadable boundary (P-4), `show_uri`, root
window access and an unclassified toolkit API (P-6), an imported type
alias and a method value (P-6b), every source of the agents that
clearing stops and a loop kept under another spelling (C8-3), the desktop's
check-then-act and an agent-wide consent wake (C8-4), racing starts, the
place given back, the rate limit, a cancelled session's sub-tasks, and the
owner's cancellation, emergency stops and quitting (C8-5), and an agent's
action in an owner run (P-7a).

The controls that need a running X server or Chrome are limited to the
display tests (Xvfb) and the confirmation window (`xvfb-run`); the browser
controls use the crate's unit tests and the source guard, not a live
browser.
