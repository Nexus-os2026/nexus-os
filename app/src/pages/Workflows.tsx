import { useMemo, useRef, useState } from "react";
import "./workflows.css";

type PaletteKind =
  | "Schedule"
  | "Webhook"
  | "File Change"
  | "Email Received"
  | "Manual"
  | "LLM Query"
  | "Web Search"
  | "Post to Social"
  | "Send Email"
  | "Create File"
  | "Run Code"
  | "HTTP Request"
  | "Database Query"
  | "If/Else"
  | "Switch"
  | "Loop"
  | "Merge"
  | "Wait"
  | "Error Handler"
  | "Summarize"
  | "Classify"
  | "Extract Data"
  | "Generate Content"
  | "Analyze Image"
  | "Transcribe Audio";

interface PaletteGroup {
  name: string;
  items: PaletteKind[];
}

interface CanvasNode {
  id: string;
  kind: PaletteKind;
  label: string;
  x: number;
  y: number;
  configJson: string;
}

interface CanvasConnection {
  id: string;
  from: string;
  to: string;
}

type NodeRunState = "idle" | "running" | "success" | "error";

const PALETTE: PaletteGroup[] = [
  {
    name: "Trigger",
    items: ["Schedule", "Webhook", "File Change", "Email Received", "Manual"]
  },
  {
    name: "Action",
    items: [
      "LLM Query",
      "Web Search",
      "Post to Social",
      "Send Email",
      "Create File",
      "Run Code",
      "HTTP Request",
      "Database Query"
    ]
  },
  {
    name: "Logic",
    items: ["If/Else", "Switch", "Loop", "Merge", "Wait", "Error Handler"]
  },
  {
    name: "AI",
    items: ["Summarize", "Classify", "Extract Data", "Generate Content", "Analyze Image", "Transcribe Audio"]
  }
];

function makeNodeId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `node-${Date.now()}-${Math.floor(Math.random() * 10000)}`;
}

function makeConnectionId(from: string, to: string): string {
  return `${from}->${to}`;
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

export function Workflows(): JSX.Element {
  const canvasRef = useRef<HTMLDivElement | null>(null);
  const [nodes, setNodes] = useState<CanvasNode[]>([]);
  const [connections, setConnections] = useState<CanvasConnection[]>([]);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [pendingOutputNode, setPendingOutputNode] = useState<string | null>(null);
  const [runState, setRunState] = useState<Record<string, NodeRunState>>({});
  const [logs, setLogs] = useState<string[]>([]);
  const [isRunning, setIsRunning] = useState(false);

  const selectedNode = useMemo(
    () => nodes.find((node) => node.id === selectedNodeId) ?? null,
    [nodes, selectedNodeId]
  );

  const nodeById = useMemo(() => {
    const map = new Map<string, CanvasNode>();
    for (const node of nodes) {
      map.set(node.id, node);
    }
    return map;
  }, [nodes]);

  const connectionPaths = useMemo(() => {
    return connections
      .map((connection) => {
        const fromNode = nodeById.get(connection.from);
        const toNode = nodeById.get(connection.to);
        if (!fromNode || !toNode) {
          return null;
        }
        const fromX = fromNode.x + 180;
        const fromY = fromNode.y + 42;
        const toX = toNode.x;
        const toY = toNode.y + 42;
        const midX = (fromX + toX) / 2;
        return {
          id: connection.id,
          d: `M ${fromX} ${fromY} C ${midX} ${fromY}, ${midX} ${toY}, ${toX} ${toY}`
        };
      })
      .filter((value): value is { id: string; d: string } => value !== null);
  }, [connections, nodeById]);

  const miniMapNodes = useMemo(() => {
    const maxX = Math.max(1, ...nodes.map((node) => node.x + 180));
    const maxY = Math.max(1, ...nodes.map((node) => node.y + 84));
    return nodes.map((node) => ({
      ...node,
      miniX: (node.x / maxX) * 170,
      miniY: (node.y / maxY) * 98
    }));
  }, [nodes]);

  function appendLog(entry: string): void {
    setLogs((previous) => [`${new Date().toLocaleTimeString()} ${entry}`, ...previous].slice(0, 120));
  }

  function handlePaletteDragStart(event: React.DragEvent<HTMLButtonElement>, kind: PaletteKind): void {
    event.dataTransfer.setData("application/nexus-node-kind", kind);
    event.dataTransfer.effectAllowed = "copy";
  }

  function handleNodeDragStart(event: React.DragEvent<HTMLDivElement>, nodeId: string): void {
    event.dataTransfer.setData("application/nexus-node-move", nodeId);
    event.dataTransfer.effectAllowed = "move";
  }

  function handleCanvasDragOver(event: React.DragEvent<HTMLDivElement>): void {
    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
  }

  function handleCanvasDrop(event: React.DragEvent<HTMLDivElement>): void {
    event.preventDefault();
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }

    const rect = canvas.getBoundingClientRect();
    const x = clamp(event.clientX - rect.left - 90, 12, rect.width - 196);
    const y = clamp(event.clientY - rect.top - 22, 12, rect.height - 108);

    const moveNodeId = event.dataTransfer.getData("application/nexus-node-move");
    if (moveNodeId.length > 0) {
      setNodes((previous) =>
        previous.map((node) =>
          node.id === moveNodeId
            ? {
                ...node,
                x,
                y
              }
            : node
        )
      );
      return;
    }

    const kind = event.dataTransfer.getData("application/nexus-node-kind") as PaletteKind;
    if (!kind) {
      return;
    }

    const node: CanvasNode = {
      id: makeNodeId(),
      kind,
      label: kind,
      x,
      y,
      configJson: "{}"
    };
    setNodes((previous) => [...previous, node]);
    setRunState((previous) => ({ ...previous, [node.id]: "idle" }));
    appendLog(`Added node ${node.label}`);
  }

  function handleConnectOutput(nodeId: string): void {
    setPendingOutputNode(nodeId);
    appendLog(`Selected output from ${nodeId}`);
  }

  function handleConnectInput(nodeId: string): void {
    if (!pendingOutputNode || pendingOutputNode === nodeId) {
      return;
    }

    const connectionId = makeConnectionId(pendingOutputNode, nodeId);
    setConnections((previous) => {
      if (previous.some((connection) => connection.id === connectionId)) {
        return previous;
      }
      return [...previous, { id: connectionId, from: pendingOutputNode, to: nodeId }];
    });
    appendLog(`Connected ${pendingOutputNode} -> ${nodeId}`);
    setPendingOutputNode(null);
  }

  function updateSelectedNodeConfig(nextConfigJson: string): void {
    if (!selectedNodeId) {
      return;
    }
    setNodes((previous) =>
      previous.map((node) =>
        node.id === selectedNodeId
          ? {
              ...node,
              configJson: nextConfigJson
            }
          : node
      )
    );
  }

  function updateSelectedNodeLabel(nextLabel: string): void {
    if (!selectedNodeId) {
      return;
    }
    setNodes((previous) =>
      previous.map((node) =>
        node.id === selectedNodeId
          ? {
              ...node,
              label: nextLabel
            }
          : node
      )
    );
  }

  async function runWorkflow(): Promise<void> {
    if (nodes.length === 0 || isRunning) {
      return;
    }

    const levels = buildExecutionLevels(nodes, connections);
    if (!levels) {
      appendLog("Execution aborted: graph has unresolved cycle or orphaned dependencies.");
      return;
    }

    setIsRunning(true);
    setLogs([]);
    setRunState(Object.fromEntries(nodes.map((node) => [node.id, "idle" as NodeRunState])));
    appendLog("Workflow execution started.");

    try {
      for (const level of levels) {
        setRunState((previous) => {
          const next = { ...previous };
          for (const nodeId of level) {
            next[nodeId] = "running";
          }
          return next;
        });

        await Promise.all(
          level.map(async (nodeId) => {
            const node = nodeById.get(nodeId);
            if (!node) {
              return;
            }
            appendLog(`Running ${node.label} (${node.kind})`);
            await new Promise((resolve) => setTimeout(resolve, 220 + Math.floor(Math.random() * 380)));
            setRunState((previous) => ({ ...previous, [nodeId]: "success" }));
            appendLog(`Completed ${node.label}`);
          })
        );
      }

      appendLog("Workflow execution completed.");
    } finally {
      setIsRunning(false);
    }
  }

  return (
    <section className="workflow-studio">
      <header className="workflow-header">
        <div>
          <h2 className="workflow-title">WORKFLOW STUDIO // VISUAL DAG AUTOMATION</h2>
          <p className="workflow-subtitle">Drag nodes, connect outputs to inputs, run and inspect logs in real time.</p>
        </div>
        <div className="workflow-header-actions">
          <button type="button" className="workflow-btn" onClick={() => setPendingOutputNode(null)}>
            Clear Wire
          </button>
          <button type="button" className="workflow-btn workflow-btn-primary" onClick={() => void runWorkflow()} disabled={isRunning || nodes.length === 0}>
            {isRunning ? "Running..." : "Run Workflow"}
          </button>
        </div>
      </header>

      <div className="workflow-body">
        <aside className="workflow-palette" aria-label="node palette">
          {PALETTE.map((group) => (
            <section key={group.name} className="workflow-palette-group">
              <h3>{group.name}</h3>
              <div className="workflow-palette-items">
                {group.items.map((kind) => (
                  <button
                    key={kind}
                    type="button"
                    draggable
                    onDragStart={(event) => handlePaletteDragStart(event, kind)}
                    className="workflow-palette-item"
                  >
                    {kind}
                  </button>
                ))}
              </div>
            </section>
          ))}
        </aside>

        <div className="workflow-canvas-shell">
          <div
            ref={canvasRef}
            className="workflow-canvas"
            onDragOver={handleCanvasDragOver}
            onDrop={handleCanvasDrop}
            aria-label="workflow canvas"
          >
            <svg className="workflow-connections" width="100%" height="100%" aria-hidden="true">
              {connectionPaths.map((path) => (
                <path key={path.id} d={path.d} />
              ))}
            </svg>

            {nodes.map((node) => (
              <div
                key={node.id}
                className={`workflow-node ${selectedNodeId === node.id ? "selected" : ""} ${runState[node.id] ?? "idle"}`}
                style={{ left: node.x, top: node.y }}
                draggable
                onDragStart={(event) => handleNodeDragStart(event, node.id)}
                onClick={() => setSelectedNodeId(node.id)}
              >
                <div className="workflow-node-head">
                  <strong>{node.label}</strong>
                  <span>{node.kind}</span>
                </div>
                <div className="workflow-node-actions">
                  <button
                    type="button"
                    className={`workflow-port ${pendingOutputNode === node.id ? "pending" : ""}`}
                    onClick={(event) => {
                      event.stopPropagation();
                      handleConnectOutput(node.id);
                    }}
                  >
                    Out
                  </button>
                  <button
                    type="button"
                    className="workflow-port"
                    onClick={(event) => {
                      event.stopPropagation();
                      handleConnectInput(node.id);
                    }}
                  >
                    In
                  </button>
                </div>
              </div>
            ))}

            <aside className="workflow-minimap" aria-label="mini map">
              <h4>Mini-map</h4>
              <div className="workflow-minimap-canvas">
                <svg width="100%" height="100%" aria-hidden="true">
                  {connections.map((connection) => {
                    const fromNode = miniMapNodes.find((node) => node.id === connection.from);
                    const toNode = miniMapNodes.find((node) => node.id === connection.to);
                    if (!fromNode || !toNode) {
                      return null;
                    }
                    return (
                      <line
                        key={connection.id}
                        x1={fromNode.miniX + 12}
                        y1={fromNode.miniY + 8}
                        x2={toNode.miniX + 12}
                        y2={toNode.miniY + 8}
                      />
                    );
                  })}
                </svg>
                {miniMapNodes.map((node) => (
                  <span
                    key={node.id}
                    className="workflow-minimap-node"
                    style={{ left: node.miniX, top: node.miniY }}
                    title={node.label}
                  />
                ))}
              </div>
            </aside>
          </div>

          <section className="workflow-log-panel" aria-label="execution logs">
            <h3>Execution Log</h3>
            <div>
              {logs.length === 0 ? <p>No run logs yet.</p> : null}
              {logs.map((line, index) => (
                <p key={`${line}-${index}`}>{line}</p>
              ))}
            </div>
          </section>
        </div>

        <aside className="workflow-config" aria-label="node configuration">
          <h3>Node Settings</h3>
          {!selectedNode ? <p>Select a node to edit label and config.</p> : null}
          {selectedNode ? (
            <>
              <label>
                Label
                <input value={selectedNode.label} onChange={(event) => updateSelectedNodeLabel(event.target.value)} />
              </label>
              <label>
                Config JSON
                <textarea
                  value={selectedNode.configJson}
                  onChange={(event) => updateSelectedNodeConfig(event.target.value)}
                  rows={10}
                />
              </label>
            </>
          ) : null}
        </aside>
      </div>
    </section>
  );
}

function buildExecutionLevels(nodes: CanvasNode[], connections: CanvasConnection[]): string[][] | null {
  const indegree = new Map<string, number>();
  const outgoing = new Map<string, string[]>();

  for (const node of nodes) {
    indegree.set(node.id, 0);
    outgoing.set(node.id, []);
  }

  for (const connection of connections) {
    outgoing.set(connection.from, [...(outgoing.get(connection.from) ?? []), connection.to]);
    indegree.set(connection.to, (indegree.get(connection.to) ?? 0) + 1);
  }

  let ready = [...indegree.entries()]
    .filter(([, degree]) => degree === 0)
    .map(([nodeId]) => nodeId)
    .sort();

  const levels: string[][] = [];
  const visited = new Set<string>();

  while (ready.length > 0) {
    const level = [...ready];
    levels.push(level);
    ready = [];

    for (const nodeId of level) {
      visited.add(nodeId);
      for (const neighbor of outgoing.get(nodeId) ?? []) {
        const nextDegree = (indegree.get(neighbor) ?? 1) - 1;
        indegree.set(neighbor, nextDegree);
        if (nextDegree === 0) {
          ready.push(neighbor);
        }
      }
    }

    ready.sort();
  }

  return visited.size === nodes.length ? levels : null;
}
