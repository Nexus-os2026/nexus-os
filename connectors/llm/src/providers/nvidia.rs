use super::{curl_post_json_with_timeout, LlmProvider, LlmResponse, ProviderRequest};
use nexus_kernel::errors::AgentError;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;

const NVIDIA_NIM_ENDPOINT: &str = "https://integrate.api.nvidia.com/v1/chat/completions";

/// Available NVIDIA NIM models — free tier (1000 credits on signup).
/// 93 models from 18 providers. All free on build.nvidia.com as of March 2026.
pub const NVIDIA_MODELS: &[(&str, &str)] = &[
    // ═══ DeepSeek (10 models) ═══
    (
        "deepseek-ai/deepseek-v3.1-terminus",
        "DeepSeek V3.1 Terminus 671B — Best for agents, hybrid Think/Non-Think, 128K ctx",
    ),
    (
        "deepseek-ai/deepseek-v3.1",
        "DeepSeek V3.1 — Hybrid thinking, smarter tool calling, 128K ctx",
    ),
    (
        "deepseek-ai/deepseek-v3.2",
        "DeepSeek V3 — Coding specialist, 128K ctx",
    ),
    (
        "deepseek-ai/deepseek-r1",
        "DeepSeek R1 — Reasoning optimized, chain-of-thought",
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
    (
        "deepseek-ai/deepseek-coder-v2-instruct",
        "DeepSeek Coder V2 236B MoE — Code generation specialist",
    ),
    (
        "deepseek-ai/deepseek-coder-v2-lite-instruct",
        "DeepSeek Coder V2 Lite 16B — Lightweight code generation",
    ),
    // ═══ Meta / Llama (12 models) ═══
    (
        "meta/llama-4-scout-17b-16e-instruct",
        "Llama 4 Scout 17B — Fast general purpose, 16 experts",
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
        "Llama 3.1 405B — Largest open model, 128K ctx",
    ),
    (
        "meta/llama-3.1-70b-instruct",
        "Llama 3.1 70B — High quality general purpose",
    ),
    (
        "meta/llama-3.1-8b-instruct",
        "Llama 3.1 8B — Fast lightweight",
    ),
    (
        "meta/llama-3.2-90b-vision-instruct",
        "Llama 3.2 90B Vision — Multimodal",
    ),
    (
        "meta/llama-3.2-11b-vision-instruct",
        "Llama 3.2 11B Vision — Lightweight multimodal",
    ),
    (
        "meta/llama-3.2-3b-instruct",
        "Llama 3.2 3B — Ultra-lightweight edge model",
    ),
    (
        "meta/llama-3.2-1b-instruct",
        "Llama 3.2 1B — Smallest Llama, edge inference",
    ),
    (
        "meta/codellama-70b-instruct",
        "Code Llama 70B — Dedicated code generation",
    ),
    (
        "meta/llama-guard-3-8b",
        "Llama Guard 3 8B — Content safety classifier",
    ),
    // ═══ NVIDIA Nemotron (8 models) ═══
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
        "Nemotron 3 Super 120B — Hybrid Mamba-Transformer, 1M ctx",
    ),
    (
        "nvidia/nemotron-3-nano-30b-a3b",
        "Nemotron 3 Nano 30B — Fast efficient agentic",
    ),
    (
        "nvidia/nemotron-mini-4b-instruct",
        "Nemotron Mini 4B — Edge-optimized small model",
    ),
    (
        "nvidia/llama-3.1-nemotron-51b-instruct",
        "Nemotron 51B — Mid-range balanced model",
    ),
    (
        "nvidia/usdcode-llama3.1-70b-instruct",
        "USD Code 70B — 3D/USD code generation",
    ),
    // ═══ Qwen (12 models) ═══
    (
        "qwen/qwen3.5-397b-a17b",
        "Qwen 3.5 VLM 400B MoE — Vision, chat, RAG, agentic",
    ),
    (
        "qwen/qwen2.5-72b-instruct",
        "Qwen 2.5 72B — Alibaba frontier",
    ),
    (
        "qwen/qwen2.5-32b-instruct",
        "Qwen 2.5 32B — Strong mid-range general purpose",
    ),
    (
        "qwen/qwen2.5-14b-instruct",
        "Qwen 2.5 14B — Balanced performance/efficiency",
    ),
    (
        "qwen/qwen2.5-7b-instruct",
        "Qwen 2.5 7B — Lightweight general purpose",
    ),
    (
        "qwen/qwen2.5-coder-32b-instruct",
        "Qwen 2.5 Coder 32B — Coding specialist",
    ),
    (
        "qwen/qwen2.5-coder-7b-instruct",
        "Qwen 2.5 Coder 7B — Lightweight coding",
    ),
    (
        "qwen/qwq-32b",
        "QwQ 32B — Reasoning model with chain-of-thought",
    ),
    (
        "qwen/qwen2-vl-72b-instruct",
        "Qwen 2 VL 72B — Vision-language multimodal",
    ),
    (
        "qwen/qwen2-vl-7b-instruct",
        "Qwen 2 VL 7B — Lightweight vision-language",
    ),
    (
        "qwen/qwen2.5-1.5b-instruct",
        "Qwen 2.5 1.5B — Ultra-lightweight edge model",
    ),
    (
        "qwen/qwen2.5-math-72b-instruct",
        "Qwen 2.5 Math 72B — Mathematical reasoning",
    ),
    // ═══ Mistral (8 models) ═══
    (
        "mistralai/mistral-large-2-instruct",
        "Mistral Large 2 — 123B flagship",
    ),
    (
        "mistralai/mixtral-8x22b-instruct-v0.1",
        "Mixtral 8x22B — Fast MoE architecture",
    ),
    (
        "mistralai/mixtral-8x7b-instruct-v0.1",
        "Mixtral 8x7B — Original MoE model",
    ),
    (
        "mistralai/mistral-7b-instruct-v0.3",
        "Mistral 7B v0.3 — Lightweight fast",
    ),
    (
        "mistralai/mistral-small-24b-instruct-2501",
        "Mistral Small 24B — Efficient mid-range",
    ),
    (
        "mistralai/codestral-22b-instruct-v0.1",
        "Codestral 22B — Code generation specialist",
    ),
    (
        "mistralai/devstral-2-123b-instruct-2512",
        "Devstral 2 123B — Coding focused, SWE agent",
    ),
    (
        "mistralai/mamba-codestral-7b-v0.1",
        "Mamba Codestral 7B — Mamba architecture for code",
    ),
    // ═══ Google Gemma (6 models) ═══
    (
        "google/gemma-3-27b-it",
        "Gemma 3 27B — Efficient open model, vision capable",
    ),
    (
        "google/gemma-3-12b-it",
        "Gemma 3 12B — Edge and fast inference",
    ),
    ("google/gemma-3-4b-it", "Gemma 3 4B — Ultra-lightweight"),
    (
        "google/gemma-2-27b-it",
        "Gemma 2 27B — Previous generation flagship",
    ),
    ("google/gemma-2-9b-it", "Gemma 2 9B — Balanced efficiency"),
    (
        "google/codegemma-7b",
        "CodeGemma 7B — Code generation specialist",
    ),
    // ═══ Microsoft Phi (6 models) ═══
    ("microsoft/phi-4", "Phi-4 14B — Small but smart reasoning"),
    (
        "microsoft/phi-4-mini-instruct",
        "Phi-4 Mini — Ultra-compact reasoning",
    ),
    (
        "microsoft/phi-3.5-moe-instruct",
        "Phi-3.5 MoE 42B — Mixture of experts",
    ),
    (
        "microsoft/phi-3.5-mini-instruct",
        "Phi-3.5 Mini 3.8B — Compact general purpose",
    ),
    (
        "microsoft/phi-3-medium-128k-instruct",
        "Phi-3 Medium 14B — Long context 128K",
    ),
    (
        "microsoft/phi-3.5-vision-instruct",
        "Phi-3.5 Vision — Multimodal understanding",
    ),
    // ═══ Zhipu GLM (4 models) ═══
    (
        "z-ai/glm4.7",
        "GLM-4.7 — Multilingual agentic coding, tool use, UI skills",
    ),
    ("z-ai/glm5", "GLM-5 — Complex reasoning, MIT license"),
    (
        "zhipuai/glm-4-9b-chat",
        "GLM-4 9B — Lightweight Chinese/English chat",
    ),
    (
        "zhipuai/codegeex-4-9b",
        "CodeGeeX 4 9B — Multilingual code generation",
    ),
    // ═══ IBM Granite (5 models) ═══
    (
        "ibm/granite-3.1-8b-instruct",
        "Granite 3.1 8B — Enterprise lightweight",
    ),
    (
        "ibm/granite-3.3-8b-instruct",
        "Granite 3.3 8B — Updated enterprise",
    ),
    (
        "ibm/granite-3.1-2b-instruct",
        "Granite 3.1 2B — Ultra-compact enterprise",
    ),
    (
        "ibm/granite-34b-code-instruct",
        "Granite 34B Code — Enterprise code generation",
    ),
    (
        "ibm/granite-guardian-3.1-8b",
        "Granite Guardian 8B — Content safety and hallucination detection",
    ),
    // ═══ Moonshot / Kimi (2 models) ═══
    (
        "moonshotai/kimi-k2-instruct",
        "Kimi K2 Instruct — Coding and long context specialist",
    ),
    (
        "moonshotai/kimi-vl-a3b-thinking",
        "Kimi VL A3B — Vision-language with reasoning",
    ),
    // ═══ MiniMax (2 models) ═══
    (
        "minimax/minimax-m2.5",
        "MiniMax M2.5 230B — Coding, reasoning, office tasks",
    ),
    ("minimax/minimax-m1-80b", "MiniMax M1 80B — General purpose"),
    // ═══ Writer (2 models) ═══
    (
        "writer/palmyra-x-004",
        "Palmyra X 004 — Enterprise writing and reasoning",
    ),
    (
        "writer/palmyra-fin-70b-32k",
        "Palmyra Fin 70B — Financial domain specialist",
    ),
    // ═══ Databricks / DBRX (2 models) ═══
    (
        "databricks/dbrx-instruct",
        "DBRX 132B MoE — Databricks open MoE model",
    ),
    (
        "databricks/dolly-v2-12b",
        "Dolly V2 12B — Open instruction-following",
    ),
    // ═══ Cohere (2 models) ═══
    (
        "cohere/command-r-plus-08-2024",
        "Command R+ — RAG-optimized enterprise model",
    ),
    (
        "cohere/command-r-08-2024",
        "Command R — Lightweight RAG model",
    ),
    // ═══ Snowflake (2 models) ═══
    (
        "snowflake/arctic-instruct",
        "Arctic 480B MoE — Snowflake enterprise SQL/code",
    ),
    (
        "snowflake/arctic-embed-l-v2.0",
        "Arctic Embed L v2 — Enterprise embedding model",
    ),
    // ═══ Nous Research (2 models) ═══
    (
        "nousresearch/hermes-3-llama-3.1-70b",
        "Hermes 3 70B — Uncensored general purpose",
    ),
    (
        "nousresearch/hermes-3-llama-3.1-8b",
        "Hermes 3 8B — Lightweight uncensored",
    ),
    // ═══ Upstage (2 models) ═══
    (
        "upstage/solar-10.7b-instruct-v1.0",
        "Solar 10.7B — Depth-upscaled efficient model",
    ),
    (
        "upstage/solar-pro-preview-instruct",
        "Solar Pro Preview — Advanced instruction following",
    ),
    // ═══ Ai21 (2 models) ═══
    (
        "ai21labs/jamba-1.5-large",
        "Jamba 1.5 Large 398B — Hybrid SSM-Transformer, 256K ctx",
    ),
    (
        "ai21labs/jamba-1.5-mini",
        "Jamba 1.5 Mini — Lightweight hybrid architecture",
    ),
    // ═══ Embedding Models (4 models) ═══
    (
        "nvidia/nv-embedqa-e5-v5",
        "NV Embed QA E5 — NVIDIA embedding for RAG retrieval",
    ),
    (
        "nvidia/nv-embedqa-mistral-7b-v2",
        "NV Embed QA Mistral 7B — Mistral-based embeddings",
    ),
    (
        "baai/bge-m3",
        "BGE-M3 — Multilingual multi-granularity embedding",
    ),
    (
        "nvidia/nv-rerankqa-mistral-4b-v3",
        "NV Rerank QA 4B — Re-ranking for RAG pipelines",
    ),
];

pub const NVIDIA_VISION_MODELS: &[&str] = &[
    "meta/llama-3.2-90b-vision-instruct",
    "meta/llama-3.2-11b-vision-instruct",
    "qwen/qwen3.5-397b-a17b",
    "qwen/qwen2-vl-72b-instruct",
    "qwen/qwen2-vl-7b-instruct",
    "microsoft/phi-3.5-vision-instruct",
    "google/gemma-3-27b-it",
    "moonshotai/kimi-vl-a3b-thinking",
];

pub const NVIDIA_CODE_MODELS: &[&str] = &[
    "deepseek-ai/deepseek-coder-v2-instruct",
    "deepseek-ai/deepseek-coder-v2-lite-instruct",
    "meta/codellama-70b-instruct",
    "qwen/qwen2.5-coder-32b-instruct",
    "qwen/qwen2.5-coder-7b-instruct",
    "mistralai/codestral-22b-instruct-v0.1",
    "mistralai/devstral-2-123b-instruct-2512",
    "mistralai/mamba-codestral-7b-v0.1",
    "google/codegemma-7b",
    "zhipuai/codegeex-4-9b",
    "ibm/granite-34b-code-instruct",
    "nvidia/usdcode-llama3.1-70b-instruct",
];

pub const NVIDIA_EMBEDDING_MODELS: &[&str] = &[
    "nvidia/nv-embedqa-e5-v5",
    "nvidia/nv-embedqa-mistral-7b-v2",
    "baai/bge-m3",
    "nvidia/nv-rerankqa-mistral-4b-v3",
    "snowflake/arctic-embed-l-v2.0",
];

/// Look up a model by ID. Returns (model_id, description) if found.
pub fn lookup_model(model_id: &str) -> Option<(&'static str, &'static str)> {
    NVIDIA_MODELS
        .iter()
        .find(|(id, _)| *id == model_id)
        .copied()
}

/// Filter models by provider prefix (e.g., "meta/", "nvidia/", "qwen/").
pub fn models_by_provider(provider_prefix: &str) -> Vec<(&'static str, &'static str)> {
    NVIDIA_MODELS
        .iter()
        .filter(|(id, _)| id.starts_with(provider_prefix))
        .copied()
        .collect()
}

/// List all unique provider prefixes.
pub fn list_providers() -> Vec<&'static str> {
    let mut providers: Vec<&str> = NVIDIA_MODELS
        .iter()
        .filter_map(|(id, _)| id.split('/').next())
        .collect();
    providers.sort_unstable();
    providers.dedup();
    providers
}

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
        // Optional: API key may not be configured in environment
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

        // NVIDIA NIM can be slow for large models (671B+ may need 3-5 min cold start)
        let timeout = if model.contains("671b")
            || model.contains("405b")
            || model.contains("340b")
            || model.contains("744b")
        {
            300 // 5 min for very large models
        } else {
            120 // 2 min for normal models
        };
        let (status, payload) = curl_post_json_with_timeout(
            request.endpoint.as_str(),
            &request.headers,
            &request.body,
            timeout,
        )?;
        if !(200..300).contains(&status) {
            let detail = payload
                .get("detail")
                .and_then(Value::as_str)
                .or_else(|| payload.get("title").and_then(Value::as_str))
                .or_else(|| payload.get("error").and_then(Value::as_str))
                .unwrap_or("unknown error");
            return Err(AgentError::SupervisorError(format!(
                "nvidia nim request failed with status {status}: {detail} (model={model})"
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
            input_tokens: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_count_93() {
        assert_eq!(
            NVIDIA_MODELS.len(),
            93,
            "Expected 93 NVIDIA NIM models, got {}",
            NVIDIA_MODELS.len()
        );
    }

    #[test]
    fn test_no_duplicate_model_ids() {
        let mut seen = std::collections::HashSet::new();
        for (id, _) in NVIDIA_MODELS {
            assert!(seen.insert(id), "Duplicate model ID: {id}");
        }
    }

    #[test]
    fn test_model_lookup() {
        let found = lookup_model("deepseek-ai/deepseek-r1");
        assert!(found.is_some());
        let (id, desc) = found.unwrap();
        assert_eq!(id, "deepseek-ai/deepseek-r1");
        assert!(desc.contains("Reasoning"));

        assert!(lookup_model("nonexistent/model").is_none());
    }

    #[test]
    fn test_models_by_provider() {
        let meta = models_by_provider("meta/");
        assert!(
            meta.len() >= 10,
            "Expected ≥10 Meta models, got {}",
            meta.len()
        );

        let nvidia = models_by_provider("nvidia/");
        assert!(
            nvidia.len() >= 7,
            "Expected ≥7 NVIDIA models, got {}",
            nvidia.len()
        );

        let qwen = models_by_provider("qwen/");
        assert!(
            qwen.len() >= 10,
            "Expected ≥10 Qwen models, got {}",
            qwen.len()
        );

        let empty = models_by_provider("nonexistent/");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_list_providers() {
        let providers = list_providers();
        assert!(
            providers.len() >= 14,
            "Expected ≥14 providers, got {}",
            providers.len()
        );
        assert!(providers.contains(&"meta"));
        assert!(providers.contains(&"nvidia"));
        assert!(providers.contains(&"qwen"));
        assert!(providers.contains(&"mistralai"));
        assert!(providers.contains(&"google"));
        assert!(providers.contains(&"microsoft"));
        assert!(providers.contains(&"deepseek-ai"));
    }

    #[test]
    fn test_vision_models_exist_in_catalog() {
        for vision_id in NVIDIA_VISION_MODELS {
            assert!(
                lookup_model(vision_id).is_some(),
                "Vision model '{vision_id}' not found in NVIDIA_MODELS"
            );
        }
    }

    #[test]
    fn test_code_models_exist_in_catalog() {
        for code_id in NVIDIA_CODE_MODELS {
            assert!(
                lookup_model(code_id).is_some(),
                "Code model '{code_id}' not found in NVIDIA_MODELS"
            );
        }
    }

    #[test]
    fn test_embedding_models_exist_in_catalog() {
        for embed_id in NVIDIA_EMBEDDING_MODELS {
            assert!(
                lookup_model(embed_id).is_some(),
                "Embedding model '{embed_id}' not found in NVIDIA_MODELS"
            );
        }
    }

    #[test]
    fn test_provider_new_and_env() {
        let provider = NvidiaProvider::new(Some("test-key".into()));
        assert_eq!(provider.name(), "nvidia");
        assert!(provider.cost_per_token() < 0.001);
    }

    #[test]
    fn test_build_request_format() {
        let provider = NvidiaProvider::new(Some("test-key".into()));
        let req = provider.build_request("hello", 100, "meta/llama-3.1-8b-instruct");
        assert_eq!(req.body["model"], "meta/llama-3.1-8b-instruct");
        assert_eq!(req.body["max_tokens"], 100);
        assert!(req.headers.contains_key("authorization"));
    }
}
