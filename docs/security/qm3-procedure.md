# Q-M3 evidence: the live harness procedure and the Owner steps (QM3-H2)

Spec: `/home/nexus/.m2d/replan/q10-mini-spec-v2.md` §§3, 6-8, 10-11 (SB3E-R156), split and rulings SB3E-R168.

The frozen values and the replay contract are QM3-H1's half (`docs/security/qm3-evidence.md`). That half is kept in its own file so the two units merge independently. Nothing here authorizes a live run.

## The harness (`tools/qm3/qm3_live.py`, stdlib only, run as `python3 -I -B`)

- **Isolation.** It is not part of any Cargo package, build script, installer or workflow (G-Q1, in the H1 replay tests).
- **Network (G-Q3).** It speaks only to `127.0.0.1:11434` through one `http.client.HTTPConnection` site, on `/api/{version,status,tags,show,ps,chat}`.
- **Subprocesses (G-Q3).** It runs subprocesses only with literal argv: `pgrep`, `ss`, `nvidia-smi --query-gpu`, the replay test binary and, in `confirm`, the installed binary.
- **Confirm's program (ruled on G-Q3).** `confirm` may start exactly one more program: the installed binary at the path given to `--binary`. No other new program is allowed. Its sha256 is printed for C-1's comparison with the approved artifact; the harness does not itself compare it against an expected value.
- **Environment.** It sets no variable, flag or file that a Mother binary reads. `QM3_STEP` is read only by the cfg(test) replay test binary (SB3E-R168).

| Mode | What it does | Network |
|---|---|---|
| `golden` | The 63 masked body hashes from the frozen texts. It refuses unless they equal table sha256 `5d5d6e7b…` | none |
| `preflight` | O-1, read only: `/api/version`, `/api/status`, `/api/tags`, `/api/show`. One line `QM3 RUNTIME version=… cloud_disabled=… digest=… thinking_capability=…`, then PASS/FAIL per line | metadata only |
| `run` | Phase M (O-2): the 31 cases of §6.1, once, top to bottom | local runtime |
| `verify` | Recompute `evidence_sha256` from the committed record and `raw_bundle_sha256` from the bundle | none |
| `freplay` | Phase R2: each case's final step replayed on the code head with the recorded replies. The step has no floor field, so F reaches it only through the production record | none |
| `confirm` | Phase C (O-5): the installed binary carries no measure marker; one registration-only startup under a PTY prints `MODEL VERIFIED … identity=` equal to the record's identity. It types `quit`; no chat is made | metadata only (the binary's own registration) |

**G-Q5.** `run` and `freplay` refuse unless all three hold:
- the harness file's sha256 equals `--harness-sha` (the frozen value recorded below);
- the replay binary's sha256 equals its `--replay-meta`;
- that meta's `built_from` equals `--candidate`.

### Per live call (§6.2)

1. **Quiet.** No compiler process, load1 ≤ 4.0, MemAvailable ≥ 6 GiB, swap-in growth ≤ 1 GiB, no other client on :11434, no other loaded model, no Mother process. It waits up to 600 s; if the host is still not quiet, the run is ABORTED.
2. **Runtime check.** It is the same as preflight, every call. Any change: ABORTED.
3. **Marker.** A fresh uuid4 hex, different from the case's previous marker.
4. **Bind.**
   - Write the step file and run `qm3_step` (B1: equal body sha256, the same correction code).
   - B2: the masked hash equals the golden for its shape.
   - Any failure: ABORTED before sending.
5. **Send.** `POST /api/chat` with a 180 s timeout. Keep the exact reply bytes.
   - **A timeout is a miss** (SB3E-R168): the case ends with outcome `live_timeout`, and the run becomes BLOCKED.
6. **Decide.** Run `qm3_step` again with the reply appended. `done` ends the case with the production outcome; `next` means the production lane would correct.
7. **Scan.** Check the reply content for the case's forbidden tokens.

### After the last case

1. **Floor.** The floor = the smallest QM-MIN first-call `prompt_eval_count` − 32, kept only if positive (checked).
2. **Consistency replay.** Every final step is replayed again; any outcome difference gives BLOCKED.
3. **Verdict.** Evaluate §8.1, then write the canonical record `evidence-<run_id>.json` and the ledger line. The record is content-free: hashes, counts, codes and booleans only, with no model text. The raw bundle stays in `/home/nexus/.m2d/qm3/<run_id>/` (mode 0700) and is bound by `raw_bundle_sha256`.

## Owner steps (§11)

| # | Step | The Owner sees |
|---|---|---|
| O-1 | Model present and runtime configured: Ollama 0.40.13, `qwen3.6:35b-a3b` digest `a7eb95c5…`, cloud disabled. `preflight` reads `/api/version`, `/api/status`, `/api/tags` and `/api/show` only (SB3E-R168). Already satisfied on this host (SB3E-R168) | `QM3 RUNTIME version=0.40.13 cloud_disabled=true digest=a7eb95c5… thinking_capability=true`, PASS per line |
| O-2 | Approves the live run (D13) in a scheduled quiet slot. The operator is the Owner or someone the Owner names | `QM3 PREFLIGHT …`, `QM3 PLAN cases=31 calls<=54 bound=180s`, one line per call and per case, then `QM3 FLOOR`, `QM3 VERDICT`, `QM3 EVIDENCE id=QM3-PB-1:<hex>`. No model text is shown |
| O-3 | Merges the integrated Cq-2 stack PR (record, F-replay CONFIRMED, Cq units) after U22, gate C, review, CI, audit and Gate Box | the PR |
| O-4 | Installs from the approved merged SHA | the installer's hash lines |
| O-5 | Approves one registration-only startup run | `QM3 CONFIRM binary=… measure_marker=absent`, `QM3 CONFIRM identity=c5b838e3… record=QM3-PB-1:… match=yes PASS` |

## Harness tests and plants (local, recorded by sha; SB3E-R168: no CI job runs tools/ Python tests)

`python3 -I -B -m unittest discover -s tools/qm3 -p test_qm3_live.py`

| Plant | Mutation | Killer |
|---|---|---|
| P-Q2 | masks 3 of 4 markers | `test_b2_masks_every_marker_and_needs_exactly_four` |
| P-Q4 | the floor over all calls | `test_floor_uses_first_calls_only` |
| P-Q5 | wrapping subtraction | `test_floor_is_checked_never_wrapping` |
| P-Q6 | the encoder accepts a float (early return, not only a deleted check) | `test_canonical_bytes_and_refusals` |
| P-Q7 | a second host | `test_gq3` |
| P-Q9 | `<script` dropped from the S scan | `test_forbidden_tokens_per_family` |
| P-Q10 | ABORT skips the ledger line | `test_abort_writes_its_ledger_line` |
| P-Q12 | freplay accepts another `built_from` | `test_gq5_replay_must_be_built_from_the_candidate` |

The frozen harness sha256 is recorded in the QM3-H2 hand-back and is frozen with P0 (`run --harness-sha`).
