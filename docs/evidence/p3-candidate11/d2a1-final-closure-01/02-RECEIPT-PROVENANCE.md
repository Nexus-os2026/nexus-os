# Stage 2 — receipt provenance for the 20 configurations

Every value in `02-CONFIGURATIONS.tsv` was recomputed by the frozen `tools/s2_configs.py` (372 of 372 checks,
`receipts/23-S2-CHECKS.txt`) from raw receipt bytes. For the 15 full configurations the Git copy was compared byte for
byte with the local original receipt, and the Git blob at its evidence commit with the blob at the evidence head.

## 1. Where each receipt lives

| Configurations | Original receipts (outside the repository, contract line 865) | Git review copy | Packet custody |
|---|---|---|---|
| X-1 … X-5 (optimised, type (e1)) | `p3-c11-dr01-d2a1-exact-source-validation-02/receipts/41-…45-stage-C-X-n` | none (VALIDATION-02 is unpublished) | MANIFEST `36f10bf9…b822`, 177/177 |
| X-1 … X-5 (default suite, optimised) | `p3-c11-dr01-d2a1-final-baseline-known-runs-01/receipts/30-suite` | `f023cc3d`, raw `d6f58742…a3b3` | SHA256SUMS 179/179 |
| X-1 … X-5 (default suite, unoptimised, inside `cargo test --workspace --locked`) | VALIDATION-02 `receipts/30-stage-B-workspace-test`, raw `ce11ea91…9533` | none | 177/177 |
| F-1, F-2, F-3 (quotient), F-PT (quotient), F-B (quotient) | `p3-c11-dr01-d2a1-final-baseline-known-runs-01/receipts/41–45` | `f023cc3d` (Stage 1) | SHA256SUMS 179/179 (`receipts/25-PACKET-CUSTODY.txt`) |
| F-G, F-B2, F-4, F-W, F-8, F-5, F-6, F-7, F-D, F-R | `p3-c11-dr01-d2a1-final-baseline-remaining-configs-01/receipts/60–69` | `89326b85` (Stage 2) | SHA256SUMS 268/268 |

Each X run's Stage C block, through its state-key line, matched its Stage-1 suite block in counts and branches; each X
test printed `... ok` exactly once in Stage B (`receipts/23-S2-CHECKS.txt`).

## 2. Raw receipt hashes (sha256 of `raw-output.txt`)

| Config | sha256 | | Config | sha256 |
|---|---|---|---|---|
| X-1 | `db1707f08db7a15cf6a1ea2803bd9914c39f614c050b86a3c98ba83452a279e0` (Stage C) | | F-8 | `2e0f15299b7ed66a30cd12cf8ab6414139480a52687c83796b9d5718d4d3e3ee` |
| X-2 | `edb43eb5be78972ebe5007a12140651be5265568bc3c0f6ad4530a4c48c474a7` | | F-PT | `0863794ef7500c25382f55307213c65557f2d02b7bc4bb433511a71e082119a8` |
| X-3 | `03ebeb6be03dfea651c36f4b7b9664a73b6f10478a09c545461e6f8500844494` | | F-B | `c8b612e9ee7e2b8478bf7d4a8bb1dc37d737da7a06828136156752772c8ccaa7` |
| X-4 | `d276336a9c3b60c65c0a37d58f714168eec85c757e0c41eb5ca650d46d1abca2` | | F-B2 | `516adc39418a5cc0db8432e1ce7c1051029d05cb30dc133ac9b1147439633aa3` |
| X-5 | `622f0cf9d88394d003e95cbe3c3cd7e246e73ff2f7e19b950547ec40c2ceec09` | | F-G | `fabcbf13cfccec2744d03d38042dec7e72e3f3699f941a9a2ce17869d55705d1` |
| F-1 | `c1455a1ba3e55ae1ddd83feb97e13585882727613608eca01a669439e8f62c6c` | | F-W | `d309de48cde0febf20ab897d3e01fad05339e27d2fbb7a42abe674670900ba71` |
| F-2 | `985a3976713bc77bdf358744ec770e6220310f2ce2ed72b3e9ae10b3fb314ab5` | | F-D | `2f80e958de47a72a81f6d7ffb7f5ff00c95895a21f387d66eebb0b696304c6b3` |
| F-3 | `6dfc796a0b5144184e3e50c9b1b873c21bb31bf26405c572fe858fb694245562` | | F-R | `78e44c335b6ce65f8df6aea18b71e488613403ba8c0471c5cf1c33057ab58ccd` |
| F-4 | `44b0023eb9106365e4bf88039f8cc914c1e542f8a8490f604dfa0274aacfb948` | | | |
| F-5 | `a6d17e7691c80b07c79d1c8115eb40acfa149f8dab3574bce1e1564cecb47d8b` | | | |
| F-6 | `3660a0b2b88146f787aed4cf7d6fefe16ebfa658bfa172c33d90e9df3df28452` | | | |
| F-7 | `69e152df74318f497bfd1b8f2b6525de73c00317651f42934f4b22b69a3b417a` | | | |

X hashes are of the VALIDATION-02 Stage C receipts; the same values are in `02-CONFIGURATIONS.tsv`.

## 3. Common identity of every run

- Source: exactly Candidate C before each run (`verify_tree.py`, EXACTLY_C; 5,364 blobs, 97,246,341 bytes).
- Executable: the pinned E-C `cde9f9d2…d30c` (8,984,464 bytes), sha256 equal before and after every run; built once by
  `cargo test -p nexus-governed-control --locked --release --lib --no-run` with rustc 1.94.0 (4a4ef493e 2026-03-02),
  `opt-level=3, lto=thin, codegen-units=16, strip=symbols` (`final-baseline-known-runs-01/receipts/10-build`, `11-pin`);
  its dep-info lists 49 Rust inputs, each C's blob.
- Selection: exactly the configuration's test with `--exact` (and `--ignored` for the full set), one match,
  `--nocapture --test-threads=1`; environment `env -i PATH=/usr/bin:/bin LANG=C.UTF-8 RUST_BACKTRACE=0`.
- Limits: 16 GiB, zero swap, 2,700 s (180 s for the X runs) in a transient systemd user scope; no OOM, no swap, no
  timeout in any of the 20.

## 4. Decision provenance

| Configurations | Admissibility (recomputed) | Architect decision and its record |
|---|---|---|
| F-1, F-2, F-4–F-8, F-B2, F-G, F-W, F-D (11) | PASS — admissible unchanged-core original-explorer PASS | accepted under the relayed FINAL-EVIDENCE-STAGE2 ruling — **executor-written restatement only** (AD-1-R5 §0; `PROPOSED-20-CONFIGURATION-ACCOUNTING.md`, blob `5ac925af`); no verbatim ruling record in Git |
| F-3, F-PT, F-B (3) | PASS — admissible unchanged-core composed-quotient PASS; the original graph was **not** explored | the same relayed acceptance; the quotient prerequisite B(1)/N rests on the relayed FINAL-EVIDENCE-STAGE1 proof-audit acceptance (`FREEZE-RECORD.txt` at `f023cc3d`) and AD-1-R4's Owner adoption (E.1) |
| X-1–X-5 (5) | PASS (Stage C optimised e1; also PASS in the Stage-1 suite and in Stage B) | part of the passing Stage-1 suite accepted under STAGE2 as restated; **the form of the X evidence (unoptimised Stage B plus optimised Stage C) has not been separately ruled** (docketed earlier as G-7) |
| F-R (1) | INCOMPLETE — STATE CEILING REACHED; establishes no property | ruled INCOMPLETE (AD-1-R5 §0, ARCHITECT-RULED); discharged only by AD-1-R5 (Owner adopted; record `1492e185`) |
