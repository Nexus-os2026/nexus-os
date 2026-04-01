//! /rollback [n] — Rollback the last N commits via the agent loop.

pub fn execute(args: &str) -> super::CommandResult {
    let n = args.parse::<u32>().unwrap_or(1);
    super::CommandResult::AgentPrompt(format!(
        "Rollback the last {} commit(s) using `git reset --soft HEAD~{}`. \
         Show what was undone.",
        n, n
    ))
}
