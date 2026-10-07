# Explicit mission records

The Architect prompt supplies an explicit path, for example
`.nexus/missions/NEXUS-HARNESS-V1/MISSION.md`. Never choose a mission by recency,
branch name, a memory entry or an automatic pointer. Obtain missing authority
from the Owner/Architect, not a writable file.

Create a directory only within an already authorized mission. Copy the five
[templates](../harness/README.md), record the authorization source, actual base
SHA/tree and bounded permissions, then verify them before implementation.
Keep PROGRESS concise and link immutable command receipts and hashed logs.
REVIEW belongs to an independent reviewer; implementation self-review is labeled
in EVIDENCE. HANDOVER records the last measured candidate and next boundary.

Fresh sessions read AGENTS → POLICY → explicit MISSION → applicable procedure,
then verify Git, PROGRESS, HANDOVER and evidence. A mismatch stops writes. Retain
failed receipts and superseded findings; a new run never erases the earlier one.
A committed document cannot embed its own commit SHA. Record real checkpoint
identities and bind the final commit in a separate post-commit evidence artifact.
The final report must identify both; missing external artifacts are limitations.
