//! Code analyzer — reads source files and performs static analysis to identify
//! functions with high complexity, excessive allocations, and other issues.

use serde::{Deserialize, Serialize};

use super::SelfRewriteError;

/// Severity of a code issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
}

/// Analysis of a single function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionAnalysis {
    pub name: String,
    pub line_start: usize,
    pub line_end: usize,
    pub complexity: u32,
    pub allocations_estimated: u32,
}

/// A code quality issue found during analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeIssue {
    pub description: String,
    pub severity: IssueSeverity,
    pub location: String,
    pub suggested_fix: String,
}

/// Result of analyzing a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub file_path: String,
    pub functions: Vec<FunctionAnalysis>,
    pub issues: Vec<CodeIssue>,
}

/// Static code analyzer that reads source files and identifies optimization
/// opportunities.
#[derive(Debug, Clone)]
pub struct CodeAnalyzer {
    /// Complexity threshold above which a function is flagged.
    complexity_threshold: u32,
    /// Allocation count threshold above which a function is flagged.
    allocation_threshold: u32,
    /// Maximum function length in lines before flagging.
    max_function_lines: usize,
}

impl CodeAnalyzer {
    pub fn new() -> Self {
        Self {
            complexity_threshold: 10,
            allocation_threshold: 5,
            max_function_lines: 100,
        }
    }

    pub fn with_thresholds(
        complexity_threshold: u32,
        allocation_threshold: u32,
        max_function_lines: usize,
    ) -> Self {
        Self {
            complexity_threshold,
            allocation_threshold,
            max_function_lines,
        }
    }

    /// Analyze a source file given its content.
    pub fn analyze_file(
        &self,
        file_path: &str,
        source: &str,
    ) -> Result<AnalysisResult, SelfRewriteError> {
        let functions = self.extract_functions(source);
        let mut issues = Vec::new();

        for func in &functions {
            let func_lines = func.line_end.saturating_sub(func.line_start) + 1;

            if func.complexity > self.complexity_threshold {
                issues.push(CodeIssue {
                    description: format!(
                        "function '{}' has cyclomatic complexity {} (threshold {})",
                        func.name, func.complexity, self.complexity_threshold
                    ),
                    severity: if func.complexity > self.complexity_threshold * 2 {
                        IssueSeverity::Error
                    } else {
                        IssueSeverity::Warning
                    },
                    location: format!("{file_path}:{}", func.line_start),
                    suggested_fix: "Break into smaller functions or simplify control flow"
                        .to_string(),
                });
            }

            if func.allocations_estimated > self.allocation_threshold {
                issues.push(CodeIssue {
                    description: format!(
                        "function '{}' has ~{} heap allocations (threshold {})",
                        func.name, func.allocations_estimated, self.allocation_threshold
                    ),
                    severity: IssueSeverity::Warning,
                    location: format!("{file_path}:{}", func.line_start),
                    suggested_fix: "Reuse buffers, use stack allocation, or reduce cloning"
                        .to_string(),
                });
            }

            if func_lines > self.max_function_lines {
                issues.push(CodeIssue {
                    description: format!(
                        "function '{}' is {} lines long (max {})",
                        func.name, func_lines, self.max_function_lines
                    ),
                    severity: IssueSeverity::Info,
                    location: format!("{file_path}:{}", func.line_start),
                    suggested_fix: "Extract helper functions to improve readability".to_string(),
                });
            }
        }

        Ok(AnalysisResult {
            file_path: file_path.to_string(),
            functions,
            issues,
        })
    }

    /// Analyze all files in a crate directory (given as a list of
    /// (path, content) pairs).
    pub fn analyze_crate(
        &self,
        files: &[(&str, &str)],
    ) -> Result<Vec<AnalysisResult>, SelfRewriteError> {
        let mut results = Vec::new();
        for (path, content) in files {
            results.push(self.analyze_file(path, content)?);
        }
        Ok(results)
    }

    /// Find functions that appear in the given hot-path list (by name).
    pub fn find_hot_functions(
        &self,
        analysis: &[AnalysisResult],
        hot_function_names: &[&str],
    ) -> Vec<FunctionAnalysis> {
        let mut found = Vec::new();
        for result in analysis {
            for func in &result.functions {
                if hot_function_names.contains(&func.name.as_str()) {
                    found.push(func.clone());
                }
            }
        }
        found
    }

    /// Extract functions from Rust source code using simple parsing.
    fn extract_functions(&self, source: &str) -> Vec<FunctionAnalysis> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        let mut i = 0;
        while i < lines.len() {
            let trimmed = lines[i].trim();

            // Detect function declarations (pub fn, fn, pub(crate) fn, etc.)
            if let Some(fn_name) = Self::parse_fn_name(trimmed) {
                let line_start = i + 1; // 1-indexed
                let line_end = self.find_function_end(&lines, i);

                let body = &lines[i..=line_end.min(lines.len() - 1)];
                let body_text = body.join("\n");

                let complexity = self.estimate_complexity(&body_text);
                let allocations = self.estimate_allocations(&body_text);

                functions.push(FunctionAnalysis {
                    name: fn_name,
                    line_start,
                    line_end: line_end + 1, // 1-indexed
                    complexity,
                    allocations_estimated: allocations,
                });

                i = line_end + 1;
                continue;
            }
            i += 1;
        }

        functions
    }

    /// Parse a function name from a line like `pub fn foo(...`.
    fn parse_fn_name(line: &str) -> Option<String> {
        // Skip comments, attributes, and trait/impl declarations
        if line.starts_with("//") || line.starts_with('#') || line.starts_with("/*") {
            return None;
        }

        // Look for `fn ` keyword
        let fn_idx = line.find("fn ")?;

        // Make sure `fn` is preceded by whitespace or is at the start (after
        // visibility modifiers).
        if fn_idx > 0 {
            let before = &line[..fn_idx];
            let before_trimmed = before.trim();
            // Must be a visibility or qualifier
            let valid_prefixes = [
                "pub",
                "pub(crate)",
                "pub(super)",
                "pub(self)",
                "async",
                "const",
                "unsafe",
                "extern",
                "",
            ];
            let words: Vec<&str> = before_trimmed.split_whitespace().collect();
            if !words.iter().all(|w| valid_prefixes.contains(w)) {
                return None;
            }
        }

        let after_fn = &line[fn_idx + 3..];
        let name_end = after_fn.find(|c: char| !c.is_alphanumeric() && c != '_')?;
        let name = &after_fn[..name_end];
        if name.is_empty() {
            return None;
        }
        Some(name.to_string())
    }

    /// Find the closing brace of a function starting at `start_line`.
    fn find_function_end(&self, lines: &[&str], start_line: usize) -> usize {
        let mut brace_depth = 0i32;
        let mut found_opening = false;

        for (offset, line) in lines[start_line..].iter().enumerate() {
            for ch in line.chars() {
                if ch == '{' {
                    brace_depth += 1;
                    found_opening = true;
                } else if ch == '}' {
                    brace_depth -= 1;
                }
            }
            if found_opening && brace_depth == 0 {
                return start_line + offset;
            }
        }

        // If we never found matching braces, return the last line
        lines.len().saturating_sub(1)
    }

    /// Estimate cyclomatic complexity by counting branching keywords.
    fn estimate_complexity(&self, body: &str) -> u32 {
        let mut complexity: u32 = 1; // Base complexity
        let keywords = [
            "if ", "else ", "match ", "for ", "while ", "loop ", "&&", "||", "?",
        ];
        for kw in &keywords {
            complexity += body.matches(kw).count() as u32;
        }
        complexity
    }

    /// Estimate heap allocations by counting common allocation patterns.
    fn estimate_allocations(&self, body: &str) -> u32 {
        let patterns = [
            "Vec::new",
            "vec![",
            "String::new",
            "String::from",
            ".to_string()",
            ".to_owned()",
            ".clone()",
            "Box::new",
            "HashMap::new",
            "HashSet::new",
            "BTreeMap::new",
            ".into()",
            "format!",
        ];
        let mut count: u32 = 0;
        for pat in &patterns {
            count += body.matches(pat).count() as u32;
        }
        count
    }
}

impl Default for CodeAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RS: &str = r#"
use std::collections::HashMap;

pub fn simple_add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn complex_function(input: &str) -> String {
    let mut result = String::new();
    if input.is_empty() {
        return result;
    }
    for ch in input.chars() {
        if ch.is_alphanumeric() {
            result.push(ch);
        } else if ch == ' ' {
            result.push('_');
        } else {
            let s = format!("\\x{:02x}", ch as u32);
            result.push_str(&s);
        }
    }
    if result.len() > 100 {
        result.truncate(100);
    }
    result
}

fn private_fn() -> Vec<u8> {
    let v = Vec::new();
    v
}
"#;

    #[test]
    fn extract_functions_from_source() {
        let analyzer = CodeAnalyzer::new();
        let result = analyzer.analyze_file("test.rs", SAMPLE_RS).unwrap();
        assert_eq!(result.functions.len(), 3);
        assert_eq!(result.functions[0].name, "simple_add");
        assert_eq!(result.functions[1].name, "complex_function");
        assert_eq!(result.functions[2].name, "private_fn");
    }

    #[test]
    fn complexity_flags_complex_functions() {
        let analyzer = CodeAnalyzer::with_thresholds(3, 100, 1000);
        let result = analyzer.analyze_file("test.rs", SAMPLE_RS).unwrap();
        // complex_function has if/for/else branches → should be flagged
        let complexity_issues: Vec<_> = result
            .issues
            .iter()
            .filter(|i| i.description.contains("complexity"))
            .collect();
        assert!(!complexity_issues.is_empty());
    }

    #[test]
    fn allocation_detection() {
        let analyzer = CodeAnalyzer::with_thresholds(100, 1, 1000);
        let result = analyzer.analyze_file("test.rs", SAMPLE_RS).unwrap();
        let alloc_issues: Vec<_> = result
            .issues
            .iter()
            .filter(|i| i.description.contains("allocation"))
            .collect();
        assert!(!alloc_issues.is_empty());
    }

    #[test]
    fn find_hot_functions_by_name() {
        let analyzer = CodeAnalyzer::new();
        let result = analyzer.analyze_file("test.rs", SAMPLE_RS).unwrap();
        let hot = analyzer.find_hot_functions(&[result], &["simple_add"]);
        assert_eq!(hot.len(), 1);
        assert_eq!(hot[0].name, "simple_add");
    }

    #[test]
    fn analyze_crate_multiple_files() {
        let analyzer = CodeAnalyzer::new();
        let file_a = "pub fn a() { }";
        let file_b = "pub fn b() { }";
        let results = analyzer
            .analyze_crate(&[("a.rs", file_a), ("b.rs", file_b)])
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn analysis_result_serialization() {
        let result = AnalysisResult {
            file_path: "src/lib.rs".into(),
            functions: vec![FunctionAnalysis {
                name: "test".into(),
                line_start: 1,
                line_end: 5,
                complexity: 3,
                allocations_estimated: 1,
            }],
            issues: vec![],
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: AnalysisResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.file_path, "src/lib.rs");
        assert_eq!(back.functions.len(), 1);
    }

    #[test]
    fn parse_fn_name_variants() {
        assert_eq!(
            CodeAnalyzer::parse_fn_name("pub fn foo(x: i32) -> i32 {"),
            Some("foo".into())
        );
        assert_eq!(
            CodeAnalyzer::parse_fn_name("fn bar() {"),
            Some("bar".into())
        );
        assert_eq!(
            CodeAnalyzer::parse_fn_name("pub(crate) fn baz() {"),
            Some("baz".into())
        );
        assert_eq!(
            CodeAnalyzer::parse_fn_name("async fn async_fn() {"),
            Some("async_fn".into())
        );
        // Not a function
        assert_eq!(CodeAnalyzer::parse_fn_name("// fn comment() {"), None);
        assert_eq!(CodeAnalyzer::parse_fn_name("let fn_name = 5;"), None);
    }
}
