# Configuration identity at Candidate C

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01, section 9; basis AD-1-R4 N.9, N.10, E.1 and K.
Executor: Claude Code, Claude Opus 5.5. Static: nothing was run. The raw record is
`receipts/13-config-identity.json`, written by `tools/config_identity.py`.

**Result: F-3, F-PT and F-B match AD-1-R4 N.9 field by field; their actor lists, derived from `fn actors`, match
N.10; the cap is 30,000,000 and the time bound 45 minutes for each; `SYMMETRIC` is exactly the eight approved
shapes, and no other named configuration passes `assert_symmetric`'s static checks.**

## Method

Every top-level `const NAME: Config = …;` of `model_tests.rs` at C is evaluated from its tokens: struct literals,
struct update (`..BASE`, `..F3`), `default_run(…)` evaluated from its own body, and the cap and time constants. No
value is taken from a comment or a run. Where the check models a function's behaviour, it is bound to that
function's exact bytes:

| Item | Lines at C | sha256 at C | Audited at | Same bytes |
|---|---|---|---|---|
| `struct Config` (20 fields) | 36–74 | `a40ac57ee2eaa872a687b3d1714deacf9d49062281fc4a028f912a8a30331a7e` | SOURCE-R2 | yes |
| `fn default_run` | 104–110 | `24584e118b8843423257a3382f5439b0286b69d3e28bafa1c8165805f023b449` | SOURCE-R2 | yes |
| `fn actors` (transliterated, and its full text compared) | 2332–2347 | `5a96285fc20c241994e3e79f530afeeaf2e17ff9968455bdae1e5d359b11b956` | SOURCE-R2 | yes |
| `fn shape` | 6082–6091 | `e23f7f25d8f7059e418a7d8780163ab17674908c8c4ca98aa7f8645211a9c742` | EXT | yes |
| `fn assert_symmetric` | 6095–6128 | `e32321eea0ca217348bda5aea4e49a918b9325f003e1f6e912a65949aa139ac0` | EXT | yes |

## 1. F-3, F-PT, F-B against AD-1-R4 N.9

`struct Config` has exactly N.9's 19 fields plus `name`.

| Field | N.9 (F3) | F-3 | F-PT | F-B |
|---|---|---|---|---|
| name | — | `F-3` | `F-PT` | `F-B` |
| facts | 2 | 2 | 2 | 2 |
| conflict | [true, true] | [true, true] | [true, true] | [true, true] |
| env | true | true | true | true |
| batch | false (F-B: true) | false | false | **true** |
| faults | 2 | 2 | 2 | 2 |
| admissions | 2 | 2 | 2 | 2 |
| gating | 2 | 2 | 2 | 2 |
| gating_conflict | false | false | false | false |
| acks | 0 | 0 | 0 | 0 |
| ack_attempts | 0 | 0 | 0 | 0 |
| malformed | false | false | false | false |
| cell_constraint | false | false | false | false |
| worker_unwinds | false | false | false | false |
| drop | false | false | false | false |
| replays | 0 | 0 | 0 | 0 |
| blanket_uncertain | false (F-PT: true) | false | **true** | false |
| l1 | true | true | true | true |
| cap | 30,000,000 | 30,000,000 | 30,000,000 | 30,000,000 |
| time | 45 min | 2,700 s | 2,700 s | 2,700 s |

`FULL_CAP = 30_000_000` and `FULL_TIME = Duration::from_secs(45 * 60)`; `F3` takes both from `BASE`, and `FPT` and
`FB` are `..F3` with one field each. None goes through `default_run` (which sets 250,000 and 180 s).

## 2. Actors against AD-1-R4 N.10

`fn actors`: `Fw(i)` for each writer (one if `batch`, else `facts`), `Se`, `Wk`, `Gc(g)` for each gating caller,
`Ack(a)` for each acknowledgement actor, `Adm`, `Env` if `env`, `Replay` if `replays > 0`.

| Configuration | Derived | N.10 | Match |
|---|---|---|---|
| F-3 | [Fw(0), Fw(1), Se, Wk, Gc(0), Gc(1), Adm, Env] | [Fw(0), Fw(1), Se, Wk, Gc(0), Gc(1), Adm, Env] | yes |
| F-PT | [Fw(0), Fw(1), Se, Wk, Gc(0), Gc(1), Adm, Env] | same | yes |
| F-B | [Fw(0), Se, Wk, Gc(0), Gc(1), Adm, Env] | [Fw(0), Se, Wk, Gc(0), Gc(1), Adm, Env] | yes |

N.10 is about the *printed* bounds and actors of the run; that remains EXECUTION_PENDING. This is the static
derivation only.

## 3. The cap and the time bound

- F-3, F-PT, F-B: cap 30,000,000 keys and time 45 minutes, as above.
- `explore`, `explore_experimental` and `explore_symmetric_kept` each check `visited.len() as u64 > config.cap`
  and `start.elapsed() > config.time` before expanding a key (AD-1-R4 K): a run is within its cap if it completes
  with at most the cap's keys discovered.
- The three required tests pass the constants unmodified:

| Test (lines at C) | Ignored | Calls |
|---|---|---|
| `gc_symmetry_f3` (6931–6943) | yes, run only by name | `explore_symmetric(&F3, step_adm, Mode::Quotient)` |
| `gc_symmetry_fpt` (8232–8236) | yes | `symmetric_full_exploration(&FPT)` |
| `gc_symmetry_fb` (8240–8244) | yes | `symmetric_full_exploration(&FB)` |
| `symmetric_full_exploration` (8218–8228) | — | `explore_symmetric(config, step_adm, Mode::Quotient)` |

So each required run composes the admission observer (`step_adm`, `Mode::Quotient`) with the symmetry quotient.

## 4. The SYMMETRIC allow-list

`const SYMMETRIC: [Config; 8] = [X2, SYM1, F3, SYM_PT, SYM_B, SYM_BI, FPT, FB];` — names
`X-2, SYM-1, F-3, SYM-PT, SYM-B, SYM-BI, F-PT, F-B`, eight distinct shapes. X-2 and SYM-1 were shown symmetric by
GC-SYMMETRY-01's exhaustive differential and F-3 by its analysis; SYM-PT, SYM-B and SYM-BI by CLOSURE-PREP-01's
differential and F-PT and F-B by its analysis. Under AD-1-R4 E.1 only F-3, F-PT and F-B may use the composed
quotient for 8.3; the other five are small differential fixtures with the default caps.

`assert_symmetric`, statically: it refuses unless `gating == 2`, `!gating_conflict`, `acks == 0`,
`ack_attempts == 0` and `!malformed`; unless some member has the same `shape` (the `Debug` text of the whole
configuration, name included, with cap and time zeroed); and unless `cap` and `time` are no higher than that
member's. Its last check (the initial state is its own mirror) is a runtime guard and is not evaluated here.

Every named configuration at C, through those checks:

| Constant | Name | gating | acks | gating_conflict | cap | time (s) | Result |
|---|---|---|---|---|---|---|---|
| BASE | (empty) | 0 | 0 | false | 30,000,000 | 2,700 | refused: callers |
| X1 | X-1 | 0 | 0 | false | 250,000 | 180 | refused: callers |
| X2 | X-2 | 2 | 0 | false | 250,000 | 180 | admitted, as X2 |
| X3 | X-3 | 0 | 0 | false | 250,000 | 180 | refused: callers |
| X4 | X-4 | 0 | 1 | false | 250,000 | 180 | refused: callers |
| X5 | X-5 | 0 | 0 | false | 250,000 | 180 | refused: callers |
| F1 | F-1 | 2 | 0 | false | 30,000,000 | 2,700 | refused: shape not listed |
| F2 | F-2 | 2 | 0 | false | 30,000,000 | 2,700 | refused: shape not listed |
| F3 | F-3 | 2 | 0 | false | 30,000,000 | 2,700 | **admitted, as F3** |
| F4 | F-4 | 2 | 0 | false | 30,000,000 | 2,700 | refused: shape not listed |
| F5 | F-5 | 0 | 1 | false | 30,000,000 | 2,700 | refused: callers |
| F6 | F-6 | 0 | 1 | false | 30,000,000 | 2,700 | refused: callers |
| F7 | F-7 | 0 | 2 | false | 30,000,000 | 2,700 | refused: callers |
| F8 | F-8 | 1 | 1 | false | 30,000,000 | 2,700 | refused: callers |
| FPT | F-PT | 2 | 0 | false | 30,000,000 | 2,700 | **admitted, as FPT** |
| FB | F-B | 2 | 0 | false | 30,000,000 | 2,700 | **admitted, as FB** |
| FB2 | F-B2 | 0 | 0 | false | 30,000,000 | 2,700 | refused: callers |
| FG | F-G | 2 | 0 | true | 30,000,000 | 2,700 | refused: callers (G2's seeded conflict) |
| FW | F-W | 2 | 0 | false | 30,000,000 | 2,700 | refused: shape not listed |
| FD | F-D | 2 | 0 | false | 30,000,000 | 2,700 | refused: shape not listed |
| FR | F-R | 2 | 0 | false | 30,000,000 | 2,700 | refused: shape not listed |
| SYM1 | SYM-1 | 2 | 0 | false | 250,000 | 180 | admitted, as SYM1 |
| SYM_PT | SYM-PT | 2 | 0 | false | 250,000 | 180 | admitted, as SYM_PT |
| SYM_B | SYM-B | 2 | 0 | false | 250,000 | 180 | admitted, as SYM_B |
| SYM_BI | SYM-BI | 2 | 0 | false | 250,000 | 180 | admitted, as SYM_BI |

Exactly the eight members pass, each by its own shape. Every configuration AD-1-R4 E.1 forbids is refused: F-5,
F-6, F-7, F-8, F-B2 (acknowledgements or the wrong number of callers) and F-G (G2's seeded conflict); and F-1, F-2,
F-4, F-W, F-D, F-R and X-1, X-3, X-4, X-5. The receipt names the first check that refuses; its "callers" check
is `gating == 2 && !gating_conflict`.

Configurations built inside tests (for example `Config { cap: 99_999, ..F3 }` in `gc_symmetry_prefix_f3`, or the
raised-cap refusal tests) are not named constants; `assert_symmetric` treats them the same way: a lower cap passes,
a raised cap or another shape is refused, which the refusal tests among N.8's 51 exercise at run time.
