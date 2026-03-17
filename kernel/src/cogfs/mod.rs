//! Cognitive Filesystem — every file gets a semantic understanding layer.
//!
//! The filesystem IS the knowledge graph. Files are indexed, linked, and
//! queryable by meaning, not just name.
//!
//! ## Subsystems
//!
//! - **indexer** — extracts semantics from files: word frequencies, entities, topics.
//! - **graph** — auto-links files into a knowledge graph by shared semantics.
//! - **query** — natural language queries over the indexed corpus.
//! - **watcher** — watches directories and re-indexes on change.
//! - **context** — builds rich context packages for agent system prompts.

pub mod context;
pub mod graph;
pub mod indexer;
pub mod query;
pub mod watcher;

pub use context::{ContextBuilder, ContextPackage};
pub use graph::{GraphLink, KnowledgeGraph, LinkType};
pub use indexer::{IndexedFile, SemanticIndexer};
pub use query::{NaturalQuery, QueryResult};
pub use watcher::{FileWatcher, WatchConfig};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors produced by the cognitive filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum CogFsError {
    #[error("file not found: {0}")]
    FileNotFound(String),
    #[error("file not indexed: {0}")]
    FileNotIndexed(String),
    #[error("unsupported file type: {0}")]
    UnsupportedFileType(String),
    #[error("index error: {0}")]
    IndexError(String),
    #[error("watch error: {0}")]
    WatchError(String),
    #[error("query error: {0}")]
    QueryError(String),
}
