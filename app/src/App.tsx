import { useEffect, useState } from "react";
import {
  getAuditLog,
  hasDesktopRuntime,
  jarvisStatus,
  listAgents,
  pauseAgent,
  resumeAgent,
  startAgent,
  startJarvisMode,
  stopAgent,
  stopJarvisMode
} from "./api/backend";
import { VoiceOverlay, type VoiceOverlayState } from "./components/VoiceOverlay";
import { Audit } from "./pages/Audit";
import { Chat } from "./pages/Chat";
import { Dashboard } from "./pages/Dashboard";
import type { AgentSummary, AuditEventRow, VoiceRuntimeState } from "./types";

type Page = "chat" | "dashboard" | "audit";
type RuntimeMode = "desktop" | "mock";

function mockAgents(): AgentSummary[] {
  return [
    {
      id: "a-1",
      name: "rust-social-publisher",
      status: "Running",
      fuel_remaining: 8200,
      last_action: "Published daily digest"
    },
    {
      id: "a-2",
      name: "market-researcher",
      status: "Paused",
      fuel_remaining: 4100,
      last_action: "Awaiting approval"
    }
  ];
}

function mockAudit(): AuditEventRow[] {
  return [
    {
      event_id: "evt-1",
      timestamp: 1_700_000_001,
      agent_id: "a-1",
      event_type: "StateChange",
      payload: { state: "Running" },
      previous_hash: "genesis",
      hash: "hash-1"
    },
    {
      event_id: "evt-2",
      timestamp: 1_700_000_002,
      agent_id: "a-1",
      event_type: "ToolCall",
      payload: { action: "social.post" },
      previous_hash: "hash-1",
      hash: "hash-2"
    }
  ];
}

export default function App(): JSX.Element {
  const [page, setPage] = useState<Page>("chat");
  const [runtimeMode, setRuntimeMode] = useState<RuntimeMode>("mock");
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [auditEvents, setAuditEvents] = useState<AuditEventRow[]>([]);
  const [overlay, setOverlay] = useState<VoiceOverlayState>({
    visible: false,
    listening: false,
    transcription: "",
    responseText: ""
  });

  useEffect(() => {
    if (!hasDesktopRuntime()) {
      setRuntimeMode("mock");
      setAgents(mockAgents());
      setAuditEvents(mockAudit());
      return;
    }

    let cancelled = false;

    const hydrateDesktop = async (): Promise<void> => {
      try {
        const [loadedAgents, loadedAudit, voice] = await Promise.all([
          listAgents(),
          getAuditLog(),
          jarvisStatus()
        ]);
        if (cancelled) {
          return;
        }
        setRuntimeMode("desktop");
        setRuntimeError(null);
        setAgents(loadedAgents);
        setAuditEvents(loadedAudit);
        applyVoiceState(voice);
      } catch (error) {
        if (cancelled) {
          return;
        }
        setRuntimeMode("mock");
        setRuntimeError(`Desktop backend unavailable: ${formatError(error)}`);
        setAgents(mockAgents());
        setAuditEvents(mockAudit());
      }
    };

    void hydrateDesktop();

    return () => {
      cancelled = true;
    };
  }, []);

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

    const [loadedAgents, loadedAudit] = await Promise.all([listAgents(), getAuditLog()]);
    setAgents(loadedAgents);
    setAuditEvents(loadedAudit);
  }

  function updateAgentStatus(id: string, status: AgentSummary["status"]): void {
    setAgents((prev) =>
      prev.map((agent) => (agent.id === id ? { ...agent, status, last_action: `Status set to ${status}` } : agent))
    );
  }

  async function handleStartAgent(id: string): Promise<void> {
    if (runtimeMode !== "desktop") {
      updateAgentStatus(id, "Running");
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
      setRuntimeError(`Unable to start agent ${id}: ${formatError(error)}`);
    }
  }

  async function handlePauseAgent(id: string): Promise<void> {
    if (runtimeMode !== "desktop") {
      updateAgentStatus(id, "Paused");
      return;
    }

    try {
      await pauseAgent(id);
      await refreshDesktopData();
      setRuntimeError(null);
    } catch (error) {
      setRuntimeError(`Unable to pause agent ${id}: ${formatError(error)}`);
    }
  }

  async function handleStopAgent(id: string): Promise<void> {
    if (runtimeMode !== "desktop") {
      updateAgentStatus(id, "Stopped");
      return;
    }

    try {
      await stopAgent(id);
      await refreshDesktopData();
      setRuntimeError(null);
    } catch (error) {
      setRuntimeError(`Unable to stop agent ${id}: ${formatError(error)}`);
    }
  }

  async function enableJarvisMode(): Promise<void> {
    if (runtimeMode !== "desktop") {
      setOverlay({
        visible: true,
        listening: true,
        transcription: "Hey NEXUS",
        responseText: "Listening for your command..."
      });
      return;
    }

    try {
      const voice = await startJarvisMode();
      applyVoiceState(voice);
      setOverlay((prev) => ({ ...prev, responseText: "Jarvis mode active." }));
      setRuntimeError(null);
    } catch (error) {
      setRuntimeError(`Unable to start Jarvis mode: ${formatError(error)}`);
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
      setRuntimeError(`Unable to stop Jarvis mode: ${formatError(error)}`);
    }
  }

  async function handleRefresh(): Promise<void> {
    if (runtimeMode !== "desktop") {
      setAgents(mockAgents());
      setAuditEvents(mockAudit());
      return;
    }

    try {
      await refreshDesktopData();
      setRuntimeError(null);
    } catch (error) {
      setRuntimeError(`Unable to refresh backend data: ${formatError(error)}`);
    }
  }

  return (
    <main className="mx-auto max-w-6xl px-4 py-6 sm:px-6">
      <header className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="font-display text-3xl text-ink">NEXUS OS Desktop</h1>
          <p className="text-sm text-slate-600">Governed agent operations cockpit</p>
        </div>
        <nav className="flex rounded-xl bg-white/75 p-1 shadow-sm">
          {(["chat", "dashboard", "audit"] as const).map((item) => (
            <button
              key={item}
              onClick={() => setPage(item)}
              className={`rounded-lg px-3 py-2 text-sm font-semibold transition ${
                page === item ? "bg-ink text-white" : "text-slate-700"
              }`}
            >
              {item[0].toUpperCase() + item.slice(1)}
            </button>
          ))}
        </nav>
        <div className="flex gap-2 text-xs">
          <button onClick={() => void enableJarvisMode()} className="rounded bg-mint px-3 py-2 font-semibold text-white">
            Start Jarvis
          </button>
          <button onClick={() => void disableJarvisMode()} className="rounded bg-slate-600 px-3 py-2 font-semibold text-white">
            Stop Jarvis
          </button>
          <button onClick={() => void handleRefresh()} className="rounded bg-accent px-3 py-2 font-semibold text-white">
            Refresh
          </button>
        </div>
      </header>

      <section className="mb-4 flex flex-wrap items-center gap-3 text-xs">
        <span
          className={`rounded-full px-3 py-1 font-semibold ${
            runtimeMode === "desktop" ? "bg-emerald-100 text-emerald-800" : "bg-amber-100 text-amber-800"
          }`}
        >
          Runtime: {runtimeMode === "desktop" ? "Desktop backend" : "Mock fallback"}
        </span>
        {runtimeError && <span className="text-rose-700">{runtimeError}</span>}
      </section>

      {page === "chat" && <Chat />}
      {page === "dashboard" && (
        <Dashboard
          agents={agents}
          onStart={(id) => void handleStartAgent(id)}
          onPause={(id) => void handlePauseAgent(id)}
          onStop={(id) => void handleStopAgent(id)}
        />
      )}
      {page === "audit" && <Audit events={auditEvents} />}
      <VoiceOverlay state={overlay} onDismiss={() => void disableJarvisMode()} />
    </main>
  );
}

function formatError(value: unknown): string {
  if (value instanceof Error) {
    return value.message;
  }
  return String(value);
}
