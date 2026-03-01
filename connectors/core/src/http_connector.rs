use crate::challenge::{detect_challenge, handle_challenge, ChallengeType};
use crate::connector::{Connector, HealthStatus, RetryPolicy};
use crate::idempotency::IdempotencyManager;
use crate::rate_limit::{RateLimitDecision, RateLimiter};
use crate::vault::{SecretsVault, VaultUserKey};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::lifecycle::AgentState;
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
    user_key: VaultUserKey,
}

pub struct HttpConnector {
    id: String,
    name: String,
    required_capabilities: Vec<String>,
    retry_policy: RetryPolicy,
    degrade_gracefully: bool,
    pub rate_limiter: RateLimiter,
    pub idempotency: IdempotencyManager,
    pub vault: SecretsVault,
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
            vault: SecretsVault::new(),
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

    pub fn bind_auth_secret(&mut self, secret_name: &str, user_key: VaultUserKey) {
        self.auth_binding = Some(AuthBinding {
            secret_name: secret_name.to_string(),
            user_key,
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
            let token = self
                .vault
                .get_secret(binding.secret_name.as_str(), &binding.user_key)?;
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
        let _ = self.audit_trail.append_event(
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
        );
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
