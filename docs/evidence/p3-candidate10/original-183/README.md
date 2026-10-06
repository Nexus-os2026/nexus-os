# Adapted Candidate-10 canonical campaign, version 1

This is a new, versioned adaptation. It is **not the original harness** and
is not byte-for-byte the original 183. `manifest.json` retains M01–M183,
every corrected historical edit/command, source hashes, current edits,
intended failing tests, old/new meanings and review notes.
`control-review.md` is the complete static adaptation table, not run results.

## Provenance and historical truth

Historical Candidate 8: `7d1eacedf2ef7962099ffe308c5e1051a4342951`, tree
`1b894f764cd75dac7e424f247a468a0b45d6149d`.
Corrected historical source:
`/home/nexus/NEXUS/phase3-final-evidence/05-mutations/candidate8-7d1eaced/rerun-M160/mutations.py`.
SHA-256: `9bdd045121e10fa03d35b300283b0bbc487b3ef29d69ad7e00d33a6a8c607623`.
The pre-correction source SHA-256 is
`f955f4faac2f2eafed0d2ec6146a06a82a9cdd5f8adeb3a67cf4d36a7ac06886`.

The historical complete run executed 183: 182 killed and M160 survived.
The corrected isolated M160 rerun executed one and killed one. There was
never one complete historical 183/183 run. Those results are not combined.
Historical source and evidence are preserved in place, unchanged.

## Adaptation decisions

The mission approved semantic re-anchors for M01, M04, M42, M60, M62, M63,
M73, M101, M110, M136, M147, M157, M158, M159, M165, M168 and M173.
M146 keeps its formatter mutation and selects the renamed live GTK test,
without the obsolete `--ignored` flag. Corrected M160 removes both the early
quitting refusal and the cancellation callback's quitting refusal.

M166 is **VERSION-SUPERSEDED** by the Architect. Historical M166 protected
same-client drag-image origin return. Candidate 10 must reject same-client
identity as a substitute for the exact source window. Its mutant restores
that forbidden equivalence and the existing Own/ImageAbove cases must kill
it. The mutation-local helper uses Candidate 8's actual X11 connection
resource-id mask, not a guessed bit shift. No same-client helper exists in
the unmutated production baseline. M166 is NOT a faithful historical mutation.

The remaining 165 controls were reviewed for property and intended-test
continuity. M146 requires the mechanical test-command re-anchor above;
164 retain their historical edits. Guard changes were compared for owner
stop ordering, new goal/session identity, native approval layout, proxy
policy/observation additions and async command boundaries. Static review
alone does not prove a kill. The manifest identifies the actual judge for
every control; a full baseline and full run are still required.

T60b isolates commitment wall expiry from live-grant expiry. T165b uses
explicit POINTER_ROOT with every corner covered and an ungrabbed keyboard.
T173b exercises real goal timeout and a usable governed tool action, with
and without a replacement goal. M159 uses a test-only channel inside the
start-turn wait, replacing the one-second scheduling assumption.

## Execution and evidence contract

Use only the isolated `repair/p3-candidate10-closure` checkout and run no
other Cargo/build/source-changing job against it or its target directory
while the campaign runs. Use a private HOME, CI=1, no DISPLAY or session bus,
the pinned repository Node runtime, and installed Xvfb and real Chrome.
The environment JSON is an explicit subprocess environment, not an overlay.
It needs PATH, HOME, CARGO_HOME, RUSTUP_HOME, CARGO_TARGET_DIR and CI; normal
locale and Cargo profile/build settings may also be supplied. Do not include
secrets. A warm target cache is permitted; tracked input mtimes are refreshed
before baseline/campaign so another worktree's artifacts cannot be assumed
current. File bytes and Git refs are not changed by this refresh.

Commands (substitute reviewed absolute paths and the exact implementation SHA):

```text
python3 -B campaign_test.py
python3 -B campaign.py check --root CHECKOUT
python3 -B campaign.py baseline --root CHECKOUT --sha SHA --out NEW_BASELINE_DIR --environment ENV_JSON
python3 -B campaign.py campaign --root CHECKOUT --sha SHA --out NEW_CAMPAIGN_DIR --environment ENV_JSON --gates GATES_JSON --baseline BASELINE_JSON
python3 -B campaign.py review --out CAMPAIGN_DIR --decisions SEMANTIC_REVIEW_JSON
```

The baseline subcommand does not apply mutations. Every distinct command
must execute its intended judges and pass. The campaign additionally requires
all mission baseline gates, bound to the same SHA/tree and runner/manifest
hashes. `REQUIRED_GATES` in the runner lists the minimum gates. A gate receipt
has `identity`, `status: PASS` and `jobs`; each job contains its exact command,
returncode 0, log path relative to the receipt and log SHA-256. Existing gate
output must establish the requested checks actually ran; a zero status alone
is not sufficient to certify a job. In particular Builder packaged tests
must cover integrity and governed launch, both webview profiles must reach
the live marker, and CI-mode live tests may not skip missing facilities.

There is no subset or resume campaign mode. The campaign writes one ordered
M01–M183 observation sequence. Compiler errors, infrastructure failures,
timeouts and zero-test invocations do not count as kills. An intended test
failure remains unreviewed until the separate semantic review quotes its
failure and explains how it demonstrates this control's property. The review
JSON binds to the campaign SHA-256 and has one entry per ID with log SHA-256,
`verdict`, literal `quote`, and `explanation`. Final verdicts are KILLED,
SURVIVED, COMPILE ERROR or INFRA ERROR. `reviewed-results.json` can pass only
with 183 KILLED entries from this single complete, restored campaign.

Every mutation's source bytes are backed up outside the checkout and
restored in `finally`. Per-control before/mutant/after SHA-256 and status,
whole-campaign before/after hashes, SHA/tree and full logs are preserved.
A restoration failure stops execution. An interrupted or incomplete campaign
cannot be promoted by collecting isolated reruns. Preserve its evidence and
return it for review. SIGKILL/power loss cannot run Python finally: preserve
backups and require explicit recovery review before any further campaign.

Only after this adapted campaign succeeds may the unchanged Candidate-9
52-control campaign run, separately, against the same SHA. Never merge the
result sets or substitute the 52 controls for these 183. Local results do
not authorize phase progression or integration.
