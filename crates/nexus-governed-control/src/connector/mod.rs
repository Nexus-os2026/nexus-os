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

pub mod catalog;

use crate::authority::effect::{CapabilityKind, EffectClass};
use crate::authority::evidence::escaped;
use crate::authority::ids::{AgentId, GrantId, RunId};
use crate::authority::policy::GrantScope;
use crate::authority::{Authority, AuthorityError};
use crate::broker::{CredentialBroker, CredentialSpec};
use crate::control::Preparation;
use crate::egress::destination::Destination;
use crate::egress::transport::Method;
use crate::egress::{Coverage, CredentialPlan, Egress, EgressIntent};
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
    pub fn production(egress: Arc<Egress>, broker: Arc<CredentialBroker>) -> Self {
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

    /// Prepare an operation for `agent` in `run`.
    pub(crate) fn prepare(
        &self,
        authority: &Authority,
        agent: &AgentId,
        run: RunId,
        intent: &ConnectorIntent,
    ) -> Result<Preparation, AuthorityError> {
        let (connector, operation) = self
            .find(&intent.operation)
            .ok_or(AuthorityError::Closed("no such connector operation"))?;
        let (grant, account) = covering_grant(authority, connector.id, operation.id)?;
        let request = (operation.build)(&intent.input)?;
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
        summary.extend(request.summary.iter().cloned());
        summary.extend(preparation.action.summary.iter().cloned());
        preparation.action.summary = summary;
        preparation.ttl = OPERATION_TTL;
        Ok(preparation)
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
