import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  nxStatus,
  nxChat,
  nxChatCancel,
  nxConsentRespond,
  nxDoctor,
  nxSwitchProvider,
  type NxGovernanceStatus,
  type NxDiagnosticResult,
} from "../api/backend";

/* ================================================================== */
/*  Types                                                              */
/* ================================================================== */

interface ChatMessage {
  id: number;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  ts: number;
  toolName?: string;
  toolSuccess?: boolean;
  toolDuration?: number;
}

interface ConsentModal {
  requestId: string;
  toolName: string;
  tier: string;
  details: string;
}

/* ================================================================== */
/*  Styles — inline per project convention                             */
/* ================================================================== */

const S = {
  root: {
    display: "flex",
    flexDirection: "column" as const,
    height: "100%",
    minHeight: 0,
    overflow: "hidden",
    background: "#080d1a",
    fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
    color: "#e0f2fe",
  },
  govBar: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    padding: "0.4rem 0.8rem",
    borderBottom: "1px solid rgba(56, 189, 248, 0.12)",
    flexShrink: 0,
    gap: "0.5rem",
    fontSize: "0.7rem",
    background: "rgba(15, 23, 42, 0.5)",
  },
  govLeft: { display: "flex", alignItems: "center", gap: "0.6rem" },
  govRight: { display: "flex", alignItems: "center", gap: "0.6rem" },
  badge: {
    display: "flex",
    alignItems: "center",
    gap: "0.3rem",
    padding: "0.15rem 0.4rem",
    background: "rgba(15, 23, 42, 0.7)",
    border: "1px solid rgba(56, 189, 248, 0.12)",
    borderRadius: "0.3rem",
    color: "#a5f3fc",
  },
  fuelBarOuter: {
    width: "48px",
    height: "4px",
    borderRadius: "2px",
    background: "rgba(56, 189, 248, 0.15)",
    overflow: "hidden",
  },
  chatArea: {
    flex: 1,
    overflowY: "auto" as const,
    padding: "0.8rem",
    display: "flex",
    flexDirection: "column" as const,
    gap: "0.4rem",
    minHeight: 0,
  },
  msgUser: {
    alignSelf: "flex-end" as const,
    maxWidth: "80%",
    padding: "0.5rem 0.7rem",
    background: "rgba(56, 189, 248, 0.15)",
    border: "1px solid rgba(56, 189, 248, 0.25)",
    borderRadius: "0.5rem 0.5rem 0 0.5rem",
    fontSize: "0.8rem",
    lineHeight: "1.5",
    whiteSpace: "pre-wrap" as const,
  },
  msgAssistant: {
    alignSelf: "flex-start" as const,
    maxWidth: "85%",
    padding: "0.5rem 0.7rem",
    background: "rgba(15, 23, 42, 0.7)",
    border: "1px solid rgba(56, 189, 248, 0.08)",
    borderRadius: "0.5rem 0.5rem 0.5rem 0",
    fontSize: "0.8rem",
    lineHeight: "1.5",
    whiteSpace: "pre-wrap" as const,
    color: "#cbd5e1",
  },
  msgTool: {
    alignSelf: "flex-start" as const,
    maxWidth: "85%",
    padding: "0.35rem 0.6rem",
    background: "rgba(15, 23, 42, 0.5)",
    border: "1px solid rgba(148, 163, 184, 0.12)",
    borderRadius: "0.3rem",
    fontSize: "0.7rem",
    fontFamily: "'JetBrains Mono', monospace",
    color: "#94a3b8",
  },
  msgSystem: {
    alignSelf: "center" as const,
    padding: "0.3rem 0.6rem",
    fontSize: "0.7rem",
    color: "rgba(165, 243, 252, 0.5)",
    textAlign: "center" as const,
  },
  inputBar: {
    display: "flex",
    alignItems: "center",
    gap: "0.5rem",
    padding: "0.5rem 0.8rem",
    borderTop: "1px solid rgba(56, 189, 248, 0.12)",
    background: "rgba(15, 23, 42, 0.5)",
    flexShrink: 0,
  },
  input: {
    flex: 1,
    background: "rgba(15, 23, 42, 0.7)",
    border: "1px solid rgba(56, 189, 248, 0.2)",
    borderRadius: "0.4rem",
    padding: "0.5rem 0.7rem",
    color: "#e0f2fe",
    fontSize: "0.8rem",
    fontFamily: "'JetBrains Mono', monospace",
    outline: "none",
  },
  btn: {
    padding: "0.4rem 0.8rem",
    background: "rgba(56, 189, 248, 0.15)",
    border: "1px solid rgba(56, 189, 248, 0.3)",
    borderRadius: "0.4rem",
    color: "#38bdf8",
    fontSize: "0.75rem",
    cursor: "pointer",
    fontFamily: "'JetBrains Mono', monospace",
  },
  btnDanger: {
    padding: "0.4rem 0.8rem",
    background: "rgba(239, 68, 68, 0.15)",
    border: "1px solid rgba(239, 68, 68, 0.3)",
    borderRadius: "0.4rem",
    color: "#ef4444",
    fontSize: "0.75rem",
    cursor: "pointer",
    fontFamily: "'JetBrains Mono', monospace",
  },
  consentOverlay: {
    position: "fixed" as const,
    inset: 0,
    background: "rgba(0,0,0,0.65)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    zIndex: 9999,
  },
  consentModal: {
    background: "#0f172a",
    border: "1px solid rgba(56, 189, 248, 0.25)",
    borderRadius: "0.6rem",
    padding: "1.2rem",
    maxWidth: "460px",
    width: "90%",
  },
  setupRoot: {
    display: "flex",
    flexDirection: "column" as const,
    alignItems: "center",
    justifyContent: "center",
    height: "100%",
    background: "#080d1a",
    color: "#e0f2fe",
    fontFamily: "'JetBrains Mono', monospace",
    padding: "2rem",
    gap: "1rem",
  },
  setupCard: {
    background: "rgba(15, 23, 42, 0.7)",
    border: "1px solid rgba(56, 189, 248, 0.15)",
    borderRadius: "0.6rem",
    padding: "1.5rem",
    maxWidth: "520px",
    width: "100%",
  },
};

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function NexusCode(): JSX.Element {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [isRunning, setIsRunning] = useState(false);
  const [governance, setGovernance] = useState<NxGovernanceStatus | null>(null);
  const [diagnostic, setDiagnostic] = useState<NxDiagnosticResult | null>(null);
  const [consent, setConsent] = useState<ConsentModal | null>(null);
  const [loading, setLoading] = useState(true);
  const [streamBuffer, setStreamBuffer] = useState("");
  const [showProviderPicker, setShowProviderPicker] = useState(false);
  const [switchingProvider, setSwitchingProvider] = useState(false);

  const chatRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const msgId = useRef(0);

  // ── Initialize ──
  useEffect(() => {
    (async () => {
      try {
        const [diag, status] = await Promise.all([nxDoctor(), nxStatus()]);
        setDiagnostic(diag);
        setGovernance(status);
      } catch {
        // Bridge not ready — show setup
      }
      setLoading(false);
    })();
  }, []);

  // ── Subscribe to Tauri events ──
  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];
    let currentBuffer = "";

    const sub = async () => {
      unlisteners.push(
        await listen<{ text: string }>("nx:text-delta", (e) => {
          currentBuffer += e.payload.text;
          setStreamBuffer(currentBuffer);
        })
      );

      unlisteners.push(
        await listen<{ name: string; id: string }>("nx:tool-start", (e) => {
          const id = msgId.current++;
          setMessages((prev) => [
            ...prev,
            {
              id,
              role: "tool",
              content: `Running ${e.payload.name}...`,
              ts: Date.now(),
              toolName: e.payload.name,
            },
          ]);
        })
      );

      unlisteners.push(
        await listen<{ name: string; success: boolean; duration_ms: number; summary: string }>(
          "nx:tool-complete",
          (e) => {
            const { name, success, duration_ms, summary } = e.payload;
            const icon = success ? "\u2713" : "\u2717";
            const id = msgId.current++;
            setMessages((prev) => [
              ...prev,
              {
                id,
                role: "tool",
                content: `${icon} ${name} (${duration_ms}ms) — ${summary.slice(0, 120)}`,
                ts: Date.now(),
                toolName: name,
                toolSuccess: success,
                toolDuration: duration_ms,
              },
            ]);
          }
        )
      );

      unlisteners.push(
        await listen<{ name: string; reason: string }>("nx:tool-denied", (e) => {
          const id = msgId.current++;
          setMessages((prev) => [
            ...prev,
            {
              id,
              role: "tool",
              content: `\u2717 DENIED: ${e.payload.name} — ${e.payload.reason}`,
              ts: Date.now(),
              toolName: e.payload.name,
              toolSuccess: false,
            },
          ]);
        })
      );

      unlisteners.push(
        await listen<{ request_id: string; tool_name: string; tier: string; details: string }>(
          "nx:consent-required",
          (e) => {
            setConsent({
              requestId: e.payload.request_id,
              toolName: e.payload.tool_name,
              tier: e.payload.tier,
              details: e.payload.details,
            });
          }
        )
      );

      unlisteners.push(
        await listen<{ fuel_remaining: number; fuel_consumed: number; audit_entries: number }>(
          "nx:governance-update",
          (e) => {
            setGovernance((prev) =>
              prev
                ? {
                    ...prev,
                    fuel_remaining: e.payload.fuel_remaining,
                    fuel_consumed: e.payload.fuel_consumed,
                    audit_entries: e.payload.audit_entries,
                    fuel_percentage:
                      prev.fuel_total > 0
                        ? (e.payload.fuel_remaining / prev.fuel_total) * 100
                        : 0,
                  }
                : prev
            );
          }
        )
      );

      unlisteners.push(
        await listen<{ reason: string; total_turns: number }>("nx:done", () => {
          // Flush stream buffer as assistant message
          if (currentBuffer.trim()) {
            const id = msgId.current++;
            const text = currentBuffer;
            setMessages((prev) => [
              ...prev,
              { id, role: "assistant", content: text, ts: Date.now() },
            ]);
          }
          currentBuffer = "";
          setStreamBuffer("");
          setIsRunning(false);
          // Refresh governance status
          nxStatus()
            .then((s) => setGovernance(s))
            .catch(() => {});
        })
      );

      unlisteners.push(
        await listen<{ message: string }>("nx:error", (e) => {
          const msg = e.payload.message;
          const isProviderError =
            /provider error|no provider configured|api key|unauthorized|authentication/i.test(msg);
          const id = msgId.current++;
          setMessages((prev) => [
            ...prev,
            {
              id,
              role: "system",
              content: isProviderError
                ? `Provider error: ${msg}\nUse the provider switcher below to select an available provider.`
                : `Error: ${msg}`,
              ts: Date.now(),
            },
          ]);
          if (isProviderError) {
            setShowProviderPicker(true);
          }
          setIsRunning(false);
        })
      );
    };

    sub().catch(console.error);

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, []);

  // ── Auto-scroll ──
  useEffect(() => {
    const el = chatRef.current;
    if (el) {
      requestAnimationFrame(() => el.scrollTo(0, el.scrollHeight));
    }
  }, [messages, streamBuffer]);

  // ── Send message ──
  const handleSend = useCallback(async () => {
    const msg = input.trim();
    if (!msg || isRunning) return;

    const id = msgId.current++;
    setMessages((prev) => [...prev, { id, role: "user", content: msg, ts: Date.now() }]);
    setInput("");
    setIsRunning(true);
    setStreamBuffer("");

    try {
      await nxChat(msg);
    } catch (err: unknown) {
      const id2 = msgId.current++;
      setMessages((prev) => [
        ...prev,
        { id: id2, role: "system", content: `Failed: ${err}`, ts: Date.now() },
      ]);
      setIsRunning(false);
    }
  }, [input, isRunning]);

  // ── Consent handlers ──
  const handleConsent = useCallback(
    async (granted: boolean) => {
      if (!consent) return;
      try {
        await nxConsentRespond(consent.requestId, granted);
      } catch {
        // Consent channel may have expired
      }
      setConsent(null);
    },
    [consent]
  );

  // ── Switch provider ──
  const handleSwitchProvider = useCallback(
    async (provider: string) => {
      setSwitchingProvider(true);
      try {
        const status = await nxSwitchProvider(provider);
        setGovernance(status);
        setShowProviderPicker(false);
        const id = msgId.current++;
        setMessages((prev) => [
          ...prev,
          {
            id,
            role: "system",
            content: `Switched to ${status.provider}/${status.model}`,
            ts: Date.now(),
          },
        ]);
      } catch (err: unknown) {
        const id = msgId.current++;
        setMessages((prev) => [
          ...prev,
          { id, role: "system", content: `Switch failed: ${err}`, ts: Date.now() },
        ]);
      }
      setSwitchingProvider(false);
    },
    []
  );

  // ── Loading ──
  if (loading) {
    return (
      <div style={S.setupRoot}>
        <div style={{ color: "#38bdf8", fontSize: "0.85rem" }}>Initializing Nexus Code...</div>
      </div>
    );
  }

  // ── Setup guide (no provider) ──
  if (diagnostic && !diagnostic.ready) {
    return (
      <div style={S.setupRoot}>
        <div style={{ fontSize: "1.1rem", fontWeight: 700, color: "#38bdf8" }}>
          Nexus Code Setup
        </div>
        <div style={{ fontSize: "0.75rem", color: "#94a3b8", maxWidth: "400px", textAlign: "center" }}>
          Configure at least one LLM provider to start using the governed coding agent.
        </div>
        <div style={S.setupCard}>
          <div style={{ fontSize: "0.8rem", fontWeight: 600, marginBottom: "0.6rem", color: "#a5f3fc" }}>
            Configured Providers
          </div>
          {diagnostic.configured_providers.length > 0 ? (
            diagnostic.configured_providers.map((p) => (
              <div key={p} style={{ fontSize: "0.75rem", color: "#22c55e", padding: "0.2rem 0" }}>
                {"\u2713"} {p}
              </div>
            ))
          ) : (
            <div style={{ fontSize: "0.75rem", color: "#94a3b8", fontStyle: "italic" }}>
              None configured
            </div>
          )}

          <div
            style={{
              fontSize: "0.8rem",
              fontWeight: 600,
              marginTop: "0.8rem",
              marginBottom: "0.6rem",
              color: "#a5f3fc",
            }}
          >
            Available Providers
          </div>
          {diagnostic.unconfigured_providers.map((p) => (
            <div key={p.name} style={{ fontSize: "0.75rem", color: "#94a3b8", padding: "0.2rem 0" }}>
              {"\u2717"} {p.name}{" "}
              <span style={{ color: "#64748b" }}>
                — set <code style={{ color: "#fbbf24" }}>{p.env_var}</code>
              </span>
            </div>
          ))}

          <div
            style={{
              marginTop: "1rem",
              padding: "0.6rem",
              background: "rgba(56, 189, 248, 0.08)",
              borderRadius: "0.3rem",
              fontSize: "0.7rem",
              color: "#94a3b8",
            }}
          >
            <div style={{ color: "#a5f3fc", fontWeight: 600, marginBottom: "0.3rem" }}>Quick Start</div>
            <div>export ANTHROPIC_API_KEY=sk-ant-...</div>
            <div style={{ marginTop: "0.2rem" }}>
              Or install Ollama for free local models: ollama pull qwen3:8b
            </div>
          </div>

          {!diagnostic.has_git && (
            <div
              style={{
                marginTop: "0.6rem",
                fontSize: "0.7rem",
                color: "#ef4444",
              }}
            >
              {"\u2717"} git is required but not installed
            </div>
          )}
        </div>
      </div>
    );
  }

  // ── Fuel bar color ──
  const fuelPct = governance?.fuel_percentage ?? 100;
  const fuelColor = fuelPct > 50 ? "#22c55e" : fuelPct > 20 ? "#eab308" : "#ef4444";

  return (
    <div style={S.root}>
      {/* ── Governance Bar ── */}
      <div style={S.govBar}>
        <div style={S.govLeft}>
          <span style={{ color: "#38bdf8", fontWeight: 700, fontSize: "0.85rem" }}>
            nx
          </span>
          <div style={S.badge}>
            <span style={{ color: "#64748b", fontSize: "0.6rem" }}>SESSION</span>
            <span>{governance?.session_id ?? "..."}</span>
          </div>
          <div style={S.badge}>
            <span style={{ color: "#64748b", fontSize: "0.6rem" }}>MODEL</span>
            <span>
              {governance?.provider ?? "?"}/{governance?.model ?? "?"}
            </span>
          </div>
        </div>
        <div style={S.govRight}>
          <div style={S.badge}>
            <span style={{ color: "#64748b", fontSize: "0.6rem" }}>FUEL</span>
            <div style={S.fuelBarOuter}>
              <div
                style={{
                  width: `${fuelPct}%`,
                  height: "100%",
                  borderRadius: "2px",
                  background: fuelColor,
                  transition: "width 0.3s, background 0.3s",
                }}
              />
            </div>
            <span style={{ color: fuelColor }}>{Math.round(fuelPct)}%</span>
          </div>
          <div style={S.badge}>
            <span style={{ color: "#64748b", fontSize: "0.6rem" }}>AUDIT</span>
            <span>{governance?.audit_entries ?? 0}</span>
            <span style={{ color: governance?.audit_chain_valid ? "#22c55e" : "#ef4444" }}>
              {governance?.audit_chain_valid ? "\u2713" : "\u2717"}
            </span>
          </div>
          <div style={S.badge}>
            <span style={{ color: "#64748b", fontSize: "0.6rem" }}>TOOLS</span>
            <span>{governance?.tool_count ?? 0}</span>
          </div>
        </div>
      </div>

      {/* ── Chat Area ── */}
      <div ref={chatRef} style={S.chatArea}>
        {messages.length === 0 && !streamBuffer && (
          <div style={{ ...S.msgSystem, marginTop: "2rem" }}>
            <div style={{ fontSize: "0.9rem", color: "#38bdf8", fontWeight: 700, marginBottom: "0.4rem" }}>
              Nexus Code — Governed Coding Agent
            </div>
            <div style={{ maxWidth: "420px", margin: "0 auto", color: "#64748b", fontSize: "0.7rem" }}>
              Every action flows through the governance pipeline: capability check, fuel reservation,
              consent classification, execution, audit recording. Type a message to start.
            </div>
          </div>
        )}

        {messages.map((m) => {
          if (m.role === "user") {
            return (
              <div key={m.id} style={S.msgUser}>
                {m.content}
              </div>
            );
          }
          if (m.role === "tool") {
            const borderColor =
              m.toolSuccess === true
                ? "rgba(34, 197, 94, 0.25)"
                : m.toolSuccess === false
                ? "rgba(239, 68, 68, 0.25)"
                : "rgba(148, 163, 184, 0.12)";
            return (
              <div key={m.id} style={{ ...S.msgTool, borderColor }}>
                {m.content}
              </div>
            );
          }
          if (m.role === "system") {
            return (
              <div key={m.id} style={S.msgSystem}>
                {m.content}
              </div>
            );
          }
          // assistant
          return (
            <div key={m.id} style={S.msgAssistant}>
              {m.content}
            </div>
          );
        })}

        {/* Streaming text */}
        {streamBuffer && (
          <div style={{ ...S.msgAssistant, opacity: 0.85 }}>
            {streamBuffer}
            <span
              style={{
                display: "inline-block",
                width: "6px",
                height: "14px",
                background: "#38bdf8",
                marginLeft: "2px",
                animation: "blink 1s infinite",
              }}
            />
          </div>
        )}
      </div>

      {/* ── Input Bar ── */}
      <div style={S.inputBar}>
        <input
          ref={inputRef}
          style={S.input}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              handleSend();
            }
          }}
          placeholder={isRunning ? "Agent is working..." : "Ask the governed agent..."}
          disabled={isRunning}
        />
        {isRunning ? (
          <button style={S.btnDanger} onClick={() => nxChatCancel()}>
            Cancel
          </button>
        ) : (
          <button style={S.btn} onClick={handleSend} disabled={!input.trim()}>
            Send
          </button>
        )}
      </div>

      {/* ── Provider Picker ── */}
      {showProviderPicker && diagnostic && (
        <div
          style={{
            padding: "0.5rem 0.8rem",
            borderTop: "1px solid rgba(56, 189, 248, 0.12)",
            background: "rgba(15, 23, 42, 0.7)",
            flexShrink: 0,
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: "0.5rem",
              flexWrap: "wrap",
            }}
          >
            <span style={{ fontSize: "0.7rem", color: "#94a3b8" }}>
              Switch provider:
            </span>
            {diagnostic.configured_providers.map((p) => (
              <button
                key={p}
                style={{
                  ...S.btn,
                  padding: "0.25rem 0.5rem",
                  fontSize: "0.7rem",
                  opacity: switchingProvider ? 0.5 : 1,
                  background:
                    p === governance?.provider
                      ? "rgba(34, 197, 94, 0.2)"
                      : "rgba(56, 189, 248, 0.15)",
                  borderColor:
                    p === governance?.provider
                      ? "rgba(34, 197, 94, 0.4)"
                      : "rgba(56, 189, 248, 0.3)",
                  color:
                    p === governance?.provider ? "#22c55e" : "#38bdf8",
                }}
                disabled={switchingProvider || p === governance?.provider}
                onClick={() => handleSwitchProvider(p)}
              >
                {p}
              </button>
            ))}
            <button
              style={{
                ...S.btn,
                padding: "0.25rem 0.5rem",
                fontSize: "0.65rem",
                color: "#64748b",
                borderColor: "rgba(100, 116, 139, 0.2)",
                background: "transparent",
              }}
              onClick={() => setShowProviderPicker(false)}
            >
              dismiss
            </button>
          </div>
        </div>
      )}

      {/* ── Consent Modal ── */}
      {consent && (
        <div style={S.consentOverlay} onClick={() => handleConsent(false)}>
          <div style={S.consentModal} onClick={(e) => e.stopPropagation()}>
            <div
              style={{
                fontSize: "0.9rem",
                fontWeight: 700,
                marginBottom: "0.6rem",
                color: consent.tier === "Tier3" ? "#ef4444" : "#eab308",
              }}
            >
              {consent.tier === "Tier3" ? "\u26a0 " : "\u26a0 "}
              Consent Required ({consent.tier})
            </div>
            <div style={{ fontSize: "0.75rem", color: "#94a3b8", marginBottom: "0.3rem" }}>
              Tool: <span style={{ color: "#a5f3fc" }}>{consent.toolName}</span>
            </div>
            <div
              style={{
                fontSize: "0.75rem",
                color: "#cbd5e1",
                padding: "0.5rem",
                background: "rgba(15, 23, 42, 0.5)",
                borderRadius: "0.3rem",
                marginBottom: "0.8rem",
                whiteSpace: "pre-wrap",
              }}
            >
              {consent.details}
            </div>
            <div style={{ display: "flex", gap: "0.5rem", justifyContent: "flex-end" }}>
              <button style={S.btnDanger} onClick={() => handleConsent(false)}>
                Deny
              </button>
              <button style={S.btn} onClick={() => handleConsent(true)}>
                Approve
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Cursor blink animation */}
      <style>{`
        @keyframes blink {
          0%, 50% { opacity: 1; }
          51%, 100% { opacity: 0; }
        }
      `}</style>
    </div>
  );
}
