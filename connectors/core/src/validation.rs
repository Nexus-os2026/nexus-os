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

fn http_status_with_headers(url: &str, headers: &[String]) -> Option<u16> {
    let mut command = Command::new("curl");
    command.args([
        "-sS",
        "-L",
        "-m",
        "5",
        "-o",
        "/dev/null",
        "-w",
        "%{http_code}",
    ]);
    for header in headers {
        command.arg("-H").arg(header);
    }
    command.arg(url);

    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
    status.parse::<u16>().ok()
}

fn http_get_body(url: &str, headers: &[String]) -> Option<String> {
    let mut command = Command::new("curl");
    command.args(["-sS", "-L", "-m", "5"]);
    for header in headers {
        command.arg("-H").arg(header);
    }
    command.arg(url);

    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn is_success_status(status: u16) -> bool {
    (200..300).contains(&status)
}
