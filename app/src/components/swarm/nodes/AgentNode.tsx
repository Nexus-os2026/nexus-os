/**
 * Custom @xyflow/react node for a swarm DAG node. Reads the live status
 * out of the store (not from xyflow's node data) so status transitions
 * animate via React re-renders instead of re-laying out the graph.
 */

import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { DagNode, DagNodeStatus } from "../../../lib/swarm/types";
import { selectFocusedNode, useSwarmStore } from "../../../lib/swarm/store";
import { selectNodeStatus } from "../../../lib/swarm/selectors";

export interface AgentNodeData extends Record<string, unknown> {
  dag: DagNode;
}

/** Coarse category for layout + styling. Mirrors DagNodeStatus but collapses the value-carrying variants. */
export type AgentNodeStatusKind =
  | "Pending"
  | "Ready"
  | "Running"
  | "Done"
  | "Failed"
  | "Skipped";

export function toStatusKind(status: DagNodeStatus): AgentNodeStatusKind {
  if (status === "Pending" || status === "Ready" || status === "Running" || status === "Skipped") {
    return status;
  }
  if ("Done" in status) return "Done";
  return "Failed";
}

function statusBg(kind: AgentNodeStatusKind): string {
  switch (kind) {
    case "Pending":
      return "rgba(71, 85, 105, 0.85)";
    case "Ready":
      return "rgba(37, 99, 235, 0.8)";
    case "Running":
      return "rgba(37, 99, 235, 0.9)";
    case "Done":
      return "rgba(22, 163, 74, 0.85)";
    case "Failed":
      return "rgba(220, 38, 38, 0.9)";
    case "Skipped":
      return "rgba(71, 85, 105, 0.35)";
  }
}

function statusDot(kind: AgentNodeStatusKind): string {
  switch (kind) {
    case "Pending":
      return "#94a3b8";
    case "Ready":
      return "#60a5fa";
    case "Running":
      return "#38bdf8";
    case "Done":
      return "#4ade80";
    case "Failed":
      return "#f87171";
    case "Skipped":
      return "#475569";
  }
}

/**
 * Pulse keyframe for Running. Injected as a global stylesheet rule the
 * first time the component mounts (idempotent — `data-swarm-pulse` guard
 * prevents duplicate <style> tags across StrictMode double-mount and
 * multiple AgentNode instances).
 */
function ensurePulseKeyframe(): void {
  if (typeof document === "undefined") return;
  if (document.querySelector("style[data-swarm-pulse]")) return;
  const style = document.createElement("style");
  style.setAttribute("data-swarm-pulse", "");
  style.textContent = `
    @keyframes swarm-node-pulse {
      0%   { box-shadow: 0 0 0 0 rgba(56, 189, 248, 0.55); }
      70%  { box-shadow: 0 0 0 10px rgba(56, 189, 248, 0); }
      100% { box-shadow: 0 0 0 0 rgba(56, 189, 248, 0); }
    }
  `;
  document.head.appendChild(style);
}

export function AgentNode({ data, selected }: NodeProps): JSX.Element {
  // @xyflow/react's generic NodeProps types `data` as `unknown`; cast once
  // at the boundary so the rest of the component stays strictly typed.
  const { dag } = data as AgentNodeData;
  const status = useSwarmStore(selectNodeStatus(dag.id));
  const kind = toStatusKind(status);
  ensurePulseKeyframe();

  const initial = dag.capability_id.charAt(0).toUpperCase();
  const label = dag.capability_id;

  return (
    <div
      data-testid={`agent-node-${dag.id}`}
      data-status={kind}
      onClick={(): void => selectFocusedNode(dag.id)}
      style={{
        width: 180,
        minHeight: 60,
        padding: "8px 10px",
        borderRadius: 10,
        background: statusBg(kind),
        border: selected
          ? "1px solid rgba(59,130,246,0.9)"
          : "1px solid rgba(15,23,42,0.6)",
        color: "#e2e8f0",
        boxShadow: kind === "Running" ? "0 0 0 0 rgba(56,189,248,0.55)" : "0 2px 6px rgba(0,0,0,0.3)",
        animation: kind === "Running" ? "swarm-node-pulse 1.5s ease-out infinite" : undefined,
        opacity: kind === "Skipped" ? 0.4 : 1,
        fontFamily: "var(--font-display, system-ui)",
      }}
    >
      <Handle type="target" position={Position.Top} style={{ background: "#475569" }} />
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span
          aria-hidden
          style={{
            width: 22,
            height: 22,
            borderRadius: "50%",
            background: "rgba(15,23,42,0.55)",
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 11,
            fontWeight: 700,
            flexShrink: 0,
          }}
        >
          {initial}
        </span>
        <div style={{ display: "flex", flexDirection: "column", minWidth: 0, flex: 1 }}>
          <span
            style={{
              fontSize: 12,
              fontWeight: 600,
              whiteSpace: "nowrap",
              overflow: "hidden",
              textOverflow: "ellipsis",
            }}
            title={label}
          >
            {label}
          </span>
          <span style={{ fontSize: 10, color: "rgba(226,232,240,0.65)" }}>{dag.id}</span>
        </div>
        <span
          data-testid={`agent-node-dot-${dag.id}`}
          aria-label={`status ${kind}`}
          style={{
            width: 10,
            height: 10,
            borderRadius: "50%",
            background: statusDot(kind),
            boxShadow: `0 0 6px ${statusDot(kind)}`,
            flexShrink: 0,
          }}
        />
        {kind === "Failed" && (
          <span
            data-testid={`agent-node-fail-${dag.id}`}
            aria-label="failed"
            style={{ color: "#fef2f2", fontWeight: 700, fontSize: 12 }}
          >
            !
          </span>
        )}
      </div>
      <Handle type="source" position={Position.Bottom} style={{ background: "#475569" }} />
    </div>
  );
}
