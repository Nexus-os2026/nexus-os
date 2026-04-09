//! Phase 1.5 Group A — ground truth parser.
//!
//! Parses the hand-maintained `docs/qa/*_ground_truth_v1.md` files into
//! structured [`GroundTruthEntry`] records consumed by the comparison
//! harness. Page-agnostic: the grammar is the `GT-NNN: Title` section
//! header at column 0, followed by indented `Label:` field lines and
//! further-indented continuation lines.

use serde::{Deserialize, Serialize};

/// A single ground-truth entry parsed from a sealed QA doc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundTruthEntry {
    /// The GT-NNN identifier (e.g. "GT-001").
    pub id: String,
    /// Everything after "GT-NNN: " on the section header line.
    pub title: String,
    pub where_location: String,
    pub symptom: String,
    pub expected: String,
    /// Preserved for human reference. Never included in scout-visible prompts.
    #[serde(skip)]
    pub hypothesis: String,
    /// Derived from the title line. Empty for most entries. For
    /// GT-006 (and anything else whose title contains "verified
    /// working"), this is set to "verified working".
    pub status: String,
}

/// Result of parsing a ground-truth file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    pub entries: Vec<GroundTruthEntry>,
    /// Non-fatal warnings (e.g. a section with no recognised fields).
    pub warnings: Vec<String>,
}

/// Parser error type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParseError {
    IoError(String),
    NoEntriesFound,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::IoError(msg) => write!(f, "io error: {msg}"),
            ParseError::NoEntriesFound => {
                write!(f, "no GT-NNN entries found in ground truth file")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Returns true if `line` starts a new `GT-NNN:` section at column 0.
fn is_section_header(line: &str) -> bool {
    if !line.starts_with("GT-") {
        return false;
    }
    let Some(colon_pos) = line.find(':') else {
        return false;
    };
    let left = &line[..colon_pos];
    // "GT-" + digits, digits non-empty
    let digits = &left[3..];
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

/// Returns Some(field_name) if `line` is a two-space-indented recognised
/// field label line. Field name returned is lowercased canonical.
fn detect_field_line(line: &str) -> Option<(&'static str, &str)> {
    if !line.starts_with("  ") || line.starts_with("   ") {
        return None;
    }
    let rest = &line[2..];
    for label in ["Where:", "Symptom:", "Expected:", "Hypothesis:"] {
        if let Some(value) = rest.strip_prefix(label) {
            let canonical = match label {
                "Where:" => "where",
                "Symptom:" => "symptom",
                "Expected:" => "expected",
                "Hypothesis:" => "hypothesis",
                _ => unreachable!(),
            };
            return Some((canonical, value.trim()));
        }
    }
    None
}

/// Counts the number of leading space characters.
fn leading_spaces(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

/// Parses a ground-truth markdown file at `path`.
pub fn parse_ground_truth_file(path: &std::path::Path) -> Result<ParseResult, ParseError> {
    let contents = std::fs::read_to_string(path).map_err(|e| ParseError::IoError(e.to_string()))?;

    let mut entries: Vec<GroundTruthEntry> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    let mut current: Option<GroundTruthEntry> = None;
    let mut current_field: Option<&'static str> = None;

    let finalise = |entry: GroundTruthEntry,
                    entries: &mut Vec<GroundTruthEntry>,
                    warnings: &mut Vec<String>| {
        if entry.where_location.is_empty()
            && entry.symptom.is_empty()
            && entry.expected.is_empty()
            && entry.hypothesis.is_empty()
        {
            warnings.push(format!("entry {} has no recognised fields", entry.id));
        }
        entries.push(entry);
    };

    for raw_line in contents.lines() {
        if is_section_header(raw_line) {
            if let Some(prev) = current.take() {
                finalise(prev, &mut entries, &mut warnings);
            }
            let colon_pos = raw_line.find(':').unwrap();
            let id = raw_line[..colon_pos].trim().to_string();
            let title = raw_line[colon_pos + 1..].trim().to_string();
            let status = if title.to_lowercase().contains("verified working") {
                "verified working".to_string()
            } else {
                String::new()
            };
            current = Some(GroundTruthEntry {
                id,
                title,
                where_location: String::new(),
                symptom: String::new(),
                expected: String::new(),
                hypothesis: String::new(),
                status,
            });
            current_field = None;
            continue;
        }

        let Some(entry) = current.as_mut() else {
            continue;
        };

        if let Some((field, value)) = detect_field_line(raw_line) {
            current_field = Some(field);
            let target = match field {
                "where" => &mut entry.where_location,
                "symptom" => &mut entry.symptom,
                "expected" => &mut entry.expected,
                "hypothesis" => &mut entry.hypothesis,
                _ => unreachable!(),
            };
            *target = value.to_string();
            continue;
        }

        // Continuation line: indentation > 2 spaces and not a new field.
        if leading_spaces(raw_line) > 2 {
            if let Some(field) = current_field {
                let target = match field {
                    "where" => &mut entry.where_location,
                    "symptom" => &mut entry.symptom,
                    "expected" => &mut entry.expected,
                    "hypothesis" => &mut entry.hypothesis,
                    _ => unreachable!(),
                };
                let trimmed = raw_line.trim();
                if !trimmed.is_empty() {
                    if target.is_empty() {
                        target.push_str(trimmed);
                    } else {
                        target.push(' ');
                        target.push_str(trimmed);
                    }
                }
            }
            continue;
        }

        // Any other line (blank, comment, heading) terminates the current
        // field's continuation context but does not finalise the entry.
        if raw_line.trim().is_empty() {
            current_field = None;
        }
    }

    if let Some(prev) = current.take() {
        finalise(prev, &mut entries, &mut warnings);
    }

    if entries.is_empty() {
        return Err(ParseError::NoEntriesFound);
    }

    Ok(ParseResult { entries, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sg2_parse_chat_ground_truth_doc() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let doc_path = manifest_dir
            .join("../..")
            .join("docs/qa/chat_page_ground_truth_v1.md");

        let result = parse_ground_truth_file(&doc_path)
            .expect("sg2: parser must succeed on the sealed Chat ground truth doc");

        assert_eq!(
            result.entries.len(),
            9,
            "sg2: expected 9 GT entries, got {}. If the sealed doc changed, update this and document why.",
            result.entries.len()
        );

        for entry in &result.entries {
            assert!(
                entry.id.starts_with("GT-"),
                "sg2: entry id '{}' does not start with GT-",
                entry.id
            );
            assert!(
                !entry.title.is_empty(),
                "sg2: entry {} has empty title",
                entry.id
            );
        }

        let gt006 = result
            .entries
            .iter()
            .find(|e| e.id == "GT-006")
            .expect("sg2: GT-006 must be present");
        assert!(
            gt006.title.to_lowercase().contains("working"),
            "sg2: GT-006 title '{}' must contain 'working' — it is the verified baseline entry",
            gt006.title
        );
        assert_eq!(
            gt006.status, "verified working",
            "sg2: GT-006 status must be derived as 'verified working' from its title"
        );

        let json =
            serde_json::to_string(&result.entries[0]).expect("sg2: entry must serialise to JSON");
        assert!(
            !json.contains("\"hypothesis\""),
            "sg2: hypothesis must not appear in serialised JSON (it has #[serde(skip)])"
        );
    }

    #[test]
    fn test_is_section_header() {
        assert!(is_section_header("GT-001: foo"));
        assert!(is_section_header("GT-042: bar baz"));
        assert!(!is_section_header("  GT-001: foo"));
        assert!(!is_section_header("GT-: foo"));
        assert!(!is_section_header("GT-abc: foo"));
        assert!(!is_section_header("## Confirmed bugs"));
    }

    #[test]
    fn test_detect_field_line() {
        assert_eq!(
            detect_field_line("  Where: somewhere"),
            Some(("where", "somewhere"))
        );
        assert_eq!(
            detect_field_line("  Symptom: bad"),
            Some(("symptom", "bad"))
        );
        assert_eq!(detect_field_line("   Where: nope"), None);
        assert_eq!(detect_field_line("Where: nope"), None);
        assert_eq!(detect_field_line("  Other: nope"), None);
    }

    #[test]
    fn test_parser_handles_missing_file() {
        let path = std::path::PathBuf::from("/nonexistent/path/to/file.md");
        let err = parse_ground_truth_file(&path).unwrap_err();
        assert!(matches!(err, ParseError::IoError(_)));
    }
}
