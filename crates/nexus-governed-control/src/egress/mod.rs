//! P3-B: governed egress.
//!
//! Every network request a Phase Three route makes goes through here: the
//! URL becomes a canonical [`destination::Destination`], the owner's egress
//! grant for exactly that origin and method must be live, the destination's
//! addresses are checked when the action is prepared and again immediately
//! before it runs (and the connection is pinned to the addresses checked
//! then), the request carries only allow-listed headers and a bounded body,
//! and the response is bounded. Safe methods (GET, HEAD, OPTIONS) are R1;
//! the others are R2 and need the owner's native approval of the exact
//! request. Redirects are followed only for safe requests without a
//! credential and only within the same origin, every hop checked again; any
//! other redirect is returned as data, not followed.

pub mod destination;
pub mod transport;

use crate::authority::commitment::{ExecutionGuard, FailureClass, PreparedAction, TargetIdentity};
use crate::authority::effect::{CapabilityKind, EffectClass};
use crate::authority::evidence::{bounded, escaped, quoted, wrapped};
use crate::authority::ids::{Digest, GrantId, LeaseId};
use crate::authority::policy::GrantScope;
use crate::authority::{Authority, AuthorityError};
use crate::control::{EffectOutput, PendingEffect, Preparation};
use destination::{
    resolve_checked, DestHost, Destination, DestinationError, Resolver, SystemResolver,
};
use reqwest::header::{HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use transport::{
    HttpResponse, HttpTransport, Method, PinnedRequest, SecretHeader, Transport, TransportError,
};

/// Bounds on every governed request.
#[derive(Clone, Copy, Debug)]
pub struct EgressLimits {
    pub timeout: Duration,
    pub max_response: usize,
    pub max_request_body: usize,
    pub max_redirects: u8,
}

impl Default for EgressLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(20),
            max_response: 2 * 1024 * 1024,
            max_request_body: 64 * 1024,
            max_redirects: 5,
        }
    }
}

/// A request, as data: what a planner, an agent action or a command asks
/// for. It carries no credential; credentials reach requests only through
/// connector operations and broker leases.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EgressIntent {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub body: Option<String>,
}

/// Headers a request may carry. Everything else (`Authorization`, `Cookie`,
/// `Proxy-*`, `Host`, custom `X-*` keys, ...) is refused.
const REQUEST_HEADERS: [&str; 6] = [
    "accept",
    "accept-language",
    "cache-control",
    "content-type",
    "if-modified-since",
    "if-none-match",
];
const MAX_HEADERS: usize = 8;
const MAX_HEADER_VALUE: usize = 256;
/// How long a prepared request may wait for authorization.
const REQUEST_TTL: Duration = Duration::from_secs(10 * 60);

/// Releases a leased credential to exactly the commitment executing under a
/// guard, for exactly one destination (the credential broker).
pub trait ReleaseCredential: Send + Sync {
    fn release(
        &self,
        lease: LeaseId,
        guard: &ExecutionGuard,
        destination: &Destination,
    ) -> Result<SecretHeader, AuthorityError>;
}

/// A leased credential a connector request will use.
pub(crate) struct CredentialPlan {
    pub lease: LeaseId,
    pub service: String,
    pub release: Arc<dyn ReleaseCredential>,
}

/// What authorizes a request.
pub(crate) enum Coverage {
    /// An owner egress grant for exactly the origin and method.
    EgressGrant,
    /// A connector operation: the owner's connector grant covers requests
    /// to the connector's own (code-defined) origin.
    Connector {
        grant: GrantId,
        allow_private: bool,
        class: EffectClass,
        operation: &'static str,
    },
}

/// The egress actuator.
pub struct Egress {
    resolver: Arc<dyn Resolver>,
    transport: Arc<dyn Transport>,
    limits: EgressLimits,
}

fn destination_error(error: DestinationError) -> AuthorityError {
    match error {
        DestinationError::NotPermitted => {
            AuthorityError::Closed("destination address not permitted")
        }
        DestinationError::Unresolvable => {
            AuthorityError::Unavailable("destination does not resolve")
        }
        other => AuthorityError::InvalidAction(other.as_str()),
    }
}

impl Egress {
    pub fn new(
        resolver: Arc<dyn Resolver>,
        transport: Arc<dyn Transport>,
        limits: EgressLimits,
    ) -> Self {
        Self {
            resolver,
            transport,
            limits,
        }
    }

    /// The system resolver and the real transport.
    pub fn system() -> Self {
        Self::new(
            Arc::new(SystemResolver),
            Arc::new(HttpTransport),
            EgressLimits::default(),
        )
    }

    pub fn resolver(&self) -> &Arc<dyn Resolver> {
        &self.resolver
    }

    /// The scope the owner may grant: requests with `methods` to exactly the
    /// origin `origin` (`scheme://host[:port]` with no path, query or user
    /// information). `allow_private` lets it reach private, loopback and
    /// link-local addresses; it is never the default.
    pub fn grant_scope(
        origin: &str,
        methods: &[String],
        allow_private: bool,
    ) -> Result<GrantScope, AuthorityError> {
        let destination = Destination::parse(origin).map_err(destination_error)?;
        if destination.url().path() != "/" || destination.url().query().is_some() {
            return Err(AuthorityError::InvalidAction(
                "an origin has no path or query",
            ));
        }
        let mut granted: Vec<String> = Vec::new();
        for method in methods {
            let method =
                Method::parse(method).ok_or(AuthorityError::InvalidAction("unknown method"))?;
            if !granted.iter().any(|m| m == method.as_str()) {
                granted.push(method.as_str().to_string());
            }
        }
        if granted.is_empty() {
            return Err(AuthorityError::InvalidAction("no method"));
        }
        Ok(GrantScope::Egress {
            scheme: destination.scheme().to_string(),
            host: destination.host().text(),
            port: destination.port(),
            methods: granted,
            allow_private,
        })
    }

    /// Prepare a request from an intent.
    pub fn prepare(
        &self,
        authority: &Authority,
        intent: &EgressIntent,
    ) -> Result<Preparation, AuthorityError> {
        self.prepare_with(authority, intent, None, Coverage::EgressGrant)
    }

    pub(crate) fn prepare_with(
        &self,
        authority: &Authority,
        intent: &EgressIntent,
        credential: Option<CredentialPlan>,
        coverage: Coverage,
    ) -> Result<Preparation, AuthorityError> {
        let method =
            Method::parse(&intent.method).ok_or(AuthorityError::InvalidAction("unknown method"))?;
        let destination = Destination::parse(&intent.url).map_err(destination_error)?;
        let headers = checked_headers(&intent.headers)?;
        let body = match &intent.body {
            Some(_) if method.is_safe() => {
                return Err(AuthorityError::InvalidAction("a safe request has no body"))
            }
            Some(body) if body.len() > self.limits.max_request_body => {
                return Err(AuthorityError::InvalidAction("request body too large"))
            }
            Some(body) => Some(body.as_bytes().to_vec()),
            None => None,
        };
        let method_class = if method.is_safe() {
            EffectClass::R1
        } else {
            EffectClass::R2
        };
        // A connector shows its own content (recipient, subject, text); the
        // body it sends is an encoding of exactly that.
        let from_connector = matches!(coverage, Coverage::Connector { .. });
        let (grant, allow_private, kind, class, operation) = match coverage {
            Coverage::EgressGrant => {
                let (grant, allow_private) = covering_grant(authority, &destination, method)?;
                (
                    grant,
                    allow_private,
                    CapabilityKind::Egress,
                    method_class,
                    "egress.request",
                )
            }
            // A connector operation is never classed below its method.
            Coverage::Connector {
                grant,
                allow_private,
                class,
                operation,
            } => (
                grant,
                allow_private,
                CapabilityKind::Connector,
                class.max(method_class),
                operation,
            ),
        };
        // Checked now so a refused destination fails early; checked again
        // immediately before the request.
        resolve_checked(&destination, allow_private, &*self.resolver).map_err(destination_error)?;
        let header_text: String = headers
            .iter()
            .map(|(name, value)| format!("{}:{}\n", name, value.to_str().unwrap_or_default()))
            .collect();
        let service = credential
            .as_ref()
            .map(|c| c.service.clone())
            .unwrap_or_default();
        let parameters = Digest::of(
            "nexus.p3.egress.request.v1",
            &[
                method.as_str().as_bytes(),
                destination.url().as_str().as_bytes(),
                header_text.as_bytes(),
                body.as_deref().unwrap_or_default(),
                service.as_bytes(),
            ],
        );
        let mut summary = wrapped(&format!("{} {}", method.as_str(), destination.url()));
        if !headers.is_empty() {
            let names: Vec<&str> = headers.iter().map(|(name, _)| name.as_str()).collect();
            summary.push(format!("Headers: {}", names.join(", ")));
        }
        if let Some(text) = &intent.body {
            if from_connector {
                summary.push(format!(
                    "Request body: {} bytes encoding the content above (digest {})",
                    text.len(),
                    Digest::of("nexus.p3.egress.body.v1", &[text.as_bytes()]).short()
                ));
            } else {
                summary.extend(quoted("Body", text));
            }
        }
        if let Some(plan) = &credential {
            summary.push(escaped(&format!(
                "Credential: {} (released once, only to this origin)",
                plan.service
            )));
        }
        let leases = credential.iter().map(|c| c.lease).collect();
        let action = PreparedAction {
            kind,
            class,
            operation,
            target: TargetIdentity {
                display: destination.origin_text(),
                digest: destination.origin_digest(),
            },
            parameters,
            grants: vec![grant],
            leases,
            summary,
        };
        let effect = EgressEffect {
            method,
            destination,
            headers,
            body,
            allow_private,
            parameters,
            resolver: self.resolver.clone(),
            transport: self.transport.clone(),
            limits: self.limits,
            credential,
            pinned: Mutex::new(None),
        };
        Ok(Preparation {
            action,
            effect: Box::new(effect),
            ttl: REQUEST_TTL,
        })
    }
}

/// The live egress grant that covers exactly this origin and method.
fn covering_grant(
    authority: &Authority,
    destination: &Destination,
    method: Method,
) -> Result<(GrantId, bool), AuthorityError> {
    let host = destination.host().text();
    authority
        .grants()
        .live_of(CapabilityKind::Egress)
        .into_iter()
        .find_map(|grant| match &grant.scope {
            GrantScope::Egress {
                scheme,
                host: granted_host,
                port,
                methods,
                allow_private,
            } if scheme == destination.scheme()
                && *granted_host == host
                && *port == destination.port()
                && methods.iter().any(|m| m == method.as_str()) =>
            {
                Some((grant.id, *allow_private))
            }
            _ => None,
        })
        .ok_or(AuthorityError::NoCoveringGrant)
}

fn checked_headers(
    headers: &[(String, String)],
) -> Result<Vec<(HeaderName, HeaderValue)>, AuthorityError> {
    if headers.len() > MAX_HEADERS {
        return Err(AuthorityError::InvalidAction("too many headers"));
    }
    let mut out: Vec<(HeaderName, HeaderValue)> = Vec::new();
    for (name, value) in headers {
        let lower = name.to_ascii_lowercase();
        if !REQUEST_HEADERS.contains(&lower.as_str()) {
            return Err(AuthorityError::InvalidAction("header not permitted"));
        }
        if value.len() > MAX_HEADER_VALUE || !value.bytes().all(|b| (0x20..0x7f).contains(&b)) {
            return Err(AuthorityError::InvalidAction("header value not plain"));
        }
        if out.iter().any(|(existing, _)| existing.as_str() == lower) {
            return Err(AuthorityError::InvalidAction("header repeated"));
        }
        let name = HeaderName::from_bytes(lower.as_bytes())
            .map_err(|_| AuthorityError::InvalidAction("header not permitted"))?;
        let value = HeaderValue::from_str(value)
            .map_err(|_| AuthorityError::InvalidAction("header value not plain"))?;
        out.push((name, value));
    }
    out.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    Ok(out)
}

struct EgressEffect {
    method: Method,
    destination: Destination,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: Option<Vec<u8>>,
    allow_private: bool,
    parameters: Digest,
    resolver: Arc<dyn Resolver>,
    transport: Arc<dyn Transport>,
    limits: EgressLimits,
    credential: Option<CredentialPlan>,
    /// The addresses checked by the last revalidation; the request goes to
    /// these and nowhere else.
    pinned: Mutex<Option<Vec<SocketAddr>>>,
}

fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

fn failure_of(error: TransportError) -> FailureClass {
    match error {
        TransportError::Timeout => FailureClass::Timeout,
        TransportError::Bounds => FailureClass::Bounds,
        TransportError::Cancelled => FailureClass::Actuator,
        TransportError::Unpinned => FailureClass::TargetChanged,
        TransportError::Unavailable => FailureClass::Unavailable,
        TransportError::Connect | TransportError::Protocol => FailureClass::Transport,
    }
}

impl PendingEffect for EgressEffect {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        let addresses = resolve_checked(&self.destination, self.allow_private, &*self.resolver)
            .map_err(|_| AuthorityError::TargetChanged)?;
        *self.pinned.lock().expect("pinned") = Some(addresses);
        Ok(self.destination.origin_digest())
    }

    fn parameters(&self) -> Digest {
        self.parameters
    }

    fn execute(
        self: Box<Self>,
        guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        let mut addresses = self.pinned.lock().expect("pinned").take().ok_or((
            FailureClass::TargetChanged,
            "the destination was not revalidated".to_string(),
        ))?;
        let mut destination = self.destination.clone();
        // A cancelled run releases nothing.
        if guard.is_cancelled() {
            return Err((FailureClass::Actuator, "cancelled".into()));
        }
        let mut secret = match &self.credential {
            Some(plan) => Some(
                plan.release
                    .release(plan.lease, guard, &destination)
                    .map_err(|error| (FailureClass::Unavailable, error.class().to_string()))?,
            ),
            None => None,
        };
        let follow = self.method.is_safe() && secret.is_none();
        let mut redirects = 0u8;
        loop {
            if guard.is_cancelled() {
                return Err((FailureClass::Actuator, "cancelled".into()));
            }
            let request = PinnedRequest {
                method: self.method,
                url: destination.url().clone(),
                domain: match destination.host() {
                    DestHost::Domain(domain) => Some(domain.clone()),
                    DestHost::Ip(_) => None,
                },
                addresses: addresses.clone(),
                headers: self.headers.clone(),
                body: self.body.clone(),
                secret: secret.take(),
                timeout: self.limits.timeout,
                max_body: self.limits.max_response,
            };
            let response = self
                .transport
                .exchange(request, guard.cancel_token())
                .map_err(|error| (failure_of(error), error.as_str().to_string()))?;
            if follow && is_redirect(response.status) && redirects < self.limits.max_redirects {
                let next = response
                    .location
                    .as_deref()
                    .and_then(|location| destination.join(location).ok())
                    .filter(|next| next.same_origin(&destination));
                if let Some(next) = next {
                    addresses = resolve_checked(&next, self.allow_private, &*self.resolver)
                        .map_err(|error| {
                            (FailureClass::TargetChanged, error.as_str().to_string())
                        })?;
                    destination = next;
                    redirects += 1;
                    continue;
                }
            }
            return Ok(output(&destination, response, redirects));
        }
    }
}

fn path_digest(destination: &Destination) -> String {
    let url = destination.url();
    let path = match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_string(),
    };
    Digest::of("nexus.p3.egress.path.v1", &[path.as_bytes()]).short()
}

fn output(destination: &Destination, response: HttpResponse, redirects: u8) -> EffectOutput {
    let textual = response.content_type.as_deref().is_none_or(|kind| {
        let kind = kind.to_ascii_lowercase();
        kind.starts_with("text/")
            || kind.contains("json")
            || kind.contains("xml")
            || kind.contains("javascript")
    });
    let mut meta = vec![
        ("status".to_string(), response.status.to_string()),
        ("origin".to_string(), destination.origin_text()),
        // The path and query only as a digest: a URL may carry a token, and
        // this record is permanent.
        ("path_digest".to_string(), path_digest(destination)),
        ("bytes".to_string(), response.body.len().to_string()),
        ("redirects".to_string(), redirects.to_string()),
    ];
    if let Some(kind) = &response.content_type {
        meta.push(("content_type".to_string(), bounded(kind)));
    }
    if is_redirect(response.status) {
        // Where it pointed, without its query (which may carry codes).
        if let Some(next) = response
            .location
            .as_deref()
            .and_then(|location| destination.join(location).ok())
        {
            meta.push((
                "redirect_not_followed".to_string(),
                format!(
                    "{} (path digest {})",
                    next.origin_text(),
                    path_digest(&next)
                ),
            ));
        }
    }
    if response.redacted {
        meta.push(("credential_redacted".to_string(), "true".to_string()));
    }
    let (text, bytes) = if textual {
        (
            Some(String::from_utf8_lossy(&response.body).into_owned()),
            None,
        )
    } else {
        (None, Some(response.body))
    };
    EffectOutput { text, bytes, meta }
}

#[cfg(test)]
mod tests;
