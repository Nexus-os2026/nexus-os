//! Shared test helpers gated behind the `test-utils` feature.
//! Available to integration tests and downstream crates that enable
//! `nexus-kernel/test-utils` in their dev-dependencies.

use crate::actuators::web::{WebSearchBackend, WebSearchResult};

/// A hermetic web search backend that returns deterministic results
/// without any network access. Use this in governance tests that must
/// not depend on internet reachability.
pub struct HermeticSearchBackend;

impl WebSearchBackend for HermeticSearchBackend {
    fn search(&self, query: &str) -> Result<Vec<WebSearchResult>, String> {
        Ok(vec![WebSearchResult {
            title: format!("Result for: {query}"),
            url: "https://hermetic.test/result".to_string(),
            snippet: "Hermetic test result".to_string(),
            relevance_score: 1.0,
        }])
    }
    fn fetch_content(&self, _url: &str) -> Result<String, String> {
        Ok("Hermetic test content".to_string())
    }
}
