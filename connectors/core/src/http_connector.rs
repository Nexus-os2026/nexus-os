use crate::challenge::{detect_challenge, handle_challenge, ChallengeType};
use crate::connector::{Connector, HealthStatus, RetryPolicy};
use crate::idempotency::IdempotencyManager;
use crate::rate_limit::{RateLimitDecision, RateLimiter};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::lifecycle::AgentState;
use nexus_kernel::secrets::SecretError;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status_code: u16,
    pub body: String,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct AuthBinding {
    secret_name: String,
}

pub struct HttpConnector {
    id: String,
    name: String,
    required_capabilities: Vec<String>,
    retry_policy: RetryPolicy,
    degrade_gracefully: bool,
    pub rate_limiter: RateLimiter,
    pub idempotency: IdempotencyManager,
    pub audit_trail: AuditTrail,
    agent_id: Uuid,
    agent_state: AgentState,
    auth_binding: Option<AuthBinding>,
}

impl HttpConnector {
    pub fn new(id: &str, name: &str, agent_id: Uuid) -> Self {
        let limiter = RateLimiter::new();
        limiter.configure(id, 100, 60);

        Self {
            id: id.to_string(),
            name: name.to_string(),
            required_capabilities: vec!["net.outbound".to_string()],
            retry_policy: RetryPolicy {
                max_retries: 3,
                backoff_ms: 200,
                backoff_multiplier: 2.0,
            },
            degrade_gracefully: true,
            rate_limiter: limiter,
            idempotency: IdempotencyManager::new(300),
            audit_trail: AuditTrail::new(),
            agent_id,
            agent_state: AgentState::Running,
            auth_binding: None,
        }
    }

    pub fn set_rate_limit(&self, max_requests: usize, window_seconds: u64) {
        self.rate_limiter
            .configure(self.id.as_str(), max_requests, window_seconds);
    }

    /// Bug AK Commit 5: the legacy connector-local secrets module
    /// is retired. Auth-secret resolution now goes through the
    /// kernel `SecretsFacade` (scope `"http"`), so the per-call
    /// user-key argument is gone — the facade owns its master
    /// key. Caller passes the secret name only; lookup happens
    /// in `build_headers` at request time via
    /// `kernel::secrets::global::try_facade()`.
    pub fn bind_auth_secret(&mut self, secret_name: &str) {
        self.auth_binding = Some(AuthBinding {
            secret_name: secret_name.to_string(),
        });
    }

    pub fn get(
        &mut self,
        url: &str,
        headers: HashMap<String, String>,
    ) -> Result<HttpResponse, AgentError> {
        match self.rate_limiter.check(self.id.as_str()) {
            RateLimitDecision::Allowed => {}
            RateLimitDecision::RateLimited { retry_after_ms } => {
                return Err(AgentError::SupervisorError(format!(
                    "connector '{}' rate limited; retry after {} ms",
                    self.id, retry_after_ms
                )));
            }
        }

        let final_headers = self.build_headers(headers)?;
        let response = if url.to_lowercase().contains("captcha") {
            HttpResponse {
                status_code: 403,
                body: "<html>captcha required</html>".to_string(),
                headers: final_headers,
            }
        } else {
            HttpResponse {
                status_code: 200,
                body: json!({
                    "url": url,
                    "method": "GET",
                    "status": "ok"
                })
                .to_string(),
                headers: final_headers,
            }
        };

        self.log_http_event("GET", url, response.status_code, None);
        self.maybe_escalate_challenge(&response);

        Ok(response)
    }

    pub fn post(
        &mut self,
        url: &str,
        body: &str,
        headers: HashMap<String, String>,
    ) -> Result<HttpResponse, AgentError> {
        match self.rate_limiter.check(self.id.as_str()) {
            RateLimitDecision::Allowed => {}
            RateLimitDecision::RateLimited { retry_after_ms } => {
                return Err(AgentError::SupervisorError(format!(
                    "connector '{}' rate limited; retry after {} ms",
                    self.id, retry_after_ms
                )));
            }
        }

        let final_headers = self.build_headers(headers)?;

        let request_id = final_headers
            .get("x-request-id")
            .cloned()
            .unwrap_or_else(IdempotencyManager::generate_request_id);

        if let Some(cached_body) = self.idempotency.check_duplicate(request_id.as_str()) {
            let cached_response = HttpResponse {
                status_code: 200,
                body: cached_body,
                headers: final_headers,
            };
            self.log_http_event("POST", url, cached_response.status_code, Some(&request_id));
            return Ok(cached_response);
        }

        let response_body = json!({
            "url": url,
            "method": "POST",
            "request_id": request_id,
            "payload": body,
            "status": "created"
        })
        .to_string();

        self.idempotency
            .record_completion(request_id.as_str(), response_body.clone());

        let response = HttpResponse {
            status_code: 201,
            body: response_body,
            headers: final_headers,
        };

        self.log_http_event("POST", url, response.status_code, Some(&request_id));
        Ok(response)
    }

    fn build_headers(
        &mut self,
        mut headers: HashMap<String, String>,
    ) -> Result<HashMap<String, String>, AgentError> {
        if let Some(binding) = &self.auth_binding {
            // Bug AK Commit 5: lookup via the kernel SecretsFacade.
            // try_facade() returns None if startup wiring hasn't
            // run; surface as SupervisorError matching the prior
            // connector-local missing-creds shape.
            let facade = nexus_kernel::secrets::global::try_facade().ok_or_else(|| {
                AgentError::SupervisorError(format!(
                    "vault not initialized; cannot resolve secret '{}'",
                    binding.secret_name
                ))
            })?;
            let token = facade
                .get_secret("http", binding.secret_name.as_str())
                .map(|s| s.value.to_string())
                .map_err(|e| match e {
                    SecretError::NotFound => AgentError::SupervisorError(format!(
                        "secret '{}' not found",
                        binding.secret_name
                    )),
                    other => AgentError::SupervisorError(format!(
                        "vault read failed for '{}': {}",
                        binding.secret_name, other
                    )),
                })?;
            headers.insert("authorization".to_string(), format!("Bearer {token}"));
        }
        Ok(headers)
    }

    fn log_http_event(
        &mut self,
        method: &str,
        url: &str,
        status_code: u16,
        request_id: Option<&str>,
    ) {
        if let Err(e) = self.audit_trail.append_event(
            self.agent_id,
            EventType::ToolCall,
            json!({
                "event": "http_request",
                "connector_id": self.id,
                "method": method,
                "url": url,
                "status_code": status_code,
                "request_id": request_id
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }
    }

    fn maybe_escalate_challenge(&mut self, response: &HttpResponse) {
        if detect_challenge(response.body.as_str()).is_some() {
            let _ = handle_challenge(
                self.agent_id,
                response.body.as_str(),
                &mut self.agent_state,
                &mut self.audit_trail,
            );
        }
    }

    pub fn latest_challenge_type(&self, response: &HttpResponse) -> Option<ChallengeType> {
        detect_challenge(response.body.as_str())
    }
}

impl Connector for HttpConnector {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn required_capabilities(&self) -> Vec<String> {
        self.required_capabilities.clone()
    }

    fn health_check(&self) -> Result<HealthStatus, AgentError> {
        Ok(HealthStatus::Healthy)
    }

    fn retry_policy(&self) -> RetryPolicy {
        self.retry_policy.clone()
    }

    fn degrade_gracefully(&self) -> bool {
        self.degrade_gracefully
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bug AK Commit 5: bind_auth_secret no longer takes a
    /// per-call user-key arg. The facade global is not
    /// installed in this test — `build_headers` therefore
    /// returns `SupervisorError("vault not initialized; …")`,
    /// which matches the pre-Commit-5 missing-creds shape
    /// (the connector-local `secret 'X' not found` path)
    /// closely enough that callers handling
    /// `AgentError::SupervisorError` generically continue to
    /// work. End-to-end facade integration is covered by
    /// kernel::secrets::tests::ak_commit4_real_keyring_falls_through_to_memory
    /// (Commit 4) — that test runs the chain with a real
    /// SecretsFacade and validates resolve semantics; the
    /// http_connector slot is just a thin caller.
    #[test]
    fn bind_auth_secret_stores_name_only() {
        let mut conn = HttpConnector::new("http.test", "test", Uuid::new_v4());
        assert!(conn.auth_binding.is_none());
        conn.bind_auth_secret("github_token");
        let binding = conn.auth_binding.as_ref().expect("binding set");
        assert_eq!(binding.secret_name, "github_token");
    }

    #[test]
    fn build_headers_without_binding_returns_unchanged() {
        let mut conn = HttpConnector::new("http.test", "test", Uuid::new_v4());
        let mut hdrs = HashMap::new();
        hdrs.insert("x-test".to_string(), "v".to_string());
        let out = conn.build_headers(hdrs.clone()).expect("ok");
        assert_eq!(out, hdrs);
    }

    #[test]
    fn build_headers_with_binding_but_no_facade_surfaces_supervisor_error() {
        let mut conn = HttpConnector::new("http.test", "test", Uuid::new_v4());
        conn.bind_auth_secret("github_token");
        let err = conn
            .build_headers(HashMap::new())
            .expect_err("missing facade -> SupervisorError");
        match err {
            AgentError::SupervisorError(msg) => {
                assert!(
                    msg.contains("github_token"),
                    "error message must include secret name: {msg}"
                );
            }
            other => panic!("expected SupervisorError, got {other:?}"),
        }
    }
}
