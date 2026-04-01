import { useCallback, useEffect, useRef, useState } from "react";
import { Send, Square, Shield, Zap, Link2, Activity, Wrench, AlertTriangle, CheckCircle, XCircle, ChevronDown } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { nxStatus, nxChat, nxChatCancel, nxConsentRespond, nxDoctor, type NxGovernanceStatus, type NxDiagnosticResult } from "../api/backend";
import "./terminal.css";

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

interface ConsentRequest {
  requestId: string;
  toolName: string;
  tier: string;
  details: string;
}

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function Terminal(): JSX.Element {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [isRunning, setIsRunning] = useState(false);
  const [streamingText, setStreamingText] = useState("");
  const [governance, setGovernance] = useState<NxGovernanceStatus | null>(null);
  const [diagnostic, setDiagnostic] = useState<NxDiagnosticResult | null>(null);
  const [consent, setConsent] = useState<ConsentRequest | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeToolName, setActiveToolName] = useState<string | null>(null);

  const msgId = useRef(0);
  const chatRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  const scrollToBottom = useCallback(() => {
    setTimeout(() => chatRef.current?.scrollTo(0, chatRef.current.scrollHeight), 30);
  }, []);

  const addMessage = useCallback((role: ChatMessage["role"], content: string, extra?: Partial<ChatMessage>) => {
    const id = msgId.current++;
    setMessages(prev => [...prev, { id, role, content, ts: Date.now(), ...extra }]);
  }, []);

  /* ---- Init: load governance status + diagnostics ---- */
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [status, diag] = await Promise.all([nxStatus(), nxDoctor()]);
        if (cancelled) return;
        setGovernance(status);
        setDiagnostic(diag);
      } catch {
        // Bridge not available — show setup guidance
      }
      setLoading(false);
    })();
    return () => { cancelled = true; };
  }, []);

  /* ---- Subscribe to Tauri events ---- */
  useEffect(() => {
    const unlisteners: (() => void)[] = [];

    const setup = async () => {
      unlisteners.push(await listen<{ text: string }>("nx:text-delta", (e) => {
        setStreamingText(prev => prev + e.payload.text);
        scrollToBottom();
      }));

      unlisteners.push(await listen<{ name: string; id: string }>("nx:tool-start", (e) => {
        setActiveToolName(e.payload.name);
        scrollToBottom();
      }));

      unlisteners.push(await listen<{ name: string; success: boolean; duration_ms: number; summary: string }>("nx:tool-complete", (e) => {
        const { name, success, duration_ms, summary } = e.payload;
        setActiveToolName(null);
        setMessages(prev => [...prev, {
          id: msgId.current++,
          role: "tool",
          content: summary || (success ? "Completed" : "Failed"),
          ts: Date.now(),
          toolName: name,
          toolSuccess: success,
          toolDuration: duration_ms,
        }]);
        scrollToBottom();
      }));

      unlisteners.push(await listen<{ name: string; reason: string }>("nx:tool-denied", (e) => {
        setMessages(prev => [...prev, {
          id: msgId.current++,
          role: "tool",
          content: `Denied: ${e.payload.reason}`,
          ts: Date.now(),
          toolName: e.payload.name,
          toolSuccess: false,
        }]);
        scrollToBottom();
      }));

      unlisteners.push(await listen<{ request_id: string; tool_name: string; tier: string; details: string }>("nx:consent-required", (e) => {
        setConsent({
          requestId: e.payload.request_id,
          toolName: e.payload.tool_name,
          tier: e.payload.tier,
          details: e.payload.details,
        });
      }));

      unlisteners.push(await listen<{ fuel_remaining: number; fuel_consumed: number; audit_entries: number; envelope_similarity: number }>("nx:governance-update", (e) => {
        setGovernance(prev => prev ? {
          ...prev,
          fuel_remaining: e.payload.fuel_remaining,
          fuel_consumed: e.payload.fuel_consumed,
          audit_entries: e.payload.audit_entries,
          envelope_similarity: e.payload.envelope_similarity,
        } : prev);
      }));

      unlisteners.push(await listen<{ reason: string; total_turns: number }>("nx:done", (e) => {
        setStreamingText(prev => {
          if (prev.trim()) {
            setMessages(msgs => [...msgs, {
              id: msgId.current++,
              role: "assistant",
              content: prev,
              ts: Date.now(),
            }]);
          }
          return "";
        });
        setIsRunning(false);
        setActiveToolName(null);
        // Refresh governance status
        nxStatus().then(setGovernance).catch(() => {});
        scrollToBottom();
      }));

      unlisteners.push(await listen<{ message: string }>("nx:error", (e) => {
        setStreamingText(prev => {
          if (prev.trim()) {
            setMessages(msgs => [...msgs, {
              id: msgId.current++,
              role: "assistant",
              content: prev,
              ts: Date.now(),
            }]);
          }
          return "";
        });
        addMessage("system", `Error: ${e.payload.message}`);
        setIsRunning(false);
        setActiveToolName(null);
        scrollToBottom();
      }));
    };

    setup();
    return () => { unlisteners.forEach(fn => fn()); };
  }, [addMessage, scrollToBottom]);

  /* ---- Send message ---- */
  const handleSubmit = useCallback(async () => {
    const msg = input.trim();
    if (!msg || isRunning) return;
    setInput("");
    addMessage("user", msg);
    setStreamingText("");
    setIsRunning(true);

    try {
      await nxChat(msg);
    } catch (err) {
      addMessage("system", `Failed to start agent: ${err instanceof Error ? err.message : String(err)}`);
      setIsRunning(false);
    }
    scrollToBottom();
  }, [input, isRunning, addMessage, scrollToBottom]);

  const handleCancel = useCallback(async () => {
    try {
      await nxChatCancel();
    } catch { /* ignore */ }
    setStreamingText(prev => {
      if (prev.trim()) {
        setMessages(msgs => [...msgs, {
          id: msgId.current++,
          role: "assistant",
          content: prev + "\n\n[Cancelled]",
          ts: Date.now(),
        }]);
      }
      return "";
    });
    setIsRunning(false);
    setActiveToolName(null);
  }, []);

  const handleConsent = useCallback(async (granted: boolean) => {
    if (!consent) return;
    try {
      await nxConsentRespond(consent.requestId, granted);
    } catch { /* ignore */ }
    addMessage("system", granted
      ? `Approved: ${consent.toolName}`
      : `Denied: ${consent.toolName}`);
    setConsent(null);
  }, [consent, addMessage]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void handleSubmit();
    }
  }, [handleSubmit]);

  /* ---- Governance metrics ---- */
  const fuelPct = governance
    ? (governance.fuel_total > 0 ? (governance.fuel_remaining / governance.fuel_total) * 100 : 0)
    : 100;
  const fuelColor = fuelPct > 50 ? "var(--nexus-accent, #22c55e)" : fuelPct > 20 ? "#f59e0b" : "#ef4444";

  /* ---- Loading ---- */
  if (loading) {
    return (
      <div style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "100%", color: "#64748b", fontSize: 14 }}>
        Loading Nexus Code bridge...
      </div>
    );
  }

  /* ---- Setup guide (no provider configured) ---- */
  if (diagnostic && !diagnostic.ready) {
    return (
      <section className="tm-root">
        <header className="tm-header">
          <div className="tm-header-left">
            <h2 className="tm-title">NEXUS CODE</h2>
            <span className="tm-subtitle">governed ai agent</span>
          </div>
        </header>
        <div className="tm-setup">
          <div className="tm-setup-card">
            <Shield size={32} style={{ color: "#f59e0b", marginBottom: 12 }} />
            <h3 className="tm-setup-title">Setup Required</h3>
            <p className="tm-setup-desc">Configure at least one LLM provider and ensure git is available.</p>

            <div className="tm-setup-section">
              <h4>Configured Providers</h4>
              {diagnostic.configured_providers.length > 0 ? (
                diagnostic.configured_providers.map(p => (
                  <div key={p} className="tm-setup-provider tm-setup-ok">
                    <CheckCircle size={14} /> {p}
                  </div>
                ))
              ) : (
                <p className="tm-setup-none">None configured</p>
              )}
            </div>

            <div className="tm-setup-section">
              <h4>Unconfigured Providers</h4>
              {diagnostic.unconfigured_providers.map(p => (
                <div key={p.name} className="tm-setup-provider tm-setup-missing">
                  <XCircle size={14} /> {p.name} — set <code>{p.env_var}</code>
                </div>
              ))}
            </div>

            <div className="tm-setup-section">
              <h4>System Tools</h4>
              <div className={`tm-setup-provider ${diagnostic.has_git ? "tm-setup-ok" : "tm-setup-missing"}`}>
                {diagnostic.has_git ? <CheckCircle size={14} /> : <XCircle size={14} />} git {diagnostic.has_git ? "(found)" : "(not found)"}
              </div>
              <div className={`tm-setup-provider ${diagnostic.has_ripgrep ? "tm-setup-ok" : "tm-setup-missing"}`}>
                {diagnostic.has_ripgrep ? <CheckCircle size={14} /> : <XCircle size={14} />} ripgrep {diagnostic.has_ripgrep ? "(found)" : "(optional)"}
              </div>
            </div>

            <p className="tm-setup-hint">
              Set an API key (e.g. <code>ANTHROPIC_API_KEY</code>) in your environment, then restart Nexus OS.
            </p>
          </div>
        </div>
      </section>
    );
  }

  /* ---- Main chat UI ---- */
  return (
    <section className="tm-root">
      {/* ---- Governance Bar ---- */}
      <header className="tm-header">
        <div className="tm-header-left">
          <h2 className="tm-title">NEXUS CODE</h2>
          <span className="tm-subtitle">governed ai agent</span>
        </div>
        <div className="tm-header-center">
          {governance && (
            <>
              <span className="tm-shell-badge" title="Session ID">
                <Shield size={11} style={{ color: "var(--nexus-accent, #22c55e)" }} />
                <span className="tm-shell-name">{governance.session_id}</span>
              </span>
              <span className="tm-shell-badge" title={`${governance.provider}/${governance.model}`}>
                <Activity size={11} style={{ color: "#a78bfa" }} />
                <span className="tm-shell-name">{governance.model.split("/").pop()?.slice(0, 20)}</span>
              </span>
              <span className="tm-shell-badge" title={`Audit: ${governance.audit_entries} entries${governance.audit_chain_valid ? " (chain valid)" : " (chain INVALID)"}`}>
                <Link2 size={11} style={{ color: governance.audit_chain_valid ? "var(--nexus-accent, #22c55e)" : "#ef4444" }} />
                <span className="tm-shell-name">{governance.audit_entries} audit</span>
              </span>
            </>
          )}
        </div>
        <div className="tm-header-right">
          {governance && (
            <div className="tm-fuel-badge">
              <Zap size={11} style={{ color: fuelColor }} />
              <span className="tm-fuel-label">FUEL</span>
              <div className="tm-fuel-bar">
                <div className="tm-fuel-fill" style={{ width: `${fuelPct}%`, background: fuelColor }} />
              </div>
              <span className="tm-fuel-value">{governance.fuel_remaining.toLocaleString()}</span>
            </div>
          )}
          <span className="tm-shell-badge">
            <Wrench size={11} style={{ color: "#64748b" }} />
            <span className="tm-shell-name">{governance?.tool_count ?? 0} tools</span>
          </span>
        </div>
      </header>

      {/* ---- Chat Area ---- */}
      <div className="tm-body">
        <div className="tm-main">
          <div className="tm-output" ref={chatRef}>
            {/* Welcome message */}
            {messages.length === 0 && !streamingText && (
              <div className="tm-welcome">
                <Shield size={28} style={{ color: "var(--nexus-accent, #22c55e)", marginBottom: 8 }} />
                <h3>Nexus Code — Governed AI Agent</h3>
                <p>Every action is governed: capability-checked, fuel-metered, consent-gated, and hash-chain audited.</p>
                <div className="tm-welcome-examples">
                  <button type="button" className="tm-welcome-example" onClick={() => setInput("What files are in this project?")}>
                    What files are in this project?
                  </button>
                  <button type="button" className="tm-welcome-example" onClick={() => setInput("Run the test suite and report results")}>
                    Run the test suite and report results
                  </button>
                  <button type="button" className="tm-welcome-example" onClick={() => setInput("Find and fix any clippy warnings")}>
                    Find and fix any clippy warnings
                  </button>
                </div>
              </div>
            )}

            {/* Messages */}
            {messages.map(msg => (
              <div key={msg.id} className={`tm-msg tm-msg-${msg.role}`}>
                {msg.role === "user" && (
                  <div className="tm-msg-bubble tm-msg-user-bubble">
                    <span className="tm-msg-label">You</span>
                    <div className="tm-msg-text">{msg.content}</div>
                  </div>
                )}
                {msg.role === "assistant" && (
                  <div className="tm-msg-bubble tm-msg-assistant-bubble">
                    <span className="tm-msg-label">Nexus Code</span>
                    <pre className="tm-msg-text">{msg.content}</pre>
                  </div>
                )}
                {msg.role === "tool" && (
                  <div className={`tm-tool-msg ${msg.toolSuccess ? "tm-tool-ok" : "tm-tool-fail"}`}>
                    {msg.toolSuccess ? <CheckCircle size={12} /> : <XCircle size={12} />}
                    <span className="tm-tool-name">{msg.toolName}</span>
                    <span className="tm-tool-summary">{msg.content}</span>
                    {msg.toolDuration !== undefined && (
                      <span className="tm-tool-duration">{msg.toolDuration}ms</span>
                    )}
                  </div>
                )}
                {msg.role === "system" && (
                  <div className="tm-system-msg">
                    <AlertTriangle size={12} />
                    <span>{msg.content}</span>
                  </div>
                )}
              </div>
            ))}

            {/* Streaming text */}
            {streamingText && (
              <div className="tm-msg tm-msg-assistant">
                <div className="tm-msg-bubble tm-msg-assistant-bubble">
                  <span className="tm-msg-label">Nexus Code</span>
                  <pre className="tm-msg-text">{streamingText}<span className="tm-cursor">|</span></pre>
                </div>
              </div>
            )}

            {/* Active tool indicator */}
            {activeToolName && (
              <div className="tm-tool-active">
                <span className="tm-tool-spinner" />
                Running <strong>{activeToolName}</strong>...
              </div>
            )}
          </div>

          {/* ---- Consent Modal ---- */}
          {consent && (
            <div className="tm-consent-overlay">
              <div className="tm-consent-modal">
                <div className="tm-consent-header">
                  <AlertTriangle size={18} style={{ color: consent.tier === "Tier3" ? "#ef4444" : "#f59e0b" }} />
                  <span>CONSENT REQUIRED — {consent.tier}</span>
                </div>
                <div className="tm-consent-body">
                  <div className="tm-consent-row">
                    <span className="tm-consent-label">Tool:</span>
                    <code>{consent.toolName}</code>
                  </div>
                  <div className="tm-consent-row">
                    <span className="tm-consent-label">Action:</span>
                    <span>{consent.details}</span>
                  </div>
                  <div className="tm-consent-row">
                    <span className="tm-consent-label">Tier:</span>
                    <span className={`tm-consent-tier tm-tier-${consent.tier.toLowerCase()}`}>{consent.tier}</span>
                  </div>
                </div>
                <div className="tm-consent-actions">
                  <button type="button" className="tm-consent-btn tm-consent-approve" onClick={() => void handleConsent(true)}>
                    <CheckCircle size={14} /> Approve
                  </button>
                  <button type="button" className="tm-consent-btn tm-consent-deny" onClick={() => void handleConsent(false)}>
                    <XCircle size={14} /> Deny
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* ---- Input Row ---- */}
          <div className="tm-input-row">
            <textarea
              ref={inputRef}
              className="tm-input"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={isRunning ? "Agent is working..." : "Ask Nexus Code anything..."}
              spellCheck={false}
              autoFocus
              disabled={isRunning}
              rows={1}
            />
            {isRunning ? (
              <button type="button" className="tm-send-btn tm-cancel-btn" onClick={() => void handleCancel()} title="Cancel">
                <Square size={16} />
              </button>
            ) : (
              <button type="button" className="tm-send-btn" onClick={() => void handleSubmit()} disabled={!input.trim()} title="Send">
                <Send size={16} />
              </button>
            )}
          </div>
        </div>
      </div>
    </section>
  );
}
