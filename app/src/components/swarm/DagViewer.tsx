/**
 * DagViewer — the @xyflow/react canvas for the current swarm plan or run.
 *
 * Reads nodes + edges from the store via selectors and lays them out with
 * the BFS tier layout helper. The node component (`AgentNode`) owns its
 * own live-status subscription so color changes don't require the viewer
 * to rebuild the xyflow node array on every status tick.
 */

import { useMemo } from "react";
import {
  Background,
  Controls,
  MarkerType,
  MiniMap,
  ReactFlow,
  type Edge,
  type Node,
  type NodeTypes,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import { useSwarmStore } from "../../lib/swarm/store";
import { selectDagEdges, selectDagNodes } from "../../lib/swarm/selectors";
import { selectFocusedNode } from "../../lib/swarm/store";
import { layoutDag } from "../../lib/swarm/dag_layout";
import { AgentNode, type AgentNodeData } from "./nodes/AgentNode";

const NODE_TYPES: NodeTypes = { agent: AgentNode };

function EmptyState(): JSX.Element {
  return (
    <div
      data-testid="dag-empty-state"
      style={{
        position: "absolute",
        inset: 0,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        color: "#475569",
        fontSize: 13,
        fontFamily: "var(--font-mono, monospace)",
        pointerEvents: "none",
      }}
    >
      Submit an intent above to begin.
    </div>
  );
}

export function DagViewer(): JSX.Element {
  const dagNodes = useSwarmStore(selectDagNodes);
  const dagEdges = useSwarmStore(selectDagEdges);

  const flowNodes = useMemo<Node<AgentNodeData>[]>(() => {
    const laid = layoutDag(dagNodes, dagEdges);
    return laid.map((l) => ({
      id: l.id,
      type: "agent",
      position: { x: l.x, y: l.y },
      data: { dag: l.node },
    }));
  }, [dagNodes, dagEdges]);

  const flowEdges = useMemo<Edge[]>(() => {
    return dagEdges.map((e, i) => ({
      id: `e${i}-${e.from}-${e.to}`,
      source: e.from,
      target: e.to,
      markerEnd: { type: MarkerType.ArrowClosed, color: "#64748b" },
      style: { stroke: "#64748b", strokeWidth: 1.2 },
    }));
  }, [dagEdges]);

  const isEmpty = dagNodes.length === 0;

  return (
    <div
      data-testid="dag-viewer"
      style={{
        position: "relative",
        width: "100%",
        height: "100%",
        minHeight: 0,
        background: "rgba(10,14,26,0.6)",
        borderRadius: 10,
        border: "1px solid rgba(100,116,139,0.25)",
        overflow: "hidden",
      }}
    >
      {isEmpty ? (
        <EmptyState />
      ) : (
        <ReactFlow
          nodes={flowNodes}
          edges={flowEdges}
          nodeTypes={NODE_TYPES}
          fitView
          proOptions={{ hideAttribution: true }}
          onNodeClick={(_evt, node): void => selectFocusedNode(node.id)}
          onPaneClick={(): void => selectFocusedNode(null)}
        >
          <Background color="#334155" gap={20} size={1} style={{ opacity: 0.35 }} />
          <Controls
            style={{ background: "rgba(15,23,42,0.7)", borderColor: "rgba(100,116,139,0.3)" }}
          />
          <MiniMap
            pannable
            zoomable
            maskColor="rgba(10,14,26,0.7)"
            nodeColor={(): string => "#334155"}
            style={{ background: "rgba(10,14,26,0.8)" }}
          />
        </ReactFlow>
      )}
    </div>
  );
}
