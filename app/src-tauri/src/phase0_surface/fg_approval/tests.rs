//! P0-FINAL-GATE-CLOSURE guards for item G (approval channels) and C5
//! (the residual capability-measurement route).
//!
//! The guards read sources through compile-time `include_str!` paths,
//! relative to this file.

use crate::phase0_surface::{closed, Closure};

fn without_whitespace(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Index just past the brace closing the block that opens at `open`. String
/// literals are skipped; the bodies read here contain no char literal braces.
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

/// The text without whole-line comments. A trailing comment on a code line is
/// kept, which only makes the guards below stricter.
fn code_lines(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whitespace-free parameter list and body (comment lines dropped) of the
/// only `fn <name>(` in `src`.
fn fn_shape(src: &str, name: &str) -> (String, String) {
    let needle = format!("fn {name}(");
    let mut found = src.match_indices(&needle).map(|(at, _)| at);
    let at = found
        .next()
        .unwrap_or_else(|| panic!("{name}: function not found"));
    assert!(found.next().is_none(), "{name}: defined more than once");
    let params_start = at + needle.len() - 1;
    let mut depth = 0usize;
    let mut params_end = params_start;
    for (offset, c) in src[params_start..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    params_end = params_start + offset + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    assert!(params_end > params_start, "{name}: unterminated parameters");
    let open = params_end + src[params_end..].find('{').unwrap();
    let body = code_lines(&src[open + 1..block_end(src, open) - 1]);
    (
        without_whitespace(&src[params_start..params_end]),
        without_whitespace(&body).replace(",)", ")"),
    )
}

// ── C5: the residual capability-measurement route ───────────────────────────

/// P0-FINAL-GATE C5: `cm_run_ab_validation` read a provider key from the
/// environment through a fallback chain and built clients for a fixed Groq
/// endpoint. It now takes no input and only returns its bounded reason, so
/// no argument, credential, client or provider is used.
#[test]
fn p0_fg_c5_ab_validation_route_is_closed_before_any_input() {
    let reason = closed("cm_run_ab_validation", Closure::AmbientResource);
    match crate::commands::crate_bridges::cm_run_ab_validation() {
        Ok(_) => panic!("the A/B validation route ran"),
        Err(error) => assert_eq!(error, reason),
    }
    let (params, body) = fn_shape(
        include_str!("../../commands/crate_bridges.rs"),
        "cm_run_ab_validation",
    );
    assert_eq!(params, "()");
    assert_eq!(
        body,
        "Err(crate::phase0_surface::closed(\"cm_run_ab_validation\",crate::phase0_surface::Closure::AmbientResource))"
    );
}

/// The capability-measurement functions the desktop may call: in-memory
/// session, scorecard and report bookkeeping, with no environment, file,
/// process or network access.
const IN_MEMORY_MEASUREMENT_CALLS: &[&str] = &[
    "compare_agents",
    "evaluate_single_response",
    "get_agent_scorecard",
    "get_boundary_map",
    "get_calibration_report",
    "get_capability_profile",
    "get_classification_census",
    "get_gaming_flags",
    "get_gaming_report_batch",
    "get_locked_batteries",
    "get_measurement_session",
    "list_measurement_sessions",
    "start_measurement_session",
    "trigger_evolution_feedback",
    "upload_to_darwin",
];

/// P0-FINAL-GATE C5: after the closure, the desktop reaches the
/// capability-measurement crate only through its in-memory functions and
/// types. No desktop source names the evaluation client, runner or
/// validation-run modules, and the desktop state holds no test battery.
#[test]
fn p0_fg_c5_desktop_reaches_only_in_memory_measurement() {
    const CRATE_PATH: &str = "nexus_capability_measurement::";
    for (file, src) in [
        (
            "commands/crate_bridges.rs",
            include_str!("../../commands/crate_bridges.rs"),
        ),
        ("lib.rs", include_str!("../../lib.rs")),
    ] {
        let code = code_lines(src);
        for (at, _) in code.match_indices(CRATE_PATH) {
            let path: String = code[at + CRATE_PATH.len()..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':')
                .collect();
            let segments: Vec<&str> = path.split("::").collect();
            for module in [
                "nim_client",
                "openrouter_client",
                "validation_run",
                "ab_validation",
                "runner",
                "agent_adapter",
            ] {
                assert!(!segments.contains(&module), "{file}: {CRATE_PATH}{path}");
            }
            let item = segments.last().copied().unwrap_or_default();
            let is_call = segments.len() == 2
                && segments[0] == "tauri_commands"
                && item.starts_with(|c: char| c.is_ascii_lowercase());
            if is_call {
                assert!(
                    IN_MEMORY_MEASUREMENT_CALLS.contains(&item),
                    "{file}: {CRATE_PATH}{path} is not an in-memory measurement call"
                );
            }
        }
    }
    let lib = code_lines(include_str!("../../lib.rs"));
    assert_eq!(lib.matches("MeasurementState::").count(), 2);
    assert_eq!(
        lib.matches("MeasurementState::with_batteries(Vec::new())")
            .count(),
        2
    );
}

/// A directory of the workspace, located from this crate's manifest.
fn workspace_dir(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// Every Rust source under `dir`, recursively, as (path relative to `dir`
/// with `/` separators, contents), sorted by path. The sources are read at
/// run time, so a file added later is checked without a list to update.
fn rust_sources_under(dir: &std::path::Path) -> Vec<(String, String)> {
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
            .map(|entry| entry.expect("directory entry").path())
            .collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                walk(root, &path, out);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let relative = path
                    .strip_prefix(root)
                    .expect("inside the directory")
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
                out.push((relative, text));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out
}

/// The sources under the workspace directory `relative`, which must include
/// each of `expected` (so a wrong path cannot pass as an empty scan).
fn sources_including(relative: &str, expected: &[&str]) -> Vec<(String, String)> {
    let sources = rust_sources_under(&workspace_dir(relative));
    for file in expected {
        assert!(
            sources.iter().any(|(path, _)| path == file),
            "{relative}/{file} not found"
        );
    }
    sources
}

/// Every production source of the capability-measurement crate, read at run
/// time from its source directory. (P0-FINAL-GATE F7: this used to be a
/// compile-time list of the 32 files the directory held, which a file added
/// later would have escaped.)
fn measurement_sources() -> Vec<(String, String)> {
    sources_including(
        "crates/nexus-capability-measurement/src",
        &["lib.rs", "tauri_commands.rs", "evaluation/nim_client.rs"],
    )
}

/// Production code of a capability-measurement source: this crate keeps its
/// tests in one trailing `#[cfg(test)] mod tests` block per file, so the
/// production code is the text before it, without comment lines.
fn measurement_production_code(file: &str, src: &str) -> String {
    let src = src.replace("\r\n", "\n");
    let production = match src.find("#[cfg(test)]") {
        Some(at) => {
            assert_eq!(src.matches("#[cfg(test)]").count(), 1, "{file}");
            assert!(
                src[at..].starts_with("#[cfg(test)]\nmod tests {"),
                "{file}: the test block must be the trailing tests module"
            );
            &src[..at]
        }
        None => &src[..],
    };
    code_lines(production)
}

/// P0-FINAL-GATE C5: no NVIDIA NIM or OpenRouter credential can fall through
/// to the Groq endpoint. In the capability-measurement crate's production
/// code the one environment read is the Groq key lookup, which asks for
/// `GROQ_API_KEY` alone; the evaluation client posts only to the Groq
/// endpoint; and no other provider's key variable is named at all.
#[test]
fn p0_fg_c5_measurement_clients_take_only_the_groq_key() {
    let sources = measurement_sources();
    let mut env_reads = Vec::new();
    for (file, src) in &sources {
        let code = measurement_production_code(file, src);
        for other_provider in ["NVIDIA_NIM_API_KEY", "OPENROUTER_API_KEY"] {
            assert!(!code.contains(other_provider), "{file}: {other_provider}");
        }
        assert!(!code.contains("var_os("), "{file}: var_os");
        let reads = code.matches("env::var").count();
        if reads > 0 {
            env_reads.push((file.as_str(), reads));
        }
    }
    assert_eq!(
        env_reads,
        [("evaluation/nim_client.rs", 1)],
        "the crate reads one environment variable, through the Groq key lookup"
    );

    let client = sources
        .iter()
        .find(|(file, _)| file == "evaluation/nim_client.rs")
        .map(|(file, src)| measurement_production_code(file, src))
        .unwrap();
    let (params, body) = fn_shape(&client, "groq_api_key");
    assert_eq!(params, "(lookup:implFnOnce(&str)->Option<String>)");
    assert_eq!(body, "lookup(GROQ_API_KEY_VAR)");
    let (params, body) = fn_shape(&client, "groq_api_key_from_env");
    assert_eq!(params, "()");
    assert_eq!(body, "groq_api_key(|name|std::env::var(name).ok())");
    let client = without_whitespace(&client);
    assert!(client.contains("pubconstGROQ_API_KEY_VAR:&str=\"GROQ_API_KEY\";"));
    assert!(client
        .contains("constNIM_ENDPOINT:&str=\"https://api.groq.com/openai/v1/chat/completions\";"));
}

// ── G: approval channels ─────────────────────────────────────────────────────

/// The whitespace-free denial `surface` returns for `closure`.
fn denial(surface: &str, closure: &str) -> String {
    format!(
        "returnErr(crate::phase0_surface::closed(\"{surface}\",crate::phase0_surface::Closure::{closure}));"
    )
}

/// Byte offset of the first `needle` in `body`, which must contain it.
fn position(body: &str, needle: &str) -> usize {
    body.find(needle)
        .unwrap_or_else(|| panic!("{needle} not found in {body}"))
}

/// P0-FINAL-GATE (item G): an L6 (transcendent) agent needs a human approval
/// the backend cannot verify, so L6 is unavailable. Before any state
/// changes, the desktop routes that create, start, register or load an agent
/// refuse an L6 one with the bounded `ApprovalRequired` reason, or skip it:
/// - `create_agent` refuses level 6 right after parsing, before anything is
///   written;
/// - `start_agent` refuses before the agent is restarted, stored or audited;
/// - restore registers no level-6 record and writes nothing to it;
/// - the prebuilt load skips a level-6 manifest before the manifest is
///   registered, stored, named or published;
/// - `approve_consent_request` and `batch_approve_consents` refuse a
///   transcendent request before resolving anything, and nothing in the
///   consent module creates or starts an agent;
/// - nothing enqueues a transcendent request any more.
///
/// (The earlier version of this guard said each such route refused first.
/// It did not check the prebuilt load, which still registered the twelve L6
/// prebuilt manifests whenever it found them.)
#[test]
fn p0_fg_g_transcendent_agents_are_refused_before_any_state_change() {
    let agents = include_str!("../../commands/agents.rs");
    let consent = include_str!("../../commands/consent.rs");
    let chat = include_str!("../../commands/chat_llm.rs");

    let (_, load) = fn_shape(chat, "load_prebuilt_agents");
    let skip = position(&load, "ifmanifest.autonomy_level==Some(6){");
    assert!(
        load[skip..].starts_with(concat!(
            "ifmanifest.autonomy_level==Some(6){eprintln!(",
            "\"prebuilt:{}notloaded:transcendent(L6)agentsareunavailableinPhaseZero\",",
            "manifest.name);continue;}",
        )),
        "{load}"
    );
    assert!(skip > position(&load, "parse_agent_manifest_json(&manifest_json)"));
    for later in [
        "existing_names.contains(",
        "supervisor.start_agent(",
        "self.db.save_agent(",
        "meta.insert(",
        "publish_prebuilt_manifest_to_marketplace(",
        "existing_names.insert(",
    ] {
        assert!(
            skip < position(&load, later),
            "load_prebuilt_agents: {later}"
        );
    }
    let lib = without_whitespace(&code_lines(include_str!("../../lib.rs")));
    assert!(lib.contains(
        "fnload_agents_deferred(&self){restore_persisted_agents(self);self.load_prebuilt_agents();}"
    ));

    let (_, create) = fn_shape(agents, "create_agent");
    assert!(
        create.starts_with(&format!(
            "letmanifest=parse_agent_manifest_json(manifest_json.as_str())?;ifmanifest.autonomy_level==Some(6){{{}}}",
            denial("create_agent", "ApprovalRequired")
        )),
        "{create}"
    );
    assert!(create.ends_with("create_agent_immediately(state,manifest,manifest_json)"));

    let (_, start) = fn_shape(agents, "start_agent");
    let refusal = position(&start, &denial("start_agent", "ApprovalRequired"));
    assert!(start.contains(
        "letstored_transcendent=find_manifest(state,&agent_id).is_some_and(|manifest|manifest.autonomy_level==Some(6));"
    ));
    assert!(start.contains(".get_agent(parsed).is_some_and(|handle|handle.autonomy_level==6);"));
    assert!(start.contains("ifstored_transcendent||registered_transcendent{"));
    for later in [
        "restart_agent(",
        "update_agent_state(",
        "persist_agent_fuel_ledger(",
        "register_manifest_schedule(",
        "update_last_action(",
        "log_event(",
    ] {
        assert!(refusal < position(&start, later), "start_agent: {later}");
    }

    let (_, restore) = fn_shape(agents, "restore_persisted_agents");
    let skip = position(&restore, "ifmanifest.autonomy_level==Some(6){");
    assert!(restore[skip..].contains("continue;"));
    assert!(skip < position(&restore, "start_agent_with_id("));
    assert!(skip > position(&restore, "validate_stored_manifest(&manifest)"));

    for (surface, name) in [
        ("approve_consent_request", "approve_consent_request"),
        ("batch_approve_consents", "batch_approve_consents"),
    ] {
        let (_, body) = fn_shape(consent, name);
        let refusal = position(&body, &denial(surface, "ApprovalRequired"));
        assert!(
            refusal < position(&body, "resolve_consent("),
            "{name}: refused after resolving"
        );
        assert!(body.contains("TRANSCENDENT_CREATION"), "{name}");
    }
    let consent_code = code_lines(consent);
    for forbidden in [
        "create_agent_immediately(",
        "restart_agent(",
        "start_agent(",
    ] {
        assert!(!consent_code.contains(forbidden), "consent: {forbidden}");
    }

    // Nothing enqueues a transcendent request: the operation type is named
    // only by the consent module's refusal constant.
    for (file, src) in [
        ("commands/agents.rs", agents),
        ("commands/consent.rs", consent),
        (
            "commands/cognitive.rs",
            include_str!("../../commands/cognitive.rs"),
        ),
        (
            "commands/chat_llm.rs",
            include_str!("../../commands/chat_llm.rs"),
        ),
        ("lib.rs", include_str!("../../lib.rs")),
    ] {
        let code = code_lines(src);
        let expected = usize::from(file == "commands/consent.rs");
        assert_eq!(
            code.matches("\"transcendent_creation\"").count(),
            expected,
            "{file}"
        );
        assert!(!code.contains("enqueue_transcendent_review"), "{file}");
    }
    assert!(without_whitespace(&code_lines(consent))
        .contains("constTRANSCENDENT_CREATION:&str=\"transcendent_creation\";"));
}

/// P0-FINAL-GATE (item G): the goal, autonomous-loop and tool routes check
/// for an L6 (transcendent) agent first:
/// - `assign_agent_goal` refuses before the rate limit, the input check, the
///   goal assignment and the audit event. `execute_agent_goal` only takes a
///   snapshot before it calls `assign_agent_goal`, and every other goal
///   route (scheduled, schedule-runner and hivemind) goes through
///   `execute_agent_goal`;
/// - `start_autonomous_loop` refuses before the manifest is read or the
///   scheduler registers anything, and the command only delegates to it;
/// - `tool_call_autonomy` refuses a registered L6 agent before it returns a
///   level;
/// - the shared check only reads.
#[test]
fn p0_fg_g_goal_loop_and_tool_routes_check_for_transcendent_agents_first() {
    let cognitive = include_str!("../../commands/cognitive.rs");
    let check = "ifis_transcendent_agent(state,&agent_id){";

    let (_, assign) = fn_shape(cognitive, "assign_agent_goal");
    assert!(
        assign.starts_with(&format!(
            "{check}{}}}state.check_rate(",
            denial("assign_agent_goal", "ApprovalRequired")
        )),
        "{assign}"
    );
    let (_, execute) = fn_shape(cognitive, "execute_agent_goal");
    assert!(
        execute.starts_with(concat!(
            "letbefore_snapshot=capture_agent_snapshot(state,&agent_id);",
            "letgoal_id=assign_agent_goal(",
        )),
        "{execute}"
    );
    // The runtime's goal assignment is reached only through
    // `assign_agent_goal`.
    let code = without_whitespace(&code_lines(cognitive));
    assert_eq!(
        code.matches(".assign_goal(").count(),
        1,
        "one goal assignment"
    );
    assert_eq!(
        without_whitespace(&code_lines(include_str!("../../lib.rs")))
            .matches(".assign_goal(")
            .count(),
        0
    );

    let (_, looping) = fn_shape(cognitive, "start_autonomous_loop");
    assert!(
        looping.starts_with(&format!(
            "{check}{}}}letinterval=",
            denial("start_autonomous_loop", "ApprovalRequired")
        )),
        "{looping}"
    );
    assert!(looping.ends_with(
        ".register_agent(&agent_id,&cron_expr,&full_goal).map_err(agent_error)?;Ok(())"
    ));
    let (_, command) = fn_shape(include_str!("../../lib.rs"), "start_autonomous_loop");
    assert_eq!(
        command,
        "super::start_autonomous_loop(state.inner(),agent_id,interval_seconds,goal_override)"
    );

    let (_, tools) = fn_shape(
        include_str!("../../commands/crate_bridges.rs"),
        "tool_call_autonomy",
    );
    assert!(
        tools.ends_with(&format!(
            "ifagent.autonomy_level==6{{{}}}Ok(claimed.min(agent.autonomy_level))",
            denial("tools_execute", "ApprovalRequired")
        )),
        "{tools}"
    );

    let (params, helper) = fn_shape(
        include_str!("../../commands/agents.rs"),
        "is_transcendent_agent",
    );
    assert_eq!(params, "(state:&AppState,agent_id:&str)");
    assert!(helper.contains(
        "find_manifest(state,agent_id).is_some_and(|manifest|manifest.autonomy_level==Some(6));"
    ));
    assert!(helper.contains(".get_agent(id).is_some_and(|handle|handle.autonomy_level==6)"));
    assert!(helper.ends_with("stored||registered"));
    for write in [
        "start_agent",
        "restart_agent",
        "stop_agent",
        "save_agent",
        "update_agent_state",
        "delete_agent",
        "log_event",
        "register_agent",
    ] {
        assert!(!helper.contains(write), "is_transcendent_agent: {write}");
    }
}

/// P0-FINAL-GATE (item G): an enabled Warden review with no Warden able to
/// run denies with the bounded reason. It used to allow the action as
/// "Warden inactive", and the only prebuilt Warden is L6, which is never
/// registered. A disabled review (the default) still allows.
#[test]
fn p0_fg_g_enabled_warden_review_without_a_warden_denies() {
    let cognitive = include_str!("../../commands/cognitive.rs");
    let (_, review) = fn_shape(cognitive, "review_with");
    assert!(
        review.starts_with(concat!(
            "if!enabled{returnOk(nexus_kernel::actuators::ActionReviewDecision::Allow{",
            "reason:\"Wardengovernancereviewdisabled\".to_string(),});}",
        )),
        "{review}"
    );
    let unavailable = concat!(
        "else{returnOk(nexus_kernel::actuators::ActionReviewDecision::Deny{",
        "reason:WARDEN_REVIEW_UNAVAILABLE.to_string(),});};",
    );
    let deny = position(&review, unavailable);
    assert!(deny < position(&review, "default_model"), "{review}");
    assert!(deny < position(&review, "query("), "{review}");
    let code = without_whitespace(&code_lines(cognitive));
    assert!(code.contains(concat!(
        "pub(crate)constWARDEN_REVIEW_UNAVAILABLE:&str=",
        "\"WardenreviewisunavailableinPhaseZero:noWardenagentcanrun\";",
    )));
    assert!(!code.contains("\"Wardeninactive\""));
}

/// Entries of the desktop command registration, whitespace-free.
fn registered_commands() -> Vec<String> {
    let lib = include_str!("../../lib.rs");
    let start = lib
        .find("generate_handler![")
        .expect("command registration")
        + "generate_handler![".len();
    let end = start + lib[start..].find(']').expect("end of registration");
    lib[start..end]
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .flat_map(|line| line.split(','))
        .map(without_whitespace)
        .filter(|entry| !entry.is_empty())
        .collect()
}

/// P0-FINAL-GATE (item G): three commands took a caller's boolean, or the
/// call itself, as a human approval:
/// - `nx_consent_respond(granted)` and `nx_agent_approve(approved)` answered
///   consent requests that only the closed nx agent loops produced;
/// - `self_rewrite_apply_patch` marked a patch approved because the frontend
///   had called it.
///
/// Each now takes no input and only returns the bounded `ApprovalRequired`
/// reason. Each stays registered exactly once, and no pending-consent channel
/// is left for a boolean to answer.
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[test]
fn p0_fg_g_caller_asserted_approval_commands_only_deny() {
    let approval = |command| Err(closed(command, Closure::ApprovalRequired));
    assert_eq!(
        crate::nx_bridge::commands::nx_consent_respond(),
        approval("nx_consent_respond")
    );
    assert_eq!(
        crate::nx_bridge::commands::nx_agent_approve(),
        approval("nx_agent_approve")
    );
    assert_eq!(
        crate::runtime::self_rewrite_apply_patch(),
        approval("self_rewrite_apply_patch")
    );

    let nx = include_str!("../../nx_bridge/commands.rs");
    let lib = include_str!("../../lib.rs");
    let registered = registered_commands();
    for (src, module, name) in [
        (nx, "nx_bridge::commands::", "nx_consent_respond"),
        (nx, "nx_bridge::commands::", "nx_agent_approve"),
        (lib, "", "self_rewrite_apply_patch"),
    ] {
        let (params, body) = fn_shape(src, name);
        assert_eq!(params, "()", "{name} must take no input");
        assert_eq!(
            body,
            format!(
                "Err(crate::phase0_surface::closed(\"{name}\",crate::phase0_surface::Closure::ApprovalRequired))"
            ),
            "{name} must only deny"
        );
        let entry = format!("{module}{name}");
        assert_eq!(
            registered.iter().filter(|e| **e == entry).count(),
            1,
            "{entry} must stay registered once"
        );
    }

    let bridge = code_lines(include_str!("../../nx_bridge/mod.rs")) + &code_lines(nx);
    for gone in ["pending_consents", "ConsentPending", "oneshot"] {
        assert!(!bridge.contains(gone), "nx bridge: {gone}");
    }
    let advanced = code_lines(include_str!("../../commands/advanced.rs"));
    for gone in ["fn self_rewrite_apply_patch(", "PatchStatus::Approved"] {
        assert!(!advanced.contains(gone), "advanced: {gone}");
    }
}

/// P0-FINAL-GATE (item G): a name the caller supplies is not an approver
/// identity. Neither the five consent-resolution commands nor the consent
/// functions behind them take an approved_by, denied_by or reviewed_by
/// name. Each function records the fixed `DESKTOP_UI_RESOLVER` label itself,
/// in the consent row and the audit event, so the label cannot be changed
/// through a wrapper. The approval is not forwarded to the kernel consent
/// runtime, whose queue records approvals by approver identity. (Before F6
/// the functions took the name and the commands passed the label; this
/// guard pinned the label in the commands.)
#[test]
fn p0_fg_g_consent_decisions_record_no_caller_identity() {
    let lib = include_str!("../../lib.rs");
    let consent_src = include_str!("../../commands/consent.rs");
    for (command, status, audit_field) in [
        ("approve_consent_request", "approved", "approved_by"),
        ("deny_consent_request", "denied", "denied_by"),
        ("batch_approve_consents", "approved", "approved_by"),
        ("review_consent_batch", "review_each", "reviewed_by"),
        ("batch_deny_consents", "denied", "denied_by"),
    ] {
        let (params, body) = fn_shape(lib, command);
        for name in [
            "approved_by",
            "denied_by",
            "reviewed_by",
            "_by:",
            "RESOLVER",
        ] {
            assert!(!params.contains(name), "{command} takes {name}: {params}");
            assert!(!body.contains(name), "{command} passes {name}: {body}");
        }
        assert!(
            body.contains(&format!("super::{command}(state.inner(),")),
            "{command}: {body}"
        );

        let (params, body) = fn_shape(consent_src, command);
        assert!(!params.contains("_by"), "{command} takes a name: {params}");
        assert_eq!(
            body.matches(".resolve_consent(").count(),
            1,
            "{command}: {body}"
        );
        assert!(
            body.contains(&format!("\"{status}\",DESKTOP_UI_RESOLVER)")),
            "{command} must resolve with the interface label: {body}"
        );
        assert!(
            body.contains(&format!("\"{audit_field}\":DESKTOP_UI_RESOLVER,")),
            "{command} must audit the interface label: {body}"
        );
    }

    let consent = code_lines(consent_src);
    assert!(without_whitespace(&consent)
        .contains("pubconstDESKTOP_UI_RESOLVER:&str=\"desktop-ui(unverified)\";"));
    for name in ["approved_by:", "denied_by:", "reviewed_by:"] {
        assert!(!consent.contains(name), "consent takes {name}");
    }
    assert!(
        !consent.contains(".approve_consent("),
        "an IPC approval must not reach the kernel consent runtime"
    );
}

/// P0-FINAL-GATE (item G): accepting a self-improvement proposal claims no
/// HITL approval. The pipeline neither sets `hitl_approved` nor builds a
/// validated proposal or a HITL signature. It checks every invariant except
/// #9, which it cannot satisfy. It records the proposal as `Proposed`, never
/// validated, applied or monitored, with no checkpoint, and audits it as
/// recorded, not applied.
#[test]
fn p0_fg_g_self_improvement_acceptance_is_recorded_truthfully() {
    let pipeline = include_str!("../../commands/self_improvement.rs");
    let (_, accept) = fn_shape(pipeline, "self_improve_approve_proposal");
    assert!(accept.contains("hitl_approved:false,"), "{accept}");
    assert!(accept.contains(".filter(|invariant|**invariant!=HardInvariant::HitlApprovalRequired)"));
    assert!(accept.contains("status:ImprovementStatus::Proposed,"));
    assert!(accept.contains("checkpoint_id:uuid::Uuid::nil(),"));
    assert!(accept.contains("\"type\":\"self_improvement_recorded\""));
    assert!(accept.contains("\"hitl_approved\":false"));
    assert!(accept.contains("\"applied\":false"));
    for claim in [
        "hitl_approved:true",
        "ValidatedProposal",
        "hitl_signature",
        "validate_all_invariants",
        "ImprovementStatus::Monitoring",
        "ImprovementStatus::Applied",
        "ImprovementStatus::Validated",
        "self_improvement_applied",
    ] {
        assert!(
            !accept.contains(claim),
            "self_improve_approve_proposal: {claim}"
        );
    }
    let pipeline = code_lines(pipeline);
    assert!(!pipeline.contains("awaiting HITL approval"));
    assert!(!pipeline.contains("hitl_approved: true"));
}

/// P0-FINAL-GATE (item G): the self-improvement report gets only the
/// history entries whose status says the change was applied, the number of
/// cycles actually run, and no fuel figure (none is metered). The recorded
/// acceptances (`Proposed`) are not counted as applied.
#[test]
fn p0_fg_g_self_improvement_report_counts_only_applied_changes() {
    let pipeline = include_str!("../../commands/self_improvement.rs");
    let (_, report) = fn_shape(pipeline, "self_improve_get_report");
    assert!(
        report.contains(concat!(
            "letapplied:Vec<AppliedImprovement>=si.history.iter()",
            ".filter(|improvement|was_applied(improvement.status)).cloned().collect();",
            "letreport=nexus_self_improve::report::ImprovementReport::generate(",
            "&applied,si.cycles_run,0,0,0,period_start,now);",
        )),
        "{report}"
    );
    let (params, applied) = fn_shape(pipeline, "was_applied");
    assert_eq!(params, "(status:ImprovementStatus)");
    assert_eq!(
        applied,
        concat!(
            "matches!(status,ImprovementStatus::Applied|ImprovementStatus::Monitoring",
            "|ImprovementStatus::Committed|ImprovementStatus::RolledBack)",
        )
    );
    let (_, cycle) = fn_shape(pipeline, "self_improve_run_cycle");
    assert!(cycle.contains("si.cycles_run=si.cycles_run.saturating_add(1);"));
    assert_eq!(pipeline.matches(".cycles_run").count(), 3);
}

/// Why the computer-use loop controller source would let end of input or a
/// read error approve a step, or `None` if it would not.
fn eof_approval_defect(controller: &str) -> Option<String> {
    let controller = controller.replace("\r\n", "\n");
    let (params, body) = fn_shape(&controller, "read_approval_decision");
    if params != "(input:&mutimplBufRead)" {
        return Some(format!("decision reader parameters: {params}"));
    }
    let first_line = concat!(
        "letmutline=String::new();matchinput.read_line(&mutline){",
        "Ok(0)=>{warn!(\"stdinclosedbeforeanapprovaldecision,aborting\");returnApprovalDecision::Abort;}",
        "Ok(_)=>{}",
        "Err(_)=>{warn!(\"Failedtoreadstdin,aborting\");returnApprovalDecision::Abort;}}",
    );
    if !body.starts_with(first_line) {
        return Some(format!("EOF or a read error does not abort: {body}"));
    }
    if !body.contains("matchinput.read_line(&mutmod_line){Ok(0)|Err(_)=>ApprovalDecision::Abort,") {
        return Some("EOF before a modify line does not abort".into());
    }
    if body.matches("ApprovalDecision::Approve").count() != 1
        || !body.contains("\"\"|\"y\"|\"yes\"=>ApprovalDecision::Approve,")
    {
        return Some("approval is reachable other than by an entered line".into());
    }
    let (_, prompt) = fn_shape(&controller, "prompt_user_approval");
    if !prompt.ends_with("read_approval_decision(&mutio::stdin().lock())") {
        return Some("the step prompt does not decide through the reader".into());
    }
    // Standard input is read only inside the decision function.
    if code_lines(&controller).matches("read_line(").count() != 2 {
        return Some("standard input is read outside the decision function".into());
    }
    for test in [
        "test_approval_eof_aborts_instead_of_approving",
        "test_approval_read_error_aborts",
        "test_approval_entered_blank_line_is_explicit_approval",
        "test_approval_modify_requires_an_entered_replacement",
    ] {
        if !controller.contains(&format!("#[test]\n    fn {test}()")) {
            return Some(format!("{test} is missing"));
        }
    }
    None
}

/// P0-FINAL-GATE (item G): preserved from C5A (E6). The computer-use step
/// approval reads a line from standard input. End of input (`read_line`
/// returning `Ok(0)`: a closed, null or absent stdin) and a read error abort;
/// they are never taken for pressing Enter, which approves. EOF before a
/// modify line aborts as well. That module's own tests of these cases must
/// stay present. The in-memory negative controls show that the check rejects
/// each regression.
#[test]
fn p0_fg_g_eof_or_a_read_error_is_never_an_approval() {
    let controller =
        include_str!("../../../../../crates/nexus-computer-use/src/agent/loop_controller.rs");
    assert_eq!(eof_approval_defect(controller), None);

    for (original, regression) in [
        (
            "            warn!(\"stdin closed before an approval decision, aborting\");\n            return ApprovalDecision::Abort;",
            "            warn!(\"stdin closed before an approval decision, aborting\");\n            return ApprovalDecision::Approve;",
        ),
        (
            "            warn!(\"Failed to read stdin, aborting\");\n            return ApprovalDecision::Abort;",
            "            warn!(\"Failed to read stdin, aborting\");\n            return ApprovalDecision::Approve;",
        ),
        (
            "                Ok(0) | Err(_) => ApprovalDecision::Abort,",
            "                Ok(0) | Err(_) => ApprovalDecision::Modify(String::new()),",
        ),
        (
            "fn test_approval_eof_aborts_instead_of_approving()",
            "fn test_approval_eof_removed()",
        ),
    ] {
        let source = controller.replace("\r\n", "\n");
        assert_eq!(source.matches(original).count(), 1, "{original}");
        let mutated = source.replace(original, regression);
        assert!(
            eof_approval_defect(&mutated).is_some(),
            "the check must reject: {regression}"
        );
    }
}

/// Every source of the kernel actuators, read at run time from their
/// directory. (P0-FINAL-GATE F7: this used to be a compile-time list of the
/// 20 files the directory held, which a file added later would have
/// escaped.)
fn actuator_sources() -> Vec<(String, String)> {
    sources_including("kernel/src/actuators", &["mod.rs", "shell.rs", "types.rs"])
}

/// Each line of the actuator sources that reads the HITL approval flag.
fn hitl_flag_reads(sources: &[(String, String)]) -> Vec<(String, String)> {
    sources
        .iter()
        .flat_map(|(file, src)| {
            src.lines()
                .filter(|line| line.contains(".hitl_approved"))
                .map(move |line| (file.clone(), without_whitespace(line)))
        })
        .collect()
}

/// P0-FINAL-GATE (item G): a HITL approval is not authority for an actuator.
/// The cognitive loop passes whether a step was HITL-approved into the
/// actuator context. No actuator reads that flag; only the context's `Debug`
/// output shows it. So an approval delivered over IPC changes no actuator's
/// behaviour: `Phase0AgentExecutor` alone decides which actions run.
#[test]
fn p0_fg_g_no_actuator_reads_the_hitl_approval_flag() {
    let actuators = actuator_sources();
    let debug_only = vec![(
        "types.rs".to_string(),
        ".field(\"hitl_approved\",&self.hitl_approved)".to_string(),
    )];
    assert_eq!(hitl_flag_reads(&actuators), debug_only);

    // Negative control, in memory: an actuator that branched on the flag
    // would be caught.
    let branch = "\nfn approved(context: &ActuatorContext) -> bool { context.hitl_approved }\n";
    let mut branching = actuators.clone();
    let (_, shell) = branching
        .iter_mut()
        .find(|(file, _)| file == "shell.rs")
        .unwrap();
    shell.push_str(branch);
    assert_ne!(hitl_flag_reads(&branching), debug_only);
}

/// P0-FINAL-GATE F7: the C5 and actuator guards read their sources from the
/// directories at run time. A file added later, at any depth, is read and
/// checked, and only Rust sources are read. Shown on a scratch directory.
#[test]
fn p0_fg_source_lists_follow_their_directories() {
    let dir = std::env::temp_dir().join(format!(
        "p0-fg-source-list-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(dir.join("added/deeper")).unwrap();
    std::fs::write(dir.join("first.rs"), "// first\n").unwrap();
    std::fs::write(dir.join("added/deeper/later.rs"), "context.hitl_approved\n").unwrap();
    std::fs::write(dir.join("notes.txt"), "context.hitl_approved\n").unwrap();
    let sources = rust_sources_under(&dir);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(
        sources,
        [
            (
                "added/deeper/later.rs".to_string(),
                "context.hitl_approved\n".to_string()
            ),
            ("first.rs".to_string(), "// first\n".to_string()),
        ]
    );
    assert_eq!(
        hitl_flag_reads(&sources),
        [(
            "added/deeper/later.rs".to_string(),
            "context.hitl_approved".to_string()
        )]
    );
}

/// P0-FINAL-GATE (item K, cross-stream request from stream 6): both
/// resource-heavy consent-module commands check their caller-chosen amount
/// first. A parallel simulation refuses a variant count outside 1..=10 before
/// the model is built or the seed parsed. An adversarial session refuses a
/// round count outside 1..=50 before the arena is built. (The kernel holds the
/// authoritative bounds; these are the desktop's early checks.)
#[test]
fn p0_fg_k_simulation_and_arena_bounds_precede_any_work() {
    let consent = include_str!("../../commands/consent.rs");
    let constants = without_whitespace(&code_lines(consent));
    assert!(constants.contains("constMAX_PARALLEL_SIMULATION_VARIANTS:u32=10;"));
    assert!(constants.contains("constMAX_ADVERSARIAL_ROUNDS:u32=50;"));

    let (_, reports) = fn_shape(consent, "run_parallel_simulation_reports");
    assert_eq!(
        reports,
        "run_parallel_simulation_reports_with(state,seed_text,variant_count,build_simulation_llm)"
    );
    let (_, bounded) = fn_shape(consent, "run_parallel_simulation_reports_with");
    assert!(
        bounded.starts_with(concat!(
            "if!(1..=MAX_PARALLEL_SIMULATION_VARIANTS).contains(&variant_count){",
            "returnErr(format!(\"variant_countmustbebetween1and{MAX_PARALLEL_SIMULATION_VARIANTS}\"));}",
            "letllm=simulation_llm();letseed=parse_seed(",
        )),
        "{bounded}"
    );

    let (_, arena) = fn_shape(consent, "run_adversarial_session");
    assert!(
        arena.starts_with(concat!(
            "if!(1..=MAX_ADVERSARIAL_ROUNDS).contains(&rounds){",
            "returnErr(format!(\"roundsmustbebetween1and{MAX_ADVERSARIAL_ROUNDS}\"));}",
            "letmutarena=nexus_kernel::immune::AdversarialArena::new();",
        )),
        "{arena}"
    );
}
