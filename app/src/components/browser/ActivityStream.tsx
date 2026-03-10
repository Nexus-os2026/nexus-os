import { useEffect, useRef } from "react";
import type { ActivityMessage } from "../../types";

interface ActivityStreamProps {
  messages: ActivityMessage[];
}

const TYPE_COLORS: Record<string, string> = {
  searching: "#f59e0b",
  reading: "#3b82f6",
  extracting: "#8b5cf6",
  deciding: "#10b981",
  navigating: "#00ffd5",
  blocked: "#ef4444",
  info: "#94a3b8",
};

const TYPE_ICONS: Record<string, string> = {
  searching: "⌕",
  reading: "◎",
  extracting: "⬡",
  deciding: "◈",
  navigating: "→",
  blocked: "⛔",
  info: "ℹ",
};

const AGENT_COLORS = [
  "#00ffd5",
  "#3b82f6",
  "#f59e0b",
  "#8b5cf6",
  "#10b981",
  "#ef4444",
];

function agentColor(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = (hash * 31 + name.charCodeAt(i)) | 0;
  }
  return AGENT_COLORS[Math.abs(hash) % AGENT_COLORS.length];
}

function formatTime(ts: number): string {
  const d = new Date(ts);
  return d.toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function ActivityStream({ messages }: ActivityStreamProps): JSX.Element {
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length]);

  return (
    <div className="activity-stream">
      <div className="activity-stream-header">
        <span className="activity-stream-title">⌁ Agent Activity</span>
        <span className="activity-stream-count">{messages.length}</span>
      </div>
      <div className="activity-stream-list">
        {messages.length === 0 && (
          <div className="activity-stream-empty">
            No activity yet. Navigate to a URL to begin.
          </div>
        )}
        {messages.map((msg) => (
          <div key={msg.id} className="activity-stream-item">
            <div className="activity-item-header">
              <span
                className="activity-type-badge"
                style={{
                  color: TYPE_COLORS[msg.message_type] ?? "#94a3b8",
                  borderColor: TYPE_COLORS[msg.message_type] ?? "#94a3b8",
                }}
              >
                {TYPE_ICONS[msg.message_type] ?? "•"}{" "}
                {msg.message_type}
              </span>
              <span
                className="activity-agent-name"
                style={{ color: agentColor(msg.agent_name) }}
              >
                {msg.agent_name}
              </span>
              <span className="activity-timestamp">
                {formatTime(msg.timestamp)}
              </span>
            </div>
            <div className="activity-item-content">{msg.content}</div>
          </div>
        ))}
        <div ref={endRef} />
      </div>
    </div>
  );
}
