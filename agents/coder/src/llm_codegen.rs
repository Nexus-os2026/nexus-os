//! LLM-enhanced code generation: produces multi-file projects via an LLM provider.

use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::LlmProvider;
use nexus_sdk::errors::AgentError;
use std::path::{Path, PathBuf};

/// Generate code for a task using an LLM provider.
///
/// Sends a crafted system prompt, parses multi-file code blocks from the response,
/// creates subdirectories as needed, and writes each file to `output_dir`.
pub fn generate_code_with_llm<P: LlmProvider>(
    task: &str,
    output_dir: &Path,
    gateway: &mut GovernedLlmGateway<P>,
    context: &mut AgentRuntimeContext,
    model: &str,
) -> Result<Vec<PathBuf>, AgentError> {
    let prompt = format!(
        "You are an expert software engineer. Generate clean, production-ready code for the \
         task described. Return each file as a code block with the filename in the info string: \
         ```typescript:src/auth.ts or ```rust:src/main.rs. Include ALL necessary files. \
         Add error handling. Add comments.\n\nTask: {task}"
    );

    let response = gateway.query(context, &prompt, 4000, model)?;
    let files = parse_multi_file_response(&response.output_text);

    std::fs::create_dir_all(output_dir)
        .map_err(|e| AgentError::ManifestError(format!("failed to create output dir: {e}")))?;

    let mut created = Vec::new();
    for (filename, content) in &files {
        if filename.is_empty() || content.is_empty() {
            continue;
        }
        let path = output_dir.join(filename);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AgentError::ManifestError(format!("failed to create dir for {filename}: {e}"))
            })?;
        }
        std::fs::write(&path, content)
            .map_err(|e| AgentError::ManifestError(format!("failed to write {filename}: {e}")))?;
        created.push(path);
    }

    Ok(created)
}

/// Strip markdown fences from LLM output.
fn strip_markdown_fences(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") {
        let without_end = &trimmed[..trimmed.len() - 3];
        if let Some(first_newline) = without_end.find('\n') {
            let inner = without_end[first_newline + 1..].trim();
            if !inner.is_empty() {
                return inner.to_string();
            }
        }
    }
    trimmed.to_string()
}

/// Generate code using decomposed per-file LLM calls.
///
/// First asks the LLM to plan which files are needed, then generates each
/// file in a separate call with max_tokens=1024. Retries once on empty.
/// Continues on individual task failure.
pub fn generate_code_decomposed<P: LlmProvider>(
    task: &str,
    output_dir: &Path,
    gateway: &mut GovernedLlmGateway<P>,
    context: &mut AgentRuntimeContext,
    model: &str,
) -> Result<Vec<PathBuf>, AgentError> {
    // Step 1: Ask LLM to plan which files we need
    let plan_prompt = format!(
        "List the files needed for this task. Return ONLY a JSON array of objects with \
         \"filename\" and \"description\" keys. No commentary.\n\nTask: {task}"
    );

    let plan_response = gateway.query(context, &plan_prompt, 512, model)?;
    let plan_text = strip_markdown_fences(&plan_response.output_text);

    #[derive(serde::Deserialize)]
    struct FilePlan {
        filename: String,
        description: String,
    }

    let file_plans: Vec<FilePlan> = serde_json::from_str(&plan_text).unwrap_or_else(|_| {
        // Fallback: infer files from the task description
        let lower = task.to_lowercase();
        let mut plans = vec![FilePlan {
            filename: "src/main.rs".into(),
            description: task.to_string(),
        }];
        if lower.contains("api") || lower.contains("server") {
            plans.push(FilePlan {
                filename: "src/routes.rs".into(),
                description: "API route handlers".into(),
            });
        }
        if lower.contains("auth") {
            plans.push(FilePlan {
                filename: "src/auth.rs".into(),
                description: "Authentication logic".into(),
            });
        }
        if lower.contains("database") || lower.contains("db") {
            plans.push(FilePlan {
                filename: "src/db.rs".into(),
                description: "Database models and queries".into(),
            });
        }
        plans
    });

    if file_plans.is_empty() {
        return Ok(Vec::new());
    }

    eprintln!("[conductor] Code gen plan: {} files", file_plans.len());

    // Step 2: Generate each file in a separate call
    let system_prompt = "You are a code generator. Output ONLY the file content. \
                         No explanations, no markdown fences, no commentary.";

    let mut accumulated: Vec<(String, String)> = Vec::new();

    for (i, plan) in file_plans.iter().enumerate() {
        eprintln!(
            "[conductor] Generating file {}/{}: {}",
            i + 1,
            file_plans.len(),
            plan.filename
        );

        let file_context: String = accumulated
            .iter()
            .map(|(name, content)| {
                let preview: String = content.lines().take(3).collect::<Vec<_>>().join("\n");
                format!(
                    "- {} ({} lines): {}",
                    name,
                    content.lines().count(),
                    preview
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let user_prompt = format!(
            "{system_prompt}\n\n\
             Generate the code for: {desc}\n\
             File to create: {filename}\n\n\
             Files already created:\n{ctx}\n\n\
             Output ONLY the raw file content for {filename}. Nothing else.",
            desc = plan.description,
            filename = plan.filename,
            ctx = if file_context.is_empty() {
                "(none yet)".to_string()
            } else {
                file_context
            },
        );

        let result = gateway.query(context, &user_prompt, 1024, model);

        let content = match result {
            Ok(resp) if !resp.output_text.trim().is_empty() => resp.output_text,
            _ => {
                eprintln!(
                    "[conductor] File {} returned empty, retrying",
                    plan.filename
                );
                let simple = format!(
                    "Write the complete content of '{}'. It should: {}. Output ONLY the code.",
                    plan.filename, plan.description,
                );
                match gateway.query(context, &simple, 1024, model) {
                    Ok(resp) if !resp.output_text.trim().is_empty() => resp.output_text,
                    _ => {
                        eprintln!(
                            "[conductor] File {} failed after retry, skipping",
                            plan.filename
                        );
                        continue;
                    }
                }
            }
        };

        let cleaned = strip_markdown_fences(&content);
        eprintln!(
            "[conductor] File {}/{} complete: {} ({} lines)",
            i + 1,
            file_plans.len(),
            plan.filename,
            cleaned.lines().count()
        );
        accumulated.push((plan.filename.clone(), cleaned));
    }

    // Write all accumulated files
    std::fs::create_dir_all(output_dir)
        .map_err(|e| AgentError::ManifestError(format!("failed to create output dir: {e}")))?;

    let mut created = Vec::new();
    for (filename, content) in &accumulated {
        let path = output_dir.join(filename);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AgentError::ManifestError(format!("failed to create dir: {e}")))?;
        }
        std::fs::write(&path, content)
            .map_err(|e| AgentError::ManifestError(format!("failed to write {filename}: {e}")))?;
        created.push(path);
    }

    eprintln!(
        "[conductor] Code gen complete: {} files, {} total lines",
        created.len(),
        accumulated
            .iter()
            .map(|(_, c)| c.lines().count())
            .sum::<usize>()
    );

    Ok(created)
}

/// Parse a multi-file LLM response into (filename, content) pairs.
///
/// Looks for fenced code blocks with the pattern ````language:filename`
/// (e.g. ````rust:src/main.rs`, ````typescript:src/auth.ts`).
pub fn parse_multi_file_response(response: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let lines: Vec<&str> = response.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        if let Some(stripped) = line.strip_prefix("```") {
            // Check for language:filename pattern
            if let Some(colon_pos) = stripped.find(':') {
                let lang_part = &stripped[..colon_pos];
                let filename = stripped[colon_pos + 1..].trim();

                // Validate: language part should be non-empty alphanumeric, filename should be non-empty
                if !lang_part.is_empty()
                    && !filename.is_empty()
                    && lang_part
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '+')
                {
                    // Collect content until closing ```
                    let mut content_lines = Vec::new();
                    i += 1;
                    while i < lines.len() {
                        if lines[i].starts_with("```")
                            && lines[i].trim_start_matches('`').trim().is_empty()
                        {
                            break;
                        }
                        content_lines.push(lines[i]);
                        i += 1;
                    }
                    let content = content_lines.join("\n");
                    if !content.trim().is_empty() {
                        results.push((filename.to_string(), content));
                    }
                }
            }
        }
        i += 1;
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_file() {
        let response = r#"Here's the code:

```rust:src/main.rs
fn main() {
    println!("Hello, world!");
}
```
"#;
        let files = parse_multi_file_response(response);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "src/main.rs");
        assert!(files[0].1.contains("println!"));
    }

    #[test]
    fn test_parse_multi_file() {
        let response = r#"I'll create the project:

```typescript:src/auth.ts
export function authenticate(token: string): boolean {
    return token.length > 0;
}
```

```typescript:src/index.ts
import { authenticate } from './auth';

const isValid = authenticate('test-token');
console.log(isValid);
```

```json:package.json
{
    "name": "my-app",
    "version": "1.0.0"
}
```
"#;
        let files = parse_multi_file_response(response);
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].0, "src/auth.ts");
        assert!(files[0].1.contains("authenticate"));
        assert_eq!(files[1].0, "src/index.ts");
        assert!(files[1].1.contains("import"));
        assert_eq!(files[2].0, "package.json");
        assert!(files[2].1.contains("my-app"));
    }

    #[test]
    fn test_parse_no_code_blocks() {
        let response = "Just some plain text without any code blocks.";
        let files = parse_multi_file_response(response);
        assert!(files.is_empty());
    }

    #[test]
    fn test_parse_code_blocks_without_filename() {
        let response = r#"```rust
fn main() {}
```
"#;
        let files = parse_multi_file_response(response);
        assert!(files.is_empty());
    }

    #[test]
    fn test_parse_empty_code_block() {
        let response = r#"```rust:src/empty.rs
```
"#;
        let files = parse_multi_file_response(response);
        assert!(files.is_empty());
    }

    #[test]
    fn test_parse_nested_directories() {
        let response = r#"```rust:src/api/handlers/auth.rs
pub fn login() -> Result<(), Error> {
    Ok(())
}
```
"#;
        let files = parse_multi_file_response(response);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "src/api/handlers/auth.rs");
    }

    #[test]
    fn test_parse_mixed_languages() {
        let response = r#"```rust:Cargo.toml
[package]
name = "demo"
```

```c++:src/lib.cpp
int main() { return 0; }
```
"#;
        let files = parse_multi_file_response(response);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].0, "Cargo.toml");
        assert_eq!(files[1].0, "src/lib.cpp");
    }

    #[test]
    fn test_parse_ignores_regular_fences() {
        let response = r#"Here's an example:

```
just a regular code block
```

```rust:src/main.rs
fn real_file() {}
```
"#;
        let files = parse_multi_file_response(response);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "src/main.rs");
    }
}
