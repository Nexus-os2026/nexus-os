# F-R to F-2 — stuttering-refinement proof — DRAFT

**EXECUTOR ARGUMENT — NOT ARCHITECT ACCEPTED.**

Mission P3-C11-DR01-D2A1-FR-STUTTER-R5-01. Written by the executor (Claude Code, Claude Opus 5.5,
`claude-opus-5-5`, effort High) for the Sole Architect's independent review. Read-only proof work: nothing was
built, tested or run for it. It establishes nothing until the Architect reviews and accepts it, and it changes no
contract obligation unless the Owner adopts AD-1-R5.

**What this document argues.** Every reachable state and transition of the F-R graph that the original explorer
searches maps, by a projection π that erases the replay-only history, onto the F-2 graph that the same explorer
completely searched at the exact final baseline; REPLAY's own transitions are stutters (they change nothing that π
keeps). From the accepted exact-final F-2 receipt (PASS; 12,602,670 states; every applicable property HOLDS; D1 and
L1 HOLD) it then derives, for F-R's reachable graph: every applicable safety property, D1, L1, and the complete
branch accounting.

**What it does not argue.** It does not explore the original F-R graph, does not count its states or transitions,
and does not change the F-R receipt, which stays **INCOMPLETE** (30,000,001 discovered, 26,539,923 expanded at the
30,000,000-state cap). It gives no verdict of a run.

---

## 1. Authenticated basis

| Item | Identity |
|---|---|
| Candidate C | commit `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`, single parent `71264aa81a40b86189783facc8437e0427887c16` |
| `protocol.rs` | blob `a2df2841b99cad5d452008a6d4150e9369d07e66` |
| `model_tests.rs` | blob `28085ce232827301104b5d757af042ec933976f8` |
| `unit_tests.rs` | blob `55dc66409eaccdaa93e8f0a56f2ccb14be81e114` |
| Rust inputs of the pinned executable | the 49 `.rs` files of `crates/nexus-governed-control/src` at C, blob for blob (Stage-1 `receipts/11-pin/DEP-INFO.txt`) |
| Pinned executable (reference only) | sha256 `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c` |
| F-2 receipt | Stage-1 commit `f023cc3d031a2a89e5ecbd4086e15f2e50869acd`, `final-baseline-known-runs-01/receipts/41-full-F-2/raw-output.txt`, sha256 `985a3976713bc77bdf358744ec770e6220310f2ce2ed72b3e9ae10b3fb314ab5`: `outcome: PASS`; `states 12602670 (expanded 12602670), transitions 54214081 (with a fault 3558228), states with no enabled step 3206`; 24 property lines HOLD, E10 not applicable; `D1: HOLDS`; `L1: HOLDS: 50655853 non-fault edges, no cycle; 3206 end states, each terminal (D1)`; classified PASS — admissible unchanged-core original-explorer PASS |
| F-R receipt | Stage-2 commit `89326b859792a442c1853c3d35d30a48301c009e`, `final-baseline-remaining-configs-01/receipts/69-full-F-R/raw-output.txt`, sha256 `78e44c335b6ce65f8df6aea18b71e488613403ba8c0471c5cf1c33057ab58ccd`: `outcome: INCOMPLETE`; 30,000,001 discovered, 26,539,923 expanded; classified INCOMPLETE |
| Diagnostic | Stage-1 `receipts/30-suite/raw-output.txt` (sha256 `d6f58742ea483dbb95452b186dcf72f729538e672ae60741d978b0eddc6ca3b3`): `test ledger::protocol::model_tests::e8_holds_for_stale_worker_operations ... ok` in `test result: ok. 313 passed; 0 failed` |
| Contract | P3-C11-D2A1-CONTRACT-02-R1, sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c`; line 639 (REPLAY), line 713 (C15), line 851 (F-R) |
| AD-1-R4 (governing) | sha256 `00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693` |

Every input above was authenticated by `tools/authenticate.py` (packet `receipts/00-authentication.txt`).

**Citations.** A source assertion carries `[file · item · lines · blob · item]`: the file, the enclosing item (named
by the convention of `tools/NAMING.md`), the line range at the blob, the blob, and the first 16 hex digits of the
sha256 of the item's exact bytes; the full hash and byte range are in `SOURCE-ITEM-IDENTITY-MATRIX.tsv`. Both
checkers resolve every citation (packet `receipts/`).

---

## 2. Definitions

### 2.1 The two configurations

`FR` is defined as `Config { name: "F-R", replays: 3, ..F2 }` [model_tests.rs · const FR · L255–259 · blob 28085ce2 · item a99f7e62b4f460ef]; `F2` is
`Config { name: "F-2", conflict: [false, true], faults: 2, admissions: 2, gating: 2, ..BASE }`
[model_tests.rs · const F2 · L162–169 · blob 28085ce2 · item 58f9b91152d156c9], with `BASE.replays = 0` [model_tests.rs · const BASE · L81–102 · blob 28085ce2 · item e4f3fac425b44bf7]. Field by field
(the declaration is [model_tests.rs · struct Config · L36–74 · blob 28085ce2 · item a40ac57ee2eaa872]):

| Field | F-2 | F-R | Equal |
|---|---|---|---|
| name | "F-2" | "F-R" | **no — the configuration's label** (see below) |
| facts | 2 | 2 | yes |
| conflict | [false, true] | [false, true] | yes |
| env | false | false | yes |
| batch | false | false | yes |
| faults | 2 | 2 | yes |
| admissions | 2 | 2 | yes |
| gating | 2 | 2 | yes |
| gating_conflict | false | false | yes |
| acks | 0 | 0 | yes |
| ack_attempts | 0 | 0 | yes |
| malformed | false | false | yes |
| cell_constraint | false | false | yes |
| worker_unwinds | false | false | yes |
| drop | false | false | yes |
| **replays** | **0** | **3** | **no — the permitted difference** |
| blanket_uncertain | false | false | yes |
| l1 | true | true | yes |
| cap | 30,000,000 | 30,000,000 | yes |
| time | 45 min | 45 min | yes |

Both receipts print the same bounds line except `REPLAY 0` / `REPLAY 3`, and the actor lists
`[Fw(0), Fw(1), Se, Wk, Gc(0), Gc(1), Adm]` and `[Fw(0), Fw(1), Se, Wk, Gc(0), Gc(1), Adm, Replay]`.

**Disclosed deviation: `name`.** The mission states that `replays` is the only permitted difference. `name` also
differs. Occurrence closure (`REPLAY-OCCURRENCE-MATRIX.tsv`, rows of token `.name`) shows that inside the call closure
of `explore` the label is read exactly once, as the argument of the report header's `writeln!`
[model_tests.rs · explore · L4012 · blob 28085ce2 · item d7d79ebfe32ce732]; no state, transition, property, terminality or branch computation reads it
(Checker B check B86). The executor's position is that `name` is a report label, not a bound. Whether that reading is
acceptable is an Architect decision (section 11, D-1). If it is not, FR-1's configuration clause fails literally and
this argument is blocked at FR-1.

### 2.2 Worlds and the explored graph

A world is the record `World` [model_tests.rs · struct World · L1046–1064 · blob 28085ce2 · item d1f0b91ba0f5ebac]: `core`, `health`, `rows`, `fw`, `se`,
`wk` (`worker`, `pos`, `last`) [model_tests.rs · struct Wk · L999–1005 · blob 28085ce2 · item 446750c40973db5a], `gc`, `ack`, `adm`, `adm_kept`, `env_done`,
`replays`, `table`, `faults`, `ghost`.

For a configuration c, the explorer's graph G(c) is what `explore` [model_tests.rs · explore · L3881–4110 · blob 28085ce2 · item d7d79ebfe32ce732] searches:

- its nodes are keys; the initial key is `encode(c, initial(c))`;
- from a key k, `successors(c, actors(c), k)` [model_tests.rs · successors · L3803–3845 · blob 28085ce2 · item c9832f65d954e041] rebuilds a world by
  `decode(c, k)` for every actor index i of `actors(c)` and every choice prefix the Oracle enumerates, runs
  `step`, and yields an edge `(k, pack(i, choices), k')` with `k' = encode(c, w')`, a fault flag (`oracle.fault`),
  the branch labels the step pushed, and its violations (those pushed inside the step and those of
  `transition_properties`);
- the state's own violations are `state_properties` and the key round trip (`KEY`); a key with no successor is
  judged by `terminal` (D1); after a complete search L1 is Kahn's algorithm over the non-fault edges.

"Reachable" means reachable from the initial key in G(c). Equality of worlds, written `=f`, is equality of every
field's value; which allocation an `Arc` points to is not part of it (every `Arc` of a decoded world points to that
world's own new allocation, INV-T below).

### 2.3 The projection π

For a world w, **π(w)** is w with `wk.last := None` and `replays := 0`; every other field is unchanged: the
production core and its word, the simulated ledger rows, every fact writer's ticket and position, settlement, the
worker's possession and `WkPos`, the gating callers, the acknowledgement actors (none in either configuration),
admission, the test's table, the fault budget, the A2 bits, ENV, and every ghost history fact. π uses the F-2
configuration. The REPLAY actor does not exist in F-2; π erases REPLAY's transitions (section 4, FR-RS).

For keys, **πK** is the byte splice defined in PROJ-1 below: the segment L that encodes `wk.last` becomes the single
byte `0`, and the byte r that encodes `replays` becomes `0`; every other byte is kept in order.

---

## 3. Exact source facts

Each fact is read from C's blobs. Checker B checks the form of every fact marked [B#] (packet `receipts/`).

**SF-1 (actors).** `actors(c)` pushes `Fw(0..writers)`, `Se`, `Wk`, `Gc(0..gating)`, `Ack(0..acks)`, `Adm`, `Env`
when `env`, and `Replay` last when `config.replays > 0` [model_tests.rs · actors · L2332–2347 · blob 28085ce2 · item 5a96285fc20c2419]; `Replay` is the last
variant of `Actor` [model_tests.rs · enum Actor · L2320–2330 · blob 28085ce2 · item 55501f19b6808324]. [B10, B11]

**SF-2 (initial values).** `initial(c)` sets `wk.last = None` and `replays = config.replays`, and the worker from
`core.take_worker()` on a fresh core [model_tests.rs · initial · L1102–1161 · blob 28085ce2 · item b695213fd65cfd10], which returns `Some` the first time
[protocol.rs · Core::take_worker · L912–921 · blob a2df2841 · item 65e5afa73a3e74e5]. [B12]

**SF-3 (key layout).** `encode` writes, in order: the word; the core; the ledger rows beyond the initial ones; each
writer's ticket and position; settlement; the worker's presence; `wk.pos`; then **segment L**: byte 0 if `wk.last`
is `None`, otherwise byte 1 followed by the action [model_tests.rs · encode · L1657–1663 · blob 28085ce2 · item 0258487c6ec1f4b1]; then the gating positions,
the acknowledgement actors, `adm`, `adm_kept`, `env_done`; then **byte r** = `w.replays`
[model_tests.rs · encode · L1700 · blob 28085ce2 · item 0258487c6ec1f4b1]; then the table, the fault budget and the ghost facts
[model_tests.rs · encode · L1521–1721 · blob 28085ce2 · item 0258487c6ec1f4b1]. `decode` reads the same fields in the same order: L directly after `wk_pos`
[model_tests.rs · decode · L2093 · blob 28085ce2 · item 912dd837b0bd5afe], r directly after `env_done` [model_tests.rs · decode · L2126 · blob 28085ce2 · item 912dd837b0bd5afe], builds the world from
exactly these values [model_tests.rs · decode · L2144–2167 · blob 28085ce2 · item 912dd837b0bd5afe], and asserts that the whole key was read
[model_tests.rs · decode · L2143 · blob 28085ce2 · item 912dd837b0bd5afe]. [B13–B18]

**SF-4 (action and token codec).** An action encodes as a tag 0–5 (Nothing, Barrier, Resolve, AppendC, AppendX,
EmptyX), then for the four token actions the token (cell code, occupancy), then for EmptyX the sequence
[model_tests.rs · Enc::action · L1355–1377 · blob 28085ce2 · item e09166862e6c73b7] [model_tests.rs · Enc::token · L1304–1311 · blob 28085ce2 · item 7144c10498d2820c]; decoding mirrors it and makes
every token refer to the world's own allocation [model_tests.rs · Dec::action · L1900–1912 · blob 28085ce2 · item 9e07add290792fd3]
[model_tests.rs · Dec::token · L1853–1860 · blob 28085ce2 · item 0ef039840b47a217]. [B19–B21]

**SF-5 (REPLAY).** `step_replay` returns false unless `replays != 0`, `wk.pos` is `Idle`, `wk.last` is a token
action (Resolve, AppendC, AppendX, EmptyX), and the token is not present; otherwise it does exactly
`w.replays -= 1`, a trace note, and `worker_op(config, w, &last, o, obs)`, and returns true
[model_tests.rs · step_replay · L3334–3354 · blob 28085ce2 · item 070538e2755752f0]. It writes no other field, uses no Oracle choice itself and never writes
`wk.last`. `present_token` is "same allocation and the cell's present occupancy"
[model_tests.rs · present_token · L2828–2831 · blob 28085ce2 · item 4ddd2d21d5ad3bfa]. [B30, B31, B89]

**SF-6 (the only write of Wk.last after initialisation).** After a worker operation, `step_wk` sets
`w.wk.last = Some(action)` only when `config.replays > 0` and the action is neither Barrier nor Nothing, then returns
the worker to `Idle` [model_tests.rs · step_wk · L3002–3008 · blob 28085ce2 · item 8b55d861cc15ad82]. [B32, B88]

**SF-7 (the writes of World.replays).** `World.replays` is written only by `initial` (from `config.replays`), by
`decode` (from byte r) and by REPLAY's `w.replays -= 1`; `Config.replays` is set only by the constants `BASE` (0,
inherited by `F2`) and `FR` (3) (occurrence matrix; [B87]).

**SF-8 (actions carry their own cell).** The worker's actions are made only by `allowed(agenda)`: Resolve takes the
agenda's U token, AppendC its C token, AppendX (X Built) and EmptyX (X Committed) its X token
[model_tests.rs · allowed · L2802–2826 · blob 28085ce2 · item 4b7cc28e7b80af9c]; the agenda's tokens name their own cells at their occupancies
[protocol.rs · Core::agenda · L951–966 · blob a2df2841 · item 62b3497cd08f842c]. `step_wk` carries the chosen action from `Seen` to `Act` unchanged and saves
it in `wk.last` (SF-6) [model_tests.rs · step_wk · L2962–3016 · blob 28085ce2 · item 8b55d861cc15ad82]. [B33, B34]

**SF-9 (the stale worker operation).** `worker_op` computes `stale = !present_token(token)`, records the core key,
the allocation, the word and the rows before the operation, takes the worker by reference (returning only if there
is none), and builds `Io` without making a choice [model_tests.rs · worker_op · L2852–2868 · blob 28085ce2 · item 17807375d8f03a8e]. For a cell append it skips
E5's verb-order check when the token is stale [model_tests.rs · worker_op · L2895–2922 · blob 28085ce2 · item 17807375d8f03a8e]. Every operation that reaches
the core adds one store operation and exactly one branch label [model_tests.rs · worker_op · L2938–2940 · blob 28085ce2 · item 17807375d8f03a8e]; E4 fails if a
label ending `Stale` changed the rows [model_tests.rs · worker_op · L2941–2945 · blob 28085ce2 · item 17807375d8f03a8e]; E8 fails, for a stale token, if the
answer is not stale or the core key, the allocation or the word changed [model_tests.rs · worker_op · L2946–2959 · blob 28085ce2 · item 17807375d8f03a8e].
`Io::new` only stores its arguments [model_tests.rs · Io::new · L636–650 · blob 28085ce2 · item 167306822544d7e6]; only a verb's `pick` consumes a choice
[model_tests.rs · Io::pick · L652–666 · blob 28085ce2 · item 6237d22bb8ba7dcd]. [B40–B44]

**SF-10 (the three stale returns, before any verb or mutation).**
- `Core::resolve` creates the guard, calls `Publish::resolve`, sets `done = true` and returns
  [protocol.rs · Core::resolve · L1034–1044 · blob a2df2841 · item d5c9bedefa176345]; `Publish::resolve` returns `OtherLedger` if the worker is another
  ledger's, then **`Resolve::Stale` when the token does not name U** [protocol.rs · Publish::resolve · L1732–1739 · blob a2df2841 · item 1db7146778571fc0],
  before any read, latch or change.
- `Core::append_cell` [protocol.rs · Core::append_cell · L1046–1056 · blob a2df2841 · item 632f5e817e2ef327]; `Publish::cell` returns **`CellAppend::Stale`
  when the token names neither C nor X** [protocol.rs · Publish::cell · L1859–1866 · blob a2df2841 · item 1c1d5d24d48d7ffc], before `io.open()`.
- `Core::empty` [protocol.rs · Core::empty · L1058–1063 · blob a2df2841 · item bc4176e6eea5e9e4]; `Publish::empty` returns **`Emptying::Stale` when the token
  does not name X** [protocol.rs · Publish::empty · L1949–1955 · blob a2df2841 · item e1c077a54d6b5f43], before X is read or emptied; it has no `io` at all.
- `names` is "same allocation, same cell, present occupancy" [protocol.rs · Core::names · L1111–1116 · blob a2df2841 · item 809a4b1cfd5fefdb];
  `worker_belongs` compares the worker's allocation with the core's [protocol.rs · Publish::worker_belongs · L1708–1710 · blob a2df2841 · item fdcbf75bbf98e3cb].
  [B50–B58]

**SF-11 (the guard's publication on a stale return).** `Publish::new` reads `core.flags()` as its guess and holds no
ticket and nothing in U [protocol.rs · Publish::new · L1258–1271 · blob a2df2841 · item ea77a65365479f1f]. Its drop, with `done = true`, nothing held and no
ticket, changes no core field and makes one exchange publishing `(word & !FLAGS) | (word & HANDOVER_FAULT) | flags`
with `flags = core.flags()` [protocol.rs · <Publish as Drop>::drop · L1967–2008 · blob a2df2841 · item f6ee3c97579b9315]. `exchange` is one compare-and-exchange
loop that reports each failure and the one success to the test-only hook [protocol.rs · exchange · L196–220 · blob a2df2841 · item f8560c190ff49c93]
[protocol.rs · hook::report · L2048–2068 · blob a2df2841 · item b86933d12d9e32fb]. `core.flags()` is pure [protocol.rs · Core::flags · L1067–1086 · blob a2df2841 · item b9ed487fbc8f6adc] and equals the
test's `expected_flags(core)` bit for bit [model_tests.rs · expected_flags · L3375–3400 · blob 28085ce2 · item b0232c8c5ae0490d]; E1 requires
`word & FLAGS & !HANDOVER_FAULT == expected_flags(core)` [model_tests.rs · state_properties · L3402–3414 · blob 28085ce2 · item e1d53be10ebf79b3]. [B59–B64]

**SF-12 (properties, terminality, transitions).** `state_properties` reads `config.facts`, `config.gating`, the word,
the core, the tickets, the ghost, the table and the rows [model_tests.rs · state_properties · L3402–3511 · blob 28085ce2 · item e1d53be10ebf79b3];
`transition_properties` reads only the two cores' words, latches and refusal tables, the after-rows, the step's
observations and the hook counts [model_tests.rs · transition_properties · L3513–3545 · blob 28085ce2 · item b09267fd92f90ad2]; `admission_properties` reads
`config.batch`, the ghost, the core, the rows and the word [model_tests.rs · admission_properties · L3274–3321 · blob 28085ce2 · item 8f88fbd2fc017dbd];
`terminal` reads `config.env`, `config.facts` and the fields listed in FR-12 [model_tests.rs · terminal · L3547–3569 · blob 28085ce2 · item fb398b42fa3c841c].
None of them names `replays`, `Replay` or `wk.last`. [B80–B83]

**SF-13 (edges and observations).** `successors` takes the hook's counts immediately before and after each step,
breaks on a disabled step, stores `w.faults = oracle.faults`, applies `transition_properties`, and records
`pack(index, oracle.sequence())` and `oracle.fault` [model_tests.rs · successors · L3817–3842 · blob 28085ce2 · item c9832f65d954e041]; `pack` puts the actor
index in the low 4 bits and each choice in 3 bits [model_tests.rs · pack · L3573–3583 · blob 28085ce2 · item 11799d3e22cf052d]. `explore` judges D1 at a key
with no successor and builds L1's graph from the non-fault edges [model_tests.rs · explore · L3921–3950 · blob 28085ce2 · item d7d79ebfe32ce732]
[model_tests.rs · explore · L3957–4001 · blob 28085ce2 · item d7d79ebfe32ce732]. [B84, B85]

**SF-14 (the X cell's writers).** X is given a new occupancy only by `Core::place`
[protocol.rs · Core::place · L1137–1149 · blob a2df2841 · item 2600f966ada6d746], reached only through `Core::explanation`; every caller fills X only when it is
empty: `Core::classify` twice under `if self.x.is_none()` [protocol.rs · Core::classify · L1210–1225 · blob a2df2841 · item 89309480a147b292],
`Publish::explain` in the arm where X is `None` [protocol.rs · Publish::explain · L1569–1584 · blob a2df2841 · item 2b36a7ec60c8fdec], and
`Publish::resolve_gating`, reached only from `Publish::resolve` with `room` (C and X empty)
[protocol.rs · Publish::resolve · L1749–1754 · blob a2df2841 · item 1db7146778571fc0]. `Publish::settle` changes only X's stage
[protocol.rs · Publish::settle · L1932–1946 · blob a2df2841 · item c3c777f19ec8ed91]; `Publish::empty` sets X to `None` [protocol.rs · Publish::empty · L1956–1962 · blob a2df2841 · item e1c077a54d6b5f43];
in the model only `worker_op`'s EmptyX arm calls `Core::empty`. [B70–B77]

**SF-15 (the worker is never replaced; the explorer's thread state).** No step assigns `wk.worker` [B45]; a decoded
world's worker refers to that world's allocation [model_tests.rs · decode · L2081–2083 · blob 28085ce2 · item 912dd837b0bd5afe]. The hook's counts are
thread-local and reset by `take_counts` [protocol.rs · hook::take_counts · L2070–2073 · blob a2df2841 · item e86e0ebba44a4834]; no code reachable from
`explore` installs a hook callback (occurrence and closure analyses; no `hook::install` in `model_tests.rs`).
`protocol.rs` imports only `std` [B79].

---

## 4. Lemmas

### PROJ-1 — the key splice is well defined

For every key k in the image of `encode(FR, ·)`, write k = P · L · M · r · S where P is everything up to and including
`wk.pos`, L the `wk.last` segment, M the gating positions through `env_done`, r the one replay byte and S the rest
(SF-3). Then πK(k) := P · [0] · M · [0] · S.

*Argument.* `decode` is a sequential parser: each field's decoder reads forward from the previous field's end and
decides where it stops from the bytes it has read (SF-3, SF-4). Its decoders mirror `encode`'s emitters primitive by
primitive (table below), so on any encoder output each field's decoder consumes exactly the bytes that field's
emitter wrote (self-delimitation, **SD**). Hence the parse positions of P, L, M, r and S in k are the encoder's own
boundaries, and the splice replaces exactly the bytes of L and r.

| Primitive / field | Encoder | Decoder | Width on encoder output |
|---|---|---|---|
| unsigned integer | `Enc::u`: 7-bit groups, high bit = more | `Dec::u`: reads until a byte without the high bit | equal |
| byte, bool | one byte; `b` writes 0 or 1 | one byte; `b` reads `!= 0` | 1 |
| 32-byte value | first 3 bytes (rest asserted zero) | 3 bytes | 3 |
| nonce | first 2 bytes (rest asserted zero) | 2 bytes | 2 |
| tagged choice (cell presence, latch, ticket, positions, occupants, causes, actions, …) | tag, then that variant's fields | reads the tag, then the same variant's fields | equal by induction |
| counted loops (rows, refusals) | length, then each element | length, then that many elements | equal |
| configuration loops (writers, gating callers, acknowledgement actors) | over `w.fw`, `w.gc`, `w.ack` | `writers`, `config.gating`, `config.acks` times | equal, because `initial` and `decode` make exactly that many and no step adds or removes one |

### PROJ-2 — πK commutes with encoding

For every world w: `encode(F2, π(w)) = πK(encode(FR, w))`.

*Argument.* `encode` reads the configuration only through `initial_rows(config)` (facts, conflict, env,
gating_conflict: equal in FR and F2) and the loop counts of PROJ-1 (equal) [model_tests.rs · encode · L1521–1721 · blob 28085ce2 · item 0258487c6ec1f4b1]
[model_tests.rs · initial_rows · L1085–1100 · blob 28085ce2 · item 68a45e5469ea3ec6]. The only bytes that depend on `wk.last` or `replays` are L and r (SF-3).
π(w) has `wk.last = None` (L = [0]) and `replays = 0` (r = [0]) and every other field of w. By SD, the remaining
bytes are emitted identically and at the same relative order. So the F-2 encoding of π(w) is the F-R encoding of w
with L and r replaced by single zero bytes.

### PROJ-3 — decoding commutes with π

For every key k in the image of `encode(FR, ·)`: `decode(F2, πK(k)) =f π(decode(FR, k))`.

*Argument.* `decode` reads the configuration through `initial_rows(config)`, `config.batch`, `config.facts`,
`config.gating` and `config.acks` [model_tests.rs · decode · L1974–2168 · blob 28085ce2 · item 912dd837b0bd5afe]: equal in FR and F2. On k and on πK(k) the
decoders of P, M and S see identical bytes and produce identical values; the L decoder reads byte 0 of πK(k) and
gives `None`; the r decoder reads 0. These are π's values.

### INV-W, INV-T, INV-AC — three invariants of every reachable key (F-R or F-2)

- **INV-W.** The worker-presence byte is 1, and the decoded worker refers to the world's own allocation. The initial
  world has the worker (SF-2); no step assigns `wk.worker` (SF-15); `encode` writes 1 for a present worker. Hence
  `worker_belongs` holds in every decoded reachable world, and `worker_op` never returns early for want of a worker.
- **INV-T.** Every token, ticket, snapshot and the worker in a decoded world refer to that world's allocation
  (SF-4, SF-15; tokens made during a step come from `Core::token`, which clones the core's allocation
  [protocol.rs · Core::token · L1095–1101 · blob a2df2841 · item 63ae37127987ad9a]). Hence
  `present_token(t)` holds iff the cell `t.cell` is at occupancy `t.occupancy`.
- **INV-AC.** In every reachable F-R world, every action held in `wk.pos` (`Seen`, `Act`) or in `wk.last` has a token
  of its own variant's cell: Resolve → U, AppendC → C, AppendX and EmptyX → X. Actions are made only by `allowed`
  (SF-8); `Seen` → `Act` → `wk.last` copy them (SF-6, SF-8); the codec keeps the cell (SF-4); REPLAY writes neither
  `wk.pos` nor `wk.last` (SF-5); no other actor touches `wk`.

### FR-1 — initial states

**Claim.** π(initial(FR)) =f initial(F2), and πK(encode(FR, initial(FR))) = encode(F2, initial(F2)).

**Argument.** `initial` [model_tests.rs · initial · L1102–1161 · blob 28085ce2 · item b695213fd65cfd10] reads `batch`, `facts`, `initial_rows(config)`,
`gating`, `acks`, `ack_attempts`, `admissions`, `replays` and `faults`. All are equal in FR and F2 except
`replays` (section 2.1); `name` is not read by `initial`. So `initial(FR)` and `initial(F2)` differ only in
`replays` (3 and 0); both have `wk.last = None`. π sets `replays := 0`. The key statement follows by PROJ-2.

**Status.** ESTABLISHED (executor argument), with the disclosed `name` deviation of section 2.1 (decision D-1).

### FR-2, FR-3, FR-4, FR-5 — an ordinary actor simulates exactly

Let k be a reachable F-R key, t = πK(k), and a an actor of `actors(F2)` (so a ∈ {Fw(0), Fw(1), Se, Wk, Gc(0), Gc(1),
Adm}, at the same index i in both lists, FR-6). Run `successors`' loop body for a, at index i, with the same
choice prefix p, once on (FR, k) and once on (F2, t).

**Lockstep argument.** By PROJ-3 the two decoded worlds W_R and W_2 satisfy W_2 =f π(W_R): they agree on every
field except `wk.last` and `replays`. `step` dispatches a to `step_fw`, `step_se`, `step_wk`, `step_gc` or `step_adm`
[model_tests.rs · step · L3356–3371 · blob 28085ce2 · item 9224d2aecd0815c5]. These, and every function they reach, read the configuration fields `facts`,
`batch`, `drop`, `blanket_uncertain`, `worker_unwinds`, `cell_constraint`, `malformed`, `ack_attempts` (equal), and
`replays` only at SF-6's guard; they read no `wk.last` and no `World.replays` (SF-12, occurrence closure, checks
B80 and A-closure). Every value they read is therefore equal in both runs, including the word (equal value in two
allocations), allocation comparisons (each made between objects of the same world, true in both, INV-T), the
Oracle's prefix p, and the hook's thread-local counts (reset before the step, SF-13, SF-15). The program is
deterministic in these inputs: it uses ordered maps, no clock and no global state in the step. So both runs execute
the same statements with the same values, step for step. The only statement whose effect differs is SF-6's
`w.wk.last = Some(action)`, which runs in F-R (config.replays = 3) and not in F-2 (0); it is the last change to `wk`
before the step returns, and nothing in the step reads `wk.last` afterwards.

From the lockstep:

- **FR-2 (enabledness).** `step` returns the same boolean in both runs, for every prefix; a is enabled at k exactly
  when it is enabled at t.
- **FR-3 (choices).** Both Oracles record the same `(choice, options)` list, so the option structure, the consumed
  choices, `sequence()` and `next()` are equal, and the two runs enumerate the same prefixes in the same order
  [model_tests.rs · Oracle::choose · L544–561 · blob 28085ce2 · item f5618eba9948e034] [model_tests.rs · Oracle::next · L571–580 · blob 28085ce2 · item 79eef47e7d71d1e2].
- **FR-4 (effect).** The two successor worlds W_R' and W_2' agree on every field except `wk.last` (possibly saved in
  F-R) and `replays` (untouched by an ordinary step: SF-7), so W_2' =f π(W_R'); by PROJ-2,
  `encode(F2, W_2') = πK(encode(FR, W_R'))`. The saved `wk.last` disappears under π, and no ordinary actor's later
  enabledness or effect depends on it, because no ordinary step reads it.
- **FR-5 (observations).** Equal in both runs: the fault flag `oracle.fault`; the remaining fault budget
  `oracle.faults`; the hook counts; the branch labels; the violations pushed in the step and those of
  `transition_properties` (whose inputs are equal, SF-12); the trace text (the prefix `{actor:?}: ` and the same
  notes); and the effect on the production state (core, word, rows). No replay-history-only text arises on an
  ordinary step; REPLAY's own trace note ("the worker repeats its last operation with an old token") occurs only on
  a REPLAY step.

**Status.** FR-2, FR-3, FR-4, FR-5: ESTABLISHED (executor argument).

### FR-6 — edge identity

**Claim.** Every ordinary actor has the same index in both actor lists and every ordinary edge has the same packed
identity; REPLAY's edges are distinct from every ordinary edge.

**Argument.** SF-1 with `env = false`, `acks = 0`, `batch = false`, `facts = 2`, `gating = 2`:
`actors(F2) = [Fw(0), Fw(1), Se, Wk, Gc(0), Gc(1), Adm]` (indices 0–6) and `actors(FR)` is the same list followed by
`Replay` (index 7), as both receipts print. `successors` packs `pack(index, choices)` (SF-13); by FR-3 the choices
are equal, so an ordinary edge packs identically. A REPLAY step consumes no choice (FR-RS), so its edge is
`pack(7, []) = 7`, whose low 4 bits differ from every ordinary edge's (0–6). Each actor was inspected:

| Index | Actor | Step function | Reads a replay value? |
|---|---|---|---|
| 0, 1 | Fw(0), Fw(1) | `step_fw` [model_tests.rs · step_fw · L2509–2638 · blob 28085ce2 · item 8f275a9e87e2dcd4] | no |
| 2 | Se | `step_se` [model_tests.rs · step_se · L2701–2800 · blob 28085ce2 · item 7cf06d9d435640e0] | no |
| 3 | Wk | `step_wk` [model_tests.rs · step_wk · L2962–3016 · blob 28085ce2 · item 8b55d861cc15ad82] | writes `wk.last` (SF-6) when `config.replays > 0`; reads none |
| 4, 5 | Gc(0), Gc(1) | `step_gc` [model_tests.rs · step_gc · L3018–3105 · blob 28085ce2 · item c07807aa74518ca0] | no |
| 6 | Adm | `step_adm` [model_tests.rs · step_adm · L3249–3272 · blob 28085ce2 · item a2e589fb0b9fc75e] | no |
| 7 (F-R only) | Replay | `step_replay` [model_tests.rs · step_replay · L3334–3354 · blob 28085ce2 · item 070538e2755752f0] | reads `replays` and `wk.last`; writes `replays` |

**Status.** ESTABLISHED (executor argument).

### FR-RS — a REPLAY step is a stutter

**Claim.** Let k be a reachable F-R key at which REPLAY is enabled, and suppose E1 and KEY hold at k. Then REPLAY has
exactly one successor k'; the edge is not a fault; it consumes no Oracle choice; k' differs from k only in byte r,
which decreases by exactly one; πK(k') = πK(k); its only branch label is one of `resolve: Stale`,
`append_cell C: Stale`, `append_cell X: Stale`, `empty: Stale`; and its violation list is empty.

**Preconditions** (SF-5): `replays != 0`; `wk.pos = Idle`; `wk.last = Some(a)` with a ∈ {Resolve, AppendC, AppendX,
EmptyX}; a's token t is not present.

**Argument, step by step** on the decoded world W.

1. `w.replays -= 1` (no underflow: `replays != 0`). Nothing else of W is written by `step_replay` (SF-5).
2. `worker_op(FR, W, a)` (SF-9): `stale` is true (t not present); it records `before_core`, the allocation,
   `before_word`, `before_rows`; the worker is present and belongs (INV-W); `Io::new` makes no choice.
3. The core operation, by a's variant (SF-10):
   - **Resolve:** `Core::resolve` → `Publish::new` (reads `core.flags()` only) → `Publish::resolve`:
     `worker_belongs` is true; `core.names(t, U)` is false — for a stale t, `names(t, c)` is false for **every**
     cell c, since `names(t, c)` would give `t.cell = c` and occupancy(c) = t.occupancy, i.e. t present (INV-T) —
     so it returns `Resolve::Stale` before reading storage, latching, or touching U, C or X.
   - **AppendC, AppendX:** `Core::append_cell` → `Publish::cell`: `worker_belongs`; `names(t, C) || names(t, X)` is
     false → `CellAppend::Stale`, before `io.open()`.
   - **EmptyX:** `Core::empty` → `Publish::empty`: `worker_belongs`; `names(t, X)` is false → `Emptying::Stale`,
     before X is read or emptied (no `io` exists here).
   In every case no `io` verb is called, so no Oracle choice is consumed and no verb can unwind (only a verb's
   `pick` can raise `Unwound`, SF-9); `run_op` returns `Some(Stale)`.
4. The guard drops with `done = true`, nothing held, no ticket (SF-11): no core field changes, and one exchange
   publishes `(word & !FLAGS) | (word & HANDOVER_FAULT) | core.flags()`. **Because E1 holds at k**, the word's bits
   of FLAGS other than the handover fault equal `expected_flags(core) = core.flags()` (SF-11), so this value equals
   the word: the word is unchanged. The hook records one successful exchange (and at most one failed one, when the
   guess differed; one thread, so the second attempt succeeds) and no load.
5. Back in `worker_op`: the label is `resolve: Stale`, `append_cell {t.cell}: Stale` or `empty: Stale` (format at
   [model_tests.rs · worker_op · L2882–2934 · blob 28085ce2 · item 17807375d8f03a8e]); for AppendC t.cell = C and for AppendX t.cell = X (INV-AC). One store
   operation and that one label are recorded (SF-9). E4: the label ends `Stale` and the rows are unchanged — no
   violation. E8: the answer is stale, the core key, the allocation and the word are unchanged — no violation.
   E5 is not checked for a stale cell append, and no verb was issued.
6. In `successors` (SF-13): `w.faults = oracle.faults` is W's budget, unchanged; `oracle.fault` is false;
   `transition_properties`: E2 — loads 0 = views 0, and 0 ≤ exchanges = 1 ≤ ticket steps 0 + store operations 1;
   E9 — latch and handover fault unchanged; E11 — the refusal table is unchanged, so no entry changed or left. The
   edge is `pack(7, [])`. `oracle.next()` is `None` (no choice was taken), so this is REPLAY's only successor.
7. `encode(FR, W')` re-encodes k's decoded values with `replays` one lower: the core, the word, the rows, every
   actor, the table, the budget, the ghost facts and `wk.last` are unchanged (`wk.last` is read, cloned, never
   written). **Because KEY holds at k**, re-encoding k's decoded values gives k's own bytes, so k' equals k except
   byte r, which is one lower; πK(k') = πK(k), since πK zeroes r.

No stale path can invoke storage, consume a choice, change the word, core, ledger, table or a property field,
produce a fault, unwind, or answer other than `Stale`; that is what FR_STUTTER_PROOF_BLOCKED would have required.

**Status.** ESTABLISHED (executor argument), conditional on E1 and KEY at k, which INV-E1 supplies for every
reachable k.

### FR-7 — projection of paths (reachability)

**Claim.** For every path k₀ → … → kₙ of G(FR) from the initial key, erasing its REPLAY edges leaves a path of G(F2)
from the initial key through πK(k₀), …, πK(kₙ) (consecutive equal keys merged); in particular πK(k) is reachable in
G(F2) for every reachable k of G(FR).

**Argument.** Induction on n. n = 0: FR-1. Step: πK(kₙ) is reachable in G(F2) by the induction hypothesis.
- If the edge kₙ → kₙ₊₁ is an ordinary edge of actor a with choices p: by FR-2 and FR-3 the same actor and prefix are
  enabled at πK(kₙ) in G(F2), and by FR-4 its successor is πK(kₙ₊₁), with the same packed edge (FR-6).
- If it is a REPLAY edge: E1 and KEY hold at πK(kₙ) (a reachable F-2 key; the F-2 receipt's `E1: HOLDS` and
  `KEY: HOLDS`), hence at kₙ (FR-9: E1 reads only the word and the core, which π keeps, and KEY at kₙ is equivalent to
  KEY at πK(kₙ)); so FR-RS applies and πK(kₙ₊₁) = πK(kₙ).

**Status.** ESTABLISHED (executor argument).

### INV-E1 — E1 and KEY at every reachable F-R key

By FR-7, πK(k) is reachable in G(F2), where E1 and KEY HOLD (F-2 receipt); E1 and KEY at k agree with E1 and KEY at
πK(k) (FR-9). These are the conditions FR-RS needs; the induction of FR-7 supplies them at each step before they are
used.

### FR-8 — lifting of paths

**Claim.** For every path t₀ → … → tₙ of G(F2) from its initial key there is a path k₀ → … → kₙ of G(FR) from its
initial key with πK(kᵢ) = tᵢ, using the same edges and scheduling no REPLAY step.

**Argument.** Induction on n. n = 0: k₀ = the F-R initial key, πK(k₀) = t₀ (FR-1). Step: kₙ is reachable in G(FR)
and πK(kₙ) = tₙ. The F-2 edge tₙ → tₙ₊₁ is some actor a (index i < 7) with prefix p; by FR-2 and FR-3 a is enabled
at kₙ with p, and by FR-4 its successor kₙ₊₁ satisfies πK(kₙ₊₁) = tₙ₊₁. The lift accumulates `wk.last` (SF-6) and
keeps `replays = 3`; neither changes any ordinary step (FR-4).

**Status.** ESTABLISHED (executor argument).

### FR-9 — state properties agree under π

**Claim.** For every reachable F-R key k, the state violations of k in G(FR) equal those of πK(k) in G(F2): the
`state_properties` list and the `KEY` check.

**Argument.** `state_properties` reads the configuration fields `facts` and `gating` (equal) and world fields that π
keeps (SF-12); its violation list is a function of those, so it is the same list on W and π(W) (PROJ-3).
`KEY` compares `encode(c, decode(c, k))` with k. Write F_X(X) for the re-encoding of the values decoded from segment
X; by PROJ-2 and PROJ-3 the F-2 re-encoding of πK(k) is F_P(P) · [0] · F_M(M) · [0] · F_S(S), and the F-R re-encoding
of k is F_P(P) · F_L(L) · F_M(M) · F_r(r) · F_S(S). KEY holds at πK(k) (receipt: `KEY: HOLDS`), so
F_P(P) · [0] · F_M(M) · [0] · F_S(S) = P · [0] · M · [0] · S. Each F_X(X) is an encoder output, so by SD the parse of
the left string stops its fields at |F_P(P)|, then 1, then |F_M(M)|, then 1; the parse of the right string, the same
string, stops them at |P|, 1, |M|, 1. Hence F_P(P) = P, F_M(M) = M and F_S(S) = S. For L and r: k is the initial key or
a successor key, both produced by `encode`, so L and r are encoder outputs (tags 0–5, cell codes 0–2, canonical
integers), and decoding then re-encoding returns them unchanged (SF-4): F_L(L) = L, F_r(r) = r. So KEY holds at k
exactly when it holds at πK(k). D1 is treated in FR-12.

**Status.** ESTABLISHED (executor argument).

### FR-10 — REPLAY transitions satisfy every applicable transition property

**Claim.** No REPLAY transition from a reachable F-R key violates E2, E4, E8, E9, E11 or any property pushed inside
the step, and its successor violates no state property and not KEY.

**Argument.** FR-RS steps 5 and 6 give E4, E8 (inside `worker_op`) and E2, E9, E11 (`transition_properties`); E5 is
not checked and no verb is issued; `step_replay` pushes no violation itself. The successor k' has πK(k') = πK(k),
reachable in G(F2), so by FR-9 its state properties and KEY agree with those of a reachable F-2 key, where all HOLD.

**Supporting executable evidence (not the proof).** `e8_holds_for_stale_worker_operations`
[model_tests.rs · e8_holds_for_stale_worker_operations · L4778–4814 · blob 28085ce2 · item f81728a047604aa9] ran at the exact final baseline inside the passing
Stage-1 suite (`... ok`). It uses configuration FR, a core forced through private state by `force_sample`
[model_tests.rs · force_sample · L4610–4634 · blob 28085ce2 · item 27d5943511394b26] (U at 3, C at 2, X at 4 Committed(5)), and calls `worker_op` with a stale
Resolve (U, 2), AppendC (C, 1), AppendX (X, 3) and EmptyX (X, 3, 5) in turn; it requires no violation and that every
label emitted ends in `Stale`. Two limits: the state is forced, not reachable; and the label assertion is vacuous on an
empty label list — that a label is emitted follows from the source (the worker is present in `initial(FR)`; SF-9),
not from the assertion. It checks neither the hook counts (E2) nor choice consumption.

**Status.** ESTABLISHED (executor argument).

### FR-11 — safety transfer

**Claim.** No reachable F-R state or transition violates an applicable safety property: P1, P2, P4, P5, P6, P7, P8,
P9, P12, P13, P15, E1–E9, E11 and KEY (E10 is not applicable: `worker_unwinds` is false in both).

**Argument.** Suppose one did.
- **A.** A state property at reachable k: by FR-9 it fails at πK(k), reachable in G(F2) by FR-7 — but F-2's complete
  search expanded every reachable key and found none.
- **A.** A property of an ordinary transition from reachable k (pushed in the step, such as E4, E5, E7, P1, P12, P15,
  E6, E9 at admission, or of `transition_properties`): by FR-5 the same violation occurs on the corresponding edge of
  G(F2) from πK(k) — but F-2 judged every edge of every expanded key and found none.
- **B.** A property of a REPLAY transition: contradicts FR-10.

So none exists. This rests on the accepted exact-final F-2 receipt: 12,602,670 states, PASS, all applicable safety
properties HOLD, D1 HOLDS, L1 HOLDS.

**Status.** ESTABLISHED (executor argument). F-R's original graph was not explored and no run of it reached a
verdict; the statement is derived through FR-7, FR-9, FR-5 and FR-10 from the F-2 receipt.

### FR-12 — D1 transfer

**Claim.** If a reachable F-R key k has no enabled transition, then no REPLAY step is enabled at k, πK(k) has no
enabled F-2 transition, and terminal(FR, k) = terminal(F2, πK(k)); hence k is terminal.

**Argument.** "No enabled transition" includes REPLAY. By FR-2 every ordinary actor is disabled at πK(k) as at k,
and `actors(F2)` has no other actor, so πK(k) has no successor; it is reachable (FR-7), so F-2's search judged it and
`D1: HOLDS` gives terminal(F2, πK(k)). `terminal` reads `config.env`, `config.facts` (equal) and, of the world:
every writer's position and ticket, settlement's position and ticket, `wk.pos`, the gating positions, the
acknowledgement actors, `adm`, `env_done`, the core's latch, word (handover fault), U, C, X and phase, and the table's
classes [model_tests.rs · terminal · L3547–3569 · blob 28085ce2 · item fb398b42fa3c841c] — never `replays` or `wk.last` [B81]. π keeps all of them, so the two
values are equal. The cases the mission names:

| State at k | REPLAY enabled? | Consequence |
|---|---|---|
| `replays > 0`, `wk.last` absent | no (SF-5) | D1 decided by the ordinary actors; transfers as above |
| `wk.last` present, its token still present | no | the same |
| `wk.last` present with a stale token, `replays > 0`, `wk.pos = Idle` | **yes** | k has a successor: not a no-step key |
| `wk.last` stale, `replays > 0`, `wk.pos ≠ Idle` | no | `step_wk` is enabled from `Seen` and `Act` [model_tests.rs · step_wk · L2987–3008 · blob 28085ce2 · item 8b55d861cc15ad82]: not a no-step key |
| `replays = 0` | no | as the first row |

A terminal F-R key may keep unspent replays: `terminal` does not read them, in F-R as in the contract's D1
(contract 7.3).

**Status.** ESTABLISHED (executor argument).

### FR-13 — no cycle contains a REPLAY transition

**Claim.** Every REPLAY transition strictly decreases `replays`, and no transition increases it; hence no directed
cycle of G(FR) contains a REPLAY transition.

**Argument.** The writes of `replays` are the configuration constants, `initial`, `decode` and REPLAY's `-= 1`
(SF-7); `decode` restores exactly the encoded byte r and `encode` writes it back (SF-3), so an ordinary transition
keeps r and a REPLAY transition lowers it by exactly one (FR-RS). Along a cycle the value returns to its start, so
its total decrease is zero, and no REPLAY edge lies on it.

**Status.** ESTABLISHED (executor argument).

### FR-14 — a non-fault F-R cycle projects to a non-fault F-2 cycle

**Claim.** If G(FR) had a cycle of non-fault transitions among reachable keys, G(F2) would have one among reachable
keys.

**Argument.** By FR-13 the cycle's edges are ordinary. By FR-4 and FR-5 each edge kᵢ → kᵢ₊₁ has the corresponding
edge πK(kᵢ) → πK(kᵢ₊₁) of G(F2), with the same fault flag (non-fault), between reachable keys (FR-7). The images form a
closed walk of length at least one in G(F2)'s non-fault graph, and every closed walk of positive length contains a
directed cycle (possibly a self-loop).

**Status.** ESTABLISHED (executor argument).

### FR-15 — L1 transfer

**Claim.** G(FR)'s reachable non-fault graph has no cycle, and each of its end states is terminal.

**Argument.** F-2's complete search ran Kahn's algorithm over all its reachable non-fault edges and removed every key
(`L1: HOLDS: 50655853 non-fault edges, no cycle`); a self-loop or any cycle would have left a key with positive
in-degree (SF-13). So G(F2) has no such cycle, and by FR-14 neither has G(FR). End states (keys with no successor) are
terminal by FR-12. Cycle preservation is argued edge by edge, never from a state-count relation.

**Status.** ESTABLISHED (executor argument).

### FR-16 — the ordinary branch set equals F-2's

**Claim.** The set of branch labels that ordinary transitions take from reachable F-R keys equals F-2's taken set
(54 labels in the complete F-2 receipt).

**Argument.** ⊆: an ordinary transition from reachable k has the labels of the corresponding transition from πK(k)
(FR-5), reachable by FR-7; F-2's complete search added every successor's labels to its taken set [model_tests.rs · explore · L3932–3934 · blob 28085ce2 · item d7d79ebfe32ce732].
⊇: every reachable F-2 key t has a reachable F-R lift k with πK(k) = t (FR-8), and each transition from t has a
corresponding ordinary transition from k with the same labels (FR-5).

**Status.** ESTABLISHED (executor argument).

### FR-17 — REPLAY adds only stale labels

**Claim.** Every REPLAY transition takes exactly one label, one of `resolve: Stale`, `append_cell C: Stale`,
`append_cell X: Stale`, `empty: Stale` (all four are rows of the frozen 193-row vocabulary: B132, B146, B160, B186).
Of these, `resolve: Stale`, `append_cell C: Stale` and `empty: Stale` are taken in G(FR), and `append_cell X: Stale`
is not.

**Argument.** FR-RS step 5 gives the label set and its exactness (with INV-AC: AppendC's token is C's, AppendX's is
X's, so `append_cell U: Stale`, B174, never arises). For reachability:
- `resolve: Stale`, `append_cell C: Stale` and `empty: Stale` are printed taken in the F-R receipt's prefix. Each was
  taken on a transition from an expanded key, and expanded keys are reachable, so each is taken in G(FR). F-2 does not
  take them, so by FR-16 each was taken by a REPLAY transition, and FR-RS shows that transition is a stutter that
  violates nothing. The prefix is INCOMPLETE and is not coverage; only these existence observations are used.
- **FR-17.X** — `append_cell X: Stale` is never taken. *Claim:* in every reachable F-R world with
  `wk.last = Some(AppendX(t))`, X is occupied at `t.occupancy`; hence REPLAY never runs an AppendX. *Argument:* X
  gets a new occupancy number only by `Core::place` when X is empty, and loses it only by `Publish::empty`
  (SF-14); only the worker calls `Core::empty`, through an EmptyX action. (i) When the worker chooses AppendX(t) in
  `Idle`, X is occupied (Built) at `t.occupancy` (SF-8). While `wk.pos` is `Seen` or `Act`, REPLAY is disabled
  (SF-5) and no other actor can empty an occupied X or refill it, so X keeps that occupancy until `Act` runs the
  append, which keeps X's occupancy (`Publish::settle` changes only the stage; the refusals change nothing in X), and
  saves `wk.last = AppendX(t)` (SF-6). (ii) From then on, while `wk.last` is AppendX(t): REPLAY is disabled (t is
  present); the other actors cannot change an occupied X's occupancy; the worker's Barrier and Nothing leave
  `wk.last` and X alone; Resolve requires X empty to place into X, and AppendC touches only C; and an EmptyX that
  empties X replaces `wk.last` by that EmptyX in the same step. So X stays at `t.occupancy` as long as `wk.last` is
  AppendX(t). The F-R prefix agrees (`append_cell X: Stale` NOT TAKEN there), but that observation is not used.

Per row, the complete disposition is `FR-BRANCH-DISPOSITION.tsv`: of 193 rows, 57 TAKEN, 106 NOT TAKEN, 30 NOT
APPLICABLE, each with its provenance; none NOT_ESTABLISHED. The derived TAKEN set equals the prefix's printed TAKEN
set (57); that equality corroborates and measures nothing about the complete graph.

**Status.** ESTABLISHED (executor argument).

---

## 5. Dependency graph

```text
SF-1..SF-15 (exact source facts, C's blobs)        F-2 receipt (exact final, PASS)
        |                                                   |
  PROJ-1 (splice, SD) --> PROJ-2 --> PROJ-3                 |
        |                   |          |                    |
  INV-W, INV-T, INV-AC      |          |                    |
        |                   v          v                    |
        |      FR-1    FR-2/3/4/5 (lockstep)  FR-6          |
        |        \         |        \                       |
        +-------> FR-RS <-(E1, KEY at k)- INV-E1 <-- FR-9 <-+
                   |   \                  ^                 |
                   v    \                 |                 |
                 FR-7 ---+--------------> (induction) ------+
                   |
     +-------------+----------------+------------------+
     v             v                v                  v
   FR-8          FR-10            FR-12              FR-13
     |             |                |                  |
     v             v                v                  v
   FR-16 <------ FR-11 (safety)   D1 transfer        FR-14 --> FR-15 (L1)
     |
   FR-17 (+ FR-17.X: SF-14, SF-5, SF-6, SF-8)
```

FR-7 and INV-E1 are proved together by one induction: at each REPLAY step, E1 and KEY at the current key come from
the already-established reachability of its projection.

## 6. Safety

FR-11. Every applicable safety property of 7.3 and 7.4 holds on F-R's reachable graph, as a consequence of the F-2
receipt through FR-7, FR-9, FR-5 and FR-10.

## 7. D1

FR-12. Every reachable F-R key with no enabled step is terminal.

## 8. L1

FR-13, FR-14, FR-15. F-R's reachable non-fault graph is acyclic, and its end states are terminal. F-R has no DROP,
so the contract claims L1 for it (7.4).

## 9. Branch accounting

FR-16, FR-17, FR-17.X; per-row table `FR-BRANCH-DISPOSITION.tsv` (built by `tools/branch_disposition.py` from the
vocabulary, the complete F-2 receipt, the incomplete F-R receipt and the diagnostic).

| Provenance | Meaning here |
|---|---|
| MEASURED_IN_COMPLETE_F2 | the complete F-2 receipt's own branch list |
| MEASURED_IN_FR_PREFIX_ONLY | printed taken (or not taken) by the incomplete F-R run; a taken label is an existence observation, never coverage |
| MEASURED_IN_EXACT_FINAL_DIAGNOSTIC | `e8_holds_for_stale_worker_operations`, exact-final suite, a forced state |
| DERIVED_BY_FR_PROOF | FR-16, FR-17, FR-17.X |
| NOT_ESTABLISHED | none |

## 10. Scope and limitations

1. **The explored model only.** Like the F-2 receipt it rests on, every statement here is about the explorer's graph
   of the model at C, conditional on A-1, A-2 (AD-1-R4 J). Nothing is said about SQLite, a real execution, or
   contract 6.2/6.3.
2. **Source to executable.** The argument is about C's source text. The F-2 receipt was produced by the pinned
   executable whose rustc dep-info inputs are exactly C's 49 blobs. That the compiled program implements the source
   semantics is assumed, as it is for every receipt.
3. **No F-R cardinality.** No total state count, transition count, completion ceiling or bound is derived or implied.
   The F-R receipt remains INCOMPLETE: 30,000,001 discovered, 26,539,923 expanded.
4. **Mechanical checks are syntactic.** Both checkers verify occurrence closure, source shapes, citations and their
   agreement. They support the semantic argument; they do not replace review of it.
5. **SD by source mirror.** Self-delimitation (PROJ-1) is argued from the encoder/decoder mirror; Checker B checks the
   replay segments' positions and the action and token codecs mechanically, not the whole codec.
6. **Determinism.** FR-2–FR-5 assume the step code is deterministic in the decoded world, the configuration, the
   Oracle prefix and the reset hook counts; SF-15 records what was checked (no hook callback, no clock, ordered maps,
   `std`-only imports). A hidden source of nondeterminism would void the lockstep.
7. **The diagnostic** is supporting evidence only (FR-10's two limits).
8. **`name`** differs between the configurations (section 2.1, decision D-1).
9. **Executor argument.** Written and checked by the executor; not accepted.

## 11. What would invalidate it

Any of the following, found by review or by a changed input, defeats the stated lemma:

| # | Condition | Defeats |
|---|---|---|
| 1 | any of C's 49 Rust inputs differs from the cited blob | every source fact |
| 2 | a read of `wk.last` or `World.replays` by an ordinary actor, a property, `terminal`, or the branch labelling | FR-2–FR-5, FR-9, FR-12, FR-16 |
| 3 | an ordinary transition that writes `replays`, or a REPLAY step that does not decrement it by exactly one | FR-13, FR-RS |
| 4 | a REPLAY step that changes the core, the word, the rows, the table, the fault budget, the ghost, `wk.pos` or `wk.last`, consumes a choice, or creates a fault edge | FR-RS, FR-7, FR-10 |
| 5 | a stale path that reaches a storage verb, latches, mutates, unwinds or answers other than `Stale`, or a guard publication that is not the same word (for example a reachable state where E1 fails) | FR-RS |
| 6 | REPLAY placed before an existing actor, or a choice in REPLAY's step | FR-6 |
| 7 | the L segment or the r byte not where SF-3 puts them, or a decoder that is not self-delimiting | PROJ-1–3, FR-9 |
| 8 | nondeterminism or shared state in a step (time, hash order, an installed hook callback, a global) | FR-2–FR-5 |
| 9 | the F-2 receipt not being a complete, admissible PASS at C | FR-7, FR-11, FR-12, FR-15, FR-16 |
| 10 | a writer of X other than `place` (into an empty X), `settle` (stage) and `empty`, or a caller of `Core::empty` other than the EmptyX action | FR-17.X |
| 11 | an action whose token is not its variant's cell | INV-AC, FR-17 |
| 12 | a reachable world without the worker | INV-W (a REPLAY step would then emit no label) |
| 13 | the Architect not accepting `name` as a non-semantic label | FR-1 (decision D-1) |

Decisions for the Architect and the Owner:

- **D-1.** Whether `name` ("F-R" against "F-2"), read only by the report header, satisfies FR-1's "every other field
  mechanically equal".
- **D-2.** Acceptance of this argument (FR-1 to FR-17, FR-RS, FR-17.X, PROJ-1 to PROJ-3, the invariants).
- **D-3.** Adoption of AD-1-R5 (Owner).

## 12. Status of every obligation

| Obligation | Status | Rests on |
|---|---|---|
| FR-1 initial states | ESTABLISHED (executor argument); D-1 open | SF-2, PROJ-2 |
| FR-2 enabledness | ESTABLISHED (executor argument) | lockstep: PROJ-3, SF-12, SF-15, occurrence closure |
| FR-3 choices | ESTABLISHED (executor argument) | lockstep |
| FR-4 effect | ESTABLISHED (executor argument) | lockstep, SF-6, SF-7, PROJ-2 |
| FR-5 observations | ESTABLISHED (executor argument) | lockstep, SF-12, SF-13 |
| FR-6 edge identity | ESTABLISHED (executor argument) | SF-1, SF-13, FR-3 |
| FR-7 path projection | ESTABLISHED (executor argument) | FR-1–FR-6, FR-RS, FR-9, F-2 E1 and KEY |
| FR-8 path lifting | ESTABLISHED (executor argument) | FR-1–FR-6 |
| FR-RS replay stutter | ESTABLISHED (executor argument) | SF-5, SF-9–SF-11, INV-W, INV-T, INV-AC, INV-E1 (E1 and KEY) |
| FR-9 state properties | ESTABLISHED (executor argument) | SF-12, PROJ-1–3, F-2 KEY |
| FR-10 replay transition properties | ESTABLISHED (executor argument) | FR-RS, FR-9; diagnostic supporting |
| FR-11 safety transfer | ESTABLISHED (executor argument) | FR-5, FR-7, FR-9, FR-10, F-2 receipt |
| FR-12 D1 transfer | ESTABLISHED (executor argument) | FR-2, FR-7, SF-12, F-2 D1 |
| FR-13 no REPLAY on a cycle | ESTABLISHED (executor argument) | SF-7, FR-RS |
| FR-14 cycle projection | ESTABLISHED (executor argument) | FR-13, FR-4, FR-5, FR-7 |
| FR-15 L1 transfer | ESTABLISHED (executor argument) | FR-14, F-2 L1, FR-12 |
| FR-16 ordinary branch set | ESTABLISHED (executor argument) | FR-5, FR-7, FR-8, F-2 branches |
| FR-17 replay labels | ESTABLISHED (executor argument) | FR-RS, INV-AC, FR-16, F-R prefix (existence only), FR-17.X |

No obligation is REJECTED. Occurrence closure: **REPLAY_OCCURRENCE_CLOSURE_COMPLETE** (both checkers; section 3 and
`REPLAY-OCCURRENCE-MATRIX.tsv`).
