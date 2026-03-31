//! Visual workflow studio runtime for DAG-based automation with AI-capable nodes.

pub mod engine;
pub mod nodes;
pub mod serialize;

#[cfg(test)]
mod tests {
    use super::*;
    use nodes::*;
    use serde_json::json;

    fn simple_node(id: &str, kind: NodeKind) -> WorkflowNode {
        WorkflowNode {
            id: id.to_string(),
            label: id.to_string(),
            kind,
            inputs: Vec::new(),
            outputs: Vec::new(),
            config: serde_json::Value::Null,
            capabilities_required: Vec::new(),
            fuel_cost: 10,
            retry_limit: 1,
            error_strategy: NodeErrorStrategy::Halt,
        }
    }

    fn two_node_workflow() -> Workflow {
        Workflow {
            id: "wf-1".into(),
            name: "Test Workflow".into(),
            description: "A two-node test workflow".into(),
            nodes: vec![
                simple_node("trigger", NodeKind::Trigger(TriggerNode::Manual)),
                simple_node("action", NodeKind::Action(ActionNode::LlmQuery)),
            ],
            connections: vec![WorkflowConnection {
                from_node: "trigger".into(),
                from_output: "out".into(),
                to_node: "action".into(),
                to_input: "in".into(),
            }],
        }
    }

    // ── Built-in templates ──

    #[test]
    fn built_in_templates_count() {
        let templates = nodes::built_in_templates();
        assert_eq!(templates.len(), 4);
    }

    #[test]
    fn social_media_template_has_nodes_and_connections() {
        let t = nodes::social_media_manager_template();
        assert!(!t.workflow.nodes.is_empty());
        assert!(!t.workflow.connections.is_empty());
        assert!(!t.name.is_empty());
    }

    #[test]
    fn code_reviewer_template_has_nodes() {
        let t = nodes::code_reviewer_template();
        assert!(t.workflow.nodes.len() >= 3);
    }

    #[test]
    fn content_pipeline_template_has_connections() {
        let t = nodes::content_pipeline_template();
        assert!(!t.workflow.connections.is_empty());
    }

    #[test]
    fn data_processor_template_starts_with_webhook() {
        let t = nodes::data_processor_template();
        let first = &t.workflow.nodes[0];
        assert!(matches!(
            first.kind,
            NodeKind::Trigger(TriggerNode::Webhook)
        ));
    }

    // ── Serialization / archiving ──

    #[test]
    fn archive_from_workflow_has_one_version() {
        let wf = two_node_workflow();
        let archive = serialize::WorkflowArchive::from_workflow(wf, "initial");
        assert_eq!(archive.versions.len(), 1);
        assert!(archive.current_workflow().is_some());
    }

    #[test]
    fn archive_add_version_tracks_history() {
        let wf = two_node_workflow();
        let mut archive = serialize::WorkflowArchive::from_workflow(wf.clone(), "v1");
        let v2_id = archive.add_version(wf.clone(), "v2");
        assert_eq!(archive.versions.len(), 2);
        assert_eq!(archive.current_version_id, v2_id);
        let latest = archive.versions.last().unwrap();
        assert!(latest.parent_version_id.is_some());
    }

    #[test]
    fn export_import_roundtrip() {
        let wf = two_node_workflow();
        let archive = serialize::WorkflowArchive::from_workflow(wf, "test");
        let json = serialize::export_workflow(&archive).unwrap();
        let restored = serialize::import_workflow(&json).unwrap();
        assert_eq!(restored.versions.len(), 1);
        assert_eq!(restored.current_workflow().unwrap().name, "Test Workflow");
    }

    #[test]
    fn import_rejects_empty_versions() {
        let json = r#"{"schema_version":"1.0.0","current_version_id":"xxx","versions":[]}"#;
        let result = serialize::import_workflow(json);
        assert!(result.is_err());
    }

    #[test]
    fn import_rejects_bad_current_version_id() {
        let wf = two_node_workflow();
        let mut archive = serialize::WorkflowArchive::from_workflow(wf, "test");
        archive.current_version_id = "nonexistent".into();
        let json = serde_json::to_string(&archive).unwrap();
        let result = serialize::import_workflow(&json);
        assert!(result.is_err());
    }

    // ── Engine: validation ──

    #[test]
    fn engine_execute_simple_workflow() {
        let wf = two_node_workflow();
        let engine = engine::WorkflowEngine::default();
        let mut ctx = engine::WorkflowContext {
            capabilities: std::collections::HashSet::new(),
            fuel_remaining: 1000,
            agent_id: uuid::Uuid::nil(),
            autonomy_guard: nexus_sdk::autonomy::AutonomyGuard::new(
                nexus_sdk::autonomy::AutonomyLevel::L3,
            ),
            audit_trail: nexus_sdk::audit::AuditTrail::new(),
            consent_runtime: nexus_sdk::consent::ConsentRuntime::default(),
        };
        let report = engine
            .execute(&wf, json!({"start": true}), &mut ctx)
            .unwrap();
        assert_eq!(report.execution_order.len(), 2);
        assert!(!report.halted);
    }

    #[test]
    fn engine_rejects_empty_workflow() {
        let wf = Workflow {
            id: "empty".into(),
            name: "Empty".into(),
            description: String::new(),
            nodes: Vec::new(),
            connections: Vec::new(),
        };
        let engine = engine::WorkflowEngine::default();
        let mut ctx = engine::WorkflowContext {
            capabilities: std::collections::HashSet::new(),
            fuel_remaining: 1000,
            agent_id: uuid::Uuid::nil(),
            autonomy_guard: nexus_sdk::autonomy::AutonomyGuard::new(
                nexus_sdk::autonomy::AutonomyLevel::L3,
            ),
            audit_trail: nexus_sdk::audit::AuditTrail::new(),
            consent_runtime: nexus_sdk::consent::ConsentRuntime::default(),
        };
        let result = engine.execute(&wf, json!({}), &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn engine_rejects_cyclic_workflow() {
        let wf = Workflow {
            id: "cycle".into(),
            name: "Cycle".into(),
            description: String::new(),
            nodes: vec![
                simple_node("a", NodeKind::Action(ActionNode::RunCode)),
                simple_node("b", NodeKind::Action(ActionNode::RunCode)),
            ],
            connections: vec![
                WorkflowConnection {
                    from_node: "a".into(),
                    from_output: "out".into(),
                    to_node: "b".into(),
                    to_input: "in".into(),
                },
                WorkflowConnection {
                    from_node: "b".into(),
                    from_output: "out".into(),
                    to_node: "a".into(),
                    to_input: "in".into(),
                },
            ],
        };
        let engine = engine::WorkflowEngine::default();
        let mut ctx = engine::WorkflowContext {
            capabilities: std::collections::HashSet::new(),
            fuel_remaining: 1000,
            agent_id: uuid::Uuid::nil(),
            autonomy_guard: nexus_sdk::autonomy::AutonomyGuard::new(
                nexus_sdk::autonomy::AutonomyLevel::L3,
            ),
            audit_trail: nexus_sdk::audit::AuditTrail::new(),
            consent_runtime: nexus_sdk::consent::ConsentRuntime::default(),
        };
        let result = engine.execute(&wf, json!({}), &mut ctx);
        assert!(result.is_err());
    }

    // ── Node serialization ──

    #[test]
    fn workflow_node_roundtrip() {
        let node = simple_node("n1", NodeKind::Ai(AiNode::Summarize));
        let json = serde_json::to_string(&node).unwrap();
        let restored: WorkflowNode = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "n1");
        assert!(matches!(restored.kind, NodeKind::Ai(AiNode::Summarize)));
    }

    #[test]
    fn workflow_connection_roundtrip() {
        let conn = WorkflowConnection {
            from_node: "a".into(),
            from_output: "out".into(),
            to_node: "b".into(),
            to_input: "in".into(),
        };
        let json = serde_json::to_string(&conn).unwrap();
        let restored: WorkflowConnection = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, conn);
    }
}
