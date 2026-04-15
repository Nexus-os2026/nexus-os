use super::filesystem::GovernedFilesystem;
use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::capabilities::has_capability;
use crate::cognitive::types::{BrowserAction, PlannedAction};
use chrono::Utc;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

const FUEL_COST_BROWSER: f64 = 10.0;

#[derive(Debug, Clone)]
pub struct BrowserActuator;

impl BrowserActuator {
    fn ensure_allowlisted(url: &str, context: &ActuatorContext) -> Result<(), ActuatorError> {
        if context
            .egress_allowlist
            .iter()
            .any(|allowed| url.starts_with(allowed))
        {
            return Ok(());
        }
        Err(ActuatorError::EgressDenied(format!(
            "browser URL '{url}' not in allowlist"
        )))
    }

    fn contains_financial_intent(text: &str) -> bool {
        let text = text.to_lowercase();
        [
            "checkout",
            "purchase",
            "buy now",
            "payment",
            "credit card",
            "card number",
            "billing",
            "wallet",
            "bank transfer",
            "subscribe",
        ]
        .iter()
        .any(|pattern| text.contains(pattern))
    }

    fn require_no_financial_transaction(
        start_url: &str,
        actions: &[BrowserAction],
    ) -> Result<(), ActuatorError> {
        if Self::contains_financial_intent(start_url) {
            return Err(ActuatorError::HumanApprovalRequired(
                "financial browser flows require HITL approval".to_string(),
            ));
        }

        for action in actions {
            let risky = match action {
                BrowserAction::Navigate { url } => Self::contains_financial_intent(url),
                BrowserAction::Click { selector } => Self::contains_financial_intent(selector),
                BrowserAction::Fill { selector, text } => {
                    Self::contains_financial_intent(selector)
                        || Self::contains_financial_intent(text)
                }
                BrowserAction::Press { selector, key } => {
                    Self::contains_financial_intent(selector)
                        || Self::contains_financial_intent(key)
                }
                BrowserAction::WaitFor { selector, .. } => selector
                    .as_deref()
                    .map(Self::contains_financial_intent)
                    .unwrap_or(false),
                BrowserAction::ExtractText { selector } => {
                    Self::contains_financial_intent(selector)
                }
            };
            if risky {
                return Err(ActuatorError::HumanApprovalRequired(
                    "financial browser flows require HITL approval".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn resolve_screenshot_dir(
        context: &ActuatorContext,
        screenshot_dir: Option<&str>,
    ) -> Result<PathBuf, ActuatorError> {
        let relative = screenshot_dir
            .map(str::to_string)
            .unwrap_or_else(|| format!("browser_audit/{}", Utc::now().format("%Y%m%dT%H%M%SZ")));
        let safe_path = GovernedFilesystem::resolve_safe_path(&context.working_dir, &relative)?;
        std::fs::create_dir_all(&safe_path)
            .map_err(|error| ActuatorError::IoError(format!("create screenshot dir: {error}")))?;
        Ok(safe_path)
    }

    fn runner_script() -> &'static str {
        r#"const fs = require('fs');
const path = require('path');

async function run() {
  const specPath = process.argv[2];
  const spec = JSON.parse(fs.readFileSync(specPath, 'utf8'));
  const playwright = require(process.env.NEXUS_PLAYWRIGHT_MODULE || 'playwright');
  const browser = await playwright.chromium.launch({ headless: true });
  const page = await browser.newPage();
  const extractions = [];

  await page.goto(spec.start_url, { waitUntil: 'networkidle' });

  for (let index = 0; index < spec.actions.length; index += 1) {
    const action = spec.actions[index];
    const before = path.join(spec.screenshot_dir, `${String(index).padStart(2, '0')}-before.png`);
    const after = path.join(spec.screenshot_dir, `${String(index).padStart(2, '0')}-after.png`);
    await page.screenshot({ path: before, fullPage: true });

    switch (action.kind) {
      case 'navigate':
        await page.goto(action.url, { waitUntil: 'networkidle' });
        break;
      case 'click':
        await page.click(action.selector);
        break;
      case 'fill':
        await page.fill(action.selector, action.text);
        break;
      case 'press':
        await page.press(action.selector, action.key);
        break;
      case 'wait_for':
        if (action.selector) {
          await page.waitForSelector(action.selector, { timeout: action.timeout_ms || 10000 });
        } else {
          await page.waitForTimeout(action.timeout_ms || 1000);
        }
        break;
      case 'extract_text':
        extractions.push(await page.textContent(action.selector));
        break;
      default:
        throw new Error(`unsupported browser action ${action.kind}`);
    }

    await page.screenshot({ path: after, fullPage: true });
  }

  await browser.close();
  process.stdout.write(JSON.stringify({ extracted_text: extractions.filter(Boolean) }));
}

run().catch((error) => {
  console.error(error && error.stack ? error.stack : String(error));
  process.exit(1);
});
"#
    }

    fn write_runner_files(
        spec_path: &Path,
        screenshot_dir: &Path,
        start_url: &str,
        actions: &[BrowserAction],
    ) -> Result<PathBuf, ActuatorError> {
        let runner_path = screenshot_dir.join("playwright-runner.cjs");
        std::fs::write(&runner_path, Self::runner_script())
            .map_err(|error| ActuatorError::IoError(format!("write browser runner: {error}")))?;
        std::fs::write(
            spec_path,
            serde_json::to_vec_pretty(&json!({
                "start_url": start_url,
                "actions": actions,
                "screenshot_dir": screenshot_dir,
            }))
            .map_err(|error| ActuatorError::IoError(format!("encode browser spec: {error}")))?,
        )
        .map_err(|error| ActuatorError::IoError(format!("write browser spec: {error}")))?;
        Ok(runner_path)
    }
}

impl Actuator for BrowserActuator {
    fn name(&self) -> &str {
        "browser_actuator"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["browser.automate".into()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        let (start_url, actions, screenshot_dir) = match action {
            PlannedAction::BrowserAutomate {
                start_url,
                actions,
                screenshot_dir,
            } => (
                start_url.as_str(),
                actions.as_slice(),
                screenshot_dir.as_deref(),
            ),
            _ => return Err(ActuatorError::ActionNotHandled),
        };

        if !has_capability(
            context.capabilities.iter().map(String::as_str),
            "browser.automate",
        ) {
            return Err(ActuatorError::CapabilityDenied("browser.automate".into()));
        }

        Self::ensure_allowlisted(start_url, context)?;
        for action in actions {
            if let BrowserAction::Navigate { url } = action {
                Self::ensure_allowlisted(url, context)?;
            }
        }
        Self::require_no_financial_transaction(start_url, actions)?;

        let screenshot_dir = Self::resolve_screenshot_dir(context, screenshot_dir)?;
        let spec_path = screenshot_dir.join("spec.json");
        let runner_path =
            Self::write_runner_files(&spec_path, &screenshot_dir, start_url, actions)?;
        let output = Command::new("node")
            .arg(&runner_path)
            .arg(&spec_path)
            .current_dir(&context.working_dir)
            .output()
            .map_err(|error| {
                ActuatorError::IoError(format!("spawn browser automation: {error}"))
            })?;
        if !output.status.success() {
            return Err(ActuatorError::IoError(format!(
                "browser automation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let parsed = serde_json::from_slice::<Value>(&output.stdout)
            .map_err(|error| ActuatorError::IoError(format!("parse browser output: {error}")))?;
        let mut side_effects = vec![SideEffect::HttpRequest {
            url: start_url.to_string(),
        }];
        side_effects.push(SideEffect::FileCreated {
            path: screenshot_dir.clone(),
        });

        Ok(ActionResult {
            success: true,
            output: parsed.to_string(),
            fuel_cost: FUEL_COST_BROWSER,
            side_effects,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autonomy::AutonomyLevel;
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn make_context(tempdir: &TempDir) -> ActuatorContext {
        let mut capabilities = HashSet::new();
        capabilities.insert("browser.automate".to_string());
        ActuatorContext {
            agent_id: "agent".into(),
            agent_name: "agent".into(),
            working_dir: tempdir.path().to_path_buf(),
            autonomy_level: AutonomyLevel::L2,
            capabilities,
            fuel_remaining: 100.0,
            egress_allowlist: vec!["https://example.com".into()],
            action_review_engine: None,
            hitl_approved: false,
        }
    }

    #[test]
    fn blocks_non_allowlisted_urls() {
        let tempdir = TempDir::new().unwrap();
        let context = make_context(&tempdir);
        let error = BrowserActuator::ensure_allowlisted("https://evil.com", &context).unwrap_err();
        assert!(matches!(error, ActuatorError::EgressDenied(_)));
    }

    #[test]
    fn blocks_financial_flows() {
        let error = BrowserActuator::require_no_financial_transaction(
            "https://example.com/cart",
            &[BrowserAction::Click {
                selector: "button.checkout".into(),
            }],
        )
        .unwrap_err();
        assert!(matches!(error, ActuatorError::HumanApprovalRequired(_)));
    }
}
