use super::{
    curl_post_json_with_timeout, require_real_api, LlmProvider, LlmResponse, ProviderRequest,
};
use nexus_kernel::errors::AgentError;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;

const NVIDIA_NIM_ENDPOINT: &str = "https://integrate.api.nvidia.com/v1/chat/completions";

/// Available NVIDIA NIM models — free tier (1000 credits on signup).
pub const NVIDIA_MODELS: &[(&str, &str)] = &[
    // Meta / Llama
    (
        "meta/llama-4-scout-17b-16e-instruct",
        "Llama 4 Scout 17B — Fast general purpose",
    ),
    (
        "meta/llama-4-maverick-17b-128e-instruct",
        "Llama 4 Maverick 17B — Long context specialist",
    ),
    (
        "meta/llama-3.3-70b-instruct",
        "Llama 3.3 70B — Meta flagship open model",
    ),
    (
        "meta/llama-3.1-405b-instruct",
        "Llama 3.1 405B — Largest open model",
    ),
    (
        "meta/llama-3.2-90b-vision-instruct",
        "Llama 3.2 90B Vision — Multimodal",
    ),
    // NVIDIA Nemotron
    (
        "nvidia/llama-3.1-nemotron-ultra-253b-v1",
        "Nemotron Ultra 253B — NVIDIA most capable",
    ),
    (
        "nvidia/llama-3.1-nemotron-70b-instruct",
        "Nemotron 70B — Reward-tuned Llama",
    ),
    (
        "nvidia/nemotron-4-340b-instruct",
        "Nemotron 4 340B — High quality reasoning",
    ),
    (
        "nvidia/nemotron-3-super-120b-a12b",
        "Nemotron 3 Super 120B — Agentic reasoning",
    ),
    (
        "nvidia/nemotron-3-nano-30b-a3b",
        "Nemotron 3 Nano 30B — Fast lightweight",
    ),
    // DeepSeek
    (
        "deepseek-ai/deepseek-r1",
        "DeepSeek R1 — Reasoning optimized",
    ),
    (
        "deepseek-ai/deepseek-r1-distill-llama-70b",
        "DeepSeek R1 Distill 70B — Reasoning distilled",
    ),
    ("deepseek-ai/deepseek-v3", "DeepSeek V3 — Coding specialist"),
    // Moonshot / Kimi
    (
        "moonshotai/kimi-k2-instruct",
        "Kimi K2 — Coding and long context",
    ),
    // MiniMax
    (
        "minimax/minimax-m2.5",
        "MiniMax M2.5 — Office and document tasks",
    ),
    // Zhipu
    (
        "zhipuai/glm-5-744b",
        "GLM-5 744B — Complex reasoning heavyweight",
    ),
    // Mistral
    (
        "mistralai/mistral-large-2-instruct",
        "Mistral Large 2 — 123B flagship",
    ),
    (
        "mistralai/mixtral-8x22b-instruct-v0.1",
        "Mixtral 8x22B — Fast MoE architecture",
    ),
    (
        "mistralai/mistral-7b-instruct-v0.3",
        "Mistral 7B — Lightweight fast",
    ),
    // Google Gemma
    (
        "google/gemma-3-27b-it",
        "Gemma 3 27B — Efficient open model",
    ),
    (
        "google/gemma-3-12b-it",
        "Gemma 3 12B — Edge and fast inference",
    ),
    // Microsoft Phi
    ("microsoft/phi-4", "Phi-4 — Small but smart reasoning"),
    (
        "microsoft/phi-3-medium-128k-instruct",
        "Phi-3 Medium 128K — Long context specialist",
    ),
    // Qwen
    (
        "qwen/qwen2.5-72b-instruct",
        "Qwen 2.5 72B — Alibaba frontier",
    ),
    (
        "qwen/qwen2.5-coder-32b-instruct",
        "Qwen 2.5 Coder 32B — Coding specialist",
    ),
];

pub const NVIDIA_VISION_MODELS: &[&str] = &[
    "meta/llama-3.2-90b-vision-instruct",
    "nvidia/llama-3.2-neva-72b-preview",
    "microsoft/phi-3.5-vision-instruct",
    "google/gemma-3-27b-it",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NvidiaProvider {
    api_key: Option<String>,
    endpoint: String,
}

impl NvidiaProvider {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            api_key,
            endpoint: NVIDIA_NIM_ENDPOINT.to_string(),
        }
    }

    pub fn from_env() -> Self {
        let endpoint =
            env::var("NVIDIA_NIM_URL").unwrap_or_else(|_| NVIDIA_NIM_ENDPOINT.to_string());
        let mut provider = Self::new(env::var("NVIDIA_NIM_API_KEY").ok());
        provider.endpoint = endpoint;
        provider
    }

    pub fn build_request(&self, prompt: &str, max_tokens: u32, model: &str) -> ProviderRequest {
        let api_key = self.api_key.clone().unwrap_or_default();
        let mut headers = BTreeMap::new();
        headers.insert(
            "authorization".to_string(),
            format!("Bearer {}", api_key.trim()),
        );
        headers.insert("content-type".to_string(), "application/json".to_string());

        ProviderRequest {
            endpoint: self.endpoint.clone(),
            headers,
            body: json!({
                "model": model,
                "messages": [
                    {
                        "role": "user",
                        "content": prompt
                    }
                ],
                "max_tokens": max_tokens
            }),
        }
    }

    fn api_key(&self) -> Option<String> {
        self.api_key
            .clone()
            .or_else(|| env::var("NVIDIA_NIM_API_KEY").ok())
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
    }
}

impl LlmProvider for NvidiaProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        require_real_api(cfg!(feature = "real-api-tests"))?;

        let Some(api_key) = self.api_key() else {
            return Err(AgentError::SupervisorError(
                "NVIDIA_NIM_API_KEY is not set. Get a free key at https://build.nvidia.com"
                    .to_string(),
            ));
        };
        let request = NvidiaProvider::new(Some(api_key)).build_request(prompt, max_tokens, model);

        // NVIDIA NIM can be slow for large models — use 120s timeout
        let (status, payload) = curl_post_json_with_timeout(
            request.endpoint.as_str(),
            &request.headers,
            &request.body,
            120,
        )?;
        if !(200..300).contains(&status) {
            return Err(AgentError::SupervisorError(format!(
                "nvidia nim request failed with status {status}"
            )));
        }

        let output_text = payload
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let token_count = payload
            .get("usage")
            .and_then(|usage| usage.get("total_tokens"))
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(max_tokens.min(256));

        Ok(LlmResponse {
            output_text,
            token_count,
            model_name: model.to_string(),
            tool_calls: Vec::new(),
        })
    }

    fn name(&self) -> &str {
        "nvidia"
    }

    fn cost_per_token(&self) -> f64 {
        0.000_001 // Free tier / very low cost
    }

    fn requires_real_api_opt_in(&self) -> bool {
        true
    }

    fn endpoint_url(&self) -> String {
        "https://integrate.api.nvidia.com".to_string()
    }
}
