//! Narrow source guard for the executor-failure diagnostic before policy handling.
//! Run without Cargo: rustc --test --edition=2021 <this file>.
//! This exact metadata allowlist is not a general Rust parser or data-flow audit.

const SOURCE: &str = include_str!("../src/cognitive/loop_runtime.rs");
const DISPATCH: &str = "let (step_executed, step_fuel, step_error) = match executed {";
const FAILURE: &str = "Err(error) => {";
const POLICY: &str = "if error.starts_with(\"human approval required:\")";
const CONTENT_FREE_DIAGNOSTIC: &str = r#"
    eprintln!(
        "[agent:{}] executor FAILED for {}",
        agent_id,
        action_clone.action_type()
    );
"#;

fn diagnostic_range(source: &str) -> Option<std::ops::Range<usize>> {
    for anchor in [DISPATCH, FAILURE, POLICY] {
        if source.matches(anchor).count() != 1 {
            return None;
        }
    }
    let dispatch = source.find(DISPATCH)? + DISPATCH.len();
    let start = dispatch + source[dispatch..].find(FAILURE)? + FAILURE.len();
    let end = start + source[start..].find(POLICY)?;
    Some(start..end)
}

fn has_content_free_failure_diagnostic(source: &str) -> bool {
    let Some(range) = diagnostic_range(source) else {
        return false;
    };
    source[range]
        .split_whitespace()
        .eq(CONTENT_FREE_DIAGNOSTIC.split_whitespace())
}

fn with_diagnostic(source: &str, diagnostic: &str) -> String {
    let range = diagnostic_range(source).expect("identified executor failure diagnostic");
    format!(
        "{}\n{diagnostic}\n{}",
        &source[..range.start],
        &source[range.end..]
    )
}

#[test]
fn executor_failure_diagnostic_is_content_free() {
    assert!(
        has_content_free_failure_diagnostic(SOURCE),
        "executor-failure diagnostic must contain only agent ID and static action type"
    );
}

#[test]
fn guard_accepts_metadata_only_form_and_whitespace() {
    for diagnostic in [
        CONTENT_FREE_DIAGNOSTIC.to_owned(),
        CONTENT_FREE_DIAGNOSTIC.replace('\n', " "),
    ] {
        assert!(has_content_free_failure_diagnostic(&with_diagnostic(
            SOURCE,
            &diagnostic
        )));
    }
}

#[test]
fn guard_rejects_unsafe_mutations_in_actual_source() {
    let mutations = [
        (
            "original_preview",
            r#"eprintln!(
            "[agent:{}] executor FAILED for {}: {}",
            agent_id,
            action_clone.action_type(),
            &error[..error.floor_char_boundary(300)]
        );"#,
        ),
        ("raw_error", r#"eprintln!("{}", error);"#),
        ("stdout_error", r#"println!("{}", error);"#),
        ("interpolated_error", r#"eprintln!("{error}");"#),
        ("debug_error", r#"eprintln!("{error:?}");"#),
        (
            "alias_preview",
            r#"let preview = &error[..error.floor_char_boundary(20)]; eprintln!("{}", preview);"#,
        ),
        (
            "tracing_error",
            r#"tracing::error!(error = %error, "executor failure");"#,
        ),
        ("tracing_warn", r#"tracing::warn!("{error}");"#),
        (
            "tracing_debug",
            r#"tracing::debug!(error = ?error, "executor failure");"#,
        ),
        ("log_error", r#"log::error!("{}", error);"#),
    ];
    for (name, diagnostic) in mutations {
        for append in [false, true] {
            let diagnostic = if append {
                format!("{CONTENT_FREE_DIAGNOSTIC}\n{diagnostic}")
            } else {
                diagnostic.to_owned()
            };
            let mutant = with_diagnostic(SOURCE, &diagnostic);
            assert_ne!(mutant, SOURCE, "mutation {name} did not change the source");
            assert!(
                !has_content_free_failure_diagnostic(&mutant),
                "unsafe mutation {name} (append={append}) was accepted"
            );
            println!("rejected synthetic mutation: {name} (append={append})");
        }
    }
}

#[test]
fn guard_rejects_missing_or_ambiguous_boundaries_and_empty_diagnostic() {
    for source in ["", DISPATCH, FAILURE, POLICY] {
        assert!(!has_content_free_failure_diagnostic(source));
    }
    for anchor in [DISPATCH, FAILURE, POLICY] {
        let source = SOURCE.replacen(anchor, "missing_boundary", 1);
        assert!(!has_content_free_failure_diagnostic(&source));
    }
    for anchor in [DISPATCH, FAILURE, POLICY] {
        let duplicate = format!("{anchor}\n{SOURCE}");
        assert!(!has_content_free_failure_diagnostic(&duplicate));
    }
    assert!(!has_content_free_failure_diagnostic(&with_diagnostic(
        SOURCE, ""
    )));
}
