# P3-C11-DR01-D2A1-C7-W-EVIDENCE-EXEC-01 — C7 evidence packet (AD-C7-W)

Continued under P3-C11-DR01-D2A1-C7-W-EVIDENCE-EXEC-01-OPUS-R3 (the original EXEC-01 authorization; not a new campaign).
Executor: Claude Opus 5.5 (`claude-opus-5-5`). Effort requested: Extra High (not independently readable back by the executor).

**Result (maximum permitted locally): KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING.**
This is not "C7 ACCEPTED" and not "35/35 ACCEPTED". C7 stays MUTATION_CONTROL_BLOCKED until the Primary Architect rules.
All local results are EXECUTOR-REPORTED until independently audited. A string is not authority.

## Identities

| Item | Value |
|---|---|
| C-BASE / tree | `d5964ee5326c2733177307db6f507328023d361a` / `51bac5fd335999cc995c0ab9f582a30244973ef0` |
| W-COMMIT / W-TREE | `6c424c3aec579401543ae010e3f8729e17c4ed98` / `5cc7ae59a6a5ef38b5630e76849a1f5b468ac4a4` |
| W-BLOB / sha256 | `c18a6235a9eb32314931c51dd7bb574df77fbe28` / `6762d91f3e4de61902efb8f32eccf38fcf4166f5c036ba7a2832f05944b0be25` (329,569 bytes) |
| W-PATCH sha256 | `c8509c025542c81fdc44e296a0da256e04ba533a971dfbe1b379ee6f9f76b0a9` |
| K2 / K3 sha256 | `989490ec9272a528fe0185ad0dc9f41e3aed1e414d2eb4a88346197dc60b3f3b` / `c5a9f3d1ade663ca4ef57143c5ae5a1392296d9a05bf226888ba99df2ca44af7` |
| K4 commit / record sha256 | `4c29b2f0035ecaa86bca8d49238331485a1140e6` / `800f767d4df66f6a8c1c30306395042ff62a22bc3a354349d813ffc100066a54` |
| C7.patch sha256 | `b5d8bc6c9254e0ca2853fcf2f9f220f24782ac6cfa77efe7032365d76f0efbc4` |
| W7 protocol.rs (never committed, never in the object store) | blob `0d4ab9929c0e15d910735a52b72a5f3855bef58a`, sha256 `6d2f2d49842cdf5cc155dd1cb0984e121ada154cfc69a412219c7ac12dc846aa` |
| E-W | `/home/nexus/NEXUS/nexus-os-p3-c11-c7w-healthy/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3`, sha256 `042b11e69964df40487a93f307900499a8c0253e24abe4ea598b11b53871c825`, 8,995,112 bytes |
| E-W7 | `/home/nexus/NEXUS/nexus-os-p3-c11-c7w-mutant/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3`, sha256 `aa0c0577ebbd651dbcc6a7a27849cb6a2d1f7004f87732a59f3a123fda9b1223`, 8,995,176 bytes |

## Results (13.3 order)

| Step | Receipt | Result |
|---|---|---|
| Healthy worktree | `receipts/00-worktrees/creation-H.json` | EXACTLY_W at W-COMMIT |
| E-W build (one) | `receipts/10-build-E-W` | exit 0, 2 m 49 s, peak 2,616,541,184 B, swap 0, oom 0, 0 warnings, 0 errors |
| Mutant worktree + C7 patch (once) | `receipts/00-worktrees/{creation-M.json,apply-C7.txt}` | EXACTLY_W7 (only protocol.rs differs; unstaged) |
| E-W7 build (one) | `receipts/11-build-E-W7` | exit 0, 2 m 48 s, peak 2,611,060,736 B, swap 0, oom 0, 0 warnings, 0 errors |
| Pins | `receipts/12-pin` | E-W and E-W7 pinned; fingerprint rustflags `[]` |
| Listings (8 + 8, supervised) | `receipts/20-list-E-W`, `receipts/21-list-E-W7` | 349 tests / 35 ignored on both; byte-identical between executables |
| B-W/C | `receipts/30-bwc/B-W-C.txt` | **MECHANICALLY SATISFIED** (276 PASS / 0 FAIL), subject to independent Architect audit |
| (C) healthy witness on E-W | `receipts/40-run-C-healthy-witness` | **PASS — ADMISSIBLE** (34/0): exit 0, 24 steps, 0 VIOLATION lines, healthy step-23 end, kept predicate |
| (A) original F-PT on E-W7, A4 on | `receipts/41-run-A-full_fpt` | **FAIL E11 — ADMISSIBLE** (24/0): exit 101, 1,071,165 states, 4.65 s, two byte-equal instances |
| (B) mutant witness on E-W7 | `receipts/42-run-B-mutant-witness` | **ADMISSIBLE** (47/0): exactly E11 (step 13 transition) and P6 (after 24); 4 raw VIOLATION lines = 2 records + 2 summary copies |
| F-3 on E-W7, A4 off | `receipts/43-run-F3-informational` | INFORMATIONAL — NOT A KILL RECEIPT — NOT A G.7 RECEIPT: **FAIL E11 (as expected)** (21/0) |
| Restoration (7.3A R-1..R-8, one `git apply -R`) | `receipts/50-restoration` | **RESTORATION COMPLETE: EXACTLY_W**; E-W7 sha256 unchanged |
| (D) link of accepted 44-full-F-PT / 43-full-F-3 | `receipts/60-link/LINK.txt` | **USABLE** (50/0) — C-BASE PASS on E-C; NOT W-SRC PASS; not rerun |

Every invocation: normal exit, no signal, complete scope record, no timeout, oom 0, oom_kill 0, swap peak 0,
memory.max 17179869184, memory.swap.max 0. One invocation at a time. No retry, no rebuild, no resource change.

The separation required by EXEC-01 section 12 and the receipt hashes are in `r3/90-LOCAL-CLASSIFICATION.txt`.

## Execution record

- `r3/MISSION-R3.txt` — the R3 mission as relayed (executor transcription); `preflight/sonnet-7b42496f/mission.txt` — original EXEC-01.
- `r3/cmd/` — every stage's exact command file, executed verbatim with `bash`; `r3/gates/` — the disk/inode/process/tool-hash gate record of each stage; `r3/console/` — console output.
- `r3/01-PRESERVATION.txt` — the preflight evidence preservation record.
- **Gate deviation (fail-closed, nothing started):** the first stage gate (`r3/gates/10-create-H.txt`, prologue `r3/cmd/00-gates.v1.sh`) matched by command line (`pgrep -f`) a stale Oct-13 bash wait loop (PID 2306315; not this mission's; runs only `pgrep`/`sleep`; left untouched) and refused to start. The prologue was corrected to match process names (`00-gates.sh`, v2; proven against decoy processes), and the stage ran as `10b-create-H`.

## Preflight evidence

`preflight/` publishes a text-only subset of the preserved preflight evidence: gate-run1 (FAIL), gate-run2 (PASS), gate-run3 (PASS),
TOOLS/FIXTURES/MANIFEST hashes, the R2 command log and results, the six self-test outcomes, the R1 records, the frozen helper sources and fixtures,
and the complete per-file manifests (`*.SOURCE.manifest`, `*.COPY.manifest`) of the three preserved scratchpads.

The self-test working directories (scratch crates and repositories, compiled scratch binaries, synthetic fixtures) are retained locally in
`/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/preflight/` and are **not published**: three of them hold
synthetic full copies of the mutant protocol.rs (blob `0d4ab992`) written by the self-tests to scratch directories, which must never enter an
evidence commit or the object store. Their hashes are in the published manifests.

All files are committed with mode 100644. `SHA256SUMS` covers every other file of this packet.

## Not claimed

No C7 acceptance; no change to the 34 accepted controls, A4, Candidate C, the catalog or W-COMMIT; no CI or Actions dispatch; no PR, merge or protected-ref change.
Open for the Architect: independent audit of B-W/C, of every receipt and of this packet; AD-C7-W 11 items; the gate deviation above.
