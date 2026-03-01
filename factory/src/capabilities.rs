use crate::intent::{ParsedIntent, TaskType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityPlan {
    pub required: Vec<String>,
    pub optional: Vec<String>,
}

pub fn map_intent_to_capabilities(intent: &ParsedIntent) -> CapabilityPlan {
    match intent.task_type {
        TaskType::ContentPosting => {
            let mut required = vec!["llm.query".to_string(), "web.search".to_string()];

            if intent
                .platforms
                .iter()
                .any(|platform| platform == "twitter")
            {
                required.insert(0, "social.x.post".to_string());
            }
            if intent
                .platforms
                .iter()
                .any(|platform| platform == "instagram")
            {
                required.push("social.instagram.post".to_string());
            }
            if intent
                .platforms
                .iter()
                .any(|platform| platform == "facebook")
            {
                required.push("social.facebook.post".to_string());
            }

            required.dedup();
            CapabilityPlan {
                required,
                optional: vec!["audit.read".to_string()],
            }
        }
        TaskType::Research => CapabilityPlan {
            required: vec!["web.search".to_string(), "llm.query".to_string()],
            optional: vec!["audit.read".to_string()],
        },
        TaskType::Monitoring => CapabilityPlan {
            required: vec!["web.search".to_string(), "llm.query".to_string()],
            optional: vec!["messaging.send".to_string(), "audit.read".to_string()],
        },
        TaskType::Unknown => CapabilityPlan {
            required: vec!["llm.query".to_string()],
            optional: vec!["web.search".to_string()],
        },
    }
}

pub fn manifest_compatible_capabilities(capabilities: &[String]) -> Vec<String> {
    let mut normalized = capabilities
        .iter()
        .map(|capability| match capability.as_str() {
            "social.x.post" | "social.instagram.post" | "social.facebook.post" => {
                "social.post".to_string()
            }
            _ => capability.clone(),
        })
        .collect::<Vec<_>>();

    normalized.sort();
    normalized.dedup();
    normalized
}

#[cfg(test)]
mod tests {
    use super::map_intent_to_capabilities;
    use crate::intent::{ParsedIntent, TaskType};

    #[test]
    fn test_capability_mapping() {
        let intent = ParsedIntent {
            task_type: TaskType::ContentPosting,
            platforms: vec!["twitter".to_string()],
            schedule: "daily".to_string(),
            content_topic: "ai".to_string(),
            raw_request: "Post about AI on Twitter daily".to_string(),
        };

        let plan = map_intent_to_capabilities(&intent);
        assert_eq!(
            plan.required,
            vec![
                "social.x.post".to_string(),
                "llm.query".to_string(),
                "web.search".to_string()
            ]
        );
    }
}
