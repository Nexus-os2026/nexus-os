# Mother Nexus security review

Use for a scoped review requested by the current Mother mission. Read AGENTS.md
§18, MOTHER.md and the mission before acting. This skill grants no edit,
security-core, host, network, push or integration permission.

1. Establish the base, candidate, changed files and expected trust boundary.
   Check every raw database query for SQL injection and every unsafe block for a documented safety argument.
   The unsafe review check never authorizes an exception to the workspace's
   `unsafe_code = "forbid"` policy. Check source for hardcoded secrets/API keys
   without printing their values, validate inputs at every exposed command/API
   boundary, and inspect unhandled errors for information leaks.
2. Trace authority ownership, lifetime, narrowing, expiry, revocation and exact
   request binding. Strings, saved records, model answers and typed advice never
   grant permission. No old actuator or service path may bypass Mother's port.
3. Check Stop/halt before waits, races at every admission boundary, uncertain
   File publication, retained settlement, cleanup and real error propagation.
   Check capability escalation, prompt injection, data exfiltration and governance
   bypass explicitly, including capability and fuel checks before actions,
   accurate fuel accounting, approval for sensitive/legacy L2+ operations, and
   audit events on state changes and LLM/tool transitions. Review retained
   prompt/output filters and suspicious-output circuit breakers without treating
   them as authority. Any exposed content/request boundary must also be checked
   for injection, forged cross-origin requests and direct-object access without
   authorization; removed frontend tooling stays removed.
4. Check closed evidence fields, canary coverage, private-store separation and
   no secrets or private content in records/errors. Unkeyed chains are not
   authenticated memory and private content is not encrypted.
5. Check native GTK4 boundaries and fail-closed absent dialogs, local transport
   identity/egress/budgets and its qualification closure, inert-page non-claims
   and deterministic checks dominating advisory typed decisions.
6. Inspect callers and failure paths in a separate mental pass. Source scans
   are guards, not proof. Keep pre-registered oracles strict and unchanged unless
   an explicit amendment is authorized for a stale documentation assumption.
7. Run the mission's exact checks and shared slot. Use scripts/security-audit.sh
   only if the mission authorizes that gate; never truncate findings to make
   green, fetch/install tools by assumption or print credential values.
8. Report findings with file/line, evidence, severity, limits and ALL blockers.
   Report PASS/FAIL per check, or BLOCKED with the missing evidence; never turn
   an unperformed check into PASS or ALL CLEAR.
   Security-core changes still need fresh-context review and Owner approval.

See SECURITY.md and THREAT_MODEL.md for current claims. A review is not a
release approval or physical acceptance.
