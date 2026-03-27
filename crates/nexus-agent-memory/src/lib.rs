pub mod consolidation;
pub mod context;
pub mod economy;
pub mod governance;
pub mod index;
pub mod persistence;
pub mod store;
pub mod tauri_commands;
pub mod types;

pub use consolidation::{MemoryConsolidator, MergeCandidate};
pub use context::{ContextBuilder, ContextEntry, MemoryContext};
pub use governance::{MemoryPolicy, MEMORY_CAPABILITY};
pub use index::MemoryIndex;
pub use persistence::MemoryPersistence;
pub use store::AgentMemoryStore;
pub use tauri_commands::MemoryState;
pub use types::{Memory, MemoryContent, MemoryMetadata, MemoryQuery, MemoryType, Valence};
