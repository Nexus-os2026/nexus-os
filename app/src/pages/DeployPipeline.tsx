import { useState, useCallback, useMemo, useEffect } from "react";
import {
  factoryCreateProject,
  factoryBuildProject,
  factoryTestProject,
  factoryRunPipeline,
  factoryListProjects,
  factoryGetBuildHistory,
  hasDesktopRuntime,
} from "../api/backend";
import "./deploy-pipeline.css";

/* ─── types ─── */
type View = "projects" | "pipeline" | "history" | "logs";

interface FactoryProject {
  id: string;
  name: string;
  language: string;
  source_dir: string;
  build_command: string;
  test_command: string;
  deploy_command: string | null;
  status: string;
  created_at: number;
  last_build_at: number | null;
}

interface BuildResult {
  project_id: string;
  success: boolean;
  output: string;
  errors: string[];
  duration_ms: number;
  timestamp: number;
}

interface TestResult {
  project_id: string;
  success: boolean;
  passed: number;
  failed: number;
  skipped: number;
  output: string;
  duration_ms: number;
  timestamp: number;
}

interface DeployResult {
  project_id: string;
  success: boolean;
  environment: string;
  output: string;
  url: string | null;
  timestamp: number;
}

interface PipelineResult {
  project_id: string;
  build: BuildResult;
  test: TestResult | null;
  deploy: DeployResult | null;
  overall_success: boolean;
  total_duration_ms: number;
}

interface LogEntry {
  timestamp: number;
  level: "info" | "warn" | "error" | "success";
  message: string;
  stage?: string;
}

type PipelineStage = "idle" | "building" | "testing" | "deploying" | "done" | "failed";

const LANGUAGES = ["rust", "javascript", "typescript", "python", "go"];
const STATUS_COLORS: Record<string, string> = {
  Created: "#64748b",
  Building: "#f59e0b",
  Testing: "#3b82f6",
  Deploying: "#a78bfa",
  Running: "#22c55e",
  Stopped: "#64748b",
};

/* ─── component ─── */
export default function DeployPipeline() {
  const [view, setView] = useState<View>("projects");
  const [projects, setProjects] = useState<FactoryProject[]>([]);
  const [selectedProject, setSelectedProject] = useState<string | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [buildHistory, setBuildHistory] = useState<BuildResult[]>([]);
  const [pipelineStage, setPipelineStage] = useState<PipelineStage>("idle");
  const [lastPipelineResult, setLastPipelineResult] = useState<PipelineResult | null>(null);
  const [loading, setLoading] = useState(false);

  // new project form
  const [showNewProject, setShowNewProject] = useState(false);
  const [newName, setNewName] = useState("");
  const [newLang, setNewLang] = useState("rust");
  const [newSourceDir, setNewSourceDir] = useState(".");

  // HITL state
  const [hitlPending, setHitlPending] = useState<string | null>(null);

  const isDesktop = hasDesktopRuntime();
  const hasPipelines = projects.length > 0;

  const addLog = useCallback((level: LogEntry["level"], message: string, stage?: string) => {
    setLogs(prev => [{ timestamp: Date.now(), level, message, stage }, ...prev].slice(0, 200));
  }, []);

  const activeProject = useMemo(
    () => projects.find(p => p.id === selectedProject) ?? null,
    [projects, selectedProject],
  );

  /* ─── load projects ─── */
  const loadProjects = useCallback(async () => {
    if (!isDesktop) return;
    try {
      const raw = await factoryListProjects();
      const parsed: FactoryProject[] = JSON.parse(raw);
      setProjects(parsed);
    } catch (e) {
      addLog("error", `Failed to load projects: ${e}`);
    }
  }, [isDesktop, addLog]);

  useEffect(() => {
    loadProjects();
  }, [loadProjects]);

  /* ─── load build history ─── */
  const loadBuildHistory = useCallback(async (projectId: string) => {
    if (!isDesktop) return;
    try {
      const raw = await factoryGetBuildHistory(projectId);
      const parsed: BuildResult[] = JSON.parse(raw);
      setBuildHistory(parsed);
    } catch {
      setBuildHistory([]);
    }
  }, [isDesktop]);

  useEffect(() => {
    if (selectedProject) loadBuildHistory(selectedProject);
  }, [selectedProject, loadBuildHistory]);

  /* ─── actions ─── */
  const createProject = useCallback(async () => {
    if (!newName.trim()) return;
    setLoading(true);
    try {
      const raw = await factoryCreateProject(newName, newLang, newSourceDir);
      const project: FactoryProject = JSON.parse(raw);
      setProjects(prev => [project, ...prev]);
      setSelectedProject(project.id);
      setShowNewProject(false);
      setNewName("");
      addLog("success", `Project created: ${project.name} (${project.language})`);
    } catch (e) {
      addLog("error", `Create failed: ${e}`);
    }
    setLoading(false);
  }, [newName, newLang, newSourceDir, addLog]);

  const buildProject = useCallback(async (projectId: string) => {
    setPipelineStage("building");
    addLog("info", "Build started...", "build");
    try {
      const raw = await factoryBuildProject(projectId);
      const result: BuildResult = JSON.parse(raw);
      if (result.success) {
        addLog("success", `Build succeeded in ${result.duration_ms}ms`, "build");
      } else {
        addLog("error", `Build failed: ${result.errors.join("; ")}`, "build");
      }
      result.output.split("\n").filter(Boolean).forEach(line => {
        addLog(result.success ? "info" : "warn", line, "build");
      });
      await loadProjects();
      await loadBuildHistory(projectId);
      setPipelineStage(result.success ? "done" : "failed");
      return result;
    } catch (e) {
      addLog("error", `Build error: ${e}`, "build");
      setPipelineStage("failed");
      return null;
    }
  }, [addLog, loadProjects, loadBuildHistory]);

  const testProject = useCallback(async (projectId: string) => {
    setPipelineStage("testing");
    addLog("info", "Tests started...", "test");
    try {
      const raw = await factoryTestProject(projectId);
      const result: TestResult = JSON.parse(raw);
      if (result.success) {
        addLog("success", `Tests passed: ${result.passed} passed, ${result.failed} failed, ${result.skipped} skipped (${result.duration_ms}ms)`, "test");
      } else {
        addLog("error", `Tests failed: ${result.passed} passed, ${result.failed} failed`, "test");
      }
      result.output.split("\n").filter(Boolean).forEach(line => {
        addLog(result.success ? "info" : "warn", line, "test");
      });
      await loadProjects();
      setPipelineStage(result.success ? "done" : "failed");
      return result;
    } catch (e) {
      addLog("error", `Test error: ${e}`, "test");
      setPipelineStage("failed");
      return null;
    }
  }, [addLog, loadProjects]);

  const runFullPipeline = useCallback(async (projectId: string) => {
    setPipelineStage("building");
    addLog("info", "Full pipeline started: build → test → deploy", "pipeline");
    try {
      const raw = await factoryRunPipeline(projectId);
      const result: PipelineResult = JSON.parse(raw);
      setLastPipelineResult(result);

      // Log build stage
      if (result.build.success) {
        addLog("success", `Build succeeded (${result.build.duration_ms}ms)`, "build");
      } else {
        addLog("error", `Build failed: ${result.build.errors.join("; ")}`, "build");
      }
      result.build.output.split("\n").filter(Boolean).forEach(line => {
        addLog("info", line, "build");
      });

      // Log test stage
      if (result.test) {
        if (result.test.success) {
          addLog("success", `Tests passed: ${result.test.passed}/${result.test.passed + result.test.failed} (${result.test.duration_ms}ms)`, "test");
        } else {
          addLog("error", `Tests failed: ${result.test.failed} failures`, "test");
        }
      }

      // Log deploy stage
      if (result.deploy) {
        if (result.deploy.success) {
          addLog("success", `Deployed to ${result.deploy.url ?? "local"} (${result.deploy.environment})`, "deploy");
        } else {
          addLog("error", `Deploy failed: ${result.deploy.output}`, "deploy");
        }
      }

      addLog(
        result.overall_success ? "success" : "error",
        `Pipeline ${result.overall_success ? "completed" : "failed"} in ${result.total_duration_ms}ms`,
        "pipeline",
      );
      await loadProjects();
      await loadBuildHistory(projectId);
      setPipelineStage(result.overall_success ? "done" : "failed");
    } catch (e) {
      addLog("error", `Pipeline error: ${e}`, "pipeline");
      setPipelineStage("failed");
    }
  }, [addLog, loadProjects, loadBuildHistory]);

  const handleRunPipeline = useCallback((projectId: string) => {
    // HITL for deploy (Tier2)
    setHitlPending(projectId);
  }, []);

  const formatTime = (ts: number) => {
    const diff = Date.now() - ts;
    if (diff < 60000) return "now";
    if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
    if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;
    return `${Math.floor(diff / 86400000)}d ago`;
  };

  const getStatusColor = (status: string) => {
    // status comes as e.g. "Building" or {"Failed":"msg"}
    if (typeof status === "string" && status in STATUS_COLORS) return STATUS_COLORS[status];
    if (typeof status === "object") return "#ef4444";
    return "#64748b";
  };

  const getStatusLabel = (status: string | Record<string, string>) => {
    if (typeof status === "string") return status;
    if (typeof status === "object" && status !== null) {
      const key = Object.keys(status)[0];
      return `${key}: ${(status as Record<string, string>)[key]}`;
    }
    return "Unknown";
  };

  /* ─── render ─── */
  return (
    <div className="dp-container">
      {/* ─── Sidebar ─── */}
      <aside className="dp-sidebar">
        <div className="dp-sidebar-header">
          <h2 className="dp-sidebar-title">Deploy Pipeline</h2>
          <button className="dp-new-btn" onClick={() => setShowNewProject(true)}>+ Project</button>
        </div>

        <div className="dp-views">
          {([["projects", "⚡", "Projects"], ["pipeline", "▶", "Pipeline"], ["history", "◷", "History"], ["logs", "▤", "Logs"]] as const).map(([id, icon, label]) => (
            <button key={id} className={`dp-view-btn ${view === id ? "active" : ""}`} onClick={() => setView(id)}>
              <span>{icon}</span> {label}
            </button>
          ))}
        </div>

        {/* Project list */}
        <div className="dp-env-summary">
          <div className="dp-section-header">Projects ({projects.length})</div>
          {projects.map(p => (
            <div
              key={p.id}
              className={`dp-env-card ${selectedProject === p.id ? "active" : ""}`}
              onClick={() => { setSelectedProject(p.id); setView("pipeline"); }}
            >
              <span className="dp-env-dot" style={{ background: getStatusColor(p.status) }} />
              <span className="dp-env-name">{p.name}</span>
              <span className="dp-env-count">{p.language}</span>
            </div>
          ))}
          {projects.length === 0 && (
            <div className="dp-audit-entry">
              No deployment pipelines configured. Create a pipeline to deploy your agent-built projects.
            </div>
          )}
        </div>

        {/* Pipeline stage */}
        {pipelineStage !== "idle" && (
          <div className="dp-providers">
            <div className="dp-section-header">Pipeline Status</div>
            <div className="dp-provider-btn active" style={{ color: pipelineStage === "failed" ? "#ef4444" : pipelineStage === "done" ? "#22c55e" : "#f59e0b" }}>
              {pipelineStage === "building" ? "Building..." :
               pipelineStage === "testing" ? "Testing..." :
               pipelineStage === "deploying" ? "Deploying..." :
               pipelineStage === "done" ? "Complete" : "Failed"}
            </div>
          </div>
        )}

        {/* Cloud providers note */}
        <div className="dp-audit">
          <div className="dp-section-header">Cloud Providers</div>
          <div className="dp-audit-entry" style={{ opacity: 0.5 }}>▲ Vercel — Coming Soon</div>
          <div className="dp-audit-entry" style={{ opacity: 0.5 }}>◆ Netlify — Coming Soon</div>
          <div className="dp-audit-entry" style={{ opacity: 0.5 }}>☁ Cloudflare — Coming Soon</div>
          <div className="dp-audit-entry">⬢ Self-Hosted — Active</div>
        </div>
      </aside>

      {/* ─── Main ─── */}
      <div className="dp-main">

        {/* ═══ HITL APPROVAL ═══ */}
        {hitlPending && (
          <div className="dp-hitl-overlay">
            <div className="dp-hitl-dialog">
              <div className="dp-hitl-icon">⛨</div>
              <h3>HITL Approval Required</h3>
              <p className="dp-hitl-msg">
                Running the full pipeline (build → test → deploy) for project <strong>{activeProject?.name}</strong>.
                Deploy step will execute the configured deploy command.
              </p>
              <div className="dp-hitl-meta">
                <span>Action: Full Pipeline Run</span>
                <span>Governance: Tier 2 — HITL mandatory for deploy</span>
              </div>
              <div className="dp-hitl-actions">
                <button className="dp-hitl-approve" onClick={() => {
                  const pid = hitlPending;
                  setHitlPending(null);
                  runFullPipeline(pid);
                }}>Approve & Execute</button>
                <button className="dp-hitl-deny" onClick={() => { setHitlPending(null); addLog("warn", "Pipeline denied by user"); }}>Deny</button>
              </div>
            </div>
          </div>
        )}

        {/* ═══ NEW PROJECT MODAL ═══ */}
        {showNewProject && (
          <div className="dp-hitl-overlay">
            <div className="dp-new-dialog">
              <h3 className="dp-new-title">New Project</h3>
              <div className="dp-form-grid">
                <div className="dp-form-group">
                  <label>Project Name</label>
                  <input value={newName} onChange={e => setNewName(e.target.value)} placeholder="my-app" />
                </div>
                <div className="dp-form-group">
                  <label>Language</label>
                  <select value={newLang} onChange={e => setNewLang(e.target.value)}>
                    {LANGUAGES.map(l => <option key={l} value={l}>{l}</option>)}
                  </select>
                </div>
                <div className="dp-form-group">
                  <label>Source Directory</label>
                  <input value={newSourceDir} onChange={e => setNewSourceDir(e.target.value)} placeholder="." />
                </div>
              </div>
              <div className="dp-form-actions">
                <button className="dp-form-deploy" onClick={createProject} disabled={loading || !newName.trim()}>
                  {loading ? "Creating..." : "Create Project"}
                </button>
                <button className="dp-form-cancel" onClick={() => setShowNewProject(false)}>Cancel</button>
              </div>
            </div>
          </div>
        )}

        {/* ═══ PROJECTS VIEW ═══ */}
        {view === "projects" && (
          <div className="dp-deploys">
            <div className="dp-deploys-header">
              <h3 className="dp-view-title">Factory Projects</h3>
              <div className="dp-filters">
                <button className="dp-new-btn" onClick={() => setShowNewProject(true)}>+ New Project</button>
              </div>
            </div>

            {!isDesktop ? (
              <div className="dp-envs">
                <div className="dp-env-panel" style={{ textAlign: "center", padding: "3rem" }}>
                  <h4>Desktop Runtime Required</h4>
                  <p style={{ color: "#94a3b8", marginTop: "0.5rem" }}>
                    Deploy Pipeline requires the Tauri desktop runtime to execute real builds.
                  </p>
                </div>
              </div>
            ) : projects.length === 0 ? (
              <div className="dp-envs">
                <div className="dp-env-panel" style={{ textAlign: "center", padding: "3rem" }}>
                  <h4>No Deployment Pipelines Configured</h4>
                  <p style={{ color: "#94a3b8", marginTop: "0.5rem" }}>
                    No deployment pipelines configured. Create a pipeline to deploy your agent-built projects.
                  </p>
                  <button className="dp-new-btn" style={{ marginTop: "1rem" }} onClick={() => setShowNewProject(true)}>+ Create First Project</button>
                </div>
              </div>
            ) : (
              <div className="dp-deploys-grid">
                <div className="dp-deploy-list">
                  {projects.map(p => (
                    <div
                      key={p.id}
                      className={`dp-deploy-item ${selectedProject === p.id ? "active" : ""}`}
                      onClick={() => { setSelectedProject(p.id); setView("pipeline"); }}
                    >
                      <div className="dp-deploy-status">
                        <span className="dp-status-dot" style={{ background: getStatusColor(p.status) }} />
                        <span className="dp-status-text" style={{ color: getStatusColor(p.status) }}>{getStatusLabel(p.status)}</span>
                      </div>
                      <div className="dp-deploy-info">
                        <div className="dp-deploy-project">{p.name}</div>
                        <div className="dp-deploy-meta">
                          <span>{p.language}</span>
                          <span>{p.source_dir}</span>
                          <span>{p.last_build_at ? formatTime(p.last_build_at * 1000) : "never built"}</span>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

        {/* ═══ PIPELINE VIEW ═══ */}
        {view === "pipeline" && (
          <div className="dp-deploys">
            <div className="dp-deploys-header">
              <h3 className="dp-view-title">▶ Pipeline {activeProject ? `— ${activeProject.name}` : ""}</h3>
            </div>

            {activeProject ? (
              <div className="dp-deploys-grid">
                <div className="dp-deploy-detail" style={{ flex: 1, maxWidth: "100%" }}>
                  <div className="dp-detail-header">
                    <h4>{activeProject.name}</h4>
                    <span className="dp-status-badge" style={{
                      background: getStatusColor(activeProject.status) + "22",
                      color: getStatusColor(activeProject.status),
                      borderColor: getStatusColor(activeProject.status) + "44",
                    }}>
                      {getStatusLabel(activeProject.status)}
                    </span>
                  </div>
                  <div className="dp-detail-grid">
                    <div className="dp-detail-row"><span>Language</span><span>{activeProject.language}</span></div>
                    <div className="dp-detail-row"><span>Source</span><span className="dp-mono">{activeProject.source_dir}</span></div>
                    <div className="dp-detail-row"><span>Build Cmd</span><span className="dp-mono">{activeProject.build_command}</span></div>
                    <div className="dp-detail-row"><span>Test Cmd</span><span className="dp-mono">{activeProject.test_command}</span></div>
                    <div className="dp-detail-row"><span>Deploy Cmd</span><span className="dp-mono">{activeProject.deploy_command ?? "—"}</span></div>
                  </div>

                  {/* Pipeline stages visual */}
                  <div style={{ display: "flex", gap: "0.5rem", margin: "1.5rem 0" }}>
                    {(["Build", "Test", "Deploy"] as const).map((stage, i) => {
                      const stageKey = stage.toLowerCase() as "build" | "test" | "deploy";
                      const isActive = pipelineStage === (stageKey + "ing").replace("tesing", "testing").replace("deploying", "deploying");
                      const isPassed = lastPipelineResult && (
                        (stageKey === "build" && lastPipelineResult.build.success) ||
                        (stageKey === "test" && lastPipelineResult.test?.success) ||
                        (stageKey === "deploy" && lastPipelineResult.deploy?.success)
                      );
                      const isFailed = lastPipelineResult && (
                        (stageKey === "build" && !lastPipelineResult.build.success) ||
                        (stageKey === "test" && lastPipelineResult.test && !lastPipelineResult.test.success) ||
                        (stageKey === "deploy" && lastPipelineResult.deploy && !lastPipelineResult.deploy.success)
                      );
                      return (
                        <div key={stage} style={{
                          flex: 1, padding: "1rem", borderRadius: 8,
                          border: `1px solid ${isActive ? "#f59e0b" : isPassed ? "#22c55e" : isFailed ? "#ef4444" : "#1e293b"}`,
                          background: isActive ? "#f59e0b11" : isPassed ? "#22c55e11" : isFailed ? "#ef444411" : "#0f172a",
                          textAlign: "center",
                        }}>
                          <div style={{ fontSize: "1.2rem", marginBottom: "0.25rem" }}>
                            {isPassed ? "✓" : isFailed ? "✗" : isActive ? "⟳" : `${i + 1}`}
                          </div>
                          <div style={{ fontSize: "0.85rem", color: "#e2e8f0" }}>{stage}</div>
                        </div>
                      );
                    })}
                  </div>

                  {/* Pipeline result */}
                  {lastPipelineResult && lastPipelineResult.project_id === selectedProject && (
                    <div style={{
                      padding: "1rem", borderRadius: 8, marginBottom: "1rem",
                      border: `1px solid ${lastPipelineResult.overall_success ? "#22c55e44" : "#ef444444"}`,
                      background: lastPipelineResult.overall_success ? "#22c55e11" : "#ef444411",
                    }}>
                      <div style={{ fontWeight: 600, color: lastPipelineResult.overall_success ? "#22c55e" : "#ef4444" }}>
                        Pipeline {lastPipelineResult.overall_success ? "Succeeded" : "Failed"} — {lastPipelineResult.total_duration_ms}ms
                      </div>
                      {lastPipelineResult.deploy?.url && (
                        <div style={{ marginTop: "0.5rem", color: "var(--nexus-accent)" }}>
                          URL: {lastPipelineResult.deploy.url}
                        </div>
                      )}
                    </div>
                  )}

                  <div className="dp-detail-actions">
                    <button
                      className="dp-btn-retry"
                      onClick={() => buildProject(activeProject.id)}
                      disabled={pipelineStage === "building" || pipelineStage === "testing" || pipelineStage === "deploying"}
                    >
                      Build Only
                    </button>
                    <button
                      className="dp-btn-retry"
                      onClick={() => testProject(activeProject.id)}
                      disabled={pipelineStage === "building" || pipelineStage === "testing" || pipelineStage === "deploying"}
                    >
                      Test Only
                    </button>
                    <button
                      className="dp-btn-rollback"
                      onClick={() => handleRunPipeline(activeProject.id)}
                      disabled={pipelineStage === "building" || pipelineStage === "testing" || pipelineStage === "deploying"}
                      style={{ background: "#22c55e22", color: "#22c55e", borderColor: "#22c55e44" }}
                    >
                      Run Full Pipeline
                    </button>
                  </div>
                </div>
              </div>
            ) : (
              <div className="dp-envs">
                <div className="dp-env-panel" style={{ textAlign: "center", padding: "3rem" }}>
                  <h4>No Deployment Pipelines Configured</h4>
                  <p style={{ color: "#94a3b8", marginTop: "0.5rem" }}>
                    No deployment pipelines configured. Create a pipeline to deploy your agent-built projects.
                  </p>
                  <button className="dp-new-btn" style={{ marginTop: "1rem" }} onClick={() => setShowNewProject(true)}>+ Create First Project</button>
                </div>
              </div>
            )}
          </div>
        )}

        {/* ═══ HISTORY VIEW ═══ */}
        {view === "history" && (
          <div className="dp-deploys">
            <div className="dp-deploys-header">
              <h3 className="dp-view-title">Build History {activeProject ? `— ${activeProject.name}` : ""}</h3>
            </div>
            {buildHistory.length === 0 ? (
              <div className="dp-envs">
                <div className="dp-env-panel" style={{ textAlign: "center", padding: "3rem" }}>
                  <h4>No Pipeline History Yet</h4>
                  <p style={{ color: "#94a3b8", marginTop: "0.5rem" }}>
                    {selectedProject
                      ? "Run a build or pipeline to see deployment history here."
                      : "No deployment pipelines configured. Create a pipeline to deploy your agent-built projects."}
                  </p>
                </div>
              </div>
            ) : (
              <div className="dp-deploy-list">
                {buildHistory.map((b, i) => (
                  <div key={i} className="dp-deploy-item">
                    <div className="dp-deploy-status">
                      <span className="dp-status-dot" style={{ background: b.success ? "#22c55e" : "#ef4444" }} />
                      <span className="dp-status-text" style={{ color: b.success ? "#22c55e" : "#ef4444" }}>
                        {b.success ? "passed" : "failed"}
                      </span>
                    </div>
                    <div className="dp-deploy-info">
                      <div className="dp-deploy-project">{b.duration_ms}ms</div>
                      <div className="dp-deploy-meta">
                        <span>{new Date(b.timestamp * 1000).toLocaleString()}</span>
                        {b.errors.length > 0 && <span style={{ color: "#ef4444" }}>{b.errors.length} errors</span>}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* ═══ LOGS VIEW ═══ */}
        {view === "logs" && (
          <div className="dp-logs-view">
            <div className="dp-deploys-header">
              <h3 className="dp-view-title">Pipeline Logs</h3>
              <button className="dp-new-btn" onClick={() => setLogs([])}>Clear</button>
            </div>
            <div className="dp-logs-list">
              {logs.length === 0 ? (
                <div style={{ padding: "2rem", textAlign: "center", color: "#64748b" }}>
                  {hasPipelines
                    ? "No logs yet. Run a build or pipeline to see deployment output."
                    : "No deployment pipelines configured. Create a pipeline to deploy your agent-built projects."}
                </div>
              ) : (
                logs.map((log, i) => (
                  <div key={i} className={`dp-log-line dp-log-${log.level === "success" ? "info" : log.level}`}>
                    <span className="dp-log-time">{new Date(log.timestamp).toLocaleTimeString()}</span>
                    {log.stage && <span className="dp-log-proj">[{log.stage}]</span>}
                    <span className="dp-log-level" style={{
                      color: log.level === "success" ? "#22c55e" : log.level === "error" ? "#ef4444" : log.level === "warn" ? "#f59e0b" : "#94a3b8",
                    }}>
                      {log.level === "success" ? "OK" : log.level.toUpperCase()}
                    </span>
                    <span className="dp-log-msg">{log.message}</span>
                  </div>
                ))
              )}
            </div>
          </div>
        )}
      </div>

      {/* ─── Status Bar ─── */}
      <div className="dp-status-bar">
        {hasPipelines ? (
          <>
            <span className="dp-status-item">{projects.length} projects</span>
            <span className="dp-status-item">{projects.filter(p => p.status === "Running").length} running</span>
            <span className="dp-status-item">{buildHistory.length} builds</span>
            <span className="dp-status-item">{logs.length} log entries</span>
          </>
        ) : (
          <span className="dp-status-item">No deployment pipelines configured yet</span>
        )}
        <span className="dp-status-item dp-status-right">Factory Pipeline (Real Backend)</span>
      </div>
    </div>
  );
}
