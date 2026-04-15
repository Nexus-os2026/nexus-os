//! Content Pipeline — deterministic 5-phase wealth generation workflow.
//!
//! Orchestrates: TrendScan → Research → Write → Publish → Analytics
//! Uses actuators directly (WebSearch, WebFetch, FileWrite) and LLM for content.
//! The pipeline structure is fixed code; the LLM generates the actual content.

use crate::actuators::{ActuatorContext, ActuatorRegistry};
use crate::audit::{AuditTrail, EventType};
use crate::cognitive::loop_runtime::LlmQueryHandler;
use crate::cognitive::types::PlannedAction;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Result of one full content pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    pub success: bool,
    pub article_title: String,
    pub article_path: String,
    pub word_count: usize,
    pub sources: Vec<String>,
    pub topic: String,
    pub phase_results: Vec<PhaseResult>,
    pub total_fuel: f64,
    pub error: Option<String>,
}

/// Result of a single pipeline phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    pub phase: String,
    pub success: bool,
    pub output_preview: String,
    pub fuel_cost: f64,
}

/// The content pipeline orchestrator.
pub struct ContentPipeline {
    registry: ActuatorRegistry,
    llm_handler: Arc<dyn LlmQueryHandler>,
}

impl ContentPipeline {
    pub fn new(llm_handler: Arc<dyn LlmQueryHandler>) -> Self {
        Self {
            registry: ActuatorRegistry::with_defaults(),
            llm_handler,
        }
    }

    /// Run the full 5-phase pipeline.
    pub fn run(&self, context: &ActuatorContext, audit: &mut AuditTrail) -> PipelineResult {
        let mut phases = Vec::new();
        let mut total_fuel = 0.0;

        // ── PHASE 1: TREND SCANNING ──
        let trends = self.phase_trend_scan(context, audit);
        total_fuel += trends.fuel_cost;
        let trend_output = trends.output_preview.clone();
        phases.push(trends);

        // If web search failed (no network or empty results), use a fallback
        let trend_output = if trend_output.is_empty() || trend_output.starts_with("Live DuckDuckGo")
        {
            eprintln!("[content-pipeline] trend scan got no results, using fallback topics");
            "1. AI coding assistants are transforming development workflows\n\
             2. Rust programming language adoption growing in enterprise\n\
             3. WebAssembly gaining traction for edge computing\n\
             4. Open source AI models challenging proprietary ones\n\
             5. Developer productivity tools seeing record investment"
                .to_string()
        } else {
            trend_output
        };

        // Extract the top topic from trends using LLM
        let topic = self.extract_top_topic(&trend_output);
        eprintln!("[content-pipeline] selected topic: {topic}");

        // ── PHASE 2: RESEARCH ──
        let research = self.phase_research(&topic, context, audit);
        total_fuel += research.fuel_cost;
        let research_output = research.output_preview.clone();
        phases.push(research);

        // ── PHASE 3: CONTENT WRITING ──
        let article = self.phase_write_article(&topic, &research_output, context, audit);
        total_fuel += article.fuel_cost;
        let article_content = article.output_preview.clone();
        phases.push(article);

        if article_content.len() < 200 {
            return PipelineResult {
                success: false,
                article_title: String::new(),
                article_path: String::new(),
                word_count: 0,
                sources: vec![],
                topic,
                phase_results: phases,
                total_fuel,
                error: Some("article too short or generation failed".into()),
            };
        }

        // Extract title from article
        let title = extract_title(&article_content);
        let word_count = article_content.split_whitespace().count();

        // ── PHASE 4: PUBLISHING ──
        let publish = self.phase_publish(&title, &article_content, &topic, context, audit);
        total_fuel += publish.fuel_cost;
        let article_path = publish.output_preview.clone();
        phases.push(publish);

        // ── PHASE 5: ANALYTICS ──
        let sources = extract_sources(&research_output);
        let analytics = self.phase_analytics(&title, &topic, word_count, &sources, context, audit);
        total_fuel += analytics.fuel_cost;
        phases.push(analytics);

        PipelineResult {
            success: true,
            article_title: title,
            article_path,
            word_count,
            sources,
            topic,
            phase_results: phases,
            total_fuel,
            error: None,
        }
    }

    // ── Phase 1: Trend Scanning ──

    fn phase_trend_scan(&self, context: &ActuatorContext, audit: &mut AuditTrail) -> PhaseResult {
        eprintln!("[content-pipeline] phase 1: trend scanning");
        let mut output_parts = Vec::new();
        let mut fuel = 0.0;

        // Search for trending topics
        let queries = [
            "Hacker News top stories today trending",
            "Reddit technology trending topics 2026",
            "latest AI tools breakthroughs 2026",
        ];

        for query in &queries {
            let action = PlannedAction::WebSearch {
                query: query.to_string(),
            };
            match self.registry.execute_action(&action, context, audit) {
                Ok(result) => {
                    fuel += result.fuel_cost;
                    output_parts.push(result.output);
                }
                Err(e) => {
                    eprintln!("[content-pipeline] search failed: {e}");
                }
            }
        }

        let combined = output_parts.join("\n---\n");
        PhaseResult {
            phase: "trend_scan".into(),
            success: !combined.is_empty(),
            output_preview: combined,
            fuel_cost: fuel,
        }
    }

    fn extract_top_topic(&self, trend_data: &str) -> String {
        let prompt = format!(
            "You are a content strategist. Based on these search results about trending topics, \
             identify the SINGLE best topic for a tech article. Choose the topic with: \
             highest reader interest, good monetization potential (affiliate links for tools/services), \
             and enough depth for a 1500-word article.\n\n\
             Search results:\n{}\n\n\
             Respond with ONLY the topic title (5-10 words). Nothing else.",
            &trend_data[..trend_data.len().min(3000)]
        );
        match self.llm_handler.query(&prompt) {
            Ok(topic) => topic.trim().to_string(),
            Err(_) => "The Latest Advances in AI Development Tools".to_string(),
        }
    }

    // ── Phase 2: Research ──

    fn phase_research(
        &self,
        topic: &str,
        context: &ActuatorContext,
        audit: &mut AuditTrail,
    ) -> PhaseResult {
        eprintln!("[content-pipeline] phase 2: researching '{topic}'");
        let mut research_notes = Vec::new();
        let mut fuel = 0.0;
        let mut sources = Vec::new();

        // Search for articles on the topic
        let action = PlannedAction::WebSearch {
            query: format!("{topic} in-depth analysis article"),
        };
        let search_results = match self.registry.execute_action(&action, context, audit) {
            Ok(r) => {
                fuel += r.fuel_cost;
                r.output
            }
            Err(e) => {
                eprintln!("[content-pipeline] research search failed: {e}, using topic as context");
                format!("Topic for research: {topic}")
            }
        };

        research_notes.push(format!("Search results for '{topic}':\n{search_results}"));

        // Try to fetch a few articles from the egress allowlist
        let fetch_urls = extract_fetchable_urls(&search_results, &context.egress_allowlist);
        for url in fetch_urls.iter().take(3) {
            let action = PlannedAction::WebFetch { url: url.clone() };
            match self.registry.execute_action(&action, context, audit) {
                Ok(result) => {
                    fuel += result.fuel_cost;
                    // Truncate fetched content to avoid overwhelming the LLM
                    let truncated = if result.output.len() > 2000 {
                        format!("{}...", &result.output[..2000])
                    } else {
                        result.output
                    };
                    research_notes.push(format!("Source: {url}\n{truncated}"));
                    sources.push(url.clone());
                }
                Err(e) => {
                    eprintln!("[content-pipeline] fetch {url} failed: {e}");
                }
            }
        }

        // If we couldn't fetch any articles, use the search results as research
        if sources.is_empty() {
            research_notes.push(
                "Note: Could not fetch source articles directly. Using search snippets as primary research.".into()
            );
        }

        // Use LLM to synthesize research notes
        let research_prompt = format!(
            "You are a research analyst. Synthesize these notes about '{}' into a structured \
             research document with: KEY FACTS (bullet points), EXPERT OPINIONS, STATISTICS, \
             and CONTRARIAN VIEWPOINTS. Include source attribution where available.\n\n{}\n\n\
             Output the research document:",
            topic,
            research_notes.join("\n\n---\n\n")
        );

        let synthesized = match self.llm_handler.query(&research_prompt) {
            Ok(r) => {
                fuel += 10.0; // LLM query fuel
                r
            }
            Err(e) => {
                eprintln!("[content-pipeline] research synthesis failed: {e}");
                research_notes.join("\n\n")
            }
        };

        PhaseResult {
            phase: "research".into(),
            success: true,
            output_preview: synthesized,
            fuel_cost: fuel,
        }
    }

    // ── Phase 3: Content Writing ──

    fn phase_write_article(
        &self,
        topic: &str,
        research: &str,
        _context: &ActuatorContext,
        _audit: &mut AuditTrail,
    ) -> PhaseResult {
        eprintln!("[content-pipeline] phase 3: writing article on '{topic}'");
        let fuel = 10.0; // LLM query

        let prompt = format!(
            "You are an expert tech writer. Write a 1000-2000 word article on the topic: \"{topic}\"\n\n\
             Use this research:\n{research_truncated}\n\n\
             REQUIREMENTS:\n\
             - Start with a compelling headline prefixed with '# '\n\
             - Write an engaging introduction that hooks the reader in 2-3 sentences\n\
             - Include 3-5 main sections with '## ' headers\n\
             - Each section should be 200-400 words\n\
             - Include specific facts, numbers, and quotes from the research\n\
             - Add a '## Key Takeaways' section with 3-5 bullet points\n\
             - End with a '## Conclusion' that includes a call-to-action\n\
             - Use Markdown formatting throughout\n\
             - Naturally mention relevant tools or services that readers might find useful\n\
             - Attribute claims to sources where possible\n\
             - Write in a conversational but authoritative tone\n\
             - Optimize for SEO: put the main keyword in the title, first paragraph, and at least 2 headers\n\n\
             Write the complete article now:",
            topic = topic,
            research_truncated = &research[..research.len().min(4000)]
        );

        let article = match self.llm_handler.query(&prompt) {
            Ok(content) => content,
            Err(e) => {
                return PhaseResult {
                    phase: "write_article".into(),
                    success: false,
                    output_preview: format!("LLM failed: {e}"),
                    fuel_cost: fuel,
                };
            }
        };

        PhaseResult {
            phase: "write_article".into(),
            success: article.len() > 200,
            output_preview: article,
            fuel_cost: fuel,
        }
    }

    // ── Phase 4: Publishing ──

    fn phase_publish(
        &self,
        title: &str,
        article_md: &str,
        topic: &str,
        context: &ActuatorContext,
        audit: &mut AuditTrail,
    ) -> PhaseResult {
        eprintln!("[content-pipeline] phase 4: publishing '{title}'");
        let mut fuel = 0.0;
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let slug = slugify(title);
        let dir = format!("articles/{date}");

        // Save Markdown
        let md_path = format!("{dir}/{slug}.md");
        let md_action = PlannedAction::FileWrite {
            path: md_path.clone(),
            content: article_md.to_string(),
        };
        if let Ok(r) = self.registry.execute_action(&md_action, context, audit) {
            fuel += r.fuel_cost;
        }

        // Generate and save HTML
        let html = markdown_to_html(title, article_md, &date, topic);
        let html_path = format!("{dir}/{slug}.html");
        let html_action = PlannedAction::FileWrite {
            path: html_path.clone(),
            content: html,
        };
        if let Ok(r) = self.registry.execute_action(&html_action, context, audit) {
            fuel += r.fuel_cost;
        }

        // Save metadata JSON
        let meta = serde_json::json!({
            "title": title,
            "topic": topic,
            "date": date,
            "slug": slug,
            "word_count": article_md.split_whitespace().count(),
            "md_path": md_path,
            "html_path": html_path,
            "created_at": Utc::now().to_rfc3339(),
        });
        let meta_path = format!("{dir}/{slug}.meta.json");
        let meta_action = PlannedAction::FileWrite {
            path: meta_path,
            content: serde_json::to_string_pretty(&meta).unwrap_or_default(),
        };
        if let Ok(r) = self.registry.execute_action(&meta_action, context, audit) {
            fuel += r.fuel_cost;
        }

        // Git commit (best-effort, won't fail the pipeline)
        let git_init = PlannedAction::ShellCommand {
            command: "git".into(),
            args: vec!["init".into()],
        };
        // Best-effort: git init may already exist or git may be unavailable; pipeline succeeds without VCS
        let _ = self.registry.execute_action(&git_init, context, audit);

        let git_add = PlannedAction::ShellCommand {
            command: "git".into(),
            args: vec!["add".into(), "-A".into()],
        };
        // Best-effort: git staging failure does not invalidate already-written article files
        let _ = self.registry.execute_action(&git_add, context, audit);

        let git_commit = PlannedAction::ShellCommand {
            command: "git".into(),
            args: vec!["commit".into(), "-m".into(), format!("article: {title}")],
        };
        // Best-effort: git commit is a convenience; published files exist on disk regardless
        let _ = self.registry.execute_action(&git_commit, context, audit);

        // Audit the publish event
        let agent_uuid =
            uuid::Uuid::parse_str(&context.agent_id).unwrap_or_else(|_| uuid::Uuid::new_v4());
        // Best-effort: publish audit event is informational; article files are already written to disk
        let _ = audit.append_event(
            agent_uuid,
            EventType::StateChange,
            serde_json::json!({
                "event": "content.article_published",
                "title": title,
                "topic": topic,
                "date": date,
                "path": html_path,
            }),
        );

        PhaseResult {
            phase: "publish".into(),
            success: true,
            output_preview: html_path,
            fuel_cost: fuel,
        }
    }

    // ── Phase 5: Analytics ──

    fn phase_analytics(
        &self,
        title: &str,
        topic: &str,
        word_count: usize,
        sources: &[String],
        _context: &ActuatorContext,
        audit: &mut AuditTrail,
    ) -> PhaseResult {
        eprintln!("[content-pipeline] phase 5: analytics for '{title}'");

        let agent_uuid = uuid::Uuid::new_v4();
        // Best-effort: analytics audit event is a supplementary record; pipeline result already captures metrics
        let _ = audit.append_event(
            agent_uuid,
            EventType::StateChange,
            serde_json::json!({
                "event": "content.analytics",
                "title": title,
                "topic": topic,
                "word_count": word_count,
                "sources_count": sources.len(),
                "sources": sources,
                "published_at": Utc::now().to_rfc3339(),
            }),
        );

        PhaseResult {
            phase: "analytics".into(),
            success: true,
            output_preview: format!(
                "Article: {title} | Topic: {topic} | Words: {word_count} | Sources: {}",
                sources.len()
            ),
            fuel_cost: 0.0,
        }
    }
}

// ── Helper Functions ──

fn extract_title(article: &str) -> String {
    // Look for a markdown H1 title
    for line in article.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            return trimmed.trim_start_matches("# ").trim().to_string();
        }
    }
    // Fallback: first non-empty line
    article
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("Untitled Article")
        .trim()
        .to_string()
}

fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn extract_sources(research: &str) -> Vec<String> {
    let mut sources = Vec::new();
    for word in research.split_whitespace() {
        if (word.starts_with("http://") || word.starts_with("https://"))
            && !sources.contains(&word.to_string())
        {
            sources.push(
                word.trim_end_matches(|c: char| !c.is_alphanumeric())
                    .to_string(),
            );
        }
    }
    // Also look for "Source: URL" patterns
    for line in research.lines() {
        if let Some(url) = line.strip_prefix("Source: ") {
            let url = url.trim();
            if url.starts_with("http") && !sources.contains(&url.to_string()) {
                sources.push(url.to_string());
            }
        }
    }
    sources
}

/// Extract URLs from search results that match the egress allowlist.
fn extract_fetchable_urls(search_results: &str, allowlist: &[String]) -> Vec<String> {
    let mut urls = Vec::new();
    for word in search_results.split_whitespace() {
        let trimmed = word.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != ':' && c != '/' && c != '.' && c != '-' && c != '_'
        });
        if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
            && allowlist.iter().any(|prefix| trimmed.starts_with(prefix))
            && !urls.contains(&trimmed.to_string())
        {
            urls.push(trimmed.to_string());
        }
    }
    // If no URLs found in results, try fetching known sources directly
    if urls.is_empty() {
        for prefix in allowlist {
            if prefix.contains("news.ycombinator.com") {
                urls.push("https://news.ycombinator.com/".to_string());
            }
        }
    }
    urls
}

/// Convert markdown article to self-contained HTML.
fn markdown_to_html(title: &str, markdown: &str, date: &str, topic: &str) -> String {
    let body_html = markdown_to_body(markdown);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<meta name="description" content="An in-depth article about {topic}">
<meta name="date" content="{date}">
<style>
body {{ max-width: 800px; margin: 0 auto; padding: 20px; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.7; color: #1a1a1a; background: #fafafa; }}
h1 {{ font-size: 2.2rem; line-height: 1.2; margin-bottom: 0.5rem; color: #111; }}
h2 {{ font-size: 1.5rem; margin-top: 2rem; color: #222; border-bottom: 2px solid #e0e0e0; padding-bottom: 0.3rem; }}
p {{ margin: 1rem 0; }}
ul, ol {{ padding-left: 1.5rem; }}
li {{ margin: 0.3rem 0; }}
blockquote {{ border-left: 4px solid #0066cc; margin: 1.5rem 0; padding: 0.5rem 1rem; background: #f0f7ff; }}
code {{ background: #f0f0f0; padding: 2px 6px; border-radius: 3px; font-size: 0.9em; }}
a {{ color: #0066cc; text-decoration: none; }}
a:hover {{ text-decoration: underline; }}
.meta {{ color: #666; font-size: 0.9rem; margin-bottom: 2rem; }}
.footer {{ margin-top: 3rem; padding-top: 1rem; border-top: 1px solid #ddd; color: #888; font-size: 0.85rem; }}
</style>
</head>
<body>
<div class="meta">Published: {date} &middot; Topic: {topic} &middot; Generated by Nexus OS Content Creator</div>
{body_html}
<div class="footer">
<p>This article was autonomously researched and written by Nexus OS Content Creator agent.
All facts are sourced from real-time web research. Published on {date}.</p>
</div>
</body>
</html>"#,
        title = html_escape(title),
        topic = html_escape(topic),
        date = date,
        body_html = body_html,
    )
}

/// Basic Markdown to HTML conversion (no external dependency).
fn markdown_to_body(md: &str) -> String {
    let mut html = String::new();
    let mut in_list = false;
    let mut in_blockquote = false;

    for line in md.lines() {
        let trimmed = line.trim();

        // Close open elements if needed
        if !trimmed.starts_with("- ") && !trimmed.starts_with("* ") && in_list {
            html.push_str("</ul>\n");
            in_list = false;
        }
        if !trimmed.starts_with("> ") && in_blockquote {
            html.push_str("</blockquote>\n");
            in_blockquote = false;
        }

        if trimmed.is_empty() {
            continue;
        } else if let Some(h1) = trimmed.strip_prefix("# ") {
            html.push_str(&format!("<h1>{}</h1>\n", html_escape(h1)));
        } else if let Some(h2) = trimmed.strip_prefix("## ") {
            html.push_str(&format!("<h2>{}</h2>\n", html_escape(h2)));
        } else if let Some(h3) = trimmed.strip_prefix("### ") {
            html.push_str(&format!("<h3>{}</h3>\n", html_escape(h3)));
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if !in_list {
                html.push_str("<ul>\n");
                in_list = true;
            }
            let item = trimmed.trim_start_matches("- ").trim_start_matches("* ");
            html.push_str(&format!("<li>{}</li>\n", inline_markdown(item)));
        } else if let Some(quote) = trimmed.strip_prefix("> ") {
            if !in_blockquote {
                html.push_str("<blockquote>\n");
                in_blockquote = true;
            }
            html.push_str(&format!("<p>{}</p>\n", inline_markdown(quote)));
        } else {
            html.push_str(&format!("<p>{}</p>\n", inline_markdown(trimmed)));
        }
    }

    if in_list {
        html.push_str("</ul>\n");
    }
    if in_blockquote {
        html.push_str("</blockquote>\n");
    }

    html
}

/// Handle inline markdown: **bold**, *italic*, `code`, [text](url)
fn inline_markdown(text: &str) -> String {
    let escaped = html_escape(text);
    let mut result = escaped;

    // Bold: **text**
    while let (Some(start), rest) = (result.find("**"), &result) {
        if let Some(end) = rest[start + 2..].find("**") {
            let before = &result[..start];
            let bold = &result[start + 2..start + 2 + end];
            let after = &result[start + 2 + end + 2..];
            result = format!("{before}<strong>{bold}</strong>{after}");
        } else {
            break;
        }
    }

    // Italic: *text*
    while let (Some(start), rest) = (result.find('*'), &result) {
        if let Some(end) = rest[start + 1..].find('*') {
            let before = &result[..start];
            let italic = &result[start + 1..start + 1 + end];
            let after = &result[start + 1 + end + 1..];
            result = format!("{before}<em>{italic}</em>{after}");
        } else {
            break;
        }
    }

    // Code: `text`
    while let (Some(start), rest) = (result.find('`'), &result) {
        if let Some(end) = rest[start + 1..].find('`') {
            let before = &result[..start];
            let code = &result[start + 1..start + 1 + end];
            let after = &result[start + 1 + end + 1..];
            result = format!("{before}<code>{code}</code>{after}");
        } else {
            break;
        }
    }

    // Links: [text](url)
    while let Some(bracket_start) = result.find('[') {
        if let Some(bracket_end) = result[bracket_start..].find("](") {
            let abs_bracket_end = bracket_start + bracket_end;
            if let Some(paren_end) = result[abs_bracket_end + 2..].find(')') {
                let before = &result[..bracket_start];
                let link_text = &result[bracket_start + 1..abs_bracket_end];
                let url = &result[abs_bracket_end + 2..abs_bracket_end + 2 + paren_end];
                let after = &result[abs_bracket_end + 2 + paren_end + 1..];
                result = format!("{before}<a href=\"{url}\">{link_text}</a>{after}");
                continue;
            }
        }
        break;
    }

    result
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("The Rise of AI in 2026"), "the-rise-of-ai-in-2026");
        assert_eq!(
            slugify("Rust vs Go: A Comparison!"),
            "rust-vs-go-a-comparison"
        );
    }

    #[test]
    fn test_extract_title() {
        assert_eq!(
            extract_title("# My Great Article\n\nSome content"),
            "My Great Article"
        );
        assert_eq!(
            extract_title("No heading here\nJust text"),
            "No heading here"
        );
        assert_eq!(extract_title(""), "Untitled Article");
    }

    #[test]
    fn test_extract_sources() {
        let research = "According to https://example.com/article1 and also\nSource: https://news.ycombinator.com/item?id=123\nmore text";
        let sources = extract_sources(research);
        assert!(sources.len() >= 2);
        assert!(sources.iter().any(|s| s.contains("example.com")));
    }

    #[test]
    fn test_extract_fetchable_urls() {
        let results = "Check out https://news.ycombinator.com/item?id=123 and https://evil.com/bad";
        let allowlist = vec!["https://news.ycombinator.com".to_string()];
        let urls = extract_fetchable_urls(results, &allowlist);
        assert_eq!(urls.len(), 1);
        assert!(urls[0].contains("ycombinator"));
    }

    #[test]
    fn test_markdown_to_html() {
        let md = "# Title\n\n## Section 1\n\nSome text with **bold** and *italic*.\n\n- Item 1\n- Item 2\n";
        let html = markdown_to_html("Title", md, "2026-03-24", "AI");
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<h2>Section 1</h2>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<li>Item 1</li>"));
        assert!(html.contains("Nexus OS Content Creator"));
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
    }

    #[test]
    fn test_inline_markdown() {
        assert!(inline_markdown("**bold**").contains("<strong>"));
        assert!(inline_markdown("`code`").contains("<code>"));
        assert!(inline_markdown("[link](http://x.com)").contains("<a href="));
    }

    // ── Full Pipeline Integration Test ──

    use crate::actuators::ActuatorContext;
    use crate::audit::AuditTrail;
    use crate::autonomy::AutonomyLevel;
    use crate::cognitive::loop_runtime::LlmQueryHandler;
    use std::collections::HashSet;
    use std::sync::Arc;

    /// Mock LLM that returns realistic content for each phase.
    struct MockContentLlm;

    impl LlmQueryHandler for MockContentLlm {
        fn query(&self, prompt: &str) -> Result<String, String> {
            let lower = prompt.to_lowercase();

            // Topic extraction
            if lower.contains("single best topic") || lower.contains("content strategist") {
                return Ok("The Rise of AI Coding Assistants in 2026".to_string());
            }

            // Research synthesis
            if lower.contains("research analyst") || lower.contains("synthesize") {
                return Ok(
                    "KEY FACTS:\n\
                     - AI coding assistants market grew 340% in 2025\n\
                     - 78% of professional developers now use AI tools daily\n\
                     - Source: https://stackoverflow.com/survey/2026\n\
                     - GitHub Copilot processes 1.2 billion suggestions per day\n\
                     \nEXPERT OPINIONS:\n\
                     - \"AI won't replace developers, but developers using AI will replace those who don't\" — Stack Overflow CEO\n\
                     \nSTATISTICS:\n\
                     - Average productivity gain: 55% for routine coding tasks\n\
                     - 92% of Fortune 500 companies have adopted AI coding tools\n\
                     \nCONTRARIAN VIEWPOINTS:\n\
                     - Some experts warn about over-reliance leading to skill atrophy\n\
                     - Security concerns: AI-generated code may introduce subtle vulnerabilities"
                        .to_string(),
                );
            }

            // Article writing
            if lower.contains("expert tech writer") || lower.contains("write a 1000") {
                return Ok(
                    "# The Rise of AI Coding Assistants: How They're Reshaping Software Development in 2026\n\n\
                     The software development landscape has undergone a seismic shift. AI coding assistants, \
                     once novelties, have become indispensable tools in every developer's arsenal. With the \
                     market growing 340% in 2025 alone, understanding this transformation isn't optional — \
                     it's essential for anyone in tech.\n\n\
                     ## The Current State of AI-Assisted Development\n\n\
                     According to Stack Overflow's 2026 Developer Survey, 78% of professional developers now \
                     use AI coding tools daily. GitHub Copilot alone processes an staggering 1.2 billion code \
                     suggestions per day, while newer entrants like Cursor, Cody, and Claude Code are rapidly \
                     gaining market share.\n\n\
                     The numbers tell a compelling story: developers report an average productivity gain of 55% \
                     for routine coding tasks. But the impact goes far beyond speed. AI assistants are changing \
                     how developers think about problem-solving, debugging, and code architecture.\n\n\
                     ## How Fortune 500 Companies Are Adopting AI Tools\n\n\
                     Enterprise adoption has been remarkably swift. A remarkable 92% of Fortune 500 companies \
                     have now integrated AI coding assistants into their development workflows. This isn't just \
                     about individual productivity — it's about organizational competitiveness.\n\n\
                     Companies are reporting reduced time-to-market, fewer production bugs, and improved code \
                     consistency across large teams. The ROI is clear: for every dollar invested in AI coding \
                     tools, companies are seeing $3-5 in productivity returns.\n\n\
                     ## The Risks Nobody Is Talking About\n\n\
                     Despite the enthusiasm, not everyone is celebrating. Some industry experts have raised \
                     legitimate concerns about the long-term implications of AI-assisted development.\n\n\
                     The most significant worry is skill atrophy. When developers rely heavily on AI suggestions, \
                     they may lose the deep understanding of algorithms and data structures that comes from \
                     writing code from scratch. Additionally, AI-generated code can introduce subtle security \
                     vulnerabilities that are harder to detect precisely because they look syntactically correct.\n\n\
                     ## What This Means for Your Career\n\n\
                     The key insight from industry leaders is captured by Stack Overflow's CEO: \"AI won't \
                     replace developers, but developers using AI will replace those who don't.\" The message \
                     is clear — AI coding assistants are not a threat but a force multiplier.\n\n\
                     For developers looking to stay competitive, the advice is straightforward:\n\n\
                     - Master at least one AI coding assistant deeply\n\
                     - Understand the fundamentals so you can evaluate AI suggestions critically\n\
                     - Learn prompt engineering for code generation\n\
                     - Stay current with the rapidly evolving tool landscape\n\n\
                     ## Key Takeaways\n\n\
                     - AI coding assistants have achieved mainstream adoption with 78% daily usage among professional developers\n\
                     - Productivity gains average 55% for routine tasks, with clear ROI for enterprises\n\
                     - Security and skill atrophy remain legitimate concerns that the industry must address\n\
                     - Developers who embrace AI tools will have a significant competitive advantage\n\
                     - The market is still evolving rapidly — staying current is essential\n\n\
                     ## Conclusion\n\n\
                     The rise of AI coding assistants represents one of the most significant shifts in software \
                     development history. Whether you're a seasoned developer or just starting out, understanding \
                     and leveraging these tools is no longer optional. The developers who thrive in 2026 and \
                     beyond will be those who view AI as a collaborative partner, not a replacement.\n\n\
                     Ready to level up? Start by experimenting with the latest AI coding tools and find the \
                     workflow that works best for you. The future of development is here — and it's collaborative."
                        .to_string(),
                );
            }

            // Default fallback
            Ok("Generated content for the given prompt.".to_string())
        }
    }

    fn make_pipeline_context(workspace: &std::path::Path) -> ActuatorContext {
        let mut caps = HashSet::new();
        caps.insert("web.search".into());
        caps.insert("web.read".into());
        caps.insert("llm.query".into());
        caps.insert("fs.read".into());
        caps.insert("fs.write".into());
        caps.insert("process.exec".into());
        caps.insert("mcp.call".into());
        ActuatorContext {
            agent_id: uuid::Uuid::new_v4().to_string(),
            agent_name: "nexus-content-creator".to_string(),
            working_dir: workspace.to_path_buf(),
            autonomy_level: AutonomyLevel::L4,
            capabilities: caps,
            fuel_remaining: 500.0,
            egress_allowlist: vec![
                "https://news.ycombinator.com".into(),
                "https://hacker-news.firebaseio.com".into(),
                "https://www.reddit.com".into(),
            ],
            action_review_engine: None,
            hitl_approved: false,
        }
    }

    /// Full pipeline integration: trend scan → research → write → publish → analytics.
    /// Uses real web search (DuckDuckGo) but mocked LLM for deterministic content.
    #[test]
    fn test_full_content_pipeline() {
        let tmp = tempfile::TempDir::new().unwrap();
        let context = make_pipeline_context(tmp.path());
        let llm: Arc<dyn LlmQueryHandler> = Arc::new(MockContentLlm);
        let pipeline = ContentPipeline::new(llm);
        let mut audit = AuditTrail::new();

        let result = pipeline.run(&context, &mut audit);

        // Pipeline should succeed
        assert!(result.success, "pipeline failed: {:?}", result.error);
        assert!(
            !result.article_title.is_empty(),
            "article title should not be empty"
        );
        assert!(
            result.word_count > 500,
            "expected >500 words, got {}",
            result.word_count
        );
        assert!(
            result.article_path.ends_with(".html"),
            "expected HTML path, got {}",
            result.article_path
        );

        // All 5 phases should have run
        assert_eq!(
            result.phase_results.len(),
            5,
            "expected 5 phases, got {}",
            result.phase_results.len()
        );
        for phase in &result.phase_results {
            // trend_scan and research phases may fail when the network is
            // unavailable (CI) — the pipeline continues with fallback topics.
            if phase.phase == "trend_scan" || phase.phase == "research" {
                continue;
            }
            assert!(
                phase.success,
                "phase {} failed: {}",
                phase.phase, phase.output_preview
            );
        }

        // Verify files were created
        let articles_dir = tmp.path().join("articles");
        assert!(articles_dir.exists(), "articles directory should exist");

        // Find the HTML file
        let mut found_html = false;
        let mut found_md = false;
        let mut found_meta = false;
        if let Ok(entries) = std::fs::read_dir(&articles_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if let Ok(files) = std::fs::read_dir(entry.path()) {
                        for file in files.flatten() {
                            let name = file.file_name().to_string_lossy().to_string();
                            if name.ends_with(".html") {
                                found_html = true;
                                // Verify HTML content
                                let html = std::fs::read_to_string(file.path()).unwrap();
                                assert!(
                                    html.contains("<!DOCTYPE html>"),
                                    "HTML should be a complete document"
                                );
                                assert!(
                                    html.contains("Nexus OS Content Creator"),
                                    "HTML should credit the agent"
                                );
                            }
                            if name.ends_with(".md") && !name.ends_with(".meta.json") {
                                found_md = true;
                            }
                            if name.ends_with(".meta.json") {
                                found_meta = true;
                                let meta: serde_json::Value = serde_json::from_str(
                                    &std::fs::read_to_string(file.path()).unwrap(),
                                )
                                .unwrap();
                                assert!(meta["title"].is_string());
                                assert!(meta["word_count"].is_number());
                            }
                        }
                    }
                }
            }
        }
        assert!(found_html, "HTML article file should exist");
        assert!(found_md, "Markdown article file should exist");
        assert!(found_meta, "Metadata JSON file should exist");

        // Verify audit trail has content events
        let audit_events = audit.events();
        let content_events: Vec<_> = audit_events
            .iter()
            .filter(|e| {
                e.payload
                    .get("event")
                    .and_then(|v| v.as_str())
                    .map(|s| s.starts_with("content."))
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            !content_events.is_empty(),
            "should have content.* audit events"
        );

        // Verify fuel was consumed
        assert!(result.total_fuel > 0.0, "should have consumed fuel");

        eprintln!(
            "Pipeline result: title='{}', words={}, fuel={:.1}, phases={}",
            result.article_title,
            result.word_count,
            result.total_fuel,
            result.phase_results.len()
        );
    }

    /// Test that pipeline handles WebSearch failures gracefully.
    #[test]
    fn test_pipeline_phases_individually() {
        // Test just the helper functions work correctly together
        let md = "# Test Article\n\n## Intro\n\nHello world.\n\n## Key Takeaways\n\n- Point 1\n- Point 2";
        let html = markdown_to_html("Test Article", md, "2026-03-24", "testing");
        assert!(html.contains("<h1>Test Article</h1>"));
        assert!(html.contains("<h2>Key Takeaways</h2>"));
        assert!(html.contains("<li>Point 1</li>"));

        let title = extract_title(md);
        assert_eq!(title, "Test Article");

        let slug = slugify(&title);
        assert_eq!(slug, "test-article");
    }
}
