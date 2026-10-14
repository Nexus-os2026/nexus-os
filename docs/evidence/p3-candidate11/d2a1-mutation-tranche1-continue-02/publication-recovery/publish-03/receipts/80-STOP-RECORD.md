# Stop record: repair control P3 failed; 14 unauthorized duplicates beyond the seven ruled link receipts

Mission P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-03. Status: **BLOCKED** (mission section 15: "failed repair
control" and "unauthorized duplicate"; section 10: "A failure stops this publication mission").

Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. The mission named Claude Sonnet 5.5
(`claude-sonnet-5-5`); this session runs on Opus 5.5. That is a deviation, reported here.

Nothing was frozen, staged in a repository, committed or pushed. No evidence worktree or branch was created. Nothing was
built, compiled, tested, explored, mutated or executed: only Python publication tools and read-only git ran. No control
was rerun.

## What completed (receipts under this mission's root)

| Step | Receipt | Result |
|---|---|---|
| Source-evidence manifest, before any tool ran on the root | `receipts/00-SOURCE-MANIFEST-BEFORE.tsv` (+ `.txt`) | 4409 entries (794 directories, 3577 text files, 30 binary `.pyc` files, 8 symlinks, no ELF); manifest sha256 `9eb18bd2…8757` |
| Authentication | `receipts/01-AUTHENTICATION.txt` | RESULT: ALL CHECKS HOLD (144 checks): C; catalog and seven patches; EXEC-01 and RESUME-01 commits, packets, SHA256SUMS and review refs; CONTINUE-02 freeze (116 frozen files + pre-freeze receipts byte-equal), pre-execution gate, both final-gate outputs, the frozen failed builder, the failed packet result, the stop record and development log; all seven control receipt sets; seven pinned mutant executables and the baseline executable (hashed, not executed); worktrees locked at C; mutant blobs absent from the object store; destination ref absent |
| Ruling record and map | `documents/ARCHITECT-RULING-DUPLICATE-LINK-RECEIPTS.md`, `documents/DUPLICATE-CONTENT-MAP.tsv`, `.md` (`receipts/02-MAKE-MAP.txt`) | seven rows, each byte-equal to its one EXEC-01 prior file; `link()` source identical in EXEC-01, RESUME-01 and CONTINUE-02 |
| Repaired builder and exact diff | `tools/build_packet.py`, `receipts/03-BUILDER-DIFF/BUILDER.diff` (+ console) | BUILDER DIFF: ONLY THE DUPLICATE-CONTENT POLICY CHANGED (12 frozen lines removed, all of the frozen duplicate check; 6 added segments, all policy) |
| Repair controls | `receipts/04-REPAIR-CONTROLS/` (+ console) | 30 controls: **29 as expected, P3 not as expected** (below) |
| Source evidence after | `receipts/05-SOURCE-MANIFEST-COMPARE.txt` | SOURCE EVIDENCE: UNCHANGED (manifest identical, sha256 `9eb18bd2…8757`, mtimes included) |
| State at stop | `receipts/06-STATE-AT-STOP.txt` | destination ref absent (remote and local); no publish-03 worktree or branch; old CONTINUE-02 evidence worktree untouched; main repository clean |
| Duplicate inventory | `receipts/07-DUPLICATE-INVENTORY.tsv` (+ `.txt`) | 7 accepted ruled, 14 unauthorized, 8 zero-length not compared |

The repaired builder stages exactly what the frozen builder staged, plus `receipts/80-STOP-RECORD.md` (written by
CONTINUE-02 after its failed staging); only `development/DEVELOPMENT-LOG.txt` (CONTINUE-02 entry 24 appended) and
`SHA256SUMS` differ (control output S0).

All seven ruled link receipts verify on every map field and are accepted (P1). Every negative control rejects for its
own injected reason, both differentially and absolutely (N4 to N15, 27 controls), and the over-broad "every 06-link
duplicate" policy wrongly accepts what the repaired policy rejects (N11a, N11b).

## Why the mission stopped

Control P3 ("the existing CONTINUE-02 evidence builds successfully with the seven-path allowlist") fails. The repaired
builder implements section 6 and 7 as written: duplicate content is permitted **only** for the seven exact current files;
there is **no size-threshold bypass** and **no broad exemption for deterministic files**. Under that policy the existing
evidence holds 14 further nonempty files, each byte-equal to files of the previous packets, which no ruling permits:

| Bytes | Content | Current paths | Previous-packet files with the same bytes |
|---|---|---|---|
| 95 | `sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)` | `receipts/01-supervision-selftest/{1,5,7}/cgroup-samples.txt`; `receipts/{36-C28,37-C12,38-C9,39-C11,40-C17,41-C18a,42-C18b}/04-run/cgroup-samples.txt` (10 files) | 61 each |
| 19 | `allocated 10485760` | `receipts/01-supervision-selftest/1/out.txt` | 2 |
| 43 | `timeout: the monitored command dumped core` | `receipts/01-supervision-selftest/6/out.txt` | 2 |
| 42 | `['CARGO_NET_OFFLINE', 'LC_CTYPE', 'PATH']` | `receipts/01-supervision-selftest/7/out.txt` | 2 |
| 10 | `to stderr` | `receipts/01-supervision-selftest/7/err.txt` | 2 |

(Every line ends with one newline. Exact sha256, git blob and the first previous-packet paths: `receipts/07-DUPLICATE-INVENTORY.tsv`.)

These are independently produced constant records (the sampler's one-line note for a short run, and fixed self-test
outputs). The frozen CONTINUE-02 builder did not reject them only because its rule covered files of **1 KiB or more**;
it printed them as "info" (`publication/10-build-packet.txt`: "14 small staged file(s) coincide…"). Eight zero-length
files (`receipts/01-supervision-selftest/{3,5}/out.txt` and the six `03-build/build.stdout`) carry no content and are not
compared, as in the frozen builder.

## The conflict the mission text does not resolve

- Keeping the frozen 1 KiB floor makes P3 pass (diagnostic D1: 14 problems become 0), but is a size-threshold bypass
  (section 7). It would also accept a duplicated run classification receipt: real run `CLASSIFICATION.txt` files are
  about 900 to 1000 bytes. D1 shows that with the floor, C12's run classification replaced by RESUME-01 C14's (937 bytes)
  is **not** rejected; that is the natural instance of section 8 control 10 ("Duplicate a … classification …
  receipt: reject").
- Removing the floor (as built) makes control 10 hold for all four receipt kinds, but then the 14 constant records are
  duplicates outside the seven paths: section 6 ("permit duplicate content only for these current files"), section 7
  ("the builder must fail if … a duplicate occurs outside the seven paths") and section 15 stop.
- Accepting the 14 by any rule would be a new duplicate exception, which the ruling explicitly does not grant ("no other
  duplicate exception is granted"; "no broad exemption for deterministic files").

No policy within this mission's authorization satisfies P3 and control 10 together on this evidence. Per the
constitution, the narrower fail-closed reading was taken and the mission stops.

## Architect decision required

One of:

1. **Recommended:** an exact, separately mapped exception for these 14 constant-record paths (each exact path, its exact
   bytes and sha256, and its previous-packet equal files), keeping the floor-free default. Control 10 then stays strong
   for every receipt size, and every permitted duplicate is explicit.
2. Retain the frozen 1 KiB floor as the default, accepting that sub-1-KiB duplicates (including real run
   classification receipts) are not rejected, and restate control 10 accordingly.
3. Another ruling.

Also rule whether the eight zero-length files stay outside the comparison, as in the frozen builder.

Everything built here is reusable by a follow-on publication mission: the authentication, the map and the ruling
record, the repaired builder (one bounded change to add the ruled exception), the repair controls and the source
manifest.

## State preserved

- This mission's root `/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-mutation-tranche1-publish-03`
  (documents, tools, receipts, development; `scratch/` holds the control stagings and is not evidence).
- CONTINUE-02 root unchanged (manifest identical before and after).
- Old evidence worktree `nexus-os-p3-c11-d2a1-mutation-tranche1-continue-02-evidence` untouched (HEAD `3695a817`,
  its untracked failed staging intact).
- Remote: `refs/heads/review/p3-c11-d2a1-mutation-tranche1-continue-02` absent. No push, no PR, no tag, no dispatch.
- Candidates C28, C12, C9, C11, C17, C18a, C18b: unchanged, executor candidates pending Architect review. Accepted:
  C22, C14, C19, C21, C27 (unchanged).
