use nexus_connectors_web::reader::CleanContent;
use nexus_connectors_web::search::SearchResult;
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citation {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedInsight {
    pub source_url: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchReport {
    pub topic: String,
    pub citations: Vec<Citation>,
    pub insights: Vec<ExtractedInsight>,
    pub read_articles: usize,
    pub fuel_budget: u64,
    pub fuel_consumed: u64,
    pub remaining_fuel: u64,
}

pub trait ResearchDataSource {
    fn search(&mut self, topic: &str, max_results: usize) -> Result<Vec<SearchResult>, AgentError>;
    fn read(&mut self, url: &str) -> Result<CleanContent, AgentError>;
}

pub struct ResearchPipeline<D: ResearchDataSource> {
    data_source: D,
    pub audit_trail: AuditTrail,
    search_cost: u64,
    read_cost: u64,
    read_top_n: usize,
    search_max_results: usize,
}

impl<D: ResearchDataSource> ResearchPipeline<D> {
    pub fn new(data_source: D) -> Self {
        Self {
            data_source,
            audit_trail: AuditTrail::new(),
            search_cost: 10,
            read_cost: 20,
            read_top_n: 3,
            search_max_results: 5,
        }
    }

    pub fn set_cost_model(&mut self, search_cost: u64, read_cost: u64) {
        self.search_cost = search_cost;
        self.read_cost = read_cost;
    }

    pub fn set_limits(&mut self, read_top_n: usize, search_max_results: usize) {
        self.read_top_n = read_top_n;
        self.search_max_results = search_max_results;
    }

    pub fn research(
        &mut self,
        topic: &str,
        fuel_budget: u64,
    ) -> Result<ResearchReport, AgentError> {
        let mut remaining_fuel = fuel_budget;
        if remaining_fuel < self.search_cost {
            return Err(AgentError::FuelExhausted);
        }

        remaining_fuel -= self.search_cost;
        let search_results = self.data_source.search(topic, self.search_max_results)?;

        let mut citations = Vec::new();
        let mut insights = Vec::new();

        for result in search_results.into_iter().take(self.read_top_n) {
            if remaining_fuel < self.read_cost {
                break;
            }

            remaining_fuel -= self.read_cost;
            let content = self.data_source.read(result.url.as_str())?;
            let insight = summarize_content(content.text.as_str(), 160);

            citations.push(Citation {
                title: result.title,
                url: result.url,
                snippet: result.snippet,
            });
            insights.push(ExtractedInsight {
                source_url: content.source_url,
                summary: insight,
            });
        }

        let fuel_consumed = fuel_budget.saturating_sub(remaining_fuel);
        let report = ResearchReport {
            topic: topic.to_string(),
            citations,
            insights,
            read_articles: fuel_consumed
                .saturating_sub(self.search_cost)
                .checked_div(self.read_cost)
                .unwrap_or(0) as usize,
            fuel_budget,
            fuel_consumed,
            remaining_fuel,
        };

        self.audit_trail.append_event(
            uuid::Uuid::nil(),
            EventType::ToolCall,
            json!({
                "event": "research_completed",
                "topic": report.topic,
                "citations": report.citations.len(),
                "read_articles": report.read_articles,
                "fuel_budget": report.fuel_budget,
                "fuel_consumed": report.fuel_consumed,
                "remaining_fuel": report.remaining_fuel
            }),
        )?;

        Ok(report)
    }
}

fn summarize_content(content: &str, max_chars: usize) -> String {
    let compact = content.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars = compact.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return compact;
    }

    chars.into_iter().take(max_chars).collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::{ResearchDataSource, ResearchPipeline};
    use nexus_connectors_web::reader::CleanContent;
    use nexus_connectors_web::search::SearchResult;
    use nexus_kernel::errors::AgentError;

    struct MockDataSource {
        results: Vec<SearchResult>,
    }

    impl MockDataSource {
        fn new() -> Self {
            let mut results = Vec::new();
            for idx in 0..5 {
                results.push(SearchResult {
                    title: format!("Result {idx}"),
                    url: format!("https://example.com/article-{idx}"),
                    snippet: format!("Snippet {idx}"),
                    relevance_score: 0.9,
                });
            }
            Self { results }
        }
    }

    impl ResearchDataSource for MockDataSource {
        fn search(
            &mut self,
            _topic: &str,
            _max_results: usize,
        ) -> Result<Vec<SearchResult>, AgentError> {
            Ok(self.results.clone())
        }

        fn read(&mut self, url: &str) -> Result<CleanContent, AgentError> {
            Ok(CleanContent {
                title: format!("Title for {url}"),
                text: format!("Detailed article content for {url}"),
                word_count: 6,
                source_url: url.to_string(),
                extracted_at: 0,
            })
        }
    }

    #[test]
    fn test_research_pipeline() {
        let data_source = MockDataSource::new();
        let mut pipeline = ResearchPipeline::new(data_source);

        let report = pipeline.research("rust social strategy", 200);
        assert!(report.is_ok());

        if let Ok(report) = report {
            assert_eq!(report.citations.len(), 3);
            assert_eq!(report.read_articles, 3);
            assert_eq!(report.insights.len(), 3);
        }
    }

    #[test]
    fn test_research_respects_fuel_budget() {
        let data_source = MockDataSource::new();
        let mut pipeline = ResearchPipeline::new(data_source);
        pipeline.set_limits(5, 5);

        let report = pipeline.research("bounded fuel", 100);
        assert!(report.is_ok());

        if let Ok(report) = report {
            assert_eq!(report.fuel_consumed, 90);
            assert_eq!(report.remaining_fuel, 10);
            assert_eq!(report.read_articles, 4);

            let fuel_logged = pipeline.audit_trail.events().iter().any(|event| {
                event
                    .payload
                    .get("fuel_consumed")
                    .and_then(|value| value.as_u64())
                    == Some(90)
            });
            assert!(fuel_logged);
        }
    }
}
