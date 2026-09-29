# Phase Zero Final-Gate Dossier

Status: prepared by P0-002C5C (final authority sweep). This dossier collects
the evidence for the Phase Zero Final Gate. It does not decide any item and
does not claim that Phase Zero is complete. Items that C5C repaired are named
as such; everything else is open for the Final Gate.

P0-FG1 updates item J1 only: the withdrawal of `crates/nexus-server` is
implemented on its validation branch, and Architect review is pending.

**P0-FINAL-GATE-CLOSURE (internal work, not approval).** The Architect's
closure mission implements contracts A–K, C5 and DEP on local component
branches, composed into the closure candidate
(`implement/p0-final-gate-closure`). Blocks headed "P0-FINAL-GATE-CLOSURE"
describe that candidate as composed for this update: every workstream except
item D. They are claims for Architect review, checked against the code; the
internal reviews are recorded on the mission's evidence branch. Nothing here
approves an item, accepts a risk, integrates a change, or declares FG1, the
Final Gate or Phase Zero complete. Earlier text stays as it was; where it is
now stale, a labelled correction or note says so. Text marked `[PENDING: …]`
describes work that is not yet composed.

Every location below is a repository path and function at the C5C head,
except where P0-FG1 or P0-FINAL-GATE-CLOSURE is named. An
**untrusted surface** means one of:

- a script running in the desktop webview (the CSP is `null`, item D, so every
  registered IPC command is callable by any script there);
- model output (planner steps, agent actions, generated text);
- remote content (fetched pages, API responses, peers);
- a persisted record (database rows, stored files).

"Immediately exploitable" means reachable from one of these surfaces today,
without a local same-user foothold or an operator mistake.

## Summary

### On the closure candidate (P0-FINAL-GATE-CLOSURE)

No row is closed, complete or approved: each awaits Architect review. The
numbered decision requests are collected at the end of this dossier.

| Item | Topic | Closure candidate | Status |
|---|---|---|---|
| A | Configuration encryption key | A new or changed credential is written only under the operator key `NEXUS_CONFIG_KEY`; legacy files open through an explicit two-key read path; a load never rewrites a file; protection changes are reported and audited | Repaired on the closure candidate (P0-FINAL-GATE-CLOSURE); Architect review pending. Decision requests 9 and 10 |
| B | Egress policy | The 11 IPC commands that sent requests to a caller-chosen destination are closed; the Ollama address is `OLLAMA_URL` or the fixed local default; agent web fetch and caller-destination external tools are refused; SearXNG only at `SEARXNG_URL`; search redirects https only | Repaired on the closure candidate; Architect review pending. No address or DNS policy was added (remaining destinations are backend constants or operator configuration). Decision request 13 |
| C | Secrets in subprocess argv | Reachable credentials leave the command line: four hosted providers post in process with no redirect and total time and size bounds; `perception_init`, the credential-bearing external tools and credentialed MCP servers are closed or refused; remaining credential curl sites are counted, each CLI-only, latent or behind a closed route | Partly repaired on the closure candidate; Architect review pending. Decision request 12 (the in-process transport) |
| E | Operator overrides | The vault key file is validated on the opened file (Linux, macOS) and refused elsewhere; the vault key must open every stored secret before the vault is used; `key_env` other than `NEXUS_ENCRYPTION_KEY` is refused | Repaired on the closure candidate; Architect review pending. Decision request 11 |
| F | Network peers (Nexus Link) | `nexus_link_send_model` closed; the library admits no peer under an empty policy, matches exact IP socket addresses only, resolves no names and requires a shared secret and a key | Repaired on the closure candidate; transfer unavailable (no pairing exists); Architect review pending |
| H | Secrets at rest outside the vault | No new plaintext token or credential persistence: OAuth sign-in and deploy/Supabase storage closed; messaging uses the stored configuration token; API Client collections holding secrets refused; backups skip credential stores | Repaired for new writes on the closure candidate; historical files unchanged; Architect review pending. Decision request 10 |
| I | PATH-resolved helper programs | No unowned `ollama serve` launch; `is_ollama_installed` closed; no program run to report on Ollama from the desktop's own sources; curl children reaped on early errors; in-flight model downloads owned and ended at a normal exit | Partly repaired on the closure candidate: the Nexus Code diagnostics' `which` probes remain [PENDING: S3 final], and helpers still resolve from `PATH`; Architect review pending. Decision requests 1 and 17 |

### At C5C (historical)

| Item | Topic | State at C5C | Final-Gate status |
|---|---|---|---|
| A | Configuration encryption key | Derived from ambient user and host values | Open: design |
| B | Egress policy | Allowlist matching, explicit scheme and effective port enforced (C5C); destinations, addresses and DNS open | Open: policy |
| C | Secrets in subprocess argv | API keys in curl `-H` arguments and one URL path | Open: design |
| D | `null` webview CSP | CSP unchanged; C5C repaired the frontend text-as-markup sinks and stopped returning stored credentials | Open: design |
| E | Operator overrides | Location overrides absolute-only and launch-only (C5C); the vault key file is accepted as an operator trust assumption (Architect); endpoint and key overrides recorded | Open: review |
| F | Network peers (Nexus Link) | Bounded in C5C; any peer allowed by default | Open: policy |
| G | Approval channel | Approvals arrive over webview IPC; the desktop swarm only drafts (C5C) | Open: design |
| H | Secrets at rest outside the vault | OAuth token files are plaintext | Open: design |
| I | PATH-resolved helper programs | Fixed arguments; `ollama serve` detached | Open: review |
| J | Shipped non-desktop binaries | `crates/nexus-server` is unauthenticated on all interfaces | **Blocker**; J1 withdrawal implemented on the P0-FG1 validation branch, review pending |
| K | Reliability signals | Windows `executes_python_code`, retained as debt | Open: debt |
| L | Screen observation from the interface | Unbrokered observation is unavailable: the four capture routes are closed and enabling is refused (Architect decision) | Closed; a brokered mechanism is future work |

Items A–F are the Final-Gate topics named by the Architect. G–L were found or
confirmed by the C5C sweep. Two parts of these items were immediately
exploitable from untrusted surfaces when C5C started, and C5C repaired both:
the egress allowlist matching of item B (from model output) and the frontend
text-as-markup sinks of item D (from model output and remote content).

After review, the Architect required two further repairs:
- Item B: an explicit scheme and port are restrictions.
- Item L: unbrokered screen observation is unavailable.

Both are in place. As the desktop stands at C5C, none of A–I or L is known to
be immediately exploitable from an untrusted surface; the reasoning is given
per item. J concerns a separately deployed server, not the desktop.

## A. Configuration encryption key

- **Where.** `kernel/src/config.rs`, `config_user_key()`.
- **What.** The key that encrypts the Nexus configuration file is
  `SHA-256("nexus-config-key-v1" || NEXUS_CONFIG_KEY)` when the operator sets
  `NEXUS_CONFIG_KEY`. Otherwise it is
  `SHA-256("nexus-config-key-v1" || HOME || USER || USERNAME || HOSTNAME)`.
  These are not secrets: anyone who knows the account name, home path and
  host name can derive the key.
- **Related, unreached.**
  - `kernel/src/hardware_security/sealed_store.rs` derives a sealing secret
    from `/etc/machine-id`, falling back to `/etc/hostname`.
  - The TEE backend keeps key material in a shared-temp directory
    (`kernel/src/hardware_security/tee_backend.rs`, `nexus-tee-keys`).
  - Neither is reachable from the desktop.
- **Builder deploy credentials.**
  - Netlify, Cloudflare and Vercel tokens and the Supabase service key are
    stored in `<home>/.nexus/deploy_credentials.json`
    (`agents/web-builder/src/deploy/credentials.rs`).
  - They are XORed with `SHA-256(HOSTNAME:USER)`, not encrypted, and written
    with default permissions. That contradicts the "encrypted on disk"
    comment at the store's desktop wiring (`app/src-tauri/src/lib.rs`).
  - The legacy deploy commands that used them are closed (C5A).
- **What protects the file today.** It lives under the validated identity
  home with owner-only permissions (`set_restrictive_permissions`, 0o600 on
  Unix). The vault master-key source inside it cannot be changed from the
  interface: `save_config` refuses any change to the security section
  (C5B, `save_config_with` in `app/src-tauri/src/commands/chat_llm.rs`).
- **Exploitability.** Not from an untrusted surface: no open IPC command reads
  the file or its key, and nothing a model or peer controls reaches it. A
  local process of the same user can read the file and derive the key; that
  process already has the user's authority.
- **Decision needed.** Whether the key must come from an OS keystore or a
  user secret, and how existing files migrate. This stays unresolved after
  C5C. The Architect's repair review authorized no crypto, vault or server
  redesign.

**Corrections (P0-FINAL-GATE-CLOSURE) to the C5C text above.**

- The key description omitted three cases. An empty `NEXUS_CONFIG_KEY` gave
  the constant key `SHA-256("nexus-config-key-v1")`. A value that is not valid
  UTF-8 was ignored in favour of the ambient derivation. With none of the
  ambient values set, the key was that same constant.
- "The legacy deploy commands that used them are closed (C5A)" was
  incomplete. C5A closed `builder_deploy`, but `builder_deploy_store_credentials`
  and `builder_backend_connect` (the Supabase service key) stayed open and
  wrote the XOR store until P0-FINAL-GATE-CLOSURE (item H).
  `builder_deploy_check_credentials` and `builder_deploy_list_sites` still
  read it.
- "What protects the file today": the configuration writer no longer calls
  `set_restrictive_permissions`. It creates each new file owner-only before
  writing (see below).

**Repaired in P0-FINAL-GATE-CLOSURE (contract A; Architect review pending).**

- **Where.** `kernel/src/config.rs`: `ConfigKeyMaterial`,
  `save_config_checked_to_path`, `load_config_from_path_with`,
  `credential_fields`. The interface writer is `save_config_with` and
  `write_keeping_stored_credentials`
  (`app/src-tauri/src/commands/chat_llm.rs`). The CLI setup library
  (`cli/src/setup.rs`) uses the same kernel writer; its binary is withdrawn
  (item J).
- **Operator key.** A new or changed credential is written only under the
  operator key: `NEXUS_CONFIG_KEY` from the launch environment, when it is
  valid UTF-8, not empty or whitespace-only, and does not derive the ambient
  key (the account and host names run together). The credentials are the 17
  fields of `credential_fields` and every provider `api_key`. Otherwise the
  save is refused and nothing is written (`configuration_key_required`,
  `configuration_key_is_ambient`). Clearing a credential, or a save that adds
  or changes none, needs no key.
- **Key quality (what is enforced).** Presence, non-blank content and not the
  ambient derivation; nothing else. The key is
  `SHA-256("nexus-config-key-v1" || value)` with no salt and no stretching, so
  a short or guessable value gives a correspondingly weak key.
- **Legacy read path (explicit and tested).** A version 1 envelope is tried
  with at most two keys, and the AES-GCM tag decides: the legacy explicit
  derivation (`NEXUS_CONFIG_KEY` as any valid UTF-8 value, even empty) and the
  ambient derivation (`HOME`, `USER`, `USERNAME`, `HOSTNAME`, each when set).
  No other key is tried. Both derivations are byte-for-byte those of earlier
  builds (`p0_fg_a_legacy_key_derivations_are_unchanged`, independent
  vectors). Legacy plaintext is read as it is.
- **No silent migration.**
  - A load never rewrites an existing file. Before, a load re-encrypted a
    plaintext file and replaced an empty file with the default.
  - A missing file is still created with the default configuration, which
    holds no credential. An empty or whitespace-only file is refused, not
    replaced.
  - A save keeps the key that opened the file, with two exceptions. A
    credential save moves the file to the operator key
    (`rekeyed_to_operator_key`). The first explicit save of a legacy
    plaintext file encrypts it, under the operator key when one is set,
    otherwise under the legacy ambient key (`encrypted_legacy_plaintext`).
  - Every save reports such a protection change: one bounded line on standard
    error, and the desktop's recorder, which appends a `save_config` audit
    event by reason class (`install_protection_recorder`, installed in
    `AppState::new` before the credential migration can re-save the file).
    The interface save audits its own outcome.
  - A file on disk that does not open (unreadable, empty, malformed,
    undecryptable or another envelope) is never overwritten
    (`existing_configuration_unreadable`).
- **Writes.** A new owner-only file (0600 on Unix) with a unique name, synced,
  then renamed over the configuration. On Windows it keeps the directory's
  inherited access. Load errors carry no configuration text.
- **Interface.** `save_config` keeps the C5B rule that an existing, loadable
  security baseline is required, and refuses a changed `llm.ollama_url`
  (`ollama_endpoint_backend_owned`, item B).
- **Coverage.** A kernel test walks every configuration field and requires
  each field named like a secret (key, token, secret, password) to be a listed
  credential, a provider key, or on an explicit list of non-secret fields.
- **Vault master key (related).** `EncryptionKey::from_env`
  (`kernel/src/crypto.rs`) refuses an unset, empty or whitespace-only
  `NEXUS_ENCRYPTION_KEY`, which gave a constant key. `from_config` refuses a
  `key_env` other than `NEXUS_ENCRYPTION_KEY`, which it used to ignore.
- **Consequence.** Without a usable `NEXUS_CONFIG_KEY`, no new credential can
  be saved from the desktop's settings, including messaging bot tokens. Once
  a credential save moves a file to the operator key, the file opens only
  while the same key is set.
- **Tests and guards.** `p0_fg_a_*` in `kernel/src/config.rs`,
  `kernel/tests/phase0_config_key.rs` (unset, empty, whitespace-only and, on
  Unix, non-UTF-8 values), the desktop's
  `p0_fg_a_interface_credential_edits_need_the_operator_key`, and the
  `phase0_surface/fg_secrets/tests.rs` guards.
- **Non-claims.**
  - Credentials already under the ambient key stay under it until a
    credential save. Historical files are not re-encrypted.
  - A same-user process can still read or replace the file, derive the
    ambient key of a legacy file, or change the launch environment.
  - No key strength is measured.
  - Key material outside the configuration file is outside this contract.
- **Decision requested.** Requests 9 and 10.

## B. Egress policy

**Repaired in C5C: allowlist matching.**

- `kernel/src/firewall/egress.rs`, `endpoint_admits(entry, url)`.
- Every allowlist check now requires an entry to match whole host and path
  segments: the web actuator (`kernel/src/actuators/web.rs`, `check_egress`),
  the API actuator (`kernel/src/actuators/api.rs`), the egress governor
  (`EgressGovernor::check_egress`), and the latent browser actuator and
  content pipeline.
- Before C5C, `starts_with` matching let `https://example.com` admit
  `https://example.com.evil.net`, `https://example.community` and other ports.
  That was immediately exploitable from model output: an agent's web-fetch
  URL is model-chosen, and a prompt-injected model could reach a look-alike
  host past the agent's allowlist.
- An agent's web fetch no longer follows redirects (`curl_get(url, false)`):
  the allowlist admitted the requested URL, not its target.

**Repaired in C5C: shape and bounds.**

- The theme-extraction fetch (`agents/web-builder/src/theme_extract.rs`,
  `extract_theme_from_url`) now uses `governed_http::http_url`, allows only
  https redirects and reads at most 4 MiB.
- The Ollama health probe (`connectors/llm/src/providers/ollama.rs`,
  `health_check`) needs a governed http(s) URL.
- Every production curl site has a time bound and a response-size bound
  (guard `p0_002c5c_every_production_curl_site_is_bounded_in_time_and_size`).
- Credential-bearing external tools take caller values into URLs only
  through identifier grammars (`crates/nexus-external-tools/src/tools/mod.rs`).

**Open: destinations are chosen by the caller.** Each of these sends a request
of governed shape to a destination the interface chooses:

| Surface | Location | Destination |
|---|---|---|
| `api_client_request` | `app/src-tauri/src/commands/apps.rs` | any http(s) URL |
| `a2a_discover_agent`, `a2a_send_task`, `a2a_get_task_status`, `a2a_cancel_task` | `app/src-tauri/src/commands/governance.rs`, kernel `A2aClient` | any http(s) agent URL |
| `a2a_crate_*` | `app/src-tauri/src/commands/crate_bridges.rs`, `crates/nexus-a2a` | any agent URL |
| `mcp_host_*` | `protocols/src/mcp_client.rs` (stdio refused) | any http(s) MCP server |
| `tools_execute` (`rest_api`, `webhook`) | `crates/nexus-external-tools` | any http(s) URL; a substring denylist only |
| `nexus_link_send_model` | `connectors/llm/src/nexus_link.rs` | any `host:port` (item F) |
| `builder_theme_extract_from_url` | `agents/web-builder/src/theme_extract.rs` | any https URL |
| Ollama family (`check_ollama`, `ensure_ollama`, chat, delete) | `app/src-tauri/src/commands/chat_llm.rs` | caller `base_url` |
| `save_config`, `run_setup_wizard` | persist `llm.ollama_url` | all later planner and agent LLM traffic |
| Agent web fetch | `kernel/src/actuators/web.rs` | the agent manifest's `allowed_endpoints`, which the interface chooses at creation |

**Repaired in C5C (Architect repair B): explicit transport restrictions.**

`endpoint_admits` (`kernel/src/firewall/egress.rs`) now compares a request and
an allowlist entry as a scheme, a normalized host, an effective port and whole
leading path segments. It uses the existing `url` parser and
`governed_http::http_url`. The rules:

- **Explicit scheme.** An entry with a scheme admits only that scheme. For
  example, `https://example.test/v1` never admits `http://example.test/v1`,
  and an `http` entry never admits `https`.
- **Effective port.** It must match. `https://host` and `https://host:443`
  are the same entry, and another port is denied.
- **Host.** Hosts are compared after normalization (case, IDNA, IPv4 forms),
  never as text prefixes. `/v1` never admits `/v11`, and paths are
  case-sensitive.
- **Legacy entries without a scheme.** They keep a documented compatibility
  meaning. `host[:port][/path]` admits `http` and `https` to that host: on its
  explicit port, or else on the request scheme's default port. An explicit
  `:80` stays port 80. The rule applies only to entries written without a
  scheme. Each entry keeps its own meaning, so a legacy entry never changes
  what a schemed entry admits. A legacy entry for the same host and path does
  admit `http` by its own meaning; that is a separate entry the operator or
  interface listed.
- **Rejected input.** Malformed, credential-bearing, ambiguous or
  non-HTTP(S) entries and requests admit nothing. That covers user
  information, a query, fragment or backslash in an entry, `.` or `..`
  segments, a non-numeric or empty port, and another scheme.
- **Unchanged.** Default deny for an absent or empty policy, rate limiting
  and auditing are unchanged.

The kernel comments at the web and API actuators' `check_egress` still
describe the earlier scheme stripping. Those files were outside the approved
change area, and `endpoint_admits` alone decides the behaviour.

- **Also open.**
  - There is no address policy: loopback, private and link-local
    destinations are reachable wherever a caller chooses the destination
    (the external-tools denylist is substring-based).
  - DNS results are not pinned between a check and the request.
- **Exploitability.** A webview script can already send requests of governed
  shape to any host through `api_client_request`, so the rows above add no
  authority beyond that surface. None of them lets a caller choose a file, a
  program, a curl option or another protocol (C5B). The allowlist-matching
  defect, the one model-reachable bypass, is repaired. Since the Architect
  repair, a model can no longer downgrade an agent's `https` endpoint to
  plain `http`.
- **Decision needed.** A destination policy for interface-chosen requests, an
  address policy, DNS pinning and peer policy (item F). These remain
  unresolved; no general egress redesign was authorized.

**Corrections (P0-FINAL-GATE-CLOSURE) to the C5C text above.**

- The exploitability note was wrong: the rows above did add authority beyond
  `api_client_request`. `save_config` and `run_setup_wizard` persisted
  `llm.ollama_url`, and when `OLLAMA_URL` was unset the backend sent all later
  planner, agent and chat traffic for Ollama to that persisted address
  (`build_provider_config`). And `api_client_request` read back responses that
  a webview script cannot read cross-origin.
- The comment at the web actuator's `check_egress` now describes
  `endpoint_admits` (`kernel/src/actuators/web.rs`). The API actuator's
  comment (`kernel/src/actuators/api.rs`, `check_egress`) still describes the
  old scheme stripping; `endpoint_admits` alone decides the behaviour.

**Repaired in P0-FINAL-GATE-CLOSURE (contract B; Architect review
pending).** A syntactically valid URL, DNS answer or `host:port` is not an
egress grant.

- **Caller-chosen destinations closed.** Eleven commands are closed with
  `Closure::NetworkDestination`; each handler takes no input and only denies,
  and the implementations are removed: `api_client_request`;
  `a2a_discover_agent`, `a2a_send_task`, `a2a_get_task_status`,
  `a2a_cancel_task`; `a2a_crate_send_task`, `a2a_crate_get_task`,
  `a2a_crate_discover_agent`; `mcp_host_connect`, `mcp_host_call_tool`;
  `builder_theme_extract_from_url`. `nexus_link_send_model` is closed too
  (item F). Commands that send nothing stay (`a2a_known_agents`, the MCP
  server list, saved API Client collections).
- **External tools.** `tools_execute` refuses `rest_api`, `webhook` and
  `file_storage` (a caller-chosen URL or bucket host) at every autonomy level
  (`crates/nexus-external-tools/src/execution.rs`, `phase0_refusal`). The
  order is: tool lookup, the level check, the Phase Zero refusal, then
  availability, so the refusal is the same whether or not the tool's token is
  set, and comes before any credential read or request. The substring URL
  denylist is removed; `127.1`, `0x7f000001` and `[::1]` passed it.
- **Agent web fetch.** `Phase0AgentExecutor` refuses an http(s) `WebFetch`
  with `Closure::NetworkDestination` (`phase0_agent_action_closure`,
  `app/src-tauri/src/commands/cognitive.rs`): the agent's
  `allowed_endpoints` came from `create_agent` or a stored record, neither of
  which is an egress grant. Web search stays.
- **Ollama address.** It is backend configuration: the operator's
  `OLLAMA_URL` (read with `var_os`), or else the fixed `http://localhost:11434`
  (`authorized_ollama_base_url`, `chat_llm.rs`). A set but unusable value makes
  Ollama unavailable; nothing falls back to the default.
  - The persisted `llm.ollama_url` and `ollama.base_url` are never used as a
    destination: `build_provider_config` (`commands/agents.rs`) ignores the
    configuration for it, and `save_config` refuses a changed
    `llm.ollama_url`.
  - A caller `base_url` (`check_ollama`, `pull_ollama_model`, `ensure_ollama`,
    `chat_with_ollama`, `delete_model`, `run_setup_wizard`) must normalize to
    exactly the authorized address, or it is refused
    (`Closure::NetworkDestination`) before anything connects.
    `run_setup_wizard` persists only the authorized address.
  - After a model download, the model is registered only at the authorized
    address, and not at all when Ollama is unavailable
    (`register_downloaded_model_with_ollama`, `model_hub.rs`).
  - A pull must name a model of Ollama's default registry; `hf.co` and other
    registry hosts are refused. The Ollama curl helpers follow no redirect.
- **SearXNG and search.** SearXNG is used only at the operator's
  `SEARXNG_URL`, with no guessed local default (`searxng_base`,
  `kernel/src/actuators/web.rs`). Search follows redirects only to https
  addresses (`--proto-redir =https`); an agent fetch follows none.
- **Credentialed providers follow no redirect** (item C).
- **Remaining destinations** are compile-time provider endpoints, fixed
  service hosts and operator launch configuration (item E).
- **Guards.** `phase0_surface/fg_egress/tests.rs`: the caller-destination
  closures, the absence of the destination clients from desktop sources
  (with 15 new latent-API needles), agent fetch, tool calls, the Ollama
  address rules, the persisted address and model registration
  (`p0_fg_model_registration_uses_the_authorized_ollama_address`);
  `p0_fg_searxng_needs_an_operator_address`; the external-tools refusal
  tests. The guards read LF and CRLF sources alike.
- **Non-claims.**
  - No address or DNS policy was added. The remaining destinations are
    backend constants or operator configuration, and their DNS answers are
    not pinned.
  - The webview can still send requests itself; the CSP of item D is the
    control there [PENDING: S1 final].
  - Operator endpoints are trusted as configured: a non-loopback
    `OLLAMA_URL` is accepted.
  - `OllamaProvider::health_check` probes `127.0.0.1:11434` when the
    authorized address names a host rather than an IP address, so
    `ensure_ollama` can report a local service while the operator's host is
    down. Requests still go to the operator's host.
- **Decision requested.** Request 13.

## C. Secrets in subprocess argv

Credentials passed as curl arguments are visible in the process table (for
example `/proc/<pid>/cmdline`) while the request runs. On Linux without
`hidepid`, that includes other local users.

| Location | Secret | Reachable from the desktop |
|---|---|---|
| `connectors/llm/src/providers/mod.rs`, `curl_post_json_with_timeout` | provider API keys (`x-api-key`, `Authorization: Bearer`) | yes: every hosted-provider LLM call |
| `connectors/core/src/validation.rs` | Anthropic key (`x-api-key`), Brave key (`X-Subscription-Token`), Telegram bot token in the URL path | yes: key validation |
| `connectors/web/src/search.rs` | Brave key (`X-Subscription-Token`) | yes, when Brave is configured |
| `protocols/src/mcp_client.rs`, `send_http` | MCP bearer token | yes: `mcp_host_*` with bearer auth |
| `crates/nexus-perception/src/vision.rs`, `call_api` | Groq or NIM key, supplied by the interface to `perception_init_provider` | yes: the `perception_*` commands |
| `kernel/src/actuators/image_gen.rs`, `kernel/src/actuators/tts.rs` | provider keys | no: the actuators are refused by the Phase Zero executor |
| `crates/nexus-mcp/src/tools.rs` (`nexus_github`) | `GITHUB_TOKEN` | no: `mcp2_server_handle` is closed; `crates/nexus-server` reached it until its withdrawal (item J, P0-FG1, review pending) |
| `crates/nexus-capability-measurement` (NIM, OpenRouter clients) | provider keys | no: the `cm_*` commands are closed |

- **Exploitability.** Not from an untrusted surface of the desktop: a webview
  script or a model cannot read the process table. The exposure is to other
  processes on the host.
- **Decision needed.** Pass credentials on stdin (for example curl's
  `-H @-`), or move these calls to an in-process HTTP client. The C5B curl
  guard forbids `-K` and `--config`, so a stdin form needs a guard update.

**Corrections (P0-FINAL-GATE-CLOSURE) to the C5C table above.**

- First row: only OpenAI, DeepSeek, Gemini and NVIDIA sent their key through
  curl (`Authorization: Bearer`). Anthropic (`x-api-key`), Cohere and the
  OpenAI-compatible providers already sent theirs from the process.
- `connectors/core/src/validation.rs` was reached only by the `nexus-cli`
  setup flow, not by the desktop; that binary is now withdrawn (item J).
- `connectors/web/src/search.rs` is latent: only the social-poster pipeline's
  real search step builds that connector, and the desktop never runs it.
- The perception command is `perception_init`.
- "The `cm_*` commands are closed" was wrong for `cm_run_ab_validation`, which
  was open (C5, below).
- Missing row: `tools_execute`'s `github`, `slack` and `jira` tools put the
  operator's token on curl's command line as an `Authorization` header
  (`crates/nexus-external-tools/src/adapter.rs`).

**Repaired in P0-FINAL-GATE-CLOSURE (contract C, argv part; Architect review
pending).**

- **Hosted providers.** OpenAI, DeepSeek, Gemini and NVIDIA send from the
  process (`post_json_in_process`, `connectors/llm/src/providers/mod.rs`),
  on a thread of its own:
  - no redirect is followed, so a key reaches only its endpoint;
  - the same timeouts as before bound the whole exchange (20 s; NVIDIA NIM
    120 s, or 300 s for very large models), from connecting until the body
    is read;
  - at most 32 MiB of response is read;
  - credential header values are marked sensitive, and errors name neither
    the URL nor a header;
  - certificate verification is always on.
- **TLS stack (stated precisely).** A client built without an explicit TLS
  choice follows the build's unified reqwest features. In the desktop build,
  `nexus-auth` and `nexus-code` enable reqwest's default features, so the
  platform TLS library with the operating system's trust store is used, and
  the system and environment proxy settings apply. The connector crate built
  alone uses rustls with bundled roots (`credential_client` documentation).
- **No curl credential.** The curl POST helper refuses a credential header
  (`authorization`, `proxy-authorization`, `x-api-key`, `api-key`,
  `x-goog-api-key`, `x-subscription-token`, `cookie`) before a command
  exists.
- **Other credentialed clients.** The Claude, Cohere and OpenAI-compatible
  providers (Groq, Mistral, Together, Fireworks, Perplexity, OpenRouter) now
  build their clients with `credential_client`: no redirect is followed; a
  redirect is reported as a failed request.
- **Closed or refused routes.** `perception_init` is closed
  (`Closure::CredentialTransport`), so no key reaches the perception client.
  `tools_execute` refuses `github`, `slack` and `jira` at every level, before
  any token is read (item B's order). The MCP client refuses a server with a
  bearer token or API key before a command is built
  (`protocols/src/mcp_client.rs`, `http_command`). `api_client_request` is
  closed (item B).
- **Remaining credential curl sites** are counted, each with its reason, in
  `CREDENTIAL_CURL_SITES` (`phase0_surface/fg_egress/tests.rs`): key
  validation (CLI only), the refusal list in the provider helper, the latent
  search connector, the capability-measurement clients (their only desktop
  route is closed, C5 below), the GitHub MCP tool (only through the closed
  `mcp2_server_handle`), perception (closed), and the image and speech
  actuators (refused by the Phase Zero executor).
- **Guards and tests.** `p0_fg_no_reachable_credential_reaches_a_curl_command_line`,
  `p0_fg_perception_takes_no_key_and_sends_nothing` and
  `p0_fg_messaging_errors_never_carry_the_bot_token` (`fg_egress`); in
  `connectors/llm/src/providers/mod.rs`,
  `p0_fg_credential_headers_never_reach_a_process_command_line`,
  `p0_fg_credentialed_posts_follow_no_redirect`,
  `p0_fg_credentialed_reqwest_providers_follow_no_redirect`,
  `p0_fg_credentialed_posts_read_a_bounded_response` and
  `p0_fg_credentialed_posts_are_bounded_in_total_time`;
  `p0_fg_mcp_credentials_never_reach_a_process_command_line`;
  `p0_fg_operator_credential_tools_are_refused`.
- **Non-claims.**
  - Keys set by `save_provider_api_key` are still in the process environment,
    which child processes inherit (not on their command line).
  - The CREDENTIAL_CURL_SITES guard sees only literal curl sites and a fixed
    list of credential markers.
  - Response size caps apply to the in-process POST; the other credentialed
    reqwest providers read their responses without one.
  - The desktop swarm's Anthropic client (`crates/nexus-swarm`) still follows
    redirects with its key [PENDING: S3 final].
- **Decision requested.** Request 12.

## D. `null` webview CSP

- **Where.** `app/src-tauri/tauri.conf.json`, `app.security.csp: null`.
  `app/src-tauri/capabilities/default.json` grants core permissions to the
  `main` window only.
- **Effect.** Any script that runs in the app's origin can call every
  registered IPC command. Phase Zero therefore treats every IPC argument as
  untrusted: C5A–C5C close or govern commands on that basis, and the
  reachability guard (`app/src-tauri/src/phase0_surface/tests.rs`) keeps them
  that way.

**Repaired in C5C: text rendered as markup.** The frontend put text it does
not control into HTML:

- The chat (`app/src/pages/AiChatHub.tsx`) rendered every model reply with
  `dangerouslySetInnerHTML`, escaping only the inside of code fences.
- The notes preview (`app/src/pages/NotesApp.tsx`) did the same with note
  text and passed link and image URLs through unchecked.
- The design preview (`app/src/pages/DesignStudio.tsx`) rendered its markup
  in an unsandboxed srcdoc iframe, which shares the app's origin, or through
  `innerHTML`.
- The crash screen (`app/src/main.tsx`) put the error message into
  `innerHTML`.
- Links and `window.open` targets taken from records (knowledge entries,
  deploy results, marketplace entries) were used as given. React 18 renders
  `javascript:` URLs.

A model reply carrying `<img src=x onerror=…>` (for example repeated from a
fetched page by a prompt-injected model) would therefore have run script
with every IPC command. That was immediately exploitable from model output
and remote content. `app/src/lib/safeHtml.ts` now escapes text before adding
markup (`renderChatContent`, `renderNoteMarkdown`, `escapeHtml`) and admits
only http(s) link and window targets (`safeHttpUrl`). The design preview has
an empty sandbox. Vitest cases parse hostile renderings. The guard
`p0_002c5c_frontend_html_sinks_are_escaped_and_previews_sandboxed` counts
every remaining raw-HTML sink with the reason it is safe, and requires
checked link targets and sandboxed srcdoc iframes.

**Repaired in C5C: stored credentials in the interface.** `get_config`
returned the whole decrypted configuration, including every credential it
still holds in plaintext, so a script in the webview could read them. It now
returns each stored credential as a fixed placeholder; a save that returns
the placeholder keeps the stored value (`redacted_config`,
`save_keeping_stored_credentials`, `app/src-tauri/src/commands/chat_llm.rs`).

**Open.**

- The Builder and web previews run generated script in srcdoc frames
  sandboxed with `allow-scripts` only, so each frame has an opaque origin.
  These are `app/src/components/WebPreview.tsx`,
  `app/src/components/builder/VisualEditor.tsx`,
  `app/src/components/browser/BuildMode.tsx` and
  `app/src/pages/NexusBuilder.tsx`. C5C did not verify on every platform
  whether Tauri's IPC bridge (Tauri 2.10, wry 0.54) is unreachable from such
  a frame.
- The CSP itself stays `null`.

- **Decision needed.** A restrictive CSP (script sources, `connect-src`,
  `frame-src`), a check of IPC reachability from sandboxed frames, and the
  frontend changes a CSP requires.

## E. Operator overrides

Launch configuration: environment variables the operator sets before the
desktop starts. No IPC command sets any of them. The only production
environment write is `save_provider_api_key`, whose seven variable names are
all `*_API_KEY` (guard
`p0_002c5c_operator_overrides_stay_launch_configuration`).

**State locations (C5C).**

- `NEXUS_DB_PATH`: `kernel/src/identity_home.rs`, `nexus_db_path()`.
- `NEXUS_CONFIG_PATH`: `kernel/src/config.rs`, `config_path()`.
- Both pass the value to `identity_home::operator_override`, which accepts
  only a non-empty absolute path. Anything else is "no location", never the
  working directory. A rejected database override leaves the desktop on an
  in-memory database, and a rejected config override makes the config
  unavailable.
- The legacy-database cleanup (`maybe_cleanup_legacy_agent_db`,
  `app/src-tauri/src/commands/chat_llm.rs`) now reads `NEXUS_DB_PATH` with
  `var_os` too. Before C5C, a non-UTF-8 absolute override selected the
  operator's database but hid it from the cleanup, which deleted that
  database.
- An absolute override may still contain `..` or name a UNC share. A database
  override also moves `schedules.json` beside it.
- **Vault key file** (`security.key_file`, read by
  `EncryptionKey::from_file`, `kernel/src/crypto.rs`, at startup when the
  vault key source is `file`).
  - It is a location stored in the configuration file, not an environment
    override.
  - C5B requires it to be absolute, and the interface cannot change the
    security section.
  - **Architect decision: an approved operator trust assumption.** The key
    file is accepted in principle only as an operator-controlled startup
    secret source. It is not frontend or model file selection, not agent
    workspace authority, and security-section editing over IPC is not
    reopened.
  - The decision is a trust assumption about the operator, not proof of
    secure secret storage. Key-file ownership, permissions, redirection and
    key-source integrity remain Final-Gate review.
  - The inventory counts it separately, as an approved operator assumption.
    It is neither unresolved nor fixed.

**Endpoints.** Each sets where requests go:

- `OLLAMA_URL` (it received screen captures for `analyze_screen` until
  Architect repair A closed that command);
- `SEARXNG_URL`;
- the provider base URLs `ANTHROPIC_URL`, `OPENAI_URL`, `DEEPSEEK_URL`,
  `GEMINI_URL`, `GROQ_URL`, `MISTRAL_URL`, `COHERE_URL`, `FIREWORKS_URL`,
  `TOGETHER_URL`, `PERPLEXITY_URL`, `OPENROUTER_URL`, `NVIDIA_NIM_URL`;
- `STABLE_DIFFUSION_WEBUI_URL`, `JIRA_BASE_URL`, `S3_ENDPOINT`, `SMTP_HOST`;
- `NEXUS_WEBHOOK_OUTBOUND_URL`, `NEXUS_MATRIX_HOMESERVER`;
- `NEXUS_TELEGRAM_VALIDATE_BASE_URL`, `NEXUS_ANTHROPIC_VALIDATE_URL`,
  `NEXUS_BRAVE_VALIDATE_URL`.

**Key material and flags.**

- `NEXUS_CONFIG_KEY` (item A) and `NEXUS_ENCRYPTION_KEY`
  (`kernel/src/crypto.rs`);
- provider API keys;
- `NEXUS_MESSAGING_ENABLED`, `NEXUS_MAX_L6_AGENTS`;
- `LLM_PROVIDER` and `FLASH_MODEL_PATH` (`connectors/llm/src/gateway.rs`).
  Since C5C the flash provider takes its model file only from an absolute
  path. Before, it defaulted to `flash-local`, relative to the working
  directory, which `test_llm_connection` could reach.

- **Exploitability.** None of these can be set from an untrusted surface.
  They are operator authority by definition.
- **Decision needed.** Which of these survive into a shipped configuration
  model, and whether endpoint overrides need the egress policy of item B.

**Repaired in P0-FINAL-GATE-CLOSURE (contract E: the vault key source;
Architect review pending).** The key file stays an operator-controlled
startup source only; security-section editing and frontend key-file
selection stay closed.

- **Where.** `EncryptionKey::from_file`, `key_file::open`, `key_file::read`
  and `check_opened_key_file` (`kernel/src/crypto.rs`); `verify_vault_key`
  and `run_migrations` (`kernel/src/startup/mod.rs`).
- **Validated on what is read.** The file is opened once, with `O_NOFOLLOW`,
  `O_NONBLOCK` and `O_NOCTTY`, and every check applies to that open
  descriptor, never to the path resolved again:
  - a regular file owned by the effective user, with no access for group or
    others, of 1 to 4,096 bytes, not all whitespace;
  - after a bounded read, the file is unchanged (device, inode, size,
    modification time, mode and owner) and the bytes read equal its size.
- **Derivation unchanged.** Exactly 32 bytes are the raw key; other contents
  are hashed once with SHA-256.
- **Refused.** A symbolic link as the last path component; directories,
  FIFOs and devices; a file owned by another user or accessible to group or
  others; an empty, whitespace-only or oversized file; a file that changes
  during the read; and any key file on a platform other than Linux and macOS
  (Windows included), where these checks are not implemented (use
  `key_source = "env"`). Reasons are bounded and never name the file.
- **Stored-row check.** Before the vault is used, the master key must open
  every secret stored in the scopes `llm`, `social`, `messaging.whatsapp`,
  `messaging.matrix`, `http` and `auth.oidc`. A failed authenticated
  decryption reports a key that does not open the vault; a malformed row
  reports a damaged secret; a storage error reports an unreadable vault. In
  each case the vault is not used, and nothing is migrated or written. An
  empty vault accepts its first key.
- **Key material.** `NEXUS_ENCRYPTION_KEY` must be set and not empty or
  whitespace-only, and `key_env` must name it (item A).
- **Consequence.** A symlinked key file (for example a projected secret
  mount), one accessible to others, an empty one, a key file on another
  platform, or a key that does not open the stored rows leaves the vault
  unavailable until the operator fixes it. Startup then prints
  "secrets vault unavailable: …; vault-backed operations are refused", and
  `save_api_key` refuses the six vault-backed providers ("vault not
  initialized"); `groq` is written only to the process environment, as
  before. Nexus never modifies the key file or re-encrypts the vault.
- **Tests.** `p0_fg_e_*` in `kernel/src/crypto.rs` (Linux and macOS: symlink,
  swap after open, change during the read, FIFO without blocking, device,
  directory, sizes, modes, owner; elsewhere: refusal) and in
  `kernel/src/startup/mod.rs`; guard
  `p0_fg_e_vault_key_sources_are_validated_on_what_is_read`.
- **Non-claims.**
  - Intermediate directories, their permissions and macOS extended ACLs are
    not examined.
  - A same-user process can still replace the key file between startups.
  - Only the six scopes above are verified.
  - The Architect's operator-trust decision above still applies: this is
    validation of an operator source, not proof of secure secret storage.
- **Decision requested.** Request 11.

**Update and correction (P0-FINAL-GATE-CLOSURE, item B) to the endpoint list
above.**

- `OLLAMA_URL` is now the only Ollama address authority, with the fixed
  `http://localhost:11434` when it is unset. A set but unusable value makes
  Ollama unavailable (item B).
- `SEARXNG_URL` is required for SearXNG; there is no default instance.
- Correction: the twelve provider base URLs (`ANTHROPIC_URL` …
  `NVIDIA_NIM_URL`) did not set where desktop requests go. Only the
  providers' `from_env` constructors read them, and no production code calls
  those; `select_provider` (`connectors/llm/src/gateway.rs`) builds each
  hosted provider with its fixed endpoint.

## F. Network peers (Nexus Link)

- **Where.** `connectors/llm/src/nexus_link.rs`; IPC `nexus_link_send_model`
  (`app/src-tauri/src/lib.rs`).
- **Repaired in C5C.**
  - Peers are connected with a 10-second connect timeout and read and write
    timeouts (`connect_peer`).
  - A peer's length prefix above 16 MiB is refused before allocation
    (`read_message`). Before C5C a peer could force an allocation of up to
    4 GiB.
- **Open.**
  - `check_peer_allowed`: an empty `allowed_peers` list admits every peer, and
    the desktop never sets the list.
  - Sending does not check `sharing_enabled`.
  - No shared secret or encryption key is set by the desktop, so the peer
    handshake and payload are unauthenticated and in plaintext.
  - `receive_model` is latent (guard needle `receive_model(`).
- **Exploitability.** The interface can send a model file (C5B: only a
  regular file beneath the models directory, spelled exactly) to any
  `host:port`. Those files are downloaded model weights, not user documents.
  The interface can already send data anywhere through `api_client_request`.
- **Decision needed.** A default peer policy (deny by default, pairing), and
  authentication and encryption for transfers.

**Repaired in P0-FINAL-GATE-CLOSURE (contract F; Architect review
pending).**

- **Command.** `nexus_link_send_model` is closed (`Closure::PeerTransfer`): it
  takes no input and only denies.
- **Library policy** (`connectors/llm/src/nexus_link.rs`, `send_model`,
  `check_peer_allowed`, `require_authenticated_transport`):
  - an empty peer policy admits no peer;
  - a peer is named by its IP socket address and must equal a policy entry,
    compared as socket addresses; a host name is never resolved;
  - a transfer needs both a shared secret and an encryption key;
  - each check runs before any connection, after the C5B model-file check.
- **Consequence.** The desktop configures no peer, secret or key, and Phase
  Zero builds no pairing, so model transfer is unavailable.
- **Tests.** `p0_fg_an_empty_peer_policy_admits_no_peer`,
  `p0_fg_the_peer_policy_admits_only_exact_socket_addresses`,
  `p0_fg_an_unauthenticated_transfer_is_refused_before_connecting`,
  `p0_fg_an_admitted_authenticated_peer_is_reached_at_its_policy_address`.
- **Non-claims.** No pairing or peer authentication infrastructure was
  added, and `send_model` still does not consult `sharing_enabled`.
  `receive_model` stays latent (guard needle), and no production code
  listens for peers.

## G. Approval channel

- **Where.**
  - `approve_consent_request` (`app/src-tauri/src/commands/consent.rs`) takes
    an interface-supplied `approved_by`.
  - `swarm_approve(ticket_id)` (`app/src-tauri/src/commands/swarm.rs`) starts
    a model-planned swarm run, including the Herald social-post adapter
    (`crates/nexus-swarm/src/adapters/herald.rs`), which publishes with stored
    X credentials when a planned node sets `dry_run: false`.
  - L6 (transcendent) agent approvals go through
    `enqueue_transcendent_review` (`app/src-tauri/src/commands/agents.rs`).
  - In the cognitive loop, agents at autonomy 3 or higher get an HITL
    allowance for every planned step
    (`kernel/src/cognitive/loop_runtime.rs`, `hitl_approval_allowance`).
- **Effect.** A human approval is delivered by an IPC call from the webview,
  the same channel as the request. With item D, a script in the webview could
  approve its own request. In particular:
  - an L6 agent can be created and approved by the same caller, with no
    delay (the "60-second review" is display-only) and no approver identity;
  - `create_agent` accepts autonomy 0–5 without approval, so the registered
    level that bounds `tools_execute` (C5C) is itself interface-chosen.
- **Bounds today.**
  - Approvals never grant filesystem, process or OS-input authority: the
    Phase Zero executor refuses those actions whatever is approved.
  - A2A delegation is refused (C5C).
  - The desktop's Herald only drafts (C5C, `HeraldAdapter::drafts_only`). An
    approved swarm plan can no longer publish a model-written post with the
    stored X credentials.
- **Decision needed.** Whether approvals need an out-of-band confirmation
  (for example a native dialog), and whether a plan approval should bind the
  generated content.

## H. Secrets at rest outside the vault

- **Where.**
  - Email OAuth tokens: `{provider}_tokens.json` under the email OAuth
    directory (`email_start_oauth`, `app/src-tauri/src/commands/apps.rs`).
  - Integration OAuth tokens: `integrations/{provider_id}_oauth.json`
    (`integration_start_oauth`).
  - Both are plaintext JSON under the identity home. C5C corrected a code
    comment that called the email file encrypted.
- **C5C change to the flows.** Both flows now accept only
  `GET /oauth/callback?…` carrying the flow's state. Before C5C any local
  process or rendered page could inject an authorization code (login CSRF),
  and the deadline never fired.
- **Exploitability.** No open IPC command reads these files back. A local
  same-user process can.
- **Decision needed.** Move the tokens into the secrets facade.

**Correction (P0-FINAL-GATE-CLOSURE) to the C5C text above.** The list of
secrets at rest was incomplete. It omitted: the messaging bot tokens, which
`messaging_connect_platform` copied in plaintext to
`messaging_tokens/<platform>.json`; the Slack Socket Mode URL it cached in
`slack_ws_url.txt`; the API Client collections, which could hold tokens,
passwords and API keys; the XOR deploy store of item A; and backups, which
copied all of these.

**Repaired in P0-FINAL-GATE-CLOSURE (contract H: new writes; Architect review
pending).** No approved secret store exists for these credentials, so the
operations that would persist them are closed or refuse, and stored files are
left as they are.

- **OAuth sign-in.** `email_start_oauth` and `integration_start_oauth` are
  closed (`Closure::SecretStorage`) before any listener, browser launch,
  request or write. The C5C loopback helpers are compiled for their tests
  only, and no production code opens a browser (`open::that`). The email
  commands still read token files stored earlier, and `email_disconnect`
  still removes one on request.
- **Deploy and Supabase credentials.** `builder_deploy_store_credentials` and
  `builder_backend_connect` are closed (`Closure::SecretStorage`), and the
  store refuses new entries (`agents/web-builder/src/deploy/credentials.rs`,
  `agents/web-builder/src/backend/credentials.rs`). Entries stored earlier
  stay readable; a deletion never rewrites a store it cannot read or parse.
- **Messaging.** `messaging_connect_platform`
  (`app/src-tauri/src/commands/apps.rs`) accepts only the stored
  configuration token, which the interface sees as the placeholder; a new
  token is saved by the settings save, which needs the operator key (item
  A). It writes no token file and no Socket Mode URL. `read_messaging_token`
  uses the configuration's token first, then reads a legacy token file as it
  is. Connect, send and poll errors carry no URL, so the Telegram token in the
  URL path never reaches the interface. The connectivity check is bounded
  (10 s in total, 64 KiB per body); send and poll too (30 s, 4 MiB).
- **API Client collections.** `api_client_save_collections` refuses
  collections that are not JSON or that hold a non-empty `authToken`,
  `authPass` or `authKeyValue`, or a credential header entry
  (`Authorization`, `Proxy-Authorization`, `Cookie`, `X-Api-Key`, `Api-Key`,
  `X-Auth-Token`, `Private-Token`); field and header names match in any
  letter case. The stored file is left as it was.
- **Backups.** `create_backup` (`kernel/src/backup.rs`, IPC `backup_create`)
  never copies `email_oauth/`, `integrations/`, `messaging_tokens/`,
  `deploy_credentials.json`, `oauth_settings.json` or
  `api_collections.json`. It copies the configuration file only when that
  file is an exact configuration encryption envelope or the archive is
  encrypted; otherwise it skips it and says so in the backup metadata. An
  encrypted backup checks for its key before writing anything. The archive
  is created owner-only (0600 on Unix); on Windows it keeps the directory's
  inherited access.
- **Example program.** `kernel/examples/dump_config.rs` prints only whether an
  NVIDIA key is stored, never the key.
- **Tests and guards.** `p0_fg_h_*` in `commands/apps/tests.rs` and
  `kernel/src/backup.rs`, `p0_fg_a_*` in the web-builder stores, and the
  `fg_secrets` guards (`p0_fg_h_sign_in_flows_persist_no_token`,
  `p0_fg_h_messaging_tokens_are_never_copied_to_plaintext_files`,
  `p0_fg_h_api_client_collections_are_checked_before_writing`,
  `p0_fg_a_deploy_credentials_are_never_newly_stored`); for the messaging
  transport, `p0_fg_messaging_errors_never_carry_the_bot_token` and
  `p0_fg_messaging_requests_are_bounded_in_time_and_size` (`fg_egress`).
- **Non-claims.**
  - Closing new writes does not encrypt anything written earlier: OAuth token
    files, messaging token files, the XOR store, API Client collections and
    earlier backups stay as they are, readable by a same-user process.
  - Secrets typed into other API Client headers, parameters, URLs or bodies
    are user content and are not detected.
  - Backup exclusion is by name, directly under the data directory.
- **Decision requested.** Request 10.

## I. PATH-resolved helper programs

The desktop runs these programs by name from `PATH`, with fixed or validated
arguments:

- curl (every governed network site);
- `ollama serve` (`ensure_ollama`, `app/src-tauri/src/commands/chat_llm.rs`),
  spawned detached and never reaped;
- `ollama --version`;
- the `which` presence probes for git, ripgrep and ollama (`nexus-code/src/setup.rs`, `check_command_exists`);
- `notify-send`, `osascript` and `powershell` (notifications; C5C passes the
  message only as data);
- `import` (Linux) and `screencapture` (macOS) screen capture into a private
  temp directory (`kernel/src/computer_control.rs`). No desktop route reaches
  them since Architect repair A (item L). `nx_computer_use_status` still
  probes for capture tools with `which`;
- `df` (flash disk space).

A program earlier on `PATH` with the same name runs instead. That requires
control of the user's environment, which is operator authority. The
Builder's Node toolchain is not among these: it is packaged and verified
(C4C–C4D).

- **Decision needed.** Whether helper programs must resolve from fixed
  locations, and whether `ollama serve` needs a lifecycle.

**Correction (P0-FINAL-GATE-CLOSURE) to the C5C list above.** It omitted the
hardware probes: `nvidia-smi`, `rocm-smi`, `lspci`, `sysctl` and `wmic`
(`kernel/src/hardware.rs`; `nvidia-smi` also for the flash metrics), and on
Linux `dmesg` and `dmidecode`, run at every startup by the flash engine's
hardware detection (`crates/nexus-flash-infer/src/hardware.rs`). It also
omitted `which sd`, run by the open `builder_image_gen_status`
(`agents/web-builder/src/image_gen/local.rs`), and the `open` crate's browser
launcher for the OAuth flows (closed since, item H).

**Repaired in P0-FINAL-GATE-CLOSURE (contract I; Architect review
pending).**

- **No unowned launch.** `ensure_ollama` (`chat_llm.rs`) starts nothing. It
  probes the authorized Ollama address (item B) and returns
  `Closure::HelperLaunch` when nothing answers; nothing is started, waited for
  or stopped, and nothing is found or stopped by PID, name or port. Before,
  each failed probe spawned another `ollama serve` from `PATH`, detached,
  never reaped, with the desktop's environment (including the `*_API_KEY`
  values that `save_provider_api_key` sets). Connecting to an Ollama service
  started outside Nexus stays available.
- **No program run to report on Ollama.** `is_ollama_installed` is closed
  (`Closure::HelperLaunch`; it ran `ollama --version`), and
  `check_ollama_smart` no longer runs `which`. The desktop's own sources hold
  no `Command::new("ollama")` or `Command::new("which")` (latent-API needles).
- **Owned cleanup.** curl children are reaped on early-error paths
  (`reap_child`), and a panicking pull-progress callback no longer unwinds
  past a running child.
- **In-flight model downloads (I5).** An in-flight registry
  (`connectors/llm/src/model_hub.rs`) starts each download's curl child under
  its lock and owns it until the download ends.
  `terminate_in_flight_downloads`, called from the normal-exit hook
  (`RunEvent::Exit` in `lib.rs`, after the Builder dev-server cleanup):
  - kills each running transfer through its owned handle and reaps it, within
    one 5-second deadline;
  - removes the partial files, and refuses any later start;
  - treats a transfer that had already ended as reaped, not as an error;
  - counts transfers whose exit it cannot confirm; they stay registered and
    are logged by count only, with no URL, path or process id.
- **Remaining `which` probes.** The Nexus Code diagnostics the desktop runs
  (`nexus_code::setup::diagnose_for_desktop`: `ollama`, `git`, `rg`) and
  Nexus Code's provider detection still run `which` from `PATH`
  [PENDING: S3 final]; the computer-use readiness probe
  (`nx_computer_use_status`) and `which sd` (above) remain.
- **Guards and tests.** `p0_fg_nexus_starts_no_ollama_and_runs_no_helper_to_find_it`
  and `p0_fg_the_application_exit_ends_in_flight_model_downloads` (the exit
  hook and the single registered spawn), in `fg_egress`;
  `p0_fg_curl_children_are_reaped_on_early_errors`; the `model_hub`
  registry tests (`p0_fg_terminating_in_flight_downloads_reaps_each_running_transfer`,
  `p0_fg_an_unconfirmed_exit_is_reported_and_stays_registered`,
  `p0_fg_no_download_starts_after_the_exit_cleanup`, and others).
- **Non-claims.**
  - The download cleanup runs at a normal exit only. After a crash or a kill
    (SIGKILL, a forced end of task), no exit code runs and a transfer in
    flight can outlive the application. Downloads have a stall bound (120 s
    below 1 byte/s) and the 64 GiB size bound, but no total time limit.
  - Ollama pull and chat curl children are not registered; each is bounded by
    curl's `-m` (900 s by default).
  - Every remaining helper, curl included, is still found through `PATH`.
- **Decision requested.** Requests 1 and 17.

## J. Shipped non-desktop binaries

The installers ship only the desktop app (`app/src-tauri/tauri.conf.json`
bundle). The desktop never reaches the binaries below.

- **`crates/nexus-server` (`nexus-server`): BLOCKER. Withdrawal implemented
  on the P0-FG1 validation branch; Architect review pending.**
  - **Before P0-FG1.**
    - `src/main.rs` bound `0.0.0.0` on ports 3000, 3001 and 3002 with no
      authentication and a permissive CORS layer (`Any` origin, method and
      header).
    - It served `/mcp/tools/invoke`, `/mcp/handle` and the MCP port's
      `/tools/invoke`, which ran the `nexus-mcp` tools. Those include
      `nexus_github` with the operator's `GITHUB_TOKEN`
      (`crates/nexus-mcp/src/tools.rs`) and tools that read files relative
      to the working directory.
    - Any network peer, and any web page through the permissive CORS, could
      invoke them.
    - `deploy/Dockerfile`, `deploy/docker-compose.yml`,
      `deploy/docker-compose.cpu.yml` and the `deploy/helm/nexus-os` chart
      built and ran it.
  - **Withdrawal (P0-FG1).** The Architect chose withdrawal over repair: no
    authentication, no loopback-only server, no health-only listener and no
    way to reactivate it.
    - The binary's only entry point writes `nexus-server: unavailable during
      Phase Zero; deployment withdrawn` to standard error and exits with
      status 69.
    - It reads no argument, environment variable, configuration or
      credential first. It creates no file, starts no runtime, binds no
      socket, builds no MCP server and starts no process.
    - No flag or environment variable restores the server.
    - The package and its `nexus-server` target remain, so workspace builds
      still compile. Its dependencies stay declared but unused.
    - `deploy/Dockerfile` fails at its first step (a `RUN` that prints a
      withdrawal message and exits 1). That is before any package
      installation, source copy or compilation, and the file has no other
      stage.
    - Both `deploy/` Compose files define no services, ports, volumes,
      credentials or restart policies. The Ollama service they published on
      port 11434 is gone too.
    - The chart's only template is an unconditional `fail`. No install,
      upgrade or values override renders a resource. An upgrade of an
      existing release fails before it changes or deletes anything,
      including its data volume claim.
  - **Tests.** `crates/nexus-server/tests/phase0_withdrawal.rs`:
    - Runs the executable built from this package. Each run has an empty
      environment apart from synthetic sentinels, a scratch home and working
      directory, and a deadline after which the test kills its own child.
    - Checks the fixed status and message for the default invocation, for
      the retired `--port`, `--mcp-port`, `--a2a-port`, `--data-dir` and
      `--log-level` arguments (port 0 included), and for help, version and
      subcommand-like words.
    - Checks that no requested or default data directory is created or
      modified, and that no argument or environment value is echoed.
    - A source guard pins the entry point to exactly the withdrawal and the
      package to that one target.
    - Deployment guards pin the Dockerfile, both Compose files, the chart,
      `deploy/README.md` and the J1 part of `docs/DEPLOYMENT.md`.
  - **Binary name collision.**
    - Before P0-FG1-R1, `nexus-protocols` also built a binary named
      `nexus-server`.
    - When one Cargo invocation built both, the shared output path Cargo
      gives this package's tests could hold the protocols server.
    - On Windows (MSVC) Cargo gives executables no hash, so the two packages
      also shared `deps\nexus_server.exe`. This package's build could be
      overwritten there.
    - The tests run a Cargo build only if its dep-info names
      `crates/nexus-server/src/main.rs` and its bytes carry the withdrawal
      message.
    - When no such build can be identified on Windows, the tests compile the
      package's only source file with the `rustc` beside the `cargo` that
      built them, check the result, and run that. On other platforms an
      unidentified build fails the tests.
    - They never run the protocols server or a `PATH` lookup.
    - **Architect decision (P0-FG1).** The Windows fallback is accepted for
      this checkpoint only, because the same tests pin the package to its one
      withdrawal-only source file. It does not make direct-`rustc`
      substitutes a normal testing pattern, and it does not resolve the
      duplicate binary name, which stays build debt (item K).
    - **Hosted failure (run #106).** The first complete hosted run of the
      composed FG1 and CI candidate (`2b47bb09`) failed on Windows before any
      test ran. Linking this package's binary stopped with `LNK1104: cannot
      open file ...\target\debug\deps\nexus_server.exe`, the output path the
      two binaries shared (item K).
    - **Naming repair (P0-FG1-R1; hosted verification, review and integration
      pending).** The protocols binary target is renamed
      `nexus-protocols-server`; its source file is unchanged. J1 is then the
      only workspace binary named `nexus-server`. The identification and the
      Windows fallback above are unchanged and stay as defensive checks; the
      fallback's narrow acceptance is not widened.
  - **What shipped.** Nothing published this binary or an image built from
    `deploy/`.
    - CI and the release workflow compile it as a workspace member
      (`cargo test --workspace`, `cargo build --release`).
    - The release uploads only the desktop bundles
      (`.github/workflows/release.yml`, `create-release`), and the desktop
      bundle has no sidecar.
    - So the recipes were source-built deployment paths, not published
      artifacts.
  - **Correction.** Before P0-FG1 the package had two unit tests, which
    checked command-line parsing of the retired arguments. They described
    the retired behaviour and are replaced by the withdrawal tests.
  - **Non-claims.** The change does not:
    - stop, remove or update a deployment, container, image, volume or Helm
      release made from an earlier version of these files;
    - revoke a credential that such a deployment could use;
    - make `nexus-mcp` safe to expose on a network;
    - change the protocols server, `nexus-cli` or `nx`.
  - **Deployment guide (Architect scope extension).**
    - `docs/DEPLOYMENT.md` replaced its "CLI server (alternative)" subsection,
      which built and started this binary with the retired port arguments.
    - The replacement states that the package is withdrawn, that it only
      prints the withdrawal, that no supported deployment exists, and that
      existing deployments are not stopped automatically.
    - The FG1 documentation guard rejects a return of those instructions.
    - The guide's protocols server instructions belong to J2/J3. P0-FG1-R1
      changes only the binary name in them (`nexus-protocols-server`); their
      J2/J3 disposition is unchanged and unresolved.
- **protocols `nexus-server` (`protocols/src/bin/`, built by the root
  `Dockerfile`, compose and helm); renamed `nexus-protocols-server` by
  P0-FG1-R1 (review and integration pending).**
  - `protocols/src/server_runtime.rs` binds `NEXUS_HTTP_ADDR`, default
    `0.0.0.0:8080`.
  - Routes sit behind EdDSA JWT verification against a per-process gateway
    key: `/api/*`, `/a2a`, `/mcp/tools/*` (the kernel `McpServer` over the
    full default actuator registry), `/ws` and `/v1/*`. `JWT_SECRET` is
    ignored, and no production route issues tokens.
  - Unauthenticated: `/health`, `/ready`, `/metrics`, `/auth/jwks`,
    marketplace search and agent detail, and `/a2a/agent-card`.
  - Port 9090 is exposed, but nothing listens on it.
  - Needs Final-Gate review of its routes, token issuance and the actuator
    registry behind `/mcp/tools/invoke`.
- **`nexus-cli` (`cli/`), shipped.** The GitLab `release-build` job and the
  Homebrew, WiX, systemd and launchd packaging ship it.
  - `run_voice_python` runs `python3 jarvis.py` in a `voice/` directory
    found from the working directory, or else from the build-time checkout
    path (`resolve_voice_dir`).
  - It links the coding agent's `sh -lc` / `cmd /C` runner
    (`agents/coding-agent/src/lib.rs`).
  - Its authority is the invoking user's, in the directory the user runs it
    from. Final-Gate review.
- **`nx` (`nexus-code` binary), shipped** by `nexus-code/Dockerfile` and
  `nexus-code/install.sh`. It is a terminal coding agent that reads
  `NEXUSCODE.md` and `.nxrc` from its project directory by design, and runs
  its tools and MCP servers in the directory the user starts it in. The
  desktop uses `load_for_desktop`, `diagnose_for_desktop` and
  `new_for_desktop`, which read none of these files (C5C). Final-Gate review
  of the standalone agent's tools.
- **protocols `nexus-os`.** The `Makefile` target `nexus-os` builds it
  (`cargo build --release -p nexus-protocols --bin nexus-os`), and
  `install.sh` installs a `nexus-os` binary from release assets. It runs the
  same server runtime as the protocols server (`nexus-server`;
  `nexus-protocols-server` after P0-FG1-R1).
- **Build note, not executed.** The root `Dockerfile` copies every workspace
  member except `nexus-code/`, which is a member, so its image build appears
  unable to load the workspace. `deploy/Dockerfile` had the same omission;
  P0-FG1 withdrew it.
- **Decision needed.**
  - J1 (`crates/nexus-server`): withdrawal implemented on the P0-FG1
    validation branch; Architect review pending. It remains a Final-Gate
    blocker until that review, integration and final verification.
  - The others need review.

## K. Reliability signals

- **Windows `executes_python_code`**
  (`kernel/src/actuators/code_exec.rs`, test `executes_python_code`,
  `CommandTimeout { seconds: 5 }`).
  - A Category C signal: native Windows only, in the kernel code-execution
    actuator, which the desktop's Phase Zero executor never calls.
  - C5C does not change the test or the production timeout.
  - It stays Final-Gate reliability debt. The Category C rule (one `--failed`
    diagnostic rerun under identical conditions) still applies.
- **C4B exit race** (`app/src-tauri/src/builder_workspace/process_lifecycle/tests.rs`,
  `p0_002c4b_stop_racing_natural_exit_finalizes_exactly_once`). Resolved in
  C5C.
  - A test defect: the fake-tree test did not accept the valid ordering in
    which the monitor finalizes a natural exit before `stop` arrives (`stop`
    then returns `NoOwnedServer`). The real-process sibling test already
    accepted it.
  - Reproduced read-only (3 failures in 4,400 runs). After the test-only
    repair it passed 8,000 of 8,000 runs, 16-way parallel, with no sleeps
    added.
  - The production code and the finalize-exactly-once invariant are
    unchanged.
- **`nexus-ui-repair` `HOME` race** (`crates/nexus-ui-repair/tests/report_format.rs`).
  Repaired on the P0-FG1 validation branch (test-only; Architect scope
  extension); review pending.
  - A test defect: the tests of that binary run in parallel in one process,
    and each set and restored the process-wide `HOME`. One test could remove
    or change `HOME` while another built `Acl::default_scout()` from it.
  - CI saw it on Windows, where `HOME` is not set: `HOME environment
    variable must be set`.
  - Reproduced before the repair: 64 of 1,000 runs failed with `HOME` absent
    and 23 of 1,000 with it present.
  - The repair: one process-wide lock serializes the `HOME` change. Each
    guard holds it for its life and restores `HOME` before releasing it.
  - After the repair: 0 failures in 3,000 runs (`HOME` absent and present,
    default and 16 test threads). That is stress evidence, not proof.
  - A deterministic test checks that the lock is held while a guard is
    alive.
  - The ACL code and the test assertions are unchanged.
- **Duplicate binary name `nexus-server`** (`crates/nexus-server` and
  `nexus-protocols`). Build debt for the Final Gate; naming repair on the
  P0-FG1-R1 repair branch, verification and integration pending.
  - Before P0-FG1-R1, one Cargo invocation that built both wrote them to the
    same output paths: `target/<profile>/nexus-server`, and on Windows also
    `deps\nexus_server.exe`. Cargo warns that this may become a hard error.
  - P0-FG1 works around it in its tests (item J).
  - Hosted run #106 (candidate `2b47bb09`) failed on Windows at link time with
    `LNK1104: cannot open file ...\deps\nexus_server.exe`, before any test
    ran. FG1 runs #103–#105 printed the same collision warnings without that
    error. Whether two links overlapped or another process held the file is
    not established.
  - **P0-FG1-R1 (Architect decision).** The protocols binary target is
    renamed `nexus-protocols-server`; its source file is unchanged. A
    target-identity test (`protocols/tests/binary_target_identity.rs`) checks
    that `nexus-protocols` builds `nexus-protocols-server` and `nexus-os` and
    no `nexus-server`. The item stays open until a complete hosted run on the
    repaired candidate is green and the repair is reviewed and integrated.

## L. Screen observation from the interface

**Closed in C5C (Architect repair A).** Unbrokered desktop screen observation
is unavailable in Phase Zero. Before the repair, any script in the webview
could capture the whole screen, and two routes rearmed the engine after the
emergency stop.

**Denied unconditionally** (`Closure::ScreenObservation`). Each handler takes
no input, and its whole body is the denial:

| Command | Behaviour before the repair |
|---|---|
| `capture_screen` (`runtime`, `lib.rs`) | Enabled a disabled engine, even after the emergency stop, then `capture_and_store_screen` ran `import` or `screencapture` and wrote a PNG and an audit line under the identity home |
| `analyze_screen` (`runtime`) | The same, then `capture_and_analyze_screen` sent the capture to a vision model at `OLLAMA_URL` |
| `computer_control_capture_screen` (`runtime`) | `engine.capture_screen` whenever the engine was enabled; the emergency stop was never consulted |
| `nx_computer_use_screenshot` (`app/src-tauri/src/nx_bridge/commands.rs`) | Captured the whole screen directly (grim, scrot or import, `crates/nexus-computer-use`) and returned it as base64, ignoring the engine and the stop |

The live capture implementations were removed from the desktop:
`capture_screen`, `analyze_screen`, `computer_control_capture_screen` and
`desktop_control_workspace` in `trust_security.rs`, and the nx body.

**Refused enabling branch.** `computer_control_toggle(enabled: true)` returns
the same bounded denial before it reads or changes state. An IPC request is
not proof of consent. `computer_control_toggle(false)` still disables.

**Kept available:** disabling, `stop_computer_action`, the emergency-stop
shortcut, `computer_control_status`, `get_input_control_status`,
`computer_control_get_history` and `nx_computer_use_status`. Readiness
reporting is not a condition for capture.

**Inert and open: Omniscience.** `omniscience_enable` and
`omniscience_get_screen_context` stay open. `ScreenUnderstanding::start()`
only sets an in-memory flag. `capture_context` only stores a context it is
given, and nothing in production supplies one. Neither starts a screen
capture. This classification does not authorize future observation wiring.
The guard fails if either the kernel module or the desktop wrapper gains a
capture primitive, a process, a worker or network access.

**Dormant.** The kernel capture functions still exist, and they do not
consult the emergency stop. So do the computer-use capture backend and the
refused kernel screen, input and computer-use actuators. No desktop startup,
scheduler, agent or IPC route reaches them. The following fail if one does:
- the `LATENT_UNSAFE_APIS` needles;
- `p0_002c5c_no_desktop_route_observes_the_screen`, which also covers
  aliases, the kernel names still imported unused, engine enabling and
  emergency-stop resets.

Stored screenshots were not touched.

- **Decision needed (future).** A brokered observation mechanism with
  out-of-band consent, if screen observation is ever reopened. That needs a
  separately approved mission.

## Resource bounds (non-authority)

These surfaces are bounded in authority but not in amount:

- `log_frontend_error` and `builder_record_build` append to fixed files with
  no size limit.
- `run_parallel_simulations` threads, `run_dilated_session` iterations and
  `stress_generate_personas` counts are caller-chosen.
- An AgentScheduler cron schedule may fire every second.

They are denial-of-service items for the Final Gate, not authority items.

## Unavailable features (Phase Zero closures)

These are unavailable rather than working unsafely. The C5 inventory lists each closure.

- **132 IPC commands closed**, each taking no input and returning one bounded
  reason. C5A closed 125: user-file selection, the legacy Builder, process
  execution, caller-asserted approval, external CLI agents, agent execution
  and ambient resources. C5C closed seven:
  - OS keyboard and mouse input: `computer_control_execute_action`,
    `start_computer_action`;
  - the browser screenshot's raw output path: `browser_screenshot`;
  - screen observation (Architect repair A): `capture_screen`,
    `analyze_screen`, `computer_control_capture_screen`,
    `nx_computer_use_screenshot`.
- **Enabling computer control over IPC** is refused, and disabling remains
  (`computer_control_toggle`).
- **Agent actions.** Filesystem, shell, code, Docker, API, image, speech,
  browser, screen and input actions are refused by `Phase0AgentExecutor`. So
  is A2A delegation, since C5C.
- **Recovery and records.**
  - Time Machine file undo and redo: no desktop file authority.
  - Backup restore: the command is closed; only the library is hardened.
- **Nexus Code from the desktop.** The agent, chat and tool entry points are
  closed; sessions, memory and diagnostics remain.
- **Other C5C restrictions.**
  - The desktop swarm drafts social posts but does not publish them.
  - The flash LLM provider needs an absolute model path; none is configured
    by default. A `flash/<path>` chat model id is refused.
  - Simulations report nothing about the host's files or environment.

## TOCTOU non-claims

- `governed_path` checks are point-in-time pathname checks: grammars,
  `regular_file_beneath`, `case_exact_entry`, `existing_root` and
  `private_temp_dir`. A same-user process racing the filesystem between a
  check and a use is not defended against.
- Case-exact lookups compare directory entries at lookup time. A file created
  with another spelling after the check is not seen.
- The OAuth listeners bind fixed loopback ports. A local process that binds a
  port first makes the flow fail; it cannot supply a code without the flow's
  state.
- DNS answers are not pinned between an egress check and the request (item B).

## No OS sandbox

Phase Zero builds no filesystem, process or network sandbox. Permitted agent
actions run in the desktop process: LLM, memory, notification, messaging,
HITL, web search and fetch, and knowledge graph. Helper programs run with the
user's full authority. The Builder's sealed spawn and resource limits (C4)
constrain a dev server's arguments, environment and resources; they do not
isolate its filesystem or network.

## Corrections to earlier checkpoint claims

- **C5B, "all production curl sites are bounded in time and size."** The C5C
  recount found every site time-bounded, but 14 of the 35 had no response-size
  bound:
  - the Ollama tag, delete, chat-stream, pull and JSON helpers;
  - the provider status and JSON POST helpers;
  - the hub HEAD and Ollama create requests;
  - the SearXNG health check;
  - both A2A requests.

  C5C added `--max-filesize` to each and a guard that requires both bounds
  per file. The C5B guard had checked curl syntax only.
- **Not a correction, an update.** C5B recorded that model downloads had
  connect and stall bounds only. C5C adds the size bound (64 GiB, enforced on
  the bytes written) and fixes the flash downloader's redirect, stall and
  resume handling.
