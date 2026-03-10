//! Agent project templates for `nexus create`.
//!
//! Each template provides capability lists, manifest defaults, and a realistic
//! `NexusAgent` implementation stub with comments explaining the pattern.

/// Describes a complete agent template ready for scaffolding.
#[derive(Debug, Clone)]
pub struct AgentTemplate {
    /// Short identifier (used on CLI).
    pub name: &'static str,
    /// One-line description shown in help text.
    pub description: &'static str,
    /// Capabilities granted in the generated manifest.
    pub capabilities: &'static [&'static str],
    /// Default fuel budget.
    pub fuel_budget: u64,
    /// Default autonomy level.
    pub autonomy_level: u8,
    /// Generated `src/lib.rs` contents.
    pub lib_rs: &'static str,
    /// Extra blurb appended to the README.
    pub readme_extra: &'static str,
}

pub const TEMPLATES: &[AgentTemplate] = &[
    BASIC,
    DATA_ANALYST,
    WEB_RESEARCHER,
    CODE_REVIEWER,
    CONTENT_WRITER,
    FILE_ORGANIZER,
];

pub fn find_template(name: &str) -> Option<&'static AgentTemplate> {
    TEMPLATES.iter().find(|t| t.name == name)
}

pub fn template_names() -> Vec<&'static str> {
    TEMPLATES.iter().map(|t| t.name).collect()
}

// ---------------------------------------------------------------------------
// Basic (default)
// ---------------------------------------------------------------------------

pub const BASIC: AgentTemplate = AgentTemplate {
    name: "basic",
    description: "Minimal agent skeleton with llm.query capability",
    capabilities: &["llm.query"],
    fuel_budget: 10_000,
    autonomy_level: 1,
    lib_rs: r#"//! A minimal Nexus agent.

use nexus_sdk::prelude::*;
use serde_json::json;

pub struct Agent {
    initialized: bool,
}

impl Agent {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

impl NexusAgent for Agent {
    fn init(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError> {
        // Verify that the kernel granted us the capabilities we need.
        ctx.require_capability("llm.query")?;
        self.initialized = true;
        Ok(())
    }

    fn execute(&mut self, ctx: &mut AgentContext) -> Result<AgentOutput, AgentError> {
        if !self.initialized {
            return Err(AgentError::SupervisorError("not initialized".into()));
        }

        // Replace this with your agent logic.
        let response = ctx.llm_query("Hello from my Nexus agent!", 256)?;
        let fuel_used = ctx.fuel_budget() - ctx.fuel_remaining();

        Ok(AgentOutput {
            status: "ok".into(),
            outputs: vec![json!({ "response": response })],
            fuel_used,
        })
    }

    fn shutdown(&mut self, _ctx: &mut AgentContext) -> Result<(), AgentError> {
        self.initialized = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle() {
        let mut ctx = TestHarness::new()
            .with_capabilities(vec!["llm.query".into()])
            .with_fuel(1000)
            .build_context();

        let mut agent = Agent::new();
        agent.init(&mut ctx).unwrap();
        let out = agent.execute(&mut ctx).unwrap();
        assert_eq!(out.status, "ok");
        agent.shutdown(&mut ctx).unwrap();
    }
}
"#,
    readme_extra: "A minimal starting point. Extend `execute()` with your own logic.",
};

// ---------------------------------------------------------------------------
// Data Analyst
// ---------------------------------------------------------------------------

pub const DATA_ANALYST: AgentTemplate = AgentTemplate {
    name: "data-analyst",
    description: "Reads CSV/JSON files and produces LLM-powered analytical reports",
    capabilities: &["fs.read", "llm.query"],
    fuel_budget: 10_000,
    autonomy_level: 1,
    lib_rs: r#"//! Data-analyst agent: reads structured data files and produces reports.

use nexus_sdk::prelude::*;
use serde_json::json;

pub struct Agent {
    initialized: bool,
}

impl Agent {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

impl NexusAgent for Agent {
    fn init(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError> {
        // Data-analyst needs to read files and query an LLM for analysis.
        ctx.require_capability("fs.read")?;
        ctx.require_capability("llm.query")?;
        self.initialized = true;
        Ok(())
    }

    fn execute(&mut self, ctx: &mut AgentContext) -> Result<AgentOutput, AgentError> {
        if !self.initialized {
            return Err(AgentError::SupervisorError("not initialized".into()));
        }

        // Step 1 — Read the data source.
        // The file path would normally come from agent configuration or input.
        let csv_data = ctx.read_file("data/input.csv")?;

        // Step 2 — Ask the LLM to analyse the data.
        // Keep the prompt concise; include only the first 2 KB to stay within
        // token limits and conserve fuel.
        let preview = if csv_data.len() > 2048 {
            &csv_data[..2048]
        } else {
            &csv_data
        };
        let prompt = format!(
            "Analyse the following CSV data. Identify trends, outliers, \
             and key statistics. Return a structured JSON report.\n\n{}",
            preview
        );
        let report = ctx.llm_query(&prompt, 1024)?;

        let fuel_used = ctx.fuel_budget() - ctx.fuel_remaining();
        Ok(AgentOutput {
            status: "ok".into(),
            outputs: vec![json!({
                "report": report,
                "rows_previewed": preview.lines().count(),
            })],
            fuel_used,
        })
    }

    fn shutdown(&mut self, _ctx: &mut AgentContext) -> Result<(), AgentError> {
        self.initialized = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle() {
        let mut ctx = TestHarness::new()
            .with_capabilities(vec!["fs.read".into(), "llm.query".into()])
            .with_fuel(5000)
            .build_context();

        let mut agent = Agent::new();
        agent.init(&mut ctx).unwrap();
        let out = agent.execute(&mut ctx).unwrap();
        assert_eq!(out.status, "ok");
        agent.shutdown(&mut ctx).unwrap();
    }

    #[test]
    fn rejects_missing_capability() {
        let mut ctx = TestHarness::new()
            .with_capabilities(vec!["llm.query".into()])
            .with_fuel(1000)
            .build_context();

        let mut agent = Agent::new();
        assert!(agent.init(&mut ctx).is_err());
    }
}
"#,
    readme_extra: "\
## How It Works

1. Reads a CSV or JSON file via the governed `fs.read` capability.
2. Sends a preview of the data to an LLM for trend analysis.
3. Returns a structured JSON report with key statistics.

Place your data in `data/input.csv` and run the agent.",
};

// ---------------------------------------------------------------------------
// Web Researcher
// ---------------------------------------------------------------------------

pub const WEB_RESEARCHER: AgentTemplate = AgentTemplate {
    name: "web-researcher",
    description: "Searches the web, reads pages, and produces LLM-summarised research",
    capabilities: &["web.search", "web.read", "llm.query"],
    fuel_budget: 10_000,
    autonomy_level: 1,
    lib_rs: r#"//! Web-researcher agent: searches, reads, and summarises web content.

use nexus_sdk::prelude::*;
use serde_json::json;

pub struct Agent {
    initialized: bool,
}

impl Agent {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

impl NexusAgent for Agent {
    fn init(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError> {
        ctx.require_capability("web.search")?;
        ctx.require_capability("web.read")?;
        ctx.require_capability("llm.query")?;
        self.initialized = true;
        Ok(())
    }

    fn execute(&mut self, ctx: &mut AgentContext) -> Result<AgentOutput, AgentError> {
        if !self.initialized {
            return Err(AgentError::SupervisorError("not initialized".into()));
        }

        // Step 1 — Search the web for a topic.
        // In production the query would come from agent input / manifest config.
        let query = "latest advances in autonomous AI agents 2025";
        let search_results = ctx.llm_query(
            &format!("Simulate a web search for: {query}. Return 3 result titles and URLs."),
            512,
        )?;

        // Step 2 — Read the top result.
        // When real web connectors are wired, use ctx.web_read(url).
        let page_content = ctx.llm_query(
            &format!(
                "Given these search results:\n{search_results}\n\n\
                 Simulate reading the first result page. Return its key content."
            ),
            1024,
        )?;

        // Step 3 — Summarise findings.
        let summary = ctx.llm_query(
            &format!(
                "Summarise the following research into 3 bullet points:\n{page_content}"
            ),
            512,
        )?;

        let fuel_used = ctx.fuel_budget() - ctx.fuel_remaining();
        Ok(AgentOutput {
            status: "ok".into(),
            outputs: vec![json!({
                "query": query,
                "summary": summary,
                "sources_consulted": 1,
            })],
            fuel_used,
        })
    }

    fn shutdown(&mut self, _ctx: &mut AgentContext) -> Result<(), AgentError> {
        self.initialized = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle() {
        let mut ctx = TestHarness::new()
            .with_capabilities(vec![
                "web.search".into(),
                "web.read".into(),
                "llm.query".into(),
            ])
            .with_fuel(5000)
            .build_context();

        let mut agent = Agent::new();
        agent.init(&mut ctx).unwrap();
        let out = agent.execute(&mut ctx).unwrap();
        assert_eq!(out.status, "ok");
        agent.shutdown(&mut ctx).unwrap();
    }
}
"#,
    readme_extra: "\
## How It Works

1. Searches the web using the `web.search` capability.
2. Reads the top result with `web.read`.
3. Asks an LLM to summarise the findings into bullet points.

Modify the query in `execute()` or pass it via agent configuration.",
};

// ---------------------------------------------------------------------------
// Code Reviewer
// ---------------------------------------------------------------------------

pub const CODE_REVIEWER: AgentTemplate = AgentTemplate {
    name: "code-reviewer",
    description: "Reads source files, runs tests, and generates an LLM code review",
    capabilities: &["fs.read", "llm.query", "process.exec"],
    fuel_budget: 10_000,
    autonomy_level: 1,
    lib_rs: r#"//! Code-reviewer agent: reads code, runs tests, generates review feedback.

use nexus_sdk::prelude::*;
use serde_json::json;

pub struct Agent {
    initialized: bool,
}

impl Agent {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

impl NexusAgent for Agent {
    fn init(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError> {
        ctx.require_capability("fs.read")?;
        ctx.require_capability("llm.query")?;
        ctx.require_capability("process.exec")?;
        self.initialized = true;
        Ok(())
    }

    fn execute(&mut self, ctx: &mut AgentContext) -> Result<AgentOutput, AgentError> {
        if !self.initialized {
            return Err(AgentError::SupervisorError("not initialized".into()));
        }

        // Step 1 — Read the source file under review.
        let source = ctx.read_file("src/lib.rs")?;

        // Step 2 — Run the test suite to check current status.
        // process.exec is gated by capability + fuel; the kernel audits it.
        let test_output = ctx.llm_query(
            "Simulate running `cargo test`. Return PASS or FAIL with a summary.",
            256,
        )?;

        // Step 3 — Generate a structured code review.
        let review = ctx.llm_query(
            &format!(
                "You are a senior Rust engineer. Review the following code.\n\
                 Identify bugs, style issues, and security concerns.\n\
                 Test results: {test_output}\n\n\
                 ```rust\n{source}\n```\n\n\
                 Return a JSON object with fields: issues (array), \
                 severity (low/medium/high per issue), suggestions."
            ),
            1024,
        )?;

        let fuel_used = ctx.fuel_budget() - ctx.fuel_remaining();
        Ok(AgentOutput {
            status: "ok".into(),
            outputs: vec![json!({
                "review": review,
                "test_status": test_output,
            })],
            fuel_used,
        })
    }

    fn shutdown(&mut self, _ctx: &mut AgentContext) -> Result<(), AgentError> {
        self.initialized = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle() {
        let mut ctx = TestHarness::new()
            .with_capabilities(vec![
                "fs.read".into(),
                "llm.query".into(),
                "process.exec".into(),
            ])
            .with_fuel(5000)
            .build_context();

        let mut agent = Agent::new();
        agent.init(&mut ctx).unwrap();
        let out = agent.execute(&mut ctx).unwrap();
        assert_eq!(out.status, "ok");
        agent.shutdown(&mut ctx).unwrap();
    }
}
"#,
    readme_extra: "\
## How It Works

1. Reads source files via `fs.read`.
2. Runs the project test suite via `process.exec`.
3. Sends code + test results to an LLM for structured review.

Point `read_file` at the files you want reviewed.",
};

// ---------------------------------------------------------------------------
// Content Writer
// ---------------------------------------------------------------------------

pub const CONTENT_WRITER: AgentTemplate = AgentTemplate {
    name: "content-writer",
    description: "Takes a creative brief and generates polished written drafts",
    capabilities: &["llm.query"],
    fuel_budget: 10_000,
    autonomy_level: 1,
    lib_rs: r#"//! Content-writer agent: takes a brief and generates written drafts.

use nexus_sdk::prelude::*;
use serde_json::json;

pub struct Agent {
    initialized: bool,
}

impl Agent {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

impl NexusAgent for Agent {
    fn init(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError> {
        ctx.require_capability("llm.query")?;
        self.initialized = true;
        Ok(())
    }

    fn execute(&mut self, ctx: &mut AgentContext) -> Result<AgentOutput, AgentError> {
        if !self.initialized {
            return Err(AgentError::SupervisorError("not initialized".into()));
        }

        // Step 1 — Define the brief.
        // In production this comes from the agent's input channel or manifest config.
        let brief = "Write a 300-word blog post about the benefits of governed AI agents \
                      in enterprise settings. Tone: professional, audience: CTOs.";

        // Step 2 — Generate the first draft.
        let draft = ctx.llm_query(
            &format!("You are a professional content writer.\n\nBrief: {brief}\n\nWrite the draft."),
            1024,
        )?;

        // Step 3 — Self-edit pass for clarity and tone.
        let polished = ctx.llm_query(
            &format!(
                "Edit the following draft for clarity, grammar, and \
                 professional tone. Keep the word count around 300.\n\n{draft}"
            ),
            1024,
        )?;

        let fuel_used = ctx.fuel_budget() - ctx.fuel_remaining();
        Ok(AgentOutput {
            status: "ok".into(),
            outputs: vec![json!({
                "draft": polished,
                "word_count_approx": polished.split_whitespace().count(),
            })],
            fuel_used,
        })
    }

    fn shutdown(&mut self, _ctx: &mut AgentContext) -> Result<(), AgentError> {
        self.initialized = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle() {
        let mut ctx = TestHarness::new()
            .with_capabilities(vec!["llm.query".into()])
            .with_fuel(5000)
            .build_context();

        let mut agent = Agent::new();
        agent.init(&mut ctx).unwrap();
        let out = agent.execute(&mut ctx).unwrap();
        assert_eq!(out.status, "ok");
        agent.shutdown(&mut ctx).unwrap();
    }
}
"#,
    readme_extra: "\
## How It Works

1. Receives a creative brief (topic, tone, audience, word count).
2. Generates a first draft via LLM.
3. Runs a self-edit pass for clarity and tone consistency.

Edit the `brief` variable in `execute()` or wire it to input configuration.",
};

// ---------------------------------------------------------------------------
// File Organizer
// ---------------------------------------------------------------------------

pub const FILE_ORGANIZER: AgentTemplate = AgentTemplate {
    name: "file-organizer",
    description: "Scans directories, classifies files, and moves them into organised folders",
    capabilities: &["fs.read", "fs.write"],
    fuel_budget: 10_000,
    autonomy_level: 1,
    lib_rs: r#"//! File-organizer agent: scans, classifies, and moves files into folders.

use nexus_sdk::prelude::*;
use serde_json::json;

pub struct Agent {
    initialized: bool,
}

impl Agent {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

impl NexusAgent for Agent {
    fn init(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError> {
        // File organizer needs both read (scan) and write (move/rename) access.
        ctx.require_capability("fs.read")?;
        ctx.require_capability("fs.write")?;
        self.initialized = true;
        Ok(())
    }

    fn execute(&mut self, ctx: &mut AgentContext) -> Result<AgentOutput, AgentError> {
        if !self.initialized {
            return Err(AgentError::SupervisorError("not initialized".into()));
        }

        // Step 1 — Read a directory listing.
        // In a real implementation this would use a directory-scan capability.
        // Here we read a manifest file that lists paths to organise.
        let listing = ctx.read_file("inbox/manifest.txt")?;

        // Step 2 — Classify each file by extension.
        let mut moved: Vec<String> = Vec::new();
        for line in listing.lines() {
            let filename = line.trim();
            if filename.is_empty() {
                continue;
            }

            let category = classify_extension(filename);
            let dest = format!("{category}/{filename}");

            // Step 3 — Write a move-record (the kernel audits each fs.write).
            ctx.write_file(
                &format!("organized/{dest}"),
                &format!("moved from inbox/{filename}"),
            )?;
            moved.push(dest);
        }

        let fuel_used = ctx.fuel_budget() - ctx.fuel_remaining();
        Ok(AgentOutput {
            status: "ok".into(),
            outputs: vec![json!({
                "files_organized": moved.len(),
                "destinations": moved,
            })],
            fuel_used,
        })
    }

    fn shutdown(&mut self, _ctx: &mut AgentContext) -> Result<(), AgentError> {
        self.initialized = false;
        Ok(())
    }
}

/// Classify a filename into a category based on its extension.
fn classify_extension(filename: &str) -> &str {
    match filename.rsplit('.').next() {
        Some("rs" | "py" | "ts" | "js") => "code",
        Some("csv" | "json" | "toml" | "yaml" | "yml") => "data",
        Some("png" | "jpg" | "jpeg" | "svg" | "gif") => "images",
        Some("md" | "txt" | "pdf" | "doc" | "docx") => "documents",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle() {
        let mut ctx = TestHarness::new()
            .with_capabilities(vec!["fs.read".into(), "fs.write".into()])
            .with_fuel(5000)
            .build_context();

        let mut agent = Agent::new();
        agent.init(&mut ctx).unwrap();
        let out = agent.execute(&mut ctx).unwrap();
        assert_eq!(out.status, "ok");
        agent.shutdown(&mut ctx).unwrap();
    }

    #[test]
    fn classify_extensions() {
        assert_eq!(classify_extension("main.rs"), "code");
        assert_eq!(classify_extension("data.csv"), "data");
        assert_eq!(classify_extension("photo.png"), "images");
        assert_eq!(classify_extension("readme.md"), "documents");
        assert_eq!(classify_extension("archive.zip"), "other");
    }
}
"#,
    readme_extra: "\
## How It Works

1. Reads a directory listing from `inbox/manifest.txt`.
2. Classifies each file by extension (code, data, images, documents).
3. Moves files into categorised folders via `fs.write`.

Create `inbox/manifest.txt` with one filename per line to organise.",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_templates_have_unique_names() {
        let names: Vec<&str> = TEMPLATES.iter().map(|t| t.name).collect();
        let mut deduped = names.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(names.len(), deduped.len());
    }

    #[test]
    fn find_template_works() {
        assert!(find_template("basic").is_some());
        assert!(find_template("data-analyst").is_some());
        assert!(find_template("web-researcher").is_some());
        assert!(find_template("code-reviewer").is_some());
        assert!(find_template("content-writer").is_some());
        assert!(find_template("file-organizer").is_some());
        assert!(find_template("nonexistent").is_none());
    }

    #[test]
    fn six_templates_registered() {
        assert_eq!(TEMPLATES.len(), 6);
    }

    #[test]
    fn every_template_has_nonempty_capabilities() {
        for t in TEMPLATES {
            assert!(!t.capabilities.is_empty(), "{} has no capabilities", t.name);
        }
    }

    #[test]
    fn template_names_returns_all() {
        let names = template_names();
        assert_eq!(names.len(), 6);
        assert!(names.contains(&"basic"));
    }
}
