/**
 * ModelConfigPanel — User-selectable model configuration per build step.
 *
 * Detects all available models (Ollama, CLI providers, API keys) and lets
 * the user choose which model handles each pipeline step. Persists to
 * ~/.nexus/builder_model_config.json via Tauri commands.
 */
import { useEffect, useState, useCallback, useRef } from "react";
import {
  builderGetModelConfig,
  builderSaveModelConfig,
  builderResetModelConfig,
  builderGetModelChoices,
  builderGetAvailableModels,
} from "../../api/backend";
import ConnectProviderCard from "./ConnectProviderCard";

/* ─── Inline design tokens (no CSS vars) ─────────────────────────────── */
const C = {
  bg: "#0a0e14",
  surface: "#111820",
  surfaceAlt: "#0d1219",
  border: "#1a2332",
  borderFocus: "#2d6a5a",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#00d4aa",
  accentDim: "rgba(0,212,170,0.10)",
  err: "#f85149",
  warn: "#f0c040",
  ok: "#3fb950",
  mono: "'JetBrains Mono','Fira Code','Cascadia Code',monospace",
  sans: "system-ui,-apple-system,sans-serif",
};

/* ─── Helpers (hoisted above component to avoid Vite HMR reference errors) ── */

/** Parse a speed estimate string like "~7s", "~2 min", "~3-5 min" into seconds (midpoint). */
function parseSpeedEstimate(s: string): number {
  if (!s) return 0;
  const minMatch = s.match(/(\d+)(?:\s*-\s*(\d+))?\s*min/i);
  if (minMatch) {
    const lo = parseInt(minMatch[1], 10);
    const hi = minMatch[2] ? parseInt(minMatch[2], 10) : lo;
    return ((lo + hi) / 2) * 60;
  }
  const secMatch = s.match(/(\d+)\s*s/i);
  if (secMatch) return parseInt(secMatch[1], 10);
  return 0;
}

function fmtTotalTime(seconds: number): string {
  if (seconds <= 0) return "N/A";
  const m = Math.floor(seconds / 60);
  const s = Math.round(seconds % 60);
  if (m > 0) return s > 0 ? `~${m}m ${s}s` : `~${m}m`;
  return `~${s}s`;
}

/* ─── Types ───────────────────────────────────────────────────────────── */

interface ModelChoice {
  model_id: string;
  provider: string;
  display_name: string;
  cost_per_build: number;
  speed_estimate: string;
  warning?: string;
}

interface BuildModelConfig {
  version: number;
  planning: ModelChoice;
  content_generation: ModelChoice;
  section_edit: ModelChoice;
  full_build: ModelChoice;
  security_policies: ModelChoice;
}

interface ModelChoices {
  planning: ModelChoice[];
  content_generation: ModelChoice[];
  section_edit: ModelChoice[];
  full_build: ModelChoice[];
  security_policies: ModelChoice[];
}

type StepKey = keyof Omit<BuildModelConfig, "version">;

const STEPS: { key: StepKey; label: string; detail: string; icon: string }[] = [
  { key: "planning", label: "Planning & Classification", detail: "Structured JSON, fast", icon: "\u{1F4CB}" },
  { key: "content_generation", label: "Content Generation", detail: "Text for content slots", icon: "\u{270D}\u{FE0F}" },
  { key: "section_edit", label: "Section Edit", detail: "Section-level HTML edits", icon: "\u{2702}\u{FE0F}" },
  { key: "full_build", label: "Full Build", detail: "Complete HTML/React generation", icon: "\u{26A1}" },
  { key: "security_policies", label: "Security Policies", detail: "RLS, Firestore rules", icon: "\u{1F512}" },
];

interface Props {
  onClose: () => void;
  /** Pre-loaded data from parent (cache). Skips internal fetch when all three provided. */
  cachedConfig?: BuildModelConfig | null;
  cachedChoices?: ModelChoices | null;
  cachedAvailable?: AvailableModels | null;
  /** True while parent is still fetching data — show skeletons */
  loading?: boolean;
}

interface AvailableModels {
  ollama_models: any[];
  ollama_running: boolean;
  codex_cli: { authenticated: boolean; version: string } | null;
  claude_cli: { authenticated: boolean; version: string } | null;
  anthropic_api_key_set: boolean;
  openai_api_key_set: boolean;
}

/* ─── Component ──────────��────────────────────────────────────────────── */

export default function ModelConfigPanel({ onClose, cachedConfig, cachedChoices, cachedAvailable, loading: parentLoading }: Props) {
  const [config, setConfig] = useState<BuildModelConfig | null>(cachedConfig ?? null);
  const [choices, setChoices] = useState<ModelChoices | null>(cachedChoices ?? null);
  const [available, setAvailable] = useState<AvailableModels | null>(cachedAvailable ?? null);
  const [loadError, setLoadError] = useState("");
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [toast, setToast] = useState("");
  const dirtyRef = useRef(false);

  // Keep dirtyRef in sync so the effect closures read the latest value
  dirtyRef.current = dirty;

  // Load data — either from parent cache or by fetching internally
  useEffect(() => {
    // If parent is supplying all cached data, sync from props
    if (cachedConfig && cachedChoices && cachedAvailable) {
      if (!dirtyRef.current) setConfig(cachedConfig);
      setChoices(cachedChoices);
      setAvailable(cachedAvailable);
      return;
    }
    // Otherwise fetch ourselves
    let cancelled = false;
    (async () => {
      try {
        const [cfg, ch, av] = await Promise.all([
          builderGetModelConfig(),
          builderGetModelChoices(),
          builderGetAvailableModels(),
        ]);
        if (cancelled) return;
        setConfig(cfg);
        setChoices(ch);
        setAvailable(av);
        setLoadError("");
      } catch (e: any) {
        if (cancelled) return;
        console.error("[ModelConfigPanel] load error:", e);
        setLoadError(String(e?.message ?? e));
      }
    })();
    return () => { cancelled = true; };
  }, [cachedConfig, cachedChoices, cachedAvailable]);

  const handleAuthChanged = useCallback(() => {
    // Re-detect everything after a CLI is authenticated
    (async () => {
      try {
        const [cfg, ch, av] = await Promise.all([
          builderGetModelConfig(),
          builderGetModelChoices(),
          builderGetAvailableModels(),
        ]);
        setConfig(cfg);
        setChoices(ch);
        setAvailable(av);
      } catch (e) {
        console.error("[ModelConfigPanel] reload after auth error:", e);
      }
    })();
  }, []);

  const handleSelect = useCallback(
    (step: StepKey, choice: ModelChoice) => {
      setConfig(prev => prev ? { ...prev, [step]: choice } : prev);
      setDirty(true);
    },
    [],
  );

  const handleSave = useCallback(async () => {
    if (!config) return;
    setSaving(true);
    try {
      await builderSaveModelConfig(config);
      setDirty(false);
      setToast("Saved");
      setTimeout(() => setToast(""), 2000);
    } catch (e: any) {
      console.error("[ModelConfigPanel] save error:", e);
      setToast(`Error: ${e?.message ?? e}`);
    } finally {
      setSaving(false);
    }
  }, [config]);

  const handleReset = useCallback(async () => {
    try {
      const fresh = await builderResetModelConfig();
      setConfig(fresh);
      setDirty(false);
      setToast("Reset to recommended");
      setTimeout(() => setToast(""), 2000);
    } catch (e: any) {
      console.error("[ModelConfigPanel] reset error:", e);
      setToast(`Error: ${e?.message ?? e}`);
    }
  }, []);

  // --- ALL hooks are above this line. No hooks below. ---

  const isLoading = parentLoading || !config || !choices;

  // Loading / skeleton state
  if (isLoading && !loadError) {
    return (
      <div style={panelStyle}>
        {/* Header */}
        <div style={headerStyle}>
          <div style={{ fontFamily: C.mono, fontSize: 13, fontWeight: 700, color: C.text, letterSpacing: 0.5 }}>MODEL CONFIGURATION</div>
          <button type="button" onClick={onClose} style={closeBtnStyle}>X</button>
        </div>
        {/* Skeleton: Connected Providers */}
        <div style={{ padding: "10px 16px 6px" }}>
          <div style={{ fontSize: 9, fontWeight: 700, color: C.dim, textTransform: "uppercase" as const, letterSpacing: 1.2, marginBottom: 6, fontFamily: C.mono }}>Connected Providers</div>
          <div style={{ display: "flex", flexDirection: "column" as const, gap: 4 }}>
            {[0, 1].map(i => (
              <div key={i} style={{ background: C.surfaceAlt, border: `1px solid ${C.border}`, borderRadius: 6, padding: "10px 12px", height: 36, animation: "nbpulse 1.5s ease-in-out infinite" }} />
            ))}
          </div>
        </div>
        {/* Skeleton: Build Steps */}
        <div style={{ padding: "8px 16px 12px" }}>
          {STEPS.map(step => (
            <div key={step.key} style={{ marginBottom: 14 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 4 }}>
                <span style={{ fontSize: 12 }}>{step.icon}</span>
                <span style={{ fontSize: 11, fontWeight: 600, color: C.text, fontFamily: C.mono, textTransform: "uppercase" as const, letterSpacing: 0.6 }}>{step.label}</span>
              </div>
              <div style={{ background: C.surfaceAlt, border: `1px solid ${C.border}`, borderRadius: 4, height: 30, animation: "nbpulse 1.5s ease-in-out infinite" }} />
            </div>
          ))}
        </div>
      </div>
    );
  }

  // Error state
  if (loadError) {
    return (
      <div style={panelStyle}>
        <div style={headerStyle}>
          <div style={{ fontFamily: C.mono, fontSize: 13, fontWeight: 700, color: C.text, letterSpacing: 0.5 }}>MODEL CONFIGURATION</div>
          <button type="button" onClick={onClose} style={closeBtnStyle}>X</button>
        </div>
        <div style={{ padding: 16, color: C.err, fontSize: 12, fontFamily: C.mono }}>
          <p style={{ margin: "0 0 8px" }}>{"\u26A0\uFE0F"} Failed to load model configuration.</p>
          <p style={{ margin: "0 0 12px", fontSize: 11, color: C.muted, wordBreak: "break-word" as const }}>{loadError}</p>
          <button type="button"
            onClick={() => { setLoadError(""); }}
            style={{ ...actionBtnStyle, background: C.accentDim, color: C.accent }}
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  // Safe access — guard against null (should not happen after isLoading check, but belt-and-suspenders)
  const safeConfig = config!;
  const safeChoices = choices!;

  const totalCost = STEPS.reduce((sum, s) => {
    const mc = safeConfig[s.key] as ModelChoice | undefined;
    return sum + (mc?.cost_per_build ?? 0);
  }, 0);
  const anyNone = STEPS.some((s) => {
    const mc = safeConfig[s.key] as ModelChoice | undefined;
    return mc?.provider === "none";
  });
  const totalSeconds = STEPS.reduce((sum, s) => {
    const mc = safeConfig[s.key] as ModelChoice | undefined;
    return sum + parseSpeedEstimate(mc?.speed_estimate ?? "");
  }, 0);

  const claudeAuth = available?.claude_cli?.authenticated ?? false;
  const codexAuth = available?.codex_cli?.authenticated ?? false;
  const hasOllama = (available?.ollama_models?.length ?? 0) > 0;
  const hasAnthropicKey = available?.anthropic_api_key_set ?? false;
  const hasOpenaiKey = available?.openai_api_key_set ?? false;
  const noProviders = !claudeAuth && !codexAuth && !hasOllama && !hasAnthropicKey && !hasOpenaiKey;

  return (
    <div style={panelStyle}>
      {/* Header — flexShrink: 0 so it stays at top */}
      <div style={headerStyle}>
        <div style={{ fontFamily: C.mono, fontSize: 13, fontWeight: 700, color: C.text, letterSpacing: 0.5 }}>
          MODEL CONFIGURATION
        </div>
        <button type="button" onClick={onClose} style={closeBtnStyle}>
          X
        </button>
      </div>

      {/* Scrollable content area */}
      <div style={{ flex: 1, overflowY: "auto", minHeight: 0 }}>
        {/* First-run onboarding: no providers at all */}
        {noProviders && (
          <div style={{ padding: "24px 16px", display: "flex", flexDirection: "column" as const, gap: 12 }}>
            <div style={{ fontSize: 13, fontWeight: 700, color: C.text, textAlign: "center" as const, marginBottom: 4 }}>
              Connect a model to start building
            </div>
            <ConnectProviderCard cli="claude" displayName="Claude CLI" authenticated={false} onAuthChanged={handleAuthChanged} />
            <ConnectProviderCard cli="codex" displayName="Codex CLI" authenticated={false} onAuthChanged={handleAuthChanged} />
            <div style={{ ...apiKeyCardStyle, cursor: "pointer" }} onClick={() => { window.location.hash = "#/settings"; }}>
              <span style={{ fontSize: 16, flexShrink: 0 }}>{"\u{1F511}"}</span>
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: 11, fontWeight: 600, color: C.text, fontFamily: C.mono }}>Anthropic API Key</div>
                <div style={{ fontSize: 10, color: C.muted, fontFamily: C.mono, marginTop: 1 }}>Add key in Settings</div>
              </div>
              <span style={{ fontSize: 10, color: C.dim, fontFamily: C.mono }}>{"\u2192"}</span>
            </div>
            <div style={{ ...apiKeyCardStyle, cursor: "pointer" }} onClick={() => { window.location.hash = "#/settings"; }}>
              <span style={{ fontSize: 16, flexShrink: 0 }}>{"\u{1F511}"}</span>
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: 11, fontWeight: 600, color: C.text, fontFamily: C.mono }}>OpenAI API Key</div>
                <div style={{ fontSize: 10, color: C.muted, fontFamily: C.mono, marginTop: 1 }}>Add key in Settings</div>
              </div>
              <span style={{ fontSize: 10, color: C.dim, fontFamily: C.mono }}>{"\u2192"}</span>
            </div>
            <a
              href="https://ollama.ai"
              target="_blank"
              rel="noopener noreferrer"
              style={{ ...apiKeyCardStyle, textDecoration: "none", cursor: "pointer" }}
            >
              <span style={{ fontSize: 16, flexShrink: 0 }}>{"\u{1F999}"}</span>
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: 11, fontWeight: 600, color: C.text, fontFamily: C.mono }}>Install Ollama</div>
                <div style={{ fontSize: 10, color: C.muted, fontFamily: C.mono, marginTop: 1 }}>Free, local inference</div>
              </div>
              <span style={{ fontSize: 10, color: C.dim, fontFamily: C.mono }}>{"\u2197"}</span>
            </a>
          </div>
        )}

        {/* Connected Providers section (shown when at least one provider exists) */}
        {!noProviders && (
          <>
          <div style={{ padding: "10px 16px 6px" }}>
            <div style={{ fontSize: 9, fontWeight: 700, color: C.dim, textTransform: "uppercase" as const, letterSpacing: 1.2, marginBottom: 6, fontFamily: C.mono }}>
              Connected Providers
            </div>
            <div style={{ display: "flex", flexDirection: "column" as const, gap: 4 }}>
              <ConnectProviderCard cli="claude" displayName="Claude CLI" authenticated={claudeAuth} onAuthChanged={handleAuthChanged} />
              <ConnectProviderCard cli="codex" displayName="Codex CLI" authenticated={codexAuth} onAuthChanged={handleAuthChanged} />
              {!hasAnthropicKey && (
                <div style={{ ...apiKeyCardStyle, cursor: "pointer" }} onClick={() => { window.location.hash = "#/settings"; }}>
                  <span style={{ fontSize: 12, flexShrink: 0 }}>{"\u{1F511}"}</span>
                  <div style={{ flex: 1 }}>
                    <div style={{ fontSize: 10, color: C.muted, fontFamily: C.mono }}>Anthropic API &mdash; Add key in Settings</div>
                  </div>
                  <span style={{ fontSize: 10, color: C.dim }}>{"\u2192"}</span>
                </div>
              )}
              {hasAnthropicKey && (
                <div style={apiKeyCardStyle}>
                  <span style={{ fontSize: 12, flexShrink: 0 }}>{"\u{1F511}"}</span>
                  <div style={{ flex: 1 }}>
                    <div style={{ fontSize: 10, color: C.ok, fontFamily: C.mono }}>{"\u2713"} Anthropic API key configured</div>
                  </div>
                </div>
              )}
              {!hasOpenaiKey && (
                <div style={{ ...apiKeyCardStyle, cursor: "pointer" }} onClick={() => { window.location.hash = "#/settings"; }}>
                  <span style={{ fontSize: 12, flexShrink: 0 }}>{"\u{1F511}"}</span>
                  <div style={{ flex: 1 }}>
                    <div style={{ fontSize: 10, color: C.muted, fontFamily: C.mono }}>OpenAI API &mdash; Add key in Settings</div>
                  </div>
                  <span style={{ fontSize: 10, color: C.dim }}>{"\u2192"}</span>
                </div>
              )}
              {hasOpenaiKey && (
                <div style={apiKeyCardStyle}>
                  <span style={{ fontSize: 12, flexShrink: 0 }}>{"\u{1F511}"}</span>
                  <div style={{ flex: 1 }}>
                    <div style={{ fontSize: 10, color: C.ok, fontFamily: C.mono }}>{"\u2713"} OpenAI API key configured</div>
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* Steps */}
          <div style={{ padding: "8px 16px 12px" }}>
            {STEPS.map((step) => {
              const current = (safeConfig[step.key] ?? {}) as ModelChoice;
              const opts = ((safeChoices[step.key]) || []) as ModelChoice[];
              const isBuild = step.key === "full_build";
              const isSecurity = step.key === "security_policies";

              return (
                <div key={step.key} style={{ marginBottom: 14 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 4 }}>
                    <span style={{ fontSize: 12 }}>{step.icon}</span>
                    <span
                      style={{
                        fontSize: 11,
                        fontWeight: 600,
                        color: C.text,
                        fontFamily: C.mono,
                        textTransform: "uppercase" as const,
                        letterSpacing: 0.6,
                      }}
                    >
                      {step.label}
                    </span>
                    {isBuild && (
                      <span style={{ fontSize: 9, color: C.accent, fontFamily: C.mono }}>most impact on speed</span>
                    )}
                    {isSecurity && (
                      <span style={{ fontSize: 9, color: C.warn, fontFamily: C.mono }}>security-critical</span>
                    )}
                  </div>
                  <select
                    value={`${current.provider ?? "none"}/${current.model_id ?? ""}`}
                    onChange={(e) => {
                      const match = opts.find((o) => `${o.provider}/${o.model_id}` === e.target.value);
                      if (match) handleSelect(step.key, match);
                    }}
                    style={selectStyle}
                  >
                    {opts.length === 0 && (
                      <option value="none/" disabled>
                        No models available
                      </option>
                    )}
                    {opts.map((o) => (
                      <option key={`${o.provider}/${o.model_id}`} value={`${o.provider}/${o.model_id}`}>
                        {o.display_name} {(o.cost_per_build ?? 0) > 0 ? `$${o.cost_per_build.toFixed(2)}` : "FREE"} {o.speed_estimate ?? ""}
                      </option>
                    ))}
                  </select>
                  <div style={{ display: "flex", gap: 12, marginTop: 3, fontSize: 10, fontFamily: C.mono }}>
                    <span style={{ color: C.muted }}>Speed: {current.speed_estimate ?? "N/A"}</span>
                    <span style={{ color: (current.cost_per_build ?? 0) > 0 ? C.warn : C.ok }}>
                      Cost: {(current.cost_per_build ?? 0) > 0 ? `~$${current.cost_per_build.toFixed(2)}` : "FREE"}
                    </span>
                  </div>
                  {current.warning && (
                    <div style={{ fontSize: 10, color: C.warn, fontFamily: C.mono, marginTop: 2 }}>
                      {"\u26A0\uFE0F"} {current.warning}
                    </div>
                  )}
                  {isSecurity && !current.warning && current.provider === "ollama" && (
                    <div style={{ fontSize: 10, color: C.warn, fontFamily: C.mono, marginTop: 2 }}>
                      {"\u26A0\uFE0F"} Local models may produce weaker security policies
                    </div>
                  )}
                </div>
              );
            })}
          </div>
          </>
        )}
      </div>

      {/* Footer — sticky at bottom, never scrolls away */}
      {!noProviders && (
        <div
          style={{
            borderTop: `1px solid ${C.border}`,
            padding: "10px 16px",
            display: "flex",
            flexDirection: "column" as const,
            gap: 8,
            flexShrink: 0,
          }}
        >
          {/* Totals */}
          <div style={{ display: "flex", justifyContent: "space-between", fontSize: 11, fontFamily: C.mono }}>
            <span style={{ color: C.muted }}>
              Est. total: {fmtTotalTime(totalSeconds)} | {totalCost > 0 ? `~$${totalCost.toFixed(2)}` : "$0.00"}
            </span>
            {anyNone && <span style={{ color: C.err }}>Some steps have no model</span>}
          </div>

          {/* Buttons */}
          <div style={{ display: "flex", gap: 8 }}>
            <button type="button"
              onClick={handleSave}
              disabled={!dirty || saving}
              style={{
                ...actionBtnStyle,
                background: dirty ? C.accent : C.dim,
                color: dirty ? C.bg : C.muted,
                cursor: dirty ? "pointer" : "default",
                flex: 1,
              }}
            >
              {saving ? "Saving..." : "Save as Default"}
            </button>
            <button type="button" onClick={handleReset} style={{ ...actionBtnStyle, background: C.surfaceAlt, color: C.muted, flex: 1 }}>
              Reset to Recommended
            </button>
          </div>

          {toast && (
            <div
              style={{
                fontSize: 10,
                fontFamily: C.mono,
                color: toast.startsWith("Error") ? C.err : C.ok,
                textAlign: "center" as const,
              }}
            >
              {toast}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

/* ─── Styles ──────────────────────────────────────────────────────── */

const panelStyle: React.CSSProperties = {
  width: "min(340px, calc(100vw - 32px))",
  maxHeight: "min(70vh, calc(100vh - 120px))",
  background: C.surface,
  border: `1px solid ${C.border}`,
  borderRadius: 8,
  display: "flex",
  flexDirection: "column",
  boxShadow: "0 8px 32px rgba(0,0,0,0.5), 0 2px 8px rgba(0,0,0,0.3)",
  overflow: "hidden",
  boxSizing: "border-box",
};

const headerStyle: React.CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  alignItems: "center",
  padding: "12px 16px",
  borderBottom: `1px solid ${C.border}`,
  flexShrink: 0,
};

const closeBtnStyle: React.CSSProperties = {
  background: "transparent",
  border: "none",
  color: C.muted,
  cursor: "pointer",
  fontSize: 14,
  fontFamily: C.mono,
  padding: "2px 6px",
};

const selectStyle: React.CSSProperties = {
  width: "100%",
  background: C.surfaceAlt,
  color: C.text,
  border: `1px solid ${C.border}`,
  borderRadius: 4,
  padding: "6px 8px",
  fontSize: 11,
  fontFamily: C.mono,
  outline: "none",
  cursor: "pointer",
  appearance: "auto" as any,
};

const actionBtnStyle: React.CSSProperties = {
  border: "none",
  borderRadius: 4,
  padding: "6px 12px",
  fontSize: 11,
  fontFamily: C.mono,
  fontWeight: 600,
  cursor: "pointer",
  letterSpacing: 0.3,
};

const apiKeyCardStyle: React.CSSProperties = {
  background: C.surfaceAlt,
  border: `1px solid ${C.border}`,
  borderRadius: 6,
  padding: "6px 12px",
  display: "flex",
  alignItems: "center",
  gap: 10,
};
