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

/// Every production source of the capability-measurement crate, read at
/// compile time.
macro_rules! measurement_sources {
    ($($file:literal),* $(,)?) => {
        [$((
            $file,
            include_str!(concat!(
                "../../../../../crates/nexus-capability-measurement/src/",
                $file
            )),
        )),*]
    };
}

const MEASUREMENT_SOURCES: [(&str, &str); 32] = measurement_sources!(
    "battery/difficulty.rs",
    "battery/expected_chain.rs",
    "battery/mod.rs",
    "battery/test_problem.rs",
    "darwin_bridge.rs",
    "evaluation/ab_validation.rs",
    "evaluation/agent_adapter.rs",
    "evaluation/batch.rs",
    "evaluation/comparator.rs",
    "evaluation/mod.rs",
    "evaluation/nim_client.rs",
    "evaluation/openrouter_client.rs",
    "evaluation/repeatability.rs",
    "evaluation/runner.rs",
    "evaluation/three_way.rs",
    "evaluation/validation_run.rs",
    "framework.rs",
    "lib.rs",
    "reporting/audit_trail.rs",
    "reporting/cross_vector.rs",
    "reporting/mod.rs",
    "reporting/scorecard.rs",
    "scoring/articulation.rs",
    "scoring/asymmetric.rs",
    "scoring/gaming_detection.rs",
    "scoring/mod.rs",
    "tauri_commands.rs",
    "vectors/adaptation.rs",
    "vectors/mod.rs",
    "vectors/planning_coherence.rs",
    "vectors/reasoning_depth.rs",
    "vectors/tool_use_integrity.rs",
);

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
    let mut env_reads = Vec::new();
    for (file, src) in MEASUREMENT_SOURCES {
        let code = measurement_production_code(file, src);
        for other_provider in ["NVIDIA_NIM_API_KEY", "OPENROUTER_API_KEY"] {
            assert!(!code.contains(other_provider), "{file}: {other_provider}");
        }
        assert!(!code.contains("var_os("), "{file}: var_os");
        let reads = code.matches("env::var").count();
        if reads > 0 {
            env_reads.push((file, reads));
        }
    }
    assert_eq!(
        env_reads,
        [("evaluation/nim_client.rs", 1)],
        "the crate reads one environment variable, through the Groq key lookup"
    );

    let client = MEASUREMENT_SOURCES
        .iter()
        .find(|(file, _)| *file == "evaluation/nim_client.rs")
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
/// the backend cannot verify, so L6 is unavailable. Each route that created,
/// started or registered one refuses first, with the bounded
/// `ApprovalRequired` reason, before any state changes:
/// - `create_agent` refuses level 6 right after parsing, before anything is
///   written;
/// - `start_agent` refuses before the agent is restarted, stored or audited;
/// - restore registers no level-6 record and writes nothing to it;
/// - `approve_consent_request` and `batch_approve_consents` refuse a
///   transcendent request before resolving anything, and nothing in the
///   consent module creates or starts an agent;
/// - nothing enqueues a transcendent request any more.
#[test]
fn p0_fg_g_transcendent_agents_are_refused_before_any_state_change() {
    let agents = include_str!("../../commands/agents.rs");
    let consent = include_str!("../../commands/consent.rs");

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
/// identity. The five consent-resolution commands take no approved_by,
/// denied_by or reviewed_by name. Each passes the fixed `DESKTOP_UI_RESOLVER`
/// label, which the consent row and the audit event record. The approval is
/// not forwarded to the kernel consent runtime, whose queue records approvals
/// by approver identity.
#[test]
fn p0_fg_g_consent_decisions_record_no_caller_identity() {
    let lib = include_str!("../../lib.rs");
    for command in [
        "approve_consent_request",
        "deny_consent_request",
        "batch_approve_consents",
        "review_consent_batch",
        "batch_deny_consents",
    ] {
        let (params, body) = fn_shape(lib, command);
        for name in ["approved_by", "denied_by", "reviewed_by", "_by:"] {
            assert!(!params.contains(name), "{command} takes {name}: {params}");
        }
        assert!(
            body.contains(&format!("super::{command}(")),
            "{command}: {body}"
        );
        assert_eq!(
            body.matches("super::DESKTOP_UI_RESOLVER.to_string()")
                .count(),
            1,
            "{command} must record the interface label: {body}"
        );
    }

    let consent = code_lines(include_str!("../../commands/consent.rs"));
    assert!(without_whitespace(&consent)
        .contains("pubconstDESKTOP_UI_RESOLVER:&str=\"desktop-ui(unverified)\";"));
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

/// Every source of the kernel actuators, read at compile time.
macro_rules! actuator_sources {
    ($($file:literal),* $(,)?) => {
        [$((
            $file,
            include_str!(concat!("../../../../../kernel/src/actuators/", $file)),
        )),*]
    };
}

/// Each line of the actuator sources that reads the HITL approval flag.
fn hitl_flag_reads<'a>(sources: &[(&'a str, &str)]) -> Vec<(&'a str, String)> {
    sources
        .iter()
        .flat_map(|(file, src)| {
            src.lines()
                .filter(|line| line.contains(".hitl_approved"))
                .map(move |line| (*file, without_whitespace(line)))
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
    let actuators = actuator_sources!(
        "agent_lifecycle.rs",
        "api.rs",
        "browser.rs",
        "code_exec.rs",
        "cognitive_param.rs",
        "computer_use.rs",
        "docker.rs",
        "execution_platform.rs",
        "filesystem.rs",
        "governance_policy.rs",
        "image_gen.rs",
        "input.rs",
        "knowledge_graph.rs",
        "mod.rs",
        "screen.rs",
        "self_evolution.rs",
        "shell.rs",
        "tts.rs",
        "types.rs",
        "web.rs",
    );
    let debug_only = vec![(
        "types.rs",
        ".field(\"hitl_approved\",&self.hitl_approved)".to_string(),
    )];
    assert_eq!(hitl_flag_reads(&actuators), debug_only);

    // Negative control, in memory: an actuator that branched on the flag
    // would be caught.
    let mut branching = actuators.to_vec();
    let shell = format!(
        "{}\nfn approved(context: &ActuatorContext) -> bool {{ context.hitl_approved }}\n",
        actuators[16].1
    );
    assert_eq!(actuators[16].0, "shell.rs");
    branching[16].1 = &shell;
    assert_ne!(hitl_flag_reads(&branching), debug_only);
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
