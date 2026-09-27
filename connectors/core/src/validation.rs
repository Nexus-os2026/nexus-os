use std::env;
use std::process::Command;

pub fn validate_anthropic_key(key: &str) -> bool {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return false;
    }

    let url = env::var("NEXUS_ANTHROPIC_VALIDATE_URL")
        .unwrap_or_else(|_| "https://api.anthropic.com/v1/models".to_string());
    let headers = vec![
        format!("x-api-key: {trimmed}"),
        "anthropic-version: 2023-06-01".to_string(),
    ];

    http_status_with_headers(url.as_str(), &headers).is_some_and(is_success_status)
}

pub fn validate_brave_key(key: &str) -> bool {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return false;
    }

    let url = env::var("NEXUS_BRAVE_VALIDATE_URL").unwrap_or_else(|_| {
        "https://api.search.brave.com/res/v1/web/search?q=nexus&count=1".to_string()
    });
    let headers = vec![format!("X-Subscription-Token: {trimmed}")];

    http_status_with_headers(url.as_str(), &headers).is_some_and(is_success_status)
}

pub fn validate_telegram_token(token: &str) -> bool {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return false;
    }

    let base = env::var("NEXUS_TELEGRAM_VALIDATE_BASE_URL")
        .unwrap_or_else(|_| "https://api.telegram.org".to_string());
    let url = format!("{base}/bot{trimmed}/getMe");

    let Some(status) = http_status_with_headers(url.as_str(), &[]) else {
        return false;
    };
    if !is_success_status(status) {
        return false;
    }

    let Some(body) = http_get_body(url.as_str(), &[]) else {
        return false;
    };
    body.contains("\"ok\":true")
}

/// The curl invocation shared by the validators (P0-002C5B): an http(s) URL
/// only, header lines without line breaks, and the URL after `--`.
fn curl_command(url: &str, headers: &[String], args: &[&str]) -> Option<Command> {
    let url = nexus_kernel::governed_http::http_url(url).ok()?;
    let mut command = Command::new("curl");
    command
        .args(nexus_kernel::governed_http::CURL_HTTP_ONLY)
        .args(args);
    for header in headers {
        let (name, value) = header.split_once(": ")?;
        let header = nexus_kernel::governed_http::http_header(name, value).ok()?;
        command.arg("-H").arg(header);
    }
    command.arg("--").arg(url.as_str());
    Some(command)
}

fn http_status_with_headers(url: &str, headers: &[String]) -> Option<u16> {
    let mut command = curl_command(
        url,
        headers,
        &[
            "-sS",
            "-L",
            "-m",
            "5",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
        ],
    )?;

    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
    status.parse::<u16>().ok()
}

fn http_get_body(url: &str, headers: &[String]) -> Option<String> {
    let mut command = curl_command(
        url,
        headers,
        &["-sS", "-L", "-m", "5", "--max-filesize", "1048576"],
    )?;

    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn is_success_status(status: u16) -> bool {
    (200..300).contains(&status)
}

#[cfg(test)]
mod tests {
    use super::curl_command;

    #[test]
    fn p0_002c5b_validators_build_curl_only_for_http_urls_and_clean_headers() {
        for url in [
            "file:///etc/passwd",
            "-K/etc/passwd",
            "@/etc/passwd",
            "https://a@example.com/",
        ] {
            assert!(curl_command(url, &[], &[]).is_none(), "{url:?}");
        }
        let injected = vec!["x-api-key: k\r\nX-Injected: 1".to_string()];
        assert!(curl_command("https://example.com/", &injected, &[]).is_none());
        let command = curl_command("https://example.com/v1", &["x-api-key: k".into()], &["-sS"])
            .expect("valid request");
        let args: Vec<_> = command.get_args().map(|a| a.to_string_lossy()).collect();
        assert_eq!(args.first().map(|a| a.as_ref()), Some("-q"));
        assert_eq!(
            args.iter()
                .rev()
                .take(2)
                .map(|a| a.as_ref())
                .collect::<Vec<_>>(),
            ["https://example.com/v1", "--"]
        );
    }
}
