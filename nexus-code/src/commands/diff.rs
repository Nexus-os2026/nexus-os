//! /diff [args] — Show uncommitted changes using git diff.

/// Execute the /diff command (read-only, runs directly).
pub fn execute(args: &str) -> super::CommandResult {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("diff");
    if !args.is_empty() {
        for arg in args.split_whitespace() {
            cmd.arg(arg);
        }
    }

    match cmd.output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            if !out.status.success() {
                super::CommandResult::Error(format!(
                    "git diff failed (exit {}): {}",
                    out.status.code().unwrap_or(-1),
                    stderr
                ))
            } else if stdout.is_empty() {
                super::CommandResult::Output("No uncommitted changes.".to_string())
            } else {
                super::CommandResult::Output(stdout.to_string())
            }
        }
        Err(e) => super::CommandResult::Error(format!("git diff failed: {}", e)),
    }
}
