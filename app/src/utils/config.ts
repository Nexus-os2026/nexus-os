import type { NexusConfig } from "../types";

export function createDefaultConfig(): NexusConfig {
  return {
    llm: {
      default_model: "claude-sonnet-4-5",
      anthropic_api_key: "",
      openai_api_key: "",
      deepseek_api_key: "",
      gemini_api_key: "",
      nvidia_api_key: "",
      ollama_url: "http://localhost:11434",
    },
    search: {
      brave_api_key: "",
    },
    social: {
      x_api_key: "",
      x_api_secret: "",
      x_access_token: "",
      x_access_secret: "",
      facebook_page_token: "",
      instagram_access_token: "",
    },
    messaging: {
      telegram_bot_token: "",
      whatsapp_business_id: "",
      whatsapp_api_token: "",
      discord_bot_token: "",
      slack_bot_token: "",
    },
    voice: {
      whisper_model: "auto",
      wake_word: "hey nexus",
      tts_voice: "default",
    },
    privacy: {
      telemetry: false,
      audit_retention_days: 365,
    },
    governance: {
      enable_warden_review: false,
    },
  };
}

export function normalizeConfig(config: Partial<NexusConfig> | null | undefined): NexusConfig {
  const defaults = createDefaultConfig();

  return {
    ...defaults,
    ...config,
    llm: {
      ...defaults.llm,
      ...(config?.llm ?? {}),
    },
    search: {
      ...defaults.search,
      ...(config?.search ?? {}),
    },
    social: {
      ...defaults.social,
      ...(config?.social ?? {}),
    },
    messaging: {
      ...defaults.messaging,
      ...(config?.messaging ?? {}),
    },
    voice: {
      ...defaults.voice,
      ...(config?.voice ?? {}),
    },
    privacy: {
      ...defaults.privacy,
      ...(config?.privacy ?? {}),
    },
    governance: {
      ...defaults.governance,
      ...(config?.governance ?? {}),
    },
    hardware: config?.hardware,
    ollama: config?.ollama,
    models: config?.models,
    agents: config?.agents,
    agent_llm_assignments: config?.agent_llm_assignments,
  };
}
