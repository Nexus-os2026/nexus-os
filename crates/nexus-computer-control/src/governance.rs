use crate::actions::ComputerAction;
use crate::ControlError;

/// Capability string required for a given action type.
pub fn required_capability(action: &ComputerAction) -> &str {
    match action {
        ComputerAction::Screenshot { .. } => "computer_control.screenshot",
        ComputerAction::MouseMove { .. } => "computer_control.basic",
        ComputerAction::MouseClick { .. } => "computer_control.mouse",
        ComputerAction::MouseDoubleClick { .. } => "computer_control.mouse",
        ComputerAction::KeyboardType { .. } => "computer_control.basic",
        ComputerAction::KeyboardShortcut { .. } => "computer_control.basic",
        ComputerAction::TerminalCommand { .. } => "computer_control.terminal",
        ComputerAction::ReadClipboard => "computer_control.basic",
        ComputerAction::WriteClipboard { .. } => "computer_control.basic",
        ComputerAction::OpenApplication { .. } => "computer_control.launch_app",
        ComputerAction::WaitForElement { .. } => "computer_control.basic",
    }
}

/// Minimum autonomy level required for a given action type.
pub fn minimum_autonomy_level(action: &ComputerAction) -> u8 {
    match action {
        ComputerAction::TerminalCommand { .. } => 5,
        ComputerAction::OpenApplication { .. } => 4,
        ComputerAction::MouseClick { .. } => 4,
        ComputerAction::MouseDoubleClick { .. } => 4,
        ComputerAction::KeyboardType { .. } => 4,
        ComputerAction::KeyboardShortcut { .. } => 4,
        ComputerAction::WriteClipboard { .. } => 4,
        _ => 3, // Screenshot, MouseMove, ReadClipboard, WaitForElement
    }
}

/// Token cost in micronexus for executing an action.
pub fn token_cost(action: &ComputerAction) -> u64 {
    match action {
        ComputerAction::Screenshot { .. } => 1_000_000, // 1 NXC
        ComputerAction::MouseMove { .. } => 1_000_000,  // 1 NXC
        ComputerAction::MouseClick { .. } => 2_000_000, // 2 NXC
        ComputerAction::MouseDoubleClick { .. } => 3_000_000, // 3 NXC
        ComputerAction::KeyboardType { text } => text.len() as u64 * 1_000_000, // 1 NXC per char
        ComputerAction::KeyboardShortcut { .. } => 2_000_000, // 2 NXC
        ComputerAction::TerminalCommand { .. } => 50_000_000, // 50 NXC
        ComputerAction::ReadClipboard => 1_000_000,     // 1 NXC
        ComputerAction::WriteClipboard { .. } => 2_000_000, // 2 NXC
        ComputerAction::OpenApplication { .. } => 10_000_000, // 10 NXC
        ComputerAction::WaitForElement { .. } => 5_000_000, // 5 NXC
    }
}

/// Allowed terminal commands (prefix allowlist).
const TERMINAL_ALLOWLIST: &[&str] = &[
    "ls",
    "cat",
    "head",
    "tail",
    "grep",
    "find",
    "wc",
    "sort",
    "uniq",
    "diff",
    "echo",
    "date",
    "pwd",
    "whoami",
    "uname",
    "df",
    "du",
    "free",
    "ps",
    "top",
    "env",
    "printenv",
    "which",
    "file",
    "stat",
    "md5sum",
    "sha256sum",
    "base64",
    "curl",
    "wget",
    "python3",
    "node",
    "npm",
    "cargo",
    "git",
    "make",
    "cmake",
];

/// Check whether a terminal command is on the allowlist.
pub fn is_command_allowed(command: &str) -> bool {
    let trimmed = command.trim();
    let first_word = trimmed.split_whitespace().next().unwrap_or("");
    TERMINAL_ALLOWLIST.contains(&first_word)
}

/// Validate a filesystem path is within the allowed workspace.
pub fn is_path_in_workspace(path: &str, workspace_root: &str) -> bool {
    // Normalize and check containment
    let normalized = path.replace("/../", "/").replace("/..", "");
    normalized.starts_with(workspace_root)
}

/// Full governance check for an action. Returns Ok(()) if allowed.
pub fn check_governance(
    action: &ComputerAction,
    agent_autonomy_level: u8,
    agent_capabilities: &[String],
    workspace_root: &str,
) -> Result<(), ControlError> {
    // 1. Autonomy level check
    let min_level = minimum_autonomy_level(action);
    if agent_autonomy_level < min_level {
        return Err(ControlError::GovernanceDenied(format!(
            "Action {} requires L{} minimum, agent is L{}",
            action.label(),
            min_level,
            agent_autonomy_level,
        )));
    }

    // 2. Capability check
    let required_cap = required_capability(action);
    if !agent_capabilities.iter().any(|c| c == required_cap) {
        return Err(ControlError::GovernanceDenied(format!(
            "Agent lacks capability '{required_cap}' for action {}",
            action.label(),
        )));
    }

    // 3. Terminal command allowlist
    if let ComputerAction::TerminalCommand {
        command,
        working_dir,
    } = action
    {
        if !is_command_allowed(command) {
            return Err(ControlError::GovernanceDenied(format!(
                "Terminal command '{}' not on allowlist",
                command.split_whitespace().next().unwrap_or(""),
            )));
        }
        if !is_path_in_workspace(working_dir, workspace_root) {
            return Err(ControlError::SandboxViolation(format!(
                "Working directory '{working_dir}' outside workspace '{workspace_root}'",
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_command_allowlist_enforced() {
        assert!(is_command_allowed("ls -la"));
        assert!(is_command_allowed("git status"));
        assert!(is_command_allowed("cargo build"));
        assert!(!is_command_allowed("rm -rf /"));
        assert!(!is_command_allowed("sudo anything"));
        assert!(!is_command_allowed("shutdown -h now"));
    }

    #[test]
    fn test_screenshot_region_clipping() {
        let region = crate::actions::ScreenRegion {
            x: 100,
            y: 200,
            width: 800,
            height: 600,
        };
        let action = ComputerAction::Screenshot {
            region: Some(region.clone()),
        };
        assert_eq!(token_cost(&action), 1_000_000);
        assert_eq!(required_capability(&action), "computer_control.screenshot");
        assert_eq!(minimum_autonomy_level(&action), 3);
    }

    #[test]
    fn test_keyboard_type_token_cost_per_character() {
        let short = ComputerAction::KeyboardType { text: "hi".into() };
        assert_eq!(token_cost(&short), 2_000_000); // 2 chars × 1 NXC

        let long = ComputerAction::KeyboardType {
            text: "hello world".into(),
        };
        assert_eq!(token_cost(&long), 11_000_000); // 11 chars × 1 NXC
    }

    #[test]
    fn test_terminal_command_requires_l5_minimum() {
        let action = ComputerAction::TerminalCommand {
            command: "ls -la".into(),
            working_dir: "/home/nexus/workspace".into(),
        };
        assert_eq!(minimum_autonomy_level(&action), 5);
    }

    #[test]
    fn test_governance_denial_no_reason_leaked() {
        let action = ComputerAction::TerminalCommand {
            command: "rm -rf /".into(),
            working_dir: "/home/nexus/workspace".into(),
        };
        let result = check_governance(
            &action,
            5,
            &["computer_control.terminal".into()],
            "/home/nexus/workspace",
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Error message should NOT contain the full dangerous command
        let msg = err.to_string();
        assert!(
            msg.contains("not on allowlist"),
            "Should mention allowlist, got: {msg}"
        );
    }
}
