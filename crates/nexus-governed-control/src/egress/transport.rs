//! The pinned HTTP exchange: one request, to exactly the checked addresses.
//!
//! No proxy from the environment, no redirect following (the actuator
//! decides about every hop), no cookie store, no referer, no ambient
//! credentials; the connection may only reach the addresses the destination
//! policy checked (and the peer is verified against them), the body is
//! bounded, the deadline is enforced and cancellation drops the exchange.
//! A credential released by the broker travels only in the one header it is
//! meant for, marked sensitive, and is redacted from anything the response
//! echoes back.

use super::destination::MAX_URL;
use crate::authority::run::CancelToken;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE, LOCATION};
use std::net::SocketAddr;
use std::time::Duration;
use url::Url;
use zeroize::Zeroizing;

/// What every governed request says it is.
pub const USER_AGENT: &str = "NexusOS-GovernedControl/1";

/// A request method. GET, HEAD and OPTIONS are safe (R1); the others change
/// something at the destination (R2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Head,
    Options,
    Post,
    Put,
    Patch,
    Delete,
}

impl Method {
    /// Exactly an upper-case method name.
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "GET" => Method::Get,
            "HEAD" => Method::Head,
            "OPTIONS" => Method::Options,
            "POST" => Method::Post,
            "PUT" => Method::Put,
            "PATCH" => Method::Patch,
            "DELETE" => Method::Delete,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Head => "HEAD",
            Method::Options => "OPTIONS",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }

    /// Reads without changing anything at the destination.
    pub fn is_safe(self) -> bool {
        matches!(self, Method::Get | Method::Head | Method::Options)
    }

    fn to_reqwest(self) -> reqwest::Method {
        match self {
            Method::Get => reqwest::Method::GET,
            Method::Head => reqwest::Method::HEAD,
            Method::Options => reqwest::Method::OPTIONS,
            Method::Post => reqwest::Method::POST,
            Method::Put => reqwest::Method::PUT,
            Method::Patch => reqwest::Method::PATCH,
            Method::Delete => reqwest::Method::DELETE,
        }
    }
}

/// A credential the broker released for exactly one exchange.
pub struct SecretHeader {
    pub name: HeaderName,
    pub value: Zeroizing<String>,
}

impl std::fmt::Debug for SecretHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretHeader({}: [redacted])", self.name)
    }
}

/// One request, pinned to checked addresses.
#[derive(Debug)]
pub struct PinnedRequest {
    pub method: Method,
    pub url: Url,
    /// The host name `addresses` were resolved for; `None` when the URL's
    /// host is an address.
    pub domain: Option<String>,
    pub addresses: Vec<SocketAddr>,
    pub headers: Vec<(HeaderName, HeaderValue)>,
    pub body: Option<Vec<u8>>,
    pub secret: Option<SecretHeader>,
    pub timeout: Duration,
    pub max_body: usize,
}

/// What came back (bounded).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub peer: Option<SocketAddr>,
    pub content_type: Option<String>,
    pub location: Option<String>,
    pub body: Vec<u8>,
    /// A released credential was found in the response and removed.
    pub redacted: bool,
}

/// Why an exchange failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportError {
    Connect,
    Timeout,
    /// The response was larger than allowed.
    Bounds,
    Cancelled,
    Protocol,
    /// The connection did not reach a checked address.
    Unpinned,
    Unavailable,
}

impl TransportError {
    pub fn as_str(self) -> &'static str {
        match self {
            TransportError::Connect => "connect",
            TransportError::Timeout => "timeout",
            TransportError::Bounds => "response too large",
            TransportError::Cancelled => "cancelled",
            TransportError::Protocol => "protocol",
            TransportError::Unpinned => "peer is not a checked address",
            TransportError::Unavailable => "transport unavailable",
        }
    }
}

/// The exchange, injectable for tests.
pub trait Transport: Send + Sync {
    fn exchange(
        &self,
        request: PinnedRequest,
        cancel: &CancelToken,
    ) -> Result<HttpResponse, TransportError>;
}

/// The real transport: `reqwest` over rustls, on a runtime of its own (so
/// it never blocks or nests inside a caller's runtime).
pub struct HttpTransport;

impl Transport for HttpTransport {
    fn exchange(
        &self,
        request: PinnedRequest,
        cancel: &CancelToken,
    ) -> Result<HttpResponse, TransportError> {
        let wait = request.timeout + Duration::from_secs(2);
        let cancel = cancel.clone();
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("nexus-p3-egress".into())
            .spawn(move || {
                let result = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime.block_on(exchange(request, cancel)),
                    Err(_) => Err(TransportError::Unavailable),
                };
                let _ = sender.send(result);
            })
            .map_err(|_| TransportError::Unavailable)?;
        match receiver.recv_timeout(wait) {
            Ok(result) => result,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(TransportError::Timeout),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err(TransportError::Unavailable)
            }
        }
    }
}

async fn exchange(
    request: PinnedRequest,
    cancel: CancelToken,
) -> Result<HttpResponse, TransportError> {
    let PinnedRequest {
        method,
        url,
        domain,
        addresses,
        headers,
        body,
        secret,
        timeout,
        max_body,
    } = request;
    if addresses.is_empty() {
        return Err(TransportError::Unpinned);
    }
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .referer(false)
        .https_only(url.scheme() == "https")
        .connect_timeout(timeout.min(Duration::from_secs(10)))
        .timeout(timeout)
        .pool_max_idle_per_host(0)
        .user_agent(USER_AGENT)
        .use_rustls_tls();
    if let Some(domain) = &domain {
        builder = builder.resolve_to_addrs(domain, &addresses);
    }
    let client = builder.build().map_err(|_| TransportError::Unavailable)?;
    let mut header_map = HeaderMap::new();
    for (name, value) in headers {
        header_map.append(name, value);
    }
    let redact_with = secret.as_ref().map(|s| s.value.clone());
    if let Some(secret) = secret {
        let mut value =
            HeaderValue::from_str(&secret.value).map_err(|_| TransportError::Protocol)?;
        value.set_sensitive(true);
        header_map.insert(secret.name, value);
    }
    let mut request = client.request(method.to_reqwest(), url).headers(header_map);
    if let Some(body) = body {
        request = request.body(body);
    }
    let work = async move {
        let mut response = request.send().await.map_err(classify)?;
        let peer = response.remote_addr();
        if let Some(peer) = peer {
            if !addresses.iter().any(|a| a.ip() == peer.ip()) {
                return Err(TransportError::Unpinned);
            }
        }
        let status = response.status().as_u16();
        let content_type = header_text(response.headers().get(CONTENT_TYPE), 128);
        let location = header_text(response.headers().get(LOCATION), MAX_URL);
        if response
            .content_length()
            .is_some_and(|length| length > max_body as u64)
        {
            return Err(TransportError::Bounds);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(classify)? {
            if body.len() + chunk.len() > max_body {
                return Err(TransportError::Bounds);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(HttpResponse {
            status,
            peer,
            content_type,
            location,
            body,
            redacted: false,
        })
    };
    let mut response = tokio::select! {
        result = work => result?,
        () = until_cancelled(&cancel) => return Err(TransportError::Cancelled),
    };
    if let Some(secret) = redact_with {
        redact(&mut response, &secret);
    }
    Ok(response)
}

async fn until_cancelled(cancel: &CancelToken) {
    while !cancel.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn classify(error: reqwest::Error) -> TransportError {
    if error.is_timeout() {
        TransportError::Timeout
    } else if error.is_connect() {
        TransportError::Connect
    } else {
        TransportError::Protocol
    }
}

fn header_text(value: Option<&HeaderValue>, max: usize) -> Option<String> {
    let text = value?.to_str().ok()?;
    (text.len() <= max).then(|| text.to_string())
}

/// Remove every occurrence of a released credential from what came back.
fn redact(response: &mut HttpResponse, secret: &str) {
    const MARK: &[u8] = b"[redacted]";
    if secret.len() < 4 {
        return;
    }
    let needle = secret.as_bytes();
    if response
        .body
        .windows(needle.len())
        .any(|window| window == needle)
    {
        let mut out = Vec::with_capacity(response.body.len());
        let mut i = 0;
        while i < response.body.len() {
            if response.body[i..].starts_with(needle) {
                out.extend_from_slice(MARK);
                i += needle.len();
            } else {
                out.push(response.body[i]);
                i += 1;
            }
        }
        response.body = out;
        response.redacted = true;
    }
    if let Some(location) = &response.location {
        if location.contains(secret) {
            response.location = Some(location.replace(secret, "[redacted]"));
            response.redacted = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn methods_are_exact_and_classed() {
        assert_eq!(Method::parse("GET"), Some(Method::Get));
        assert_eq!(Method::parse("get"), None);
        assert_eq!(Method::parse("CONNECT"), None);
        assert_eq!(Method::parse("TRACE"), None);
        for safe in ["GET", "HEAD", "OPTIONS"] {
            assert!(Method::parse(safe).unwrap().is_safe());
        }
        for unsafe_ in ["POST", "PUT", "PATCH", "DELETE"] {
            assert!(!Method::parse(unsafe_).unwrap().is_safe());
        }
    }

    #[test]
    fn released_credentials_are_redacted_from_responses() {
        let mut response = HttpResponse {
            status: 302,
            peer: None,
            content_type: None,
            location: Some("https://x.example/?t=tok-SECRET-123".into()),
            body: b"echo tok-SECRET-123 and tok-SECRET-123!".to_vec(),
            redacted: false,
        };
        redact(&mut response, "tok-SECRET-123");
        assert!(response.redacted);
        assert_eq!(response.body, b"echo [redacted] and [redacted]!");
        assert_eq!(
            response.location.as_deref(),
            Some("https://x.example/?t=[redacted]")
        );
        let header = SecretHeader {
            name: HeaderName::from_static("authorization"),
            value: Zeroizing::new("Bearer tok-SECRET-123".into()),
        };
        assert!(!format!("{header:?}").contains("SECRET"));
    }
}
