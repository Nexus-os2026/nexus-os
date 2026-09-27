# Phase Zero C5 — Authority Inventory and Reachability Closure

Status: P0-002C5A (fail-closed reachable surface closure). C5 is split into
bounded checkpoints:

| Checkpoint | Scope |
|---|---|
| **P0-002C5A** | Make every reachable E1–E5 surface fail closed unless an approved backend-owned authority mechanism already governs it; fix the computer-use EOF approval bug; stabilise the Darwin sealed-spawn fixture; record this inventory and the reachability guard. |
| P0-002C5B | Governed recovery and the remaining straightforward filesystem migrations (deferred items below). |
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

The desktop registered 804 IPC commands. `capabilities/default.json` is core-only and the webview CSP is `null`, so every registered command is callable by any script running in the webview. A frontend value is therefore never evidence of user intent.

## 2. Families

| Family | Meaning | P0-002C5A disposition |
|---|---|---|
| A-BUILDER | Governed Builder workspace (C1–C4) | Kept; guarded by the C2–C4 suites. |
| A-KERNEL-PRIM | Kernel primitives: P0-002A resolver, ResourceLimiter, sealed spawn | Kept. |
| B-* | Fixed app data, fixed system files, fixed helper programs, network helpers, backend random temp | Kept. |
| C-USER-FILE | A raw user-selected path | Closed (E1). |
| C-CLI | CLI and developer binaries (OS user chooses argv/cwd) | Outside the desktop; kept. |
| D-BUILDER-LEGACY | Raw project id/dir/output dir over the governed storage root | Closed (E2). |
| D-MODEL-WRITERS | Conductor / coder writers under a raw or model-chosen root | Closed or refused (E2/E4). |
| D-EXEC | Command text is the process authority | Closed or refused (E3/E4). |
| D-AGENT-ROOT | Agent root is the process working directory | Closed (E4). |
| D-AMBIENT-ROOT | cwd / environment / compile-time roots | Closed (E5). |
| D-TIME-MACHINE | Recorded raw paths replayed by undo/redo | Deferred to C5B; no production producer remains. |
| D-BACKUP-RESTORE | Archive entries choose restore targets | Command closed (E1); safe restore deferred to C5B. |
| D-ID-JOIN | Identifier joined into a fixed root | Deferred to C5B. |
| D-HOMEFALLBACK | Raw `HOME` with a `.`/`/tmp`/`~` fallback | Deferred to C5B. |
| D-SHARED-TEMP | Predictable names in shared temp | Deferred to C5B. |
| D-CURL | curl argument / `file:` / `@file` injection | Deferred to C5B, except the agent path (see §4). |
| D-RECORD | A serialized record chooses an authority file | Deferred to C5B. |
| D-CHECKS | Broken containment checks | All affected commands closed (E1/E2). |
| E-OS-INPUT, E-KEYS | OS input; ambient key material | See §7. |

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

## 5. Deferred to P0-002C5B (still reachable, not migrated here)

- **Time Machine** undo, undo_checkpoint, redo and what_if still replay recorded raw paths.
  - The Conductor, the only producer of file entries, is closed. Production now records agent-state and config entries only.
  - Grant-bound safe restore is C5B work.
- **Backup restore safety.** Entry sanitisation and descriptor-relative restore; the IPC command is closed until then.
- **Identifier joins:**
  - notes_{get,save,delete}, email_{save,delete,disconnect} and provider-selected email token reads;
  - messaging_{connect_platform,send,poll_messages};
  - project_{get,save,delete};
  - agent_memory_save;
  - nx_session_save;
  - download_model (including its Modelfile and model config);
  - nexus_link_send_model;
  - flash_download_model, flash_download_multi, flash_delete_local_model.
- **curl argument and file injection:**
  - api_client_request (`@file` bodies, `file:` URLs);
  - tools_execute (rest_api/webhook URLs);
  - mcp_host_{add_server,connect,call_tool};
  - a2a_discover_agent, a2a_send_task;
  - delete_model (base URL).
- **HOME fallbacks.**
  - Fixed stores fall back to `.`, `/tmp` or a literal `~`: Builder budget, model config, deploy credentials, improvement store, model directory, metering, marketplace, SDK memory, desktop control workspace, the MCP sandbox, the default database path and the config path.
  - Use the validated identity-home policy and fail closed.
- **Predictable shared temp:** screen/window capture files, the benchmark report, and CLI-only temp files.
- **Serialized records as authority:** `consent_policy_path` in agent manifests (IPC create_agent and restored database rows) chooses the agent's own consent policy and approval-queue file.

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
   - `tools_execute` and `mcp_host_*` accept caller URLs.
   - Nexus Link allows every peer when `allowed_peers` is empty.

Also recorded for the final inventory:

- **Operator state-location environment overrides** (`NEXUS_DB_PATH`, `NEXUS_CONFIG_PATH`). They relocate app state; they are not code-bearing.
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

## 9. Explicit non-claims

- No filesystem, process or network sandbox exists. `execute_tool`, the agent executor and the closed commands deny; they do not contain anything.
- No user-selected file access exists: E1 surfaces are unavailable rather than brokered.
- Builder projects, plans and legacy undo/rollback are not durable across restarts.
- C5B items (§5) remain reachable. In particular, identifier-join and curl-injection families still accept caller strings.
- The guard covers the approved surface: registered commands, named latent APIs, named ambient roots and the agent executor. It is not a complete call graph.
- Point-in-time checks only. There is no protection against a hostile same-user process racing filesystem state.
- Nothing here claims Phase Zero is complete.
