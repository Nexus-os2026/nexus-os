use super::{curl_post_json_with_timeout, LlmProvider, LlmResponse, ProviderRequest};
use nexus_kernel::errors::AgentError;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;

const NVIDIA_NIM_ENDPOINT: &str = "https://integrate.api.nvidia.com/v1/chat/completions";

/// Available NVIDIA NIM models — free tier (1000 credits on signup).
/// 42 models from 12 providers. All free on build.nvidia.com as of March 2026.
pub const NVIDIA_MODELS: &[(&str, &str)] = &[
    // ═══ DeepSeek (Best for Agents) ═══
    (
        "deepseek-ai/deepseek-v3_1-terminus",
        "DeepSeek V3.1 Terminus 671B — Best for agents, hybrid Think/Non-Think, 128K ctx",
    ),
    (
        "deepseek-ai/deepseek-v3_1",
        "DeepSeek V3.1 — Hybrid thinking, smarter tool calling, 128K ctx",
    ),
    ("deepseek-ai/deepseek-v3", "DeepSeek V3 — Coding specialist"),
    (
        "deepseek-ai/deepseek-r1",
        "DeepSeek R1 — Reasoning optimized",
    ),
    (
        "deepseek-ai/deepseek-r1-distill-llama-70b",
        "DeepSeek R1 Distill 70B — Reasoning distilled to Llama",
    ),
    (
        "deepseek-ai/deepseek-r1-distill-qwen-32b",
        "DeepSeek R1 Distill Qwen 32B — Reasoning distilled",
    ),
    (
        "deepseek-ai/deepseek-r1-distill-qwen-14b",
        "DeepSeek R1 Distill Qwen 14B — Lightweight reasoning",
    ),
    (
        "deepseek-ai/deepseek-r1-distill-llama-8b",
        "DeepSeek R1 Distill 8B — Fast reasoning",
    ),
    // ═══ Zhipu GLM (Agentic Coding) ═══
    (
        "zhipuai/glm-4.7",
        "GLM-4.7 — Multilingual agentic coding, stronger reasoning, tool use, UI skills",
    ),
    (
        "zhipuai/glm-5-744b",
        "GLM-5 744B MoE — Complex reasoning, 205K ctx, MIT license, 40 RPM free",
    ),
    // ═══ Moonshot / Kimi ═══
    (
        "moonshotai/kimi-k2-instruct",
        "Kimi K2 Instruct — Coding and long context specialist",
    ),
    // ═══ Meta / Llama ═══
    (
        "meta/llama-4-scout-17b-16e-instruct",
        "Llama 4 Scout 17B — Fast general purpose",
    ),
    (
        "meta/llama-4-maverick-17b-128e-instruct",
        "Llama 4 Maverick 17B — Long context 128 experts",
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
    (
        "meta/llama-3.1-8b-instruct",
        "Llama 3.1 8B — Fast lightweight",
    ),
    // ═══ NVIDIA Nemotron ═══
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
        "Nemotron 3 Super 120B — Hybrid Mamba-Transformer, 1M ctx, agentic",
    ),
    (
        "nvidia/nemotron-3-nano-30b-a3b",
        "Nemotron 3 Nano 30B — Fast efficient agentic",
    ),
    // ═══ Qwen ═══
    (
        "qwen/qwen3.5-vl-400b",
        "Qwen 3.5 VLM 400B MoE — Vision, chat, RAG, agentic",
    ),
    (
        "qwen/qwen2.5-72b-instruct",
        "Qwen 2.5 72B — Alibaba frontier",
    ),
    (
        "qwen/qwen2.5-coder-32b-instruct",
        "Qwen 2.5 Coder 32B — Coding specialist",
    ),
    ("qwen/qwen2.5-7b-instruct", "Qwen 2.5 7B — Lightweight"),
    // ═══ Mistral ═══
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
    (
        "mistralai/devstral-2-123b-instruct-2512",
        "Devstral 2 123B — Coding focused",
    ),
    // ═══ MiniMax ═══
    (
        "minimax/minimax-m2.5",
        "MiniMax M2.5 230B — Coding, reasoning, office tasks",
    ),
    // ═══ Google Gemma ═══
    (
        "google/gemma-3-27b-it",
        "Gemma 3 27B — Efficient open model",
    ),
    (
        "google/gemma-3-12b-it",
        "Gemma 3 12B — Edge and fast inference",
    ),
    // ═══ Microsoft Phi ═══
    ("microsoft/phi-4", "Phi-4 — Small but smart reasoning"),
    (
        "microsoft/phi-3-medium-128k-instruct",
        "Phi-3 Medium 128K — Long context",
    ),
    (
        "microsoft/phi-3.5-vision-instruct",
        "Phi-3.5 Vision — Multimodal",
    ),
    // ═══ IBM Granite ═══
    (
        "ibm/granite-3.1-8b-instruct",
        "Granite 3.1 8B — Enterprise lightweight",
    ),
    (
        "ibm/granite-3.3-8b-instruct",
        "Granite 3.3 8B — Updated enterprise",
    ),
    // ═══ Writer ═══
    (
        "writer/palmyra-x-004",
        "Palmyra X 004 — Enterprise writing and reasoning",
    ),
];

pub const NVIDIA_VISION_MODELS: &[&str] = &[
    "meta/llama-3.2-90b-vision-instruct",
    "qwen/qwen3.5-vl-400b",
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

    fn endpoint_url(&self) -> String {
        "https://integrate.api.nvidia.com".to_string()
    }
}
