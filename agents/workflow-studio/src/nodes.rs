use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodePort {
    pub name: String,
    pub data_type: String,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerNode {
    Schedule,
    Webhook,
    FileChange,
    EmailReceived,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionNode {
    LlmQuery,
    WebSearch,
    PostToSocial,
    SendEmail,
    CreateFile,
    RunCode,
    HttpRequest,
    DatabaseQuery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogicNode {
    IfElse,
    Switch,
    Loop,
    Merge,
    Wait,
    ErrorHandler,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiNode {
    Summarize,
    Classify,
    ExtractData,
    GenerateContent,
    AnalyzeImage,
    TranscribeAudio,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Trigger(TriggerNode),
    Action(ActionNode),
    Logic(LogicNode),
    Ai(AiNode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeErrorStrategy {
    Retry,
    Skip,
    Halt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    pub label: String,
    pub kind: NodeKind,
    pub inputs: Vec<NodePort>,
    pub outputs: Vec<NodePort>,
    pub config: Value,
    pub capabilities_required: Vec<String>,
    pub fuel_cost: u64,
    pub retry_limit: u8,
    pub error_strategy: NodeErrorStrategy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowConnection {
    pub from_node: String,
    pub from_output: String,
    pub to_node: String,
    pub to_input: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub nodes: Vec<WorkflowNode>,
    pub connections: Vec<WorkflowConnection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub workflow: Workflow,
}

pub fn built_in_templates() -> Vec<WorkflowTemplate> {
    vec![
        social_media_manager_template(),
        code_reviewer_template(),
        content_pipeline_template(),
        data_processor_template(),
    ]
}

pub fn social_media_manager_template() -> WorkflowTemplate {
    let nodes = vec![
        trigger_node("trigger", "Schedule", TriggerNode::Schedule),
        action_node(
            "research",
            "Web Research",
            ActionNode::WebSearch,
            &["web.search"],
        ),
        ai_node("generate", "Generate Draft", AiNode::GenerateContent),
        logic_node("approve", "Approval Gate", LogicNode::IfElse),
        action_node(
            "post",
            "Post Content",
            ActionNode::PostToSocial,
            &["social.x.post"],
        ),
        ai_node("analyze", "Analyze Performance", AiNode::Summarize),
    ];

    let connections = vec![
        connection("trigger", "result", "research", "input"),
        connection("research", "result", "generate", "input"),
        connection("generate", "result", "approve", "input"),
        connection("approve", "result", "post", "input"),
        connection("post", "result", "analyze", "input"),
    ];

    WorkflowTemplate {
        id: "social-media-manager".to_string(),
        name: "Social Media Manager".to_string(),
        description: "research -> generate -> approve -> post -> analyze".to_string(),
        workflow: Workflow {
            id: "wf-social-media".to_string(),
            name: "Social Media Manager".to_string(),
            description: "Automated social posting with human approval".to_string(),
            nodes,
            connections,
        },
    }
}

pub fn code_reviewer_template() -> WorkflowTemplate {
    let nodes = vec![
        trigger_node("trigger", "Manual", TriggerNode::Manual),
        action_node(
            "pull",
            "Git Pull",
            ActionNode::RunCode,
            &["terminal.execute"],
        ),
        action_node(
            "scan",
            "Code Scan",
            ActionNode::RunCode,
            &["terminal.execute"],
        ),
        ai_node("analyze", "Analyze Findings", AiNode::Summarize),
        action_node(
            "report",
            "Send Report",
            ActionNode::SendEmail,
            &["messaging.send"],
        ),
    ];

    let connections = vec![
        connection("trigger", "result", "pull", "input"),
        connection("pull", "result", "scan", "input"),
        connection("scan", "result", "analyze", "input"),
        connection("analyze", "result", "report", "input"),
    ];

    WorkflowTemplate {
        id: "code-reviewer".to_string(),
        name: "Code Reviewer".to_string(),
        description: "git pull -> scan -> analyze -> report".to_string(),
        workflow: Workflow {
            id: "wf-code-reviewer".to_string(),
            name: "Code Reviewer".to_string(),
            description: "Automated code review pipeline".to_string(),
            nodes,
            connections,
        },
    }
}

pub fn content_pipeline_template() -> WorkflowTemplate {
    let nodes = vec![
        trigger_node("trigger", "Schedule", TriggerNode::Schedule),
        action_node(
            "research",
            "Research",
            ActionNode::WebSearch,
            &["web.search"],
        ),
        ai_node("write", "Write Draft", AiNode::GenerateContent),
        ai_node("edit", "Edit Draft", AiNode::Summarize),
        action_node(
            "publish",
            "Publish",
            ActionNode::PostToSocial,
            &["social.x.post"],
        ),
        action_node(
            "distribute",
            "Distribute",
            ActionNode::SendEmail,
            &["messaging.send"],
        ),
    ];

    let connections = vec![
        connection("trigger", "result", "research", "input"),
        connection("research", "result", "write", "input"),
        connection("write", "result", "edit", "input"),
        connection("edit", "result", "publish", "input"),
        connection("publish", "result", "distribute", "input"),
    ];

    WorkflowTemplate {
        id: "content-pipeline".to_string(),
        name: "Content Pipeline".to_string(),
        description: "research -> write -> edit -> publish -> distribute".to_string(),
        workflow: Workflow {
            id: "wf-content-pipeline".to_string(),
            name: "Content Pipeline".to_string(),
            description: "Editorial production automation".to_string(),
            nodes,
            connections,
        },
    }
}

pub fn data_processor_template() -> WorkflowTemplate {
    let nodes = vec![
        trigger_node("trigger", "Webhook", TriggerNode::Webhook),
        action_node(
            "fetch",
            "Fetch Data",
            ActionNode::HttpRequest,
            &["web.read"],
        ),
        ai_node("clean", "Clean Data", AiNode::ExtractData),
        logic_node("transform", "Transform", LogicNode::Loop),
        action_node(
            "store",
            "Store Data",
            ActionNode::DatabaseQuery,
            &["database.query"],
        ),
        ai_node("report", "Summarize Report", AiNode::Summarize),
    ];

    let connections = vec![
        connection("trigger", "result", "fetch", "input"),
        connection("fetch", "result", "clean", "input"),
        connection("clean", "result", "transform", "input"),
        connection("transform", "result", "store", "input"),
        connection("store", "result", "report", "input"),
    ];

    WorkflowTemplate {
        id: "data-processor".to_string(),
        name: "Data Processor".to_string(),
        description: "fetch -> clean -> transform -> store -> report".to_string(),
        workflow: Workflow {
            id: "wf-data-processor".to_string(),
            name: "Data Processor".to_string(),
            description: "Data ingestion and reporting".to_string(),
            nodes,
            connections,
        },
    }
}

fn trigger_node(id: &str, label: &str, kind: TriggerNode) -> WorkflowNode {
    WorkflowNode {
        id: id.to_string(),
        label: label.to_string(),
        kind: NodeKind::Trigger(kind),
        inputs: Vec::new(),
        outputs: vec![port("result", "json", true)],
        config: Value::Object(Default::default()),
        capabilities_required: Vec::new(),
        fuel_cost: 1,
        retry_limit: 0,
        error_strategy: NodeErrorStrategy::Halt,
    }
}

fn action_node(id: &str, label: &str, kind: ActionNode, capabilities: &[&str]) -> WorkflowNode {
    WorkflowNode {
        id: id.to_string(),
        label: label.to_string(),
        kind: NodeKind::Action(kind),
        inputs: vec![port("input", "json", false)],
        outputs: vec![port("result", "json", true)],
        config: Value::Object(Default::default()),
        capabilities_required: capabilities.iter().map(|value| value.to_string()).collect(),
        fuel_cost: 2,
        retry_limit: 2,
        error_strategy: NodeErrorStrategy::Retry,
    }
}

fn logic_node(id: &str, label: &str, kind: LogicNode) -> WorkflowNode {
    WorkflowNode {
        id: id.to_string(),
        label: label.to_string(),
        kind: NodeKind::Logic(kind),
        inputs: vec![port("input", "json", false)],
        outputs: vec![port("result", "json", true)],
        config: Value::Object(Default::default()),
        capabilities_required: Vec::new(),
        fuel_cost: 1,
        retry_limit: 0,
        error_strategy: NodeErrorStrategy::Skip,
    }
}

fn ai_node(id: &str, label: &str, kind: AiNode) -> WorkflowNode {
    WorkflowNode {
        id: id.to_string(),
        label: label.to_string(),
        kind: NodeKind::Ai(kind),
        inputs: vec![port("input", "json", false)],
        outputs: vec![port("result", "json", true)],
        config: Value::Object(Default::default()),
        capabilities_required: vec!["llm.query".to_string()],
        fuel_cost: 3,
        retry_limit: 1,
        error_strategy: NodeErrorStrategy::Retry,
    }
}

fn port(name: &str, data_type: &str, required: bool) -> NodePort {
    NodePort {
        name: name.to_string(),
        data_type: data_type.to_string(),
        required,
    }
}

fn connection(
    from_node: &str,
    from_output: &str,
    to_node: &str,
    to_input: &str,
) -> WorkflowConnection {
    WorkflowConnection {
        from_node: from_node.to_string(),
        from_output: from_output.to_string(),
        to_node: to_node.to_string(),
        to_input: to_input.to_string(),
    }
}
