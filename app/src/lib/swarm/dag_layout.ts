/**
 * Topological-tier layout for a `DagSnapshot`. Assigns each node an `{x, y}`
 * coordinate suitable for an @xyflow/react canvas. Root nodes (no incoming
 * edges) go in the first column; each subsequent tier contains nodes whose
 * deepest predecessor sits one tier earlier.
 *
 * Keeps the layout deterministic and dependency-free — dagre / elkjs would
 * give a nicer look but add weight we don't need for Phase 3a.
 */

import type { DagEdge, DagNode } from "./types";

const TIER_X = 220;
const ROW_Y = 100;
const PADDING_X = 40;
const PADDING_Y = 40;

export interface LaidOutNode {
  id: string;
  node: DagNode;
  x: number;
  y: number;
}

export function layoutDag(
  nodes: readonly DagNode[],
  edges: readonly DagEdge[],
): LaidOutNode[] {
  if (nodes.length === 0) return [];

  const incoming = new Map<string, string[]>();
  const outgoing = new Map<string, string[]>();
  for (const n of nodes) {
    incoming.set(n.id, []);
    outgoing.set(n.id, []);
  }
  for (const e of edges) {
    incoming.get(e.to)?.push(e.from);
    outgoing.get(e.from)?.push(e.to);
  }

  const tier = new Map<string, number>();
  const queue: string[] = [];
  for (const n of nodes) {
    if ((incoming.get(n.id)?.length ?? 0) === 0) {
      tier.set(n.id, 0);
      queue.push(n.id);
    }
  }

  // Orphan safety: any node that never got visited because of a cycle or a
  // disconnected component gets tier 0 so we still render it.
  while (queue.length > 0) {
    const id = queue.shift() as string;
    const t = tier.get(id) ?? 0;
    for (const child of outgoing.get(id) ?? []) {
      const existing = tier.get(child);
      if (existing === undefined || existing < t + 1) {
        tier.set(child, t + 1);
        queue.push(child);
      }
    }
  }
  for (const n of nodes) {
    if (!tier.has(n.id)) tier.set(n.id, 0);
  }

  const tierBuckets = new Map<number, DagNode[]>();
  for (const n of nodes) {
    const t = tier.get(n.id) ?? 0;
    const bucket = tierBuckets.get(t);
    if (bucket) bucket.push(n);
    else tierBuckets.set(t, [n]);
  }

  const out: LaidOutNode[] = [];
  for (const [t, bucket] of tierBuckets) {
    bucket.forEach((node, i) => {
      out.push({
        id: node.id,
        node,
        x: PADDING_X + t * TIER_X,
        y: PADDING_Y + i * ROW_Y,
      });
    });
  }
  return out;
}
