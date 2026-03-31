use crate::types::ConvertedTool;

/// Map a CrewAI tool name to a Nexus OS capability.
pub fn map_crewai_tool(tool_name: &str) -> ConvertedTool {
    let trimmed = tool_name.trim();
    match trimmed {
        // ── Web / search ──
        "SerperDevTool"
        | "SearchTool"
        | "GoogleSearchTool"
        | "DuckDuckGoSearchTool"
        | "BingSearchTool"
        | "EXASearchTool" => ConvertedTool {
            original_name: trimmed.to_string(),
            nexus_capability: "web.search".to_string(),
            mapped: true,
            notes: None,
        },
        "ScrapeWebsiteTool"
        | "WebsiteSearchTool"
        | "SeleniumScrapingTool"
        | "FirecrawlScrapeWebsiteTool"
        | "FirecrawlCrawlWebsiteTool"
        | "FirecrawlSearchTool"
        | "BrowserbaseLoadTool"
        | "ScrapflyScrapeWebsiteTool" => ConvertedTool {
            original_name: trimmed.to_string(),
            nexus_capability: "web.fetch".to_string(),
            mapped: true,
            notes: None,
        },
        // ── File operations ──
        "FileReadTool" | "DirectoryReadTool" | "TXTSearchTool" | "CSVSearchTool"
        | "JSONSearchTool" | "XMLSearchTool" | "MDXSearchTool" | "DOCXSearchTool"
        | "PDFSearchTool" => ConvertedTool {
            original_name: trimmed.to_string(),
            nexus_capability: "file.read".to_string(),
            mapped: true,
            notes: None,
        },
        "FileWriteTool" | "DirectoryWriteTool" => ConvertedTool {
            original_name: trimmed.to_string(),
            nexus_capability: "file.write".to_string(),
            mapped: true,
            notes: None,
        },
        // ── Code ──
        "CodeInterpreterTool" | "CodeDocsSearchTool" => ConvertedTool {
            original_name: trimmed.to_string(),
            nexus_capability: "code.execute".to_string(),
            mapped: true,
            notes: None,
        },
        "GithubSearchTool" => ConvertedTool {
            original_name: trimmed.to_string(),
            nexus_capability: "code.search".to_string(),
            mapped: true,
            notes: None,
        },
        // ── Database ──
        "PGSearchTool" | "MySQLSearchTool" | "NL2SQLTool" => ConvertedTool {
            original_name: trimmed.to_string(),
            nexus_capability: "database.query".to_string(),
            mapped: true,
            notes: None,
        },
        // ── Knowledge / RAG ──
        "RagTool" | "QdrantVectorSearchTool" | "ChromaDBSearchTool" => ConvertedTool {
            original_name: trimmed.to_string(),
            nexus_capability: "knowledge.search".to_string(),
            mapped: true,
            notes: None,
        },
        // ── Media ──
        "YoutubeVideoSearchTool" | "YoutubeChannelSearchTool" => ConvertedTool {
            original_name: trimmed.to_string(),
            nexus_capability: "media.search".to_string(),
            mapped: true,
            notes: None,
        },
        "DallETool" | "VisionTool" => ConvertedTool {
            original_name: trimmed.to_string(),
            nexus_capability: "media.generate".to_string(),
            mapped: true,
            notes: None,
        },
        // ── Communication ──
        "SlackTool" | "ComposioTool" | "LlamaIndexTool" | "MultiOnTool" => ConvertedTool {
            original_name: trimmed.to_string(),
            nexus_capability: "integration.external".to_string(),
            mapped: true,
            notes: Some(format!(
                "'{trimmed}' mapped to generic integration capability. \
                 Configure the specific integration in Nexus OS settings."
            )),
        },
        // ── Unmapped ──
        _ => ConvertedTool {
            original_name: trimmed.to_string(),
            nexus_capability: format!(
                "custom.{}",
                trimmed
                    .trim_end_matches("Tool")
                    .to_lowercase()
                    .replace(' ', "_")
            ),
            mapped: false,
            notes: Some(format!(
                "No direct Nexus OS equivalent for '{trimmed}'. \
                 Create a custom capability or map to an existing one."
            )),
        },
    }
}

/// Map an LLM model string to (provider, model).
pub fn map_llm_config(llm_str: &str) -> (Option<String>, Option<String>) {
    let s = llm_str.trim();
    if s.is_empty() {
        return (None, None);
    }

    // Provider-prefixed formats: "provider/model"
    if let Some((prefix, model)) = s.split_once('/') {
        let provider = match prefix.to_lowercase().as_str() {
            "openai" | "azure" => "openai",
            "anthropic" => "anthropic",
            "google" | "gemini" | "vertex_ai" | "models" => "google",
            "groq" => "groq",
            "ollama" => "ollama",
            "deepseek" => "deepseek",
            "together" | "together_ai" => "together",
            "fireworks" | "fireworks_ai" => "fireworks",
            "mistral" | "mistralai" => "mistral",
            "cohere" => "cohere",
            "perplexity" => "perplexity",
            "openrouter" => "openrouter",
            _ => return (Some(prefix.to_string()), Some(model.to_string())),
        };
        return (Some(provider.to_string()), Some(model.to_string()));
    }

    // Bare model names
    match s {
        m if m.starts_with("gpt-") || m.starts_with("o1") || m.starts_with("o3") => {
            (Some("openai".into()), Some(m.into()))
        }
        m if m.starts_with("claude-") => (Some("anthropic".into()), Some(m.into())),
        m if m.starts_with("gemini") => (Some("google".into()), Some(m.into())),
        m if m.starts_with("deepseek") => (Some("deepseek".into()), Some(m.into())),
        m if m.starts_with("command") || m.starts_with("c4ai") => {
            (Some("cohere".into()), Some(m.into()))
        }
        m if m.starts_with("mixtral") || m.starts_with("mistral") || m.starts_with("codestral") => {
            (Some("mistral".into()), Some(m.into()))
        }
        m if m.starts_with("llama")
            || m.starts_with("qwen")
            || m.starts_with("phi-")
            || m.starts_with("starcoder") =>
        {
            (Some("ollama".into()), Some(m.into()))
        }
        m => (None, Some(m.into())),
    }
}

/// Collect unique Nexus OS capabilities from a list of converted tools.
pub fn collect_capabilities(tools: &[ConvertedTool]) -> Vec<String> {
    let mut caps: Vec<String> = tools.iter().map(|t| t.nexus_capability.clone()).collect();
    caps.sort();
    caps.dedup();
    caps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_serper_tool() {
        let t = map_crewai_tool("SerperDevTool");
        assert!(t.mapped);
        assert_eq!(t.nexus_capability, "web.search");
    }

    #[test]
    fn test_map_scrape_tool() {
        let t = map_crewai_tool("ScrapeWebsiteTool");
        assert!(t.mapped);
        assert_eq!(t.nexus_capability, "web.fetch");
    }

    #[test]
    fn test_map_file_read_tool() {
        let t = map_crewai_tool("FileReadTool");
        assert!(t.mapped);
        assert_eq!(t.nexus_capability, "file.read");
    }

    #[test]
    fn test_map_code_interpreter() {
        let t = map_crewai_tool("CodeInterpreterTool");
        assert!(t.mapped);
        assert_eq!(t.nexus_capability, "code.execute");
    }

    #[test]
    fn test_map_unknown_tool() {
        let t = map_crewai_tool("MyCustomTool");
        assert!(!t.mapped);
        assert_eq!(t.nexus_capability, "custom.mycustom");
        assert!(t.notes.is_some());
    }

    #[test]
    fn test_map_unknown_tool_no_tool_suffix() {
        let t = map_crewai_tool("SomeWidget");
        assert!(!t.mapped);
        assert_eq!(t.nexus_capability, "custom.somewidget");
    }

    #[test]
    fn test_llm_gpt4o() {
        let (p, m) = map_llm_config("gpt-4o");
        assert_eq!(p.as_deref(), Some("openai"));
        assert_eq!(m.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn test_llm_claude() {
        let (p, m) = map_llm_config("claude-3-opus");
        assert_eq!(p.as_deref(), Some("anthropic"));
        assert_eq!(m.as_deref(), Some("claude-3-opus"));
    }

    #[test]
    fn test_llm_ollama_llama() {
        let (p, m) = map_llm_config("llama3.1");
        assert_eq!(p.as_deref(), Some("ollama"));
        assert_eq!(m.as_deref(), Some("llama3.1"));
    }

    #[test]
    fn test_llm_prefixed() {
        let (p, m) = map_llm_config("openai/gpt-4-turbo");
        assert_eq!(p.as_deref(), Some("openai"));
        assert_eq!(m.as_deref(), Some("gpt-4-turbo"));
    }

    #[test]
    fn test_llm_gemini() {
        let (p, m) = map_llm_config("gemini-1.5-pro");
        assert_eq!(p.as_deref(), Some("google"));
        assert_eq!(m.as_deref(), Some("gemini-1.5-pro"));
    }

    #[test]
    fn test_llm_deepseek() {
        let (p, m) = map_llm_config("deepseek-coder");
        assert_eq!(p.as_deref(), Some("deepseek"));
        assert_eq!(m.as_deref(), Some("deepseek-coder"));
    }

    #[test]
    fn test_llm_empty() {
        let (p, m) = map_llm_config("");
        assert!(p.is_none());
        assert!(m.is_none());
    }

    #[test]
    fn test_llm_unknown_bare() {
        let (p, m) = map_llm_config("some-custom-model");
        assert!(p.is_none());
        assert_eq!(m.as_deref(), Some("some-custom-model"));
    }

    #[test]
    fn test_collect_capabilities_dedup() {
        let tools = vec![
            map_crewai_tool("SerperDevTool"),
            map_crewai_tool("GoogleSearchTool"),
            map_crewai_tool("FileReadTool"),
        ];
        let caps = collect_capabilities(&tools);
        assert_eq!(caps, vec!["file.read", "web.search"]);
    }
}
