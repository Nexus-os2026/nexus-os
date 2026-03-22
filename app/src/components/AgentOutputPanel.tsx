import { useEffect, useState, useRef } from "react";
import { hasDesktopRuntime, getAgentOutputs } from "../api/backend";
import { Terminal, Clock, Loader } from "lucide-react";

interface OutputEntry {
  id: string;
  time: string | number;
  action: string;
  type: string;
  content: string;
}

interface Props {
  agentId: string;
  maxHeight?: string;
}

export default function AgentOutputPanel({ agentId, maxHeight = "400px" }: Props) {
  const [outputs, setOutputs] = useState<OutputEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!agentId || !hasDesktopRuntime()) {
      setLoading(false);
      return;
    }

    let active = true;

    const poll = async () => {
      try {
        const raw = await getAgentOutputs(agentId, 50);
        if (!active) return;
        const parsed: OutputEntry[] = JSON.parse(raw);
        setOutputs(parsed);
      } catch {
        // silent
      } finally {
        if (active) setLoading(false);
      }
    };

    poll();
    const interval = setInterval(poll, 3000);
    return () => {
      active = false;
      clearInterval(interval);
    };
  }, [agentId]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [outputs.length]);

  const formatTime = (t: string | number) => {
    if (typeof t === "number") {
      return new Date(t * 1000).toLocaleTimeString();
    }
    return new Date(t).toLocaleTimeString();
  };

  return (
    <div
      style={{
        background: "var(--bg-secondary, #1e293b)",
        border: "1px solid var(--border, #334155)",
        borderRadius: 8,
        overflow: "hidden",
        fontFamily: "var(--font-mono, monospace)",
      }}
    >
      <div
        style={{
          padding: "0.5rem 0.75rem",
          borderBottom: "1px solid var(--border, #334155)",
          display: "flex",
          alignItems: "center",
          gap: 8,
          fontSize: "0.8rem",
          fontWeight: 600,
          color: "var(--text-primary, #e2e8f0)",
        }}
      >
        <Terminal size={14} />
        Agent Output
        {loading && <Loader size={12} style={{ animation: "spin 1s linear infinite" }} />}
        <span style={{ marginLeft: "auto", opacity: 0.5, fontSize: "0.7rem" }}>
          {outputs.length} events
        </span>
      </div>

      <div
        style={{
          maxHeight,
          overflowY: "auto",
          padding: "0.5rem",
        }}
      >
        {outputs.length === 0 && !loading && (
          <div
            style={{
              padding: "2rem",
              textAlign: "center",
              opacity: 0.5,
              fontSize: "0.8rem",
            }}
          >
            No output yet. Start the agent to see live activity.
          </div>
        )}
        {outputs.map((entry) => (
          <div
            key={entry.id}
            style={{
              padding: "0.4rem 0.5rem",
              borderBottom: "1px solid rgba(255,255,255,0.05)",
              fontSize: "0.75rem",
              lineHeight: 1.5,
            }}
          >
            <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 2 }}>
              <Clock size={10} style={{ opacity: 0.4 }} />
              <span style={{ opacity: 0.5 }}>{formatTime(entry.time)}</span>
              <span
                style={{
                  background: "rgba(129,140,248,0.15)",
                  color: "#818cf8",
                  padding: "1px 6px",
                  borderRadius: 4,
                  fontSize: "0.65rem",
                  fontWeight: 600,
                }}
              >
                {entry.action}
              </span>
            </div>
            <div style={{ opacity: 0.8, wordBreak: "break-all" }}>
              {entry.type === "code" ? (
                <pre style={{ margin: 0, whiteSpace: "pre-wrap" }}>
                  <code>{entry.content}</code>
                </pre>
              ) : (
                <span>{entry.content.slice(0, 500)}</span>
              )}
            </div>
          </div>
        ))}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}
