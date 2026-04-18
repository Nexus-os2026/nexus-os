//! ExecutionDag — a directed acyclic graph of DAG nodes with status.
//!
//! Built on `petgraph::Graph` for cycle detection and topological queries.

use crate::error::SwarmError;
use crate::profile::TaskProfile;
use petgraph::algo::is_cyclic_directed;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DagNodeStatus {
    Pending,
    Ready,
    Running,
    Done(Value),
    Failed(String),
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagNode {
    pub id: String,
    pub capability_id: String,
    pub profile: TaskProfile,
    pub inputs: Value,
    pub status: DagNodeStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagEdge {
    pub from: String,
    pub to: String,
}

/// Storage-form of the DAG (serializable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagSnapshot {
    pub nodes: Vec<DagNode>,
    pub edges: Vec<DagEdge>,
}

/// In-memory DAG with petgraph backing for fast neighbor queries.
#[derive(Debug, Clone)]
pub struct ExecutionDag {
    graph: DiGraph<DagNode, ()>,
    index: BTreeMap<String, NodeIndex>,
}

impl Default for ExecutionDag {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionDag {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            index: BTreeMap::new(),
        }
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn add_node(&mut self, node: DagNode) -> Result<(), SwarmError> {
        if self.index.contains_key(&node.id) {
            return Err(SwarmError::DagCycle {
                from: node.id.clone(),
                to: node.id,
            });
        }
        let id = node.id.clone();
        let idx = self.graph.add_node(node);
        self.index.insert(id, idx);
        Ok(())
    }

    /// Add an edge, rejecting any insertion that would create a cycle.
    pub fn add_edge(&mut self, from: &str, to: &str) -> Result<(), SwarmError> {
        let fi = *self
            .index
            .get(from)
            .ok_or_else(|| SwarmError::RegistryMiss(format!("dag node `{from}` missing")))?;
        let ti = *self
            .index
            .get(to)
            .ok_or_else(|| SwarmError::RegistryMiss(format!("dag node `{to}` missing")))?;
        let edge_idx = self.graph.add_edge(fi, ti, ());
        if is_cyclic_directed(&self.graph) {
            self.graph.remove_edge(edge_idx);
            return Err(SwarmError::DagCycle {
                from: from.into(),
                to: to.into(),
            });
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&DagNode> {
        self.index.get(id).and_then(|i| self.graph.node_weight(*i))
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut DagNode> {
        let idx = self.index.get(id).copied()?;
        self.graph.node_weight_mut(idx)
    }

    /// All nodes whose status is `Ready` (roots with no prerequisites, or
    /// nodes whose every parent has status `Done`).
    pub fn ready_nodes(&self) -> Vec<String> {
        self.graph
            .node_indices()
            .filter_map(|idx| {
                let node = self.graph.node_weight(idx)?;
                if !matches!(node.status, DagNodeStatus::Pending | DagNodeStatus::Ready) {
                    return None;
                }
                let parents_done =
                    self.graph
                        .neighbors_directed(idx, Direction::Incoming)
                        .all(|p| {
                            self.graph
                                .node_weight(p)
                                .map(|n| matches!(n.status, DagNodeStatus::Done(_)))
                                .unwrap_or(false)
                        });
                if parents_done {
                    Some(node.id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn mark_running(&mut self, id: &str) {
        if let Some(n) = self.get_mut(id) {
            n.status = DagNodeStatus::Running;
        }
    }

    pub fn mark_done(&mut self, id: &str, value: Value) {
        if let Some(n) = self.get_mut(id) {
            n.status = DagNodeStatus::Done(value);
        }
    }

    /// Mark a node failed and cascade all descendants to `Skipped`.
    pub fn mark_failed_and_cascade(&mut self, id: &str, reason: String) {
        let Some(&root) = self.index.get(id) else {
            return;
        };
        if let Some(n) = self.graph.node_weight_mut(root) {
            n.status = DagNodeStatus::Failed(reason);
        }
        let descendants = self.descendants(root);
        for d in descendants {
            if let Some(n) = self.graph.node_weight_mut(d) {
                if !matches!(n.status, DagNodeStatus::Done(_) | DagNodeStatus::Failed(_)) {
                    n.status = DagNodeStatus::Skipped;
                }
            }
        }
    }

    /// BFS through outgoing edges collecting every descendant.
    fn descendants(&self, root: NodeIndex) -> Vec<NodeIndex> {
        let mut seen = std::collections::BTreeSet::new();
        let mut stack = vec![root];
        let mut out = Vec::new();
        while let Some(n) = stack.pop() {
            for child in self.graph.neighbors_directed(n, Direction::Outgoing) {
                if seen.insert(child) {
                    out.push(child);
                    stack.push(child);
                }
            }
        }
        out
    }

    pub fn parent_outputs(&self, id: &str) -> BTreeMap<String, Value> {
        let Some(&idx) = self.index.get(id) else {
            return BTreeMap::new();
        };
        self.graph
            .neighbors_directed(idx, Direction::Incoming)
            .filter_map(|p| {
                let n = self.graph.node_weight(p)?;
                if let DagNodeStatus::Done(v) = &n.status {
                    Some((n.id.clone(), v.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn is_complete(&self) -> bool {
        self.graph.node_indices().all(|i| {
            matches!(
                self.graph.node_weight(i).map(|n| &n.status),
                Some(DagNodeStatus::Done(_))
                    | Some(DagNodeStatus::Failed(_))
                    | Some(DagNodeStatus::Skipped)
            )
        })
    }

    pub fn to_snapshot(&self) -> DagSnapshot {
        let nodes: Vec<DagNode> = self
            .graph
            .node_indices()
            .filter_map(|i| self.graph.node_weight(i).cloned())
            .collect();
        let edges: Vec<DagEdge> = self
            .graph
            .edge_references()
            .filter_map(|e| {
                let f = self.graph.node_weight(e.source())?.id.clone();
                let t = self.graph.node_weight(e.target())?.id.clone();
                Some(DagEdge { from: f, to: t })
            })
            .collect();
        DagSnapshot { nodes, edges }
    }

    pub fn from_snapshot(snap: DagSnapshot) -> Result<Self, SwarmError> {
        let mut dag = Self::new();
        for n in snap.nodes {
            dag.add_node(n)?;
        }
        for e in snap.edges {
            dag.add_edge(&e.from, &e.to)?;
        }
        Ok(dag)
    }

    pub fn to_json(&self) -> Value {
        serde_json::to_value(self.to_snapshot()).unwrap_or(Value::Null)
    }

    pub fn from_json(v: Value) -> Result<Self, SwarmError> {
        let snap: DagSnapshot = serde_json::from_value(v)
            .map_err(|e| SwarmError::DirectorParse(format!("dag snapshot: {e}")))?;
        Self::from_snapshot(snap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mk_node(id: &str) -> DagNode {
        DagNode {
            id: id.into(),
            capability_id: "cap".into(),
            profile: TaskProfile::local_light(),
            inputs: json!({}),
            status: DagNodeStatus::Pending,
        }
    }

    #[test]
    fn ready_nodes_initially_all_roots() {
        let mut dag = ExecutionDag::new();
        dag.add_node(mk_node("a")).unwrap();
        dag.add_node(mk_node("b")).unwrap();
        dag.add_edge("a", "b").unwrap();
        let ready = dag.ready_nodes();
        assert_eq!(ready, vec!["a".to_string()]);
    }

    #[test]
    fn cycle_insertion_is_rejected() {
        let mut dag = ExecutionDag::new();
        dag.add_node(mk_node("a")).unwrap();
        dag.add_node(mk_node("b")).unwrap();
        dag.add_edge("a", "b").unwrap();
        let err = dag.add_edge("b", "a").unwrap_err();
        assert!(matches!(err, SwarmError::DagCycle { .. }));
    }

    #[test]
    fn mark_done_exposes_child_as_ready() {
        let mut dag = ExecutionDag::new();
        dag.add_node(mk_node("a")).unwrap();
        dag.add_node(mk_node("b")).unwrap();
        dag.add_edge("a", "b").unwrap();
        dag.mark_done("a", json!({"x": 1}));
        let ready = dag.ready_nodes();
        assert_eq!(ready, vec!["b".to_string()]);
    }

    #[test]
    fn failure_cascades_to_descendants() {
        let mut dag = ExecutionDag::new();
        for id in ["a", "b", "c", "d"] {
            dag.add_node(mk_node(id)).unwrap();
        }
        dag.add_edge("a", "b").unwrap();
        dag.add_edge("b", "c").unwrap();
        dag.add_edge("c", "d").unwrap();
        dag.mark_failed_and_cascade("b", "oops".into());
        assert!(matches!(
            dag.get("b").unwrap().status,
            DagNodeStatus::Failed(_)
        ));
        assert!(matches!(
            dag.get("c").unwrap().status,
            DagNodeStatus::Skipped
        ));
        assert!(matches!(
            dag.get("d").unwrap().status,
            DagNodeStatus::Skipped
        ));
        assert!(matches!(
            dag.get("a").unwrap().status,
            DagNodeStatus::Pending
        ));
    }

    #[test]
    fn parent_outputs_collected_for_ready_child() {
        let mut dag = ExecutionDag::new();
        dag.add_node(mk_node("p1")).unwrap();
        dag.add_node(mk_node("p2")).unwrap();
        dag.add_node(mk_node("child")).unwrap();
        dag.add_edge("p1", "child").unwrap();
        dag.add_edge("p2", "child").unwrap();
        dag.mark_done("p1", json!("hello"));
        dag.mark_done("p2", json!({"count": 7}));
        let outs = dag.parent_outputs("child");
        assert_eq!(outs.get("p1"), Some(&json!("hello")));
        assert_eq!(outs.get("p2"), Some(&json!({"count": 7})));
    }

    #[test]
    fn completion_after_all_terminal() {
        let mut dag = ExecutionDag::new();
        dag.add_node(mk_node("a")).unwrap();
        dag.add_node(mk_node("b")).unwrap();
        dag.mark_done("a", json!(1));
        dag.mark_failed_and_cascade("b", "x".into());
        assert!(dag.is_complete());
    }

    #[test]
    fn snapshot_round_trips() {
        let mut dag = ExecutionDag::new();
        dag.add_node(mk_node("a")).unwrap();
        dag.add_node(mk_node("b")).unwrap();
        dag.add_edge("a", "b").unwrap();
        let v = dag.to_json();
        let back = ExecutionDag::from_json(v).unwrap();
        assert_eq!(back.node_count(), 2);
        assert_eq!(back.ready_nodes(), vec!["a".to_string()]);
    }

    #[test]
    fn unknown_edge_endpoint_is_rejected() {
        let mut dag = ExecutionDag::new();
        dag.add_node(mk_node("a")).unwrap();
        let err = dag.add_edge("a", "missing").unwrap_err();
        assert!(matches!(err, SwarmError::RegistryMiss(_)));
    }
}
