//! Narrow source guard for the main StepOutput success diagnostic before item storage.
//! This exact metadata allowlist is not a general Rust parser or data-flow audit.
//! Run without building the kernel: rustc --test --edition=2021 <this file>.

const SOURCE: &str = include_str!("../src/cognitive/loop_runtime.rs");
const DISPATCH: &str = "let (step_executed, step_fuel, step_error) = match executed {";
const SUCCESS: &str = "Ok(StepOutput { text: result, item }) => {";
const STORE_ITEM: &str = "state.results.insert(step.id.clone(), item);";
const CONTENT_FREE_DIAGNOSTIC: &str = r#"
    eprintln!(
        "[agent:{}] actuator result for {}: {} bytes",
        agent_id,
        action_clone.action_type(),
        result.len()
    );
"#;

// Deliberately allow only this metadata expression at the identified sink.
// This is a narrow source guard, not a general Rust logging/data-flow audit.
fn diagnostic_range(source: &str) -> Option<std::ops::Range<usize>> {
    for anchor in [DISPATCH, SUCCESS, STORE_ITEM] {
        if source.matches(anchor).count() != 1 {
            return None;
        }
    }
    let dispatch = source.find(DISPATCH)? + DISPATCH.len();
    let start = dispatch + source[dispatch..].find(SUCCESS)? + SUCCESS.len();
    let end = start + source[start..].find(STORE_ITEM)?;
    Some(start..end)
}

fn has_content_free_result_diagnostic(source: &str) -> bool {
    let Some(range) = diagnostic_range(source) else {
        return false;
    };
    source[range]
        .split_whitespace()
        .eq(CONTENT_FREE_DIAGNOSTIC.split_whitespace())
}

fn with_diagnostic(diagnostic: &str) -> String {
    format!("{DISPATCH}\n{SUCCESS}\n{diagnostic}\n{STORE_ITEM}")
}

#[test]
fn actuator_success_diagnostic_is_content_free() {
    assert!(
        has_content_free_result_diagnostic(SOURCE),
        "actuator-success diagnostic must contain only approved metadata"
    );
}

#[test]
fn guard_accepts_corrected_metadata_only_form() {
    assert!(has_content_free_result_diagnostic(&with_diagnostic(
        CONTENT_FREE_DIAGNOSTIC
    )));
}

#[test]
fn guard_rejects_synthetic_result_content_printing() {
    let unsafe_diagnostics = [
        // Original sink, reproduced with source only; no result is executed.
        r#"eprintln!(
            "[agent:{}] actuator result for {}: {} chars — {}",
            agent_id,
            action_clone.action_type(),
            result.len(),
            &result[..result.floor_char_boundary(200)]
        );"#,
        r#"eprintln!("{}", result);"#,
        r#"eprintln!("{result}");"#,
        r#"eprintln!("{:?}", result);"#,
        r#"let preview = &result[..20]; eprintln!("{}", preview);"#,
        r#"tracing::info!(result = %result, "actuator result");"#,
    ];
    for (index, diagnostic) in unsafe_diagnostics.iter().enumerate() {
        assert!(
            !has_content_free_result_diagnostic(&with_diagnostic(diagnostic)),
            "unsafe synthetic diagnostic {index} was accepted"
        );
    }
    let appended_print = format!("{CONTENT_FREE_DIAGNOSTIC}\neprintln!(\"{{}}\", result);");
    assert!(!has_content_free_result_diagnostic(&with_diagnostic(
        &appended_print
    )));
}

#[test]
fn guard_rejects_content_printing_reintroduced_into_actual_source() {
    assert!(has_content_free_result_diagnostic(SOURCE));
    let range = diagnostic_range(SOURCE).unwrap();
    let mutations = [
        (
            "original_preview",
            r#"eprintln!(
            "[agent:{}] actuator result for {}: {} chars — {}",
            agent_id,
            action_clone.action_type(),
            result.len(),
            &result[..result.floor_char_boundary(200)]
        );"#,
        ),
        ("raw_result", r#"eprintln!("{}", result);"#),
        ("stdout_result", r#"println!("{}", result);"#),
        ("interpolated_result", r#"eprintln!("{result}");"#),
        ("debug_result", r#"eprintln!("{result:?}");"#),
        (
            "alias_preview",
            r#"let preview = &result[..20]; eprintln!("{}", preview);"#,
        ),
        (
            "tracing_result",
            r#"tracing::info!(result = %result, "actuator result");"#,
        ),
        ("log_result", r#"log::info!("{}", result);"#),
    ];
    for (name, diagnostic) in mutations {
        for append in [false, true] {
            let diagnostic = if append {
                format!("{CONTENT_FREE_DIAGNOSTIC}\n{diagnostic}")
            } else {
                diagnostic.to_owned()
            };
            let mutant = format!(
                "{}\n{diagnostic}\n{}",
                &SOURCE[..range.start],
                &SOURCE[range.end..]
            );
            assert_ne!(mutant, SOURCE, "mutation {name} did not change the source");
            assert!(
                !has_content_free_result_diagnostic(&mutant),
                "unsafe mutation {name} (append={append}) was accepted"
            );
            println!("rejected synthetic mutation: {name} (append={append})");
        }
    }
}

#[test]
fn guard_fails_closed_on_missing_sink_or_metadata() {
    for source in ["", DISPATCH, SUCCESS, STORE_ITEM] {
        assert!(!has_content_free_result_diagnostic(source));
    }
    for anchor in [DISPATCH, SUCCESS, STORE_ITEM] {
        let missing = SOURCE.replacen(anchor, "missing_boundary", 1);
        assert!(!has_content_free_result_diagnostic(&missing));
        let duplicate = format!("{anchor}\n{SOURCE}");
        assert!(!has_content_free_result_diagnostic(&duplicate));
    }
    assert!(!has_content_free_result_diagnostic(&with_diagnostic("")));
}
