# Gate Box Slice 1 / PR-1 frozen mission and oracle register

Authority: Owner request, 2026-11-07, authorizes PR-1 implementation only.
Branch: mother/gatebox-slice1-core.
Base: 899e3872c5b61223c87b905f63c66947a816259b; no branch movement.
Frozen sheet: /home/nexus/NEXUS/gatebox/slice1-mission.md.
Sheet SHA-256: 28ac57fc6eb1565353d2ea956225703db396ebe009efd66fc34fef0b2f8a8094.
This register incorporates the frozen sheet by byte identity. The sheet's
planning-only restriction is superseded only for the Owner-authorized PR-1.

Scope: snapshot.py, receipt.py, policy/gates.json, policy/exclusions.json,
tests/test_unit_core.py, tests/test_unit_contract.py, and this MISSION.md.
No Rust/Cargo/workflow/validator changes. No installation, network, sudo,
secrets, Git writes, PR-2 implementation, phase advance or acceptance claim.
Commit checkpoints are folders containing message.txt, files/<repo paths>,
and deleted.txt, not Git commits. Maximum final added/deleted lines: 1,500.
Offline gate: python3 -I -m unittest discover -s tools/gatebox/tests -v.
Also run git diff --check and an untracked-file whitespace/scope review.

## Frozen interfaces and boundaries

snapshot.parse_request consumes one complete length-prefixed UTF-8 JSON frame,
with exactly repo_id/full_sha/gate_set_id string fields, a full lowercase SHA-1,
trusted registration and peer UID. It is a pure validation primitive, not a
socket server. Admission, deadlines, BUSY and actual SO_PEERCRED belong to PR-2.
Registration must declare sha1 and UID. No submitted paths or commands.

snapshot.GitSource takes the trusted canonical common-directory path and
registered device/inode. It uses bounded raw cat-file plumbing with no inherited
Git configuration/environment, hooks, replacements, lazy fetch or transport.
snapshot.export independently hashes commit/tree/blob bytes, validates the
complete tree/blob closure, and exports to a fresh private destination.
Parent objects are not required. Git archive, checkout, filters and ls-tree
are never authenticity roots. No candidate code executes during import.
Reject replacements, alternates/promisors, missing/wrong objects, gitlinks,
unsafe/duplicate names, unsupported modes, escaping links and link ancestors.
Bounds: 300s; 100,000 objects and entries; 3GiB raw; 2GiB regular export;
64MiB/blob; 8MiB/tree; 1MiB/commit; depth 64; path 4096B; component 255B.
Snapshot records commit/tree, sorted compact JSON content manifest and Cargo.lock
SHA-256; files/directories are frozen read-only, executable bits preserved.
Snapshot construction failure must remain an error; partial data is not success.

receipt.publish creates a fresh pending directory, writes summary/artifacts and
an exact hash/size inventory, fsyncs files/directories and atomically renames
within one filesystem, never overwriting a run ID. Completed dirs 0755/files 0444.
Failure preserves pending evidence. Schema records all sheet §4 evidence groups;
PASS_SCOPED requires complete successful invocation/coverage/log/cleanup evidence.
receipt.verify_integrity checks owner/modes/types, no-follow ancestry and exact
inventory/hashes. It ALWAYS returns UNAUTHENTICATED: ownership checked against a
caller-supplied UID in a unit fixture cannot authenticate a host receipt.
Host-context/root registration/ACL authentication and selectors belong to PR-2.
Authentic failure and success will remain separate in that verifier.

Policy fixes all nine §6 gates, offline absolute Cargo argv, no-fail-fast
orchestration, emitted-artifact libtest discovery/reconciliation, host coverage,
desktop live repeat (23 registered names; minimum 14), doctests, walk and
snapshot whitespace approximation. The custom phase2_live_sandbox target is
compiled, never executed/discovered, with exactly 39 individually named exclusions.
A static contract test is not an execution/installed E2E claim.
Every receipt carries the mutable host Rust/registry limitation. Inventories do
not prove immutable provenance or defeat change-and-restore attacks. Candidate
assertions are not independent correctness proofs. GitHub CI/audits stay required.

## Reuse provenance (read-only untracked Phase 1 bytes)

/home/nexus/NEXUS/mother-gatebox-sbx/tools/gatebox/gateboxlib.py
SHA-256 ae59a71222bc1c3c785098fcdc6cf2a6996f7eec9d1e5fb75975253303879b5c.
Inspected lines 216–257: path/link checks and sorted content-manifest encoding;
adapted with stricter component validation and raw object closure checks.
Inspected lines 302–414: blob identity/executable-bit checks, adapted without
archive/ls-tree authority. Lines 516–602: receipt inventory/hash/schema checks,
adapted to atomic publication, fd-based checks and explicit UNAUTHENTICATED result.
No claim that these Phase 1 bytes are committed. No bwrap, Chrome waiver,
same-user chmod authentication or workspace test execution copied.

## Pre-registered oracles (immutable after first red run)

SnapshotTests: exact bytes/attributes/modes/manifest/lock; wrong hash;
wrong types/missing objects; unsafe trees/links; resource bounds; actual
raw Git plumbing with synthetic loose objects, replacements and alternates.
ReceiptTests: publication, failure remains FAILED/UNAUTHENTICATED, never overwrite;
changed/missing/link/writable artifacts and unsafe ancestry; schema/fabricated
success rejection; rename failure retains pending evidence.
ContractTests: exact request acceptance; abbreviated SHA and unknown key
rejection; framing/duplicate/UID/type/encoding/registration rejection; fixed
nine gates, desktop names/adapters and exact 39 exclusions.
No installed-qualification PASS line may be printed by these unit tests.

## Pre-registered mutation obligations (§8 IDs)

MP-6: remove raw hash comparison -> test_wrong_hash_rejected.
MP-7: allow short SHA -> test_request_abbreviation; allow unknown field ->
test_request_unknown_key. Exercise both variants separately.
MP-11: permit group/other artifact writes ->
test_artifact_tamper_missing_symlink_and_write_modes (primitive only).
MP-15: remove one exclusion -> test_exact_exclusions (static contract only).
MP-16: permit fabricated successful summary -> test_schema_and_fabricated_success
(schema rejection only; installed lifecycle/E2E not exercised).
MP-1..5, MP-8..10, MP-12..14: not exercised, PR-2 installed behavior absent.
In particular no detached-copy host authentication is claimed by PR-1.
Run each applicable plant alone, record its failing assertion, restore exact
bytes and rerun the positive control. Never checkpoint planted bytes. If masked
by another guard, report not exercised. Installed portions of plants remain
unqualified even when a primitive assertion kills the corresponding mutation.

Stop: unresolved OPEN, wrong provenance, scope/size violation, weakened trust,
missing prerequisites, surviving required PR-1 plant or two failed repairs on
one signature. Do not change these tests after seeing results. Report actual
red/green/plant evidence externally; independent review and Owner acceptance
remain outstanding. No CI, installation or whole-Slice-1 success is claimed.

## Review-1 repair contract (Owner mission, 2026-11-07)

The review-1 mission authorizes bounded PR-1 repairs and numbered checkpoint
folders only. The original installed acceptance and PR-2 boundaries remain.
PASS_SCOPED now binds both receipt and command environments to gates.json.
Host and whitespace adapters record fixed isolated Python init.py --gate argv;
PR-2 must implement these registered adapter entry points and reconstruct the
exact environment. No adapter has been installed or executed by PR-1 tests.
Host reconciliation requires a nonempty union of every host artifact's
workspace test names, with the same union in the host command inventory.

Additional oracles cover valid scoped publication and single-change negatives,
unknown logs without hashes, composed-link escape/deadline, a competing receipt
publisher after lexists, Git administration redirects and malformed evidence.
The content-manifest digest is frozen from explicit fixture records, independent
of manifest(). Review-specific mutations and all PR-1 plants are recorded in
the external repair report; each mutated source is restored byte-for-byte.
Missing-status and desktop-no-op mutations exercise schema primitives only;
installed private-status authority and actual desktop execution remain PR-2.

## Q9 / F5 custody amendment (Owner mission, 2026-11-07)

This bounded stacked amendment starts at 0b2a37079969a4baf7700f0b6cefcbda62f0b303,
branch mother/gatebox-slice1-custody. It implements proposal §3 Design A only.
The earlier PR-1-only wording describes that historical checkpoint, not this
mission. #111 and this slice form one integration unit; #115 must compose and
rerun. Q8 installation HOLD remains. No Git writes, network, sudo, installation,
host service actions, phase advancement or installed acceptance are authorized.

The external frozen sheet remains historical text. Q9 overrides exactly its
§3/§4/§5/§7/§8 custody provisions below. policy/custody.json records literal
replacements to the original frozen unit text; tests apply only those exact
replacements and still pin the rest of all three units. The tmpfiles text is
pinned independently in tests/test_custody.py.

- §3 storage/shim: /run/mothergate-control is root:root 0755; incoming is
  mothergate:mothergate 0700; sealed is root:root 0700. Lifecycle exclusively
  creates sealed/RUN_ID root:root 0700 and root-owned used/pending markers.
  Protected ancestry, modes, ownership and ACLs are checked without following
  directory links. A bounded root lock serializes both verbs. Only the two
  incoming leaf entries are renamed with directory descriptors; a supervisor
  directory is never moved. After relocation both leaves must be real sockets,
  mothergate-owned, mode 0700 (service UMask=0077), without ACLs or hard links.
  Only then may the fixed bounded blocking start execute. Used IDs cannot recur
  within this runtime. Any root pending marker blocks another start.
- §4 capture: supervisor binds incoming/RUN_ID.{log,status}.sock, retains both
  listener descriptors and collects through them after handoff. Root custody
  protects sealed/RUN_ID/{log,status}.sock. Supervisor closes descriptors but
  never unlinks sealed entries. Capture protocol and init.py remain unchanged.
- §5 units/provisioning: mothergate.service has one common writable mount at
  /run/mothergate-control and no RuntimeDirectory ownership directive. Both
  payload file: outputs name sealed/%i/{log,status}.sock. The fixed root-owned
  policy/mothergate-control.conf must be installed as tmpfiles configuration
  and applied before service start, including boot-time recreation. Installer
  composition, installed identity coverage and host provisioning belong to
  #115; this slice contains no installer and runs no host commands.
- §7 B2/B4: start is blocking with a 40-second client bound against the existing
  30-second unit start bound. Lock acquisition is bounded to five seconds.
  Stop is bounded to 25 seconds; manager show to five seconds. The outer shim
  caller allows 90 seconds. Timeout never proves cancellation. Before cleanup,
  stop the exact instance, prove exactly empty Job=, inactive/failed state and no populated
  exact cgroup. Missing or ambiguous evidence retains custody and blocks.
  Normal cleanup validates both leaves and removes only those fixed names,
  the empty root run directory and its pending marker. Never recursively
  delete a quarantined object; unexpected directories require maintenance.
- §8 MP-9: restore the vulnerable file: path, remove post-relocation validation,
  and allow recursive quarantine deletion as separate plants. Each must turn
  its named oracle RED; a pre-existing failure masking it is NOT EXERCISED.
  Restore exact bytes after each plant. Offline fixtures model socket types
  with FIFOs; actual AF_UNIX rename/connect and root/mothergate DAC tests remain
  separately named. A8 must rerun installed with no descriptor leaks.

Pre-registered custody oracles: deterministic pre-relocation replacement with
unchanged canary and no start; post-relocation validation of both leaves;
missing/regular/dangling/directory/mode/owner/ACL/unsafe-ancestor negatives;
duplicate ID; concurrent start/stop; timeout and collector loss across restart;
normal cleanup; active/pending/populated/ambiguous completion retains custody;
quarantine canary survives; supervisor closes descriptors without unlinking.
Installed-only cases explicitly skip with their root/manager prerequisite;
AF_UNIX EPERM is sandbox_blocked, never an installed PASS. Deferred staging,
receipt and bind-input compromise risks under sheet §12 remain disclosed.

## Q12 / #116 review-1 amendment (Owner mission, 2026-11-07)

Q12 supersedes the Q9 no-job sentinel: only an empty Job value proves no job;
any nonempty value, including "0", retains custody. The fixed bounded stop is
issued before layout/lock validation; settle and cleanup still require both.
A failed start cleans only incoming leaves successfully bound by this supervisor,
including partial bind failure. Relocated sealed leaves remain exclusively root
custody. The optional endpoint preflight is legacy offline coverage, never the
production custody check. F-3 negatives isolate directory ownership/GID, endpoint
GID/link count, unsafe/symlinked lock files and disappeared pending custody.
Oracles precede fixes in numbered checkpoint folders. Real custody and its plants
require an outside run with protected TMPDIR (Q11); no ownership-remap modelling.
Q12 F-6 remains #115 installer scope. No installation, host action, Git write,
network, phase advancement or installed acceptance is performed by this mission.

## Q13 installer qualification amendment

C1-SEALED, C2-CANARY and C3-RESTART are mandatory installed custody rows in
qualification and admission. Without installed host root, direct custody tests
skip; qualification rejects a skipped, failed or unavailable proof. Installed
results remain UNVERIFIED until the Owner runs the separately reviewed procedure.
B2 proves denial of tested manager calls and verifies the installed polkit rule
against installed-files.json. Attribution of denial to that rule and precedence
over earlier conflicting policy remain UNVERIFIED. The Owner must close agents,
remove nexus from docker and reboot before install (README H1). This amendment
claims no host action, installed PASS, phase advancement or CI parity.

## Q15 installer correction (supersedes Q13 custody procedure)

C2 is the real installed F5 pre-handoff substitution, runs LAST, and leaves
root quarantine until a mandatory reviewed reboot. Enable-qualified requires
content-checked row/run_id/pre-reboot boot_id evidence, a different current
boot_id, and clean custody. C1 covers the two endpoint files only. C1/C3 call
lifecycle.py directly as root and collect on the host as mothergate; these
proofs do not exercise privilege elevation or the service collector sandbox.
Cleanup is registered before setUp writes, continues after stop errors, proves
settlement before exact-name persistent removal as mothergate, reports errors
and retained paths, and fails qualification on unexpected leftovers. README
specifies failed-proof recovery and the reboot; no generic custody deletion.
Linux oracle socket addresses use an open private parent descriptor and a
short absolute /proc/self/fd name, independent of cwd and TMPDIR length.
The installed FILES allowlist remains exact and unchanged (27 files). The
host-only plant removes lifecycle's post-relocation endpoint check: Owner must
observe installed C2 RED, restore the SHA-256-verified reviewed bytes and reboot.
This mission claims no installed PASS, CI parity or phase advancement.
