//! P0-FINAL-GATE-CLOSURE guards for items B and F (destinations and Nexus
//! Link), C (credentials in process arguments) and I (helper programs).
//!
//! This module is compiled only for tests, but the workspace text guards in
//! `tests.rs` read every source file under `src/` whose name does not end in
//! `tests.rs`, so they read this one too. Every needle those guards look for
//! (and every client entry point this module names) is therefore spelled
//! with `concat!`: the guards keep describing production code only.

use super::{closed, Closure};

const LIB_RS: &str = include_str!("../lib.rs");
const CRATE_BRIDGES_RS: &str = include_str!("../commands/crate_bridges.rs");

/// Final Gate items B and F: IPC commands closed because the destination they
/// reached (a URL, an agent or server address, a peer) was the caller's
/// choice, and no backend-owned policy makes such a destination an egress
/// grant. `(module defining the handler, command, closure)`; `""` is the
/// `runtime` module in `lib.rs`.
const CLOSED_DESTINATION_COMMANDS: &[(&str, &str, Closure)] = &[
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
];

/// Every closure reason this workstream adds.
const EGRESS_CLOSURES: &[Closure] = &[Closure::NetworkDestination, Closure::PeerTransfer];

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
    for (module, command, closure) in CLOSED_DESTINATION_COMMANDS {
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
    let calls: [ClosedCall; 12] = [
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
    ];
    assert_eq!(calls.len(), CLOSED_DESTINATION_COMMANDS.len());
    for (command, call) in calls {
        let (_, _, closure) = CLOSED_DESTINATION_COMMANDS
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
            include_str!("../commands/governance.rs"),
        ),
        (
            "commands/tools_infra.rs",
            include_str!("../commands/tools_infra.rs"),
        ),
        ("commands/apps.rs", include_str!("../commands/apps.rs")),
        (
            "commands/model_hub.rs",
            include_str!("../commands/model_hub.rs"),
        ),
    ];
    let clients = [
        concat!(".discover", "_agent("),
        concat!(".send", "_task("),
        concat!(".get_task", "_status("),
        concat!(".cancel", "_task("),
        concat!(".connect", "_server("),
        concat!(".call", "_tool("),
        concat!(".send", "_model("),
        concat!("extract_theme", "_from_url"),
        concat!("a2a_crate_cmds::a2a_crate", "_send_task"),
        concat!("a2a_crate_cmds::a2a_crate", "_get_task"),
        concat!("a2a_crate_cmds::a2a_crate", "_discover_agent"),
        concat!("Command::new(\"cu", "rl\")"),
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
