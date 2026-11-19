# Q-M3 evidence: frozen values and the replay (QM3-H1)

Spec: `/home/nexus/.m2d/replan/q10-mini-spec-v2.md` (accepted by SB3E-R156), Step 0 `/home/nexus/.m2d/qm3h/step0-report.md`,
split and rulings SB3E-R168. This file has two halves:
- QM3-H1 writes this one: the frozen values and the replay contract.
- QM3-H2 adds the harness procedure and the Owner steps.

Nothing here authorizes a live run. Phase M needs the Owner's approval (O-2, D13).

## Frozen request values (bound by hash; M3-O20 at `mother/src/page_builder/oracles.rs:27-162`)

| Item | Value |
|---|---|
| `SYSTEM_M3` (593 B) | sha256 `ec1b50642f9c77a54318fcd94daa8ac8c7424cffda1181fd62d2735d4a55ede9` |
| `TASK_M3` (559 B) | sha256 `d63a0249f74fcc32f2cca81894cb6beeb49b6d9d5b49b7a6db7187daa4211528` |
| `SCHEMA` (574 B) | sha256 `e7d163a2958ae6211fefaa0f69028915071654657e0fdb2e484c26e9fd8b0ce1` |
| Correction prefix (34 B) | sha256 `0b521a8fb752f8110b647faaf785b21728bfdd6b28f8015d1825986c29f3f3d6` |
| The 8 correction blocks | `m3-design-approved.md:682-689` (`fixtures.rs` `CORRECTIONS`) |
| The 11 fixtures | `m3-design-approved.md:693-705`; the facts/notes hashes are asserted again in `qm3_body_goldens` |
| Fixture marker | `0123456789abcdef0123456789abcdef` (`page_builder/fixtures.rs:4`) |
| Decoding | `stream=false;think=false;truncate=false;temperature=0;num_ctx=8192;seed=7`, no `num_predict` (`kernel/src/inference/ollama.rs:354-389`) |
| Pin | `qwen3.6:35b-a3b`, digest `a7eb95c53bcf96b4bdd008d0fab4a5dac88047d9c1a7a9ab88ed453423fbd87c`, runtime `0.40.13` (`mother/host/src/hands.rs:76-89`) |
| PageBuilder identity v2 | `c5b838e3070d2ffcfb20f15e4420cca44c4f17456ff08a2232c0cd2a91c5b978` (Cq v6 §3.1) |
| Rendered bytes | QM-MIN 1,301; MAX first requests 3,686; QM-C1..C8 3,745-3,801 |
| Measurement floor | `SCAFFOLD_FLOOR` 256 (`kernel/src/inference/request.rs:31`, read at `ollama.rs:448`); a reply counting 255 gives `model_truncated` on the replay path |

## The 63 masked body goldens

These are the exact `/api/chat` bodies of the real path, with every occurrence of the call's marker (4 per body) replaced by the fixture marker. There are 7 bases, each with the first call (0) and corrections 1-8 (`cb_fixtures::rejected(i)` served as the first reply).

| Base | Notes, facts |
|---|---|
| N | NOTES, F-BAKERY |
| I | INJECT, F-BAKERY |
| S | STRONG-W, F-BAKERY |
| MIN | NOTES-MIN, F-MIN |
| MAX-A, MAX-L, MAX-C | the MAX fixtures |

The literals are `GOLDENS` in `mother/src/adapter/qm3_replay_tests.rs`, in base order then 0-8.

Table sha256 = `5d5d6e7bd81cb3ea306a219b83f89b76a52b16b59dbc461f5f2d1507fde131a6`. It is the SHA-256 of the lines `<base> <n> <hash>\n` in that order. The harness `golden` mode (QM3-H2) computes the same table independently from the frozen texts.

## The replay (`mother/src/adapter/qm3_replay_tests.rs`, `#[cfg(target_os = "linux")] #[cfg(test)]`)

**Path:** the real `PageBuilderLane::invoke_in` → `GovernedPort::model_call`, which takes the Legacy `SessionPort` arm (`session_port.rs:206`) → `VerifiedOllama` → `chat_body`. The c1b loopback fake captures the bodies; it serves the production pin, registered through the unchanged string route (`c1b_port_oracles.rs:211-215`).

The caller maps attempts as the coordinator's caller does (`coordinator/model_calls.rs:464-500`).

There is no fixture and no floor input: the floor is whatever the built code applies.

- **One broker home per process.** Each mission runs in its own child of the test binary (`c1b_port_oracles.rs:61`).
- **The 8-attempt ceiling per run.** The broker admits 8 attempts per run (`inference.rs:67`), so each base runs as 3 missions: 0-3, 4-7 and 8.

| Test | What it proves |
|---|---|
| `qm3_body_goldens` | **B1+B2.** The 63 bodies of the real path equal the literals. Each body carries its injected marker exactly 4 times. **B3:** the re-derived fixtures equal the M3-O20 hashes |
| `qm3_step_replay_is_consistent` | The same step in two fresh processes gives the same body hashes and outcome. A reply under the built floor is refused |
| `qm3_step` (`#[ignore]`) | The measurement step, run only from the prebuilt binary with `--exact` |
| `qm3_gq1_no_production_reference` | **G-Q1.** Nothing in `mother/`, `crates/`, `kernel/` or `.github/` names `tools/qm3` or the measure marker, except this test file |
| `qm3_gq2_declaration_is_test_only` | **G-Q2.** The one declaration is exactly `#[cfg(target_os = "linux")] #[cfg(test)] mod qm3_replay_tests;`. The step variable `QM3_STEP` occurs nowhere in those trees outside this file (SB3E-R168) |
| `qm3_gq6_no_cq_coupling` | **G-Q6.** No `ModelRequest::new(` and no `ModelPinSpec {` in this file |

### `qm3_step` input and output

**Input.** `QM3_STEP=<path>` names a JSON step file:

```
{"shape": "N|I|S|MIN|MAX-A|MAX-L|MAX-C", "markers": ["<32 hex>", ...], "replies": [{"status": 200, "path": "<file beside the step>"}], "scripted": null | 0..7}
```

- `markers`: the markers so far plus the proposed next one.
- `replies`: the exact earlier reply bodies, served in order.
- `scripted`: the frozen first rejection, for QM-C1..C8.

There is no floor field. The first unserved chat is answered 503.

**Output.** One line:

```
QM3STEP {"bodies": [<sha256>...], "next": null | {"sha256", "correction"}, "outcome": {"kind": "file"|"fail", "code" | "page_sha256", "page_bytes", "check", "check_facts", "email_scan"}}
```

## Plants (local scripted runs, recorded by sha; SB3E-R134.2 form)

| Plant | Mutation | Killer |
|---|---|---|
| P-Q1 | one golden literal changed | `qm3_body_goldens` |
| P-Q3 | the step ignores the injected marker | `qm3_body_goldens` (the marker count, B1) |
| P-Q8 | the mod line loses `#[cfg(test)]` | `qm3_gq2_declaration_is_test_only` |
| P-Q8b (SB3E-R168) | a production file reads `QM3_STEP` | `qm3_gq2_declaration_is_test_only` |
| P-Q11 | the replay module gains a `ModelRequest::new(` call | `qm3_gq6_no_cq_coupling` |
