# Phase Zero C5 — Authority Inventory and Reachability Closure

Status: P0-002C5C (final authority sweep and closure), on top of P0-002C5B
(governed recovery and the remaining straightforward filesystem migrations)
and P0-002C5A (fail-closed reachable surface closure). C5 is split into
bounded checkpoints:

| Checkpoint | Scope |
|---|---|
| **P0-002C5A** | Make every reachable E1–E5 surface fail closed unless an approved backend-owned authority mechanism already governs it; fix the computer-use EOF approval bug; stabilise the Darwin sealed-spawn fixture; record this inventory and the reachability guard. |
| **P0-002C5B** | Governed recovery and the remaining straightforward filesystem migrations: every reachable surface deferred by C5A is migrated onto an existing Phase Zero primitive or fails closed (§5). |
| **P0-002C5C** | A fresh whole-repository audit, repair of every reachable finding it could bound, the final recount and the final trust-surface guard (§10). After the Architect's review, it also closes unbrokered screen observation and enforces explicit egress schemes and ports (§10.10). One finding remains unresolved and one is an approved operator assumption (§10.8). The Final-Gate evidence is in `phase0-final-gate-dossier.md`. |

The invariant C5 serves: **a path is not authority.** C5C states it in its
general form: **a string is never authority**, including frontend, model and
serialized paths, the working directory, `HOME`, temp, environment variables,
program names, URL prefixes, caller approval flags and persisted identifiers. No security-sensitive
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
  - `validate_storage_identifier`, the identifier grammar in ASCII lowercase only, for ids used directly as file stems;
  - `storage_stem`, which keeps a lowercase storage identifier and otherwise uses a deterministic digest of the original bytes;
  - `case_exact_entry` and `case_exact_relative`, which refuse a name that differs from a stored entry only by letter case;
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
| notes_{get,save,delete}, project_{get,save,delete} | `validate_storage_identifier` before any path or log: ASCII `[a-z0-9._-]`, alphanumeric first, at most 128 bytes, no trailing dot, no device name. The interface issues `n-<millis>` and `default`. An uppercase spelling is refused, never folded. |
| email_{save,delete} | `storage_stem`. A lowercase storage identifier outside the reserved `h-` namespace keeps its name. Any other value becomes `h-<sha256 of its original bytes>`; that covers a case variant such as `MessageA`, and any spelling of `h-`. |
| Email token reads, fetch, send, search, disconnect | provider allowlist (`gmail`, `outlook`) as `&'static str` |
| messaging_{connect_platform,send,poll_messages} | platform allowlist (`telegram`, `discord`, `slack`); Discord channel ids are 1–20 digit snowflakes; Telegram tokens are checked before any URL use |
| Agent memory (SDK persistence) | the agent id must parse as a UUID; the file is named by its hyphenated form |
| nx_session_save | `validate_identifier(name, 64)` for the label. The file is `storage_stem(name)`, and the listing reads names from file contents. |
| download_model, its Modelfile and model config | Hugging Face ids: 1–2 segments, at most 96 bytes. File names: at most 4 segments of 128 bytes. The storage directory is `hf-<digest>` of the id, never a `/` replacement. The target is absolute and joined with `join_relative`. A file name that differs from a stored file only by letter case is refused (`case_exact_relative`). |
| nexus_link_send_model | `regular_file_beneath` the models directory: a no-follow walk to a regular file, with each component spelled exactly as stored |
| flash_download_model, flash_download_multi, flash_delete_local_model | `validate_model_filename` (one hub name, at most 200 bytes) and `validate_hf_repo`. `ModelStorage::model_path` returns an error, and a model or `.part` name that differs from a stored file only by letter case is refused. |

- **Unchanged, already safe:** the integration OAuth token file (`provider_id` must match github, gitlab, slack or jira before any write) and email OAuth status (a fixed provider list).
- **Case-insensitive filesystems.** Windows and default macOS resolve two spellings that differ only by case to one file. Every identifier-to-file mapping above therefore holds at most one spelling per stored object, on every platform:
  - Backend-grammar ids (notes, projects) are lowercase-only storage identifiers.
  - Case-sensitive ids (email, nx session labels) map through `storage_stem`. Raw stems are lowercase and never start with `h-`; generated stems are lowercase `h-` digests of the original bytes. Stems are therefore distinct even under ASCII case folding, and no spelling of `h-` selects a generated stem.
  - Filename-keyed stores (Hugging Face downloads, flash models, Nexus Link send) refuse a name that differs from a stored entry only by case. Their grammars are ASCII, so ASCII folding is the folding those filesystems apply.
  - The token files use fixed lowercase names from allowlists, and SDK agent memory is named by the canonical hyphenated UUID (UUIDs are case-insensitive by definition).
- **Tests:**
  - `governed_path` grammars: separators, `.`/`..`, absolute and prefixed forms, drives, UNC, ADS, device names, controls and length;
  - case aliasing: stems stay distinct after folding, no spelling of `h-` reaches a generated stem, storage identifiers have one spelling, and exact-spelling checks refuse variants. A native Windows test shows NTFS resolving a variant to the stored file while the rules still refuse it.
  - desktop `commands::apps` (identifiers, providers, platforms, snowflakes, tokens, and audit records that never echo a value) and the nx session file helper;
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
| `NexusConfig.security` (vault master-key source: `enabled`, `key_source`, `key_env`, `key_file`) | `save_config` (interface IPC) writes only when the request carries exactly the security section of the *existing* configuration, as read by `load_current_security_baseline`. That helper never bootstraps: it creates, rewrites and migrates nothing. A missing, empty or whitespace-only (truncated), unreadable, undecryptable or unparsable configuration, or no identity home, leaves no baseline. The save is then refused, nothing is written, and no default stands in. Only backend startup (`load_config`) creates the first-run default. Refusals are audited by reason class only (`security_settings_backend_owned`, `current_security_settings_unavailable`). A configured key file must be an absolute path, and key-file errors no longer name the file. |
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

C5C moved these items, with exact evidence and the items it found, into
`docs/security/phase0-final-gate-dossier.md`. The list below is the C5A/C5B
record.

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

P0-002C5C extends these guards into the final trust-surface guard (§10.9).

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
- **C5C adds none of the following:**
  - a sandbox, a user-file broker, durable Builder authority, a new agent
    runtime or a secret vault;
  - a destination policy for interface-chosen requests;
  - an out-of-band approval channel;
  - a brokered screen-observation mechanism: unbrokered observation is
    closed instead (§10.10);
  - a CSP.

  `docs/security/phase0-final-gate-dossier.md` records these as Final-Gate
  items.

### Remaining C5C debt (resolved in C5C, §10)

- **The final authority inventory and recount:** the latent items in §6, the out-of-closure members and the benchmarks.
- **Nexus Code configuration** read at nx bridge startup: `NEXUSCODE.md` and `.nxrc` from the working directory, and the user config from the platform config directory.
- **A2A delegation** is dispatched in the cognitive loop before the Phase Zero agent executor. It needs an `a2a.delegate` capability that the registry does not grant.
- **Behavioural changes:**
  - Email ids that are not lowercase storage identifiers are now stored under a digest name: Microsoft Graph ids containing `=`, and any mixed-case id. Files written earlier under the raw id are still listed, because the list reads file contents, but they are no longer addressed by that id.
  - Note and project ids with an uppercase letter are refused. The interface never issues them, so such a file written earlier is listed but not addressed.
  - An nx session saved earlier under a mixed-case name keeps its old file. Saving that name again writes the digest-named file, so both are listed.
  - A model file name that differs from a stored file only by case is refused. Stored files are unchanged.
  - The external-tools `email` and `database` tools fail closed.
  - Model downloads have no total-size bound.
- **The desktop test-support state** (`AppState::new_in_memory`) still gives its schedule store the shared temp directory. It is compiled only for tests and the `test-support` feature.

## 10. P0-002C5C: final authority sweep and closure

### 10.1 Fresh audit

The sweep started read-only at the authoritative C5B head `58e5a4c0` (tree
`eb90cd84`). It covered every workspace member and five topics: filesystem
and ambient roots, processes, network, recovery and records, and IPC and agent
dispatch. Each finding was re-verified in source and traced to its IPC
command, startup task or agent path. C5C repaired every reachable finding it
could bound (§10.2), recounted the desktop closure (§10.8) and repaired what
the recount found.

### 10.2 Findings and repairs

| Finding | Before C5C | Repair | Evidence |
|---|---|---|---|
| Desktop Nexus Code read `NEXUSCODE.md`, `.nxrc` and the platform config directory from the working directory (C5B debt) | reachable at nx bridge startup | `NxConfig::load_for_desktop`, `setup::diagnose_for_desktop`, `App::new_for_desktop`: defaults, an absolute backend file under the identity home, and the `NX_*` provider settings only. The standalone `nx` keeps its project files. | `nexus-code/tests/phase0_desktop_config.rs`; guard `desktop_nexus_code_takes_no_configuration_from_the_working_directory`; NC1, NC2 |
| Interface- and model-chosen OS input: `computer_control_execute_action`, `start_computer_action` (xdotool text passed without `--`, so `type --file=` read files) | reachable | Both commands closed (`Closure::OsInput`); the backend action path removed; `InputAction` and `execute_input_action(` named latent | `CLOSED_COMMANDS`; latent guard |
| `NEXUS_DB_PATH` / `NEXUS_CONFIG_PATH` accepted relative values (state in the working directory) | operator | `identity_home::operator_override`: non-empty and absolute, or no location | `kernel/tests/phase0_identity_home.rs`; NC8b |
| Legacy-database cleanup read `NEXUS_DB_PATH` with `var`, so a non-UTF-8 override hid the operator database from the guard and the cleanup deleted it | startup | `var_os`, like `nexus_db_path` | guard `p0_002c5c_operator_overrides_stay_launch_configuration` |
| OAuth loopback (email, integrations): `state` never checked, `GET /?` parsed instead of the redirect path, blocking `accept` so the deadline never fired | reachable (login CSRF from any local process or rendered page) | `await_oauth_code`: nonblocking poll to the deadline, bounded request head, only `GET /oauth/callback?` with the flow's single `state`, decoded code; client ids from a safe charset; bind before opening the browser | `commands::apps` tests; NX1 |
| A2A delegation sent by the cognitive loop before and instead of the executor; restore trusted stored manifests, so a row could grant `a2a.delegate` | reachable through a stored record | The loop owns no A2A client and dispatches every action through the executor; Phase0AgentExecutor refuses delegation; `validate_stored_manifest` on restore | §10.4; NC3, NC4, NX4 |
| Model downloads had no size bound (hub, flash); flash followed any redirect, never timed out a stall, and appended a 200 answer to a Range request | reachable | `MAX_MODEL_FILE_BYTES` (64 GiB) enforced on the bytes written; https-only redirects; stall timeout; restart on 200 | hub monitor and loopback flash tests; NC7a, NC7b |
| 14 of the 35 production curl sites had no response-size bound (C5B had described all as bounded) | reachable | `--max-filesize` at each; guard requires time and size bounds | `p0_002c5c_every_production_curl_site_is_bounded_in_time_and_size`; NX3 |
| Notification helper formatted its message into AppleScript and PowerShell with `{:?}` quoting (PowerShell does not honour it) | fixed literal caller only | `notification_invocation`: the message is an argument after `--`, an AppleScript `argv` item or an environment value read by a fixed script | `p0_002c5c_notification_text_never_becomes_a_script`; NC6 |
| `browser_screenshot(output_path)` took a raw interface path for the (never started) browser bridge | dormant | Closed (`Closure::FileSelection`) | `CLOSED_COMMANDS` |
| Nexus Link send: no connect timeout; a peer's length prefix allocated up to 4 GiB | reachable | `connect_peer` (connect, read and write timeouts); 16 MiB message cap | `nexus_link` test |
| Theme extraction fetched a caller URL after a prefix check, followed redirects to any scheme, read the whole body | reachable | `governed_http::http_url`, https-only redirects, 4 MiB cap | `theme_extract` tests |
| Ollama health probe stripped the scheme by prefix and connected to any IP:port | reachable | The base URL must pass `governed_http::http_url` | `providers::ollama` test |
| Egress allowlists matched by string prefix (`example.com` admitted `example.com.evil.net`); agent fetches followed redirects past the allowlist; byte-offset text cuts could panic on remote UTF-8 | reachable from model output | `firewall::egress::endpoint_admits` in every matcher; no redirect following for agent fetches; `floor_char_boundary` cuts | egress, web actuator tests; NX2 |
| `tools_execute` took the autonomy level from the caller; credential-bearing tools interpolated caller values into URLs | reachable | `tool_call_autonomy` (the registered agent's level, never raised by the claim); identifier grammars for GitHub, Slack, Jira and S3 values | `p0_002c5c_tool_calls_run_at_the_registered_agents_autonomy`; tools grammar test; NX5 |
| Legacy mixed-case store files (from before C5B) were the same file as a new lowercase id on case-insensitive filesystems | reachable | `stored_file` and `nx_session_file` run `case_exact_entry`; listings keep legacy files | legacy compatibility tests; NX6 |
| The webview rendered model replies, note text, the design preview and the crash message as HTML, and opened data-sourced links as given (`javascript:` included); with the `null` CSP, script there reaches every IPC command | reachable from model output and remote content | `app/src/lib/safeHtml.ts`: escape before markup, http(s)-only link and window targets; the design preview sandboxed | vitest `safeHtml.test.ts`; guard `p0_002c5c_frontend_html_sinks_are_escaped_and_previews_sandboxed`; NX7–NX9 |
| `sim_run` answered file and environment preconditions and simulated writes and deletes by probing the host for the caller's paths and names (an existence oracle) | reachable | The sandbox has no view of the host: files exist only if the scenario wrote them, and no variable is set | `sandbox::tests::p0_002c5c_simulations_reveal_nothing_about_the_host`; NX10 |
| `swarm_approve` ran a model-planned swarm whose Herald node could publish a model-written post with the stored X credentials when the plan set `dry_run: false` | reachable (model choice released by an IPC approval) | `HeraldAdapter::drafts_only()` in the desktop registry | `p0_002c5c_a_drafts_only_herald_never_publishes`; delegation guard; NX11 |
| `get_config` returned every stored credential still held in the configuration, in plaintext, to the interface | reachable | Credentials are returned as a placeholder; a save of the placeholder keeps the stored value; messaging connect resolves it | `p0_002c5c_the_interface_never_reads_stored_credentials`; serialized-records guard; NX12 |
| The flash provider defaulted its model file to the relative `flash-local` (`test_llm_connection`, `LLM_PROVIDER=flash`), so the working directory chose the file for the llama loader and its page-cache warm-up | reachable; only builds linked with real llama.cpp open the file (no release build sets `NEXUS_LLAMA_CPP_PATH`, so released desktops link the stub loader) | Only an absolute configured path; otherwise the provider is unavailable | `connectors/llm/tests/phase0_flash_model_path.rs`; NX13 |
| A chat model id `flash/<path>` (`send_chat`, and the same resolver in the cognitive and Builder routes) would have named a file for the native model loader | not compiled: the arm required a `flash-infer` feature that the desktop crate does not declare, so the id failed as an unknown prefix. The message of `1b61903c` calls the arm reachable; it was not. | Hardening: the `flash` prefix returns the file-selection closure, whatever the build features | `p0_002c5c_a_flash_model_id_never_names_a_file` |
| `email_send_message` wrote the caller's recipient and subject into raw header lines (CR/LF added headers such as `Bcc`) | reachable | CR, LF and NUL refused before any token is read | `p0_002c5c_email_header_values_cannot_add_headers`; NX14 |
| C4B exit-race test failed about 1 in 1,500 runs | test defect | Test accepts the valid finalized-before-stop ordering | stress evidence in the C5C report |
| Unbrokered screen observation over IPC (Final-Gate item L), in four routes. `capture_screen` and `analyze_screen` enabled a disabled engine, even after the emergency stop, and captured; `analyze_screen` also sent the capture to a vision model. `computer_control_capture_screen` captured without consulting the stop. `nx_computer_use_screenshot` captured the whole screen directly, ignoring the engine and the stop. Separately, `computer_control_toggle(true)` enabled the engine over IPC | reachable (classified FIXED at the recount) | Architect repair A: the four routes closed (`Closure::ScreenObservation`) and their live implementations removed; the toggle's enabling branch refused; the capture, vision and stop-reset APIs named latent (§10.10) | `p0_002c5c_screen_observation_requests_are_denied_and_change_nothing`; `p0_002c5c_no_desktop_route_observes_the_screen`; closed-command guards; controls A1–A3 |
| An explicit `https` allowlist entry admitted plain `http`: C5C's matcher compared entries without their scheme, and ports as text | reachable from model output (agent web fetch, API and browser actuators, content pipeline) | Architect repair B: scheme, normalized host, effective port and whole path segments; a documented compatibility rule for entries without a scheme (§10.10) | four `firewall::egress` tests; `http_does_not_match_https_allowlist`; control B |

### 10.3 Desktop Nexus Code configuration

`app/src-tauri/src/nx_bridge/mod.rs` builds Nexus Code only through
`NxConfig::load_for_desktop(nexus_state_path("nexus-code/config.toml"))`,
`setup::diagnose_for_desktop()` and
`App::new_for_desktop(config, nexus_state_path("nexus-code/memory.json"))`.
None of them reads the working directory, a project or git ancestor, or the
platform configuration directory, and a relative configuration file is
ignored. The `_without_cli_agents` forms, which still read the working
directory's `NEXUSCODE.md` and `.nxrc`, are now forbidden entry points for the
desktop.

### 10.4 A2A dispatch

- `kernel/src/cognitive/loop_runtime.rs`: `run_cycle_with_evolution` has one
  dispatch site, `executor.execute`, for every planned action. The runtime
  holds no A2A client, so an `A2aDelegation` step reaches the host's executor
  like any other action.
- The desktop executor, `Phase0AgentExecutor`, refuses it with the closed
  `AgentExecution` reason. The kernel registry does not handle it either.
- `restore_persisted_agents` runs `validate_stored_manifest`: a stored record
  may name only registered capabilities (`a2a.delegate` is not one), a defined
  autonomy level and no consent policy path.
- **Tests:**
  - `p0_002c5c_a2a_and_agent_actions_are_decided_by_the_production_executor`
    runs the real loop with the production executor, for an agent holding
    delegation, filesystem and process capabilities. A delegation toward a
    local file (an A2A filesystem action) and one toward a peer (an A2A
    process action, since the transport runs a client process) are refused,
    and so are a file write and a shell command. Nothing is sent, read or
    created. A permitted action runs under the same policy.
  - The kernel test shows a delegation reaching the host executor.
  - The restore test refuses records naming `a2a.delegate`, unknown
    capabilities or an undefined autonomy level.
  - The structural guard `p0_002c5c_no_delegation_path_runs_around_the_executor`
    pins the single dispatch site and keeps the swarm coder LLM-only.

No inbound A2A path dispatches to local agents: the `nexus-a2a` bridge's
`route_task` has no desktop caller. The `a2a_*` and `a2a_crate_*` IPC commands
are interface-initiated A2A client calls of governed shape (Final-Gate egress,
dossier item B).

### 10.5 Latent APIs

Each `LATENT_UNSAFE_APIS` entry was re-evaluated against IPC commands,
startup, agent dispatch, schedulers, A2A and MCP paths and dynamic
registries:

- **Agent dispatch.** Every loop runs through `phase0_agent_executor`, and the
  structural guard pins the kernel loop.
- **Schedulers.** `ScheduledGoalExecutor` feeds the same loop. The kernel
  scheduler task only echoes, and `ScheduleRunner` is never started.
- **A2A.** It reaches the executor (§10.4).
- **MCP.** The desktop builds the kernel `McpServer`, whose registry is the
  full default actuator set, only to list tools. C5C names `.invoke_tool(` so
  that invoking one would fail the guard.
- **Dynamic registries.** External tools are HTTP-only and bound to a
  registered agent. `mcp_host_*` refuses stdio, and `mcp2_*` is closed.

No entry was removed, since nothing was migrated or deleted. C5C adds:

- `InputAction` and `execute_input_action(` (kernel OS input);
- `.invoke_tool(` (kernel MCP tool invocation);
- `tauri_commands::screenshot(` (the browser bridge's raw-path screenshot);
- `coder_agent::llm_codegen` (coder writers rooted at a raw output dir).

After the recount (§10.8), C5C also named the APIs behind every latent site
that no needle named:
- 100 process and network sites (fc9a416b, aae835f5);
- 110 filesystem sites (3e6a0506).

Among them are the gaps the recounts found in earlier needles: needles that
named a wrapper but not the API behind it, `enable_retention(`, the
messaging `BridgeDaemon` polling loop, and `ProviderSelectionConfig::from_env()`.

**Architect repair A (§10.10).** C5C also names the screen-capture,
vision-request and emergency-stop APIs:
- **Computer-use capture:** `take_screenshot`, `ScreenshotOptions` and
  `nexus_computer_use::capture`.
- **Kernel capture:** `computer_control::capture_screen`,
  `computer_control::capture_window`, `capture_window(`,
  `capture_and_store_window` and `.capture_screen(`.
- **Vision requests:** `query_vision_model` and `detect_vision_model`.
- **Engine and emergency stop:** `reset_emergency_kill_switch` and
  `ComputerControlEngine::enable`.

The shared command-module import blocks still name three kernel capture
functions, unused: `capture_and_store_screen`, `capture_and_analyze_screen`
and `analyze_stored_screenshot`. The guard
`p0_002c5c_no_desktop_route_observes_the_screen` forbids them outside
imports, forbids renaming them, and forbids any `capture_screen(` call. It
also forbids enabling the engine anywhere that holds it.

### 10.6 Out-of-desktop members

The installers ship only the desktop app. See §10.8 for the recount, and
dossier item J for the shipped non-desktop binaries: `crates/nexus-server`
(a Final-Gate blocker; its withdrawal is implemented on the P0-FG1
validation branch, review pending), the protocols server, `nexus-cli` and
`nx`. None is reachable from desktop IPC or agents. The desktop depends on the libraries of
`nexus-code` and `protocols`, never on their binaries.

### 10.7 Benchmarks

`benchmarks/conductor-bench` runs curl (six sites) and `date` (four sites)
without the production governance, from benchmark binaries only.
`BENCHMARK_PROCESS_SITES` counts them per file, and
`p0_002c5c_benchmark_process_sites_stay_benchmark_only` fails when a new site
appears or when any non-benchmark member depends on a benchmark crate.

### 10.8 Final recount

**Method.** After the main repairs, three independent read-only recounts ran
at `d5e068d8`:

- the IPC command surface, agent dispatch and startup;
- filesystem and ambient-root sites;
- process and network sites.

Each used a lexer-aware stripper (with a self-test) to keep production text
only: tests, benches, examples, fixtures, build scripts and
`#[cfg(test)]`/`#[cfg(any(test, ..))]` items are removed.

- **Scope.** The desktop closure is the 56 workspace crates reachable from
  `nexus-desktop-backend` through normal dependencies. Binary-target sources
  in closure crates (`nx`, the protocols binaries, harness and healthcheck
  binaries) count as outside the desktop.
- **Reachability.** Every reachable claim, and every "no caller" claim, was
  traced by hand from the 676 open commands, startup, and the actions the
  Phase Zero executor permits.
- **Re-verification.** Every UNRESOLVED claim was re-verified in source.
- **Cross-checks.** The recounts reproduce `CURL_SITES` (35 sites in 21 files)
  and `BENCHMARK_PROCESS_SITES` (10 in 5) exactly.
- **After the recount.** The findings were repaired (the last rows of §10.2)
  and the latent APIs named (§10.5). The tables give both states. Every Rust
  change after `d5e068d8` is a repair listed there, or test and guard code.
  Both site scans were re-run at the C5C head: the production site sets are
  unchanged, so only the classes moved.

**IPC commands:** 804 registered. 128 were closed and 676 open until the
Architect repair; since then 132 are closed and 672 open.

| Class of the open commands | Recount | After C5C repairs | After the Architect repair (§10.10) |
|---|---:|---:|---:|
| NONE (no filesystem, process, network, input or secret effect) | 490 | 492 | 492 |
| GOVERNED | 65 | 66 | 66 |
| FIXED | 109 | 110 | 106 |
| DENIED | 8 | 8 | 8 |
| UNRESOLVED (D/E) | 4 | 0 | 0 |
| Open in total | 676 | 676 | 672 |

The Architect repair closed four commands that the recount had classified
FIXED: `capture_screen`, `analyze_screen`, `computer_control_capture_screen`
and `nx_computer_use_screenshot`. `computer_control_toggle` stays open and
NONE; only its enabling branch is refused.

The four unresolved commands and how each was reclassified:
- `sim_run` became NONE: it no longer probes the host.
- `get_config` became NONE: it no longer returns credentials.
- `swarm_approve` became GOVERNED: the Herald only drafts.
- `test_llm_connection` became FIXED: it needs an absolute flash path.

Independent of class:
- 23 commands send a governed-shape request to a caller-chosen destination
  (Final-Gate egress).
- 17 act on an approval delivered over IPC (Final-Gate approval channel).

**Dispatchers and startup.**
- Every production cognitive loop, the AgentScheduler and hivemind subtasks
  run through `phase0_agent_executor`, which is GOVERNED.
- The team orchestrator and the scheduled executor are FIXED (LLM-only, or
  echo). `ScheduleRunner` is never started.
- The swarm is GOVERNED: its Herald only drafts.
- The MCP host, external tools and the A2A client are GOVERNED with
  caller-chosen destinations.
- `execute_tool`, the nx agent loop and the browser bridge are DENIED.
- None of the 18 startup items is unresolved as a command or dispatcher. The
  configuration load reaches the two filesystem entries described below.

**Sites in the desktop closure** (at the recount → after the C5C repairs →
after the Architect repair):

| Bucket | Filesystem and ambient roots | Process and network |
|---|---:|---:|
| Reachable: GOVERNED | 105 → 105 → 101 | 46 → 46 → 43 |
| Reachable: FIXED | 152 → 150 → 145 | 112 → 108 → 100 |
| Reachable: DENIED | 76 → 87 → 94 | 0 → 4 → 4 |
| Reachable: approved operator assumption | 0 → 0 → 1, see below | 0 |
| Reachable: UNRESOLVED (D/E) | 11 → 2 → 1, see below | 0 → 0 → 0 |
| Latent: GUARDED | 259 → 369 → 371 | 100 → 200 → 211 |
| Latent: UNGUARDED | 110 → 0 → 0 | 100 → 0 → 0 |
| Total | 713 | 358 |

What moved after the recount:

- **Flash model file** (`23489c1d`). At the recount, 9 filesystem sites were
  unresolved: the llama loader and its page-cache warm-up, reached through the
  relative `flash-local` (§10.2). The desktop configures no absolute flash
  path, so it now refuses the provider. Those 9 sites are DENIED, and so are
  the 2 fixed `/sys` and `/proc` reads and the `blockdev` run on the same
  load path.
- **Herald** (`7570d6f2`). The desktop swarm only drafts, so the three X
  request sites are DENIED.
- **Latent APIs** (`fc9a416b`, `aae835f5`, `3e6a0506`). Every latent site is
  now named.
- **Screen observation** (Architect repair A, §10.10). Closing the four
  capture routes moves 9 filesystem and 11 process and network sites:
  - **Kernel, 7 filesystem sites:** the capture reads, the stored screenshot
    and its read, and the audit log. They were FIXED or GOVERNED and are now
    DENIED, because only the kernel screen, input and computer-use actuators
    still reach them, and `Phase0AgentExecutor` refuses those.
  - **Computer-use, 2 filesystem sites:** the capture temp directory and its
    read. They are now latent and guarded.
  - **Process and network, 11 sites:** the six grim, scrot and import
    captures, the kernel `import` and `screencapture` runs, `run_command`,
    and the two vision-model curl helpers. They are now latent and guarded.
    The `which` probes stay reachable through `nx_computer_use_status`.
- **Vault key file** (Architect decision, §10.10). It is now an approved
  operator assumption, counted in its own row. It is not FIXED.

The process and network recount counted a site as latent when its only path
runs through a closed command, the executor or a named latent API. The
filesystem recount counted such a site as DENIED. Neither convention hides a
reachable effect.

Of the two filesystem entries that the C5C repairs left open, one remains
unresolved and one is an approved operator assumption:

1. **`config_user_key`** (`kernel/src/config.rs`): **unresolved (E).**
   `HOME`, `USER`, `USERNAME` and `HOSTNAME` are the configuration-encryption
   key material. The Architect deferred it to the Final Gate (dossier item A),
   so C5C did not change it. No crypto, vault or server redesign was
   authorized.
2. **`EncryptionKey::from_file`** (`kernel/src/crypto.rs`): **approved
   operator trust assumption** (Architect decision). It reads the vault key
   file named by the configuration's `security.key_file`.
   - Under the C5B rule, the path must be absolute and the interface cannot
     change the security section.
   - It is accepted in principle only as an operator-controlled startup
     secret source. It is not frontend or model file selection, not agent
     workspace authority, and security-section editing over IPC is not
     reopened.
   - This is a trust assumption about the operator, not proof of secure
     secret storage. Key-file ownership, permissions, redirection and
     key-source integrity remain Final-Gate review (dossier item E).

**Outside the desktop:**

| Category | Filesystem and ambient | Process and network |
|---|---:|---:|
| SHIPPED non-desktop binaries | 53 | 9 |
| DEVELOPER-only | 43 | 4 |
| BENCHMARK | 28 | 13 |
| TEST-only | 0 | 0 |

- **Shipped:**
  - `nexus-cli`, with the coding-agent and self-improve libraries it links;
  - `crates/nexus-server` (`deploy/Dockerfile`);
  - protocols `nexus-server` (root `Dockerfile`);
  - protocols `nexus-os` (`Makefile`, `install.sh`);
  - `nx` (`nexus-code/Dockerfile`, `nexus-code/install.sh`).

  Dossier item J classifies them. `crates/nexus-server` is a Final-Gate
  blocker.
  - P0-FG1 (validation branch, review pending) withdraws `crates/nexus-server`
    and its `deploy/` recipes. Its binary now reaches no filesystem, process
    or network site.
  - The SHIPPED counts above are the C5C recount. They are not recomputed
    for P0-FG1.
- **Developer-only:** the UI-repair tools, the computer-use harness, the
  swarm healthcheck, the social-poster and coding-agent binaries, and the
  unpackaged agent libraries.
- **Benchmarks:** `benchmarks/` and `benchmarks/conductor-bench`.

**Summary:** after the C5C and Architect repairs:
- **Unresolved.** One reachable filesystem entry remains unresolved (E): the
  configuration key (item A, deferred to the Final Gate). No other reachable
  D/E remains.
- **Operator assumption.** The vault key file is an approved operator trust
  assumption (item E). It is reported as such and is neither unresolved nor
  fixed.
- **Latent.** Unguarded latent is 0.
- **Everything else** is governed, fixed backend state, denied or guarded
  latent.

### 10.9 Final trust-surface guard

`app/src-tauri/src/phase0_surface/tests.rs` is the final trust-surface guard.
`p0_002c5c_final_trust_surface_guard_is_complete` fails if any guard or
registry below is removed or renamed. Each regression class is caught by
these guards; the negative controls are those of the C5C validation report,
each caught by an assertion, never a compile error.

| Regression | Guards | Negative control |
|---|---|---|
| A closed C5A command reopened | `closed_commands_stay_registered_take_no_input_and_only_deny`, `closed_handlers_return_only_their_bounded_reason`, `closure_reasons_are_bounded_and_echo_no_input` | C5A validation |
| A legacy Builder raw path reused | `latent_unsafe_apis_have_no_desktop_production_caller` (legacy Builder needles), `CLOSED_COMMANDS` (LegacyBuilder) | C5A validation |
| An external CLI agent re-enabled | `desktop_sources_start_no_external_cli_agent` | C5A validation |
| Desktop Nexus Code reading working-directory configuration | `desktop_nexus_code_takes_no_configuration_from_the_working_directory`, `nexus-code/tests/phase0_desktop_config.rs` | NC1, NC2 |
| A2A or another delegation bypassing the executor | `p0_002c5c_no_delegation_path_runs_around_the_executor`, `p0_002c5c_a2a_and_agent_actions_are_decided_by_the_production_executor`, kernel `p0_002c5c_a2a_delegation_is_decided_by_the_executor`, `production_agent_executor_refuses_filesystem_and_process_actions` | NC3, NC4, NX11 |
| An unclassified or unbounded production curl site | `p0_002c5b_curl_invocations_keep_caller_values_out_of_curl_syntax` (`CURL_SITES`), `p0_002c5c_every_production_curl_site_is_bounded_in_time_and_size`, `p0_002c5c_benchmark_process_sites_stay_benchmark_only` | NX3 |
| A new ambient root | `desktop_sources_hold_no_ambient_authority_roots`, `p0_002c5b_state_roots_take_no_home_cwd_or_shared_temp_fallback` | C5B validation (identity-home and private-temp controls) |
| An identifier-to-file join without a grammar or stem | `p0_002c5b_identifier_joins_stay_behind_their_grammars` (C5C counts include `stored_file` and `case_exact_entry`) | NX6 |
| A manifest or persisted path becoming authority | `p0_002c5b_serialized_records_choose_no_authority`, `p0_002c5c_persisted_agents_holding_unregistered_authority_are_not_restored` | NX4, NX12 |
| A latent actuator becoming desktop-reachable | `latent_unsafe_apis_have_no_desktop_production_caller` (every recounted latent API named) | NC5 |
| An operator path override becoming IPC- or model-controlled | `p0_002c5c_operator_overrides_stay_launch_configuration`, kernel `p0_002c5c_operator_overrides_are_absolute_or_no_location` | NC8, NC8b |
| Caller text becoming script, markup or a link in the webview | `p0_002c5c_frontend_html_sinks_are_escaped_and_previews_sandboxed`, vitest `safeHtml.test.ts` | NX7, NX8, NX9 |
| Notification text becoming a script | `p0_002c5c_notification_text_never_becomes_a_script` | NC6 |
| A caller-asserted tool autonomy level | `p0_002c5c_tool_calls_run_at_the_registered_agents_autonomy` | NX5 |
| Screen observation, or enabling it, over desktop IPC | `p0_002c5c_no_desktop_route_observes_the_screen`, the closed-command guards, `p0_002c5c_screen_observation_requests_are_denied_and_change_nothing` | A1, A2, A3 |
| An egress entry admitting another scheme, port, host or path | `firewall::egress` tests `p0_002c5c_entries_admit_only_whole_hosts_and_path_segments`, `p0_002c5c_explicit_schemes_and_ports_are_enforced`, `p0_002c5c_legacy_scheme_less_entries_keep_their_documented_meaning`, `p0_002c5c_malformed_or_ambiguous_endpoints_admit_nothing`; web `http_does_not_match_https_allowlist` | B, NX2 |

### 10.10 Architect repair: screen observation and egress transport

After reviewing C5C at `443e24ab`, the Architect required two bounded repairs
and recorded four decisions.

**Decisions.**

1. **Screen observation.** Unbrokered desktop screen observation is
   unavailable in Phase Zero. It is closed, not left as deferred item L.
2. **Egress transport.** Explicit egress scheme and port constraints are
   enforced (repair B). Broader destination, address, DNS and peer policy
   stays unresolved (dossier items B and F).
3. **Vault key file.** `security.key_file` is accepted in principle only as
   an operator-controlled startup secret source. It is not frontend or model
   file selection, not agent workspace authority, and security-section
   editing over IPC is not reopened. This is an approved operator trust
   assumption, not proof of secure secret storage. Ownership, permissions,
   redirection and key-source integrity remain Final-Gate review.
4. **Unchanged.** Ambient configuration-key derivation stays unresolved
   (item A). The unauthenticated shipped server stays a Final-Gate blocker
   (item J1). No crypto, vault or server redesign was authorized.

**Repair A: screen observation.**

| Route | Call path before the repair | After |
|---|---|---|
| `capture_screen` | `runtime::capture_screen` → `trust_security::capture_screen`: enable the engine if disabled (including after the emergency stop), then `capture_and_store_screen` → `capture_screen` → `import`/`screencapture`, a PNG and an audit line under the identity home | Denied unconditionally (`Closure::ScreenObservation`) |
| `analyze_screen` | The same enable, then `capture_and_analyze_screen` → capture, store, and `query_vision_model` (curl to `OLLAMA_URL`) | Denied unconditionally |
| `computer_control_capture_screen` | `engine.capture_screen` when enabled; the emergency stop was not consulted | Denied unconditionally |
| `nx_computer_use_screenshot` | `nexus_computer_use` capture (grim, scrot or import, private temp directory), returned as base64; ignores the engine and the stop | Denied unconditionally; the no-input async handler keeps its name and `Result<NxScreenshot, String>` type |
| `computer_control_toggle(true)` | `engine.enable()` | The enabling branch is refused before any state is read or changed. `toggle(false)` still disables. The command stays open and NONE |

- **Removed from the desktop:** the live capture implementations
  (`trust_security.rs`: `capture_screen`, `analyze_screen`,
  `computer_control_capture_screen`, `desktop_control_workspace`) and the nx
  capture body.
- **Still available:**
  - `computer_control_status`, `get_input_control_status` and
    `computer_control_get_history`;
  - disabling, `stop_computer_action` and the emergency-stop shortcut, which
    sets the kill switch and disables the engine;
  - `nx_computer_use_status`, which only probes readiness. Readiness is not
    a condition for capture.
- **Untouched:** stored screenshots.
- **Inert and open: Omniscience.** `omniscience_enable` and
  `omniscience_get_screen_context` stay open.
  - `ScreenUnderstanding::start()` only sets an in-memory flag.
  - `capture_context` only stores a context it is given, and nothing in
    production supplies one.
  - Neither starts a screen capture, and the classification does not
    authorize future observation wiring. The guard pins both kernel methods
    and both desktop wrappers. It also forbids processes, workers, network
    and capture primitives in the kernel Omniscience module.
- **Dormant library code:** the kernel capture functions, the computer-use
  capture backend, and the refused kernel screen, input and computer-use
  actuators. None of them consults the emergency stop, and no desktop route
  reaches them (§10.5, §10.9).

**Repair B: egress transport.** `endpoint_admits`
(`kernel/src/firewall/egress.rs`) compares a scheme, a normalized host, an
effective port and whole leading path segments. It uses the `url` parser and
`governed_http::http_url`. Dossier item B gives the full rules. In short:
- **Schemed entries.** An entry with a scheme admits only that scheme, on its
  effective port.
- **Legacy entries without a scheme.** They keep the documented compatibility
  meaning: `http` and `https` to their host, on their explicit port or else
  on the request scheme's default port.
- **Rejected input.** Malformed, credential-bearing, ambiguous and
  non-HTTP(S) input admits nothing.
- **Unchanged.** Default deny, rate limiting and auditing.
- **Tests.**
  - The C5C egress test used to list the `https`-to-`http` downgrade as
    admitted. So did the kernel web test `http_matches_https_allowlist`,
    now `http_does_not_match_https_allowlist`.
  - Both now assert the corrected, Architect-approved policy. This is a
    policy correction, not a weakened test.
- **Comments.** The kernel comments at the web and API actuators'
  `check_egress` still describe the old scheme stripping. Those files'
  production code was outside the approved change area.

**Classification after the repair.**

- **Corrected or closed paths (C5C).** Every row of §10.2. That includes the
  four screen-observation routes and the toggle's enabling branch (repair A),
  and the `https`-to-`http` downgrade and port comparison (repair B).
- **Governed paths.** The GOVERNED buckets of §10.8. Interface-chosen
  destinations of governed shape remain open (item B).
- **Approved operator assumptions.** `NEXUS_DB_PATH` and `NEXUS_CONFIG_PATH`
  (absolute, launch-only, C5C), and the vault key file `security.key_file`
  (Architect decision above).
- **Unresolved Final-Gate items.**
  - A: configuration key derivation;
  - J1: the unauthenticated `crates/nexus-server`, a blocker; its
    withdrawal is implemented on the P0-FG1 validation branch, review
    pending;
  - B: destination, address and DNS policy;
  - C: secrets in argv;
  - D: the CSP;
  - E: ownership and integrity of operator-supplied key material;
  - F: peers;
  - G: the approval channel;
  - H: secrets at rest;
  - I: PATH helpers;
  - K: reliability debt.
