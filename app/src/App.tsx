import { useEffect, useMemo, useRef, useState } from "react";
import {
  chatWithOllama,
  checkOllama,
  clearAllAgents,
  createAgent,
  deleteModel,
  detectHardware,
  ensureOllama,
  executeAgentGoal,
  getAuditLog,
  getConfig,
  hasDesktopRuntime,
  isOllamaInstalled,
  jarvisStatus,
  listAgents,
  listAvailableModels,
  pauseAgent,
  pullModel,
  resumeAgent,
  runSetupWizard,
  saveConfig,
  sendChat,
  getSystemInfo,
  setAgentModel,
  startAgent,
  startJarvisMode,
  stopAgent,
  stopJarvisMode
} from "./api/backend";
import { useUiAudio } from "./audio/soundEngine";
import { SplashScreen } from "./components/SplashScreen";
import { HoloPanel } from "./components/fx/HoloPanel";
import { NeuralBackground } from "./components/fx/NeuralBackground";
import { PageTransition } from "./components/fx/PageTransition";
import { Sidebar, type SidebarItem } from "./components/layout/Sidebar";
import { VoiceOverlay, type VoiceOverlayState } from "./components/VoiceOverlay";
import { PulseRing } from "./components/viz/PulseRing";
import type { ConsentNotification, SystemInfo } from "./types";
import { Agents } from "./pages/Agents";
import { Audit } from "./pages/Audit";
import { Chat } from "./pages/Chat";
import Dashboard from "./pages/Dashboard";
import { Settings } from "./pages/Settings";
import { SetupWizard } from "./pages/SetupWizard";
import { Workflows } from "./pages/Workflows";
import CommandCenter from "./pages/CommandCenter";
import AuditTimeline from "./pages/AuditTimeline";
import ComplianceDashboard from "./pages/ComplianceDashboard";
import ClusterStatusPage from "./pages/ClusterStatus";
import TrustDashboard from "./pages/TrustDashboard";
import DistributedAudit from "./pages/DistributedAudit";
import { PermissionDashboard } from "./pages/PermissionDashboard";
import Protocols from "./pages/Protocols";
import Identity from "./pages/Identity";
import Firewall from "./pages/Firewall";
import DeveloperPortal from "./pages/DeveloperPortal";
import { AgentBrowser } from "./pages/AgentBrowser";
import CodeEditor from "./pages/CodeEditor";
import Terminal from "./pages/Terminal";
import FileManager from "./pages/FileManager";
import SystemMonitor from "./pages/SystemMonitor";
import NotesApp from "./pages/NotesApp";
import ProjectManager from "./pages/ProjectManager";
import DatabaseManager from "./pages/DatabaseManager";
import ApiClient from "./pages/ApiClient";
import DesignStudio from "./pages/DesignStudio";
import EmailClient from "./pages/EmailClient";
import MediaStudio from "./pages/MediaStudio";
import Messaging from "./pages/Messaging";
import AppStore from "./pages/AppStore";
import AiChatHub from "./pages/AiChatHub";
import DeployPipeline from "./pages/DeployPipeline";
import LearningCenter from "./pages/LearningCenter";
import ApprovalCenter from "./pages/ApprovalCenter";
import PolicyManagement from "./pages/PolicyManagement";
import Documents from "./pages/Documents";
import ModelHub from "./pages/ModelHub";
import TimeMachine from "./pages/TimeMachine";
import VoiceAssistant from "./pages/VoiceAssistant";
import WorldSimulation from "./pages/WorldSimulation";
import ComputerControl from "./pages/ComputerControl";
import type {
  AgentStatusEvent,
  AgentSummary,
  AuditEventRow,
  ChatMessage,
  ChatResponse,
  ChatTokenEvent,
  ConnectionStatus,
  HardwareInfo,
  NexusConfig,
  OllamaStatus,
  VoiceRuntimeState
} from "./types";
import { PushToTalk } from "./voice/PushToTalk";

type Page = "dashboard" | "chat" | "agents" | "audit" | "workflows" | "marketplace" | "settings" | "command-center" | "audit-timeline" | "marketplace-browser" | "developer-portal" | "compliance" | "cluster" | "trust" | "distributed-audit" | "permissions" | "protocols" | "identity" | "firewall" | "browser" | "computer-control" | "code-editor" | "terminal" | "file-manager" | "system-monitor" | "notes" | "project-manager" | "database" | "api-client" | "design-studio" | "email-client" | "messaging" | "media-studio" | "app-store" | "ai-chat-hub" | "deploy-pipeline" | "learning-center" | "policy-management" | "documents" | "model-hub" | "time-machine" | "voice-assistant" | "approvals" | "simulation";
type RuntimeMode = "desktop" | "mock";

const NAV_ITEMS: SidebarItem[] = [
  { id: "dashboard", label: "Dashboard", icon: "◫", shortcut: "Alt+1" },
  { id: "audit", label: "Audit", icon: "⧉", shortcut: "Alt+2" },
  { id: "workflows", label: "Workflows", icon: "⎇", shortcut: "Alt+3" },
  { id: "design-studio", label: "Design", icon: "◇", shortcut: "" },
  { id: "messaging", label: "Messaging", icon: "✉", shortcut: "" },
  { id: "media-studio", label: "Media", icon: "🖼", shortcut: "" },
  { id: "file-manager", label: "Files", icon: "📁", shortcut: "" },
  { id: "computer-control", label: "Computer Control", icon: "⌘", shortcut: "" }
];

function defaultConfig(): NexusConfig {
  return {
    llm: {
      default_model: "claude-sonnet-4-5",
      anthropic_api_key: "",
      openai_api_key: "",
      deepseek_api_key: "",
      gemini_api_key: "",
      nvidia_api_key: "",
      ollama_url: "http://localhost:11434"
    },
    search: {
      brave_api_key: ""
    },
    social: {
      x_api_key: "",
      x_api_secret: "",
      x_access_token: "",
      x_access_secret: "",
      facebook_page_token: "",
      instagram_access_token: ""
    },
    messaging: {
      telegram_bot_token: "",
      whatsapp_business_id: "",
      whatsapp_api_token: "",
      discord_bot_token: "",
      slack_bot_token: ""
    },
    voice: {
      whisper_model: "auto",
      wake_word: "hey nexus",
      tts_voice: "default"
    },
    privacy: {
      telemetry: false,
      audit_retention_days: 365
    },
    governance: {
      enable_warden_review: false
    }
  };
}

// Browser-mode agent IDs — deterministic UUIDs for offline chat when no desktop runtime
const BROWSER_AGENT_IDS = {
  coder: "a0000000-0000-4000-8000-000000000001",
  designer: "a0000000-0000-4000-8000-000000000002",
  screenPoster: "a0000000-0000-4000-8000-000000000003",
  webBuilder: "a0000000-0000-4000-8000-000000000004",
  workflowStudio: "a0000000-0000-4000-8000-000000000005",
  selfImprove: "a0000000-0000-4000-8000-000000000006",
};

const BROWSER_AGENT_ID_SET = new Set(Object.values(BROWSER_AGENT_IDS));

function agentStatusRank(status: AgentSummary["status"]): number {
  switch (status) {
    case "Running":
      return 6;
    case "Starting":
      return 5;
    case "Paused":
      return 4;
    case "Created":
      return 3;
    case "Stopping":
      return 2;
    case "Stopped":
      return 1;
    case "Destroyed":
      return 0;
    default:
      return -1;
  }
}

function dedupeAgentsById(agents: AgentSummary[]): AgentSummary[] {
  const byId = new Map<string, AgentSummary>();
  for (const agent of agents) {
    const existing = byId.get(agent.id);
    if (!existing || agentStatusRank(agent.status) >= agentStatusRank(existing.status)) {
      byId.set(agent.id, agent);
    }
  }
  return Array.from(byId.values());
}

function coreAgents(): AgentSummary[] {
  return [
    {
      id: BROWSER_AGENT_IDS.coder,
      name: "Coder",
      status: "Running",
      fuel_remaining: 9200,
      fuel_budget: 10000,
      last_action: "refactored auth middleware",
      isSystem: true,
      sandbox_runtime: "wasmtime",
      memory_usage_bytes: 131072,
      capabilities: ["llm.query", "fs.read", "fs.write"]
    },
    {
      id: BROWSER_AGENT_IDS.designer,
      name: "Designer",
      status: "Running",
      fuel_remaining: 6500,
      fuel_budget: 10000,
      last_action: "generated landing page mockup",
      isSystem: true,
      sandbox_runtime: "wasmtime",
      memory_usage_bytes: 98304,
      capabilities: ["llm.query", "fs.read"]
    },
    {
      id: BROWSER_AGENT_IDS.screenPoster,
      name: "Screen Poster",
      status: "Paused",
      fuel_remaining: 4100,
      fuel_budget: 10000,
      last_action: "awaiting human approval for X post",
      isSystem: true,
      sandbox_runtime: "wasmtime",
      memory_usage_bytes: 65536,
      capabilities: ["llm.query", "fs.read", "request_approval"]
    },
    {
      id: BROWSER_AGENT_IDS.webBuilder,
      name: "Web Builder",
      status: "Running",
      fuel_remaining: 7800,
      fuel_budget: 10000,
      last_action: "deployed staging build v2.4.1",
      isSystem: true,
      sandbox_runtime: "wasmtime",
      memory_usage_bytes: 196608,
      capabilities: ["llm.query", "fs.read", "fs.write"]
    },
    {
      id: BROWSER_AGENT_IDS.workflowStudio,
      name: "Workflow Studio",
      status: "Stopped",
      fuel_remaining: 2300,
      fuel_budget: 10000,
      last_action: "completed daily analytics pipeline",
      isSystem: true,
      sandbox_runtime: "wasmtime",
      memory_usage_bytes: 0,
      capabilities: ["llm.query", "fs.read"]
    },
    {
      id: BROWSER_AGENT_IDS.selfImprove,
      name: "Self-Improve",
      status: "Running",
      fuel_remaining: 8400,
      fuel_budget: 10000,
      last_action: "optimized prompt routing latency",
      isSystem: true,
      sandbox_runtime: "wasmtime",
      memory_usage_bytes: 114688,
      capabilities: ["llm.query", "fs.read", "fs.write", "request_approval"]
    }
  ];
}

function browserAgents(): AgentSummary[] {
  return coreAgents();
}


function emptyAudit(): AuditEventRow[] {
  return [];
}

function browserChatReply(message: string): ChatResponse {
  const lowered = message.toLowerCase();
  let text: string;
  if (lowered.includes("status")) {
    text = "6 agents deployed across the governed runtime. 4 running, 1 paused (Screen Poster - awaiting HITL approval for social post), 1 stopped (Workflow Studio - pipeline complete). Average fuel: 64%. All capability checks passing. Open the Agents page for full mission control.";
  } else if (lowered.includes("search") || lowered.includes("find") || lowered.includes("look up") || lowered.includes("browse")) {
    text = `I can help with that! Let me use the Web Search connector (Brave Search) to find information. Querying now via the governed web.search capability... Here are the top results I found. Would you like me to dig deeper into any of these, or shall I have the Web Builder agent create a summary page?`;
  } else if (lowered.includes("post") || lowered.includes("tweet") || lowered.includes("social") || lowered.includes("share")) {
    text = "I'll route this through the Screen Poster agent. Since social posting is a Tier 1 operation, it requires your approval before publishing. I've drafted the content and submitted it for HITL review. Check the Agents page to approve or edit before it goes live.";
  } else if (lowered.includes("code") || lowered.includes("build") || lowered.includes("compile") || lowered.includes("fix")) {
    text = "On it. I'm dispatching this to the Coder agent with process.exec and fs.write capabilities. It will analyze the codebase, implement the changes, and run the test suite. Fuel cost estimate: ~800 units. You can monitor progress in real-time on the Agents page.";
  } else if (lowered.includes("hello") || lowered.includes("hi") || lowered.includes("hey")) {
    text = "Hello! I'm NexusOS, your governed agent operating system. I have 6 specialized agents ready to help: Coder, Designer, Screen Poster, Web Builder, Workflow Studio, and Self-Improve. I can search the web, post content, read/write files, execute code, and more - all through governed capabilities with full audit trails. What can I help you with?";
  } else if (lowered.includes("help") || lowered.includes("what can you")) {
    text = "I'm NexusOS with full access to: Web Search (Brave connector), Social Posting (Screen Poster agent with HITL approval), File System (read/write), Code Execution (Coder agent), LLM Queries (multiple providers), Browser Automation (Web Builder), and Workflow Orchestration. All actions go through kernel capability checks with fuel budgeting and append-only audit trails. Just tell me what you need!";
  } else {
    text = "Understood. Let me route this through the appropriate agent. I have web search via Brave Search connector, social media posting via Screen Poster, file system access, LLM capabilities across multiple providers, and browser automation through Web Builder. All actions are governed with capability checks and fuel budgets. Processing your request now...";
  }
  return { text, model: "nexus-mock", token_count: text.split(" ").length, cost: 0, latency_ms: 18 };
}

function makeMessage(role: ChatMessage["role"], content: string, extra?: Partial<ChatMessage>): ChatMessage {
  return {
    id: makeId(),
    role,
    content,
    timestamp: Date.now(),
    ...extra
  };
}

function makeId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.floor(Math.random() * 100_000)}`;
}

function formatError(value: unknown): string {
  if (value instanceof Error) {
    return value.message;
  }
  return String(value);
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export default function App(): JSX.Element {
  const [page, setPage] = useState<Page>("dashboard");
  const [runtimeMode, setRuntimeMode] = useState<RuntimeMode>("mock");
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [auditEvents, setAuditEvents] = useState<AuditEventRow[]>([]);
  const [config, setConfig] = useState<NexusConfig>(defaultConfig);
  const [draft, setDraft] = useState("");
  const [selectedAgent, setSelectedAgent] = useState("");
  const [selectedModel, setSelectedModel] = useState("mock");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [isSending, setIsSending] = useState(false);
  const [isRecording, setIsRecording] = useState(false);
  const [isSavingConfig, setIsSavingConfig] = useState(false);
  const [factoryTrigger, setFactoryTrigger] = useState(0);
  const [activityPulse, setActivityPulse] = useState(0);
  const [backendRestarting, setBackendRestarting] = useState(false);
  const reconnectTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const [appReady, setAppReady] = useState(false);
  const [splashVisible, setSplashVisible] = useState(true);
  const [showSetupWizard, setShowSetupWizard] = useState(false);
  const [ollamaConnected, setOllamaConnected] = useState(false);
  const [ollamaModels, setOllamaModels] = useState<OllamaStatus["models"]>([]);
  const [overlay, setOverlay] = useState<VoiceOverlayState>({
    visible: false,
    listening: false,
    transcription: "",
    responseText: ""
  });
  const [permissionAgentId, setPermissionAgentId] = useState<string>("");
  const [pendingApprovalCount, setPendingApprovalCount] = useState(0);
  const pushToTalk = useRef<PushToTalk | null>(null);
  const previousPageRef = useRef<Page>(page);
  const uniqueAgents = useMemo(() => dedupeAgentsById(agents), [agents]);
  const { enabled: uiSoundEnabled, volume: uiSoundVolume, setEnabled: setUiSoundEnabled, setVolume: setUiSoundVolume, play } =
    useUiAudio();

  function bumpActivity(): void {
    setActivityPulse((previous) => previous + 1);
  }

  useEffect(() => {
    pushToTalk.current = new PushToTalk();
  }, []);

  useEffect(() => {
    if (!hasDesktopRuntime()) {
      setRuntimeMode("mock");
      setAgents(browserAgents());
      setAuditEvents(emptyAudit());
      setConfig(defaultConfig());
      setMessages([
        makeMessage("user", "Review the auth middleware in src/auth.rs for security issues"),
        makeMessage(
          "assistant",
          "Scanning src/auth.rs... Found 3 items:\n\n1. JWT token validation does not check expiration claim (line 42). Add `validate_exp: true` to the validation params.\n\n2. CORS origin is set to wildcard `*` (line 18). Restrict to your domain in production.\n\n3. Rate limiter uses in-memory store. Consider Redis-backed store for multi-instance deployments.\n\nShall I generate patches for these findings?",
          { model: "claude-sonnet-4-5" }
        ),
        makeMessage("user", "Yes, fix issue 1 and 2. Leave the rate limiter for now."),
        makeMessage(
          "assistant",
          "Patches applied:\n\n`src/auth.rs:42` - Added `validate_exp: true` to JWT validation config.\n`src/auth.rs:18` - Replaced wildcard CORS with `https://app.nexus-os.dev`.\n\nBoth changes verified with `cargo test --lib auth`. All 12 auth tests pass.",
          { model: "claude-sonnet-4-5" }
        ),
        makeMessage("user", "What's the current agent status?"),
        makeMessage(
          "assistant",
          "6 agents deployed. 4 running, 1 paused (Screen Poster - awaiting approval), 1 stopped (Workflow Studio - task complete). Average fuel: 64%. Open the Agents page for full mission control.",
          { model: "mock-1" }
        )
      ]);
      bumpActivity();
      setAppReady(true);
      return;
    }

    let cancelled = false;

    const hydrateDesktop = async (): Promise<void> => {
      try {
        const [loadedAgents, loadedAudit, loadedConfig, voice] = await Promise.all([
          listAgents(),
          getAuditLog(undefined, 500),
          getConfig(),
          jarvisStatus()
        ]);
        if (cancelled) {
          return;
        }
        setRuntimeMode("desktop");
        setRuntimeError(null);
        setAgents(loadedAgents);
        setAuditEvents(loadedAudit);
        setConfig(loadedConfig);
        applyVoiceState(voice);

        // Refresh Ollama status in background
        checkOllama().then((status) => {
          if (!cancelled) {
            setOllamaConnected(status.connected);
            setOllamaModels(status.models);
          }
        }).catch(() => {});

        // Check if first-run setup is needed
        const needsSetup = !loadedConfig.hardware?.gpu || loadedConfig.hardware.gpu.length === 0;
        if (needsSetup) {
          setShowSetupWizard(true);
        }

        setMessages([
          makeMessage(
            "assistant",
            `Connected to desktop backend. Default model: ${loadedConfig.llm.default_model || "mock-1"}.`
          )
        ]);
        play("notification");
        bumpActivity();
        setAppReady(true);
      } catch (error) {
        if (cancelled) {
          return;
        }
        setRuntimeMode("mock");
        setRuntimeError(`Desktop backend unavailable: ${formatError(error)}`);
        setAgents(browserAgents());
        setAuditEvents(emptyAudit());
        setConfig(defaultConfig());
        setMessages([
          makeMessage("assistant", "Backend connection failed; running in mock mode.")
        ]);
        play("error");
        setAppReady(true);
      }
    };

    void hydrateDesktop();

    return () => {
      cancelled = true;
      if (reconnectTimer.current) {
        clearInterval(reconnectTimer.current);
        reconnectTimer.current = null;
      }
    };
  }, []);

  // Listen for real-time agent status updates from the backend
  useEffect(() => {
    if (!hasDesktopRuntime()) return;
    let unlisten: (() => void) | undefined;
    import("@tauri-apps/api/event").then((mod) => {
      mod.listen<AgentStatusEvent>("agent-status-changed", (event) => {
        const { agent_id, status, fuel_remaining } = event.payload;
        setAgents((prev) =>
          prev.map((a) =>
            a.id === agent_id
              ? { ...a, status: status as AgentSummary["status"], fuel_remaining }
              : a
          )
        );
      }).then((fn) => { unlisten = fn; });
    });
    return () => { unlisten?.(); };
  }, []);

  // Global listener for consent-request-pending — fires on ALL pages
  useEffect(() => {
    if (!hasDesktopRuntime()) return;
    const cleanups: (() => void)[] = [];

    // Request browser notification permission eagerly
    if (typeof Notification !== "undefined" && Notification.permission === "default") {
      Notification.requestPermission();
    }

    import("@tauri-apps/api/event").then((mod) => {
      mod
        .listen<ConsentNotification>("consent-request-pending", (event) => {
          setPendingApprovalCount((prev) => prev + 1);
          play("notification");

          // Desktop notification via browser Notification API (works in Tauri webview)
          if (typeof Notification !== "undefined" && Notification.permission === "granted") {
            new Notification("Nexus OS — Agent Approval Required", {
              body: `${event.payload.agent_name} wants to: ${event.payload.operation_summary}`,
              tag: `consent-${event.payload.consent_id}`,
            });
          }
        })
        .then((fn) => cleanups.push(fn));

      mod
        .listen<{ consent_id: string; status: string }>("consent-resolved", () => {
          setPendingApprovalCount((prev) => Math.max(0, prev - 1));
        })
        .then((fn) => cleanups.push(fn));
    });

    return () => { for (const fn of cleanups) fn(); };
  }, [play]);

  useEffect(() => {
    if (previousPageRef.current !== page) {
      previousPageRef.current = page;
      play("transition");
      bumpActivity();
    }
  }, [page, play]);

  const connectionStatus: ConnectionStatus = runtimeMode === "desktop" ? "connected" : "mock";
  const runningAgents = useMemo(
    () => uniqueAgents.filter((agent) => agent.status === "Running").length,
    [uniqueAgents]
  );
  const [sysInfo, setSysInfo] = useState<SystemInfo | null>(null);

  useEffect(() => {
    let active = true;
    function poll(): void {
      getSystemInfo()
        .then((info) => { if (active) setSysInfo(info); })
        .catch(() => {});
    }
    poll();
    const id = setInterval(poll, 3000);
    return () => { active = false; clearInterval(id); };
  }, []);

  function applyVoiceState(state: VoiceRuntimeState): void {
    setOverlay((prev) => ({
      ...prev,
      visible: state.overlay_visible,
      listening: state.overlay_visible,
      phase: state.overlay_visible ? "listening" : "idle",
      amplitude: state.overlay_visible ? 0.42 : 0.18
    }));
  }

  async function refreshOllamaStatus(): Promise<void> {
    try {
      const status = await checkOllama();
      setOllamaConnected(status.connected);
      setOllamaModels(status.models);
    } catch {
      setOllamaConnected(false);
      setOllamaModels([]);
    }
  }

  async function handleDeleteModel(name: string): Promise<void> {
    try {
      await deleteModel(name);
      await refreshOllamaStatus();
      play("success");
    } catch (error) {
      setRuntimeError(`Failed to delete model: ${formatError(error)}`);
      play("error");
    }
  }

  async function refreshDesktopData(): Promise<void> {
    if (runtimeMode !== "desktop") {
      return;
    }
    try {
      const [loadedAgents, loadedAudit] = await Promise.all([listAgents(), getAuditLog(undefined, 500)]);
      setAgents(loadedAgents);
      setAuditEvents(loadedAudit);
      if (backendRestarting) {
        setBackendRestarting(false);
        setRuntimeError(null);
        if (reconnectTimer.current) {
          clearInterval(reconnectTimer.current);
          reconnectTimer.current = null;
        }
      }
    } catch {
      if (!backendRestarting) {
        setBackendRestarting(true);
        if (!reconnectTimer.current) {
          reconnectTimer.current = setInterval(() => {
            void refreshDesktopData();
          }, 2000);
        }
      }
    }
  }

  function updateMockAgentStatus(id: string, status: AgentSummary["status"]): void {
    setAgents((prev) =>
      prev.map((agent) =>
        agent.id === id
          ? { ...agent, status, last_action: `status changed to ${status.toLowerCase()}` }
          : agent
      )
    );
  }

  async function handleStartAgent(id: string): Promise<void> {
    if (runtimeMode !== "desktop") {
      updateMockAgentStatus(id, "Running");
      play("success");
      bumpActivity();
      return;
    }
    try {
      try {
        await resumeAgent(id);
      } catch {
        await startAgent(id);
      }
      await refreshDesktopData();
      setRuntimeError(null);
      play("success");
      bumpActivity();
    } catch (error) {
      setRuntimeError(`Unable to start agent: ${formatError(error)}`);
      play("error");
    }
  }

  async function handlePauseAgent(id: string): Promise<void> {
    if (runtimeMode !== "desktop") {
      updateMockAgentStatus(id, "Paused");
      play("click");
      bumpActivity();
      return;
    }
    try {
      await pauseAgent(id);
      await refreshDesktopData();
      setRuntimeError(null);
      play("click");
      bumpActivity();
    } catch (error) {
      setRuntimeError(`Unable to pause agent: ${formatError(error)}`);
      play("error");
    }
  }

  async function handleStopAgent(id: string): Promise<void> {
    if (runtimeMode !== "desktop") {
      updateMockAgentStatus(id, "Stopped");
      play("click");
      bumpActivity();
      return;
    }
    try {
      await stopAgent(id);
      await refreshDesktopData();
      setRuntimeError(null);
      play("click");
      bumpActivity();
    } catch (error) {
      setRuntimeError(`Unable to stop agent: ${formatError(error)}`);
      play("error");
    }
  }

  async function handleCreateAgent(manifestJson: string): Promise<void> {
    if (runtimeMode !== "desktop") {
      const newAgent: AgentSummary = {
        id: makeId(),
        name: "mock-created-agent",
        status: "Running",
        fuel_remaining: 10_000,
        last_action: "created from factory"
      };
      setAgents((prev) => [newAgent, ...prev]);
      setAuditEvents((prev) => [
        {
          event_id: makeId(),
          timestamp: Math.floor(Date.now() / 1000),
          agent_id: newAgent.id,
          event_type: "UserAction",
          payload: { action: "create_agent", manifest: manifestJson },
          previous_hash: prev[0]?.hash ?? "genesis",
          hash: makeId()
        },
        ...prev
      ]);
      setRuntimeError(null);
      play("success");
      bumpActivity();
      return;
    }

    try {
      const agentId = await createAgent(manifestJson);
      await refreshDesktopData();
      setRuntimeError(null);
      setMessages((prev) => [
        ...prev,
        makeMessage(
          "assistant",
          agentId.startsWith("approval-requested:")
            ? `Approval requested for transcendent agent creation: ${agentId.replace("approval-requested:", "")}`
            : `Agent created: ${agentId}`,
          { model: "system" }
        )
      ]);
      play("success");
      bumpActivity();
    } catch (error) {
      setRuntimeError(`Unable to create agent: ${formatError(error)}`);
      play("error");
    }
  }

  async function handleDeleteAgent(id: string): Promise<void> {
    if (runtimeMode !== "desktop") {
      // In mock mode, prevent deleting core mock agents
      if (BROWSER_AGENT_ID_SET.has(id)) {
        return;
      }
      setAgents((prev) => prev.filter((a) => a.id !== id));
      play("click");
      bumpActivity();
      return;
    }
    try {
      await stopAgent(id);
      await refreshDesktopData();
      play("click");
      bumpActivity();
    } catch (error) {
      setRuntimeError(`Unable to delete agent: ${formatError(error)}`);
      play("error");
    }
  }

  async function handleClearAllAgents(): Promise<void> {
    if (runtimeMode !== "desktop") {
      setAgents([]);
      play("click");
      bumpActivity();
      return;
    }
    try {
      await clearAllAgents();
      setAgents([]);
      play("click");
      bumpActivity();
    } catch (error) {
      setRuntimeError(`Unable to clear agents: ${formatError(error)}`);
      play("error");
    }
  }

  const AGENT_PROMPTS: Record<string, string> = {
    "": "You are NexusOS, a governed AI operating system. You help users with coding, design, automation, and content. Be concise and helpful.",
    [BROWSER_AGENT_IDS.coder]: "You are the NexusOS Coder Agent. You write clean code in Rust, TypeScript, and Python. You analyze architecture, review code, fix bugs, and run tests. Show code in fenced blocks.",
    [BROWSER_AGENT_IDS.designer]: "You are the NexusOS Designer Agent. You create UI components, design systems, and design tokens. Output React/TypeScript.",
    [BROWSER_AGENT_IDS.screenPoster]: "You are the NexusOS Screen Poster Agent. You draft social media posts for X, Instagram, Facebook, Reddit. Optimize for engagement.",
    [BROWSER_AGENT_IDS.webBuilder]: "You are the NexusOS Web Builder Agent. You generate websites from descriptions using React and modern web tech.",
    [BROWSER_AGENT_IDS.workflowStudio]: "You are the NexusOS Workflow Studio Agent. You design automation pipelines with DAG nodes, retries, and checkpoints.",
    [BROWSER_AGENT_IDS.selfImprove]: "You are the NexusOS Self-Improve Agent. You analyze performance metrics and optimize prompts.",
  };

  function getModelForAgent(agentId: string): string {
    // Look up model from config agents map
    const agentKey = agentId.replace("agent-", "").replace("-", "_");
    const agentConfig = config.agents?.[agentKey];
    if (agentConfig?.model) return agentConfig.model;
    // Fallback to default model
    return config.llm.default_model || "qwen3.5:9b";
  }

  async function handleSend(): Promise<void> {
    const input = draft.trim();
    if (input.length === 0 || isSending) {
      return;
    }

    setDraft("");
    play("click");
    setMessages((prev) => [...prev, makeMessage("user", input)]);

    if (/^\s*create agent\b/i.test(input)) {
      setPage("agents");
      setFactoryTrigger((prev) => prev + 1);
      setMessages((prev) => [
        ...prev,
        makeMessage(
          "assistant",
          "Routing to Agent Factory. Confirm manifest details, then click Create."
        )
      ]);
      bumpActivity();
      return;
    }

    setIsSending(true);
    const assistantId = makeId();
    const model = selectedModel === "mock" ? getModelForAgent(selectedAgent) : selectedModel;
    const isOllamaModel = model.startsWith("ollama/") || (!model.includes("/") && model !== "mock");
    const ollamaModelName = model.startsWith("ollama/") ? model.slice("ollama/".length) : model;
    setMessages((prev) => [
      ...prev,
      {
        id: assistantId,
        role: "assistant",
        content: "",
        timestamp: Date.now(),
        model,
        streaming: true
      }
    ]);

    if (runtimeMode === "desktop") {
      // If a real agent is selected (UUID but NOT a browser-mode stub), ALWAYS route through cognitive loop
      const isRealAgent = selectedAgent.length > 0
        && /^[0-9a-f]{8}-[0-9a-f]{4}-/i.test(selectedAgent)
        && !BROWSER_AGENT_ID_SET.has(selectedAgent);
      if (isRealAgent) {
        try {
          const eventMod = await import("@tauri-apps/api/event");
          let stepMessages: string[] = [];

          // Listen for cognitive cycle events (skip blocked — handled by agent-blocked)
          const unlistenCycle = await eventMod.listen<{
            agent_id: string; goal_id: string; phase: string;
            steps_executed: number; fuel_consumed: number;
            should_continue: boolean; blocked_reason: string | null;
          }>("agent-cognitive-cycle", (event) => {
            const p = event.payload;
            if (p.agent_id !== selectedAgent) return;
            if (p.phase === "Blocked") return; // handled by agent-blocked event
            const phaseMsg = `Phase: ${p.phase}${p.steps_executed > 0 ? ` (${p.steps_executed} step, ${p.fuel_consumed.toFixed(1)} fuel)` : ""}`;
            stepMessages.push(phaseMsg);
            setMessages((prev) =>
              prev.map((m) =>
                m.id === assistantId
                  ? { ...m, content: stepMessages.join("\n"), streaming: true }
                  : m
              )
            );
          });

          // Listen for HITL approval-needed events (amber info style, not error)
          const unlistenBlocked = await eventMod.listen<{
            agent_id: string; goal_id: string; message: string;
            action: string; agent_name: string;
          }>("agent-blocked", (event) => {
            const p = event.payload;
            if (p.agent_id !== selectedAgent) return;
            const approvalMsgId = makeId();
            setMessages((prev) => [
              ...prev,
              {
                id: approvalMsgId,
                role: "assistant" as const,
                content: p.message,
                timestamp: Date.now(),
                model: "system",
                variant: "approval" as const,
              }
            ]);
          });

          // Listen for agent-resumed after approval granted
          const unlistenResumed = await eventMod.listen<{
            agent_id: string; goal_id: string; message: string;
          }>("agent-resumed", (event) => {
            const p = event.payload;
            if (p.agent_id !== selectedAgent) return;
            const resumedMsgId = makeId();
            setMessages((prev) => [
              ...prev,
              {
                id: resumedMsgId,
                role: "assistant" as const,
                content: p.message,
                timestamp: Date.now(),
                model: "system",
                variant: "resumed" as const,
              }
            ]);
          });

          // Listen for goal completion
          const goalDone = new Promise<{ success: boolean; reason?: string; result_summary?: string }>((resolve) => {
            eventMod.listen<{
              agent_id: string; goal_id: string; success: boolean; reason?: string; result_summary?: string;
            }>("agent-goal-completed", (event) => {
              if (event.payload.agent_id === selectedAgent) {
                resolve(event.payload);
              }
            });
          });

          const goalId = await executeAgentGoal(selectedAgent, input, 5);
          stepMessages.push(`Goal assigned: ${goalId}`);
          setMessages((prev) =>
            prev.map((m) =>
              m.id === assistantId
                ? { ...m, content: stepMessages.join("\n") }
                : m
            )
          );

          // Wait for completion (with 10-minute timeout)
          const result = await Promise.race([
            goalDone,
            new Promise<{ success: boolean; reason?: string; result_summary?: string }>((resolve) =>
              setTimeout(() => resolve({ success: false, reason: "Timed out after 10 minutes waiting for the agent to finish." }), 600_000)
            ),
          ]);

          const summary = result.success
            ? (result.result_summary || "Goal completed successfully.")
            : (result.result_summary || result.reason || "Goal failed — unknown error. Check the audit log for details.");
          stepMessages.push(summary);
          const finalVariant = result.success ? undefined : ("error" as const);
          setMessages((prev) =>
            prev.map((m) =>
              m.id === assistantId
                ? { ...m, content: stepMessages.join("\n"), streaming: false, variant: finalVariant }
                : m
            )
          );
          unlistenCycle();
          unlistenBlocked();
          unlistenResumed();
          setRuntimeError(null);
          play(result.success ? "notification" : "error");
          bumpActivity();
        } catch (error) {
          setMessages((prev) =>
            prev.map((m) =>
              m.id === assistantId
                ? { ...m, content: `Error: ${formatError(error)}`, model: "system", streaming: false }
                : m
            )
          );
          setRuntimeError(`Agent goal failed: ${formatError(error)}`);
          play("error");
        } finally {
          setIsSending(false);
          setOverlay((prev) => ({ ...prev, phase: prev.listening ? "listening" : "idle", amplitude: 0.18 }));
        }
        return;
      }

      const systemPrompt = AGENT_PROMPTS[selectedAgent] || AGENT_PROMPTS[""];
      const apiMessages = [
        { role: "system", content: systemPrompt },
        ...messages.filter(m => m.role === "user" || m.role === "assistant").slice(-20).map(m => ({
          role: m.role,
          content: m.content
        })),
        { role: "user", content: input }
      ];

      if (isOllamaModel) {
        // Stream via Ollama
        let unlisten: (() => void) | undefined;
        let fullText = "";
        try {
          const eventMod = await import("@tauri-apps/api/event");
          unlisten = await eventMod.listen<ChatTokenEvent>("chat-token", (event) => {
            const { full, done } = event.payload;
            fullText = full;

            if (done) {
              setMessages((prev) =>
                prev.map((m) =>
                  m.id === assistantId ? { ...m, content: fullText, streaming: false } : m
                )
              );
            } else {
              setMessages((prev) =>
                prev.map((m) =>
                  m.id === assistantId ? { ...m, content: full } : m
                )
              );
            }
          });

          await chatWithOllama(apiMessages, ollamaModelName);
          setRuntimeError(null);
          setOverlay((prev) => ({ ...prev, phase: "speaking", amplitude: 0.5 }));
          play("notification");
          bumpActivity();
        } catch (error) {
          setMessages((prev) =>
            prev.map((message) =>
              message.id === assistantId
                ? {
                    ...message,
                    content: `Error: Could not reach ${model}. ${formatError(error)}`,
                    model: "system",
                    streaming: false
                  }
                : message
            )
          );
          setRuntimeError(`Chat request failed: ${formatError(error)}`);
          play("error");
        } finally {
          unlisten?.();
          setIsSending(false);
          setOverlay((prev) => ({ ...prev, phase: prev.listening ? "listening" : "idle", amplitude: 0.18 }));
        }
      } else {
        // Cloud model — use governed send_chat with provider-prefixed model
        try {
          const response = await sendChat(input, model);
          setMessages((prev) =>
            prev.map((m) =>
              m.id === assistantId ? { ...m, content: response.text, model, streaming: false } : m
            )
          );
          setRuntimeError(null);
          setOverlay((prev) => ({ ...prev, phase: "speaking", amplitude: 0.5 }));
          play("notification");
          bumpActivity();
        } catch (error) {
          setMessages((prev) =>
            prev.map((message) =>
              message.id === assistantId
                ? {
                    ...message,
                    content: `Error: ${formatError(error)}`,
                    model: "system",
                    streaming: false
                  }
                : message
            )
          );
          setRuntimeError(`Chat request failed: ${formatError(error)}`);
          play("error");
        } finally {
          setIsSending(false);
          setOverlay((prev) => ({ ...prev, phase: prev.listening ? "listening" : "idle", amplitude: 0.18 }));
        }
      }
    } else {
      // Mock mode fallback
      try {
        const response = browserChatReply(input);
        // Simulate streaming word-by-word
        const chunks = response.text.split(" ");
        let current = "";
        for (let index = 0; index < chunks.length; index += 1) {
          current = current.length === 0 ? chunks[index] : `${current} ${chunks[index]}`;
          const done = index === chunks.length - 1;
          setMessages((prev) =>
            prev.map((message) =>
              message.id === assistantId
                ? { ...message, content: current, model: response.model, streaming: !done }
                : message
            )
          );
          await sleep(done ? 0 : 16);
        }
        setRuntimeError(null);
        setOverlay((prev) => ({ ...prev, phase: "speaking", amplitude: 0.5 }));
        play("notification");
        bumpActivity();
      } catch (error) {
        setMessages((prev) =>
          prev.map((message) =>
            message.id === assistantId
              ? { ...message, content: `Request failed: ${formatError(error)}`, model: "system", streaming: false }
              : message
          )
        );
        setRuntimeError(`Chat request failed: ${formatError(error)}`);
        play("error");
      } finally {
        setIsSending(false);
        setOverlay((prev) => ({ ...prev, phase: prev.listening ? "listening" : "idle", amplitude: 0.18 }));
      }
    }
  }

  async function handleToggleMic(): Promise<void> {
    const recorder = pushToTalk.current;
    if (!recorder) {
      return;
    }

    if (!isRecording) {
      recorder.startRecording();
      setIsRecording(true);
      setRuntimeError(null);
      setOverlay((prev) => ({ ...prev, visible: true, listening: true, phase: "listening", amplitude: 0.45 }));
      play("click");
      return;
    }

    setIsRecording(false);
    try {
      const result = await recorder.stopAndTranscribe();
      if (result.transcript.trim().length > 0) {
        setDraft(result.transcript.trim());
      }
      setOverlay((prev) => ({ ...prev, phase: "processing", amplitude: 0.32 }));
      bumpActivity();
    } catch (error) {
      setRuntimeError(`Push-to-talk failed: ${formatError(error)}`);
      play("error");
    }
  }

  async function handleSaveConfig(): Promise<void> {
    if (isSavingConfig) {
      return;
    }
    setIsSavingConfig(true);
    try {
      if (runtimeMode === "desktop") {
        await saveConfig(config);
      } else {
        await sleep(140);
      }
      setRuntimeError(null);
      play("success");
    } catch (error) {
      setRuntimeError(`Unable to save settings: ${formatError(error)}`);
      play("error");
    } finally {
      setIsSavingConfig(false);
    }
  }

  async function handleSetupComplete(hw: HardwareInfo, ollamaStatus: OllamaStatus): Promise<void> {
    // Run the full setup wizard on the backend
    if (runtimeMode === "desktop") {
      try {
        const result = await runSetupWizard(ollamaStatus.base_url);
        if (result.config_saved) {
          const refreshedConfig = await getConfig();
          setConfig(refreshedConfig);
        }
      } catch (error) {
        setRuntimeError(`Setup failed: ${formatError(error)}`);
      }
    } else {
      // Mock mode: update config locally
      setConfig((prev) => ({
        ...prev,
        hardware: {
          gpu: hw.gpu,
          vram_mb: hw.vram_mb,
          ram_mb: hw.ram_mb,
          detected_at: hw.detected_at
        },
        ollama: {
          base_url: ollamaStatus.base_url,
          status: ollamaStatus.connected ? "connected" : "disconnected"
        },
        models: {
          primary: hw.recommended_primary,
          fast: hw.recommended_fast
        },
        llm: {
          ...prev.llm,
          default_model: hw.recommended_primary
        }
      }));
    }
    setShowSetupWizard(false);
    play("success");
    bumpActivity();
  }

  async function handleRefresh(): Promise<void> {
    if (runtimeMode !== "desktop") {
      setAgents(browserAgents());
      setAuditEvents(emptyAudit());
      setRuntimeError(null);
      bumpActivity();
      return;
    }
    try {
      const [loadedConfig, voice] = await Promise.all([getConfig(), jarvisStatus(), refreshDesktopData()]);
      setConfig(loadedConfig);
      applyVoiceState(voice);
      setRuntimeError(null);
      play("notification");
      bumpActivity();
    } catch (error) {
      setRuntimeError(`Unable to refresh data: ${formatError(error)}`);
      play("error");
    }
  }

  async function enableJarvisMode(): Promise<void> {
    if (runtimeMode !== "desktop") {
      setOverlay({
        visible: true,
        listening: true,
        transcription: "hey nexus",
        responseText: "mock voice mode active",
        phase: "listening",
        amplitude: 0.44
      });
      play("notification");
      return;
    }
    try {
      const voice = await startJarvisMode();
      applyVoiceState(voice);
      setOverlay((prev) => ({ ...prev, responseText: "Jarvis mode active.", phase: "listening", amplitude: 0.44 }));
      setRuntimeError(null);
      play("notification");
    } catch (error) {
      setRuntimeError(`Unable to start voice mode: ${formatError(error)}`);
      play("error");
    }
  }

  async function disableJarvisMode(): Promise<void> {
    if (runtimeMode !== "desktop") {
      setOverlay({
        visible: false,
        listening: false,
        transcription: "",
        responseText: "",
        phase: "idle",
        amplitude: 0.12
      });
      play("click");
      return;
    }
    try {
      const voice = await stopJarvisMode();
      applyVoiceState(voice);
      setOverlay((prev) => ({ ...prev, transcription: "", responseText: "", phase: "idle", amplitude: 0.12 }));
      setRuntimeError(null);
      play("click");
    } catch (error) {
      setRuntimeError(`Unable to stop voice mode: ${formatError(error)}`);
      play("error");
    }
  }

  function renderPage(): JSX.Element {
    if (page === "dashboard") {
      return <Dashboard />;
    }
    if (page === "chat") {
      return (
        <Chat
          messages={messages}
          draft={draft}
          isRecording={isRecording}
          isSending={isSending}
          agents={uniqueAgents}
          selectedAgent={selectedAgent}
          selectedModel={selectedModel}
          onAgentChange={setSelectedAgent}
          onModelChange={setSelectedModel}
          onDraftChange={setDraft}
          onSend={() => {
            void handleSend();
          }}
          onToggleMic={() => {
            void handleToggleMic();
          }}
          onClearMessages={() => {
            setMessages([]);
            setDraft("");
          }}
          onNavigate={(p) => setPage(p as Page)}
        />
      );
    }
    if (page === "agents") {
      return (
        <Agents
          agents={agents}
          auditEvents={auditEvents}
          factoryTrigger={factoryTrigger}
          onStart={(id) => {
            void handleStartAgent(id);
          }}
          onPause={(id) => {
            void handlePauseAgent(id);
          }}
          onStop={(id) => {
            void handleStopAgent(id);
          }}
          onCreate={(manifestJson) => {
            void handleCreateAgent(manifestJson);
          }}
          onDelete={handleDeleteAgent}
          onClearAll={() => { void handleClearAllAgents(); }}
          onPermissions={(id) => {
            setPermissionAgentId(id);
            setPage("permissions");
          }}
        />
      );
    }
    if (page === "permissions") {
      const permAgent = agents.find((a) => a.id === permissionAgentId);
      if (!permAgent && agents.length > 0) {
        return (
          <div style={{ padding: "1.5rem", maxWidth: 800, margin: "0 auto" }}>
            <h2 style={{ fontFamily: "var(--font-display, monospace)", color: "var(--text-primary, #e2e8f0)", marginBottom: "1rem" }}>
              Permission Dashboard
            </h2>
            <p style={{ color: "var(--text-secondary, #94a3b8)", marginBottom: "1.5rem", fontSize: "0.9rem" }}>
              Select an agent to manage its permissions.
            </p>
            <div style={{ display: "grid", gap: "0.6rem" }}>
              {agents.map((a) => (
                <button
                  key={a.id}
                  onClick={() => setPermissionAgentId(a.id)}
                  style={{
                    background: "var(--bg-secondary, #1e293b)",
                    border: "1px solid var(--border, #334155)",
                    borderRadius: 8,
                    padding: "0.8rem 1rem",
                    cursor: "pointer",
                    textAlign: "left",
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    color: "var(--text-primary, #e2e8f0)",
                    fontFamily: "var(--font-mono, monospace)",
                    fontSize: "0.9rem",
                  }}
                >
                  <span>{a.name}</span>
                  <span style={{ color: "var(--text-secondary, #64748b)", fontSize: "0.8rem" }}>{a.status}</span>
                </button>
              ))}
            </div>
          </div>
        );
      }
      return (
        <PermissionDashboard
          agentId={permissionAgentId}
          agentName={permAgent?.name ?? "Agent"}
          fuelRemaining={permAgent?.fuel_remaining}
          fuelBudget={permAgent?.fuel_budget ?? 10000}
          memoryUsageBytes={permAgent?.memory_usage_bytes}
          onBack={() => setPage("agents")}
        />
      );
    }
    if (page === "audit") {
      return <Audit events={auditEvents} onRefresh={() => void refreshDesktopData()} />;
    }
    if (page === "workflows") {
      return <Workflows />;
    }
    if (page === "command-center") {
      return <CommandCenter />;
    }
    if (page === "audit-timeline") {
      return <AuditTimeline events={auditEvents} />;
    }
    if (page === "developer-portal") {
      return <DeveloperPortal />;
    }
    if (page === "compliance") {
      return <ComplianceDashboard />;
    }
    if (page === "cluster") {
      return <ClusterStatusPage />;
    }
    if (page === "trust") {
      return <TrustDashboard />;
    }
    if (page === "protocols") {
      return <Protocols />;
    }
    if (page === "distributed-audit") {
      return <DistributedAudit />;
    }
    if (page === "identity") {
      return <Identity agents={agents.map((a) => ({ id: a.id, name: a.name }))} />;
    }
    if (page === "code-editor") {
      return <CodeEditor />;
    }
    if (page === "terminal") {
      return <Terminal />;
    }
    if (page === "file-manager") {
      return <FileManager />;
    }
    if (page === "system-monitor") {
      return <SystemMonitor />;
    }
    if (page === "documents") {
      return <Documents />;
    }
    if (page === "model-hub") {
      return <ModelHub />;
    }
    if (page === "time-machine") {
      return <TimeMachine />;
    }
    if (page === "simulation") {
      return <WorldSimulation />;
    }
    if (page === "notes") {
      return <NotesApp />;
    }
    if (page === "project-manager") {
      return <ProjectManager />;
    }
    if (page === "database") {
      return <DatabaseManager />;
    }
    if (page === "api-client") {
      return <ApiClient />;
    }
    if (page === "design-studio") {
      return <DesignStudio />;
    }
    if (page === "email-client") {
      return <EmailClient />;
    }
    if (page === "messaging") {
      return <Messaging />;
    }
    if (page === "media-studio") {
      return <MediaStudio />;
    }
    if (page === "marketplace" || page === "marketplace-browser" || page === "app-store") {
      return <AppStore />;
    }
    if (page === "ai-chat-hub") {
      return <AiChatHub />;
    }
    if (page === "voice-assistant") {
      return <VoiceAssistant />;
    }
    if (page === "deploy-pipeline") {
      return <DeployPipeline />;
    }
    if (page === "learning-center") {
      return <LearningCenter />;
    }
    if (page === "approvals") {
      return <ApprovalCenter />;
    }
    if (page === "browser") {
      return <AgentBrowser />;
    }
    if (page === "computer-control") {
      return <ComputerControl />;
    }
    if (page === "policy-management") {
      return <PolicyManagement />;
    }
    if (page === "firewall") {
      return <Firewall />;
    }
    return (
      <Settings
        config={config}
        saving={isSavingConfig}
        onChange={setConfig}
        uiSoundEnabled={uiSoundEnabled}
        uiSoundVolume={uiSoundVolume}
        onUiSoundEnabledChange={setUiSoundEnabled}
        onUiSoundVolumeChange={setUiSoundVolume}
        onSave={() => {
          void handleSaveConfig();
        }}
        ollamaConnected={ollamaConnected}
        ollamaModels={ollamaModels}
        onDeleteModel={runtimeMode === "desktop" ? handleDeleteModel : undefined}
        onRerunSetup={() => setShowSetupWizard(true)}
        onRefreshOllama={runtimeMode === "desktop" ? refreshOllamaStatus : undefined}
      />
    );
  }

  return (
    <>
      <NeuralBackground activityPulse={activityPulse} />
      <SplashScreen
        ready={appReady}
        visible={splashVisible}
        onDismiss={() => {
          setSplashVisible(false);
        }}
      />
      <div className="nexus-shell text-slate-100">
        <Sidebar
          items={NAV_ITEMS.map((item) =>
            item.id === "approvals" && pendingApprovalCount > 0
              ? { ...item, badge: pendingApprovalCount }
              : item
          )}
          activeId={page}
          onSelect={(id) => {
            setPage(id as Page);
            play("click");
          }}
          version="v8.0.0"
        />

        <div className="nexus-main-column">
          <header className="nexus-shell-header px-4 py-4 sm:px-6">
            <HoloPanel depth="foreground" className="nexus-topbar">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <p className="nexus-display text-2xl text-cyan-100">Desktop Command Grid</p>
                  <div className="mt-1 flex items-center gap-2 text-xs">
                    <span
                      className={`inline-flex h-2.5 w-2.5 rounded-full ${
                        connectionStatus === "connected" ? "bg-cyan-300 shadow-[0_0_12px_rgba(56,189,248,0.95)]" : "bg-amber-300"
                      }`}
                    />
                    <span className="text-cyan-100/70">
                      {connectionStatus === "connected" ? "Connected to governed kernel backend" : "Mock runtime mode"}
                    </span>
                  </div>
                </div>
                <div className="flex items-center gap-4">
                  <div className="hidden items-center gap-3 md:flex">
                    <span className="font-mono text-xs text-cyan-300 whitespace-nowrap">
                      {sysInfo
                        ? `CPU: ${sysInfo.cpu_usage_percent}% | RAM: ${sysInfo.ram_used_gb}/${sysInfo.ram_total_gb} GB`
                        : "CPU: --% | RAM: --/-- GB"}
                    </span>
                    <PulseRing active={runningAgents > 0} size={44} />
                  </div>
                  <div className="flex flex-wrap items-center gap-2 text-xs">
                    <button
                      onClick={() => {
                        void handleRefresh();
                      }}
                      className="nexus-btn nexus-btn-secondary"
                    >
                      Refresh
                    </button>
                    <button
                      onClick={() => {
                        if (overlay.visible) {
                          void disableJarvisMode();
                          return;
                        }
                        void enableJarvisMode();
                      }}
                      className={`nexus-btn font-semibold ${
                        overlay.visible ? "bg-rose-600/90 text-white hover:bg-rose-500" : "nexus-btn-primary"
                      }`}
                    >
                      {overlay.visible ? "Stop Jarvis" : "Start Jarvis"}
                    </button>
                  </div>
                </div>
              </div>
              <div className="mt-3 flex flex-wrap items-center gap-4 text-xs">
                <span className="text-cyan-100/60">
                  Active agents: <span className="text-cyan-100">{runningAgents}</span>
                </span>
                <span className="text-cyan-100/60">
                  Runtime: <span className="text-cyan-100">{connectionStatus}</span>
                </span>
              </div>
              {backendRestarting ? (
                <div className="nexus-notification mt-3" style={{ background: "rgba(250,204,21,0.15)", borderColor: "#facc15", color: "#facc15", padding: "0.4rem 0.8rem", borderRadius: 6, border: "1px solid", fontSize: "0.8rem" }}>
                  <p className="text-xs">Backend restarting... Reconnecting every 2s.</p>
                </div>
              ) : null}
              {runtimeError && !backendRestarting ? (
                <div className="nexus-notification nexus-notification-error mt-3">
                  <p className="text-xs text-rose-100">{runtimeError}</p>
                </div>
              ) : null}
            </HoloPanel>
          </header>

          <main className="nexus-shell-content px-4 py-4 sm:px-6 sm:py-6">
            <PageTransition pageKey={page}>
              <HoloPanel depth="mid" className="nexus-page-panel">
                {renderPage()}
              </HoloPanel>
            </PageTransition>
          </main>
        </div>
      </div>

      <VoiceOverlay
        state={overlay}
        onDismiss={() => {
          void disableJarvisMode();
        }}
        onTranscript={(text) => {
          setOverlay((prev) => ({ ...prev, transcription: text }));
          setDraft(text);
        }}
      />

      {showSetupWizard && (
        <SetupWizard
          onDetectHardware={async () => {
            if (runtimeMode === "desktop") return detectHardware();
            return {
              gpu: "Mock GPU",
              vram_mb: 8192,
              ram_mb: 16384,
              detected_at: new Date().toISOString(),
              tier: "Medium (8-24GB VRAM)",
              recommended_primary: "qwen3.5:9b",
              recommended_fast: "qwen3.5:4b"
            };
          }}
          onCheckOllama={async (url?: string) => {
            if (runtimeMode === "desktop") return checkOllama(url);
            return { connected: false, base_url: url ?? "http://localhost:11434", models: [] };
          }}
          onEnsureOllama={async () => {
            if (runtimeMode === "desktop") return ensureOllama();
            return false;
          }}
          onIsOllamaInstalled={async () => {
            if (runtimeMode === "desktop") return isOllamaInstalled();
            return false;
          }}
          onPullModel={async (model: string) => {
            if (runtimeMode === "desktop") return pullModel(model);
            return "success";
          }}
          onListAvailableModels={async () => {
            if (runtimeMode === "desktop") return listAvailableModels();
            return [];
          }}
          onSetAgentModel={async (agent: string, model: string) => {
            if (runtimeMode === "desktop") return setAgentModel(agent, model);
          }}
          onComplete={(hw, ollamaStatus) => {
            void handleSetupComplete(hw, ollamaStatus);
          }}
          onSkip={() => {
            setShowSetupWizard(false);
          }}
        />
      )}
    </>
  );
}
