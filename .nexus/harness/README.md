# Nexus development harness

This repository carries policy, procedures, mission scope, execution state,
evidence and handover separately. It does not grant runtime capabilities.

Start with root `AGENTS.md`, then [POLICY.md](POLICY.md), then the explicit
mission path supplied by the Architect. There is no authoritative CURRENT file.
[Mission records](../missions/README.md) explain creation and resumption.

Provider skill adapters route to the same [procedures](procedures/mission.md).
Use [RECEIPTS.md](RECEIPTS.md) and [contract.json](contract.json) for evidence.
The Markdown templates use exact level-two field headings; unknown values must
say UNRECORDED with a reason, and absent permission is DENIED.

Run offline from the repository root:

```text
python3 -B scripts/verify-agent-harness.py --mission .nexus/missions/NEXUS-HARNESS-V1
python3 -B -m unittest discover -s scripts/tests -p test_agent_harness.py -v
```

Use `python` instead of `python3` where that is the installed Python 3 launcher.
Only the standard library and Git are required. No executable file bits,
symlinks, cloud services or shell-specific invocations are required.
The validator never runs commands taken from a receipt or mission file.
Schema/coverage validation does not prove authorization or semantic security.

Root instructions have a 12,000-byte project budget, leaving headroom under the
[documented default 32 KiB combined Codex limit](https://learn.chatgpt.com/docs/agent-configuration/agents-md).
This is not a guarantee against unrelated global/nested instruction bloat.
