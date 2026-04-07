//! /computer-use on|off — Toggle computer use capabilities at runtime.

/// Execute the /computer-use command.
/// `/computer-use on`  — Enable computer use tools (grants ComputerUse capability).
/// `/computer-use off` — Disable computer use tools (revokes capability).
pub async fn execute(args: &str, app: &mut crate::app::App) -> super::CommandResult {
    let args = args.trim();

    match args {
        "on" => {
            if app.is_computer_use_active() {
                super::CommandResult::Output("Computer use is already active.".to_string())
            } else {
                app.enable_computer_use();
                super::CommandResult::Output(
                    "Computer use enabled. Tools: screen_capture, screen_interact, screen_analyze"
                        .to_string(),
                )
            }
        }
        "off" => {
            if !app.is_computer_use_active() {
                super::CommandResult::Output("Computer use is already disabled.".to_string())
            } else {
                app.disable_computer_use();
                super::CommandResult::Output("Computer use disabled.".to_string())
            }
        }
        "" => {
            let status = if app.is_computer_use_active() {
                "active"
            } else {
                "disabled"
            };
            super::CommandResult::Output(format!("Computer use: {}", status))
        }
        _ => super::CommandResult::Error("Usage: /computer-use [on|off]".to_string()),
    }
}
