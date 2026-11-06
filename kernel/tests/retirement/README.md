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
production source. Cargo metadata is the resolved dependency policy input; manifests
are presence fixtures, never byte-frozen snapshots. Ruling R-6 below is the current
scope and supersedes the earlier rounds' resolution-scanner claims.
There is no live-tree fallback, old-or-new acceptance or missing-file skip.

| Oracle | Activation and current evidence preserved |
|---|---|
| F7 | K05–K21 cumulatively prohibit exact retired roots and alternate file/directory spellings, root declarations, imports and exports. K11/K12/K14/K15/K17/K19 add runtime-type prohibitions. Free-api/orchestration/delegation bans remain at every stage. Current m0_f7_legacy_supervisor_cut.rs remains active throughout K01. |
| Dependency closure (R-6) | production-paths.txt retains 327 source seeds and cargo-manifests.json retains ten required manifest paths as presence minima. Extraction destinations enter at their stage. Exact source/module inventories apply to kernel/ only. The shared desktop tripwire helper runs Cargo metadata locked/offline, default and all features, without a target filter. All reported packages (including implicit members and renamed normal/dev/build/target dependencies) must avoid retired identities and manifest paths under retired roots. Kernel direct declaration names/renames must stay within the unchanged 39-name cap. Cargo/rustc own resolution and post-deletion absence; K01 has no cross-crate source contact scanner. |
| Logging | K11 replaces loop-source obligations with its absence and no replacement execution route. K12 removes actuator source obligations. logging.jsonl freezes the six retained kernel logging macro expressions and contexts; missing, changed or extra kernel sinks fail, including rebinding and appended logging. Non-kernel sinks may grow. Mother/broker/host P0 privacy and isolation scanners retain sole ownership of their privacy policy. |
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
   Kernel source paths/import edges/dependencies/sinks need reviewed registration.
   Under R-6, non-kernel growth requires no K01 source inventory amendment.
   Before deletion, existing Mother isolation/structure guards own cross-crate
   contact policy; after deletion, rustc rejects imports and path includes that
   reach removed code. Cargo metadata enforces the resolved dependency policy
   at registration and every activated stage; synthetic source trees do not
   claim to resolve or compile as deletion builds.

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

Each of the six kernel logging entries pins its containing production function's
literal-preserving text, including parameters, branches and local bindings.
Controls mutate the real kernel audit-retention sink and rebind `e` while leaving
its macro expression unchanged. R-5 supersedes the original 22-entry cross-crate
freeze and Mother main_entry control. Existing non-kernel privacy guards remain
unchanged and required; K01 does not duplicate them. All twelve SourceKind
persisted names have exact assertions,
including DbMemoryStore -> `db_memory_store`.

The four retained Mother review scaffolds remain scanner inputs. R-5 removes
K01's whole-file self-spawn hashes: unrelated changes to these non-kernel files
must pass. Their registered cfg(test) loading contracts remain specific acceptance
predecessor subjects; promoting a helper into production still changes that
contract. K01's process/network/Stop/SQLite/HM effect predicates now apply only
inside kernel/. Their existing component guards are unchanged. Under R-6,
K01 does not scan non-kernel source contacts; rustc and the existing isolation
guards supply that evidence.

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

## Fix round 4 registration (historical; resolution replaced by R-6)

The following records round 4, not the current dependency implementation. R-6
removes the hand-written manifest, path and membership checker and its controls.

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

## Fix round 5 (Ruling R-5; contact scanning superseded by R-6)

The following records round 5. Its cross-crate contact scanner and associated
red plants are removed by R-6; exact kernel registrations remain unchanged.

`test(oracle-fix)` — Old: "freeze repository-wide modules, source files and logs".
New: "freeze kernel retirement subjects; reject retired contacts elsewhere".
R-5 confines exact inventories to kernel/ and specific named predecessor subjects.
No kernel fixture entry, sink expression or context hash was added or rehashed.
The 95 kernel module entries are identical and all six kernel sink rows are
byte-preserved from 55e08245; only the 57 non-kernel module rows and 16 non-kernel sinks are removed.
LIVE_STAGE stays 1.

Complete sweep of K01 registrations and their disposition:

1. Live module inventory and staged module subset: kernel/ only. New non-kernel
   declarations are checked for retired contacts instead of inventory equality.
2. Staged source-path closure: kernel/ only for unknown-file rejection. The
   current scanner still discovers and loads every non-kernel production file;
   unknown non-kernel paths are scanned, not excluded. Original seed existence
   requirements remain, as required by the preceding dependency ruling.
3. Live and staged logging inventories/context hashes: kernel/ only. The real
   sink/rebinding control now targets audit/retention.rs. Typed-code diagnostics
   and new unrelated non-kernel sinks pass without updating fixtures.
4. Four non-kernel self-spawn whole-file hashes: removed, with the duplicate
   broad process/network/inference/Stop/SQLite/HM predicates scoped to kernel/.
   Scaffold comments pass; renamed retired imports fail. The prior desktop Stop
   plant now exercises the same prohibited claim inside kernel/. Existing P0
   privacy, Stop, Mother isolation and other component guards are untouched.
5. Broker classifier: retained as the named PlannedAction/F3 classification
   subject, bounded to `classify` only. Appending unrelated helpers passes; the
   unchanged digest still rejects tag/refusal mutations. No snapshot rehash.
6. Acceptance successor: 159 named predecessor tests and eight loading contracts
   remain exact (155 kernel tests, four persistence HM guard tests). They protect
   the specific predecessor subjects, not all functions/modules in their files;
   unrelated additions pass. Missing declarations, changed cfg/ignore/loading
   contracts and source promotion still fail.
7. Cargo manifests: already use shared absence/dependency policy, with a narrowing
   39-name kernel dependency cap. Ten required manifest paths and 327 source
   seeds are presence minima, not an equality against the evolving repository.
   No count of newly discovered non-kernel files or members is frozen.
8. Retirement roots/declarations/reexports/items, extraction anchors, KEEP/counts,
   HM rows, codec/default/error/SourceKind goldens, and the frozen direct F3 guard:
   all subjects are in kernel/. Their existing inventories and bytes stay exact.
   The shared HM scanner's own regression tests remain unchanged.

The shared live/staged contact check rejects retired declarations, direct/grouped
imports/reexports, renamed module and kernel-namespace aliases, `extern crate`
aliases, root re-export symbols, retired crate spellings, ordinary/raw-string
`#[path]`, conditional path attributes and `include!` paths. Local non-kernel
modules may reuse a name without reaching the retired subject. Flat generated
`OUT_DIR` includes are recognized structurally outside kernel/, without a filename
inventory. Paths are lexically resolved through traversal and absolute checkout
paths,
including nonexistent retired targets. Known retired runtime symbols and original
F7 prohibitions still apply, including a renamed wrapper around a legacy runtime.
Static source checks do not prove arbitrary semantic equivalence; existing resolved
graph and behavioral guards remain required.

New controls exercise all K02–K22 stages with fresh scanner reachability: an
unrelated non-kernel module/new file, local reused module name, generated include
and typed-code logs pass; retired contacts through declarations, aliases and path
overrides fail at the exact planted
subject; new kernel module/file/sink growth fails. Existing omission, source
promotion, F3, F7, HM, logging rebinding and dependency controls remain active.
No production code, dependency, existing guard or trust boundary changes.


## Fix round 6 (Ruling R-6)

`test(oracle-fix)` — Old: "re-implement Rust/Cargo resolution in text scanners".
New: "use rustc/Cargo as authoritative absence oracles; apply dependency policy
to Cargo's resolved graph". Architect Ruling R-6 (2026-11-05) is the Deep Diagnostic
outcome for the repeated completeness findings in rounds 1–5.

`dependencies.rs` reuses `desktop_tripwire::resolve` and its default/all-feature
configurations: `cargo metadata --locked --offline --format-version 1`, with no
host target filter. Unavailable Cargo, timeout, nonzero status, unreadable output
or malformed/inconsistent graph fails closed. Every reported package is checked,
including implicit workspace members and transitive dependencies, irrespective
of edge name, kind or target. The six merged retired package identities and all
registered retired roots remain forbidden. Manifest paths use component checks,
including Cargo's resolved spelling and canonical spelling when available.
The kernel's metadata declaration names (using `rename` when present), including
normal/dev/build/target/optional declarations, must be a subset of the unchanged
39-name fixture. No TOML, workspace-glob or dependency-path resolver remains.
The live gate runs at K01 and at every explicitly activated successor stage.

Controls use synthetic Cargo metadata OUTPUT, not synthetic manifests that
pretend to implement Cargo. Exact findings prove retired identities are red via
an implicit helper member and renamed normal/dev/build dependencies with and
without a target, as well as when the retired package itself is a member.
An innocent identity under every retired root is red; component neighbors pass.
New kernel names and renames fail across kinds/targets; shrinkage and unrelated
host declarations pass. Missing/malformed/inconsistent graphs fail, and the
actual locked/offline graph passes. Source fixtures load Cargo's actual member
manifests (including implicit members), plus the original ten required paths as
presence minima. The same synthetic implicit-member control checks that fixture
membership includes the helper, without parsing TOML. Dependency controls no
longer mutate unresolved in-memory TOML and claim Cargo equivalence.

Removed: K01's cross-crate alias discovery, namespace tracking, `#[path]` and
`include!` contact resolver, live non-kernel contact registration, associated red
plants, and the hand-written Cargo manifest/membership/dependency checker.
Mother's existing isolation scanner, desktop tripwire and structure guards
remain unchanged. Once a reviewed K-PR removes a module/crate, rustc/Cargo must
reject an import, path include or dependency reaching the missing subject.
K01 does not claim static completeness for Rust/Cargo resolution.

Kept unchanged: retirement manifest/validation, KEEP and extraction obligations,
compatibility/default/error/SourceKind goldens, all 95 kernel module rows, all six
logging rows, staged kernel path/declaration/re-export absence, original F7
prohibitions until each subject disappears, F3 error/refusal and logging
successors, and their source mutation/omission controls. Removing calls to the
deleted contact scanner does not replace these kernel inventories. The same
159 named acceptance tests/loading contracts, HM rules and source-promotion
controls remain. LIVE_STAGE stays 1; no production code, manifest, lockfile,
existing guard, phase or trust boundary changes.


## K02 registration (LIVE_STAGE = 2)

`test(retirement)` — K02 extracts `AgentId` to `kernel/src/agent_identity.rs`
and `FsPermissionLevel` to `kernel/src/workspace_permissions.rs`, each with the
registered same-type transitional export left at `supervisor.rs` / `manifest.rs`.
`LIVE_STAGE` advances from 1 to 2, so the live loader now runs the staged
successor against the real tree, and the stage-1-only inventories (module
edges, logging sinks) give way to their staged equivalents. The history above
describes K01 and is kept as written.

Derived, with no fixture edit: the two destinations enter through `paths_at(2)`,
the `lib.rs` module edges for them are derived from the manifest row, and
`manifest.jsonl` and `module-edges.json` are unchanged. Types and goldens keep
their assertions; only their import lines follow the types to the new homes.

One reviewed registration: `production-paths.txt` grows from 327 to 330 seeds,
and the seed-count assertion with it. Activating the live successor was the
first time K01's inventory met the real tree, and the production scanner already
loads three one-line stub crate sources of the shared desktop-tripwire fixture
(`kernel/tests/desktop_tripwire/fixture/{member,webkit6,wrapper}/src/lib.rs`, each
`//! G0 fixture stub: no code.`). K01's synthetic trees never contained them, so
the live check reported exactly those three as unregistered kernel files
(`f7-dependency`). They are registered by name, not by pattern; no seed was
removed, no check relaxed, and a fourth unregistered kernel file still fails.

`kernel/tests/k02_extraction_parity.rs` is the pre-registered K02 parity oracle.
Its two `transitional_*_export_is_the_same_type_as_its_new_home` tests name the
old paths on purpose and are deleted with them: `supervisor` at K14,
`manifest` at K19. K22 still requires no old-path export or import anywhere.

## K03 oracle registration (before extraction; LIVE_STAGE = 2)

`test(oracle-add)` — `kernel/tests/k03_extraction_parity.rs` adds independent
Rust constructions for all 46 planned variants against K01's unchanged JSON
bytes and labels, including every temporary `required_capabilities` result.
The existing K01 goldens independently cover all six browser variants. Both
binaries run before the move through the old paths and afterwards through the
registered new homes; only their import lines follow the definitions.

Additional K03 assertions pin all six `string_or_vec` consumers (including
missing, null, scalar, object and nested-array inputs), strict vectors outside
that codec, optional-field defaults, malformed-action refusal and each of the
four screen-region fields at and beyond the u32 boundary. The current Debug
output and derives remain unchanged. No fixture is regenerated or amended.
The same-type old-path checks retire with cognitive at K11 and computer_control
at K13; K22 still forbids old-path imports/exports. The capability parity check
must retire with the temporary helper at K12.

## K03 F3 import formatting registration (before extraction)

`test(oracle-amend)` — K01's staged F3 source expectation replaced the type
path in place. Rustfmt moves the new `action_types` import ahead of `actuators`;
leaving it at the old `cognitive` position cannot pass both exact source equality
and fmt. Before extraction, `web_test_source(3..)` now registers exactly that
path replacement and import relocation. The new reversal control reconstructs
K01's frozen snapshot byte for byte and rejects the old-position rendering.
The snapshot itself, every test body/assertion, the classifier hash, all JSON
fixtures and all inventories remain unchanged. No old-or-new acceptance is added.

## K03 extraction registration (LIVE_STAGE = 3)

`refactor` — move the complete `BrowserAction`, `PlannedAction`,
`string_or_vec::deserialize`, `action_type` and temporary `required_capabilities`
payloads unchanged from `cognitive/types.rs` to `action_types.rs`. Move
`ScreenRegion` unchanged from `computer_control.rs` to `screen_region.rs`.
Keep exactly the three registered same-type exports at the old homes. All
existing inline behavioral tests remain present; no Debug redaction is included.
Kernel callers, the broker classifier and its typed test fixtures import the
same definitions at their new homes. The cognitive export group is unchanged.

No fixture inventory amendment is needed: `paths_at(3)` and the staged module
edges derive both new homes and the codec module from the unchanged manifest.
`production-paths.txt` stays at 330 seeds; all JSONL bytes/hashes, module edges,
logging/HM/retained-test inventories and classifier bytes stay unchanged. The
only oracle amendment is the pre-registered F3 import formatting above. Stage 3
activates the registered live checks; it authorizes no later retirement step.


## K04 oracle registration (before extraction; LIVE_STAGE = 3)

`test(oracle-add)` — `kernel/tests/k04_extraction_parity.rs` pins all five
registered compatibility types before extraction. Independent constructions
freeze nondefault JSON/TOML and Debug bytes; unchanged K01 goldens freeze
schema defaults. Every individually missing field, explicit false/zero/empty
value, unknown field, invalid scalar and numeric boundary keeps its behavior.
Every state/violation name and variant index, state Display, transition error,
fuel error and remaining AgentError text/strategy is pinned. Only imports may
follow the moved definitions; assertions and existing fixtures stay unchanged.

HOME cases run in isolated children, including absent HOME on non-Windows and
non-UTF-8 HOME on Unix. They check both the lossy schema string and the retained
archive config's lossless PathBuf. Plaintext config loading must not migrate;
explicit encrypted save/read preserves all three schemas, and a wrong key must
fail without rewriting. Existing encrypted-config and vault-migration tests
remain required. Transitional same-type checks retire with fuel/lifecycle at
K16, backup at K20 and rate_limit at K21; K22 still forbids old paths.

Pre-extraction stage-4 activation was RED: the manifest payload-home,
real-declaration and stage-seed checks failed for the missing new homes; the
live successor reported exactly config/schema_compat.rs and errors/compat.rs
as omissions. Stage 3 was restored before this oracle was added.


## K04 extraction registration (LIVE_STAGE = 4)

`refactor` — move FuelViolation and AgentState/Display unchanged to
errors/compat.rs, and BackupScheduleConfig, RateLimitConfig, ApiHardeningConfig
and every registered default helper unchanged to config/schema_compat.rs.
Keep exactly the five registered same-type old-home exports. Config, errors,
retained kernel runtime callers and typed tests use the new homes; there are
no outside-kernel import changes. Existing behavioral tests stay present.

The private default_backup_dir helper also served BackupConfig::default.
That one retained runtime caller inlines its identical validated-home
expression, preserving the lossless PathBuf even for non-UTF-8 HOME. The moved
helper stays private and byte-identical; no helper export or string roundtrip
is added. No config migration, behavior, Debug or serde change is introduced.

For the live K04 tree, paths_at(4) and the staged module edges derive both
nested homes and their parent declarations from the unchanged manifest. All
JSONL bytes/hashes, production seeds, module/logging/HM/retained-test and
dependency inventories remain unchanged. Stage 4 activates only this approved
extraction; later retirement and authoritative integration remain separate.


K04 mutation receipts: backup serde retention default, rate default, API depth
default, AgentState Display, FuelViolation serde name and the backup transitional
export were each planted separately. All six failed by the named assertion
(no compile-error substitute), then their source bytes were restored and the
same named test passed. The pre-extraction stage-4 RED probe remains separate.


K04 verification is BLOCKED. The required full kernel command reached the
successor binary, then stopped with 1845 passed, 11 failed and 15 ignored.
All 11 failures report the same historical K02 synthetic baseline finding:
`f7-dependency` at kernel/src/config.rs. build_clean_tree copies today's config
source, including the new schema_compat child, into synthetic K02/K03; their
module inventory correctly adds that child only at K04. The live K04 gate,
manifest, K02/K03/K04 parity and goldens pass; fmt and workspace Clippy pass.
No check or fixture was changed after observing the failure. Historical
fixture projection needs Architect review and separate oracle registration;
this candidate does not satisfy the zero-failure exit condition.

The separate no-fail-fast diagnostic ran all 23 binaries not reached after
that failure: 201 passed, 0 failed, 13 ignored. Combined candidate inventory:
2046 passed, 11 failed, 28 ignored; this is not a green single-command result.


## K04 R-8 historical projection registration (LIVE_STAGE = 4)

`test(oracle-fix)` — Architect Ruling R-8. The K04 module edge
`kernel/src/config.rs -> schema_compat` stays registered with stage 4 only, by
the existing manifest-derived `build_module_edges`; `module-edges.json` and
every other inventory are unchanged. The defect was in `build_clean_tree`: it
copied today's `config.rs` (which declares the K04 child) into synthetic K02 and
K03, whose stage-correct inventories do not contain that edge.

Old: `let mut raw = read(&sink.path);`
New: `let mut raw = project_live_source(stage, &sink.path, read(&sink.path));`

`project_live_source` is derived from the manifest. For each extraction row
later than the replayed stage and no later than LIVE_STAGE whose nested
destination's parent is the copied file, it removes exactly the one registered
`pub mod <child>;` declaration and reverts that row's identifier payload paths
to their registered old home. A duplicate declaration or any residual child
edge panics. For stage 3 the projected `config.rs` equals the pre-K04 bytes
(bd883593) exactly. The K01 forward rewrite for stage >= 4 is kept unchanged.

Nothing else is projected. The new R-8 control proves at every stage 2..=22
that an unregistered `pub mod` in the projected parent, an unregistered file
under `kernel/src/config/`, and the K04 child name declared in another parent
are each `f7-dependency`; at K02/K03 the K04 declaration itself is still RED.
A live plant (`kernel/src/config/planted_unregistered.rs` plus its declaration)
was RED and then restored byte for byte; it was never committed. The completed
K04 BLOCKED note above is historical and superseded by this registration.
