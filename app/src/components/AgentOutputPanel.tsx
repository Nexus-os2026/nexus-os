import { useEffect, useRef, useState, useCallback } from "react";
import {
  Terminal,
  ChevronDown,
  ChevronRight,
  CheckCircle2,
  XCircle,
  Loader2,
  Search,
  Brain,
  FileText,
  Code2,
  Globe,
  Zap,
  Sparkles,
} from "lucide-react";

/* ─── types ─── */

export interface StepDetail {
  action: string;
  status: string;
  result: string;
  fuel_cost: number;
}

interface Props {
  steps: StepDetail[];
  phase: string | null;
  running: boolean;
  totalSteps: number;
  fuelConsumed: number;
  query: string;
  resultSummary?: string | null;
}

/* ─── helpers ─── */

const ACTION_ICONS: Record<string, typeof Brain> = {
  llm_query: Brain,
  web_search: Search,
  file_read: FileText,
  file_write: Code2,
  shell_command: Terminal,
  web_fetch: Globe,
  image_generate: Sparkles,
};

const ACTION_COLORS: Record<string, string> = {
  llm_query: "#a78bfa",
  web_search: "#22d3ee",
  file_read: "#60a5fa",
  file_write: "#f59e0b",
  shell_command: "#10b981",
  web_fetch: "#06b6d4",
  image_generate: "#ec4899",
};

function actionColor(action: string): string {
  return ACTION_COLORS[action] ?? "#64748b";
}

function ActionIcon({ action }: { action: string }) {
  const Icon = ACTION_ICONS[action] ?? Zap;
  return <Icon size={13} />;
}

function truncate(text: string, max: number): string {
  if (text.length <= max) return text;
  return text.slice(0, max) + "…";
}

/* ─── step row ─── */

function StepRow({
  step,
  index,
  total,
  isLast,
}: {
  step: StepDetail;
  index: number;
  total: number;
  isLast: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
  const succeeded = step.status === "Succeeded" || step.status === "succeeded";
  const failed = step.status === "Failed" || step.status === "failed";
  const isFinalLlm = isLast && step.action === "llm_query" && succeeded;

  const accentColor = actionColor(step.action);

  return (
    <div
      style={{
        borderLeft: `2px solid ${succeeded ? accentColor : failed ? "#ef4444" : "#334155"}`,
        marginLeft: 8,
        paddingLeft: 12,
        paddingBottom: isLast ? 0 : 12,
        position: "relative",
      }}
    >
      {/* timeline dot */}
      <div
        style={{
          position: "absolute",
          left: -6,
          top: 4,
          width: 10,
          height: 10,
          borderRadius: "50%",
          background: succeeded ? accentColor : failed ? "#ef4444" : "#334155",
          border: "2px solid #0d1117",
          boxShadow: succeeded ? `0 0 8px ${accentColor}55` : "none",
        }}
      />

      {/* header */}
      <button type="button"
        onClick={() => setExpanded(!expanded)}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          width: "100%",
          background: "transparent",
          border: "none",
          color: "#e0e0e0",
          cursor: "pointer",
          padding: "2px 0",
          fontSize: 13,
          textAlign: "left",
        }}
      >
        {expanded ? (
          <ChevronDown size={12} style={{ opacity: 0.5, flexShrink: 0 }} />
        ) : (
          <ChevronRight size={12} style={{ opacity: 0.5, flexShrink: 0 }} />
        )}

        <span
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 4,
            fontSize: 11,
            fontWeight: 600,
            color: "#94a3b8",
            fontFamily: "var(--font-mono, monospace)",
            flexShrink: 0,
          }}
        >
          {index + 1}/{total}
        </span>

        <span
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 4,
            color: accentColor,
            fontSize: 12,
            fontWeight: 600,
            fontFamily: "var(--font-mono, monospace)",
          }}
        >
          <ActionIcon action={step.action} />
          {step.action}
        </span>

        <span style={{ marginLeft: "auto", display: "flex", alignItems: "center", gap: 4, flexShrink: 0 }}>
          {succeeded ? (
            <CheckCircle2 size={13} color="#22c55e" />
          ) : failed ? (
            <XCircle size={13} color="#ef4444" />
          ) : (
            <Loader2 size={13} color="#00e5ff" style={{ animation: "spin 1s linear infinite" }} />
          )}
          {step.fuel_cost > 0 && (
            <span style={{ fontSize: 10, color: "#64748b", fontFamily: "var(--font-mono, monospace)" }}>
              {step.fuel_cost.toFixed(1)} fuel
            </span>
          )}
        </span>
      </button>

      {/* result preview — collapsed */}
      {!expanded && step.result && !isFinalLlm && (
        <div
          style={{
            fontSize: 12,
            color: "#94a3b8",
            marginTop: 4,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            maxWidth: "100%",
            fontFamily: "var(--font-mono, monospace)",
          }}
        >
          {truncate(step.result, 120)}
        </div>
      )}

      {/* result — expanded */}
      {expanded && step.result && (
        <pre
          style={{
            margin: "6px 0 0",
            padding: "8px 10px",
            background: "#0a0f1a",
            border: "1px solid #1e3a5f",
            borderRadius: 6,
            fontSize: 12,
            color: "#cbd5e1",
            lineHeight: 1.6,
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
            maxHeight: 300,
            overflowY: "auto",
            fontFamily: "var(--font-mono, monospace)",
          }}
        >
          {step.result}
        </pre>
      )}

      {/* Final LLM response — always shown prominently */}
      {isFinalLlm && step.result && (
        <div
          style={{
            marginTop: 8,
            padding: "12px 14px",
            background: "linear-gradient(135deg, rgba(0, 229, 255, 0.06), rgba(167, 139, 250, 0.06))",
            border: "1px solid rgba(0, 229, 255, 0.2)",
            borderRadius: 8,
            fontSize: 13,
            color: "#e0e0e0",
            lineHeight: 1.7,
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              marginBottom: 8,
              fontSize: 11,
              fontWeight: 700,
              color: "#00e5ff",
              letterSpacing: "0.06em",
              textTransform: "uppercase",
              fontFamily: "var(--font-mono, monospace)",
            }}
          >
            <Sparkles size={12} />
            Agent Response
          </div>
          {step.result}
        </div>
      )}
    </div>
  );
}

/* ─── result summary block ─── */

function ResultSummaryBlock({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }).catch(() => {});
  }, [text]);

  return (
    <div
      style={{
        marginTop: 12,
        padding: "14px 16px",
        background: "linear-gradient(135deg, rgba(0, 229, 255, 0.06), rgba(34, 197, 94, 0.06))",
        border: "1px solid rgba(34, 197, 94, 0.25)",
        borderRadius: 8,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 10,
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
            fontSize: 11,
            fontWeight: 700,
            color: "#22c55e",
            letterSpacing: "0.06em",
            textTransform: "uppercase",
            fontFamily: "var(--font-mono, monospace)",
          }}
        >
          <CheckCircle2 size={12} />
          Result
        </div>
        <button
          type="button"
          onClick={handleCopy}
          style={{
            background: "rgba(255,255,255,0.06)",
            border: "1px solid rgba(255,255,255,0.1)",
            borderRadius: 4,
            padding: "3px 10px",
            fontSize: 11,
            color: copied ? "#22c55e" : "#94a3b8",
            cursor: "pointer",
            fontFamily: "var(--font-mono, monospace)",
          }}
        >
          {copied ? "Copied!" : "Copy"}
        </button>
      </div>
      <div
        style={{
          fontSize: 13,
          color: "#e0e0e0",
          lineHeight: 1.7,
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
        }}
      >
        {text}
      </div>
    </div>
  );
}

/* ─── main panel ─── */

export default function AgentOutputPanel({
  steps,
  phase,
  running,
  totalSteps,
  fuelConsumed,
  query,
  resultSummary,
}: Props) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);

  // auto-scroll to latest
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [steps.length]);

  // Find the last LLM result for prominent display
  const hasError = phase != null && phase.startsWith("Error");
  const hasResult = resultSummary != null && resultSummary.length > 0;
  const isComplete = phase === "Complete";
  const hasOutput = steps.length > 0 || running || hasError || hasResult || isComplete;

  return (
    <div
      style={{
        background: "#0d1117",
        border: "1px solid #1e3a5f",
        borderRadius: 8,
        overflow: "hidden",
        marginBottom: 12,
      }}
    >
      {/* header */}
      <div
        style={{
          padding: "8px 14px",
          borderBottom: "1px solid #1e3a5f",
          display: "flex",
          alignItems: "center",
          gap: 8,
          background: "linear-gradient(90deg, rgba(0, 229, 255, 0.04), transparent)",
        }}
      >
        <Terminal size={14} color="#00e5ff" />
        <span
          style={{
            fontSize: 13,
            fontWeight: 700,
            color: "#e0e0e0",
            letterSpacing: "0.04em",
          }}
        >
          Agent Output
        </span>

        {running && (
          <Loader2
            size={13}
            color="#00e5ff"
            style={{ animation: "spin 1s linear infinite" }}
          />
        )}

        {phase && (
          <span
            style={{
              marginLeft: "auto",
              fontSize: 11,
              fontWeight: 600,
              color:
                phase === "Complete"
                  ? "#22c55e"
                  : phase.startsWith("Error")
                    ? "#ef4444"
                    : "#00e5ff",
              fontFamily: "var(--font-mono, monospace)",
              letterSpacing: "0.06em",
              textTransform: "uppercase",
            }}
          >
            {phase}
            {totalSteps > 0 &&
              ` · ${steps.length}/${totalSteps} steps · ${fuelConsumed.toFixed(0)} fuel`}
          </span>
        )}
      </div>

      {/* body */}
      <div
        ref={scrollRef}
        style={{
          maxHeight: 480,
          overflowY: "auto",
          padding: "12px 14px",
        }}
      >
        {/* empty state */}
        {!hasOutput && (
          <div
            style={{
              padding: "32px 16px",
              textAlign: "center",
              color: "#475569",
              fontSize: 13,
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: 8,
            }}
          >
            <Terminal size={24} style={{ opacity: 0.3 }} />
            Run a goal to see agent output here
          </div>
        )}

        {/* query label */}
        {query && hasOutput && (
          <div
            style={{
              fontSize: 12,
              color: "#64748b",
              marginBottom: 12,
              padding: "6px 10px",
              background: "#161b22",
              borderRadius: 6,
              border: "1px solid #21262d",
              fontFamily: "var(--font-mono, monospace)",
            }}
          >
            <span style={{ color: "#00e5ff", fontWeight: 600 }}>Goal:</span>{" "}
            {query}
          </div>
        )}

        {/* error banner */}
        {phase && phase.startsWith("Error") && (
          <div
            style={{
              padding: "10px 14px",
              background: "#2d0a0a",
              border: "1px solid #ef444466",
              borderRadius: 6,
              marginBottom: 12,
              fontSize: 13,
              color: "#fca5a5",
              lineHeight: 1.5,
              wordBreak: "break-word",
            }}
          >
            {phase.replace(/^Error:\s*/, "")}
          </div>
        )}

        {/* step timeline */}
        {steps.map((step, i) => (
          <StepRow
            key={`${i}-${step.action}`}
            step={step}
            index={i}
            total={totalSteps || steps.length}
            isLast={i === steps.length - 1 && !running}
          />
        ))}

        {/* running indicator */}
        {running && (
          <div
            style={{
              borderLeft: "2px solid #334155",
              marginLeft: 8,
              paddingLeft: 12,
              paddingTop: 8,
              position: "relative",
            }}
          >
            <div
              style={{
                position: "absolute",
                left: -5,
                top: 10,
                width: 8,
                height: 8,
                borderRadius: "50%",
                background: "#00e5ff",
                animation: "pulse 1.5s ease-in-out infinite",
                boxShadow: "0 0 10px rgba(0, 229, 255, 0.5)",
              }}
            />
            <span
              style={{
                fontSize: 12,
                color: "#00e5ff",
                fontFamily: "var(--font-mono, monospace)",
                display: "flex",
                alignItems: "center",
                gap: 6,
              }}
            >
              <Loader2 size={12} style={{ animation: "spin 1s linear infinite" }} />
              {phase && phase !== "Starting..." ? `Phase: ${phase}` : "Executing next step..."}
            </span>
          </div>
        )}

        {/* result summary from goal completion */}
        {!running && (() => {
          // Prefer the last LLM result from steps (actual agent output)
          const lastLlmResult = [...steps]
            .reverse()
            .find(s => s.action === "llm_query" && (s.status === "Succeeded" || s.status === "succeeded") && s.result);
          const displayText = lastLlmResult?.result || resultSummary;
          if (displayText) {
            return <ResultSummaryBlock text={displayText} />;
          }
          // If complete but no text, show the phase as confirmation
          if (isComplete) {
            return (
              <div style={{
                marginTop: 12,
                padding: "10px 14px",
                background: "rgba(34, 197, 94, 0.08)",
                border: "1px solid rgba(34, 197, 94, 0.2)",
                borderRadius: 6,
                fontSize: 13,
                color: "#22c55e",
              }}>
                Goal completed successfully.
                {fuelConsumed > 0 && ` (${fuelConsumed.toFixed(0)} fuel used)`}
              </div>
            );
          }
          return null;
        })()}

        <div ref={bottomRef} />
      </div>
    </div>
  );
}
