# MOTHER.md — Mother Nexus Charter and Governance

> **Your Mother. Our Home. Our Future.**
>
> Effective 2026-10-31 by Owner decision. This file is the current source of
> roles and product direction. Where `AGENTS.md` or `CLAUDE.md` name an
> "Architect", this file says who that is and how independence is preserved.
> It does not weaken any permanent engineering or trust-boundary rule.

## 1. What Mother Nexus is

Mother Nexus is a governed, local-first Hybrid Intelligence. It turns a human
goal into a verified result: Mother understands and coordinates; independent
specialist lanes research, challenge, invent, build and verify; the human
learns. A deterministic capability broker, never a model, holds real
authority.

Mother protects within limits the Owner sets in advance. She never seizes
control, invents permissions, conceals risk "for the user's own good" or keeps
the user dependent.

Every capability is **PLANNED** until its physical test passes on the intended
build. Specified ≠ implemented ≠ observed ≠ corroborated.

## 2. Roles

| Role | Holder | Authority |
|---|---|---|
| Owner | The project Owner | Final authority. Goals, approvals, physical acceptance |
| Mother Architect | Claude Code | Designs, writes missions, implements, tests, reports |
| Independent review | See §3 | Checks work the Architect produced |

There is no ChatGPT, GPT or Codex Architect role. Earlier documents that
assign one are historical.

## 3. Independence without a second architect

The Mother Architect writes and builds the plan, so it never approves its own
work. Acceptance comes from evidence it cannot talk past:

1. **Pre-registered oracles.** Tests, expected receipts and negative controls
   are written and committed before the implementation they judge. They are
   not edited after a result is seen.
2. **Deterministic gates.** CI, negative tests, mutation and tampering checks.
   No `continue-on-error`, skipped security test or loosened assertion.
3. **Owner physical acceptance.** Nothing counts as working until the Owner
   performs it on the installed build and backend/OS receipts agree.
4. **Fresh-context review.** Security-core changes are reviewed by a separate
   Claude Code session that receives only the diff, the oracles and the
   invariants, not the implementing session's reasoning.
5. **Owner approval of security-core changes**, after a plain-language summary.

### Owner-only approvals

- any change to the security core (below);
- merging into `main`;
- destructive Git (see `AGENTS.md` §7.3);
- host changes: package installs, `sudo`, system services, new test users;
- spending, publishing, external accounts, network egress beyond granted origins;
- accepting a gate or capability;
- changing this file's roles.

### Security core

Kernel authority and containment (`workspace_authority`, `governed_path`,
`resource_limiter`, `permissions`, `capabilities`, audit, emergency Stop),
`crates/nexus-governed-control`, `crates/nexus-verifier-sandbox`, CI
workflows, and the security guard tests. Mother code may call the core; it may
not change it without Owner approval.

## 4. Product direction

- Phases 0–3 (Linux support profile) are the kept foundation. Old Nexus is not
  the goal; it is not polished further.
- Mother is built as a new, small layer (`mother/` plus one Mother screen) on
  that foundation. It reaches real effects only through the existing broker.
- Old surfaces that Mother does not need stay closed with an honest backend
  refusal. They are not repaired and not resurrected wholesale.
- Mother may improve her own skills through the governed evolution loop
  (sandbox candidate → benchmark → independent verifier → Owner promotion →
  rollback). She never changes or promotes her security core.

## 5. Gates

**Gate A (narrowed): the core Mother stands on.** Required physical tests:
C-01 launch and data, C-04 file broker, C-05 approve and undo, C-06 deny and
replay, C-07 revocation, C-08 Stop, C-10 legacy closures, C-11 backup and
rollback. C-02, C-03 and C-09 are folded into the Mother screen's own tests.

**Gates B–G: Mother intelligence.** B first mission (MN-01, MN-06) · C
independent research (MN-02, MN-03) · D discovery and invention (MN-04,
MN-05) · E protect and remember (MN-07, MN-08) · F improve and teach (MN-09,
MN-10) · G Owner signoff of intelligence v1. Each gate opens on corroborated
physical evidence, never on dates.

**Mission ladder:** M0 this charter · M1 mission contract and coordinator
core · M2 Mother screen · M3 builder lane, first real local website. Gate A
physical tests run on the same install as M2 and M3.

**Result labels:** NOT TESTED · OBSERVED PASS · CORROBORATED PASS · FAIL ·
BLOCKED · INCONCLUSIVE. Only CORROBORATED PASS on the intended build opens a
gate.

## 6. Everywhere

One Mother; her home, memory, broker and keys live on the Owner's machine.
Other devices are paired doors: per-device keys approved on the home machine,
instant revocation, Stop from any door, honest offline state, end-to-end
encryption. Each surface (Android companion, Windows, macOS) has its own gate.

## 7. Claude usage policy

- One mission per fresh Claude Code session; `/clear` after a sealed mission.
- Opus for architecture, security and hard root causes; Sonnet for routine
  scoped code and tests.
- No subagents by default. No repeated full CI or full-workspace runs.
- Check usage before each mission; at 75% of an allowance, start no new mission.
- Scripts and tests do the checking, not extra model turns.
