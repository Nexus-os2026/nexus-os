# Phase Zero Final-Gate Dossier

Status: prepared by P0-002C5C (final authority sweep). This dossier collects
the evidence for the Phase Zero Final Gate. It does not decide any item and
does not claim that Phase Zero is complete. Items that C5C repaired are named
as such; everything else is open for the Final Gate.

P0-FG1 (item J1) withdrew `crates/nexus-server`. It is integrated into the
authoritative branch at `71c47acb`, and its designated post-integration hosted
validation, run #108, passed. J1 does not await integration or Architect
review. P0-FG1-R1's duplicate-name repair is integrated with it. See item J.

**P0-FINAL-GATE-CLOSURE (internal work, not approval).** The Architect's
closure mission implements contracts A–K, C5 and DEP on local component
branches, composed into the closure candidate
(`implement/p0-final-gate-closure`). Blocks headed "P0-FINAL-GATE-CLOSURE"
describe that candidate with every workstream composed (`d1735577`). They
are claims for Architect review, checked against the code; the internal
reviews are recorded on the mission's evidence branch. Nothing here approves
an item, accepts a risk, integrates a change, or declares FG1, the Final Gate
or Phase Zero complete. Earlier text stays as it was; where it is now stale, a
labelled correction or note says so. [P0-LINUX-FINAL-R2: headings below that
say "Architect review pending" were written before the Architect's decisions.
The current disposition of each item is in the Summary and in "Architect
decisions (P0-LINUX-FINAL-R1)" and "Architect dispositions
(P0-LINUX-FINAL-R2)" at the end of this dossier.]

**P0-LINUX-FINAL-R1 (platform scope and Architect decisions).** By the
Owner's decision, the Phase Zero active target is **Linux only**; Windows and
macOS are deferred to later portability stages. `.github/workflows/ci.yml` is
the Linux Phase Zero final gate (jobs `test-linux`, `security-audit-linux`,
`test-frontend`, `test-python`). The Windows and macOS jobs are kept
unchanged in `.github/workflows/ci-portability.yml`, which runs only on a
manual dispatch. This dossier makes no Windows or macOS claim: where a block
describes Windows or macOS behaviour, it records code that was read, not a
Phase Zero property. The one hosted run of the closure work (run #109,
commit `9ed607f8`) passed the Linux, frontend and Python jobs; its macOS
compile error and single Windows test failure are deferred with those
platforms (the mission's evidence file `PLATFORM-SCOPE.md`). The Architect's
decisions for Linux Phase Zero are recorded as dispositions under
"Architect decisions (P0-LINUX-FINAL-R1)" at the end of this dossier; they
replace the corresponding decision requests.

Every location below is a repository path and function at the C5C head,
except where P0-FG1 or P0-FINAL-GATE-CLOSURE is named. An
**untrusted surface** means one of:

- a script running in the desktop webview (the CSP is `null`, item D, so every
  registered IPC command is callable by any script there) [P0-FINAL-GATE-CLOSURE:
  item D grants application commands only to the privileged window at the
  app's own origin and keeps non-app documents out of it; a script that runs
  inside that document still reaches every registered command];
- model output (planner steps, agent actions, generated text);
- remote content (fetched pages, API responses, peers);
- a persisted record (database rows, stored files).

"Immediately exploitable" means reachable from one of these surfaces today,
without a local same-user foothold or an operator mistake.

## Summary

### On the closure candidate (P0-FINAL-GATE-CLOSURE)

No row declares the Final Gate or Phase Zero complete. [P0-LINUX-FINAL-R2
correction: the rows no longer simply await Architect review.] The Architect's
Linux Phase Zero dispositions are collected at the end of this dossier:
decisions A–M (P0-LINUX-FINAL-R1) and the dispositions of requests 7, 9, 10,
11, 14 and 19 (P0-LINUX-FINAL-R2). Each holds only within the limits and
non-claims it states. The numbered decision requests are kept there as they
were.

| Item | Topic | Closure candidate | Status |
|---|---|---|---|
| A | Configuration encryption key | A new or changed credential is written only under the operator key `NEXUS_CONFIG_KEY`; legacy files open through an explicit two-key read path; a load never rewrites a file; protection changes are reported and audited | Repaired on the closure candidate (P0-FINAL-GATE-CLOSURE). For Linux Phase Zero the Architect accepted `NEXUS_CONFIG_KEY` as the operator key source (decision B) and legacy-read compatibility (decision C). Request 9 (R2): the legacy ambient derivation is bounded legacy compatibility only and never secret protection; every new or changed credential needs the operator key. Request 10 (R2): the closed token flows stay closed; reopening one needs a separately approved secret-store design |
| B | Egress policy | The 11 IPC commands that sent requests to a caller-chosen destination are closed; the Ollama address is `OLLAMA_URL` or the fixed local default; agent web fetch and caller-destination external tools are refused; SearXNG only at `SEARXNG_URL`; search redirects https only | Repaired on the closure candidate. No address or DNS policy was added (remaining destinations are backend constants or operator configuration). A launch-time non-loopback `OLLAMA_URL` is operator configuration (decision F; plaintext HTTP risk stated in item B) |
| C | Secrets in subprocess argv | Reachable credentials leave the command line; every reachable credential-bearing HTTP client follows no redirect and is bounded in total time and size (final Linux network-client inventory); `perception_init`, the credential-bearing external tools and credentialed MCP servers are closed or refused; remaining credential curl sites are counted, each CLI-only, latent or behind a closed route | Repaired on the closure candidate for Linux; the in-process transport is accepted by the Architect (decision E) |
| C5 (contract) | Residual capability-measurement route | `cm_run_ab_validation` closed (`AmbientResource`) before any input use; the Groq-endpoint client takes only `GROQ_API_KEY` | Repaired on the closure candidate (the route denies before any input use); `AmbientResource` accepted as the closure reason (decision H) |
| D | Privileged webview, navigation and IPC origin | On Linux: an app ACL grants all 804 application commands only to the `main` window at the local app origin; a navigation guard admits only the exact app origin (redirects included) and every new window is denied; frames are script-free or removed; a CSP with no third-party origin; a live native harness in the Linux gate | Accepted by the Architect for Linux Phase Zero as the combined boundary (decision K): the guard is load-bearing and the ACL alone is not sufficient. Windows and macOS deferred |
| E | Operator overrides | The vault key file is validated on the opened file (Linux, macOS) and refused elsewhere; the vault key must open every stored secret before the vault is used; `key_env` other than `NEXUS_ENCRYPTION_KEY` is refused | Repaired on the closure candidate; the Linux vault key-file semantics accepted (decision D), Windows deferred. Request 11 (R2): accepted for Linux after the inventory guard; the six current vault scopes are verified before the facade is installed; no protection is claimed for a future scope that is not added to and verified by that inventory |
| F | Network peers (Nexus Link) | `nexus_link_send_model` closed; the library admits no peer under an empty policy, matches exact IP socket addresses only, resolves no names and requires a shared secret and a key | Repaired on the closure candidate; transfer unavailable (no pairing exists); no separate Architect disposition is recorded for this item |
| G | Approval channel | L6 (autonomy 6 or above) refused at create, start, resume, approve and review-each, never registered by restore or the prebuilt load, and refused for goals, autonomous loops, scheduled ticks and tool calls; caller-asserted approval commands closed; consent resolutions labelled `desktop-ui (unverified)`; self-improvement recorded truthfully; an enabled Warden review denies | Repaired on the closure candidate; no out-of-band approval exists (non-claim). Stored L6 records stay untouched but inert, and `desktop-ui (unverified)` is an audit channel label, not a verified identity (decision G). Request 14 (R2): IPC HITL approval is not security authority and not verified human identity; it is only a UX/consent signal whose released operation is bounded by the backend-owned Phase Zero executor policy |
| H | Secrets at rest outside the vault | No new plaintext token or credential persistence: OAuth sign-in and deploy/Supabase storage closed; messaging uses the stored configuration token; API Client collections holding secrets refused; backups skip credential stores | Repaired for new writes on the closure candidate; historical files unchanged. Request 10 (R2): not an active Phase Zero blocker while these flows stay closed; reopening any of them needs a separately approved secret-store design and mission; not permanently solved |
| I | PATH-resolved helper programs | No unowned `ollama serve` launch; `is_ollama_installed` closed; no program run to report on Ollama from the desktop's own sources; curl children reaped on early errors; in-flight model downloads owned and ended at a normal exit | Partly repaired on the closure candidate: the desktop's Nexus Code checks start no process, but fixed-argument helpers still resolve from `PATH`, which the Architect accepted as operator launch configuration for fixed-name, fixed-argument helpers (decision A); owned-download cleanup covers a normal managed exit only (decision J) |
| J | Shipped non-desktop binaries | J1 integrated at `71c47acb`. J2–J5: the protocols server and its `nexus-os` alias, `nexus-cli` and `nx` withdrawn on the J1 pattern, with the alternate agent binaries, the `nx-*` harness and six key-leaking benchmarks (18 withdrawn binaries); install, deploy and packaging recipes withdrawn; libraries kept, not governed | J1: integrated at `71c47acb`; its designated post-integration hosted validation (run #108) passed; J1 does not await integration or Architect review. J2–J5: withdrawn on the closure candidate; the withdrawal accepted (decision L); the fixed wording, status 69 and the coordinator closure of D1, D2 and D4–D7 accepted for the current closure candidate under the inventory and withdrawal guards (request 19, R2); J2–J5 are not declared complete until the candidate is integrated and validated; the developer and benchmark binaries may remain, not shipped, inventory-guarded and not documented as runtime entry points (decision M) |
| K | Reliability signals and resource bounds | `executes_python_code`: no retained hosted failure (evidence corrected); GPU-host voice stderr repaired; the duplicate binary name repaired by P0-FG1-R1 (integrated); resource surfaces bounded by refusal (see "Resource bounds") | Repaired or bounded on the closure candidate; the voice tests install from a hash-pinned Linux lock (R1); `ArenaRun` and the one-token minimum accepted as bounded compatibility debt (decision I); request 7 (R2): not a Linux Phase Zero blocker; `executes_python_code` is monitored debt, unreachable from the Phase Zero desktop executor, and the self-hosted GPU driver/library condition is Owner infrastructure; no general reliability claim for that actuator or that GPU host |
| L | Screen observation from the interface | Unchanged since C5C: unbrokered observation is unavailable (Architect repair A) | No change in P0-FINAL-GATE-CLOSURE; a brokered mechanism is future work |
| DEP (contract) | Dependency advisories | Measured on the final candidate's lockfiles: Cargo.lock 19 → 7 cargo-audit vulnerabilities (rsa removed with the unused `openidconnect`); cargo-deny advisory errors 16 → 10; npm (app) 14 → 6. R1: one pinned gate (`scripts/security-audit.sh`) with one exception set in `deny.toml` (10 IDs, then 11 with RUSTSEC-2026-0316); six unsound fixes; npm (app) 4, development-only | Governed by the R1 gate (see "Linux security gate"). RUSTSEC-2026-0316 (wasmtime 43.0.2, dynamic `Val` API, unused and unreachable) accepted narrowly by the Architect; 11-ID exception set |

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
| J | Shipped non-desktop binaries | `crates/nexus-server` is unauthenticated on all interfaces | **Blocker** at C5C. Since withdrawn by P0-FG1, integrated at `71c47acb`; post-integration run #108 passed |
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
  - The kernel's `save_config` and `save_config_to_path`, which the backend's
    other saves use, report such a protection change: one bounded line on
    standard error, and the desktop's recorder, which appends a `save_config`
    audit event by reason class (`install_protection_recorder`, installed in
    `AppState::new` before the credential migration can re-save the file;
    never in test builds).
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
  `phase0_surface/fg_secrets/tests.rs` guards (among them
  `p0_fg_a_backend_protection_changes_are_audited`).
- **Non-claims.**
  - Credentials already under the ambient key stay under it until a
    credential save. Historical files are not re-encrypted.
  - A same-user process can still read or replace the file, derive the
    ambient key of a legacy file, or change the launch environment.
  - No key strength is measured.
  - Key material outside the configuration file is outside this contract.
- **Decision requested.** Requests 9 and 10. [P0-LINUX-FINAL-R1: decisions B
  and C for Linux Phase Zero. P0-LINUX-FINAL-R2: requests 9 and 10 are
  dispositioned, with their non-claims, under "Architect dispositions
  (P0-LINUX-FINAL-R2)".]

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
  - The desktop's Nexus Code bridge registers Ollama only at the authorized
    address, not at `OLLAMA_BASE_URL`; the standalone `nx` (withdrawn) is
    unchanged.
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
  - The privileged document's own requests are limited by the CSP of item D
    (`connect-src 'self' ipc: http://ipc.localhost`), a defence-in-depth
    control, not an egress policy.
  - Operator endpoints are trusted as configured: a non-loopback
    `OLLAMA_URL` is accepted.
  - `OllamaProvider::health_check` probes `127.0.0.1:11434` when the
    authorized address names a host rather than an IP address, so
    `ensure_ollama` can report a local service while the operator's host is
    down. Requests still go to the operator's host.
- **Decision requested.** Request 13. [P0-LINUX-FINAL-R1: decision F.]
- **Disposition (P0-LINUX-FINAL-R1, decision F).** A non-loopback
  `OLLAMA_URL` set at launch is operator configuration. Risk, stated
  explicitly: with an `http://` address, prompts, model output and anything
  else exchanged with Ollama cross the network in plaintext and can be read
  or altered in transit; Nexus adds no transport protection or peer
  authentication for it.

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
| `crates/nexus-mcp/src/tools.rs` (`nexus_github`) | `GITHUB_TOKEN` | no: `mcp2_server_handle` is closed; `crates/nexus-server` reached it until its withdrawal (item J, P0-FG1, integrated at `71c47acb`) |
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
  redirect is reported as a failed request. A crate-wide guard allows HTTP
  clients in the LLM providers only through that helper.
- **Nexus Code bridge and swarm clients.** The desktop's Nexus Code bridge
  (Anthropic, OpenAI-compatible and Google providers, `nexus-code/src/llm/providers/`)
  and the desktop swarm (Anthropic, OpenAI, OpenRouter and Hugging Face
  providers, `crates/nexus-swarm/src/providers/`) build clients that follow
  no redirect. The bridge's Google provider sends its key only in a header,
  never in the URL, and its errors carry no URL.
- **Messaging clients.** The messaging connectivity check and the send and
  poll client follow no redirect and send no `Referer` (item H).
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
  - [Superseded by P0-LINUX-FINAL-R1: the other credentialed providers
    had no size cap and a per-phase timeout, and the email, deploy and some
    messaging clients kept reqwest's default redirects. The final inventory
    below replaces this non-claim.]
- **Decision requested.** Request 12. [Decided in P0-LINUX-FINAL-R1:
  decision E.]

**Network-client inventory (Linux, final; P0-LINUX-FINAL-R1 repair D).**
Every HTTP client that carries a credential and is reachable in the Linux
desktop runtime sends it from the process. No credential is on a process
command line, and keys travel in request headers (Telegram's bot API, whose
token is in the URL path, is the exception; see the non-claims). Each such
request follows no redirect (a redirect is reported as its status), is
bounded in total time from connecting until its body is read, and reads any
body into memory under an explicit size cap. Errors carry no credential;
read errors carry no URL.

| Client | Where | Total time | Size cap |
|---|---|---|---|
| Hosted providers' in-process POST (OpenAI, DeepSeek, Gemini, NVIDIA) | `post_json_in_process`, `connectors/llm/src/providers/mod.rs` | 20 s (NVIDIA 120 or 300 s) | 32 MiB |
| Claude query | `query_bounded`, `providers/claude.rs` | 120 s per attempt; up to 3 retries on 429/529 | 32 MiB |
| Claude stream | `providers/claude.rs` | 300 s, connect to last byte | 32 MiB streamed; error bodies 64 KiB |
| Cohere; OpenAI-compatible (Groq, Mistral, Together, Fireworks, Perplexity, OpenRouter) | `providers/cohere.rs`, `providers/openai_compatible.rs` (`credential_client`, `read_bounded`) | 20 s; 120 s | 32 MiB |
| Desktop swarm: OpenAI, OpenRouter, Anthropic, Hugging Face | `crates/nexus-swarm/src/providers/` (`read_capped`) | 60 s | 32 MiB; error bodies 64 KiB |
| Gmail and Outlook | `email_client`, `app/src-tauri/src/commands/apps.rs` (fetch, send, search) | 30 s per request | 16 MiB; error bodies 64 KiB |
| Deploy token check and site listing (Netlify, Cloudflare, Vercel) | `api_client`, `agents/web-builder/src/deploy/mod.rs` | 30 s | 8 MiB; error bodies 64 KiB |
| Messaging connectivity check | `commands/apps.rs` | 10 s | 64 KiB |
| Messaging send and poll | `messaging_client_with`, `commands/apps.rs` | 30 s | 4 MiB |

The email, deploy and messaging clients also send no `Referer`.
Destinations are fixed provider hosts, or an operator's launch setting
(`ANTHROPIC_URL` for the Claude stream; `OLLAMA_URL`, which carries no
credential). The Cloudflare account id in a deploy path comes from stored
credentials; the host stays fixed.

The workstream's reachability audit classifies the other credential-bearing
clients in the workspace as unreachable from the Linux desktop:
- closed commands: perception, A2A, the MCP host, capability measurement,
  Builder deploy upload and image generation, theme extraction, and the nx
  chat, tool and agent commands;
- refused actions: the external GitHub, Slack and Jira tools; the kernel API,
  image and speech actuators;
- empty or unused wiring: the integrations router is built with no provider,
  the messaging gateway is never polled, and Herald is drafts-only;
- withdrawn or developer-only binaries: the CLI's key validation, the
  social-poster connectors and the ui-repair tools.

Guards and tests: `p0_r1_credentialed_provider_answers_are_size_bounded`,
`p0_r1_credentialed_provider_exchanges_are_bounded_in_total_time`,
`p0_r1_capped_reader_allows_the_cap_and_refuses_more`
(`connectors/llm/src/providers/mod.rs`);
`p0_r1_swarm_bodies_are_read_within_their_cap` and
`p0_r1_credentialed_swarm_answers_are_size_bounded` (`crates/nexus-swarm`);
`p0_r1_deploy_api_requests_are_bounded_and_follow_no_redirect`
(web-builder); `p0_r1_email_requests_are_bounded_and_follow_no_redirect` and
`p0_r1_deploy_token_requests_use_the_bounded_client` (`fg_egress`, pinned by
the final guard); `p0_fg_messaging_*`; `p0_fg_credentialed_*`;
`p0_fg_providers_build_http_clients_only_through_credential_client` (a text
guard); `p0_fg_no_reachable_credential_reaches_a_curl_command_line`.

Non-claims:
- The Claude query retries 429 and 529 up to three times, so its worst case
  is four 120-second attempts plus 35 s of back-off waits, about 515 s.
- A send error may name the endpoint's URL. Keys travel in headers; the one
  URL that carries a credential, Telegram's (bot token in the path), is
  dropped from every messaging error by `without_url`.
- The unauthenticated GitLab marketplace search (`marketplace_search_gitlab`)
  carries no credential and has no time or size bound.
- The reachability classification above is the workstream's audit; the
  bounds in the table were checked in the code.

**Repaired in P0-FINAL-GATE-CLOSURE: the residual capability-measurement
route (contract C5; Architect review pending).**

- **Before.** `cm_run_ab_validation` was open. It read `GROQ_API_KEY`, else
  `NVIDIA_NIM_API_KEY`, else `OPENROUTER_API_KEY` from the process
  environment (which `save_provider_api_key` fills), and built clients whose
  endpoint is fixed at Groq's, so an NVIDIA or OpenRouter key was addressed to
  Groq. Argument use, the supervisor read, the credential read and client
  construction all ran; only the desktop's empty battery kept a request from
  being sent.
- **Now.** `cm_run_ab_validation` takes no input and returns only the bounded
  `Closure::AmbientResource` reason, before any argument use, credential
  read, client construction or provider contact
  (`app/src-tauri/src/commands/crate_bridges.rs`). In the crate, the
  Groq-endpoint client's key comes only from `GROQ_API_KEY` (`groq_api_key`,
  `evaluation/nim_client.rs`), and the NVIDIA and OpenRouter fall-throughs
  are removed from `run_ab_validation`, `run_batch_evaluation` and
  `execute_validation_run_real`. The evaluator is unchanged; no other `cm_*`
  command changed.
- **Guards.** `p0_fg_c5_ab_validation_route_is_closed_before_any_input`,
  `p0_fg_c5_measurement_clients_take_only_the_groq_key`,
  `p0_fg_c5_desktop_reaches_only_in_memory_measurement`,
  `p0_fg_source_lists_follow_their_directories` (`fg_approval`), the crate's
  `p0_fg_groq_client_key_never_falls_back_to_another_provider`, and six new
  latent-API needles for the runners, the clients and the messaging consent
  reply parser.
- **Non-claims.** The runners and clients stay compiled, with no desktop
  caller. When a client runs outside the desktop, its key is still a curl
  argument.
- **Decision requested.** Request 15. [P0-LINUX-FINAL-R1: decision H. See "Architect decisions (P0-LINUX-FINAL-R1)".]

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

**Corrections (P0-FINAL-GATE-CLOSURE) to the C5C text above** (checked on the
code before item D's repair).

- The "Open" list named only the scriptable srcdoc previews. It omitted:
  - the Research view's frame, which embedded arbitrary remote pages with
    `allow-scripts allow-same-origin allow-forms allow-popups`
    (`app/src/components/browser/ResearchMode.tsx`);
  - the React-mode Builder frame, which embedded a loopback dev server with
    `allow-scripts allow-same-origin`
    (`app/src/components/builder/VisualEditor.tsx`);
  - the code editor (`app/src/pages/CodeEditor.tsx`), whose
    `@monaco-editor/react` loader fetches the Monaco script from a public CDN
    into the privileged document at run time (its default configuration);
  - the remote font stylesheet the privileged document loads
    (`app/index.html`);
  - the Settings page, which sent provider keys from the webview with
    `fetch` to test them (`app/src/pages/Settings.tsx`).
- The C5C guard `p0_002c5c_frontend_html_sinks_are_escaped_and_previews_sandboxed`
  inspected `srcDoc` frames only, so the `src` frames above were outside it.
- No application-command ACL existed: `app/src-tauri/build.rs` called
  `tauri_build::build()` with no app manifest, so Tauri checked no origin for
  application commands, and the invoke key injected into the page was the only
  gate.

**Repaired in P0-FINAL-GATE-CLOSURE (contract D; Architect review
pending).** The sources checked are the pinned tauri 2.10.3 and wry 0.54.4,
cited in `app/src-tauri/src/webview_boundary.rs`.

- **Application-command ACL (origin).** `app/src-tauri/build.rs` passes the
  exact list of registered application commands
  (`src/webview_boundary/app_commands.rs`, all 804, checked against
  `generate_handler!`) to `tauri_build::AppManifest::commands`. Tauri then
  checks every application command against the ACL instead of skipping it.
  `capabilities/app-commands.json` grants them only to the window `main` at
  the local app origin; the capabilities directory holds only that file and
  the core-only `default.json` (pinned). Tauri classifies each request from
  native context, never from a frontend field: the request's `Origin` header
  on the custom-protocol path, or the URL wry reports on the `postMessage`
  path. A non-app origin matches no capability and is refused; a sandboxed
  frame's `Origin: null` is rejected before the ACL.
- **Privileged window.** `build_main_window` builds the one window
  (`"create": false` in `tauri.conf.json`) with:
  - a navigation guard that admits only the exact app origin:
    `tauri://localhost` on Linux and macOS, `http://tauri.localhost` on
    Windows (`https` when the window sets `useHttpsScheme`), the configured
    `devUrl` only when `tauri::is_dev()`, exactly `about:blank` and
    `about:srcdoc`, and `blob:` URLs created by the app origin. `data:`,
    `file:` and every other URL are refused;
  - a new-window handler that denies every request. wry's default without a
    handler also refuses; that default is observed live only on Linux.
- **Frontend.** Every iframe renders inline `srcDoc` with `sandbox=""` (an
  opaque origin, no script), and none is created from script. The remote
  Research frame and the loopback React-mode Builder frame are removed; the
  Monaco editor is not imported; the collaboration WebSocket is disabled;
  Settings no longer sends keys from the webview, and its check says "Format
  looks valid (not verified)"; Builder output is downloaded as a literally
  typed blob.
- **CSP** (`tauri.conf.json`), defence in depth and not proof of the IPC
  boundary: `default-src 'self'`, `script-src 'self'`, `connect-src 'self'
  ipc: http://ipc.localhost`, `frame-src 'self'`, `object-src 'none'`,
  `form-action 'none'`, `frame-ancestors 'none'`, and no third-party origin:
  the remote font stylesheet is removed, and the interface falls back to
  local fonts.
- **Native evidence.** A live native harness
  (`app/src-tauri/tests/webview_boundary_live.rs`, its own `main`) drives the
  real Wry runtime with the production context and window builder, in the
  dev profile and the release (`custom-protocol`) profile. It re-checks every
  refusal after a settle interval, and on Windows exercises the raw
  `postMessage` path from subframes. CI runs it as a blocking step, in both
  profiles, in `test-linux` of `ci.yml` (under Xvfb) and in
  `ci-fast-local.yml`. The workstream reports 20 of 20 local Linux runs per
  profile. [P0-LINUX-FINAL-R1: the Windows and macOS harness steps are kept
  in the manual-only `ci-portability.yml` and are deferred; no Windows or
  macOS result is claimed.]
- **Redirects (correction of the draft).** The draft recorded that on Linux
  the engine followed a server redirect without consulting the navigation
  guard. The final harness shows WebKitGTK consulting the guard for server
  redirects: a `302` from the app origin to a non-app origin is cancelled
  before the target is requested (Linux, 20 of 20 per the workstream). On
  Windows, WebView2's `NavigationStarting` covers main-frame navigations,
  redirects included (source reading); macOS was not observed.
- **Finding for the Architect: on Linux neither layer is sufficient alone.**
  wry's WebKitGTK handler attributes a `postMessage` to the webview's URL
  when the message is handled (`webkitgtk/mod.rs` 640-650), not to the
  document that sent it. With the navigation guard removed (a live negative
  control), a non-app main-frame document that starts a navigation back to
  the app origin can have an invoke accepted in that window. The navigation
  guard, which keeps every non-app document out of the main frame, is
  therefore load-bearing, and the ACL refuses callers that reach IPC by other
  routes. Windows and macOS were not observed. This may be an upstream
  wry or tauri issue (decision request 18).
- **Guards.** `phase0_surface/fg_webview/tests.rs` (11 guards, all pinned by
  the final trust-surface guard), and the extended
  `p0_002c5c_frontend_html_sinks_are_escaped_and_previews_sandboxed`.
- **Non-claims.**
  - A script running inside the privileged document itself still reaches
    every registered command: item D keeps other documents out; it does not
    narrow what the app document may call.
  - A same-origin, non-sandboxed frame would count as the app origin, so the
    frontend rule (`sandbox=""`, no frame created from script) is
    load-bearing; its guards are source-text recognisers.
  - Tauri 2.10.3 exempts `plugin:__TAURI_CHANNEL__|fetch` from the ACL; the
    app uses no IPC channels.
  - The Windows and macOS behaviour described above rests on reading the
    pinned sources only, and is deferred with those platforms
    (P0-LINUX-FINAL-R1).
  - Unavailable in Phase Zero: the Monaco editor, Builder collaboration, the
    Settings live key test, scripted Builder previews and click-to-edit, and
    remote images in notes (`img-src 'self' data:`).
- **Decision requested.** Request 18. [P0-LINUX-FINAL-R1: decision K, Linux
  only. See "Architect decisions (P0-LINUX-FINAL-R1)".]

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
- **Decision requested.** Request 11. [P0-LINUX-FINAL-R1: decision D, Linux
  only. P0-LINUX-FINAL-R2: accepted for Linux after the vault-scope inventory
  guard (`p0_fg_e_vault_key_sources_are_validated_on_what_is_read`), which pins
  exactly the six `VAULT_SCOPES`, that `verify_vault_key` iterates them and
  runs before the one production `SecretsFacade::new`, and that every scope
  literal a production caller passes to the facade is one of the six. See
  "Architect dispositions (P0-LINUX-FINAL-R2)".]

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

**Repaired in P0-FINAL-GATE-CLOSURE (contract G; Architect review
pending).** A caller's boolean, name, `approved_by` value or webview message
is not independent human approval, and no desktop mechanism verifies a human
approver. Operations that need one are unavailable; the approvals that stay
available release only what backend-owned policy already bounds.

- **Transcendent (L6) agents.** Before, one caller could create and approve an
  L6 agent at once; restore registered stored L6 records; and startup
  registered the 12 prebuilt autonomy-6 manifests, which a goal could then
  run. Now each route refuses an L6 agent with `Closure::ApprovalRequired`
  before any state change. An agent counts as L6 when its stored record
  (found under any spelling of its id) or its registration has an autonomy
  level of 6 or above (`is_transcendent_level`, `TRANSCENDENT_AUTONOMY`):
  - `create_agent` at level 6, and `start_agent` and `resume_agent` for an L6
    agent (`app/src-tauri/src/commands/agents.rs`);
  - `approve_consent_request`, `batch_approve_consents` and
    `review_consent_batch` for a `transcendent_creation` request
    (`commands/consent.rs`); such a request can still be denied;
  - goal assignment (`assign_agent_goal`, reached by `execute_agent_goal` and
    every caller of it), `start_autonomous_loop` and a scheduled tick
    (`ScheduledGoalExecutor::execute`, before anything is audited, restarted
    or assigned), in `commands/cognitive.rs`, through `is_transcendent_agent`;
  - tool calls (`tool_call_autonomy`, `commands/crate_bridges.rs`), at any
    claimed level.
  - Restore registers no L6 record, and the prebuilt load
    (`load_prebuilt_agents`, `chat_llm.rs`) loads no L6 manifest, on the
    first run or any later one: each run registers the same prebuilt agents,
    all but the 12 L6 manifests. Stored records are left untouched.
- **Commands that took a caller's word as approval** are closed with
  `Closure::ApprovalRequired`: `nx_consent_respond` and `nx_agent_approve`
  (they answered the closed nx loops' consents with a caller boolean;
  `app/src-tauri/src/nx_bridge/commands.rs`) and `self_rewrite_apply_patch`
  (it applied a patch because the interface asked).
- **Consent resolutions** take no approver name. The consent module records
  the fixed label `desktop-ui (unverified)` (`DESKTOP_UI_RESOLVER`) as
  `resolved_by` and in the audit event, and approvals are no longer forwarded
  to the kernel consent queue as an approver identity.
- **Self-improvement.** `self_improve_approve_proposal` no longer asserts
  `hitl_approved: true` for invariant 9, fabricates a signature, or records a
  canary and an application. The acceptance is recorded as `Proposed` (no
  checkpoint, no canary) and audited as `self_improvement_recorded` with
  `hitl_approved: false` and `applied: false`; the other nine invariants are
  still checked. The report counts only entries whose status says a change
  was applied (none, with this pipeline), `cycles_run` counts runs, and
  `fuel_consumed` is 0.
- **Warden review.** With `governance.enable_warden_review` set (off by
  default), every reviewed action is denied with the bounded reason "Warden
  review is unavailable in Phase Zero: no agent can be verified as the
  Warden" (`WardenReviewEngine::review_with`, `commands/cognitive.rs`). No
  Warden is looked up, by name or otherwise, and no model is queried, so no
  caller-created agent can stand in for it. Before, an inactive Warden meant
  Allow. Of the actions the Phase Zero executor permits, only knowledge-graph
  updates reach the review (`should_apply_governance_review`,
  `kernel/src/actuators/mod.rs`).
- **Manifest schedules.** `create_agent` and `start_agent` refuse a manifest
  schedule the scheduler rejects, before any state change, with the
  scheduler's bounded reason (`check_manifest_schedule`, item K).
- **Stays available (bounded, not human approval).**
  - HITL step approvals and denials, batch, review-each and review
    mode. An approval releases a step only as far as `Phase0AgentExecutor`
    permits: LLM, memory, notification, agent message, HITL request, web
    search and knowledge graph. No actuator reads the `hitl_approved` flag
    (guard).
  - Swarm plan, approve, reject and cancel: a server-held plan under a
    backend-issued ticket, run with LLM-only adapters and the drafts-only
    Herald (C5C).
  - `create_agent` and `start_agent` for levels 0 to 5,
    `override_security_block` (it records a statistic), and permission edits.
  - E6: end of input and read errors abort a computer-use step approval.
- **Guards and tests.** `phase0_surface/fg_approval/tests.rs`
  (`p0_fg_g_transcendent_agents_are_refused_before_any_state_change`,
  `p0_fg_g_goal_loop_and_tool_routes_check_for_transcendent_agents_first`,
  `p0_fg_g_enabled_warden_review_denies_without_any_lookup`,
  `p0_fg_g_l6_checks_use_the_named_bound`,
  `p0_fg_g_caller_asserted_approval_commands_only_deny`,
  `p0_fg_g_consent_decisions_record_no_caller_identity`,
  `p0_fg_g_self_improvement_acceptance_is_recorded_truthfully`,
  `p0_fg_g_self_improvement_report_counts_only_applied_changes`,
  `p0_fg_g_no_actuator_reads_the_hitl_approval_flag`,
  `p0_fg_g_eof_or_a_read_error_is_never_an_approval`); behavioural tests in
  `lib_tests.rs`, among them
  `p0_fg_startup_registers_no_transcendent_agent_on_any_run` (the real
  startup order), `p0_fg_goal_loop_and_tool_routes_refuse_a_transcendent_agent`,
  `p0_fg_transcendent_resume_is_refused_and_changes_nothing`,
  `p0_fg_transcendent_check_matches_every_spelling_of_a_stored_id`,
  `p0_fg_stored_levels_above_l6_count_as_transcendent` and
  `p0_fg_enabled_warden_review_denies_and_no_stand_in_can_allow`; and
  `p0_fg_scheduled_ticks_refuse_a_transcendent_agent_before_any_state_change`
  (`commands/cognitive/scheduled_tests.rs`).
- **Non-claims.**
  - There is no out-of-band approval. Approvals still arrive over webview
    IPC, and a script running in the privileged document can deliver them
    (item D). An IPC approval still releases HITL-gated Phase Zero steps; it
    is not verified approval.
  - `create_agent` accepts levels 0 to 5 without approval; the registered
    level bounds `tools_execute`, whose credential-bearing and
    caller-destination tools are refused at every level (items B and C).
  - The label names a channel, not a person.
  - Stored L6 records and pending transcendent requests stay in the database,
    neither registered nor approvable.
  - Denials are still forwarded to the kernel consent queue.
  - The Warden review setting itself is interface-editable.
  - A stored agent whose manifest schedule the bounded scheduler now refuses
    cannot be started again: no route edits a stored manifest.
- **Decision requested.** Request 14. [P0-LINUX-FINAL-R1: decision G. See "Architect decisions (P0-LINUX-FINAL-R1)". P0-LINUX-FINAL-R2: IPC HITL approval is not security authority; see "Architect dispositions (P0-LINUX-FINAL-R2)".]

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
  (10 s in total, 64 KiB per body); send and poll too (30 s, 4 MiB). Both
  clients follow no redirect and send no `Referer`: the internal review found
  that a redirect carried the Telegram token to its target in the `Referer`
  header, and that is fixed.
- **API Client collections.** `api_client_save_collections` refuses
  collections that are not JSON or that hold a non-empty `authToken`,
  `authPass` or `authKeyValue`, or a credential header entry
  (`Authorization`, `Proxy-Authorization`, `Cookie`, `X-Api-Key`, `Api-Key`,
  `X-Auth-Token`, `Private-Token`); field and header names match in any
  letter case, and every spelling of a header entry's key and value fields is
  checked, so a decoy key cannot hide a credential header. The stored file is
  left as it was.
- **Backups.** `create_backup` (`kernel/src/backup.rs`, IPC `backup_create`)
  never copies `email_oauth/`, `integrations/`, `messaging_tokens/`,
  `deploy_credentials.json`, `oauth_settings.json` or
  `api_collections.json`. It copies the configuration file only when that
  file is an exact configuration encryption envelope or the archive is
  encrypted; otherwise, or when the file cannot be read, it skips it and says
  so in the backup metadata. An
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
- **Decision requested.** Request 10. [P0-LINUX-FINAL-R1: not covered by
  decisions A to M. P0-LINUX-FINAL-R2: not an active Phase Zero blocker while
  these flows stay closed; see "Architect dispositions (P0-LINUX-FINAL-R2)".]

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
- **Nexus Code checks start no process.** The Nexus Code diagnostics the
  desktop runs (`diagnose_for_desktop`: `ollama`, `git`, `rg`) and Nexus
  Code's provider auto-detection find programs with an in-process `PATH`
  lookup (`nexus_code::setup::program_on_path`, which skips relative entries)
  and run no `which`. The standalone `nx` (withdrawn) keeps its behaviour.
- **Remaining `which` probes.** The computer-use readiness probe
  (`nx_computer_use_status`), `which sd` (above), and `which rg` in the
  Nexus Code tools (`nexus-code/src/tools/search.rs`, latent in the desktop)
  remain, under decision request 1.
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
- **Decision requested.** Requests 1 and 17. [P0-LINUX-FINAL-R1: decisions A
  and J. See "Architect decisions (P0-LINUX-FINAL-R1)".]

## J. Shipped non-desktop binaries

The installers ship only the desktop app (`app/src-tauri/tauri.conf.json`
bundle). The desktop never reaches the binaries below.

[Correction (P0-FINAL-GATE-CLOSURE): the release workflow's installers ship
only the desktop app (`.github/workflows/release.yml` publishes the Windows,
Linux and macOS desktop installers; the bundle has no sidecar). Until
P0-FINAL-GATE-CLOSURE the repository also held standalone install and
packaging recipes for the binaries below: the root and `nexus-code` install
scripts, the Homebrew formula, the WiX source, the systemd and launchd files,
`scripts/build_linux_deb.sh`, `scripts/build_macos_release.sh`,
`scripts/build_windows_msi.ps1` and the GitLab `release-build` job. The GitHub
release workflow also packaged `nexus-cli` until `39701bff`.]

- **`crates/nexus-server` (`nexus-server`): a BLOCKER at C5C; withdrawn by
  P0-FG1, integrated at `71c47acb`.**
  - **Status.** P0-FG1, with P0-FG1-R1 and the reviewed CI composition, is
    integrated into `rebuild/phase0-trust-boundary` at `71c47acb` by
    fast-forward (the Architect-approved candidate). Its designated
    post-integration hosted validation, run #108, passed on all five jobs
    (Linux, Windows, macOS, frontend, Python) and tested that exact commit.
    J1 does not await integration or Architect review.
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
      substitutes a normal testing pattern, and it did not resolve the
      duplicate binary name, which was then build debt (item K; repaired
      since by P0-FG1-R1, integrated at `71c47acb`).
    - **Hosted failure (run #106).** The first complete hosted run of the
      composed FG1 and CI candidate (`2b47bb09`) failed on Windows before any
      test ran. Linking this package's binary stopped with `LNK1104: cannot
      open file ...\target\debug\deps\nexus_server.exe`, the output path the
      two binaries shared (item K).
    - **Naming repair (P0-FG1-R1; integrated at `71c47acb`).** The protocols binary target is renamed
      `nexus-protocols-server`; its source file is unchanged. J1 is then the
      only workspace binary named `nexus-server`. The identification and the
      Windows fallback above are unchanged and stay as defensive checks; the
      fallback's narrow acceptance is not widened. [Update
      (P0-FINAL-GATE-CLOSURE): integrated at `71c47acb`; hosted runs #107 and
      #108 on that commit succeeded.]
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
  P0-FG1-R1 (integrated at `71c47acb`).**
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
  - [Corrections (P0-FINAL-GATE-CLOSURE): the unauthenticated list omitted the
    router's fallback, `serve_frontend`, which served `NEXUS_FRONTEND_DIST` or
    else the working directory's `app/dist`. And the root `helm/nexus-os`
    chart built nothing: it deployed a prebuilt image from the project's
    GitLab container registry (tag defaulting to the chart's `appVersion`,
    `10.5.0`), with CORS open to any origin and the JWT secret `changeme` by
    default.]
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
- **protocols `nexus-os` (as at C5C; withdrawn since, J3 below).** The
  `Makefile` target `nexus-os` built it from the `nexus-protocols` package,
  and `install.sh` installed a `nexus-os` binary from release assets. It ran
  the same server runtime as the protocols server (`nexus-server`;
  `nexus-protocols-server` after P0-FG1-R1). [P0-FINAL-GATE-CLOSURE: the
  build command formerly quoted here is removed; both recipes now fail
  without building or installing anything.]
- **Build note, not executed.** The root `Dockerfile` copies every workspace
  member except `nexus-code/`, which is a member, so its image build appears
  unable to load the workspace. `deploy/Dockerfile` had the same omission;
  P0-FG1 withdrew it.
- **Decisions (as asked at C5C, and their current state).**
  - J1 (`crates/nexus-server`): withdrawn by P0-FG1, integrated at
    `71c47acb`; its designated post-integration hosted validation (run #108)
    passed. J1 does not await integration or Architect review.
  - The others needed review; they are withdrawn on the closure candidate,
    below.

**Repaired in P0-FINAL-GATE-CLOSURE (contracts J2–J5: standalone surfaces
withdrawn; accepted by decision L, request 19 dispositioned by
P0-LINUX-FINAL-R2; J2–J5 not declared complete until the closure candidate is
integrated and validated).** The Architect's disposition for
Phase Zero is withdrawal of standalone execution and deployment paths that
bypass the governed desktop, on the J1 pattern. J1's own identification
checks are unchanged.

- **Withdrawn entry points (18 binaries in all, J1 included).** Each entry
  point writes one fixed message naming itself (`<name>: unavailable during
  Phase Zero; deployment withdrawn` for the servers, `…; standalone use
  withdrawn` for the others) and exits with status 69. It reads no argument,
  environment variable, configuration or credential first, and starts no
  runtime, socket or process. No flag, variable or alias restores it; target
  names and paths are unchanged, so workspace builds still compile them.
  - J2 and J3: `nexus-protocols-server` (`protocols/src/bin/nexus-server.rs`)
    and its alias `nexus-os` (`protocols/src/bin/nexus-os.rs`).
  - J4: `nexus-cli` (`cli/src/main.rs`). Before, it ran every `nexus` command
    with the invoking user's authority in its working directory: `voice` ran
    `python3 jarvis.py` from the working directory's or the build checkout's
    `voice/`; `agent start coding-agent` ran the manifest's test command
    through `sh -lc` or `cmd /C` with every write and run approved; `agent
    start social-poster` published unless `--dry-run`; `setup` rewrote the
    Nexus configuration.
  - J5: `nx` (`nexus-code/src/main.rs`). Before, it spawned `which claude` and
    `claude --version` at startup and read the working directory's
    `NEXUSCODE.md` and `.nxrc` (which could set `auto_approve`), and its tools,
    MCP servers and computer use ran rooted at the working directory. The
    desktop's Nexus Code entry points are unchanged (C5C, item I).
  - Alternate entry points (coordinator decision D1): `coding-agent` and
    `social-poster-agent`.
  - The computer-use harness (coordinator decision D2): `nx-screen`,
    `nx-input`, `nx-agent`, `nx-govern`, `nx-learn` (screen capture to any
    path, OS input, an autonomous capture, vision and input loop).
  - Six conductor benchmarks that put a provider key on curl's command line
    (`benchmarks/conductor-bench`): `nim-cloud-bench`, `cloud-models-bench`,
    `inference-consistency-bench`, `local-vs-cloud-battle` and
    `real-agent-validation` read `GROQ_API_KEY` and sent it to the NVIDIA
    endpoint as a bearer token; `real-battery-validation` sent it to Groq
    through the capability-measurement client. Their reports stay as
    historical results.
- **Recipes and packaging withdrawn.** The root `Dockerfile` and
  `nexus-code/Dockerfile` fail at their first build step; the root Compose
  file defines no services; the root chart's only template is an
  unconditional `fail` (its 13 templates are removed, and it is marked
  deprecated); `make nexus-os`, both `install.sh` scripts,
  `nexus-code/scripts/run_benchmarks.sh` and the three `scripts/build_*`
  scripts print the withdrawal and exit 1; the systemd unit has no
  `[Service]` or `[Install]` section; the launchd job names no program and is
  disabled; the Homebrew formula raises when loaded; the WiX source stops at
  `<?error?>`; the GitLab `release-build` job keeps its manual trigger but
  fails with no artifact. The deployment guide, user guide, README and
  benchmark documents say the surfaces are withdrawn and give no command for
  them.
- **Libraries preserved, not governed.** The libraries behind the withdrawn
  binaries remain (the desktop uses some): `nexus_protocols` (including
  `server_runtime` and `http_gateway`), `nexus_cli`, the standalone
  `nexus_code` entry points, `coding_agent`, `social_poster_agent`,
  `nexus_computer_use` and the capability-measurement clients. None is
  claimed governed.
- **Guards.** `phase0_surface/fg_standalone/tests.rs` (10 tests) inventories
  every effective binary target (30: 18 withdrawn, 1 desktop, 4 developer,
  7 benchmark), example target (2) and bench target (5), and requires every
  withdrawn one to be a J1-pattern entry with a unique message; pins the entry
  APIs no other production source may name (13 alias needles, including the
  gateway module and its router constructor); pins the build scripts of the
  packages with a withdrawn entry point; pins the 14 recipe files, searching
  dot directories and CI configurations too; forbids any workflow to build,
  install or publish a withdrawn binary, an image or a chart; and requires
  the README to name every withdrawn binary. Per-package
  `tests/phase0_withdrawal.rs` files run each withdrawn executable (after
  identifying it by source and bytes, with a cleared environment and a
  deadline) and pin its recipes and documents: `protocols` (11), `cli` (11),
  `nexus-code` (8), `coding-agent`, `social-poster` and `nexus-computer-use`
  (2 each), `conductor-bench` (3).
- **Non-claims.**
  - Nothing stops, removes or changes an existing deployment, container,
    image, volume, Helm release, installed package, service unit or published
    artifact, and no credential is revoked. Units installed earlier keep
    running their binary as before.
  - The withdrawn Dockerfiles still pull their base image before the failing
    step.
  - `nexus-mcp` is not made safe to expose.
  - The kept developer and benchmark binaries are not governed (D3).
- **Decision requested.** Requests 2 and 19. [P0-LINUX-FINAL-R1: decisions M
  and L. See "Architect decisions (P0-LINUX-FINAL-R1)".]

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
  Repaired by P0-FG1 (test-only; Architect scope extension) and integrated
  with it at `71c47acb`.
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
  `nexus-protocols`). Build debt at C5C; repaired by P0-FG1-R1, integrated
  at `71c47acb`, and covered by the designated post-integration hosted
  validation (run #108).
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
    no `nexus-server`.
  - **Status.** P0-FG1-R1 is integrated at `71c47acb`. Hosted runs #107 and
    #108 on that commit succeeded, #108 being the designated
    post-integration validation, and the #108 logs show no output-filename
    collision warning. The repair does not await integration or Architect
    review.

**Corrections and repairs (P0-FINAL-GATE-CLOSURE, contract K; Architect review
pending).**

- **Windows `executes_python_code`: evidence corrected.** No retained hosted
  log shows this test failing. The 26 hosted Windows job logs the workstream
  retained (runs #79–#108, including #107 and #108 on `71c47acb`) all show
  it passing; the reliability workstream counted 43 such jobs from
  runs #16–#108. The `CommandTimeout { seconds: 5 }` recorded above has no
  retained hosted evidence. The only timeout in this test family is the
  sibling `native_python_and_node_execute_within_workspace` (10 s,
  `CommandTimeout { seconds: 10 }` at `code_exec.rs:568`) in run #87 on
  `889f1e52`; run #86 passed on the same commit. A plausible, unproven
  mechanism: the deadline starts at spawn and covers interpreter start-up
  while about 2,100 kernel tests run in parallel. The Phase Zero executor
  never calls this actuator. No code or test changed. Proposed disposition:
  monitored debt (decision request 7). [P0-LINUX-FINAL-R2: the Architect's
  disposition of request 7 is under "Architect dispositions
  (P0-LINUX-FINAL-R2)"; no general reliability claim is made for this
  actuator.]
- **GPU-host voice CLI: repaired.** On the self-hosted runner, a GPU driver
  and library mismatch made torch's CUDA probe print a warning (CUDA Error
  804, seen in the fast-local Python job log), which broke the voice CLI's
  stderr contract (stderr carries one JSON error object, or nothing).
  `voice/stt.py` probed CUDA through torch, which is not the Whisper backend.
  `detect_gpu()` now uses CTranslate2's device count and never imports torch;
  without CTranslate2 only the `nvidia-smi -L` hint remains, for the model
  tier. A stand-in torch that warns like that host holds the contract in
  `voice/tests/test_stt_cli.py`. Every voice test module now keeps the
  Hugging Face hub offline (`voice/tests/_offline.py`), so the tests fetch
  no model from the hub; a test that needs an uncached model skips. The
  fast-local workflow keeps its CUDA mask (coordinator decision), and the
  host driver is an Owner matter (decision request 7). [P0-LINUX-FINAL-R2:
  Owner infrastructure; no reliability claim for that GPU host.]
- **Voice dependencies pinned (P0-LINUX-FINAL-R1).** Both Linux gates
  (`test-python` in `ci.yml` and `fast-python` in `ci-fast-local.yml`)
  install the voice test environment from
  `voice/requirements-linux-py311.lock`: a complete, hash-pinned set of 67
  packages, installed with pip 26.2.1 as
  `pip install --require-hashes --no-deps`, followed by `pip check`. Nothing
  is resolved at install time and every downloaded file must match a listed
  hash; the hashes are PyPI's sha256 digests for the pinned versions. The
  tests stay offline for models. Only Linux x86_64 with Python 3.11 is
  claimed. [This supersedes the "unpinned" note of decision request 8.]
- **Resource bounds:** see "Resource bounds" below.

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

[The list above is the C5C state. The review for P0-FINAL-GATE-CLOSURE also
found two unbounded surfaces it did not name: `run_adversarial_session`
passed a caller's `u32` round count to `Vec::with_capacity` (about 137 GB for
`u32::MAX`, which aborts the process), and `temporal_fork` /
`set_temporal_config` let the caller set the fork count and token budget.]

**Bounded in P0-FINAL-GATE-CLOSURE (contract K; Architect review pending).**
Each surface below was reachable from any script in the webview. Every bound
refuses rather than clamps, and nothing is deleted to stay within one.

- **Frontend error log** (`log_frontend_error`, also called by the global
  error handlers; `app/src-tauri/src/commands/frontend_errors.rs`). Each field
  is cut to 8 KiB on a character boundary with a marker, on stderr and in the
  log. `frontend_errors.log` is appended only while it stays within 4 MiB; a
  record that would pass the cap is dropped whole, with one stderr notice per
  process. Records are only appended: nothing is rotated, truncated or
  deleted, and an existing larger log is kept.
- **Build budget store** (`builder_record_build`, `builder_set_budget`,
  `builder_set_remaining` and the governed plan's cost record;
  `agents/web-builder/src/budget.rs`). Text fields of at most 256 bytes with
  no control characters; a provider name of 1 to 32 ASCII letters, digits,
  `.`, `_` or `-`; finite amounts from 0 to 1,000,000 USD; elapsed time at
  most one week; counts at most 10^9; at most 16 providers and 1,000 records
  (a full history refuses a new record); a file of at most 4 MiB. No write
  replaces a file that is unreadable, unparsable or oversized. Before, two
  records costing about 1.7e308 wrote `null`, and the next write replaced the
  whole history with defaults. A plan's cost record keeps a cleaned project
  name cut to the text bound, so a model-supplied name never costs the
  record, and a record the store still refuses is reported on stderr in one
  bounded line.
- **Stress personas** (`stress_generate_personas`,
  `app/src-tauri/src/commands/autopilot.rs`): 1 to 1,000, refused before the
  simulator is locked.
- **Parallel simulations** (`run_parallel_simulations`): 1 to 10 variants. The
  desktop refuses before the model is built, the seed is parsed or any variant
  starts (`commands/consent.rs`, with the kernel's constant), and the kernel
  refuses again before any thread or model call
  (`kernel/src/simulation/runtime.rs`).
- **Dilated sessions** (`run_dilated_session`): 1 to 50 iterations, refused by
  the desktop before the configuration is read and by the kernel before any
  model call (`kernel/src/temporal/dilation.rs`). The dilator's settings are
  copied out of its lock, so no lock is held across provider calls.
- **Adversarial arena** (`run_adversarial_session`): 1 to 50 rounds. The
  desktop checks the kernel's bound, and the arena refuses again before any
  allocation, computation or record (`kernel/src/immune/arena.rs`,
  `try_run_session`). The IPC reply's JSON is unchanged for accepted runs.
- **Temporal forks** (`temporal_fork`, `set_temporal_config`): 1 to 10 forks and
  a token budget of 1 to 200,000, refused before a provider is built or the
  model is called, and never stored. The kernel's `fork_and_evaluate`
  (`kernel/src/temporal/engine.rs`) also refuses an out-of-bound
  configuration first, whichever path stored it.
- **Agent schedules** (`AgentScheduler`, `kernel/src/cognitive/scheduler.rs`).
  A schedule fires at most once per minute: its seconds field must be one
  value from 0 to 59, and a five-field expression runs at second 0. A
  refused schedule is refused before anything is spawned, and a refused
  re-registration keeps the existing schedule. A sub-minute schedule stored
  earlier is not registered at startup (logged).
  `register_agent` and `validate_cron` share one parser, and every refusal is
  one of a few fixed texts that repeat no part of the caller's expression.
  `create_agent` and `start_agent` refuse a manifest schedule the scheduler
  rejects before any state change (item G), and `start_autonomous_loop`
  accepts only intervals from 60 to 3,599 seconds, which the bounded
  scheduler can express.
- **Scheduled ticks** (`ScheduledGoalExecutor::execute`,
  `commands/cognitive.rs`). A tick is skipped, and audited as
  `scheduled_execution_skipped` (`agent_loop_active`), while the agent's
  desktop loop runs. The signal is the loop's cancellation entry, and a loop
  that ends removes that entry only if it is still its own (`Arc::ptr_eq`),
  so the Stop button and the scheduler always see the newest running loop. An
  L6 agent's tick is refused first (item G).
- **Test isolation.** The cognitive loop records its L6 cooldown and its
  algorithm selection in the host's injected state database (the same file
  in production), so a test on the in-memory application state writes no
  database under the identity home. No desktop test builds the real
  application state (guard `p0_fg_a_no_desktop_test_builds_the_real_application_state`),
  and the configuration's protection recorder is not installed in test
  builds.
- **Guards and tests.** `phase0_surface/fg_reliability/tests.rs` (12 guards,
  among them `p0_fg_k_approved_limits_are_pinned` and
  `p0_fg_k_an_in_memory_state_loop_writes_no_identity_home_database`),
  `p0_fg_autonomous_loop_intervals_outside_the_schedule_bound_are_refused`
  and `p0_fg_sub_minute_manifest_schedules_fail_create_and_start`
  (`commands/cognitive/scheduled_tests.rs`), the kernel, budget and
  frontend-error tests, and the desktop tests
  `p0_fg_parallel_simulation_variants_are_bounded_before_any_model_call` and
  `p0_fg_adversarial_session_rounds_are_bounded_before_any_work`. The
  workstreams recorded a mutation control for each bound.
- **Non-claims.**
  - These are per-request bounds, not rate limits.
  - The scheduled-tick check and the loop spawn are not atomic.
  - Older overlapping loops of one agent keep running and can be stopped only
    through the kernel's stop path, not the map entry.
  - The temporal engine's lock is still held across the model calls of one
    fork request.
  - Two desktop processes can each overshoot the log cap by at most one
    record, and the frontend-error stderr output is not rate-limited.
  - An existing budget file larger than 4 MiB now displays defaults. The
    internal review recorded that a FIFO at the budget file's path blocks
    its read.
  - A refused arena run seen through the older `run_session` API (the
    benchmark's) reports a NaN win rate.
  - The Warden review reads the configuration (`load_config`), which creates
    a first-run `config.toml` under the identity home if none exists, so a
    test that reaches it can still write that file.
- **Decision requested.** Request 16. [P0-LINUX-FINAL-R1: decision I. See "Architect decisions (P0-LINUX-FINAL-R1)".]

## Dependency evidence (P0-FINAL-GATE-CLOSURE)

**Method.** Security advisories were measured on the closure candidate's own
lockfiles, never on `main`'s: before (the approved candidate `71c47acb`) and
after, on the final candidate's lockfiles (`d1735577`), with cargo-audit
0.22.1 and cargo-deny 0.19.6 against one advisory-database snapshot (1,273
advisories), and `npm audit --package-lock-only`. The cargo-deny figure was
taken on the lockfile after the `openidconnect` removal; the final lockfile
differs from it only by one dependency edge to an already-locked package
(`webkit2gtk`, item D), which adds no package. The internal reviews
re-checked the lockfile changes independently.

| Lockfile | Tool | Before (`71c47acb`) | After |
|---|---|---|---|
| `Cargo.lock` | cargo-audit | 19 vulnerabilities, 26 warnings, 1,110 packages | 7 vulnerabilities, 25 warnings (13 unmaintained, 12 unsound), 1,086 packages |
| `Cargo.lock` | cargo-deny (advisories) | 16 errors (13 vulnerability, 3 unmaintained) | 10 errors (7 vulnerability, 3 unmaintained) |
| `app/package-lock.json` | npm audit | 14 (2 low, 6 moderate, 6 high) | 6 (5 moderate, 1 high) |
| `nexus-website/package-lock.json` | npm audit | 12 (1 low, 2 moderate, 9 high) | 12, unchanged |
| `scripts/page-audit/package-lock.json` | npm audit | 8 (high) | 8, unchanged |
| `packaging/builder-toolchain/package-lock.json` | npm audit | 0 | 0 |

cargo-deny's bans, licenses and sources checks report no error. cargo-deny
already ignored RUSTSEC-2023-0071 (rsa), so removing rsa lowered only the
cargo-audit count; cargo-deny reports the three unmaintained crates as
errors, cargo-audit as warnings.

**Updates taken** (precise and semver-compatible; `Cargo.toml` unchanged
except the removal):

- crossbeam-epoch 0.9.18 → 0.9.20 (RUSTSEC-2026-0204);
- h2 0.4.13 → 0.4.16 (RUSTSEC-2026-0258 on the 0.4 line);
- quinn-proto 0.11.14 → 0.11.15 (RUSTSEC-2026-0185);
- tar 0.4.44 → 0.4.45 (RUSTSEC-2026-0067, RUSTSEC-2026-0068);
- rustls 0.23.37 → 0.23.45 (RUSTSEC-2026-0285), with aws-lc-rs 1.16.1 →
  1.18.1, aws-lc-sys 0.38.0 → 0.45.0 (RUSTSEC-2026-0044, RUSTSEC-2026-0048)
  and rustls-webpki 0.103.13 → 0.103.15;
- plist 1.8.0 → 1.10.0, with quick-xml 0.38.4 → 0.41.0 (RUSTSEC-2026-0194,
  RUSTSEC-2026-0195 on 0.38.4);
- the unused `rmcp` dependency of `nexus-protocols` removed (RUSTSEC-2026-0189),
  which also dropped rmcp-macros, pastey and schemars_derive 1.2.1;
- the unused `openidconnect` dependency of `nexus-auth` removed
  (RUSTSEC-2023-0071, rsa 0.9.10): no source imported it. The lockfile was
  only pruned: 20 packages removed (rsa, openidconnect, oauth2 and their
  exclusive dependencies, the yanked spin 0.9.8 among them), with no version,
  source or checksum change;
- npm: 29 development-only entries of `app/package-lock.json`; `package.json`
  unchanged.

Cargo re-resolved 16 unrelated ranged dependency edges while applying the
updates; the workstream kept those edges at their base versions and verified
them against each parent's published requirements and with `--locked` builds.

**Residual advisories: proposed dispositions for the Architect.** The
implementer accepts no risk; each line is a proposal (decision request 3).
[P0-LINUX-FINAL-R1: these five rows are now the `deny.toml` exception set;
see "Linux security gate" below.]

| Advisory | Package | Dependency path | Reachability | Proposed disposition |
|---|---|---|---|---|
| RUSTSEC-2026-0193, RUSTSEC-2026-0213 (sanitizer bypasses) | ammonia 4.1.2 | `web-builder-agent` → desktop | Only `design_import::import_design` uses it, and no production code calls that; its desktop command `builder_import_design` is closed | Update to 4.1.4 (adds four crates), or accept for Phase Zero with this reason |
| RUSTSEC-2026-0258 (unbounded empty DATA frames) | h2 0.3.27 | `readability` 0.3 → `reqwest` 0.11 → `hyper` 0.14 → `nexus-kernel` | The kernel calls only `readability::extractor::extract` on text it already holds and builds no reqwest 0.11 client; no fix exists on the 0.3 line | Accept until `readability` is replaced |
| RUSTSEC-2026-0194, RUSTSEC-2026-0195 | quick-xml 0.30.0 | `zbus_xml` → `atspi` 0.24 → `nexus-ui-repair` | Developer tool only, not shipped (D3) | Accept with D3, or withdraw the tool |
| RUSTSEC-2026-0269, RUSTSEC-2026-0222 | wasmtime 43.0.2 | `nexus-sdk`, `nexus-protocols`, `nexus-benchmarks` | `wasmtime-wasi` is not in the lockfile; the SDK's WASM sandbox is latent (C5A) | Accept until a planned major upgrade (46.0.3+ or 47.0.4+) |
| RUSTSEC-2026-0247, RUSTSEC-2026-0250, RUSTSEC-2026-0251 (unmaintained) | bitmaps 2.1.0, im-rc 15.1.0, sized-chunks 0.6.5 | `wasm-compose` → wasmtime 43.0.2 | As wasmtime | Accept with wasmtime |

Warning-level entries: 25 (13 unmaintained, 12 unsound); the yanked spin
0.9.8 left with the `openidconnect` removal. Fixes exist but were not taken
for anyhow, event-listener, memmap2 and rand 0.8, 0.9 and 0.10; lru, glib, scc
and rand 0.7 have no semver-compatible fix (decision request 3). [R1: the
six fixes were taken; see below.]

npm residue in `app/package-lock.json` (6, all but two development tooling):
vite 5 and esbuild (fix is a major upgrade), vitest and `@vitest/mocker`;
`monaco-editor` and its `dompurify` are packages of the Monaco editor, which
item D no longer imports; `@monaco-editor/react` stays declared in
`app/package.json`, so they stay in the lockfile (decision request 5).
`nexus-website` and `scripts/page-audit` are measured only (decision request
6). [R1: Monaco, yjs and y-websocket removed; 4 findings remain; both
projects out of scope. See below.]

**Audit governance** (decision request 4). `audit.yml` runs on pushes to
`main` and weekly only, so it never scans a closure candidate, and its five
`--ignore` IDs match nothing in the candidate's audit. cargo-audit does not
read the root `audit.toml` (it reads `.cargo/audit.toml`, which does not
exist). `deny.toml` holds 13 ignores that match nothing any more: eight GTK3
IDs; RUSTSEC-2026-0044, -0048, -0067 and -0068, fixed by the updates; and
RUSTSEC-2023-0071, since rsa is gone (its ignores in `audit.toml` and
`.gitlab-ci.yml` are stale too). Its
note that RUSTSEC-2026-0097 has "no semver-compatible bump" is
contradicted by the advisory, which lists rand 0.8.6, 0.9.3 and 0.10.1 as
fixed. `.gitlab-ci.yml` ignores RUSTSEC-2026-0114, which matches nothing, and
installs both scanners unpinned with `|| true`. The four ignore lists
(`audit.yml`, `audit.toml`, `deny.toml`, `.gitlab-ci.yml`) diverge.
[Superseded by P0-LINUX-FINAL-R1 repair B; see the next block.]

**Linux security gate (P0-LINUX-FINAL-R1 repair B).** The measurements above
are historical. At the R1 head:

- **One gate.** `scripts/security-audit.sh` (`set -euo pipefail`) runs
  cargo-audit 0.22.1 and cargo-deny 0.19.6, both pinned and installed with
  `--locked`, over the candidate's `Cargo.lock`. The Linux final gate runs it
  in the `security-audit-linux` job of `ci.yml` with fresh advisory
  databases; `ci-fast-local.yml`, `audit.yml`, the GitLab `security-audit`
  job (`allow_failure: false`) and `scripts/ci-local.sh` run the same script.
  No scanner call is masked with `|| true`.
- **One exception set.** `deny.toml` holds the only exception set, 11 IDs,
  each with its package, path and Phase Zero reachability:
  - RUSTSEC-2026-0193 and -0213 (ammonia 4.1.2);
  - RUSTSEC-2026-0258 (h2 0.3.27);
  - RUSTSEC-2026-0194 and -0195 (quick-xml 0.30.0);
  - RUSTSEC-2026-0269 and -0222 (wasmtime 43.0.2);
  - RUSTSEC-2026-0316 (wasmtime 43.0.2; accepted narrowly, see below);
  - the unmaintained RUSTSEC-2026-0247, -0250 and -0251 (bitmaps, im-rc and
    sized-chunks, through wasmtime's `wasm-compose`).

  The script passes the same IDs to cargo-audit as `--ignore`; cargo-deny
  runs with `-W unmaintained -W unsound -D advisory-not-detected`.
- **What fails the gate.** Any vulnerability outside the set; a stale
  exception; a missing scanner or one that is not the pinned version; any
  install or scanner error; an empty exception set. Unmaintained, unsound and
  yanked findings are warnings across the whole graph.
- **Removed.** The root `audit.toml`, which cargo-audit never read (it reads
  `.cargo/audit.toml` or `$CARGO_HOME/audit.toml`); the stale rsa
  RUSTSEC-2023-0071 ignores; the 12 `deny.toml` ignores that matched nothing;
  the unreferenced `scripts/refresh_rust_audit_toolchain.sh`.
- **Six precise fixes** for crates flagged unsound (semver-compatible,
  `Cargo.toml` unchanged): anyhow 1.0.102 → 1.0.103, event-listener 5.4.1 →
  5.4.2, memmap2 0.9.10 → 0.9.11, rand 0.8.5 → 0.8.6, 0.9.2 → 0.9.3 and
  0.10.0 → 0.10.1. This resolves the `deny.toml` note on RUSTSEC-2026-0097
  quoted above.
- **Residual warnings, recorded with their paths** (no compatible fix):

  | Package | Kind | Path |
  |---|---|---|
  | glib 0.18.5 | unsound | `webkit2gtk` → desktop |
  | lru 0.12.5 | unsound | `ratatui` → `nexus-code` |
  | scc 2.4.0 | unsound | `serial_test` (tests only) |
  | rand 0.7.3 | unsound | `tauri-build` → `tauri-utils` → `kuchikiki` → `phf_codegen` (build time) |
  | sized-chunks | unsound | `wasm-compose` → wasmtime |
  | fxhash | unmaintained | `scraper` → `nexus-ui-repair` |
  | number_prefix | unmaintained | `hf-hub` → `indicatif` |
  | paste | unmaintained | `ratatui` |
  | proc-macro-error | unmaintained | `glib-macros` |
  | rustls-pemfile | unmaintained | `reqwest` 0.11 → `readability` |
  | unic-* (5 crates) | unmaintained | `tauri-build` → `urlpattern` (build time) |

- **npm (`app/`).** `@monaco-editor/react`, `yjs` and `y-websocket` were
  removed from `app/package.json` (nothing in `app/src` imported them),
  which removed `monaco-editor` and `dompurify` from the lockfile. Four
  findings remain, all development tooling outside the bundle: vite 5.4.21
  (high) and esbuild 0.21.5 (moderate), whose fix is a major vite upgrade;
  vitest 4.1.2 and `@vitest/mocker` (moderate).
- **Scope.** `nexus-website` and `scripts/page-audit` are not built or
  shipped by the Linux product and are out of Phase Zero scope.
- **RUSTSEC-2026-0316: accepted for Linux Phase Zero (Architect decision,
  P0-LINUX-FINAL-R1), narrowly.** wasmtime 43.0.2, low severity
  (GHSA-jqpg-j7w6-42pr, CVSS 1.0): Wasmtime's dynamically typed `Val` API can
  allocate beyond the hostcall fuel limit when lifting guest values into host
  allocations ("Dynamic record lifting can allocate beyond the hostcall fuel
  limit"); statically typed APIs are unaffected. Nexus's Wasmtime use is the
  core `Module`/`Linker`/`Store` path with typed entry functions
  (`get_typed_func::<(), ()>`), not the component-model `Val`/`Func` API, and
  the SDK WASM sandbox is latent, unreachable from the Linux Phase Zero
  desktop (`WasmtimeSandbox`, `WasmAgent` are latent-API needles). No patched
  43.x release exists; the patched lines are 36.0.16, 48.0.3 and 49.0.1, and
  the move off 43.x (a major-version transition) is deferred; nothing here
  suggests a downgrade to 36 or claims wasmtime 43.0.2 is generally safe. It
  is the one additional entry in `deny.toml` (11 IDs); the guard
  `p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api` fails if a
  production source uses Wasmtime's component API, if the SDK sandbox stops
  using typed entry functions, if the latent-API needles are removed, or if
  the exception's recorded reasons change. The acceptance lapses if that API
  use or reachability changes.

  **The guard's invariant (P0-LINUX-FINAL-R2, narrowed by
  P0-LINUX-FINAL-R2A).** While the exception is accepted:

  - production Nexus does not use Wasmtime's `component` module, where the
    affected dynamic `Val`/`Func` API lives: no direct, spaced, multiline,
    grouped or nested path to it, and no `component as …` inside a group;
  - production Nexus does not alias, rename or glob-import the Wasmtime
    crate while the exception is accepted: `[pub] use wasmtime as …`,
    `[pub] use wasmtime::{self as …}` and `[pub] extern crate wasmtime as …`
    (raw identifiers included) are refused as declarations, whether or not
    the same file names `component`, so no other file can reach the module
    through a name that does not spell `wasmtime`; `wasmtime::*` and
    `wasmtime::{*}` are refused;
  - no workspace package renames its `wasmtime` dependency, as read from
    Cargo's effective metadata (`cargo metadata --no-deps --format-version 1
    --locked` for this workspace, run with the Cargo that built the test);
  - direct core-Wasm use (`Engine`, `Linker`, `Module`, `Store`,
    `get_typed_func`) stays allowed.

  - The source check removes comments first and keeps `cfg`-disabled code in
    scope. It is a text check, not a Rust name resolver, and adds no parser
    dependency. Refusing every crate alias is what closes a cross-file
    re-export (`pub use wasmtime as wt;` in one file, `wt::component` in
    another).
  - Its probes: 14 crate aliases, 2 glob imports and 15 component-module
    spellings must be caught, and 9 core, item-rename or comment-only sources
    must be accepted.
  - Negative controls, each restored byte for byte, all failed the guard for
    the named reason:
    - `sdk/src/wasm_agent.rs`: a `cfg`-disabled nested component group and a
      grouped `component as` alias (component module);
    - the same file: `#[cfg(any())] pub use wasmtime as wt;` (crate alias);
    - `sdk/Cargo.toml`: `wt = { package = "wasmtime", version = "43.0.2" }`
      ("nexus-sdk": wasmtime dependency renamed; `Cargo.lock` untouched).
  - The pins on the core import, `get_typed_func`, the latent-API needles and
    the exception's recorded reasons are unchanged.
  - This is a Nexus non-use and reachability invariant. It is not proof that
    wasmtime 43.0.2 is generally safe.

Evidence: the gate's exit 0 against the reviewed snapshot, the two negative
controls (a removed exception and a stale exception each fail the gate) and
the npm count of 4 are the workstream's recorded runs; this document did not
re-run them.

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

**P0-FINAL-GATE-CLOSURE update.**

- **154 IPC commands closed** on the closure candidate (counted from
  `CLOSED_COMMANDS` in `app/src-tauri/src/phase0_surface/tests.rs` at
  `d1735577`): C5A 125, C5C 7, P0-FINAL-GATE-CLOSURE 22. Of the 804 registered
  commands, 650 stay open. Item D closes no command; it restricts which
  documents may call them. The 22, each taking no input and returning one
  bounded reason:
  - `NetworkDestination` (11): `api_client_request`; `a2a_discover_agent`,
    `a2a_send_task`, `a2a_get_task_status`, `a2a_cancel_task`;
    `a2a_crate_send_task`, `a2a_crate_get_task`, `a2a_crate_discover_agent`;
    `mcp_host_connect`, `mcp_host_call_tool`;
    `builder_theme_extract_from_url`;
  - `PeerTransfer` (1): `nexus_link_send_model`;
  - `CredentialTransport` (1): `perception_init`;
  - `HelperLaunch` (1): `is_ollama_installed`;
  - `SecretStorage` (4): `builder_deploy_store_credentials`,
    `builder_backend_connect`, `email_start_oauth`,
    `integration_start_oauth`;
  - `AmbientResource` (1): `cm_run_ab_validation` (C5);
  - `ApprovalRequired` (3): `nx_consent_respond`, `nx_agent_approve`,
    `self_rewrite_apply_patch`.
- **Refused without closing a command.**
  - Transcendent (L6) agents, on every route (item G); an enabled Warden
    review with no Warden denies.
  - Agent web fetch; Ollama at any address but `OLLAMA_URL` or the default;
    pulls from registries other than Ollama's default; SearXNG without
    `SEARXNG_URL` (item B).
  - The external tools `rest_api`, `webhook`, `file_storage`, `github`,
    `slack` and `jira` (items B and C); MCP servers that need credentials
    (item C).
  - Starting Ollama from Nexus: `ensure_ollama` connects only (item I).
  - A new or changed credential without a usable `NEXUS_CONFIG_KEY` (item
    A); a vault key file that fails its checks, or on a platform other than
    Linux and macOS (item E).
  - Storing a messaging token except through the settings save; API Client
    collections holding secrets; credential stores in backups (item H).
  - The 18 withdrawn standalone binaries and their recipes (item J).
  - Schedules firing more than once per minute, and inputs outside the
    resource bounds (item K).
  - The Monaco editor, Builder collaboration, the Settings live key test,
    scripted Builder previews and click-to-edit, remote images in notes, and
    new windows (item D).
  - Autonomous-loop intervals outside 60 to 3,599 seconds, and manifest
    schedules the scheduler rejects (item K).

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

**P0-FINAL-GATE-CLOSURE additions.**

- The OAuth line above is superseded: the sign-in flows are closed (item H).
  Correction: while they were open, the flow's `state` was also on the
  browser launcher's command line (the authorization URL was passed as an
  argument), and the flows used no PKCE.
- The vault key file's checks bind to the descriptor that is read, so the
  file checked is the file read. Intermediate directories are not examined,
  and a same-user process can replace the file between startups (item E).
- The configuration writer reads the stored file, decides, and renames a new
  file over it without a lock. A same-user process changing the file in
  between is not detected, and two desktop processes saving at once can lose
  one update (item A).
- The scheduled-tick check and the loop spawn are not atomic (item K).
- DNS: agent web fetch is refused (item B). The remaining destinations
  (provider constants, fixed hosts, operator configuration) are resolved when
  each request is made and not pinned. Nexus Link resolves no names (item F).

## No OS sandbox

Phase Zero builds no filesystem, process or network sandbox. Permitted agent
actions run in the desktop process: LLM, memory, notification, messaging,
HITL, web search and fetch, and knowledge graph. Helper programs run with the
user's full authority. The Builder's sealed spawn and resource limits (C4)
constrain a dev server's arguments, environment and resources; they do not
isolate its filesystem or network.

**P0-FINAL-GATE-CLOSURE update.** The closure mission adds no sandbox.

- Web fetch is no longer a permitted agent action (item B). The permitted set
  is LLM, memory, notification, agent message, HITL request, web search and
  knowledge graph.
- The in-process credential transport and the owned download registry are
  not isolation. Credentialed requests run inside the desktop process, and
  child processes still inherit its environment, including the provider keys
  `save_provider_api_key` sets.
- Withdrawing a standalone binary does not contain its library (item J).

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

**P0-FINAL-GATE-CLOSURE corrections to C5C claims.** Each is labelled at its
item; in brief:

- **Item A.** The key description omitted three cases that gave a constant
  or ambient key; the deploy store and Supabase connect commands were open,
  not closed by C5A.
- **Item B.** The rows of caller-chosen destinations did add authority beyond
  `api_client_request` (the persisted Ollama address, and responses read
  back). The web actuator's stale comment is corrected; the API actuator's is
  not.
- **Item C.** The table named the wrong provider set and command, missed the
  credential-bearing external tools, and called every `cm_*` command closed
  while `cm_run_ab_validation` was open.
- **Item D.** The open list missed the remote and loopback frames, the Monaco
  CDN script, the remote fonts and the Settings key test; the C5C guard saw
  `srcDoc` frames only; no application-command ACL existed.
- **Item E.** The provider base-URL overrides set no desktop destination.
- **Item H.** The list of secrets at rest missed messaging tokens, the Slack
  Socket Mode URL, API Client collections, the XOR store and backups.
- **Item I.** The helper list missed the hardware probes, `which sd` and the
  OAuth browser launcher.
- **Item J.** The intro missed the repository's install and packaging
  recipes; the protocols server's unauthenticated list missed its frontend
  fallback; the root chart deployed a prebuilt image rather than building
  one.
- **Item K.** The recorded `executes_python_code` timeout has no retained
  hosted evidence.
- **TOCTOU.** The OAuth state was also on the launcher's command line, and the
  flows had no PKCE.
- The authority inventory's §11.7 lists the corrections to its own earlier
  sections and the notes recorded for the documentation audit.

## Architect decision requests (P0-FINAL-GATE-CLOSURE)

Collected from the workstream reports and the coordinator's decision record.
Each is for the Architect; none is accepted by the implementer. The numbers
are those cited in the items above.

[Historical. P0-LINUX-FINAL-R1 recorded the Architect's decisions for Linux
Phase Zero; they are under "Architect decisions (P0-LINUX-FINAL-R1)" below,
with a mapping from these numbers. The list is kept as it was.]

1. **PATH as operator configuration (item I).** Treat the launch environment's
   `PATH` as operator configuration, so the fixed-argument helpers found
   through it (curl, notifications, the hardware and disk probes, the
   remaining `which` probes) stay, while unowned launches stay removed.
2. **D3 (item J).** Keep as developer and benchmark tools, pinned by the
   inventory guard: 7 benchmark binaries (`audit-retention-bench`,
   `audit-throughput-bench`, `conductor-bench`, `darwin-drift-bench`,
   `genesis-protocol-bench`, `memory-profile`,
   `multiagent-coordination-bench`), 4 developer binaries (`nexus-ui-repair`,
   `scout`, `sg5_probe`, `nexus-swarm-healthcheck`), the 5 bench targets of
   `benchmarks` and the 2 kernel examples. Evidence: the swarm healthcheck
   runs `sh -c "command -v codex"` and reads `~/.codex/auth.json`;
   `sg5_probe` reads the session's accessibility registry; no kept binary's
   source binds a listener.
3. **Residual advisories (DEP).** The dispositions proposed in "Dependency
   evidence", and the warning-level entries not updated.
4. **Audit governance (DEP).** How a closure candidate is audited
   (`audit.yml` never scans one), and the four divergent ignore lists.
5. **npm residue (DEP).** vite 5 and esbuild, vitest and `@vitest/mocker`
   (development-only; majors or large updates); `monaco-editor` and
   `dompurify` with item D.
6. **Scope (DEP).** Whether the `nexus-website` and `scripts/page-audit`
   lockfiles belong to Phase Zero.
7. **Item K.** `executes_python_code` as monitored debt; the self-hosted
   runner's GPU driver mismatch as an Owner matter.
8. **Voice.** The Python voice dependencies are unpinned (model downloads in
   tests are now kept offline).
9. **Item A.** `NEXUS_CONFIG_KEY` as the configuration key source; any
   key-quality minimum beyond "not blank and not the ambient derivation";
   whether the first explicit save of a legacy plaintext file may use the
   ambient key when no operator key is set (the current, audited
   interpretation); whether loads reachable from IPC keep creating the
   first-run default for a missing file.
10. **Items A and H.** Whether an approved secret store later reopens OAuth
    sign-in, deploy and Supabase storage, and new messaging tokens.
11. **Item E.** Whether to implement an access-control check that would accept
    key files on Windows (refused now), and whether to verify every vault
    scope (six are verified).
12. **Item C.** The in-process transport for credentialed provider calls,
    including the desktop build's TLS stack (platform TLS, the operating
    system's trust store and proxy settings).
13. **Item B.** A non-loopback `OLLAMA_URL` as acceptable operator
    configuration.
14. **Item G.** The `desktop-ui (unverified)` label as the audit semantics;
    the proposed statement that neither a loop's HITL allowance nor an
    agent's lifecycle state is authority in Phase Zero (the executor alone
    decides); stored L6 records and pending transcendent requests (left
    untouched); the inert deny forward; whether HITL-gated Phase Zero steps
    should be refused rather than released by an IPC approval.
15. **Contract C5.** `AmbientResource` as the closure reason; a dedicated
    variant is an option.
16. **Item K.** The `ArenaRun` compatibility type, whose refusal surfaces as a
    serialization error to the older API; the 1-token minimum budget (the
    Time Machine inputs now stop at 10 forks and 200,000 tokens).
17. **Item I (I5).** Download termination at a normal exit only; a crash or a
    kill is a non-claim.
18. **Item D.** The Linux finding that neither the ACL nor the navigation
    guard is sufficient alone (wry attributes a `postMessage` to the
    webview's URL at handling time; possibly an upstream wry or tauri issue,
    worth reporting); accepting the Windows and macOS behaviour once their
    harness runs are observed; and the feature reductions item D makes.
19. **Item J.** The withdrawal message wording and the uniform status 69; the
    coordinator decisions D1 (alternate agent binaries), D2 (the computer-use
    harness) and D4 to D7 (the GitLab release job, recipes and packaging).

## Architect decisions (P0-LINUX-FINAL-R1)

The Architect's decisions for **Linux Phase Zero**, recorded as dispositions.
They replace the corresponding decision requests above. Each applies to Linux
only; Windows and macOS are deferred with those platforms. None of them
declares FG1, the Final Gate or Phase Zero complete or approved, and each
holds only within the limits it states.

- **A. `PATH` as operator launch configuration (item I; request 1).**
  Helpers with a fixed name and fixed arguments (curl, notifications, the
  hardware and disk probes, the remaining `which` probes) may be found
  through the launch environment's `PATH`. The decision does not extend to a
  path the interface selects, arbitrary arguments, secrets on a command line
  or unowned launches.
- **B. `NEXUS_CONFIG_KEY` as the Linux operator key source (item A;
  request 9).** The key must not be blank and must not derive the ambient
  key. The configuration key is derived from it with a SHA-256-labelled
  derivation. There is no KDF stretching and no key-strength claim; a
  high-entropy operator secret is recommended.
- **C. Legacy-read compatibility (item A; request 9).** Accepted: legacy
  files open through the explicit two-key read path.
- **D. Linux vault key-file semantics (item E; request 11).** Accepted for
  Linux. The Windows behaviour is deferred.
- **E. In-process credentialed transport (item C; request 12).** reqwest
  with native TLS and the system proxy is accepted, given: no credential on
  a command line, no redirect followed by a credentialed request, the
  endpoint's authority preserved, and bounds on time and size. The final
  network-client inventory is in item C.
- **F. Non-loopback `OLLAMA_URL` (item B; request 13).** An address set at
  launch is operator configuration. The plaintext-HTTP risk is stated in
  item B.
- **G. Stored L6 records and the approval label (item G; request 14).**
  Stored L6 records stay untouched but inert. `desktop-ui (unverified)` is an
  audit channel label, not a verified identity.
- **H. `AmbientResource` (contract C5; request 15).** Acceptable as the
  closure reason for C5.
- **I. `ArenaRun` and the one-token minimum (item K; request 16).** Accepted
  as bounded compatibility debt.
- **J. Owned-download cleanup (item I; request 17).** Covers a normal
  managed exit only; a crash or a kill stays a non-claim.
- **K. Item D (request 18).** Accepted on Linux only as the combined
  boundary: the app ACL, the exact-origin navigation guard, the frame
  restrictions, and the CSP and HTML controls. The navigation guard is
  load-bearing; the ACL alone is not sufficient.
- **L. J2–J5 withdrawal (item J; request 19).** Accepted.
- **M. Developer and benchmark binaries (item J; request 2).** They may
  remain, provided they are not shipped, stay pinned by the inventory guard
  and are not documented as runtime entry points.

**Mapping from the earlier requests.**

| Request | Disposition |
|---|---|
| 1 | Decision A |
| 2 | Decision M |
| 3, 4, 5, 6 | Addressed by R1 repair B ("Linux security gate" under "Dependency evidence"); RUSTSEC-2026-0316 accepted narrowly (Architect) |
| 7 | Not covered by A–M; dispositioned by P0-LINUX-FINAL-R2 (below) |
| 8 | Addressed by R1: the hash-pinned Linux voice lock (item K) |
| 9 | Decisions B and C; the rest dispositioned by P0-LINUX-FINAL-R2 (below) |
| 10 | Not covered by A–M; dispositioned by P0-LINUX-FINAL-R2 (below) |
| 11 | Decision D (Linux); vault-scope verification dispositioned by P0-LINUX-FINAL-R2 (below) |
| 12 | Decision E |
| 13 | Decision F |
| 14 | Decision G; the rest dispositioned by P0-LINUX-FINAL-R2 (below) |
| 15 | Decision H |
| 16 | Decision I |
| 17 | Decision J |
| 18 | Decision K (Linux); Windows and macOS deferred |
| 19 | Decision L; the withdrawal wording and the coordinator decisions D1, D2 and D4 to D7 dispositioned by P0-LINUX-FINAL-R2 (below) |

**Open after P0-LINUX-FINAL-R1.** [Historical; superseded by
P0-LINUX-FINAL-R2, below.]
- None for dependencies: RUSTSEC-2026-0316 (wasmtime 43.0.2) was accepted
  narrowly by the Architect (see "Linux security gate").
- The parts of requests 7, 9, 10, 11, 14 and 19 listed as not covered above.
- Windows and macOS, all items: deferred.

## Architect dispositions (P0-LINUX-FINAL-R2)

The Architect's dispositions of the parts of requests 7, 9, 10, 11, 14 and 19
that decisions A–M did not cover. They replace the earlier statement that
those parts were simply open. Each applies to **Linux Phase Zero** only and
holds only within the limits and non-claims it states. None of them declares
the Final Gate or Phase Zero complete or approved.

- **Request 7 (item K).** Not a Linux Phase Zero blocker.
  `executes_python_code` is monitored debt and is unreachable from the Phase
  Zero desktop executor. The self-hosted runner's GPU driver and library
  condition is Owner infrastructure. No general reliability claim is made for
  that actuator or for that GPU host.
- **Request 9 (item A).** Accepted only as bounded legacy compatibility. A
  legacy plaintext or configuration file may remain, or may be explicitly
  saved under the legacy ambient derivation, only when no credential is new
  or changed. The ambient derivation is **not** secret protection and must
  never be described as such. Every new or changed credential still requires
  the operator key `NEXUS_CONFIG_KEY`, which must not be blank and must not
  be the ambient derivation. The missing-file bootstrap is acceptable only
  because it creates the default configuration, which contains no
  credential; interface updates gain no bootstrap authority. No KDF-strength
  or password-hardening claim is made.
- **Request 10 (items A and H).** Not an active Phase Zero blocker while
  OAuth sign-in, deploy and Supabase storage, and new messaging credential
  persistence stay closed. Reopening any of those flows requires a separately
  approved secret-store design and mission. They are not permanently solved.
- **Request 11 (item E).** Accepted for Linux after the vault-scope inventory
  guard: the six current vault scopes are verified before the facade is
  installed. Windows remains deferred. No protection is claimed for a future
  new scope unless it is added to, and verified by, that inventory.
  - The guard is `p0_fg_e_vault_key_sources_are_validated_on_what_is_read`
    (`fg_secrets`). It pins `VAULT_SCOPES` in `kernel/src/startup/mod.rs` to
    exactly `llm`, `social`, `messaging.whatsapp`, `messaging.matrix`, `http`
    and `auth.oidc`; checks that `verify_vault_key` iterates `VAULT_SCOPES`
    and runs before the one production `SecretsFacade::new`; and checks that
    every scope literal a production `get_secret`, `set_secret` or
    `delete_secret` call passes is one of the six. Adding a seventh scope, or
    a facade call with a scope outside the six, fails the guard until the
    inventory and this disposition are updated.
  - Production facade callers at the R2 head, all inside the six: `llm`
    (the four swarm providers, `agents.rs`, `chat_llm.rs`, which also writes,
    and the legacy migration, which writes), `social` (`twitter.rs` and the
    legacy migration), `messaging.whatsapp`, `messaging.matrix`, `http`
    (`http_connector.rs`) and `auth.oidc` (`auth/src/config.rs`).
- **Request 14 (item G).** IPC HITL approval is **not** security authority
  and is not verified human identity. It is acceptable in Linux Phase Zero
  only as a UX and consent signal whose released operation is independently
  bounded by the backend-owned Phase Zero executor policy. No actuator may
  use `hitl_approved` as authority (pinned by
  `p0_fg_g_no_actuator_reads_the_hitl_approval_flag`). Any future operation
  that needs independent human authorization requires a separately approved
  mechanism. Stored L6 records stay inert, as decision G records.
- **Request 19 (item J).** The fixed withdrawal wording, the uniform status
  69 and the coordinator closure of D1, D2 and D4 to D7 are accepted for the
  current Phase Zero closure candidate, subject to the existing inventory and
  withdrawal guards. J2–J5 are not declared complete until the closure
  candidate is integrated and validated. Historical descriptions stay
  historical.

**Correction: P0-FG1 (J1).** P0-FG1 is integrated at the authoritative
`71c47acb`, and its designated post-integration validation (hosted run #108)
passed. J1 does not wait for integration or Architect review, and
P0-FG1-R1's duplicate-name repair is integrated. The current-status text in
the header, the Summary, item J and item K states this directly
(P0-LINUX-FINAL-R2A). Only explicitly historical text (the C5C table, "Before
P0-FG1", dated runs) keeps the earlier status.

**Final-Gate guard counts.** The figure "244 Final Gate guards" in earlier
evidence was a name-pattern count over the Linux workspace test step: every
test named with `p0_fg`, plus every test in the six `phase0_surface::fg_*`
modules. It is not one module or one test binary. At `b8402283` (hosted run
#110), the workspace test step (`cargo test --workspace --locked`) executed:

| Selection | Count | Test binaries |
|---|---|---|
| `p0_fg_*` | 231 | desktop backend 116 (80 in the six `fg_*` modules), `nexus_kernel` 58, `nexus_connectors_llm` 25, `web_builder_agent` 13, `nexus_code` 8, `nexus_external_tools` 5, `nexus_swarm` 4, `nexus_capability_measurement` 1, `nexus_protocols` 1 |
| `p0_fg1_*` (J1 withdrawal) | 11 | `phase0_withdrawal` 10, `binary_target_identity` 1 |
| `p0_r1_*` (network-client bounds) | 8 | 2 of them in `fg_egress`; the others in `nexus_connectors_llm` 3, `nexus_swarm` 2, `web_builder_agent` 1 |

- The earlier figure is 231 + 11 + the 2 `fg_egress` `p0_r1_*` tests = 244.
- The Architect's framing is 231 `p0_fg_*` workspace test executions plus
  the separate 13-test packaged Builder gate (a later step of the same Linux
  job: 11 `p0_002c4c3_packaged_*` and 2 `p0_002c4d2_assembled_*` tests) =
  244 selected checks.
- The two totals are equal only numerically; they are different selections.
  All the tests above passed in run #110.
- P0-LINUX-FINAL-R2 adds no test function (its checks extend two existing
  guards), so these counts are unchanged at the R2 head. The R2 evidence
  recomputes them from the fast-local run on the R2 commit.

**Open after P0-LINUX-FINAL-R2.**
- No part of requests 7, 9, 10, 11, 14 or 19 is left undispositioned for
  Linux Phase Zero. Each disposition keeps its non-claims, and the flows
  request 10 names stay closed.
- J2–J5 wait for the closure candidate's integration and validation.
- Windows and macOS, all items: deferred.
- This dossier declares neither the Final Gate nor Phase Zero complete.
