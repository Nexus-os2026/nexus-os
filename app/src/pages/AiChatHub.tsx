import { useState, useCallback, useMemo, useRef, useEffect } from "react";
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
const MODELS: Model[] = [
  { id: "claude-opus", name: "Claude Opus 4.5", provider: "Anthropic", icon: "◈", color: "#d4a574", speed: "medium", capability: "expert", fuelCost: 25 },
  { id: "claude-sonnet", name: "Claude Sonnet 4.5", provider: "Anthropic", icon: "◈", color: "#c49b6a", speed: "fast", capability: "advanced", fuelCost: 12 },
  { id: "claude-haiku", name: "Claude Haiku 4.5", provider: "Anthropic", icon: "◈", color: "#b08e60", speed: "fast", capability: "basic", fuelCost: 4 },
  { id: "gpt-4o", name: "GPT-4o", provider: "OpenAI", icon: "●", color: "#10a37f", speed: "fast", capability: "expert", fuelCost: 20 },
  { id: "gpt-4o-mini", name: "GPT-4o Mini", provider: "OpenAI", icon: "●", color: "#0d8c6d", speed: "fast", capability: "basic", fuelCost: 3 },
  { id: "gemini-pro", name: "Gemini 2.0 Pro", provider: "Google", icon: "◆", color: "#4285f4", speed: "medium", capability: "expert", fuelCost: 18 },
  { id: "gemini-flash", name: "Gemini 2.0 Flash", provider: "Google", icon: "◆", color: "#34a853", speed: "fast", capability: "advanced", fuelCost: 6 },
  { id: "llama-70b", name: "Llama 3.3 70B", provider: "Meta (local)", icon: "🦙", color: "#0668e1", speed: "slow", capability: "advanced", fuelCost: 8 },
  { id: "qwen-72b", name: "Qwen 3 72B", provider: "Alibaba (local)", icon: "Q", color: "#ff6a00", speed: "slow", capability: "advanced", fuelCost: 8 },
];

const AGENTS = [
  { id: "coder", name: "Coder Agent", icon: "⬢", color: "#22d3ee" },
  { id: "designer", name: "Designer Agent", icon: "⬢", color: "#a78bfa" },
  { id: "research", name: "Research Agent", icon: "⬢", color: "#22c55e" },
  { id: "self-improve", name: "Self-Improve", icon: "⬢", color: "#f59e0b" },
];

const MOCK_RESPONSES: Record<string, (input: string) => string> = {
  "claude-opus": (input) => `I've carefully analyzed your request: "${input.slice(0, 50)}..."\n\nHere's my comprehensive response:\n\n1. **Analysis**: This involves several interconnected considerations.\n2. **Recommendation**: Based on the Nexus OS governance model, I suggest a capability-checked approach.\n3. **Implementation**: I can provide detailed code with full audit trail integration.\n\nShall I elaborate on any of these points?`,
  "claude-sonnet": (input) => `Here's my take on "${input.slice(0, 40)}...":\n\nThe most efficient approach would be to leverage the existing kernel capability system. This ensures governance compliance while maintaining performance.\n\nI can write the implementation if you'd like.`,
  "gpt-4o": (input) => `Great question about "${input.slice(0, 40)}..."!\n\nI'd approach this by:\n1. Breaking it down into manageable components\n2. Implementing each with proper error handling\n3. Adding comprehensive test coverage\n\nWant me to start with the implementation?`,
  "gemini-pro": (input) => `Analyzing: "${input.slice(0, 40)}..."\n\nBased on my analysis, here are the key insights:\n\n- **Approach A**: Higher performance, more complex setup\n- **Approach B**: Simpler implementation, easier maintenance\n\nFor Nexus OS specifically, I'd recommend Approach B with governance hooks. Let me know if you want details.`,
  "llama-70b": (input) => `Processing: "${input.slice(0, 40)}..."\n\nRunning locally on your hardware. Here's what I found:\n\nThe core logic can be implemented with about 50 lines of Rust. The key is to ensure the fuel budget is checked before each operation.\n\n\`\`\`rust\nfn process(ctx: &Context) -> Result<()> {\n    ctx.check_fuel()?;\n    // implementation\n    Ok(())\n}\n\`\`\``,
  "qwen-72b": (input) => `Analysis of "${input.slice(0, 40)}..."\n\nI can help with this. Here's a structured approach:\n\n1. Define the data model\n2. Implement the core logic\n3. Add governance checks\n4. Write tests\n\nAll processing stays on your local machine.`,
};

function getDefaultResponse(model: string, input: string): string {
  const fn = MOCK_RESPONSES[model];
  if (fn) return fn(input);
  const m = MODELS.find(m => m.id === model);
  return `[${m?.name ?? model}] Processing: "${input.slice(0, 50)}..."\n\nHere's my response based on analysis of your request. All actions are governed with capability checks and audit logging.`;
}

const INITIAL_CONVERSATIONS: Conversation[] = [
  {
    id: "conv-1", title: "Auth middleware security review", model: "claude-opus", pinned: true, tags: ["security", "code"],
    createdAt: Date.now() - 86400000, updatedAt: Date.now() - 3600000,
    messages: [
      { id: "m-1", role: "user", content: "Review the auth middleware in src/auth.rs for security issues", timestamp: Date.now() - 86400000 },
      { id: "m-2", role: "assistant", content: "Found 3 issues:\n\n1. JWT token validation missing expiration check (line 42)\n2. CORS wildcard `*` in production (line 18)\n3. In-memory rate limiter won't work multi-instance\n\nShall I generate patches?", model: "claude-opus", timestamp: Date.now() - 86300000 },
      { id: "m-3", role: "user", content: "Fix issues 1 and 2", timestamp: Date.now() - 86200000 },
      { id: "m-4", role: "assistant", content: "Patches applied:\n\n`src/auth.rs:42` — Added `validate_exp: true`\n`src/auth.rs:18` — CORS restricted to `https://app.nexus-os.dev`\n\nAll 12 auth tests pass.", model: "claude-opus", timestamp: Date.now() - 86100000 },
    ],
  },
  {
    id: "conv-2", title: "WASM runtime comparison", model: "gemini-pro", pinned: false, tags: ["research", "wasm"],
    createdAt: Date.now() - 172800000, updatedAt: Date.now() - 172000000,
    messages: [
      { id: "m-5", role: "user", content: "Compare Wasmtime vs Wasmer for agent sandboxing", timestamp: Date.now() - 172800000 },
      { id: "m-6", role: "assistant", content: "**Wasmtime vs Wasmer for Agent Sandboxing**\n\n| Feature | Wasmtime | Wasmer |\n|---------|----------|--------|\n| Security | Capability-based ✓ | Filesystem isolation |\n| Speed | ~5ms startup | ~3ms startup |\n| WASI | Full support | Full support |\n| Memory | Lower overhead | Higher throughput |\n\n**Recommendation**: Wasmtime for production — better security model aligns with Nexus OS governance.", model: "gemini-pro", timestamp: Date.now() - 172700000 },
    ],
  },
  {
    id: "conv-3", title: "Landing page design", model: "claude-sonnet", pinned: false, tags: ["design"],
    createdAt: Date.now() - 259200000, updatedAt: Date.now() - 259100000,
    messages: [
      { id: "m-7", role: "user", content: "Design a cyberpunk landing page for Nexus OS", timestamp: Date.now() - 259200000 },
      { id: "m-8", role: "assistant", content: "Here's a concept:\n\n- **Hero**: Dark navy (#0b1120) with cyan (#22d3ee) accent particles\n- **Tagline**: \"Don't trust. Verify.\" in monospace\n- **CTA**: Glowing cyan button with pulse animation\n- **Features**: 3-column grid with hover-reveal cards\n- **Footer**: Audit trail live-stream ticker\n\nI can generate the React component if you want.", model: "claude-sonnet", timestamp: Date.now() - 259100000 },
      { id: "m-9", role: "agent", content: "I've generated a mockup based on the description. The design uses the Nexus OS design tokens and is responsive. Preview available in Design Studio.", agent: "Designer Agent", timestamp: Date.now() - 259000000 },
    ],
  },
];

/* ─── component ─── */
export default function AiChatHub() {
  const [view, setView] = useState<View>("chat");
  const [conversations, setConversations] = useState<Conversation[]>(INITIAL_CONVERSATIONS);
  const [activeConvId, setActiveConvId] = useState("conv-1");
  const [selectedModel, setSelectedModel] = useState("claude-opus");
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [historySearch, setHistorySearch] = useState("");
  const [fuelUsed, setFuelUsed] = useState(87);
  const [voiceActive, setVoiceActive] = useState(false);
  const [showModelPicker, setShowModelPicker] = useState(false);
  const [showAgentPanel, setShowAgentPanel] = useState(false);
  const [joinedAgents, setJoinedAgents] = useState<string[]>([]);
  const [auditLog, setAuditLog] = useState<string[]>(["Chat session started", "Model: Claude Opus 4.5"]);

  // compare state
  const [compareModels, setCompareModels] = useState<[string, string]>(["claude-opus", "gpt-4o"]);
  const [comparePrompt, setComparePrompt] = useState("");
  const [compareResults, setCompareResults] = useState<[string, string]>(["", ""]);
  const [comparing, setComparing] = useState(false);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  const activeConv = useMemo(() => conversations.find(c => c.id === activeConvId), [conversations, activeConvId]);
  const activeModel = useMemo(() => MODELS.find(m => m.id === selectedModel), [selectedModel]);

  const logAudit = useCallback((msg: string) => setAuditLog(prev => [msg, ...prev].slice(0, 30)), []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [activeConv?.messages.length]);

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
    let html = content
      .replace(/</g, "&lt;").replace(/>/g, "&gt;")
      .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
      .replace(/`([^`]+)`/g, '<code class="ch-inline-code">$1</code>')
      .replace(/\n/g, "<br/>");
    // Restore code blocks (they were double-escaped, undo)
    html = content;
    html = highlightCode(html);
    html = html
      .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
      .replace(/(?<!`)`(?!`)([^`\n]+)`(?!`)/g, '<code class="ch-inline-code">$1</code>');
    html = html.replace(/\n/g, "<br/>");
    return html;
  };

  /* ─── actions ─── */
  const sendMessage = useCallback(() => {
    if (!input.trim() || sending) return;
    const model = MODELS.find(m => m.id === selectedModel);
    const msg: ChatMsg = { id: `m-${Date.now()}`, role: "user", content: input, timestamp: Date.now() };

    setConversations(prev => prev.map(c => c.id === activeConvId ? {
      ...c, messages: [...c.messages, msg], updatedAt: Date.now(),
      title: c.messages.length === 0 ? input.slice(0, 50) : c.title,
    } : c));

    setInput("");
    setSending(true);
    setFuelUsed(f => f + (model?.fuelCost ?? 10));
    logAudit(`Sent to ${model?.name ?? selectedModel}`);

    // Simulate response
    setTimeout(() => {
      const response = getDefaultResponse(selectedModel, input);
      const assistantMsg: ChatMsg = {
        id: `m-${Date.now()}`, role: "assistant", content: response,
        model: selectedModel, timestamp: Date.now(),
      };
      setConversations(prev => prev.map(c => c.id === activeConvId ? {
        ...c, messages: [...c.messages, assistantMsg], updatedAt: Date.now(),
      } : c));
      setSending(false);

      // Simulate agent join if agents are joined
      if (joinedAgents.length > 0) {
        setTimeout(() => {
          const agent = AGENTS.find(a => a.id === joinedAgents[0]);
          if (!agent) return;
          const agentMsg: ChatMsg = {
            id: `m-${Date.now()}`, role: "agent", content: `I've reviewed the response and can add context from my recent work. This aligns with the patterns I've seen in the codebase. I can assist further if needed.`,
            agent: agent.name, timestamp: Date.now(),
          };
          setConversations(prev => prev.map(c => c.id === activeConvId ? {
            ...c, messages: [...c.messages, agentMsg], updatedAt: Date.now(),
          } : c));
        }, 1500);
      }
    }, 1200 + Math.random() * 1000);
  }, [input, sending, selectedModel, activeConvId, joinedAgents, logAudit]);

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

  const handleCompare = useCallback(() => {
    if (!comparePrompt.trim()) return;
    setComparing(true);
    setCompareResults(["", ""]);
    setFuelUsed(f => f + (MODELS.find(m => m.id === compareModels[0])?.fuelCost ?? 10) + (MODELS.find(m => m.id === compareModels[1])?.fuelCost ?? 10));
    logAudit(`Comparing ${compareModels[0]} vs ${compareModels[1]}`);
    setTimeout(() => {
      setCompareResults([
        getDefaultResponse(compareModels[0], comparePrompt),
        getDefaultResponse(compareModels[1], comparePrompt),
      ]);
      setComparing(false);
    }, 2000);
  }, [comparePrompt, compareModels, logAudit]);

  const generateImage = useCallback(() => {
    if (!input.trim()) return;
    const msg: ChatMsg = { id: `m-${Date.now()}`, role: "user", content: `🖼 Generate image: ${input}`, timestamp: Date.now() };
    const imgMsg: ChatMsg = {
      id: `m-${Date.now() + 1}`, role: "assistant", content: `Generated image for: "${input.slice(0, 40)}..."`,
      model: selectedModel, timestamp: Date.now(),
      imageUrl: `linear-gradient(${Math.floor(Math.random() * 360)}deg, #0f172a 0%, #${Math.floor(Math.random() * 16777215).toString(16).padStart(6, "0")} 50%, #22d3ee 100%)`,
    };
    setConversations(prev => prev.map(c => c.id === activeConvId ? {
      ...c, messages: [...c.messages, msg, imgMsg], updatedAt: Date.now(),
    } : c));
    setInput("");
    setFuelUsed(f => f + 20);
    logAudit("Image generated in chat");
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
              <span className="ch-model-name">{activeModel?.name}</span>
              <span className="ch-model-provider">{activeModel?.provider} · ⚡{activeModel?.fuelCost}</span>
            </div>
            <span className="ch-model-arrow">{showModelPicker ? "▲" : "▼"}</span>
          </button>
          {showModelPicker && (
            <div className="ch-model-list">
              {MODELS.map(m => (
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
                <span style={{ color: MODELS.find(m => m.id === conv.model)?.color }}>{MODELS.find(m => m.id === conv.model)?.icon}</span>
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
                     <span style={{ color: MODELS.find(m => m.id === msg.model)?.color }}>{MODELS.find(m => m.id === msg.model)?.icon ?? "◈"}</span>}
                  </div>
                  <div className="ch-msg-body">
                    <div className="ch-msg-header">
                      <span className="ch-msg-name">
                        {msg.role === "user" ? "You" :
                         msg.role === "agent" ? msg.agent :
                         MODELS.find(m => m.id === msg.model)?.name ?? msg.model}
                      </span>
                      <span className="ch-msg-time">{formatTime(msg.timestamp)}</span>
                    </div>
                    {msg.imageUrl && (
                      <div className="ch-msg-image" style={{ background: msg.imageUrl }} />
                    )}
                    <div className="ch-msg-content" dangerouslySetInnerHTML={{ __html: renderContent(msg.content) }} />
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
              {sending && (
                <div className="ch-msg ch-msg-assistant">
                  <div className="ch-msg-avatar"><span style={{ color: activeModel?.color }}>{activeModel?.icon}</span></div>
                  <div className="ch-msg-body">
                    <div className="ch-msg-header"><span className="ch-msg-name">{activeModel?.name}</span></div>
                    <div className="ch-typing"><span /><span /><span /></div>
                  </div>
                </div>
              )}
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
                  {MODELS.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}
                </select>
              </div>
              <span className="ch-cmp-vs">VS</span>
              <div className="ch-cmp-select">
                <label>Model B</label>
                <select value={compareModels[1]} onChange={e => setCompareModels([compareModels[0], e.target.value])}>
                  {MODELS.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}
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
                  const m = MODELS.find(m => m.id === compareModels[i]);
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
                const m = MODELS.find(m => m.id === conv.model);
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
        <span className="ch-status-item">{MODELS.length} models</span>
      </div>
    </div>
  );
}
