//! Governed tools, launched for real through the kernel's sealed spawn,
//! with harmless system binaries as fixtures.

use super::catalog;
use super::{ToolDefinition, ToolIntent, ToolInvocation, ToolOutput, Tools};
use crate::authority::commitment::CommitmentState;
use crate::authority::effect::EffectClass;
use crate::authority::AuthorityError;
use crate::control::EffectOutput;
use crate::executable::Trust;
use crate::harness_tests::{harness, temp_root, Harness, TempRoot, Yes};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn no_args(input: &Value) -> Result<ToolInvocation, AuthorityError> {
    crate::connector::only_fields(input, &[])?;
    Ok(ToolInvocation {
        args: vec![],
        files: vec![],
        summary: vec![],
    })
}

fn sleep_long(input: &Value) -> Result<ToolInvocation, AuthorityError> {
    crate::connector::only_fields(input, &[])?;
    Ok(ToolInvocation {
        args: vec!["30".into()],
        files: vec![],
        summary: vec![],
    })
}

fn fixture(
    key: &'static str,
    executable: &'static str,
    build: fn(&Value) -> Result<ToolInvocation, AuthorityError>,
    timeout: Duration,
    max_output: usize,
) -> ToolDefinition {
    ToolDefinition {
        key,
        executable,
        class: EffectClass::R0,
        build,
        env: &[("LC_ALL", "C")],
        timeout,
        max_output,
        output: ToolOutput::Text,
        trust: Trust::System,
    }
}

fn fixtures() -> Vec<ToolDefinition> {
    let mut all = catalog::production();
    all.push(fixture(
        "fixture.environment",
        "/usr/bin/env",
        no_args,
        Duration::from_secs(10),
        64 * 1024,
    ));
    all.push(fixture(
        "fixture.sleep.short",
        "/usr/bin/sleep",
        sleep_long,
        Duration::from_millis(300),
        1024,
    ));
    all.push(fixture(
        "fixture.sleep.long",
        "/usr/bin/sleep",
        sleep_long,
        Duration::from_secs(60),
        1024,
    ));
    all.push(fixture(
        "fixture.noisy",
        "/usr/bin/yes",
        no_args,
        Duration::from_secs(10),
        1024,
    ));
    all.push(fixture(
        "fixture.false",
        "/usr/bin/false",
        no_args,
        Duration::from_secs(10),
        1024,
    ));
    all
}

fn grant(h: &Harness, tools: &Tools, key: &str) {
    let scope = tools.grant_scope(key).unwrap();
    h.control
        .authority()
        .grants()
        .request(scope, Duration::from_secs(600), &Yes::new(true))
        .unwrap();
}

fn run(
    h: &Harness,
    tools: &Tools,
    key: &str,
    input: Value,
) -> Result<EffectOutput, AuthorityError> {
    let preparation = tools.prepare(
        h.control.authority(),
        &ToolIntent {
            tool: key.into(),
            input,
        },
    )?;
    let view = h.control.propose(&h.agent, h.run, preparation)?;
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))?;
    h.control.execute(view.id, &h.agent, h.run)
}

fn last_failure(h: &Harness) -> Option<&'static str> {
    h.evidence
        .records()
        .into_iter()
        .rev()
        .find(|r| r.outcome.is_some())
        .and_then(|r| r.failure)
}

fn entries(root: &TempRoot) -> usize {
    std::fs::read_dir(root.0.path()).unwrap().count()
}

/// A temporary root kept alive by `roots` for the rest of the test.
fn root_guard(roots: &mut Vec<TempRoot>) -> crate::runtime_root::RuntimeRoot {
    roots.push(temp_root("tools"));
    roots.last().unwrap().0.clone()
}

#[test]
fn a_tool_runs_end_to_end_in_a_sealed_process_and_leaves_nothing_behind() {
    let h = harness();
    let root = temp_root("tools");
    let tools = Tools::new(fixtures(), root.0.clone());
    grant(&h, &tools, "text.sha256");
    let out = run(&h, &tools, "text.sha256", json!({ "text": "hello" })).unwrap();
    assert_eq!(
        out.text.as_deref(),
        Some("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824  input.txt\n")
    );
    assert_eq!(entries(&root), 0, "the working directory was removed");
    let views = h.control.authority().commitments().views_of_run(h.run);
    assert_eq!(views[0].class, EffectClass::R0);
    assert_eq!(views[0].state, CommitmentState::Succeeded);
}

#[test]
fn the_environment_is_sealed() {
    let h = harness();
    let root = temp_root("tools");
    let tools = Tools::new(fixtures(), root.0.clone());
    grant(&h, &tools, "fixture.environment");
    let out = run(&h, &tools, "fixture.environment", json!({})).unwrap();
    let text = out.text.unwrap();
    let mut names: Vec<&str> = text.lines().filter_map(|l| l.split('=').next()).collect();
    names.sort();
    assert_eq!(names, vec!["HOME", "LC_ALL", "TMPDIR"], "{text}");
    for line in text.lines() {
        if let Some(dir) = line
            .strip_prefix("HOME=")
            .or_else(|| line.strip_prefix("TMPDIR="))
        {
            assert!(PathBuf::from(dir).starts_with(root.0.path()), "{line}");
        }
    }
}

#[test]
fn nothing_runs_without_a_grant_or_with_untyped_input() {
    let h = harness();
    let mut _roots = Vec::new();
    let tools = Tools::new(fixtures(), root_guard(&mut _roots));
    assert_eq!(
        run(&h, &tools, "text.sha256", json!({ "text": "x" })).unwrap_err(),
        AuthorityError::NoCoveringGrant
    );
    grant(&h, &tools, "text.sha256");
    assert!(matches!(
        run(
            &h,
            &tools,
            "text.sha256",
            json!({ "text": "x", "command": "ls" })
        )
        .unwrap_err(),
        AuthorityError::InvalidAction(_)
    ));
    for missing in ["shell", "bash", "docker", "code.execute", "/bin/sh"] {
        assert_eq!(
            run(&h, &tools, missing, json!({})).unwrap_err(),
            AuthorityError::Closed("no such tool"),
            "{missing}"
        );
    }
}

/// A user-owned copy of `/usr/bin/true`, pinned as a test fixture.
fn copied_true(dir: &std::path::Path) -> &'static str {
    std::fs::create_dir_all(dir).unwrap();
    let path = std::fs::canonicalize(dir).unwrap().join("fixture-true");
    std::fs::copy("/usr/bin/true", &path).unwrap();
    Box::leak(path.to_string_lossy().into_owned().into_boxed_str())
}

#[test]
fn a_changed_executable_needs_a_new_grant_and_fails_a_pending_run() {
    let h = harness();
    let mut _roots = Vec::new();
    let dir = std::env::temp_dir().join(format!("nexus-p3-tool-change-{}", std::process::id()));
    let executable = copied_true(&dir);
    let definition = ToolDefinition {
        trust: Trust::Fixture,
        ..fixture(
            "fixture.true",
            executable,
            no_args,
            Duration::from_secs(10),
            1024,
        )
    };
    let tools = Tools::new(vec![definition], root_guard(&mut _roots));
    grant(&h, &tools, "fixture.true");
    run(&h, &tools, "fixture.true", json!({})).unwrap();
    // Prepared and authorized, then the executable changes before launch.
    let preparation = tools
        .prepare(
            h.control.authority(),
            &ToolIntent {
                tool: "fixture.true".into(),
                input: json!({}),
            },
        )
        .unwrap();
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    std::fs::copy("/usr/bin/false", executable).unwrap();
    assert_eq!(
        h.control.execute(view.id, &h.agent, h.run).unwrap_err(),
        AuthorityError::TargetChanged
    );
    assert_eq!(
        h.control
            .authority()
            .commitments()
            .view(view.id)
            .unwrap()
            .state,
        CommitmentState::Failed
    );
    // A new preparation is refused until the owner grants the new identity.
    assert_eq!(
        run(&h, &tools, "fixture.true", json!({})).unwrap_err(),
        AuthorityError::Closed("the tool changed since it was granted; grant it again")
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_deadline_ends_the_whole_process_group() {
    let h = harness();
    let mut _roots = Vec::new();
    let tools = Tools::new(fixtures(), root_guard(&mut _roots));
    grant(&h, &tools, "fixture.sleep.short");
    let started = Instant::now();
    assert!(run(&h, &tools, "fixture.sleep.short", json!({})).is_err());
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(last_failure(&h), Some("timeout"));
}

#[test]
fn cancelling_the_run_ends_a_running_tool() {
    let h = harness();
    let mut _roots = Vec::new();
    let tools = std::sync::Arc::new(Tools::new(fixtures(), root_guard(&mut _roots)));
    grant(&h, &tools, "fixture.sleep.long");
    let preparation = tools
        .prepare(
            h.control.authority(),
            &ToolIntent {
                tool: "fixture.sleep.long".into(),
                input: json!({}),
            },
        )
        .unwrap();
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    let control = h.control.clone();
    let (agent, run) = (h.agent.clone(), h.run);
    let started = Instant::now();
    let worker = std::thread::spawn(move || control.execute(view.id, &agent, run));
    std::thread::sleep(Duration::from_millis(200));
    h.control.cancel_run(h.run).unwrap();
    assert_eq!(
        worker.join().unwrap().unwrap_err(),
        AuthorityError::RunCancelled
    );
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(
        h.control
            .authority()
            .commitments()
            .view(view.id)
            .unwrap()
            .state,
        CommitmentState::Cancelled
    );
}

fn touch_marker(input: &Value) -> Result<ToolInvocation, AuthorityError> {
    crate::connector::only_fields(input, &["path"])?;
    let path = input["path"]
        .as_str()
        .ok_or(AuthorityError::InvalidAction("a path"))?;
    Ok(ToolInvocation {
        args: vec![path.into()],
        files: vec![],
        summary: vec![],
    })
}

/// Between `begin` and the launch nothing new may start: a tool whose run
/// is cancelled there is never spawned (it would leave its marker).
#[test]
fn a_tool_is_not_spawned_once_its_run_is_cancelled() {
    use crate::control::Preparation;
    let mut _roots = Vec::new();
    let mut definitions = fixtures();
    definitions.push(fixture(
        "fixture.touch",
        "/usr/bin/touch",
        touch_marker,
        Duration::from_secs(10),
        1024,
    ));
    let tools = Tools::new(definitions, root_guard(&mut _roots));
    let markers = temp_root("markers");
    let prepare = |h: &Harness, marker: &std::path::Path| {
        tools
            .prepare(
                h.control.authority(),
                &ToolIntent {
                    tool: "fixture.touch".into(),
                    input: json!({ "path": marker.to_str().unwrap() }),
                },
            )
            .unwrap()
    };

    // The control: with its run live, the tool runs and leaves its marker.
    let live = harness();
    grant(&live, &tools, "fixture.touch");
    let ran = markers.0.path().join("ran");
    let view = live
        .control
        .propose(&live.agent, live.run, prepare(&live, &ran))
        .unwrap();
    live.control
        .authorize(view.id, &live.agent, live.run, &Yes::new(true))
        .unwrap();
    live.control
        .execute(view.id, &live.agent, live.run)
        .unwrap();
    assert!(ran.exists(), "the fixture tool did not run");

    // The pipeline's own steps (`Control::execute`), with the run cancelled
    // after `begin`.
    let h = harness();
    grant(&h, &tools, "fixture.touch");
    let spawned = markers.0.path().join("spawned");
    let Preparation {
        action,
        effect,
        ttl,
    } = prepare(&h, &spawned);
    let commitments = h.control.authority().commitments();
    let view = commitments.prepare(&h.agent, h.run, action, ttl).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    let target = effect.revalidate().unwrap();
    let guard = commitments
        .begin(view.id, &h.agent, h.run, &target, &effect.parameters())
        .unwrap();
    h.control.cancel_run(h.run).unwrap();
    // Refused before the launch: a tool launched and then killed at its
    // first poll could leave no marker, so the refusal is asked for exactly.
    assert_eq!(
        effect.execute(&guard).unwrap_err().1,
        "cancelled before the tool started"
    );
    assert!(
        !spawned.exists(),
        "a tool was spawned after its run was cancelled"
    );
}

#[test]
fn output_is_bounded_and_failures_are_recorded() {
    let h = harness();
    let mut _roots = Vec::new();
    let tools = Tools::new(fixtures(), root_guard(&mut _roots));
    grant(&h, &tools, "fixture.noisy");
    grant(&h, &tools, "fixture.false");
    assert!(run(&h, &tools, "fixture.noisy", json!({})).is_err());
    assert_eq!(last_failure(&h), Some("bounds"));
    assert!(run(&h, &tools, "fixture.false", json!({})).is_err());
    assert_eq!(last_failure(&h), Some("actuator"));
}

#[test]
fn the_production_catalog_has_no_shell_interpreter_or_runner() {
    const NEVER: [&str; 27] = [
        "sh",
        "bash",
        "dash",
        "zsh",
        "fish",
        "ksh",
        "csh",
        "tcsh",
        "pwsh",
        "powershell",
        "cmd",
        "python",
        "python3",
        "perl",
        "ruby",
        "node",
        "env",
        "busybox",
        "docker",
        "podman",
        "sudo",
        "su",
        "xargs",
        "find",
        "ssh",
        "curl",
        "wget",
    ];
    for definition in catalog::production() {
        let name = std::path::Path::new(definition.executable)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(!NEVER.contains(&name.as_str()), "{}", definition.executable);
        assert!(definition.executable.starts_with('/'));
        assert_eq!(definition.trust, Trust::System);
        // No builder accepts a free-form command, script or argument list.
        for field in ["command", "script", "args", "argv", "program", "path"] {
            assert!(
                (definition.build)(&json!({ field: "x" })).is_err(),
                "{} {field}",
                definition.key
            );
        }
    }
}

#[test]
fn speech_is_synthesized_when_the_local_engine_is_installed() {
    if !std::path::Path::new("/usr/bin/espeak-ng").exists() {
        // CI installs the engine (ci.yml), so there a missing engine fails
        // this test instead of skipping it.
        assert!(
            std::env::var_os("CI").is_none(),
            "espeak-ng is missing: CI must exercise speech synthesis"
        );
        eprintln!("espeak-ng is not installed: speech synthesis is not exercised here");
        return;
    }
    let h = harness();
    let mut _roots = Vec::new();
    let tools = Tools::new(fixtures(), root_guard(&mut _roots));
    grant(&h, &tools, "speech.synthesize");
    let out = run(
        &h,
        &tools,
        "speech.synthesize",
        json!({ "text": "governed", "voice": "en" }),
    )
    .unwrap();
    let audio = out.bytes.unwrap();
    assert!(audio.starts_with(b"RIFF"), "a WAV stream");
    assert!(run(
        &h,
        &tools,
        "speech.synthesize",
        json!({ "text": "x", "voice": "../../etc" })
    )
    .is_err());
}
