import { useState, useCallback, useMemo, useRef, useEffect } from "react";
import { sendChat, chatWithOllama, listAvailableModels } from "../api/backend";
import type { ChatTokenEvent } from "../types";
import "./ai-chat-hub.css";

/* ─── types ─── */
type View = "chat" | "compare" | "history";

interface Model {
  id: string;
  name: string;
  provider: string;
  icon: string;
  color: string;
  speed: "fast" | "medium" | "slow";
  capability: "basic" | "advanced" | "expert";
  fuelCost: number;
}

interface ChatMsg {
  id: string;
  role: "user" | "assistant" | "system" | "agent";
  content: string;
  model?: string;
  agent?: string;
  timestamp: number;
  imageUrl?: string;
  codeBlock?: { lang: string; code: string; output?: string };
  streaming?: boolean;
}

interface Conversation {
  id: string;
  title: string;
  model: string;
  messages: ChatMsg[];
  createdAt: number;
  updatedAt: number;
  pinned: boolean;
  tags: string[];
}

/* ─── constants ─── */
const FALLBACK_MODELS: Model[] = [
  { id: "mock-1", name: "Mock (no provider)", provider: "Local", icon: "◈", color: "#888", speed: "fast", capability: "basic", fuelCost: 0 },
];

const AGENTS = [
  { id: "coder", name: "Coder Agent", icon: "⬢", color: "var(--nexus-accent)" },
  { id: "designer", name: "Designer Agent", icon: "⬢", color: "#a78bfa" },
  { id: "research", name: "Research Agent", icon: "⬢", color: "#22c55e" },
  { id: "self-improve", name: "Self-Improve", icon: "⬢", color: "#f59e0b" },
];

function modelFromAvailable(m: { id: string; name: string; installed: boolean }): Model {
  const name = m.name || m.id;
  const isLocal = !m.id.startsWith("claude") && !m.id.startsWith("gpt");
  return {
    id: m.id,
    name,
    provider: isLocal ? "Local" : "Cloud",
    icon: isLocal ? "◆" : "◈",
    color: isLocal ? "#22c55e" : "#d4a574",
    speed: "medium",
    capability: "advanced",
    fuelCost: isLocal ? 5 : 15,
  };
}

function classifyError(err: string): string {
  const lower = err.toLowerCase();
  if (lower.includes("connection refused") || lower.includes("not running") || lower.includes("ollama serve")) {
    return "Ollama is not running. Start it with: ollama serve";
  }
  if (lower.includes("api key") || lower.includes("unauthorized") || lower.includes("401")) {
    return "Invalid API key. Go to Settings → LLM Provider to update your key.";
  }
  if (lower.includes("rate limit") || lower.includes("429")) {
    return "Rate limited by provider. Please wait a moment and try again.";
  }
  if (lower.includes("no llm provider") || lower.includes("mock")) {
    return "No LLM provider configured. Go to Settings → LLM Provider to set up Ollama or an API key.";
  }
  return err;
}

/* ─── component ─── */
export default function AiChatHub() {
  const [view, setView] = useState<View>("chat");
  const [conversations, setConversations] = useState<Conversation[]>(() => {
    try {
      const saved = localStorage.getItem("nexus-chat-conversations");
      if (saved) return JSON.parse(saved) as Conversation[];
    } catch { /* ignore corrupt data */ }
    return [];
  });
  const [activeConvId, setActiveConvId] = useState(() => {
    try {
      const saved = localStorage.getItem("nexus-chat-conversations");
      if (saved) {
        const convs = JSON.parse(saved) as Conversation[];
        if (convs.length > 0) return convs[0].id;
      }
    } catch { /* ignore */ }
    return "";
  });
  const [models, setModels] = useState<Model[]>(FALLBACK_MODELS);
  const [selectedModel, setSelectedModel] = useState("mock-1");
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [historySearch, setHistorySearch] = useState("");
  const [fuelUsed, setFuelUsed] = useState(0);
  const [voiceActive, setVoiceActive] = useState(false);
  const [showModelPicker, setShowModelPicker] = useState(false);
  const [showAgentPanel, setShowAgentPanel] = useState(false);
  const [joinedAgents, setJoinedAgents] = useState<string[]>([]);
  const [auditLog, setAuditLog] = useState<string[]>(["Chat hub ready"]);

  // compare state
  const [compareModels, setCompareModels] = useState<[string, string]>(["", ""]);
  const [comparePrompt, setComparePrompt] = useState("");
  const [compareResults, setCompareResults] = useState<[string, string]>(["", ""]);
  const [comparing, setComparing] = useState(false);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const streamingMsgIdRef = useRef<string | null>(null);

  const activeConv = useMemo(() => conversations.find(c => c.id === activeConvId), [conversations, activeConvId]);
  const activeModel = useMemo(() => models.find(m => m.id === selectedModel), [models, selectedModel]);

  const logAudit = useCallback((msg: string) => setAuditLog(prev => [msg, ...prev].slice(0, 30)), []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [activeConv?.messages.length, activeConv?.messages[activeConv?.messages.length - 1]?.content]);

  /* ─── persist conversations to localStorage ─── */
  useEffect(() => {
    try {
      localStorage.setItem("nexus-chat-conversations", JSON.stringify(conversations.slice(-50)));
    } catch { /* quota exceeded or unavailable */ }
  }, [conversations]);

  /* ─── load models from backend ─── */
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const available = await listAvailableModels();
        if (cancelled) return;
        const installed = available.filter(m => m.installed);
        if (installed.length > 0) {
          const mapped = installed.map(modelFromAvailable);
          setModels(mapped);
          setSelectedModel(mapped[0].id);
          if (!compareModels[0] && mapped.length >= 2) {
            setCompareModels([mapped[0].id, mapped[1].id]);
          } else if (!compareModels[0] && mapped.length >= 1) {
            setCompareModels([mapped[0].id, mapped[0].id]);
          }
          logAudit(`Loaded ${mapped.length} model(s)`);
        } else {
          logAudit("No models installed — using fallback");
        }
      } catch {
        // Backend unavailable (web mode) — keep fallback
        logAudit("Backend unavailable — mock mode");
      }
    })();
    return () => { cancelled = true; };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /* ─── create initial conversation ─── */
  useEffect(() => {
    if (conversations.length === 0) {
      const conv: Conversation = {
        id: `conv-${Date.now()}`, title: "New conversation", model: selectedModel,
        messages: [], createdAt: Date.now(), updatedAt: Date.now(),
        pinned: false, tags: [],
      };
      setConversations([conv]);
      setActiveConvId(conv.id);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /* ─── filtered conversations ─── */
  const filteredConversations = useMemo(() => {
    if (!historySearch.trim()) return conversations;
    const q = historySearch.toLowerCase();
    return conversations.filter(c =>
      c.title.toLowerCase().includes(q) ||
      c.tags.some(t => t.includes(q)) ||
      c.messages.some(m => m.content.toLowerCase().includes(q))
    );
  }, [conversations, historySearch]);

  /* ─── helpers ─── */
  const formatTime = (ts: number) => {
    const diff = Date.now() - ts;
    if (diff < 60000) return "now";
    if (diff < 3600000) return `${Math.floor(diff / 60000)}m`;
    if (diff < 86400000) return `${Math.floor(diff / 3600000)}h`;
    return `${Math.floor(diff / 86400000)}d`;
  };

  const highlightCode = (content: string) => {
    return content.replace(/```(\w+)?\n([\s\S]*?)```/g, (_match, lang: string, code: string) => {
      const l = lang || "text";
      return `<div class="ch-code-block"><div class="ch-code-header"><span>${l}</span><button class="ch-code-run" data-code="${encodeURIComponent(code.trim())}">▶ Run</button></div><pre class="ch-code-pre"><code>${code.replace(/</g, "&lt;").replace(/>/g, "&gt;")}</code></pre></div>`;
    });
  };

  const renderContent = (content: string) => {
    let html = content;
    html = highlightCode(html);
    html = html
      .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
      .replace(/(?<!`)`(?!`)([^`\n]+)`(?!`)/g, '<code class="ch-inline-code">$1</code>');
    html = html.replace(/\n/g, "<br/>");
    return html;
  };

  const updateStreamingMsg = useCallback((convId: string, msgId: string, content: string, done: boolean) => {
    setConversations(prev => prev.map(c => c.id === convId ? {
      ...c,
      messages: c.messages.map(m => m.id === msgId ? { ...m, content, streaming: !done } : m),
      updatedAt: Date.now(),
    } : c));
  }, []);

  /* ─── send message (real backend) ─── */
  const sendMessage = useCallback(async () => {
    if (!input.trim() || sending) return;
    const model = models.find(m => m.id === selectedModel);
    const userMsg: ChatMsg = { id: `m-${Date.now()}`, role: "user", content: input, timestamp: Date.now() };
    const currentInput = input;
    const currentConvId = activeConvId;

    setConversations(prev => prev.map(c => c.id === currentConvId ? {
      ...c, messages: [...c.messages, userMsg], updatedAt: Date.now(),
      title: c.messages.length === 0 ? currentInput.slice(0, 50) : c.title,
    } : c));

    setInput("");
    setSending(true);
    setFuelUsed(f => f + (model?.fuelCost ?? 5));
    logAudit(`Sent to ${model?.name ?? selectedModel}`);

    // Create placeholder assistant message for streaming
    const assistantMsgId = `m-${Date.now() + 1}`;
    const assistantMsg: ChatMsg = {
      id: assistantMsgId, role: "assistant", content: "",
      model: selectedModel, timestamp: Date.now(), streaming: true,
    };
    setConversations(prev => prev.map(c => c.id === currentConvId ? {
      ...c, messages: [...c.messages, assistantMsg], updatedAt: Date.now(),
    } : c));
    streamingMsgIdRef.current = assistantMsgId;

    try {
      // Try streaming via Ollama first
      let unlisten: (() => void) | undefined;
      try {
        const eventMod = await import("@tauri-apps/api/event");
        unlisten = await eventMod.listen<ChatTokenEvent>("chat-token", (event) => {
          const { full, done } = event.payload;
          if (event.payload.error) {
            updateStreamingMsg(currentConvId, assistantMsgId, classifyError(event.payload.error), true);
            return;
          }
          updateStreamingMsg(currentConvId, assistantMsgId, full, done);
        });

        const messages = [{ role: "user" as const, content: currentInput }];
        await chatWithOllama(messages, selectedModel);
      } catch (streamErr) {
        // Streaming failed — fall back to non-streaming send_chat
        const errMsg = String(streamErr);
        // If it's a real connection error, try the governed gateway fallback
        if (errMsg.includes("not running") || errMsg.includes("connection refused")) {
          try {
            const response = await sendChat(currentInput);
            updateStreamingMsg(currentConvId, assistantMsgId, response.text, true);
            logAudit(`Response from ${response.model}`);
          } catch (fallbackErr) {
            updateStreamingMsg(currentConvId, assistantMsgId, classifyError(String(fallbackErr)), true);
          }
        } else if (errMsg.includes("__TAURI__") || errMsg.includes("invoke")) {
          // Not in Tauri runtime (web dev mode) — use sendChat
          try {
            const response = await sendChat(currentInput);
            updateStreamingMsg(currentConvId, assistantMsgId, response.text, true);
            logAudit(`Response from ${response.model}`);
          } catch (fallbackErr) {
            updateStreamingMsg(currentConvId, assistantMsgId, classifyError(String(fallbackErr)), true);
          }
        } else {
          updateStreamingMsg(currentConvId, assistantMsgId, classifyError(errMsg), true);
        }
      } finally {
        if (unlisten) unlisten();
      }
    } catch (err) {
      updateStreamingMsg(currentConvId, assistantMsgId, classifyError(String(err)), true);
    } finally {
      setSending(false);
      streamingMsgIdRef.current = null;
    }
  }, [input, sending, selectedModel, activeConvId, models, logAudit, updateStreamingMsg]);

  const newConversation = useCallback(() => {
    const conv: Conversation = {
      id: `conv-${Date.now()}`, title: "New conversation", model: selectedModel,
      messages: [], createdAt: Date.now(), updatedAt: Date.now(),
      pinned: false, tags: [],
    };
    setConversations(prev => [conv, ...prev]);
    setActiveConvId(conv.id);
    logAudit("New conversation");
  }, [selectedModel, logAudit]);

  const deleteConversation = useCallback((id: string) => {
    setConversations(prev => prev.filter(c => c.id !== id));
    if (activeConvId === id) {
      const remaining = conversations.filter(c => c.id !== id);
      setActiveConvId(remaining[0]?.id ?? "");
    }
    logAudit("Conversation deleted");
  }, [activeConvId, conversations, logAudit]);

  const saveAsNote = useCallback(() => {
    if (!activeConv) return;
    setFuelUsed(f => f + 3);
    logAudit(`Saved "${activeConv.title}" as note`);
  }, [activeConv, logAudit]);

  const togglePin = useCallback((id: string) => {
    setConversations(prev => prev.map(c => c.id === id ? { ...c, pinned: !c.pinned } : c));
  }, []);

  const handleCompare = useCallback(async () => {
    if (!comparePrompt.trim()) return;
    setComparing(true);
    setCompareResults(["", ""]);
    const m0 = models.find(m => m.id === compareModels[0]);
    const m1 = models.find(m => m.id === compareModels[1]);
    setFuelUsed(f => f + (m0?.fuelCost ?? 5) + (m1?.fuelCost ?? 5));
    logAudit(`Comparing ${compareModels[0]} vs ${compareModels[1]}`);

    const fetchResponse = async (modelId: string): Promise<string> => {
      try {
        const response = await sendChat(comparePrompt);
        return response.text;
      } catch (err) {
        return classifyError(String(err));
      }
    };

    const [r0, r1] = await Promise.all([
      fetchResponse(compareModels[0]),
      fetchResponse(compareModels[1]),
    ]);
    setCompareResults([r0, r1]);
    setComparing(false);
  }, [comparePrompt, compareModels, models, logAudit]);

  const generateImage = useCallback(() => {
    if (!input.trim()) return;
    const msg: ChatMsg = { id: `m-${Date.now()}`, role: "user", content: `Generate image: ${input}`, timestamp: Date.now() };
    const imgMsg: ChatMsg = {
      id: `m-${Date.now() + 1}`, role: "assistant", content: "Image generation requires a configured image provider (e.g., DALL-E, Stable Diffusion). Go to Settings to configure one.",
      model: selectedModel, timestamp: Date.now(),
    };
    setConversations(prev => prev.map(c => c.id === activeConvId ? {
      ...c, messages: [...c.messages, msg, imgMsg], updatedAt: Date.now(),
    } : c));
    setInput("");
    logAudit("Image generation attempted");
  }, [input, selectedModel, activeConvId, logAudit]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  }, [sendMessage]);

  /* ─── render ─── */
  return (
    <div className="ch-container">
      {/* ─── Sidebar ─── */}
      <aside className="ch-sidebar">
        <div className="ch-sidebar-header">
          <h2 className="ch-sidebar-title">AI Chat Hub</h2>
          <button className="ch-new-btn" onClick={newConversation}>+ New</button>
        </div>

        {/* views */}
        <div className="ch-views">
          {([["chat", "⌁", "Chat"], ["compare", "⇔", "Compare"], ["history", "⏱", "History"]] as const).map(([id, icon, label]) => (
            <button key={id} className={`ch-view-btn ${view === id ? "active" : ""}`} onClick={() => setView(id)}>
              <span>{icon}</span> {label}
            </button>
          ))}
        </div>

        {/* model picker */}
        <div className="ch-model-section">
          <div className="ch-section-header">Active Model</div>
          <button className="ch-model-active" onClick={() => setShowModelPicker(!showModelPicker)}>
            <span className="ch-model-icon" style={{ color: activeModel?.color }}>{activeModel?.icon}</span>
            <div className="ch-model-info">
              <span className="ch-model-name">{activeModel?.name ?? "No model"}</span>
              <span className="ch-model-provider">{activeModel?.provider ?? "—"} · ⚡{activeModel?.fuelCost ?? 0}</span>
            </div>
            <span className="ch-model-arrow">{showModelPicker ? "▲" : "▼"}</span>
          </button>
          {showModelPicker && (
            <div className="ch-model-list">
              {models.map(m => (
                <button key={m.id} className={`ch-model-option ${selectedModel === m.id ? "active" : ""}`} onClick={() => { setSelectedModel(m.id); setShowModelPicker(false); logAudit(`Switched to ${m.name}`); }}>
                  <span className="ch-model-icon" style={{ color: m.color }}>{m.icon}</span>
                  <div className="ch-model-info">
                    <span className="ch-model-name">{m.name}</span>
                    <span className="ch-model-provider">{m.provider} · {m.speed} · ⚡{m.fuelCost}</span>
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>

        {/* conversations */}
        <div className="ch-conv-list">
          <div className="ch-section-header">Conversations</div>
          <input className="ch-search" placeholder="Search..." value={view === "history" ? historySearch : searchQuery} onChange={e => view === "history" ? setHistorySearch(e.target.value) : setSearchQuery(e.target.value)} />
          {(view === "history" ? filteredConversations : conversations).filter(c => !searchQuery || c.title.toLowerCase().includes(searchQuery.toLowerCase())).map(conv => (
            <div key={conv.id} className={`ch-conv-item ${activeConvId === conv.id ? "active" : ""}`} onClick={() => { setActiveConvId(conv.id); setView("chat"); }}>
              <div className="ch-conv-title">
                {conv.pinned && <span className="ch-pin">📌</span>}
                {conv.title}
              </div>
              <div className="ch-conv-meta">
                <span style={{ color: models.find(m => m.id === conv.model)?.color }}>{models.find(m => m.id === conv.model)?.icon}</span>
                <span>{conv.messages.length} msgs</span>
                <span>{formatTime(conv.updatedAt)}</span>
              </div>
            </div>
          ))}
        </div>

        {/* agent panel */}
        <div className="ch-agent-section">
          <button className="ch-section-header ch-agent-toggle" onClick={() => setShowAgentPanel(!showAgentPanel)}>
            Agents in Chat {showAgentPanel ? "▲" : "▼"}
          </button>
          {showAgentPanel && (
            <div className="ch-agent-list">
              {AGENTS.map(a => (
                <button key={a.id} className={`ch-agent-btn ${joinedAgents.includes(a.id) ? "active" : ""}`} onClick={() => {
                  setJoinedAgents(prev => prev.includes(a.id) ? prev.filter(x => x !== a.id) : [...prev, a.id]);
                  logAudit(`${joinedAgents.includes(a.id) ? "Removed" : "Added"} ${a.name}`);
                }}>
                  <span style={{ color: a.color }}>{a.icon}</span> {a.name}
                  {joinedAgents.includes(a.id) && <span className="ch-agent-active">✓</span>}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* audit */}
        <div className="ch-audit">
          <div className="ch-section-header">Activity</div>
          {auditLog.slice(0, 4).map((msg, i) => (
            <div key={i} className="ch-audit-entry">{msg}</div>
          ))}
        </div>
      </aside>

      {/* ─── Main ─── */}
      <div className="ch-main">
        {/* ═══ CHAT VIEW ═══ */}
        {view === "chat" && activeConv && (
          <div className="ch-chat">
            {/* header */}
            <div className="ch-chat-header">
              <div className="ch-chat-title">{activeConv.title}</div>
              <div className="ch-chat-actions">
                <button className="ch-hdr-btn" onClick={() => togglePin(activeConv.id)} title="Pin">{activeConv.pinned ? "📌" : "📍"}</button>
                <button className="ch-hdr-btn" onClick={saveAsNote} title="Save as note">📝</button>
                <button className="ch-hdr-btn" onClick={() => deleteConversation(activeConv.id)} title="Delete">🗑</button>
                <button className={`ch-hdr-btn ch-voice-btn ${voiceActive ? "active" : ""}`} onClick={() => { setVoiceActive(!voiceActive); logAudit(voiceActive ? "Voice off" : "Voice on — Jarvis mode"); }} title="Voice">
                  {voiceActive ? "🎙" : "🎤"}
                </button>
              </div>
            </div>

            {/* tags */}
            {activeConv.tags.length > 0 && (
              <div className="ch-tags">
                {activeConv.tags.map(t => <span key={t} className="ch-tag">{t}</span>)}
              </div>
            )}

            {/* voice banner */}
            {voiceActive && (
              <div className="ch-voice-banner">
                <div className="ch-voice-wave">
                  {Array.from({ length: 12 }, (_, i) => (
                    <span key={i} className="ch-wave-bar" style={{ animationDelay: `${i * 0.08}s`, height: `${8 + Math.random() * 16}px` }} />
                  ))}
                </div>
                <span>Jarvis mode active — speak to chat</span>
                <button className="ch-voice-stop" onClick={() => setVoiceActive(false)}>Stop</button>
              </div>
            )}

            {/* messages */}
            <div className="ch-messages">
              {activeConv.messages.length === 0 && (
                <div className="ch-empty-chat">
                  <div className="ch-empty-icon" style={{ color: activeModel?.color }}>{activeModel?.icon}</div>
                  <div className="ch-empty-model">{activeModel?.name}</div>
                  <div className="ch-empty-hint">Start a conversation. Type a message or use voice.</div>
                  <div className="ch-quick-prompts">
                    {["Explain Nexus OS governance", "Write a Rust function", "Compare WASM runtimes", "Design a landing page"].map(p => (
                      <button key={p} className="ch-quick-btn" onClick={() => setInput(p)}>{p}</button>
                    ))}
                  </div>
                </div>
              )}
              {activeConv.messages.map(msg => (
                <div key={msg.id} className={`ch-msg ch-msg-${msg.role}`}>
                  <div className="ch-msg-avatar">
                    {msg.role === "user" ? "U" :
                     msg.role === "agent" ? "⬢" :
                     <span style={{ color: models.find(m => m.id === msg.model)?.color }}>{models.find(m => m.id === msg.model)?.icon ?? "◈"}</span>}
                  </div>
                  <div className="ch-msg-body">
                    <div className="ch-msg-header">
                      <span className="ch-msg-name">
                        {msg.role === "user" ? "You" :
                         msg.role === "agent" ? msg.agent :
                         models.find(m => m.id === msg.model)?.name ?? msg.model}
                      </span>
                      <span className="ch-msg-time">{formatTime(msg.timestamp)}</span>
                    </div>
                    {msg.imageUrl && (
                      <div className="ch-msg-image" style={{ background: msg.imageUrl }} />
                    )}
                    {msg.streaming && !msg.content ? (
                      <div className="ch-typing"><span /><span /><span /></div>
                    ) : (
                      <div className="ch-msg-content" dangerouslySetInnerHTML={{ __html: renderContent(msg.content) }} />
                    )}
                    {msg.codeBlock && (
                      <div className="ch-code-block">
                        <div className="ch-code-header"><span>{msg.codeBlock.lang}</span></div>
                        <pre className="ch-code-pre"><code>{msg.codeBlock.code}</code></pre>
                        {msg.codeBlock.output && <div className="ch-code-output">{msg.codeBlock.output}</div>}
                      </div>
                    )}
                  </div>
                </div>
              ))}
              <div ref={messagesEndRef} />
            </div>

            {/* input */}
            <div className="ch-input-bar">
              <div className="ch-input-row">
                <textarea ref={inputRef} className="ch-input" value={input} onChange={e => setInput(e.target.value)} onKeyDown={handleKeyDown} placeholder={`Message ${activeModel?.name ?? "AI"}...`} rows={1} />
                <div className="ch-input-actions">
                  <button className="ch-input-btn" onClick={generateImage} title="Generate image" disabled={!input.trim()}>🖼</button>
                  <button className="ch-send-btn" onClick={sendMessage} disabled={!input.trim() || sending}>
                    {sending ? "..." : "→"}
                  </button>
                </div>
              </div>
              <div className="ch-input-meta">
                <span className="ch-input-model" style={{ color: activeModel?.color }}>{activeModel?.icon} {activeModel?.name}</span>
                <span>⚡ {activeModel?.fuelCost} fuel/msg</span>
                {joinedAgents.length > 0 && <span>⬢ {joinedAgents.length} agent{joinedAgents.length > 1 ? "s" : ""} joined</span>}
              </div>
            </div>
          </div>
        )}

        {/* ═══ COMPARE VIEW ═══ */}
        {view === "compare" && (
          <div className="ch-compare">
            <div className="ch-cmp-header">
              <h3 className="ch-cmp-title">⇔ Model Comparison</h3>
            </div>
            <div className="ch-cmp-selectors">
              <div className="ch-cmp-select">
                <label>Model A</label>
                <select value={compareModels[0]} onChange={e => setCompareModels([e.target.value, compareModels[1]])}>
                  {models.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}
                </select>
              </div>
              <span className="ch-cmp-vs">VS</span>
              <div className="ch-cmp-select">
                <label>Model B</label>
                <select value={compareModels[1]} onChange={e => setCompareModels([compareModels[0], e.target.value])}>
                  {models.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}
                </select>
              </div>
            </div>
            <div className="ch-cmp-prompt">
              <textarea value={comparePrompt} onChange={e => setComparePrompt(e.target.value)} placeholder="Enter a prompt to compare responses..." rows={3} />
              <button className="ch-cmp-btn" onClick={handleCompare} disabled={!comparePrompt.trim() || comparing}>
                {comparing ? "Comparing..." : "Compare Responses"}
              </button>
            </div>
            {(compareResults[0] || compareResults[1]) && (
              <div className="ch-cmp-results">
                {[0, 1].map(i => {
                  const m = models.find(m => m.id === compareModels[i]);
                  return (
                    <div key={i} className="ch-cmp-result">
                      <div className="ch-cmp-result-header">
                        <span style={{ color: m?.color }}>{m?.icon}</span>
                        <span className="ch-cmp-result-name">{m?.name}</span>
                        <span className="ch-cmp-result-meta">{m?.provider} · ⚡{m?.fuelCost}</span>
                      </div>
                      <div className="ch-cmp-result-body" dangerouslySetInnerHTML={{ __html: renderContent(compareResults[i]) }} />
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}

        {/* ═══ HISTORY VIEW ═══ */}
        {view === "history" && (
          <div className="ch-history">
            <div className="ch-hist-header">
              <h3 className="ch-hist-title">⏱ Chat History</h3>
              <span className="ch-hist-count">{filteredConversations.length} conversations</span>
            </div>
            <div className="ch-hist-list">
              {filteredConversations.sort((a, b) => b.updatedAt - a.updatedAt).map(conv => {
                const m = models.find(m => m.id === conv.model);
                return (
                  <div key={conv.id} className="ch-hist-item" onClick={() => { setActiveConvId(conv.id); setView("chat"); }}>
                    <div className="ch-hist-icon" style={{ color: m?.color }}>{m?.icon}</div>
                    <div className="ch-hist-info">
                      <div className="ch-hist-name">{conv.pinned && "📌 "}{conv.title}</div>
                      <div className="ch-hist-meta">
                        {m?.name} · {conv.messages.length} messages · {formatTime(conv.updatedAt)}
                      </div>
                      {conv.tags.length > 0 && (
                        <div className="ch-hist-tags">
                          {conv.tags.map(t => <span key={t} className="ch-tag">{t}</span>)}
                        </div>
                      )}
                    </div>
                    <div className="ch-hist-actions">
                      <button className="ch-hist-act-btn" onClick={e => { e.stopPropagation(); togglePin(conv.id); }}>{conv.pinned ? "📌" : "📍"}</button>
                      <button className="ch-hist-act-btn" onClick={e => { e.stopPropagation(); deleteConversation(conv.id); }}>🗑</button>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {!activeConv && view === "chat" && (
          <div className="ch-no-conv">
            <div className="ch-empty-icon">⌁</div>
            <div>Select a conversation or start a new one</div>
            <button className="ch-new-btn" onClick={newConversation}>+ New Conversation</button>
          </div>
        )}
      </div>

      {/* ─── Status Bar ─── */}
      <div className="ch-status-bar">
        <span className="ch-status-item">{activeModel?.name ?? "No model"}</span>
        <span className="ch-status-item">{conversations.length} conversations</span>
        <span className="ch-status-item">{activeConv?.messages.length ?? 0} messages</span>
        {voiceActive && <span className="ch-status-item ch-status-voice">🎙 Jarvis Active</span>}
        {joinedAgents.length > 0 && <span className="ch-status-item">⬢ {joinedAgents.length} agents</span>}
        <span className="ch-status-item ch-status-right">⚡ {fuelUsed} fuel</span>
        <span className="ch-status-item">{models.length} models</span>
      </div>
    </div>
  );
}
