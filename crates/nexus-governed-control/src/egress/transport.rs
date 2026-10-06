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
use zeroize::{Zeroize, Zeroizing};

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
    /// The token itself (in `value`, after any scheme word), so that it is
    /// redacted wherever it comes back alone.
    pub token: Zeroizing<String>,
}

/// Every form in which a released credential could come back: the header
/// value, the bare token, and the token JSON-escaped and percent-encoded
/// (upper- and lower-case hex), longest first. Forms shorter than four bytes
/// are skipped. Each form is written straight into its own zeroized buffer,
/// sized up front so that it never grows (a growing buffer leaves copies
/// behind), and a duplicate form is dropped, zeroized.
fn needles(secret: &SecretHeader) -> Vec<Zeroizing<String>> {
    use std::fmt::Write as _;
    let token = secret.token.as_str();
    let mut json = Zeroizing::new(String::with_capacity(token.len() * 2));
    for c in token.chars() {
        if matches!(c, '"' | '\\' | '/') {
            json.push('\\');
        }
        json.push(c);
    }
    let percent = |upper: bool| {
        let mut out = Zeroizing::new(String::with_capacity(token.len() * 3));
        for b in token.bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                out.push(char::from(b));
            } else if upper {
                let _ = write!(out, "%{b:02X}");
            } else {
                let _ = write!(out, "%{b:02x}");
            }
        }
        out
    };
    let mut forms: Vec<Zeroizing<String>> = Vec::with_capacity(5);
    for form in [
        Zeroizing::new(secret.value.to_string()),
        Zeroizing::new(token.to_string()),
        json,
        percent(true),
        percent(false),
    ] {
        if form.len() >= 4 && !forms.iter().any(|kept| kept.as_str() == form.as_str()) {
            forms.push(form);
        }
    }
    forms.sort_by_key(|form| std::cmp::Reverse(form.len()));
    forms
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
    let redact_with = secret.as_ref().map(needles);
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
        // A peer the client cannot name is not known to be a pinned one.
        if !peer.is_some_and(|peer| addresses.iter().any(|a| a.ip() == peer.ip())) {
            return Err(TransportError::Unpinned);
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
        // The body may echo a released credential: it only ever lives in
        // buffers zeroized when they are dropped (outgrown, or on any early
        // exit, a cancellation included).
        let mut body = Zeroizing::new(Vec::with_capacity(
            response
                .content_length()
                .map_or(INITIAL_BODY, |length| length as usize)
                .min(max_body),
        ));
        while let Some(chunk) = response.chunk().await.map_err(classify)? {
            if body.len() + chunk.len() > max_body {
                return Err(TransportError::Bounds);
            }
            append(&mut body, &chunk, max_body);
        }
        Ok(HttpResponse {
            status,
            peer,
            content_type,
            location,
            body: std::mem::take(&mut *body),
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

/// A response body's first buffer when its length is not announced.
const INITIAL_BODY: usize = 64 * 1024;

/// Append `chunk`, never letting the vector reallocate in place: an
/// outgrown buffer is copied into a larger one and dropped, zeroized.
fn append(body: &mut Zeroizing<Vec<u8>>, chunk: &[u8], max_body: usize) {
    let needed = body.len() + chunk.len();
    if needed > body.capacity() {
        let mut grown = Zeroizing::new(Vec::with_capacity(
            needed
                .max(body.capacity().saturating_mul(2))
                .min(max_body.max(needed)),
        ));
        grown.extend_from_slice(body);
        *body = grown;
    }
    body.extend_from_slice(chunk);
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

/// Remove every occurrence of a released credential, in any of its forms,
/// from everything that came back: the body, the location and the content
/// type.
fn redact(response: &mut HttpResponse, forms: &[Zeroizing<String>]) {
    for form in forms {
        redact_one(response, form);
    }
}

fn redact_one(response: &mut HttpResponse, secret: &str) {
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
        // What came back held the credential: zeroized as it is replaced.
        std::mem::replace(&mut response.body, out).zeroize();
        response.redacted = true;
    }
    for field in [&mut response.location, &mut response.content_type] {
        if let Some(text) = field.as_deref().filter(|text| text.contains(secret)) {
            let redacted = text.replace(secret, "[redacted]");
            if let Some(mut held) = field.replace(redacted) {
                held.zeroize();
            }
            response.redacted = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every form a credential could come back in is kept once (a token
    /// whose escaped forms equal it is not kept three times), longest first.
    #[test]
    fn each_form_of_a_credential_is_kept_once_longest_first() {
        let secret = |value: &str, token: &str| SecretHeader {
            name: HeaderName::from_static("authorization"),
            value: Zeroizing::new(value.into()),
            token: Zeroizing::new(token.into()),
        };
        let forms = |secret: &SecretHeader| -> Vec<String> {
            needles(secret).iter().map(|f| f.to_string()).collect()
        };
        assert_eq!(
            forms(&secret("Bearer abc_DEF-123", "abc_DEF-123")),
            ["Bearer abc_DEF-123", "abc_DEF-123"]
        );
        assert_eq!(
            forms(&secret("Bearer a/b+c", "a/b+c")),
            ["Bearer a/b+c", "a%2Fb%2Bc", "a%2fb%2bc", "a\\/b+c", "a/b+c"]
        );
    }

    /// A body grows only into fresh buffers (the outgrown one is dropped,
    /// zeroized), never by reallocating in place, and asks for no more than
    /// its bound.
    #[test]
    fn a_body_grows_only_into_fresh_buffers() {
        let mut body = Zeroizing::new(Vec::with_capacity(4));
        let first = body.as_ptr();
        append(&mut body, b"abc", 64);
        append(&mut body, b"d", 64);
        assert_eq!((body.as_ptr(), body.capacity()), (first, 4), "it fitted");
        append(&mut body, b"efgh", 64);
        assert_ne!(body.as_ptr(), first, "a fresh buffer");
        assert_eq!(body.capacity(), 8);
        assert_eq!(&body[..], b"abcdefgh");
        append(&mut body, &[b'x'; 50], 60);
        assert_eq!(body.len(), 58);
        assert!(body.capacity() <= 60, "{}", body.capacity());
    }

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

    /// A released credential is redacted from everything that comes back,
    /// in every form: the header value, the bare token, and the token
    /// JSON-escaped or percent-encoded.
    #[test]
    fn released_credentials_are_redacted_from_responses() {
        let header = SecretHeader {
            name: HeaderName::from_static("authorization"),
            value: Zeroizing::new("Bearer tok/SECRET+123".into()),
            token: Zeroizing::new("tok/SECRET+123".into()),
        };
        assert!(!format!("{header:?}").contains("SECRET"));
        let mut response = HttpResponse {
            status: 302,
            peer: None,
            content_type: Some("text/plain; token=tok/SECRET+123".into()),
            location: Some("https://x.example/?t=tok%2FSECRET%2B123&u=tok%2fSECRET%2b123".into()),
            body: b"{\"auth\": \"Bearer tok/SECRET+123\", \"token\": \"tok\\/SECRET+123\", \"raw\": \"tok/SECRET+123\"}"
                .to_vec(),
            redacted: false,
        };
        redact(&mut response, &needles(&header));
        assert!(response.redacted);
        let body = String::from_utf8(response.body.clone()).unwrap();
        assert!(!body.contains("SECRET"), "{body}");
        assert_eq!(
            body,
            "{\"auth\": \"[redacted]\", \"token\": \"[redacted]\", \"raw\": \"[redacted]\"}"
        );
        assert_eq!(
            response.location.as_deref(),
            Some("https://x.example/?t=[redacted]&u=[redacted]")
        );
        assert_eq!(
            response.content_type.as_deref(),
            Some("text/plain; token=[redacted]")
        );
    }
}
