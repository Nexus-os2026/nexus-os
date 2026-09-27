# Phase Zero C5 — Authority Inventory and Reachability Closure

Status: P0-002C5B (governed recovery and the remaining straightforward
filesystem migrations), on top of P0-002C5A (fail-closed reachable surface
closure). C5 is split into bounded checkpoints:

| Checkpoint | Scope |
|---|---|
| **P0-002C5A** | Make every reachable E1–E5 surface fail closed unless an approved backend-owned authority mechanism already governs it; fix the computer-use EOF approval bug; stabilise the Darwin sealed-spawn fixture; record this inventory and the reachability guard. |
| **P0-002C5B** | Governed recovery and the remaining straightforward filesystem migrations: every reachable surface deferred by C5A is migrated onto an existing Phase Zero primitive or fails closed (§5). |
| P0-002C5C | Final authority inventory and guard closure. |

The invariant C5 serves: **a path is not authority.** No security-sensitive
filesystem or process operation may obtain authority merely from a path or
program string controlled by the frontend, a model, an agent, a serialized
record, the process working directory, `HOME`, the environment or another
ambient source.

Permanent Phase Zero rule: *a working unsafe feature is worse than an
unavailable safe feature.* Where a safe implementation would need a new OS
sandbox, durable authority persistence, a native user-selection broker, a new
process-supervision architecture, a new secret vault or a network sandbox,
Phase Zero fails closed instead.

## 1. Audit methodology

The audit was read-only, at authoritative base `889f1e52` (tree `a076f99a`).

- **Scope.**
  - All 70 workspace members.
  - The 56-crate dependency closure of the desktop backend (`app/src-tauri`), with 942 production sites.
  - The 14 members outside that closure (CLI, benchmarks, developer tools, the standalone server), with 114 sites. None is reachable from desktop IPC.
- **Excluded as non-production:** tests, benches, examples, fixtures, build scripts and every `#[cfg(test)]` item.
  - A lexer-aware scanner stripped test items while skipping comments, strings, raw strings and char literals.
  - Every stripped span was checked to end at the brace matching its attribute.
  - Test files not named by convention were confirmed to be `#[cfg(test)]`-gated.
- **Patterns scanned:**
  - filesystem mutation and reads;
  - process creation (`Command::new`, `current_dir`);
  - ambient roots: process cwd, `HOME`/`USERPROFILE`/XDG/temp environment, `temp_dir`, `dirs::*`, `current_exe`.
- **Blind-spot sweep** for writers a call-pattern scan misses: SQLite opens, `open::that`, zip, image, `unpack` and tempfile. It added 24 sites.
- **Classification.**
  - Every site was traced to its IPC command, agent path or startup caller.
  - Each site received exactly one family and class:
    - **A** — governed by an approved Phase Zero mechanism;
    - **B** — fixed internal path or fixed program;
    - **C** — user-selected path;
    - **D** — an untrusted path, identifier or working directory acts as authority (*latent* when nothing in production calls it);
    - **E** — needs an architectural decision.
  - Every E claim and every high-risk D claim was re-verified in source.

Desktop-closure totals at the audit base (966 sites):

| Class | Sites |
|---|---:|
| A | 30 |
| B | 182 |
| C | 89 |
| D, reachable | 484 |
| D, latent | 164 |
| E | 14 sites, plus 5 cross-cutting boundaries |
| test-support only | 3 |

These are audit-base totals. Neither C5A nor C5B re-ran the class audit, so the
table is not re-derived here. §4 and §5 record the transitions site by site,
and the C5C inventory recounts.

The desktop registered 804 IPC commands. `capabilities/default.json` is core-only and the webview CSP is `null`, so every registered command is callable by any script running in the webview. A frontend value is therefore never evidence of user intent.

## 2. Families

| Family | Meaning | P0-002C5A disposition | P0-002C5B disposition |
|---|---|---|---|
| A-BUILDER | Governed Builder workspace (C1–C4) | Kept; guarded by the C2–C4 suites. | Unchanged. |
| A-KERNEL-PRIM | Kernel primitives: P0-002A resolver, ResourceLimiter, sealed spawn | Kept. | Joined by `governed_path`, `identity_home` and `governed_http` (§5). |
| B-* | Fixed app data, fixed system files, fixed helper programs, network helpers, backend random temp | Kept. | Unchanged. |
| C-USER-FILE | A raw user-selected path | Closed (E1). | Stays closed. |
| C-CLI | CLI and developer binaries (OS user chooses argv/cwd) | Outside the desktop; kept. | Unchanged. |
| D-BUILDER-LEGACY | Raw project id/dir/output dir over the governed storage root | Closed (E2). | Stays closed. |
| D-MODEL-WRITERS | Conductor / coder writers under a raw or model-chosen root | Closed or refused (E2/E4). | Stays closed. |
| D-EXEC | Command text is the process authority | Closed or refused (E3/E4). | Stays closed. |
| D-AGENT-ROOT | Agent root is the process working directory | Closed (E4). | Stays closed. |
| D-AMBIENT-ROOT | cwd / environment / compile-time roots | Closed (E5). | Stays closed. |
| D-TIME-MACHINE | Recorded raw paths replayed by undo/redo | Deferred to C5B; no production producer remains. | Grant-bound; production file replay unavailable (§5.1). |
| D-BACKUP-RESTORE | Archive entries choose restore targets | Command closed (E1); safe restore deferred to C5B. | Library hardened; command stays closed (§5.2). |
| D-ID-JOIN | Identifier joined into a fixed root | Deferred to C5B. | Migrated to grammars, allowlists or digests (§5.3). |
| D-HOMEFALLBACK | Raw `HOME` with a `.`/`/tmp`/`~` fallback | Deferred to C5B. | Migrated to the validated identity home, or unavailable (§5.4). |
| D-SHARED-TEMP | Predictable names in shared temp | Deferred to C5B. | Migrated to private temp or identity-home state (§5.5). |
| D-CURL | curl argument / `file:` / `@file` injection | Deferred to C5B, except the agent path (see §4). | Migrated at every production curl site (§5.6). |
| D-RECORD | A serialized record chooses an authority file | Deferred to C5B. | Refused or bound to live authority (§5.7, §5.8). |
| D-CHECKS | Broken containment checks | All affected commands closed (E1/E2). | Stays closed. |
| E-OS-INPUT, E-KEYS | OS input; ambient key material | See §7. | See §7. |

## 3. Architect decisions (P0-002C5A)

- **E1 — user-selected raw paths.**
  - No native picker or opaque-handle broker is built in Phase Zero, and `WorkspaceAuthoritySource::UserSelected` stays reserved.
  - A raw path from frontend text, HTML file inputs, localStorage, model output, caller JSON or a picker fallback never becomes filesystem authority. Such commands fail closed.
- **E2 — legacy Builder.**
  - The raw-path Builder surface is retired. The governed C1–C4 flow is the authority path.
  - The governed planning response carries only the opaque project selector, never an absolute directory to hand back.
  - Projects without durable authority may be unavailable after a restart, and Builder undo/rollback may be unavailable until C5B.
- **E3 — caller-asserted execution and approval.**
  - Caller assertion is not user approval, and no sandbox is authorised.
  - Surfaces that turn command text into a process are closed.
  - External CLI agents are never used as an automatic fallback, and no permission-bypass path exists.
- **E4 — agent cwd, shell, code and Docker.**
  - The "agents run in the process cwd" model grants nothing.
  - Uncontained shell, code and Docker execution is unavailable, and so is any agent action needing filesystem or process authority.
- **E5 — ambient code-bearing resources.**
  - Such resources may come only from a deterministic Nexus-owned installed location.
  - cwd, parent cwd, `HOME`, arbitrary environment overrides and compile-time checkout paths are not authority.
  - Self-rewrite, and loading genesis or self-improvement code from ambient paths, are unavailable.
- **E6 — computer-use stdin.** EOF aborts; it is never an approval.
- **E7 and network egress.** Recorded for final-gate review (§7) and not changed here.

P0-002C5B works within these decisions and adds none. It builds no user-file
broker, general process sandbox, durable Builder authority, network sandbox,
process lifecycle or secret vault. Each reachable surface it covers either
uses an existing Phase Zero primitive (a live workspace grant, a fixed
backend-owned location under the validated identity home, a grammar, an
allowlist or a digest) or fails closed. No command closed by C5A is reopened.

## 4. P0-002C5A dispositions

### Closed IPC commands (125)

Each stays registered and returns one bounded reason. Its handler takes no input, and its whole body is the denial (`app/src-tauri/src/phase0_surface.rs`).

- **E1, file selection (28):**
  - file_manager_{list,read,write,create_dir,delete,rename};
  - analyze_media_file, index_document, cogfs_index_file, cogfs_watch_directory;
  - db_{connect,execute_query,list_tables,export_table,disconnect};
  - voice_load_whisper_model;
  - airgap_{create,validate,install}_bundle;
  - backup_verify, backup_restore;
  - flash_{profile_model,auto_configure,create_session,estimate_performance,run_benchmark,enable_speculative};
  - factory_create_project.
- **E2, legacy Builder (49):**
  - conduct_build, conduct_build_streaming, read_build_file;
  - builder_list_projects, builder_load_project, builder_delete_project;
  - the legacy checkpoint and iteration commands (read_preview, list_checkpoints, rollback, init_checkpoint, iterate);
  - every raw project-id command for plans, state, archive, export, visual edit, deploy and deploy history, quality, conversion, collaboration, design import, variants, themes, images, trust pack and audit trail.
- **E3, process execution (9):**
  - terminal_execute;
  - factory_build_project, factory_test_project, factory_run_pipeline;
  - cc_execute_action;
  - mcp2_client_{add,discover,call};
  - nx_agent_run.
- **E3, approval required (1):** terminal_execute_approved.
- **E3, external CLI agents (6):**
  - detect_claude_code_cli, detect_codex_cli;
  - trigger_claude_code_login, trigger_codex_cli_login;
  - builder_check_cli_auth, builder_authenticate_cli.
- **E4, agent execution (3):** nx_chat, nx_tool, run_content_pipeline.
- **E5, ambient resources (29):**
  - self_rewrite_{analyze,test_patch,rollback};
  - genesis_{analyze_gap,preview_agent,create_agent,store_pattern,list_generated,delete_agent};
  - get_agent_genome, mutate_agent, breed_agents, get_agent_lineage, generate_all_genomes, evolve_population, force_evolve_agent;
  - trigger_immune_scan, get_git_repo_status;
  - voice_start_listening, voice_pipeline_health, transcribe_push_to_talk;
  - cm_{execute_validation_run,list_validation_runs,get_validation_run,three_way_comparison};
  - memory_{save,load,list_agents};
  - mcp2_server_handle.

### Gated command

`execute_tool` never runs anything, because every typed tool spawns a process in the process working directory. A tool needing approval (destructive or custom) is refused as approval-required. Every other tool is refused for lack of backend authority. The audit records the tool kind only.

### Non-IPC closures

- **Production agent executor** (`Phase0AgentExecutor`, `app/src-tauri/src/commands/cognitive.rs`).
  - It allows only LLM, memory, notification, messaging, HITL-request, web-search, HTTP(S)-fetch and knowledge-graph actions.
  - File, shell, code, Docker, API, image, speech, browser, screen/input, cognitive-parameter and ecosystem actions fail closed. So do non-HTTP fetch URLs and any action variant added later.
  - The kernel registry receives no workspace root.
- **Planner:** it receives neither the process cwd nor a listing of it.
- **Swarm coder:** it refuses a model-chosen `output_dir` before any provider call and returns files inline.
- **Claude Code and Codex CLI providers** (`connectors/llm`):
  - detection reports them unavailable without running anything;
  - every query, stream and login fails closed;
  - no process spawn or permission bypass remains.
- **Desktop swarm:** it registers no Codex provider.
- **Nexus Code bridge** (`nx_bridge`). It configures, diagnoses and builds Nexus Code through the `_without_cli_agents` entry points of `nexus-code`, so the desktop:
  - never runs the Claude CLI, not even `claude --version`;
  - never prefers it during provider auto-detection;
  - never registers it as a provider; `nx_doctor` and `nx_providers` report it unavailable.
  - The standalone `nx` terminal CLI keeps its own behaviour.
- **Process cwd:** it is no longer set from `NEXUS_WORKSPACE_ROOT` or a git ancestor at startup.
- **Prebuilt agent manifests** (authority-bearing):
  - they resolve from no ambient location;
  - production registers none, because the bundle ships no resources;
  - the developer checkout is a `#[cfg(test)]`-only source.
- **Chat:**
  - it reads no agent system prompt from a cwd genome file;
  - auto-evolution neither reads nor writes genome files.
- **Capability measurement:** it reads no cwd-relative battery.
- **`FLASH_MODEL_PATH`:** it no longer chooses a file for the native model loader.
- **Governed Builder:** the planning response no longer returns `project_dir`, and the Builder UI never forwards a location as build authority.

### E6 and reliability debt

- **E6:** computer-use approval treats EOF (`read_line` → `Ok(0)`) and read errors as abort. A real Enter line keeps its explicit approval.
- **Darwin fixture:** the sealed-spawn fixtures now finalize a self-exiting helper only after its observed exit, which is the production discipline. No production termination semantics change.

## 5. P0-002C5B dispositions

New kernel primitives, all pathname and argument checks rather than OS
isolation:

- **`governed_path`**:
  - grammars: `validate_relative`, `validate_component` (the C4 grammar) and `validate_identifier`;
  - `storage_stem`, a deterministic digest for identifiers outside the grammar;
  - `join_relative`, `existing_root` and `regular_file_beneath`, a no-follow walk;
  - `FileIdentity` (device and inode, or volume and 128-bit file id);
  - `private_temp_dir`.
- **`identity_home`**: the validated per-user home and the Nexus state locations beneath it.
- **`governed_http`**: `http_url`, `http_method`, `http_header` and the `CURL_HTTP_ONLY` / `CURL_HTTPS_ONLY` argument prefixes.

Refusals are audited by operation and reason class. The refused value, token,
content and absolute path are never recorded.

### 5.1 Time Machine (D-TIME-MACHINE)

- **Before.** File entries carried raw path strings, and undo, redo and undo_checkpoint wrote, restored or deleted whatever they named. C5A had already closed the only producer, the Conductor.
- **Model.** A file entry is a `GrantedFile`, with unknown fields denied. It holds:
  - a workspace grant id, an agent id and a run id;
  - the grant root's native identity;
  - a relative path in the C4 grammar.
- **Replay** needs a `FileAuthority`: a live `WorkspaceAuthorityRegistry` plus the recording `WorkspaceBinding`. The replay:
  1. requires a ReadWrite grant whose root still exists, is canonical, is not a redirect and has the recorded identity;
  2. derives the target by `join_relative`;
  3. prechecks every change for its identity, its content and no redirect on the way, then applies;
  4. reports a failure after the first write as `PartiallyApplied { applied, total, reason }`.
- **Production.** `undo`, `redo` and `undo_checkpoint` pass no authority, so a checkpoint with file entries is refused (`FileAuthorityRequired`). Agent-state and config entries still replay.
  - The desktop constructs no `FileAuthority`. The guard names `FileAuthority::new(`, `.undo_with(`, `.redo_with(` and `undo_checkpoint_with(`.
  - The Conductor no longer records file entries (guarded).
  - No UI command was added.
- **Tests** (`kernel/src/time_machine/tests.rs`, 34):
  - arbitrary absolute paths and `../` cannot undo or redo;
  - an absolute spelling of the recorded file itself, inside the root or through a hard link outside it, is refused;
  - the wrong workspace, run or agent, a replaced root, a revoked grant and read-only grants all deny;
  - legacy raw-path entries do not deserialize;
  - serialized entries recreate no authority after a restart;
  - a `FileCreate` entry never deletes an original file;
  - changed content denies;
  - redirects deny: Unix symlinks, and Windows reparse points in native tests;
  - a denied change applies nothing, and partial failure is truthful.

### 5.2 Backup restore (D-BACKUP-RESTORE)

- `backup_restore` stays closed (E1). The library `restore_backup` is hardened, and the desktop still has no caller: `restore_backup` stays a latent guard needle.
- **Validation first.** Every entry is checked before anything is written:
  - only regular files, named `backup-metadata.json`, `config/config.toml` or `data/<relative>`;
  - relative names pass `validate_relative`, which rejects:
    - absolute, root and prefix forms, `..`, and empty or `.` components;
    - drive, UNC and ADS/colon forms;
    - device names and control characters.
  - symlink, hard-link, device, FIFO, directory, sparse and global-header entries are refused;
  - so are case-folded duplicates and file/directory aliases;
  - bounds: 65,536 entries, 2 GiB per entry and 8 GiB in total, with metadata capped at 1 MiB.
- **Then extraction** into a fresh `restore-<uuid>` directory (0o700 on Unix):
  - it is created beneath an existing canonical restore root that is not a redirect;
  - directories are created one component at a time and files exclusively (`create_new`), so nothing is overwritten through a redirect;
  - copied sizes are verified;
  - the partial directory is removed on any failure.
- Backup creation skips symlinks instead of following them.
- **Tests** (`kernel/src/backup.rs`, 9 new): hostile names written as raw tar headers, special entries, aliases, bounds, fresh directories, root validation, and Unix and Windows-native redirect tests.

### 5.3 Identifier joins (D-ID-JOIN)

| Surface | Model |
|---|---|
| notes_{get,save,delete}, project_{get,save,delete} | `validate_identifier` (ASCII `[A-Za-z0-9._-]`, alphanumeric first, at most 128 bytes, no trailing dot, no device name) before any path or log |
| email_{save,delete} | `storage_stem`: grammar-valid ids keep their name; any other value becomes `h-<sha256>` |
| Email token reads, fetch, send, search, disconnect | provider allowlist (`gmail`, `outlook`) as `&'static str` |
| messaging_{connect_platform,send,poll_messages} | platform allowlist (`telegram`, `discord`, `slack`); Discord channel ids are 1–20 digit snowflakes; Telegram tokens are checked before any URL use |
| Agent memory (SDK persistence) | the agent id must parse as a UUID; the file is named by its hyphenated form |
| nx_session_save | `validate_identifier(name, 64)` |
| download_model, its Modelfile and model config | Hugging Face ids: 1–2 segments, at most 96 bytes. File names: at most 4 segments of 128 bytes. The storage directory is `hf-<digest>` of the id, never a `/` replacement. The target is absolute and joined with `join_relative`. |
| nexus_link_send_model | `regular_file_beneath` the models directory (a no-follow walk to a regular file) |
| flash_download_model, flash_download_multi, flash_delete_local_model | `validate_model_filename` (one hub name, at most 200 bytes) and `validate_hf_repo`; `ModelStorage::model_path` returns an error |

- **Unchanged, already safe:** the integration OAuth token file (`provider_id` must match github, gitlab, slack or jira before any write) and email OAuth status (a fixed provider list).
- **Tests:**
  - `governed_path` grammars: separators, `.`/`..`, absolute and prefixed forms, drives, UNC, ADS, device names, controls and length;
  - desktop `commands::apps` (identifiers, providers, platforms, snowflakes, tokens, and audit records that never echo a value);
  - SDK memory, connectors-llm hub and Nexus Link;
  - flash downloader (with the `download` feature).

### 5.4 Identity home (D-HOMEFALLBACK)

- **Policy (`nexus_kernel::identity_home`).**
  - HOME must be non-empty and absolute.
  - Only an absent HOME on Windows falls back, to the native profile. Anything else is `IdentityHomeMissing`.
  - There is no fallback to the working directory, `/tmp`, a literal `~`, a frontend value or an arbitrary environment root.
  - The desktop oracle identity resolver delegates to it. Application state is never workspace authority.
- **Migrated to `<home>/.nexus` (or the platform data directory under the same home)** — and unavailable without a home:
  - kernel: the config path, the default backup directory (`create_backup` also requires an absolute output directory), knowledge-graph and cognitive-loop databases, the MCP sandbox, the policy directory and the scheduler store;
  - desktop: AppState (model registry, Nexus Link models, agent memory, metering, database), builder improvement analysis, the frontend error log, apps data, the desktop-control workspace, the file-manager home helper, governance policies, the marketplace database, system metrics, legacy database cleanup, flash storage (`ModelStorage::for_home`), benchmark reports, the swarm Herald database, nx sessions, learned patterns and action memory;
  - web-builder: budget, model config, deploy credentials and the improvement store;
  - elsewhere: SDK memory, the connectors-llm model registry, the swarm spend ledger and routing overlay, nexus-code memory (an absolute data directory or none) and the social-poster database.
- **Refusal.** Without a valid home, the desktop refuses to start, and the persistent oracle already required one. Library stores become unavailable and write nothing.
- **Retained operator overrides:** `NEXUS_DB_PATH` and `NEXUS_CONFIG_PATH` (§7).
- **Tests:**
  - `kernel/tests/phase0_identity_home.rs` sets HOME to `""`, `"."`, a relative path, `"~"` and `"~/nexus"`. Every location errors, backup refuses, no policy loads, and the working directory stays empty. A valid home gives deterministic paths.
  - Per-store unit tests.

### 5.5 Private temp (D-SHARED-TEMP)

- **`private_temp_dir`:** an unpredictable name, exclusive creation, owner-only (0o700) on Unix, and removal on drop. A creation error fails the operation.
- **Migrated:**
  - kernel computer-control screen and window captures (Linux and macOS);
  - nexus-computer-use screenshot capture;
  - the flash benchmark report, now written to identity-home `reports/` with `create_new` and a UUID name.
- The remaining shared-temp uses are latent, outside the desktop, or final-gate items. The C5B guard counts them (§6, §8).

### 5.6 URL / curl (D-CURL)

- **Every production curl invocation (35 invocation sites in 21 files, as counted by `CURL_SITES`) now:**
  - starts with `-q` and `--globoff`;
  - allows only `http,https` (or `https`) for the request and for every redirect;
  - receives the URL after `--`, in the normalized spelling of a URL that parsed as http(s) with a host and no userinfo;
  - sends bodies with `--data-raw`, or backend JSON on stdin through the fixed `--data-binary @-` form;
  - passes headers only as HTTP tokens whose values carry no CR, LF or NUL;
  - is bounded by a timeout and a response size. Model downloads have connect and stall bounds only, because model files are large.
- **Sites:**
  - desktop: api_client_request, the Ollama probe and delete_model;
  - connectors-llm: shared helpers, Ollama and the Hugging Face hub;
  - kernel: A2A discovery and send (a2a_*), and the web, API, image, speech and vision actuators;
  - protocols: the MCP HTTP client (mcp_host_*);
  - connectors: web reader and search, and key validation;
  - crates: the external-tools adapter (tools_execute), nexus-mcp tools, memory embeddings, perception vision and evaluation clients.
- **Before the process runs:** hostile URLs (`file:`, leading `-`, `@file`, userinfo, other schemes, whitespace), unsupported methods and injected headers are refused. api_client_request audits the refusal by reason class.
- **Egress checks.** The API and web actuators now apply their allowlists to the normalized URL, so `allowed.example@other.example` no longer passes a prefix match. Egress itself stays prefix-based (§7).
- **Now fail closed:** external-tools pseudo-requests that are not http(s): the `smtp://` email tool and the `local://database` tool. Neither ever worked through curl.
- **Unchanged:** the benchmarks' curl calls (six sites in `benchmarks/conductor-bench`), which are outside the desktop closure and use configured endpoints.
- **Tests:** `governed_http`, plus a refusal test at every family of sites above.

### 5.7 Serialized records (D-RECORD)

| Record | Disposition |
|---|---|
| Time Machine entries | Grant-bound (§5.1); legacy raw-path entries do not deserialize |
| Backup archive entries | Validated names inside a fresh directory (§5.2) |
| Agent manifests, `consent_policy_path` | Refused (§5.8) |
| `NexusConfig.security` (vault master-key source: `enabled`, `key_source`, `key_env`, `key_file`) | `save_config` (interface IPC) writes only when the request carries exactly the section loaded from the backend's own configuration. If that configuration cannot be loaded (unreadable, undecryptable, unparsable, or no identity home), no baseline exists: the save is refused, nothing is written, and no default stands in. Refusals are audited by reason class only (`security_settings_backend_owned`, `current_security_settings_unavailable`). A configured key file must be an absolute path, and key-file errors no longer name the file. |
| nx sessions, notes, projects, email, tokens | Named only through §5.3 |
| Scheduler records | Name agents and goals, no path; the store lives under the identity home |
| `NexusConfig.backup.output_dir` | Stored, but read by no code (there is no scheduled-backup consumer) |

Found during the C5B preflight rather than by the original audit: `save_config`
accepted the whole `NexusConfig` from the webview, including the key source
that startup uses for the secrets vault.

### 5.8 Consent policy (`consent_policy_path`)

- **Before.** The field chose a policy TOML to read and an approval-queue file to create beside it. It reached registration through create_agent, L6 approval, restored database rows and model-written sub-agent manifests.
- **Model.** Consent policy is backend-owned: the default policy engine and an in-memory queue.
  - `parse_manifest` refuses the key in any form: absolute, relative, `..`, `~`, drive-prefixed or empty.
  - `ConsentRuntime::from_manifest` refuses any value before touching the filesystem. JSON records that bypass the TOML parser therefore fail closed, and such an agent never registers.
  - The agent-lifecycle actuator refuses a sub-agent manifest carrying it before persisting anything.
  - The coding and social-poster agents refuse it through the same runtime.
  - The field stays on `AgentManifest` only so older records deserialize.
- **Migration:** deterministic fail-closed. A stored row that names a policy file is not restored; rows without the field restore as before.
- **Tests:** manifest, supervisor, lifecycle, desktop restore and agent-crate refusals. Each shows that nothing is read or created beside the named file.

### 5.9 C5A closures

Unchanged. `CLOSED_COMMANDS` (125 closed commands, plus the gated
`execute_tool`), the agent executor, the CLI-agent closures and
`AMBIENT_ROOTS` still pass their guards. C5B reopens no command.

### Count transitions

The audit-base class totals (§1) are historical and not recomputed here.
Transitions this checkpoint made, counted from its own changes:

| Family | C5A state | After C5B |
|---|---|---|
| D-TIME-MACHINE | 4 IPC replay paths over raw strings (undo, undo_checkpoint, redo, what_if); no producer | Raw strings no longer deserialize; production file replay unavailable |
| D-BACKUP-RESTORE | Command closed; library unsafe | Command closed; library validates and extracts into a fresh directory |
| D-ID-JOIN | The C5A §5 list (notes, email and email tokens, messaging, projects, agent memory, nx sessions, model download and send, flash download and delete) accepting caller identifiers | Every listed surface bound by a grammar, allowlist, digest or no-follow walk (§5.3) |
| D-HOMEFALLBACK | 12 fixed-store fallbacks named by C5A, plus the stores found in preflight | All migrated to the identity home (§5.4); remaining uses are latent or outside the desktop, each counted by the guard |
| D-SHARED-TEMP | The C5A-listed screen and window capture files and benchmark report (plus CLI-only files) | Desktop-reachable names migrated; each remaining shared-temp use is counted by the state-root guard as latent, final-gate or outside the desktop |
| D-CURL | The C5A-listed families: api_client_request, tools_execute, mcp_host_*, a2a_*, delete_model | 35 production invocation sites in 21 files, all governed (`CURL_SITES`) |
| D-RECORD | `consent_policy_path` | Refused; `NexusConfig.security` added and closed |

## 6. Latent Category D

These implementations stay compiled, in their crates, with no desktop production caller. `latent_unsafe_apis_have_no_desktop_production_caller` fails if any desktop production source references one:

- the typed-tool executor;
- the Conductor pipeline;
- the legacy web-builder checkpoint, llm_codegen and dev_server modules;
- the genesis engine;
- backup restore;
- computer-control `sh -c` execution;
- the MCP stdio client and MCP tool handler;
- agent-memory persistence;
- the nexus-code and computer-use agent loops (the nexus-code Claude CLI provider and screen-analysis tool are never registered by the desktop);
- the content pipeline;
- Time Machine file recording;
- the full default actuator registry;
- the dormant process-, code- and OS-input-bearing APIs of the desktop's dependency closure:
  - the coder terminal, test runner and fix loop;
  - the nexus-code agent module, its tools, and direct execution of a registered tool;
  - computer-use vision (it runs the Claude CLI);
  - SDK typed tools and the WASM agent sandboxes;
  - the factory pipeline's build, test, deploy and full-pipeline shell runs (the desktop reads only its in-memory project list and build history);
  - the shell, filesystem, code, Docker, browser, computer-use, input, screen-capture, API, image, speech and self-evolution actuators.

The kernel action executor may be constructed only once in desktop production: as the Phase Zero agent executor, with an empty workspace root. Separately, the browser agent's Python bridge is never started by its session code.

P0-002C5B adds these latent items. Each is named by the C5B guards, and none
has a desktop production caller:

- Time Machine file replay with authority: `FileAuthority::new(`, `.undo_with(`, `.redo_with(` and `undo_checkpoint_with(`.
- The computer-use learning stores' `with_default_path()`, which falls back to a `/tmp` home. The desktop passes identity-home paths.
- Message polling (`poll_platform(`). Telegram voice notes would land in a predictable shared-temp directory, named by a remote file id.
- Nexus Link `receive_model(`. It joins a peer-chosen file name behind a partial `..`, `/` and `\` check.
- The audit archive (`RetentionBuffer::new(`), whose default is shared temp. Only benchmarks use it.
- Counted in the state-root guard:
  - the Codex auth-file read, whose only caller (`builder_check_cli_auth`) is closed;
  - the control-crate vision loop;
  - the browser agent's default screenshot path;
  - the legacy web-builder checkpoint and project listing;
  - the nexus-code slash commands and tools.

Other latent D remains as recorded by the audit. It has no desktop caller, is not registered, and is not named by the guard:

- dormant raw-path storage APIs;
- the kernel governance-policy, agent-lifecycle and cognitive-parameter actuators, which are not routed and change kernel state rather than files or processes;
- the out-of-closure developer tools.

The `nexus-swarm` healthcheck binary is a developer tool, not part of the desktop, and it still registers the swarm Codex CLI provider. Reaching any of these from desktop production requires a new registered command or agent route, which the C5C guard will inventory.

## 7. Final-gate review items (not fixed in C5A)

PHASE ZERO FINAL-GATE REVIEW REQUIRED:

1. **Machine- or ambient-derived key material.**
   - The configuration encryption key is derived from `HOME`, `USER`, `USERNAME` and `HOSTNAME`.
   - The sealing secret comes from `/etc/machine-id` and `/etc/hostname`; this path is unreached.
   - The TEE key directory is in shared temp; also unreached.
2. **Broad outbound network policy.**
   - Egress checks are prefix/substring based.
     - C5B normalizes the URL and refuses userinfo before the API and web actuators check their allowlists.
     - It does not narrow any allowlist.
   - `tools_execute` and `mcp_host_*` accept caller http(s) URLs. Since C5B, no URL can select another protocol, a file or a curl option.
   - Nexus Link allows every peer when `allowed_peers` is empty.

Also recorded for the final inventory:

- **Operator state-location environment overrides** (`NEXUS_DB_PATH`, `NEXUS_CONFIG_PATH`). They relocate app state; they are not code-bearing. C5B keeps them as recorded operator overrides.
- **Secrets in curl arguments.** Some provider calls pass API keys in `-H` arguments, where a same-user process listing can see them. This is unchanged.
- **PATH-resolved helper binaries run with fixed arguments.** This includes curl, git, and `which` presence probes for git, ripgrep and ollama at nx bridge startup. No external CLI agent is among them.
- **OS input** by the kernel computer-control commands. The E6 decision covered only the EOF approval bug.
- The `null` webview CSP.

## 8. Reachability guard

`app/src-tauri/src/phase0_surface/tests.rs`:

- `CLOSED_COMMANDS` is the classified registry of closed IPC commands. For each entry, the guard proves the command:
  - is registered exactly once;
  - takes no input;
  - has a body that is exactly its `closed(..)` denial;
  - returns that bounded reason when invoked.
- `execute_tool` refuses before any process runs; the filesystem is checked unchanged.
- The production agent executor refuses every authority-bearing action and non-HTTP fetch, while authority-free actions still run.
- No desktop production source:
  - starts an external CLI agent;
  - bypasses its permissions;
  - registers a CLI agent in the swarm;
  - calls a Nexus Code entry point that runs, prefers or registers the Claude CLI (the nx bridge must use the `_without_cli_agents` forms);
  - uses an ambient root. The only exceptions are the counted C4D2 packaged-toolchain lookup and the test-only prebuilt source.
- Latent unsafe APIs have no desktop production caller; the executor is constructed only as the Phase Zero agent executor.

P0-002C5B adds workspace-wide regression guards in the same file. They scan
the production text of every workspace member except the benchmarks, with
comments and `#[cfg(test)]` / `#[cfg(any(test, ..))]` items removed. The
scanner has its own self-test.

- `p0_002c5b_curl_invocations_keep_caller_values_out_of_curl_syntax`:
  - `CURL_SITES` counts every production curl invocation per file, so a new site fails until it is classified.
  - A file with curl must use no file-reading or config option (`-d`, `--data`, `-F`, `-T`, `-K`, `--config`, `--url` and relatives). It may use `--data-binary` only with `@-`.
  - It must start curl with `-q` and a protocol allowlist, and must terminate options with `--`.
- `p0_002c5b_state_roots_take_no_home_cwd_or_shared_temp_fallback`: `APPROVED_STATE_ROOTS` counts every remaining production read of HOME, `dirs::`, `~/`, `temp_dir()`, `/tmp` and `.`-as-root, each with its reason.
- `p0_002c5b_identifier_joins_stay_behind_their_grammars`:
  - the joins that remain are counted, and each sits behind a grammar or allowlist;
  - the pre-C5B spellings are gone;
  - the validators are called.
- `p0_002c5b_serialized_records_choose_no_authority`:
  - no production code loads a consent policy or file-backed queue;
  - `from_manifest` starts with the refusal;
  - `parse_manifest` refuses the key;
  - `save_config` checks the security section before writing;
  - the Conductor records no file entries.
- `LATENT_UNSAFE_APIS` gains the §6 C5B needles.

These carry their own closure tests:

- the CLI providers (`connectors/llm`);
- the Nexus Code desktop entry points (`nexus-code/tests/phase0_cli_agents.rs`, with a fake `claude` on `PATH`);
- the planner and loop (`kernel`);
- the swarm coder (`agents/coder`);
- computer-use approval (`crates/nexus-computer-use`).

**Maintainers.**

- To re-open a closed command, or wire a latent API back in:
  1. obtain an Architect-approved authority mechanism;
  2. implement the surface on that mechanism;
  3. remove its entry from `CLOSED_COMMANDS`, `LATENT_UNSAFE_APIS` or `AMBIENT_ROOTS` in the same change;
  4. update this document.
- To add a new desktop command that touches the filesystem or starts a process, classify it here first. A command that holds no approved authority is added to `CLOSED_COMMANDS`, not given a working body.
- The guard is a focused surface guard, not a ban on `std::fs`. It does not detect a semantically equivalent surface under a new name; the C5C inventory closes that gap.
- To add a curl call, an ambient-root read or a raw identifier join, route it through `governed_http`, `identity_home` or `governed_path`, then update `CURL_SITES`, `APPROVED_STATE_ROOTS` or the counted joins, together with this document.

## 9. Explicit non-claims

- No filesystem, process or network sandbox exists. `execute_tool`, the agent executor and the closed commands deny; they do not contain anything.
- No user-selected file access exists: E1 surfaces are unavailable rather than brokered.
- Builder projects, plans and legacy undo/rollback are not durable across restarts.
- **Time Machine file undo and redo are unavailable in production.** Grant-bound replay exists only as a kernel capability that nothing in the desktop holds.
- **Backup restore is not available to users.** The command stays closed; only the library was hardened.
- **`governed_http` checks the shape of a request, not its destination.** It is not a network sandbox or an egress policy.
- **`private_temp_dir` and the no-follow checks are point-in-time pathname checks.** They are not atomic protection against a same-user process racing the namespace.
- The guards cover the approved surface: registered commands, named latent APIs, named ambient roots, counted curl sites, counted state roots, counted joins and the agent executor. They are not a complete call graph.
- Point-in-time checks only. There is no protection against a hostile same-user process racing filesystem state.
- Nothing here claims Phase Zero is complete.

### Remaining C5C debt

- **The final authority inventory and recount:** the latent items in §6, the out-of-closure members and the benchmarks.
- **Nexus Code configuration** read at nx bridge startup: `NEXUSCODE.md` and `.nxrc` from the working directory, and the user config from the platform config directory.
- **A2A delegation** is dispatched in the cognitive loop before the Phase Zero agent executor. It needs an `a2a.delegate` capability that the registry does not grant.
- **Behavioural changes:**
  - Email ids outside the grammar (for example Microsoft Graph ids containing `=`) are now stored under a digest name. Files written before C5B under the raw id are no longer addressed by that id.
  - The external-tools `email` and `database` tools fail closed.
  - Model downloads have no total-size bound.
- **The desktop test-support state** (`AppState::new_in_memory`) still gives its schedule store the shared temp directory. It is compiled only for tests and the `test-support` feature.
