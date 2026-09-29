//! P0-FINAL-GATE-CLOSURE guards for items B and F (destinations and Nexus
//! Link), C (credentials in process arguments) and I (helper programs).

use crate::phase0_surface::{closed, Closure};

const LIB_RS: &str = include_str!("../../lib.rs");
const CRATE_BRIDGES_RS: &str = include_str!("../../commands/crate_bridges.rs");

/// Final Gate items B, C, F and I: IPC commands this workstream closes.
/// `(module defining the handler, command, closure)`; `""` is the `runtime`
/// module in `lib.rs`.
/// - B and F: the destination they reached (a URL, an agent or server
///   address, a peer) was the caller's choice, and no backend-owned policy
///   makes such a destination an egress grant.
/// - C: the request would have placed a credential on a process command
///   line.
/// - I: the answer came from running a helper program found on `PATH`.
const CLOSED_EGRESS_COMMANDS: &[(&str, &str, Closure)] = &[
    ("", "api_client_request", Closure::NetworkDestination),
    ("", "a2a_discover_agent", Closure::NetworkDestination),
    ("", "a2a_send_task", Closure::NetworkDestination),
    ("", "a2a_get_task_status", Closure::NetworkDestination),
    ("", "a2a_cancel_task", Closure::NetworkDestination),
    (
        "commands::crate_bridges",
        "a2a_crate_send_task",
        Closure::NetworkDestination,
    ),
    (
        "commands::crate_bridges",
        "a2a_crate_get_task",
        Closure::NetworkDestination,
    ),
    (
        "commands::crate_bridges",
        "a2a_crate_discover_agent",
        Closure::NetworkDestination,
    ),
    ("", "mcp_host_connect", Closure::NetworkDestination),
    ("", "mcp_host_call_tool", Closure::NetworkDestination),
    (
        "",
        "builder_theme_extract_from_url",
        Closure::NetworkDestination,
    ),
    ("", "nexus_link_send_model", Closure::PeerTransfer),
    (
        "commands::crate_bridges",
        "perception_init",
        Closure::CredentialTransport,
    ),
    ("", "is_ollama_installed", Closure::HelperLaunch),
];

/// Every closure reason this workstream adds.
const EGRESS_CLOSURES: &[Closure] = &[
    Closure::NetworkDestination,
    Closure::PeerTransfer,
    Closure::CredentialTransport,
    Closure::HelperLaunch,
];

fn module_source(module: &str) -> &'static str {
    match module {
        "" => LIB_RS,
        "commands::crate_bridges" => CRATE_BRIDGES_RS,
        other => panic!("module {other} is not mapped in the egress guard"),
    }
}

/// Entries of the desktop `generate_handler![..]` list.
fn registered_handlers() -> Vec<String> {
    let start = LIB_RS
        .find("generate_handler![")
        .expect("desktop command registration")
        + "generate_handler![".len();
    let end = start + LIB_RS[start..].find(']').expect("end of registration");
    LIB_RS[start..end]
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect()
}

fn is_registered(handlers: &[String], command: &str) -> usize {
    handlers
        .iter()
        .filter(|entry| entry.rsplit("::").next() == Some(command))
        .count()
}

fn without_whitespace(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// A source with LF line endings. On a CRLF checkout (Windows CI checks out
/// with `core.autocrlf=true`), every `include_str!` text has `\r\n`, so the
/// guards that match across lines read their sources through this.
fn lf(source: &str) -> String {
    source.replace("\r\n", "\n")
}

/// The same source as a CRLF checkout gives it. The guards that match across
/// lines run on both forms.
fn crlf(source: &str) -> String {
    lf(source).replace('\n', "\r\n")
}

/// Index just past the brace closing the block that opens at `open`. String
/// literals are skipped; the handler bodies read here contain no others.
fn block_end(src: &str, open: usize) -> usize {
    let bytes = src.as_bytes();
    let (mut depth, mut i, mut in_string) = (0usize, open, false);
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if in_string => i += 1,
            b'"' => in_string = !in_string,
            b'{' if !in_string => depth += 1,
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    panic!("unterminated block");
}

/// Parameter list and body (both whitespace-free) of the only `fn <name>(`
/// in `src`.
fn handler_shape(src: &str, name: &str) -> (String, String) {
    let needle = format!("fn {name}(");
    let mut found = src.match_indices(&needle).map(|(at, _)| at);
    let at = found
        .next()
        .unwrap_or_else(|| panic!("{name}: handler not found"));
    assert!(found.next().is_none(), "{name}: defined more than once");
    let params_start = at + needle.len() - 1;
    let params_end = params_start + src[params_start..].find(')').unwrap() + 1;
    let open = params_end + src[params_end..].find('{').unwrap();
    let body = &src[open + 1..block_end(src, open) - 1];
    (
        without_whitespace(&src[params_start..params_end]),
        without_whitespace(body).replace(",)", ")"),
    )
}

/// Every reason this workstream adds is bounded, names Phase Zero, carries no
/// path separator, and echoes nothing of the caller.
#[test]
fn p0_fg_egress_closure_reasons_are_bounded_and_echo_no_input() {
    for closure in EGRESS_CLOSURES {
        let reason = closure.reason();
        assert!(reason.contains("Phase Zero"), "{reason}");
        assert!(reason.len() <= 160, "{reason}");
        assert!(!reason.contains('/') && !reason.contains('\\'), "{reason}");
        assert_eq!(closed("surface", *closure), format!("surface: {reason}"));
    }
}

/// Final Gate items B and F: each command that reached a caller-chosen
/// destination stays registered once, takes no input, and only denies.
#[test]
fn p0_fg_caller_chosen_destinations_are_closed_commands() {
    let handlers = registered_handlers();
    for (module, command, closure) in CLOSED_EGRESS_COMMANDS {
        assert_eq!(
            is_registered(&handlers, command),
            1,
            "{command} must stay registered once"
        );
        let (params, body) = handler_shape(module_source(module), command);
        assert_eq!(params, "()", "{command} must accept no caller input");
        assert_eq!(
            body,
            format!(
                "Err(crate::phase0_surface::closed(\"{command}\",crate::phase0_surface::Closure::{closure:?}))"
            ),
            "{command} must only deny"
        );
    }
}

/// The closed handlers return exactly their bounded reason when invoked.
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[test]
fn p0_fg_closed_destination_handlers_return_only_their_reason() {
    use crate::commands::crate_bridges as bridges;
    use crate::runtime;
    /// A closed command and a no-input invocation of its handler.
    type ClosedCall = (&'static str, fn() -> Result<(), String>);
    let calls: [ClosedCall; 14] = [
        ("api_client_request", || {
            runtime::api_client_request().map(|_| ())
        }),
        ("a2a_discover_agent", || {
            runtime::a2a_discover_agent().map(|_| ())
        }),
        ("a2a_send_task", || runtime::a2a_send_task().map(|_| ())),
        ("a2a_get_task_status", || {
            runtime::a2a_get_task_status().map(|_| ())
        }),
        ("a2a_cancel_task", runtime::a2a_cancel_task),
        ("a2a_crate_send_task", || {
            bridges::a2a_crate_send_task().map(|_| ())
        }),
        ("a2a_crate_get_task", || {
            bridges::a2a_crate_get_task().map(|_| ())
        }),
        ("a2a_crate_discover_agent", || {
            bridges::a2a_crate_discover_agent().map(|_| ())
        }),
        ("mcp_host_connect", || {
            runtime::mcp_host_connect().map(|_| ())
        }),
        ("mcp_host_call_tool", || {
            runtime::mcp_host_call_tool().map(|_| ())
        }),
        ("builder_theme_extract_from_url", || {
            runtime::builder_theme_extract_from_url().map(|_| ())
        }),
        ("nexus_link_send_model", || {
            runtime::nexus_link_send_model().map(|_| ())
        }),
        ("perception_init", || bridges::perception_init().map(|_| ())),
        ("is_ollama_installed", || {
            runtime::is_ollama_installed().map(|_| ())
        }),
    ];
    assert_eq!(calls.len(), CLOSED_EGRESS_COMMANDS.len());
    for (command, call) in calls {
        let (_, _, closure) = CLOSED_EGRESS_COMMANDS
            .iter()
            .find(|(_, name, _)| *name == command)
            .unwrap_or_else(|| panic!("{command} is not classified"));
        assert_eq!(call(), Err(closed(command, *closure)), "{command}");
    }
}

/// Final Gate item B: the implementations behind the closed commands are
/// gone from the desktop. No desktop module reaches the kernel A2A client's
/// network operations, the MCP host's connect or tool call, the Nexus Link
/// sender or the theme fetch, and the interface's HTTP client runs no
/// process. The companions that send nothing stay open.
#[test]
fn p0_fg_the_desktop_calls_no_caller_chosen_destination_client() {
    let sources = [
        ("lib.rs", LIB_RS),
        ("commands/crate_bridges.rs", CRATE_BRIDGES_RS),
        (
            "commands/governance.rs",
            include_str!("../../commands/governance.rs"),
        ),
        (
            "commands/tools_infra.rs",
            include_str!("../../commands/tools_infra.rs"),
        ),
        ("commands/apps.rs", include_str!("../../commands/apps.rs")),
        (
            "commands/model_hub.rs",
            include_str!("../../commands/model_hub.rs"),
        ),
    ];
    let clients = [
        ".discover_agent(",
        ".send_task(",
        ".get_task_status(",
        ".cancel_task(",
        ".connect_server(",
        ".call_tool(",
        ".send_model(",
        "extract_theme_from_url",
        "a2a_crate_cmds::a2a_crate_send_task",
        "a2a_crate_cmds::a2a_crate_get_task",
        "a2a_crate_cmds::a2a_crate_discover_agent",
        "Command::new(\"curl\")",
    ];
    for (file, text) in sources {
        for client in clients {
            assert!(!text.contains(client), "{file}: {client}");
        }
    }
    let handlers = registered_handlers();
    for open in [
        "a2a_known_agents",
        "mcp_host_add_server",
        "mcp_host_list_servers",
        "mcp_host_remove_server",
        "mcp_host_disconnect",
        "mcp_host_list_tools",
        "nexus_link_status",
        "nexus_link_add_peer",
        "nexus_link_list_peers",
        "api_client_list_collections",
    ] {
        assert_eq!(
            is_registered(&handlers, open),
            1,
            "{open} must stay registered"
        );
    }
}

/// A loopback listener that must never be contacted, and its http base URL.
fn quiet_listener() -> (std::net::TcpListener, String) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    (listener, base)
}

fn assert_never_contacted(listener: &std::net::TcpListener) {
    assert!(matches!(
        listener.accept(),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));
}

/// Final Gate item B: the Ollama address is backend configuration, the
/// operator's `OLLAMA_URL` or else the fixed local address. A set but
/// unusable value leaves Ollama unavailable and never falls back to the
/// default, and the refusal does not echo the value.
#[test]
fn p0_fg_the_ollama_address_is_backend_configuration() {
    use crate::commands::chat_llm::{
        authorized_ollama_base_url_from, DEFAULT_OLLAMA_URL, OLLAMA_ADDRESS_UNAVAILABLE,
    };
    use std::ffi::OsString;
    assert_eq!(
        authorized_ollama_base_url_from(None),
        Ok(DEFAULT_OLLAMA_URL.to_string())
    );
    for (operator, authorized) in [
        ("http://127.0.0.1:12345", "http://127.0.0.1:12345"),
        (
            "HTTP://Ollama.Example:11434/",
            "http://ollama.example:11434",
        ),
        ("https://ollama.example:443/", "https://ollama.example"),
        ("http://[::1]:11434", "http://[::1]:11434"),
        ("http://ollama.example/base/", "http://ollama.example/base"),
    ] {
        assert_eq!(
            authorized_ollama_base_url_from(Some(OsString::from(operator))),
            Ok(authorized.to_string()),
            "{operator}"
        );
    }
    let mut unusable: Vec<OsString> = [
        "",
        "localhost:11434",
        "ftp://ollama.example",
        "file:///ollama.example",
        "http://user:secret@ollama.example",
        "http://ollama.example/?key=1",
        "http://ollama.example/#top",
        " http://ollama.example",
        "http://ollama.example\n",
    ]
    .map(OsString::from)
    .to_vec();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        unusable.push(OsString::from_vec(b"http://ollama.example\xff".to_vec()));
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        unusable.push(OsString::from_wide(&[0x68, 0xD800]));
    }
    for operator in unusable {
        assert_eq!(
            authorized_ollama_base_url_from(Some(operator.clone())),
            Err(OLLAMA_ADDRESS_UNAVAILABLE.to_string()),
            "{operator:?}"
        );
    }
    assert!(!OLLAMA_ADDRESS_UNAVAILABLE.contains("ollama.example"));
}

/// Final Gate item B: an address the interface passes is accepted only when
/// it names the authorized address (any spelling of it). Every other
/// address, including another spelling of the host (`127.0.0.1` for
/// `localhost`), another port or path, or a URL that is not a base, is
/// refused before anything connects, by every Ollama command.
#[test]
fn p0_fg_caller_ollama_addresses_are_refused_before_anything_connects() {
    use crate::commands::chat_llm::{authorized_ollama_base_url, ollama_base_url_for};
    let authorized = authorized_ollama_base_url();
    let (listener, foreign) = quiet_listener();
    assert_ne!(authorized.as_deref().ok(), Some(foreign.as_str()));
    let expected = |surface: &'static str| match &authorized {
        Ok(_) => closed(surface, Closure::NetworkDestination),
        Err(reason) => format!("{surface}: {reason}"),
    };
    let state = crate::AppState::new_in_memory();
    let user = vec![serde_json::json!({"role": "user", "content": "hello"})];
    let results: [(&'static str, Result<(), String>); 7] = [
        (
            "check_ollama",
            crate::check_ollama(Some(foreign.clone())).map(|_| ()),
        ),
        (
            "pull_ollama_model",
            crate::pull_ollama_model("llama3".into(), Some(foreign.clone())).map(|_| ()),
        ),
        (
            "pull_model",
            crate::pull_ollama_model_throttled("llama3".into(), Some(foreign.clone()), |_| {})
                .map(|_| ()),
        ),
        (
            "ensure_ollama",
            crate::ensure_ollama(Some(foreign.clone())).map(|_| ()),
        ),
        (
            "delete_model",
            crate::delete_ollama_model("llama3".into(), Some(foreign.clone())),
        ),
        (
            "chat_with_ollama",
            crate::chat_with_ollama_streaming(
                &state,
                user,
                "m".into(),
                Some(foreign.clone()),
                |_| {},
            )
            .map(|_| ()),
        ),
        (
            "run_setup_wizard",
            crate::run_setup_wizard(Some(foreign.clone())).map(|_| ()),
        ),
    ];
    for (surface, result) in results {
        assert_eq!(result, Err(expected(surface)), "{surface}");
    }
    assert_never_contacted(&listener);

    if let Ok(authorized) = &authorized {
        // The authorized address, with or without a trailing `/`, is that
        // address.
        for spelling in [authorized.clone(), format!("{authorized}/")] {
            assert_eq!(
                ollama_base_url_for("check_ollama", Some(spelling.clone())).as_ref(),
                Ok(authorized),
                "{spelling}"
            );
        }
        assert_eq!(
            ollama_base_url_for("check_ollama", None).as_ref(),
            Ok(authorized)
        );
    }
    if authorized.as_deref() == Ok(crate::commands::chat_llm::DEFAULT_OLLAMA_URL) {
        assert_eq!(
            ollama_base_url_for("check_ollama", Some("HTTP://LocalHost:11434/".into())).as_deref(),
            Ok(crate::commands::chat_llm::DEFAULT_OLLAMA_URL)
        );
        for alias in [
            "http://127.0.0.1:11434",
            "http://[::1]:11434",
            "https://localhost:11434",
            "http://localhost:11435",
            "http://localhost",
            "http://localhost:11434/api",
            "http://localhost:11434/?x=1",
            "http://localhost.:11434",
        ] {
            assert_eq!(
                ollama_base_url_for("check_ollama", Some(alias.into())),
                Err(closed("check_ollama", Closure::NetworkDestination)),
                "{alias}"
            );
        }
    }
    // A pull naming a registry host is refused before any request, on the
    // authorized address too.
    let error = crate::pull_ollama_model("hf.co/user/model".into(), None).unwrap_err();
    assert!(
        error.contains("default registry") || authorized.is_err(),
        "{error}"
    );
}

/// Final Gate item B: the persisted `llm.ollama_url` and `ollama.base_url`
/// choose no destination. Provider selection uses the authorized address.
#[test]
fn p0_fg_the_persisted_ollama_address_chooses_no_destination() {
    let mut config = nexus_kernel::config::NexusConfig::default();
    config.llm.ollama_url = "http://persisted.invalid:1".into();
    config.ollama.base_url = "http://persisted.invalid:2".into();
    let selection = crate::build_provider_config(&config);
    let authorized = crate::commands::chat_llm::authorized_ollama_base_url().unwrap_or_default();
    assert_eq!(selection.ollama_url, Some(authorized));
    assert!(!format!("{:?}", selection.ollama_url).contains("persisted"));
}

/// Final Gate item B (Architect decision D4): an agent's http(s) web fetch
/// is refused, although the agent holds `web.read` and its manifest's
/// `allowed_endpoints` admits the URL. That allowlist came from the
/// interface at creation (or from a stored record), and neither is an egress
/// grant. Nothing is contacted. A non-HTTP fetch keeps its C5A refusal, and
/// web search, which reaches fixed or operator-configured hosts, still runs.
#[test]
fn p0_fg_agent_web_fetch_is_not_egress_authority() {
    use nexus_kernel::cognitive::loop_runtime::ActionExecutor;
    use nexus_kernel::cognitive::PlannedAction;
    let (listener, base) = quiet_listener();
    let state = crate::AppState::new_in_memory();
    let manifest = nexus_kernel::manifest::parse_manifest(&format!(
        "name = \"fg-fetcher\"\nversion = \"1.0.0\"\ncapabilities = [\"web.read\", \"web.search\", \"llm.query\"]\nfuel_budget = 1000\nautonomy_level = 3\nallowed_endpoints = [\"{base}\"]\n"
    ))
    .unwrap();
    let agent = state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(manifest)
        .unwrap()
        .to_string();
    let memory = std::sync::Arc::new(nexus_kernel::cognitive::AgentMemoryManager::new(Box::new(
        crate::DbMemoryStore {
            db: state.db.clone(),
        },
    )));
    let executor = crate::phase0_agent_executor(&state, memory);
    let mut audit = state.audit.clone();
    for url in [
        format!("{base}/page"),
        format!("{base}/"),
        format!("  {}", base.to_uppercase()),
        base.replacen("http://", "https://", 1),
    ] {
        let action = PlannedAction::WebFetch { url: url.clone() };
        assert_eq!(
            executor.execute(&agent, &action, &mut audit, true),
            Err(closed("web_fetch", Closure::NetworkDestination)),
            "{url}"
        );
    }
    assert_never_contacted(&listener);
    for url in [
        "file:///etc/hosts",
        "ftp://127.0.0.1/",
        "javascript:alert(1)",
    ] {
        let action = PlannedAction::WebFetch { url: url.into() };
        assert_eq!(
            crate::phase0_agent_action_closure(&action),
            Some(Closure::AgentExecution),
            "{url}"
        );
    }
    let search = PlannedAction::WebSearch {
        query: "phase zero".into(),
    };
    assert_eq!(crate::phase0_agent_action_closure(&search), None);
}

/// Final Gate item C: no interface key reaches the vision provider, which
/// would have placed it on curl's command line. With `perception_init`
/// closed, no provider exists, and each perception task is refused before
/// anything is sent.
#[test]
fn p0_fg_perception_takes_no_key_and_sends_nothing() {
    let state = nexus_perception::tauri_commands::PerceptionState::default();
    let error =
        nexus_perception::tauri_commands::perceive_describe(&state, "aGVsbG8=", "png").unwrap_err();
    assert!(error.contains("not initialized"), "{error}");
    assert!(state.engine.read().unwrap().is_none());
    let (_, body) = handler_shape(CRATE_BRIDGES_RS, "perception_init");
    assert!(!body.contains("init_provider"), "{body}");
}

/// Final Gate items B, C and G, through the desktop's tool state: a
/// registered agent at the highest level cannot send a request to an address
/// it chooses (`rest_api`, `webhook`), and nothing is contacted.
#[test]
fn p0_fg_desktop_tool_calls_reach_no_caller_chosen_destination() {
    let (listener, base) = quiet_listener();
    let port = listener.local_addr().unwrap().port();
    let state = crate::AppState::new_in_memory();
    let manifest = nexus_kernel::manifest::parse_manifest(
        "name = \"fg-tools\"\nversion = \"1.0.0\"\ncapabilities = [\"llm.query\"]\nfuel_budget = 100\nautonomy_level = 5\n",
    )
    .unwrap();
    let agent = state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(manifest)
        .unwrap()
        .to_string();
    let level = crate::commands::crate_bridges::tool_call_autonomy(&state, &agent, 5).unwrap();
    for (tool, params) in [
        (
            "rest_api",
            serde_json::json!({"url": format!("{base}/x"), "method": "GET"}),
        ),
        (
            "rest_api",
            serde_json::json!({"url": format!("http://127.1:{port}/x"), "method": "POST"}),
        ),
        (
            "webhook",
            serde_json::json!({"url": format!("{base}/hook")}),
        ),
    ] {
        let error = nexus_external_tools::tauri_commands::tools_execute(
            &state.external_tools,
            &agent,
            level,
            tool,
            &params.to_string(),
        )
        .unwrap_err();
        assert!(
            error.contains("destination the caller chose"),
            "{tool}: {error}"
        );
    }
    assert_never_contacted(&listener);
    for tool in ["github", "slack", "jira"] {
        assert!(
            nexus_external_tools::execution::phase0_refusal(tool)
                .is_some_and(|reason| reason.contains("operator credential")),
            "{tool}"
        );
    }
}

/// Final Gate item I: Nexus starts no Ollama service and runs no helper
/// program to find one. `ensure_ollama` only probes the authorized address:
/// a service that answers is used, and one that does not is refused with the
/// `HelperLaunch` reason, with nothing started, waited for or cleaned up. The
/// desktop's Nexus Code diagnostic and configuration (the nx bridge start and
/// the nx IPC commands) look `ollama`, `git` and `rg` up on `PATH` in process
/// (their behaviour is tested in `nexus-code/src/setup.rs`); the standalone
/// `nx` terminal keeps its `which` lookups.
#[test]
fn p0_fg_nexus_starts_no_ollama_and_runs_no_helper_to_find_it() {
    use crate::commands::chat_llm::ollama_service_answers;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let running = format!("http://{}", listener.local_addr().unwrap());
    assert_eq!(ollama_service_answers(&running), Ok(true));
    // The probe connected and sent nothing.
    let (mut probe, _) = listener.accept().unwrap();
    probe
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let mut byte = [0u8; 1];
    assert_eq!(std::io::Read::read(&mut probe, &mut byte).unwrap(), 0);

    let stopped = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    assert_eq!(
        ollama_service_answers(&format!("http://{stopped}")),
        Err(closed("ensure_ollama", Closure::HelperLaunch))
    );

    let chat_llm = include_str!("../../commands/chat_llm.rs");
    assert_no_ollama_helper(chat_llm, LIB_RS);
    assert_no_ollama_helper(&crlf(chat_llm), &crlf(LIB_RS));

    let nexus_code = [
        include_str!("../../../../../nexus-code/src/setup.rs"),
        include_str!("../../../../../nexus-code/src/config.rs"),
        include_str!("../../nx_bridge/mod.rs"),
        include_str!("../../nx_bridge/commands.rs"),
    ];
    assert_desktop_nexus_code_runs_no_program(nexus_code);
    assert_desktop_nexus_code_runs_no_program(nexus_code.map(crlf).each_ref().map(String::as_str));
}

/// The desktop's Nexus Code side of the guard above, for either line ending:
/// `[setup.rs, config.rs, nx_bridge/mod.rs, nx_bridge/commands.rs]`.
fn assert_desktop_nexus_code_runs_no_program(sources: [&str; 4]) {
    let [setup, config, bridge, commands] = sources.map(|source| production_text(&lf(source)));
    let (_, desktop) = handler_shape(&setup, "diagnose_for_desktop");
    assert_eq!(desktop, "diagnose_with(false,false)");
    let (_, diagnose) = handler_shape(&setup, "diagnose_with");
    assert!(
        diagnose.starts_with("letinstalled=|name:&str|program_installed(name,project);"),
        "{diagnose}"
    );
    assert!(!diagnose.contains("check_command_exists("), "{diagnose}");
    let (_, installed) = handler_shape(&setup, "program_installed");
    assert_eq!(
        installed,
        "ifrun_which{check_command_exists(name)}else{program_on_path(name)}"
    );
    for lookup in [
        "program_on_path",
        "program_on_path_in",
        "program_file_names",
        "is_program",
    ] {
        let (_, body) = handler_shape(&setup, lookup);
        for spawn in ["Command", "spawn(", ".output(", ".status("] {
            assert!(!body.contains(spawn), "{lookup}: {spawn}");
        }
    }
    let (_, detect) = handler_shape(&config, "auto_detect_provider");
    assert!(
        detect.contains("program_installed(\"ollama\",cli_agents)"),
        "{detect}"
    );
    assert!(!detect.contains("check_command_exists("), "{detect}");
    for (file, text) in [
        ("nx_bridge/mod.rs", &bridge),
        ("nx_bridge/commands.rs", &commands),
    ] {
        assert!(text.contains("setup::diagnose_for_desktop()"), "{file}");
        for other in [
            "check_command_exists(",
            "program_installed(",
            "Command::new(\"which\")",
        ] {
            assert!(!text.contains(other), "{file}: {other}");
        }
    }
}

/// The source side of the guard above, for either line ending.
fn assert_no_ollama_helper(chat_llm: &str, lib_rs: &str) {
    let chat_llm = lf(chat_llm);
    for helper in [
        "Command::new(\"ollama\")",
        "Command::new(\"which\")",
        "\"serve\"",
    ] {
        assert!(!chat_llm.contains(helper), "chat_llm.rs: {helper}");
    }
    assert!(!lf(lib_rs).contains("Command::new(\"ollama\")"));
    let ensure = chat_llm
        .split("pub(crate) fn ensure_ollama(")
        .nth(1)
        .and_then(|rest| rest.split("\n}\n").next())
        .expect("ensure_ollama");
    for forbidden in ["spawn(", "Command::", "sleep("] {
        assert!(!ensure.contains(forbidden), "ensure_ollama: {forbidden}");
    }
}

/// Final Gate item I: at a normal exit, model downloads do not outlive the
/// application. The normal-exit hook ends the downloads in flight (bounded,
/// through their owned handles; see the `model_hub` tests) and reports a
/// failure by counts only. A download's transfer is started only by the
/// in-flight registry, so none escapes it.
///
/// Non-claim: if the application crashes or is killed (SIGKILL, a forced end
/// of task), the hook does not run and a transfer in flight can outlive it.
#[test]
fn p0_fg_the_application_exit_ends_in_flight_model_downloads() {
    let model_hub = include_str!("../../../../../connectors/llm/src/model_hub.rs");
    assert_downloads_end_at_exit(LIB_RS, model_hub);
    assert_downloads_end_at_exit(&crlf(LIB_RS), &crlf(model_hub));
}

/// The guard above, for either line ending.
fn assert_downloads_end_at_exit(lib_rs: &str, model_hub: &str) {
    let lib_rs = lf(lib_rs);
    let needle = "if let tauri::RunEvent::Exit = event {";
    assert_eq!(lib_rs.matches(needle).count(), 1);
    let open = lib_rs.find(needle).unwrap() + needle.len() - 1;
    assert_eq!(
        without_whitespace(&production_text(&lib_rs[open..block_end(&lib_rs, open)])),
        without_whitespace(
            "{
                super::builder_workspace::shutdown_dev_servers(&app.state::<AppState>());
                if let Err(error) = nexus_connectors_llm::model_hub::terminate_in_flight_downloads() {
                    eprintln!(\"[shutdown] {error}\");
                }
            }"
        ),
        "the exit arm"
    );

    let model_hub = production_text(&lf(model_hub));
    assert_eq!(
        model_hub.matches(".spawn()").count(),
        1,
        "one transfer spawn"
    );
    assert_eq!(model_hub.matches("impl Transfer for ").count(), 1);
    assert!(model_hub.contains("impl Transfer for RegisteredTransfer<'_>"));
    let download = model_hub
        .split("pub fn download_model_file(")
        .nth(1)
        .and_then(|rest| rest.split("\n}\n").next())
        .expect("download_model_file");
    let start = download
        .find("IN_FLIGHT_DOWNLOADS.start(")
        .expect("the registry starts the transfer");
    let spawn = download.find(".spawn()").expect("the transfer spawn");
    let started = start + download[start..].find("})?;").unwrap();
    assert!(
        start < spawn && spawn < started,
        "spawned inside the registry"
    );
}

/// Final Gate item B: a downloaded model is registered with Ollama only at
/// the authorized Ollama address (the operator's `OLLAMA_URL` or the fixed
/// default), and not at all when that address is unavailable. `model_hub.rs`
/// names no Ollama address of its own (its registration test posts to a
/// loopback stand-in).
#[test]
fn p0_fg_model_registration_uses_the_authorized_ollama_address() {
    let model_hub = include_str!("../../../../../connectors/llm/src/model_hub.rs");
    assert_model_registration_is_authorized(LIB_RS, model_hub);
    assert_model_registration_is_authorized(&crlf(LIB_RS), &crlf(model_hub));
}

/// The guard above, for either line ending.
fn assert_model_registration_is_authorized(lib_rs: &str, model_hub: &str) {
    let model_hub = production_text(&lf(model_hub));
    assert!(
        !model_hub.contains("11434"),
        "model_hub.rs names an address"
    );
    assert!(
        !model_hub.contains("localhost"),
        "model_hub.rs names a host"
    );
    let lib_rs = without_whitespace(&production_text(&lf(lib_rs)));
    assert_eq!(
        lib_rs
            .matches("register_downloaded_model_with_ollama(")
            .count(),
        1
    );
    assert!(lib_rs.contains(&without_whitespace(
        "if let Ok(ollama_base) = super::authorized_ollama_base_url() {
            let _ = super::model_hub::register_downloaded_model_with_ollama(
                &ollama_base,"
    )));
}

/// Final Gate item C (redaction): a messaging transport error names no
/// request URL, so a stored Telegram bot token, which travels in the URL
/// path, never reaches the interface through an error. The failing request
/// here goes to a closed loopback port.
#[test]
fn p0_fg_messaging_errors_never_carry_the_bot_token() {
    let closed_port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let url = format!("http://{closed_port}/bot123456:fg-secret-token/sendMessage");
    let error = crate::block_on_async(async { reqwest::Client::new().post(&url).send().await })
        .unwrap_err();
    assert!(
        error.to_string().contains("fg-secret-token"),
        "precondition: reqwest names the URL in its errors"
    );
    let reported = crate::commands::apps::messaging_transport_error("telegram send", error);
    assert!(reported.starts_with("telegram send: "), "{reported}");
    assert!(!reported.contains("fg-secret-token"), "{reported}");
    assert!(!reported.contains("sendMessage"), "{reported}");

    let apps = include_str!("../../commands/apps.rs");
    assert_messaging_errors_are_redacted(apps);
    assert_messaging_errors_are_redacted(&crlf(apps));
}

/// The source side of the guard above, for either line ending: every
/// request error goes through `messaging_transport_error`, and every client
/// and body read through the bounded helpers, which redact the same way.
fn assert_messaging_errors_are_redacted(apps: &str) {
    let apps = lf(apps);
    let helpers = apps
        .find("pub(crate) fn messaging_transport_error(")
        .unwrap();
    let start = apps.find("pub(crate) fn messaging_send(").unwrap();
    let end = apps[start..]
        .find("pub(crate) fn messaging_poll_messages(")
        .map(|at| start + at)
        .unwrap();
    let poll_end = apps[end..].find("\n}\n").map(|at| end + at).unwrap();
    let helper_text = &apps[helpers..start];
    assert_eq!(
        helper_text.matches("messaging_transport_error(").count(),
        3,
        "the definition, the client and the body read"
    );
    assert!(
        !helper_text.contains("{e}\"))"),
        "a helper formats a raw error"
    );
    for (name, body) in [
        ("messaging_send", &apps[start..end]),
        ("messaging_poll_messages", &apps[end..poll_end]),
    ] {
        assert_eq!(
            body.matches("messaging_transport_error(").count(),
            3,
            "{name}"
        );
        assert_eq!(body.matches("messaging_client()?").count(), 3, "{name}");
        assert_eq!(
            body.matches("messaging_body(resp).await").count(),
            3,
            "{name}"
        );
        for unbounded in ["reqwest::Client::new()", ".text()"] {
            assert!(!body.contains(unbounded), "{name}: {unbounded}");
        }
        assert!(!body.contains("{e}\"))"), "{name} formats a raw error");
    }
}

/// Answer one loopback request with `head` and then `body`, all at once or,
/// with `drip`, one byte per interval until the client gives up.
fn serve_answer(
    head: String,
    body: Vec<u8>,
    drip: Option<std::time::Duration>,
) -> (String, std::thread::JoinHandle<()>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(30)))
            .unwrap();
        let mut request = Vec::new();
        let mut byte = [0u8; 1];
        while !request.ends_with(b"\r\n\r\n") && matches!(stream.read(&mut byte), Ok(1)) {
            request.push(byte[0]);
        }
        if stream.write_all(head.as_bytes()).is_err() {
            return;
        }
        match drip {
            None => {
                let _ = stream.write_all(&body);
            }
            Some(interval) => {
                for byte in body {
                    std::thread::sleep(interval);
                    if stream.write_all(&[byte]).is_err() {
                        return;
                    }
                }
            }
        }
    });
    (base, server)
}

/// Final Gate resource bound: messaging sends and polls are bounded in total
/// time and in the response they read, and a timeout says so without naming
/// the URL. (Loopback stand-ins answer; no platform is contacted.)
#[test]
fn p0_fg_messaging_requests_are_bounded_in_time_and_size() {
    use crate::commands::apps::{
        messaging_body_bounded, messaging_client, messaging_client_with, messaging_transport_error,
        MAX_MESSAGING_RESPONSE_BYTES, MESSAGING_REQUEST_TIMEOUT,
    };
    assert_eq!(
        MESSAGING_REQUEST_TIMEOUT,
        std::time::Duration::from_secs(30)
    );
    assert_eq!(MAX_MESSAGING_RESPONSE_BYTES, 4 * 1024 * 1024);

    let chunk = "b".repeat(1000);
    for (head, body) in [
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 2048\r\nConnection: close\r\n\r\n".to_string(),
            "a".repeat(2048),
        ),
        (
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
                .to_string(),
            format!("3e8\r\n{chunk}\r\n3e8\r\n{chunk}\r\n0\r\n\r\n"),
        ),
    ] {
        let (base, server) = serve_answer(head, body.into_bytes(), None);
        let result = crate::block_on_async(async {
            let response = messaging_client()?
                .get(format!("{base}/bot1:fg-secret/getUpdates"))
                .send()
                .await
                .map_err(|e| messaging_transport_error("poll", e))?;
            messaging_body_bounded(response, 1024).await
        });
        assert_eq!(
            result,
            Err("body: the response is larger than 1024 bytes".to_string())
        );
        server.join().unwrap();
    }

    // An answer that drips its 18-byte body over 5.4 s is abandoned at a
    // 1 s timeout.
    let body = br#"{"ok":true,"r":[]}"#.to_vec();
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let (base, server) = serve_answer(head, body, Some(std::time::Duration::from_millis(300)));
    let result = crate::block_on_async(async {
        let response = messaging_client_with(std::time::Duration::from_secs(1))?
            .get(format!("{base}/bot1:fg-secret/getUpdates"))
            .send()
            .await
            .map_err(|e| messaging_transport_error("poll", e))?;
        messaging_body_bounded(response, 1024).await
    });
    let error = result.expect_err("a 5.4 s body must not be read within a 1 s timeout");
    assert!(error.contains("timed out"), "{error}");
    assert!(!error.contains("fg-secret"), "{error}");
    server.join().unwrap();
}

// ── Final Gate item C: credential-bearing process arguments ──────────────
//
// The production-text reader below is the same algorithm as the one in
// `phase0_surface/tests.rs` (whose helpers are private to that module): it
// drops comments and every `#[cfg(test)]` / `#[cfg(any(test, ..))]` item and
// keeps literals.

/// Directories that hold no production source: build output, dependencies,
/// benchmarks, integration tests, examples, benches and fixtures. Dot
/// directories are skipped as well.
const NOT_PRODUCTION_DIRS: &[&str] = &[
    "target",
    "node_modules",
    "dist",
    "benchmarks",
    "tests",
    "examples",
    "benches",
    "fixtures",
];

/// (workspace-relative path, production text) of every production Rust
/// source beneath a `src` directory of the workspace.
fn workspace_production_sources() -> Vec<(String, String)> {
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                if !name.starts_with('.') && !NOT_PRODUCTION_DIRS.contains(&name.as_str()) {
                    walk(root, &path, out);
                }
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let in_src = relative.split('/').any(|component| component == "src");
            if in_src
                && name.ends_with(".rs")
                && !name.ends_with("tests.rs")
                && !name.ends_with("_test.rs")
                && name != "build.rs"
            {
                let text = std::fs::read_to_string(&path).unwrap();
                out.push((relative, production_text(&text)));
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    assert!(out.len() > 500, "workspace sources not found");
    out
}

/// End of the string, raw-string or char literal starting at `i`, if one
/// starts there (a lifetime is not a literal).
fn literal_end(b: &[u8], i: usize) -> Option<usize> {
    let ident = |at: usize| b[at].is_ascii_alphanumeric() || b[at] == b'_';
    match b[i] {
        b'"' => {
            let mut j = i + 1;
            while j < b.len() {
                match b[j] {
                    b'\\' => j += 2,
                    b'"' => return Some(j + 1),
                    _ => j += 1,
                }
            }
            Some(b.len())
        }
        b'r' if i == 0 || !ident(i - 1) || (b[i - 1] == b'b' && (i == 1 || !ident(i - 2))) => {
            let mut j = i + 1;
            while j < b.len() && b[j] == b'#' {
                j += 1;
            }
            if j >= b.len() || b[j] != b'"' {
                return None;
            }
            let hashes = j - i - 1;
            let mut k = j + 1;
            while k < b.len() {
                if b[k] == b'"' && b[k + 1..].iter().take_while(|&&c| c == b'#').count() >= hashes {
                    return Some(k + 1 + hashes);
                }
                k += 1;
            }
            Some(b.len())
        }
        b'\'' if i + 2 < b.len() => {
            if b[i + 1] == b'\\' {
                let close = b[i + 3..].iter().position(|&c| c == b'\'')?;
                return Some(i + 3 + close + 1);
            }
            let width = match b[i + 1] {
                0x00..=0x7f => 1,
                0xc0..=0xdf => 2,
                0xe0..=0xef => 3,
                _ => 4,
            };
            (b.get(i + 1 + width) == Some(&b'\'')).then_some(i + 2 + width)
        }
        _ => None,
    }
}

/// End of the comment starting at `i`, if one starts there. A line comment
/// ends before its newline; block comments nest.
fn comment_end(b: &[u8], i: usize) -> Option<usize> {
    if b[i..].starts_with(b"//") {
        return Some(
            b[i..]
                .iter()
                .position(|&c| c == b'\n')
                .map_or(b.len(), |at| i + at),
        );
    }
    if !b[i..].starts_with(b"/*") {
        return None;
    }
    let (mut depth, mut j) = (0usize, i);
    while j < b.len() {
        if b[j..].starts_with(b"/*") {
            depth += 1;
            j += 2;
        } else if b[j..].starts_with(b"*/") {
            depth -= 1;
            j += 2;
            if depth == 0 {
                return Some(j);
            }
        } else {
            j += 1;
        }
    }
    Some(b.len())
}

/// End of what a `#[cfg(..)]` attribute ending at `i` applies to. An item
/// (or statement) ends at its `;` outside any bracket or at the brace closing
/// its block. A field, variant, match arm or argument also ends at its `,`,
/// or just before the bracket closing the enclosing list.
fn item_end(b: &[u8], mut i: usize) -> usize {
    let first = b[i..]
        .iter()
        .position(|c| !c.is_ascii_whitespace())
        .map_or(b.len(), |at| i + at);
    let word: String = b[first..]
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric() || **c == b'_')
        .map(|&c| c as char)
        .collect();
    let item = b.get(first) == Some(&b'#')
        || [
            "mod",
            "fn",
            "pub",
            "use",
            "impl",
            "struct",
            "enum",
            "const",
            "static",
            "type",
            "trait",
            "macro_rules",
            "async",
            "unsafe",
            "extern",
            "let",
        ]
        .contains(&word.as_str());
    let (mut nesting, mut braces) = (0usize, 0usize);
    while i < b.len() {
        if let Some(end) = literal_end(b, i).or_else(|| comment_end(b, i)) {
            i = end;
            continue;
        }
        let outermost = nesting == 0 && braces == 0;
        match b[i] {
            b'(' | b'[' => nesting += 1,
            b')' | b']' if outermost => return i,
            b')' | b']' => nesting = nesting.saturating_sub(1),
            b'{' => braces += 1,
            b'}' if braces == 0 => return i,
            b'}' => {
                braces -= 1;
                if braces == 0 && nesting == 0 {
                    return i + 1;
                }
            }
            b';' if outermost => return i + 1,
            b',' if outermost && !item => return i + 1,
            _ => {}
        }
        i += 1;
    }
    b.len()
}

/// Production text of a Rust source: comments removed, and every item under
/// `#[cfg(test)]` or `#[cfg(any(test, ..))]` removed. Literals are kept and
/// are never read as delimiters or attributes.
fn production_text(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if let Some(end) = literal_end(b, i) {
            out.extend_from_slice(&b[i..end]);
            i = end;
        } else if let Some(end) = comment_end(b, i) {
            out.push(b' ');
            i = end;
        } else if b[i..].starts_with(b"#[cfg(test)]") || b[i..].starts_with(b"#[cfg(any(test") {
            let attribute = i + b[i..].windows(2).position(|w| w == b")]").unwrap() + 2;
            i = item_end(b, attribute);
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("cuts fall on ASCII boundaries")
}

/// Credential markers in the production text of a file that runs curl: a
/// credential-bearing header name, a bearer scheme, or a Telegram bot path,
/// compared in lower case.
const CREDENTIAL_MARKERS: &[&str] = &[
    "authorization",
    "x-api-key",
    "x-subscription-token",
    "x-goog-api-key",
    "bearer ",
    "/bot{",
];

/// Every production file that runs curl and still names a credential, with
/// the exact marker count and why no desktop route puts that credential on
/// a process command line. A new site, or a changed count, fails until it
/// is classified here.
const CREDENTIAL_CURL_SITES: &[(&str, usize, &str)] = &[
    (
        "connectors/core/src/validation.rs",
        3,
        "key validation is reached only by the nexus-cli setup flow, not by the desktop",
    ),
    (
        "connectors/llm/src/providers/mod.rs",
        5,
        "the refusal list: curl_post_command refuses these headers before a command exists; keys go through post_json_in_process",
    ),
    (
        "connectors/web/src/search.rs",
        1,
        "latent: only the social-poster pipeline's real search step builds this connector, and the desktop never runs it",
    ),
    (
        "crates/nexus-capability-measurement/src/evaluation/nim_client.rs",
        2,
        "the validation-run commands are closed, and cm_run_ab_validation is closed by the C5 closure (item G workstream), so no query is sent",
    ),
    (
        "crates/nexus-capability-measurement/src/evaluation/openrouter_client.rs",
        2,
        "the validation-run commands are closed; the three-way comparison is closed",
    ),
    (
        "crates/nexus-mcp/src/tools.rs",
        2,
        "the GitHub tool runs only through mcp2_server_handle, which is closed",
    ),
    (
        "crates/nexus-perception/src/vision.rs",
        2,
        "perception_init is closed (CredentialTransport), so no key is ever set",
    ),
    (
        "kernel/src/actuators/image_gen.rs",
        4,
        "the image actuator is refused by Phase0AgentExecutor",
    ),
    (
        "kernel/src/actuators/tts.rs",
        2,
        "the speech actuator is refused by Phase0AgentExecutor",
    ),
];

/// Final Gate item C: no reachable credential is passed to curl on its
/// command line. Every production file that runs curl is scanned for
/// credential markers, and the files that have any must be exactly the
/// classified ones above, each with its count. The four hosted providers
/// that did this now post in process, the external tools that did are
/// refused, and the MCP client refuses credentials before curl.
#[test]
fn p0_fg_no_reachable_credential_reaches_a_curl_command_line() {
    let mut found = Vec::new();
    for (relative, text) in workspace_production_sources() {
        let invocations =
            text.matches("Command::new(\"curl\")").count() + text.matches("(\"curl\",").count();
        if invocations == 0 {
            continue;
        }
        let lower = text.to_ascii_lowercase();
        let markers: usize = CREDENTIAL_MARKERS
            .iter()
            .map(|marker| lower.matches(marker).count())
            .sum();
        if markers > 0 {
            found.push((relative, markers));
        }
    }
    let expected: Vec<_> = CREDENTIAL_CURL_SITES
        .iter()
        .map(|(file, count, _)| (file.to_string(), *count))
        .collect();
    assert_eq!(
        found, expected,
        "a curl site that names a credential must be classified"
    );
    for (_, _, reason) in CREDENTIAL_CURL_SITES {
        assert!(!reason.is_empty());
    }
    // The routes this workstream moved or closed stay that way.
    let sources: std::collections::HashMap<_, _> =
        workspace_production_sources().into_iter().collect();
    for provider in ["openai", "deepseek", "gemini", "nvidia"] {
        let text = &sources[&format!("connectors/llm/src/providers/{provider}.rs")];
        assert!(text.contains("post_json_in_process("), "{provider}");
        assert!(!text.contains("curl_post_json"), "{provider}");
    }
    for tool in ["github", "slack", "jira"] {
        assert!(
            nexus_external_tools::execution::phase0_refusal(tool).is_some(),
            "{tool}"
        );
    }
    assert!(sources["protocols/src/mcp_client.rs"].contains("needs credentials"));
}
