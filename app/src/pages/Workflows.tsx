import { useState } from "react";
import "./workflows.css";

interface WorkflowDef {
  id: string;
  name: string;
  description: string;
  nodeCount: number;
  lastRun: { status: "success" | "failed" | "never"; when: string; detail?: string };
  nodes: { name: string; status: "success" | "failed" | "pending" | "idle" }[];
}

interface RunHistoryEntry {
  workflow: string;
  startedAt: string;
  duration: string;
  status: "success" | "failed";
  nodesCompleted: string;
}

const WORKFLOWS: WorkflowDef[] = [
  {
    id: "wf-social",
    name: "Daily Social Post Pipeline",
    description: "Research trending topics -> Generate content -> Human approval -> Post to X, Instagram, Facebook",
    nodeCount: 5,
    lastRun: { status: "success", when: "2 hours ago" },
    nodes: [
      { name: "Research Topics", status: "success" },
      { name: "Generate Content", status: "success" },
      { name: "Quality Gate", status: "success" },
      { name: "Human Approval", status: "success" },
      { name: "Publish Posts", status: "success" }
    ]
  },
  {
    id: "wf-review",
    name: "Code Review Pipeline",
    description: "Watch repo -> Scan changes -> Analyze architecture -> Write review -> Submit PR comments",
    nodeCount: 8,
    lastRun: { status: "success", when: "30 min ago" },
    nodes: [
      { name: "Watch Repo", status: "success" },
      { name: "Fetch Diff", status: "success" },
      { name: "Scan Changes", status: "success" },
      { name: "Architecture Check", status: "success" },
      { name: "Security Scan", status: "success" },
      { name: "Style Lint", status: "success" },
      { name: "Write Review", status: "success" },
      { name: "Submit Comments", status: "success" }
    ]
  },
  {
    id: "wf-research",
    name: "Content Research & Publish",
    description: "Brave search -> Extract insights -> Draft article -> Compliance check -> Publish to CMS",
    nodeCount: 6,
    lastRun: { status: "failed", when: "1 day ago", detail: "node 4: compliance_check timeout" },
    nodes: [
      { name: "Brave Search", status: "success" },
      { name: "Extract Facts", status: "success" },
      { name: "Draft Article", status: "success" },
      { name: "Compliance Check", status: "failed" },
      { name: "Review Gate", status: "idle" },
      { name: "Publish CMS", status: "idle" }
    ]
  },
  {
    id: "wf-analytics",
    name: "Weekly Analytics Report",
    description: "Collect metrics -> Evaluate performance -> Generate charts -> Email stakeholders",
    nodeCount: 4,
    lastRun: { status: "never", when: "Never run" },
    nodes: [
      { name: "Collect Metrics", status: "idle" },
      { name: "Evaluate Perf", status: "idle" },
      { name: "Generate Charts", status: "idle" },
      { name: "Email Report", status: "idle" }
    ]
  }
];

const RUN_HISTORY: RunHistoryEntry[] = [
  { workflow: "Daily Social Post Pipeline", startedAt: "2026-03-05 07:00", duration: "2m 14s", status: "success", nodesCompleted: "5/5" },
  { workflow: "Code Review Pipeline", startedAt: "2026-03-05 09:30", duration: "1m 48s", status: "success", nodesCompleted: "8/8" },
  { workflow: "Daily Social Post Pipeline", startedAt: "2026-03-04 07:00", duration: "2m 02s", status: "success", nodesCompleted: "5/5" },
  { workflow: "Content Research & Publish", startedAt: "2026-03-04 14:00", duration: "4m 51s", status: "failed", nodesCompleted: "3/6" },
  { workflow: "Code Review Pipeline", startedAt: "2026-03-04 11:15", duration: "1m 33s", status: "success", nodesCompleted: "8/8" },
  { workflow: "Daily Social Post Pipeline", startedAt: "2026-03-03 07:00", duration: "2m 22s", status: "success", nodesCompleted: "5/5" },
  { workflow: "Code Review Pipeline", startedAt: "2026-03-03 16:45", duration: "1m 55s", status: "success", nodesCompleted: "8/8" },
  { workflow: "Content Research & Publish", startedAt: "2026-03-02 10:00", duration: "3m 12s", status: "success", nodesCompleted: "6/6" },
  { workflow: "Daily Social Post Pipeline", startedAt: "2026-03-02 07:00", duration: "2m 08s", status: "success", nodesCompleted: "5/5" },
  { workflow: "Code Review Pipeline", startedAt: "2026-03-01 13:20", duration: "2m 01s", status: "success", nodesCompleted: "8/8" }
];

function nodeStatusColor(status: string): string {
  if (status === "success") return "var(--green)";
  if (status === "failed") return "var(--red)";
  if (status === "pending") return "var(--amber)";
  return "var(--text-muted)";
}

export function Workflows(): JSX.Element {
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [workflows, setWorkflows] = useState<WorkflowDef[]>(WORKFLOWS);
  const [runHistory, setRunHistory] = useState<RunHistoryEntry[]>(RUN_HISTORY);
  const [editingId, setEditingId] = useState<string | null>(null);

  function handleRunNow(wfId: string): void {
    const wf = workflows.find((w) => w.id === wfId);
    if (!wf) return;
    setWorkflows((prev) =>
      prev.map((w) =>
        w.id === wfId
          ? { ...w, lastRun: { status: "success", when: "just now" }, nodes: w.nodes.map((n) => ({ ...n, status: "success" as const })) }
          : w
      )
    );
    const now = new Date();
    const ts = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")} ${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
    setRunHistory((prev) => [
      { workflow: wf.name, startedAt: ts, duration: `${Math.floor(Math.random() * 3) + 1}m ${Math.floor(Math.random() * 59)}s`, status: "success", nodesCompleted: `${wf.nodeCount}/${wf.nodeCount}` },
      ...prev
    ]);
  }

  function handleCreateWorkflow(): void {
    const id = `wf-new-${Date.now()}`;
    const newWf: WorkflowDef = {
      id,
      name: "New Workflow",
      description: "Untitled workflow -- click Edit to configure",
      nodeCount: 2,
      lastRun: { status: "never", when: "Never run" },
      nodes: [
        { name: "Start", status: "idle" },
        { name: "End", status: "idle" }
      ]
    };
    setWorkflows((prev) => [...prev, newWf]);
    setEditingId(id);
  }

  return (
    <section className="wf-engine">
      <header className="wf-header">
        <div>
          <h2 className="wf-title">WORKFLOW ENGINE // {workflows.length} WORKFLOWS</h2>
          <p className="wf-subtitle">Visual DAG automation pipelines with execution history</p>
        </div>
        <button type="button" className="wf-create-btn" onClick={handleCreateWorkflow}>+ CREATE WORKFLOW</button>
      </header>

      <div className="wf-grid">
        {workflows.map((wf) => {
          const expanded = expandedId === wf.id;
          const statusClass = wf.lastRun.status === "success" ? "success" : wf.lastRun.status === "failed" ? "failed" : "never";
          return (
            <article key={wf.id} className={`wf-card ${statusClass}`}>
              <div className={`wf-card-status-bar ${statusClass}`} />
              <div className="wf-card-body">
                <h3 className="wf-card-name">{wf.name}</h3>
                <p className="wf-card-desc">{wf.description}</p>
                <div className="wf-card-meta">
                  <span className="wf-card-nodes">{wf.nodeCount} nodes</span>
                  <span className="wf-card-separator">|</span>
                  <span className={`wf-card-run-status ${statusClass}`}>
                    {wf.lastRun.status === "success" && "Last run: Success \u2713"}
                    {wf.lastRun.status === "failed" && "Last run: Failed \u2717"}
                    {wf.lastRun.status === "never" && "Never run"}
                  </span>
                  <span className="wf-card-separator">|</span>
                  <span className="wf-card-when">{wf.lastRun.when}</span>
                </div>
                {wf.lastRun.detail && (
                  <p className="wf-card-detail">{wf.lastRun.detail}</p>
                )}
                <div className="wf-card-actions">
                  <button type="button" className="wf-action-btn wf-action-run" onClick={() => handleRunNow(wf.id)}>Run Now</button>
                  <button type="button" className="wf-action-btn wf-action-edit" onClick={() => setEditingId(editingId === wf.id ? null : wf.id)}>
                    {editingId === wf.id ? "Done" : "Edit"}
                  </button>
                  <button
                    type="button"
                    className="wf-action-btn wf-action-logs"
                    onClick={() => setExpandedId(expanded ? null : wf.id)}
                  >
                    {expanded ? "Hide Flow" : "Show Flow"}
                  </button>
                </div>
              </div>
              {expanded && (
                <div className="wf-node-flow">
                  {wf.nodes.map((node, idx) => (
                    <div key={node.name} className="wf-node-item">
                      <div
                        className="wf-node-box"
                        style={{ borderColor: nodeStatusColor(node.status) }}
                      >
                        <span className="wf-node-dot" style={{ background: nodeStatusColor(node.status) }} />
                        <span className="wf-node-label">{node.name}</span>
                      </div>
                      {idx < wf.nodes.length - 1 && (
                        <span className="wf-node-arrow">&rarr;</span>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </article>
          );
        })}
      </div>

      <section className="wf-history">
        <h3 className="wf-history-title">EXECUTION HISTORY</h3>
        <div className="wf-history-table-wrap">
          <table className="wf-history-table">
            <thead>
              <tr>
                <th>Workflow</th>
                <th>Started</th>
                <th>Duration</th>
                <th>Status</th>
                <th>Nodes</th>
              </tr>
            </thead>
            <tbody>
              {runHistory.map((entry, idx) => (
                <tr key={idx} className={idx % 2 === 0 ? "even" : "odd"}>
                  <td>{entry.workflow}</td>
                  <td className="mono">{entry.startedAt}</td>
                  <td className="mono">{entry.duration}</td>
                  <td>
                    <span className={`wf-status-pill ${entry.status}`}>
                      {entry.status === "success" ? "Success \u2713" : "Failed \u2717"}
                    </span>
                  </td>
                  <td className="mono">{entry.nodesCompleted}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </section>
  );
}
