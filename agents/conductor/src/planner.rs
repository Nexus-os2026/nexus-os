use crate::types::{AgentRole, ConductorPlan, PlannedTask, UserRequest};
use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::LlmProvider;
use nexus_kernel::errors::AgentError;
use serde::Deserialize;
use std::collections::HashSet;
use uuid::Uuid;

/// Decomposes a user request into an executable plan.
pub struct Planner {
    model_name: String,
    max_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct LlmSubtask {
    role: Option<String>,
    description: Option<String>,
    expected_outputs: Option<Vec<String>>,
    estimated_fuel: Option<u64>,
    depends_on_indices: Option<Vec<usize>>,
}

impl Planner {
    pub fn new(model_name: &str) -> Self {
        Self {
            model_name: model_name.to_string(),
            max_tokens: 512,
        }
    }

    /// Plan via LLM first, falling back to rule-based.
    pub fn plan<P: LlmProvider>(
        &self,
        request: &UserRequest,
        gateway: &mut GovernedLlmGateway<P>,
    ) -> Result<ConductorPlan, AgentError> {
        let capabilities: HashSet<String> = ["llm.query".to_string()].into_iter().collect();
        let mut ctx = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities,
            fuel_remaining: 10_000,
        };

        let prompt = format!(
            r#"Decompose this user request into subtasks. Return a JSON array of objects with keys:
- "role": one of "web_builder", "coder", "designer", "fixer", "general"
- "description": what the subtask does
- "expected_outputs": array of output file paths/names
- "estimated_fuel": integer fuel cost (1000-10000)
- "depends_on_indices": array of zero-based indices of subtasks this depends on

Respond with ONLY a valid JSON array. Request: {}"#,
            request.prompt
        );

        let response = gateway.query(
            &mut ctx,
            prompt.as_str(),
            self.max_tokens,
            self.model_name.as_str(),
        )?;

        if let Ok(subtasks) = serde_json::from_str::<Vec<LlmSubtask>>(&response.output_text) {
            if !subtasks.is_empty() {
                let tasks: Vec<PlannedTask> = subtasks
                    .into_iter()
                    .map(|s| {
                        let role = parse_role(s.role.as_deref().unwrap_or("general"));
                        let capabilities_needed = role.default_capabilities();
                        PlannedTask {
                            description: s.description.unwrap_or_else(|| "subtask".to_string()),
                            role,
                            capabilities_needed,
                            estimated_fuel: s.estimated_fuel.unwrap_or(2000),
                            depends_on: s.depends_on_indices.unwrap_or_default(),
                            expected_outputs: s.expected_outputs.unwrap_or_default(),
                        }
                    })
                    .collect();
                return Ok(ConductorPlan::new(tasks));
            }
        }

        Ok(plan_with_rules(request))
    }

    /// Rule-based planning only (no LLM).
    pub fn plan_with_rules(&self, request: &UserRequest) -> ConductorPlan {
        plan_with_rules(request)
    }
}

fn parse_role(s: &str) -> AgentRole {
    match s.to_lowercase().as_str() {
        "web_builder" | "web-builder" | "webbuilder" => AgentRole::WebBuilder,
        "coder" | "code_gen" => AgentRole::Coder,
        "designer" | "design_gen" => AgentRole::Designer,
        "fixer" | "fix_project" => AgentRole::Fixer,
        _ => AgentRole::General,
    }
}

fn plan_with_rules(request: &UserRequest) -> ConductorPlan {
    let lower = request.prompt.to_lowercase();
    let mut tasks = Vec::new();

    // Detect web building
    let is_web = lower.contains("website")
        || lower.contains("landing page")
        || lower.contains("portfolio")
        || lower.contains("html")
        || lower.contains("3d scene");

    // Detect code generation
    let is_code = lower.contains("app")
        || lower.contains("api")
        || lower.contains("auth")
        || lower.contains("backend")
        || lower.contains("database")
        || lower.contains("stripe")
        || lower.contains("saas");

    // Detect design
    let is_design = lower.contains("design system")
        || lower.contains("component librar")
        || lower.contains("theme")
        || lower.contains("branding");

    // Detect fix
    let is_fix = lower.contains("fix") || lower.contains("debug") || lower.contains("bug");

    // Detect clone
    let has_url = lower.contains("http://") || lower.contains("https://") || lower.contains(".com");
    let is_clone = has_url
        && (lower.contains("clone") || lower.contains("copy") || lower.contains("recreate"));

    if is_clone {
        tasks.push(PlannedTask {
            description: "Fetch and analyze the target website".into(),
            role: AgentRole::WebBuilder,
            capabilities_needed: AgentRole::WebBuilder.default_capabilities(),
            estimated_fuel: 3000,
            depends_on: vec![],
            expected_outputs: vec!["site-analysis.json".into()],
        });
        tasks.push(PlannedTask {
            description: "Rebuild the site with modern technologies".into(),
            role: AgentRole::WebBuilder,
            capabilities_needed: AgentRole::WebBuilder.default_capabilities(),
            estimated_fuel: 5000,
            depends_on: vec![0],
            expected_outputs: vec!["index.html".into(), "styles.css".into()],
        });
    } else if is_web && is_code {
        // Full-stack: design first, then web, then code
        if is_design {
            tasks.push(PlannedTask {
                description: "Create design system and tokens".into(),
                role: AgentRole::Designer,
                capabilities_needed: AgentRole::Designer.default_capabilities(),
                estimated_fuel: 3000,
                depends_on: vec![],
                expected_outputs: vec!["design-tokens.json".into()],
            });
        }
        tasks.push(PlannedTask {
            description: "Build the frontend / website".into(),
            role: AgentRole::WebBuilder,
            capabilities_needed: AgentRole::WebBuilder.default_capabilities(),
            estimated_fuel: 4000,
            depends_on: if is_design { vec![0] } else { vec![] },
            expected_outputs: vec!["index.html".into()],
        });
        let web_idx = tasks.len() - 1;
        tasks.push(PlannedTask {
            description: "Build backend / API".into(),
            role: AgentRole::Coder,
            capabilities_needed: AgentRole::Coder.default_capabilities(),
            estimated_fuel: 5000,
            depends_on: vec![],
            expected_outputs: vec!["server.rs".into()],
        });
        tasks.push(PlannedTask {
            description: "Integrate frontend with backend".into(),
            role: AgentRole::Coder,
            capabilities_needed: AgentRole::Coder.default_capabilities(),
            estimated_fuel: 3000,
            depends_on: vec![web_idx, tasks.len() - 1],
            expected_outputs: vec!["integration-report.md".into()],
        });
    } else if is_web {
        tasks.push(PlannedTask {
            description: "Build the website".into(),
            role: AgentRole::WebBuilder,
            capabilities_needed: AgentRole::WebBuilder.default_capabilities(),
            estimated_fuel: 5000,
            depends_on: vec![],
            expected_outputs: vec!["index.html".into(), "styles.css".into()],
        });
    } else if is_code {
        tasks.push(PlannedTask {
            description: "Generate application code".into(),
            role: AgentRole::Coder,
            capabilities_needed: AgentRole::Coder.default_capabilities(),
            estimated_fuel: 5000,
            depends_on: vec![],
            expected_outputs: vec!["main.rs".into()],
        });
        if lower.contains("auth") {
            tasks.push(PlannedTask {
                description: "Add authentication layer".into(),
                role: AgentRole::Coder,
                capabilities_needed: AgentRole::Coder.default_capabilities(),
                estimated_fuel: 3000,
                depends_on: vec![0],
                expected_outputs: vec!["auth.rs".into()],
            });
        }
    } else if is_design {
        tasks.push(PlannedTask {
            description: "Create design system".into(),
            role: AgentRole::Designer,
            capabilities_needed: AgentRole::Designer.default_capabilities(),
            estimated_fuel: 4000,
            depends_on: vec![],
            expected_outputs: vec!["design-tokens.json".into(), "components.css".into()],
        });
    } else if is_fix {
        tasks.push(PlannedTask {
            description: "Scan and diagnose issues".into(),
            role: AgentRole::Fixer,
            capabilities_needed: AgentRole::Fixer.default_capabilities(),
            estimated_fuel: 2000,
            depends_on: vec![],
            expected_outputs: vec!["diagnosis.json".into()],
        });
        tasks.push(PlannedTask {
            description: "Apply fixes and verify".into(),
            role: AgentRole::Fixer,
            capabilities_needed: AgentRole::Fixer.default_capabilities(),
            estimated_fuel: 4000,
            depends_on: vec![0],
            expected_outputs: vec!["fix-report.md".into()],
        });
    } else {
        // Fallback: general research + execution
        tasks.push(PlannedTask {
            description: "Analyze request and execute".into(),
            role: AgentRole::General,
            capabilities_needed: AgentRole::General.default_capabilities(),
            estimated_fuel: 3000,
            depends_on: vec![],
            expected_outputs: vec!["output.md".into()],
        });
    }

    ConductorPlan::new(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_based_website_plan() {
        let req = UserRequest::new("build a website", "/tmp/out");
        let planner = Planner::new("mock");
        let plan = planner.plan_with_rules(&req);
        assert!(!plan.tasks.is_empty());
        assert_eq!(plan.tasks[0].role, AgentRole::WebBuilder);
    }

    #[test]
    fn test_rule_based_saas_with_auth() {
        let req = UserRequest::new(
            "build a SaaS with auth and a landing page website",
            "/tmp/out",
        );
        let planner = Planner::new("mock");
        let plan = planner.plan_with_rules(&req);
        assert!(plan.tasks.len() >= 2);
        let roles: Vec<_> = plan.tasks.iter().map(|t| &t.role).collect();
        assert!(roles.contains(&&AgentRole::WebBuilder));
        assert!(roles.contains(&&AgentRole::Coder));
    }

    #[test]
    fn test_fallback_never_empty() {
        let req = UserRequest::new("do something random", "/tmp/out");
        let planner = Planner::new("mock");
        let plan = planner.plan_with_rules(&req);
        assert!(!plan.tasks.is_empty());
    }

    #[test]
    fn test_dependency_ordering() {
        let req = UserRequest::new("fix the bugs in my project", "/tmp/out");
        let planner = Planner::new("mock");
        let plan = planner.plan_with_rules(&req);
        assert!(plan.tasks.len() >= 2);
        // Second task depends on first
        assert!(plan.tasks[1].depends_on.contains(&0));
    }

    #[test]
    fn test_clone_site_plan() {
        let req = UserRequest::new("clone https://example.com and modernize it", "/tmp/out");
        let planner = Planner::new("mock");
        let plan = planner.plan_with_rules(&req);
        assert!(plan.tasks.len() >= 2);
        assert_eq!(plan.tasks[0].role, AgentRole::WebBuilder);
    }

    #[test]
    fn test_design_plan() {
        let req = UserRequest::new("create a design system with dark mode", "/tmp/out");
        let planner = Planner::new("mock");
        let plan = planner.plan_with_rules(&req);
        assert_eq!(plan.tasks[0].role, AgentRole::Designer);
    }
}
