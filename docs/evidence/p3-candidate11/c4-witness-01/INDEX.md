# P3-C11-DR01-D2A1-C4-WITNESS-01 — review packet

**Exploratory witness work. No mutation kill, contract adoption, component acceptance, integration or phase change.** Prepared by the executor (Claude Code, Claude Opus 5.5) for the Sole Architect's review.

## Identities

| Item | Identity |
|---|---|
| Source commit A (unaccepted witness prototype) | `04e42a7ddf0d0a2af17b6506c51997006e7a08dc` tree `fca5c2a28e1c6cfef3cd92fbb81484a56769a9b3` parent `42dd1bd1cf21ffdea5968b256d2983f6575fab5b` |
| `model_tests.rs` at A | blob `28085ce232827301104b5d757af042ec933976f8` (append-only over `5ad26ddc`) |
| `protocol.rs` at A | blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (unchanged) |
| Engineering base | `42dd1bd1cf21ffdea5968b256d2983f6575fab5b` |
| Supplemented draft | AD-1-R3 blob `7f2463f5…`, sha256 `485d9256932220644d7368d64b7b9b4d8df8a3a5c65aba4a1c5fcdf390ae286e` |
| Disposition sheet | `disposition/AD1R3-DISPOSITION-01-DRAFT.md` sha256 `a8a6d29376cf6f393a7eaa9047923719a339377caa554527a8878760cbdcf11c` |
| C4 patch | `patches/C4-test1-removed.patch` sha256 `24ac7add3716a030dd710539e6082e197fdc5bd624c26a595b378b3971e0838d`; mutant `protocol.rs` blob `f3793c2ff61f8adbf43fe17d6b5237b49bc507b0` |
| Healthy executable | sha256 `f72a09a338df176121e69bda8cba8bb30ff11b4ef9666a9a796cf77c6a52f62e` (8984464 bytes) |
| Mutant executable | sha256 `5a4c4f17ea5b3883fb87a1a16fe19d53810608d4c229548a37932c405aa1fe8c` (8988320 bytes); never committed |

## Results

- Gate: GATE RESULT: BOTH GATES PASSED (healthy PASS; mutant P4 and P8 OBSERVED); mutant restored to A
- Healthy build: receipt 08-build-healthy type build role baseline: BUILD OK; wall 167.92000000000002 s; memory.peak 2610831360 B; swap peak 0 B
- Healthy witness: receipt 09-run-healthy type c4-healthy-witness role baseline: PASS; exit 0; wall 0.0 s
- Mutant build: receipt 11-build-mutant type build role mutant: BUILD OK; wall 167.54 s; memory.peak 2613620736 B
- Mutant witness: receipt 12-run-mutant type c4-mutant-witness role mutant: POSITIVE: P4 AND P8 OBSERVED; exit 101; wall 0.0 s
- Restoration: RESULT: every file equals A by blob; the worktree is clean at A (kept; its target/ holds the mutant executable, never committed)

## Where to look

- `disposition/`: the short disposition sheet (R3-01, R3-02, the C4 route, C9-b, C10/C24).
- `frozen/`: the frozen schedules and the freeze record. `patches/`: the C4 patch only.
- `receipts/09-run-healthy/raw-output.txt` and `receipts/12-run-mutant/raw-output.txt`: both full replay outputs.
- `receipts/*/RECEIPT.txt`, `scope-record.txt`, `time-v.txt`, `descriptor.json`, `CLASSIFICATION.txt`: commands, identities, exit or signal, resource limits and peaks, and the classification of each receipt.
- `tools/`: the complete runner, classifier, checker, supervisor and packet code; `tools/revisions/` and `receipts/revisions/`: development failures and their revisions, kept.

Excluded by design: the applied mutant source (only its patch travels), every executable and build artefact.
The post-publication read-back is kept outside this commit.

## Files (128 besides this index and the manifest)

- `disposition/AD1R3-DISPOSITION-01-DRAFT.md`
- `frozen/FREEZE-SHA256SUMS`
- `frozen/FREEZE.txt`
- `frozen/c4-schedule.json`
- `patches/C4-test1-removed.patch`
- `receipts/00-STAGE1-AUTHENTICATION.txt`
- `receipts/01-worktree-add.txt`
- `receipts/02-c4-patch-derivation.txt`
- `receipts/03-classifier-synthetic-cases.txt`
- `receipts/04-static-checks-of-A.txt`
- `receipts/05-supervision-selftest/1/cgroup-samples.txt`
- `receipts/05-supervision-selftest/1/out.txt`
- `receipts/05-supervision-selftest/1/scope-record.txt`
- `receipts/05-supervision-selftest/1/time-v.txt`
- `receipts/05-supervision-selftest/2/scope-record.txt`
- `receipts/05-supervision-selftest/5/cgroup-samples.txt`
- `receipts/05-supervision-selftest/5/out.txt`
- `receipts/05-supervision-selftest/5/scope-record.txt`
- `receipts/05-supervision-selftest/5/time-v.txt`
- `receipts/05-supervision-selftest/6/cgroup-samples.txt`
- `receipts/05-supervision-selftest/6/out.txt`
- `receipts/05-supervision-selftest/6/scope-record.txt`
- `receipts/05-supervision-selftest/6/time-v.txt`
- `receipts/05-supervision-selftest/SELFTEST.txt`
- `receipts/06-delegated-readonly-review.txt`
- `receipts/07-commit-A.txt`
- `receipts/08-build-healthy/CLASSIFICATION.txt`
- `receipts/08-build-healthy/RECEIPT.txt`
- `receipts/08-build-healthy/build.json`
- `receipts/08-build-healthy/build.stderr`
- `receipts/08-build-healthy/cgroup-samples.txt`
- `receipts/08-build-healthy/descriptor.json`
- `receipts/08-build-healthy/pins.json`
- `receipts/08-build-healthy/scope-record.txt`
- `receipts/08-build-healthy/time-v.txt`
- `receipts/09-run-healthy/CLASSIFICATION.txt`
- `receipts/09-run-healthy/RECEIPT.txt`
- `receipts/09-run-healthy/cgroup-samples.txt`
- `receipts/09-run-healthy/descriptor.json`
- `receipts/09-run-healthy/pins.json`
- `receipts/09-run-healthy/raw-output.txt`
- `receipts/09-run-healthy/scope-record.txt`
- `receipts/09-run-healthy/time-v.txt`
- `receipts/10-mutant-apply.txt`
- `receipts/11-build-mutant/CLASSIFICATION.txt`
- `receipts/11-build-mutant/RECEIPT.txt`
- `receipts/11-build-mutant/build.json`
- `receipts/11-build-mutant/build.stderr`
- `receipts/11-build-mutant/cgroup-samples.txt`
- `receipts/11-build-mutant/descriptor.json`
- `receipts/11-build-mutant/pins.json`
- `receipts/11-build-mutant/scope-record.txt`
- `receipts/11-build-mutant/time-v.txt`
- `receipts/12-run-mutant/CLASSIFICATION.txt`
- `receipts/12-run-mutant/RECEIPT.txt`
- `receipts/12-run-mutant/cgroup-samples.txt`
- `receipts/12-run-mutant/descriptor.json`
- `receipts/12-run-mutant/pins.json`
- `receipts/12-run-mutant/raw-output.txt`
- `receipts/12-run-mutant/scope-record.txt`
- `receipts/12-run-mutant/time-v.txt`
- `receipts/13-mutant-restore.txt`
- `receipts/20-GATE-SUMMARY.txt`
- `receipts/gate-console.txt`
- `receipts/github-refs-before.txt`
- `receipts/pins-final.json`
- `receipts/revisions/02-c4-patch-derivation.v1.txt`
- `receipts/revisions/04-check-schedule-negative-controls.v1.txt`
- `receipts/revisions/05-supervision-selftest.v1/1/cgroup-samples.txt`
- `receipts/revisions/05-supervision-selftest.v1/1/out.txt`
- `receipts/revisions/05-supervision-selftest.v1/1/scope-record.txt`
- `receipts/revisions/05-supervision-selftest.v1/1/time-v.txt`
- `receipts/revisions/05-supervision-selftest.v1/2/scope-record.txt`
- `receipts/revisions/05-supervision-selftest.v1/SELFTEST.txt`
- `receipts/revisions/05-supervision-selftest.v2/1/cgroup-samples.txt`
- `receipts/revisions/05-supervision-selftest.v2/1/out.txt`
- `receipts/revisions/05-supervision-selftest.v2/1/scope-record.txt`
- `receipts/revisions/05-supervision-selftest.v2/1/time-v.txt`
- `receipts/revisions/05-supervision-selftest.v2/2/scope-record.txt`
- `receipts/revisions/05-supervision-selftest.v2/5/cgroup-samples.txt`
- `receipts/revisions/05-supervision-selftest.v2/5/out.txt`
- `receipts/revisions/05-supervision-selftest.v2/5/scope-record.txt`
- `receipts/revisions/05-supervision-selftest.v2/5/time-v.txt`
- `receipts/revisions/05-supervision-selftest.v2/6/cgroup-samples.txt`
- `receipts/revisions/05-supervision-selftest.v2/6/out.txt`
- `receipts/revisions/05-supervision-selftest.v2/6/scope-record.txt`
- `receipts/revisions/05-supervision-selftest.v2/6/time-v.txt`
- `receipts/revisions/05-supervision-selftest.v2/SELFTEST.txt`
- `receipts/revisions/05-supervision-selftest.v3/1/cgroup-samples.txt`
- `receipts/revisions/05-supervision-selftest.v3/1/out.txt`
- `receipts/revisions/05-supervision-selftest.v3/1/scope-record.txt`
- `receipts/revisions/05-supervision-selftest.v3/1/time-v.txt`
- `receipts/revisions/05-supervision-selftest.v3/2/scope-record.txt`
- `receipts/revisions/05-supervision-selftest.v3/5/cgroup-samples.txt`
- `receipts/revisions/05-supervision-selftest.v3/5/out.txt`
- `receipts/revisions/05-supervision-selftest.v3/5/scope-record.txt`
- `receipts/revisions/05-supervision-selftest.v3/5/time-v.txt`
- `receipts/revisions/05-supervision-selftest.v3/6/cgroup-samples.txt`
- `receipts/revisions/05-supervision-selftest.v3/6/out.txt`
- `receipts/revisions/05-supervision-selftest.v3/6/scope-record.txt`
- `receipts/revisions/05-supervision-selftest.v3/6/time-v.txt`
- `receipts/revisions/05-supervision-selftest.v3/SELFTEST.txt`
- `receipts/revisions/07-commit-A.v1.txt`
- `receipts/revisions/21-packet-build.v1.txt`
- `receipts/revisions/README-make-c4-patch.txt`
- `receipts/revisions/README-packet-build.txt`
- `tools/build-packet.py`
- `tools/c4-mutant.py`
- `tools/check-schedule-negative-controls.sh`
- `tools/check-schedule.py`
- `tools/classifier-cases.py`
- `tools/classify.py`
- `tools/gate.py`
- `tools/make-c4-patch.py`
- `tools/revisions/README-c4-append.txt`
- `tools/revisions/README-check-schedule-negative-controls.txt`
- `tools/revisions/README-check-schedule.txt`
- `tools/revisions/README-supervise.txt`
- `tools/revisions/c4-append.v1.rs.txt`
- `tools/revisions/check-schedule-negative-controls.v1.sh`
- `tools/revisions/check-schedule.v1.py`
- `tools/revisions/make-c4-patch.v1.py`
- `tools/revisions/supervise.v1.py`
- `tools/revisions/supervise.v2.py`
- `tools/revisions/supervise.v3.py`
- `tools/scope-wrap.sh`
- `tools/stage1-auth.sh`
- `tools/supervise.py`
