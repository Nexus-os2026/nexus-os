# Mother Nexus M1: mission contract and coordinator core

Status: M1 candidate on `mother/m1-coordinator`, based on `main`
`12eec16c`. Headless library only. It is not integrated and is not a passed
gate. Every capability here is PLANNED until its physical test passes on the
installed build (`MOTHER.md` §1).

## What M1 is

Mother turns an Owner goal into a **mission contract**: the goal in plain
text and the exact list of steps she would take, each one a broker `Intent`
(data, never authority). The **coordinator** keeps each mission's state and
moves it forward one step at a time. It holds no authority of its own: every
real effect goes through the existing broker, `GovernedControl`
(`crates/nexus-governed-control`), as an agent's action in an agent's run.

```text
draft ──► Drafted ──consent──► Approved{next} ──step──► Running{step}
             │  declined          ▲   │                     │
             ▼                    │   │ expiry              ├─ Done ──► Approved{next+1} / Completed
          Declined                │   ▼                     ├─ R2 waits ─► AwaitingApproval ─refresh─┐
                                  │ Expired                 └─ refused ─► Failed                    │
                                  └──────────────── broker: succeeded ◄─────────────────────────────┘
Stop (Mother's, or the broker's emergency stop) ──► Stopped, from every non-terminal state
```

## Invariants (the oracles in `src/oracles/` and `tests/` pin each)

- **I1. No effect before consent.** Until the Owner consents to a mission's
  exact contract, the coordinator makes no broker call for it at all: no run
  is opened, nothing is proposed. This covers every effect category the
  contract names: **file**, **network**, **model-tool** and **spend**.
- **I2. Consent binds the exact contract.** Consent is given to a
  `ConsentRequest` the coordinator builds itself: the contract's SHA-256
  digest, the goal, every step in full (its broker intent as canonical JSON)
  with its effect categories and declared cost, and the consent lifetime. The
  contract cannot change after drafting; another plan is another mission.
  Only steps of the consented contract can run, in order, each once.
- **I3. Consent is not authority.** Mother's consent is an additional gate.
  The broker still needs the Owner's live grant for every action and the
  Owner's native approval for every R2 action. A consented mission with no
  covering grant is refused by the broker, and the mission fails.
- **I4. Stop is honoured from every state.** `stop` and `stop_all` never
  fail for a known mission and never wait for a broker call in flight. From
  every non-terminal state the mission becomes `Stopped`; its broker run, if
  one is open, is cancelled (an effect in flight sees the run's cancellation
  token). A terminal mission stays as it ended, and no effect can start for
  it. Nothing restarts a stopped mission. A Stop that arrives while the
  consent dialog is open, while a run is being opened or while an effect
  runs wins over whatever that call returns.
- **I5. The Owner's emergency stop is Mother's stop.** Before every step the
  coordinator asks the broker whether it is emergency-stopped; if it is, or
  the broker refuses with `emergency_stopped`, the mission is `Stopped`.
- **I6. Consent expires.** Consent lasts at most `consent_ttl_ms` (at most
  8 hours) on the broker's monotonic clock. A step after that ends the
  mission as `Expired` and cancels its run.
- **I7. Effects only through the existing broker.** The `Broker` trait is
  sealed: outside the crate, only `GovernedControl` can be one. Mother's
  production code calls only `open_run` (agent runs), `agent_action`,
  `cancel_run`, `finish_when_settled`, a commitment's state and whether runs
  are stopped. It never opens an owner run, authorizes, executes, requests or
  revokes a grant, resumes, or raises the Owner's broker dialog. It has no
  file, process, network or async runtime dependency of its own, and no
  built-in consent (no production `OwnerConsent` implementation).
- **I8. Bounded, content-free journal.** The coordinator keeps an in-memory
  journal (at most 1024 entries) of mission identities, digests, step
  indexes, broker run and commitment identities and refusal classes. It
  never records a goal, an intent's content or an effect's output.
- **I9. Fail closed on what M1 cannot route.** Perception, input and the
  synthetic probe are refused when a contract is drafted. Steps are
  categorised from the intent's domain, never from a model's claim:

  | Intent domain | Categories |
  |---|---|
  | `request` (egress) | network |
  | `connector` | network |
  | `browse` | network, file (a session may download) |
  | `tool` | model-tool, file (a tool may write files) |
  | any step with `declared_cost_minor > 0` | adds spend |

## Non-claims

- Mother's consent is only as strong as the host's `OwnerConsent`. M1 has no
  native dialog and no desktop wiring (M2). Real authority stays with the
  broker's grants and native approvals.
- Spend is a declared figure the Owner sees and consents to. M1 does not
  meter what an effect costs.
- M1 has no file broker route of its own: file effects arise only from the
  broker's tools and browser downloads, under the broker's own controls.
- The journal is in memory only. Persistence, recovery and the Mother
  screen are later missions.
- Path validation, workspace grants and the broker's process-group
  ownership are not OS sandboxes. Mother adds none and claims none.
- Linux headless tests only. Windows and macOS are not claimed.
