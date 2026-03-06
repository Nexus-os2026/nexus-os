import { useEffect, useMemo, useRef, useState } from "react";
import {
  chatWithOllama,
  checkOllama,
  createAgent,
  deleteModel,
  detectHardware,
  ensureOllama,
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
import { RadialGauge } from "./components/viz/RadialGauge";
import { Agents } from "./pages/Agents";
import { Audit } from "./pages/Audit";
import { Chat } from "./pages/Chat";
import { Marketplace } from "./pages/Marketplace";
import { Settings } from "./pages/Settings";
import { SetupWizard } from "./pages/SetupWizard";
import { Workflows } from "./pages/Workflows";
import CommandCenter from "./pages/CommandCenter";
import AuditTimeline from "./pages/AuditTimeline";
import MarketplaceBrowser from "./pages/MarketplaceBrowser";
import ComplianceDashboard from "./pages/ComplianceDashboard";
import ClusterStatusPage from "./pages/ClusterStatus";
import TrustDashboard from "./pages/TrustDashboard";
import type {
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

type Page = "chat" | "agents" | "audit" | "workflows" | "marketplace" | "settings" | "command-center" | "audit-timeline" | "marketplace-browser" | "compliance" | "cluster" | "trust";
type RuntimeMode = "desktop" | "mock";

const NAV_ITEMS: SidebarItem[] = [
  { id: "chat", label: "Chat", icon: "⌁", shortcut: "Alt+1" },
  { id: "agents", label: "Agents", icon: "⬢", shortcut: "Alt+2" },
  { id: "command-center", label: "Command", icon: "⊞", shortcut: "" },
  { id: "audit", label: "Audit", icon: "⧉", shortcut: "Alt+3" },
  { id: "audit-timeline", label: "Timeline", icon: "⏱", shortcut: "" },
  { id: "workflows", label: "Workflows", icon: "⎇", shortcut: "Alt+4" },
  { id: "marketplace", label: "Marketplace", icon: "◈", shortcut: "Alt+5" },
  { id: "marketplace-browser", label: "Registry", icon: "⬡", shortcut: "" },
  { id: "compliance", label: "Compliance", icon: "⛨", shortcut: "" },
  { id: "cluster", label: "Cluster", icon: "⬣", shortcut: "" },
  { id: "trust", label: "Trust", icon: "◉", shortcut: "" },
  { id: "settings", label: "Settings", icon: "⚙", shortcut: "Alt+6" }
];

function defaultConfig(): NexusConfig {
  return {
    llm: {
      default_model: "claude-sonnet-4-5",
      anthropic_api_key: "",
      openai_api_key: "",
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
    }
  };
}

// Stable mock agent IDs — deterministic UUIDs so all pages reference the same agents
const MOCK_AGENT_IDS = {
  coder: "a0000000-0000-4000-8000-000000000001",
  designer: "a0000000-0000-4000-8000-000000000002",
  screenPoster: "a0000000-0000-4000-8000-000000000003",
  webBuilder: "a0000000-0000-4000-8000-000000000004",
  workflowStudio: "a0000000-0000-4000-8000-000000000005",
  selfImprove: "a0000000-0000-4000-8000-000000000006",
};

const CORE_AGENT_IDS = new Set(Object.values(MOCK_AGENT_IDS));

function coreAgents(): AgentSummary[] {
  return [
    {
      id: MOCK_AGENT_IDS.coder,
      name: "Coder",
      status: "Running",
      fuel_remaining: 9200,
      last_action: "refactored auth middleware",
      isSystem: true
    },
    {
      id: MOCK_AGENT_IDS.designer,
      name: "Designer",
      status: "Running",
      fuel_remaining: 6500,
      last_action: "generated landing page mockup",
      isSystem: true
    },
    {
      id: MOCK_AGENT_IDS.screenPoster,
      name: "Screen Poster",
      status: "Paused",
      fuel_remaining: 4100,
      last_action: "awaiting human approval for X post",
      isSystem: true
    },
    {
      id: MOCK_AGENT_IDS.webBuilder,
      name: "Web Builder",
      status: "Running",
      fuel_remaining: 7800,
      last_action: "deployed staging build v2.4.1",
      isSystem: true
    },
    {
      id: MOCK_AGENT_IDS.workflowStudio,
      name: "Workflow Studio",
      status: "Stopped",
      fuel_remaining: 2300,
      last_action: "completed daily analytics pipeline",
      isSystem: true
    },
    {
      id: MOCK_AGENT_IDS.selfImprove,
      name: "Self-Improve",
      status: "Running",
      fuel_remaining: 8400,
      last_action: "optimized prompt routing latency",
      isSystem: true
    }
  ];
}

function mockAgents(): AgentSummary[] {
  return coreAgents();
}

/** Merge core agents with loaded agents, ensuring core agents are always present */
function ensureCoreAgents(loaded: AgentSummary[]): AgentSummary[] {
  const loadedIds = new Set(loaded.map((a) => a.id));
  const missing = coreAgents().filter((a) => !loadedIds.has(a.id));
  // Mark loaded agents that match core IDs as system agents
  const tagged = loaded.map((a) => CORE_AGENT_IDS.has(a.id) ? { ...a, isSystem: true } : a);
  return [...missing, ...tagged];
}

function mockAudit(): AuditEventRow[] {
  const base = 1_700_100_000;
  const agents = [MOCK_AGENT_IDS.coder, MOCK_AGENT_IDS.designer, MOCK_AGENT_IDS.screenPoster, MOCK_AGENT_IDS.webBuilder, MOCK_AGENT_IDS.workflowStudio, MOCK_AGENT_IDS.selfImprove];
  const events: AuditEventRow[] = [
    { event_id: "evt-01", timestamp: base + 1, agent_id: agents[0], event_type: "StateChange", payload: { state: "Running", trigger: "startup" }, previous_hash: "genesis", hash: "a1b2c3" },
    { event_id: "evt-02", timestamp: base + 12, agent_id: agents[0], event_type: "LlmCall", payload: { model: "claude-sonnet-4-5", tokens: 1840, cost: 0.012 }, previous_hash: "a1b2c3", hash: "d4e5f6" },
    { event_id: "evt-03", timestamp: base + 25, agent_id: agents[1], event_type: "StateChange", payload: { state: "Running", trigger: "scheduler" }, previous_hash: "d4e5f6", hash: "g7h8i9" },
    { event_id: "evt-04", timestamp: base + 38, agent_id: agents[2], event_type: "StateChange", payload: { state: "Running", trigger: "manual" }, previous_hash: "g7h8i9", hash: "j0k1l2" },
    { event_id: "evt-05", timestamp: base + 51, agent_id: agents[1], event_type: "LlmCall", payload: { model: "claude-sonnet-4-5", tokens: 3200, cost: 0.021 }, previous_hash: "j0k1l2", hash: "m3n4o5" },
    { event_id: "evt-06", timestamp: base + 64, agent_id: agents[2], event_type: "ApprovalRequired", payload: { action: "social.post", channel: "x", content: "Product launch teaser" }, previous_hash: "m3n4o5", hash: "p6q7r8" },
    { event_id: "evt-07", timestamp: base + 80, agent_id: agents[3], event_type: "StateChange", payload: { state: "Running", trigger: "webhook" }, previous_hash: "p6q7r8", hash: "s9t0u1" },
    { event_id: "evt-08", timestamp: base + 95, agent_id: agents[0], event_type: "ToolExec", payload: { tool: "file_write", path: "src/auth.rs", bytes: 2480 }, previous_hash: "s9t0u1", hash: "v2w3x4" },
    { event_id: "evt-09", timestamp: base + 110, agent_id: agents[3], event_type: "LlmCall", payload: { model: "claude-sonnet-4-5", tokens: 980, cost: 0.006 }, previous_hash: "v2w3x4", hash: "y5z6a7" },
    { event_id: "evt-10", timestamp: base + 125, agent_id: agents[4], event_type: "StateChange", payload: { state: "Running", trigger: "cron" }, previous_hash: "y5z6a7", hash: "b8c9d0" },
    { event_id: "evt-11", timestamp: base + 140, agent_id: agents[5], event_type: "StateChange", payload: { state: "Running", trigger: "self-schedule" }, previous_hash: "b8c9d0", hash: "e1f2g3" },
    { event_id: "evt-12", timestamp: base + 155, agent_id: agents[4], event_type: "ToolExec", payload: { tool: "sql_query", table: "analytics", rows: 1450 }, previous_hash: "e1f2g3", hash: "h4i5j6" },
    { event_id: "evt-13", timestamp: base + 170, agent_id: agents[5], event_type: "LlmCall", payload: { model: "claude-sonnet-4-5", tokens: 4100, cost: 0.028 }, previous_hash: "h4i5j6", hash: "k7l8m9" },
    { event_id: "evt-14", timestamp: base + 185, agent_id: agents[2], event_type: "ApprovalGranted", payload: { approver: "user", action: "social.post" }, previous_hash: "k7l8m9", hash: "n0o1p2" },
    { event_id: "evt-15", timestamp: base + 200, agent_id: agents[2], event_type: "ToolExec", payload: { tool: "social.publish", platform: "x", post_id: "1823456789" }, previous_hash: "n0o1p2", hash: "q3r4s5" },
    { event_id: "evt-16", timestamp: base + 215, agent_id: agents[0], event_type: "FuelBurn", payload: { consumed: 1200, remaining: 8000 }, previous_hash: "q3r4s5", hash: "t6u7v8" },
    { event_id: "evt-17", timestamp: base + 230, agent_id: agents[3], event_type: "ToolExec", payload: { tool: "deploy", target: "staging", version: "2.4.1" }, previous_hash: "t6u7v8", hash: "w9x0y1" },
    { event_id: "evt-18", timestamp: base + 245, agent_id: agents[5], event_type: "ToolExec", payload: { tool: "benchmark", metric: "p95_latency_ms", before: 320, after: 185 }, previous_hash: "w9x0y1", hash: "z2a3b4" },
    { event_id: "evt-19", timestamp: base + 260, agent_id: agents[4], event_type: "StateChange", payload: { state: "Stopped", trigger: "task-complete" }, previous_hash: "z2a3b4", hash: "c5d6e7" },
    { event_id: "evt-20", timestamp: base + 275, agent_id: agents[1], event_type: "ToolExec", payload: { tool: "image_gen", prompt: "landing hero", format: "webp" }, previous_hash: "c5d6e7", hash: "f8g9h0" },
    { event_id: "evt-21", timestamp: base + 290, agent_id: agents[2], event_type: "StateChange", payload: { state: "Paused", trigger: "rate-limit" }, previous_hash: "f8g9h0", hash: "i1j2k3" },
    { event_id: "evt-22", timestamp: base + 305, agent_id: agents[0], event_type: "LlmCall", payload: { model: "claude-sonnet-4-5", tokens: 2600, cost: 0.017 }, previous_hash: "i1j2k3", hash: "l4m5n6" },
    { event_id: "evt-23", timestamp: base + 320, agent_id: agents[3], event_type: "FuelBurn", payload: { consumed: 800, remaining: 7000 }, previous_hash: "l4m5n6", hash: "o7p8q9" },
    { event_id: "evt-24", timestamp: base + 335, agent_id: agents[5], event_type: "StateChange", payload: { state: "Running", trigger: "optimization-cycle" }, previous_hash: "o7p8q9", hash: "r0s1t2" },
    { event_id: "evt-25", timestamp: base + 350, agent_id: agents[0], event_type: "ToolExec", payload: { tool: "run_tests", suite: "auth", passed: 12, failed: 0 }, previous_hash: "r0s1t2", hash: "u3v4w5" },
    { event_id: "evt-26", timestamp: base + 365, agent_id: agents[0], event_type: "ToolExec", payload: { tool: "fix_bug", file: "src/middleware.rs", line: 88, description: "null check" }, previous_hash: "u3v4w5", hash: "x6y7z8" },
    { event_id: "evt-27", timestamp: base + 380, agent_id: agents[1], event_type: "ToolExec", payload: { tool: "create_tokens", theme: "dark-cyber", tokens: 42 }, previous_hash: "x6y7z8", hash: "a9b0c1" },
    { event_id: "evt-28", timestamp: base + 395, agent_id: agents[2], event_type: "ToolExec", payload: { tool: "track_engagement", post_id: "1823456789", likes: 847, reposts: 123 }, previous_hash: "a9b0c1", hash: "d2e3f4" },
    { event_id: "evt-29", timestamp: base + 410, agent_id: agents[4], event_type: "ToolExec", payload: { tool: "execute_dag", workflow: "daily-analytics", nodes: 6 }, previous_hash: "d2e3f4", hash: "g5h6i7" },
    { event_id: "evt-30", timestamp: base + 425, agent_id: agents[5], event_type: "ToolExec", payload: { tool: "evaluate_performance", metric: "response_quality", score: 0.94 }, previous_hash: "g5h6i7", hash: "j8k9l0" },
    { event_id: "evt-31", timestamp: base + 440, agent_id: agents[5], event_type: "ToolExec", payload: { tool: "optimize_prompt", agent: "coder", improvement: "+12% accuracy" }, previous_hash: "j8k9l0", hash: "m1n2o3" },
    { event_id: "evt-32", timestamp: base + 455, agent_id: agents[3], event_type: "ToolExec", payload: { tool: "generate_site", pages: 4, framework: "astro", status: "complete" }, previous_hash: "m1n2o3", hash: "p4q5r6" },
    { event_id: "evt-33", timestamp: base + 470, agent_id: agents[0], event_type: "ToolExec", payload: { tool: "analyze_architecture", crate: "kernel", modules: 12, issues: 0 }, previous_hash: "p4q5r6", hash: "s7t8u9" },
    { event_id: "evt-34", timestamp: base + 485, agent_id: agents[2], event_type: "ToolExec", payload: { tool: "read_comments", post_id: "1823456789", comments: 34, sentiment: "positive" }, previous_hash: "s7t8u9", hash: "v0w1x2" },
    { event_id: "evt-35", timestamp: base + 500, agent_id: agents[1], event_type: "ToolExec", payload: { tool: "screenshot_analysis", url: "https://staging.nexus-os.dev", issues: ["contrast", "alignment"] }, previous_hash: "v0w1x2", hash: "y3z4a5" },
    { event_id: "evt-36", timestamp: base + 515, agent_id: agents[5], event_type: "ToolExec", payload: { tool: "learn_strategy", context: "code_review", strategy: "chain-of-thought", result: "adopted" }, previous_hash: "y3z4a5", hash: "b6c7d8" }
  ];
  return events;
}

function mockChatReply(message: string): ChatResponse {
  const lowered = message.toLowerCase();
  if (lowered.includes("status")) {
    return {
      text: "Two agents are active in this local mock runtime. Open Agents to inspect fuel and controls.",
      model: "mock-1",
      token_count: 29,
      cost: 0,
      latency_ms: 28
    };
  }
  return {
    text: "Mock runtime is active. Connect through Tauri to call real kernel services and external providers.",
    model: "mock-1",
    token_count: 24,
    cost: 0,
    latency_ms: 22
  };
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
  const [page, setPage] = useState<Page>("chat");
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
  const pushToTalk = useRef<PushToTalk | null>(null);
  const previousPageRef = useRef<Page>(page);
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
      setAgents(mockAgents());
      setAuditEvents(mockAudit());
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
        setAgents(ensureCoreAgents(loadedAgents));
        setAuditEvents(loadedAudit.length > 0 ? loadedAudit : mockAudit());
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
        setAgents(mockAgents());
        setAuditEvents(mockAudit());
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
    };
  }, []);

  useEffect(() => {
    if (previousPageRef.current !== page) {
      previousPageRef.current = page;
      play("transition");
      bumpActivity();
    }
  }, [page, play]);

  const connectionStatus: ConnectionStatus = runtimeMode === "desktop" ? "connected" : "mock";
  const runningAgents = useMemo(
    () => agents.filter((agent) => agent.status === "Running").length,
    [agents]
  );
  const averageFuel = useMemo(() => {
    if (agents.length === 0) {
      return 0;
    }
    const total = agents.reduce((sum, agent) => sum + Math.max(0, Math.min(100, agent.fuel_remaining / 100)), 0);
    return total / agents.length;
  }, [agents]);

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
    const [loadedAgents, loadedAudit] = await Promise.all([listAgents(), getAuditLog(undefined, 500)]);
    setAgents(ensureCoreAgents(loadedAgents));
    setAuditEvents(loadedAudit.length > 0 ? loadedAudit : mockAudit());
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
        makeMessage("assistant", `Agent created: ${agentId}`, { model: "system" })
      ]);
      play("success");
      bumpActivity();
    } catch (error) {
      setRuntimeError(`Unable to create agent: ${formatError(error)}`);
      play("error");
    }
  }

  function handleDeleteAgent(id: string): void {
    // Never allow deleting core system agents
    if (CORE_AGENT_IDS.has(id)) {
      return;
    }
    setAgents((prev) => prev.filter((a) => a.id !== id));
    play("click");
    bumpActivity();
  }

  const AGENT_PROMPTS: Record<string, string> = {
    "": "You are NexusOS, a governed AI operating system. You help users with coding, design, automation, and content. Be concise and helpful.",
    [MOCK_AGENT_IDS.coder]: "You are the NexusOS Coder Agent. You write clean code in Rust, TypeScript, and Python. You analyze architecture, review code, fix bugs, and run tests. Show code in fenced blocks.",
    [MOCK_AGENT_IDS.designer]: "You are the NexusOS Designer Agent. You create UI components, design systems, and design tokens. Output React/TypeScript.",
    [MOCK_AGENT_IDS.screenPoster]: "You are the NexusOS Screen Poster Agent. You draft social media posts for X, Instagram, Facebook, Reddit. Optimize for engagement.",
    [MOCK_AGENT_IDS.webBuilder]: "You are the NexusOS Web Builder Agent. You generate websites from descriptions using React and modern web tech.",
    [MOCK_AGENT_IDS.workflowStudio]: "You are the NexusOS Workflow Studio Agent. You design automation pipelines with DAG nodes, retries, and checkpoints.",
    [MOCK_AGENT_IDS.selfImprove]: "You are the NexusOS Self-Improve Agent. You analyze performance metrics and optimize prompts.",
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
      // REAL Ollama streaming chat
      const systemPrompt = AGENT_PROMPTS[selectedAgent] || AGENT_PROMPTS[""];
      const apiMessages = [
        { role: "system", content: systemPrompt },
        ...messages.filter(m => m.role === "user" || m.role === "assistant").slice(-20).map(m => ({
          role: m.role,
          content: m.content
        })),
        { role: "user", content: input }
      ];

      // Listen for streaming tokens
      let unlisten: (() => void) | undefined;
      let fullText = "";
      try {
        const eventMod = await import("@tauri-apps/api/event");
        unlisten = await eventMod.listen<ChatTokenEvent>("chat-token", (event) => {
          const { full, done } = event.payload;
          fullText = full;

          if (done) {
            // Final setState ONCE when streaming is complete
            setMessages((prev) =>
              prev.map((m) =>
                m.id === assistantId ? { ...m, content: fullText, streaming: false } : m
              )
            );
          } else {
            // Update content via setState — throttled at 50ms on backend side
            setMessages((prev) =>
              prev.map((m) =>
                m.id === assistantId ? { ...m, content: full } : m
              )
            );
          }
        });

        await chatWithOllama(apiMessages, model);
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
      // Mock mode fallback
      try {
        const response = mockChatReply(input);
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
      setAgents(mockAgents());
      setAuditEvents(mockAudit());
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
    if (page === "chat") {
      return (
        <Chat
          messages={messages}
          draft={draft}
          isRecording={isRecording}
          isSending={isSending}
          agents={agents}
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
        />
      );
    }
    if (page === "audit") {
      return <Audit events={auditEvents} />;
    }
    if (page === "workflows") {
      return <Workflows />;
    }
    if (page === "marketplace") {
      return <Marketplace />;
    }
    if (page === "command-center") {
      return <CommandCenter />;
    }
    if (page === "audit-timeline") {
      return <AuditTimeline />;
    }
    if (page === "marketplace-browser") {
      return <MarketplaceBrowser />;
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
          items={NAV_ITEMS}
          activeId={page}
          onSelect={(id) => {
            setPage(id as Page);
            play("click");
          }}
          version="v5.0.0"
        />

        <div className="flex min-h-screen flex-1 flex-col">
          <header className="px-4 py-4 sm:px-6">
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
                    <RadialGauge value={averageFuel} label="Avg Fuel" size={88} />
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
              {runtimeError ? (
                <div className="nexus-notification nexus-notification-error mt-3">
                  <p className="text-xs text-rose-100">{runtimeError}</p>
                </div>
              ) : null}
            </HoloPanel>
          </header>

          <div className="flex-1 px-4 py-4 sm:px-6 sm:py-6">
            <PageTransition pageKey={page}>
              <HoloPanel depth="mid" className="min-h-[calc(100vh-11.5rem)]">
                {renderPage()}
              </HoloPanel>
            </PageTransition>
          </div>
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
