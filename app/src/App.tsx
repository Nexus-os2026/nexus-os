import { useEffect, useMemo, useState } from "react";
import {
  createAgent,
  getAuditLog,
  getConfig,
  hasDesktopRuntime,
  jarvisStatus,
  listAgents,
  pauseAgent,
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
import type { AgentSummary, AuditEventRow, ChatMessage, ConnectionStatus, NexusConfig } from "./types";
import { PushToTalk } from "./voice/PushToTalk";

type Page = "chat" | "agents" | "audit" | "settings";

const NAV_ITEMS: Array<{ id: Page; label: string }> = [
  { id: "chat", label: "Chat" },
  { id: "agents", label: "Agents" },
  { id: "audit", label: "Audit" },
  { id: "settings", label: "Settings" }
];

function mockAgents(): AgentSummary[] {
  return [
    {
      id: "mock-a1",
      name: "research-scout",
      status: "Running",
      fuel_remaining: 7800,
      last_action: "Web scan complete"
    },
    {
      id: "mock-a2",
      name: "content-pilot",
      status: "Paused",
      fuel_remaining: 4200,
      last_action: "Awaiting approval"
    }
  ];
}

function mockAudit(): AuditEventRow[] {
  return [
    {
      event_id: "evt-001",
      timestamp: 1_700_010_111,
      agent_id: "mock-a1",
      event_type: "ToolCall",
      payload: { tool: "web.search", query: "nexus os launch checklist" },
      previous_hash: "genesis",
      hash: "hash-001"
    },
    {
      event_id: "evt-002",
      timestamp: 1_700_010_221,
      agent_id: "mock-a2",
      event_type: "StateChange",
      payload: { from: "Running", to: "Paused" },
      previous_hash: "hash-001",
      hash: "hash-002"
    }
  ];
}

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

export default function App(): JSX.Element {
  const [page, setPage] = useState<Page>("chat");
  const [connection, setConnection] = useState<ConnectionStatus>("mock");
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [auditEvents, setAuditEvents] = useState<AuditEventRow[]>([]);
  const [config, setConfig] = useState<NexusConfig>(defaultConfig());
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [isSending, setIsSending] = useState(false);
  const [isRecording, setIsRecording] = useState(false);
  const [isSavingSettings, setIsSavingSettings] = useState(false);
  const [overlay, setOverlay] = useState<VoiceOverlayState>({
    visible: false,
    listening: false,
    transcription: "",
    responseText: ""
  });
  const pushToTalk = useMemo(() => new PushToTalk(), []);

  useEffect(() => {
    void hydrate();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function hydrate(): Promise<void> {
    if (!hasDesktopRuntime()) {
      setConnection("mock");
      setAgents(mockAgents());
      setAuditEvents(mockAudit());
      setRuntimeError("Desktop runtime unavailable. Running in mock mode.");
      return;
    }

    try {
      const [loadedAgents, loadedAudit, loadedConfig, voice] = await Promise.all([
        listAgents(),
        getAuditLog(undefined, 300),
        getConfig(),
        jarvisStatus()
      ]);
      setConnection("connected");
      setRuntimeError(null);
      setAgents(loadedAgents);
      setAuditEvents(loadedAudit);
      setConfig(loadedConfig);
      setOverlay((prev) => ({
        ...prev,
        visible: voice.overlay_visible,
        listening: voice.overlay_visible
      }));
    } catch (error) {
      setConnection("mock");
      setRuntimeError(`Backend unavailable: ${formatError(error)}`);
      setAgents(mockAgents());
      setAuditEvents(mockAudit());
    }
  }

  async function refreshRuntimeState(): Promise<void> {
    if (connection !== "connected") {
      return;
    }
    const [loadedAgents, loadedAudit] = await Promise.all([listAgents(), getAuditLog(undefined, 300)]);
    setAgents(loadedAgents);
    setAuditEvents(loadedAudit);
  }

  function addMessage(role: ChatMessage["role"], content: string, model?: string, streaming = false): string {
    const id = crypto.randomUUID();
    setMessages((prev) => [
      ...prev,
      {
        id,
        role,
        content,
        model,
        timestamp: Date.now(),
        streaming
      }
    ]);
    return id;
  }

  async function streamAssistantMessage(messageId: string, fullText: string, model?: string): Promise<void> {
    let index = 0;
    const batch = 7;
    await new Promise<void>((resolve) => {
      const timer = setInterval(() => {
        index = Math.min(fullText.length, index + batch);
        setMessages((prev) =>
          prev.map((message) =>
            message.id === messageId
              ? {
                  ...message,
                  content: fullText.slice(0, index),
                  model,
                  streaming: index < fullText.length
                }
              : message
          )
        );
        if (index >= fullText.length) {
          clearInterval(timer);
          resolve();
        }
      }, 18);
    });
  }

  async function handleSendMessage(): Promise<void> {
    const content = draft.trim();
    if (!content || isSending) {
      return;
    }

    setDraft("");
    addMessage("user", content);
    const assistantId = addMessage("assistant", "", undefined, true);
    setIsSending(true);

    try {
      const createRequest = parseCreateAgentCommand(content);
      if (createRequest) {
        await handleCreateAgent(createRequest.manifestJson);
        await streamAssistantMessage(
          assistantId,
          `Agent ${createRequest.name} created from command route.`,
          "agent-factory"
        );
        return;
      }

      if (connection === "connected") {
        const response = await sendChat(content);
        await streamAssistantMessage(assistantId, response.text, response.model);
      } else {
        await streamAssistantMessage(
          assistantId,
          "Mock response: your request is queued for governed policy checks and execution.",
          "mock-provider"
        );
      }
    } catch (error) {
      await streamAssistantMessage(assistantId, `Failed to process request: ${formatError(error)}`, "error");
    } finally {
      setIsSending(false);
    }
  }

  async function handleCreateAgent(manifestJson: string): Promise<void> {
    if (connection === "connected") {
      await createAgent(manifestJson);
      await refreshRuntimeState();
      return;
    }

    const parsed = JSON.parse(manifestJson) as {
      name: string;
      fuel_budget: number;
    };
    setAgents((prev) => [
      ...prev,
      {
        id: `mock-${crypto.randomUUID()}`,
        name: parsed.name,
        status: "Running",
        fuel_remaining: parsed.fuel_budget,
        last_action: "Created from Agent Factory"
      }
    ]);
  }

  async function handleStartAgent(id: string): Promise<void> {
    if (connection === "connected") {
      await startAgent(id);
      await refreshRuntimeState();
      return;
    }
    setAgents((prev) => prev.map((agent) => (agent.id === id ? { ...agent, status: "Running" } : agent)));
  }

  async function handlePauseAgent(id: string): Promise<void> {
    if (connection === "connected") {
      await pauseAgent(id);
      await refreshRuntimeState();
      return;
    }
    setAgents((prev) => prev.map((agent) => (agent.id === id ? { ...agent, status: "Paused" } : agent)));
  }

  async function handleStopAgent(id: string): Promise<void> {
    if (connection === "connected") {
      await stopAgent(id);
      await refreshRuntimeState();
      return;
    }
    setAgents((prev) => prev.map((agent) => (agent.id === id ? { ...agent, status: "Stopped" } : agent)));
  }

  async function handleToggleMic(): Promise<void> {
    if (!isRecording) {
      pushToTalk.startRecording();
      setIsRecording(true);
      return;
    }
    const result = await pushToTalk.stopAndTranscribe();
    setIsRecording(false);
    if (result.transcript.trim()) {
      setDraft(result.transcript.trim());
    }
  }

  async function handleSaveSettings(): Promise<void> {
    if (connection !== "connected") {
      return;
    }
    setIsSavingSettings(true);
    try {
      await saveConfig(config);
      setRuntimeError(null);
    } catch (error) {
      setRuntimeError(`Failed to save config: ${formatError(error)}`);
    } finally {
      setIsSavingSettings(false);
    }
  }

  async function enableVoiceOverlay(): Promise<void> {
    if (connection !== "connected") {
      setOverlay({
        visible: true,
        listening: true,
        transcription: "Hey NEXUS",
        responseText: "Mock voice runtime enabled"
      });
      return;
    }

    const voice = await startJarvisMode();
    setOverlay((prev) => ({
      ...prev,
      visible: voice.overlay_visible,
      listening: true,
      responseText: "Jarvis mode active"
    }));
  }

  async function disableVoiceOverlay(): Promise<void> {
    if (connection !== "connected") {
      setOverlay({
        visible: false,
        listening: false,
        transcription: "",
        responseText: ""
      });
      return;
    }
    const voice = await stopJarvisMode();
    setOverlay((prev) => ({
      ...prev,
      visible: voice.overlay_visible,
      listening: false
    }));
  }

  const runningAgents = agents.filter((agent) => agent.status === "Running").length;

  return (
    <main className="min-h-screen bg-zinc-950 text-zinc-100">
      <div className="mx-auto flex min-h-screen max-w-[1600px] flex-col md:flex-row">
        <aside className="w-full border-b border-zinc-800 bg-zinc-900/95 p-4 md:w-64 md:border-b-0 md:border-r">
          <div className="mb-4">
            <h1 className="font-display text-2xl text-white">NEXUS OS</h1>
            <p className="text-xs uppercase tracking-[0.2em] text-zinc-400">Governed Agent Desktop</p>
          </div>
          <nav className="grid grid-cols-2 gap-2 md:grid-cols-1">
            {NAV_ITEMS.map((item) => (
              <button
                key={item.id}
                onClick={() => setPage(item.id)}
                className={`rounded-xl px-3 py-2 text-left text-sm transition ${
                  page === item.id
                    ? "bg-emerald-600/20 text-emerald-300"
                    : "bg-zinc-900 text-zinc-300 hover:bg-zinc-800"
                }`}
              >
                {item.label}
              </button>
            ))}
          </nav>
        </aside>

        <section className="flex-1 p-4 md:p-6">
          <header className="mb-4 flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-zinc-800 bg-zinc-900/80 px-4 py-3">
            <div className="flex items-center gap-3">
              <span
                className={`h-2.5 w-2.5 rounded-full ${
                  connection === "connected" ? "bg-emerald-400" : "bg-amber-400"
                }`}
              />
              <p className="text-sm text-zinc-200">
                {connection === "connected" ? "Connected to kernel" : "Mock mode"}
              </p>
              <span className="rounded-full bg-zinc-800 px-2 py-0.5 text-xs text-zinc-300">
                {runningAgents} running agents
              </span>
            </div>
            <div className="flex gap-2">
              <button onClick={() => void enableVoiceOverlay()} className="rounded-lg bg-sky-600 px-3 py-2 text-xs font-semibold text-white">
                Start Voice
              </button>
              <button onClick={() => void disableVoiceOverlay()} className="rounded-lg bg-zinc-700 px-3 py-2 text-xs font-semibold text-zinc-200">
                Stop Voice
              </button>
              <button onClick={() => void hydrate()} className="rounded-lg bg-emerald-700 px-3 py-2 text-xs font-semibold text-white">
                Refresh
              </button>
            </div>
          </header>

          {runtimeError ? (
            <p className="mb-4 rounded-xl border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-300">
              {runtimeError}
            </p>
          ) : null}

          {page === "chat" ? (
            <Chat
              messages={messages}
              draft={draft}
              isRecording={isRecording}
              isSending={isSending}
              onDraftChange={setDraft}
              onSend={() => void handleSendMessage()}
              onToggleMic={() => void handleToggleMic()}
            />
          ) : null}

          {page === "agents" ? (
            <Agents
              agents={agents}
              auditEvents={auditEvents}
              onStart={(id) => void handleStartAgent(id)}
              onPause={(id) => void handlePauseAgent(id)}
              onStop={(id) => void handleStopAgent(id)}
              onCreate={(manifestJson) => void handleCreateAgent(manifestJson)}
            />
          ) : null}

          {page === "audit" ? <Audit events={auditEvents} /> : null}

          {page === "settings" ? (
            <Settings config={config} onChange={setConfig} onSave={() => void handleSaveSettings()} saving={isSavingSettings} />
          ) : null}
        </section>
      </div>

      <VoiceOverlay
        state={overlay}
        onDismiss={() =>
          setOverlay((prev) => ({
            ...prev,
            visible: false,
            listening: false
          }))
        }
      />
    </main>
  );
}

interface ParsedCreateAgentCommand {
  name: string;
  manifestJson: string;
}

function parseCreateAgentCommand(input: string): ParsedCreateAgentCommand | null {
  const match = input.trim().match(/^create\s+agent\s+([a-zA-Z0-9-]+)/i);
  if (!match) {
    return null;
  }

  const name = match[1];
  const manifest = {
    name,
    version: "0.1.0",
    capabilities: ["web.search", "llm.query", "fs.read"],
    fuel_budget: 10000,
    schedule: null,
    llm_model: "claude-sonnet-4-5"
  };
  return {
    name,
    manifestJson: JSON.stringify(manifest)
  };
}

function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}
