//! Caller- or model-supplied HTTP request parts, checked before they reach an
//! HTTP client or a curl command line (P0-002C5B).
//!
//! An untrusted URL, method, header or body never becomes curl syntax:
//! - the URL must parse as `http` or `https` with a host and no userinfo, and
//!   its normalized spelling (which cannot begin with `-`) is what callers
//!   pass on;
//! - methods come from an allowlist;
//! - header names are HTTP tokens (so never `@file`) and values carry no line
//!   breaks;
//! - curl invocations start with [`CURL_HTTP_ONLY`] or [`CURL_HTTPS_ONLY`]
//!   (no `~/.curlrc`, no URL globbing, no other protocol even on redirect),
//!   take bodies through `--data-raw` (never `-d`, which reads `@file`) or a
//!   fixed `@-` stdin marker, and put the URL after `--`.
//!
//! This validates the shape of a request, not where it may go: outbound
//! network policy is separate.

/// Why a request part was refused. Carries no request text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HttpDenied {
    #[error("URL must be an http or https URL with a host")]
    Url,
    #[error("unsupported HTTP method")]
    Method,
    #[error("invalid HTTP header")]
    Header,
}

/// The first curl arguments for any request built from untrusted parts: do
/// not read `~/.curlrc`, treat `[]{}` in the URL literally rather than as a
/// glob, and allow only HTTP(S) for the request and for every redirect.
pub const CURL_HTTP_ONLY: [&str; 6] = [
    "-q",
    "--globoff",
    "--proto",
    "=http,https",
    "--proto-redir",
    "=http,https",
];

/// [`CURL_HTTP_ONLY`] for a service that is reached only over HTTPS.
pub const CURL_HTTPS_ONLY: [&str; 6] = [
    "-q",
    "--globoff",
    "--proto",
    "=https",
    "--proto-redir",
    "=https",
];

/// Parses an `http` or `https` URL with a host and no userinfo (so a
/// `trusted.example@other.example` spelling cannot pass a prefix check). The
/// raw text may carry no whitespace or control characters; callers pass on
/// `url.as_str()`.
pub fn http_url(raw: &str) -> Result<url::Url, HttpDenied> {
    if raw.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(HttpDenied::Url);
    }
    let url = url::Url::parse(raw).map_err(|_| HttpDenied::Url)?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none_or(str::is_empty)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(HttpDenied::Url);
    }
    Ok(url)
}

/// An allowlisted HTTP method, in canonical upper case.
pub fn http_method(raw: &str) -> Result<&'static str, HttpDenied> {
    const METHODS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
    METHODS
        .into_iter()
        .find(|method| method.eq_ignore_ascii_case(raw))
        .ok_or(HttpDenied::Method)
}

/// A header whose name is an HTTP token and whose value has no line breaks
/// or NUL, rendered as one `-H` argument.
pub fn http_header(name: &str, value: &str) -> Result<String, HttpDenied> {
    let token = !name.is_empty()
        && name.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        });
    if !token || value.chars().any(|c| matches!(c, '\r' | '\n' | '\0')) {
        return Err(HttpDenied::Header);
    }
    Ok(format!("{name}: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_http_and_https_urls_with_a_host_pass() {
        for ok in [
            "http://example.com",
            "https://example.com/a?b=c#d",
            "HTTPS://Example.com:8443/x",
            "http://127.0.0.1:11434/api/tags",
            "http://[::1]:8080/",
        ] {
            let url = http_url(ok).unwrap();
            assert!(url.as_str().starts_with("http"), "{ok}");
        }
        for bad in [
            "",
            "-http://example.com",
            "--config=/etc/passwd",
            "-K /etc/passwd",
            "file:///etc/passwd",
            "FILE:/etc/passwd",
            "ftp://example.com",
            "gopher://example.com",
            "dict://example.com",
            "smtp://example.com",
            "ldap://example.com",
            "scp://example.com/x",
            "example.com",
            "/etc/passwd",
            "@/etc/passwd",
            "https:",
            " https://example.com",
            "https://example.com /x",
            "https://example.com\r\nX-Injected: 1",
            "https://exa\tmple.com",
            "https://api.example.com@evil.example/x",
            "https://user:secret@example.com/",
            "https://:secret@example.com/",
        ] {
            assert_eq!(http_url(bad), Err(HttpDenied::Url), "{bad:?}");
        }
        // The normalized spelling passed to curl never starts an option.
        let url = http_url("https://example.com/a/../b?q={1..9}").unwrap();
        assert_eq!(url.as_str(), "https://example.com/b?q={1..9}");
        assert!(CURL_HTTP_ONLY[0] == "-q" && CURL_HTTP_ONLY.contains(&"--globoff"));
        assert!(CURL_HTTPS_ONLY[0] == "-q" && CURL_HTTPS_ONLY.contains(&"=https"));
    }

    #[test]
    fn methods_come_from_an_allowlist() {
        assert_eq!(http_method("get"), Ok("GET"));
        assert_eq!(http_method("DELETE"), Ok("DELETE"));
        for bad in ["", "TRACE", "CONNECT", "GET / HTTP/1.1", "-K", "POST\r\n"] {
            assert_eq!(http_method(bad), Err(HttpDenied::Method), "{bad:?}");
        }
    }

    #[test]
    fn headers_are_tokens_without_line_breaks() {
        assert_eq!(
            http_header("Content-Type", "application/json").unwrap(),
            "Content-Type: application/json"
        );
        for (name, value) in [
            ("@/etc/passwd", "x"),
            ("", "x"),
            ("Bad Name", "x"),
            ("Bad:Name", "x"),
            ("X", "a\r\nInjected: 1"),
            ("X", "a\nb"),
            ("X", "a\0b"),
        ] {
            assert_eq!(
                http_header(name, value),
                Err(HttpDenied::Header),
                "{name:?}"
            );
        }
    }
}
