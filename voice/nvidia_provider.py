"""NVIDIA NIM provider for the voice pipeline.

Uses the OpenAI-compatible API at https://integrate.api.nvidia.com/v1.
Free tier includes 1000 API credits on signup.

Usage:
    from nvidia_provider import NvidiaNimProvider

    provider = NvidiaNimProvider(api_key="nvapi-xxx")
    response = provider.chat("What is the weather today?")

    # Or use as a Jarvis llm_responder:
    jarvis = JarvisPipeline(llm_responder=provider.chat)
"""

import os
from typing import Optional

try:
    from openai import OpenAI

    _HAS_OPENAI = True
except ImportError:
    _HAS_OPENAI = False


NVIDIA_NIM_BASE_URL = "https://integrate.api.nvidia.com/v1"

AVAILABLE_MODELS = [
    # DeepSeek (Best for Agents)
    "deepseek-ai/deepseek-v3_1-terminus",
    "deepseek-ai/deepseek-v3_1",
    "deepseek-ai/deepseek-v3",
    "deepseek-ai/deepseek-r1",
    "deepseek-ai/deepseek-r1-distill-llama-70b",
    "deepseek-ai/deepseek-r1-distill-qwen-32b",
    "deepseek-ai/deepseek-r1-distill-qwen-14b",
    "deepseek-ai/deepseek-r1-distill-llama-8b",
    # Zhipu GLM
    "zhipuai/glm-4.7",
    "zhipuai/glm-5-744b",
    # Moonshot / Kimi
    "moonshotai/kimi-k2-instruct",
    # Meta / Llama
    "meta/llama-4-scout-17b-16e-instruct",
    "meta/llama-4-maverick-17b-128e-instruct",
    "meta/llama-3.3-70b-instruct",
    "meta/llama-3.1-405b-instruct",
    "meta/llama-3.2-90b-vision-instruct",
    "meta/llama-3.1-8b-instruct",
    # NVIDIA Nemotron
    "nvidia/llama-3.1-nemotron-ultra-253b-v1",
    "nvidia/llama-3.1-nemotron-70b-instruct",
    "nvidia/nemotron-4-340b-instruct",
    "nvidia/nemotron-3-super-120b-a12b",
    "nvidia/nemotron-3-nano-30b-a3b",
    # Qwen
    "qwen/qwen3.5-vl-400b",
    "qwen/qwen2.5-72b-instruct",
    "qwen/qwen2.5-coder-32b-instruct",
    "qwen/qwen2.5-7b-instruct",
    # Mistral
    "mistralai/mistral-large-2-instruct",
    "mistralai/mixtral-8x22b-instruct-v0.1",
    "mistralai/mistral-7b-instruct-v0.3",
    "mistralai/devstral-2-123b-instruct-2512",
    # MiniMax
    "minimax/minimax-m2.5",
    # Google Gemma
    "google/gemma-3-27b-it",
    "google/gemma-3-12b-it",
    # Microsoft Phi
    "microsoft/phi-4",
    "microsoft/phi-3-medium-128k-instruct",
    "microsoft/phi-3.5-vision-instruct",
    # IBM Granite
    "ibm/granite-3.1-8b-instruct",
    "ibm/granite-3.3-8b-instruct",
    # Writer
    "writer/palmyra-x-004",
]


class NvidiaNimProvider:
    """NVIDIA NIM provider using OpenAI-compatible API."""

    def __init__(
        self,
        api_key: Optional[str] = None,
        model: Optional[str] = None,
    ):
        self.api_key = api_key or os.environ.get("NVIDIA_NIM_API_KEY", "")
        self.model = model or "deepseek-ai/deepseek-v3_1-terminus"

        if not self.api_key:
            raise ValueError(
                "NVIDIA NIM API key required. "
                "Set NVIDIA_NIM_API_KEY env var or pass api_key parameter. "
                "Get a free key at https://build.nvidia.com"
            )

        if not _HAS_OPENAI:
            raise ImportError(
                "openai package is required for NVIDIA NIM provider. "
                "Install with: pip install openai"
            )

        self.client = OpenAI(
            base_url=NVIDIA_NIM_BASE_URL,
            api_key=self.api_key,
        )

    def chat(
        self,
        user_message: str,
        system_prompt: str = "You are a helpful voice assistant. Keep responses concise.",
    ) -> str:
        """Send a chat message and return the response text."""
        try:
            response = self.client.chat.completions.create(
                model=self.model,
                messages=[
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_message},
                ],
                temperature=0.7,
                max_tokens=512,
                top_p=0.95,
            )
            return response.choices[0].message.content or ""
        except Exception as e:
            return f"NVIDIA NIM error: {e}"

    def test_connection(self) -> str:
        """Test the connection and return status."""
        try:
            result = self.chat("Say hello in exactly 5 words.")
            return f"Connected to NVIDIA NIM ({self.model}): {result}"
        except Exception as e:
            return f"Connection failed: {e}"
