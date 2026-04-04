import { useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ── Types ──

type GenerationPhase =
  | "Analyzing"
  | "Scaffolding"
  | "Styling"
  | "Building"
  | "Scripting"
  | "Finalizing";

interface GovernanceStatus {
  owasp_passed: boolean;
  xss_clean: boolean;
  aria_present: boolean;
  signed: boolean;
}

interface BuildStartedPayload {
  BuildStarted: {
    project_name: string;
    estimated_cost: number;
    estimated_tasks: number;
    model_name: string;
    timestamp: string;
  };
}

interface GenerationProgressPayload {
  GenerationProgress: {
    phase: GenerationPhase;
    tokens_generated: number;
    estimated_total_tokens: number;
    elapsed_seconds: number;
    raw_chunk: string;
  };
}

interface BuildCompletedPayload {
  BuildCompleted: {
    project_name: string;
    total_lines: number;
    total_chars: number;
    input_tokens: number;
    output_tokens: number;
    actual_cost: number;
    model_name: string;
    elapsed_seconds: number;
    checkpoint_id: string;
    governance_status: GovernanceStatus;
    output_dir: string;
  };
}

interface BuildFailedPayload {
  BuildFailed: {
    error: string;
    tokens_consumed: number;
    cost_consumed: number;
  };
}

type BuildStreamEvent =
  | BuildStartedPayload
  | GenerationProgressPayload
  | BuildCompletedPayload
  | BuildFailedPayload;

type BuildState = "idle" | "estimating" | "generating" | "complete" | "failed";

// ── Phase config ──

const PHASE_LABELS: Record<GenerationPhase, string> = {
  Analyzing: "Analyzing prompt...",
  Scaffolding: "Building scaffold...",
  Styling: "Applying styles...",
  Building: "Generating components...",
  Scripting: "Adding interactivity...",
  Finalizing: "Finalizing output...",
};

const PHASE_ICONS: Record<GenerationPhase, string> = {
  Analyzing: "\u25B6",    // ▶
  Scaffolding: "\u2692",  // ⚒
  Styling: "\u2728",      // (sparkles - but let's use a safe one)
  Building: "\u2593",     // ▓
  Scripting: "\u2699",    // ⚙
  Finalizing: "\u2714",   // ✔
};

const PHASE_ORDER: GenerationPhase[] = [
  "Analyzing",
  "Scaffolding",
  "Styling",
  "Building",
  "Scripting",
  "Finalizing",
];

// ── Colors ──

const BG = "#0d1117";
const BG_SURFACE = "#161b22";
const TEXT = "#e6edf3";
const TEXT_SECONDARY = "#8b949e";
const ACCENT = "#58a6ff";
const SUCCESS = "#3fb950";
const ERROR = "#f85149";
const BORDER = "#30363d";

// ── Helpers ──

function formatCost(dollars: number): string {
  return "$" + dollars.toFixed(4);
}

function formatElapsed(seconds: number): string {
  return seconds.toFixed(1) + "s";
}

function clampPercent(tokens: number, total: number): number {
  if (total <= 0) return 0;
  const pct = Math.min((tokens / total) * 100, 99);
  return Math.round(pct * 10) / 10;
}

// ── Props ──

interface BuildProgressProps {
  /** Called when build completes with the output directory path. */
  onBuildComplete?: (outputDir: string, checkpointId: string) => void;
}

// ── Component ──

export function BuildProgress({ onBuildComplete }: BuildProgressProps = {}) {
  const [state, setState] = useState<BuildState>("idle");
  const [projectName, setProjectName] = useState("");
  const [modelName, setModelName] = useState("");
  const [estimatedCost, setEstimatedCost] = useState(0);
  const [estimatedTasks, setEstimatedTasks] = useState(0);

  // Generation progress
  const [phase, setPhase] = useState<GenerationPhase>("Analyzing");
  const [tokensGenerated, setTokensGenerated] = useState(0);
  const [estimatedTotalTokens, setEstimatedTotalTokens] = useState(0);
  const [elapsed, setElapsed] = useState(0);

  // Completion
  const [completionData, setCompletionData] = useState<BuildCompletedPayload["BuildCompleted"] | null>(null);

  // Failure
  const [failureData, setFailureData] = useState<BuildFailedPayload["BuildFailed"] | null>(null);

  const estimatingTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];
    let mounted = true;

    async function setup() {
      try {
        const unlisten = await listen<BuildStreamEvent>("build-stream", (event) => {
          if (!mounted) return;
          const payload = event.payload;

          if ("BuildStarted" in payload) {
            const d = payload.BuildStarted;
            setProjectName(d.project_name);
            setModelName(d.model_name);
            setEstimatedCost(d.estimated_cost);
            setEstimatedTasks(d.estimated_tasks);
            setState("estimating");

            // Transition to generating after a brief moment
            if (estimatingTimeout.current) clearTimeout(estimatingTimeout.current);
            estimatingTimeout.current = setTimeout(() => {
              if (mounted) setState("generating");
            }, 800);
          } else if ("GenerationProgress" in payload) {
            const d = payload.GenerationProgress;
            setPhase(d.phase);
            setTokensGenerated(d.tokens_generated);
            setEstimatedTotalTokens(d.estimated_total_tokens);
            setElapsed(d.elapsed_seconds);
            setState("generating");
          } else if ("BuildCompleted" in payload) {
            const d = payload.BuildCompleted;
            setCompletionData(d);
            setProjectName(d.project_name);
            setModelName(d.model_name);
            setElapsed(d.elapsed_seconds);
            setState("complete");
            if (d.output_dir) {
              onBuildComplete?.(d.output_dir, d.checkpoint_id);
            }
          } else if ("BuildFailed" in payload) {
            const d = payload.BuildFailed;
            setFailureData(d);
            setState("failed");
          }
        });
        unlisteners.push(unlisten);
      } catch {
        // Event bridge not available (desktop-only)
      }
    }

    setup();

    return () => {
      mounted = false;
      if (estimatingTimeout.current) clearTimeout(estimatingTimeout.current);
      for (const u of unlisteners) u();
    };
  }, []);

  if (state === "idle") return null;

  const containerStyle: React.CSSProperties = {
    background: BG,
    border: "1px solid " + BORDER,
    borderRadius: 8,
    padding: 16,
    marginTop: 12,
    fontFamily: "system-ui, -apple-system, sans-serif",
  };

  // ── ESTIMATING ──
  if (state === "estimating") {
    return (
      <div style={containerStyle}>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <div style={{
            width: 14,
            height: 14,
            borderRadius: "50%",
            border: "2px solid " + ACCENT,
            borderTopColor: "transparent",
            animation: "build-spin 0.8s linear infinite",
          }} />
          <span style={{ color: TEXT, fontSize: 14, fontWeight: 600 }}>Preparing build...</span>
        </div>
        <div style={{ color: TEXT_SECONDARY, fontSize: 12, marginTop: 8 }}>
          {projectName && <span>Project: <span style={{ color: TEXT }}>{projectName}</span></span>}
          {modelName && <span style={{ marginLeft: 16 }}>Model: <span style={{ color: ACCENT }}>{modelName}</span></span>}
        </div>
        {estimatedCost > 0 && (
          <div style={{ color: TEXT_SECONDARY, fontSize: 12, marginTop: 4 }}>
            Estimated cost: <span style={{ color: TEXT }}>{formatCost(estimatedCost)}</span>
            {estimatedTasks > 0 && <span style={{ marginLeft: 12 }}>Tasks: <span style={{ color: TEXT }}>{estimatedTasks}</span></span>}
          </div>
        )}
        <style>{`@keyframes build-spin { to { transform: rotate(360deg); } }`}</style>
      </div>
    );
  }

  // ── GENERATING ──
  if (state === "generating") {
    const pct = clampPercent(tokensGenerated, estimatedTotalTokens);
    const phaseIdx = PHASE_ORDER.indexOf(phase);

    return (
      <div style={containerStyle}>
        {/* Header */}
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 10 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span style={{ color: ACCENT, fontSize: 15 }}>{PHASE_ICONS[phase]}</span>
            <span style={{ color: TEXT, fontSize: 14, fontWeight: 600 }}>{PHASE_LABELS[phase]}</span>
          </div>
          <span style={{ color: TEXT_SECONDARY, fontSize: 12 }}>{formatElapsed(elapsed)}</span>
        </div>

        {/* Progress bar */}
        <div style={{
          width: "100%",
          height: 6,
          background: BG_SURFACE,
          borderRadius: 3,
          overflow: "hidden",
          marginBottom: 8,
        }}>
          <div style={{
            width: pct + "%",
            height: "100%",
            background: ACCENT,
            borderRadius: 3,
            transition: "width 0.4s ease-out",
          }} />
        </div>

        {/* Stats row */}
        <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12 }}>
          <span style={{ color: TEXT_SECONDARY }}>
            Tokens: <span style={{ color: TEXT }}>{tokensGenerated.toLocaleString()}</span>
            <span style={{ color: TEXT_SECONDARY }}> / {estimatedTotalTokens.toLocaleString()}</span>
          </span>
          <span style={{ color: TEXT_SECONDARY }}>{pct}%</span>
        </div>

        {/* Phase indicators */}
        <div style={{ display: "flex", gap: 4, marginTop: 10 }}>
          {PHASE_ORDER.map((p, i) => (
            <div key={p} style={{
              flex: 1,
              height: 3,
              borderRadius: 2,
              background: i < phaseIdx ? SUCCESS : i === phaseIdx ? ACCENT : BORDER,
              transition: "background 0.3s ease",
            }} />
          ))}
        </div>
        <div style={{ display: "flex", justifyContent: "space-between", marginTop: 4 }}>
          {PHASE_ORDER.map((p, i) => (
            <span key={p} style={{
              fontSize: 9,
              color: i <= phaseIdx ? (i === phaseIdx ? ACCENT : SUCCESS) : TEXT_SECONDARY,
              textAlign: "center",
              flex: 1,
            }}>
              {p}
            </span>
          ))}
        </div>

        {/* Meta */}
        <div style={{ color: TEXT_SECONDARY, fontSize: 11, marginTop: 8, display: "flex", gap: 16 }}>
          {projectName && <span>Project: <span style={{ color: TEXT }}>{projectName}</span></span>}
          {modelName && <span>Model: <span style={{ color: ACCENT }}>{modelName}</span></span>}
        </div>
      </div>
    );
  }

  // ── COMPLETE ──
  if (state === "complete" && completionData) {
    const d = completionData;
    const gov = d.governance_status;
    const govItems: Array<{ label: string; passed: boolean }> = [
      { label: "OWASP", passed: gov.owasp_passed },
      { label: "XSS Clean", passed: gov.xss_clean },
      { label: "ARIA", passed: gov.aria_present },
      { label: "Signed", passed: gov.signed },
    ];

    return (
      <div style={{ ...containerStyle, borderColor: SUCCESS }}>
        {/* Header */}
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span style={{ color: SUCCESS, fontSize: 16 }}>{"\u2714"}</span>
            <span style={{ color: SUCCESS, fontSize: 14, fontWeight: 700 }}>Build Complete</span>
          </div>
          <span style={{ color: TEXT_SECONDARY, fontSize: 12 }}>{formatElapsed(d.elapsed_seconds)}</span>
        </div>

        {/* Receipt grid */}
        <div style={{
          display: "grid",
          gridTemplateColumns: "1fr 1fr",
          gap: 8,
          background: BG_SURFACE,
          borderRadius: 6,
          padding: 12,
          marginBottom: 10,
        }}>
          <div style={{ fontSize: 12 }}>
            <span style={{ color: TEXT_SECONDARY }}>Project: </span>
            <span style={{ color: TEXT }}>{d.project_name}</span>
          </div>
          <div style={{ fontSize: 12 }}>
            <span style={{ color: TEXT_SECONDARY }}>Model: </span>
            <span style={{ color: ACCENT }}>{d.model_name}</span>
          </div>
          <div style={{ fontSize: 12 }}>
            <span style={{ color: TEXT_SECONDARY }}>Lines: </span>
            <span style={{ color: TEXT }}>{d.total_lines.toLocaleString()}</span>
          </div>
          <div style={{ fontSize: 12 }}>
            <span style={{ color: TEXT_SECONDARY }}>Chars: </span>
            <span style={{ color: TEXT }}>{d.total_chars.toLocaleString()}</span>
          </div>
          <div style={{ fontSize: 12 }}>
            <span style={{ color: TEXT_SECONDARY }}>Input tokens: </span>
            <span style={{ color: TEXT }}>{d.input_tokens.toLocaleString()}</span>
          </div>
          <div style={{ fontSize: 12 }}>
            <span style={{ color: TEXT_SECONDARY }}>Output tokens: </span>
            <span style={{ color: TEXT }}>{d.output_tokens.toLocaleString()}</span>
          </div>
          <div style={{ fontSize: 12 }}>
            <span style={{ color: TEXT_SECONDARY }}>Cost: </span>
            <span style={{ color: SUCCESS, fontWeight: 600 }}>{formatCost(d.actual_cost)}</span>
          </div>
          <div style={{ fontSize: 12 }}>
            <span style={{ color: TEXT_SECONDARY }}>Checkpoint: </span>
            <span style={{ color: TEXT, fontFamily: "monospace", fontSize: 11 }}>{d.checkpoint_id.slice(0, 12)}</span>
          </div>
        </div>

        {/* Governance badges */}
        <div style={{ marginBottom: 4 }}>
          <span style={{ color: TEXT_SECONDARY, fontSize: 11, textTransform: "uppercase", letterSpacing: 1 }}>Governance</span>
        </div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          {govItems.map((g) => (
            <span key={g.label} style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 4,
              padding: "3px 8px",
              borderRadius: 4,
              fontSize: 11,
              fontWeight: 600,
              background: g.passed ? "#0d1f0d" : "#1f0d0d",
              color: g.passed ? SUCCESS : ERROR,
              border: "1px solid " + (g.passed ? "#1a3a1a" : "#3a1a1a"),
            }}>
              {g.passed ? "\u2714" : "\u2718"} {g.label}
            </span>
          ))}
        </div>
      </div>
    );
  }

  // ── FAILED ──
  if (state === "failed" && failureData) {
    const d = failureData;
    return (
      <div style={{ ...containerStyle, borderColor: ERROR }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 8 }}>
          <span style={{ color: ERROR, fontSize: 16 }}>{"\u2718"}</span>
          <span style={{ color: ERROR, fontSize: 14, fontWeight: 700 }}>Build Failed</span>
        </div>
        <div style={{
          background: BG_SURFACE,
          borderRadius: 6,
          padding: 12,
          fontSize: 12,
          color: TEXT,
          fontFamily: "monospace",
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
          marginBottom: 8,
        }}>
          {d.error}
        </div>
        <div style={{ display: "flex", gap: 16, fontSize: 12 }}>
          {d.tokens_consumed > 0 && (
            <span style={{ color: TEXT_SECONDARY }}>
              Tokens used: <span style={{ color: TEXT }}>{d.tokens_consumed.toLocaleString()}</span>
            </span>
          )}
          {d.cost_consumed > 0 && (
            <span style={{ color: TEXT_SECONDARY }}>
              Cost: <span style={{ color: ERROR }}>{formatCost(d.cost_consumed)}</span>
            </span>
          )}
        </div>
      </div>
    );
  }

  return null;
}
