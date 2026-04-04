import { useState, useCallback, useMemo, useRef, useEffect } from "react";
import RequiresLlm from "../components/RequiresLlm";
import { Clock, Lock, Pin, MapPin, StickyNote, Trash2, Mic, MicOff, ClipboardList, Image, Zap, Hexagon, Play, Pause, Square, X, AlertTriangle, Check, FolderOpen, ChevronUp, ChevronDown, RefreshCw, Paperclip } from "lucide-react";
import {
  sendChat, chatWithOllama, conductBuild, hasDesktopRuntime,
  listProviderModels, getProviderStatus, saveApiKey, getPreinstalledAgents,
  startAgent, stopAgent, pauseAgent, resumeAgent, getAuditLog,
  approveConsentRequest, denyConsentRequest,
} from "../api/backend";
import type {
  ChatTokenEvent, ConductorPlanEvent, ConductorAgentCompletedEvent,
  ConductorFinishedEvent, ConductorBuildResponse, ProviderModel, ProviderStatus,
  PreinstalledAgent, ConsentNotification, AuditEventRow,
} from "../types";
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

interface ChatAttachment {
  name: string;
  type: string;        // MIME type
  size: number;
  dataUrl: string;     // base64 data URL for images, text content for text files
  isImage: boolean;
}

interface ChatMsg {
  id: string;
  role: "user" | "assistant" | "system" | "agent" | "approval";
  content: string;
  model?: string;
  agent?: string;
  timestamp: number;
  imageUrl?: string;
  attachments?: ChatAttachment[];
  codeBlock?: { lang: string; code: string; output?: string };
  streaming?: boolean;
  buildResult?: BuildResultData;
  approval?: ConsentNotification;
  approvalStatus?: "pending" | "approved" | "denied";
}

type AgentRunStatus = "Idle" | "Running" | "Paused" | "Stopped" | "Error";

interface SelectedAgent {
  agent_id: string;
  name: string;
  description: string;
  autonomy_level: number;
  fuel_budget: number;
  capabilities: string[];
  status: AgentRunStatus;
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
  nvidia: { icon: "◈", color: "#76b900", label: "NVIDIA NIM", fuelCost: 1 },
  openrouter: { icon: "◈", color: "#f97316", label: "OpenRouter", fuelCost: 0 },
  qwen: { icon: "◈", color: "#6366f1", label: "Qwen", fuelCost: 0 },
};

// Cloud models to show as locked when no API key is configured
const LOCKED_CLOUD_MODELS: Array<{ id: string; name: string; provider: string }> = [
  // Anthropic
  { id: "anthropic/claude-sonnet-4-6", name: "Claude Sonnet 4.6", provider: "anthropic" },
  { id: "anthropic/claude-sonnet-4-20250514", name: "Claude Sonnet 4", provider: "anthropic" },
  { id: "anthropic/claude-haiku-4-5", name: "Claude Haiku 4.5", provider: "anthropic" },
  { id: "anthropic/claude-opus-4-6", name: "Claude Opus 4.6", provider: "anthropic" },
  // OpenAI
  { id: "openai/gpt-4.1-nano", name: "GPT-4.1 Nano", provider: "openai" },
  { id: "openai/gpt-4.1-mini", name: "GPT-4.1 Mini", provider: "openai" },
  { id: "openai/gpt-4.1", name: "GPT-4.1", provider: "openai" },
  { id: "openai/gpt-5-mini", name: "GPT-5 Mini", provider: "openai" },
  { id: "openai/gpt-5", name: "GPT-5", provider: "openai" },
  { id: "openai/gpt-4o", name: "GPT-4o", provider: "openai" },
  { id: "openai/gpt-4o-mini", name: "GPT-4o Mini", provider: "openai" },
  // DeepSeek
  { id: "deepseek/deepseek-chat", name: "DeepSeek Chat", provider: "deepseek" },
  { id: "deepseek/deepseek-coder", name: "DeepSeek Coder", provider: "deepseek" },
  // Google
  { id: "google/gemini-2.5-pro", name: "Gemini 2.5 Pro", provider: "google" },
  { id: "google/gemini-2.5-flash", name: "Gemini 2.5 Flash", provider: "google" },
  // OpenRouter (free tier)
  { id: "openrouter/qwen/qwen3.6-plus:free", name: "Qwen 3.6 Plus (Free, 1M ctx)", provider: "openrouter" },
  { id: "openrouter/meta-llama/llama-3.3-70b-instruct:free", name: "Llama 3.3 70B (Free)", provider: "openrouter" },
  // NVIDIA NIM (free tier)
  { id: "nvidia/deepseek-ai/deepseek-v3.1-terminus", name: "DeepSeek V3.1 Terminus 671B", provider: "nvidia" },
  { id: "nvidia/meta/llama-3.3-70b-instruct", name: "Llama 3.3 70B (NIM)", provider: "nvidia" },
  { id: "nvidia/qwen/qwen3.5-397b-a17b", name: "Qwen 3.5 397B", provider: "nvidia" },
  { id: "nvidia/z-ai/glm4.7", name: "GLM-4.7 Agentic Coding", provider: "nvidia" },
];

const AUTONOMY_LABELS: Record<number, string> = {
  0: "L0 · Inert",
  1: "L1 · Suggest",
  2: "L2 · Act-with-approval",
  3: "L3 · Act-then-report",
  4: "L4 · Autonomous-bounded",
  5: "L5 · Full autonomy",
  6: "L6 · Transcendent",
};

function autonomyShort(level: number): string {
  return `L${level}`;
}

function autonomyColor(level: number): string {
  const colors: Record<number, string> = {
    0: "#64748b", 1: "#22c55e", 2: "#60a5fa",
    3: "#f59e0b", 4: "#fb923c", 5: "#ef4444", 6: "#a855f7",
  };
  return colors[level] ?? "#64748b";
}

function normalizeAgentRunStatus(status: string): AgentRunStatus {
  if (status === "Running" || status === "Starting") {
    return "Running";
  }
  if (status === "Paused") {
    return "Paused";
  }
  if (status === "Error") {
    return "Error";
  }
  return "Idle";
}

function isBuildRequest(msg: string): boolean {
  // Only route to Conductor (file-generating build) when the message starts with
  // "/build" prefix. All other project-like requests go through the backend
  // complexity detection pipeline which handles autopilot + agent routing.
  const lower = msg.toLowerCase().trim();
  if (lower.startsWith("/build ") || lower.startsWith("/conductor ")) {
    return true;
  }
  return false;
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

function createWelcomeMessage(model?: string): ChatMsg {
  return {
    id: `welcome-${Date.now()}`,
    role: "assistant",
    content:
      "AI Chat Hub ready. Select a model and start chatting. This page provides direct LLM access without agent governance.",
    model,
    timestamp: Date.now(),
  };
}

function createConversation(model: string): Conversation {
  const now = Date.now();
  return {
    id: `conv-${now}`,
    title: "New conversation",
    model,
    messages: [createWelcomeMessage(model)],
    createdAt: now,
    updatedAt: now,
    pinned: false,
    tags: [],
  };
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
        <span className="ch-build-icon"><Hexagon size={16} aria-hidden="true" /></span>
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
              <span className="ch-build-task-check"><Check size={12} aria-hidden="true" /></span>
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
        <span className="ch-build-stat"><Hexagon size={12} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} /> {result.agents_used} agents</span>
        <span className="ch-build-stat"><Zap size={12} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} /> {result.total_fuel_used} fuel</span>
        <span className="ch-build-stat"><Clock size={12} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} /> {result.duration_secs.toFixed(1)}s</span>
      </div>

      {/* Summary */}
      {result.summary && (
        <div className="ch-build-summary">{result.summary}</div>
      )}

      {/* Actions */}
      <div className="ch-build-actions">
        {hasIndex && (
          <button className="ch-build-btn ch-build-btn-primary cursor-pointer" onClick={openPreview}>
            <Play size={12} aria-hidden="true" /> Preview
          </button>
        )}
        <button className="ch-build-btn cursor-pointer" onClick={openFileManager}>
          <FolderOpen size={12} aria-hidden="true" /> View Files
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
      if (saved) {
        const parsed = JSON.parse(saved) as Conversation[];
        if (!Array.isArray(parsed)) return [];
        return parsed
          .filter((c) => c && typeof c.id === "string" && Array.isArray(c.messages))
          .map((conversation) => ({
            ...conversation,
            // Ensure every message has a string content field
            messages: (conversation.messages.length === 0
              ? [createWelcomeMessage(conversation.model)]
              : conversation.messages
            ).map((m) => ({ ...m, content: m.content ?? "" })),
            updatedAt: conversation.updatedAt ?? Date.now(),
          }));
      }
    } catch {
      // Corrupt data — wipe it to prevent repeated crashes
      try { localStorage.removeItem("nexus-chat-conversations"); } catch { /* ignore */ }
    }
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
  const [pendingFiles, setPendingFiles] = useState<ChatAttachment[]>([]);
  const fileInputRef = useRef<HTMLInputElement>(null);
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
  const [joinedAgents, setJoinedAgents] = useState<string[]>([]);
  const [auditLog, setAuditLog] = useState<string[]>(["Chat hub ready"]);
  const [conductorProgress, setConductorProgress] = useState<string[]>([]);
  // Agent control state
  const [preinstalledAgents, setPreinstalledAgents] = useState<PreinstalledAgent[]>([]);
  const [selectedAgent, setSelectedAgent] = useState<SelectedAgent | null>(null);
  const [agentStatus, setAgentStatus] = useState<AgentRunStatus>("Idle");
  const [showAgentDropdown, setShowAgentDropdown] = useState(false);
  const [showAgentLogs, setShowAgentLogs] = useState(false);
  const [agentLogs, setAgentLogs] = useState<AuditEventRow[]>([]);
  const [agentActionLoading, setAgentActionLoading] = useState(false);
  const [showAgentInfo, setShowAgentInfo] = useState(false);

  // compare state
  const [compareModels, setCompareModels] = useState<[string, string]>(["", ""]);
  const [comparePrompt, setComparePrompt] = useState("");
  const [compareResults, setCompareResults] = useState<[string, string]>(["", ""]);
  const [comparing, setComparing] = useState(false);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messagesContainerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const streamingMsgIdRef = useRef<string | null>(null);

  const activeConv = useMemo(() => conversations.find(c => c.id === activeConvId), [conversations, activeConvId]);
  const activeModel = useMemo(() => models.find(m => m.id === selectedModel), [models, selectedModel]);

  const logAudit = useCallback((msg: string) => setAuditLog(prev => [msg, ...prev].slice(0, 30)), []);

  useEffect(() => {
    // Scroll the messages container to the bottom. Using scrollTop on the
    // container itself is more reliable than scrollIntoView when the
    // container uses flexbox with a ::before spacer.
    const el = messagesContainerRef.current;
    if (el) {
      requestAnimationFrame(() => {
        el.scrollTop = el.scrollHeight;
      });
    }
  }, [activeConv?.messages.length, activeConv?.messages[activeConv?.messages.length - 1]?.content, activeConvId]);

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
            icon: "locked",
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

  /* ─── refresh models when ModelHub downloads a new model ─── */
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const eventMod = await import("@tauri-apps/api/event");
        unlisten = await eventMod.listen("model-downloaded", () => {
          loadModels();
          logAudit("New model downloaded — refreshing model list");
        });
      } catch { /* not in desktop runtime */ }
    })();
    return () => { unlisten?.(); };
  }, [loadModels, logAudit]);

  /* ─── load prebuilt agents from backend ─── */
  const loadPreinstalledAgents = useCallback(async () => {
    if (!hasDesktopRuntime()) return;
    try {
      const agents = await getPreinstalledAgents();
      setPreinstalledAgents(agents);
      // Update selected agent status if one is selected
      if (selectedAgent) {
        const updated = agents.find(a => a.agent_id === selectedAgent.agent_id);
        if (updated) {
          const status = normalizeAgentRunStatus(updated.status);
          setAgentStatus(status);
          setSelectedAgent(prev => prev ? { ...prev, status } : null);
        }
      }
    } catch { /* backend unavailable */ }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedAgent?.agent_id]);

  useEffect(() => {
    loadPreinstalledAgents();
  }, [loadPreinstalledAgents]);

  // Refresh agent status periodically when an agent is selected and running
  useEffect(() => {
    if (!selectedAgent || agentStatus === "Idle" || agentStatus === "Stopped") return;
    const interval = setInterval(loadPreinstalledAgents, 5000);
    return () => clearInterval(interval);
  }, [selectedAgent, agentStatus, loadPreinstalledAgents]);

  /* ─── listen for HITL consent-request-pending events ─── */
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const eventMod = await import("@tauri-apps/api/event");
        unlisten = await eventMod.listen<ConsentNotification>("consent-request-pending", (event) => {
          const notification = event.payload;
          // Only show if it's for the currently selected agent (or show all if none selected)
          const approvalMsg: ChatMsg = {
            id: `approval-${notification.consent_id}`,
            role: "approval",
            content: notification.operation_summary,
            agent: notification.agent_name,
            timestamp: Date.now(),
            approval: notification,
            approvalStatus: "pending",
          };
          // Add to active conversation
          setConversations(prev => prev.map(c => c.id === activeConvId ? {
            ...c,
            messages: [...c.messages, approvalMsg],
            updatedAt: Date.now(),
          } : c));
          logAudit(`HITL: ${notification.agent_name} requests approval`);
        });
      } catch { /* not in Tauri */ }
    })();
    return () => { unlisten?.(); };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeConvId]);

  /* ─── listen for auto-evolution events ─── */
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const eventMod = await import("@tauri-apps/api/event");
        unlisten = await eventMod.listen<{ agent_id: string; old_score: number; new_score: number }>(
          "agent-evolved",
          (event) => {
            const { agent_id, old_score, new_score } = event.payload;
            const evoMsg: ChatMsg = {
              id: `evo-${Date.now()}`,
              role: "system",
              content: `Agent ${agent_id} self-improved! Score: ${old_score.toFixed(1)} → ${new_score.toFixed(1)}`,
              timestamp: Date.now(),
            };
            setConversations(prev => prev.map(c => c.id === activeConvId ? {
              ...c,
              messages: [...c.messages, evoMsg],
              updatedAt: Date.now(),
            } : c));
            logAudit(`Auto-evolution: ${agent_id} improved ${old_score.toFixed(1)} → ${new_score.toFixed(1)}`);
          },
        );
      } catch { /* not in Tauri */ }
    })();
    return () => { unlisten?.(); };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeConvId]);

  /* ─── agent lifecycle actions ─── */
  const handleAgentAction = useCallback(async (action: "start" | "pause" | "resume" | "stop" | "kill") => {
    if (!selectedAgent) return;
    setAgentActionLoading(true);
    try {
      switch (action) {
        case "start":
          await startAgent(selectedAgent.agent_id);
          setAgentStatus("Running");
          logAudit(`Started ${selectedAgent.name}`);
          break;
        case "pause":
          await pauseAgent(selectedAgent.agent_id);
          setAgentStatus("Paused");
          logAudit(`Paused ${selectedAgent.name}`);
          break;
        case "resume":
          await resumeAgent(selectedAgent.agent_id);
          setAgentStatus("Running");
          logAudit(`Resumed ${selectedAgent.name}`);
          break;
        case "stop":
          await stopAgent(selectedAgent.agent_id);
          setAgentStatus("Stopped");
          logAudit(`Stopped ${selectedAgent.name}`);
          break;
        case "kill":
          await stopAgent(selectedAgent.agent_id); // kill uses stop (no separate kill command)
          setAgentStatus("Stopped");
          logAudit(`Force stopped ${selectedAgent.name}`);
          break;
      }
      await loadPreinstalledAgents();
    } catch (err) {
      logAudit(`Agent ${action} failed: ${String(err).slice(0, 60)}`);
      setAgentStatus("Error");
    } finally {
      setAgentActionLoading(false);
    }
  }, [selectedAgent, logAudit, loadPreinstalledAgents]);

  const handleLoadAgentLogs = useCallback(async () => {
    if (!selectedAgent) return;
    try {
      const logs = await getAuditLog(selectedAgent.agent_id, 50);
      setAgentLogs(logs);
    } catch {
      setAgentLogs([]);
    }
  }, [selectedAgent]);

  const toggleAgentLogs = useCallback(() => {
    const next = !showAgentLogs;
    setShowAgentLogs(next);
    if (next) handleLoadAgentLogs();
  }, [showAgentLogs, handleLoadAgentLogs]);

  const selectAgent = useCallback((agent: PreinstalledAgent) => {
    const status = normalizeAgentRunStatus(agent.status);
    setSelectedAgent({
      agent_id: agent.agent_id,
      name: agent.name,
      description: agent.description,
      autonomy_level: agent.autonomy_level,
      fuel_budget: agent.fuel_budget,
      capabilities: agent.capabilities,
      status,
    });
    setAgentStatus(status);
    setShowAgentDropdown(false);
    setShowAgentLogs(false);
    setAgentLogs([]);
    logAudit(`Selected agent: ${agent.name} (${autonomyShort(agent.autonomy_level)})`);
  }, [logAudit]);

  // Auto-select agent from Agents page navigation (via sessionStorage)
  useEffect(() => {
    const agentId = sessionStorage.getItem("nexus-chat-agent");
    if (!agentId || preinstalledAgents.length === 0) return;
    sessionStorage.removeItem("nexus-chat-agent");
    const found = preinstalledAgents.find(a => a.agent_id === agentId);
    if (found) selectAgent(found);
  }, [preinstalledAgents, selectAgent]);

  const handleApproval = useCallback(async (consentId: string, action: "approve" | "deny") => {
    try {
      if (action === "approve") {
        await approveConsentRequest(consentId, "user");
      } else {
        await denyConsentRequest(consentId, "user", "User denied from chat");
      }
      // Update the message status
      setConversations(prev => prev.map(c => ({
        ...c,
        messages: c.messages.map(m =>
          m.approval?.consent_id === consentId
            ? { ...m, approvalStatus: action === "approve" ? "approved" as const : "denied" as const }
            : m
        ),
      })));
      logAudit(`HITL: ${action === "approve" ? "Approved" : "Denied"} consent ${consentId.slice(0, 8)}`);
    } catch (err) {
      logAudit(`HITL ${action} failed: ${String(err).slice(0, 60)}`);
    }
  }, [logAudit]);

  // Group preinstalled agents by autonomy level for the dropdown
  const agentsByLevel = useMemo(() => {
    const grouped: Record<number, PreinstalledAgent[]> = {};
    for (const agent of preinstalledAgents) {
      const level = agent.autonomy_level;
      if (!grouped[level]) grouped[level] = [];
      grouped[level].push(agent);
    }
    return Object.entries(grouped)
      .sort(([a], [b]) => Number(a) - Number(b))
      .map(([level, agents]) => ({
        level: Number(level),
        agents: agents.sort((a, b) => a.name.localeCompare(b.name)),
      }));
  }, [preinstalledAgents]);

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
      const conv = createConversation(selectedModel);
      setConversations([conv]);
      setActiveConvId(conv.id);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /* ─── event delegation for code-block Run buttons (rendered via dangerouslySetInnerHTML) ─── */
  useEffect(() => {
    const container = document.querySelector('.ch-messages') || document.body;
    const handleClick = (e: Event) => {
      const target = e.target as HTMLElement;
      if (target.classList.contains('ch-code-run')) {
        const codeBlock = target.closest('.ch-code-block');
        const codeEl = codeBlock?.querySelector('code');
        if (codeEl?.textContent) {
          navigator.clipboard.writeText(codeEl.textContent)
            .then(() => { target.textContent = 'Copied!'; setTimeout(() => { target.textContent = '\u25B6 Run'; }, 2000); })
            .catch((e) => { if (import.meta.env.DEV) console.warn("[AiChatHub]", e); });
        }
      }
    };
    container.addEventListener('click', handleClick);
    return () => container.removeEventListener('click', handleClick);
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
    if (!content) return "";
    return content.replace(/```(\w+)?\n([\s\S]*?)```/g, (_match, lang: string, code: string) => {
      const l = lang || "text";
      return `<div class="ch-code-block"><div class="ch-code-header"><span>${l}</span><button class="ch-code-run" data-code="${encodeURIComponent(code.trim())}">▶ Run</button></div><pre class="ch-code-pre"><code>${code.replace(/</g, "&lt;").replace(/>/g, "&gt;")}</code></pre></div>`;
    });
  };

  const renderContent = (content: string) => {
    if (!content) return "";
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
      messages: c.messages.map(m => m.id === msgId ? { ...m, content: content ?? "", streaming: !done } : m),
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
    updateStreamingMsg(currentConvId, assistantMsgId, "Conductor orchestrating build...", false);

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

  /* ─── file attachment helpers ─── */
  const processFiles = useCallback((files: FileList | File[]) => {
    const MAX_FILE_SIZE = 10 * 1024 * 1024; // 10MB
    const fileArray = Array.from(files);
    fileArray.forEach(file => {
      if (file.size > MAX_FILE_SIZE) return;
      const isImage = file.type.startsWith("image/");
      const reader = new FileReader();
      reader.onload = () => {
        const dataUrl = reader.result as string;
        setPendingFiles(prev => {
          if (prev.some(f => f.name === file.name && f.size === file.size)) return prev;
          return [...prev, { name: file.name, type: file.type, size: file.size, dataUrl, isImage }];
        });
      };
      if (isImage) {
        reader.readAsDataURL(file);
      } else {
        reader.readAsText(file);
      }
    });
  }, []);

  const removeFile = useCallback((name: string) => {
    setPendingFiles(prev => prev.filter(f => f.name !== name));
  }, []);

  const handleFileDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.dataTransfer.files.length > 0) processFiles(e.dataTransfer.files);
  }, [processFiles]);

  const handleFilePaste = useCallback((e: React.ClipboardEvent) => {
    const items = e.clipboardData?.items;
    if (!items) return;
    const files: File[] = [];
    for (let i = 0; i < items.length; i++) {
      const file = items[i].getAsFile();
      if (file) files.push(file);
    }
    if (files.length > 0) processFiles(files);
  }, [processFiles]);

  /* ─── send message (real backend, multi-provider) ─── */
  const sendMessage = useCallback(async () => {
    if ((!input.trim() && pendingFiles.length === 0) || sending) return;
    const model = models.find(m => m.id === selectedModel);
    if (model?.locked) {
      setShowApiKeyModal(selectedModel.split("/")[0]);
      return;
    }
    // Build message content with file context
    const attachedFiles = [...pendingFiles];
    let messageContent = input;
    const textFiles = attachedFiles.filter(f => !f.isImage);
    if (textFiles.length > 0) {
      const fileContext = textFiles.map(f => `--- ${f.name} ---\n${f.dataUrl}`).join("\n\n");
      messageContent = messageContent ? `${messageContent}\n\n${fileContext}` : fileContext;
    }

    const userMsg: ChatMsg = {
      id: `m-${Date.now()}`, role: "user", content: input || `[Attached ${attachedFiles.length} file${attachedFiles.length > 1 ? "s" : ""}]`,
      timestamp: Date.now(),
      attachments: attachedFiles.length > 0 ? attachedFiles : undefined,
    };
    const currentInput = messageContent;
    const currentConvId = activeConvId;
    const isOllamaModel = selectedModel.startsWith("ollama/");
    const ollamaModelName = isOllamaModel ? selectedModel.slice("ollama/".length) : selectedModel;

    setConversations(prev => prev.map(c => c.id === currentConvId ? {
      ...c, messages: [...c.messages, userMsg], updatedAt: Date.now(),
      title: c.messages.length === 0 ? (input || attachedFiles[0]?.name || "File upload").slice(0, 50) : c.title,
    } : c));

    setInput("");
    setPendingFiles([]);
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

          const imageAttachments = attachedFiles.filter(f => f.isImage);
          const ollamaMsg: Record<string, unknown> = { role: "user", content: currentInput };
          if (imageAttachments.length > 0) {
            ollamaMsg.images = imageAttachments.map(f => f.dataUrl.replace(/^data:image\/\w+;base64,/, ""));
          }
          const messages = [ollamaMsg as { role: string; content: string }];
          await chatWithOllama(messages, ollamaModelName);
        } catch (streamErr) {
          const errMsg = String(streamErr);
          if (errMsg.includes("__TAURI__") || errMsg.includes("invoke")) {
            // Not in Tauri runtime — fallback to send_chat
            try {
              const response = await sendChat(currentInput, selectedModel, selectedAgent?.name);
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
          const response = await sendChat(currentInput, selectedModel, selectedAgent?.name);
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
  }, [input, sending, selectedModel, activeConvId, models, logAudit, updateStreamingMsg, sendBuildRequest, pendingFiles]);

  const newConversation = useCallback(() => {
    const conv = createConversation(selectedModel);
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
    <RequiresLlm feature="Chat">
    <div className="ch-container">
      {/* ─── Sidebar ─── */}
      <aside className="ch-sidebar">
        <div className="ch-sidebar-header">
          <h2 className="ch-sidebar-title">AI Chat Hub</h2>
          <button className="ch-new-btn" onClick={newConversation}>+ New</button>
        </div>

        {/* views */}
        <div className="ch-views">
          {([["chat", "chat", "Chat"], ["compare", "compare", "Compare"], ["history", "history", "History"]] as const).map(([id, icon, label]) => (
            <button key={id} className={`ch-view-btn cursor-pointer ${view === id ? "active" : ""}`} onClick={() => setView(id)}>
              <span>{icon === "chat" ? "\u2301" : icon === "compare" ? "\u21D4" : <Clock size={14} aria-hidden="true" />}</span> {label}
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
              <span className="ch-model-provider">{activeModel?.provider ?? "—"} · <Zap size={10} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} />{activeModel?.fuelCost ?? 0}</span>
            </div>
            <span className="ch-model-arrow">{showModelPicker ? <ChevronUp size={12} aria-hidden="true" /> : <ChevronDown size={12} aria-hidden="true" />}</span>
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
                        <span className="ch-model-provider">{m.provider} · <Zap size={10} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} />{m.fuelCost}</span>
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
                      <span className="ch-model-icon" style={{ color: m.color }}>{m.locked ? <Lock size={14} aria-hidden="true" /> : m.icon}</span>
                      <div className="ch-model-info">
                        <span className="ch-model-name">{m.name}</span>
                        <span className="ch-model-provider">{m.provider} · <Zap size={10} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} />{m.fuelCost}{m.locked ? " · Add API key" : ""}</span>
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
                {conv.pinned && <span className="ch-pin"><Pin size={12} aria-hidden="true" /></span>}
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

        {/* agent panel — all prebuilt agents */}
        <div className="ch-agent-section">
          <button className="ch-section-header ch-agent-toggle" onClick={() => setShowAgentPanel(!showAgentPanel)}>
            All Agents ({preinstalledAgents.length}) {showAgentPanel ? <ChevronUp size={12} aria-hidden="true" /> : <ChevronDown size={12} aria-hidden="true" />}
          </button>
          {showAgentPanel && (
            <div className="ch-agent-list">
              {preinstalledAgents.length === 0 && <div style={{ padding: "0.5rem", opacity: 0.5, fontSize: "0.8rem" }}>No agents loaded</div>}
              {agentsByLevel.map(group => (
                <div key={group.level}>
                  <div className="ch-agent-group-header" style={{ color: autonomyColor(group.level) }}>
                    {autonomyShort(group.level)} — {AUTONOMY_LABELS[group.level] ?? `Level ${group.level}`} ({group.agents.length})
                  </div>
                  {group.agents.map(a => (
                    <button
                      key={a.agent_id}
                      className={`ch-agent-btn ${selectedAgent?.agent_id === a.agent_id ? "active" : ""}`}
                      onClick={() => selectAgent(a)}
                    >
                      <span className="ch-agent-level-dot" style={{ background: autonomyColor(a.autonomy_level) }} />
                      <span className="ch-agent-btn-name">{a.name}</span>
                      <span className="ch-agent-btn-level" style={{ color: autonomyColor(a.autonomy_level) }}>{autonomyShort(a.autonomy_level)}</span>
                      {a.status === "Running" && <span className="ch-agent-running-dot" />}
                    </button>
                  ))}
                </div>
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
              <div className="ch-chat-header-left">
                {selectedAgent ? (
                  <div className="ch-agent-header-badge">
                    <span className="ch-agent-header-dot" style={{ background: autonomyColor(selectedAgent.autonomy_level) }} />
                    <span className="ch-agent-header-name">{selectedAgent.name}</span>
                    <span className="ch-agent-header-level" style={{ color: autonomyColor(selectedAgent.autonomy_level) }}>
                      {autonomyShort(selectedAgent.autonomy_level)}
                    </span>
                    <button className="ch-agent-info-btn" onClick={() => setShowAgentInfo(!showAgentInfo)} title="Agent info">ⓘ</button>
                    <button className="ch-agent-deselect-btn cursor-pointer" onClick={() => { setSelectedAgent(null); setAgentStatus("Idle"); setShowAgentLogs(false); }} title="Deselect agent"><X size={12} aria-hidden="true" /></button>
                  </div>
                ) : (
                  <div className="ch-chat-title">{activeConv.title}</div>
                )}
                {/* Agent dropdown */}
                <div className="ch-agent-dropdown-wrap">
                  <button className="ch-agent-dropdown-btn cursor-pointer" onClick={() => setShowAgentDropdown(!showAgentDropdown)}>
                    <Hexagon size={12} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} />
                    {selectedAgent
                      ? `${selectedAgent.name} (${autonomyShort(selectedAgent.autonomy_level)})`
                      : "All Agents"} <ChevronDown size={10} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} />
                  </button>
                  {showAgentDropdown && (
                    <div className="ch-agent-dropdown">
                      <div className="ch-agent-dropdown-header">Select Agent ({preinstalledAgents.length})</div>
                      <button className="ch-agent-dropdown-item" onClick={() => { setSelectedAgent(null); setAgentStatus("Idle"); setShowAgentDropdown(false); }}>
                        <span>—</span> No agent (direct LLM)
                      </button>
                      {agentsByLevel.map(group => (
                        <div key={group.level}>
                          <div className="ch-agent-dropdown-group" style={{ color: autonomyColor(group.level) }}>
                            {AUTONOMY_LABELS[group.level] ?? `Level ${group.level}`}
                          </div>
                          {group.agents.map(a => (
                            <button
                              key={a.agent_id}
                              className={`ch-agent-dropdown-item ${selectedAgent?.agent_id === a.agent_id ? "active" : ""}`}
                              onClick={() => selectAgent(a)}
                            >
                              <span className="ch-agent-level-dot" style={{ background: autonomyColor(a.autonomy_level) }} />
                              {a.name} ({autonomyShort(a.autonomy_level)})
                            </button>
                          ))}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </div>
              <div className="ch-chat-actions">
                <button className="ch-hdr-btn cursor-pointer" onClick={() => togglePin(activeConv.id)} title="Pin">{activeConv.pinned ? <Pin size={16} aria-hidden="true" /> : <MapPin size={16} aria-hidden="true" />}</button>
                <button className="ch-hdr-btn cursor-pointer" onClick={saveAsNote} title="Save as note"><StickyNote size={16} aria-hidden="true" /></button>
                <button className="ch-hdr-btn cursor-pointer" onClick={() => deleteConversation(activeConv.id)} title="Delete"><Trash2 size={16} aria-hidden="true" /></button>
                <button className={`ch-hdr-btn ch-voice-btn cursor-pointer ${voiceActive ? "active" : ""}`} onClick={() => { setVoiceActive(!voiceActive); logAudit(voiceActive ? "Voice off" : "Voice on — Jarvis mode"); }} title="Voice">
                  {voiceActive ? <Mic size={16} aria-hidden="true" /> : <MicOff size={16} aria-hidden="true" />}
                </button>
              </div>
            </div>

            {/* Agent info panel */}
            {showAgentInfo && selectedAgent && (
              <div className="ch-agent-info-panel">
                <div className="ch-agent-info-row">
                  <span className="ch-agent-info-label">Description</span>
                  <span className="ch-agent-info-value">{selectedAgent.description.slice(0, 200)}{selectedAgent.description.length > 200 ? "..." : ""}</span>
                </div>
                <div className="ch-agent-info-row">
                  <span className="ch-agent-info-label">Autonomy</span>
                  <span className="ch-agent-info-value" style={{ color: autonomyColor(selectedAgent.autonomy_level) }}>
                    {AUTONOMY_LABELS[selectedAgent.autonomy_level]}
                  </span>
                </div>
                <div className="ch-agent-info-row">
                  <span className="ch-agent-info-label">Fuel Budget</span>
                  <span className="ch-agent-info-value">{selectedAgent.fuel_budget.toLocaleString()}</span>
                </div>
                <div className="ch-agent-info-row">
                  <span className="ch-agent-info-label">Capabilities</span>
                  <span className="ch-agent-info-value">{selectedAgent.capabilities.join(", ")}</span>
                </div>
              </div>
            )}

            {/* Agent control bar */}
            {selectedAgent && (
              <div className="ch-agent-controls">
                <div className="ch-agent-status">
                  <span className={`ch-agent-status-dot ch-agent-status-${agentStatus.toLowerCase()}`} />
                  <span className="ch-agent-status-text">{agentStatus}</span>
                </div>
                <div className="ch-agent-btns">
                  {(agentStatus === "Idle" || agentStatus === "Stopped" || agentStatus === "Error") && (
                    <button className="ch-ctrl-btn ch-ctrl-start cursor-pointer" onClick={() => handleAgentAction("start")} disabled={agentActionLoading}>
                      <Play size={12} aria-hidden="true" /> Start
                    </button>
                  )}
                  {agentStatus === "Running" && (
                    <button className="ch-ctrl-btn ch-ctrl-pause cursor-pointer" onClick={() => handleAgentAction("pause")} disabled={agentActionLoading}>
                      <Pause size={12} aria-hidden="true" /> Pause
                    </button>
                  )}
                  {agentStatus === "Paused" && (
                    <button className="ch-ctrl-btn ch-ctrl-resume cursor-pointer" onClick={() => handleAgentAction("resume")} disabled={agentActionLoading}>
                      <Play size={12} aria-hidden="true" /> Resume
                    </button>
                  )}
                  {(agentStatus === "Running" || agentStatus === "Paused") && (
                    <>
                      <button className="ch-ctrl-btn ch-ctrl-stop cursor-pointer" onClick={() => handleAgentAction("stop")} disabled={agentActionLoading}>
                        <Square size={12} aria-hidden="true" /> Stop
                      </button>
                      <button
                        className="ch-ctrl-btn ch-ctrl-kill cursor-pointer"
                        onClick={() => { if (window.confirm(`Force kill ${selectedAgent.name}?`)) handleAgentAction("kill"); }}
                        disabled={agentActionLoading}
                      >
                        <X size={12} aria-hidden="true" /> Kill
                      </button>
                    </>
                  )}
                  <button className={`ch-ctrl-btn ch-ctrl-logs cursor-pointer ${showAgentLogs ? "active" : ""}`} onClick={toggleAgentLogs}>
                    <ClipboardList size={12} aria-hidden="true" /> Logs
                  </button>
                </div>
              </div>
            )}

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
                <div className="ch-conductor-label"><Hexagon size={12} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} /> Conductor Progress</div>
                {conductorProgress.map((p, i) => (
                  <div key={i} className="ch-conductor-step">{p}</div>
                ))}
              </div>
            )}

            {/* agent logs panel */}
            {showAgentLogs && selectedAgent && (
              <div className="ch-agent-logs">
                <div className="ch-agent-logs-header">
                  <span>Agent Logs — {selectedAgent.name}</span>
                  <button className="ch-agent-logs-refresh cursor-pointer" onClick={handleLoadAgentLogs}><RefreshCw size={12} aria-hidden="true" /> Refresh</button>
                </div>
                <div className="ch-agent-logs-body">
                  {agentLogs.length === 0 && <div className="ch-agent-logs-empty">No log entries</div>}
                  {agentLogs.map((log, i) => (
                    <div key={i} className="ch-agent-log-entry">
                      <span className="ch-agent-log-time">{new Date(log.timestamp).toLocaleTimeString()}</span>
                      <span className="ch-agent-log-action">{log.event_type}</span>
                      <span className="ch-agent-log-detail">{JSON.stringify(log.payload).slice(0, 120)}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* messages */}
            <div className="ch-messages" ref={messagesContainerRef}>
              {models.length === 0 && (
                <div className="ch-empty-chat">
                  <div className="ch-empty-icon">◈</div>
                  <div className="ch-empty-model">No models available</div>
                  <div className="ch-empty-hint">
                    AI Chat Hub needs at least one configured model. Start Ollama or add a provider API key in Settings to begin.
                  </div>
                </div>
              )}
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
                     msg.role === "approval" ? <AlertTriangle size={16} aria-hidden="true" /> :
                     msg.role === "agent" ? <Hexagon size={16} aria-hidden="true" /> :
                     <span style={{ color: models.find(m => m.id === msg.model)?.color }}>{models.find(m => m.id === msg.model)?.icon ?? "\u25C8"}</span>}
                  </div>
                  <div className="ch-msg-body">
                    <div className="ch-msg-header">
                      <span className="ch-msg-name">
                        {msg.role === "user" ? "You" :
                         msg.role === "approval" ? `HITL Approval — ${msg.agent}` :
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
                    {msg.attachments && msg.attachments.length > 0 && (
                      <div className="ch-msg-attachments">
                        {msg.attachments.filter(a => a.isImage).map(a => (
                          <img key={a.name} src={a.dataUrl} alt={a.name} className="ch-msg-attach-img" title={a.name} />
                        ))}
                        {msg.attachments.filter(a => !a.isImage).map(a => (
                          <div key={a.name} className="ch-msg-attach-file">
                            <span>📄 {a.name}</span>
                            <span className="ch-msg-attach-size">{(a.size / 1024).toFixed(1)} KB</span>
                          </div>
                        ))}
                      </div>
                    )}
                    {/* HITL Approval card */}
                    {msg.approval ? (
                      <div className={`ch-approval-card ch-approval-${msg.approvalStatus}`}>
                        <div className="ch-approval-header">
                          <span className="ch-approval-icon"><AlertTriangle size={14} aria-hidden="true" /></span>
                          <span className="ch-approval-title">Human Approval Required</span>
                          <span className={`ch-approval-risk ch-risk-${msg.approval.risk_level.toLowerCase()}`}>
                            {msg.approval.risk_level}
                          </span>
                        </div>
                        <div className="ch-approval-body">
                          <div className="ch-approval-row">
                            <span className="ch-approval-label">Action</span>
                            <span className="ch-approval-value">{msg.approval.operation_type}: {msg.approval.operation_summary}</span>
                          </div>
                          <div className="ch-approval-row">
                            <span className="ch-approval-label">Agent</span>
                            <span className="ch-approval-value">{msg.approval.agent_name} ({msg.approval.agent_id.slice(0, 8)})</span>
                          </div>
                          <div className="ch-approval-row">
                            <span className="ch-approval-label">Fuel Cost</span>
                            <span className="ch-approval-value"><Zap size={12} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} /> {msg.approval.fuel_cost_estimate}</span>
                          </div>
                          {msg.approval.side_effects_preview.length > 0 && (
                            <div className="ch-approval-row">
                              <span className="ch-approval-label">Side Effects</span>
                              <span className="ch-approval-value">{msg.approval.side_effects_preview.join(", ")}</span>
                            </div>
                          )}
                          {msg.approval.auto_deny_at && (
                            <div className="ch-approval-row">
                              <span className="ch-approval-label">Timeout</span>
                              <span className="ch-approval-value ch-approval-timeout">{msg.approval.auto_deny_at}</span>
                            </div>
                          )}
                        </div>
                        {msg.approvalStatus === "pending" ? (
                          <div className="ch-approval-actions">
                            <button className="ch-approval-btn ch-approval-approve cursor-pointer" onClick={() => handleApproval(msg.approval!.consent_id, "approve")}>
                              <Check size={12} aria-hidden="true" /> Approve
                            </button>
                            <button className="ch-approval-btn ch-approval-deny cursor-pointer" onClick={() => handleApproval(msg.approval!.consent_id, "deny")}>
                              <X size={12} aria-hidden="true" /> Reject
                            </button>
                          </div>
                        ) : (
                          <div className={`ch-approval-resolved ch-approval-resolved-${msg.approvalStatus}`}>
                            {msg.approvalStatus === "approved" ? <><Check size={12} aria-hidden="true" /> Approved</> : <><X size={12} aria-hidden="true" /> Denied</>}
                          </div>
                        )}
                      </div>
                    ) : msg.buildResult ? (
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
            <div
              className="ch-input-bar"
              onDragOver={e => { e.preventDefault(); e.stopPropagation(); }}
              onDrop={handleFileDrop}
            >
              <input
                ref={fileInputRef}
                type="file"
                multiple
                accept="image/*,.txt,.md,.rs,.py,.js,.ts,.json,.toml,.yaml,.yml,.csv,.log,.sh,.sql,.go,.java,.c,.cpp,.h,.css,.html"
                onChange={e => { if (e.target.files) processFiles(e.target.files); e.target.value = ""; }}
                style={{ display: "none" }}
              />
              {pendingFiles.length > 0 && (
                <div className="ch-attach-preview">
                  {pendingFiles.map(f => (
                    <div key={f.name} className="ch-attach-chip">
                      {f.isImage ? (
                        <img src={f.dataUrl} alt={f.name} className="ch-attach-thumb" />
                      ) : (
                        <span className="ch-attach-file-icon">📄</span>
                      )}
                      <span className="ch-attach-name">{f.name}</span>
                      <button className="ch-attach-remove" onClick={() => removeFile(f.name)} title="Remove">×</button>
                    </div>
                  ))}
                </div>
              )}
              <div className="ch-input-row">
                <button
                  className="ch-input-btn cursor-pointer"
                  onClick={() => fileInputRef.current?.click()}
                  title="Attach files or images"
                >
                  <Paperclip size={16} aria-hidden="true" />
                </button>
                <textarea ref={inputRef} className="ch-input" value={input} onChange={e => setInput(e.target.value)} onKeyDown={handleKeyDown} onPaste={handleFilePaste} placeholder={`Message ${activeModel?.name ?? "AI"}... (drop files here)`} rows={1} />
                <div className="ch-input-actions">
                  <button className="ch-input-btn cursor-pointer" onClick={generateImage} title="Generate image" disabled={!input.trim()}><Image size={16} aria-hidden="true" /></button>
                  <button className="ch-send-btn" onClick={sendMessage} disabled={(!input.trim() && pendingFiles.length === 0) || sending}>
                    {sending ? "..." : "→"}
                  </button>
                </div>
              </div>
              <div className="ch-input-meta">
                <span className="ch-input-model" style={{ color: activeModel?.color }}>{activeModel?.icon} {activeModel?.name}</span>
                <span><Zap size={12} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} /> {activeModel?.fuelCost} fuel/msg</span>
                {isBuildRequest(input) && <span className="ch-input-conductor"><Hexagon size={10} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} /> Conductor mode</span>}
                {!selectedAgent && !isBuildRequest(input) && input.trim().length > 0 && <span style={{ color: "#60a5fa" }}><Hexagon size={10} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} /> Auto-routing</span>}
                {joinedAgents.length > 0 && <span><Hexagon size={10} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} /> {joinedAgents.length} agent{joinedAgents.length > 1 ? "s" : ""} joined</span>}
              </div>
            </div>
          </div>
        )}

        {/* ═══ COMPARE VIEW ═══ */}
        {view === "compare" && (
          <div className="ch-compare">
            <div className="ch-cmp-header">
              <h3 className="ch-cmp-title">Model Comparison</h3>
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
                        <span className="ch-cmp-result-meta">{m?.provider} · <Zap size={10} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} />{m?.fuelCost}</span>
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
              <h3 className="ch-hist-title"><Clock size={16} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle", marginRight: 6 }} /> Chat History</h3>
              <span className="ch-hist-count">{filteredConversations.length} conversations</span>
            </div>
            <div className="ch-hist-list">
              {filteredConversations.sort((a, b) => b.updatedAt - a.updatedAt).map(conv => {
                const m = models.find(m => m.id === conv.model);
                return (
                  <div key={conv.id} className="ch-hist-item" onClick={() => { setActiveConvId(conv.id); setView("chat"); }}>
                    <div className="ch-hist-icon" style={{ color: m?.color }}>{m?.icon}</div>
                    <div className="ch-hist-info">
                      <div className="ch-hist-name">{conv.pinned && <><Pin size={12} aria-hidden="true" /> </>}{conv.title}</div>
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
                      <button className="ch-hist-act-btn cursor-pointer" onClick={e => { e.stopPropagation(); togglePin(conv.id); }}>{conv.pinned ? <Pin size={14} aria-hidden="true" /> : <MapPin size={14} aria-hidden="true" />}</button>
                      <button className="ch-hist-act-btn cursor-pointer" onClick={e => { e.stopPropagation(); deleteConversation(conv.id); }}><Trash2 size={14} aria-hidden="true" /></button>
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
        {voiceActive && <span className="ch-status-item ch-status-voice"><Mic size={12} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} /> Jarvis Active</span>}
        {selectedAgent && <span className="ch-status-item"><Hexagon size={10} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} /> {selectedAgent.name} ({agentStatus})</span>}
        <span className="ch-status-item">{preinstalledAgents.length} agents</span>
        <span className="ch-status-item ch-status-right"><Zap size={12} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle" }} /> {fuelUsed} fuel</span>
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
    </RequiresLlm>
  );
}
