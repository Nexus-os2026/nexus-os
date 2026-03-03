import { useEffect, useMemo, useState } from "react";
import { History } from "../components/chat/History";
import { Suggestions } from "../components/chat/Suggestions";
import { VoiceVisualizer, type VoiceVisualizerState } from "../components/chat/VoiceVisualizer";
import "./chat.css";
import type { ChatMessage } from "../types";

interface ChatProps {
  messages: ChatMessage[];
  draft: string;
  isRecording: boolean;
  isSending: boolean;
  onDraftChange: (value: string) => void;
  onSend: () => void;
  onToggleMic: () => void;
}

interface HistoryEntry {
  id: string;
  timestamp: number;
  preview: string;
}

function bubbleClass(role: ChatMessage["role"]): string {
  if (role === "user") {
    return "jarvis-message jarvis-message-user";
  }
  return "jarvis-message jarvis-message-assistant";
}

function formatMilitaryTime(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString("en-GB", {
    hour12: false,
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  });
}

function deriveHistory(messages: ChatMessage[]): HistoryEntry[] {
  return messages
    .filter((message) => message.role === "user")
    .map((message) => ({
      id: message.id,
      timestamp: message.timestamp,
      preview: message.content.trim().slice(0, 84) || "(empty)"
    }))
    .reverse();
}

function deriveVisualizerState(
  isRecording: boolean,
  isSending: boolean,
  assistantStreaming: boolean
): VoiceVisualizerState {
  if (isRecording) {
    return "listening";
  }
  if (isSending) {
    return "processing";
  }
  if (assistantStreaming) {
    return "speaking";
  }
  return "idle";
}

function nextLevel(previous: number, state: VoiceVisualizerState): number {
  if (state === "idle") {
    return 0.14;
  }
  const floor = state === "listening" ? 0.26 : state === "processing" ? 0.35 : 0.42;
  const ceiling = state === "processing" ? 0.92 : 0.85;
  const jitter = Math.random() * 0.28;
  const blended = previous * 0.46 + (floor + jitter) * 0.54;
  return Math.max(floor, Math.min(ceiling, blended));
}

export function Chat({
  messages,
  draft,
  isRecording,
  isSending,
  onDraftChange,
  onSend,
  onToggleMic
}: ChatProps): JSX.Element {
  const [historyOpen, setHistoryOpen] = useState(false);
  const [audioLevel, setAudioLevel] = useState(0.14);

  const assistantStreaming = useMemo(
    () => messages.some((message) => message.role === "assistant" && message.streaming),
    [messages]
  );
  const visualizerState = deriveVisualizerState(isRecording, isSending, assistantStreaming);
  const historyEntries = useMemo(() => deriveHistory(messages), [messages]);
  const showSuggestions = draft.trim().length === 0;

  useEffect(() => {
    const interval = window.setInterval(() => {
      setAudioLevel((previous) => nextLevel(previous, visualizerState));
    }, 120);
    return () => {
      window.clearInterval(interval);
    };
  }, [visualizerState]);

  return (
    <section className="jarvis-chat-shell">
      <div className="jarvis-hud-grid" />
      <div className="jarvis-hud-corners" />

      <header className="jarvis-chat-header">
        <div className="jarvis-chat-header__left">
          <span className="jarvis-status-dot" />
          <h2 className="jarvis-title">NEXUS CORE // ACTIVE</h2>
        </div>
        <button
          type="button"
          className="jarvis-history-button"
          onClick={() => setHistoryOpen((open) => !open)}
        >
          HISTORY
        </button>
      </header>

      <main className="jarvis-chat-stream">
        {messages.length === 0 ? (
          <article className="jarvis-message jarvis-message-assistant">
            <p className="nexus-msg-typewriter">
              Awaiting command input. Try: create an agent for daily system audits.
            </p>
            <span className="jarvis-message-time">{formatMilitaryTime(Date.now())}</span>
          </article>
        ) : (
          messages.map((message) => (
            <article
              key={message.id}
              className={`jarvis-message-wrap ${message.role === "user" ? "right" : "left"}`}
            >
              <div className={bubbleClass(message.role)}>
                <p
                  className={
                    message.role === "assistant" && !message.streaming
                      ? "nexus-msg-typewriter"
                      : undefined
                  }
                >
                  {message.content || (message.streaming ? "…" : "")}
                </p>
                <span className="jarvis-message-time">
                  {formatMilitaryTime(message.timestamp)}
                  {message.model ? ` // ${message.model}` : ""}
                </span>
              </div>
            </article>
          ))
        )}
      </main>

      <footer className="jarvis-chat-footer">
        <VoiceVisualizer state={visualizerState} level={audioLevel} />

        <form
          className="jarvis-input-form"
          onSubmit={(event) => {
            event.preventDefault();
            if (!isSending) {
              onSend();
            }
          }}
        >
          <div className="jarvis-input-row">
            <button
              type="button"
              onClick={onToggleMic}
              className={`jarvis-mic-button ${isRecording ? "recording" : ""}`}
              aria-label="Toggle microphone"
            >
              <span className="jarvis-mic-ring ring-one" />
              <span className="jarvis-mic-ring ring-two" />
              <span className="jarvis-mic-core">{isRecording ? "◉" : "◎"}</span>
            </button>

            <div className="jarvis-input-shell">
              <textarea
                value={draft}
                onChange={(event) => onDraftChange(event.target.value)}
                placeholder="Transmit directive to NexusOS..."
                rows={2}
                className="jarvis-input"
              />
            </div>

            <button type="submit" disabled={isSending} className="jarvis-send-button">
              <span>SEND</span>
              <span aria-hidden="true">➤</span>
            </button>
          </div>

          <Suggestions
            visible={showSuggestions}
            onSelect={(value) => {
              onDraftChange(value);
            }}
          />

          {isSending ? (
            <div className="jarvis-thinking">
              <span>NEXUS is thinking...</span>
              <span className="dna-spinner" aria-hidden="true">
                <span />
                <span />
                <span />
                <span />
              </span>
              <span className="jarvis-thinking-scan" />
            </div>
          ) : null}
        </form>
      </footer>

      <History
        open={historyOpen}
        entries={historyEntries}
        onClose={() => setHistoryOpen(false)}
        onSelect={(entry) => {
          onDraftChange(entry.preview);
          setHistoryOpen(false);
        }}
      />
    </section>
  );
}
