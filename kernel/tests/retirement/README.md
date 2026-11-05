# K01 retirement oracle registration

Approved plan: Owner decision 2026-11-05, kernel-legacy-plan.md §§3–6, 8–10.
Baseline: 20315a56b4195f5373b83fdfd3eb553c9ffc9d5c. Test assets only.
No extraction, removal, runtime change, phase advancement or acceptance occurs here.

`manifest.jsonl` has one row for every K02–K22. `roots` names all 34 physical
candidate entries; `paths` expands them to the exact 76 current Rust files.
`declarations` names the root declarations; `reexports` freezes every cognitive
export group; `items` isolates the HM test adapters inside a KEEP file.
`extract` names every payload, helper, destination and temporary same-type export.
The temporary capability helper expires at K12 (its last caller is the actuator
registry); the K22 row closes all remaining old imports/exports. It
never authorizes deletion of the new homes. K20 still requires archive-reader
obligation disposition and leaves C-11 OPEN.

`keep.txt` is the complete 21-entry KEEP boundary. The live manifest test checks
existence, directory composition, declarations, export groups, extraction anchors,
non-overlap, duplicates and fixed inventory sizes. Destination paths and proposed
transitional exports are future declarations, deliberately not claimed to exist.
The source anchors do exist now. Permission ambiguity does not authorize a move.

The JSONL goldens freeze compact serde bytes, field order, labels, Unicode/newline
escaping, nulls, all 46 actions and all six browser tags. `bytes.jsonl` also pins
UUID bytes and large screen coordinates. These bytes are the hash preimages;
fixture SHA256 values witness bytes, not a new production hash domain. The UUID
bytes feed the existing WorkspaceBinding constructor. Multiple nonempty API
headers have no stable HashMap iteration order; the fixtures use null, and the
broker's independently frozen classifier still sorts actual headers.
Backup HOME is substituted only at the JSON string boundary; the suffix and all
other bytes are fixed. Child processes test valid, empty and relative HOME without
racing other tests' environment. No actual archive, database or secret is opened.
Error tests freeze every AgentState/FuelViolation serde name, Display text, all
AgentError texts (both AuditError causes) and on_error strategies.

## Staged guard disposition

Every current guard remains active and unchanged. `LIVE_STAGE = 1` is explicit.
The successor test driver applies K02–K22 policies to synthetic in-memory source
trees only. Synthetic empty module slots are effect-free fixtures, never purported
production source. Cargo manifests are public policy inputs, never byte-frozen snapshots.
There is no live-tree fallback, old-or-new acceptance or missing-file skip.

| Oracle | Activation and current evidence preserved |
|---|---|
| F7 | K05–K21 cumulatively prohibit exact retired roots and alternate file/directory spellings, root declarations, imports and exports. K11/K12/K14/K15/K17/K19 add runtime-type prohibitions. Free-api/orchestration/delegation bans remain at every stage. Current m0_f7_legacy_supervisor_cut.rs remains active throughout K01. |
| Dependency closure | production-paths.txt fixes 327 source seeds; manifest files and extraction homes enter/leave with the selected stage. Every expected seed must be present; unknown renamed modules/files and Cargo dependency aliases/wrappers fail. Extraction destinations enter only at their stage. cargo-manifests.json retains the ten required manifest paths, while manifests selected by current workspace membership are also checked. Retired package identities (including aliases and workspace declarations) and paths under retirement roots are forbidden in every dependency kind and target. kernel-dependency-names.json caps the kernel dependency names at the 39-name base set; later stages may shrink it. Unrelated non-kernel dependency changes do not require fixture edits. |
| Logging | K11 replaces loop-source obligations with its absence and no replacement execution route. K12 removes actuator source obligations. logging.jsonl freezes 22 retained logging macro expressions; missing, changed or extra sinks fail, including raw result/error aliases and appended logging. Mother/broker live privacy oracles remain required. |
| Web search | Through K11, preserve F3's source prohibition plus the independently runnable kernel_retirement_web_search binary's direct default/backend/registry refusal, no-connection and injected-backend checks. K03 moves its PlannedAction import to action_types; its complete source is pinned by web-search-guard.rs.txt. K11 retires only the executor checks with their subject. K12 retires this actuator-dependent binary and switches to actuator/registry/backend absence, including renamed search modules/process sinks; no curl exception remains. broker-classifier.sha256 freezes the exhaustive classifier and refusal vocabulary independently of imports. WebSearch stays inert. Existing broker egress, credentials and proxy tests remain required. |
| HM prompt/writers/separation | K11 removes exactly six prompt-content exceptions, three AgentMemoryManager writers and the world-model separation row. hm-rows.jsonl names sites/routes; the full 26-route no-handoff vocabulary and raw SQL/write/content-read plants survive. |
| HM model/transport | K11 removes planner/world-model classifications; K12 removes image/tts errata; K13 removes vision/model-list classifications and both transport exceptions. Broker speech remains a separate governed route. Existing unknown-model, AR/XS/LATENT, five-path transport and A2 tests stay active. |
| HM SQLite | K06 drops only the wrapper opener; K11 drops cognitive identity/open rows; K12 drops agent-lifecycle and knowledge-graph rows. Final identity resolver is identity_home.rs. Three raw-open files remain classified. Storage/FK/admission/label-taking/issuer/confirmer/capture/XS oracles are retained without new exceptions. |
| Restore/Stop/desktop | K17 adds TimeMachine absence. Keep actual coding-run restore/refusal/preimage/grant tests, the unchanged six-claimant Stop guard, controller compile-fail proofs, shared desktop resolved-graph tripwire, 13 callsites and process cleanup helper. K01's split Stop plant spelling does not widen the existing claimant list. |
| Acceptance POLICY successor | retained-tests.txt pre-registers 159 exact named security tests, including both acceptance audit assertions. Every named omission is a red synthetic control. It supplements today's ≥400/≥60 checks; K01 changes neither threshold. Later activation requires independent review and splitting acceptance audit tests before retiring unrelated subjects. This inventory does not authorize deleting any unlisted test. |

## Later PR use and review

1. Read the row and all dependent rows before editing. Check fresh authoritative
   base, #83 composition and the old guard inventory; K01 is no new authorization.
2. Keep type imports/goldens compiling against the same definitions as they move.
   Never regenerate fixtures from changed output to obtain green.
3. At each approved deletion advance LIVE_STAGE to activate its staged absence
   checks through load_live_tree, retaining the named existing behavioral oracles.
   The live loader's metadata walk fails on unreadable inputs and source symlinks
   and records retired empty/non-Rust paths. Every production file is loaded
   directly from the existing HM scanner's inventory, including nested docs;
   only workspace-root docs is excluded. Preserve the existing HM
   production scanner's intersection/reachability rules when wiring inventory
   comparisons; do not replace its model/authority/data-flow predicates with a
   weaker text predicate. The registered HM row sets specify only narrowing.
4. Run the synthetic controls plus actual live-tree checks at that candidate.
   New source paths/import edges/dependencies/sinks need explicit reviewed
   registration, not a permissive directory-prefix exception.

Owner-approved F7 policy: **absent and cannot return**, replacing “retained but
isolated” only as each removal lands. This policy successor needs independent
fresh-context security review before activation. Static source predicates do not
prove arbitrary semantic equivalence or data flow; existing runtime, resolved-
graph, containment, grant, privacy, refusal and tamper tests remain indispensable.
No GitHub/CI or installed physical capability is claimed by this registration.

## Fix round 1 registration

The exclusions now carry the current HM scanner's intersected proof, not the
baseline seed inventory. A changed in-memory tree invalidates its exclusions;
uncertainty scans. Disk-backed promotion controls remove `cfg(test)` from the
real workspace-authority test loading declaration and plant production effects
in its real target. Registered loading edges pin their targets and attributes.

F7 preserves the original twelve reference prohibitions on each surviving
cognitive/actuator subject before applying any legacy exemption. Its original
checkpoint exception remains local to computer_use. All six historically absent
roots (including the three removed actuators), their alternate spellings and
empty directories stay absent. Guard declarations and consent/autonomy/supervisor
contacts survive until each subject's deletion. The original F7 reference plants
run at every stage: the reference is forbidden while its subject exists, and the
subject is forbidden afterwards.

The no-handoff successor reads both masked code and literal-preserving text,
including SQL SELECTs. The original world-model rule applies before K11; the
post-K11 retained-producer rule keeps both views. The retained verifier-sandbox
launcher is an explicit process boundary. Complete module components distinguish
`workspace` from `workspace_authority` and `workspace_permissions`.

Every green stage now includes representative actual retained source: the
verifier launcher, workspace authority, governed paths, coding-run apply, broker
Files, mixed audit code/tests and every registered logging source. Only the
preregistered AgentId, workspace permission and configuration schema contacts
move to their same-type homes at their extraction stages in these fixtures. These are source-policy fixtures, not evidence of a compiling
future deletion, runtime data flow or readiness to advance LIVE_STAGE.

`retained-test-policy.json` pins all 159 real function declarations, their file
and inline-module attributes, platform cfgs and baseline ignores. The eight
external loading edges are pinned in `retained-test-edges.json`. Commenting out,
wrapping in a disabled scope, changing cfg, removing an edge or adding an ignore
fails. The existing test_support cfg remains exact. The narrow declaration
reader ignores comments, literal contents and unexpanded macro bodies; unknown
changes require reviewed registration, not a guessed cfg evaluation.

Each of the 22 logging entries also pins its containing production function's
literal-preserving text, including parameters, branches and local bindings.
Controls mutate Mother's real main_entry sink and rebind `e` while leaving its
macro expression unchanged. This protects that bounded diagnostic context; it
is not an interprocedural privacy proof. Existing behavioral privacy guards
remain required. All twelve SourceKind persisted names have exact assertions,
including DbMemoryStore -> `db_memory_store`.

The four retained Mother review scaffolds stay scanned under R-4. Their private
self-spawn helpers are registered by exact whole-file hashes in
`retained-test-process-files.json`, with their current cfg(test) loading edges
pinned separately. This permits their existing test executable only; any source
change or additional process call loses that registration. These are not new
production process boundaries or file exclusions. Source-policy registration
still does not prove arbitrary caller/data-flow equivalence.

## Fix round 2 registration

Negative controls recompute reachability from each mutated fixture and require
a new finding for the exact planted subject and oracle relative to its clean
stage baseline. Absence findings name restored files/directories themselves.
The missing-Cargo-manifest control requires the scanner's exact refusal to
establish reachability, invalidated exclusions and a separate omission finding
at that manifest; a scanner panic alone cannot supply a red receipt.
Harmless comments in production, mixed audit and test-only source remain green
at every K02–K22 stage. A disk-backed privacy.rs control preserves its existing
test module and adds a production `docs/runner.rs` through the already registered
name `tests`; both dependency and process findings must name that nested file.

The verbatim F3 source predicate is shared by the new direct test binary and
the staged successor before the legacy exemption. The predecessor's six search
mutations are red through K11, then require source absence at K12. Missing the
direct binary or any of its three tests is red before K12. The frozen text
snapshot remains available after the live binary retires; it permits only the
prescribed K03 import move. Synthetic pre-K12 source shapes do not depend on a
deleted live actuator. This registers the K11 split; it does not implement K11.

Both manifest extraction tests use the same stage-appropriate anchor check.
Synthetic K02 homes with same-type re-exports pass, as do those homes after
their original sources disappear; an empty new home fails despite an old
definition. BrowserAction fixtures are paired with independent Rust variants,
an exhaustive variant/field match and independent serialization to frozen bytes.
A swapped Click/ExtractText codec retains byte round-trips but fails that same
semantic golden. No runtime, existing guard or LIVE_STAGE change is made here.

## Fix round 3 registration

The independent direct F3 guard now also requires a failing injected backend
("backend down") to return its original error. `expect_err` rejects successful
empty fallback; separate assertions require the cause and reject a false
"web search is closed" refusal. The frozen snapshot pins all four tests.
The existing stage controls now omit the new test and each error assertion
individually and require `f3-coverage` at the direct guard through K11.

`python3 kernel/tests/retirement/web-search-mutation.py --cargo-slot PATH` is
the registered behavioral mutation control (run with explicit CARGO_TARGET_DIR
and TMPDIR). It copies tracked files
to a disposable scratch tree, stages the K03 same-type PlannedAction import used
at K11, and runs the actual independent guard. Clean and byte-restored baselines
must pass all four tests. Replacing the real actuator's error propagation with
`.unwrap_or_default()` must fail the named failing-backend test; changing the
error into a false closed refusal must fail that same test. Compile failures or
unrelated test failures cannot supply the required red receipts. Scratch source
is restored byte-for-byte in a finally block; worktree source is never mutated.
This checks the registered K11 split, not a compiling K11 production deletion.

A line-by-line comparison with m0_f3_web_search_guard.rs found no other missing
behavioral assertion: source/process prohibitions and six plants, default honest
refusal without an operator connection, and successful injected search are
already carried. The predecessor's registry/executor default-refusal and executor
injection assertions remain active with their subjects; executor checks retire
with the K11 executor, while independent direct checks survive until K12.

## Fix round 4 registration

`test(oracle-fix)` — Old: "freeze the exact bytes of ten Cargo manifests".
New: "enforce retirement dependency policy and a narrowing kernel name set".
Architect ruling: no retained manifest (root or workspace member) may depend on
retired crates or paths under registered retirement roots; kernel dependency
names across all kinds may stay equal or shrink from the base fixture; seed-path
existence checks stay unchanged. No production manifest or lockfile changes.

Both the live manifest check and the staged successor use dependencies.rs.
Normal, dev, build, inactive target, renamed `package =`, path and workspace
declarations are checked without resolving or downloading dependencies. The
six already-retired crate identities/roots are explicit; all K02–K22 roots come
from manifest.jsonl and are forbidden dependency destinations from registration
onwards. Paths are resolved relative to each manifest (including traversal and
absolute paths within the checkout), using component boundaries. The live
inventory rejects symlink aliases. Malformed or missing workspace metadata is
red. Current workspace members, including newly introduced members, cannot evade
the required-path fixture. New unrelated non-kernel dependencies remain green.

The kernel name fixture was taken from d4b205c750b291bcc2f10e9e5665cbd46ff12ee0,
including every normal/dev/build/target dependency key, with duplicates removed.
It contains 39 names. All stages accept a subset; a new name requires an explicit
fixture amendment in review. This does not authorize dependency additions.

Controls plant retired packages and aliases in every retained manifest and every
kind/target, workspace aliases at the root, retired/traversing/absolute paths,
new workspace members and malformed/missing metadata. Positive controls retain
#83's nix/uuid content, add an unrelated host dependency, accept a neighboring
non-retired path and remove a kernel dependency. New kernel names fail in every
kind. The staged successor controls exercise legitimate host changes, retired
aliases and kernel additions at every K02–K22 stage with fresh reachability.

Audit of other K01 freezes: the staged successor duplicated the manifest-byte
freeze and is fixed here. The kernel gate also exposed two broader inventory
freezes after #83: registered_module_edges_name_todays_exact_declarations rejects
additions in eight non-kernel sources, and
registered_logging_sinks_match_current_production_views rejects three new host
startup diagnostics: `eprintln!("{code}")`, `eprintln!("{}", e.code())` in hands.rs,
and UnsupportedPlatform.code() in bin/mother-hands.rs.
The old registered logging-context hashes still match. Related staged source
and logging predicates deliberately reject unknown modules and new/rebound
sinks, with explicit negative controls. They are preserved pending an Architect
ruling on the replacement module/logging policy; no inventory was rehashed or
widened just to obtain green. This round is BLOCKED on those remaining gates.

Codec byte preimages, the F3 guard snapshot, broker classifier/refusal vocabulary,
bounded logging context and four self-spawn scaffolds are deliberate
security/behavior registrations with mutation controls. No other byte drift of
registered files was found. LIVE_STAGE remains 1; no existing guard or trust
boundary is changed. No phase advancement or integration is authorized here.
