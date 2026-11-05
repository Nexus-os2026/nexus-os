# Mother's skills map: Hybrid Intelligence → Super Hybrid Intelligence

A skill is something Mother can do under the deterministic broker, the Owner's
consent, Stop and a defined proof. The goal is a **Super Hybrid Intelligence**:
nine systems working together, with human judgement, AI reasoning and rules
each covering the others' weaknesses. Care means tested behaviour, never claimed feelings.

**Truth rule:** a skill counts only when proven on the intended installed build.
Nothing in this map is a claim of capability. TODAY inventories merged code
and its limits; NEXT and SUPER describe work still needing implementation,
qualification and acceptance. Specified ≠ implemented ≠ observed ≠ corroborated.
Only CORROBORATED PASS opens a gate. A pre-registered oracle fixes a test's expected result before work.

The [charter](MOTHER.md#1-what-mother-nexus-is), [current status](README.md#built-and-merged)
and [component limits](mother/README.md#non-claims) govern this map.
Planning sources outside the repository: `mother-owner-vision.md` (THE GOAL,
Who Mother is), `mother-roadmap-v2.md` (§§2, 5–10),
`m3-design-approved.md`, `typed-decisions-design-v4.md` and `m2e-design.md`.
Mission names identify dependencies, not permission to start work.

## The agency ladder: the guardrail comes first

| Rung | What Mother may decide | Guardrail that must be proven first | Roadmap step |
|---|---|---|---|
| 0 — Scripted | No model-chosen action; follow fixed steps | Broker, exact consent, Stop, halt and records | Today |
| 1 — Reason within one tool | What to write within a fixed job | P0 privacy, M2d approved Files, S1a page check, per-call budget and qualified local transport | M2e, M3 |
| 2 — Choose among granted tools | Which offered tool and how | Broker-only tool menu, Rule of Two for untrusted research, distinct model families | Gates B–C |
| 3 — Plan and self-correct | Plan, next step and bounded recovery | S1b sandbox, keyed journal and memory, time and money budgets per mission | Gates D–E |
| 4 — Improve skills | Propose better methods and skills | Disposable labs, independent verification, Owner promotion; never the safety core | Gate F |

**Today is rung 0:** [lanes](mother/src/lane.rs) are scripted and deterministic.
Local transport exists, but the retained worker's conversation route is shut:
[`N1_QUALIFIED = false`](kernel/src/inference/mod.rs), enforced before charge
or connection in [the transport](kernel/src/inference/ollama.rs).
That worker is not Mother's model lane. M2e-C wiring and M3's qualified page
Builder are still needed; the desktop's [dialogs decline](mother/desktop/src/confirm.rs).
No complete live mission or higher rung is claimed.

The [gate names](MOTHER.md#5-gates) are A: foundation; B: first mission
(MN-01, MN-06); C: independent research (MN-02, MN-03); D: discovery and
invention (MN-04, MN-05); E: protect and remember (MN-07, MN-08);
F: improve and teach (MN-09, MN-10); G: Owner signoff of intelligence v1.
M3 alone cannot open A or B: C-11 backup/rollback remains open, MN-06 still
needs its definition and proof, and browser rendering is unproved.

## Mission Brain

- **Skills:** understand a goal, ask what is missing, set success criteria,
  plan within limits, coordinate specialists and explain the result.
- **TODAY — Hybrid Intelligence:** deterministic [contracts](mother/src/contract.rs),
  [hash-bound consent](mother/src/consent.rs), [state transitions](mother/src/state.rs)
  and [coordinator](mother/src/coordinator.rs) exist, with lane/time budgets,
  zero money budget and restart holds. Autonomous understanding and planning
  are **not built**; the current explanations are scripted, not fresh proof.
- **NEXT:** rung 1, M2e-C and M3: a fixed bakery-page plan, selected notes,
  consented facts and a real draft; M2d-D supplies the terminal flow.
  Tool choice waits for Gates B–C; multi-step planning for Gates D–E.
- **SUPER — goal:** turn difficult goals into bounded, revisable plans,
  ask useful questions, recover safely and show what the evidence supports.
- **GUARDRAIL:** a plan grants nothing; exact contract consent, broker-issued
  authority, charged calls and deadlines remain mandatory after every change.
- **PROOF:** pre-registered MN-01 and M3 composition oracles; Owner exercises
  approve, decline, Stop and recovery; independent review matches receipts to
  the installed build. Understanding is checked by what the Owner can explain.

## Research Council

- **Skills:** investigate independently, check fresh knowledge, challenge
  claims, compare sources and separate evidence from model reasoning.
- **TODAY — Hybrid Intelligence:** **not built** as live research. Researcher A,
  Researcher B and Skeptic are [roles with scripted answers](mother/src/lane.rs),
  scheduled by the coordinator; their names do not establish independence.
- **NEXT:** rung 2, Gate C (MN-02, MN-03), after the first mission at Gate B:
  two researchers from distinct model families plus a skeptic, with sources.
  M2e local transport is a prerequisite, not a research qualification.
- **SUPER — goal:** research current primary sources before high-stakes or
  fast-changing help; cite dates, expose uncertainty and re-check stale facts.
  Local models supply reasoning; fresh research supplies current knowledge.
- **GUARDRAIL:** broker-granted egress and data boundaries; the Rule of Two
  forbids one component combining untrusted input, private data and external effects.
  Cloud escalation needs explicit provider consent and budget, never fallback.
- **PROOF:** pre-register source and independence checks, including a false
  claim the skeptic must catch; Owner runs the research task; independent
  review checks citations, dates, privacy receipts and the planted challenge.

## Discovery Engine

- **Skills:** find useful knowledge, methods and tools; check provenance,
  licences, limitations and whether a candidate improves the main pathway.
- **TODAY — Hybrid Intelligence:** **not built**. There is no Mother discovery
  lane or adoption flow in [Mother's modules](mother/src/lib.rs);
  the existing [lane roles](mother/src/lane.rs) are deterministic.
- **NEXT:** rung 3, Gate D (MN-04), after Gate C's governed research:
  investigate candidates and compare them in disposable trials.
- **SUPER — goal:** find better approaches, learn from lawful material and
  recommend adoption only when measured benefits and costs justify it.
- **GUARDRAIL:** research boundaries first, then S1b for executable trials;
  licence checks before reuse, no unvetted installs or automatic adoption.
  MiroFish and the clean-room Rehearsal Room remain **PARKED** candidates.
- **PROOF:** pre-register a baseline, known-outcome benchmark, licence checks
  and rejection cases; Owner observes an isolated trial; independent review
  checks the comparison. A simulation remains labelled “simulation, not proof”.

## Invention Lab

- **Skills:** generate alternatives, build small prototypes, try to break
  them and show where an idea fails before risking the Owner's resources.
- **TODAY — Hybrid Intelligence:** **not built**. The [Builder role](mother/src/lane.rs)
  supplies scripted output; it is not an experimenting or inventing agent.
- **NEXT:** rung 1 drafting through M2e-C/M3 first; invention itself is
  rung 3, Gate D (MN-05), following research and the S1b sandbox.
- **SUPER — goal:** combine research with diverse hypotheses, controlled
  experiments and independent criticism to produce useful, reproducible ideas.
- **GUARDRAIL:** disposable workspaces, supported execution profiles,
  bounded calls/time/cost, no outside effects and no safety-core changes.
  Real business pilots remain **PARKED**, beyond Gate G and their own gate.
- **PROOF:** pre-register success and failure criteria against a baseline;
  Owner witnesses the bounded prototype; independent review reproduces its
  result and checks cleanup. A failed experiment is evidence, never success.

## Governed Hands

- **Skills:** make approved changes, show the complete change, stop during
  work and offer only the undo or recovery actually supported.
- **TODAY — Hybrid Intelligence:** [typed File sessions](mother/src/coordinator/lite.rs),
  [the File port](mother/src/adapter/lite_port.rs) and broker
  [create-only Files](crates/nexus-governed-control/src/files/mod.rs) exist.
  Creates are bounded to an admitted workspace and full review; publication
  and settlement distinguish uncertainty. Broker undo is conditional, once
  for the last create in the process. No general file/shell authority follows.
  [M2d-D's Linux terminal host](mother/README.md#run-hands-m2d-d-candidate-linux-only)
  implements deterministic File missions through one broker-owned session. After
  emergency only status, evidence and Quit remain; entered kernel calls may finish.
  Restart is report-only, with neither cleanup permission nor undo restored.
  Automated tests are not Owner acceptance; the live page workflow is **not built**.
- **NEXT:** rung 1, M2e-C/M3 drafting and wiring;
  M2b-B later presents the proven flows in native dialogs.
- **SUPER — goal:** execute complex plans through explicitly supported,
  broker-offered tools while keeping consent, inspection and recovery usable.
- **GUARDRAIL:** mission/run/grant binding, complete escaped review, single-use
  approvals, revocation, expiry and Stop; S1b containment before broader tools.
  No “any command”; cancellation cannot undo an effect already performed.
- **PROOF:** pre-registered Files and settlement oracles, then M2d-D's host
  oracles; Owner physically declines, revokes, stops and checks undo; independent
  review reconciles disk, ledger and broker receipts. C-11 needs separate proof.

## Guardian+Verifier

- **Skills:** refuse unsafe actions, protect within consented limits, check
  results independently and make uncertainty or failure visible.
- **TODAY — Hybrid Intelligence:** [door binding](crates/nexus-governed-control/src/governed/door.rs)
  and [halt](crates/nexus-governed-control/src/governed.rs) exist. The kernel's
  [inert-page checker](kernel/src/inert_page/mod.rs) and Mother's pure
  [page Verifier](mother/src/verifier.rs) exist; the Mother lane is **not wired**.
  The second reader checks text extraction only: no rendering, layout or
  general prose-truth proof. [Typed decisions T1](crates/nexus-typed-decision/src/lib.rs)
  supplies pure advisory rules, strict reading and single-use slots, with no
  live judge transport or lane integration. Broad protective intelligence is **not built**.
- **NEXT:** rung 1, M3 wiring and physical checks; typed decisions T2–T5
  need their own qualification and activation. Gate E adds protection (MN-07).
- **SUPER — goal:** detect risks, explain them, propose proportionate defensive
  fixes and verify results without approving the work she helped create.
- **GUARDRAIL:** deterministic checks always decide; advice grants nothing
  and cannot turn a failure into a pass. No hacking back or seized control.
- **PROOF:** pre-registered M3-O1–O9/O16 and future composition oracles;
  typed advice needs Q-R-TD and the frozen held-out benchmark. Owner tests
  Stop and planted faults; independent review checks refusals and receipts.

## Evidence Memory

- **Skills:** remember what happened, why it was accepted, what failed,
  source dates and corrections, without turning remembered text into authority.
- **TODAY — Hybrid Intelligence:** [journal](mother/src/journal.rs),
  [private store](mother/src/private.rs) and [durable file storage](mother/src/adapter/private_file.rs)
  exist for mission contracts, paused work and replay. The journal is unkeyed:
  a full rewrite or tail truncation by a writer is not detected. Private content
  is not encrypted; there is no retention/purge. Authenticated learning memory
  across missions is **not built**; stored content grants nothing.
- **NEXT:** rung 3, Gate E (MN-08): keyed journal/authenticated memory before
  cross-step recovery depends on remembered knowledge; record dates and provenance.
- **SUPER — goal:** retain verifiable lessons and Owner corrections, explain
  their origin, re-check stale knowledge and support learning both ways.
- **GUARDRAIL:** HMAC authentication before Gate E; it adds authenticity,
  not secrecy. Separate private content from receipts; consent controls reuse.
- **PROOF:** existing replay/privacy oracles plus pre-registered forgery,
  truncation and stale-knowledge tests for the new memory design; Owner tests
  restart and correction; independent review verifies provenance and refusals.

## Evolution Engine

- **Skills:** propose better methods, compare candidates, retain improvements
  and roll back a promoted skill when it regresses.
- **TODAY — Hybrid Intelligence:** **not built** for Mother. The broker's
  [legacy evolution action stays closed](crates/nexus-governed-control/src/planned.rs);
  [isolation guards](mother/src/adapter/scan.rs) exclude the old self-improve stack.
- **NEXT:** rung 4, Gate F (MN-09), after Gates D–E supply disposable
  experimentation, independent verification and authenticated memory.
- **SUPER — goal:** improve useful skills through sandbox candidate → benchmark
  → independent verifier → Owner promotion → rollback, with a visible history.
- **GUARDRAIL:** never modify or promote the safety core; no self-approval.
  Freeze benchmarks before evaluation; promotion remains the Owner's decision.
- **PROOF:** pre-register improvement, regression and rollback oracles;
  Owner physically promotes and reverses a candidate; independent review checks
  the comparison and confirms that the safety core and authority did not change.

## Teaching Intelligence

- **Skills:** explain choices, show evidence, teach a safe next step and
  measure what the human can do alone, including how to decline Mother.
- **TODAY — Hybrid Intelligence:** **not built** as a verified teaching handoff.
  The [coordinator's explanation](mother/src/coordinator.rs) is a scripted summary:
  it prints “holds” for criterion text, not a per-check evidence report.
  The [terminal walk](mother/host/examples/walk.rs) demonstrates deterministic
  consent/restart flows only; it is not the live bakery mission or a tutor.
- **NEXT:** rung 1, M3's honest explanation and the Owner's independently
  checked page edit; Gate F (MN-10) later proves broader teaching.
- **SUPER — goal:** adapt explanations, help after setbacks without blame,
  teach from fresh evidence and grow the user's independence and judgement.
- **GUARDRAIL:** teach only what was proven, name unchecked parts, respect no
  and Stop, use safe defaults and simple explanations, especially for children.
  Pathfinder and Coach mode remain **PARKED**, not current teaching capabilities.
- **PROOF:** pre-register M3 explanation/claims oracles and edit checks;
  Owner explains what will change, declines safely and makes the edit himself;
  independent review checks the artifact and measures unaided understanding.

## How the systems become super together

The main pathway stays **goal → plan → governed execution → verified proof → teaching**.
At the goal, Mission Brain will frame the goal; Research Council will check fresh
knowledge; Discovery Engine will find methods; Invention Lab will test alternatives;
Governed Hands will act within permission; Guardian+Verifier will challenge results;
Evidence Memory will keep provenance; Evolution Engine will improve methods;
Teaching Intelligence will teach the human. Gate G requires the Owner's signoff of
the combined intelligence, with corroborated evidence, not nine component names.

The Owner supplies purpose, judgement and consent. AI supplies proposals and
reasoning. Deterministic rules bound effects and demand evidence. Each covers
weaknesses in the others: learning goes both ways, through Owner corrections
and the human's growing ability. Care notices a setback, explains risk, offers
an honest small next step and accepts rest or refusal; it never promises profit.

The model ladder is local first: M2e transport → local typed decisions T1–T5
→ separately designed cloud transports (T6 Jev, then reasoning models).
Cloud is **PLANNED**, unavailable today; every provider needs an explicit broker
egress grant, minimal data, Owner consent and charges before calls. No silent fallback.

Mother will never break the safety core, act without consent, claim before proof
or become un-stoppable. Grit never crosses a no, Stop or failed check.
**PARKED:** Pathfinder, Coach mode, Rehearsal Room/MiroFish, business pilots,
phone/voice and companion doors, emotional sensing, external coding agents in
Mother's sandbox, and very large SSD-streamed models. Each needs an explicit
mission and its own guardrails; a future vision does not activate it.

**Builder rule:** no skill is added beside the main pathway; parked ideas stay parked; update this map only when a skill is proven or an approved mission changes it.
