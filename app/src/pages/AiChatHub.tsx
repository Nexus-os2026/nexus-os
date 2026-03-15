import { useState, useCallback, useMemo, useRef, useEffect } from "react";
import { sendChat, chatWithOllama, conductBuild, listAgents, hasDesktopRuntime, listProviderModels, getProviderStatus, saveApiKey } from "../api/backend";
import type { ChatTokenEvent, ConductorPlanEvent, ConductorAgentCompletedEvent, ConductorFinishedEvent, ConductorBuildResponse, ProviderModel, ProviderStatus } from "../types";
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
  local: boolean;
  locked: boolean;
}

interface BuildResultData {
  plan: ConductorBuildResponse["plan"];
  result: ConductorBuildResponse["result"];
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
  buildResult?: BuildResultData;
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

const PROVIDER_META: Record<string, { icon: string; color: string; label: string; fuelCost: number }> = {
  ollama: { icon: "◆", color: "#22c55e", label: "Ollama", fuelCost: 5 },
  anthropic: { icon: "◈", color: "#d4a574", label: "Anthropic", fuelCost: 15 },
  openai: { icon: "◈", color: "#74b9ff", label: "OpenAI", fuelCost: 12 },
  deepseek: { icon: "◈", color: "#a78bfa", label: "DeepSeek", fuelCost: 3 },
  google: { icon: "◈", color: "#ffd700", label: "Google", fuelCost: 10 },
};

// Cloud models to show as locked when no API key is configured
const LOCKED_CLOUD_MODELS: Array<{ id: string; name: string; provider: string }> = [
  { id: "anthropic/claude-sonnet-4-20250514", name: "Claude Sonnet 4", provider: "anthropic" },
  { id: "anthropic/claude-opus-4-6", name: "Claude Opus 4.6", provider: "anthropic" },
  { id: "openai/gpt-4o", name: "GPT-4o", provider: "openai" },
  { id: "openai/gpt-4o-mini", name: "GPT-4o Mini", provider: "openai" },
  { id: "deepseek/deepseek-chat", name: "DeepSeek Chat", provider: "deepseek" },
  { id: "deepseek/deepseek-coder", name: "DeepSeek Coder", provider: "deepseek" },
  { id: "google/gemini-2.5-pro", name: "Gemini 2.5 Pro", provider: "google" },
  { id: "google/gemini-2.5-flash", name: "Gemini 2.5 Flash", provider: "google" },
];

interface AgentEntry {
  id: string;
  name: string;
  icon: string;
  color: string;
}

const BUILD_ACTION_KEYWORDS = ["build", "create", "generate", "make me", "design", "fix", "clone"];
const BUILD_TARGET_KEYWORDS = ["website", "site", "app", "page", "project", "component", "landing", "portfolio", "dashboard", "frontend"];

function isBuildRequest(msg: string): boolean {
  const lower = msg.toLowerCase();
  return BUILD_ACTION_KEYWORDS.some(kw => lower.includes(kw))
    && BUILD_TARGET_KEYWORDS.some(kw => lower.includes(kw));
}

function providerModelToModel(m: ProviderModel, locked: boolean): Model {
  const meta = PROVIDER_META[m.provider] ?? { icon: "◈", color: "#888", label: m.provider, fuelCost: 5 };
  return {
    id: m.id,
    name: m.name,
    provider: meta.label,
    icon: meta.icon,
    color: meta.color,
    speed: m.local ? "fast" : "medium",
    capability: m.local ? "advanced" : "expert",
    fuelCost: meta.fuelCost,
    local: m.local,
    locked,
  };
}

function providerDisplayName(id: string): string {
  const prefix = id.split("/")[0];
  return PROVIDER_META[prefix]?.label ?? prefix;
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

/* ─── Build Result Card ─── */
function BuildResultCard({ data }: { data: BuildResultData }) {
  const { plan, result } = data;
  const openPreview = () => {
    const indexPath = result.output_files.find(f => f.endsWith("index.html"));
    if (!indexPath) return;
    window.open(`file://${indexPath}`, "_blank");
  };

  const openFileManager = () => {
    // Navigate to File Manager page with output_dir context via custom event
    window.dispatchEvent(new CustomEvent("nexus:navigate", { detail: { page: "file-manager", path: result.output_dir } }));
  };

  const hasIndex = result.output_files.some(f => f.endsWith("index.html"));

  return (
    <div className="ch-build-card">
      <div className="ch-build-header">
        <span className="ch-build-icon">⬢</span>
        <span className="ch-build-title">Conductor Build Complete</span>
        <span className={`ch-build-status ch-build-status-${result.status.toLowerCase()}`}>
          {result.status}
        </span>
      </div>

      {/* Plan summary */}
      <div className="ch-build-section">
        <div className="ch-build-section-title">Plan ({plan.tasks.length} tasks)</div>
        <div className="ch-build-tasks">
          {plan.tasks.map((task, i) => (
            <div key={i} className="ch-build-task">
              <span className="ch-build-task-check">✓</span>
              <span className="ch-build-task-desc">{task.description}</span>
              <span className="ch-build-task-role">{task.role}</span>
            </div>
          ))}
        </div>
      </div>

      {/* Files generated */}
      {result.output_files.length > 0 && (
        <div className="ch-build-section">
          <div className="ch-build-section-title">Files Generated ({result.output_files.length})</div>
          <div className="ch-build-files">
            {result.output_files.map((f, i) => (
              <div key={i} className="ch-build-file">{f.split("/").pop()}</div>
            ))}
          </div>
        </div>
      )}

      {/* Stats */}
      <div className="ch-build-stats">
        <span className="ch-build-stat">⬢ {result.agents_used} agents</span>
        <span className="ch-build-stat">⚡ {result.total_fuel_used} fuel</span>
        <span className="ch-build-stat">⏱ {result.duration_secs.toFixed(1)}s</span>
      </div>

      {/* Summary */}
      {result.summary && (
        <div className="ch-build-summary">{result.summary}</div>
      )}

      {/* Actions */}
      <div className="ch-build-actions">
        {hasIndex && (
          <button className="ch-build-btn ch-build-btn-primary" onClick={openPreview}>
            ▶ Preview
          </button>
        )}
        <button className="ch-build-btn" onClick={openFileManager}>
          📁 View Files
        </button>
      </div>
    </div>
  );
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
  const [models, setModels] = useState<Model[]>([]);
  const [selectedModel, setSelectedModel] = useState(() =>
    localStorage.getItem("nexus-selected-model") || ""
  );
  const [providerStatus, setProviderStatus] = useState<ProviderStatus | null>(null);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [historySearch, setHistorySearch] = useState("");
  const [fuelUsed, setFuelUsed] = useState(0);
  const [voiceActive, setVoiceActive] = useState(false);
  const [showModelPicker, setShowModelPicker] = useState(false);
  const [showAgentPanel, setShowAgentPanel] = useState(false);
  const [showApiKeyModal, setShowApiKeyModal] = useState<string | null>(null);
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [apiKeySaving, setApiKeySaving] = useState(false);
  const [agentEntries, setAgentEntries] = useState<AgentEntry[]>([]);
  const [joinedAgents, setJoinedAgents] = useState<string[]>([]);
  const [auditLog, setAuditLog] = useState<string[]>(["Chat hub ready"]);
  const [conductorProgress, setConductorProgress] = useState<string[]>([]);

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

  /* ─── load models from all providers ─── */
  const loadModels = useCallback(async () => {
    try {
      const [provModels, status] = await Promise.all([
        listProviderModels(),
        getProviderStatus(),
      ]);
      setProviderStatus(status);

      // Map provider models
      const mapped: Model[] = provModels.map(m => providerModelToModel(m, false));

      // Add locked cloud models that aren't already present (no API key)
      const existingIds = new Set(mapped.map(m => m.id));
      for (const locked of LOCKED_CLOUD_MODELS) {
        if (!existingIds.has(locked.id)) {
          const meta = PROVIDER_META[locked.provider] ?? { icon: "◈", color: "#888", label: locked.provider, fuelCost: 5 };
          mapped.push({
            id: locked.id,
            name: locked.name,
            provider: meta.label,
            icon: "🔒",
            color: "#666",
            speed: "medium",
            capability: "expert",
            fuelCost: meta.fuelCost,
            local: false,
            locked: true,
          });
        }
      }

      setModels(mapped);

      // Auto-select: prefer saved model, then first available unlocked model
      const saved = localStorage.getItem("nexus-selected-model");
      const savedValid = saved && mapped.some(m => m.id === saved && !m.locked);
      if (!savedValid) {
        const firstUnlocked = mapped.find(m => !m.locked);
        if (firstUnlocked) {
          setSelectedModel(firstUnlocked.id);
          localStorage.setItem("nexus-selected-model", firstUnlocked.id);
        }
      }

      if (!compareModels[0] && mapped.length >= 2) {
        const unlocked = mapped.filter(m => !m.locked);
        if (unlocked.length >= 2) setCompareModels([unlocked[0].id, unlocked[1].id]);
        else if (unlocked.length >= 1) setCompareModels([unlocked[0].id, unlocked[0].id]);
      }

      logAudit(`Loaded ${mapped.filter(m => !m.locked).length} model(s) from ${new Set(provModels.map(m => m.provider)).size} provider(s)`);
    } catch {
      logAudit("Backend unavailable — no models loaded");
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    loadModels();
  }, [loadModels]);

  /* ─── load agents from backend ─── */
  useEffect(() => {
    if (!hasDesktopRuntime()) return;
    const AGENT_COLORS = ["var(--nexus-accent)", "#a78bfa", "#22c55e", "#f59e0b", "#60a5fa", "#fb923c"];
    listAgents().then((agents) => {
      setAgentEntries(agents.map((a, i) => ({
        id: a.id,
        name: a.name,
        icon: "\u2B22",
        color: AGENT_COLORS[i % AGENT_COLORS.length],
      })));
    }).catch(() => {});
  }, []);

  /* ─── listen for conductor events ─── */
  useEffect(() => {
    let unlistenPlan: (() => void) | undefined;
    let unlistenAgent: (() => void) | undefined;
    let unlistenFinished: (() => void) | undefined;

    (async () => {
      try {
        const eventMod = await import("@tauri-apps/api/event");

        unlistenPlan = await eventMod.listen<ConductorPlanEvent>("conductor:plan", (event) => {
          const plan = event.payload;
          const taskList = plan.tasks.map(t => t.description).join(", ");
          setConductorProgress(prev => [...prev, `Plan: ${taskList}`]);
          logAudit(`Conductor plan: ${plan.tasks.length} tasks`);
        });

        unlistenAgent = await eventMod.listen<ConductorAgentCompletedEvent>("conductor:agent_completed", (event) => {
          const { agents_used, output_files } = event.payload;
          setConductorProgress(prev => [...prev, `Agents completed: ${agents_used}, files: ${output_files.length}`]);
          logAudit(`Conductor: ${agents_used} agents done`);
        });

        unlistenFinished = await eventMod.listen<ConductorFinishedEvent>("conductor:finished", (event) => {
          const res = event.payload;
          setConductorProgress(prev => [...prev, `Finished: ${res.status} in ${res.duration_secs.toFixed(1)}s`]);
          logAudit(`Conductor finished: ${res.status}`);
        });
      } catch {
        // Not in Tauri runtime
      }
    })();

    return () => {
      unlistenPlan?.();
      unlistenAgent?.();
      unlistenFinished?.();
    };
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

  const appendBuildResult = useCallback((convId: string, msgId: string, buildResult: BuildResultData) => {
    setConversations(prev => prev.map(c => c.id === convId ? {
      ...c,
      messages: c.messages.map(m => m.id === msgId ? {
        ...m,
        content: `Build complete: ${buildResult.result.summary || buildResult.result.status}`,
        streaming: false,
        buildResult,
      } : m),
      updatedAt: Date.now(),
    } : c));
  }, []);

  /* ─── send to Conductor ─── */
  const sendBuildRequest = useCallback(async (currentInput: string, currentConvId: string, assistantMsgId: string) => {
    logAudit("Routing to Conductor...");
    setConductorProgress([]);
    updateStreamingMsg(currentConvId, assistantMsgId, "⬢ Conductor orchestrating build...", false);

    try {
      const response = await conductBuild(currentInput, undefined, selectedModel);

      const buildData: BuildResultData = {
        plan: response.plan,
        result: response.result,
      };

      appendBuildResult(currentConvId, assistantMsgId, buildData);
      setFuelUsed(f => f + response.result.total_fuel_used);
      logAudit(`Build complete: ${response.result.output_files.length} files, ${response.result.total_fuel_used} fuel`);
    } catch (err) {
      updateStreamingMsg(currentConvId, assistantMsgId, `Build failed: ${classifyError(String(err))}`, true);
      logAudit(`Build failed: ${String(err).slice(0, 60)}`);
    }
  }, [selectedModel, logAudit, updateStreamingMsg, appendBuildResult]);

  /* ─── send message (real backend, multi-provider) ─── */
  const sendMessage = useCallback(async () => {
    if (!input.trim() || sending) return;
    const model = models.find(m => m.id === selectedModel);
    if (model?.locked) {
      setShowApiKeyModal(selectedModel.split("/")[0]);
      return;
    }
    const userMsg: ChatMsg = { id: `m-${Date.now()}`, role: "user", content: input, timestamp: Date.now() };
    const currentInput = input;
    const currentConvId = activeConvId;
    const isOllamaModel = selectedModel.startsWith("ollama/");
    const ollamaModelName = isOllamaModel ? selectedModel.slice("ollama/".length) : selectedModel;

    setConversations(prev => prev.map(c => c.id === currentConvId ? {
      ...c, messages: [...c.messages, userMsg], updatedAt: Date.now(),
      title: c.messages.length === 0 ? currentInput.slice(0, 50) : c.title,
    } : c));

    setInput("");
    setSending(true);
    setFuelUsed(f => f + (model?.fuelCost ?? 5));
    logAudit(`Sent to ${model?.name ?? selectedModel} via ${providerDisplayName(selectedModel)}`);

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
      // Check if this is a build request — route to Conductor
      if (isBuildRequest(currentInput)) {
        await sendBuildRequest(currentInput, currentConvId, assistantMsgId);
        return;
      }

      // For Ollama models: use streaming path
      if (isOllamaModel) {
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
          await chatWithOllama(messages, ollamaModelName);
        } catch (streamErr) {
          const errMsg = String(streamErr);
          if (errMsg.includes("__TAURI__") || errMsg.includes("invoke")) {
            // Not in Tauri runtime — fallback to send_chat
            try {
              const response = await sendChat(currentInput, selectedModel);
              updateStreamingMsg(currentConvId, assistantMsgId, response.text, true);
              logAudit(`Response from ${response.model} via ${providerDisplayName(selectedModel)}`);
            } catch (fallbackErr) {
              updateStreamingMsg(currentConvId, assistantMsgId, classifyError(String(fallbackErr)), true);
            }
          } else {
            updateStreamingMsg(currentConvId, assistantMsgId, classifyError(errMsg), true);
          }
        } finally {
          if (unlisten) unlisten();
        }
      } else {
        // For cloud models: use governed send_chat with provider-prefixed model
        try {
          const response = await sendChat(currentInput, selectedModel);
          updateStreamingMsg(currentConvId, assistantMsgId, response.text, true);
          logAudit(`Response from ${response.model} via ${providerDisplayName(selectedModel)}`);
        } catch (err) {
          const errMsg = classifyError(String(err));
          if (errMsg.includes("API key") || errMsg.includes("unauthorized")) {
            updateStreamingMsg(currentConvId, assistantMsgId, `${errMsg}\n\nClick the model selector to add your API key.`, true);
          } else {
            updateStreamingMsg(currentConvId, assistantMsgId, errMsg, true);
          }
        }
      }
    } catch (err) {
      updateStreamingMsg(currentConvId, assistantMsgId, classifyError(String(err)), true);
    } finally {
      setSending(false);
      streamingMsgIdRef.current = null;
    }
  }, [input, sending, selectedModel, activeConvId, models, logAudit, updateStreamingMsg, sendBuildRequest]);

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

    const fetchResponse = async (_modelId: string): Promise<string> => {
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

  const handleSaveApiKey = useCallback(async () => {
    if (!showApiKeyModal || !apiKeyInput.trim()) return;
    setApiKeySaving(true);
    try {
      await saveApiKey(showApiKeyModal, apiKeyInput.trim());
      logAudit(`API key saved for ${showApiKeyModal}`);
      setShowApiKeyModal(null);
      setApiKeyInput("");
      // Refresh models and provider status
      await loadModels();
    } catch (err) {
      logAudit(`Failed to save API key: ${String(err).slice(0, 60)}`);
    } finally {
      setApiKeySaving(false);
    }
  }, [showApiKeyModal, apiKeyInput, logAudit, loadModels]);

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

        {/* model picker (grouped by provider) */}
        <div className="ch-model-section">
          <div className="ch-section-header">Active Model</div>
          <button className="ch-model-active" onClick={() => setShowModelPicker(!showModelPicker)}>
            <span className="ch-model-icon" style={{ color: activeModel?.color }}>{activeModel?.icon}</span>
            <div className="ch-model-info">
              <span className="ch-model-name">{activeModel?.name ?? "Select a model"}</span>
              <span className="ch-model-provider">{activeModel?.provider ?? "—"} · ⚡{activeModel?.fuelCost ?? 0}</span>
            </div>
            <span className="ch-model-arrow">{showModelPicker ? "▲" : "▼"}</span>
          </button>
          {showModelPicker && (
            <div className="ch-model-list">
              {/* Local Models */}
              {models.some(m => m.local) && (
                <>
                  <div className="ch-model-group-header">LOCAL MODELS (Ollama)</div>
                  {models.filter(m => m.local).map(m => (
                    <button key={m.id} className={`ch-model-option ${selectedModel === m.id ? "active" : ""}`} onClick={() => {
                      setSelectedModel(m.id);
                      localStorage.setItem("nexus-selected-model", m.id);
                      setShowModelPicker(false);
                      logAudit(`Switched to ${m.name}`);
                    }}>
                      <span className="ch-model-icon" style={{ color: m.color }}>{m.icon}</span>
                      <div className="ch-model-info">
                        <span className="ch-model-name">{m.name}</span>
                        <span className="ch-model-provider">{m.provider} · ⚡{m.fuelCost}</span>
                      </div>
                    </button>
                  ))}
                </>
              )}
              {/* Cloud Models */}
              {models.some(m => !m.local) && (
                <>
                  <div className="ch-model-group-header">CLOUD MODELS</div>
                  {models.filter(m => !m.local).map(m => (
                    <button key={m.id} className={`ch-model-option ${selectedModel === m.id ? "active" : ""} ${m.locked ? "locked" : ""}`} onClick={() => {
                      if (m.locked) {
                        setShowApiKeyModal(m.id.split("/")[0]);
                        setShowModelPicker(false);
                      } else {
                        setSelectedModel(m.id);
                        localStorage.setItem("nexus-selected-model", m.id);
                        setShowModelPicker(false);
                        logAudit(`Switched to ${m.name}`);
                      }
                    }}>
                      <span className="ch-model-icon" style={{ color: m.color }}>{m.locked ? "🔒" : m.icon}</span>
                      <div className="ch-model-info">
                        <span className="ch-model-name">{m.name}</span>
                        <span className="ch-model-provider">{m.provider} · ⚡{m.fuelCost}{m.locked ? " · Add API key" : ""}</span>
                      </div>
                    </button>
                  ))}
                </>
              )}
              {/* Add API Key button */}
              <button className="ch-model-option ch-add-key-btn" onClick={() => { setShowApiKeyModal(""); setShowModelPicker(false); }}>
                <span className="ch-model-icon" style={{ color: "var(--nexus-accent)" }}>+</span>
                <div className="ch-model-info">
                  <span className="ch-model-name">Add API Key</span>
                  <span className="ch-model-provider">Configure cloud providers</span>
                </div>
              </button>
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
              {agentEntries.length === 0 && <div style={{ padding: "0.5rem", opacity: 0.5, fontSize: "0.8rem" }}>No agents registered</div>}
              {agentEntries.map(a => (
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

            {/* conductor progress banner */}
            {conductorProgress.length > 0 && sending && (
              <div className="ch-conductor-progress">
                <div className="ch-conductor-label">⬢ Conductor Progress</div>
                {conductorProgress.map((p, i) => (
                  <div key={i} className="ch-conductor-step">{p}</div>
                ))}
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
                    {["Explain Nexus OS governance", "Write a Rust function", "Compare WASM runtimes", "Build a portfolio site with dark mode"].map(p => (
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
                         msg.buildResult ? "Conductor" :
                         models.find(m => m.id === msg.model)?.name ?? msg.model}
                      </span>
                      {msg.role === "assistant" && msg.model && (
                        <span className="ch-msg-provider-badge" style={{ color: PROVIDER_META[msg.model.split("/")[0]]?.color ?? "#888" }}>
                          via {providerDisplayName(msg.model)}
                        </span>
                      )}
                      <span className="ch-msg-time">{formatTime(msg.timestamp)}</span>
                    </div>
                    {msg.imageUrl && (
                      <div className="ch-msg-image" style={{ background: msg.imageUrl }} />
                    )}
                    {/* Build result card */}
                    {msg.buildResult ? (
                      <BuildResultCard data={msg.buildResult} />
                    ) : msg.streaming && !msg.content ? (
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
                {isBuildRequest(input) && <span className="ch-input-conductor">⬢ Conductor mode</span>}
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
        <span className="ch-status-item">{activeModel ? `via ${activeModel.provider}` : ""}</span>
        <span className="ch-status-item">{conversations.length} conversations</span>
        <span className="ch-status-item">{activeConv?.messages.length ?? 0} messages</span>
        {voiceActive && <span className="ch-status-item ch-status-voice">🎙 Jarvis Active</span>}
        {joinedAgents.length > 0 && <span className="ch-status-item">⬢ {joinedAgents.length} agents</span>}
        <span className="ch-status-item ch-status-right">⚡ {fuelUsed} fuel</span>
        <span className="ch-status-item">{models.filter(m => !m.locked).length} models</span>
      </div>

      {/* ─── API Key Modal ─── */}
      {showApiKeyModal !== null && (
        <div className="ch-modal-overlay" onClick={() => { setShowApiKeyModal(null); setApiKeyInput(""); }}>
          <div className="ch-modal" onClick={e => e.stopPropagation()}>
            <div className="ch-modal-header">
              <h3>Configure API Key</h3>
              <button className="ch-modal-close" onClick={() => { setShowApiKeyModal(null); setApiKeyInput(""); }}>×</button>
            </div>
            <div className="ch-modal-body">
              {(showApiKeyModal === "" ? ["anthropic", "openai", "deepseek", "google"] : [showApiKeyModal]).map(provider => {
                const meta = PROVIDER_META[provider];
                const statusKey = provider === "google" ? "gemini" : provider;
                const hasKey = providerStatus ? providerStatus[statusKey as keyof ProviderStatus] : false;
                return (
                  <div key={provider} className="ch-api-key-row">
                    <div className="ch-api-key-provider">
                      <span className="ch-api-key-icon" style={{ color: meta?.color }}>{meta?.icon ?? "◈"}</span>
                      <span className="ch-api-key-name">{meta?.label ?? provider}</span>
                      <span className={`ch-api-key-status ${hasKey ? "connected" : ""}`}>
                        {hasKey ? "Connected" : "Not configured"}
                      </span>
                    </div>
                    {(!hasKey || showApiKeyModal === provider) && (
                      <div className="ch-api-key-input-row">
                        <input
                          type="password"
                          className="ch-api-key-input"
                          placeholder={`Enter ${meta?.label ?? provider} API key...`}
                          value={showApiKeyModal === provider ? apiKeyInput : ""}
                          onChange={e => { setApiKeyInput(e.target.value); setShowApiKeyModal(provider); }}
                          onFocus={() => setShowApiKeyModal(provider)}
                        />
                        <button
                          className="ch-api-key-save"
                          disabled={apiKeySaving || !apiKeyInput.trim() || showApiKeyModal !== provider}
                          onClick={handleSaveApiKey}
                        >
                          {apiKeySaving ? "..." : "Save"}
                        </button>
                      </div>
                    )}
                  </div>
                );
              })}
              {/* Ollama status */}
              <div className="ch-api-key-row">
                <div className="ch-api-key-provider">
                  <span className="ch-api-key-icon" style={{ color: PROVIDER_META.ollama.color }}>{PROVIDER_META.ollama.icon}</span>
                  <span className="ch-api-key-name">Ollama (Local)</span>
                  <span className={`ch-api-key-status ${providerStatus?.ollama ? "connected" : ""}`}>
                    {providerStatus?.ollama ? "Running" : "Not detected"}
                  </span>
                </div>
                <div className="ch-api-key-hint">localhost:11434 — no API key needed</div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
