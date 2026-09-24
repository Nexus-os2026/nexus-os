//! Portable policy spelling only. Filesystem authority is still decided by
//! the kernel's component-based canonical containment check, before matching.
use std::path::Path;

fn canonical_literal_prefix(path: &Path) -> Option<std::path::PathBuf> {
    let mut ancestor = path.to_path_buf();
    let mut missing = Vec::new();
    loop {
        match ancestor.canonicalize() {
            Ok(mut resolved) => {
                for part in missing.into_iter().rev() {
                    resolved.push(part);
                }
                return Some(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(std::path::Component::Normal(name)) = ancestor.components().next_back()
                else {
                    return None;
                };
                missing.push(name.to_os_string());
                if !ancestor.pop() {
                    return None;
                }
            }
            Err(_) => return None,
        }
    }
}

fn spelling(value: &str) -> String {
    let windows = value.starts_with(r"\\")
        || value.starts_with("//")
        || (value.as_bytes().get(1) == Some(&b':') && value.as_bytes()[0].is_ascii_alphabetic());
    if !windows {
        return value.to_owned();
    }
    let value = value.replace('\\', "/");
    let mut value = if let Some(rest) = value.strip_prefix("//?/UNC/") {
        format!("//{rest}")
    } else {
        value.strip_prefix("//?/").unwrap_or(&value).to_owned()
    };
    if value.as_bytes().get(1) == Some(&b':') {
        value[..1].make_ascii_lowercase();
    }
    value
}

fn suffix_at_root<'a>(path: &'a str, root: &str) -> Option<&'a str> {
    let suffix = path.strip_prefix(root.trim_end_matches('/'))?;
    (suffix.is_empty() || suffix.starts_with('/')).then_some(suffix)
}

fn literal(value: &str) -> String {
    let mut result = String::new();
    for c in value.chars() {
        if matches!(c, '\\' | '*' | '?' | '[' | '{' | '!') {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

fn pattern(value: &str, supplied_root: &str, canonical_root: &str) -> String {
    // Preserve glob-match's leading negation syntax independently of Windows
    // separator conversion. Backslashes in Windows absolute policies are path
    // separators, not glob escapes; forward-slash patterns remain supported.
    let negations = value.len() - value.trim_start_matches('!').len();
    let prefix = &value[..negations];
    let body = &value[negations..];
    let windows_root =
        supplied_root.starts_with(r"\\") || supplied_root.as_bytes().get(1) == Some(&b':');
    let value = spelling(&if windows_root {
        body.replace('\\', "/")
    } else {
        body.to_owned()
    });
    let canonical_root = spelling(canonical_root);
    for root in [spelling(supplied_root), canonical_root.clone()] {
        if let Some(suffix) = suffix_at_root(&value, &root) {
            return format!(
                "{prefix}{}{suffix}",
                literal(canonical_root.trim_end_matches('/'))
            );
        }
    }

    // Resolve the existing literal prefix, never a wildcard expression. This
    // handles aliases such as macOS /var -> /private/var even when the session
    // supplied an already canonical root. New literal directories are appended
    // to their canonical existing ancestor, so policies also protect creation.
    // This reads metadata, not file data; containment was checked separately.
    let first_glob = value.find(['*', '?', '[', '{']);
    let end = first_glob.map_or(value.len(), |i| value[..i].rfind('/').unwrap_or(0));
    if end > 0 && Path::new(&value[..end]).is_absolute() {
        if let Some(resolved) = canonical_literal_prefix(Path::new(&value[..end])) {
            return format!(
                "{prefix}{}{}",
                literal(&spelling(&resolved.to_string_lossy())),
                &value[end..]
            );
        }
    }
    format!("{prefix}{value}")
}

pub(super) fn matches(value: &str, path: &Path, supplied_root: &Path, root: &Path) -> bool {
    matches_spelling(
        value,
        &path.to_string_lossy(),
        &supplied_root.to_string_lossy(),
        &root.to_string_lossy(),
    )
}

fn matches_spelling(value: &str, path: &str, supplied_root: &str, root: &str) -> bool {
    glob_match::glob_match(&pattern(value, supplied_root, root), &spelling(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p0_002b_policy_spelling_is_portable() {
        for (supplied, canonical, policy, checked) in [
            (
                "/tmp/work",
                "/tmp/work",
                "/tmp/work/**/*.env",
                "/tmp/work/secret.env",
            ),
            ("/", "/", "/**/*.env", "/tmp/secret.env"),
            (r"C:\", r"\\?\C:\", r"C:\**\*.env", r"\\?\C:\secret.env"),
            (
                "/var/folders/work",
                "/private/var/folders/work",
                "/var/folders/work/**/*.env",
                "/private/var/folders/work/secret.env",
            ),
            (
                r"C:\Users\runner\work",
                r"\\?\C:\Users\runner\work",
                r"C:\Users\runner\work/**/*.env",
                r"\\?\C:\Users\runner\work\secret.env",
            ),
            (
                r"C:\work",
                r"\\?\C:\work",
                r"C:\work\**\*.env",
                r"\\?\C:\work\nested\secret.env",
            ),
            (
                r"C:\work",
                r"\\?\C:\work",
                "c:/work/**/*.env",
                r"\\?\C:\work\secret.env",
            ),
            (
                r"C:\work",
                r"\\?\C:\work",
                r"\\?\C:\work\**\*.env",
                r"\\?\C:\work\secret.env",
            ),
            (
                r"C:\work",
                r"\\?\C:\work",
                r"**\*.env",
                r"\\?\C:\work\secret.env",
            ),
            (
                r"\\server\share\work",
                r"\\?\UNC\server\share\work",
                r"\\server\share\work\**\*.env",
                r"\\?\UNC\server\share\work\secret.env",
            ),
            (
                "/var/work[1]",
                "/private/var/work[1]",
                "/var/work[1]/**/*.env",
                "/private/var/work[1]/secret.env",
            ),
        ] {
            assert!(
                matches_spelling(policy, checked, supplied, canonical),
                "{policy} vs {checked}"
            );
            assert!(!matches_spelling(
                policy,
                &checked.replace(".env", ".txt"),
                supplied,
                canonical
            ));
        }
        assert!(!matches_spelling(
            "/var/work-evil/**",
            "/private/var/work/file",
            "/var/work",
            "/private/var/work"
        ));
        assert!(matches_spelling(
            "**/*.env",
            "/tmp/work/secret.env",
            "/tmp/work",
            "/tmp/work"
        ));
        assert!(!matches_spelling(
            "!/tmp/work/**/*.env",
            "/tmp/work/secret.env",
            "/tmp/work",
            "/tmp/work"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn p0_002b_policy_literal_prefix_resolves_real_aliases() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("real");
        let alias = temp.path().join("alias");
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let policy = format!("{}/**/*.env", alias.display());
        assert!(matches(&policy, &root.join("secret.env"), &root, &root));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn p0_002b_real_tools_enforce_policy_through_root_aliases() {
        use crate::tools::{create_tool, ToolContext};
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("real");
        let alias = temp.path().join("alias");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("secret.env"), "original").unwrap();
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        for supplied_root in [&alias, &root] {
            let mut ctx = ToolContext {
                working_dir: supplied_root.clone(),
                blocked_paths: vec![],
                max_file_scope: None,
                non_interactive: true,
            };
            let input = serde_json::json!({"path":"secret.env", "content":"changed", "old_text":"original", "new_text":"changed"});
            let positive = create_tool("file_read")
                .unwrap()
                .execute(input.clone(), &ctx)
                .await;
            assert!(positive.success && positive.output.contains("original"));
            ctx.blocked_paths
                .push(format!("{}/**/*.env", alias.display()));
            for name in ["file_read", "file_write", "file_edit"] {
                let result = create_tool(name)
                    .unwrap()
                    .execute(input.clone(), &ctx)
                    .await;
                assert!(
                    !result.success && result.filesystem_error().is_some(),
                    "{name}: {}",
                    result.output
                );
                assert_eq!(std::fs::read(root.join("secret.env")).unwrap(), b"original");
                assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
            }
            ctx.blocked_paths = vec![format!("{}/missing/**/*.env", alias.display())];
            let result = create_tool("file_write")
                .unwrap()
                .execute(
                    serde_json::json!({"path":"missing/nested/secret.env", "content":"changed"}),
                    &ctx,
                )
                .await;
            assert!(
                !result.success && result.filesystem_error().is_some(),
                "{}",
                result.output
            );
            assert!(!root.join("missing").exists());
            assert_eq!(std::fs::read(root.join("secret.env")).unwrap(), b"original");
        }
    }
}
