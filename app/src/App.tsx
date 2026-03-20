import React, { Suspense, useEffect, useMemo, useRef, useState } from "react";
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
import PageErrorBoundary from "./components/PageErrorBoundary";
import { VoiceOverlay, type VoiceOverlayState } from "./components/VoiceOverlay";
import { PulseRing } from "./components/viz/PulseRing";
import LivingBackground from "./components/LivingBackground";
import type { ConsentNotification, SystemInfo } from "./types";
import { Agents } from "./pages/Agents";
import { Audit } from "./pages/Audit";
import { Chat } from "./pages/Chat";
import Dashboard from "./pages/Dashboard";
import { Settings } from "./pages/Settings";
import { SetupWizard } from "./pages/SetupWizard";
import { Workflows } from "./pages/Workflows";
const CommandCenter = React.lazy(() => import("./pages/CommandCenter"));
const AuditTimeline = React.lazy(() => import("./pages/AuditTimeline"));
const ComplianceDashboard = React.lazy(() => import("./pages/ComplianceDashboard"));
const ClusterStatusPage = React.lazy(() => import("./pages/ClusterStatus"));
const TrustDashboard = React.lazy(() => import("./pages/TrustDashboard"));
const DistributedAudit = React.lazy(() => import("./pages/DistributedAudit"));
const PermissionDashboard = React.lazy(() => import("./pages/PermissionDashboard").then(m => ({ default: m.PermissionDashboard })));
const Protocols = React.lazy(() => import("./pages/Protocols"));
const Identity = React.lazy(() => import("./pages/Identity"));
const Firewall = React.lazy(() => import("./pages/Firewall"));
const DeveloperPortal = React.lazy(() => import("./pages/DeveloperPortal"));
const AgentBrowser = React.lazy(() => import("./pages/AgentBrowser").then(m => ({ default: m.AgentBrowser })));
const CodeEditor = React.lazy(() => import("./pages/CodeEditor"));
const Terminal = React.lazy(() => import("./pages/Terminal"));
const FileManager = React.lazy(() => import("./pages/FileManager"));
const SystemMonitor = React.lazy(() => import("./pages/SystemMonitor"));
const NotesApp = React.lazy(() => import("./pages/NotesApp"));
const ProjectManager = React.lazy(() => import("./pages/ProjectManager"));
const DatabaseManager = React.lazy(() => import("./pages/DatabaseManager"));
const ApiClient = React.lazy(() => import("./pages/ApiClient"));
const DesignStudio = React.lazy(() => import("./pages/DesignStudio"));
const EmailClient = React.lazy(() => import("./pages/EmailClient"));
const MediaStudio = React.lazy(() => import("./pages/MediaStudio"));
const Messaging = React.lazy(() => import("./pages/Messaging"));
const AppStore = React.lazy(() => import("./pages/AppStore"));
const AiChatHub = React.lazy(() => import("./pages/AiChatHub"));
const DeployPipeline = React.lazy(() => import("./pages/DeployPipeline"));
const LearningCenter = React.lazy(() => import("./pages/LearningCenter"));
const ApprovalCenter = React.lazy(() => import("./pages/ApprovalCenter"));
const PolicyManagement = React.lazy(() => import("./pages/PolicyManagement"));
const Documents = React.lazy(() => import("./pages/Documents"));
const ModelHub = React.lazy(() => import("./pages/ModelHub"));
const TimeMachine = React.lazy(() => import("./pages/TimeMachine"));
const VoiceAssistant = React.lazy(() => import("./pages/VoiceAssistant"));
const WorldSimulation = React.lazy(() => import("./pages/WorldSimulation"));
const ComputerControl = React.lazy(() => import("./pages/ComputerControl"));
const MissionControl = React.lazy(() => import("./pages/MissionControl"));
const AgentDnaLab = React.lazy(() => import("./pages/AgentDnaLab"));
const TimelineViewer = React.lazy(() => import("./pages/TimelineViewer"));
const KnowledgeGraph = React.lazy(() => import("./pages/KnowledgeGraph"));
const ImmuneDashboard = React.lazy(() => import("./pages/ImmuneDashboard"));
const ConsciousnessMonitor = React.lazy(() => import("./pages/ConsciousnessMonitor"));
const DreamForge = React.lazy(() => import("./pages/DreamForge"));
const TemporalEngine = React.lazy(() => import("./pages/TemporalEngine"));
const CivilizationPage = React.lazy(() => import("./pages/Civilization"));
const SelfRewriteLab = React.lazy(() => import("./pages/SelfRewriteLab"));
const AdminDashboard = React.lazy(() => import("./pages/AdminDashboard"));
const AdminUsers = React.lazy(() => import("./pages/AdminUsers"));
const AdminFleet = React.lazy(() => import("./pages/AdminFleet"));
const AdminPolicyEditor = React.lazy(() => import("./pages/AdminPolicyEditor"));
const AdminCompliance = React.lazy(() => import("./pages/AdminCompliance"));
const AdminSystemHealth = React.lazy(() => import("./pages/AdminSystemHealth"));
const Integrations = React.lazy(() => import("./pages/Integrations"));
const Login = React.lazy(() => import("./pages/Login"));
const Workspaces = React.lazy(() => import("./pages/Workspaces"));
const Telemetry = React.lazy(() => import("./pages/Telemetry"));
const UsageBilling = React.lazy(() => import("./pages/UsageBilling"));
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
import { createDefaultConfig, normalizeConfig } from "./utils/config";
import { PushToTalk } from "./voice/PushToTalk";

type Page = "dashboard" | "chat" | "agents" | "audit" | "workflows" | "marketplace" | "settings" | "command-center" | "audit-timeline" | "marketplace-browser" | "developer-portal" | "compliance" | "cluster" | "trust" | "distributed-audit" | "permissions" | "protocols" | "identity" | "firewall" | "browser" | "computer-control" | "code-editor" | "terminal" | "file-manager" | "system-monitor" | "notes" | "project-manager" | "database" | "api-client" | "design-studio" | "email-client" | "messaging" | "media-studio" | "app-store" | "ai-chat-hub" | "deploy-pipeline" | "learning-center" | "policy-management" | "documents" | "model-hub" | "time-machine" | "voice-assistant" | "approvals" | "simulation" | "mission-control" | "dna-lab" | "timeline-viewer" | "knowledge-graph" | "immune-dashboard" | "consciousness" | "dreams" | "temporal" | "civilization" | "self-rewrite" | "admin-console" | "admin-users" | "admin-fleet" | "admin-policies" | "admin-compliance" | "admin-health" | "integrations" | "login" | "workspaces" | "telemetry" | "usage-billing";
type RuntimeMode = "desktop" | "mock";

const NAV_ITEMS: SidebarItem[] = [
  // ── CORE ──
  { id: "chat", label: "Chat", icon: "MessageSquare", shortcut: "Alt+1", section: "CORE" },
  { id: "agents", label: "Agents", icon: "Users", shortcut: "Alt+2", section: "CORE" },
  { id: "command-center", label: "Command", icon: "Terminal", shortcut: "", section: "CORE" },
  { id: "audit", label: "Audit", icon: "Shield", shortcut: "", section: "CORE" },
  { id: "audit-timeline", label: "Timeline", icon: "Clock", shortcut: "", section: "CORE" },
  { id: "time-machine", label: "Time Machine", icon: "History", shortcut: "", section: "CORE" },
  // ── INTELLIGENCE ──
  { id: "mission-control", label: "Mission Control", icon: "LayoutDashboard", shortcut: "Alt+3", section: "INTELLIGENCE" },
  { id: "dna-lab", label: "DNA Lab", icon: "Dna", shortcut: "", section: "INTELLIGENCE" },
  { id: "consciousness", label: "Consciousness", icon: "Brain", shortcut: "", section: "INTELLIGENCE" },
  { id: "dreams", label: "Dream Forge", icon: "Moon", shortcut: "", section: "INTELLIGENCE" },
  { id: "temporal", label: "Temporal Engine", icon: "GitBranch", shortcut: "", section: "INTELLIGENCE" },
  { id: "timeline-viewer", label: "Timeline", icon: "GitMerge", shortcut: "", section: "INTELLIGENCE" },
  // ── SECURITY ──
  { id: "immune-dashboard", label: "Immune System", icon: "ShieldCheck", shortcut: "", section: "SECURITY" },
  { id: "identity", label: "Identity & Mesh", icon: "Fingerprint", shortcut: "", section: "SECURITY" },
  { id: "firewall", label: "Firewall", icon: "Lock", shortcut: "", section: "SECURITY" },
  { id: "computer-control", label: "Computer Control", icon: "Monitor", shortcut: "", section: "SECURITY" },
  // ── KNOWLEDGE ──
  { id: "knowledge-graph", label: "Knowledge Graph", icon: "Network", shortcut: "", section: "KNOWLEDGE" },
  { id: "civilization", label: "Civilization", icon: "Landmark", shortcut: "", section: "KNOWLEDGE" },
  { id: "self-rewrite", label: "Self-Rewrite Lab", icon: "Code2", shortcut: "", section: "KNOWLEDGE" },
  // ── WORKFLOWS ──
  { id: "workflows", label: "Workflows", icon: "Workflow", shortcut: "", section: "WORKFLOWS" },
  { id: "marketplace", label: "Publish", icon: "Upload", shortcut: "", section: "WORKFLOWS" },
  { id: "marketplace-browser", label: "Browse Agents", icon: "Search", shortcut: "", section: "WORKFLOWS" },
  // ── GOVERNANCE ──
  { id: "trust", label: "Trust", icon: "Award", shortcut: "", section: "GOVERNANCE" },
  { id: "compliance", label: "Compliance", icon: "ShieldCheck", shortcut: "", section: "GOVERNANCE" },
  { id: "distributed-audit", label: "Chain", icon: "Link", shortcut: "", section: "GOVERNANCE" },
  { id: "protocols", label: "Protocols", icon: "Layers", shortcut: "", section: "GOVERNANCE" },
  { id: "permissions", label: "Permissions", icon: "Key", shortcut: "", section: "GOVERNANCE" },
  { id: "approvals", label: "Approvals", icon: "CheckCircle", shortcut: "", section: "GOVERNANCE" },
  { id: "policy-management", label: "Policies", icon: "Scale", shortcut: "", section: "GOVERNANCE" },
  // ── TOOLS ──
  { id: "api-client", label: "API Client", icon: "Zap", shortcut: "", section: "TOOLS" },
  { id: "design-studio", label: "Design", icon: "Palette", shortcut: "", section: "TOOLS" },
  { id: "email-client", label: "Email", icon: "Mail", shortcut: "", section: "TOOLS" },
  { id: "media-studio", label: "Media", icon: "Play", shortcut: "", section: "TOOLS" },
  { id: "app-store", label: "Agent Store", icon: "Store", shortcut: "", section: "TOOLS" },
  { id: "ai-chat-hub", label: "AI Chat", icon: "Bot", shortcut: "", section: "TOOLS" },
  { id: "voice-assistant", label: "Voice", icon: "Mic", shortcut: "", section: "TOOLS" },
  { id: "deploy-pipeline", label: "Deploy", icon: "Rocket", shortcut: "", section: "TOOLS" },
  { id: "learning-center", label: "Learn", icon: "BookOpen", shortcut: "", section: "TOOLS" },
  { id: "code-editor", label: "Code", icon: "FileCode", shortcut: "", section: "TOOLS" },
  { id: "terminal", label: "Terminal", icon: "TerminalSquare", shortcut: "", section: "TOOLS" },
  { id: "file-manager", label: "Files", icon: "FolderOpen", shortcut: "", section: "TOOLS" },
  { id: "database", label: "Database", icon: "Database", shortcut: "", section: "TOOLS" },
  { id: "browser", label: "Browser", icon: "Globe", shortcut: "", section: "TOOLS" },
  { id: "messaging", label: "Messaging", icon: "MessageCircle", shortcut: "", section: "TOOLS" },
  { id: "simulation", label: "World Sim", icon: "Globe2", shortcut: "", section: "TOOLS" },
  { id: "documents", label: "Documents", icon: "FileText", shortcut: "", section: "TOOLS" },
  { id: "model-hub", label: "Models", icon: "Cpu", shortcut: "", section: "TOOLS" },
  { id: "notes", label: "Notes", icon: "StickyNote", shortcut: "", section: "TOOLS" },
  { id: "project-manager", label: "Projects", icon: "Kanban", shortcut: "", section: "TOOLS" },
  { id: "system-monitor", label: "Monitor", icon: "Activity", shortcut: "", section: "TOOLS" },
  // ── ADMIN ──
  { id: "admin-console", label: "Admin", icon: "ShieldAlert", shortcut: "", section: "ADMIN" },
  { id: "admin-users", label: "Users", icon: "UserCog", shortcut: "", section: "ADMIN" },
  { id: "admin-fleet", label: "Fleet", icon: "Boxes", shortcut: "", section: "ADMIN" },
  { id: "admin-policies", label: "Policies", icon: "ScrollText", shortcut: "", section: "ADMIN" },
  { id: "admin-compliance", label: "Compliance", icon: "ClipboardCheck", shortcut: "", section: "ADMIN" },
  { id: "admin-health", label: "Health", icon: "HeartPulse", shortcut: "", section: "ADMIN" },
  { id: "cluster", label: "Cluster", icon: "Server", shortcut: "", section: "ADMIN" },
  // ── DEVELOPER ──
  { id: "developer-portal", label: "Developer", icon: "Code", shortcut: "", section: "DEVELOPER" },
  // ── INTEGRATIONS ──
  { id: "integrations", label: "Integrations", icon: "PlugZap", shortcut: "", section: "INTEGRATIONS" },
  // ── ENTERPRISE ──
  { id: "login", label: "Auth / Sessions", icon: "LogIn", shortcut: "", section: "ENTERPRISE" },
  { id: "workspaces", label: "Workspaces", icon: "Building2", shortcut: "", section: "ENTERPRISE" },
  { id: "telemetry", label: "Telemetry", icon: "BarChart3", shortcut: "", section: "ENTERPRISE" },
  { id: "usage-billing", label: "Usage & Billing", icon: "Receipt", shortcut: "", section: "ENTERPRISE" },
  // ── SYSTEM ──
  { id: "dashboard", label: "System Dash", icon: "Gauge", shortcut: "", section: "SYSTEM" },
  { id: "settings", label: "Settings", icon: "Settings", shortcut: "", section: "SYSTEM" },
];

const PAGE_ROUTE_OVERRIDES: Partial<Record<Page, string>> = {
  "mission-control": "/dashboard",
  "dna-lab": "/dna-lab",
  consciousness: "/consciousness",
  dreams: "/dreams",
  temporal: "/temporal",
  "immune-dashboard": "/immune",
  identity: "/identity",
  firewall: "/firewall",
  "computer-control": "/computer-control",
  "knowledge-graph": "/knowledge",
  civilization: "/civilization",
  "self-rewrite": "/self-rewrite",
  chat: "/chat",
  agents: "/agents",
  "command-center": "/command",
  audit: "/audit",
  "audit-timeline": "/timeline",
  "time-machine": "/time-machine",
  workflows: "/workflows",
  marketplace: "/publish",
  trust: "/trust",
  "distributed-audit": "/chain",
  protocols: "/protocols",
  permissions: "/permissions",
  approvals: "/approvals",
  "policy-management": "/policies",
  "design-studio": "/design",
  "email-client": "/email",
  "media-studio": "/media",
  "app-store": "/agent-store",
  "ai-chat-hub": "/ai-chat",
  "voice-assistant": "/voice",
  "deploy-pipeline": "/deploy",
  "learning-center": "/learn",
  "code-editor": "/code",
  terminal: "/terminal",
  "file-manager": "/files",
  "system-monitor": "/monitor",
  documents: "/documents",
  "model-hub": "/models",
  notes: "/notes",
  "project-manager": "/projects",
  database: "/database",
  browser: "/browser",
  messaging: "/messaging",
  simulation: "/world-simulation",
  settings: "/settings",
  dashboard: "/legacy-dashboard",
};

const ROUTE_TO_PAGE = new Map<string, Page>(
  Object.entries(PAGE_ROUTE_OVERRIDES).map(([page, route]) => [route, page as Page])
);

function pageFromLocation(pathname: string): Page | null {
  const normalized = pathname.replace(/\/+$/, "") || "/";
  if (normalized === "/" || normalized.endsWith("/index.html")) {
    return "mission-control";
  }
  return ROUTE_TO_PAGE.get(normalized) ?? null;
}

function routeForPage(page: Page): string {
  return PAGE_ROUTE_OVERRIDES[page] ?? `/${page}`;
}

const PAGE_SUMMARIES: Partial<Record<Page, string>> = {
  "mission-control": "System core telemetry, agent constellation, and live governance signals.",
  chat: "Conversational control layer for routing directives through the Nexus runtime.",
  agents: "Entity grid for supervising autonomous agents, permissions, and runtime health.",
  "dna-lab": "Evolution bay for breeding, comparing, and mutating agent genomes.",
  settings: "Control panel for runtime policy, providers, privacy posture, and system tuning.",
  "audit-timeline": "Trace temporal events, decisions, and governance history across the mesh.",
  "command-center": "Run direct commands against the governed operating layer.",
  approvals: "Resolve human-in-the-loop requests before protected actions execute.",
};

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

// ── Demo-mode fallback data ────────────────────────────────────────
// These are shown ONLY in browser demo mode (no Tauri desktop backend).
// A prominent "DEMO MODE" banner is displayed whenever these are active.

const DEMO_AGENT_IDS = {
  coder: "a0000000-0000-4000-8000-000000000001",
  designer: "a0000000-0000-4000-8000-000000000002",
  screenPoster: "a0000000-0000-4000-8000-000000000003",
  webBuilder: "a0000000-0000-4000-8000-000000000004",
  workflowStudio: "a0000000-0000-4000-8000-000000000005",
  selfImprove: "a0000000-0000-4000-8000-000000000006",
};

const DEMO_AGENT_ID_SET = new Set(Object.values(DEMO_AGENT_IDS));

function demoAgents(): AgentSummary[] {
  return [
    {
      id: DEMO_AGENT_IDS.coder,
      name: "Coder (demo)",
      status: "Running",
      autonomy_level: 3,
      fuel_remaining: 9200,
      fuel_budget: 10000,
      last_action: "demo — no backend connected",
      isSystem: true,
      sandbox_runtime: "wasmtime",
      memory_usage_bytes: 131072,
      capabilities: ["llm.query", "fs.read", "fs.write"]
    },
    {
      id: DEMO_AGENT_IDS.designer,
      name: "Designer (demo)",
      status: "Running",
      autonomy_level: 2,
      fuel_remaining: 6500,
      fuel_budget: 10000,
      last_action: "demo — no backend connected",
      isSystem: true,
      sandbox_runtime: "wasmtime",
      memory_usage_bytes: 98304,
      capabilities: ["llm.query", "fs.read"]
    },
    {
      id: DEMO_AGENT_IDS.screenPoster,
      name: "Screen Poster (demo)",
      status: "Paused",
      autonomy_level: 2,
      fuel_remaining: 4100,
      fuel_budget: 10000,
      last_action: "demo — awaiting HITL approval",
      isSystem: true,
      sandbox_runtime: "wasmtime",
      memory_usage_bytes: 65536,
      capabilities: ["llm.query", "fs.read", "request_approval"]
    },
    {
      id: DEMO_AGENT_IDS.webBuilder,
      name: "Web Builder (demo)",
      status: "Running",
      autonomy_level: 3,
      fuel_remaining: 7800,
      fuel_budget: 10000,
      last_action: "demo — no backend connected",
      isSystem: true,
      sandbox_runtime: "wasmtime",
      memory_usage_bytes: 196608,
      capabilities: ["llm.query", "fs.read", "fs.write"]
    },
    {
      id: DEMO_AGENT_IDS.workflowStudio,
      name: "Workflow Studio (demo)",
      status: "Stopped",
      autonomy_level: 3,
      fuel_remaining: 2300,
      fuel_budget: 10000,
      last_action: "demo — pipeline idle",
      isSystem: true,
      sandbox_runtime: "wasmtime",
      memory_usage_bytes: 0,
      capabilities: ["llm.query", "fs.read"]
    },
    {
      id: DEMO_AGENT_IDS.selfImprove,
      name: "Self-Improve (demo)",
      status: "Running",
      autonomy_level: 4,
      fuel_remaining: 8400,
      fuel_budget: 10000,
      last_action: "demo — no backend connected",
      isSystem: true,
      sandbox_runtime: "wasmtime",
      memory_usage_bytes: 114688,
      capabilities: ["llm.query", "fs.read", "fs.write", "request_approval"]
    }
  ];
}

function demoChatReply(message: string): ChatResponse {
  const lowered = message.toLowerCase();
  let text: string;
  if (lowered.includes("status")) {
    text = "[DEMO] 6 demo agents shown. 4 running, 1 paused, 1 stopped. Install the Nexus OS desktop app for real agent data.";
  } else if (lowered.includes("hello") || lowered.includes("hi") || lowered.includes("hey")) {
    text = "[DEMO] Hello! This is the Nexus OS demo preview. You are seeing placeholder data — install the desktop app for full governed chat, agent management, and all runtime features.";
  } else if (lowered.includes("help") || lowered.includes("what can you")) {
    text = "[DEMO] Nexus OS capabilities include: governed chat, agent management, code execution, web search, social posting, file system access, and workflow orchestration. All features require the desktop backend — install it for full functionality.";
  } else {
    text = "[DEMO] This is a demo preview. Chat responses, agent data, and all actions are simulated. Install the Nexus OS desktop app to connect to the governed runtime.";
  }
  return { text, model: "demo", token_count: text.split(" ").length, cost: 0, latency_ms: 10 };
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function emptyAudit(): AuditEventRow[] {
  return [];
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

export default function App(): JSX.Element {
  const [page, setPage] = useState<Page>(() => {
    if (typeof window === "undefined") return "mission-control";
    return pageFromLocation(window.location.pathname) ?? "mission-control";
  });
  const [runtimeMode, setRuntimeMode] = useState<RuntimeMode>("mock");
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [auditEvents, setAuditEvents] = useState<AuditEventRow[]>([]);
  const [config, setConfig] = useState<NexusConfig>(createDefaultConfig);
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
  const routeSyncRef = useRef(false);
  const uniqueAgents = useMemo(() => dedupeAgentsById(agents), [agents]);
  const { enabled: uiSoundEnabled, volume: uiSoundVolume, setEnabled: setUiSoundEnabled, setVolume: setUiSoundVolume, play } =
    useUiAudio();

  function bumpActivity(): void {
    setActivityPulse((previous) => previous + 1);
  }

  useEffect(() => {
    try {
      pushToTalk.current = new PushToTalk();
    } catch (error) {
      setRuntimeError(`Voice controls unavailable: ${formatError(error)}`);
    }

    return () => {
      pushToTalk.current = null;
    };
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const route = routeForPage(page);
    if (window.location.pathname !== route) {
      if (routeSyncRef.current) {
        window.history.pushState({ page }, "", route);
      } else {
        window.history.replaceState({ page }, "", route);
      }
    }
    routeSyncRef.current = true;
  }, [page]);

  useEffect(() => {
    if (typeof window === "undefined") return undefined;
    const handlePopState = () => {
      const next = pageFromLocation(window.location.pathname);
      if (next) {
        setPage(next);
      }
    };
    window.addEventListener("popstate", handlePopState);
    return () => window.removeEventListener("popstate", handlePopState);
  }, []);

  useEffect(() => {
    if (!hasDesktopRuntime()) {
      setRuntimeMode("mock");
      setAgents(demoAgents());
      setAuditEvents(emptyAudit());
      setConfig(createDefaultConfig());
      setMessages([
        makeMessage(
          "assistant",
          "[DEMO MODE] You are viewing a demo preview of Nexus OS. Agent data is simulated and actions are disabled. Install the desktop app for full functionality."
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
        const normalizedConfig = normalizeConfig(loadedConfig);
        if (cancelled) {
          return;
        }
        setRuntimeMode("desktop");
        setRuntimeError(null);
        setAgents(loadedAgents);
        setAuditEvents(loadedAudit);
        setConfig(normalizedConfig);
        applyVoiceState(voice);

        // Refresh Ollama status in background
        checkOllama().then((status) => {
          if (!cancelled) {
            setOllamaConnected(status.connected);
            setOllamaModels(status.models);
          }
        }).catch(() => {});

        // Check if first-run setup is needed
        const needsSetup =
          !normalizedConfig.hardware?.gpu || normalizedConfig.hardware.gpu.length === 0;
        if (needsSetup) {
          setShowSetupWizard(true);
        }

        setMessages([
          makeMessage(
            "assistant",
            `Connected to desktop backend. Default model: ${normalizedConfig.llm.default_model || "mock-1"}.`
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
        setAgents(demoAgents());
        setAuditEvents(emptyAudit());
        setConfig(createDefaultConfig());
        setMessages([
          makeMessage("assistant", "[DEMO MODE] Backend connection failed. Showing demo data. Restart the desktop app and refresh to reconnect.")
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
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    const registerListener = async (): Promise<void> => {
      try {
        const mod = await import("@tauri-apps/api/event");
        if (cancelled) {
          return;
        }
        unlisten = await mod.listen<AgentStatusEvent>("agent-status-changed", (event) => {
          const { agent_id, status, fuel_remaining } = event.payload;
          setAgents((prev) =>
            prev.map((a) =>
              a.id === agent_id
                ? { ...a, status: status as AgentSummary["status"], fuel_remaining }
                : a
            )
          );
        });
      } catch {
        // Real-time updates are optional; fail closed instead of crashing startup.
      }
    };

    void registerListener();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // Global listener for consent-request-pending — fires on ALL pages
  useEffect(() => {
    if (!hasDesktopRuntime()) return;
    const cleanups: (() => void)[] = [];
    let cancelled = false;

    const registerListeners = async (): Promise<void> => {
      try {
        if (typeof Notification !== "undefined" && Notification.permission === "default") {
          void Notification.requestPermission().catch(() => {});
        }
      } catch {
        // Ignore notification permission failures.
      }

      try {
        const mod = await import("@tauri-apps/api/event");
        if (cancelled) {
          return;
        }

        const pendingCleanup = await mod.listen<ConsentNotification>(
          "consent-request-pending",
          (event) => {
            setPendingApprovalCount((prev) => prev + 1);
            play("notification");

            try {
              if (typeof Notification !== "undefined" && Notification.permission === "granted") {
                new Notification("Nexus OS — Agent Approval Required", {
                  body: `${event.payload.agent_name} wants to: ${event.payload.operation_summary}`,
                  tag: `consent-${event.payload.consent_id}`,
                });
              }
            } catch {
              // Ignore desktop notification failures.
            }
          },
        );
        cleanups.push(pendingCleanup);

        const resolvedCleanup = await mod.listen<{ consent_id: string; status: string }>(
          "consent-resolved",
          () => {
            setPendingApprovalCount((prev) => Math.max(0, prev - 1));
          },
        );
        cleanups.push(resolvedCleanup);
      } catch {
        // Consent listeners are optional; do not crash the shell if the event bridge is unavailable.
      }
    };

    void registerListeners();

    return () => {
      cancelled = true;
      for (const fn of cleanups) {
        fn();
      }
    };
  }, [play]);

  useEffect(() => {
    if (previousPageRef.current !== page) {
      previousPageRef.current = page;
      play("transition");
      bumpActivity();
    }
  }, [page, play]);

  useEffect(() => {
    if (page !== "chat") {
      return;
    }

    const pendingAgent = sessionStorage.getItem("nexus-chat-agent");
    if (!pendingAgent) {
      return;
    }

    setSelectedAgent(pendingAgent);
    sessionStorage.removeItem("nexus-chat-agent");
  }, [page]);

  const connectionStatus: ConnectionStatus = runtimeMode === "desktop" ? "connected" : "mock";
  const runningAgents = useMemo(
    () => uniqueAgents.filter((agent) => agent.status === "Running").length,
    [uniqueAgents]
  );
  const activePageLabel = NAV_ITEMS.find((item) => item.id === page)?.label ?? "Nexus OS";
  const activePageSummary = PAGE_SUMMARIES[page] ?? "Navigate the Nexus AI operating system and monitor its living runtime.";
  const [sysInfo, setSysInfo] = useState<SystemInfo | null>(null);

  useEffect(() => {
    if (!hasDesktopRuntime()) {
      setSysInfo(null);
      return;
    }

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

  function showDemoToast(): void {
    setRuntimeError("Action unavailable in demo mode \u2014 requires desktop backend");
    play("error");
    // Auto-clear after 3s
    setTimeout(() => setRuntimeError((prev) => prev === "Action unavailable in demo mode \u2014 requires desktop backend" ? null : prev), 3000);
  }

  async function handleStartAgent(id: string): Promise<void> {
    if (runtimeMode !== "desktop") {
      showDemoToast();
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
      showDemoToast();
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
      showDemoToast();
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
      showDemoToast();
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
      showDemoToast();
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
      showDemoToast();
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
    [DEMO_AGENT_IDS.coder]: "You are the NexusOS Coder Agent. You write clean code in Rust, TypeScript, and Python. You analyze architecture, review code, fix bugs, and run tests. Show code in fenced blocks.",
    [DEMO_AGENT_IDS.designer]: "You are the NexusOS Designer Agent. You create UI components, design systems, and design tokens. Output React/TypeScript.",
    [DEMO_AGENT_IDS.screenPoster]: "You are the NexusOS Screen Poster Agent. You draft social media posts for X, Instagram, Facebook, Reddit. Optimize for engagement.",
    [DEMO_AGENT_IDS.webBuilder]: "You are the NexusOS Web Builder Agent. You generate websites from descriptions using React and modern web tech.",
    [DEMO_AGENT_IDS.workflowStudio]: "You are the NexusOS Workflow Studio Agent. You design automation pipelines with DAG nodes, retries, and checkpoints.",
    [DEMO_AGENT_IDS.selfImprove]: "You are the NexusOS Self-Improve Agent. You analyze performance metrics and optimize prompts.",
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
      // Cognitive loop disabled — route all messages through LLM chat for now.
      // To re-enable agent goal execution, uncomment and restore the isRealAgent block.
      const isRealAgent = false;
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

      // Use agent description as system prompt when a real agent is selected
      const agentDesc = agents.find((a) => a.id === selectedAgent)?.description;
      const systemPrompt = agentDesc || AGENT_PROMPTS[selectedAgent] || AGENT_PROMPTS[""];
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
            const { full, done, error } = event.payload;

            if (error) {
              setMessages((prev) =>
                prev.map((m) =>
                  m.id === assistantId
                    ? { ...m, content: `Error: ${error}`, model: "system", streaming: false }
                    : m
                )
              );
              setRuntimeError(`Chat error: ${error}`);
              return;
            }

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
      // Demo mode — simulated reply
      try {
        const response = demoChatReply(input);
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
      if (runtimeMode !== "desktop") {
        showDemoToast();
        setIsSavingConfig(false);
        return;
      }
      await saveConfig(config);
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
          setConfig(normalizeConfig(refreshedConfig));
        }
      } catch (error) {
        setRuntimeError(`Setup failed: ${formatError(error)}`);
      }
    } else {
      showDemoToast();
    }
    setShowSetupWizard(false);
    play("success");
    bumpActivity();
  }

  async function handleRefresh(): Promise<void> {
    if (runtimeMode !== "desktop") {
      showDemoToast();
      return;
    }
    try {
      const [loadedConfig, voice] = await Promise.all([getConfig(), jarvisStatus(), refreshDesktopData()]);
      setConfig(normalizeConfig(loadedConfig));
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
      showDemoToast();
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
          onNavigate={(p) => setPage(p as Page)}
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
    if (page === "mission-control") {
      return <MissionControl onNavigate={(p) => setPage(p as Page)} />;
    }
    if (page === "dna-lab") {
      return <AgentDnaLab />;
    }
    if (page === "timeline-viewer") {
      return <TimelineViewer />;
    }
    if (page === "knowledge-graph") {
      return <KnowledgeGraph />;
    }
    if (page === "immune-dashboard") {
      return <ImmuneDashboard />;
    }
    if (page === "consciousness") {
      return <ConsciousnessMonitor />;
    }
    if (page === "dreams") {
      return <DreamForge />;
    }
    if (page === "temporal") {
      return <TemporalEngine />;
    }
    if (page === "civilization") {
      return <CivilizationPage />;
    }
    if (page === "self-rewrite") {
      return <SelfRewriteLab />;
    }
    if (page === "admin-console") {
      return <AdminDashboard />;
    }
    if (page === "admin-users") {
      return <AdminUsers />;
    }
    if (page === "admin-fleet") {
      return <AdminFleet />;
    }
    if (page === "admin-policies") {
      return <AdminPolicyEditor />;
    }
    if (page === "admin-compliance") {
      return <AdminCompliance />;
    }
    if (page === "admin-health") {
      return <AdminSystemHealth />;
    }
    if (page === "integrations") {
      return <Integrations />;
    }
    if (page === "login") {
      return <Login />;
    }
    if (page === "workspaces") {
      return <Workspaces />;
    }
    if (page === "telemetry") {
      return <Telemetry />;
    }
    if (page === "usage-billing") {
      return <UsageBilling />;
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
      <LivingBackground status={runningAgents > 0 ? "healthy" : "busy"} agentCount={runningAgents} />
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
          version="v9.0.0"
        />

        <div className="nexus-main-column">
          {runtimeMode !== "desktop" && (
            <div
              style={{
                background: "linear-gradient(90deg, #b45309, #d97706, #b45309)",
                color: "#fff",
                textAlign: "center",
                padding: "8px 16px",
                fontSize: "0.82rem",
                fontWeight: 700,
                letterSpacing: "0.06em",
                textTransform: "uppercase",
                zIndex: 100,
                flexShrink: 0,
                boxShadow: "0 2px 12px rgba(217,119,6,0.4)",
              }}
            >
              Demo Mode — Running without backend. Install Nexus OS desktop for full functionality.
            </div>
          )}
          <header className="nexus-shell-header px-4 py-2.5 sm:px-6">
            <div className="nexus-control-bar">
              <div className="flex flex-wrap items-start justify-between gap-4">
                <div className="min-w-[280px] flex-1">
                  <div className="nexus-control-bar__eyebrow">
                    <span className="nexus-control-bar__eyebrow-dot" />
                    {connectionStatus === "connected" ? "Live Governed Runtime" : "Simulation Runtime"}
                  </div>
                  <div className="flex flex-wrap items-center gap-2.5">
                    <h1 className="nexus-display m-0 text-xl text-cyan-50">
                      {activePageLabel}
                    </h1>
                    <span className="nexus-topbar-chip" style={{ color: connectionStatus === "connected" ? "var(--nexus-accent)" : "var(--nexus-amber)" }}>
                      <span className="nexus-topbar-chip__signal" />
                      {connectionStatus === "connected" ? "live" : "mock"}
                    </span>
                  </div>
                  <p className="nexus-control-bar__summary">
                    {activePageSummary}
                  </p>
                </div>

                <div className="flex flex-wrap items-center justify-end gap-2.5">
                  <span className="nexus-topbar-chip">
                    <span className="nexus-topbar-chip__signal" style={{ background: "var(--nexus-accent)" }} />
                    {runningAgents} agents active
                  </span>
                  <span className="nexus-topbar-chip">
                    <span className="nexus-topbar-chip__signal" style={{ background: "var(--nexus-amber)" }} />
                    CPU {sysInfo?.cpu_usage_percent ?? "--"}%
                  </span>
                  <span className="nexus-topbar-chip">
                    <span className="nexus-topbar-chip__signal" style={{ background: "var(--nexus-purple)" }} />
                    RAM {sysInfo ? `${sysInfo.ram_used_gb}/${sysInfo.ram_total_gb}G` : "--"}
                  </span>
                  <button
                    onClick={() => { void handleRefresh(); }}
                    className="nx-btn nx-btn-ghost"
                    style={{ padding: "0.45rem 0.9rem", fontSize: "0.7rem" }}
                  >
                    Refresh
                  </button>
                  <button
                    onClick={() => {
                      if (overlay.visible) { void disableJarvisMode(); return; }
                      void enableJarvisMode();
                    }}
                    className={overlay.visible ? "nx-btn nx-btn-danger" : "nx-btn nx-btn-primary"}
                    style={{
                      padding: "0.45rem 1rem",
                      fontSize: "0.7rem",
                      ...(overlay.visible ? {} : { boxShadow: "0 0 18px rgba(74,247,211,0.16)" })
                    }}
                  >
                    {overlay.visible ? "Stop Jarvis" : "Start Jarvis"}
                  </button>
                </div>
              </div>
              {backendRestarting ? (
                <div className="mt-2 nx-badge-warning" style={{ display: "block", padding: "0.35rem 0.7rem", borderRadius: 6, fontSize: "0.75rem" }}>
                  Backend restarting... Reconnecting every 2s.
                </div>
              ) : null}
              {runtimeError && !backendRestarting ? (
                <div className="mt-2 nx-badge-error" style={{ display: "block", padding: "0.35rem 0.7rem", borderRadius: 6, fontSize: "0.75rem" }}>
                  {runtimeError}
                </div>
              ) : null}
            </div>
          </header>

          <main className="nexus-shell-content px-4 py-4 sm:px-6 sm:py-6">
            <PageTransition pageKey={page}>
              <PageErrorBoundary
                key={page}
                pageLabel={NAV_ITEMS.find((item) => item.id === page)?.label ?? page}
                onOpenSafePage={() => setPage("chat")}
              >
                <HoloPanel depth="mid" className="nexus-page-panel">
                  <Suspense fallback={<div style={{ padding: "2rem", textAlign: "center", color: "var(--text-secondary, #94a3b8)" }}>Loading...</div>}>
                    {renderPage()}
                  </Suspense>
                </HoloPanel>
              </PageErrorBoundary>
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
