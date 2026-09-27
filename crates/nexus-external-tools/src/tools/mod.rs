pub mod database;
pub mod email;
pub mod file_storage;
pub mod github;
pub mod jira;
pub mod rest_api;
pub mod slack;
pub mod web_search;
pub mod webhook;

use crate::adapter::ToolError;

/// P0-002C5C: a caller value that becomes part of a tool request's URL must
/// match its identifier grammar. It can then name a resource of the tool's
/// fixed service, but never another path, query parameter or host for the
/// tool's credentials.
pub(crate) fn url_identifier<'a>(
    what: &str,
    value: &'a str,
    valid: fn(&str) -> bool,
) -> Result<&'a str, ToolError> {
    if valid(value) {
        Ok(value)
    } else {
        Err(ToolError::InvalidParameters(format!(
            "{what} has an invalid form"
        )))
    }
}

/// A GitHub login: letters, digits and inner hyphens, at most 39 bytes.
pub(crate) fn github_login(value: &str) -> bool {
    (1..=39).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}

/// A GitHub repository, `owner/name`.
pub(crate) fn github_repo(value: &str) -> bool {
    let Some((owner, name)) = value.split_once('/') else {
        return false;
    };
    github_login(owner)
        && (1..=100).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        && name != "."
        && name != ".."
}

/// A Slack conversation ID, such as `C024BE91L`.
pub(crate) fn slack_channel_id(value: &str) -> bool {
    (1..=32).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// A Jira project key, such as `PROJ`.
pub(crate) fn jira_project_key(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value.starts_with(|c: char| c.is_ascii_uppercase())
        && value
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

/// A Jira issue key, such as `PROJ-123`.
pub(crate) fn jira_issue_key(value: &str) -> bool {
    value.rsplit_once('-').is_some_and(|(project, number)| {
        jira_project_key(project)
            && (1..=18).contains(&number.len())
            && number.bytes().all(|b| b.is_ascii_digit())
    })
}

/// An S3 bucket name: 3-63 lowercase letters, digits, `.` and `-`, starting
/// and ending with a letter or digit, with no `..`.
pub(crate) fn s3_bucket(value: &str) -> bool {
    (3..=63).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-'))
        && value.starts_with(|c: char| c.is_ascii_alphanumeric())
        && value.ends_with(|c: char| c.is_ascii_alphanumeric())
        && !value.contains("..")
}

/// An S3 object key of `/`-separated segments of safe characters, with no
/// empty, `.` or `..` segment.
pub(crate) fn s3_key(value: &str) -> bool {
    (1..=1024).contains(&value.len())
        && value.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment.bytes().all(|b| {
                    b.is_ascii_alphanumeric()
                        || matches!(b, b'.' | b'-' | b'_' | b'!' | b'*' | b'\'' | b'(' | b')')
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p0_002c5c_url_identifiers_name_one_resource() {
        for (valid, good, bad) in [
            (
                github_login as fn(&str) -> bool,
                &["octocat", "a-b", "A1"][..],
                &[
                    "",
                    "-a",
                    "a-",
                    "a/b",
                    "a?b",
                    "a#b",
                    "../x",
                    "a b",
                    &"a".repeat(40),
                ][..],
            ),
            (
                github_repo,
                &["octocat/Hello-World", "a/b.c_d-e", "a/.github"][..],
                &[
                    "octocat",
                    "a/..",
                    "a/.",
                    "../../user",
                    "a/b/c",
                    "a/b?x=",
                    "a/b#x",
                    "a/b%2F..",
                    "/a",
                    "a/",
                ][..],
            ),
            (
                slack_channel_id,
                &["C024BE91L"][..],
                &["", "C1&limit=1000", "C1?", "#general", "C 1"][..],
            ),
            (
                jira_project_key,
                &["PROJ", "AB_2"][..],
                &["", "proj", "1AB", "PROJ OR assignee=x", "PROJ&x=1"][..],
            ),
            (
                jira_issue_key,
                &["PROJ-123"][..],
                &[
                    "PROJ",
                    "PROJ-",
                    "PROJ-1a",
                    "../PROJ-1",
                    "PROJ-1/comment",
                    "PROJ-1?x",
                ][..],
            ),
            (
                s3_bucket,
                &["my-bucket", "a.b.c"][..],
                &[
                    "ab",
                    "My-Bucket",
                    "-ab",
                    "ab-",
                    "a..b",
                    "evil.example/x?",
                    "a_b",
                ][..],
            ),
            (
                s3_key,
                &["file.txt", "dir/sub/file-1.bin"][..],
                &[
                    "", "/abs", "a//b", "a/../b", "..", "a?b", "a#b", "a b", "a\\b",
                ][..],
            ),
        ] {
            for value in good {
                assert!(valid(value), "{value:?} should be accepted");
                assert!(url_identifier("value", value, valid).is_ok());
            }
            for value in bad {
                assert!(!valid(value), "{value:?} must be refused");
                assert!(url_identifier("value", value, valid).is_err());
            }
        }
    }
}
