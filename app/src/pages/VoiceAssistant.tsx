import { useCallback, useEffect, useRef, useState } from "react";
import {
  hasDesktopRuntime,
  sendChat,
  voiceGetStatus,
  voiceLoadWhisperModel,
  voiceStartListening,
  voiceStopListening,
  voiceTranscribe,
} from "../api/backend";

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

type TranscriptionEngine = "candle-whisper" | "python-server" | "stub";

interface EngineStatus {
  engine: TranscriptionEngine;
  whisperLoaded: boolean;
  whisperModel: string | null;
}

interface AudioCaptureSession {
  audioContext: AudioContext;
  mediaStream: MediaStream;
  sourceNode: MediaStreamAudioSourceNode;
  processorNode: ScriptProcessorNode;
  chunks: Float32Array[];
  sampleRate: number;
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
    flex: "0 0 min(340px, 40vw)",
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

function mergeAudioChunks(chunks: Float32Array[]): Float32Array {
  const totalLength = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const merged = new Float32Array(totalLength);
  let offset = 0;
  for (const chunk of chunks) {
    merged.set(chunk, offset);
    offset += chunk.length;
  }
  return merged;
}

function resampleTo16Khz(input: Float32Array, inputRate: number): Float32Array {
  if (inputRate === 16000 || input.length === 0) {
    return input;
  }

  const ratio = inputRate / 16000;
  const outputLength = Math.max(1, Math.round(input.length / ratio));
  const output = new Float32Array(outputLength);

  for (let index = 0; index < outputLength; index += 1) {
    const sourceIndex = index * ratio;
    const left = Math.floor(sourceIndex);
    const right = Math.min(left + 1, input.length - 1);
    const mix = sourceIndex - left;
    output[index] = input[left] * (1 - mix) + input[right] * mix;
  }

  return output;
}

function encodePcm16Base64(samples: Float32Array): string {
  const bytes = new Uint8Array(samples.length * 2);
  for (let index = 0; index < samples.length; index += 1) {
    const clamped = Math.max(-1, Math.min(1, samples[index]));
    const value = clamped < 0 ? clamped * 0x8000 : clamped * 0x7fff;
    const pcm = Math.round(value);
    bytes[index * 2] = pcm & 0xff;
    bytes[index * 2 + 1] = (pcm >> 8) & 0xff;
  }

  let binary = "";
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    const slice = bytes.subarray(offset, offset + chunkSize);
    binary += String.fromCharCode(...slice);
  }

  return window.btoa(binary);
}

async function teardownCaptureSession(session: AudioCaptureSession): Promise<Float32Array> {
  session.processorNode.disconnect();
  session.sourceNode.disconnect();
  session.mediaStream.getTracks().forEach((track) => track.stop());
  await session.audioContext.close();
  return mergeAudioChunks(session.chunks);
}

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function VoiceAssistant() {
  const pythonCommand = "pip install websockets numpy";
  const [status, setStatus] = useState<VoiceStatus>("ready");
  const [transcripts, setTranscripts] = useState<Transcript[]>([
    {
      id: 1,
      text: "Voice assistant initialized. Checking backend availability...",
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
  const [pythonAvailable, setPythonAvailable] = useState(false);
  const [backendChecked, setBackendChecked] = useState(false);
  const [engineStatus, setEngineStatus] = useState<EngineStatus>({
    engine: "stub",
    whisperLoaded: false,
    whisperModel: null,
  });
  const [modelLoading, setModelLoading] = useState(false);
  const [copyStatus, setCopyStatus] = useState<"idle" | "copied">("idle");
  const copyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const nextId = useRef(2);
  const scrollRef = useRef<HTMLDivElement>(null);
  const captureSessionRef = useRef<AudioCaptureSession | null>(null);
  const desktopRuntimeAvailable = hasDesktopRuntime();

  useEffect(() => {
    ensurePulseAnimation();
    return () => {
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
    };
  }, []);

  const addTranscript = useCallback(
    (text: string, source: Transcript["source"]) => {
      setTranscripts((prev) => [
        ...prev,
        { id: nextId.current++, text, source, ts: Date.now() },
      ]);
    },
    [],
  );

  const refreshBackendStatus = useCallback(async () => {
    if (!desktopRuntimeAvailable) {
      setServerRunning(false);
      setPythonAvailable(false);
      setEngineStatus({
        engine: "stub",
        whisperLoaded: false,
        whisperModel: null,
      });
      setBackendChecked(true);
      return;
    }

    try {
      const raw = await voiceGetStatus();
      const data = JSON.parse(raw);
      const engine: TranscriptionEngine = data.transcription_engine ?? "stub";
      const pyRunning: boolean = data.python_server_running ?? false;
      setEngineStatus({
        engine,
        whisperLoaded: data.whisper_loaded ?? false,
        whisperModel: data.whisper_model ?? null,
      });
      setServerRunning(pyRunning || data.is_listening === true);
      setPythonAvailable(pyRunning || engine !== "stub");
    } catch {
      setServerRunning(false);
      setPythonAvailable(false);
    } finally {
      setBackendChecked(true);
    }
  }, [desktopRuntimeAvailable]);

  useEffect(() => {
    void refreshBackendStatus();
  }, [refreshBackendStatus]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [transcripts]);

  useEffect(() => () => {
    const session = captureSessionRef.current;
    captureSessionRef.current = null;
    if (session) {
      void teardownCaptureSession(session);
    }
  }, []);

  const startCapture = useCallback(async () => {
    if (!desktopRuntimeAvailable) {
      setStatus("error");
      addTranscript("Voice capture requires the desktop runtime.", "system");
      return;
    }

    if (!engineStatus.whisperLoaded) {
      addTranscript("Load a Whisper model before recording. Real transcription is not available yet without it.", "system");
      return;
    }

    if (!navigator.mediaDevices?.getUserMedia) {
      setStatus("error");
      addTranscript("This environment does not expose microphone capture.", "system");
      return;
    }

    try {
      const mediaStream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const AudioCtor = window.AudioContext
        ?? ((window as Window & typeof globalThis & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext);
      if (!AudioCtor) {
        mediaStream.getTracks().forEach((track) => track.stop());
        throw new Error("AudioContext is not available in this browser.");
      }

      const audioContext = new AudioCtor();
      const sourceNode = audioContext.createMediaStreamSource(mediaStream);
      const processorNode = audioContext.createScriptProcessor(4096, 1, 1);
      const chunks: Float32Array[] = [];

      processorNode.onaudioprocess = (event) => {
        const channelData = event.inputBuffer.getChannelData(0);
        chunks.push(new Float32Array(channelData));
      };

      sourceNode.connect(processorNode);
      processorNode.connect(audioContext.destination);
      captureSessionRef.current = {
        audioContext,
        mediaStream,
        sourceNode,
        processorNode,
        chunks,
        sampleRate: audioContext.sampleRate,
      };

      await voiceStartListening();
      setStatus("listening");
      setServerRunning(true);
      addTranscript("Listening started. Speak, then press Stop Listening to transcribe.", "system");
      await refreshBackendStatus();
    } catch (error) {
      const session = captureSessionRef.current;
      captureSessionRef.current = null;
      if (session) {
        await teardownCaptureSession(session);
      }
      setStatus("error");
      addTranscript(`Failed to start listening: ${String(error)}`, "system");
    }
  }, [addTranscript, desktopRuntimeAvailable, engineStatus.whisperLoaded, refreshBackendStatus]);

  const stopCaptureAndTranscribe = useCallback(async () => {
    const session = captureSessionRef.current;
    captureSessionRef.current = null;
    if (!session) {
      setStatus("ready");
      return;
    }

    setStatus("processing");
    try {
      await voiceStopListening();
    } catch {
      // Continue cleanup and transcription even if backend stop state has already been cleared.
    }

    try {
      const merged = await teardownCaptureSession(session);
      const resampled = resampleTo16Khz(merged, session.sampleRate);
      if (resampled.length === 0) {
        addTranscript("No audio was captured. Try again and keep the mic active for a moment.", "system");
        setStatus("ready");
        await refreshBackendStatus();
        return;
      }

      const transcriptionRaw = await voiceTranscribe(encodePcm16Base64(resampled));
      const transcription = JSON.parse(transcriptionRaw) as {
        text?: string;
        engine?: string;
        error?: boolean;
      };

      const text = transcription.text?.trim() ?? "";
      if (!text) {
        addTranscript("The backend returned an empty transcription.", "system");
        setStatus("ready");
        await refreshBackendStatus();
        return;
      }

      if (transcription.error) {
        addTranscript(text, "system");
        setStatus("ready");
        await refreshBackendStatus();
        return;
      }

      addTranscript(text, "user");

      const response = await sendChat(text);
      addTranscript(response.text, "agent");
      setStatus("ready");
      await refreshBackendStatus();
    } catch (error) {
      setStatus("error");
      addTranscript(`Transcription failed: ${String(error)}`, "system");
    }
  }, [addTranscript, refreshBackendStatus]);

  const handleToggle = useCallback(() => {
    if (status === "listening") {
      void stopCaptureAndTranscribe();
      return;
    }
    void startCapture();
  }, [startCapture, status, stopCaptureAndTranscribe]);

  const handleLoadWhisper = useCallback(() => {
    setModelLoading(true);
    addTranscript("Loading Whisper model...", "system");
    const modelPath = "~/.nexus/models/whisper-base";
    voiceLoadWhisperModel(modelPath)
      .then((raw) => {
        const data = JSON.parse(raw);
        setEngineStatus({
          engine: "candle-whisper",
          whisperLoaded: true,
          whisperModel: data.model_path ?? modelPath,
        });
        addTranscript(
          `Whisper model loaded (${data.engine ?? "candle-whisper"})`,
          "system",
        );
      })
      .catch((err) => {
        addTranscript(`Failed to load Whisper model: ${err}`, "system");
      })
      .finally(async () => {
        setModelLoading(false);
        await refreshBackendStatus();
      });
  }, [addTranscript, refreshBackendStatus]);

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

  const handleCopyPythonCommand = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(pythonCommand);
      setCopyStatus("copied");
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
      copyTimerRef.current = window.setTimeout(() => setCopyStatus("idle"), 2000);
    } catch (err) {
      addTranscript(`Copy failed: ${String(err)}`, "system");
    }
  }, [addTranscript, pythonCommand]);

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
            {serverRunning ? "Capture Active" : "Capture Idle"}
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

          {!desktopRuntimeAvailable && (
            <div style={{
              padding: "0.75rem 0.9rem",
              borderRadius: 8,
              background: "rgba(245,158,11,0.1)",
              border: "1px solid rgba(245,158,11,0.3)",
              fontSize: "0.74rem",
              color: "#fbbf24",
              lineHeight: 1.5,
              width: "100%",
              boxSizing: "border-box" as const,
            }}>
              <div style={{ marginBottom: "0.45rem" }}>
                Voice capture is only available inside the desktop runtime. Open this page from the Tauri app to use `voice_start_listening`, `voice_transcribe`, and `send_chat`.
              </div>
            </div>
          )}

          {desktopRuntimeAvailable && backendChecked && !engineStatus.whisperLoaded && (
            <div style={{
              padding: "0.75rem 0.9rem",
              borderRadius: 8,
              background: "rgba(59,130,246,0.08)",
              border: "1px solid rgba(59,130,246,0.28)",
              fontSize: "0.74rem",
              color: "#93c5fd",
              lineHeight: 1.5,
              width: "100%",
              boxSizing: "border-box" as const,
            }}>
              Load a Whisper model to enable real transcription. Once the backend reports `whisper_loaded=true`, this page records microphone audio, calls `voice_transcribe`, and sends the transcript through the chat backend.
            </div>
          )}

          {desktopRuntimeAvailable && backendChecked && !pythonAvailable && (
            <div style={{
              padding: "0.75rem 0.9rem",
              borderRadius: 8,
              background: "rgba(245,158,11,0.08)",
              border: "1px solid rgba(245,158,11,0.24)",
              fontSize: "0.74rem",
              color: "#fbbf24",
              lineHeight: 1.5,
              width: "100%",
              boxSizing: "border-box" as const,
            }}>
              Optional Python listener dependencies are not installed. If you want the background listener process available too, run:
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "0.5rem",
                  justifyContent: "space-between",
                  flexWrap: "wrap" as const,
                  marginTop: "0.45rem",
                }}
              >
                <code style={{ color: "var(--text-primary, #e2e8f0)" }}>{pythonCommand}</code>
                <button
                  onClick={handleCopyPythonCommand}
                  style={{
                    padding: "0.28rem 0.6rem",
                    borderRadius: 999,
                    border: "1px solid rgba(245,158,11,0.35)",
                    background: "rgba(245,158,11,0.12)",
                    color: "#fde68a",
                    cursor: "pointer",
                    fontFamily: "var(--font-mono, monospace)",
                    fontSize: "0.72rem",
                  }}
                >
                  {copyStatus === "copied" ? "Copied" : "Copy"}
                </button>
              </div>
            </div>
          )}

          <button onClick={handleToggle} style={S.toggleBtn(status === "listening")}>
            {status === "listening" ? "Stop Listening" : "Start Listening"}
          </button>
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
            <div>
              <div style={S.settingLabel}>Transcription Engine</div>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span style={S.indicator(engineStatus.engine !== "stub")} />
                <span style={{ fontSize: "0.8rem", color: "var(--text-primary, #e2e8f0)" }}>
                  {engineStatus.engine === "candle-whisper"
                    ? "Candle Whisper"
                    : engineStatus.engine === "python-server"
                      ? "Python Listener"
                      : "Backend idle - load Whisper for transcription"}
                </span>
              </div>
            </div>
            <div style={{ marginLeft: "auto" }}>
              {!engineStatus.whisperLoaded && (
                <button
                  onClick={handleLoadWhisper}
                  disabled={modelLoading}
                  style={{
                    background: modelLoading
                      ? "var(--bg-tertiary, #0f172a)"
                      : "linear-gradient(135deg, #8b5cf6, #6d28d9)",
                    border: "1px solid var(--border, #334155)",
                    borderRadius: 6,
                    padding: "0.4rem 0.8rem",
                    color: modelLoading ? "var(--text-secondary, #64748b)" : "#fff",
                    cursor: modelLoading ? "not-allowed" : "pointer",
                    fontSize: "0.75rem",
                    fontFamily: "var(--font-mono, monospace)",
                    fontWeight: 600,
                  }}
                >
                  {modelLoading ? "Loading..." : "Load Whisper Model"}
                </button>
              )}
              {engineStatus.whisperLoaded && (
                <span style={{ fontSize: "0.7rem", color: "#10b981" }}>
                  Whisper Ready
                </span>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
