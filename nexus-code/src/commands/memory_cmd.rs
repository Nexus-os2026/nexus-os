//! /memory <list|add|verify|clear> — Cross-session memory management.

pub fn execute(args: &str) -> super::CommandResult {
    let parts: Vec<&str> = args.splitn(3, ' ').collect();
    let subcmd = parts.first().copied().unwrap_or("");

    match subcmd {
        "list" => {
            let memory_path = dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("nexus-code")
                .join("memory.json");
            let store = crate::persistence::memory::MemoryStore::load(memory_path);
            if store.is_empty() {
                super::CommandResult::Output("No memories stored.".to_string())
            } else {
                let mut out = format!("{} memories:\n", store.len());
                for entry in store.entries() {
                    out.push_str(&format!(
                        "  [{}] {}: {}\n",
                        entry.category,
                        &entry.id[..8.min(entry.id.len())],
                        if entry.content.len() > 60 {
                            format!("{}...", &entry.content[..60])
                        } else {
                            entry.content.clone()
                        }
                    ));
                }
                super::CommandResult::Output(out)
            }
        }
        "add" => {
            if parts.len() < 3 {
                return super::CommandResult::Error(
                    "Usage: /memory add <category> <content>".to_string(),
                );
            }
            let category = parts[1];
            let content = parts[2];
            let memory_path = dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("nexus-code")
                .join("memory.json");
            let mut store = crate::persistence::memory::MemoryStore::load(memory_path);
            store.add(category, content, "current");
            match store.save() {
                Ok(()) => super::CommandResult::Output(format!(
                    "Memory added: [{}] {}",
                    category, content
                )),
                Err(e) => super::CommandResult::Error(format!("Failed to save memory: {}", e)),
            }
        }
        "verify" => {
            let memory_path = dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("nexus-code")
                .join("memory.json");
            let store = crate::persistence::memory::MemoryStore::load(memory_path);
            let corrupted = store.verify_integrity();
            if corrupted.is_empty() {
                super::CommandResult::Output(format!(
                    "All {} memories verified. Integrity: OK",
                    store.len()
                ))
            } else {
                super::CommandResult::Error(format!(
                    "{} corrupted entries detected: {:?}",
                    corrupted.len(),
                    corrupted
                ))
            }
        }
        "clear" => {
            let memory_path = dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("nexus-code")
                .join("memory.json");
            let store = crate::persistence::memory::MemoryStore::load(memory_path);
            match store.save() {
                Ok(()) => super::CommandResult::Output("Memory cleared.".to_string()),
                Err(e) => super::CommandResult::Error(format!("Failed to clear: {}", e)),
            }
        }
        _ => super::CommandResult::Error("Usage: /memory <list|add|verify|clear>".to_string()),
    }
}
