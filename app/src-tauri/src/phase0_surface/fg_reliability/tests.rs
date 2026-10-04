//! P0-FINAL-GATE-CLOSURE guards for item K and the resource bounds.
//!
//! Each dossier "Resource bounds" surface is reachable from any script in the
//! webview. These guards drive the desktop entry points with out-of-range
//! requests and check that they are refused before any work, and they pin
//! the approved limits so a bound cannot be loosened silently.

use crate::AppState;
use nexus_kernel::cognitive::ScheduledGoalExecutor as _;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn agent(state: &AppState, name: &str) -> String {
    let manifest = nexus_kernel::manifest::parse_manifest(&format!(
        r#"
name = "{name}"
version = "1.0.0"
capabilities = ["llm.query"]
fuel_budget = 10000
autonomy_level = 1
"#
    ))
    .unwrap();
    state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(manifest)
        .unwrap()
        .to_string()
}

fn audited_actions(state: &AppState) -> Vec<(String, String)> {
    state
        .audit
        .lock()
        .unwrap()
        .events()
        .iter()
        .map(|event| {
            (
                event.payload["action"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                event.payload["reason"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            )
        })
        .collect()
}

#[test]
fn p0_fg_k_approved_limits_are_pinned() {
    assert_eq!(crate::MAX_STRESS_PERSONAS, 1_000);
    assert_eq!(
        nexus_kernel::simulation::runtime::MAX_PARALLEL_SIMULATION_VARIANTS,
        10
    );
    assert_eq!(nexus_kernel::temporal::dilation::MAX_DILATED_ITERATIONS, 50);
    assert_eq!(crate::MAX_FRONTEND_ERROR_FIELD_BYTES, 8 * 1024);
    assert_eq!(crate::MAX_FRONTEND_ERROR_LOG_BYTES, 4 * 1024 * 1024);
    assert_eq!(web_builder_agent::budget::MAX_BUILD_HISTORY, 1_000);
    assert_eq!(nexus_kernel::immune::arena::MAX_ARENA_ROUNDS, 50);
    assert_eq!(nexus_kernel::temporal::types::MAX_TEMPORAL_FORKS, 10);
    assert_eq!(
        nexus_kernel::temporal::types::MAX_FORK_BUDGET_TOKENS,
        200_000
    );
}

#[test]
fn p0_fg_k_adversarial_rounds_are_refused_outside_their_bound() {
    // Through the IPC command's own function (consent.rs), which checks the
    // kernel's bound first (`check_rounds`) and then runs `try_run_session`:
    // the refusal is the kernel's text and the command's error.
    for rounds in [0, 51, 10_000] {
        let error = crate::run_adversarial_session("attacker".into(), "defender".into(), rounds)
            .expect_err("an out-of-range round count must be refused");
        assert_eq!(
            error,
            format!("arena rounds must be between 1 and 50, got {rounds}")
        );
    }
    for rounds in [1, 50] {
        let session =
            crate::run_adversarial_session("attacker".into(), "defender".into(), rounds).unwrap();
        assert_eq!(session["rounds"], rounds);
        assert_eq!(
            session["results"].as_array().map(Vec::len),
            Some(rounds as usize)
        );
    }
}

#[test]
fn p0_fg_k_temporal_fork_limits_are_refused_and_never_stored() {
    let state = AppState::new_in_memory();
    let stored = |state: &AppState| {
        let engine = state.temporal_engine.lock().unwrap();
        (
            engine.config().max_parallel_forks,
            engine.config().fork_budget_tokens,
        )
    };
    let before = stored(&state);
    for (forks, tokens) in [
        (0, 50_000),
        (11, 50_000),
        (u32::MAX, 50_000),
        (5, 0),
        (5, 200_001),
        (5, u64::MAX),
    ] {
        assert!(
            crate::set_temporal_config(&state, forks, "BestFinalScore".into(), tokens).is_err(),
            "{forks} forks, {tokens} tokens"
        );
        assert_eq!(stored(&state), before, "a refused config was stored");
    }
    // A fork-count override is refused before the configuration is read, a
    // provider is built or the model is called, and is never stored.
    for forks in [0, 11, u32::MAX] {
        let error = crate::temporal_fork(&state, "request".into(), "agent".into(), Some(forks))
            .expect_err("an out-of-range fork count must be refused");
        assert_eq!(
            error,
            format!("invalid fork count: {forks} (allowed: 1 to 10)")
        );
        assert_eq!(stored(&state), before);
    }
    crate::set_temporal_config(&state, 10, "BestFinalScore".into(), 200_000).unwrap();
    assert_eq!(stored(&state), (10, 200_000));
    crate::set_temporal_config(&state, 1, "LowestRisk".into(), 1).unwrap();
    assert_eq!(stored(&state), (1, 1));
}

#[test]
fn p0_fg_k_an_older_loops_exit_keeps_a_newer_loops_cancellation_entry() {
    use crate::commands::cognitive::CognitiveCancelGuard;
    let state = AppState::new_in_memory();
    let id = agent(&state, "fg-k-cancel");
    let entry = |state: &AppState| {
        state
            .cognitive_cancellations
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
    };
    let executor = crate::commands::cognitive::ScheduledGoalExecutor {
        state: state.clone(),
    };

    let (older_flag, older) = CognitiveCancelGuard::register(&state, &id);
    let (newer_flag, newer) = CognitiveCancelGuard::register(&state, &id);
    assert!(Arc::ptr_eq(&entry(&state).unwrap(), &newer_flag));

    // The older loop ends first: the newer loop's entry must stay, so Stop
    // and the scheduler still see the loop that is running.
    drop(older);
    let current = entry(&state).expect("the newer loop's entry was erased");
    assert!(Arc::ptr_eq(&current, &newer_flag));
    assert!(!Arc::ptr_eq(&current, &older_flag));
    assert!(executor.execute(&id, "scheduled goal").is_err());

    // The newer loop ends: its own entry goes, and ticks run again.
    drop(newer);
    assert!(entry(&state).is_none());
    executor.execute(&id, "scheduled goal").unwrap();

    // Reverse order: the newer loop ends first, then the older one; the
    // older guard finds no entry of its own and removes nothing.
    let other = agent(&state, "fg-k-cancel-reverse");
    let (_, older) = CognitiveCancelGuard::register(&state, &other);
    let (_, newer) = CognitiveCancelGuard::register(&state, &other);
    drop(newer);
    let (third_flag, third) = CognitiveCancelGuard::register(&state, &other);
    drop(older);
    let current = state
        .cognitive_cancellations
        .lock()
        .unwrap()
        .get(&other)
        .cloned()
        .expect("a stale guard erased a later loop's entry");
    assert!(Arc::ptr_eq(&current, &third_flag));
    drop(third);
    assert!(state
        .cognitive_cancellations
        .lock()
        .unwrap()
        .get(&other)
        .is_none());
}

#[test]
fn p0_fg_k_stress_persona_count_is_refused_outside_its_bound() {
    let state = AppState::new_in_memory();
    for count in [0, crate::MAX_STRESS_PERSONAS + 1, u32::MAX] {
        let error = crate::stress_generate_personas(&state, count)
            .expect_err("an out-of-range persona count must be refused");
        assert!(error.contains("between 1 and 1000"), "{error}");
    }
    for count in [1, crate::MAX_STRESS_PERSONAS] {
        let json = crate::stress_generate_personas(&state, count).unwrap();
        let personas: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(personas.len(), count as usize);
    }
}

#[test]
fn p0_fg_k_parallel_simulation_count_is_refused_outside_its_bound() {
    let state = AppState::new_in_memory();
    for count in [0, 11, u32::MAX] {
        let error = crate::run_parallel_simulation_reports(
            &state,
            "A contested climate bill enters parliament".into(),
            count,
        )
        .expect_err("an out-of-range variant count must be refused");
        assert!(error.contains("between 1 and 10"), "{count}: {error}");
    }
}

#[test]
fn p0_fg_k_dilated_session_iterations_are_refused_outside_their_bound() {
    let state = AppState::new_in_memory();
    for count in [0, 51, u32::MAX] {
        let error = crate::run_dilated_session(&state, "task".into(), Vec::new(), count)
            .expect_err("an out-of-range iteration count must be refused");
        assert_eq!(
            error,
            format!("invalid iteration count: {count} (allowed: 1 to 50)")
        );
    }
}

#[test]
fn p0_fg_k_agent_schedules_fire_at_most_once_per_minute() {
    let state = AppState::new_in_memory();
    let id = agent(&state, "fg-k-cron");
    for expression in ["* * * * * *", "*/5 * * * * *", "0,30 * * * * *"] {
        assert!(nexus_kernel::cognitive::AgentScheduler::validate_cron(expression).is_err());
        // No Tokio runtime exists in this test: registration must refuse
        // before it would spawn the schedule loop.
        assert!(state
            .agent_scheduler
            .register_agent(&id, expression, "goal")
            .is_err());
        // The path create_agent, start_agent and the startup restore use.
        crate::register_manifest_schedule(&state, &id, Some(expression), Some("goal"), None);
    }
    assert!(state.agent_scheduler.list().is_empty());
    assert!(nexus_kernel::cognitive::AgentScheduler::validate_cron("0 * * * * *").is_ok());
    assert!(nexus_kernel::cognitive::AgentScheduler::validate_cron("*/5 * * * *").is_ok());
}

#[test]
fn p0_fg_k_a_scheduled_tick_never_overlaps_the_agents_running_loop() {
    let state = AppState::new_in_memory();
    let id = agent(&state, "fg-k-scheduled");
    let executor = crate::commands::cognitive::ScheduledGoalExecutor {
        state: state.clone(),
    };

    // A running desktop loop holds its cancellation entry, under any
    // spelling of the agent's id.
    for key in [id.clone(), id.to_ascii_uppercase()] {
        state
            .cognitive_cancellations
            .lock()
            .unwrap()
            .insert(key.clone(), Arc::new(AtomicBool::new(false)));
        let error = executor
            .execute(&id, "scheduled goal")
            .expect_err("a tick must be skipped while the loop runs");
        assert!(error.contains("still running"), "{error}");
        assert!(
            !state.cognitive_runtime.has_active_loop(&id),
            "a skipped tick assigned a goal"
        );
        state.cognitive_cancellations.lock().unwrap().remove(&key);
    }
    let actions = audited_actions(&state);
    assert_eq!(
        actions
            .iter()
            .filter(|(action, reason)| action == "scheduled_execution_skipped"
                && reason == "agent_loop_active")
            .count(),
        2
    );
    assert!(!actions
        .iter()
        .any(|(action, _)| action == "scheduled_execution_triggered"));

    // With no loop running the tick proceeds as before.
    executor.execute(&id, "scheduled goal").unwrap();
    assert!(state.cognitive_runtime.has_active_loop(&id));
    assert!(audited_actions(&state)
        .iter()
        .any(|(action, _)| action == "scheduled_execution_triggered"));
}

#[test]
fn p0_fg_k_the_frontend_error_command_uses_the_bounded_log() {
    let source = include_str!("../../lib.rs").replace("\r\n", "\n");
    let start = source
        .find("fn log_frontend_error(")
        .expect("log_frontend_error is registered");
    let body = &source[start..start + source[start..].find("\n    }\n").unwrap()];
    assert!(
        body.contains("super::record_frontend_error(&message, &stack, &component_stack);"),
        "{body}"
    );
    for unbounded in ["OpenOptions", "writeln!", "eprintln!", "std::fs::"] {
        assert!(!body.contains(unbounded), "{unbounded} in {body}");
    }
}

#[test]
fn p0_fg_k_build_records_outside_the_bounds_are_refused() {
    use web_builder_agent::budget::BuildRecord;
    let valid = BuildRecord {
        project_name: "site".into(),
        model_name: "model".into(),
        provider: "anthropic".into(),
        input_tokens: 1,
        output_tokens: 1,
        cost_usd: 0.01,
        elapsed_seconds: 1.0,
        lines_generated: 1,
        checkpoint_id: String::new(),
        timestamp: "2026-09-28T00:00:00Z".into(),
    };
    valid.validate().unwrap();
    for invalid in [
        BuildRecord {
            cost_usd: 1.7e308,
            ..valid.clone()
        },
        BuildRecord {
            cost_usd: f64::NAN,
            ..valid.clone()
        },
        BuildRecord {
            project_name: "x".repeat(257),
            ..valid.clone()
        },
        BuildRecord {
            provider: "not a provider".into(),
            ..valid.clone()
        },
    ] {
        assert!(invalid.validate().is_err());
    }
}

/// Test isolation: the cognitive loop of an in-memory `AppState` records its
/// L6 cooldowns and algorithm selections in the in-memory database. The
/// production-executor test, run in a child process whose identity home is a
/// scratch directory, leaves that home without a nexus.db. (The loop used to
/// open the identity home's nexus.db: on a developer machine, the real one.)
#[test]
fn p0_fg_k_an_in_memory_state_loop_writes_no_identity_home_database() {
    let home = std::env::temp_dir().join(format!("p0-fg-state-db-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(home.join(".nexus")).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "phase0_surface::tests::p0_002c5c_a2a_and_agent_actions_are_decided_by_the_production_executor",
            "--nocapture",
        ])
        .env("HOME", &home)
        .env_remove("NEXUS_DB_PATH")
        .output()
        .unwrap();
    let written = home.join(".nexus").join("nexus.db").exists();
    std::fs::remove_dir_all(&home).unwrap();
    assert!(
        output.status.success(),
        "child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "the child ran no test"
    );
    assert!(!written, "the identity home's nexus.db was written");
}

/// Rust source with comments removed (line, nested block, doc) and every
/// literal kept verbatim, so text inside a literal is still scanned
/// (fail-closed: a forbidden spelling inside a string is a false positive,
/// never a miss) while a comment cannot hide or fake a use. The lexer knows
/// each literal form that can hold an unescaped quote or a comment marker,
/// so no literal can shift it out of step with the real code that follows
/// (P0-LINUX-FINAL-R2C): raw strings `r"…"`, `r#"…"#` with any number of
/// hashes, and their `br` and `cr` forms (no escapes; they end only at a
/// quote followed by the same number of hashes); strings `"…"`, `b"…"` and
/// `c"…"` with escapes; and character literals `'"'`, `b'"'` and escaped
/// ones such as `'\''`, told apart from lifetimes. It is a lexer for this
/// guard, not a Rust parser; an unterminated literal runs to the end.
fn without_comments(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < b.len() {
        let rest = &src[i..];
        if rest.starts_with("//") {
            i += rest.find('\n').unwrap_or(rest.len());
        } else if rest.starts_with("/*") {
            let mut depth = 0usize;
            while i < b.len() {
                if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            out.push(' ');
        } else if let Some(len) = raw_literal_len(rest).filter(|_| i == 0 || !ident_byte(b[i - 1]))
        {
            out.push_str(&rest[..len]);
            i += len;
        } else if b[i] == b'"' {
            let len = quoted_literal_len(rest);
            out.push_str(&rest[..len]);
            i += len;
        } else if let Some(len) = char_literal_len(rest) {
            out.push_str(&rest[..len]);
            i += len;
        } else {
            let c = rest.chars().next().unwrap();
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

/// Length of the raw string literal `rest` starts with (`r`, `br` or `cr`,
/// any number of `#`, then `"`), through its closing quote and the same
/// number of hashes. `None` when `rest` is not one, e.g. a raw identifier
/// such as `r#component`.
fn raw_literal_len(rest: &str) -> Option<usize> {
    let b = rest.as_bytes();
    let mut at = usize::from(matches!(b.first(), Some(b'b' | b'c')));
    if b.get(at) != Some(&b'r') {
        return None;
    }
    at += 1;
    let hashes = b[at..].iter().take_while(|&&c| c == b'#').count();
    at += hashes;
    if b.get(at) != Some(&b'"') {
        return None;
    }
    at += 1;
    let close = format!("\"{}", "#".repeat(hashes));
    Some(
        rest[at..]
            .find(&close)
            .map_or(rest.len(), |end| at + end + close.len()),
    )
}

/// Length of the escaped string literal `rest` starts with (its opening
/// `"`; a `b` or `c` prefix is ordinary text before it), through the
/// closing quote.
fn quoted_literal_len(rest: &str) -> usize {
    let b = rest.as_bytes();
    let mut at = 1;
    while at < b.len() {
        match b[at] {
            b'\\' => at += 2,
            b'"' => return at + 1,
            _ => at += 1,
        }
    }
    b.len()
}

/// Length of the character literal `rest` starts with (`'x'`, `'"'`, or an
/// escape such as `'\''` or `'\u{22}'`; a `b` prefix is ordinary text
/// before it). `None` for a lifetime or label such as `'a`.
fn char_literal_len(rest: &str) -> Option<usize> {
    let b = rest.as_bytes();
    if b.first() != Some(&b'\'') {
        return None;
    }
    if b.get(1) == Some(&b'\\') {
        let end = b.iter().skip(3).position(|&c| c == b'\'' || c == b'\n')? + 3;
        return (b[end] == b'\'').then_some(end + 1);
    }
    let c = rest[1..].chars().next()?;
    let after = 1 + c.len_utf8();
    (c != '\'' && c != '\n' && rest[after..].starts_with('\'')).then_some(after + 1)
}

fn ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Whether `rest`, the text after a `wasmtime` root, starts with the path
/// segment `::component` or its raw-identifier spelling `::r#component`
/// (which names the same module), ending at an identifier boundary
/// (P0-LINUX-FINAL-R2B). Only this segment is matched; `r#` is not stripped
/// elsewhere, so raw strings are unaffected.
fn starts_with_component_segment(rest: &str) -> bool {
    ["::component", "::r#component"].iter().any(|segment| {
        rest.strip_prefix(segment)
            .is_some_and(|after| !after.bytes().next().is_some_and(ident_byte))
    })
}

/// Byte offsets where `word` occurs in `text` as a whole identifier.
fn word_at(text: &str, word: &str) -> Vec<usize> {
    let b = text.as_bytes();
    text.match_indices(word)
        .map(|(at, _)| at)
        .filter(|&at| at == 0 || !ident_byte(b[at - 1]))
        .filter(|&at| b.get(at + word.len()).is_none_or(|&c| !ident_byte(c)))
        .collect()
}

/// Comment-free `src` with whitespace dropped except a single space between
/// two identifier characters, so `wasmtime :: component`, multiline groups
/// and `use wasmtime\n    as w` compare in one spelling while `use wasmtime`
/// keeps its boundary.
fn normalized_rust(src: &str) -> String {
    let plain = without_comments(src);
    let mut code = String::with_capacity(plain.len());
    let mut chars = plain.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            while chars.peek().is_some_and(|n| n.is_whitespace()) {
                chars.next();
            }
            let before = code
                .chars()
                .last()
                .is_some_and(|p| p.is_ascii_alphanumeric() || p == '_');
            let after = chars
                .peek()
                .is_some_and(|n| n.is_ascii_alphanumeric() || *n == '_');
            if before && after {
                code.push(' ');
            }
        } else {
            code.push(c);
        }
    }
    code
}

/// The Wasmtime spellings production Nexus may not use (its Wasmtime use
/// stays core Wasm only), each tagged with its rule:
///
/// - `alias`: the crate renamed or re-exported under another name
///   (`[pub] use wasmtime as w`, `[pub] use wasmtime::{self as w}`,
///   `[pub] extern crate wasmtime as w`, raw identifiers included). The
///   declaration itself is refused, so no other file can reach the component
///   module through a name that does not spell `wasmtime`
///   (P0-LINUX-FINAL-R2A).
/// - `glob`: a glob import of the crate (`wasmtime::*`, `wasmtime::{*}`),
///   which would bring `component` into scope under its bare name.
/// - `component`: any path to Wasmtime's component module, where the
///   affected dynamic `Val`/`Func` API lives, whether direct, spaced,
///   multiline, grouped or nested (`use wasmtime::{Engine, component::{Val}}`)
///   or renamed inside a group (`wasmtime::{component as c}`).
///
/// Direct core-Wasm use (`wasmtime::{Engine, Linker, Module, Store}`,
/// `get_typed_func`) stays allowed. This is a text check over comment-free
/// source, not a name resolver: it pins Nexus's non-use of the component
/// module, not that the pinned Wasmtime is generally safe.
fn wasmtime_forbidden_uses(src: &str) -> Vec<String> {
    let code = normalized_rust(src);
    let mut found = Vec::new();
    for at in word_at(&code, "wasmtime") {
        let rest = &code[at + "wasmtime".len()..];
        if let Some(alias) = rest.strip_prefix(" as ") {
            let alias: String = alias
                .chars()
                .take_while(|&c| c.is_ascii_alphanumeric() || c == '_' || c == '#')
                .collect();
            found.push(format!("alias: wasmtime as {alias}"));
        }
        if rest.starts_with("::self as ") {
            found.push("alias: wasmtime::self as".to_string());
        }
        if starts_with_component_segment(rest) {
            found.push("component: wasmtime::component".to_string());
        }
        if rest.starts_with("::*") {
            found.push("glob: wasmtime::*".to_string());
        }
        if let Some(group) = rest.strip_prefix("::{") {
            let mut depth = 1usize;
            let end = group
                .char_indices()
                .find(|&(_, c)| {
                    match c {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                    depth == 0
                })
                .map_or(group.len(), |(end, _)| end);
            let group = &group[..end];
            if word_at(group, "self")
                .iter()
                .any(|&at| group[at + 4..].starts_with(" as "))
            {
                found.push("alias: wasmtime::{self as …}".to_string());
            }
            if group.contains('*') {
                found.push("glob: wasmtime::{*}".to_string());
            }
            if !word_at(group, "component").is_empty() {
                found.push("component: wasmtime::{…component…}".to_string());
            }
        }
    }
    found
}

/// Wasmtime's async and component-model entry points (P2 security closure
/// R1: RUSTSEC-2026-0327 is in component async-lifted callbacks) named in a
/// source that uses Wasmtime. A text check over comment-free source, like
/// `wasmtime_forbidden_uses`; the compiled API surface is the dependency
/// evidence's.
fn wasmtime_async_uses(src: &str) -> Vec<String> {
    let code = normalized_rust(src);
    if word_at(&code, "wasmtime").is_empty() {
        return Vec::new();
    }
    [
        "call_async",
        "instantiate_async",
        "func_wrap_async",
        "func_new_async",
        "async_support",
        "wasm_component_model",
        "wasm_component_model_async",
    ]
    .into_iter()
    .filter(|name| !word_at(&code, name).is_empty())
    .map(|name| format!("async/component: {name}"))
    .collect()
}

/// DEP (Architect decision, P0-LINUX-FINAL-R1; P2 security closure R1):
/// Nexus's Wasmtime use stays core Wasm only. Wasmtime is pinned to the
/// 36.x security line (36.0.17), where RUSTSEC-2026-0316 (dynamic `Val`
/// lifting, once accepted here narrowly) is patched and RUSTSEC-2026-0327
/// (component async-lifted callbacks, 39.0.0 and later) does not apply, so
/// deny.toml accepts no Wasmtime advisory. No production source uses
/// Wasmtime's component module (the dynamic `Val`/`Func` API lives there)
/// through a path, group or glob (P0-LINUX-FINAL-R2), or names Wasmtime's
/// async or component-model entry points; no production source aliases or
/// re-exports the crate under another name, and no workspace package
/// renames the wasmtime dependency in Cargo's effective metadata
/// (P0-LINUX-FINAL-R2A); the SDK sandbox runs core-Wasm modules through
/// typed entry functions; and the SDK sandbox stays a latent API the desktop
/// must not call. Direct core-Wasm use stays allowed. This is a Nexus
/// non-use and reachability invariant, not proof that the pinned Wasmtime
/// is generally safe.
#[test]
fn p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api() {
    // The scanner rejects every alias of the crate, whether or not the same
    // file names `component` (another file could use the alias) ...
    for probe in [
        "use wasmtime as wt;",
        "pub use wasmtime as wt;",
        "pub(crate) use wasmtime as wt;",
        "use wasmtime::{self as wt};",
        "pub use wasmtime::{self as wt, Engine};",
        "extern crate wasmtime as wt;",
        "pub extern crate wasmtime as wt;",
        "use r#wasmtime as wt;",
        "pub use wasmtime as r#wt;",
        "use ::wasmtime as wt;",
        "use {wasmtime as wt};",
        "pub use wasmtime\n    as\n    wt;",
        "#[cfg(any())]\npub use wasmtime as wt;",
        "use wasmtime as _;",
    ] {
        assert!(
            wasmtime_forbidden_uses(probe)
                .iter()
                .any(|use_| use_.starts_with("alias: ")),
            "the crate-alias probe was not caught: {probe:?}"
        );
    }
    // ... every glob import of the crate ...
    for probe in ["pub use wasmtime::*;", "use wasmtime::{Engine, *};"] {
        assert!(
            wasmtime_forbidden_uses(probe)
                .iter()
                .any(|use_| use_.starts_with("glob: ")),
            "the glob probe was not caught: {probe:?}"
        );
    }
    // ... and every spelling of the component module (the R2 probes) ...
    for probe in [
        "use wasmtime::component::Val;",
        "use wasmtime::{component::Val};",
        "use wasmtime::{component::{Val}};",
        "use wasmtime::{Engine, component::{Val, Func}};",
        "use wasmtime::{component as component_api};",
        "use wasmtime :: component :: Val;",
        "use wasmtime::{\n    Engine,\n    component::{\n        Val,\n    },\n};",
        "use wasmtime\n    ::\n    component\n    ::\n    Func;",
        "fn f() { let v: wasmtime::component::Val = todo!(); }",
        "use wasmtime as wt; fn f() { let _ = wt::component::Linker::<()>::new; }",
        "use wasmtime::{self as wt}; use wt::component::Val;",
        "extern crate wasmtime as wasm; use wasm::{component::Val};",
        "use wasmtime::*; fn f(v: component::Val) {}",
        "use wasmtime::{*}; use component::Func;",
        "#[cfg(any())]\nuse wasmtime::{component::{Val}};",
    ] {
        assert!(
            !wasmtime_forbidden_uses(probe).is_empty(),
            "the component-module probe was not caught: {probe:?}"
        );
    }
    // ... including its raw-identifier spelling `r#component`, which names the
    // same module (P0-LINUX-FINAL-R2B) ...
    for probe in [
        "use wasmtime::r#component::Val;",
        "fn f() { let _: wasmtime::r#component::Val = todo!(); }",
        "use ::wasmtime::r#component as component_api;",
        "use r#wasmtime::r#component::Func;",
        "use wasmtime :: r#component :: Val;",
        "use wasmtime\n    ::\n    r#component\n    ::\n    Val;",
        "#[cfg(any())]\nuse wasmtime::r#component::Val;",
        "use wasmtime::{r#component::Val};",
        "use wasmtime::{Engine, r#component as c};",
    ] {
        assert!(
            wasmtime_forbidden_uses(probe)
                .iter()
                .any(|use_| use_.starts_with("component: ")),
            "the raw component-module probe was not caught: {probe:?}"
        );
    }
    // ... and a literal never hides the code after it: raw strings (any
    // hash count, `br`, `cr`), escaped strings and character literals keep
    // the lexer in step, so a comment marker inside one cannot swallow a
    // later use (P0-LINUX-FINAL-R2C). The `/*` string after an odd-quote
    // literal is the shape that hid a use from the R2B lexer.
    let use_ = "#[cfg(any())]\nuse wasmtime::r#component::Val;\n";
    let tail = "const OPEN: &str = \"/*\";\n#[cfg(any())]\nuse wasmtime::r#component::Val;\nconst CLOSE: &str = \"*/\";";
    for probe in [
        format!("const X: &str =\n    r#\"x\" // still raw\"#;\n{use_}"),
        format!("const X: &str = r#\"x\" \"#;\n{tail}"),
        format!("const X: &str = r##\"a \"# b\"##;\n{tail}"),
        format!("const X: &str = r###\"a \"## \" // b\"###;\n{tail}"),
        format!("const X: &[u8] = br#\"x\" \"#;\n{tail}"),
        format!("const X: &[u8] = br##\"x\" // \"##;\n{use_}"),
        format!("const X: &core::ffi::CStr = cr#\"x\" \"#;\n{tail}"),
        format!("const X: &core::ffi::CStr = cr##\"x\" /* \"##;\n{use_}"),
        format!("const X: &str = r\"x\\\";\n{tail}"),
        format!("const X: &str = r#\"x\" /* \"#;\n{use_}const CLOSE: &str = \"*/\";"),
        format!("const X: &str = r#\"/* a \" b */\"#;\n{use_}"),
        format!("const X: &str = r#\"he said \"hi\" and \"\"#;\n{tail}"),
        format!("const X: &str = \"a \\\" b\";\n{tail}"),
        format!("const X: &[u8] = b\"a \\\" b\";\n{tail}"),
        format!("const X: &core::ffi::CStr = c\"a \\\" b\";\n{tail}"),
        format!("const Q: char = '\"';\n{tail}"),
        format!("const Q: u8 = b'\"';\n{tail}"),
        format!("const Q: char = '\\'';\nconst R: &str = \"x\";\n{tail}"),
    ] {
        assert!(
            wasmtime_forbidden_uses(&probe)
                .iter()
                .any(|use_| use_.starts_with("component: ")),
            "a literal hid the component-module use: {probe:?}"
        );
    }
    // The lexer keeps literals whole and removes only real comments.
    for (src, kept, removed) in [
        ("r#\"a\" // b\"# x", "r#\"a\" // b\"# x", ""),
        ("br##\"a\"# /* b\"## x", "br##\"a\"# /* b\"## x", ""),
        ("cr\"a /* b\" x", "cr\"a /* b\" x", ""),
        ("\"a \\\" // b\" x // c", "\"a \\\" // b\" x ", "// c"),
        ("'\"' x /* c */ y", "'\"' x", "/* c */"),
        (
            "fn f<'a>(x: &'a str) {} // c",
            "fn f<'a>(x: &'a str) {} ",
            "// c",
        ),
        ("r#component /* c */ x", "r#component", "/* c */"),
    ] {
        let plain = without_comments(src);
        assert!(plain.contains(kept), "{src:?} lost {kept:?}: {plain:?}");
        assert!(
            removed.is_empty() || !plain.contains(removed),
            "{src:?} kept the comment {removed:?}: {plain:?}"
        );
    }
    // ... while it accepts the core API the SDK sandbox uses, and comments.
    for safe in [
        "use wasmtime::{Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};",
        "let f = instance.get_typed_func::<(), ()>(&mut store, \"_start\")?;",
        "use wasmtime::Linker; let x = wasmtime::Val::I32(1);",
        "// wasmtime::component::Val is not used here\nuse wasmtime::Engine;",
        "/* use wasmtime::{component::Val}; */ use wasmtime::Module;",
        "/* pub use wasmtime as wt; */ use wasmtime::Store;",
        "mod my_component { pub struct Val; }",
        "use wasmtime::{Engine as WasmEngine, Store};",
        "extern crate wasmtime;",
        "let x = wasmtime::r#componentx;",
        "const X: &str = r#\"see // here\"#;\nuse wasmtime::Engine;",
        "const X: &str = r##\"a \"# // b\"##;\nuse wasmtime::Linker;",
        "const X: &str = r#\"/* not a comment */\"#;\nuse wasmtime::Store;",
        "const X: &[u8] = br#\"/* \" */\"#;\nuse wasmtime::Module;",
        "const X: &str = \"say \\\"hi\\\" // not a comment\";\nuse wasmtime::Module;",
        "const Q: char = '\"'; fn f<'a>(x: &'a str) {}\nuse wasmtime::Engine;",
    ] {
        assert_eq!(
            wasmtime_forbidden_uses(safe),
            Vec::<String>::new(),
            "a core-API or comment-only source was rejected: {safe:?}"
        );
    }

    // ... and every async or component-model entry point in a source that
    // uses Wasmtime, while the same names elsewhere, or only in comments,
    // stay allowed.
    for probe in [
        "use wasmtime::Engine; fn f() { let _ = func.call_async(&mut store, ()); }",
        "fn f(l: &wasmtime::Linker<()>) { let _ = l.instantiate_async; }",
        "use wasmtime::Linker; fn f(l: &mut Linker<()>) { l.func_wrap_async(); }",
        "use wasmtime::Func; fn f() { Func::new_async; func_new_async(); }",
        "fn f(c: &mut wasmtime::Config) { c.async_support(true); }",
        "fn f(c: &mut wasmtime::Config) { c.wasm_component_model(true); }",
        "fn f(c: &mut wasmtime::Config) { c.wasm_component_model_async(true); }",
    ] {
        assert!(
            !wasmtime_async_uses(probe).is_empty(),
            "an async or component-model use was not detected: {probe:?}"
        );
    }
    for safe in [
        "fn f() { client.call_async().await; }",
        "use wasmtime::Engine; // call_async is not used here",
        "use wasmtime::{Engine, Linker, Module, Store}; fn f() { t.call(&mut s, ()); }",
    ] {
        assert_eq!(
            wasmtime_async_uses(safe),
            Vec::<String>::new(),
            "a core-API or unrelated source was rejected: {safe:?}"
        );
    }

    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                if !name.starts_with('.')
                    && !matches!(
                        name.as_str(),
                        "target" | "node_modules" | "tests" | "benches" | "dist"
                    )
                {
                    walk(&path, out);
                }
            } else if name.ends_with(".rs") && !name.ends_with("tests.rs") {
                out.push(path);
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = Vec::new();
    walk(&root, &mut sources);
    assert!(sources.len() > 100, "workspace sources not found");
    for path in &sources {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let uses = wasmtime_forbidden_uses(&text);
        let aliases: Vec<&String> = uses.iter().filter(|u| u.starts_with("alias: ")).collect();
        assert!(
            aliases.is_empty(),
            "{}: {aliases:?} (the wasmtime crate is not aliased or renamed)",
            path.display()
        );
        assert!(
            uses.is_empty(),
            "{}: {uses:?} (Wasmtime's component module is not used)",
            path.display()
        );
        let async_uses = wasmtime_async_uses(&text);
        assert!(
            async_uses.is_empty(),
            "{}: {async_uses:?} (Wasmtime's async and component-model APIs are not used)",
            path.display()
        );
    }

    // A dependency rename would hide the crate behind another name. Cargo's
    // effective metadata for this exact workspace, from the Cargo that built
    // this test (not one the caller selects), lists each package's
    // dependencies with their `rename`; none of the wasmtime entries may
    // carry one.
    let output = std::process::Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--locked",
            "--manifest-path",
        ])
        .arg(root.join("Cargo.toml"))
        .output()
        .expect("cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata JSON");
    assert_eq!(metadata["version"], 1, "cargo metadata format");
    assert_eq!(
        std::path::Path::new(metadata["workspace_root"].as_str().expect("workspace_root")),
        root.canonicalize().unwrap(),
        "cargo metadata describes another workspace"
    );
    let packages = metadata["packages"].as_array().expect("packages");
    assert_eq!(
        packages.len(),
        metadata["workspace_members"]
            .as_array()
            .expect("workspace_members")
            .len(),
        "cargo metadata lists every workspace member"
    );
    let mut wasmtime_dependencies = 0;
    for package in packages {
        for dependency in package["dependencies"].as_array().expect("dependencies") {
            if dependency["name"] != "wasmtime" {
                continue;
            }
            wasmtime_dependencies += 1;
            let rename = dependency.get("rename").expect("dependency rename field");
            assert!(
                rename.is_null(),
                "{}: wasmtime dependency renamed to {rename} (the wasmtime crate is not aliased or renamed)",
                package["name"]
            );
        }
    }
    assert!(
        wasmtime_dependencies > 0,
        "cargo metadata shows no wasmtime dependency"
    );

    let sandbox = include_str!("../../../../../sdk/src/wasmtime_sandbox.rs");
    assert!(sandbox.contains("use wasmtime::{Engine, Linker, Module, Store"));
    assert!(sandbox.contains(".get_typed_func::<(), ()>("));

    let registry = include_str!("../tests.rs");
    for latent in ["(\"WasmtimeSandbox\",", "(\"WasmAgent\","] {
        assert!(
            registry.contains(latent),
            "latent-API needle {latent} removed"
        );
    }

    // No Wasmtime advisory is accepted (P2 security closure R1): none may
    // return to the exception set without its own review.
    let deny = include_str!("../../../../../deny.toml").replace("\r\n", "\n");
    let entries: Vec<&str> = deny
        .lines()
        .filter(|line| line.contains("id = \"RUSTSEC-"))
        .collect();
    assert!(!entries.is_empty(), "deny.toml exception entries not found");
    for entry in entries {
        for wasmtime in ["wasmtime", "RUSTSEC-2026-0316", "RUSTSEC-2026-0327"] {
            assert!(
                !entry.contains(wasmtime),
                "a Wasmtime advisory is accepted: {entry}"
            );
        }
    }
}
