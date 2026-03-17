//! Hot-deploy — register new agents into the running kernel without restart.

use std::path::{Path, PathBuf};

use crate::genome::JsonAgentManifest;

/// Directory name for AI-generated agent manifests.
pub const GENERATED_AGENTS_DIR: &str = "agents/generated";

/// Directory name for genesis creation patterns.
pub const GENESIS_MEMORY_DIR: &str = "agents/genesis_memory";

/// Save a generated agent manifest to disk.
pub fn save_manifest(base_dir: &Path, manifest: &JsonAgentManifest) -> Result<PathBuf, String> {
    let dir = base_dir.join(GENERATED_AGENTS_DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create generated agents directory: {e}"))?;

    let filename = format!("{}.json", manifest.name);
    let path = dir.join(&filename);

    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("Failed to serialize manifest: {e}"))?;

    std::fs::write(&path, json).map_err(|e| format!("Failed to write manifest: {e}"))?;

    Ok(path)
}

/// Save a genome alongside its manifest.
pub fn save_genome(
    base_dir: &Path,
    agent_name: &str,
    genome_json: &str,
) -> Result<PathBuf, String> {
    let dir = base_dir.join(GENERATED_AGENTS_DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create generated agents directory: {e}"))?;

    let filename = format!("{}.genome.json", agent_name);
    let path = dir.join(&filename);

    std::fs::write(&path, genome_json).map_err(|e| format!("Failed to write genome: {e}"))?;

    Ok(path)
}

/// List all generated agent manifests from disk.
pub fn list_generated_manifests(base_dir: &Path) -> Result<Vec<JsonAgentManifest>, String> {
    let dir = base_dir.join(GENERATED_AGENTS_DIR);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut manifests = Vec::new();

    let entries =
        std::fs::read_dir(&dir).map_err(|e| format!("Failed to read generated dir: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Directory entry error: {e}"))?;
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "json")
            && !path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().contains(".genome."))
        {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {path:?}: {e}"))?;

            let manifest: JsonAgentManifest = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse {path:?}: {e}"))?;

            manifests.push(manifest);
        }
    }

    Ok(manifests)
}

/// Delete a generated agent manifest and its genome from disk.
pub fn delete_generated_agent(base_dir: &Path, agent_name: &str) -> Result<(), String> {
    let dir = base_dir.join(GENERATED_AGENTS_DIR);

    let manifest_path = dir.join(format!("{agent_name}.json"));
    if manifest_path.exists() {
        std::fs::remove_file(&manifest_path)
            .map_err(|e| format!("Failed to delete manifest: {e}"))?;
    }

    let genome_path = dir.join(format!("{agent_name}.genome.json"));
    if genome_path.exists() {
        std::fs::remove_file(&genome_path).map_err(|e| format!("Failed to delete genome: {e}"))?;
    }

    Ok(())
}

/// Validate a manifest before deployment.
pub fn validate_manifest(manifest: &JsonAgentManifest) -> Result<(), String> {
    if manifest.name.len() < 3 {
        return Err("Agent name must be at least 3 characters".to_string());
    }
    if !manifest.name.starts_with("nexus-") {
        return Err("Generated agent name must start with 'nexus-'".to_string());
    }
    if manifest.description.is_empty() {
        return Err("Agent must have a system prompt (description)".to_string());
    }
    if manifest.autonomy_level > 5 {
        return Err("Generated agents cannot exceed L5 autonomy".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> JsonAgentManifest {
        JsonAgentManifest {
            name: "nexus-testdeploy".to_string(),
            version: "1.0.0".to_string(),
            description: "Test deployment agent.".to_string(),
            capabilities: vec!["fs.read".to_string()],
            autonomy_level: 3,
            fuel_budget: 10_000,
            llm_model: None,
            schedule: None,
            default_goal: None,
        }
    }

    #[test]
    fn save_and_load_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let manifest = sample_manifest();

        let path = save_manifest(tmp.path(), &manifest).unwrap();
        assert!(path.exists());

        let manifests = list_generated_manifests(tmp.path()).unwrap();
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].name, "nexus-testdeploy");
    }

    #[test]
    fn save_and_delete_agent() {
        let tmp = tempfile::tempdir().unwrap();
        let manifest = sample_manifest();

        save_manifest(tmp.path(), &manifest).unwrap();
        save_genome(tmp.path(), "nexus-testdeploy", "{}").unwrap();

        delete_generated_agent(tmp.path(), "nexus-testdeploy").unwrap();

        let manifests = list_generated_manifests(tmp.path()).unwrap();
        assert!(manifests.is_empty());
    }

    #[test]
    fn validate_manifest_rejects_bad_name() {
        let mut m = sample_manifest();
        m.name = "ab".to_string();
        assert!(validate_manifest(&m).is_err());

        m.name = "bad-agent".to_string();
        assert!(validate_manifest(&m).is_err());
    }

    #[test]
    fn validate_manifest_rejects_high_autonomy() {
        let mut m = sample_manifest();
        m.autonomy_level = 6;
        assert!(validate_manifest(&m).is_err());
    }

    #[test]
    fn validate_manifest_accepts_valid() {
        let m = sample_manifest();
        assert!(validate_manifest(&m).is_ok());
    }

    #[test]
    fn list_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let manifests = list_generated_manifests(tmp.path()).unwrap();
        assert!(manifests.is_empty());
    }
}
