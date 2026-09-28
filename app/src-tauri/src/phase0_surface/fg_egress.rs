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
