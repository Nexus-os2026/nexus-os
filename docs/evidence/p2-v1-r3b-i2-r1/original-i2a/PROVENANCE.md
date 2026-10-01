# Original I2A artifacts

The files in this directory, apart from this note and `PROVENANCE-CHECK.log`, are
the artifacts produced in the P2-V1-R3B-I2A session. They concern I2A candidate
`0eb2a947e839b5639d9688da4e6b51b5e72a4d13` (tree
`da9fa4175c05e5eeae52ecbaf71201c9667045f3`, parent
`b7693364b1601a00be12426cec8871c96a205953`), which is a published review
candidate that has not been accepted.

They were kept in that session's disposable job storage, then copied here
unchanged under their original names; each copy was checked byte for byte with
`cmp`. The top-level `SHA256SUMS` lists the SHA-256 of every copy, which is
therefore also the SHA-256 of the file as originally produced.

They are evidence about 0eb2a947, not about the I2-R1 candidate 45898e05.
Nothing here was rerun for I2-R1 except the two checks below.

| File | What it is |
|---|---|
| `i2a-golden-vectors.py` | The independent stdlib golden-vector generator as I2A used it. `--rust` prints the literal tables; with no argument it prints a listing. |
| `i2a-vectors.rs.txt` | The generator's `--rust` output: the `RECORD_VECTORS` (50) and `REQUEST_VECTORS` (4) literals pasted into `tests/phase2_custody_codec.rs` at 0eb2a947. |
| `i2a-all.txt` | The generator's listing: 50 record (`R`) and 4 request (`Q`) lines. |
| `i2a-records.txt`, `i2a-requests.txt` | An earlier listing, written before I2A expanded the record vectors to 50. It has 48 record lines, all present in `i2a-all.txt`, and 4 request lines identical to those in `i2a-all.txt`. |
| `i2a-negative-controls.py` | The I2A negative-control script as it was run: 30 counted controls plus 2 structural runs. |
| `i2a-negative-controls/` | That run's `run.log` (stdout plus exit status), `summary.json`, and one log per control. |
| `i2a-final-validation.log` | I2A's final validation, run on the pre-commit sources. Its head records the SHA-256 of the six envelope files. |

## Correspondence with 0eb2a947

`PROVENANCE-CHECK.log` (written for I2-R1) compares three sets of six envelope
file hashes with the blobs committed as 0eb2a947:

- the hashes at the head of `i2a-final-validation.log`;
- the hashes `summary.json` records before the controls;
- the hashes `summary.json` records after the last restoration.

All three sets match the blobs exactly, so the I2A logs were produced on exactly
the sources committed as 0eb2a947.

`../i2-r1/golden/revalidation.log`, section 4, reruns `i2a-golden-vectors.py`.
Its `--rust` output is byte-identical to `i2a-vectors.rs.txt`, and its `R` and
`Q` lines are byte-identical to `i2a-all.txt`. That section also shows:

- the I2-R1 generator prints the same I2A tables;
- `RECORD_VECTORS` and `REQUEST_VECTORS` are unchanged between 0eb2a947 and
  45898e05.

## What the I2A controls did not establish (Architect finding A)

I2A ran the Debug-text request digest mutation (`I2A-DEBUG-REQUEST`) only as a
structural check. Under it, `c13_request_receipts_bind_the_full_payload` passed
(`I2A-DEBUG-REQUEST-c13.log`), and only the source guard
`c16_core_digest_paths_use_the_codec` failed (`I2A-DEBUG-REQUEST-c16.log`).

No I2A test observed which digest the core actually stores in a request receipt.
I2-R1 adds that observation (`c17`) and two counted controls against it; see
`../REPORT.md`.
