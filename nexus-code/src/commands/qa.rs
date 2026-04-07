//! /qa — Autonomous QA workflow for Nexus OS using computer use tools.
//!
//! Navigates to each page, screenshots, analyzes for issues, fixes, and verifies.

/// Execute the /qa command.
/// `/qa` or `/qa all` — QA all pages.
/// `/qa report` — Show QA status.
/// `/qa page <name>` — QA a single page.
pub fn execute(args: &str, computer_use_active: bool) -> super::CommandResult {
    if !computer_use_active {
        return super::CommandResult::Error(
            "Computer use is not active. Enable with --computer-use flag or /computer-use on"
                .to_string(),
        );
    }

    let args = args.trim();

    if args.is_empty() || args == "all" {
        super::CommandResult::AgentPrompt(
            "Navigate to each page of Nexus OS, screenshot it, analyze for bugs and UI issues, \
             fix them, and verify. Start with the Dashboard/home page. After each page, navigate \
             to the next one in the sidebar. Continue until all pages are done. \
             For each page: 1) screenshot, 2) analyze with vision, 3) fix issues found, \
             4) rebuild and screenshot again to verify, 5) move to next page."
                .to_string(),
        )
    } else if args == "report" {
        super::CommandResult::AgentPrompt(
            "Summarize the current QA session: how many pages have been checked, \
             how many issues found, how many fixed, and what remains. \
             Use screen_capture to show the current state."
                .to_string(),
        )
    } else if let Some(page_name) = args.strip_prefix("page ") {
        let page_name = page_name.trim();
        if page_name.is_empty() {
            return super::CommandResult::Error("Usage: /qa page <name>".to_string());
        }
        super::CommandResult::AgentPrompt(format!(
            "Navigate to the {} page in Nexus OS, screenshot it, analyze for all bugs \
             and UI issues, fix them, and verify the fixes with another screenshot.",
            page_name
        ))
    } else {
        super::CommandResult::Error("Usage: /qa [all|report|page <name>]".to_string())
    }
}
