use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::{LlmProvider, MockProvider};
use nexus_sdk::audit::{AuditEvent, AuditTrail, EventType};
use nexus_sdk::errors::AgentError;
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const LLM_CAPABILITY: &str = "llm.query";

#[derive(Debug)]
pub struct ProjectInitializer {
    audit_trail: AuditTrail,
    agent_id: Uuid,
}

impl Default for ProjectInitializer {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectInitializer {
    pub fn new() -> Self {
        Self {
            audit_trail: AuditTrail::new(),
            agent_id: Uuid::new_v4(),
        }
    }

    pub fn init_project(
        &mut self,
        language: &str,
        framework: &str,
        name: &str,
    ) -> Result<PathBuf, AgentError> {
        let cwd = std::env::current_dir().map_err(|error| {
            AgentError::SupervisorError(format!("failed reading current directory: {error}"))
        })?;
        self.init_project_in(cwd, language, framework, name, None)
    }

    pub fn init_project_in(
        &mut self,
        base_dir: impl AsRef<Path>,
        language: &str,
        framework: &str,
        name: &str,
        description: Option<&str>,
    ) -> Result<PathBuf, AgentError> {
        validate_project_name(name)?;

        let root = base_dir.as_ref().join(name);
        if root.exists() {
            return Err(AgentError::SupervisorError(format!(
                "target project '{}' already exists",
                root.display()
            )));
        }
        fs::create_dir_all(root.as_path()).map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed creating project directory '{}': {error}",
                root.display()
            ))
        })?;

        let normalized_language = language.trim().to_ascii_lowercase();
        let normalized_framework = framework.trim().to_ascii_lowercase();
        match (normalized_language.as_str(), normalized_framework.as_str()) {
            ("rust", "binary") => scaffold_rust_binary(root.as_path(), name)?,
            ("rust", "library") | ("rust", "lib") => scaffold_rust_library(root.as_path(), name)?,
            ("rust", "workspace") => scaffold_rust_workspace(root.as_path(), name)?,
            ("typescript", "next.js") | ("typescript", "next") => {
                scaffold_typescript_next(root.as_path(), name)?
            }
            ("typescript", "vite") => scaffold_typescript_vite(root.as_path(), name)?,
            ("typescript", "plain") => scaffold_typescript_plain(root.as_path(), name)?,
            ("python", "fastapi") => scaffold_python_fastapi(root.as_path(), name)?,
            ("python", "flask") => scaffold_python_flask(root.as_path(), name)?,
            ("python", "cli") => scaffold_python_cli(root.as_path(), name)?,
            ("full-stack", "next.js + python api")
            | ("full-stack", "next+python")
            | ("fullstack", "next+python") => scaffold_full_stack(root.as_path(), name)?,
            _ => {
                return Err(AgentError::SupervisorError(format!(
                    "unsupported template language/framework: '{language}' / '{framework}'"
                )))
            }
        }

        let readme = build_readme(language, framework, name, description)?;
        write_file(root.join("README.md"), readme.as_str())?;
        write_file(
            root.join(".gitignore"),
            "target/\nnode_modules/\n.venv/\n.env\n__pycache__/\ndist/\n",
        )?;
        write_file(
            root.join(".github/workflows/ci.yml"),
            default_ci(language).as_str(),
        )?;

        self.audit_trail.append_event(
            self.agent_id,
            EventType::ToolCall,
            json!({
                "tool": "init_project",
                "language": language,
                "framework": framework,
                "name": name,
                "description_provided": description.is_some(),
                "path": root.to_string_lossy().to_string(),
            }),
        );

        Ok(root)
    }

    pub fn audit_events(&self) -> &[AuditEvent] {
        self.audit_trail.events()
    }
}

pub fn init_project(language: &str, framework: &str, name: &str) -> Result<PathBuf, AgentError> {
    let mut initializer = ProjectInitializer::new();
    initializer.init_project(language, framework, name)
}

pub fn init_project_in(
    base_dir: impl AsRef<Path>,
    language: &str,
    framework: &str,
    name: &str,
    description: Option<&str>,
) -> Result<PathBuf, AgentError> {
    let mut initializer = ProjectInitializer::new();
    initializer.init_project_in(base_dir, language, framework, name, description)
}

fn validate_project_name(name: &str) -> Result<(), AgentError> {
    if name.trim().is_empty() {
        return Err(AgentError::ManifestError(
            "project name cannot be empty".to_string(),
        ));
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(AgentError::ManifestError(
            "project name must not contain path traversal".to_string(),
        ));
    }
    Ok(())
}

fn scaffold_rust_binary(root: &Path, name: &str) -> Result<(), AgentError> {
    write_file(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n"
        )
        .as_str(),
    )?;
    write_file(
        root.join("src/main.rs"),
        "fn main() {\n    println!(\"Hello from NexusOS scaffold\");\n}\n",
    )?;
    write_file(
        root.join("tests/smoke.rs"),
        "#[test]\nfn smoke() {\n    assert_eq!(2 + 2, 4);\n}\n",
    )?;
    Ok(())
}

fn scaffold_rust_library(root: &Path, name: &str) -> Result<(), AgentError> {
    write_file(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n"
        )
        .as_str(),
    )?;
    write_file(
        root.join("src/lib.rs"),
        "pub fn add(left: i32, right: i32) -> i32 {\n    left + right\n}\n",
    )?;
    write_file(
        root.join("tests/smoke.rs"),
        "use super::*;\n\n#[test]\nfn smoke() {\n    assert_eq!(add(1, 2), 3);\n}\n",
    )?;
    Ok(())
}

fn scaffold_rust_workspace(root: &Path, name: &str) -> Result<(), AgentError> {
    write_file(
        root.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"crates/app\"]\n",
    )?;
    let crate_dir = root.join("crates/app");
    write_file(
        crate_dir.join("Cargo.toml"),
        format!("[package]\nname = \"{name}-app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")
            .as_str(),
    )?;
    write_file(
        crate_dir.join("src/main.rs"),
        "fn main() {\n    println!(\"workspace app\");\n}\n",
    )?;
    write_file(
        root.join("tests/smoke.rs"),
        "#[test]\nfn smoke() {\n    assert!(true);\n}\n",
    )?;
    Ok(())
}

fn scaffold_typescript_next(root: &Path, name: &str) -> Result<(), AgentError> {
    write_file(
        root.join("package.json"),
        format!(
            "{{\n  \"name\": \"{name}\",\n  \"private\": true,\n  \"scripts\": {{\n    \"dev\": \"next dev\",\n    \"build\": \"next build\",\n    \"test\": \"echo no-tests\"\n  }}\n}}\n"
        )
        .as_str(),
    )?;
    write_file(
        root.join("pages/index.tsx"),
        "export default function Home() { return <main>NexusOS Next.js scaffold</main>; }\n",
    )?;
    write_file(
        root.join("tests/smoke.test.ts"),
        "test('smoke', () => expect(2 + 2).toBe(4));\n",
    )?;
    Ok(())
}

fn scaffold_typescript_vite(root: &Path, name: &str) -> Result<(), AgentError> {
    write_file(
        root.join("package.json"),
        format!(
            "{{\n  \"name\": \"{name}\",\n  \"private\": true,\n  \"scripts\": {{\n    \"dev\": \"vite\",\n    \"build\": \"vite build\",\n    \"test\": \"echo no-tests\"\n  }}\n}}\n"
        )
        .as_str(),
    )?;
    write_file(
        root.join("src/main.ts"),
        "console.log('NexusOS Vite scaffold');\n",
    )?;
    write_file(
        root.join("tests/smoke.test.ts"),
        "test('smoke', () => expect(2 + 2).toBe(4));\n",
    )?;
    Ok(())
}

fn scaffold_typescript_plain(root: &Path, name: &str) -> Result<(), AgentError> {
    write_file(
        root.join("package.json"),
        format!(
            "{{\n  \"name\": \"{name}\",\n  \"private\": true,\n  \"scripts\": {{\n    \"build\": \"tsc\",\n    \"test\": \"echo no-tests\"\n  }}\n}}\n"
        )
        .as_str(),
    )?;
    write_file(root.join("src/index.ts"), "export const ready = true;\n")?;
    write_file(
        root.join("tests/smoke.test.ts"),
        "test('smoke', () => expect(true).toBe(true));\n",
    )?;
    Ok(())
}

fn scaffold_python_fastapi(root: &Path, _name: &str) -> Result<(), AgentError> {
    write_file(
        root.join("pyproject.toml"),
        "[project]\nname = \"nexus-app\"\nversion = \"0.1.0\"\nrequires-python = \">=3.10\"\n",
    )?;
    write_file(
        root.join("app/main.py"),
        "from fastapi import FastAPI\n\napp = FastAPI()\n\n@app.get('/health')\ndef health() -> dict[str, str]:\n    return {'status': 'ok'}\n",
    )?;
    write_file(
        root.join("tests/test_smoke.py"),
        "def test_smoke() -> None:\n    assert 2 + 2 == 4\n",
    )?;
    Ok(())
}

fn scaffold_python_flask(root: &Path, _name: &str) -> Result<(), AgentError> {
    write_file(
        root.join("pyproject.toml"),
        "[project]\nname = \"nexus-app\"\nversion = \"0.1.0\"\nrequires-python = \">=3.10\"\n",
    )?;
    write_file(
        root.join("app.py"),
        "from flask import Flask\n\napp = Flask(__name__)\n\n@app.get('/health')\ndef health() -> dict[str, str]:\n    return {'status': 'ok'}\n",
    )?;
    write_file(
        root.join("tests/test_smoke.py"),
        "def test_smoke() -> None:\n    assert 2 + 2 == 4\n",
    )?;
    Ok(())
}

fn scaffold_python_cli(root: &Path, _name: &str) -> Result<(), AgentError> {
    write_file(
        root.join("pyproject.toml"),
        "[project]\nname = \"nexus-cli\"\nversion = \"0.1.0\"\nrequires-python = \">=3.10\"\n",
    )?;
    write_file(
        root.join("src/main.py"),
        "def main() -> None:\n    print('NexusOS CLI scaffold')\n\nif __name__ == '__main__':\n    main()\n",
    )?;
    write_file(
        root.join("tests/test_smoke.py"),
        "def test_smoke() -> None:\n    assert 2 + 2 == 4\n",
    )?;
    Ok(())
}

fn scaffold_full_stack(root: &Path, name: &str) -> Result<(), AgentError> {
    scaffold_typescript_next(root.join("web").as_path(), format!("{name}-web").as_str())?;
    scaffold_python_fastapi(root.join("api").as_path(), format!("{name}-api").as_str())?;
    write_file(
        root.join("tests/smoke.md"),
        "Run `npm run build` in web and `python -m pytest` in api.\n",
    )?;
    Ok(())
}

fn write_file(path: impl AsRef<Path>, content: &str) -> Result<(), AgentError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed creating directory '{}': {error}",
                parent.display()
            ))
        })?;
    }
    fs::write(path, content).map_err(|error| {
        AgentError::SupervisorError(format!("failed writing '{}': {error}", path.display()))
    })?;
    Ok(())
}

fn default_ci(language: &str) -> String {
    let lower = language.to_ascii_lowercase();
    if lower == "rust" {
        return "name: ci\non: [push, pull_request]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - uses: dtolnay/rust-toolchain@stable\n      - run: cargo test --workspace\n".to_string();
    }
    if lower == "python" {
        return "name: ci\non: [push, pull_request]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - uses: actions/setup-python@v5\n        with:\n          python-version: '3.11'\n      - run: python -m pytest -v\n".to_string();
    }
    "name: ci\non: [push, pull_request]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: echo \"configure project-specific CI\"\n".to_string()
}

fn build_readme(
    language: &str,
    framework: &str,
    name: &str,
    description: Option<&str>,
) -> Result<String, AgentError> {
    let mut summary = format!("# {name}\n\n");
    summary.push_str(format!("Scaffold generated for {language} / {framework}.\n\n").as_str());

    if let Some(user_description) = description {
        let provider: Box<dyn LlmProvider> = Box::new(MockProvider::new());
        let mut gateway = GovernedLlmGateway::new(provider);
        let capabilities = [LLM_CAPABILITY.to_string()]
            .into_iter()
            .collect::<HashSet<_>>();
        let mut runtime = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities,
            fuel_remaining: 1_000,
        };
        let prompt =
            format!("Summarize this project intent in one short sentence: {user_description}");
        let response = gateway.query(&mut runtime, prompt.as_str(), 64, "mock-1")?;
        summary.push_str("## Intent\n\n");
        summary.push_str(format!("- User description: {user_description}\n").as_str());
        summary.push_str(format!("- LLM summary: {}\n", response.output_text).as_str());
    }

    summary.push_str("\n## Getting Started\n\n");
    summary.push_str("- Review project structure\n");
    summary.push_str("- Run tests before committing changes\n");
    Ok(summary)
}
