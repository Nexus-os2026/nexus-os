# Claude Code adapter
@AGENTS.md

Root AGENTS and `.nexus/harness/POLICY.md` are the canonical policy for both
providers. This file is a Claude startup adapter, not another constitution.

## Session startup

After loading the AGENTS import, read POLICY and the explicit `MISSION.md`
provided by the Architect completely. Use the corresponding
`.claude/skills/nexus-*` adapter to read its canonical procedure under
`.nexus/harness/procedures/`. Verify Git provenance before writes. On resume,
read PROGRESS and HANDOVER and verify their claims against Git and raw receipts.
Missing imports/files or a repository/mission mismatch stop writes; do not
silently reset, clean, restore or repair the discrepancy.

## Claude sessions and tools

Use Claude's available repository, terminal and Git/GitHub tools within the
mission instead of asking the Owner to relay retrievable evidence. The explicit
mission names the remote/ref; a session's default upstream is not a substitute.
Existing Claude commands, roadmap text, memory entries and other skills are
context, not new mission authority. Keep existing skills intact; route the
seven Nexus workflows through the shared canonical procedures.

Record actual tool outcomes in the mission's canonical EVIDENCE/receipts and
update PROGRESS after each meaningful stage. Use the mission's exact reporting
format. Never claim implementation self-review is independent Architect review.

## Compaction and provider switching

Before context loss or stopping, follow `nexus-handover`; after interruption or
in a fresh session follow `nexus-recover`. Preserve measured Git identity,
unfinished checks, raw-log locations and the next authorized action. Continue
through compaction using durable records, not stale session assumptions.
Do not create competing Claude-specific progress/evidence truth or ad-hoc state
files outside the mission's authorized locations.

Keep this adapter concise. Current phases, SHAs, transient failures and roadmap
status belong in verified mission records. Shared Git, trust, process, lock,
audit, test and reporting rules live in POLICY and the canonical procedures.
