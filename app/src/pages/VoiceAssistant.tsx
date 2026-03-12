import { useCallback, useEffect, useRef, useState } from "react";

/* ================================================================== */
/*  Types                                                              */
/* ================================================================== */

interface Transcript {
  id: number;
  text: string;
  source: "user" | "agent" | "system";
  ts: number;
}

type VoiceStatus = "ready" | "listening" | "processing" | "error";

interface VoiceSettings {
  wakeWord: string;
  sampleRate: number;
  autoListen: boolean;
}

/* ================================================================== */
/*  Styles                                                             */
/* ================================================================== */

const S = {
  page: {
    padding: "1.5rem",
    height: "100%",
    display: "flex",
    flexDirection: "column" as const,
    gap: "1.5rem",
    fontFamily: "var(--font-mono, monospace)",
    color: "var(--text-primary, #e2e8f0)",
    overflow: "hidden",
  },
  header: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    flexShrink: 0,
  },
  title: {
    fontFamily: "var(--font-display, monospace)",
    fontSize: "1.5rem",
    fontWeight: 700,
    color: "var(--text-primary, #e2e8f0)",
    margin: 0,
  },
  badge: {
    fontSize: "0.7rem",
    padding: "0.2rem 0.6rem",
    borderRadius: 999,
    fontWeight: 600,
  },
  mainArea: {
    display: "flex",
    flex: 1,
    gap: "1.5rem",
    minHeight: 0,
  },
  vizPanel: {
    flex: "0 0 340px",
    display: "flex",
    flexDirection: "column" as const,
    alignItems: "center",
    justifyContent: "center",
    gap: "1.5rem",
    background: "var(--bg-secondary, #1e293b)",
    borderRadius: 12,
    border: "1px solid var(--border, #334155)",
    padding: "2rem",
  },
  orbContainer: {
    position: "relative" as const,
    width: 180,
    height: 180,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
  },
  orbCore: (status: VoiceStatus) => ({
    width: 80,
    height: 80,
    borderRadius: "50%",
    background:
      status === "listening"
        ? "radial-gradient(circle, #3b82f6, #1d4ed8)"
        : status === "processing"
          ? "radial-gradient(circle, #f59e0b, #d97706)"
          : status === "error"
            ? "radial-gradient(circle, #ef4444, #b91c1c)"
            : "radial-gradient(circle, #475569, #334155)",
    boxShadow:
      status === "listening"
        ? "0 0 40px rgba(59,130,246,0.5), 0 0 80px rgba(59,130,246,0.2)"
        : status === "processing"
          ? "0 0 40px rgba(245,158,11,0.5)"
          : "0 0 20px rgba(71,85,105,0.3)",
    transition: "all 0.4s ease",
  }),
  ring: (i: number, status: VoiceStatus) => ({
    position: "absolute" as const,
    inset: -12 * (i + 1),
    borderRadius: "50%",
    border: `1.5px solid ${
      status === "listening"
        ? `rgba(59,130,246,${0.4 - i * 0.1})`
        : status === "processing"
          ? `rgba(245,158,11,${0.3 - i * 0.08})`
          : `rgba(100,116,139,${0.2 - i * 0.05})`
    }`,
    animation:
      status === "listening"
        ? `voicePulse ${1.5 + i * 0.4}s ease-in-out infinite`
        : "none",
  }),
  statusText: {
    fontSize: "1rem",
    fontWeight: 600,
    color: "var(--text-secondary, #94a3b8)",
    textTransform: "uppercase" as const,
    letterSpacing: "0.1em",
  },
  toggleBtn: (active: boolean) => ({
    padding: "0.7rem 2rem",
    borderRadius: 999,
    border: "none",
    cursor: "pointer",
    fontFamily: "var(--font-mono, monospace)",
    fontWeight: 600,
    fontSize: "0.85rem",
    background: active
      ? "linear-gradient(135deg, #ef4444, #b91c1c)"
      : "linear-gradient(135deg, #3b82f6, #1d4ed8)",
    color: "#fff",
    transition: "all 0.3s ease",
    boxShadow: active
      ? "0 0 20px rgba(239,68,68,0.3)"
      : "0 0 20px rgba(59,130,246,0.3)",
  }),
  rightPanel: {
    flex: 1,
    display: "flex",
    flexDirection: "column" as const,
    gap: "1rem",
    minHeight: 0,
  },
  transcriptArea: {
    flex: 1,
    background: "var(--bg-secondary, #1e293b)",
    borderRadius: 12,
    border: "1px solid var(--border, #334155)",
    padding: "1rem",
    overflow: "auto" as const,
    display: "flex",
    flexDirection: "column" as const,
    gap: "0.5rem",
  },
  transcriptItem: (source: string) => ({
    padding: "0.6rem 0.8rem",
    borderRadius: 8,
    background:
      source === "user"
        ? "rgba(59,130,246,0.1)"
        : source === "agent"
          ? "rgba(16,185,129,0.1)"
          : "rgba(100,116,139,0.1)",
    borderLeft: `3px solid ${
      source === "user" ? "#3b82f6" : source === "agent" ? "#10b981" : "#64748b"
    }`,
    fontSize: "0.85rem",
    color: "var(--text-primary, #e2e8f0)",
  }),
  transcriptTs: {
    fontSize: "0.7rem",
    color: "var(--text-secondary, #64748b)",
    marginBottom: "0.2rem",
  },
  settingsPanel: {
    background: "var(--bg-secondary, #1e293b)",
    borderRadius: 12,
    border: "1px solid var(--border, #334155)",
    padding: "1rem",
    display: "flex",
    gap: "1.5rem",
    alignItems: "center",
    flexWrap: "wrap" as const,
    flexShrink: 0,
  },
  settingLabel: {
    fontSize: "0.75rem",
    color: "var(--text-secondary, #94a3b8)",
    marginBottom: "0.3rem",
  },
  settingInput: {
    background: "var(--bg-tertiary, #0f172a)",
    border: "1px solid var(--border, #334155)",
    borderRadius: 6,
    padding: "0.4rem 0.6rem",
    color: "var(--text-primary, #e2e8f0)",
    fontFamily: "var(--font-mono, monospace)",
    fontSize: "0.8rem",
    width: 120,
    outline: "none",
  },
  indicator: (active: boolean) => ({
    width: 8,
    height: 8,
    borderRadius: "50%",
    background: active ? "#10b981" : "#64748b",
    display: "inline-block",
    marginRight: 6,
    boxShadow: active ? "0 0 8px rgba(16,185,129,0.5)" : "none",
  }),
  emptyState: {
    flex: 1,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    color: "var(--text-secondary, #64748b)",
    fontSize: "0.85rem",
    fontStyle: "italic" as const,
  },
};

/* ================================================================== */
/*  CSS Animation (injected once)                                      */
/* ================================================================== */

const ANIM_ID = "nexus-voice-pulse-anim";

function ensurePulseAnimation(): void {
  if (document.getElementById(ANIM_ID)) return;
  const style = document.createElement("style");
  style.id = ANIM_ID;
  style.textContent = `
    @keyframes voicePulse {
      0%, 100% { transform: scale(1); opacity: 1; }
      50% { transform: scale(1.08); opacity: 0.6; }
    }
  `;
  document.head.appendChild(style);
}

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function VoiceAssistant() {
  const [status, setStatus] = useState<VoiceStatus>("ready");
  const [transcripts, setTranscripts] = useState<Transcript[]>([
    {
      id: 1,
      text: "Voice assistant initialized. Say the wake word or press Start to begin.",
      source: "system",
      ts: Date.now(),
    },
  ]);
  const [settings, setSettings] = useState<VoiceSettings>({
    wakeWord: "nexus",
    sampleRate: 16000,
    autoListen: false,
  });
  const [serverRunning, setServerRunning] = useState(false);
  const nextId = useRef(2);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    ensurePulseAnimation();
  }, []);

  // Auto-scroll transcript area
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [transcripts]);

  const addTranscript = useCallback(
    (text: string, source: Transcript["source"]) => {
      setTranscripts((prev) => [
        ...prev,
        { id: nextId.current++, text, source, ts: Date.now() },
      ]);
    },
    [],
  );

  const handleToggle = useCallback(() => {
    if (status === "listening") {
      setStatus("ready");
      addTranscript("Listening stopped.", "system");
    } else {
      setStatus("listening");
      setServerRunning(true);
      addTranscript("Listening started — waiting for voice input...", "system");
    }
  }, [status, addTranscript]);

  const handleSimulateInput = useCallback(() => {
    if (status !== "listening") return;

    setStatus("processing");
    addTranscript("Detected speech input...", "system");

    // Simulate transcription delay
    setTimeout(() => {
      addTranscript(
        "What's the status of the coder agent?",
        "user",
      );
      // Simulate agent response
      setTimeout(() => {
        addTranscript(
          "The Coder agent is currently running with 8,450 fuel remaining. Last action: code review on api-client module 2 minutes ago.",
          "agent",
        );
        setStatus("listening");
      }, 800);
    }, 600);
  }, [status, addTranscript]);

  const handleClear = useCallback(() => {
    setTranscripts([
      {
        id: nextId.current++,
        text: "Transcript cleared.",
        source: "system",
        ts: Date.now(),
      },
    ]);
  }, []);

  const formatTime = useCallback((ts: number) => {
    const d = new Date(ts);
    return d.toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  }, []);

  const statusLabel =
    status === "listening"
      ? "Listening..."
      : status === "processing"
        ? "Processing..."
        : status === "error"
          ? "Error"
          : "Ready";

  const statusColor =
    status === "listening"
      ? "#3b82f6"
      : status === "processing"
        ? "#f59e0b"
        : status === "error"
          ? "#ef4444"
          : "#64748b";

  return (
    <div style={S.page}>
      {/* Header */}
      <div style={S.header}>
        <div style={{ display: "flex", alignItems: "center", gap: "0.8rem" }}>
          <h2 style={S.title}>Voice Assistant</h2>
          <span
            style={{
              ...S.badge,
              background: "rgba(139,92,246,0.15)",
              color: "#a78bfa",
            }}
          >
            Jarvis Mode
          </span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
          <span style={{ fontSize: "0.8rem", color: "var(--text-secondary, #94a3b8)" }}>
            <span style={S.indicator(serverRunning)} />
            {serverRunning ? "Server Running" : "Server Stopped"}
          </span>
          <button
            onClick={handleClear}
            style={{
              background: "var(--bg-tertiary, #0f172a)",
              border: "1px solid var(--border, #334155)",
              borderRadius: 6,
              padding: "0.4rem 0.8rem",
              color: "var(--text-secondary, #94a3b8)",
              cursor: "pointer",
              fontSize: "0.75rem",
              fontFamily: "var(--font-mono, monospace)",
            }}
          >
            Clear
          </button>
        </div>
      </div>

      {/* Main content */}
      <div style={S.mainArea}>
        {/* Left: Visualization */}
        <div style={S.vizPanel}>
          {/* Animated orb */}
          <div style={S.orbContainer}>
            {[0, 1, 2, 3].map((i) => (
              <div key={i} style={S.ring(i, status)} />
            ))}
            <div style={S.orbCore(status)} />
          </div>

          <div style={{ ...S.statusText, color: statusColor }}>
            {statusLabel}
          </div>

          <div style={{ fontSize: "0.75rem", color: "var(--text-secondary, #64748b)" }}>
            Wake word: <strong style={{ color: "var(--text-primary, #e2e8f0)" }}>
              &quot;{settings.wakeWord}&quot;
            </strong>
          </div>

          <button onClick={handleToggle} style={S.toggleBtn(status === "listening")}>
            {status === "listening" ? "Stop Listening" : "Start Listening"}
          </button>

          {status === "listening" && (
            <button
              onClick={handleSimulateInput}
              style={{
                padding: "0.5rem 1.2rem",
                borderRadius: 999,
                border: "1px solid var(--border, #334155)",
                background: "transparent",
                color: "var(--text-secondary, #94a3b8)",
                cursor: "pointer",
                fontFamily: "var(--font-mono, monospace)",
                fontSize: "0.75rem",
              }}
            >
              Simulate Voice Input
            </button>
          )}
        </div>

        {/* Right: Transcripts + settings */}
        <div style={S.rightPanel}>
          {/* Transcript area */}
          <div ref={scrollRef} style={S.transcriptArea}>
            {transcripts.length === 0 ? (
              <div style={S.emptyState}>
                No transcripts yet. Start listening to begin.
              </div>
            ) : (
              transcripts.map((t) => (
                <div key={t.id} style={S.transcriptItem(t.source)}>
                  <div style={S.transcriptTs}>
                    {formatTime(t.ts)} &middot;{" "}
                    {t.source === "user"
                      ? "You"
                      : t.source === "agent"
                        ? "Agent"
                        : "System"}
                  </div>
                  {t.text}
                </div>
              ))
            )}
          </div>

          {/* Settings */}
          <div style={S.settingsPanel}>
            <div>
              <div style={S.settingLabel}>Wake Word</div>
              <input
                style={S.settingInput}
                value={settings.wakeWord}
                onChange={(e) =>
                  setSettings((s) => ({ ...s, wakeWord: e.target.value }))
                }
              />
            </div>
            <div>
              <div style={S.settingLabel}>Sample Rate</div>
              <select
                style={{ ...S.settingInput, width: 100 }}
                value={settings.sampleRate}
                onChange={(e) =>
                  setSettings((s) => ({
                    ...s,
                    sampleRate: parseInt(e.target.value, 10),
                  }))
                }
              >
                <option value={8000}>8 kHz</option>
                <option value={16000}>16 kHz</option>
                <option value={44100}>44.1 kHz</option>
                <option value={48000}>48 kHz</option>
              </select>
            </div>
            <div>
              <div style={S.settingLabel}>Auto-Listen</div>
              <label
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 6,
                  fontSize: "0.8rem",
                  cursor: "pointer",
                  color: "var(--text-primary, #e2e8f0)",
                }}
              >
                <input
                  type="checkbox"
                  checked={settings.autoListen}
                  onChange={(e) =>
                    setSettings((s) => ({ ...s, autoListen: e.target.checked }))
                  }
                />
                On startup
              </label>
            </div>
            <div style={{ marginLeft: "auto", fontSize: "0.7rem", color: "var(--text-secondary, #64748b)" }}>
              Transcription: stub (LLM gateway pending)
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
