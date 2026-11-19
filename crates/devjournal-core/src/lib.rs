mod config;
mod entry;
mod journal;

pub use config::{JournalConfig, ProjectConfig};
pub use entry::{Author, AuthorType, Category, Entry, EntrySource, EntrySourceType};
pub use journal::Journal;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid journal data: {0}")]
    Invalid(String),
    #[error("entry not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, Error>;
