//! Agent project scaffolding for `nexus create`.

use crate::templates::{find_template, template_names, AgentTemplate};
use std::fs;
use std::path::{Path, PathBuf};

/// Result of a successful scaffold operation.
#[derive(Debug)]
pub struct ScaffoldResult {
    pub project_dir: PathBuf,
    pub agent_name: String,
    pub template: String,
    pub files_created: Vec<String>,
}

/// Scaffold a complete Nexus agent project directory.
pub fn scaffold_agent_project(
    name: &str,
    template_name: &str,
    parent_dir: &Path,
) -> Result<ScaffoldResult, String> {
    // Validate name (same rules as kernel manifest: 3-64 chars, alphanumeric + hyphens).
    validate_agent_name(name)?;

    let template = find_template(template_name).ok_or_else(|| {
        format!(
            "Unknown template '{}'. Available: {}",
            template_name,
            template_names().join(", ")
        )
    })?;

    let project_dir = parent_dir.join(name);
    if project_dir.exists() {
        return Err(format!(
            "Directory '{}' already exists",
            project_dir.display()
        ));
    }

    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir)
        .map_err(|e| format!("Failed to create {}: {e}", src_dir.display()))?;

    let mut files_created = Vec::new();

    // 1. Cargo.toml
    let cargo_toml = generate_cargo_toml(name);
    write_file(&project_dir.join("Cargo.toml"), &cargo_toml)?;
    files_created.push("Cargo.toml".into());

    // 2. manifest.toml
    let manifest_toml = generate_manifest_toml(name, template);
    write_file(&project_dir.join("manifest.toml"), &manifest_toml)?;
    files_created.push("manifest.toml".into());

    // 3. src/lib.rs
    write_file(&src_dir.join("lib.rs"), template.lib_rs)?;
    files_created.push("src/lib.rs".into());

    // 4. README.md
    let readme = generate_readme(name, template);
    write_file(&project_dir.join("README.md"), &readme)?;
    files_created.push("README.md".into());

    Ok(ScaffoldResult {
        project_dir,
        agent_name: name.to_string(),
        template: template_name.to_string(),
        files_created,
    })
}

fn validate_agent_name(name: &str) -> Result<(), String> {
    let len = name.chars().count();
    if !(3..=64).contains(&len) {
        return Err(format!("Agent name must be 3-64 characters, got {len}"));
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return Err("Agent name must contain only alphanumeric characters and hyphens".into());
    }
    Ok(())
}

fn generate_cargo_toml(name: &str) -> String {
    // Convert agent name to a valid Rust crate name (hyphens are fine in Cargo).
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
license = "MIT"

[dependencies]
nexus-sdk = {{ path = "../sdk" }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
"#
    )
}

fn generate_manifest_toml(name: &str, template: &AgentTemplate) -> String {
    let caps: Vec<String> = template
        .capabilities
        .iter()
        .map(|c| format!("\"{}\"", c))
        .collect();

    format!(
        r#"name = "{name}"
version = "0.1.0"
capabilities = [{caps}]
fuel_budget = {fuel}
autonomy_level = {autonomy}
"#,
        caps = caps.join(", "),
        fuel = template.fuel_budget,
        autonomy = template.autonomy_level,
    )
}

fn generate_readme(name: &str, template: &AgentTemplate) -> String {
    let caps_list: String = template
        .capabilities
        .iter()
        .map(|c| format!("- `{c}`"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"# {name}

{description}

## Capabilities

{caps_list}

## Quick Start

```bash
# Build the agent
cargo build

# Run tests
cargo test

# Package for marketplace (coming soon)
# nexus package .
```

{extra}
"#,
        description = template.description,
        extra = template.readme_extra,
    )
}

fn write_file(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_name_accepts_valid() {
        assert!(validate_agent_name("my-agent").is_ok());
        assert!(validate_agent_name("agent123").is_ok());
        assert!(validate_agent_name("foo").is_ok());
    }

    #[test]
    fn validate_name_rejects_short() {
        assert!(validate_agent_name("ab").is_err());
    }

    #[test]
    fn validate_name_rejects_special_chars() {
        assert!(validate_agent_name("my_agent").is_err());
        assert!(validate_agent_name("my agent").is_err());
    }

    #[test]
    fn generate_cargo_toml_has_sdk_dep() {
        let toml = generate_cargo_toml("test-agent");
        assert!(toml.contains("nexus-sdk"));
        assert!(toml.contains("name = \"test-agent\""));
        assert!(toml.contains("edition = \"2021\""));
    }

    #[test]
    fn generate_manifest_toml_valid() {
        use crate::templates::BASIC;
        let toml = generate_manifest_toml("test-agent", &BASIC);
        assert!(toml.contains("name = \"test-agent\""));
        assert!(toml.contains("fuel_budget = 10000"));
        assert!(toml.contains("\"llm.query\""));
    }

    #[test]
    fn scaffold_creates_project() {
        let tmp = std::env::temp_dir().join("nexus-scaffold-test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let result = scaffold_agent_project("test-agent", "basic", &tmp).unwrap();
        assert_eq!(result.agent_name, "test-agent");
        assert_eq!(result.template, "basic");
        assert_eq!(result.files_created.len(), 4);
        assert!(result.project_dir.join("Cargo.toml").exists());
        assert!(result.project_dir.join("manifest.toml").exists());
        assert!(result.project_dir.join("src/lib.rs").exists());
        assert!(result.project_dir.join("README.md").exists());

        // Verify manifest.toml is valid TOML
        let manifest_str = fs::read_to_string(result.project_dir.join("manifest.toml")).unwrap();
        assert!(manifest_str.contains("name = \"test-agent\""));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn scaffold_rejects_existing_dir() {
        let tmp = std::env::temp_dir().join("nexus-scaffold-exists");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("my-agent")).unwrap();

        let result = scaffold_agent_project("my-agent", "basic", &tmp);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn scaffold_unknown_template() {
        let tmp = std::env::temp_dir().join("nexus-scaffold-unknown");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let result = scaffold_agent_project("my-agent", "nonexistent", &tmp);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown template"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn scaffold_all_templates() {
        let tmp = std::env::temp_dir().join("nexus-scaffold-all-templates");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let templates = [
            "basic",
            "data-analyst",
            "web-researcher",
            "code-reviewer",
            "content-writer",
            "file-organizer",
        ];
        for tmpl in &templates {
            let name = format!("test-{tmpl}");
            let result = scaffold_agent_project(&name, tmpl, &tmp).unwrap();
            assert!(result.project_dir.join("Cargo.toml").exists());
            assert!(result.project_dir.join("manifest.toml").exists());
            assert!(result.project_dir.join("src/lib.rs").exists());
            assert!(result.project_dir.join("README.md").exists());

            // Verify manifest.toml parses with kernel parser
            let manifest_content =
                fs::read_to_string(result.project_dir.join("manifest.toml")).unwrap();
            let parsed = nexus_kernel::manifest::parse_manifest(&manifest_content);
            assert!(
                parsed.is_ok(),
                "Template '{tmpl}' produced invalid manifest: {:?}",
                parsed.err()
            );
        }

        let _ = fs::remove_dir_all(&tmp);
    }
}
