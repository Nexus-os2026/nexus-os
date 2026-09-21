//! Real backend regressions. No mock traversal, matcher, authorization or search.
use super::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MARKER: &str = "privacy_witness\n";

struct Fixture {
    temp: tempfile::TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("work");
        std::fs::create_dir(&root).unwrap();
        let f = Self { temp, root };
        f.write("public.txt", MARKER);
        f
    }

    fn write(&self, name: &str, content: impl AsRef<[u8]>) {
        let path = self.root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn ctx(&self) -> ToolContext {
        context(self.root.clone())
    }

    async fn expect(&self, input: serde_json::Value, names: &[&str]) {
        let before = snapshot(self.temp.path());
        for program in ["rg", "grep"] {
            expect(program, &self.ctx(), input.clone(), names).await;
        }
        assert_eq!(
            snapshot(self.temp.path()),
            before,
            "search changed a fixture"
        );
    }
}

fn context(root: PathBuf) -> ToolContext {
    ToolContext {
        working_dir: root,
        blocked_paths: vec![],
        max_file_scope: None,
        non_interactive: true,
    }
}

// Includes outside fixtures and empty directories; never follows test symlinks.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(path: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let meta = std::fs::symlink_metadata(path).unwrap();
        if meta.is_dir() {
            result.insert(path.to_owned(), vec![]);
            for entry in std::fs::read_dir(path).unwrap() {
                visit(&entry.unwrap().path(), result);
            }
        } else if meta.is_file() {
            result.insert(path.to_owned(), std::fs::read(path).unwrap());
        } else {
            result.insert(
                path.to_owned(),
                std::fs::read_link(path)
                    .unwrap()
                    .to_string_lossy()
                    .as_bytes()
                    .to_vec(),
            );
        }
    }
    let mut result = BTreeMap::new();
    visit(root, &mut result);
    result
}

async fn expect(program: &str, ctx: &ToolContext, input: serde_json::Value, names: &[&str]) {
    let result = execute_search(input, ctx, program).await;
    assert!(result.success, "{program}: {}", result.output);
    let mut actual: Vec<_> = result
        .output
        .lines()
        .skip(1)
        .map(|line| {
            line.split_once(':')
                .expect("backend file:line match")
                .0
                .replace('\\', "/")
        })
        .collect();
    actual.sort();
    let mut expected: Vec<_> = names.iter().map(|n| (*n).to_owned()).collect();
    expected.sort();
    assert_eq!(actual, expected, "{program}: {}", result.output);
}

#[tokio::test]
async fn p0_002b_local_ignore_nested_negation_and_explicit_paths() {
    let f = Fixture::new();
    f.write(
        ".gitignore",
        "private.env\nprivate/*\n!private/public.txt\nclosed/\n",
    );
    for path in [
        "private.env",
        "private/secret.txt",
        "private/public.txt",
        "nested/secret.txt",
        "nested/public.txt",
        "closed/public.txt",
    ] {
        f.write(path, MARKER);
    }
    f.write("nested/.gitignore", "secret.txt\n");
    f.write("closed/.gitignore", "!public.txt\n");
    f.expect(
        json!({"pattern":"privacy_witness"}),
        &["public.txt", "private/public.txt", "nested/public.txt"],
    )
    .await;
    f.expect(
        json!({"pattern":"privacy_witness", "path":"nested"}),
        &["nested/public.txt"],
    )
    .await;
    f.expect(
        json!({"pattern":"privacy_witness", "path":"private.env"}),
        &[],
    )
    .await;
    f.expect(
        json!({"pattern":"privacy_witness", "path":"closed/public.txt"}),
        &[],
    )
    .await;
    f.expect(json!({"pattern":"privacy_witness", "include":"*.env"}), &[])
        .await;
}

#[tokio::test]
async fn p0_002b_ignore_sources_and_precedence() {
    let f = Fixture::new();
    f.write(".git/info/exclude", "excluded.txt\nrestored.txt\n");
    f.write(".gitignore", "git.txt\n!restored.txt\n!higher.txt\n");
    f.write(".ignore", "ignore.txt\nhigher.txt\nrg-wins.txt\n");
    f.write(".rgignore", "rg.txt\n!rg-wins.txt\n*.secret\n");
    f.write("nested/.gitignore", "!hidden.secret\n");
    f.write("nested/.ignore", "nested-ignore.txt\n");
    f.write("nested/.rgignore", "!restored.secret\n");
    for path in [
        "excluded.txt",
        "restored.txt",
        "git.txt",
        "ignore.txt",
        "higher.txt",
        "rg.txt",
        "rg-wins.txt",
        "nested/hidden.secret",
        "nested/restored.secret",
        "nested/nested-ignore.txt",
    ] {
        f.write(path, MARKER);
    }
    f.expect(
        json!({"pattern":"privacy_witness"}),
        &[
            "public.txt",
            "restored.txt",
            "rg-wins.txt",
            "nested/restored.secret",
        ],
    )
    .await;
}

#[tokio::test]
async fn p0_002b_search_has_no_fixed_build_exclusions_and_filters_binary() {
    let f = Fixture::new();
    for path in [
        "target/a.txt",
        "node_modules/a.txt",
        "__pycache__/a.txt",
        ".hidden.txt",
        ".allowed.txt",
    ] {
        f.write(path, MARKER);
    }
    f.write(".ignore", "!.allowed.txt\n");
    f.write("binary.dat", b"privacy_witness\n\0secret");
    let mut late = MARKER.as_bytes().to_vec();
    late.extend(vec![b'x'; 40_000]);
    late.push(0);
    f.write("late-binary.dat", late);
    f.expect(
        json!({"pattern":"privacy_witness"}),
        &[
            "public.txt",
            "target/a.txt",
            "node_modules/a.txt",
            "__pycache__/a.txt",
            ".allowed.txt",
        ],
    )
    .await;
    f.expect(
        json!({"pattern":"privacy_witness", "path":"binary.dat"}),
        &[],
    )
    .await;
    f.write("-option.txt", "-f literal\n");
    f.expect(json!({"pattern":"-f literal"}), &["-option.txt"])
        .await;
}

#[tokio::test]
async fn p0_002b_negation_cannot_override_access_policy() {
    let f = Fixture::new();
    f.write(".gitignore", "private/*\n!private/public.txt\n");
    f.write("private/public.txt", MARKER);
    f.expect(
        json!({"pattern":"privacy_witness"}),
        &["public.txt", "private/public.txt"],
    )
    .await;
    let before = snapshot(f.temp.path());
    for scope in [false, true] {
        let mut ctx = f.ctx();
        if scope {
            ctx.max_file_scope = Some(format!("!{}", f.root.join("private/public.txt").display()));
        } else {
            ctx.blocked_paths.push(
                f.root
                    .join("private/public.txt")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        for program in ["rg", "grep"] {
            let result = execute_search(json!({"pattern":"privacy_witness"}), &ctx, program).await;
            assert!(!result.success);
            let error = result
                .filesystem_error()
                .expect("terminal security category");
            assert!(
                matches!(error, crate::error::NxError::CapabilityDenied { ref capability, .. } if capability == if scope { "path.scope" } else { "path.access" })
            );
            assert!(!result.output.contains(f.root.to_str().unwrap()));
        }
    }
    assert_eq!(snapshot(f.temp.path()), before);
}

#[tokio::test]
async fn p0_002b_ignore_policy_reads_cannot_bypass_access_policy() {
    let f = Fixture::new();
    f.write(".gitignore", "private.txt\n");
    f.write("private.txt", MARKER);
    f.expect(json!({"pattern":"privacy_witness"}), &["public.txt"])
        .await;
    let before = snapshot(f.temp.path());
    for scope in [false, true] {
        let mut ctx = f.ctx();
        let policy = f.root.join(".gitignore").to_string_lossy().into_owned();
        if scope {
            ctx.max_file_scope = Some(format!("!{policy}"));
        } else {
            ctx.blocked_paths.push(policy);
        }
        for program in ["rg", "grep"] {
            let result = execute_search(json!({"pattern":"privacy_witness"}), &ctx, program).await;
            assert!(!result.success && result.filesystem_error().is_some());
        }
    }
    assert_eq!(snapshot(f.temp.path()), before);
}

#[cfg(unix)]
#[tokio::test]
async fn p0_002b_ignore_symlinks_and_external_gitdir_cannot_widen_authority() {
    let f = Fixture::new();
    let outside = f.temp.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("secret.txt"), MARKER).unwrap();
    std::fs::create_dir(outside.join("info")).unwrap();
    std::fs::write(outside.join("info/exclude"), "public.txt\n").unwrap();
    f.write(".git", format!("gitdir: {}\n", outside.display()));
    f.write(
        ".ignore",
        format!("!../outside/**\n!{}/**\n!**\n", outside.display()),
    );
    std::os::unix::fs::symlink(&outside, f.root.join("escape")).unwrap();
    std::os::unix::fs::symlink(outside.join("secret.txt"), f.root.join("escape.txt")).unwrap();
    f.expect(json!({"pattern":"privacy_witness"}), &["public.txt"])
        .await;
    std::os::unix::fs::symlink(outside.join("info/exclude"), f.root.join(".gitignore")).unwrap();
    let before = snapshot(f.temp.path());
    for program in ["rg", "grep"] {
        let result = execute_search(json!({"pattern":"privacy_witness"}), &f.ctx(), program).await;
        assert!(!result.success && result.filesystem_error().is_some());
        assert!(!result.output.contains(outside.to_str().unwrap()));
    }
    assert_eq!(snapshot(f.temp.path()), before);
}

#[tokio::test]
async fn p0_002b_ambient_configuration_is_ignored() {
    const CHILD_ROOT: &str = "NEXUS_PRIVACY_TEST_ROOT";
    const CHILD_PROGRAM: &str = "NEXUS_PRIVACY_TEST_PROGRAM";
    const WITNESS: &str = "NEXUS_PRIVACY_CHILD_SEARCH_EXECUTED";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        let program = std::env::var(CHILD_PROGRAM).unwrap();
        expect(
            &program,
            &context(root.into()),
            json!({"pattern":"privacy_witness"}),
            &["public.txt"],
        )
        .await;
        println!("{WITNESS}");
        return;
    }
    let f = Fixture::new();
    std::fs::create_dir(f.root.join(".git")).unwrap();
    for path in [".gitignore", ".ignore", ".rgignore", "global-ignore"] {
        std::fs::write(f.temp.path().join(path), "public.txt\n").unwrap();
    }
    std::fs::create_dir_all(f.temp.path().join("config/git")).unwrap();
    std::fs::write(f.temp.path().join("config/git/ignore"), "public.txt\n").unwrap();
    std::fs::write(
        f.temp.path().join(".gitconfig"),
        format!(
            "[core]\nexcludesFile = \"{}\"\n",
            f.temp
                .path()
                .join("global-ignore")
                .to_string_lossy()
                .replace('\\', "/")
        ),
    )
    .unwrap();
    std::fs::write(f.temp.path().join("rg-config"), "--glob=!public.txt\n").unwrap();
    let before = snapshot(f.temp.path());
    for program in ["rg", "grep"] {
        // Isolated child environment; no global environment mutation in this
        // test process. Parent deadline also covers failure to enter test body.
        let mut cmd = tokio::process::Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "tools::search::privacy_tests::p0_002b_ambient_configuration_is_ignored",
            "--nocapture",
        ])
        .env(CHILD_ROOT, &f.root)
        .env(CHILD_PROGRAM, program)
        .env("HOME", f.temp.path())
        .env("USERPROFILE", f.temp.path())
        .env("XDG_CONFIG_HOME", f.temp.path().join("config"))
        .env("GIT_CONFIG_GLOBAL", f.temp.path().join(".gitconfig"))
        .env("RIPGREP_CONFIG_PATH", f.temp.path().join("rg-config"))
        .env("GREP_OPTIONS", "--exclude=public.txt")
        .kill_on_drop(true);
        let output = tokio::time::timeout(std::time::Duration::from_secs(30), cmd.output())
            .await
            .expect("child deadline")
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            stdout.lines().filter(|line| *line == WITNESS).count(),
            1,
            "{stdout}"
        );
        assert!(stdout.contains("1 passed; 0 failed"), "{stdout}");
    }
    assert_eq!(snapshot(f.temp.path()), before);
}

#[tokio::test]
async fn p0_002b_search_ignore_does_not_change_index_or_glob_contract() {
    let f = Fixture::new();
    f.write(".gitignore", "private.rs\n");
    f.write("private.rs", "pub fn private_definition() {}\n");
    for tool in ["glob", "project_index"] {
        let result = crate::tools::create_tool(tool)
            .unwrap()
            .execute(
                json!({"pattern":"**/*.rs", "include_definitions":true}),
                &f.ctx(),
            )
            .await;
        assert!(
            result.success && result.output.contains("private.rs"),
            "{tool}: {}",
            result.output
        );
    }
}

#[tokio::test]
async fn p0_002b_correction_include_preserves_rg_grammar() {
    let f = Fixture::new();
    for path in [
        "src/a.rs",
        "src/b.ts",
        "src/c.py",
        "literal[1].rs",
        "!literal.rs",
    ] {
        f.write(path, MARKER);
    }
    f.expect(
        json!({"pattern":"privacy_witness", "include":"*.rs"}),
        &["src/a.rs", "literal[1].rs", "!literal.rs"],
    )
    .await;
    f.expect(
        json!({"pattern":"privacy_witness", "include":"*.{rs,ts}"}),
        &["src/a.rs", "src/b.ts", "literal[1].rs", "!literal.rs"],
    )
    .await;
    f.expect(
        json!({"pattern":"privacy_witness", "include":"!*.rs"}),
        &["public.txt", "src/b.ts", "src/c.py"],
    )
    .await;
    f.expect(
        json!({"pattern":"privacy_witness", "include":"!src/"}),
        &["public.txt", "literal[1].rs", "!literal.rs"],
    )
    .await;
    f.expect(
        json!({"pattern":"privacy_witness", "path":"src", "include":"/src/*.{rs,ts}"}),
        &["src/a.rs", "src/b.ts"],
    )
    .await;
    f.expect(
        json!({"pattern":"privacy_witness", "include":r"literal\[1\].rs"}),
        &["literal[1].rs"],
    )
    .await;
    f.expect(
        json!({"pattern":"privacy_witness", "include":r"\!literal.rs"}),
        &["!literal.rs"],
    )
    .await;
}

#[tokio::test]
async fn p0_002b_correction_include_only_narrows_authorized_private_candidates() {
    let f = Fixture::new();
    f.write(".gitignore", "private.rs\n");
    f.write("private.rs", MARKER);
    f.write("public.rs", MARKER);
    f.expect(
        json!({"pattern":"privacy_witness", "include":"*.{rs,ts}"}),
        &["public.rs"],
    )
    .await;
    f.expect(
        json!({"pattern":"privacy_witness", "include":"!*.txt"}),
        &["public.rs"],
    )
    .await;
    let outside = f.temp.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("secret.rs"), MARKER).unwrap();
    for include in [
        "../outside/**".to_owned(),
        format!("{}/**", outside.display()),
    ] {
        f.expect(json!({"pattern":"privacy_witness", "include":include}), &[])
            .await;
    }
    let before = snapshot(f.temp.path());
    for scope in [false, true] {
        let mut ctx = f.ctx();
        let file = f.root.join("public.rs").to_string_lossy().into_owned();
        if scope {
            ctx.max_file_scope = Some(format!("!{file}"));
        } else {
            ctx.blocked_paths.push(file);
        }
        for program in ["rg", "grep"] {
            let result = execute_search(
                json!({"pattern":"privacy_witness", "include":"*.{rs,ts}"}),
                &ctx,
                program,
            )
            .await;
            assert!(!result.success);
            assert!(
                matches!(result.filesystem_error(), Some(crate::error::NxError::CapabilityDenied { capability, .. }) if capability == if scope { "path.scope" } else { "path.access" })
            );
        }
    }
    assert_eq!(snapshot(f.temp.path()), before);
}

#[tokio::test]
async fn p0_002b_correction_ordinary_search_errors_remain_nonterminal() {
    let f = Fixture::new();
    let before = snapshot(f.temp.path());
    for program in ["rg", "grep"] {
        for input in [
            json!({"pattern":"["}),
            json!({"pattern":"privacy_witness", "include":"["}),
        ] {
            let result = execute_search(input, &f.ctx(), program).await;
            assert!(!result.success, "{}", result.output);
            assert!(result.filesystem_error().is_none());
        }
    }
    let missing = f.temp.path().join("missing-backend");
    let result = execute_search(
        json!({"pattern":"privacy_witness"}),
        &f.ctx(),
        missing.to_str().unwrap(),
    )
    .await;
    assert!(!result.success && result.filesystem_error().is_none());
    assert_eq!(snapshot(f.temp.path()), before);
}

#[cfg(unix)]
#[tokio::test]
async fn p0_002b_correction_policy_failure_never_invokes_either_backend() {
    const CHILD_ROOT: &str = "NEXUS_POLICY_FAILURE_ROOT";
    const WITNESS: &str = "NEXUS_POLICY_FAILURE_CHILD_EXECUTED";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        let root = PathBuf::from(root);
        let marker = root.parent().unwrap().join("backend-witness");
        let ctx = context(root.clone());
        for contents in [b"{a,b\n".as_slice(), b"\xff\n".as_slice()] {
            std::fs::write(root.join(".gitignore"), contents).unwrap();
            for program in ["rg", "grep"] {
                let result =
                    execute_search(json!({"pattern":"privacy_witness"}), &ctx, program).await;
                assert!(!result.success);
                assert!(
                    matches!(result.filesystem_error(), Some(crate::error::NxError::CapabilityDenied { capability, .. }) if capability == "path.policy")
                );
                assert!(!marker.exists(), "backend ran despite failed policy");
            }
        }
        // Positive control: these same executable witnesses must run when policy
        // is valid, proving that absence above is not a broken launcher/script.
        std::fs::write(root.join(".gitignore"), "").unwrap();
        for program in ["rg", "grep"] {
            let result = execute_search(json!({"pattern":"privacy_witness"}), &ctx, program).await;
            assert!(result.success, "{}", result.output);
        }
        assert_eq!(std::fs::read_to_string(marker).unwrap(), "rg\ngrep\n");
        println!("{WITNESS}");
        return;
    }
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let bin = f.temp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    for program in ["rg", "grep"] {
        let script = bin.join(program);
        std::fs::write(
            &script,
            format!("#!/bin/sh\nprintf '%s\\n' '{program}' >> \"$NEXUS_POLICY_BACKEND_WITNESS\"\n"),
        )
        .unwrap();
        std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let mut cmd = tokio::process::Command::new(std::env::current_exe().unwrap());
    cmd.args(["--exact", "tools::search::privacy_tests::p0_002b_correction_policy_failure_never_invokes_either_backend", "--nocapture"])
        .env(CHILD_ROOT, &f.root)
        .env("PATH", &bin)
        .env("NEXUS_POLICY_BACKEND_WITNESS", f.temp.path().join("backend-witness"))
        .kill_on_drop(true);
    let output = tokio::time::timeout(std::time::Duration::from_secs(30), cmd.output())
        .await
        .expect("child deadline")
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        stdout.lines().filter(|line| *line == WITNESS).count(),
        1,
        "{stdout}"
    );
    assert!(stdout.contains("1 passed; 0 failed"), "{stdout}");
    assert_eq!(
        std::fs::read(f.root.join("public.txt")).unwrap(),
        MARKER.as_bytes()
    );
}
