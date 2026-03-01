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
import { VoiceOverlay, type VoiceOverlayState } from "./components/VoiceOverlay";
import { Agents } from "./pages/Agents";
import { Audit } from "./pages/Audit";
import { Chat } from "./pages/Chat";
import { Settings } from "./pages/Settings";
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

type Page = "chat" | "agents" | "audit" | "settings";
type RuntimeMode = "desktop" | "mock";

const NAV_ITEMS: Array<{ id: Page; label: string; hint: string }> = [
  { id: "chat", label: "Chat", hint: "LLM + command routing" },
  { id: "agents", label: "Agents", hint: "Runtime + controls" },
  { id: "audit", label: "Audit", hint: "Event chain explorer" },
  { id: "settings", label: "Settings", hint: "Config + privacy" }
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
  const [overlay, setOverlay] = useState<VoiceOverlayState>({
    visible: false,
    listening: false,
    transcription: "",
    responseText: ""
  });
  const pushToTalk = useRef<PushToTalk | null>(null);

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
      }
    };

    void hydrateDesktop();

    return () => {
      cancelled = true;
    };
  }, []);

  const connectionStatus: ConnectionStatus = runtimeMode === "desktop" ? "connected" : "mock";
  const runningAgents = useMemo(
    () => agents.filter((agent) => agent.status === "Running").length,
    [agents]
  );

  function applyVoiceState(state: VoiceRuntimeState): void {
    setOverlay((prev) => ({
      ...prev,
      visible: state.overlay_visible,
      listening: state.overlay_visible
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
    } catch (error) {
      setRuntimeError(`Unable to start agent: ${formatError(error)}`);
    }
  }

  async function handlePauseAgent(id: string): Promise<void> {
    if (runtimeMode !== "desktop") {
      updateMockAgentStatus(id, "Paused");
      return;
    }
    try {
      await pauseAgent(id);
      await refreshDesktopData();
      setRuntimeError(null);
    } catch (error) {
      setRuntimeError(`Unable to pause agent: ${formatError(error)}`);
    }
  }

  async function handleStopAgent(id: string): Promise<void> {
    if (runtimeMode !== "desktop") {
      updateMockAgentStatus(id, "Stopped");
      return;
    }
    try {
      await stopAgent(id);
      await refreshDesktopData();
      setRuntimeError(null);
    } catch (error) {
      setRuntimeError(`Unable to stop agent: ${formatError(error)}`);
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
    } catch (error) {
      setRuntimeError(`Unable to create agent: ${formatError(error)}`);
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
    } finally {
      setIsSending(false);
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
      return;
    }

    setIsRecording(false);
    try {
      const result = await recorder.stopAndTranscribe();
      if (result.transcript.trim().length > 0) {
        setDraft(result.transcript.trim());
      }
    } catch (error) {
      setRuntimeError(`Push-to-talk failed: ${formatError(error)}`);
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
    } catch (error) {
      setRuntimeError(`Unable to save settings: ${formatError(error)}`);
    } finally {
      setIsSavingConfig(false);
    }
  }

  async function handleRefresh(): Promise<void> {
    if (runtimeMode !== "desktop") {
      setAgents(mockAgents());
      setAuditEvents(mockAudit());
      setRuntimeError(null);
      return;
    }
    try {
      const [loadedConfig, voice] = await Promise.all([getConfig(), jarvisStatus(), refreshDesktopData()]);
      setConfig(loadedConfig);
      applyVoiceState(voice);
      setRuntimeError(null);
    } catch (error) {
      setRuntimeError(`Unable to refresh data: ${formatError(error)}`);
    }
  }

  async function enableJarvisMode(): Promise<void> {
    if (runtimeMode !== "desktop") {
      setOverlay({
        visible: true,
        listening: true,
        transcription: "hey nexus",
        responseText: "mock voice mode active"
      });
      return;
    }
    try {
      const voice = await startJarvisMode();
      applyVoiceState(voice);
      setOverlay((prev) => ({ ...prev, responseText: "Jarvis mode active." }));
      setRuntimeError(null);
    } catch (error) {
      setRuntimeError(`Unable to start voice mode: ${formatError(error)}`);
    }
  }

  async function disableJarvisMode(): Promise<void> {
    if (runtimeMode !== "desktop") {
      setOverlay({
        visible: false,
        listening: false,
        transcription: "",
        responseText: ""
      });
      return;
    }
    try {
      const voice = await stopJarvisMode();
      applyVoiceState(voice);
      setOverlay((prev) => ({ ...prev, transcription: "", responseText: "" }));
      setRuntimeError(null);
    } catch (error) {
      setRuntimeError(`Unable to stop voice mode: ${formatError(error)}`);
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
    return (
      <Settings
        config={config}
        saving={isSavingConfig}
        onChange={setConfig}
        onSave={() => {
          void handleSaveConfig();
        }}
      />
    );
  }

  return (
    <>
      <div className="flex min-h-screen text-zinc-100">
        <aside className="hidden w-72 flex-col border-r border-zinc-800/80 bg-zinc-950/80 px-5 py-6 backdrop-blur md:flex">
          <div>
            <h1 className="font-display text-2xl tracking-wide text-zinc-100">NEXUS OS</h1>
            <p className="mt-1 text-xs text-zinc-400">Governed agent operating system</p>
          </div>
          <nav className="mt-8 space-y-2">
            {NAV_ITEMS.map((item) => (
              <button
                key={item.id}
                onClick={() => setPage(item.id)}
                className={`w-full rounded-xl border px-3 py-3 text-left transition ${
                  page === item.id
                    ? "border-emerald-500/60 bg-emerald-500/10"
                    : "border-zinc-800 bg-zinc-900/70 hover:border-zinc-700"
                }`}
              >
                <span className="block font-display text-base text-zinc-100">{item.label}</span>
                <span className="mt-0.5 block text-xs text-zinc-400">{item.hint}</span>
              </button>
            ))}
          </nav>
          <div className="mt-auto rounded-xl border border-zinc-800 bg-zinc-900/70 p-3">
            <p className="text-xs text-zinc-400">Running agents</p>
            <p className="mt-1 font-display text-2xl text-emerald-300">{runningAgents}</p>
          </div>
        </aside>

        <div className="flex min-h-screen flex-1 flex-col">
          <header className="border-b border-zinc-800/80 bg-zinc-950/60 px-4 py-4 backdrop-blur sm:px-6">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <p className="font-display text-xl text-zinc-100">Desktop Control Plane</p>
                <div className="mt-1 flex items-center gap-2 text-xs">
                  <span
                    className={`inline-flex h-2.5 w-2.5 rounded-full ${
                      connectionStatus === "connected" ? "bg-emerald-400" : "bg-amber-400"
                    }`}
                  />
                  <span className="text-zinc-400">
                    {connectionStatus === "connected" ? "Connected to kernel backend" : "Mock runtime"}
                  </span>
                </div>
              </div>
              <div className="flex flex-wrap items-center gap-2 text-xs">
                <button
                  onClick={() => {
                    void handleRefresh();
                  }}
                  className="rounded-lg border border-zinc-700 bg-zinc-900 px-3 py-2 text-zinc-200 hover:border-zinc-600"
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
                  className={`rounded-lg px-3 py-2 font-semibold text-white ${
                    overlay.visible ? "bg-rose-600 hover:bg-rose-500" : "bg-emerald-600 hover:bg-emerald-500"
                  }`}
                >
                  {overlay.visible ? "Stop Voice" : "Start Voice"}
                </button>
              </div>
            </div>
            <nav className="mt-4 grid grid-cols-2 gap-2 md:hidden">
              {NAV_ITEMS.map((item) => (
                <button
                  key={item.id}
                  onClick={() => setPage(item.id)}
                  className={`rounded-lg border px-3 py-2 text-sm ${
                    page === item.id
                      ? "border-emerald-500/60 bg-emerald-500/10 text-zinc-100"
                      : "border-zinc-700 bg-zinc-900/70 text-zinc-300"
                  }`}
                >
                  {item.label}
                </button>
              ))}
            </nav>
            {runtimeError ? <p className="mt-3 text-xs text-rose-400">{runtimeError}</p> : null}
          </header>

          <div className="flex-1 px-4 py-4 sm:px-6 sm:py-6">{renderPage()}</div>
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
