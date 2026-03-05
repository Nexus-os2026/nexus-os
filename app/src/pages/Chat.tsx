import { useEffect, useMemo, useRef, useState } from "react";
import { History } from "../components/chat/History";
import { Suggestions } from "../components/chat/Suggestions";
import { VoiceVisualizer, type VoiceVisualizerState } from "../components/chat/VoiceVisualizer";
import "./chat.css";
import type { AgentSummary, ChatMessage } from "../types";

interface ChatProps {
  messages: ChatMessage[];
  draft: string;
  isRecording: boolean;
  isSending: boolean;
  agents: AgentSummary[];
  selectedAgent: string;
  onAgentChange: (agentId: string) => void;
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
  agents,
  selectedAgent,
  onAgentChange,
  onDraftChange,
  onSend,
  onToggleMic
}: ChatProps): JSX.Element {
  const [historyOpen, setHistoryOpen] = useState(false);
  const [audioLevel, setAudioLevel] = useState(0.14);
  const streamRef = useRef<HTMLDivElement>(null);

  const assistantStreaming = useMemo(
    () => messages.some((message) => message.role === "assistant" && message.streaming),
    [messages]
  );
  const visualizerState = deriveVisualizerState(isRecording, isSending, assistantStreaming);
  const historyEntries = useMemo(() => deriveHistory(messages), [messages]);
  const showSuggestions = draft.trim().length === 0 && messages.length <= 1;

  useEffect(() => {
    const interval = window.setInterval(() => {
      setAudioLevel((previous) => nextLevel(previous, visualizerState));
    }, 120);
    return () => {
      window.clearInterval(interval);
    };
  }, [visualizerState]);

  useEffect(() => {
    if (streamRef.current) {
      streamRef.current.scrollTop = streamRef.current.scrollHeight;
    }
  }, [messages]);

  return (
    <section className="jarvis-chat-shell">
      <div className="jarvis-hud-grid" />
      <div className="jarvis-hud-corners" />

      <header className="jarvis-chat-header">
        <div className="jarvis-chat-header__left">
          <span className="jarvis-status-dot" />
          <h2 className="jarvis-title">NEXUS CORE // ACTIVE</h2>
        </div>
        <div className="jarvis-chat-header__right">
          <select
            className="jarvis-agent-select"
            value={selectedAgent}
            onChange={(event) => onAgentChange(event.target.value)}
          >
            <option value="">All Agents</option>
            {agents.map((agent) => (
              <option key={agent.id} value={agent.id}>
                {agent.name} ({agent.status})
              </option>
            ))}
          </select>
          <button
            type="button"
            className="jarvis-clear-btn"
            onClick={() => onDraftChange("")}
          >
            Clear
          </button>
          <button
            type="button"
            className="jarvis-history-button"
            onClick={() => setHistoryOpen((open) => !open)}
          >
            HISTORY
          </button>
        </div>
      </header>

      <main className="jarvis-chat-stream" ref={streamRef}>
        {messages.length === 0 ? (
          <article className="jarvis-message jarvis-message-assistant">
            <div className="jarvis-msg-agent-header">
              <span className="jarvis-msg-agent-icon">N</span>
              <span className="jarvis-msg-agent-name">NexusOS</span>
            </div>
            <p className="nexus-msg-typewriter">
              Awaiting command input. Try: create an agent for daily system audits.
            </p>
            <span className="jarvis-message-time">{formatMilitaryTime(Date.now())}</span>
          </article>
        ) : (
          messages.map((message) => (
            <article
              key={message.id}
              className={`jarvis-message-wrap ${message.role === "user" ? "right" : "left"} fade-slide-up`}
            >
              <div className={bubbleClass(message.role)}>
                {message.role === "assistant" && (
                  <div className="jarvis-msg-agent-header">
                    <span className="jarvis-msg-agent-icon">N</span>
                    <span className="jarvis-msg-agent-name">
                      {message.model === "system" ? "System" : "NexusOS"}
                    </span>
                  </div>
                )}
                {message.streaming && !message.content ? (
                  <div className="jarvis-typing-indicator">
                    <span />
                    <span />
                    <span />
                  </div>
                ) : (
                  <p
                    className={
                      message.role === "assistant" && !message.streaming
                        ? "nexus-msg-typewriter"
                        : undefined
                    }
                  >
                    {message.content || (message.streaming ? "..." : "")}
                  </p>
                )}
                <span className="jarvis-message-time">
                  {formatMilitaryTime(message.timestamp)}
                  {message.model ? ` // ${message.model}` : ""}
                </span>
              </div>
            </article>
          ))
        )}
        {isSending && (
          <article className="jarvis-message-wrap left fade-slide-up">
            <div className="jarvis-message jarvis-message-assistant">
              <div className="jarvis-msg-agent-header">
                <span className="jarvis-msg-agent-icon">N</span>
                <span className="jarvis-msg-agent-name">NexusOS</span>
              </div>
              <div className="jarvis-typing-indicator">
                <span />
                <span />
                <span />
              </div>
            </div>
          </article>
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
              <span className="jarvis-mic-core">{isRecording ? "REC" : "MIC"}</span>
            </button>

            <div className="jarvis-input-shell">
              <textarea
                value={draft}
                onChange={(event) => onDraftChange(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && !event.shiftKey) {
                    event.preventDefault();
                    if (!isSending) {
                      onSend();
                    }
                  }
                }}
                placeholder="Transmit directive to NexusOS..."
                rows={2}
                className="jarvis-input"
              />
            </div>

            <button type="submit" disabled={isSending} className="jarvis-send-button">
              <span className="jarvis-send-arrow">&#x27A4;</span>
              <span>SEND</span>
            </button>
          </div>

          <Suggestions
            visible={showSuggestions}
            onSelect={(value) => {
              onDraftChange(value);
            }}
          />
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
