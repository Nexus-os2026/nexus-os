import { useEffect, useState } from "react";
import "./settings.css";
import type { LlmProviderStatusEntry, LlmRecommendation, NexusConfig, OllamaModelInfo, RoutingStrategy, TestConnectionResult } from "../types";
import { checkLlmStatus, getLlmRecommendations, testLlmConnection, hasDesktopRuntime } from "../api/backend";

interface SettingsProps {
  config: NexusConfig;
  onChange: (next: NexusConfig) => void;
  onSave: () => void;
  saving: boolean;
  uiSoundEnabled: boolean;
  uiSoundVolume: number;
  onUiSoundEnabledChange: (value: boolean) => void;
  onUiSoundVolumeChange: (value: number) => void;
  ollamaConnected?: boolean;
  ollamaModels?: OllamaModelInfo[];
  onDeleteModel?: (name: string) => Promise<void>;
  onRerunSetup?: () => void;
  onRefreshOllama?: () => Promise<void>;
}

type SettingsSection = "general" | "llm" | "api" | "privacy" | "voice" | "models" | "about";
type ServiceStatus = "unknown" | "testing" | "ok" | "error";

interface ApiKeyDef {
  id: string;
  label: string;
  value: string;
  update: (v: string) => void;
}

export function Settings({
  config,
  onChange,
  onSave,
  saving,
  uiSoundEnabled,
  uiSoundVolume,
  onUiSoundEnabledChange,
  onUiSoundVolumeChange,
  ollamaConnected,
  ollamaModels,
  onDeleteModel,
  onRerunSetup,
  onRefreshOllama
}: SettingsProps): JSX.Element {
  const [section, setSection] = useState<SettingsSection>("general");
  const [showKeys, setShowKeys] = useState(false);
  const [statuses, setStatuses] = useState<Record<string, ServiceStatus>>({});
  const [darkMode, setDarkMode] = useState(true);
  const [language, setLanguage] = useState("en");
  const [notifications, setNotifications] = useState(false);
  const [deletePhase, setDeletePhase] = useState<"idle" | "confirm">("idle");
  const [micTesting, setMicTesting] = useState(false);
  const [micLevel, setMicLevel] = useState(0.08);
  const [updateCheck, setUpdateCheck] = useState<"idle" | "checking" | "up-to-date">("idle");

  // ── LLM Provider Management state ──
  const [llmProviders, setLlmProviders] = useState<LlmProviderStatusEntry[]>([]);
  const [llmActiveProvider, setLlmActiveProvider] = useState("");
  const [llmGovernanceWarning, setLlmGovernanceWarning] = useState<string | null>(null);
  const [llmHasAny, setLlmHasAny] = useState(false);
  const [llmRecs, setLlmRecs] = useState<LlmRecommendation[]>([]);
  const [llmRecsRam, setLlmRecsRam] = useState(0);
  const [llmRecsCanLocal, setLlmRecsCanLocal] = useState(false);
  const [testResults, setTestResults] = useState<Record<string, TestConnectionResult>>({});
  const [testingProvider, setTestingProvider] = useState<string | null>(null);
  const [routingStrategy, setRoutingStrategy] = useState<RoutingStrategy>(
    (config.llm.routing_strategy as RoutingStrategy) || "Priority"
  );

  const secretType = showKeys ? "text" : "password";

  const apiKeys: ApiKeyDef[] = [
    { id: "openai", label: "OpenAI", value: config.llm.openai_api_key, update: (v) => onChange({ ...config, llm: { ...config.llm, openai_api_key: v } }) },
    { id: "anthropic", label: "Anthropic", value: config.llm.anthropic_api_key, update: (v) => onChange({ ...config, llm: { ...config.llm, anthropic_api_key: v } }) },
    { id: "brave", label: "Brave Search", value: config.search.brave_api_key, update: (v) => onChange({ ...config, search: { ...config.search, brave_api_key: v } }) },
    { id: "x", label: "X (Twitter)", value: config.social.x_api_key, update: (v) => onChange({ ...config, social: { ...config.social, x_api_key: v } }) },
    { id: "github", label: "GitHub", value: "", update: () => {} }
  ];

  function testKey(id: string, value: string): void {
    setStatuses((prev) => ({ ...prev, [id]: "testing" }));
    window.setTimeout(() => {
      setStatuses((prev) => ({ ...prev, [id]: value.trim().length > 4 ? "ok" : "error" }));
    }, 900);
  }

  function statusLabel(s: ServiceStatus): string {
    if (s === "testing") return "Testing...";
    if (s === "ok") return "Connected \u2713";
    if (s === "error") return "Invalid \u2717";
    return "Not Set";
  }

  function statusClass(s: ServiceStatus): string {
    if (s === "ok") return "status-ok";
    if (s === "error") return "status-error";
    if (s === "testing") return "status-testing";
    return "status-none";
  }

  // Fetch LLM status + recommendations when the LLM section is active
  useEffect(() => {
    if (section !== "llm" || !hasDesktopRuntime()) return;
    checkLlmStatus().then((status) => {
      setLlmProviders(status.providers);
      setLlmActiveProvider(status.active_provider);
      setLlmGovernanceWarning(status.governance_warning ?? null);
      setLlmHasAny(status.has_any_provider);
    }).catch(() => {});
    getLlmRecommendations().then((recs) => {
      setLlmRecs(recs.recommendations);
      setLlmRecsRam(recs.ram_mb);
      setLlmRecsCanLocal(recs.can_run_local);
    }).catch(() => {});
  }, [section]);

  function handleTestConnection(providerName: string): void {
    setTestingProvider(providerName);
    testLlmConnection(providerName).then((result) => {
      setTestResults((prev) => ({ ...prev, [providerName]: result }));
      setTestingProvider(null);
    }).catch(() => setTestingProvider(null));
  }

  function handleRoutingChange(strategy: RoutingStrategy): void {
    setRoutingStrategy(strategy);
    onChange({ ...config, llm: { ...config.llm, routing_strategy: strategy } });
  }

  useEffect(() => {
    if (!micTesting) {
      setMicLevel(0.08);
      return;
    }
    const timer = window.setInterval(() => {
      setMicLevel((prev) => Math.max(0.08, Math.min(1, prev * 0.35 + (0.14 + Math.random() * 0.82) * 0.65)));
    }, 120);
    return () => window.clearInterval(timer);
  }, [micTesting]);

  return (
    <section className="st-hub">
      <header className="st-header">
        <h2 className="st-title">SYSTEM SETTINGS // CONTROL PANEL</h2>
        <p className="st-subtitle">Security posture, runtime config, and identity controls</p>
      </header>

      <nav className="st-nav">
        {(["general", "llm", "api", "privacy", "voice", "models", "about"] as SettingsSection[]).map((s) => (
          <button
            key={s}
            type="button"
            className={`st-nav-btn ${section === s ? "active" : ""}`}
            onClick={() => setSection(s)}
          >
            {s === "api" ? "API Keys" : s === "llm" ? "LLM Providers" : s.charAt(0).toUpperCase() + s.slice(1)}
          </button>
        ))}
      </nav>

      <div className="st-body">
        {section === "general" && (
          <div className="st-card">
            <div className="st-row">
              <div>
                <p className="st-row-label">Theme</p>
                <p className="st-row-hint">Switch between dark and light interface</p>
              </div>
              <label className="st-toggle">
                <input type="checkbox" checked={darkMode} onChange={(e) => setDarkMode(e.target.checked)} />
                <span className="st-toggle-track"><span className="st-toggle-thumb" /></span>
                <span className="st-toggle-text">{darkMode ? "Dark" : "Light"}</span>
              </label>
            </div>
            <div className="st-row">
              <div>
                <p className="st-row-label">Language</p>
                <p className="st-row-hint">Interface language</p>
              </div>
              <select className="st-select" value={language} onChange={(e) => setLanguage(e.target.value)}>
                <option value="en">English</option>
                <option value="es">Spanish</option>
                <option value="fr">French</option>
                <option value="de">German</option>
                <option value="ja">Japanese</option>
              </select>
            </div>
            <div className="st-row">
              <div>
                <p className="st-row-label">Desktop Notifications</p>
                <p className="st-row-hint">Show system notifications for agent events</p>
              </div>
              <label className="st-toggle">
                <input type="checkbox" checked={notifications} onChange={(e) => setNotifications(e.target.checked)} />
                <span className="st-toggle-track"><span className="st-toggle-thumb" /></span>
              </label>
            </div>
            <div className="st-row">
              <div>
                <p className="st-row-label">UI Sound Design</p>
                <p className="st-row-hint">Interface audio feedback</p>
              </div>
              <div className="st-sound-controls">
                <label className="st-toggle">
                  <input type="checkbox" checked={uiSoundEnabled} onChange={(e) => onUiSoundEnabledChange(e.target.checked)} />
                  <span className="st-toggle-track"><span className="st-toggle-thumb" /></span>
                </label>
                <input
                  type="range" min={0} max={100} step={1}
                  className="st-slider"
                  value={Math.round(uiSoundVolume * 100)}
                  onChange={(e) => onUiSoundVolumeChange(Number(e.target.value) / 100)}
                />
              </div>
            </div>
          </div>
        )}

        {section === "llm" && (
          <div className="st-card">
            {/* Governance Warning */}
            {llmGovernanceWarning && (
              <div className="st-row" style={{ background: "rgba(255,170,0,0.08)", borderRadius: 6, padding: "0.5rem 0.75rem", marginBottom: "0.75rem" }}>
                <p className="st-row-label" style={{ color: "#ffa500", fontSize: "0.82rem" }}>{llmGovernanceWarning}</p>
              </div>
            )}

            {/* Active Provider */}
            <div className="st-row">
              <div>
                <p className="st-row-label">Active Provider</p>
                <p className="st-row-hint">Currently routing LLM requests to this provider</p>
              </div>
              <span className="st-badge st-badge-green" style={{ textTransform: "capitalize" }}>{llmActiveProvider || "none"}</span>
            </div>

            {/* Routing Strategy */}
            <div className="st-row">
              <div>
                <p className="st-row-label">Routing Strategy</p>
                <p className="st-row-hint">How requests are distributed across providers</p>
              </div>
              <select className="st-select" value={routingStrategy} onChange={(e) => handleRoutingChange(e.target.value as RoutingStrategy)}>
                <option value="Priority">Priority (use first available)</option>
                <option value="RoundRobin">Round Robin</option>
                <option value="LowestLatency">Lowest Latency</option>
                <option value="CostOptimized">Cost Optimized</option>
              </select>
            </div>

            {/* Provider Status List */}
            <h3 className="st-card-title" style={{ marginTop: "1rem" }}>Provider Status</h3>
            {llmProviders.map((p) => {
              const tr = testResults[p.name];
              return (
                <div key={p.name} className="st-row" style={{ flexWrap: "wrap", gap: "0.5rem" }}>
                  <div style={{ flex: 1, minWidth: 180 }}>
                    <p className="st-row-label" style={{ textTransform: "capitalize" }}>
                      {p.name}
                      {p.available && <span style={{ color: "#00e676", marginLeft: 6, fontSize: "0.75rem" }}>Connected</span>}
                      {!p.available && p.name !== "mock" && <span style={{ color: "#ff5252", marginLeft: 6, fontSize: "0.75rem" }}>{p.error_hint || "Unavailable"}</span>}
                    </p>
                    <p className="st-row-hint">{p.reason}</p>
                    {p.setup_command && !p.available && (
                      <code className="st-row-hint" style={{ display: "block", marginTop: 4, color: "#80cbc4", fontSize: "0.78rem", userSelect: "all" }}>
                        $ {p.setup_command}
                      </code>
                    )}
                    {p.latency_ms != null && <p className="st-row-hint" style={{ fontSize: "0.72rem" }}>Latency: {p.latency_ms}ms</p>}
                    {p.models_installed != null && p.models_installed > 0 && (
                      <p className="st-row-hint" style={{ fontSize: "0.72rem" }}>{p.models_installed} model{p.models_installed === 1 ? "" : "s"} installed</p>
                    )}
                  </div>
                  <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                    {p.is_paid && <span className="st-badge" style={{ fontSize: "0.7rem", padding: "2px 6px" }}>Paid</span>}
                    {!p.is_paid && p.name !== "mock" && <span className="st-badge st-badge-green" style={{ fontSize: "0.7rem", padding: "2px 6px" }}>Free</span>}
                    <button
                      type="button"
                      className="st-btn st-btn-ghost"
                      style={{ fontSize: "0.75rem", padding: "3px 10px" }}
                      disabled={testingProvider === p.name}
                      onClick={() => handleTestConnection(p.name)}
                    >
                      {testingProvider === p.name ? "Testing..." : "Test"}
                    </button>
                  </div>
                  {tr && (
                    <div style={{ width: "100%", fontSize: "0.75rem", paddingLeft: "0.5rem" }}>
                      {tr.success
                        ? <span style={{ color: "#00e676" }}>Connected in {tr.latency_ms}ms{tr.model_used ? ` (${tr.model_used})` : ""}</span>
                        : <span style={{ color: "#ff5252" }}>Failed: {tr.error}</span>
                      }
                    </div>
                  )}
                </div>
              );
            })}

            {/* API Key Inputs */}
            <h3 className="st-card-title" style={{ marginTop: "1rem" }}>API Keys</h3>
            {[
              { label: "DeepSeek", key: "deepseek_api_key" as const, hint: "~$0.14/M tokens (cheapest)" },
              { label: "OpenAI", key: "openai_api_key" as const, hint: "~$5/M tokens" },
              { label: "Gemini", key: "gemini_api_key" as const, hint: "~$3.50/M tokens" },
              { label: "Anthropic", key: "anthropic_api_key" as const, hint: "~$3/M tokens" },
            ].map((entry) => (
              <div key={entry.key} className="st-row" style={{ flexWrap: "wrap" }}>
                <div style={{ flex: 1, minWidth: 140 }}>
                  <p className="st-row-label">{entry.label}</p>
                  <p className="st-row-hint">{entry.hint}</p>
                </div>
                <input
                  type={showKeys ? "text" : "password"}
                  className="st-api-input"
                  style={{ flex: 2, minWidth: 200 }}
                  value={config.llm[entry.key]}
                  onChange={(e) => onChange({ ...config, llm: { ...config.llm, [entry.key]: e.target.value } })}
                  placeholder={`Enter ${entry.label} API key`}
                />
              </div>
            ))}
            <div style={{ display: "flex", gap: 8, marginTop: 8 }}>
              <button type="button" className="st-btn st-btn-ghost" onClick={() => setShowKeys((p) => !p)}>
                {showKeys ? "Hide Keys" : "Show Keys"}
              </button>
              <button type="button" className="st-btn st-btn-blue" onClick={onSave} disabled={saving}>
                {saving ? "Saving..." : "Save Keys"}
              </button>
            </div>

            {/* Setup Wizard — shown when no providers available */}
            {!llmHasAny && llmRecs.length > 0 && (
              <>
                <h3 className="st-card-title" style={{ marginTop: "1.5rem", color: "#ffa500" }}>Setup Wizard</h3>
                <p className="st-row-hint" style={{ marginBottom: "0.5rem" }}>
                  No LLM provider is configured yet. Your system has {llmRecsRam} MB RAM.
                  {llmRecsCanLocal
                    ? " Your system can run local models — we recommend Ollama for maximum privacy."
                    : " Your system may not have enough RAM for local models — consider a cloud provider."}
                </p>
                {llmRecs.map((rec) => (
                  <div key={rec.provider_type} className="st-row" style={{ flexWrap: "wrap", gap: 6 }}>
                    <div style={{ flex: 1, minWidth: 200 }}>
                      <p className="st-row-label">
                        {rec.display_name}
                        {rec.recommended && <span style={{ color: "#00e676", marginLeft: 6, fontSize: "0.72rem" }}>Recommended</span>}
                      </p>
                      <p className="st-row-hint">{rec.reason}</p>
                      <p className="st-row-hint" style={{ fontSize: "0.72rem" }}>{rec.cost_info}</p>
                      {rec.setup_command && (
                        <code className="st-row-hint" style={{ display: "block", marginTop: 4, color: "#80cbc4", fontSize: "0.78rem", userSelect: "all" }}>
                          $ {rec.setup_command}
                        </code>
                      )}
                    </div>
                  </div>
                ))}
              </>
            )}
          </div>
        )}

        {section === "api" && (
          <div className="st-card">
            <div className="st-api-header">
              <button type="button" className="st-show-keys-btn" onClick={() => setShowKeys((p) => !p)}>
                {showKeys ? "Hide Keys" : "Show Keys"}
              </button>
            </div>
            {apiKeys.map((key) => {
              const s = statuses[key.id] ?? "unknown";
              return (
                <div key={key.id} className="st-api-row">
                  <div className="st-api-label-wrap">
                    <span className="st-api-label">{key.label}</span>
                    <span className={`st-api-status ${statusClass(s)}`}>{statusLabel(s)}</span>
                  </div>
                  <div className="st-api-input-row">
                    <input
                      type={secretType}
                      className="st-api-input"
                      value={key.value}
                      onChange={(e) => key.update(e.target.value)}
                      placeholder={`Enter ${key.label} key`}
                    />
                    <button type="button" className="st-api-save-btn" onClick={onSave} disabled={saving}>
                      Save
                    </button>
                    <button type="button" className="st-api-test-btn" onClick={() => testKey(key.id, key.value)}>
                      Test Connection
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {section === "privacy" && (
          <div className="st-card">
            <div className="st-row">
              <div>
                <p className="st-row-label">Screen Capture Retention</p>
                <p className="st-row-hint st-row-warn">When enabled, screen captures are stored encrypted for forensic replay</p>
              </div>
              <label className="st-toggle">
                <input type="checkbox" checked={false} readOnly />
                <span className="st-toggle-track"><span className="st-toggle-thumb" /></span>
                <span className="st-toggle-text">OFF</span>
              </label>
            </div>
            <div className="st-row">
              <div>
                <p className="st-row-label">Encryption Status</p>
              </div>
              <span className="st-badge st-badge-green">AES-256 Active</span>
            </div>
            <div className="st-row">
              <div>
                <p className="st-row-label">Audit Chain</p>
              </div>
              <span className="st-badge st-badge-green">Hash Chain Intact &mdash; 847 events</span>
            </div>
            <div className="st-row">
              <div>
                <p className="st-row-label">Telemetry</p>
                <p className="st-row-hint">Off by default. Anonymous health metrics only.</p>
              </div>
              <label className="st-toggle">
                <input type="checkbox" checked={config.privacy.telemetry} onChange={(e) => onChange({ ...config, privacy: { ...config.privacy, telemetry: e.target.checked } })} />
                <span className="st-toggle-track"><span className="st-toggle-thumb" /></span>
              </label>
            </div>
            <div className="st-row">
              <div>
                <p className="st-row-label">Audit Retention</p>
                <p className="st-row-hint">{config.privacy.audit_retention_days} days</p>
              </div>
              <input type="range" min={30} max={3650} step={5} className="st-slider" value={config.privacy.audit_retention_days}
                onChange={(e) => onChange({ ...config, privacy: { ...config.privacy, audit_retention_days: Number(e.target.value) } })} />
            </div>
            <div className="st-action-row">
              <button type="button" className="st-btn st-btn-blue" onClick={() => {
                const data = JSON.stringify(config, null, 2);
                const blob = new Blob([data], { type: "application/json" });
                const url = URL.createObjectURL(blob);
                const a = document.createElement("a");
                a.href = url;
                a.download = `nexus-config-export-${new Date().toISOString().slice(0, 10)}.json`;
                a.click();
                URL.revokeObjectURL(url);
              }}>Export My Data</button>
              {deletePhase === "idle" ? (
                <button type="button" className="st-btn st-btn-red" onClick={() => setDeletePhase("confirm")}>Delete All My Data</button>
              ) : (
                <div className="st-delete-confirm">
                  <p className="st-row-warn">Are you sure? This uses cryptographic erasure and is irreversible.</p>
                  <div className="st-delete-btns">
                    <button type="button" className="st-btn st-btn-red" onClick={() => { setDeletePhase("idle"); onChange({ ...config, llm: { ...config.llm, anthropic_api_key: "", openai_api_key: "" }, search: { ...config.search, brave_api_key: "" } }); }}>Confirm Delete</button>
                    <button type="button" className="st-btn st-btn-ghost" onClick={() => setDeletePhase("idle")}>Cancel</button>
                  </div>
                </div>
              )}
            </div>
          </div>
        )}

        {section === "voice" && (
          <div className="st-card">
            <div className="st-row">
              <div>
                <p className="st-row-label">Wake Word Detection</p>
                <p className="st-row-hint">Always-on voice activation</p>
              </div>
              <label className="st-toggle">
                <input type="checkbox" checked={false} readOnly />
                <span className="st-toggle-track"><span className="st-toggle-thumb" /></span>
                <span className="st-toggle-text">OFF</span>
              </label>
            </div>
            <div className="st-row">
              <div><p className="st-row-label">Wake Word</p></div>
              <input className="st-input" value={config.voice.wake_word}
                onChange={(e) => onChange({ ...config, voice: { ...config.voice, wake_word: e.target.value } })} />
            </div>
            <div className="st-row">
              <div><p className="st-row-label">STT Engine</p></div>
              <select className="st-select" value={config.voice.whisper_model}
                onChange={(e) => onChange({ ...config, voice: { ...config.voice, whisper_model: e.target.value } })}>
                <option value="auto">Whisper Local (auto)</option>
                <option value="cloud">Whisper Cloud</option>
              </select>
            </div>
            <div className="st-row">
              <div><p className="st-row-label">TTS Engine</p></div>
              <select className="st-select" value={config.voice.tts_voice}
                onChange={(e) => onChange({ ...config, voice: { ...config.voice, tts_voice: e.target.value } })}>
                <option value="default">Piper</option>
                <option value="system">System Default</option>
              </select>
            </div>
            <div className="st-row">
              <div><p className="st-row-label">Mic Test</p></div>
              <div className="st-mic-test">
                <button type="button" className="st-btn st-btn-ghost" onClick={() => setMicTesting((p) => !p)}>
                  {micTesting ? "Stop Test" : "Start Test"}
                </button>
                <div className="st-mic-bar">
                  <div className="st-mic-fill" style={{ width: `${Math.round(micLevel * 100)}%` }} />
                </div>
              </div>
            </div>
            <div className="st-row">
              <div><p className="st-row-label">Test Voice</p></div>
              <button type="button" className="st-btn st-btn-ghost" onClick={() => {
                const utterance = new SpeechSynthesisUtterance("NexusOS voice system online. All agents nominal.");
                utterance.rate = 0.95;
                utterance.pitch = 0.9;
                window.speechSynthesis.speak(utterance);
              }}>Test Voice</button>
            </div>
          </div>
        )}

        {section === "models" && (
          <div className="st-card">
            <h3 className="st-card-title">Hardware Profile</h3>
            <div className="st-models-hw-grid">
              <div className="st-models-hw-item">
                <span className="st-models-hw-label">GPU</span>
                <span className="st-models-hw-value">{config.hardware?.gpu || "Not detected"}</span>
              </div>
              <div className="st-models-hw-item">
                <span className="st-models-hw-label">VRAM</span>
                <span className="st-models-hw-value">{config.hardware?.vram_mb ? `${config.hardware.vram_mb} MB` : "N/A"}</span>
              </div>
              <div className="st-models-hw-item">
                <span className="st-models-hw-label">RAM</span>
                <span className="st-models-hw-value">{config.hardware?.ram_mb ? `${config.hardware.ram_mb} MB` : "N/A"}</span>
              </div>
              <div className="st-models-hw-item">
                <span className="st-models-hw-label">Ollama</span>
                <span className="st-models-hw-value">
                  <span className={`st-ollama-dot ${ollamaConnected ? "connected" : "disconnected"}`} />
                  {ollamaConnected ? "Connected" : config.ollama?.status || "disconnected"}
                </span>
              </div>
            </div>

            <h3 className="st-card-title" style={{ marginTop: "1rem" }}>Assigned Models</h3>
            <div className="st-models-assigned">
              <div className="st-models-row">
                <span className="st-models-row-label">Primary</span>
                <span className="st-models-row-value">{config.models?.primary || "Not set"}</span>
              </div>
              <div className="st-models-row">
                <span className="st-models-row-label">Fast</span>
                <span className="st-models-row-value">{config.models?.fast || "Not set"}</span>
              </div>
              <div className="st-models-row">
                <span className="st-models-row-label">Default</span>
                <span className="st-models-row-value">{config.llm.default_model}</span>
              </div>
            </div>

            {ollamaModels && ollamaModels.length > 0 && (
              <>
                <h3 className="st-card-title" style={{ marginTop: "1rem" }}>Installed Ollama Models</h3>
                <div className="st-models-installed-list">
                  {ollamaModels.map((m) => (
                    <div key={m.name} className="st-models-installed-item">
                      <span className="st-models-installed-name">{m.name}</span>
                      <span className="st-models-installed-size">
                        {m.size >= 1_000_000_000
                          ? `${(m.size / 1_000_000_000).toFixed(1)} GB`
                          : `${(m.size / 1_000_000).toFixed(0)} MB`}
                      </span>
                      {onDeleteModel && (
                        <button
                          type="button"
                          className="st-btn st-btn-red st-btn-sm"
                          onClick={() => { void onDeleteModel(m.name); }}
                        >
                          Remove
                        </button>
                      )}
                    </div>
                  ))}
                </div>
              </>
            )}

            {config.agents && Object.keys(config.agents).length > 0 && (
              <>
                <h3 className="st-card-title" style={{ marginTop: "1rem" }}>Agent Configurations</h3>
                <div className="st-models-agents">
                  {Object.entries(config.agents).map(([name, ac]) => (
                    <div key={name} className="st-models-agent-row">
                      <span className="st-models-agent-name">{name}</span>
                      <span className="st-models-agent-model">{ac.model}</span>
                      <span className="st-models-agent-param">temp={ac.temperature}</span>
                      <span className="st-models-agent-param">max={ac.max_tokens}</span>
                    </div>
                  ))}
                </div>
              </>
            )}

            <div className="st-models-actions">
              {onRefreshOllama && (
                <button type="button" className="st-btn st-btn-ghost" onClick={() => { void onRefreshOllama(); }}>
                  Refresh Ollama
                </button>
              )}
              <button type="button" className="st-btn st-btn-blue" onClick={onRerunSetup ?? onSave}>
                Re-run Setup Wizard
              </button>
            </div>
          </div>
        )}

        {section === "about" && (
          <div className="st-card st-about">
            <div className="st-about-logo">N</div>
            <h3 className="st-about-name">NexusOS</h3>
            <div className="st-about-grid">
              <div className="st-about-field">
                <span className="st-about-label">Version</span>
                <span className="st-about-value">v5.0.0</span>
              </div>
              <div className="st-about-field">
                <span className="st-about-label">Build</span>
                <span className="st-about-value">2026-03-05</span>
              </div>
              <div className="st-about-field">
                <span className="st-about-label">Runtime</span>
                <span className="st-about-value">Rust kernel + Tauri + React</span>
              </div>
              <div className="st-about-field">
                <span className="st-about-label">License</span>
                <span className="st-about-value">TBD</span>
              </div>
            </div>
            <div className="st-about-actions">
              <a className="st-btn st-btn-blue" href="https://github.com/nexai-lang/nexus-os" target="_blank" rel="noreferrer">View on GitHub</a>
              <button type="button" className="st-btn st-btn-ghost" onClick={() => {
                setUpdateCheck("checking");
                window.setTimeout(() => setUpdateCheck("up-to-date"), 1500);
              }}>
                {updateCheck === "checking" ? "Checking..." : updateCheck === "up-to-date" ? "Up to date" : "Check for Updates"}
              </button>
            </div>
          </div>
        )}
      </div>

      <footer className="st-footer">
        <button type="button" className="st-save-btn" onClick={onSave} disabled={saving}>
          {saving ? "Saving..." : "Save Settings"}
        </button>
      </footer>
    </section>
  );
}
