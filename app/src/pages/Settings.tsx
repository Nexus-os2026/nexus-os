import { useState } from "react";
import type { NexusConfig } from "../types";

interface SettingsProps {
  config: NexusConfig;
  onChange: (next: NexusConfig) => void;
  onSave: () => void;
  saving: boolean;
}

export function Settings({ config, onChange, onSave, saving }: SettingsProps): JSX.Element {
  const [showSecrets, setShowSecrets] = useState(false);
  const secretType = showSecrets ? "text" : "password";

  return (
    <section className="grid h-[calc(100vh-10rem)] grid-cols-1 gap-4 overflow-y-auto pr-1 lg:grid-cols-2">
      <article className="nexus-panel p-5">
        <div className="mb-3 flex items-center justify-between">
          <h2 className="nexus-display text-xl text-cyan-100">API Configuration</h2>
          <button
            onClick={() => setShowSecrets((prev) => !prev)}
            className="nexus-btn nexus-btn-secondary"
          >
            {showSecrets ? "Hide Keys" : "Show Keys"}
          </button>
        </div>
        <div className="space-y-3">
          <LabelledInput
            label="Anthropic API Key"
            type={secretType}
            value={config.llm.anthropic_api_key}
            onChange={(value) => onChange({ ...config, llm: { ...config.llm, anthropic_api_key: value } })}
          />
          <LabelledInput
            label="OpenAI API Key"
            type={secretType}
            value={config.llm.openai_api_key}
            onChange={(value) => onChange({ ...config, llm: { ...config.llm, openai_api_key: value } })}
          />
          <LabelledInput
            label="Brave Search API Key"
            type={secretType}
            value={config.search.brave_api_key}
            onChange={(value) => onChange({ ...config, search: { ...config.search, brave_api_key: value } })}
          />
          <LabelledInput
            label="Telegram Bot Token"
            type={secretType}
            value={config.messaging.telegram_bot_token}
            onChange={(value) => onChange({ ...config, messaging: { ...config.messaging, telegram_bot_token: value } })}
          />
        </div>
      </article>

      <article className="nexus-panel p-5">
        <h2 className="nexus-display text-xl text-cyan-100">Voice Settings</h2>
        <div className="mt-3 space-y-3">
          <LabelledInput
            label="Whisper Model"
            value={config.voice.whisper_model}
            onChange={(value) => onChange({ ...config, voice: { ...config.voice, whisper_model: value } })}
          />
          <LabelledInput
            label="Wake Word"
            value={config.voice.wake_word}
            onChange={(value) => onChange({ ...config, voice: { ...config.voice, wake_word: value } })}
          />
          <LabelledInput
            label="TTS Voice"
            value={config.voice.tts_voice}
            onChange={(value) => onChange({ ...config, voice: { ...config.voice, tts_voice: value } })}
          />
          <LabelledInput
            label="Ollama URL"
            value={config.llm.ollama_url}
            onChange={(value) => onChange({ ...config, llm: { ...config.llm, ollama_url: value } })}
          />
        </div>
      </article>

      <article className="nexus-panel p-5">
        <h2 className="nexus-display text-xl text-cyan-100">Privacy</h2>
        <div className="mt-4 space-y-4">
          <label className="flex items-center justify-between rounded-lg border border-slate-700/80 bg-slate-950 p-3">
            <span className="text-sm text-cyan-50">Telemetry</span>
            <input
              type="checkbox"
              checked={config.privacy.telemetry}
              onChange={(event) =>
                onChange({ ...config, privacy: { ...config.privacy, telemetry: event.target.checked } })
              }
            />
          </label>
          <LabelledInput
            label="Audit Retention (days)"
            value={String(config.privacy.audit_retention_days)}
            onChange={(value) =>
              onChange({
                ...config,
                privacy: {
                  ...config.privacy,
                  audit_retention_days: Number.isFinite(Number(value)) ? Number(value) : 365
                }
              })
            }
          />
        </div>
      </article>

      <article className="nexus-panel p-5">
        <h2 className="nexus-display text-xl text-cyan-100">About</h2>
        <ul className="mt-3 space-y-2 text-sm text-slate-200">
          <li>Version: 1.0.0</li>
          <li>License: TBD</li>
          <li>Project: github.com/nex-lang/nexus-os</li>
          <li>Stack: Tauri + React + TypeScript + Tailwind</li>
        </ul>
        <button
          onClick={onSave}
          disabled={saving}
          className="nexus-btn nexus-btn-primary mt-5 px-4 py-2 text-sm font-semibold disabled:cursor-not-allowed disabled:opacity-70"
        >
          {saving ? "Saving..." : "Save Settings"}
        </button>
      </article>
    </section>
  );
}

interface LabelledInputProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  type?: string;
}

function LabelledInput({ label, value, onChange, type = "text" }: LabelledInputProps): JSX.Element {
  return (
    <label className="block">
      <span className="mb-1 block text-xs text-cyan-100/65">{label}</span>
      <input
        type={type}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className="nexus-input"
      />
    </label>
  );
}
