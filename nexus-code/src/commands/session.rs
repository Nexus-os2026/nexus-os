//! /session save|list|restore — Session management.

/// Execute the /session command.
pub fn execute(args: &str) -> super::CommandResult {
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    let subcmd = parts.first().copied().unwrap_or("");
    let name = parts.get(1).copied().unwrap_or("");

    let sessions_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("nexus-code")
        .join("sessions");

    match subcmd {
        "save" => {
            let session_name = if name.is_empty() {
                chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string()
            } else {
                name.to_string()
            };

            if let Err(e) = std::fs::create_dir_all(&sessions_dir) {
                return super::CommandResult::Error(format!(
                    "Failed to create sessions dir: {}",
                    e
                ));
            }

            let session_file = sessions_dir.join(format!("{}.json", session_name));
            let session_data = serde_json::json!({
                "name": session_name,
                "saved_at": chrono::Utc::now().to_rfc3339(),
            });

            match std::fs::write(
                &session_file,
                serde_json::to_string_pretty(&session_data).unwrap_or_default(),
            ) {
                Ok(()) => super::CommandResult::Output(format!("Session saved: {}", session_name)),
                Err(e) => super::CommandResult::Error(format!("Failed to save session: {}", e)),
            }
        }
        "list" => match std::fs::read_dir(&sessions_dir) {
            Ok(entries) => {
                let mut sessions: Vec<String> = Vec::new();
                for entry in entries.flatten() {
                    if let Some(fname) = entry.file_name().to_str() {
                        if fname.ends_with(".json") {
                            sessions.push(fname.trim_end_matches(".json").to_string());
                        }
                    }
                }
                if sessions.is_empty() {
                    super::CommandResult::Output("No saved sessions.".to_string())
                } else {
                    super::CommandResult::Output(format!(
                        "Saved sessions:\n{}",
                        sessions
                            .iter()
                            .map(|s| format!("  {}", s))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ))
                }
            }
            Err(_) => super::CommandResult::Output("No saved sessions.".to_string()),
        },
        "restore" => {
            if name.is_empty() {
                return super::CommandResult::Error("Usage: /session restore <name>".to_string());
            }
            let session_file = sessions_dir.join(format!("{}.json", name));
            match std::fs::read_to_string(&session_file) {
                Ok(content) => super::CommandResult::Output(format!("Session info:\n{}", content)),
                Err(e) => {
                    super::CommandResult::Error(format!("Failed to restore '{}': {}", name, e))
                }
            }
        }
        _ => super::CommandResult::Error("Usage: /session <save|list|restore> [name]".to_string()),
    }
}
