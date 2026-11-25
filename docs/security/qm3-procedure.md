# Q-M3 evidence: the live harness procedure and the Owner steps (QM3-H2)

Spec: `/home/nexus/.m2d/replan/q10-mini-spec-v2.md` §§3, 6-8, 10-11 (SB3E-R156), split and rulings SB3E-R168.

The frozen values and the replay contract are QM3-H1's half (`docs/security/qm3-evidence.md`). That half is kept in its own file so the two units merge independently. Nothing here authorizes a live run.

## The harness (`tools/qm3/qm3_live.py`, stdlib only, run as `python3 -I -B`)

- **Isolation.** It is not part of any Cargo package, build script, installer or workflow (G-Q1, in the H1 replay tests).
- **Network (G-Q3).** It speaks only to `127.0.0.1:11434` through one `http.client.HTTPConnection` site, on `/api/{version,status,tags,show,ps,chat}`.
- **Subprocesses (G-Q3).** It runs subprocesses only with literal argv: `/usr/bin/pgrep`, `/usr/bin/ss`, `/usr/bin/nvidia-smi --query-gpu`, the replay test binary and, in `confirm`, the installed binary. There is no PATH lookup anywhere (SB3E-R172 F6).
- **Pinned binaries (SB3E-R172 F6/D2).** The replay binary and confirm's binary must be absolute paths with no symlink in them, and regular files. Each is opened once, hashed from that open file, and the hashed bytes are copied to a 0700 private file that is the one run.
- **Confirm's program (ruled on G-Q3; SB3E-R171).** `confirm` may start exactly one more program: the installed binary at the path given to `--binary`. No other new program is allowed. Before starting anything, `confirm` computes that file's sha256 and compares it with `--artifact-sha`, the approved artifact's hash. A mismatch refuses: non-zero exit, a ledger line, and no confirm record.
- **Environment.** It sets no variable, flag or file that a Mother binary reads. `QM3_STEP` is read only by the cfg(test) replay test binary (SB3E-R168).

| Mode | What it does | Network |
|---|---|---|
| `golden` | The 63 masked body hashes from the frozen texts. It refuses unless they equal table sha256 `5d5d6e7b…` | none |
| `preflight` | O-1, read only: `/api/version`, `/api/status`, `/api/tags`, `/api/show`. One line `QM3 RUNTIME version=… cloud_disabled=… digest=… thinking_capability=…`, then PASS/FAIL per line | metadata only |
| `run` | Phase M (O-2): the 31 cases of §6.1, once, top to bottom | local runtime |
| `verify` | Recompute `evidence_sha256` from the committed record and `raw_bundle_sha256` from the bundle | none |
| `freplay` | Phase R2: refused unless the bundle's sha256 equals the record's `raw_bundle_sha256` (SB3E-R173). Then each case's final step is replayed on the code head with the recorded replies. The step has no floor field, so F reaches it only through the production record | none |
| `confirm` | Phase C (O-5): the binary's sha256 equals `--artifact-sha` (C-1, checked before it starts); it carries no measure marker (C-2); one registration-only startup under a PTY prints `MODEL VERIFIED … identity=` equal to the record's identity (C-3); the committed F-replay record (`--freplay`) is CONFIRMED for this evidence record (C-4, checked before the binary starts; SB3E-R173). It types `quit`; no chat is made | metadata only (the binary's own registration) |

**G-Q5.** `run` and `freplay` refuse unless all three hold:
- the harness file's sha256 equals `--harness-sha` (the frozen value recorded below);
- the replay binary's sha256 equals its `--replay-meta`;
- that meta's `built_from` equals `--candidate`.

### Per live call (§6.2)

0. **Ledger START** (SB3E-R172 F2), before the first request: `{"kind":"run","verdict":"STARTED"}`. Any exit that is not a verdict or an abort line writes ABORTED (`interrupted` or `harness_exception`). Every ABORTED line keeps the ids of the cases already missed (SB3E-R173).
1. **Quiet.** No compiler process, load1 ≤ 4.0, MemAvailable ≥ 6 GiB, swap-in growth ≤ 1 GiB, no other client on :11434, no other loaded model, no Mother process. It waits up to 600 s; if the host is still not quiet, the run is ABORTED.
2. **Runtime check.** It is the same as preflight, every call. Any change: ABORTED. **Q-8** (SB3E-R172 F4): after the first live call exactly one `llama-server` runner exists; its PID and `/proc` start time never change, checked before and after every call and at the end. Any change: ABORTED.
3. **Marker.** A fresh uuid4 hex, different from the case's previous marker.
4. **Bind.**
   - Write the step file and run `qm3_step` (B1: equal body sha256, the same correction code).
   - B2: the masked hash equals the golden for its shape.
   - Any failure: ABORTED before sending.
5. **Send.** `POST /api/chat` with a 180 s timeout. Keep the exact reply bytes. **Q-4 watchdog** (SB3E-R172 F4): during the call, every second, MemAvailable ≥ 6 GiB and swap-in growth ≤ 1 GiB; a trip shuts the connection (the runtime cancels the request) and the run is ABORTED. A watchdog that died (not tripped), checked before and after the call, also ends the run ABORTED (SB3E-R173).
   - **A timeout is a miss** (SB3E-R168): the case ends with outcome `live_timeout`, and the run becomes BLOCKED.
6. **Decide.** Run `qm3_step` again with the reply appended. Its bodies must start with every body sent so far (SB3E-R172 F5), else ABORTED. `done` ends the case with the production outcome; `next` means the production lane would correct.
7. **Scan.** Check the reply content for the case's forbidden tokens.

### After the last case

1. **Floor.** The floor = the smallest QM-MIN first-call `prompt_eval_count` − 32, kept only if positive (checked).
2. **Consistency replay.** Every final step is replayed again; the outcome and every body hash (the scripted call 1 for QM-C, then each body sent) must be equal, else BLOCKED (SB3E-R172 F5).
3. **read_chat** (SB3E-R172 F1). Every call must pass production read_chat: only a page or a draft-level refusal proves that. Any other outcome (a transport refusal such as `model_truncated`, a timeout, kind `other`) is the miss `read_chat`, in every case. freplay's CONFIRMED also needs it at F.
4. **Verdict.** Evaluate §8.1, then write the canonical record `evidence-<run_id>.json` and the ledger line. The record is content-free: hashes, counts, codes and booleans only, with no model text. The raw bundle stays in `/home/nexus/.m2d/qm3/<run_id>/` (mode 0700) and is bound by `raw_bundle_sha256`.

## QM3-H3: the confirm-time harness (SB3E-R196.3)

- **Run under umask 077** (O-2 and O-5). The harness now creates each case folder and its `steps/` with mode 0700 itself (SB3E-R191), so the replay no longer depends on the umask. The umask still keeps the ledger, the record and the reply files private.
- **Swap cleared before O-2** (SB3E-R192). The Owner runs `sudo swapoff -a && sudo swapon -a`; the Architect checks that swap used is about 0 and MemAvailable is above the 36 GiB start floor before giving the go.
- **The confirm form.** `confirm --binary B --artifact-sha HEX --record F --freplay F --golden-ci ID --root DIR --notes F --facts F`. The binary is started with the M-5a page flags: `--home H --workspace W --mission one-page-site --notes F --facts F`. `--notes` and `--facts` must be absolute, symlink-free regular files that group and others cannot write, each in a private 0700 folder of the operator. Anything else is refused before the binary starts (ledger code `page_fixture`).
- **The confirm-time pin.** O-5 runs the QM3-H3 harness; its sha256 is recorded in the QM3-H3 hand-back, not in this file. The Phase M record stays bound to `5f4548e4…`, the harness it ran.

## Owner steps (§11)

| # | Step | The Owner sees |
|---|---|---|
| O-1 | Model present and runtime configured: Ollama 0.40.13, `qwen3.6:35b-a3b` digest `a7eb95c5…`, cloud disabled. `preflight` reads `/api/version`, `/api/status`, `/api/tags` and `/api/show` only (SB3E-R168). Already satisfied on this host (SB3E-R168) | `QM3 RUNTIME version=0.40.13 cloud_disabled=true digest=a7eb95c5… thinking_capability=true`, PASS per line |
| O-2 | Approves the live run (D13) in a scheduled quiet slot. The operator is the Owner or someone the Owner names | `QM3 PREFLIGHT …`, `QM3 PLAN cases=31 calls<=54 bound=180s`, one line per call and per case, then `QM3 FLOOR`, `QM3 VERDICT`, `QM3 EVIDENCE id=QM3-PB-1:<hex>`. No model text is shown |
| O-3 | Merges the integrated Cq-2 stack PR (record, F-replay CONFIRMED, Cq units) after U22, gate C, review, CI, audit and Gate Box | the PR |
| O-4 | Installs from the approved merged SHA | the installer's hash lines |
| O-5 | Approves one registration-only startup run | `QM3 CONFIRM binary=… measure_marker=absent`, `QM3 CONFIRM identity=c5b838e3… record=QM3-PB-1:… match=yes PASS`, `QM3 CONFIRM golden_ci=<run> freplay=CONFIRMED PASS` (C-4; the CI run id is the operator's, printed, not fetched) |

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
| P-Q13 (SB3E-R171) | confirm ignores an artifact sha mismatch | `test_confirm_refuses_another_binary_before_starting_it` |
| P-Q14 (SB3E-R172) | a QM-C read_chat refusal is ignored | `test_a_read_chat_refusal_is_a_miss` |
| P-Q15 (SB3E-R172) | the ledger START line is skipped | `test_an_escaping_failure_leaves_started_then_aborted` |
| P-Q16 (SB3E-R172) | a PATH lookup is restored (`pgrep` by bare name) | `test_gq3` |

The frozen harness sha256 is recorded in the QM3-H2 hand-back and is frozen with P0 (`run --harness-sha`).
