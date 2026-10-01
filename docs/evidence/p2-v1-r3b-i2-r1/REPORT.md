# P2-V1-R3B-I2-R1 — Canonical codec assurance: retained evidence

This is the retained evidence for one review candidate that is awaiting
independent Architect review. It does not record acceptance, integration or
Phase Two completion.

The evidence commit adds only this directory. Its parent is the code candidate,
and every file outside this directory is identical to the candidate. This commit
is not a second implementation candidate.

## 1. Identity

| | |
|---|---|
| Repository | `Nexus-os2026/nexus-os` |
| Code candidate | `45898e05178a56efaadeb1f8f9ee7a0a521c72c9` on `review/p2-v1-cleanup-observers` |
| Candidate tree | `cd197e047cb5a57e458b2af5d32f9c5406371301` |
| Sole parent (repair base) | `0eb2a947e839b5639d9688da4e6b51b5e72a4d13`, tree `da9fa4175c05e5eeae52ecbaf71201c9667045f3`, itself with sole parent `b7693364b1601a00be12426cec8871c96a205953`. The base is the published I2A review candidate; it has not been accepted. |
| Candidate subject | `test(custody): verify canonical request receipt binding` |
| Evidence ref | `evidence/p2-v1-r3b-i2-r1`: one evidence-only commit whose parent is the code candidate |

## 2. Findings and their disposition

### A. Request receipts are now observed in the real core

Finding A: the I2A request receipt test did not establish which digest the core
actually stores. Restoring Debug-text request hashing was caught only by the c16
source guard. The original I2A run shows this: `I2A-DEBUG-REQUEST-c13` passed and
only `I2A-DEBUG-REQUEST-c16` failed (see `original-i2a/`).

I2-R1 adds the regression
`c17_core_retains_the_canonical_request_digest_in_its_receipts`
(`tests/phase2_custody_codec.rs:2240-2401`). It reads the digest the core
actually retains and compares it with an independently specified canonical frame
digest. Section 4 explains how; section 6 shows that both new counted controls
are caught by c17's canonical-receipt assertion.

### B. RecordIntent's digest coverage is stated exactly

Finding B: the I2A documentation said the digest is "the SHA-256 of the record's
canonical frame". The digest covers the frame header and payload only, not the
32-byte trailer that carries the digest. `tests/support/custody/model.rs:451-455`
now reads:

```rust
/// One record the recorder must make durable, in `id` order. The digest binds
/// the acknowledgement to exactly this record: it is the SHA-256 of the
/// record's canonical frame header and payload, every byte of the frame
/// before its 32-byte digest trailer, which holds this digest and is not
/// covered ([`super::codec::encode_record`] writes the frame).
```

The change is documentation only: one `///` line was replaced by three. No
declaration, visibility, constructor, field, derive or semantics changed.

### C. I2/I2A deviations: recorded, history unchanged

The Architect's finding, as stated:

> The original I2 mission required model.rs to remain byte-identical and
> specified a different exact commit subject. The returned I2A candidate
> deviated from those requirements.

The deviations are as follows:

1. 0eb2a947 changed `tests/support/custody/model.rs` in two documentation
   regions:
   - the module documentation: lines 8-11 at 0eb2a947, within the paragraph
     at lines 5-11. This is the identity-as-data clarification, saying an
     identity can also come, as plain data, from decoding evidence bytes, and
     grants nothing.
   - the RecordIntent digest documentation, with the inaccurate coverage
     statement of finding B.
2. 0eb2a947's subject is `test(custody): add canonical evidence and request
   codec`, not the subject the original I2 mission specified.

What this record can and cannot verify:

- The text of the original I2 mission is not among the mission texts held in
  the executing session. That session received the I2A mission ("P2-V1-R3B-I2A —
  CANONICAL EVIDENCE CODEC COMPLETION LOOP") and then this I2-R1 mission.
- The I2A text allowed, for model.rs: "Only genuinely necessary read-only
  accessors and documentation. No public constructors for opaque
  authority-bearing tokens, no enum semantic changes and no altered
  completion/admission policy."
- The I2A text specified the subject `test(custody): add canonical evidence
  and request codec`.
- The clause "model.rs must remain unchanged." appears in the earlier I1-R2A
  mission. Its commit, b7693364, left model.rs unchanged.
- The finding is recorded here exactly as the Architect stated it. The original
  I2 requirements themselves are not reproduced, because this record cannot
  verify them.

Disposition, under this mission's explicit authorisation:

- 0eb2a947 is preserved unchanged as the sole parent. Nothing was amended,
  rewritten or renamed.
- The identity-as-data clarification is retained unchanged.
- The RecordIntent documentation is corrected (finding B).
- No other model.rs change was made. Under `git diff -U0`, the I2-R1 model.rs
  diff is the single hunk `@@ -453 +453,3 @@`, all `///` lines.

## 3. Changed paths and scope

Source: `i2-r1/publication/scope-check.log`.

| Path (under `crates/nexus-verifier-sandbox/`) | Change |
|---|---|
| `tests/phase2_custody_codec.rs` | modified, +230 −3. Adds the header sentence, `G3C`/`GA5`, `RECEIPT_VECTORS` (9 literals, lines 498-547), `Run::with`, `Run::request` taking the custody's own generation, `hex`, and c17. Assertion macros go from 135 to 147 and top-level `#[test]` functions from 17 to 18; no existing test function was removed. The 3 removed lines are a doc line and the two `GENERATION` uses that `Run::with` / `Run::request` now parameterise. |
| `tests/support/custody/core.rs` | modified, +8 −0: only the accessor in section 4 |
| `tests/support/custody/model.rs` | modified, +3 −1: documentation only (finding B) |

These are byte-identical to 0eb2a947 (blob ids are in the log):

- `tests/support/custody/codec.rs`, `tests/support/custody/mod.rs` and
  `tests/phase2_custody_core.rs`;
- the cleanup observer `tests/support/cleanup_observation.rs`,
  `tests/phase2_cleanup_observation.rs`, the live harness
  `tests/phase2_live_sandbox.rs` and `tests/phase2_package_layout.rs`;
- the crate's `build.rs` and `Cargo.toml`, the workspace `Cargo.toml`,
  `Cargo.lock`, `rust-toolchain.toml` and `.cargo/config.toml`;
- the trees `crates/nexus-verifier-sandbox/src/` and `.github/`.

No manifest, lockfile, build script or toolchain file changed anywhere in the
tree. `codec.rs` is unchanged, so the version-1 wire format is unchanged.

## 4. How c17 observes the stored digest

The receipt path in the candidate's `tests/support/custody/core.rs`:

- `Control::submit` (616) queues the request.
- `Custody::serve` (2722) takes it and calls `sequence` (2739), which computes
  `let digest = codec::request_digest(request);` (2748). It refuses a foreign
  generation, a duplicate, a conflict, a replay or a gap; otherwise it calls
  `execute` (2766).
- `execute` runs the operation and stores
  `Receipt { seq, digest, response }` through `RequestGate::remember`
  (1158; call at 2777-2782).

The only core change is a read-only observation of that store (2787-2793):

```rust
/// Test observation only: the digest the retained receipt for request
/// `seq` holds, if one is retained. It reads this custody's real receipt
/// and changes nothing.
#[cfg(test)]
pub(crate) fn retained_request_digest(&self, seq: u64) -> Option<[u8; 32]> {
    self.requests.receipt(seq).map(|receipt| receipt.digest)
}
```

The accessor:

- takes `&self`;
- is compiled only under `cfg(test)`;
- is `pub(crate)` inside the test-support module;
- reads the existing `RequestGate::receipt` lookup.

It adds no mutation or injection path, no global state, no authority and no
production API. Normal behaviour is unchanged.

c17 serves real requests through the test fixture `Run::serve`, which is
`Control::submit` followed by `Custody::serve`. It uses three real custodies,
each configured as `Config::LIVE` with `recovery_spacing_millis: 1`.
`Config::LIVE` sets `receipt_limit` to 16, so every receipt here stays retained.

| Custody | Construction | Requests served, in order (each `Executed`) |
|---|---|---|
| A | `Run::recovering`, generation `3c×16`. The run requires recovery, so its retries are decided against real recovery state; after the five requests c17 asserts exactly 2 recovery attempts. | seq 1 Retry{0}, 2 Shutdown, 3 Retry{1}, 4 Retry{u64::MAX}, 5 Retry{0} |
| B | `Run::with(config, Generation::new([0xa5; 16]))` | seq 1 Retry{0} |
| C | `Run::with(config, GENERATION)` (`3c×16`) | seq 1 Shutdown, 2 Retry{0}, 3 Retry{2} |

Custody A then serves:

- the exact duplicate of its first request, which returns `Duplicate`;
- seq 1 as Shutdown, which returns `Conflict`.

`retained_request_digest(6)` is `None`. Every retained digest is read after
these, so duplicates and conflicts are shown to leave the stored digest as it
was.

For each of the 9 served requests, c17 asserts:

`retained_request_digest(seq) == Some(expected)`, with the marker `[request-receipt]`.

`expected` comes from the literal `RECEIPT_VECTORS` entry for that exact
request. c17 first checks that the literal digest is the SHA-256, computed with
the `sha2` crate, of the literal frame. c17 never calls the codec helper under
test, `codec::request_digest`, and no receipt is mocked.

Finally, five pairs that differ in exactly one field must be retained under
different digests; c17 asserts the single differing field before each
`assert_ne!`:

| Field | Pair |
|---|---|
| generation | A1 / B1 |
| sequence | A1 / A5 |
| operation | A1 / C1 and A2 / C2 |
| retry epoch | A3 / C3 |

The existing duplicate, conflict and replay checks (c13, and h19 in the core
suite) are unchanged and still pass.

## 5. Independent expected bytes and digests

The frames were assembled field by field from the version-1 table by the
stdlib-only generator `i2-r1/golden/i2r1-golden-vectors.py`, using Python
`struct` and `hashlib`, not from the Rust code.

Request frame layout:

| Field | Bytes | Value |
|---|---|---|
| magic | 4 | `4e584344` (`NXCD`) |
| domain | 1 | `51` for a request (`52` for a record) |
| version | 1 | `01` |
| payload length L | 2, u16 big-endian | `0021` = 33 for Retry, `0019` = 25 for Shutdown |
| generation | 16 | the generation bytes |
| sequence | 8, u64 big-endian | |
| operation tag | 1 | `01` Retry, `02` Shutdown |
| retry epoch | 8, u64 big-endian | Retry only |

The 32-byte trailer is SHA-256 of bytes `0..8+L`. The frames below are exactly
those covered bytes; the trailer is the digest.

| # | Request | Covered frame (hex) | SHA-256 digest |
|---|---|---|---|
| A1 | 3c×16, seq 1, Retry{0} | `4e584344 51 01 0021 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000001 01 0000000000000000` | `fa39efc25ef1f1b8d6e29a9235d66dd59b124a9081237aab0220590a950f6de4` |
| A2 | 3c×16, seq 2, Shutdown | `4e584344 51 01 0019 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000002 02` | `e65434b8803345efc5d1ff2b4911b4909356612cbe4a7d6daabf172300b82272` |
| A3 | 3c×16, seq 3, Retry{1} | `4e584344 51 01 0021 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000003 01 0000000000000001` | `9a227c00586d95650e6d5aa9493e09953b2d063269e30613fe83db67703ac694` |
| A4 | 3c×16, seq 4, Retry{u64::MAX} | `4e584344 51 01 0021 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000004 01 ffffffffffffffff` | `69a101a9786c39730c16ea2ee1c692aa96992667a1697b911ab378dd0b8c8948` |
| A5 | 3c×16, seq 5, Retry{0} | `4e584344 51 01 0021 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000005 01 0000000000000000` | `9ffecc155b972e94d992d533fa4cc4ff99e93081c861658491667ce03e831c2d` |
| B1 | a5×16, seq 1, Retry{0} | `4e584344 51 01 0021 a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5 0000000000000001 01 0000000000000000` | `b55792d6176f5c3ce7a34a8177f994340e64c53faf2b1ea81a58b543d82e6749` |
| C1 | 3c×16, seq 1, Shutdown | `4e584344 51 01 0019 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000001 02` | `0bf26bf71b4c7345f3dff3dfe9ceb77bc7f77e45a9498e343acd392bbfa7bf23` |
| C2 | 3c×16, seq 2, Retry{0} | `4e584344 51 01 0021 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000002 01 0000000000000000` | `7d486ff6b72427b30ba1525b6586a7f991d527de6052c5780ef7ec045ecfa6ea` |
| C3 | 3c×16, seq 3, Retry{2} | `4e584344 51 01 0021 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000003 01 0000000000000002` | `1a0c59e036b9e331eeedb39ee3976d23ec90e2afd8ce446d833d40a2bd5a43f2` |

Revalidation of all 63 published vectors (50 record, 4 request, 9 receipt) on
the committed test file is in `i2-r1/golden/revalidation.log`:

- **Generator comparison.** `i2r1-golden-revalidate.py` matches every entry of
  the three tables against the generator, in order, and re-hashes all 63 frames
  with `hashlib`: 63 of 63 match.
- **Coreutils re-hash.** `golden_coreutils.sh` re-hashes all 63 frames with GNU
  coreutils `basenc` and `sha256sum` alone: 63 of 63 match.
- **Listings reproduced.** The generator reproduces the retained listings
  byte for byte.
- **I2A vectors unchanged.** The I2A generator's `--rust` output equals the
  I2-R1 generator's. `RECORD_VECTORS` and `REQUEST_VECTORS` are byte-identical
  between 0eb2a947 and 45898e05.

`i2-r1/golden/tool-selfcheck.log` shows each tool rejecting, with a nonzero
exit, a copy with one altered digest and a copy with one altered frame byte.

## 6. Negative controls

The complete matrix, with exact mutations and restoration hashes, is in
`i2-r1/controls/control-matrix.md`, generated by `control_matrix.py` from the
published `summary.json` files.

There are 32 counted controls:

- the 30 retained from I2A: eight codec and integration controls, and 22
  custody controls (8 I1, 6 R1, 7 R2, R2A-SPLIT). Their definitions are
  textually identical to I2A's, checked by `control_matrix.py`.
- the two new ones, NC-REQUEST-DEBUG and NC-REQUEST-ALTERED-DIGEST.

Each control mutated one file, compiled, and ran only its intended test with
`--exact`. Each produced `test result: FAILED. 0 passed; 1 failed` and exit
101, with its marker inside the failing assertion's message. Each was then
restored in `finally`, after which all six envelope files had their original
SHA-256 and the worktree had exactly its expected status. No control
overlapped another test or edit.

There were two complete runs, both with every condition met for all 32 controls:

- **Pre-commit** (`i2-r1/controls/pre-commit/`): run with the as-run script on
  uncommitted sources whose six SHA-256 values
  (`i2r1-frozen-sources.sha256`) equal the committed blobs.
- **Candidate** (`i2-r1/controls/candidate-45898e05/`): run with the
  reproducible `negative_controls.py` on a clean checkout of 45898e05. It
  refuses to run unless the six files equal the candidate's. Its only
  differences from the as-run script are in `negative_controls.as-run.diff`.

### NC-REQUEST-DEBUG

The core's request digest is restored to the Debug-text hash, in
`tests/support/custody/core.rs` at `sequence`:

```diff
-        let digest = codec::request_digest(request);
+        let digest = {
+            use sha2::Digest as _;
+            let mut hasher = sha2::Sha256::new();
+            hasher.update(b"nexus-phase2-custody-request\0");
+            hasher.update(request.generation.bytes());
+            hasher.update(request.seq.to_be_bytes());
+            hasher.update(format!("{:?}", request.op).as_bytes());
+            let digest: [u8; 32] = hasher.finalize().into();
+            digest
+        };
```

c17 failed (exit 101) in both runs. The assertion message, wrapped here for width:

```
assertion `left == right` failed: [request-receipt] custody A retained
38d65886bde99e90330b3977184097ea533788b6f92baa77a76fd4cdc50732cd for Request {
generation: Generation([60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60,
60, 60]), seq: 1, op: Retry { epoch: 0 } }, not its canonical digest
fa39efc25ef1f1b8d6e29a9235d66dd59b124a9081237aab0220590a950f6de4
```

### NC-REQUEST-ALTERED-DIGEST

The canonical call is kept, but its result is consistently altered before the
core compares and stores it:

```diff
-        let digest = codec::request_digest(request);
+        let mut digest = codec::request_digest(request);
+        digest[0] ^= 0x5a;
```

c17 failed (exit 101) in both runs. The assertion message, wrapped here for width:

```
assertion `left == right` failed: [request-receipt] custody A retained
a039efc25ef1f1b8d6e29a9235d66dd59b124a9081237aab0220590a950f6de4 for Request {
generation: Generation([60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60,
60, 60]), seq: 1, op: Retry { epoch: 0 } }, not its canonical digest
fa39efc25ef1f1b8d6e29a9235d66dd59b124a9081237aab0220590a950f6de4
```

The retained digest differs from the canonical one only in the first byte:
`fa` XOR `5a` is `a0`.

### Informational runs (not counted)

These are identical in both runs.

| Mutation | c13 | c16 |
|---|---|---|
| NC-REQUEST-DEBUG | passes | fails, as a source guard only |
| NC-REQUEST-ALTERED-DIGEST | passes | passes |

Among c13, c16 and c17, only c17 detects either mutation behaviourally. A c16
failure was not counted for either control.

## 7. Validation

All commands ran from the repository root of the review worktree. The live
harness was only built (`--no-run`) and never executed, and no broader command
was run.

| # | Command | Pre-commit (2026-10-01T20:44Z; sources = the commit) | Post-commit (2026-10-01T20:49Z; HEAD 45898e05, clean) |
|---|---|---|---|
| 1 | `cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_codec` | exit 0; 21 passed, 0 failed | exit 0; 21 passed, 0 failed |
| 2 | `cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_core` | exit 0; 47 passed, 0 failed | exit 0; 47 passed, 0 failed |
| 3 | `cargo clippy --locked -p nexus-verifier-sandbox --test phase2_custody_codec --test phase2_custody_core -- -D warnings` | exit 0 | exit 0 |
| 4 | `cargo test --locked -p nexus-verifier-sandbox --test phase2_cleanup_observation` | exit 0; 36 passed, 0 failed | exit 0; 36 passed, 0 failed |
| 5 | `cargo test --locked -p nexus-verifier-sandbox --test phase2_live_sandbox --no-run` | exit 0 (built only) | exit 0 (built only) |
| 6 | `cargo fmt --all -- --check` | exit 0 | exit 0 |
| 7 | `git diff --check` | exit 0 | exit 0 |

Additional checks:

- **Before committing:** `git diff --cached --check` exited 0 with no output.
  That run's output was not kept as a log.
- **Commit diff:** `git diff --check 0eb2a947 45898e05` exits 0
  (`scope-check.log`).
- **Fresh build:** the post-commit run (`i2-r1/validation/i2r1-validate.sh`)
  first ran `cargo clean --locked -p nexus-verifier-sandbox`, removing
  1450 files and 397.5 MiB of this package's artifacts. Every target was
  therefore compiled and linted afresh from the committed sources.
- **Worktree state:** before and after the run, the worktree had no status
  entries.

Logs: `i2-r1/validation/i2r1-precommit-validation.log` and
`i2-r1/validation/i2r1-postcommit-validation.log`.

Toolchain and host:

- rustc 1.94.0 (4a4ef493e 2026-03-02)
- cargo 1.94.0 (85eff7c80 2026-01-15)
- clippy 0.1.94 (4a4ef493e3 2026-03-02)
- rustfmt 1.8.0-stable (4a4ef493e3 2026-03-02)
- Linux 6.17.0-35-generic x86_64
- Python 3.12.3
- GNU coreutils 9.4

## 8. Code publication record

Logs: `i2-r1/publication/`.

**Pre-push checks** (`i2r1-prepush-refs.log`), at 2026-10-01T20:47Z:

- The worktree is verified through Git: toplevel, branch
  `review/p2-v1-cleanup-observers`, HEAD 45898e05, one commit ahead of
  0eb2a947, clean.
- All seven fixed refs are unchanged, locally and on `github`:

| Ref | Commit |
|---|---|
| `main` | `4b36f60694148b60029733bc7b3d26e5a215d460` |
| `repair/p2-validation-workflow-bootstrap` | `4b36f60694148b60029733bc7b3d26e5a215d460` |
| `implement/phase2-governed-verification` | `4d6763a8afe998bb5a54f0b9ecfd420cd4abfa8f` |
| `rebuild/phase0-trust-boundary` | `f727f5c39fab8d5c729a55eb28ad576d3d56ce47` |
| `evidence/phase0-closure` | `e33cf1ff1b8de0d0c6c8751e24d85ed98b3cf9b1` |
| `rebuild/phase1-governed-coding` | `14270a9a38770ac84456c1f812042d2967edec42` |
| `evidence/phase1-closure` | `c237937189b5977eaad01acad6a4c52bcce796cc` |

- The remote review branch was still at 0eb2a947.
- The evidence ref and worktree did not exist.

**Workflow triggers at the candidate** (`i2r1-prepush-triggers.log`). The
candidate's `.github/` is identical to the base. Push triggers:

| Workflow | Triggered by |
|---|---|
| `audit.yml` | push to `main` |
| `ci-fast-local.yml` | push to `implement/**`, `repair/**` |
| `ci.yml` | push to `main` |
| `local-runner-smoke.yml` | push to `rebuild/phase0-trust-boundary` (path-filtered) |
| `pages.yml` | push to `main` (path-filtered) |
| `release.yml` | tags `v*` |
| `ci-phase2-linux-sandbox.yml`, `ci-portability.yml` | dispatch only |

Neither `review/**` nor `evidence/**` matches. No workflow, at the candidate or
on `main`, has a `create`, `delete`, `pull_request`, `pull_request_target`,
`workflow_run`, `repository_dispatch`, `check_run`, `check_suite` or `status`
trigger.

**Push** (`i2r1-code-push.log`): exit 0, output
`0eb2a947..45898e05  review/p2-v1-cleanup-observers -> review/p2-v1-cleanup-observers`.

```
git push github refs/heads/review/p2-v1-cleanup-observers:refs/heads/review/p2-v1-cleanup-observers
```

**Readback** (`i2r1-code-readback.log`):

- `ls-remote` and the GitHub API both report the branch at 45898e05;
- tree `cd197e04…`, sole parent 0eb2a947, the subject above, and exactly the
  three modified paths;
- compare base...candidate reports ahead by 1, behind by 0.

**Workflow runs** (`i2r1-run-baseline.log`, `i2r1-run-check-code.log`):

- Baseline: no runs for either branch, and 171 runs in the repository.
- Immediately after the push and about two minutes later: no runs for the
  branch or for 45898e05, and the total was still 171.
- Nothing was dispatched, rerun or cancelled, and no PR was opened.

The evidence ref's own publication and readback come after this commit. They
are reported outside it.

## 9. Evidence layout

`SHA256SUMS` lists the SHA-256 of every file in this directory except itself.

The repository's `.gitignore` excludes `*.log`. The retained logs were
therefore added deliberately with `git add -f`; `.gitignore` itself is
unchanged.

Two files are verbatim, and `git diff --cached --check` reported their trailing
whitespace:

- `negative_controls.as-run.diff`: a unified diff, whose empty context lines
  are a single space;
- `i2r1-prepush-triggers.log`: tool output with trailing spaces.

Rather than alter their bytes, `.gitattributes` in this directory unsets the
`whitespace` attribute for exactly those two paths.

```
REPORT.md                       this report (new)
SHA256SUMS                      manifest (new)
.gitattributes                  whitespace attribute for the two verbatim files above (new)
i2-r1/                          NEW evidence, produced for candidate 45898e05
  golden/
    i2r1-golden-vectors.py      independent generator, as run (I2A's, plus the receipt vectors)
    i2r1-golden-revalidate.py   revalidator, as run: every table entry and frame digest
    golden_coreutils.sh         coreutils-only re-hash of every published frame
    revalidate_all.sh           runs the checks above on the committed file
    revalidation.log            its output for 45898e05
    tool-selfcheck.log          both tools rejecting altered values
    i2r1-all-vectors.txt        generator listing: R, Q and T lines
    i2r1-receipts.txt           the 9 receipt vectors
    i2r1-receipts.rs.txt        the RECEIPT_VECTORS literal as generated
  controls/
    negative_controls.py        reproducible control script: checkout and log directory as arguments
    negative_controls.as-run.diff   its complete difference from the as-run script
    control_matrix.py           builds control-matrix.md
    control-matrix.md           32 counted controls, informational runs, restoration hashes, exact mutations
    pre-commit/                 as-run script, frozen-source manifest, run.log, summary.json, per-control logs
    candidate-45898e05/         invocation.txt, run.log, summary.json, per-control logs
  validation/
    i2r1-validate.sh            the post-commit validation script
    i2r1-precommit-validation.log
    i2r1-postcommit-validation.log
  publication/
    scope-check.log             paths, byte-identity of frozen files and trees, doc-only model.rs diff
    i2r1-prepush-refs.log, i2r1-prepush-triggers.log, i2r1-run-baseline.log,
    i2r1-code-push.log, i2r1-code-readback.log, i2r1-run-check-code.log,
    i2r1-evidence-setup.log
original-i2a/                   ORIGINAL I2A evidence for 0eb2a947, byte-identical copies
  PROVENANCE.md                 what each file is (new note)
  PROVENANCE-CHECK.log          the I2A logs' sources equal the 0eb2a947 blobs (new check)
  i2a-golden-vectors.py, i2a-vectors.rs.txt, i2a-all.txt, i2a-records.txt, i2a-requests.txt,
  i2a-negative-controls.py, i2a-negative-controls/, i2a-final-validation.log
```

Logs written in the executing session's job storage still name its paths, such
as `/home/nexus/.claude/jobs/4a4dc130/tmp`. They contain no credentials,
environment dumps, caches or binaries.

## 10. Reproduction

Run these from a copy of this directory. `<checkout>` is a clean Git checkout
whose history contains 0eb2a947.

- `revalidate_all.sh` and `negative_controls.py` accept HEAD at 45898e05 or at
  this evidence commit; the six envelope files and the test file are identical
  in both.
- `i2r1-validate.sh` requires HEAD at exactly 45898e05.
- Log and scratch directories must lie outside the checkout. The control
  script requires an unchanged worktree status throughout.

```sh
# Golden vectors (stdlib Python 3 and GNU coreutils only); run in i2-r1/golden.
# The log names the HEAD it read the test file from.
bash revalidate_all.sh <checkout> <scratch directory>

# Negative controls: mutates and restores files in <checkout>,
# so run it alone, with nothing else building, testing or editing there.
python3 -B i2-r1/controls/negative_controls.py <checkout> <log directory>

# The seven validation commands (removes this package's build artifacts first).
bash i2-r1/validation/i2r1-validate.sh <checkout> 45898e05178a56efaadeb1f8f9ee7a0a521c72c9 <log file>
```

## 11. Limitations

- **Scope of the assurance.** This evidence concerns a test-support custody
  model and its canonical codec. It is not evidence of authentication,
  durability, journal completeness, native cleanup, live acceptance,
  integration or Phase Two completion.
- **What c17 covers.** c17 checks the digest stored in each retained receipt,
  for 9 requests in three custodies under `Config::LIVE`, where receipts are
  bounded at 16. It does not examine evicted receipts, which by design hold no
  digest, or other configurations.
- **Independence of the generator.** It is independent in implementation
  (Python `struct`/`hashlib` from the format table, re-checked with coreutils),
  not in authorship. The format table, the generator and the Rust codec come
  from the same executing session, and no third party has reviewed them.
- **Which run used which sources.** The pre-commit control and validation runs
  used uncommitted sources byte-identical to the commit. The candidate control
  run and the post-commit validation used the commit itself.
- **Not exercised.** The live harness was built but not run. No CI workflow ran
  for this candidate. Only a Linux x86_64 host was used.
- **Finding C.** It rests on the Architect's statement about the original I2
  mission, whose text this record could not inspect (section 2).
