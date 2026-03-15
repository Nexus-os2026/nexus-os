//! GovernedApiClient actuator — governed HTTP API calls with method/size/egress enforcement.

use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::capabilities::has_capability;
use crate::cognitive::types::PlannedAction;

/// Request timeout in seconds.
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// Maximum request body size: 1 MB.
const MAX_REQUEST_BODY: u64 = 1024 * 1024;
/// Maximum response body size: 5 MB.
const MAX_RESPONSE_BODY: u64 = 5 * 1024 * 1024;

/// Fuel cost per API call.
const FUEL_COST_API: f64 = 3.0;

/// Allowed HTTP methods.
const ALLOWED_METHODS: &[&str] = &["GET", "POST", "PUT", "DELETE"];

/// Governed API client actuator. Executes HTTP requests with method validation,
/// egress checks, and size limits.
#[derive(Debug, Clone)]
pub struct GovernedApiClient;

impl GovernedApiClient {
    /// Validate the HTTP method.
    fn validate_method(method: &str) -> Result<(), ActuatorError> {
        let upper = method.to_uppercase();
        if ALLOWED_METHODS.contains(&upper.as_str()) {
            Ok(())
        } else {
            Err(ActuatorError::InvalidMethod(format!(
                "'{method}' not allowed; use one of: {}",
                ALLOWED_METHODS.join(", ")
            )))
        }
    }

    /// Check the request body size.
    fn check_request_body(body: &Option<String>) -> Result<(), ActuatorError> {
        if let Some(b) = body {
            let size = b.len() as u64;
            if size > MAX_REQUEST_BODY {
                return Err(ActuatorError::BodyTooLarge {
                    size,
                    max: MAX_REQUEST_BODY,
                });
            }
        }
        Ok(())
    }

    /// Check URL against egress allowlist.
    fn check_egress(url: &str, context: &ActuatorContext) -> Result<(), ActuatorError> {
        let allowed = context
            .egress_allowlist
            .iter()
            .any(|prefix| url.starts_with(prefix));

        if !allowed {
            return Err(ActuatorError::EgressDenied(format!(
                "URL '{url}' not in egress allowlist"
            )));
        }
        Ok(())
    }
}

impl Actuator for GovernedApiClient {
    fn name(&self) -> &str {
        "governed_api_client"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["mcp.call".into()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        let (method, url, body) = match action {
            PlannedAction::ApiCall { method, url, body } => (method, url, body),
            _ => return Err(ActuatorError::ActionNotHandled),
        };

        if !has_capability(context.capabilities.iter().map(String::as_str), "mcp.call") {
            return Err(ActuatorError::CapabilityDenied("mcp.call".into()));
        }

        // Validate method
        Self::validate_method(method)?;

        // Check body size
        Self::check_request_body(body)?;

        // Egress check
        Self::check_egress(url, context)?;

        // Build curl command
        let method_upper = method.to_uppercase();
        let mut args = vec![
            "-sS".to_string(),
            "-X".to_string(),
            method_upper.clone(),
            "--max-time".to_string(),
            REQUEST_TIMEOUT_SECS.to_string(),
            "--max-filesize".to_string(),
            MAX_RESPONSE_BODY.to_string(),
            "-H".to_string(),
            "Content-Type: application/json".to_string(),
        ];

        if let Some(b) = body {
            args.push("-d".to_string());
            args.push(b.clone());
        }

        args.push(url.clone());

        let output = std::process::Command::new("curl")
            .args(&args)
            .output()
            .map_err(|e| ActuatorError::IoError(format!("curl spawn: {e}")))?;

        let response_body = String::from_utf8_lossy(&output.stdout);
        let mut response = response_body.to_string();

        // Truncate if over limit
        let max = MAX_RESPONSE_BODY as usize;
        if response.len() > max {
            response.truncate(max);
            response.push_str("\n... [response truncated]");
        }

        Ok(ActionResult {
            success: output.status.success(),
            output: response,
            fuel_cost: FUEL_COST_API,
            side_effects: vec![SideEffect::HttpRequest { url: url.clone() }],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autonomy::AutonomyLevel;
    use std::collections::HashSet;

    fn make_context() -> ActuatorContext {
        let mut caps = HashSet::new();
        caps.insert("mcp.call".into());
        ActuatorContext {
            agent_id: "test-agent".into(),
            working_dir: std::path::PathBuf::from("/tmp"),
            autonomy_level: AutonomyLevel::L2,
            capabilities: caps,
            fuel_remaining: 1000.0,
            egress_allowlist: vec!["https://api.example.com".into()],
        }
    }

    #[test]
    fn valid_methods_accepted() {
        for method in &["GET", "POST", "PUT", "DELETE", "get", "post"] {
            assert!(
                GovernedApiClient::validate_method(method).is_ok(),
                "{method} should be valid"
            );
        }
    }

    #[test]
    fn invalid_methods_rejected() {
        for method in &["PATCH", "OPTIONS", "HEAD", "CONNECT", "TRACE"] {
            assert!(
                GovernedApiClient::validate_method(method).is_err(),
                "{method} should be rejected"
            );
        }
    }

    #[test]
    fn body_size_limit() {
        // Under limit
        let small = Some("hello".into());
        assert!(GovernedApiClient::check_request_body(&small).is_ok());

        // Over limit
        let big = Some("x".repeat(2 * 1024 * 1024));
        let err = GovernedApiClient::check_request_body(&big).unwrap_err();
        assert!(matches!(err, ActuatorError::BodyTooLarge { .. }));

        // No body
        assert!(GovernedApiClient::check_request_body(&None).is_ok());
    }

    #[test]
    fn egress_check() {
        let ctx = make_context();

        // Allowed
        assert!(GovernedApiClient::check_egress("https://api.example.com/v1/data", &ctx).is_ok());

        // Denied
        let err = GovernedApiClient::check_egress("https://evil.com/steal", &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::EgressDenied(_)));
    }

    #[test]
    fn capability_denied() {
        let mut ctx = make_context();
        ctx.capabilities.clear();
        let api = GovernedApiClient;

        let action = PlannedAction::ApiCall {
            method: "GET".into(),
            url: "https://api.example.com/data".into(),
            body: None,
        };
        let err = api.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::CapabilityDenied(_)));
    }

    #[test]
    fn action_not_handled() {
        let ctx = make_context();
        let api = GovernedApiClient;

        let action = PlannedAction::Noop;
        let err = api.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::ActionNotHandled));
    }

    #[test]
    fn full_validation_pipeline() {
        let ctx = make_context();
        let api = GovernedApiClient;

        // Invalid method
        let action = PlannedAction::ApiCall {
            method: "PATCH".into(),
            url: "https://api.example.com/v1".into(),
            body: None,
        };
        let err = api.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::InvalidMethod(_)));

        // Bad URL
        let action = PlannedAction::ApiCall {
            method: "GET".into(),
            url: "https://bad.com/steal".into(),
            body: None,
        };
        let err = api.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::EgressDenied(_)));

        // Body too large
        let action = PlannedAction::ApiCall {
            method: "POST".into(),
            url: "https://api.example.com/v1".into(),
            body: Some("x".repeat(2 * 1024 * 1024)),
        };
        let err = api.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::BodyTooLarge { .. }));
    }
}
