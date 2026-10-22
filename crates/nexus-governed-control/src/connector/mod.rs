//! Connectors: typed operations over governed egress, with brokered
//! credentials.
//!
//! A connector is code: one fixed origin, and a list of typed operations,
//! each with its effect class, method, credential and a builder that turns
//! a typed input into the request path and body (rejecting anything else).
//! An operation runs only under the owner's connector grant naming it, as
//! a commitment through the control pipeline: R1 for reads, R2 (native
//! approval of the exact request) for anything that writes, sends or
//! changes. Its request goes through governed egress to the connector's
//! own origin only, and its credential through a broker lease. An
//! operation that is not in the catalog does not exist.
//!
//! A post (Slack, Discord) names its destination by an opaque id, which is
//! not enough for the owner to approve. Before it is proposed, its
//! destination is identified by reads of the connector's own API (fixed
//! origin, the connector's credential, bounded answers, no redirect),
//! each a governed R1 commitment of the same run under the post's own
//! grant; the identity they answer (immutable ids, readable names and
//! types, a direct message's peer) is shown to the owner and bound into
//! the post's target and parameters. Immediately before the post is sent,
//! under its own commitment, the same reads run again: a destination that
//! changed in any way fails the post (`target_changed`) and nothing is
//! sent. Every readable word comes from the API, never the input.

pub mod catalog;

use crate::authority::commitment::{CommitmentView, ExecutionGuard, FailureClass, TargetIdentity};
use crate::authority::effect::{CapabilityKind, EffectClass};
use crate::authority::evidence::escaped;
use crate::authority::ids::{AgentId, Digest, GrantId, LeaseId, RunId};
use crate::authority::policy::GrantScope;
use crate::authority::{Authority, AuthorityError};
use crate::broker::{CredentialBroker, CredentialSpec};
use crate::control::{Control, EffectOutput, PendingEffect, Preparation};
use crate::egress::destination::Destination;
use crate::egress::transport::Method;
use crate::egress::{Coverage, CredentialPlan, Egress, EgressIntent};
use crate::governed::NeverAsk;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

/// What an operation's builder produced from its typed input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationRequest {
    /// The path and query, starting with `/`.
    pub path: String,
    pub body: Option<String>,
    pub content_type: Option<&'static str>,
    /// What the owner reads about it (made plain before display).
    pub summary: Vec<String>,
}

/// One typed operation.
pub struct ConnectorOperation {
    pub id: &'static str,
    pub class: EffectClass,
    pub method: Method,
    pub credential: Option<CredentialSpec>,
    pub build: fn(&Value) -> Result<OperationRequest, AuthorityError>,
    /// For a post: how its destination is identified, bound and checked
    /// again.
    pub destination: Option<DestinationResolver>,
}

/// How a post's destination is identified: reads of the connector's own
/// API (operations of the same connector, with inputs built from the
/// post's), and how their answers become its identity. The post's input
/// names the destination by its immutable id; the readable words come from
/// the answers.
pub struct DestinationResolver {
    pub lookups: Lookups,
    pub identify: Identify,
}

/// The reads that identify a post's destination: (operation, input).
pub type Lookups = fn(&Value) -> Result<Vec<(&'static str, Value)>, AuthorityError>;
/// A destination's identity from the post's input and the reads' answers.
pub type Identify = fn(&Value, &[EffectOutput]) -> Result<DestinationIdentity, AuthorityError>;

/// A destination as the connector's API identified it: the plain lines the
/// owner reads (immutable ids and readable metadata), and a short form for
/// the commitment's target. Two identities are the same destination only
/// if every line is the same.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DestinationIdentity {
    pub lines: Vec<String>,
    pub display: String,
}

impl DestinationIdentity {
    fn digest(&self) -> Digest {
        let lines: Vec<&[u8]> = self.lines.iter().map(|line| line.as_bytes()).collect();
        Digest::of("nexus.p3.connector.destination.v1", &lines)
    }
}

/// A connector: one origin, its operations.
pub struct Connector {
    pub id: &'static str,
    /// `scheme://host[:port]`, canonical.
    pub origin: String,
    /// Only test fixtures reach loopback servers.
    pub allow_private: bool,
    pub operations: Vec<ConnectorOperation>,
}

/// An operation request, as data.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConnectorIntent {
    pub operation: String,
    #[serde(default)]
    pub input: Value,
}

/// How long a prepared operation may wait for authorization.
const OPERATION_TTL: Duration = Duration::from_secs(10 * 60);

/// The connector actuator.
pub struct Connectors {
    connectors: Vec<Connector>,
    egress: Arc<Egress>,
    broker: Arc<CredentialBroker>,
}

impl Connectors {
    pub(crate) fn new(
        connectors: Vec<Connector>,
        egress: Arc<Egress>,
        broker: Arc<CredentialBroker>,
    ) -> Self {
        Self {
            connectors,
            egress,
            broker,
        }
    }

    /// The production catalog.
    pub(crate) fn production(egress: Arc<Egress>, broker: Arc<CredentialBroker>) -> Self {
        Self::new(catalog::production(), egress, broker)
    }

    /// Every operation id, with its connector, class and method (display).
    pub fn operations(&self) -> Vec<(&'static str, &'static str, EffectClass, &'static str)> {
        self.connectors
            .iter()
            .flat_map(|c| {
                c.operations
                    .iter()
                    .map(move |op| (c.id, op.id, op.class, op.method.as_str()))
            })
            .collect()
    }

    /// One operation of `connector`.
    fn operation<'a>(
        connector: &'a Connector,
        id: &str,
    ) -> Result<&'a ConnectorOperation, AuthorityError> {
        connector
            .operations
            .iter()
            .find(|op| op.id == id)
            .ok_or(AuthorityError::Closed("no such connector operation"))
    }

    fn find(&self, operation: &str) -> Option<(&Connector, &ConnectorOperation)> {
        self.connectors.iter().find_map(|connector| {
            connector
                .operations
                .iter()
                .find(|op| op.id == operation)
                .map(|op| (connector, op))
        })
    }

    /// The scope the owner may grant: `operations` of `connector`, acting
    /// for `account` (display).
    pub fn grant_scope(
        &self,
        connector: &str,
        account: &str,
        operations: &[String],
    ) -> Result<GrantScope, AuthorityError> {
        let known = self
            .connectors
            .iter()
            .find(|c| c.id == connector)
            .ok_or(AuthorityError::Closed("no such connector"))?;
        if operations.is_empty() {
            return Err(AuthorityError::InvalidAction("no operation"));
        }
        let mut granted: Vec<String> = Vec::new();
        for operation in operations {
            if !known.operations.iter().any(|op| op.id == operation) {
                return Err(AuthorityError::Closed("no such connector operation"));
            }
            if !granted.contains(operation) {
                granted.push(operation.clone());
            }
        }
        let account = account.trim();
        if account.is_empty()
            || account.chars().count() > 128
            || !account.chars().all(|c| c.is_ascii_graphic())
        {
            return Err(AuthorityError::InvalidAction("account is not plain"));
        }
        Ok(GrantScope::Connector {
            connector: known.id.to_string(),
            account: account.to_string(),
            operations: granted,
        })
    }

    /// Propose an operation for `agent` in `run`. A post's destination is
    /// identified first, before anyone is asked to approve it: each read
    /// runs as a governed R1 commitment of this run, under the post's own
    /// grant; the post is then prepared bound to the identity they
    /// answered, and proposed.
    pub(crate) fn propose(
        &self,
        control: &Control,
        agent: &AgentId,
        run: RunId,
        intent: &ConnectorIntent,
    ) -> Result<CommitmentView, AuthorityError> {
        let authority = control.authority();
        let (connector, operation) = self
            .find(&intent.operation)
            .ok_or(AuthorityError::Closed("no such connector operation"))?;
        let destination = match &operation.destination {
            None => None,
            Some(resolver) => {
                let (grant, account) = covering_grant(authority, connector.id, operation.id)?;
                let mut answers = Vec::new();
                for (lookup, input) in (resolver.lookups)(&intent.input)? {
                    let read = Self::operation(connector, lookup)?;
                    let preparation = self.prepare_operation(
                        authority,
                        (agent, run),
                        (connector, read),
                        &input,
                        (grant, &account),
                        Some(operation.id),
                    )?;
                    let view = control.propose(agent, run, preparation)?;
                    // A read never asks the owner (and one that would is
                    // declined).
                    control.authorize(view.id, agent, run, &NeverAsk)?;
                    answers.push(control.execute(view.id, agent, run)?);
                }
                Some((resolver.identify)(&intent.input, &answers)?)
            }
        };
        let preparation = self.prepare(authority, agent, run, intent, destination)?;
        control.propose(agent, run, preparation)
    }

    /// Prepare an operation for `agent` in `run`; a post with the identity
    /// of its destination (see `propose`).
    pub(crate) fn prepare(
        &self,
        authority: &Authority,
        agent: &AgentId,
        run: RunId,
        intent: &ConnectorIntent,
        destination: Option<DestinationIdentity>,
    ) -> Result<Preparation, AuthorityError> {
        let (connector, operation) = self
            .find(&intent.operation)
            .ok_or(AuthorityError::Closed("no such connector operation"))?;
        let (grant, account) = covering_grant(authority, connector.id, operation.id)?;
        match (&operation.destination, destination) {
            (None, None) => self.prepare_operation(
                authority,
                (agent, run),
                (connector, operation),
                &intent.input,
                (grant, &account),
                None,
            ),
            (Some(resolver), Some(identity)) => self.prepare_bound(
                authority,
                (agent, run),
                (connector, operation, resolver),
                &intent.input,
                (grant, &account),
                identity,
            ),
            (Some(_), None) => Err(AuthorityError::Closed(
                "a post's destination is identified before it is prepared",
            )),
            (None, Some(_)) => Err(AuthorityError::InvalidAction(
                "this operation has no destination to bind",
            )),
        }
    }

    /// A post bound to its destination's identity: the post itself, and the
    /// same reads to run again immediately before it, under its commitment
    /// (each with its own lease, all listed by it).
    fn prepare_bound(
        &self,
        authority: &Authority,
        (agent, run): (&AgentId, RunId),
        (connector, operation, resolver): (&Connector, &ConnectorOperation, &DestinationResolver),
        input: &Value,
        (grant, account): (GrantId, &str),
        identity: DestinationIdentity,
    ) -> Result<Preparation, AuthorityError> {
        let post = self.prepare_operation(
            authority,
            (agent, run),
            (connector, operation),
            input,
            (grant, account),
            None,
        )?;
        let mut leases: Vec<LeaseId> = post.action.leases.clone();
        let mut reads = Vec::new();
        let built: Result<(), AuthorityError> = (|| {
            for (lookup, lookup_input) in (resolver.lookups)(input)? {
                let read = Self::operation(connector, lookup)?;
                let preparation = self.prepare_operation(
                    authority,
                    (agent, run),
                    (connector, read),
                    &lookup_input,
                    (grant, account),
                    Some(operation.id),
                )?;
                leases.extend(preparation.action.leases.iter().copied());
                reads.push(preparation);
            }
            Ok(())
        })();
        if let Err(error) = built {
            // Nothing will list these leases.
            for lease in leases {
                self.broker.end_lease(lease);
            }
            return Err(error);
        }
        let destination = identity.digest();
        let mut action = post.action;
        let mut parts: Vec<&[u8]> = vec![action.parameters.as_bytes(), destination.as_bytes()];
        parts.extend(
            reads
                .iter()
                .map(|read| read.action.parameters.as_bytes().as_slice()),
        );
        let parameters = Digest::of("nexus.p3.connector.bound.parameters.v1", &parts);
        let target = Digest::of(
            "nexus.p3.connector.bound.target.v1",
            &[action.target.digest.as_bytes(), destination.as_bytes()],
        );
        action.target = TargetIdentity {
            display: identity.display.clone(),
            digest: target,
        };
        action.parameters = parameters;
        action.leases = leases;
        // The operation, then where it goes, then the request.
        let rest = action.summary.split_off(1);
        action.summary.extend(identity.lines.iter().cloned());
        action.summary.extend(rest);
        let effect = BoundPost {
            reads: reads.into_iter().map(|read| read.effect).collect(),
            post: post.effect,
            input: input.clone(),
            identify: resolver.identify,
            identity,
            target,
            parameters,
        };
        Ok(Preparation {
            action,
            effect: Box::new(effect),
            ttl: OPERATION_TTL,
        })
    }

    /// One operation's request, prepared through governed egress to the
    /// connector's own origin, under `grant`. `for_post` names the post a
    /// read identifies the destination of.
    fn prepare_operation(
        &self,
        authority: &Authority,
        (agent, run): (&AgentId, RunId),
        (connector, operation): (&Connector, &ConnectorOperation),
        input: &Value,
        (grant, account): (GrantId, &str),
        for_post: Option<&'static str>,
    ) -> Result<Preparation, AuthorityError> {
        let request = (operation.build)(input)?;
        // The one authoritative check: the request parses to the
        // connector's own origin, whatever its path says.
        let origin = Destination::parse(&connector.origin)
            .map_err(|_| AuthorityError::Unavailable("connector origin is not valid"))?;
        let url = format!("{}{}", connector.origin.trim_end_matches('/'), request.path);
        let destination = Destination::parse(&url)
            .map_err(|_| AuthorityError::InvalidAction("operation url is not valid"))?;
        if !destination.same_origin(&origin) {
            return Err(AuthorityError::InvalidAction(
                "an operation stays on its connector's origin",
            ));
        }
        let credential = match operation.credential {
            Some(spec) => Some(CredentialPlan {
                lease: self
                    .broker
                    .lease(agent, run, spec, &origin, OPERATION_TTL)?,
                service: spec.service.to_string(),
                release: self.broker.clone(),
            }),
            None => None,
        };
        let headers = request
            .content_type
            .map(|kind| vec![("content-type".to_string(), kind.to_string())])
            .unwrap_or_default();
        let leased = credential.as_ref().map(|plan| plan.lease);
        let prepared = self.egress.prepare_with(
            authority,
            &EgressIntent {
                method: operation.method.as_str().to_string(),
                url,
                headers,
                body: request.body.clone(),
            },
            credential,
            Coverage::Connector {
                grant,
                allow_private: connector.allow_private,
                class: operation.class,
                operation: operation.id,
            },
        );
        let mut preparation = match prepared {
            Ok(preparation) => preparation,
            Err(error) => {
                // The lease was issued for this preparation only.
                if let Some(lease) = leased {
                    self.broker.end_lease(lease);
                }
                return Err(error);
            }
        };
        // What the owner reads: the operation and the connector (the
        // account is only the owner's label: the connector always uses its
        // one stored credential), its content in full, then the request.
        let mut summary = vec![escaped(&format!(
            "{} via {} (your label \"{}\")",
            operation.id, connector.id, account
        ))];
        if let Some(post) = for_post {
            summary.push(format!("To identify the destination of {post}"));
        }
        summary.extend(request.summary.iter().cloned());
        summary.extend(preparation.action.summary.iter().cloned());
        preparation.action.summary = summary;
        preparation.ttl = OPERATION_TTL;
        Ok(preparation)
    }
}

/// A post bound to its destination: the reads that identify it run again
/// immediately before it, under its own commitment, and it is sent only to
/// the very destination the owner approved.
struct BoundPost {
    reads: Vec<Box<dyn PendingEffect>>,
    post: Box<dyn PendingEffect>,
    input: Value,
    identify: Identify,
    identity: DestinationIdentity,
    target: Digest,
    parameters: Digest,
}

impl PendingEffect for BoundPost {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        // The connector's origin again, for the post and every read (each
        // pinned to the addresses checked now).
        self.post.revalidate()?;
        for read in &self.reads {
            read.revalidate()?;
        }
        Ok(self.target)
    }

    fn parameters(&self) -> Digest {
        self.parameters
    }

    fn execute(
        self: Box<Self>,
        guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        let BoundPost {
            reads,
            post,
            input,
            identify,
            identity,
            ..
        } = *self;
        let mut answers = Vec::new();
        for read in reads {
            answers.push(read.execute(guard)?);
        }
        let now = identify(&input, &answers).map_err(|error| {
            (
                FailureClass::TargetChanged,
                format!(
                    "the destination is no longer identified ({})",
                    error.class()
                ),
            )
        })?;
        if now != identity {
            return Err((
                FailureClass::TargetChanged,
                "the destination changed since it was approved; nothing was sent".into(),
            ));
        }
        post.execute(guard)
    }
}

/// The live connector grant naming this operation.
fn covering_grant(
    authority: &Authority,
    connector: &str,
    operation: &str,
) -> Result<(GrantId, String), AuthorityError> {
    authority
        .grants()
        .live_of(CapabilityKind::Connector)
        .into_iter()
        .find_map(|grant| match &grant.scope {
            GrantScope::Connector {
                connector: granted,
                account,
                operations,
            } if granted == connector && operations.iter().any(|op| op == operation) => {
                Some((grant.id, account.clone()))
            }
            _ => None,
        })
        .ok_or(AuthorityError::NoCoveringGrant)
}

/// Read one bounded, plain string field of a typed input.
pub fn text_field<'a>(
    input: &'a Value,
    name: &'static str,
    max: usize,
) -> Result<&'a str, AuthorityError> {
    let text = input
        .get(name)
        .and_then(Value::as_str)
        .ok_or(AuthorityError::InvalidAction("a required field is missing"))?;
    if text.chars().count() > max || text.chars().any(|c| c.is_control() && c != '\n') {
        return Err(AuthorityError::InvalidAction(
            "a field is not plain bounded text",
        ));
    }
    Ok(text)
}

/// Reject any field outside `allowed` (inputs are exact).
pub fn only_fields(input: &Value, allowed: &[&str]) -> Result<(), AuthorityError> {
    match input {
        Value::Object(map) if map.keys().all(|k| allowed.contains(&k.as_str())) => Ok(()),
        Value::Object(_) => Err(AuthorityError::InvalidAction("unexpected input field")),
        Value::Null if allowed.is_empty() => Ok(()),
        _ => Err(AuthorityError::InvalidAction("input is not an object")),
    }
}

#[cfg(test)]
mod tests;
