//! Markdown to ratatui Lines conversion.
//! Supports: **bold**, `inline code`, ```code blocks```, # headers, - lists.

use super::theme::Theme;
use ratatui::text::{Line, Span};

/// Render markdown text into styled ratatui Lines.
pub fn render_markdown(text: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut in_code_block = false;

    for line in text.lines() {
        if line.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            lines.push(Line::from(Span::styled(
                format!("  {}", line),
                Theme::code(),
            )));
            continue;
        }

        // Headers
        if let Some(rest) = line.strip_prefix("### ") {
            lines.push(Line::from(Span::styled(rest.to_string(), Theme::bold())));
            continue;
        }
        if let Some(rest) = line.strip_prefix("## ") {
            lines.push(Line::from(Span::styled(rest.to_string(), Theme::title())));
            continue;
        }
        if let Some(rest) = line.strip_prefix("# ") {
            lines.push(Line::from(Span::styled(rest.to_string(), Theme::title())));
            continue;
        }

        // List items
        if line.starts_with("- ") || line.starts_with("* ") {
            lines.push(Line::from(vec![
                Span::styled("  \u{2022} ", Theme::dim()),
                Span::raw(line[2..].to_string()),
            ]));
            continue;
        }

        // Inline formatting
        lines.push(render_inline(line));
    }

    lines
}

/// Render inline markdown: **bold**, `code`.
fn render_inline(text: &str) -> Line<'static> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // `inline code`
        if chars[i] == '`' {
            if !current.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut current)));
            }
            i += 1;
            let mut code = String::new();
            while i < chars.len() && chars[i] != '`' {
                code.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                i += 1;
            }
            spans.push(Span::styled(code, Theme::code()));
            continue;
        }

        // **bold**
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if !current.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut current)));
            }
            i += 2;
            let mut bold_text = String::new();
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '*') {
                bold_text.push(chars[i]);
                i += 1;
            }
            if i + 1 < chars.len() {
                i += 2;
            }
            spans.push(Span::styled(bold_text, Theme::bold()));
            continue;
        }

        current.push(chars[i]);
        i += 1;
    }

    if !current.is_empty() {
        spans.push(Span::raw(current));
    }

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_plain_text() {
        let lines = render_markdown("Hello world");
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_render_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let lines = render_markdown(md);
        assert_eq!(lines.len(), 1); // Just the code line, ``` markers are consumed
    }

    #[test]
    fn test_render_header() {
        let lines = render_markdown("# Title");
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_render_list() {
        let lines = render_markdown("- item 1\n- item 2");
        assert_eq!(lines.len(), 2);
    }
}
