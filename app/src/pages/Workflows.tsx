import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ReactFlow,
  Background,
  Controls,
  Panel,
  useNodesState,
  useEdgesState,
  addEdge,
  Handle,
  Position,
  type Node,
  type Edge,
  type Connection,
  type NodeTypes,
  type OnConnect,
  MarkerType,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import {
  getAgentTaskHistory,
  getScheduledAgents,
  hasDesktopRuntime,
  listAgents,
  startHivemind,
} from "../api/backend";
import type { AgentSummary, ScheduledAgent } from "../types";
import "./workflows.css";

/* ================================================================== */
/*  Node type definitions                                              */
/* ================================================================== */

type WorkflowNodeType = "trigger" | "agent" | "llm-query" | "router" | "output" | "approval";

interface WorkflowNodeData extends Record<string, unknown> {
  label: string;
  nodeType: WorkflowNodeType;
  config?: Record<string, string>;
}

const NODE_PALETTE: { type: WorkflowNodeType; label: string; color: string }[] = [
  { type: "trigger", label: "Trigger", color: "#22d3ee" },
  { type: "agent", label: "Agent", color: "#a78bfa" },
  { type: "llm-query", label: "LLM Query", color: "#34d399" },
  { type: "router", label: "Router", color: "#fbbf24" },
  { type: "approval", label: "Human Approval", color: "#f87171" },
  { type: "output", label: "Output", color: "#60a5fa" },
];

const nodeStyle = (color: string): React.CSSProperties => ({
  background: `${color}22`,
  border: `2px solid ${color}`,
  borderRadius: 12,
  padding: "12px 20px",
  color: "#e2e8f0",
  fontFamily: "var(--font-mono, monospace)",
  fontSize: "0.85rem",
  minWidth: 140,
  textAlign: "center",
});

/* ── Custom node components ── */

function TriggerNode({ data }: { data: WorkflowNodeData }) {
  return (
    <div style={nodeStyle("#22d3ee")}>
      <div style={{ fontSize: "0.7rem", opacity: 0.6, marginBottom: 4 }}>TRIGGER</div>
      <div>{data.label}</div>
      <div style={{ fontSize: "0.7rem", opacity: 0.5 }}>{data.config?.mode ?? "Manual"}</div>
      <Handle type="source" position={Position.Bottom} style={{ background: "#22d3ee" }} />
    </div>
  );
}

function AgentNode({ data }: { data: WorkflowNodeData }) {
  return (
    <div style={nodeStyle("#a78bfa")}>
      <Handle type="target" position={Position.Top} style={{ background: "#a78bfa" }} />
      <div style={{ fontSize: "0.7rem", opacity: 0.6, marginBottom: 4 }}>AGENT</div>
      <div>{data.label}</div>
      <Handle type="source" position={Position.Bottom} style={{ background: "#a78bfa" }} />
    </div>
  );
}

function LlmQueryNode({ data }: { data: WorkflowNodeData }) {
  return (
    <div style={nodeStyle("#34d399")}>
      <Handle type="target" position={Position.Top} style={{ background: "#34d399" }} />
      <div style={{ fontSize: "0.7rem", opacity: 0.6, marginBottom: 4 }}>LLM QUERY</div>
      <div>{data.label}</div>
      <Handle type="source" position={Position.Bottom} style={{ background: "#34d399" }} />
    </div>
  );
}

function RouterNode({ data }: { data: WorkflowNodeData }) {
  return (
    <div style={{ ...nodeStyle("#fbbf24"), borderRadius: "50%", width: 80, height: 80, display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", padding: 8 }}>
      <Handle type="target" position={Position.Top} style={{ background: "#fbbf24" }} />
      <div style={{ fontSize: "0.7rem" }}>ROUTE</div>
      <div style={{ fontSize: "0.75rem" }}>{data.label}</div>
      <Handle type="source" position={Position.Bottom} style={{ background: "#fbbf24" }} id="a" />
      <Handle type="source" position={Position.Right} style={{ background: "#fbbf24" }} id="b" />
    </div>
  );
}

function ApprovalNode({ data }: { data: WorkflowNodeData }) {
  return (
    <div style={nodeStyle("#f87171")}>
      <Handle type="target" position={Position.Top} style={{ background: "#f87171" }} />
      <div style={{ fontSize: "0.7rem", opacity: 0.6, marginBottom: 4 }}>APPROVAL</div>
      <div>{data.label}</div>
      <Handle type="source" position={Position.Bottom} style={{ background: "#f87171" }} />
    </div>
  );
}

function OutputNode({ data }: { data: WorkflowNodeData }) {
  return (
    <div style={nodeStyle("#60a5fa")}>
      <Handle type="target" position={Position.Top} style={{ background: "#60a5fa" }} />
      <div style={{ fontSize: "0.7rem", opacity: 0.6, marginBottom: 4 }}>OUTPUT</div>
      <div>{data.label}</div>
    </div>
  );
}

const nodeTypes: NodeTypes = {
  trigger: TriggerNode,
  agent: AgentNode,
  "llm-query": LlmQueryNode,
  router: RouterNode,
  approval: ApprovalNode,
  output: OutputNode,
};

/* ================================================================== */
/*  Templates                                                          */
/* ================================================================== */

interface WorkflowTemplate {
  name: string;
  description: string;
  nodes: Node<WorkflowNodeData>[];
  edges: Edge[];
}

const TEMPLATES: WorkflowTemplate[] = [
  {
    name: "Simple Chain",
    description: "Trigger -> Agent -> Output",
    nodes: [
      { id: "t1", type: "trigger", position: { x: 250, y: 50 }, data: { label: "Manual Start", nodeType: "trigger", config: { mode: "Manual" } } },
      { id: "a1", type: "agent", position: { x: 250, y: 180 }, data: { label: "Worker Agent", nodeType: "agent" } },
      { id: "o1", type: "output", position: { x: 250, y: 310 }, data: { label: "Result", nodeType: "output" } },
    ],
    edges: [
      { id: "e1", source: "t1", target: "a1", markerEnd: { type: MarkerType.ArrowClosed, color: "#22d3ee" }, style: { stroke: "#22d3ee55" } },
      { id: "e2", source: "a1", target: "o1", markerEnd: { type: MarkerType.ArrowClosed, color: "#a78bfa" }, style: { stroke: "#a78bfa55" } },
    ],
  },
  {
    name: "Research Pipeline",
    description: "Trigger -> Research Agent -> Writer Agent -> Output",
    nodes: [
      { id: "t1", type: "trigger", position: { x: 250, y: 30 }, data: { label: "Cron Trigger", nodeType: "trigger", config: { mode: "Cron" } } },
      { id: "a1", type: "agent", position: { x: 250, y: 150 }, data: { label: "Research Agent", nodeType: "agent" } },
      { id: "a2", type: "agent", position: { x: 250, y: 280 }, data: { label: "Writer Agent", nodeType: "agent" } },
      { id: "o1", type: "output", position: { x: 250, y: 410 }, data: { label: "Report", nodeType: "output" } },
    ],
    edges: [
      { id: "e1", source: "t1", target: "a1", markerEnd: { type: MarkerType.ArrowClosed, color: "#22d3ee" }, style: { stroke: "#22d3ee55" } },
      { id: "e2", source: "a1", target: "a2", markerEnd: { type: MarkerType.ArrowClosed, color: "#a78bfa" }, style: { stroke: "#a78bfa55" } },
      { id: "e3", source: "a2", target: "o1", markerEnd: { type: MarkerType.ArrowClosed, color: "#a78bfa" }, style: { stroke: "#a78bfa55" } },
    ],
  },
  {
    name: "Code Review",
    description: "Webhook -> Coder -> Reviewer -> Human Approval -> Output",
    nodes: [
      { id: "t1", type: "trigger", position: { x: 250, y: 30 }, data: { label: "Webhook", nodeType: "trigger", config: { mode: "Webhook" } } },
      { id: "a1", type: "agent", position: { x: 250, y: 150 }, data: { label: "Coder Agent", nodeType: "agent" } },
      { id: "a2", type: "agent", position: { x: 250, y: 280 }, data: { label: "Reviewer Agent", nodeType: "agent" } },
      { id: "h1", type: "approval", position: { x: 250, y: 410 }, data: { label: "Human Review", nodeType: "approval" } },
      { id: "o1", type: "output", position: { x: 250, y: 540 }, data: { label: "Notify Slack", nodeType: "output" } },
    ],
    edges: [
      { id: "e1", source: "t1", target: "a1", markerEnd: { type: MarkerType.ArrowClosed, color: "#22d3ee" }, style: { stroke: "#22d3ee55" } },
      { id: "e2", source: "a1", target: "a2", markerEnd: { type: MarkerType.ArrowClosed, color: "#a78bfa" }, style: { stroke: "#a78bfa55" } },
      { id: "e3", source: "a2", target: "h1", markerEnd: { type: MarkerType.ArrowClosed, color: "#a78bfa" }, style: { stroke: "#a78bfa55" } },
      { id: "e4", source: "h1", target: "o1", markerEnd: { type: MarkerType.ArrowClosed, color: "#f87171" }, style: { stroke: "#f8717155" } },
    ],
  },
];

/* ================================================================== */
/*  Helpers                                                            */
/* ================================================================== */

function formatNextRun(epoch: number): string {
  if (!epoch) return "Not scheduled";
  return new Date(epoch * 1000).toLocaleString();
}

let idCounter = 100;
function nextId() { return `n${++idCounter}`; }

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export function Workflows(): JSX.Element {
  const [nodes, setNodes, onNodesChange] = useNodesState<Node<WorkflowNodeData>>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [selectedNode, setSelectedNode] = useState<Node<WorkflowNodeData> | null>(null);
  const [tab, setTab] = useState<"builder" | "scheduled" | "history">("builder");
  const [savedWorkflows, setSavedWorkflows] = useState<Record<string, { nodes: Node<WorkflowNodeData>[]; edges: Edge[] }>>(() => {
    try {
      const saved = localStorage.getItem("nexus-workflows");
      return saved ? JSON.parse(saved) : {};
    } catch { return {}; }
  });
  const [workflowName, setWorkflowName] = useState("Untitled Workflow");

  // Legacy data
  const [scheduledAgents, setScheduledAgents] = useState<ScheduledAgent[]>([]);
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [history, setHistory] = useState<Record<string, unknown>[]>([]);
  const [goal, setGoal] = useState("Coordinate a short status sweep across active agents.");
  const [selectedAgentIds, setSelectedAgentIds] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);
  const isDesktop = hasDesktopRuntime();
  const reactFlowWrapper = useRef<HTMLDivElement>(null);

  const onConnect: OnConnect = useCallback(
    (params: Connection) => setEdges((eds) => addEdge({
      ...params,
      markerEnd: { type: MarkerType.ArrowClosed, color: "#22d3ee" },
      style: { stroke: "#22d3ee55" },
    }, eds)),
    [setEdges],
  );

  const loadData = useCallback(async () => {
    if (!isDesktop) { setLoading(false); return; }
    setLoading(true);
    try {
      const [scheduled, registered] = await Promise.all([getScheduledAgents(), listAgents()]);
      setScheduledAgents(scheduled);
      setAgents(registered);
      setSelectedAgentIds((c) => c.length > 0 ? c : registered.slice(0, 3).map((a) => a.id));
      const histories = await Promise.all(registered.slice(0, 8).map((a) => getAgentTaskHistory(a.id, 8)));
      setHistory(histories.flat().sort((l, r) => String(r.created_at ?? "").localeCompare(String(l.created_at ?? ""))).slice(0, 20));
    } catch (e) {
      setMessage(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [isDesktop]);

  useEffect(() => { void loadData(); }, [loadData]);

  const agentNameById = useMemo(() => new Map(agents.map((a) => [a.id, a.name])), [agents]);

  /* ── Drag & Drop from palette ── */
  const onDragOver = useCallback((e: React.DragEvent) => { e.preventDefault(); e.dataTransfer.dropEffect = "move"; }, []);

  const onDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    const nodeType = e.dataTransfer.getData("application/reactflow") as WorkflowNodeType;
    if (!nodeType) return;

    const palette = NODE_PALETTE.find(p => p.type === nodeType);
    const newNode: Node<WorkflowNodeData> = {
      id: nextId(),
      type: nodeType,
      position: { x: e.clientX - (reactFlowWrapper.current?.getBoundingClientRect().left ?? 0) - 70, y: e.clientY - (reactFlowWrapper.current?.getBoundingClientRect().top ?? 0) - 20 },
      data: { label: palette?.label ?? nodeType, nodeType },
    };
    setNodes((nds) => [...nds, newNode]);
  }, [setNodes]);

  const onNodeClick = useCallback((_: React.MouseEvent, node: Node) => {
    setSelectedNode(node as unknown as Node<WorkflowNodeData>);
  }, []);

  /* ── Save / Load ── */
  const saveWorkflow = useCallback(() => {
    const updated = { ...savedWorkflows, [workflowName]: { nodes, edges } };
    setSavedWorkflows(updated);
    localStorage.setItem("nexus-workflows", JSON.stringify(updated));
    setMessage(`Saved "${workflowName}"`);
    setTimeout(() => setMessage(null), 2000);
  }, [savedWorkflows, workflowName, nodes, edges]);

  const loadWorkflow = useCallback((name: string) => {
    const wf = savedWorkflows[name];
    if (wf) {
      setNodes(wf.nodes);
      setEdges(wf.edges);
      setWorkflowName(name);
    }
  }, [savedWorkflows, setNodes, setEdges]);

  const loadTemplate = useCallback((tpl: WorkflowTemplate) => {
    setNodes(tpl.nodes);
    setEdges(tpl.edges);
    setWorkflowName(tpl.name);
  }, [setNodes, setEdges]);

  const executeWorkflow = useCallback(async () => {
    // Convert visual workflow to hivemind goal
    const agentNodes = nodes.filter(n => n.type === "agent");
    if (agentNodes.length === 0) {
      setMessage("Add at least one Agent node to execute.");
      return;
    }
    const workflowDesc = nodes.map(n => `${n.type}: ${n.data.label}`).join(" -> ");
    try {
      const agentIds = agents.slice(0, agentNodes.length).map(a => a.id);
      const session = await startHivemind(`Execute workflow: ${workflowDesc}`, agentIds);
      const sessionId = String(session.session_id ?? session.id ?? "unknown");
      setMessage(`Workflow executing — session ${sessionId}`);
    } catch (e) {
      setMessage(e instanceof Error ? e.message : String(e));
    }
  }, [nodes, agents]);

  const triggerHivemind = useCallback(async () => {
    if (selectedAgentIds.length === 0) { setMessage("Select at least one agent."); return; }
    try {
      const session = await startHivemind(goal, selectedAgentIds);
      setMessage(`Hivemind session: ${String(session.session_id ?? session.id ?? "unknown")}`);
      await loadData();
    } catch (e) { setMessage(e instanceof Error ? e.message : String(e)); }
  }, [goal, loadData, selectedAgentIds]);

  const panelBg: React.CSSProperties = {
    background: "rgba(2, 6, 23, 0.85)",
    border: "1px solid rgba(34, 211, 238, 0.18)",
    borderRadius: 12,
    padding: "0.75rem",
    backdropFilter: "blur(8px)",
  };

  return (
    <section className="wf-engine" style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      <header className="wf-header">
        <div>
          <h2 className="wf-title">WORKFLOW ENGINE // VISUAL BUILDER</h2>
          <p className="wf-subtitle">Drag nodes, connect edges, build governed agent pipelines</p>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          {(["builder", "scheduled", "history"] as const).map(t => (
            <button key={t} type="button" className={`wf-create-btn${tab === t ? "" : ""}`}
              style={{ opacity: tab === t ? 1 : 0.5 }}
              onClick={() => setTab(t)}>
              {t === "builder" ? "Builder" : t === "scheduled" ? "Scheduled" : "History"}
            </button>
          ))}
        </div>
      </header>

      {message && (
        <div className="wf-history" style={{ marginBottom: "0.5rem" }}>
          <div className="wf-history-table-wrap" style={{ padding: "0.75rem" }}>{message}</div>
        </div>
      )}

      {tab === "builder" && (
        <div style={{ flex: 1, display: "flex", gap: 12, minHeight: 500 }}>
          {/* ── Node Palette ── */}
          <aside style={{ ...panelBg, width: 180, display: "flex", flexDirection: "column", gap: 8 }}>
            <div style={{ fontSize: "0.75rem", opacity: 0.6, marginBottom: 4 }}>DRAG TO CANVAS</div>
            {NODE_PALETTE.map(p => (
              <div
                key={p.type}
                draggable
                onDragStart={(e) => { e.dataTransfer.setData("application/reactflow", p.type); e.dataTransfer.effectAllowed = "move"; }}
                style={{
                  padding: "8px 12px",
                  borderRadius: 8,
                  border: `1px solid ${p.color}66`,
                  background: `${p.color}11`,
                  color: p.color,
                  cursor: "grab",
                  fontSize: "0.8rem",
                  textAlign: "center",
                }}
              >
                {p.label}
              </div>
            ))}
            <hr style={{ border: "none", borderTop: "1px solid rgba(148,163,184,0.15)", margin: "8px 0" }} />
            <div style={{ fontSize: "0.75rem", opacity: 0.6, marginBottom: 4 }}>TEMPLATES</div>
            {TEMPLATES.map(tpl => (
              <button key={tpl.name} type="button" onClick={() => loadTemplate(tpl)}
                style={{ padding: "6px 10px", borderRadius: 8, border: "1px solid rgba(34,211,238,0.2)", background: "rgba(34,211,238,0.06)", color: "#e2e8f0", fontSize: "0.75rem", cursor: "pointer", textAlign: "left" }}>
                <div>{tpl.name}</div>
                <div style={{ fontSize: "0.65rem", opacity: 0.5 }}>{tpl.description}</div>
              </button>
            ))}
            <hr style={{ border: "none", borderTop: "1px solid rgba(148,163,184,0.15)", margin: "8px 0" }} />
            <div style={{ fontSize: "0.75rem", opacity: 0.6, marginBottom: 4 }}>SAVED</div>
            {Object.keys(savedWorkflows).map(name => (
              <button key={name} type="button" onClick={() => loadWorkflow(name)}
                style={{ padding: "4px 8px", borderRadius: 6, border: "1px solid rgba(148,163,184,0.15)", background: "transparent", color: "#94a3b8", fontSize: "0.7rem", cursor: "pointer" }}>
                {name}
              </button>
            ))}
          </aside>

          {/* ── Canvas ── */}
          <div ref={reactFlowWrapper} style={{ flex: 1, borderRadius: 12, overflow: "hidden", border: "1px solid rgba(34,211,238,0.12)" }}>
            <ReactFlow
              nodes={nodes}
              edges={edges}
              onNodesChange={onNodesChange}
              onEdgesChange={onEdgesChange}
              onConnect={onConnect}
              onDrop={onDrop}
              onDragOver={onDragOver}
              onNodeClick={onNodeClick}
              nodeTypes={nodeTypes}
              fitView
              style={{ background: "#020617" }}
            >
              <Background color="#22d3ee" gap={20} size={1} style={{ opacity: 0.06 }} />
              <Controls style={{ background: "#0f172a", borderColor: "#22d3ee33" }} />
              <Panel position="top-right" style={{ display: "flex", gap: 8 }}>
                <input value={workflowName} onChange={e => setWorkflowName(e.target.value)}
                  style={{ background: "rgba(2,6,23,0.8)", border: "1px solid rgba(34,211,238,0.3)", borderRadius: 8, padding: "6px 12px", color: "#e2e8f0", fontSize: "0.8rem" }} />
                <button type="button" onClick={saveWorkflow}
                  style={{ padding: "6px 16px", borderRadius: 8, border: "1px solid rgba(34,211,238,0.4)", background: "rgba(34,211,238,0.12)", color: "#22d3ee", fontSize: "0.8rem", cursor: "pointer" }}>
                  Save
                </button>
                <button type="button" onClick={() => void executeWorkflow()}
                  style={{ padding: "6px 16px", borderRadius: 8, border: "1px solid rgba(34,211,238,0.4)", background: "rgba(34,211,238,0.25)", color: "#fff", fontSize: "0.8rem", cursor: "pointer", fontWeight: 600 }}>
                  Execute
                </button>
              </Panel>
            </ReactFlow>
          </div>

          {/* ── Config Panel ── */}
          {selectedNode && (
            <aside style={{ ...panelBg, width: 220 }}>
              <div style={{ fontSize: "0.75rem", opacity: 0.6, marginBottom: 8 }}>NODE CONFIG</div>
              <div style={{ marginBottom: 8 }}>
                <label style={{ fontSize: "0.7rem", opacity: 0.6 }}>Label</label>
                <input value={selectedNode.data.label}
                  onChange={e => {
                    const val = e.target.value;
                    setNodes(nds => nds.map(n => n.id === selectedNode.id ? { ...n, data: { ...n.data, label: val } } : n));
                    setSelectedNode(prev => prev ? { ...prev, data: { ...prev.data, label: val } } : null);
                  }}
                  style={{ width: "100%", background: "rgba(2,6,23,0.7)", border: "1px solid rgba(34,211,238,0.2)", borderRadius: 6, padding: "6px 10px", color: "#e2e8f0", fontSize: "0.8rem" }} />
              </div>
              <div style={{ fontSize: "0.7rem", opacity: 0.5 }}>Type: {selectedNode.type}</div>
              <div style={{ fontSize: "0.7rem", opacity: 0.5 }}>ID: {selectedNode.id}</div>
              <button type="button" onClick={() => {
                setNodes(nds => nds.filter(n => n.id !== selectedNode.id));
                setEdges(eds => eds.filter(e => e.source !== selectedNode.id && e.target !== selectedNode.id));
                setSelectedNode(null);
              }}
                style={{ marginTop: 12, padding: "6px 12px", borderRadius: 6, border: "1px solid #f8717166", background: "#f8717111", color: "#f87171", fontSize: "0.75rem", cursor: "pointer", width: "100%" }}>
                Delete Node
              </button>
            </aside>
          )}
        </div>
      )}

      {tab === "scheduled" && (
        <>
          <section className="wf-history" style={{ marginBottom: "1rem" }}>
            <h3 className="wf-history-title">HIVEMIND</h3>
            <div className="wf-history-table-wrap" style={{ padding: "1rem", display: "grid", gap: "0.75rem" }}>
              <textarea value={goal} onChange={(e) => setGoal(e.target.value)} rows={3}
                style={{ width: "100%", background: "rgba(2,6,23,0.7)", color: "inherit", border: "1px solid rgba(34,211,238,0.18)", borderRadius: 12, padding: "0.85rem" }} />
              <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
                {agents.map((agent) => {
                  const selected = selectedAgentIds.includes(agent.id);
                  return (
                    <button key={agent.id} type="button"
                      onClick={() => setSelectedAgentIds((c) => selected ? c.filter((x) => x !== agent.id) : [...c, agent.id])}
                      style={{ borderRadius: 999, padding: "0.45rem 0.9rem",
                        border: selected ? "1px solid rgba(34,211,238,0.55)" : "1px solid rgba(148,163,184,0.2)",
                        background: selected ? "rgba(34,211,238,0.14)" : "rgba(15,23,42,0.8)", color: "inherit" }}>
                      {agent.name}
                    </button>
                  );
                })}
              </div>
              <button type="button" className="wf-create-btn" onClick={() => void triggerHivemind()}>Start Hivemind Task</button>
            </div>
          </section>
          <div className="wf-grid">
            {scheduledAgents.map((wf) => (
              <article key={wf.agent_id} className="wf-card success">
                <div className="wf-card-status-bar success" />
                <div className="wf-card-body">
                  <h3 className="wf-card-name">{agentNameById.get(wf.agent_id) ?? wf.agent_id}</h3>
                  <p className="wf-card-desc">{wf.default_goal || "No description"}</p>
                  <div className="wf-card-meta">
                    <span className="wf-card-nodes">1 scheduled task</span>
                    <span className="wf-card-separator">|</span>
                    <span className="wf-card-run-status success">Active</span>
                    <span className="wf-card-separator">|</span>
                    <span className="wf-card-when">{wf.cron_expression}</span>
                  </div>
                  <p className="wf-card-detail">Next run: {formatNextRun(wf.next_run_epoch)}</p>
                </div>
              </article>
            ))}
          </div>
        </>
      )}

      {tab === "history" && (
        <section className="wf-history">
          <h3 className="wf-history-title">EXECUTION HISTORY</h3>
          <div className="wf-history-table-wrap" style={{ padding: "1rem" }}>
            {loading ? "Loading..." : history.length === 0 ? "No workflow run history available." : (
              <div style={{ display: "grid", gap: "0.75rem" }}>
                {history.map((item, i) => (
                  <article key={String(item.id ?? i)} style={{ border: "1px solid rgba(34,211,238,0.14)", borderRadius: 12, padding: "0.9rem", background: "rgba(2,6,23,0.45)" }}>
                    <strong>{String(item.goal ?? item.goal_description ?? item.task_description ?? "Task")}</strong>
                    <p style={{ marginTop: 8, fontSize: "0.85rem", opacity: 0.78 }}>
                      Agent: {agentNameById.get(String(item.agent_id ?? "")) ?? String(item.agent_id ?? "unknown")}
                    </p>
                    <p style={{ marginTop: 4, fontSize: "0.85rem", opacity: 0.78 }}>Status: {String(item.status ?? "unknown")}</p>
                  </article>
                ))}
              </div>
            )}
          </div>
        </section>
      )}
    </section>
  );
}
