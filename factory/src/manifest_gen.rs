use crate::capabilities::{manifest_compatible_capabilities, CapabilityPlan};
use crate::intent::{ParsedIntent, TaskType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestGenerationResult {
    pub toml: String,
    pub schedule_cron: Option<String>,
    pub fuel_budget: u64,
}

pub fn generate_manifest_toml(
    intent: &ParsedIntent,
    plan: &CapabilityPlan,
) -> ManifestGenerationResult {
    let name = build_agent_name(intent);
    let fuel_budget = estimate_fuel_budget(intent, plan);
    let schedule_cron = normalize_schedule_to_cron(intent.schedule.as_str());
    let capabilities = manifest_compatible_capabilities(plan.required.as_slice());

    let capabilities_toml = capabilities
        .iter()
        .map(|capability| format!("\"{capability}\""))
        .collect::<Vec<_>>()
        .join(", ");

    let mut lines = vec![
        format!("name = \"{name}\""),
        "version = \"0.1.0\"".to_string(),
        format!("capabilities = [{capabilities_toml}]"),
        format!("fuel_budget = {fuel_budget}"),
    ];

    if let Some(cron) = schedule_cron.as_ref() {
        lines.push(format!("schedule = \"{cron}\""));
    }
    lines.push("llm_model = \"claude-sonnet-4-5\"".to_string());

    ManifestGenerationResult {
        toml: lines.join("\n"),
        schedule_cron,
        fuel_budget,
    }
}

fn build_agent_name(intent: &ParsedIntent) -> String {
    let mut seed = format!(
        "{}-{}",
        primary_task_label(intent.task_type.clone()),
        intent.content_topic
    );
    seed = seed
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();

    let mut parts = seed
        .split('-')
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>();

    if parts.is_empty() {
        parts.push("agent");
    }

    let mut name = parts.join("-");
    if name.len() < 3 {
        name.push_str("-os");
    }
    if name.len() > 64 {
        name.truncate(64);
        while name.ends_with('-') {
            let _ = name.pop();
        }
    }

    name
}

fn primary_task_label(task: TaskType) -> &'static str {
    match task {
        TaskType::ContentPosting => "content",
        TaskType::FileBackup => "backup",
        TaskType::Research => "research",
        TaskType::Monitoring => "monitor",
        TaskType::SelfImprove => "self-improve",
        TaskType::Unknown => "agent",
    }
}

fn estimate_fuel_budget(intent: &ParsedIntent, plan: &CapabilityPlan) -> u64 {
    let mut budget = 2_000_u64;

    budget = budget.saturating_add((intent.platforms.len() as u64).saturating_mul(600));
    budget = budget.saturating_add((plan.required.len() as u64).saturating_mul(250));

    let schedule = intent.schedule.to_lowercase();
    if schedule.contains("daily") || schedule.contains("morning") {
        budget = budget.saturating_add(1_000);
    }
    if schedule.contains("hour") {
        budget = budget.saturating_add(2_000);
    }

    budget.clamp(1_000, 1_000_000)
}

fn normalize_schedule_to_cron(schedule: &str) -> Option<String> {
    let lower = schedule.trim().to_lowercase();

    if lower.is_empty() || lower == "unspecified" {
        return None;
    }
    if lower == "0 0 * * *" || lower == "0 9 * * *" || lower == "0 * * * *" || lower == "@daily" {
        return Some(match lower.as_str() {
            "@daily" => "0 0 * * *".to_string(),
            _ => lower,
        });
    }

    if lower.contains("every morning at 9am") || (lower.contains("daily") && lower.contains("9")) {
        return Some("0 9 * * *".to_string());
    }

    if lower.contains("daily") || lower.contains("every morning") {
        return Some("0 9 * * *".to_string());
    }

    if lower.contains("every hour") {
        return Some("0 * * * *".to_string());
    }
    if lower.contains("every night") || lower.contains("nightly") {
        return Some("0 0 * * *".to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::generate_manifest_toml;
    use crate::capabilities::map_intent_to_capabilities;
    use crate::intent::{ParsedIntent, TaskType};
    use nexus_kernel::manifest::parse_manifest;

    #[test]
    fn test_manifest_generation() {
        let intent = ParsedIntent {
            task_type: TaskType::ContentPosting,
            platforms: vec!["twitter".to_string()],
            schedule: "every morning at 9am".to_string(),
            content_topic: "rust".to_string(),
            raw_request: "Create an agent that posts about Rust on Twitter every morning at 9am"
                .to_string(),
        };
        let capabilities = map_intent_to_capabilities(&intent);
        let generated = generate_manifest_toml(&intent, &capabilities);

        let parsed = parse_manifest(generated.toml.as_str());
        assert!(parsed.is_ok());
    }
}
