import { useCallback, useEffect, useState } from "react";
import {
  swfAssignMember,
  swfCreateProject,
  swfEstimateCost,
  swfGetCost,
  swfGetPipelineStages,
  swfGetPolicy,
  swfGetProject,
  swfListProjects,
  swfStartPipeline,
} from "../api/backend";
import { alpha, commandPageStyle } from "./commandCenterUi";

const ACCENT = "#ec4899";
const GREEN = "#22c55e";
const RED = "#ef4444";
const BLUE = "#3b82f6";
const YELLOW = "#eab308";

const STAGE_COLORS: Record<string, string> = {
  Requirements: "#8b5cf6",
  Architecture: "#3b82f6",
  Implementation: "#22c55e",
  Testing: "#f59e0b",
  Review: "#ec4899",
  Deployment: "#06b6d4",
  Verification: "#64748b",
};

const ROLES = ["ProductManager", "Architect", "Developer", "QualityAssurance", "DevOps"];

const cardStyle: React.CSSProperties = {
  background: alpha("#1e1e2e", 0.7),
  borderRadius: 10,
  padding: 16,
  border: "1px solid " + alpha("#ffffff", 0.08),
};

const labelStyle: React.CSSProperties = {
  fontSize: 11,
  color: "#888",
  textTransform: "uppercase" as const,
  letterSpacing: 1,
  marginBottom: 4,
};

const btnStyle: React.CSSProperties = {
  padding: "6px 14px",
  borderRadius: 6,
  border: "none",
  cursor: "pointer",
  fontWeight: 600,
  fontSize: 12,
};

const inputStyle: React.CSSProperties = {
  padding: 8,
  borderRadius: 6,
  background: "#2a2a3e",
  color: "#e0e0e0",
  border: "1px solid #444",
  width: "100%",
  boxSizing: "border-box" as const,
};

export default function SoftwareFactory() {
  const [projects, setProjects] = useState<any[]>([]);
  const [stages, setStages] = useState<any[]>([]);
  const [policy, setPolicy] = useState<any>(null);
  const [estimatedCost, setEstimatedCost] = useState(0);
  const [selectedProject, setSelectedProject] = useState<any>(null);
  const [cost, setCost] = useState<any>(null);

  const [title, setTitle] = useState("");
  const [userRequest, setUserRequest] = useState("");
  const [agentId, setAgentId] = useState("");
  const [agentName, setAgentName] = useState("");
  const [role, setRole] = useState("Developer");
  const [autonomy, setAutonomy] = useState(4);
  const [status, setStatus] = useState("");

  useEffect(() => {
    Promise.all([
      swfListProjects().catch(() => []),
      swfGetPipelineStages().catch(() => []),
      swfGetPolicy().catch(() => null),
      swfEstimateCost().catch(() => 0),
    ]).then(([p, s, pol, est]) => {
      setProjects(Array.isArray(p) ? p : []);
      setStages(Array.isArray(s) ? s : []);
      setPolicy(pol);
      setEstimatedCost(typeof est === "number" ? est : 0);
    });
  }, []);

  const refresh = useCallback(async () => {
    const p = await swfListProjects().catch(() => []);
    setProjects(Array.isArray(p) ? p : []);
    if (selectedProject) {
      const updated = await swfGetProject(selectedProject.id).catch(() => null);
      if (updated) {
        setSelectedProject(updated);
        const c = await swfGetCost(updated.id).catch(() => null);
        setCost(c);
      }
    }
  }, [selectedProject]);

  const handleCreate = useCallback(async () => {
    if (!title.trim()) return;
    const id = await swfCreateProject(title, userRequest);
    setStatus(`Created project ${id.slice(0, 8)}`);
    setTitle("");
    setUserRequest("");
    refresh();
  }, [title, userRequest, refresh]);

  const handleAssign = useCallback(async () => {
    if (!selectedProject || !agentId.trim()) return;
    await swfAssignMember(selectedProject.id, agentId, agentName || agentId, role, autonomy);
    setAgentId("");
    setAgentName("");
    refresh();
  }, [selectedProject, agentId, agentName, role, autonomy, refresh]);

  const handleStart = useCallback(async () => {
    if (!selectedProject) return;
    await swfStartPipeline(selectedProject.id);
    refresh();
  }, [selectedProject, refresh]);

  const selectProject = useCallback(async (p: any) => {
    const full = await swfGetProject(p.id).catch(() => p);
    setSelectedProject(full);
    const c = await swfGetCost(p.id).catch(() => null);
    setCost(c);
  }, []);

  return (
    <div style={{ ...commandPageStyle, padding: 24, color: "#e0e0e0" }}>
      <h1 style={{ fontSize: 22, fontWeight: 700, marginBottom: 4 }}>
        <span style={{ color: ACCENT }}>Software Factory</span>
      </h1>
      <p style={{ color: "#888", fontSize: 13, marginBottom: 16 }}>
        Autonomous SDLC pipeline — requirements, architecture, implementation, testing, review, deployment, verification.
      </p>
      {status && <div style={{ fontSize: 12, color: GREEN, marginBottom: 8 }}>{status}</div>}

      <div style={{ display: "grid", gridTemplateColumns: "300px 1fr", gap: 16 }}>
        {/* Left: Create + Projects */}
        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          <div style={cardStyle}>
            <div style={labelStyle}>New Project</div>
            <div style={{ display: "flex", flexDirection: "column", gap: 6, marginTop: 6 }}>
              <input placeholder="Project title" value={title} onChange={(e) => setTitle(e.target.value)} style={inputStyle} />
              <textarea placeholder="Describe what to build..." value={userRequest} onChange={(e) => setUserRequest(e.target.value)} rows={3} style={{ ...inputStyle, resize: "vertical" }} />
              <button onClick={handleCreate} style={{ ...btnStyle, background: ACCENT, color: "#fff" }}>Create Project</button>
            </div>
          </div>

          <div style={cardStyle}>
            <div style={labelStyle}>Projects ({projects.length})</div>
            {projects.map((p) => (
              <div key={p.id} onClick={() => selectProject(p)} style={{
                padding: 8, borderRadius: 6, marginTop: 6, cursor: "pointer",
                background: selectedProject?.id === p.id ? alpha(ACCENT, 0.15) : alpha("#fff", 0.02),
                border: selectedProject?.id === p.id ? `1px solid ${ACCENT}` : "1px solid transparent",
              }}>
                <div style={{ fontSize: 13, fontWeight: 600 }}>{p.title}</div>
                <div style={{ fontSize: 10, color: "#888" }}>
                  {typeof p.status === "string" ? p.status : Object.keys(p.status)[0]} | {p.team?.length || 0} members
                </div>
              </div>
            ))}
          </div>

          {policy && (
            <div style={cardStyle}>
              <div style={labelStyle}>Policy</div>
              <div style={{ fontSize: 12, display: "grid", gridTemplateColumns: "1fr 1fr", gap: 4, marginTop: 4 }}>
                <div>Min L: <span style={{ color: BLUE }}>{policy.min_autonomy_level}</span></div>
                <div>Max projects: <span style={{ color: BLUE }}>{policy.max_concurrent_projects}</span></div>
                <div>Estimated: <span style={{ color: ACCENT }}>{(estimatedCost / 1e6).toFixed(0)} NXC</span></div>
              </div>
            </div>
          )}
        </div>

        {/* Right: Project detail */}
        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          {!selectedProject ? (
            <div style={{ ...cardStyle, textAlign: "center", padding: 40, color: "#555" }}>Select or create a project</div>
          ) : (
            <>
              {/* Pipeline visualization */}
              <div style={cardStyle}>
                <div style={labelStyle}>Pipeline</div>
                <div style={{ display: "flex", gap: 4, marginTop: 8 }}>
                  {stages.map((s) => {
                    const isCurrent = s.name === (selectedProject.current_stage || "Requirements");
                    const isPast = stages.findIndex((x: any) => x.name === s.name) < stages.findIndex((x: any) => x.name === (selectedProject.current_stage || "Requirements"));
                    return (
                      <div key={s.name} style={{
                        flex: 1, padding: 8, borderRadius: 6, textAlign: "center",
                        background: isCurrent ? alpha(STAGE_COLORS[s.display_name] || ACCENT, 0.25) : isPast ? alpha(GREEN, 0.1) : alpha("#fff", 0.02),
                        border: isCurrent ? `2px solid ${STAGE_COLORS[s.display_name] || ACCENT}` : "1px solid transparent",
                      }}>
                        <div style={{ fontSize: 10, fontWeight: 600, color: STAGE_COLORS[s.display_name] || "#888" }}>{s.display_name}</div>
                        <div style={{ fontSize: 9, color: "#555" }}>{s.responsible_role}</div>
                        <div style={{ fontSize: 9, color: "#555" }}>{(s.base_cost / 1e6).toFixed(0)} NXC</div>
                      </div>
                    );
                  })}
                </div>
              </div>

              {/* Team + assign */}
              <div style={cardStyle}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <div style={labelStyle}>Team ({selectedProject.team?.length || 0})</div>
                  {selectedProject.status === "Initializing" && (
                    <button onClick={handleStart} style={{ ...btnStyle, background: GREEN, color: "#000" }}>Start Pipeline</button>
                  )}
                </div>
                <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginTop: 6 }}>
                  {(selectedProject.team || []).map((m: any) => (
                    <div key={m.agent_id} style={{ padding: "4px 8px", borderRadius: 4, background: alpha(ACCENT, 0.1), fontSize: 11 }}>
                      <span style={{ color: ACCENT }}>{m.agent_name}</span>
                      <span style={{ color: "#888" }}> ({m.role})</span>
                      <span style={{ color: "#555" }}> L{m.autonomy_level}</span>
                    </div>
                  ))}
                </div>
                {(selectedProject.status === "Initializing" || selectedProject.status === "InProgress") && (
                  <div style={{ display: "flex", gap: 6, marginTop: 8 }}>
                    <input placeholder="Agent ID" value={agentId} onChange={(e) => setAgentId(e.target.value)} style={{ ...inputStyle, flex: 1 }} />
                    <input placeholder="Name" value={agentName} onChange={(e) => setAgentName(e.target.value)} style={{ ...inputStyle, flex: 1 }} />
                    <select value={role} onChange={(e) => setRole(e.target.value)} style={{ ...inputStyle, width: 140 }}>
                      {ROLES.map((r) => <option key={r} value={r}>{r}</option>)}
                    </select>
                    <input type="number" min={1} max={5} value={autonomy} onChange={(e) => setAutonomy(Number(e.target.value))} style={{ ...inputStyle, width: 50 }} />
                    <button onClick={handleAssign} style={{ ...btnStyle, background: ACCENT, color: "#fff" }}>Assign</button>
                  </div>
                )}
              </div>

              {/* Cost */}
              {cost && (
                <div style={cardStyle}>
                  <div style={labelStyle}>Cost Breakdown</div>
                  <div style={{ display: "flex", gap: 12, marginTop: 6, flexWrap: "wrap" }}>
                    {cost.stages?.map(([name, c]: [string, number]) => (
                      <div key={name} style={{ fontSize: 11 }}>
                        <span style={{ color: "#888" }}>{name}:</span> <span style={{ color: YELLOW }}>{(c / 1e6).toFixed(0)} NXC</span>
                      </div>
                    ))}
                  </div>
                  <div style={{ fontSize: 13, fontWeight: 600, marginTop: 6, color: ACCENT }}>
                    Total: {(cost.total / 1e6).toFixed(0)} NXC
                  </div>
                </div>
              )}

              {/* Quality gates */}
              {selectedProject.quality_gates?.length > 0 && (
                <div style={cardStyle}>
                  <div style={labelStyle}>Quality Gates</div>
                  {selectedProject.quality_gates.map((g: any, i: number) => (
                    <div key={i} style={{ display: "flex", gap: 8, fontSize: 12, marginTop: 4, alignItems: "center" }}>
                      <span style={{ color: g.passed ? GREEN : RED }}>{g.passed ? "PASS" : "FAIL"}</span>
                      <span style={{ color: "#888" }}>{g.stage}</span>
                      <span style={{ color: YELLOW }}>{(g.score * 100).toFixed(0)}%</span>
                      {g.blocking_issues?.length > 0 && (
                        <span style={{ color: RED, fontSize: 10 }}>{g.blocking_issues.join("; ")}</span>
                      )}
                    </div>
                  ))}
                </div>
              )}

              {/* History */}
              {selectedProject.history?.length > 0 && (
                <div style={cardStyle}>
                  <div style={labelStyle}>History ({selectedProject.history.length})</div>
                  <div style={{ maxHeight: 200, overflow: "auto" }}>
                    {selectedProject.history.slice().reverse().map((e: any, i: number) => (
                      <div key={i} style={{ fontSize: 11, padding: "3px 0", display: "flex", gap: 8 }}>
                        <span style={{ color: STAGE_COLORS[e.stage] || "#888", minWidth: 80 }}>{e.stage}</span>
                        <span style={{ color: "#888" }}>{e.event_type}</span>
                        <span style={{ flex: 1 }}>{e.description}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Outcome */}
              {selectedProject.status === "Completed" && (
                <div style={{ ...cardStyle, borderColor: GREEN }}>
                  <div style={{ ...labelStyle, color: GREEN }}>Project Completed</div>
                  <div style={{ fontSize: 13, marginTop: 4 }}>
                    Duration: {selectedProject.duration_secs || "N/A"}s | Artifacts: {selectedProject.artifacts?.length || 0}
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
