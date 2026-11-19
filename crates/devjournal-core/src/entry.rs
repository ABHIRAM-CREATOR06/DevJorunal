use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Session,
    Note,
    Decision,
    Architecture,
    Research,
    Debugging,
    Issue,
    Task,
    Meeting,
    Retrospective,
    Convention,
    Question,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Note => "note",
            Self::Decision => "decision",
            Self::Architecture => "architecture",
            Self::Research => "research",
            Self::Debugging => "debugging",
            Self::Issue => "issue",
            Self::Task => "task",
            Self::Meeting => "meeting",
            Self::Retrospective => "retrospective",
            Self::Convention => "convention",
            Self::Question => "question",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Author {
    pub kind: AuthorType,
    pub name: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorType {
    Human,
    Ai,
    System,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntrySource {
    pub kind: EntrySourceType,
    pub detail: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntrySourceType {
    Manual,
    Agent,
    Imported,
    System,
}

impl EntrySourceType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Agent => "agent",
            Self::Imported => "imported",
            Self::System => "system",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub category: Category,
    pub content: String,
    pub author: Author,
    pub source: EntrySource,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<String>,
    pub related: Vec<String>,
    pub status: Option<String>,
}

impl Entry {
    pub fn new(
        title: impl Into<String>,
        category: Category,
        content: impl Into<String>,
        author: Author,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: format!("dj-{}", Uuid::new_v4()),
            title: title.into(),
            category,
            content: content.into(),
            author,
            source: EntrySource {
                kind: EntrySourceType::Manual,
                detail: None,
            },
            created_at: now,
            updated_at: now,
            tags: vec![],
            related: vec![],
            status: None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty() {
            return Err(Error::Invalid("entry title cannot be empty".into()));
        }
        if self.author.name.trim().is_empty() {
            return Err(Error::Invalid("author name cannot be empty".into()));
        }
        if self.id.len() < 4
            || self.id.len() > 80
            || !self.id.starts_with("dj-")
            || !self.id[3..]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(Error::Invalid(
                "entry id must use the dj-<letters, digits, hyphens> format".into(),
            ));
        }
        if self.tags.iter().any(|tag| tag.trim().is_empty()) {
            return Err(Error::Invalid("tags cannot be empty".into()));
        }
        for related_id in &self.related {
            crate::relationship::validate_entry_id(related_id)?;
        }
        if self.updated_at < self.created_at {
            return Err(Error::Invalid(
                "updated_at cannot precede created_at".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn to_markdown(&self) -> Result<String> {
        self.validate()?;
        let metadata = serde_json::to_string(self).map_err(|e| Error::Invalid(e.to_string()))?;
        Ok(format!("---\n{metadata}\n---\n\n{}\n", self.content))
    }

    pub(crate) fn from_markdown(text: &str) -> Result<Self> {
        let rest = text
            .strip_prefix("---\n")
            .ok_or_else(|| Error::Invalid("missing JSON-compatible YAML front matter".into()))?;
        let (metadata, body) = rest
            .split_once("\n---\n\n")
            .ok_or_else(|| Error::Invalid("unterminated front matter".into()))?;
        let mut entry: Self = serde_json::from_str(metadata)
            .map_err(|e| Error::Invalid(format!("invalid entry metadata: {e}")))?;
        entry.content = body.strip_suffix('\n').unwrap_or(body).to_owned();
        entry.validate()?;
        Ok(entry)
    }
}
