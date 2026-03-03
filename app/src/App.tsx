import { useEffect, useMemo, useRef, useState } from "react";
import {
  createAgent,
  getAuditLog,
  getConfig,
  hasDesktopRuntime,
  jarvisStatus,
  listAgents,
  pauseAgent,
  resumeAgent,
  saveConfig,
  sendChat,
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
import { Settings } from "./pages/Settings";
import { Workflows } from "./pages/Workflows";
import type {
  AgentSummary,
  AuditEventRow,
  ChatMessage,
  ChatResponse,
  ConnectionStatus,
  NexusConfig,
  VoiceRuntimeState
} from "./types";
import { PushToTalk } from "./voice/PushToTalk";

type Page = "chat" | "agents" | "audit" | "workflows" | "marketplace" | "settings";
type RuntimeMode = "desktop" | "mock";

const NAV_ITEMS: SidebarItem[] = [
  { id: "chat", label: "Chat", icon: "⌁", shortcut: "Alt+1" },
  { id: "agents", label: "Agents", icon: "⬢", shortcut: "Alt+2" },
  { id: "audit", label: "Audit", icon: "⧉", shortcut: "Alt+3" },
  { id: "workflows", label: "Workflows", icon: "⎇", shortcut: "Alt+4" },
  { id: "marketplace", label: "Marketplace", icon: "◈", shortcut: "Alt+5" },
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

function mockAgents(): AgentSummary[] {
  return [
    {
      id: "mock-agent-1",
      name: "research-briefing",
      status: "Running",
      fuel_remaining: 7800,
      last_action: "summarized overnight market activity"
    },
    {
      id: "mock-agent-2",
      name: "content-publisher",
      status: "Paused",
      fuel_remaining: 3900,
      last_action: "awaiting human approval"
    }
  ];
}

function mockAudit(): AuditEventRow[] {
  return [
    {
      event_id: "mock-evt-1",
      timestamp: 1_700_100_001,
      agent_id: "mock-agent-1",
      event_type: "StateChange",
      payload: { state: "Running", trigger: "startup" },
      previous_hash: "genesis",
      hash: "mock-hash-1"
    },
    {
      event_id: "mock-evt-2",
      timestamp: 1_700_100_052,
      agent_id: "mock-agent-2",
      event_type: "ApprovalRequired",
      payload: { action: "social.post", channel: "x" },
      previous_hash: "mock-hash-1",
      hash: "mock-hash-2"
    }
  ];
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
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [isSending, setIsSending] = useState(false);
  const [isRecording, setIsRecording] = useState(false);
  const [isSavingConfig, setIsSavingConfig] = useState(false);
  const [factoryTrigger, setFactoryTrigger] = useState(0);
  const [activityPulse, setActivityPulse] = useState(0);
  const [appReady, setAppReady] = useState(false);
  const [splashVisible, setSplashVisible] = useState(true);
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
        makeMessage(
          "assistant",
          "Desktop runtime not detected. You are in mock mode; UI remains fully interactive."
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

  async function refreshDesktopData(): Promise<void> {
    if (runtimeMode !== "desktop") {
      return;
    }
    const [loadedAgents, loadedAudit] = await Promise.all([listAgents(), getAuditLog(undefined, 500)]);
    setAgents(loadedAgents);
    setAuditEvents(loadedAudit);
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

  async function streamAssistantMessage(id: string, text: string, model: string): Promise<void> {
    const chunks = text.split(" ");
    let current = "";
    for (let index = 0; index < chunks.length; index += 1) {
      current = current.length === 0 ? chunks[index] : `${current} ${chunks[index]}`;
      const done = index === chunks.length - 1;
      setMessages((prev) =>
        prev.map((message) =>
          message.id === id
            ? {
                ...message,
                content: current,
                model,
                streaming: !done
              }
            : message
        )
      );
      await sleep(done ? 0 : 16);
    }
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
    setMessages((prev) => [
      ...prev,
      {
        id: assistantId,
        role: "assistant",
        content: "",
        timestamp: Date.now(),
        streaming: true
      }
    ]);

    try {
      const response = runtimeMode === "desktop" ? await sendChat(input) : mockChatReply(input);
      await streamAssistantMessage(assistantId, response.text, response.model);
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
                content: `Request failed: ${formatError(error)}`,
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
          onDraftChange={setDraft}
          onSend={() => {
            void handleSend();
          }}
          onToggleMic={() => {
            void handleToggleMic();
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
      return (
        <section className="nexus-panel flex h-[calc(100vh-10rem)] items-center justify-center p-8">
          <div className="text-center">
            <h2 className="nexus-display text-2xl text-cyan-100">Marketplace // Soon</h2>
            <p className="mt-2 text-sm text-cyan-100/65">
              Curated agent packages and trust policies will appear here.
            </p>
          </div>
        </section>
      );
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
          version="v1.0.0"
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
      />
    </>
  );
}
