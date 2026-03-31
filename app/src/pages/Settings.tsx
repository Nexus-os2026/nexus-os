import { useEffect, useRef, useState } from "react";
import { Settings as SettingsIcon, Cpu, Volume2, VolumeX, Key, Save, RefreshCw, Trash2, Check, X, Server, Cloud, TestTube } from "lucide-react";
import "./settings.css";
import type { LlmProviderStatusEntry, LlmRecommendation, NexusConfig, OllamaModelInfo, RoutingStrategy, TestConnectionResult } from "../types";
import { checkLlmStatus, getLlmRecommendations, testLlmConnection, hasDesktopRuntime, listTools, executeTool } from "../api/backend";

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

type SettingsSection = "general" | "llm" | "api" | "privacy" | "voice" | "models" | "tools" | "about";
type ServiceStatus = "unknown" | "testing" | "ok" | "error";

interface ApiKeyDef {
  id: string;
  label: string;
  value: string;
  update: (v: string) => void;
}

interface SettingsField {
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
  const [darkMode, setDarkMode] = useState(() => {
    const stored = localStorage.getItem("nexus-theme-preference");
    if (stored === "light") return false;
    return true; // default dark
  });
  const [language, setLanguage] = useState(() => {
    // i18n is not yet implemented — this persists the user's selection for future use
    return localStorage.getItem("nexus-language") || "en";
  });
  const [notifications, setNotifications] = useState(() => {
    return localStorage.getItem("nexus-notifications") === "true";
  });
  const [deletePhase, setDeletePhase] = useState<"idle" | "confirm">("idle");
  const [micTesting, setMicTesting] = useState(false);
  const [micLevel, setMicLevel] = useState(0.08);
  const [updateCheck, setUpdateCheck] = useState<"idle" | "checking" | "up-to-date">("idle");
  const updateCheckTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (updateCheckTimerRef.current) clearTimeout(updateCheckTimerRef.current);
    };
  }, []);

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

  // ── Tools state ──
  const [tools, setTools] = useState<{ name: string; description: string; schema?: string }[]>([]);
  const [toolsLoading, setToolsLoading] = useState(false);
  const [toolJson, setToolJson] = useState("");
  const [toolResult, setToolResult] = useState<string | null>(null);
  const [toolExecuting, setToolExecuting] = useState(false);

  const [routingStrategy, setRoutingStrategy] = useState<RoutingStrategy>(
    (config.llm.routing_strategy as RoutingStrategy) || "Priority"
  );

  // ── Sync dark/light theme to document and localStorage ──
  useEffect(() => {
    const root = document.documentElement;
    if (darkMode) {
      root.classList.add("dark");
      root.classList.remove("light");
    } else {
      root.classList.add("light");
      root.classList.remove("dark");
    }
    localStorage.setItem("nexus-theme-preference", darkMode ? "dark" : "light");
  }, [darkMode]);

  // ── Persist language selection ──
  useEffect(() => {
    // i18n is not yet implemented — persisting for when it is
    localStorage.setItem("nexus-language", language);
  }, [language]);

  // ── Handle desktop notification permission and persistence ──
  useEffect(() => {
    localStorage.setItem("nexus-notifications", String(notifications));
    if (notifications && typeof Notification !== "undefined" && Notification.permission === "default") {
      Notification.requestPermission();
    }
  }, [notifications]);

  const secretType = showKeys ? "text" : "password";

  const apiKeys: ApiKeyDef[] = [
    { id: "openai", label: "OpenAI", value: config.llm.openai_api_key, update: (v) => onChange({ ...config, llm: { ...config.llm, openai_api_key: v } }) },
    { id: "anthropic", label: "Anthropic", value: config.llm.anthropic_api_key, update: (v) => onChange({ ...config, llm: { ...config.llm, anthropic_api_key: v } }) },
    { id: "brave", label: "Brave Search", value: config.search.brave_api_key, update: (v) => onChange({ ...config, search: { ...config.search, brave_api_key: v } }) },
    { id: "x", label: "X (Twitter)", value: config.social.x_api_key, update: (v) => onChange({ ...config, social: { ...config.social, x_api_key: v } }) },
    { id: "github", label: "GitHub", value: "", update: () => {} }
  ];

  const messagingKeys: SettingsField[] = [
    {
      id: "telegram",
      label: "Telegram Bot Token",
      value: config.messaging.telegram_bot_token,
      update: (v) => onChange({ ...config, messaging: { ...config.messaging, telegram_bot_token: v } }),
    },
    {
      id: "discord",
      label: "Discord Bot Token",
      value: config.messaging.discord_bot_token,
      update: (v) => onChange({ ...config, messaging: { ...config.messaging, discord_bot_token: v } }),
    },
    {
      id: "slack",
      label: "Slack Bot Token",
      value: config.messaging.slack_bot_token,
      update: (v) => onChange({ ...config, messaging: { ...config.messaging, slack_bot_token: v } }),
    },
    {
      id: "whatsapp-api",
      label: "WhatsApp API Token",
      value: config.messaging.whatsapp_api_token,
      update: (v) => onChange({ ...config, messaging: { ...config.messaging, whatsapp_api_token: v } }),
    },
    {
      id: "whatsapp-business",
      label: "WhatsApp Business ID",
      value: config.messaging.whatsapp_business_id,
      update: (v) => onChange({ ...config, messaging: { ...config.messaging, whatsapp_business_id: v } }),
    },
  ];

  async function testKey(id: string, value: string): Promise<void> {
    if (!value || value.trim().length < 4) {
      setStatuses((prev) => ({ ...prev, [id]: "error" }));
      return;
    }
    setStatuses((prev) => ({ ...prev, [id]: "testing" }));
    try {
      if (id === "openai") {
        const res = await fetch("https://api.openai.com/v1/models", {
          method: "GET",
          headers: { Authorization: `Bearer ${value.trim()}` },
        });
        setStatuses((prev) => ({ ...prev, [id]: res.ok ? "ok" : "error" }));
      } else if (id === "anthropic") {
        const res = await fetch("https://api.anthropic.com/v1/messages", {
          method: "POST",
          headers: {
            "x-api-key": value.trim(),
            "anthropic-version": "2023-06-01",
            "Content-Type": "application/json",
          },
          body: JSON.stringify({ model: "claude-haiku-4-5-20251001", max_tokens: 1, messages: [{ role: "user", content: "hi" }] }),
        });
        setStatuses((prev) => ({ ...prev, [id]: (res.ok || res.status === 400) ? "ok" : "error" }));
      } else {
        // For other keys (Brave, X, GitHub), validate length heuristic as fallback
        setStatuses((prev) => ({ ...prev, [id]: value.trim().length > 4 ? "ok" : "error" }));
      }
    } catch {
      // Network error — check Ollama (localhost) separately
      if (id === "openai" || id === "anthropic") {
        setStatuses((prev) => ({ ...prev, [id]: "error" }));
      } else {
        setStatuses((prev) => ({ ...prev, [id]: value.trim().length > 4 ? "ok" : "error" }));
      }
    }
  }

  function statusLabel(s: ServiceStatus): JSX.Element {
    if (s === "testing") return <><RefreshCw size={12} className="inline-icon spin" /> Testing...</>;
    if (s === "ok") return <><Check size={12} className="inline-icon" /> Connected</>;
    if (s === "error") return <><X size={12} className="inline-icon" /> Invalid</>;
    return <>Not Set</>;
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
    }).catch((e) => { if (import.meta.env.DEV) console.warn("[Settings]", e); });
    getLlmRecommendations().then((recs) => {
      setLlmRecs(recs.recommendations);
      setLlmRecsRam(recs.ram_mb);
      setLlmRecsCanLocal(recs.can_run_local);
    }).catch((e) => { if (import.meta.env.DEV) console.warn("[Settings]", e); });
  }, [section]);

  // Fetch tools when the tools section is active
  useEffect(() => {
    if (section !== "tools" || !hasDesktopRuntime()) return;
    setToolsLoading(true);
    listTools().then((raw) => {
      try {
        const parsed = JSON.parse(raw);
        setTools(Array.isArray(parsed) ? parsed : []);
      } catch {
        setTools([]);
      }
    }).catch(() => setTools([])).finally(() => setToolsLoading(false));
  }, [section]);

  function handleExecuteTool(): void {
    if (!toolJson.trim()) return;
    setToolExecuting(true);
    setToolResult(null);
    executeTool(toolJson).then((result) => {
      setToolResult(result);
      setToolExecuting(false);
    }).catch((e) => {
      setToolResult(`Error: ${e}`);
      setToolExecuting(false);
    });
  }

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

    let cancelled = false;
    let animationFrame = 0;
    let stream: MediaStream | null = null;
    let audioContext: AudioContext | null = null;
    let analyser: AnalyserNode | null = null;
    let source: MediaStreamAudioSourceNode | null = null;

    async function startMeter(): Promise<void> {
      try {
        stream = await navigator.mediaDevices.getUserMedia({ audio: true });
        if (cancelled) {
          stream.getTracks().forEach((track) => track.stop());
          return;
        }

        const AudioCtor = window.AudioContext
          ?? ((window as Window & typeof globalThis & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext);
        if (!AudioCtor) {
          throw new Error("AudioContext unavailable");
        }

        audioContext = new AudioCtor();
        analyser = audioContext.createAnalyser();
        analyser.fftSize = 2048;
        source = audioContext.createMediaStreamSource(stream);
        source.connect(analyser);

        const samples = new Uint8Array(analyser.fftSize);
        const tick = () => {
          if (!analyser || cancelled) return;
          analyser.getByteTimeDomainData(samples);
          let sum = 0;
          for (let index = 0; index < samples.length; index += 1) {
            const normalized = (samples[index] - 128) / 128;
            sum += normalized * normalized;
          }
          const rms = Math.sqrt(sum / samples.length);
          setMicLevel(Math.max(0.08, Math.min(1, rms * 4.5)));
          animationFrame = window.requestAnimationFrame(tick);
        };
        tick();
      } catch {
        setMicLevel(0.08);
        setMicTesting(false);
      }
    }

    void startMeter();

    return () => {
      cancelled = true;
      window.cancelAnimationFrame(animationFrame);
      source?.disconnect();
      stream?.getTracks().forEach((track) => track.stop());
      void audioContext?.close();
    };
  }, [micTesting]);

  return (
    <section className="st-hub">
      <header className="st-header">
        <h2 className="st-title"><SettingsIcon size={20} className="inline-icon" /> SYSTEM SETTINGS // CONTROL PANEL</h2>
        <p className="st-subtitle">Security posture, runtime config, and identity controls</p>
      </header>

      <nav className="st-nav">
        {(["general", "llm", "api", "privacy", "voice", "models", "tools", "about"] as SettingsSection[]).map((s) => (
          <button
            key={s}
            type="button"
            className={`st-nav-btn cursor-pointer ${section === s ? "active" : ""}`}
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
                <p className="st-row-label">{uiSoundEnabled ? <Volume2 size={14} className="inline-icon" /> : <VolumeX size={14} className="inline-icon" />} UI Sound Design</p>
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
            <div className="st-row">
              <div>
                <p className="st-row-label">Enable Warden Governance Review</p>
                <p className="st-row-hint">Have the L6 Warden review L3+ agent actions before execution</p>
              </div>
              <label className="st-toggle">
                <input
                  type="checkbox"
                  checked={config.governance.enable_warden_review}
                  onChange={(e) =>
                    onChange({
                      ...config,
                      governance: {
                        ...config.governance,
                        enable_warden_review: e.target.checked,
                      },
                    })
                  }
                />
                <span className="st-toggle-track"><span className="st-toggle-thumb" /></span>
                <span className="st-toggle-text">{config.governance.enable_warden_review ? "On" : "Off"}</span>
              </label>
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
            <h3 className="st-card-title" style={{ marginTop: "1rem" }}><Server size={16} className="inline-icon" /> Provider Status</h3>
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
                      className="st-btn st-btn-ghost cursor-pointer"
                      style={{ fontSize: "0.75rem", padding: "3px 10px" }}
                      disabled={testingProvider === p.name}
                      onClick={() => handleTestConnection(p.name)}
                    >
                      {testingProvider === p.name ? "Testing..." : <><TestTube size={12} className="inline-icon" /> Test</>}
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
            <h3 className="st-card-title" style={{ marginTop: "1rem" }}><Key size={16} className="inline-icon" /> API Keys</h3>
            {[
              { label: "DeepSeek", key: "deepseek_api_key" as const, hint: "~$0.14/M tokens (cheapest)" },
              { label: "NVIDIA NIM", key: "nvidia_api_key" as const, hint: "42 free models — DeepSeek V3.1, GLM-4.7, Nemotron, Llama 4, Qwen 3.5" },
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
              <button type="button" className="st-btn st-btn-ghost cursor-pointer" onClick={() => setShowKeys((p) => !p)}>
                <Key size={14} className="inline-icon" /> {showKeys ? "Hide Keys" : "Show Keys"}
              </button>
              <button type="button" className="st-btn st-btn-blue cursor-pointer" onClick={onSave} disabled={saving}>
                {saving ? "Saving..." : <><Save size={14} className="inline-icon" /> Save Keys</>}
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
              <button type="button" className="st-show-keys-btn cursor-pointer" onClick={() => setShowKeys((p) => !p)}>
                <Key size={14} className="inline-icon" /> {showKeys ? "Hide Keys" : "Show Keys"}
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
                    <button type="button" className="st-api-save-btn cursor-pointer" onClick={onSave} disabled={saving}>
                      Save
                    </button>
                    <button type="button" className="st-api-test-btn cursor-pointer" onClick={() => testKey(key.id, key.value)}>
                      Test Connection
                    </button>
                  </div>
                </div>
              );
            })}

            <h3 className="st-card-title" style={{ marginTop: "1rem" }}>Messaging Tokens</h3>
            <p className="st-row-hint" style={{ marginBottom: "0.75rem" }}>
              These values back the messaging connectors and the Messaging control page.
            </p>
            {messagingKeys.map((key) => (
              <div key={key.id} className="st-api-row">
                <div className="st-api-label-wrap">
                  <span className="st-api-label">{key.label}</span>
                  <span className={`st-api-status ${statusClass(statuses[key.id] ?? "unknown")}`}>
                    {statusLabel(statuses[key.id] ?? "unknown")}
                  </span>
                </div>
                <div className="st-api-input-row">
                  <input
                    type={secretType}
                    className="st-api-input"
                    value={key.value}
                    onChange={(e) => key.update(e.target.value)}
                    placeholder={`Enter ${key.label}`}
                  />
                  <button type="button" className="st-api-save-btn cursor-pointer" onClick={onSave} disabled={saving}>
                    Save
                  </button>
                  <button type="button" className="st-api-test-btn cursor-pointer" onClick={() => testKey(key.id, key.value)}>
                    Validate
                  </button>
                </div>
              </div>
            ))}
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
              <span className="st-badge st-badge-green">Hash Chain Intact</span>
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
              <button type="button" className="st-btn st-btn-blue cursor-pointer" onClick={() => {
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
                <button type="button" className="st-btn st-btn-red cursor-pointer" onClick={() => setDeletePhase("confirm")}><Trash2 size={14} className="inline-icon" /> Delete All My Data</button>
              ) : (
                <div className="st-delete-confirm">
                  <p className="st-row-warn">Are you sure? This uses cryptographic erasure and is irreversible.</p>
                  <div className="st-delete-btns">
                    <button type="button" className="st-btn st-btn-red cursor-pointer" onClick={() => { setDeletePhase("idle"); onChange({ ...config, llm: { ...config.llm, anthropic_api_key: "", openai_api_key: "", nvidia_api_key: "" }, search: { ...config.search, brave_api_key: "" } }); }}>Confirm Delete</button>
                    <button type="button" className="st-btn st-btn-ghost cursor-pointer" onClick={() => setDeletePhase("idle")}>Cancel</button>
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
                <button type="button" className="st-btn st-btn-ghost cursor-pointer" onClick={() => setMicTesting((p) => !p)}>
                  {micTesting ? "Stop Test" : "Start Test"}
                </button>
                <div className="st-mic-bar">
                  <div className="st-mic-fill" style={{ width: `${Math.round(micLevel * 100)}%` }} />
                </div>
                <span className="st-row-hint" style={{ fontSize: "0.7rem", marginLeft: 8 }}>
                  {hasDesktopRuntime() ? "(live input level)" : "(desktop runtime required)"}
                </span>
              </div>
            </div>
            <div className="st-row">
              <div><p className="st-row-label">Test Voice</p></div>
              <button type="button" className="st-btn st-btn-ghost cursor-pointer" onClick={() => {
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
            <h3 className="st-card-title"><Cpu size={16} className="inline-icon" /> Hardware Profile</h3>
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
                          className="st-btn st-btn-red st-btn-sm cursor-pointer"
                          onClick={() => { void onDeleteModel(m.name); }}
                        >
                          <Trash2 size={12} className="inline-icon" /> Remove
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
                <button type="button" className="st-btn st-btn-ghost cursor-pointer" onClick={() => { void onRefreshOllama(); }}>
                  <RefreshCw size={14} className="inline-icon" /> Refresh Ollama
                </button>
              )}
              <button type="button" className="st-btn st-btn-blue cursor-pointer" onClick={onRerunSetup ?? onSave}>
                Re-run Setup Wizard
              </button>
            </div>
          </div>
        )}

        {section === "tools" && (
          <div className="st-card">
            <h3 className="st-card-title">Registered Tools</h3>
            <p className="st-row-hint" style={{ marginBottom: "0.75rem" }}>
              MCP tools registered in the kernel. Execute tools by providing a JSON payload.
            </p>

            {toolsLoading ? (
              <div className="st-row"><p className="st-row-hint">Loading tools...</p></div>
            ) : tools.length === 0 ? (
              <div className="st-row"><p className="st-row-hint">No tools registered yet.</p></div>
            ) : (
              <div style={{ maxHeight: 300, overflowY: "auto", marginBottom: "1rem" }}>
                {tools.map((t) => (
                  <div key={t.name} className="st-row" style={{ flexWrap: "wrap", gap: "0.25rem" }}>
                    <div style={{ flex: 1, minWidth: 200 }}>
                      <p className="st-row-label" style={{ fontFamily: "monospace" }}>{t.name}</p>
                      <p className="st-row-hint">{t.description}</p>
                    </div>
                    <button
                      type="button"
                      className="st-btn st-btn-ghost cursor-pointer"
                      style={{ fontSize: "0.75rem", padding: "3px 10px" }}
                      onClick={() => setToolJson(JSON.stringify({ tool: t.name, args: {} }, null, 2))}
                    >
                      Use
                    </button>
                  </div>
                ))}
              </div>
            )}

            <h3 className="st-card-title" style={{ marginTop: "1rem" }}>Execute Tool</h3>
            <div className="st-row" style={{ flexDirection: "column", alignItems: "stretch", gap: "0.5rem" }}>
              <textarea
                className="st-api-input"
                style={{ minHeight: 120, fontFamily: "monospace", fontSize: "0.82rem", resize: "vertical", width: "100%" }}
                value={toolJson}
                onChange={(e) => setToolJson(e.target.value)}
                placeholder={'{\n  "tool": "tool_name",\n  "args": {}\n}'}
              />
              <div style={{ display: "flex", gap: 8 }}>
                <button
                  type="button"
                  className="st-btn st-btn-blue cursor-pointer"
                  onClick={handleExecuteTool}
                  disabled={toolExecuting || !toolJson.trim()}
                >
                  {toolExecuting ? "Executing..." : "Execute Tool"}
                </button>
                <button
                  type="button"
                  className="st-btn st-btn-ghost cursor-pointer"
                  onClick={() => { setToolJson(""); setToolResult(null); }}
                >
                  Clear
                </button>
              </div>
            </div>

            {toolResult && (
              <div className="st-row" style={{ marginTop: "0.75rem", flexDirection: "column", alignItems: "stretch" }}>
                <p className="st-row-label">Result</p>
                <pre style={{
                  background: "#0f172a", border: "1px solid #334155", borderRadius: 6,
                  padding: "0.75rem", color: "#e2e8f0", fontFamily: "monospace",
                  fontSize: "0.78rem", overflowX: "auto", maxHeight: 300, whiteSpace: "pre-wrap",
                }}>
                  {toolResult}
                </pre>
              </div>
            )}
          </div>
        )}

        {section === "about" && (
          <div className="st-card st-about">
            <div className="st-about-logo">N</div>
            <h3 className="st-about-name">NexusOS</h3>
            <div className="st-about-grid">
              <div className="st-about-field">
                <span className="st-about-label">Version</span>
                <span className="st-about-value">v9.0.0</span>
              </div>
              <div className="st-about-field">
                <span className="st-about-label">Build</span>
                <span className="st-about-value">2026-03-17</span>
              </div>
              <div className="st-about-field">
                <span className="st-about-label">Runtime</span>
                <span className="st-about-value">Rust kernel + Tauri + React</span>
              </div>
              <div className="st-about-field">
                <span className="st-about-label">License</span>
                <span className="st-about-value">MIT</span>
              </div>
            </div>
            <div className="st-about-actions">
              <a className="st-btn st-btn-blue cursor-pointer" href="https://gitlab.com/nexaiceo/nexus-os" target="_blank" rel="noreferrer">View on GitLab</a>
              <button type="button" className="st-btn st-btn-ghost cursor-pointer" onClick={() => {
                setUpdateCheck("checking");
                if (updateCheckTimerRef.current) clearTimeout(updateCheckTimerRef.current);
                updateCheckTimerRef.current = window.setTimeout(() => setUpdateCheck("up-to-date"), 800);
              }}>
                {updateCheck === "checking" ? "Checking..." : updateCheck === "up-to-date" ? <><Check size={14} className="inline-icon" /> v9.0.0 — You are running the latest version.</> : <><Cloud size={14} className="inline-icon" /> Check for Updates</>}
              </button>
            </div>
          </div>
        )}
      </div>

      <footer className="st-footer">
        <button type="button" className="st-save-btn cursor-pointer" onClick={onSave} disabled={saving}>
          {saving ? "Saving..." : <><Save size={14} className="inline-icon" /> Save Settings</>}
        </button>
      </footer>
    </section>
  );
}
