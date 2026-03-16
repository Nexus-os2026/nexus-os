use super::filesystem::GovernedFilesystem;
use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::capabilities::has_capability;
use crate::cognitive::types::PlannedAction;
use base64::Engine;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const FUEL_COST_IMAGE_GEN: f64 = 12.0;
const POLL_ATTEMPTS: usize = 30;
const POLL_INTERVAL_MS: u64 = 2_000;

#[derive(Debug, Clone, Default)]
pub struct ImageGenActuator;

impl ImageGenActuator {
    fn resolve_output_path(
        context: &ActuatorContext,
        output_path: &str,
    ) -> Result<PathBuf, ActuatorError> {
        let safe_path = GovernedFilesystem::resolve_safe_path(&context.working_dir, output_path)?;
        if let Some(parent) = safe_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                ActuatorError::IoError(format!("create image output dir: {error}"))
            })?;
        }
        Ok(safe_path)
    }

    fn select_provider(provider: Option<&str>) -> Result<String, ActuatorError> {
        if let Some(provider) = provider {
            return Ok(provider.to_lowercase());
        }
        if env::var("STABLE_DIFFUSION_WEBUI_URL").is_ok() {
            return Ok("stable-diffusion".to_string());
        }
        if env::var("OPENAI_API_KEY").is_ok() {
            return Ok("dalle".to_string());
        }
        if env::var("REPLICATE_API_TOKEN").is_ok() {
            return Ok("replicate".to_string());
        }
        Err(ActuatorError::IoError(
            "no image provider configured; set STABLE_DIFFUSION_WEBUI_URL, OPENAI_API_KEY, or REPLICATE_API_TOKEN".to_string(),
        ))
    }

    fn parse_dimensions(size: Option<&str>) -> (u32, u32) {
        let Some(size) = size else {
            return (1024, 1024);
        };
        let Some((width, height)) = size.split_once('x') else {
            return (1024, 1024);
        };
        let width = width.parse::<u32>().unwrap_or(1024);
        let height = height.parse::<u32>().unwrap_or(1024);
        (width, height)
    }

    fn curl_json(
        url: &str,
        headers: &BTreeMap<String, String>,
        body: &Value,
    ) -> Result<Value, ActuatorError> {
        let encoded = serde_json::to_string(body)
            .map_err(|error| ActuatorError::IoError(format!("encode image request: {error}")))?;
        let mut command = Command::new("curl");
        command.args(["-sS", "-L", "-X", "POST"]);
        for (header_name, header_value) in headers {
            command
                .arg("-H")
                .arg(format!("{header_name}: {header_value}"));
        }
        let output = command
            .arg("-d")
            .arg(encoded)
            .arg(url)
            .output()
            .map_err(|error| ActuatorError::IoError(format!("curl image request: {error}")))?;
        if !output.status.success() {
            return Err(ActuatorError::IoError(format!(
                "image request failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        serde_json::from_slice(&output.stdout)
            .map_err(|error| ActuatorError::IoError(format!("parse image response: {error}")))
    }

    fn curl_get_json(
        url: &str,
        headers: &BTreeMap<String, String>,
    ) -> Result<Value, ActuatorError> {
        let mut command = Command::new("curl");
        command.args(["-sS", "-L"]);
        for (header_name, header_value) in headers {
            command
                .arg("-H")
                .arg(format!("{header_name}: {header_value}"));
        }
        let output = command
            .arg(url)
            .output()
            .map_err(|error| ActuatorError::IoError(format!("curl poll request: {error}")))?;
        if !output.status.success() {
            return Err(ActuatorError::IoError(format!(
                "poll request failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        serde_json::from_slice(&output.stdout)
            .map_err(|error| ActuatorError::IoError(format!("parse poll response: {error}")))
    }

    fn download_to_path(url: &str, path: &Path) -> Result<(), ActuatorError> {
        let output = Command::new("curl")
            .args(["-sS", "-L", url])
            .output()
            .map_err(|error| ActuatorError::IoError(format!("download image: {error}")))?;
        if !output.status.success() {
            return Err(ActuatorError::IoError(format!(
                "download image failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        std::fs::write(path, output.stdout)
            .map_err(|error| ActuatorError::IoError(format!("write downloaded image: {error}")))
    }

    fn generate_with_sd(
        prompt: &str,
        size: Option<&str>,
        output_path: &Path,
    ) -> Result<(), ActuatorError> {
        let endpoint = format!(
            "{}/sdapi/v1/txt2img",
            env::var("STABLE_DIFFUSION_WEBUI_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:7860".to_string())
                .trim_end_matches('/')
        );
        let (width, height) = Self::parse_dimensions(size);
        let payload = Self::curl_json(
            &endpoint,
            &BTreeMap::from([("content-type".to_string(), "application/json".to_string())]),
            &json!({
                "prompt": prompt,
                "steps": 20,
                "width": width,
                "height": height,
            }),
        )?;
        let image_b64 = payload
            .get("images")
            .and_then(Value::as_array)
            .and_then(|images| images.first())
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ActuatorError::IoError("stable diffusion response missing image".to_string())
            })?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(image_b64)
            .map_err(|error| {
                ActuatorError::IoError(format!("decode stable diffusion image: {error}"))
            })?;
        std::fs::write(output_path, bytes).map_err(|error| {
            ActuatorError::IoError(format!("write stable diffusion image: {error}"))
        })
    }

    fn generate_with_dalle(
        prompt: &str,
        model: Option<&str>,
        size: Option<&str>,
        output_path: &Path,
    ) -> Result<(), ActuatorError> {
        let api_key = env::var("OPENAI_API_KEY")
            .map_err(|_| ActuatorError::IoError("OPENAI_API_KEY is not set".to_string()))?;
        let payload = Self::curl_json(
            "https://api.openai.com/v1/images/generations",
            &BTreeMap::from([
                ("authorization".to_string(), format!("Bearer {api_key}")),
                ("content-type".to_string(), "application/json".to_string()),
            ]),
            &json!({
                "model": model.unwrap_or("gpt-image-1"),
                "prompt": prompt,
                "size": size.unwrap_or("1024x1024"),
                "response_format": "b64_json",
            }),
        )?;
        let image_b64 = payload
            .get("data")
            .and_then(Value::as_array)
            .and_then(|data| data.first())
            .and_then(|item| item.get("b64_json"))
            .and_then(Value::as_str)
            .ok_or_else(|| ActuatorError::IoError("dall-e response missing image".to_string()))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(image_b64)
            .map_err(|error| ActuatorError::IoError(format!("decode dall-e image: {error}")))?;
        std::fs::write(output_path, bytes)
            .map_err(|error| ActuatorError::IoError(format!("write dall-e image: {error}")))
    }

    fn generate_with_replicate(prompt: &str, output_path: &Path) -> Result<(), ActuatorError> {
        let api_token = env::var("REPLICATE_API_TOKEN")
            .map_err(|_| ActuatorError::IoError("REPLICATE_API_TOKEN is not set".to_string()))?;
        let version = env::var("REPLICATE_MODEL_VERSION").map_err(|_| {
            ActuatorError::IoError("REPLICATE_MODEL_VERSION is not set".to_string())
        })?;
        let headers = BTreeMap::from([
            ("authorization".to_string(), format!("Bearer {api_token}")),
            ("content-type".to_string(), "application/json".to_string()),
        ]);
        let created = Self::curl_json(
            "https://api.replicate.com/v1/predictions",
            &headers,
            &json!({
                "version": version,
                "input": {
                    "prompt": prompt,
                }
            }),
        )?;
        let poll_url = created
            .get("urls")
            .and_then(|urls| urls.get("get"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ActuatorError::IoError("replicate response missing poll URL".to_string())
            })?;

        for _ in 0..POLL_ATTEMPTS {
            let polled = Self::curl_get_json(poll_url, &headers)?;
            match polled
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default()
            {
                "succeeded" => {
                    let output_url = polled
                        .get("output")
                        .and_then(|output| match output {
                            Value::String(single) => Some(single.as_str()),
                            Value::Array(values) => values.first().and_then(Value::as_str),
                            _ => None,
                        })
                        .ok_or_else(|| {
                            ActuatorError::IoError("replicate output missing URL".to_string())
                        })?;
                    return Self::download_to_path(output_url, output_path);
                }
                "failed" | "canceled" => {
                    return Err(ActuatorError::IoError(format!(
                        "replicate image generation ended with status {}",
                        polled
                            .get("status")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown")
                    )));
                }
                _ => std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS)),
            }
        }

        Err(ActuatorError::CommandTimeout { seconds: 60 })
    }
}

impl Actuator for ImageGenActuator {
    fn name(&self) -> &str {
        "image_gen_actuator"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["image.generate".into()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        let (prompt, output_path, provider, model, size) = match action {
            PlannedAction::ImageGenerate {
                prompt,
                output_path,
                provider,
                model,
                size,
            } => (
                prompt.as_str(),
                output_path.as_str(),
                provider.as_deref(),
                model.as_deref(),
                size.as_deref(),
            ),
            _ => return Err(ActuatorError::ActionNotHandled),
        };

        if !has_capability(
            context.capabilities.iter().map(String::as_str),
            "image.generate",
        ) {
            return Err(ActuatorError::CapabilityDenied("image.generate".into()));
        }

        let safe_path = Self::resolve_output_path(context, output_path)?;
        let existed = safe_path.exists();
        match Self::select_provider(provider)?.as_str() {
            "stable-diffusion" | "stable_diffusion" | "sd" => {
                Self::generate_with_sd(prompt, size, &safe_path)?
            }
            "dalle" | "openai" => Self::generate_with_dalle(prompt, model, size, &safe_path)?,
            "replicate" => Self::generate_with_replicate(prompt, &safe_path)?,
            other => {
                return Err(ActuatorError::IoError(format!(
                    "unsupported image provider '{other}'"
                )))
            }
        }

        let side_effect = if existed {
            SideEffect::FileModified {
                path: safe_path.clone(),
            }
        } else {
            SideEffect::FileCreated {
                path: safe_path.clone(),
            }
        };

        Ok(ActionResult {
            success: true,
            output: format!("image written to {}", safe_path.display()),
            fuel_cost: FUEL_COST_IMAGE_GEN,
            side_effects: vec![side_effect],
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
        capabilities.insert("image.generate".to_string());
        ActuatorContext {
            agent_id: "agent".into(),
            agent_name: "agent".into(),
            working_dir: tempdir.path().to_path_buf(),
            autonomy_level: AutonomyLevel::L2,
            capabilities,
            fuel_remaining: 100.0,
            egress_allowlist: vec![],
            action_review_engine: None,
        }
    }

    #[test]
    fn parses_dimensions() {
        assert_eq!(
            ImageGenActuator::parse_dimensions(Some("512x768")),
            (512, 768)
        );
        assert_eq!(
            ImageGenActuator::parse_dimensions(Some("oops")),
            (1024, 1024)
        );
    }

    #[test]
    fn resolves_output_inside_workspace() {
        let tempdir = TempDir::new().unwrap();
        let context = make_context(&tempdir);
        let resolved = ImageGenActuator::resolve_output_path(&context, "images/out.png").unwrap();
        assert!(resolved.starts_with(tempdir.path()));
    }
}
