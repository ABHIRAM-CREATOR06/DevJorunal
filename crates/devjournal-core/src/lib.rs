mod config;
mod context;
mod entry;
mod graph;
mod index;
mod journal;
mod relationship;

pub use config::{JournalConfig, ProjectConfig};
pub use context::{ContextItem, ContextReason, retrieve_context};
pub use entry::{Author, AuthorType, Category, Entry, EntrySource, EntrySourceType};
pub use graph::{GraphStats, KnowledgeGraph, TraversalHit};
pub use index::{IndexStatus, IndexVerification, SearchIndex};
pub use journal::{Journal, RecoveryReport};
pub use relationship::{Relationship, RelationshipRemoval, RelationshipType};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid journal data: {0}")]
    Invalid(String),
    #[error("entry not found: {0}")]
    NotFound(String),
    #[error("entry already exists: {0}")]
    AlreadyExists(String),
    #[error("index error: {0}")]
    Index(String),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
