//! A2A Client — convenience wrapper over kernel `A2aClient`.
//!
//! Adds batch operations, multi-agent discovery, and result aggregation
//! on top of the governed kernel client.

use crate::types::{A2aClient, A2aClientError, A2aTaskResult, AgentCard};
use serde::{Deserialize, Serialize};

/// Result of discovering multiple agents at once.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchDiscoveryResult {
    pub discovered: Vec<AgentCard>,
    pub failures: Vec<DiscoveryFailure>,
}

/// A failed discovery attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryFailure {
    pub url: String,
    pub error: String,
}

/// Discover multiple agents from a list of URLs.
///
/// Unlike the kernel client's `discover_agent`, this never aborts on failure —
/// it collects both successes and failures for the caller to handle.
pub fn batch_discover(client: &mut A2aClient, urls: &[&str]) -> BatchDiscoveryResult {
    let mut discovered = Vec::new();
    let mut failures = Vec::new();

    for url in urls {
        match client.discover_agent(url) {
            Ok(card) => discovered.push(card),
            Err(e) => failures.push(DiscoveryFailure {
                url: url.to_string(),
                error: e.to_string(),
            }),
        }
    }

    BatchDiscoveryResult {
        discovered,
        failures,
    }
}

/// Send a task and poll until it reaches a terminal state or the max number
/// of iterations is hit.
///
/// Returns the final task result. Polling uses the same `get_task_status` on
/// the governed kernel client (each poll costs fuel).
pub fn send_and_wait(
    client: &mut A2aClient,
    agent_url: &str,
    message: &str,
    max_polls: usize,
) -> Result<A2aTaskResult, A2aClientError> {
    let result = client.send_task(agent_url, message)?;

    if result.status.is_terminal() {
        return Ok(result);
    }

    let mut current = result;
    for _ in 0..max_polls {
        let status = client.get_task_status(agent_url, &current.id)?;
        if status.status.is_terminal() {
            return Ok(status);
        }
        current = status;
    }

    Ok(current)
}

/// Find the best agent for a task based on tag matching.
///
/// Scores each known agent by counting how many of the requested tags it
/// supports. Returns agent names sorted by score (highest first).
pub fn rank_agents_by_tags(client: &A2aClient, tags: &[&str]) -> Vec<(String, usize)> {
    let mut scored: Vec<(String, usize)> = client
        .known_agents()
        .iter()
        .map(|card| {
            let score = card
                .skills
                .iter()
                .flat_map(|s| s.tags.iter())
                .filter(|t| tags.contains(&t.as_str()))
                .count();
            (card.name.clone(), score)
        })
        .filter(|(_, score)| *score > 0)
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentCapabilities, AgentSkill, A2A_PROTOCOL_VERSION};

    fn make_card(name: &str, tags: &[&str]) -> AgentCard {
        AgentCard {
            name: name.to_string(),
            description: Some(format!("Agent {name}")),
            url: format!("http://{name}:9000"),
            version: A2A_PROTOCOL_VERSION.to_string(),
            capabilities: AgentCapabilities::default(),
            skills: vec![AgentSkill {
                id: format!("{name}-skill"),
                name: format!("{name} Skill"),
                description: None,
                tags: tags.iter().map(|t| (*t).to_string()).collect(),
                input_modes: vec![],
                output_modes: vec![],
            }],
            authentication: vec![],
            default_input_modes: vec![],
            default_output_modes: vec![],
            rate_limit_rpm: None,
        }
    }

    #[test]
    fn rank_agents_by_tags_scoring() {
        let mut client = A2aClient::new();
        client.register_agent(make_card("web-agent", &["web", "search", "scraping"]));
        client.register_agent(make_card("code-agent", &["code", "generation"]));
        client.register_agent(make_card("mixed", &["web", "code"]));

        let ranked = rank_agents_by_tags(&client, &["web", "search"]);
        assert!(!ranked.is_empty());
        // web-agent has both "web" and "search" → score 2
        assert_eq!(ranked[0].0, "web-agent");
        assert_eq!(ranked[0].1, 2);
    }

    #[test]
    fn rank_agents_no_match() {
        let mut client = A2aClient::new();
        client.register_agent(make_card("agent-a", &["code"]));
        let ranked = rank_agents_by_tags(&client, &["finance"]);
        assert!(ranked.is_empty());
    }

    #[test]
    fn batch_discover_collects_failures() {
        let mut client = A2aClient::new();
        // These URLs won't have running servers
        let result = batch_discover(&mut client, &["http://127.0.0.1:1", "http://127.0.0.1:2"]);
        assert!(result.discovered.is_empty());
        assert_eq!(result.failures.len(), 2);
        assert!(result.failures[0].url.contains("127.0.0.1:1"));
    }

    #[test]
    fn batch_discovery_result_serialization() {
        let result = BatchDiscoveryResult {
            discovered: vec![],
            failures: vec![DiscoveryFailure {
                url: "http://example.com".to_string(),
                error: "timeout".to_string(),
            }],
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: BatchDiscoveryResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.failures.len(), 1);
        assert_eq!(parsed.failures[0].error, "timeout");
    }

    #[test]
    fn send_and_wait_fails_on_bad_url() {
        let mut client = A2aClient::new();
        let result = send_and_wait(&mut client, "http://127.0.0.1:1", "hello", 3);
        assert!(result.is_err());
    }
}
