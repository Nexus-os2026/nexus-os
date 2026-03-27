import { useEffect, useState } from "react";
import {
  browserCreateSession,
  browserExecuteTask,
  browserNavigate,
  browserGetContent,
  browserCloseSession,
  browserGetPolicy,
  browserSessionCount,
  browserScreenshot,
  listAgents,
} from "../api/backend";
import {
  ActionButton,
  EmptyState,
  Panel,
  StatusDot,
  alpha,
  commandMonoValueStyle,
  commandMutedStyle,
  commandPageStyle,
  commandScrollStyle,
  inputStyle,
  normalizeArray,
} from "./commandCenterUi";

const ACCENT = "#f97316";

interface Policy { min_autonomy_level: number; max_sessions_per_agent: number; max_steps_per_task: number; url_denylist: string[]; allow_headful: boolean; max_task_duration_secs: number }
interface ActionResult { success: boolean; action: string; result?: string; url?: string; title?: string; steps_taken?: number; error?: string; estimated_tokens: number }

export default function BrowserAgent() {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [policy, setPolicy] = useState<Policy | null>(null);
  const [sessionCount, setSessionCount] = useState(0);
  const [agents, setAgents] = useState<{ id: string; name: string }[]>([]);
  const [taskInput, setTaskInput] = useState("");
  const [urlInput, setUrlInput] = useState("");
  const [results, setResults] = useState<ActionResult[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    browserGetPolicy().then((p) => setPolicy(p as Policy)).catch(console.error);
    browserSessionCount().then(setSessionCount).catch(console.error);
    listAgents().then((a) => setAgents(normalizeArray<{ id: string; name: string }>(a).filter((x) => x.name))).catch(console.error);
  }, []);

  const handleCreateSession = (agentId: string, level: number) => {
    setLoading(true);
    browserCreateSession(agentId, level)
      .then((id) => { setSessionId(id); browserSessionCount().then(setSessionCount); })
      .catch((e) => setResults([{ success: false, action: "CreateSession", error: String(e), estimated_tokens: 0 }]))
      .finally(() => setLoading(false));
  };

  const handleTask = () => {
    if (!sessionId || !taskInput.trim()) return;
    setLoading(true);
    browserExecuteTask(sessionId, taskInput)
      .then((r) => { setResults((prev) => [r as ActionResult, ...prev]); setTaskInput(""); })
      .catch((e) => setResults((prev) => [{ success: false, action: "ExecuteTask", error: String(e), estimated_tokens: 0 }, ...prev]))
      .finally(() => setLoading(false));
  };

  const handleNavigate = () => {
    if (!sessionId || !urlInput.trim()) return;
    setLoading(true);
    browserNavigate(sessionId, urlInput)
      .then((r) => { setResults((prev) => [r as ActionResult, ...prev]); setUrlInput(""); })
      .catch((e) => setResults((prev) => [{ success: false, action: "Navigate", error: String(e), estimated_tokens: 0 }, ...prev]))
      .finally(() => setLoading(false));
  };

  const handleGetContent = () => {
    if (!sessionId) return;
    browserGetContent(sessionId)
      .then((r) => setResults((prev) => [r as ActionResult, ...prev]))
      .catch(console.error);
  };

  const handleClose = () => {
    if (!sessionId) return;
    browserCloseSession(sessionId).then(() => { setSessionId(null); browserSessionCount().then(setSessionCount); }).catch(console.error);
  };

  return (
    <div style={commandPageStyle}>
      <h1 style={{ color: ACCENT, fontSize: 22, fontWeight: 700, margin: 0, marginBottom: 4 }}>Browser Agent</h1>
      <p style={{ ...commandMutedStyle, marginBottom: 20, fontSize: 13 }}>
        Governed browser automation via browser-use. {sessionCount} active session{sessionCount !== 1 ? "s" : ""}.
      </p>

      {/* Session Manager */}
      {!sessionId ? (
        <Panel title="Create Browser Session">
          <p style={{ ...commandMutedStyle, fontSize: 12, marginBottom: 12 }}>Select an L3+ agent to start a browser session.</p>
          <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
            {agents.slice(0, 20).map((a) => (
              <button key={a.id} onClick={() => handleCreateSession(a.id, 3)} style={{
                padding: "4px 12px", borderRadius: 6, fontSize: 11, cursor: "pointer",
                border: "1px solid #334155", background: "transparent", color: "#94a3b8",
              }}>
                {a.name}
              </button>
            ))}
            {agents.length === 0 && <span style={commandMutedStyle}>No agents available</span>}
          </div>
        </Panel>
      ) : (
        <>
          {/* Active Session */}
          <Panel title={`Session: ${sessionId.slice(0, 8)}...`} action={<ActionButton accent="#ef4444" onClick={handleClose}>Close</ActionButton>}>
            {/* Task Executor */}
            <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
              <input style={{ ...inputStyle, flex: 1 }} placeholder="Enter browser task..." value={taskInput}
                onChange={(e) => setTaskInput(e.target.value)} onKeyDown={(e) => e.key === "Enter" && handleTask()} />
              <ActionButton accent={ACCENT} onClick={handleTask} disabled={loading}>
                {loading ? "Running..." : "Execute"}
              </ActionButton>
            </div>

            {/* URL Navigator */}
            <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
              <input style={{ ...inputStyle, flex: 1 }} placeholder="URL..." value={urlInput}
                onChange={(e) => setUrlInput(e.target.value)} onKeyDown={(e) => e.key === "Enter" && handleNavigate()} />
              <ActionButton accent="#3b82f6" onClick={handleNavigate}>Go</ActionButton>
              <ActionButton accent="#94a3b8" onClick={handleGetContent}>Content</ActionButton>
              <ActionButton accent="#f59e0b" onClick={() => {
                browserScreenshot(sessionId)
                  .then((r) => setResults((prev) => [{ ...r, action: "Screenshot" }, ...prev]))
                  .catch((e) => setResults((prev) => [{ success: false, action: "Screenshot", error: String(e), estimated_tokens: 0 }, ...prev]));
              }}>Screenshot</ActionButton>
            </div>
          </Panel>

          {/* Results */}
          {results.length > 0 && (
            <Panel title="Results" style={{ marginTop: 16 }}>
              <div style={{ ...commandScrollStyle, maxHeight: 400 }}>
                {results.map((r, i) => (
                  <div key={i} style={{ padding: "8px 0", borderBottom: "1px solid #1e293b", fontSize: 12 }}>
                    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                      <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
                        <StatusDot color={r.success ? "#22c55e" : "#ef4444"} />
                        <span style={{ color: "#e2e8f0" }}>{r.action}</span>
                      </span>
                      <span style={{ ...commandMutedStyle, fontSize: 10 }}>{r.estimated_tokens} tokens</span>
                    </div>
                    {r.url && <div style={{ ...commandMutedStyle, fontSize: 11, marginTop: 2 }}>{r.url}</div>}
                    {r.title && <div style={{ color: "#94a3b8", fontSize: 11 }}>{r.title}</div>}
                    {r.result && <div style={{ color: "#cbd5e1", fontSize: 11, marginTop: 4, maxHeight: 80, overflow: "auto", whiteSpace: "pre-wrap" }}>{r.result.slice(0, 500)}</div>}
                    {r.error && <div style={{ color: "#ef4444", fontSize: 11, marginTop: 2 }}>{r.error}</div>}
                  </div>
                ))}
              </div>
            </Panel>
          )}
        </>
      )}

      {/* Governance Policy */}
      {policy && (
        <Panel title="Governance Policy" style={{ marginTop: 16 }}>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 12, fontSize: 12 }}>
            <div><span style={{ color: "#94a3b8" }}>Min Level:</span> <span style={commandMonoValueStyle}>L{policy.min_autonomy_level}</span></div>
            <div><span style={{ color: "#94a3b8" }}>Max Sessions:</span> <span style={commandMonoValueStyle}>{policy.max_sessions_per_agent}</span></div>
            <div><span style={{ color: "#94a3b8" }}>Max Steps:</span> <span style={commandMonoValueStyle}>{policy.max_steps_per_task}</span></div>
            <div><span style={{ color: "#94a3b8" }}>Headful:</span> <span style={commandMonoValueStyle}>{policy.allow_headful ? "Yes" : "No"}</span></div>
            <div><span style={{ color: "#94a3b8" }}>Timeout:</span> <span style={commandMonoValueStyle}>{policy.max_task_duration_secs}s</span></div>
            <div><span style={{ color: "#94a3b8" }}>Denied URLs:</span> <span style={commandMonoValueStyle}>{policy.url_denylist.length}</span></div>
          </div>
        </Panel>
      )}
    </div>
  );
}
