C1 C25 — WITNESS-BOUND MUTATION CONTROL KILL ACCEPTED (33 OF 35); C7 MUTATION_CONTROL_BLOCKED; WITNESS-BOUND C4 AUTHORIZED (G.5, G.6)

# Architect ruling record — C1 and C25 accepted (witness-bound, AD-1-R4 G.3); 33 of 35; C7 blocked; C4 authorized (G.5, G.6)

Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission P3-C11-DR01-D2A1-MUTATION-WITNESS-C4-01, from section 2 (BINDING ARCHITECT RULING), section 3 (AUTHENTICATED BASELINE), section 4 (FROZEN C4 MUTATION) and section 15 (ALLOWED FINAL STATUS) of that mission as issued by the Sole Nexus OS Architect (ChatGPT GPT-6) via the Owner.

This file is evidence of the Architect's ruling.
A string is not authority.

Claude Code does not independently accept any control.

## The ruling, as issued (mission section 2)

> Evidence commit reviewed:
>
> 74b3f485c78d0d964b85ff958c09c69d85058ad5
>
> Independent Architect rulings:
>
> C1:
> ACCEPTED MUTATION CONTROL KILL
>
> Recording:
> C1: P1 (first failure E3; witness)
>
> C25:
> ACCEPTED MUTATION CONTROL KILL
>
> Recording:
> C25: P7 (first failure E4; witness)
>
> Campaign:
> 31/35 -> 33/35 ACCEPTED.
>
> Remaining:
> C4, C7.
>
> C7:
> MUTATION_CONTROL_BLOCKED.
> E11 observed; required P6 remains unmet.
>
> C7 is excluded from this mission.
>
> No accepted control may be rerun.

## The evidence the ruling cites (immutable; read from git objects at its evidence commit; not republished)

Evidence commit `74b3f485c78d0d964b85ff958c09c69d85058ad5` (tree `a1201237e741336bae0f4bcd3c1dcd2b892a2935`, single parent `e41c84b1c41dded6cc9816f11f6603f5d80587a9`), review ref `refs/heads/review/p3-c11-d2a1-mutation-witness-c1-c25-01`, packet `docs/evidence/p3-candidate11/d2a1-mutation-witness-c1-c25-01/**`.

| Control | Ruling | Change (catalog) | Configuration; primary first failure; named property; discovered (expanded) states | RESULT.txt (historical executor record) | Blob | sha256 | First line (unchanged) |
|---|---|---|---|---|---|---|---|
| C1 | ACCEPTED MUTATION CONTROL KILL | the count becomes a flag | F-1; first failure E3 (primary, as printed); P1 by the witness; 5,794 (1,553) | `docs/evidence/p3-candidate11/d2a1-mutation-witness-c1-c25-01/receipts/81-C1/RESULT.txt` | `d7c4cb692a00421cac7ebf0c82f9671ad64630be` | `3e178b744526ca9b3fc626675ecbb2ac6601193fd49797e3855a578828316967` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C25 | ACCEPTED MUTATION CONTROL KILL | a gating append answers Committed on Commit::Unknown | F-1; first failure E4 (primary, as printed); P7 by the witness; 4 (0) | `docs/evidence/p3-candidate11/d2a1-mutation-witness-c1-c25-01/receipts/82-C25/RESULT.txt` | `84a78ca7b0e1bddaa82e6667bc3925be3728812d` | `390c4a574ee83e84f2afc193ea09a46db1513c56e1db6487d8356e4085f98f63` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |

| Control | Pinned mutant executable sha256 | Primary classification (blob) | Primary raw output sha256 | Witness classification (blob) | Witness raw output sha256 | Healthy-witness link (blob) | Restored-side link (blob) | G.3 record (blob) |
|---|---|---|---|---|---|---|---|---|
| C1 | `f89bf7ba397c73b37e43bc6aae8851fd2ff88e5928a8ecdd69e967956d21f23c` | `9d9ef46ec2992f20390df76c1891df8ca03b6b83` | `f092dab461f063fea17925149b6266a102bf15c4cd8d9f7230ccc02f4c839c2a` | `a3675b914b83f97686fd1b4835c00b2f90203fe8` | `e3aebea065e409289bf2518f4dea4669867017c9efafd5f841620024e8514cfd` | `37c45967c64acffcefbac5b856c8985b82194c00` | `c948aa211e8ee105ea0e8769aec54598c3357ccb` | `cef4dd8ff26cb9719a0668c0435a8a2f251669d6` |
| C25 | `a0d6fc9d870b09d8bf4a17f1867d4a7bbae64ce21961d6133526c4e84d0ec315` | `e021bf2c5beca3e2160d10d0c8eeba65f9871e94` | `91dc90ab09aecf8750a4a99a6557165574e42da34c7fe0071a1cd755fb4a9f65` | `db601eaf8dfff3225b5250a07e00d1ebeae2c412` | `501cd4cd8776c4a1242ecd1110544f012e662d54c8cab3e4174c474f73222219` | `2825f5489a7229792aebb286cf826a4fb36ea754` | `7ad097de6dda51c2c7c9a761760c4438366cb855` | `c8392ed30ab597efdc2683def122a68f021c9571` |

The first lines above are the executor's historical labels; they are not relabelled. Acceptance is this ruling's, not the receipts'.

## Campaign count

Architect-accepted mutation-control kills: **33 of 35** — Tranche 1: C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b; Tranche 2: C23, C10, C16, C26, C3a; Tranche 2 continuation: C5, A3, A6, C8, C15; Tranche 3: C2; Tranche 3 continuation: C24, C3b, C3c, C6; Tranche 3 tail: A1, A2, C20, A5; witness-bound: C1 ('C1: P1 (first failure E3; witness)'), C25 ('C25: P7 (first failure E4; witness)'). A4: SATISFIED — ARCHITECT ACCEPTED (separately; not kill-required).

Outstanding (two): C4 (this mission, witness-bound under AD-1-R4 G.5 and G.6), C7 (MUTATION_CONTROL_BLOCKED: E11 observed; required P6 unmet).

| Earlier record | Path | Blob | sha256 |
|---|---|---|---|
| C1/C25 INDEX.md | `docs/evidence/p3-candidate11/d2a1-mutation-witness-c1-c25-01/INDEX.md` | `00c0cef66287669362c0bdf1fc7714fa77809676` | `d08e6bde8ac1e087a42285967d53c73d62039d8f8fc3e665cdf67dfaa1d0c913` |
| C1/C25 TRANCHE-SUMMARY.md | `docs/evidence/p3-candidate11/d2a1-mutation-witness-c1-c25-01/TRANCHE-SUMMARY.md` | `a2484f10a1d184d7f33dc97a29648f1f4e9444f7` | `a49d19f581ea6156f39d13550b8442cd87f67981c25ad59d7271e0b4c77cfd76` |
| C1/C25 SHA256SUMS | `docs/evidence/p3-candidate11/d2a1-mutation-witness-c1-c25-01/SHA256SUMS` | `1961f99057b1ef18a5defe0c3915201c0ace8f14` | `12292c91a166895ef183cc1e55aa0c4459ce65f603bd48494e061944b170ed64` |
| Tranche-3 tail acceptance (as recorded in the C1/C25 packet) | `docs/evidence/p3-candidate11/d2a1-mutation-witness-c1-c25-01/documents/ARCHITECT-RULING-TRANCHE3-TAIL-ACCEPTANCE.md` | `bd99bd84e7f81815ced66bc6d23e9f4dbc88d493` | `5364fbe49daecf89eb7a4784d43381eab9d187c9f8a2ed979cbf5b937942e0b1` |
| campaign status before the C1/C25 mission | `docs/evidence/p3-candidate11/d2a1-mutation-witness-c1-c25-01/documents/CAMPAIGN-STATUS-BEFORE-WITNESS-C1-C25.md` | `f94dc01ca464549b0a8e15a227b4633f3e2a84ba` | `4b594800c63f2ad6be8909e8abbe93d4523a20eb1795330d21ac2f282f89a412` |

## What this mission is authorized to do (mission sections 1, 4-14)

- Execute C4 only, once, from Candidate C `d5964ee5326c2733177307db6f507328023d361a` (tree `51bac5fd335999cc995c0ab9f582a30244973ef0`; source frozen), under AD-1-R4 G.5, G.6, F.1, F.2, I and K.
- One fresh disposable worktree and fresh worktree-local target; exactly the frozen C4 patch (sha256 `24ac7add3716a030dd710539e6082e197fdc5bd624c26a595b378b3971e0838d`; Test 1 of `Publish::acknowledgement` removed, 16 lines removed and none added; mutant protocol.rs blob `f3793c2ff61f8adbf43fe17d6b5237b49bc507b0`, sha256 `4037de6e33a901f933f118dde5a395b70b1e5cd7e58e54dcf0078d4a40f335b2`); one build (zero warnings and errors, at most 2 jobs, offline, no extra features, cfg or rustflags; 16 GiB, no swap, 900 s).
- On the pinned C4 mutant executable: the type-(a) primary, `ledger::protocol::model_tests::full_f8` on the original explorer (F-8, 30,000,000 states, 2700 s), whose first property is recorded exactly as printed (P4 or P8 admissible; any other stops for an Architect ruling; PASS, INCOMPLETE or a FAIL beyond the ceiling kills nothing; the primary alone never establishes both properties); then the type-(c) mutant witness `ledger::protocol::model_tests::c4_witness_p4_and_p8_on_f8` on the SAME pinned executable, with `--exact --nocapture --test-threads=1`, requiring BOTH exact positive lines at their frozen loci, the header and the two separate lines of other properties, its own verdict panic, one failed test and a normal exit 101, and the G.6 offered-versus-consumed rule verified from the raw output at every step of both scenarios.
- After the mutant receipts: restoration to exactly C; then the accepted healthy type-(e3) witness (Stage 2 `89326b859792a442c1853c3d35d30a48301c009e`: receipts/52-witness-C4) and the accepted F-8 unchanged-core type-(e1) receipt (Stage 2: receipts/64-full-F-8) linked, never rerun.
- The result is only: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING, MUTATION_CONTROL_BLOCKED, INCOMPLETE — ARCHITECT REVIEW REQUIRED, or STOPPED BEFORE EXECUTION.

## What this record does not do

- It authorizes nothing for C7 (MUTATION_CONTROL_BLOCKED): it is not executed, rebuilt or reinterpreted here.
- It reruns nothing accepted: none of the 33 accepted controls, A4, the unchanged-core configurations or the accepted healthy witnesses.
- It does not accept C4: its result in this mission is an executor result pending Architect review. Only the Architect may rule the count beyond 33 of 35.
- It does not accept D2A-1, complete the campaign, integrate anything or complete Phase Three.
- It changes no receipt, label, catalog entry, patch or source.
