//! Document chunking for RAG ingestion — splits text into overlapping chunks.

use serde::{Deserialize, Serialize};

const DEFAULT_CHUNK_SIZE: usize = 512;
const DEFAULT_CHUNK_OVERLAP: usize = 64;

/// Supported document formats for chunking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SupportedFormat {
    PlainText,
    Markdown,
    Code,
}

impl std::fmt::Display for SupportedFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SupportedFormat::PlainText => write!(f, "PlainText"),
            SupportedFormat::Markdown => write!(f, "Markdown"),
            SupportedFormat::Code => write!(f, "Code"),
        }
    }
}

/// A single chunk of a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub content: String,
    pub index: usize,
    pub start_char: usize,
    pub end_char: usize,
}

/// Split `content` into overlapping chunks of approximately `DEFAULT_CHUNK_SIZE` characters.
pub fn chunk_file(content: &str, _format: SupportedFormat) -> Vec<DocumentChunk> {
    if content.is_empty() {
        return Vec::new();
    }

    let chars: Vec<char> = content.chars().collect();
    let total = chars.len();
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut index = 0;

    while start < total {
        let end = (start + DEFAULT_CHUNK_SIZE).min(total);
        let chunk_text: String = chars[start..end].iter().collect();
        chunks.push(DocumentChunk {
            content: chunk_text,
            index,
            start_char: start,
            end_char: end,
        });
        index += 1;
        let next_start = start + DEFAULT_CHUNK_SIZE - DEFAULT_CHUNK_OVERLAP;
        if next_start <= start || end == total {
            break;
        }
        start = next_start;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_empty() {
        let chunks = chunk_file("", SupportedFormat::PlainText);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_small_text() {
        let text = "Hello, world!";
        let chunks = chunk_file(text, SupportedFormat::Markdown);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, text);
        assert_eq!(chunks[0].index, 0);
    }

    #[test]
    fn test_chunk_large_text() {
        let text = "a".repeat(1200);
        let chunks = chunk_file(&text, SupportedFormat::Code);
        assert!(chunks.len() > 1);
        // Verify overlap: second chunk starts before first chunk ends
        if chunks.len() >= 2 {
            assert!(chunks[1].start_char < chunks[0].end_char);
        }
    }

    #[test]
    fn test_chunk_indices_sequential() {
        let text = "word ".repeat(200);
        let chunks = chunk_file(&text, SupportedFormat::PlainText);
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.index, i);
        }
    }
}
