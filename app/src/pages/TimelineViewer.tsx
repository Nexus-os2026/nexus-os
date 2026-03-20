import { useCallback, useEffect, useState } from "react";
import { getTemporalHistory, temporalSelectFork } from "../api/backend";
import { Play, Check, Diamond, X, Circle, RefreshCw, GitCommit } from "lucide-react";

/* ================================================================== */
/*  Types                                                              */
/* ================================================================== */

interface TimelineFork {
  fork_id: string;
  parent_id: string | null;
  description: string;
  status: string;
  score: number;
  steps: TimelineStep[];
  created_at: number;
}

interface TimelineStep {
  step_index: number;
  action: string;
  result: string;
  score: number;
  timestamp: number;
}

interface TemporalHistory {
  forks: TimelineFork[];
  decisions: TemporalDecision[];
}

interface TemporalDecision {
  decision_id: string;
  chosen_fork: string;
  reason: string;
  timestamp: number;
}

/* ================================================================== */
/*  Helpers                                                            */
/* ================================================================== */

function scoreColor(score: number): string {
  if (score >= 0.8) return "#22c55e";
  if (score >= 0.5) return "#eab308";
  if (score >= 0.3) return "#f97316";
  return "#ef4444";
}

function statusIcon(status: string): React.ReactNode {
  switch (status) {
    case "Active": return <Play size={12} />;
    case "Completed": return <Check size={12} />;
    case "Committed": return <Diamond size={12} />;
    case "Abandoned": return <X size={12} />;
    default: return <Circle size={12} />;
  }
}

function formatTime(ts: number): string {
  if (ts === 0) return "—";
  const d = new Date(ts * 1000);
  return d.toLocaleTimeString();
}

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function TimelineViewer(): JSX.Element {
  const [history, setHistory] = useState<TemporalHistory | null>(null);
  const [selectedFork, setSelectedFork] = useState<TimelineFork | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [committing, setCommitting] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const raw = await getTemporalHistory();
      const h: TemporalHistory = typeof raw === "string" ? JSON.parse(raw) : raw;
      setHistory(h);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  const handleCommit = useCallback(async (forkId: string) => {
    setCommitting(true);
    try {
      await temporalSelectFork(forkId);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setCommitting(false);
    }
  }, [refresh]);

  const forks = history?.forks ?? [];
  const decisions = history?.decisions ?? [];

  // Build tree structure
  const roots = forks.filter((f) => !f.parent_id);
  const childrenOf = (parentId: string) => forks.filter((f) => f.parent_id === parentId);

  return (
    <div style={{ padding: 24, color: "#e2e8f0", maxWidth: 1400, margin: "0 auto" }}>
      <h1 style={{ fontFamily: "monospace", fontSize: "1.8rem", color: "#22d3ee", marginBottom: 8 }}>
        TIMELINE VIEWER
      </h1>
      <p style={{ color: "#94a3b8", marginBottom: 20, fontSize: "0.85rem" }}>
        Visualize temporal forks — branching parallel timelines
      </p>

      {error && <div style={{ color: "#f87171", marginBottom: 12 }}>{error}</div>}

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 20 }}>
        {/* Fork Tree */}
        <div style={panelStyle}>
          <h3 style={headStyle}>Timeline Tree</h3>
          {forks.length === 0 ? (
            <div style={{ color: "#64748b", fontSize: "0.82rem" }}>No temporal forks active</div>
          ) : (
            <div>
              {roots.map((root) => (
                <ForkNode key={root.fork_id} fork={root} depth={0}
                  childrenOf={childrenOf} onSelect={setSelectedFork}
                  selectedId={selectedFork?.fork_id} />
              ))}
            </div>
          )}

          {/* Recent Decisions */}
          {decisions.length > 0 && (
            <div style={{ marginTop: 20 }}>
              <h4 style={{ ...headStyle, fontSize: "0.85rem" }}>Recent Decisions</h4>
              {decisions.slice(0, 5).map((d) => (
                <div key={d.decision_id} style={{ padding: "6px 0", borderBottom: "1px solid #1e293b", fontSize: "0.78rem" }}>
                  <div style={{ color: "#22d3ee" }}>Chose: {d.chosen_fork.slice(0, 12)}...</div>
                  <div style={{ color: "#94a3b8" }}>{d.reason}</div>
                  <div style={{ color: "#64748b", fontSize: "0.7rem" }}>{formatTime(d.timestamp)}</div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Fork Detail */}
        <div style={panelStyle}>
          <h3 style={headStyle}>Fork Detail</h3>
          {selectedFork ? (
            <div>
              <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 12 }}>
                <span style={{ fontFamily: "monospace", fontSize: "0.82rem" }}>
                  {selectedFork.fork_id.slice(0, 16)}...
                </span>
                <span style={{ color: scoreColor(selectedFork.score), fontWeight: 600, fontSize: "0.9rem" }}>
                  {(selectedFork.score * 100).toFixed(1)}%
                </span>
              </div>

              <div style={{ fontSize: "0.82rem", color: "#94a3b8", marginBottom: 16 }}>
                {selectedFork.description}
              </div>

              {/* Steps */}
              <div style={{ marginBottom: 16 }}>
                {selectedFork.steps.map((step) => (
                  <div key={step.step_index} style={{
                    display: "flex", gap: 12, padding: "8px 0",
                    borderLeft: `2px solid ${scoreColor(step.score)}`,
                    paddingLeft: 12, marginLeft: 8, marginBottom: 4,
                  }}>
                    <div style={{ flex: 1 }}>
                      <div style={{ fontSize: "0.78rem", color: "#e2e8f0" }}>{step.action}</div>
                      <div style={{ fontSize: "0.72rem", color: "#64748b" }}>{step.result}</div>
                    </div>
                    <span style={{ color: scoreColor(step.score), fontFamily: "monospace", fontSize: "0.75rem", minWidth: 40, textAlign: "right" }}>
                      {(step.score * 100).toFixed(0)}%
                    </span>
                  </div>
                ))}
              </div>

              {/* Commit Button */}
              {selectedFork.status === "Active" || selectedFork.status === "Completed" ? (
                <button type="button" disabled={committing}
                  onClick={() => void handleCommit(selectedFork.fork_id)}
                  style={{
                    padding: "8px 24px", borderRadius: 6, cursor: "pointer",
                    background: "#22d3ee", color: "#0f172a", border: "none",
                    fontFamily: "monospace", fontWeight: 700, fontSize: "0.85rem",
                  }}>
                  {committing ? "Committing..." : <><GitCommit size={14} style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} />Commit This Timeline</>}
                </button>
              ) : null}
            </div>
          ) : (
            <div style={{ color: "#64748b", fontSize: "0.82rem" }}>Select a fork to view details</div>
          )}
        </div>
      </div>
    </div>
  );
}

/* ================================================================== */
/*  Fork Node (recursive tree rendering)                               */
/* ================================================================== */

function ForkNode({ fork, depth, childrenOf, onSelect, selectedId }: {
  fork: TimelineFork;
  depth: number;
  childrenOf: (id: string) => TimelineFork[];
  onSelect: (f: TimelineFork) => void;
  selectedId?: string;
}): JSX.Element {
  const children = childrenOf(fork.fork_id);
  const isSelected = fork.fork_id === selectedId;

  return (
    <div style={{ marginLeft: depth * 20 }}>
      <button type="button" className="cursor-pointer" onClick={() => onSelect(fork)} style={{
        display: "flex", alignItems: "center", gap: 8, width: "100%",
        padding: "6px 10px", borderRadius: 6, cursor: "pointer",
        background: isSelected ? "rgba(34,211,238,0.1)" : "transparent",
        border: isSelected ? "1px solid #22d3ee" : "1px solid transparent",
        color: "#e2e8f0", textAlign: "left", marginBottom: 4,
      }}>
        <span style={{ color: scoreColor(fork.score), fontSize: "0.75rem" }}>
          {statusIcon(fork.status)}
        </span>
        <span style={{ flex: 1, fontSize: "0.78rem", fontFamily: "monospace" }}>
          {fork.description.slice(0, 40)}{fork.description.length > 40 ? "..." : ""}
        </span>
        <span style={{ color: scoreColor(fork.score), fontSize: "0.72rem", fontFamily: "monospace" }}>
          {(fork.score * 100).toFixed(0)}%
        </span>
      </button>
      {children.map((child) => (
        <ForkNode key={child.fork_id} fork={child} depth={depth + 1}
          childrenOf={childrenOf} onSelect={onSelect} selectedId={selectedId} />
      ))}
    </div>
  );
}

/* ================================================================== */
/*  Styles                                                             */
/* ================================================================== */

const panelStyle: React.CSSProperties = {
  background: "rgba(15,23,42,0.7)",
  border: "1px solid #1e293b",
  borderRadius: 10,
  padding: 20,
  backdropFilter: "blur(8px)",
};

const headStyle: React.CSSProperties = {
  fontFamily: "monospace",
  fontSize: "0.95rem",
  color: "#22d3ee",
  marginBottom: 14,
  paddingBottom: 8,
  borderBottom: "1px solid #1e293b",
};
