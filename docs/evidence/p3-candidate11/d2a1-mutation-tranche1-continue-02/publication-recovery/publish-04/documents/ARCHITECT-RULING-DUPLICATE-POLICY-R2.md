DUPLICATE POLICY R2 — 21 EXACT NONEMPTY DUPLICATES AND 8 EXACT ZERO-LENGTH ARTIFACTS PERMITTED FOR PUBLICATION (NO SIZE THRESHOLD; NO WILDCARD; CANDIDATE ACCEPTANCE PENDING)

# Architect ruling record — duplicate policy R2 (CONTINUE-02 publication)

Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, from sections 1, 6, 7 and 8 of that mission as issued by the Sole Nexus OS Architect (ChatGPT GPT-6) via the Owner.

This file is evidence of the Architect's ruling.
A string is not authority.

## Background

- P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02 completed every authorized control execution and stopped when its frozen packet builder rejected seven byte-identical link receipts (`publication/10-build-packet.txt`, `receipts/80-STOP-RECORD.md`).
- P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-03 (ruling R1, sha256 `e826ff824c00660c200b7a96ab0a9c6c0ea3b9c03614a009f6806deb2f750ae9`) permitted exactly those seven link receipts. Its repaired builder removed the historical 1-KiB duplicate-content threshold, found fourteen further nonempty duplicate constant records, and correctly returned BLOCKED (its `receipts/80-STOP-RECORD.md`).

## The ruling (mission section 1)

> A. The old 1-KiB threshold is rejected. Every nonempty staged file must be checked for duplicate content, regardless of size.
>
> B. Exactly twenty-one nonempty duplicate files may be published: the seven previously approved link receipts; the fourteen deterministic constant records defined in section 6.
>
> C. Exactly eight expected zero-length artifacts may be published through the exact path-and-role policy in section 7. Zero-length artifacts are not accepted merely because their bytes equal another empty file. Their provenance comes from exact path, role and producing command/descriptor evidence.
>
> D. Every other duplicate or zero-length artifact remains fail-closed.
>
> E. No mutation control, build, test, explorer, witness or executable may be run.
>
> F. Candidate evidence remains pending independent Architect review: C28, C12, C9, C11, C17, C18a, C18b. Do not relabel any candidate as accepted.

## 1. The seven link receipts (ruling R1, carried)

| # | current path | control | canonical prior (EXEC-01 `19eda8cc`) |
|---|---|---|---|
| 1 | `receipts/38-C9/06-link/LINK.json` | C9 | EXEC-01 `receipts/04-rehearsal/links/link-C9-stage2/LINK.json` |
| 2 | `receipts/38-C9/06-link/LINK.txt` | C9 | EXEC-01 `receipts/04-rehearsal/links/link-C9-stage2/LINK.txt` |
| 3 | `receipts/39-C11/06-link/LINK.json` | C11 | EXEC-01 `receipts/04-rehearsal/links/link-C9-stage2/LINK.json` |
| 4 | `receipts/40-C17/06-link/LINK.json` | C17 | EXEC-01 `receipts/04-rehearsal/links/link-C9-stage2/LINK.json` |
| 5 | `receipts/41-C18a/06-link/LINK.json` | C18a | EXEC-01 `receipts/04-rehearsal/links/link-C18a-stage2/LINK.json` |
| 6 | `receipts/41-C18a/06-link/LINK.txt` | C18a | EXEC-01 `receipts/04-rehearsal/links/link-C18a-stage2/LINK.txt` |
| 7 | `receipts/42-C18b/06-link/LINK.json` | C18b | EXEC-01 `receipts/04-rehearsal/links/link-C18a-stage2/LINK.json` |

Every R1 map field (`DUPLICATE-CONTENT-MAP.tsv`, 19 fields per row) is carried into the R2 map and verified again by the builder; the R1 map itself is read from the read-only PUBLISH-03 root and its sha256 is pinned (`8842a5e5a5e082ae0676adef5b83ac3743154bef7ace254f15cf8f15df579dd5`).

## 2. The fourteen deterministic constant records

| # | class | current path | identity | bytes | literal content | canonical prior |
|---|---|---|---|---|---|---|
| 1 | CGROUP_CONSTANT | `receipts/01-supervision-selftest/1/cgroup-samples.txt` | self-test 1 | 95 | `sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n` | RESUME-01 `receipts/01-supervision-selftest/1/cgroup-samples.txt` |
| 2 | CGROUP_CONSTANT | `receipts/01-supervision-selftest/5/cgroup-samples.txt` | self-test 5 | 95 | `sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n` | RESUME-01 `receipts/01-supervision-selftest/5/cgroup-samples.txt` |
| 3 | CGROUP_CONSTANT | `receipts/01-supervision-selftest/7/cgroup-samples.txt` | self-test 7 | 95 | `sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n` | EXEC-01 `receipts/01-supervision-selftest/7/cgroup-samples.txt` |
| 4 | CGROUP_CONSTANT | `receipts/36-C28/04-run/cgroup-samples.txt` | control C28 | 95 | `sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n` | RESUME-01 `receipts/32-C14/04-run/cgroup-samples.txt` |
| 5 | CGROUP_CONSTANT | `receipts/37-C12/04-run/cgroup-samples.txt` | control C12 | 95 | `sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n` | RESUME-01 `receipts/32-C14/04-run/cgroup-samples.txt` |
| 6 | CGROUP_CONSTANT | `receipts/38-C9/04-run/cgroup-samples.txt` | control C9 | 95 | `sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n` | RESUME-01 `receipts/32-C14/04-run/cgroup-samples.txt` |
| 7 | CGROUP_CONSTANT | `receipts/39-C11/04-run/cgroup-samples.txt` | control C11 | 95 | `sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n` | RESUME-01 `receipts/32-C14/04-run/cgroup-samples.txt` |
| 8 | CGROUP_CONSTANT | `receipts/40-C17/04-run/cgroup-samples.txt` | control C17 | 95 | `sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n` | RESUME-01 `receipts/32-C14/04-run/cgroup-samples.txt` |
| 9 | CGROUP_CONSTANT | `receipts/41-C18a/04-run/cgroup-samples.txt` | control C18a | 95 | `sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n` | RESUME-01 `receipts/32-C14/04-run/cgroup-samples.txt` |
| 10 | CGROUP_CONSTANT | `receipts/42-C18b/04-run/cgroup-samples.txt` | control C18b | 95 | `sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n` | RESUME-01 `receipts/32-C14/04-run/cgroup-samples.txt` |
| 11 | SELFTEST_CONSTANT | `receipts/01-supervision-selftest/1/out.txt` | self-test 1 | 19 | `allocated 10485760\n` | EXEC-01 `receipts/01-supervision-selftest/1/out.txt` |
| 12 | SELFTEST_CONSTANT | `receipts/01-supervision-selftest/6/out.txt` | self-test 6 | 43 | `timeout: the monitored command dumped core\n` | EXEC-01 `receipts/01-supervision-selftest/6/out.txt` |
| 13 | SELFTEST_CONSTANT | `receipts/01-supervision-selftest/7/out.txt` | self-test 7 | 42 | `['CARGO_NET_OFFLINE', 'LC_CTYPE', 'PATH']\n` | EXEC-01 `receipts/01-supervision-selftest/7/out.txt` |
| 14 | SELFTEST_CONSTANT | `receipts/01-supervision-selftest/7/err.txt` | self-test 7 | 10 | `to stderr\n` | EXEC-01 `receipts/01-supervision-selftest/7/err.txt` |

Each row's identity is its exact path in its own self-test or control-run directory, bound by that directory's descriptor (`SELFTEST.txt` for self-tests; `04-run/descriptor.json` naming the control, its worktree and its supervisor record for control runs) and its supervisor receipts (`scope-record.txt`, `time-v.txt`). No row is inferred from equal sha256 alone.

## 3. The 1-KiB threshold is rejected

The frozen CONTINUE-02 builder compared only files of 1 KiB or more. That threshold is rejected: every nonempty staged file is compared with every file of both previous packets (EXEC-01, RESUME-01), whatever its size. A duplicated run classification receipt (about 900 to 1000 bytes; RESUME-01 C14's is 937 bytes) is rejected; the repair controls show that the threshold, if reintroduced, would wrongly accept it.

## 4. The eight zero-length artifacts

| # | current path | role | expected command |
|---|---|---|---|
| 1 | `receipts/01-supervision-selftest/3/out.txt` | supervision self-test 3: stdout and stderr merged (out.txt) of a command killed at its 3 s time limit before writing anything | `/bin/sleep 30` |
| 2 | `receipts/01-supervision-selftest/5/out.txt` | supervision self-test 5: stdout and stderr merged (out.txt) of a command that exits 101 without writing anything | `/usr/bin/python3 -c 'import sys; sys.exit(101)'` |
| 3 | `receipts/37-C12/03-build/build.stdout` | control C12 supervised mutant build (03-build): cargo's stdout (build.stdout; cargo writes its diagnostics to build.stderr) | `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2` |
| 4 | `receipts/38-C9/03-build/build.stdout` | control C9 supervised mutant build (03-build): cargo's stdout (build.stdout; cargo writes its diagnostics to build.stderr) | `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2` |
| 5 | `receipts/39-C11/03-build/build.stdout` | control C11 supervised mutant build (03-build): cargo's stdout (build.stdout; cargo writes its diagnostics to build.stderr) | `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2` |
| 6 | `receipts/40-C17/03-build/build.stdout` | control C17 supervised mutant build (03-build): cargo's stdout (build.stdout; cargo writes its diagnostics to build.stderr) | `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2` |
| 7 | `receipts/41-C18a/03-build/build.stdout` | control C18a supervised mutant build (03-build): cargo's stdout (build.stdout; cargo writes its diagnostics to build.stderr) | `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2` |
| 8 | `receipts/42-C18b/03-build/build.stdout` | control C18b supervised mutant build (03-build): cargo's stdout (build.stdout; cargo writes its diagnostics to build.stderr) | `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2` |

Each is accepted only as that exact regular, non-symlink, empty file with its role, its producing command and its descriptor, stderr and result evidence (`ZERO-LENGTH-ARTIFACT-MAP.tsv`). Byte equality with an earlier empty file is not evidence.

## 5. The exact policy (no wildcard)

- Exactly the 21 nonempty paths and the 8 zero-length paths above; each map holds exactly that many rows, and a map that is a symlink is rejected.
- No size threshold; no wildcard; no basename, directory-only, generic deterministic-file, generic `06-link`, generic `cgroup-samples`, generic `build.stdout` or generic empty-file exception.
- Every other nonempty duplicate and every other zero-length file is rejected (fail closed).
- Publication only: no existing raw receipt, classification, result, build output, execution output, restoration receipt or link receipt is edited.

## Candidate status (unchanged by this ruling)

- C28, C12, C9, C11, C17, C18a, C18b remain executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`), pending independent Architect review. This ruling accepts none of them.
- Previously Architect-accepted (CONTINUE-02 `ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md`, sha256 `cf9bf865e01c8f8b359916eae75f29147132ff67465f93b1c08473c0a8b0d776`): C22, C14, C19, C21, C27.

## No control was rerun

No mutation control, baseline, build, test, explorer, witness or executable was run for this ruling or by this mission. Every receipt is CONTINUE-02's, byte for byte; this mission only reads, maps, checks and publishes it.
