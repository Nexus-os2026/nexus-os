TRANCHE 3 TAIL — A1 A2 C20 A5 — MUTATION CONTROL KILL ACCEPTED (31 OF 35); C7 MUTATION_CONTROL_BLOCKED; C4 RESERVED; WITNESS-BOUND C1 C25 AUTHORIZED (C1 THEN C25)

# Architect ruling record — Tranche 3 tail: A1, A2, C20, A5 accepted; C7 blocked; C4 reserved; C1 and C25 authorized (witness-bound)

Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission P3-C11-DR01-D2A1-MUTATION-WITNESS-C1-C25-01, from section 1 (BINDING ARCHITECT RULING), section 2 (AUTHENTICATED SOURCE BASELINE), section 3 (EXECUTION ORDER) and section 10 (STOP CONDITIONS AND FINAL LABELS) of that mission as issued by the Sole Nexus OS Architect (ChatGPT GPT-6) via the Owner.

This file is evidence of the Architect's ruling.
A string is not authority.

Claude Code does not independently accept any control.

## The ruling, as issued (mission section 1)

> Independent review of:
>
> e41c84b1c41dded6cc9816f11f6603f5d80587a9
>
> Technical verdict:
>
> A1  — ACCEPTED MUTATION CONTROL KILL
> A2  — ACCEPTED MUTATION CONTROL KILL
> C20 — ACCEPTED MUTATION CONTROL KILL
> A5  — ACCEPTED MUTATION CONTROL KILL
>
> Architect-accepted campaign:
> 31 / 35.
>
> Remaining:
> C1, C25, C4, C7.
>
> C7 stays MUTATION_CONTROL_BLOCKED:
> E11 observed, required P6 not established.
>
> C4 remains reserved.
>
> No previously accepted control may be rerun.

## The evidence the ruling cites (immutable; read from git objects at its evidence commit; not republished)

Evidence commit `e41c84b1c41dded6cc9816f11f6603f5d80587a9` (tree `10b408a1432edeed5720024bd6b8807852ecb464`, single parent `c10681b3c65dd377b5a13d08140427e8b34441d0`), review ref `refs/heads/review/p3-c11-d2a1-mutation-tranche3-tail-01`, packet `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-tail-01/**`.

| Control | Ruling | Change (catalog) | Configuration; printed property; discovered (expanded) states | RESULT.txt (historical executor record) | Blob | sha256 | First line (unchanged) |
|---|---|---|---|---|---|---|---|
| A1 | ACCEPTED MUTATION CONTROL KILL | the writer confirms before it records | F-1; P1; 1,962 (430) | `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-tail-01/receipts/81-A1/RESULT.txt` | `36ebc7018c529f132ac9e5e4c4255d699bcc870b` | `46c246eaa30db42abb58fac9a9e2bf91f2edf7169e778459a7c5d9239d6e9b88` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| A2 | ACCEPTED MUTATION CONTROL KILL | admission is split into two steps under two separate holds of the test's lock. Step one takes a `view` and keeps only its low 16 bits: phase, session, latch, cells, handover fault. Any actor may step in between. Step two takes a second `view`, keeps only `unapplied`, reads the debt count, H and A, and decides from the kept bits of step one and the values of step two. | F-1; P1; 1,453 (277) | `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-tail-01/receipts/82-A2/RESULT.txt` | `8761e5125e3eef33bb1b616818b661564ff2e3ef` | `c8e1bab10021bcb4da2880800f44a90333d9513357d31fc6ff6739744e9bab95` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| C20 | ACCEPTED MUTATION CONTROL KILL | an explanation found already present is not marked committed | F-2; L1; 12,590,754 (12,590,754) | `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-tail-01/receipts/83-C20/RESULT.txt` | `6ceebd19d13ffd6b190640f53d656c57ed20c6cd` | `01ce97598244a5f5a8bb55abf95cc9902d8e15cb806ee108bae18f61411a8ca2` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |
| A5 | ACCEPTED MUTATION CONTROL KILL | settlement claims a Conflict obligation while the store is not Healthy | F-2; L1; 12,684,666 (12,684,666) | `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-tail-01/receipts/84-A5/RESULT.txt` | `04478cd57ce6191432eabfee05ff21fa7decd3b5` | `168b4387610002952331be1662272c73f208b0a50de3c509b844fd1c925cf617` | `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` |

| Control | Mutant (patch sha256; mutant blob) | Pinned mutant executable sha256 | Run classification (blob) | Raw output sha256 | Tranche-3 rules (blob) | Restored-side link (blob) |
|---|---|---|---|---|---|---|
| A1 | `e3a21c74babd86e1c40c58457319dc6901c7fac7a6e56d7c8f149e3389f72507`; `26c09e7ff93633095de8d7a15ee81f5f46728336` | `d58ccd31c2c42acbb065d3ef08a00f42a8966c04bd0f000f938f3b9b2bdf6b12` | `67bc4098e4e6487b013ad79edd2d231b0e1286bb` | `2a1c9413592503f43f03c0e42a1e9b53cf3b45810e1db72cf53172f422ab2244` | `780559d2ebb0090a65ae8eea701704ab8573bb3e` | `bfe421be31dbde67d0300ef6fcc90cce61e53c0e` |
| A2 | `5f1f30486da2457e2a50f0f72f1c96bfecf67e12692d47aa20d0ae3a1d677cf2`; `c0ac428f58497d8cf42458d51f86db0f2e4b84ba` | `257cb481e0de203654f7007d60ebe537e84b71dfd8c98b02b90288365249662d` | `3d073b0d3b9f8d7982e231d2594d59b671877fd3` | `d1edba58123e3301465ed89916ca80769a1d0b3ade516cd7503ed63585275e93` | `1fdd1d3a704524852a5ee4c07034cc6eeace853e` | `858475a06a6236f3557b8e3527a5861e10b877fb` |
| C20 | `5ea1889e67bc552d08c75074f3717d87a4509c56d13448d61bc574fc3d93f62f`; `7bb5aa2abbb52249efe76ab22dd921cb74dd337c` | `5433a13ba47468ff457cf665ae1383e64fdd7de26086b4a95b9ffd7c4b121a02` | `fd0015ee598bf755bb20c56053a06c857381ebd2` | `8d80f6092fca5e55f26e55d64a22637112caeda8fd3a7ef8bc5e8e3d80aa31a5` | `f0c9fd9b0d110a5706dcd2c2c8a5b0ba8f8e43e9` | `83cf05b8aeeb47d37bee2b57353eeeb859079e3d` |
| A5 | `913eba6d31d7cbe6d65f4543b9fb2d0def6ca7e7586b26209b3ee5063a2f4d32`; `cb08f5e8c62b3ce267f0688cc880261a28cc29a2` | `22de28a2f3ff3a94d480f5247523ca2139600988139e9c4769d79be874041a85` | `171d8d469829e7ff3e88869d321b5cd9e2622bd4` | `ba743f7dc3c70ee50016f201a2e3cb19cc6227e41d921c14338b27f68672e3c6` | `7212f1e420cf6174c4798fcf88c710ed4333f7d7` | `40ac1980516c49b4530f888b2725a6ed198814d2` |

The first lines above are the executor's historical labels; they are not relabelled. Acceptance is this ruling's, not the receipts'.

## Campaign count

Architect-accepted mutation-control kills: **31 of 35** — Tranche 1: C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b; Tranche 2: C23, C10, C16, C26, C3a; Tranche 2 continuation: C5, A3, A6, C8, C15; Tranche 3: C2; Tranche 3 continuation: C24, C3b, C3c, C6; Tranche 3 tail: A1, A2, C20, A5. A4: SATISFIED — ARCHITECT ACCEPTED (separately; not kill-required).

Outstanding (four): C1 and C25 (this mission, witness-bound under AD-1-R4 G.3), C4 (reserved), C7 (MUTATION_CONTROL_BLOCKED).

| Earlier record | Path | Blob | sha256 |
|---|---|---|---|
| tail INDEX.md | `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-tail-01/INDEX.md` | `14396b7a572352c34bf933c069e87185a8d365ab` | `cf8a61227b8873dc17b7ec2e27592588c0686e8bad07738356594300e4653cc3` |
| tail TRANCHE-SUMMARY.md | `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-tail-01/TRANCHE-SUMMARY.md` | `d65e81da12a55f02d92881f1a007f49b01fbbe63` | `630137aa01200a3234d570fe68ce6b3a6a27a3e55d280f7e26666fcac989dc89` |
| tail SHA256SUMS | `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-tail-01/SHA256SUMS` | `a4593032d6894318a6b440bab386000aa387585e` | `3ec5273f51d7589000050151f28170bc2a73515c2398c35da538ccaeec9b5fa4` |
| Tranche-3 continuation acceptance (as recorded in the tail) | `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-tail-01/documents/ARCHITECT-RULING-TRANCHE3-CONTINUATION-ACCEPTANCE.md` | `2a5a4309d2ec48494aa568513cc89655baa777c2` | `8a037382911967919845da590e52ade34162b638d32aeb88b4ecd9f6c138a88b` |
| campaign status before the tail | `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-tail-01/documents/CAMPAIGN-STATUS-BEFORE-TRANCHE3-TAIL.md` | `3ec7a63af0f45faaf636df42b35b30b228a2ae9d` | `4677efc54ad75db1d19c30888ea04ad885535ec0f0ca1bd1edef23eeaa5b5843` |

## What this mission is authorized to do (mission sections 2-9)

- Execute C1, then C25 only if C1 produces an admissible evidence candidate, from Candidate C `d5964ee5326c2733177307db6f507328023d361a` (tree `51bac5fd335999cc995c0ab9f582a30244973ef0`; source frozen); stop at the first unsuccessful control or gate.
- Each: one fresh disposable worktree and fresh worktree-local target, exactly its frozen patch (C1 sha256 `5ecd716a8f123d61034b0d274238898e13a116a9cc464c7645a9736934970252`, mutant protocol.rs blob `7d95dd425fcdf17b12dda2fdce461f97ba7b9a66`; C25, the narrow patch, sha256 `29a2c1e571b4f7fd278d106fe3d40a8cf912119dbe44fbbbbd92855431ecd732`, mutant blob `0456a0c1c1fe40f71593edc6ba5938a3b0c29900`, which preserves `self.failed();`, `self.rolled(io.rollback());` and `self.keep = true;`), one build (zero warnings and errors, at most 2 jobs, offline, no extra features, cfg or rustflags).
- On the pinned mutant executable: the type-(a) primary, `ledger::protocol::model_tests::full_f1` on the original explorer (F-1, 60,000,000 states), whose first failure is recorded exactly as printed (predicted E3 for C1 and E4 for C25; E3 is never counted as P1 and E4 never as P7; another first property stops the mission for an Architect ruling); then the type-(c) mutant witness on the SAME pinned executable, with `--exact`, requiring the exact positive line, header, separate E3/E4 report, its own verdict panic, one failed test and a normal exit 101.
- After the mutant receipts: restoration to exactly C; then the accepted healthy type-(e3) witness receipt (Stage 2 `89326b859792a442c1853c3d35d30a48301c009e`: receipts/50-witness-C1, receipts/51-witness-C25) and the accepted F-1 unchanged-core type-(e1) receipt (Stage 1 `f023cc3d031a2a89e5ecbd4086e15f2e50869acd`: receipts/45-full-F-1) linked, never rerun.
- Individual results are only: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING, MUTATION_CONTROL_BLOCKED, INCOMPLETE — ARCHITECT REVIEW REQUIRED, or STOPPED BEFORE EXECUTION.

## What this record does not do

- It authorizes nothing for C7 (MUTATION_CONTROL_BLOCKED) or C4 (reserved): neither is executed, rebuilt or reinterpreted here.
- It reruns nothing accepted: none of the 31 accepted controls, A4, the unchanged-core configurations or the accepted healthy witnesses.
- It does not accept C1 or C25: their results in this mission are executor results pending Architect review. Only the Architect may rule the count beyond 31 of 35.
- It does not accept D2A-1, complete the campaign, integrate anything or complete Phase Three.
- It changes no receipt, label, catalog entry, patch or source.
