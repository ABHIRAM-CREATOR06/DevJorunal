use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Author, Entry, EntrySource, Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipType {
    RelatesTo,
    Implements,
    DependsOn,
    Supersedes,
    DiscoveredDuring,
    CausedBy,
    Resolves,
    References,
    DerivedFrom,
    Blocks,
}

impl RelationshipType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RelatesTo => "relates_to",
            Self::Implements => "implements",
            Self::DependsOn => "depends_on",
            Self::Supersedes => "supersedes",
            Self::DiscoveredDuring => "discovered_during",
            Self::CausedBy => "caused_by",
            Self::Resolves => "resolves",
            Self::References => "references",
            Self::DerivedFrom => "derived_from",
            Self::Blocks => "blocks",
        }
    }

    pub fn is_symmetric(self) -> bool {
        self == Self::RelatesTo
    }
}

impl std::str::FromStr for RelationshipType {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "relates_to" => Ok(Self::RelatesTo),
            "implements" => Ok(Self::Implements),
            "depends_on" => Ok(Self::DependsOn),
            "supersedes" => Ok(Self::Supersedes),
            "discovered_during" => Ok(Self::DiscoveredDuring),
            "caused_by" => Ok(Self::CausedBy),
            "resolves" => Ok(Self::Resolves),
            "references" => Ok(Self::References),
            "derived_from" => Ok(Self::DerivedFrom),
            "blocks" => Ok(Self::Blocks),
            _ => Err(Error::Invalid(format!(
                "unknown relationship type: {value}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relationship {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    #[serde(rename = "type")]
    pub kind: RelationshipType,
    pub author: Author,
    pub provenance: EntrySource,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl Relationship {
    pub fn new(
        source_id: impl Into<String>,
        target_id: impl Into<String>,
        kind: RelationshipType,
        author: Author,
        provenance: EntrySource,
    ) -> Self {
        Self {
            id: format!("djr-{}", Uuid::new_v4()),
            source_id: source_id.into(),
            target_id: target_id.into(),
            kind,
            author,
            provenance,
            created_at: Utc::now(),
            metadata: BTreeMap::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        validate_relationship_id(&self.id)?;
        validate_entry_id(&self.source_id)?;
        validate_entry_id(&self.target_id)?;
        if self.source_id == self.target_id {
            return Err(Error::Invalid(
                "self-referencing relationships are not allowed".into(),
            ));
        }
        if self.author.name.trim().is_empty() {
            return Err(Error::Invalid("relationship author cannot be empty".into()));
        }
        Ok(())
    }

    pub(crate) fn to_markdown(&self) -> Result<String> {
        self.validate()?;
        let metadata =
            serde_json::to_string(self).map_err(|error| Error::Invalid(error.to_string()))?;
        Ok(format!("---\n{metadata}\n---\n\n"))
    }

    pub(crate) fn from_markdown(text: &str) -> Result<Self> {
        let metadata = text
            .strip_prefix("---\n")
            .and_then(|rest| rest.split_once("\n---\n").map(|(json, _)| json))
            .ok_or_else(|| Error::Invalid("invalid relationship front matter".into()))?;
        let relation: Self = serde_json::from_str(metadata)
            .map_err(|error| Error::Invalid(format!("invalid relationship: {error}")))?;
        relation.validate()?;
        Ok(relation)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipRemoval {
    pub id: String,
    pub relationship_id: String,
    pub author: Author,
    pub provenance: EntrySource,
    pub removed_at: DateTime<Utc>,
}

impl RelationshipRemoval {
    pub fn new(
        relationship_id: impl Into<String>,
        author: Author,
        provenance: EntrySource,
    ) -> Self {
        Self {
            id: format!("djr-{}", Uuid::new_v4()),
            relationship_id: relationship_id.into(),
            author,
            provenance,
            removed_at: Utc::now(),
        }
    }

    pub(crate) fn to_markdown(&self) -> Result<String> {
        validate_relationship_id(&self.id)?;
        validate_relationship_id(&self.relationship_id)?;
        if self.author.name.trim().is_empty() {
            return Err(Error::Invalid(
                "relationship removal author cannot be empty".into(),
            ));
        }
        let metadata =
            serde_json::to_string(self).map_err(|error| Error::Invalid(error.to_string()))?;
        Ok(format!("---\n{metadata}\n---\n\n"))
    }

    pub(crate) fn from_markdown(text: &str) -> Result<Self> {
        let metadata = text
            .strip_prefix("---\n")
            .and_then(|rest| rest.split_once("\n---\n").map(|(json, _)| json))
            .ok_or_else(|| Error::Invalid("invalid relationship-removal front matter".into()))?;
        let removal: Self = serde_json::from_str(metadata)
            .map_err(|error| Error::Invalid(format!("invalid relationship removal: {error}")))?;
        validate_relationship_id(&removal.id)?;
        validate_relationship_id(&removal.relationship_id)?;
        if removal.author.name.trim().is_empty() {
            return Err(Error::Invalid(
                "relationship removal author cannot be empty".into(),
            ));
        }
        Ok(removal)
    }
}

pub(crate) fn validate_entry_id(id: &str) -> Result<()> {
    if id.len() < 4
        || id.len() > 80
        || !id.starts_with("dj-")
        || !id[3..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(Error::Invalid(format!("invalid entry id: {id}")));
    }
    Ok(())
}

pub(crate) fn validate_relationship_id(id: &str) -> Result<()> {
    if id.len() < 5
        || id.len() > 90
        || !id.starts_with("djr-")
        || !id[4..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(Error::Invalid(format!("invalid relationship id: {id}")));
    }
    Ok(())
}

pub(crate) fn legacy_relationship(entry: &Entry, target_id: &str) -> Relationship {
    Relationship {
        id: format!("legacy-{}-{target_id}", entry.id),
        source_id: entry.id.clone(),
        target_id: target_id.to_owned(),
        kind: RelationshipType::References,
        author: entry.author.clone(),
        provenance: entry.source.clone(),
        created_at: entry.created_at,
        metadata: BTreeMap::new(),
    }
}
