import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

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

interface DilatedSession {
  task: string;
  iterations: number;
  final_score: number;
  agent_ids: string[];
}

interface AgentEntry {
  id: string;
  name: string;
  status: string;
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

function statusLabel(status: string): { text: string; color: string } {
  switch (status) {
    case "Active": return { text: "ACTIVE", color: "#22d3ee" };
    case "Completed": return { text: "COMPLETED", color: "#22c55e" };
    case "Committed": return { text: "COMMITTED", color: "#a78bfa" };
    case "Abandoned": return { text: "PRUNED", color: "#64748b" };
    default: return { text: status, color: "#94a3b8" };
  }
}

function formatTime(ts: number): string {
  if (ts === 0) return "—";
  return new Date(ts * 1000).toLocaleTimeString();
}

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function TemporalEngine(): JSX.Element {
  const [history, setHistory] = useState<TemporalHistory | null>(null);
  const [selectedFork, setSelectedFork] = useState<TimelineFork | null>(null);
  const [agents, setAgents] = useState<AgentEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<"timelines" | "fork" | "dilated">("timelines");

  // Fork creation
  const [forkRequest, setForkRequest] = useState("");
  const [forkAgentId, setForkAgentId] = useState("");
  const [forkCount, setForkCount] = useState(3);
  const [forking, setForking] = useState(false);

  // Dilated session
  const [dilatedTask, setDilatedTask] = useState("");
  const [dilatedAgents, setDilatedAgents] = useState("");
  const [dilatedIterations, setDilatedIterations] = useState(5);
  const [dilatedResult, setDilatedResult] = useState<DilatedSession | null>(null);
  const [running, setRunning] = useState(false);

  // Commit/rollback
  const [committing, setCommitting] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const results = await Promise.allSettled([
        invoke<TemporalHistory>("get_temporal_history", { count: 10 }),
        invoke<AgentEntry[]>("list_agents"),
      ]);
      if (results[0].status === "fulfilled") setHistory(results[0].value);
      if (results[1].status === "fulfilled") setAgents(Array.isArray(results[1].value) ? results[1].value : []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  const handleFork = useCallback(async () => {
    if (!forkRequest.trim()) return;
    setForking(true);
    setError(null);
    try {
      await invoke("temporal_fork", { request: forkRequest, agentId: forkAgentId || null, count: forkCount });
      await refresh();
      setForkRequest("");
      setTab("timelines");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setForking(false);
    }
  }, [forkRequest, forkAgentId, forkCount, refresh]);

  const handleCommit = useCallback(async (forkId: string) => {
    setCommitting(true);
    try {
      await invoke("temporal_select_fork", { decisionId: selectedFork?.parent_id ?? forkId, forkId });
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setCommitting(false);
    }
  }, [refresh, selectedFork]);

  const handleRollback = useCallback(async (decisionId: string) => {
    try {
      await invoke("temporal_rollback", { decisionId });
      setSelectedFork(null);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [refresh]);

  const handleDilated = useCallback(async () => {
    if (!dilatedTask.trim()) return;
    setRunning(true);
    setError(null);
    try {
      const agentIds = dilatedAgents.split(",").map((s) => s.trim()).filter(Boolean);
      const result = await invoke<DilatedSession>("run_dilated_session", {
        task: dilatedTask, agents: agentIds, iterations: dilatedIterations,
      });
      setDilatedResult(result);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRunning(false);
    }
  }, [dilatedTask, dilatedAgents, dilatedIterations]);

  const forks = history?.forks ?? [];
  const decisions = history?.decisions ?? [];

  // Group forks by decision (parent_id)
  const rootForks = forks.filter((f) => !f.parent_id);
  const childrenOf = (parentId: string) => forks.filter((f) => f.parent_id === parentId);

  return (
    <div style={{ padding: 24, color: "#e2e8f0", maxWidth: 1400, margin: "0 auto" }}>
      <h1 style={{ fontFamily: "monospace", fontSize: "1.8rem", color: "#22d3ee", marginBottom: 8 }}>
        TEMPORAL ENGINE
      </h1>
      <p style={{ color: "#94a3b8", marginBottom: 20, fontSize: "0.85rem" }}>
        Fork timelines, compare parallel approaches, commit the best path
      </p>

      {/* Tabs */}
      <div style={{ display: "flex", gap: 8, marginBottom: 20 }}>
        {(["timelines", "fork", "dilated"] as const).map((t) => (
          <button key={t} type="button" onClick={() => setTab(t)} style={{
            padding: "6px 18px", borderRadius: 6, border: "1px solid #334155", cursor: "pointer",
            background: tab === t ? "#22d3ee" : "transparent",
            color: tab === t ? "#0f172a" : "#94a3b8",
            fontFamily: "monospace", fontSize: "0.82rem", fontWeight: 600,
          }}>
            {t === "timelines" ? "TIMELINES" : t === "fork" ? "NEW FORK" : "DILATED SESSION"}
          </button>
        ))}
      </div>

      {error && <div style={{ color: "#f87171", marginBottom: 12, fontSize: "0.85rem" }}>{error}</div>}

      {/* Timelines View */}
      {tab === "timelines" && (
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 20 }}>
          {/* Fork List */}
          <div style={panelStyle}>
            <h3 style={headStyle}>Timeline Tree</h3>
            {forks.length === 0 ? (
              <div style={{ color: "#64748b", fontSize: "0.82rem" }}>No temporal forks. Create one to get started.</div>
            ) : (
              <div>
                {rootForks.map((root) => (
                  <ForkNode key={root.fork_id} fork={root} depth={0}
                    childrenOf={childrenOf} onSelect={setSelectedFork}
                    selectedId={selectedFork?.fork_id} />
                ))}
                {rootForks.length === 0 && forks.map((f) => (
                  <ForkNode key={f.fork_id} fork={f} depth={0}
                    childrenOf={() => []} onSelect={setSelectedFork}
                    selectedId={selectedFork?.fork_id} />
                ))}
              </div>
            )}

            {/* Decisions */}
            {decisions.length > 0 && (
              <div style={{ marginTop: 20 }}>
                <h4 style={{ ...headStyle, fontSize: "0.85rem" }}>Decision History</h4>
                {decisions.slice(0, 8).map((d) => (
                  <div key={d.decision_id} style={{ padding: "6px 0", borderBottom: "1px solid #1e293b", fontSize: "0.78rem" }}>
                    <div style={{ display: "flex", justifyContent: "space-between" }}>
                      <span style={{ color: "#22d3ee" }}>Chose: {d.chosen_fork.slice(0, 12)}...</span>
                      <span style={{ color: "#64748b" }}>{formatTime(d.timestamp)}</span>
                    </div>
                    <div style={{ color: "#94a3b8", marginTop: 2 }}>{d.reason}</div>
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
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
                  <div>
                    <span style={{ fontFamily: "monospace", fontSize: "0.82rem" }}>
                      {selectedFork.fork_id.slice(0, 16)}...
                    </span>
                    <span style={{ marginLeft: 8, fontSize: "0.72rem", padding: "2px 6px", borderRadius: 4, background: `${statusLabel(selectedFork.status).color}20`, color: statusLabel(selectedFork.status).color }}>
                      {statusLabel(selectedFork.status).text}
                    </span>
                  </div>
                  <span style={{ color: scoreColor(selectedFork.score), fontWeight: 700, fontSize: "1.2rem", fontFamily: "monospace" }}>
                    {(selectedFork.score * 10).toFixed(1)}/10
                  </span>
                </div>

                <div style={{ fontSize: "0.85rem", color: "#94a3b8", marginBottom: 16 }}>
                  {selectedFork.description}
                </div>

                {/* Steps */}
                <div style={{ marginBottom: 16 }}>
                  {selectedFork.steps.map((step) => (
                    <div key={step.step_index} style={{
                      display: "flex", gap: 12, padding: "8px 0",
                      borderLeft: `3px solid ${scoreColor(step.score)}`,
                      paddingLeft: 12, marginLeft: 4, marginBottom: 4,
                    }}>
                      <div style={{ flex: 1 }}>
                        <div style={{ fontSize: "0.82rem", color: "#e2e8f0" }}>Step {step.step_index + 1}: {step.action}</div>
                        <div style={{ fontSize: "0.72rem", color: "#64748b", marginTop: 2 }}>{step.result}</div>
                      </div>
                      <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
                        <div style={{ width: 60, height: 6, background: "#1e293b", borderRadius: 3, overflow: "hidden" }}>
                          <div style={{ width: `${step.score * 100}%`, height: "100%", background: scoreColor(step.score), borderRadius: 3 }} />
                        </div>
                        <span style={{ color: scoreColor(step.score), fontFamily: "monospace", fontSize: "0.72rem", minWidth: 30, textAlign: "right" }}>
                          {(step.score * 10).toFixed(0)}/10
                        </span>
                      </div>
                    </div>
                  ))}
                </div>

                {/* Actions */}
                <div style={{ display: "flex", gap: 8 }}>
                  {(selectedFork.status === "Active" || selectedFork.status === "Completed") && (
                    <button type="button" disabled={committing}
                      onClick={() => void handleCommit(selectedFork.fork_id)}
                      style={{ ...btnStyle, background: "#22d3ee", color: "#0f172a", border: "none", fontWeight: 700 }}>
                      {committing ? "Committing..." : "Commit Selected"}
                    </button>
                  )}
                  <button type="button"
                    onClick={() => void handleRollback(selectedFork.fork_id)}
                    style={{ ...btnStyle, background: "rgba(239,68,68,0.15)", borderColor: "#ef4444", color: "#ef4444" }}>
                    Rollback
                  </button>
                </div>
              </div>
            ) : (
              <div style={{ color: "#64748b", fontSize: "0.82rem" }}>Select a fork to view details</div>
            )}
          </div>
        </div>
      )}

      {/* New Fork */}
      {tab === "fork" && (
        <div style={panelStyle}>
          <h3 style={headStyle}>Create New Fork</h3>
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            <div>
              <label style={labelStyle}>Decision to explore</label>
              <input type="text" value={forkRequest} onChange={(e) => setForkRequest(e.target.value)}
                placeholder="e.g. Design database schema" style={inputStyle} />
            </div>
            <div style={{ display: "flex", gap: 12 }}>
              <div style={{ flex: 1 }}>
                <label style={labelStyle}>Agent (optional)</label>
                <select value={forkAgentId} onChange={(e) => setForkAgentId(e.target.value)} style={inputStyle}>
                  <option value="">Auto-select</option>
                  {agents.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}
                </select>
              </div>
              <div style={{ width: 120 }}>
                <label style={labelStyle}>Fork count</label>
                <input type="number" value={forkCount} onChange={(e) => setForkCount(Number(e.target.value))}
                  min={2} max={10} style={inputStyle} />
              </div>
            </div>
            <button type="button" onClick={() => void handleFork()} disabled={forking || !forkRequest.trim()} style={btnStyle}>
              {forking ? "Forking..." : "Create Forks"}
            </button>
          </div>
        </div>
      )}

      {/* Dilated Session */}
      {tab === "dilated" && (
        <div style={panelStyle}>
          <h3 style={headStyle}>Time-Dilated Session</h3>
          <p style={{ color: "#94a3b8", fontSize: "0.82rem", marginBottom: 16 }}>
            Compress multiple iterations of work into a single burst. Agents iterate on a task rapidly.
          </p>
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            <div>
              <label style={labelStyle}>Task</label>
              <input type="text" value={dilatedTask} onChange={(e) => setDilatedTask(e.target.value)}
                placeholder="e.g. Build a web scraper for news articles" style={inputStyle} />
            </div>
            <div style={{ display: "flex", gap: 12 }}>
              <div style={{ flex: 1 }}>
                <label style={labelStyle}>Agent IDs (comma separated)</label>
                <input type="text" value={dilatedAgents} onChange={(e) => setDilatedAgents(e.target.value)}
                  placeholder="agent-1, agent-2" style={inputStyle} />
              </div>
              <div style={{ width: 120 }}>
                <label style={labelStyle}>Iterations</label>
                <input type="number" value={dilatedIterations} onChange={(e) => setDilatedIterations(Number(e.target.value))}
                  min={1} max={50} style={inputStyle} />
              </div>
            </div>
            <button type="button" onClick={() => void handleDilated()} disabled={running || !dilatedTask.trim()} style={btnStyle}>
              {running ? "Running..." : "Start Dilated Session"}
            </button>
          </div>

          {dilatedResult && (
            <div style={{ marginTop: 20, padding: 16, background: "rgba(34,211,238,0.05)", border: "1px solid #22d3ee", borderRadius: 8 }}>
              <div style={{ fontSize: "0.85rem", color: "#22d3ee", fontWeight: 600, marginBottom: 8 }}>Session Complete</div>
              <StatRow label="Task" value={dilatedResult.task} />
              <StatRow label="Iterations" value={dilatedResult.iterations} />
              <StatRow label="Final Score" value={`${(dilatedResult.final_score * 10).toFixed(1)}/10`} />
              <StatRow label="Agents" value={dilatedResult.agent_ids.length} />
            </div>
          )}
        </div>
      )}
    </div>
  );
}

/* ================================================================== */
/*  Fork Node                                                          */
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
  const sl = statusLabel(fork.status);

  return (
    <div style={{ marginLeft: depth * 20 }}>
      <button type="button" onClick={() => onSelect(fork)} style={{
        display: "flex", alignItems: "center", gap: 8, width: "100%",
        padding: "6px 10px", borderRadius: 6, cursor: "pointer",
        background: isSelected ? "rgba(34,211,238,0.1)" : "transparent",
        border: isSelected ? "1px solid #22d3ee" : "1px solid transparent",
        color: "#e2e8f0", textAlign: "left", marginBottom: 4,
      }}>
        <span style={{ width: 8, height: 8, borderRadius: "50%", background: sl.color, display: "inline-block", flexShrink: 0 }} />
        <span style={{ flex: 1, fontSize: "0.78rem", fontFamily: "monospace" }}>
          {fork.description.slice(0, 40)}{fork.description.length > 40 ? "..." : ""}
        </span>
        <span style={{ color: scoreColor(fork.score), fontSize: "0.72rem", fontFamily: "monospace" }}>
          {(fork.score * 10).toFixed(1)}
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
/*  Sub-components & Styles                                            */
/* ================================================================== */

function StatRow({ label, value }: { label: string; value: string | number }): JSX.Element {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", padding: "3px 0", fontSize: "0.82rem" }}>
      <span style={{ color: "#94a3b8" }}>{label}</span>
      <span style={{ fontFamily: "monospace", color: "#e2e8f0" }}>{value}</span>
    </div>
  );
}

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

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "8px 12px",
  background: "#0f172a",
  border: "1px solid #334155",
  borderRadius: 6,
  color: "#e2e8f0",
  fontFamily: "monospace",
  fontSize: "0.82rem",
};

const labelStyle: React.CSSProperties = {
  display: "block",
  fontSize: "0.72rem",
  color: "#64748b",
  marginBottom: 4,
  textTransform: "uppercase",
};

const btnStyle: React.CSSProperties = {
  padding: "8px 20px",
  background: "rgba(34,211,238,0.15)",
  border: "1px solid #22d3ee",
  borderRadius: 6,
  color: "#22d3ee",
  cursor: "pointer",
  fontFamily: "monospace",
  fontSize: "0.82rem",
  fontWeight: 600,
};
