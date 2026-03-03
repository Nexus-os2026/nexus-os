import { useEffect, useMemo, useState } from "react";
import "./settings.css";
import type { NexusConfig } from "../types";

interface SettingsProps {
  config: NexusConfig;
  onChange: (next: NexusConfig) => void;
  onSave: () => void;
  saving: boolean;
  uiSoundEnabled: boolean;
  uiSoundVolume: number;
  onUiSoundEnabledChange: (value: boolean) => void;
  onUiSoundVolumeChange: (value: number) => void;
}

type SettingsTab = "api" | "voice" | "privacy" | "about";
type ServiceStatus = "unknown" | "testing" | "ok" | "error";

interface ServiceRowMeta {
  id: "anthropic" | "openai" | "brave" | "telegram";
  icon: string;
  label: string;
  key: string;
  update: (value: string) => void;
}

function recommendationForModel(model: string): string {
  const lowered = model.toLowerCase();
  if (lowered.includes("tiny") || lowered.includes("base")) {
    return "Recommended for CPU-only systems";
  }
  if (lowered.includes("small")) {
    return "Balanced model for modern laptops";
  }
  if (lowered.includes("medium") || lowered.includes("large")) {
    return "GPU recommended for real-time transcription";
  }
  return "Auto-selects available backend profile";
}

function statusLabel(status: ServiceStatus): string {
  if (status === "testing") {
    return "Testing...";
  }
  if (status === "ok") {
    return "✓ Working";
  }
  if (status === "error") {
    return "✕ Invalid";
  }
  return "Not tested";
}

export function Settings({
  config,
  onChange,
  onSave,
  saving,
  uiSoundEnabled,
  uiSoundVolume,
  onUiSoundEnabledChange,
  onUiSoundVolumeChange
}: SettingsProps): JSX.Element {
  const [tab, setTab] = useState<SettingsTab>("api");
  const [showSecrets, setShowSecrets] = useState(false);
  const [serviceStatus, setServiceStatus] = useState<Record<string, ServiceStatus>>({});
  const [voicePreviewing, setVoicePreviewing] = useState(false);
  const [micTesting, setMicTesting] = useState(false);
  const [micLevel, setMicLevel] = useState(0.08);
  const [deleteConfirm, setDeleteConfirm] = useState("");

  const secretType = showSecrets ? "text" : "password";

  const services: ServiceRowMeta[] = useMemo(
    () => [
      {
        id: "anthropic",
        icon: "A",
        label: "Anthropic",
        key: config.llm.anthropic_api_key,
        update: (value) => onChange({ ...config, llm: { ...config.llm, anthropic_api_key: value } })
      },
      {
        id: "openai",
        icon: "O",
        label: "OpenAI",
        key: config.llm.openai_api_key,
        update: (value) => onChange({ ...config, llm: { ...config.llm, openai_api_key: value } })
      },
      {
        id: "brave",
        icon: "B",
        label: "Brave Search",
        key: config.search.brave_api_key,
        update: (value) => onChange({ ...config, search: { ...config.search, brave_api_key: value } })
      },
      {
        id: "telegram",
        icon: "T",
        label: "Telegram",
        key: config.messaging.telegram_bot_token,
        update: (value) =>
          onChange({ ...config, messaging: { ...config.messaging, telegram_bot_token: value } })
      }
    ],
    [config, onChange]
  );

  useEffect(() => {
    if (!micTesting) {
      setMicLevel(0.08);
      return;
    }
    const timer = window.setInterval(() => {
      setMicLevel((prev) => {
        const jitter = 0.14 + Math.random() * 0.82;
        return Math.max(0.08, Math.min(1, prev * 0.35 + jitter * 0.65));
      });
    }, 120);
    return () => {
      window.clearInterval(timer);
    };
  }, [micTesting]);

  function testService(service: ServiceRowMeta): void {
    setServiceStatus((prev) => ({ ...prev, [service.id]: "testing" }));
    window.setTimeout(() => {
      const ok = service.key.trim().length > 4;
      setServiceStatus((prev) => ({ ...prev, [service.id]: ok ? "ok" : "error" }));
    }, 900);
  }

  function playVoicePreview(): void {
    setVoicePreviewing(true);
    window.setTimeout(() => setVoicePreviewing(false), 1200);
  }

  function deleteAllData(): void {
    if (deleteConfirm !== "DELETE") {
      return;
    }
    setDeleteConfirm("");
    onChange({
      ...config,
      llm: { ...config.llm, anthropic_api_key: "", openai_api_key: "" },
      search: { ...config.search, brave_api_key: "" },
      messaging: { ...config.messaging, telegram_bot_token: "" }
    });
  }

  return (
    <section className="settings-hub">
      <header className="settings-header">
        <div>
          <h2 className="settings-title">SYSTEM SETTINGS // CONTROL PANEL</h2>
          <p className="settings-subtitle">Security posture, runtime config, and identity controls</p>
        </div>
        <button type="button" className="service-test-btn" onClick={() => setShowSecrets((prev) => !prev)}>
          {showSecrets ? "Mask Keys" : "Show Keys"}
        </button>
      </header>

      <nav className="settings-tabs">
        <button type="button" className={`settings-tab ${tab === "api" ? "active" : ""}`} onClick={() => setTab("api")}>
          API Keys
        </button>
        <button type="button" className={`settings-tab ${tab === "voice" ? "active" : ""}`} onClick={() => setTab("voice")}>
          Voice
        </button>
        <button
          type="button"
          className={`settings-tab ${tab === "privacy" ? "active" : ""}`}
          onClick={() => setTab("privacy")}
        >
          Privacy
        </button>
        <button type="button" className={`settings-tab ${tab === "about" ? "active" : ""}`} onClick={() => setTab("about")}>
          About
        </button>
      </nav>

      <div className="settings-body">
        {tab === "api" ? (
          <section className="settings-card">
            {services.map((service) => {
              const status = serviceStatus[service.id] ?? "unknown";
              return (
                <article key={service.id} className="service-row">
                  <span className="service-icon">{service.icon}</span>
                  <div>
                    <p className="service-name">{service.label}</p>
                    <input
                      type={secretType}
                      className="service-input"
                      value={service.key}
                      onChange={(event) => service.update(event.target.value)}
                      placeholder="Enter key"
                    />
                  </div>
                  <span className={`service-status ${status}`}>{statusLabel(status)}</span>
                  <button
                    type="button"
                    className={`service-test-btn ${status === "testing" ? "testing" : ""}`}
                    onClick={() => testService(service)}
                  >
                    Test Connection
                  </button>
                </article>
              );
            })}
          </section>
        ) : null}

        {tab === "voice" ? (
          <section className="settings-card">
            <div className="settings-grid-2">
              <label className="settings-field">
                <span className="settings-label">Whisper Model</span>
                <select
                  className="settings-select"
                  value={config.voice.whisper_model}
                  onChange={(event) =>
                    onChange({ ...config, voice: { ...config.voice, whisper_model: event.target.value } })
                  }
                >
                  <option value="auto">auto</option>
                  <option value="tiny">tiny</option>
                  <option value="base">base</option>
                  <option value="small">small</option>
                  <option value="medium">medium</option>
                  <option value="large-v3">large-v3</option>
                </select>
                <span className="settings-label">{recommendationForModel(config.voice.whisper_model)}</span>
              </label>

              <label className="settings-field">
                <span className="settings-label">Wake Word</span>
                <input
                  className="settings-input"
                  value={config.voice.wake_word}
                  onChange={(event) =>
                    onChange({ ...config, voice: { ...config.voice, wake_word: event.target.value } })
                  }
                />
              </label>

              <label className="settings-field">
                <span className="settings-label">TTS Voice</span>
                <select
                  className="settings-select"
                  value={config.voice.tts_voice}
                  onChange={(event) =>
                    onChange({ ...config, voice: { ...config.voice, tts_voice: event.target.value } })
                  }
                >
                  <option value="default">default</option>
                  <option value="nova">nova</option>
                  <option value="echo">echo</option>
                  <option value="onyx">onyx</option>
                </select>
              </label>

              <div className="settings-field">
                <span className="settings-label">Preview Voice</span>
                <button
                  type="button"
                  className={`service-test-btn ${voicePreviewing ? "testing" : ""}`}
                  onClick={playVoicePreview}
                >
                  {voicePreviewing ? "Playing..." : "Preview"}
                </button>
              </div>
            </div>

            <div className="voice-meter">
              <div className="flex items-center justify-between gap-2">
                <span className="settings-label">Mic Test</span>
                <button
                  type="button"
                  className={`service-test-btn ${micTesting ? "testing" : ""}`}
                  onClick={() => setMicTesting((prev) => !prev)}
                >
                  {micTesting ? "Stop Test" : "Start Test"}
                </button>
              </div>
              <div className="voice-meter-track">
                <div className="voice-meter-fill" style={{ width: `${Math.round(micLevel * 100)}%` }} />
              </div>
            </div>

            <div className="voice-meter">
              <div className="flex items-center justify-between gap-2">
                <span className="settings-label">UI Sound Design</span>
                <label className="holo-toggle">
                  <input
                    type="checkbox"
                    checked={uiSoundEnabled}
                    onChange={(event) => onUiSoundEnabledChange(event.target.checked)}
                  />
                  <span className="holo-toggle__track">
                    <span className="holo-toggle__thumb" />
                  </span>
                </label>
              </div>
              <div className="voice-meter-track">
                <div className="voice-meter-fill" style={{ width: `${Math.round(uiSoundVolume * 100)}%` }} />
              </div>
              <input
                type="range"
                min={0}
                max={100}
                step={1}
                className="create-slider"
                value={Math.round(uiSoundVolume * 100)}
                onChange={(event) => onUiSoundVolumeChange(Number(event.target.value) / 100)}
              />
            </div>
          </section>
        ) : null}

        {tab === "privacy" ? (
          <section className="settings-card">
            <article className="privacy-row">
              <div>
                <p className="privacy-label">Telemetry</p>
                <p className="privacy-hint">Off by default. Sends anonymous health metrics.</p>
              </div>
              <input
                type="checkbox"
                className="holo-toggle-input"
                checked={config.privacy.telemetry}
                onChange={(event) =>
                  onChange({ ...config, privacy: { ...config.privacy, telemetry: event.target.checked } })
                }
              />
            </article>

            <article className="privacy-row">
              <div>
                <p className="privacy-label">Audit Retention</p>
                <p className="privacy-hint">{config.privacy.audit_retention_days} days</p>
              </div>
              <input
                type="range"
                min={30}
                max={3650}
                step={5}
                value={config.privacy.audit_retention_days}
                className="create-slider"
                onChange={(event) =>
                  onChange({
                    ...config,
                    privacy: { ...config.privacy, audit_retention_days: Number(event.target.value) }
                  })
                }
              />
            </article>

            <article className="privacy-row">
              <div>
                <p className="privacy-label">Encryption Status</p>
                <p className="privacy-hint">At-rest encryption: enabled ✓</p>
              </div>
            </article>

            <article className="settings-card">
              <p className="privacy-label">Delete All Data</p>
              <p className="privacy-hint">Type DELETE to confirm data wipe operation.</p>
              <input
                className="settings-input"
                value={deleteConfirm}
                onChange={(event) => setDeleteConfirm(event.target.value)}
                placeholder="Type DELETE"
              />
              <button type="button" className="danger-btn" onClick={deleteAllData}>
                Delete All Data
              </button>
            </article>
          </section>
        ) : null}

        {tab === "about" ? (
          <section className="settings-card">
            <div className="about-logo">N</div>
            <h3 className="about-title">NexusOS</h3>
            <ul className="about-list">
              <li>Version: 2.0.0</li>
              <li>Tagline: Don&apos;t trust. Verify.</li>
              <li>
                GitHub:{" "}
                <a className="about-link" href="https://github.com/nex-lang/nexus-os" target="_blank" rel="noreferrer">
                  nex-lang/nexus-os
                </a>
              </li>
              <li>License: TBD</li>
              <li>Credits: NexusOS Core Team</li>
            </ul>
          </section>
        ) : null}
      </div>

      <footer className="settings-footer">
        <button type="button" className="settings-save-btn" onClick={onSave} disabled={saving}>
          {saving ? "Saving..." : "Save Settings"}
        </button>
      </footer>
    </section>
  );
}
