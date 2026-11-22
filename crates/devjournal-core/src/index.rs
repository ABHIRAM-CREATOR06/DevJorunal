use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    time::Duration,
};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, Transaction, params};
use serde::Serialize;
use uuid::Uuid;

use crate::{Category, Entry, Error, Journal, KnowledgeGraph, Relationship, Result};

pub struct SearchIndex {
    connection: Connection,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct IndexStatus {
    pub available: bool,
    pub entries: usize,
    pub relationships: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct IndexVerification {
    pub consistent: bool,
    pub missing_entries: Vec<String>,
    pub stale_entries: Vec<String>,
    pub changed_entries: Vec<String>,
    pub missing_relationships: Vec<String>,
    pub stale_relationships: Vec<String>,
    pub changed_relationships: Vec<String>,
    pub missing_fts_entries: Vec<String>,
    pub stale_fts_entries: Vec<String>,
    pub changed_fts_entries: Vec<String>,
    pub fts_ok: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SearchFilters {
    pub category: Option<Category>,
    pub author: Option<String>,
    pub tag: Option<String>,
    pub status: Option<String>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

impl SearchIndex {
    pub fn open(journal: &Journal) -> Result<Self> {
        journal.ensure_index_ignored()?;
        let path = journal.index_path();
        if let Ok(metadata) = fs::symlink_metadata(&path)
            && (metadata.file_type().is_symlink() || !metadata.is_file())
        {
            return Err(Error::Invalid(
                "the journal index path must be a regular file, not a symlink".into(),
            ));
        }
        let connection = Connection::open(&path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA synchronous = FULL;
             CREATE TABLE IF NOT EXISTS entries (
                id TEXT PRIMARY KEY, category TEXT NOT NULL, title TEXT NOT NULL,
                author_name TEXT NOT NULL, author_type TEXT NOT NULL, provenance TEXT NOT NULL,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL, file_path TEXT NOT NULL,
                fingerprint TEXT NOT NULL, tags TEXT NOT NULL, status TEXT
             );
             CREATE INDEX IF NOT EXISTS entries_category ON entries(category);
             CREATE INDEX IF NOT EXISTS entries_author ON entries(author_name);
             CREATE INDEX IF NOT EXISTS entries_created ON entries(created_at);
             CREATE VIRTUAL TABLE IF NOT EXISTS entry_fts USING fts5(
                entry_id UNINDEXED, title, body, tags, tokenize='unicode61'
             );
             CREATE TABLE IF NOT EXISTS relationships (
                id TEXT PRIMARY KEY, source_id TEXT NOT NULL, target_id TEXT NOT NULL,
                kind TEXT NOT NULL, author_name TEXT NOT NULL, author_type TEXT NOT NULL,
                provenance TEXT NOT NULL, created_at TEXT NOT NULL, fingerprint TEXT NOT NULL,
                record_json TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS relationships_source ON relationships(source_id, kind);
             CREATE INDEX IF NOT EXISTS relationships_target ON relationships(target_id, kind);
             PRAGMA user_version = 1;",
        )?;
        Ok(Self { connection })
    }

    pub fn status(journal: &Journal) -> Result<IndexStatus> {
        let path = journal.index_path();
        if !path.exists() {
            return Ok(IndexStatus::default());
        }
        let index = Self::open(journal)?;
        Ok(IndexStatus {
            available: true,
            entries: index
                .connection
                .query_row("SELECT count(*) FROM entries", [], |row| {
                    row.get::<_, i64>(0)
                })? as usize,
            relationships: index.connection.query_row(
                "SELECT count(*) FROM relationships",
                [],
                |row| row.get::<_, i64>(0),
            )? as usize,
        })
    }

    pub fn verify_existing(journal: &Journal) -> Result<IndexVerification> {
        if journal.index_path().exists() {
            return Self::open(journal)?.verify(journal);
        }
        Ok(IndexVerification {
            consistent: false,
            missing_entries: journal
                .entry_files()?
                .into_iter()
                .map(|(entry, _, _)| entry.id)
                .collect(),
            missing_relationships: journal
                .relationships()?
                .into_iter()
                .map(|relation| relation.id)
                .collect(),
            fts_ok: false,
            ..IndexVerification::default()
        })
    }

    pub fn sync(&mut self, journal: &Journal) -> Result<()> {
        let entries = journal.entry_files()?;
        let relationships = journal.relationships()?;
        let tx = self.connection.transaction()?;
        sync_records(&tx, journal, &entries, &relationships, false)?;
        tx.commit()?;
        Ok(())
    }

    pub fn rebuild(journal: &Journal) -> Result<Self> {
        let mut index = match Self::open(journal) {
            Ok(index) if index.integrity_ok().unwrap_or(false) => index,
            Ok(index) => {
                drop(index);
                archive_corrupt(&journal.index_path())?;
                Self::open(journal)?
            }
            Err(error) if journal.index_path().exists() => {
                archive_corrupt(&journal.index_path()).map_err(|archive_error| Error::Index(format!("index open failed ({error}); could not preserve corrupt index ({archive_error})")))?;
                Self::open(journal)?
            }
            Err(error) => return Err(error),
        };
        let entries = journal.entry_files()?;
        let relationships = journal.relationships()?;
        let tx = index.connection.transaction()?;
        tx.execute("DELETE FROM entry_fts", [])?;
        tx.execute("DELETE FROM entries", [])?;
        tx.execute("DELETE FROM relationships", [])?;
        sync_records(&tx, journal, &entries, &relationships, true)?;
        tx.commit()?;
        Ok(index)
    }

    pub fn search_ids(
        &self,
        query: &str,
        category: Option<Category>,
        limit: usize,
    ) -> Result<Vec<String>> {
        self.search_ids_filtered(
            query,
            &SearchFilters {
                category,
                ..SearchFilters::default()
            },
            limit,
        )
    }

    pub fn search_ids_filtered(
        &self,
        query: &str,
        filters: &SearchFilters,
        limit: usize,
    ) -> Result<Vec<String>> {
        if query.trim().is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        if query.len() > 512 || limit > 100 {
            return Err(Error::Invalid(
                "search query cannot exceed 512 bytes and result limit cannot exceed 100".into(),
            ));
        }
        if [
            filters.author.as_deref(),
            filters.tag.as_deref(),
            filters.status.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|value| value.len() > 256)
        {
            return Err(Error::Invalid(
                "search metadata filter values cannot exceed 256 bytes".into(),
            ));
        }
        let expression = fts_expression(query);
        let category = filters.category.map(|value| value.as_str().to_owned());
        let tag = filters
            .tag
            .as_deref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| Error::Index(error.to_string()))?;
        let created_after = filters.created_after.map(|value| value.to_rfc3339());
        let created_before = filters.created_before.map(|value| value.to_rfc3339());
        let mut statement = self.connection.prepare(
            "SELECT entries.id FROM entry_fts
             JOIN entries ON entries.id = entry_fts.entry_id
             WHERE entry_fts MATCH ?1
               AND (?2 IS NULL OR entries.category = ?2)
               AND (?3 IS NULL OR entries.author_name = ?3)
               AND (?4 IS NULL OR instr(entries.tags, ?4) > 0)
               AND (?5 IS NULL OR entries.status = ?5)
               AND (?6 IS NULL OR entries.created_at >= ?6)
               AND (?7 IS NULL OR entries.created_at <= ?7)
             ORDER BY bm25(entry_fts), entries.created_at DESC LIMIT ?8",
        )?;
        let ids = statement
            .query_map(
                params![
                    expression,
                    category,
                    filters.author,
                    tag,
                    filters.status,
                    created_after,
                    created_before,
                    limit as i64
                ],
                |row| row.get(0),
            )?
            .collect::<std::result::Result<Vec<String>, _>>()?;
        Ok(ids)
    }

    pub fn graph(&self) -> Result<KnowledgeGraph> {
        let mut statement = self
            .connection
            .prepare("SELECT id FROM entries ORDER BY id")?;
        let nodes = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut statement = self
            .connection
            .prepare("SELECT record_json FROM relationships ORDER BY id")?;
        let relationships = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|json| {
                serde_json::from_str::<Relationship>(&json?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        KnowledgeGraph::new(nodes, relationships)
    }

    pub fn verify(&self, journal: &Journal) -> Result<IndexVerification> {
        let expected_entries = journal
            .entry_files()?
            .into_iter()
            .map(|(entry, path, bytes)| {
                (
                    entry.id,
                    (fingerprint(&bytes), relative_path(journal, &path)),
                )
            })
            .collect::<HashMap<_, _>>();
        let expected_relationships = journal
            .relationships()?
            .into_iter()
            .map(|relationship| {
                let text = serde_json::to_string(&relationship).unwrap_or_default();
                (relationship.id, fingerprint(text.as_bytes()))
            })
            .collect::<HashMap<_, _>>();
        let actual_entries = read_entry_fingerprints(&self.connection)?;
        let actual_relationships = read_relationship_fingerprints(&self.connection)?;
        let expected_fts = journal
            .entry_files()?
            .into_iter()
            .map(|(entry, _, _)| (entry.id, (entry.title, entry.content, entry.tags.join(" "))))
            .collect::<HashMap<_, _>>();
        let actual_fts = read_fts_records(&self.connection)?;
        let missing_entries = expected_entries
            .keys()
            .filter(|id| !actual_entries.contains_key(*id))
            .cloned()
            .collect();
        let stale_entries = actual_entries
            .keys()
            .filter(|id| !expected_entries.contains_key(*id))
            .cloned()
            .collect();
        let changed_entries = expected_entries
            .iter()
            .filter_map(|(id, expected)| {
                actual_entries
                    .get(id)
                    .filter(|actual| *actual != expected)
                    .map(|_| id.clone())
            })
            .collect();
        let missing_relationships = expected_relationships
            .keys()
            .filter(|id| !actual_relationships.contains_key(*id))
            .cloned()
            .collect();
        let stale_relationships = actual_relationships
            .keys()
            .filter(|id| !expected_relationships.contains_key(*id))
            .cloned()
            .collect();
        let changed_relationships = expected_relationships
            .iter()
            .filter_map(|(id, expected)| {
                actual_relationships
                    .get(id)
                    .filter(|actual| *actual != expected)
                    .map(|_| id.clone())
            })
            .collect();
        let missing_fts_entries = expected_fts
            .keys()
            .filter(|id| !actual_fts.contains_key(*id))
            .cloned()
            .collect();
        let stale_fts_entries = actual_fts
            .keys()
            .filter(|id| !expected_fts.contains_key(*id))
            .cloned()
            .collect();
        let changed_fts_entries = expected_fts
            .iter()
            .filter_map(|(id, expected)| {
                actual_fts
                    .get(id)
                    .filter(|actual| *actual != expected)
                    .map(|_| id.clone())
            })
            .collect();
        let fts_ok = self.fts_integrity_ok();
        let mut report = IndexVerification {
            fts_ok,
            ..IndexVerification::default()
        };
        report.missing_entries = missing_entries;
        report.stale_entries = stale_entries;
        report.changed_entries = changed_entries;
        report.missing_relationships = missing_relationships;
        report.stale_relationships = stale_relationships;
        report.changed_relationships = changed_relationships;
        report.missing_fts_entries = missing_fts_entries;
        report.stale_fts_entries = stale_fts_entries;
        report.changed_fts_entries = changed_fts_entries;
        report.consistent = report.missing_entries.is_empty()
            && report.stale_entries.is_empty()
            && report.changed_entries.is_empty()
            && report.missing_relationships.is_empty()
            && report.stale_relationships.is_empty()
            && report.changed_relationships.is_empty()
            && report.missing_fts_entries.is_empty()
            && report.stale_fts_entries.is_empty()
            && report.changed_fts_entries.is_empty()
            && report.fts_ok;
        Ok(report)
    }

    fn integrity_ok(&self) -> Result<bool> {
        let quick_check: String = self
            .connection
            .query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        Ok(quick_check == "ok" && self.fts_integrity_ok())
    }

    fn fts_integrity_ok(&self) -> bool {
        self.connection
            .execute(
                "INSERT INTO entry_fts(entry_fts) VALUES('integrity-check')",
                [],
            )
            .is_ok()
    }
}

fn sync_records(
    tx: &Transaction<'_>,
    journal: &Journal,
    entries: &[(Entry, PathBuf, Vec<u8>)],
    relationships: &[Relationship],
    force: bool,
) -> Result<()> {
    let existing_entries = read_entry_fingerprints(tx)?;
    let expected_ids: HashSet<_> = entries
        .iter()
        .map(|(entry, _, _)| entry.id.as_str())
        .collect();
    for (entry, path, bytes) in entries {
        let hash = fingerprint(bytes);
        let rel_path = relative_path(journal, path);
        let unchanged = !force
            && existing_entries
                .get(&entry.id)
                .is_some_and(|(old_hash, old_path)| old_hash == &hash && old_path == &rel_path);
        if unchanged {
            continue;
        }
        tx.execute(
            "INSERT INTO entries(id,category,title,author_name,author_type,provenance,created_at,updated_at,file_path,fingerprint,tags,status)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
             ON CONFLICT(id) DO UPDATE SET category=excluded.category,title=excluded.title,author_name=excluded.author_name,author_type=excluded.author_type,provenance=excluded.provenance,created_at=excluded.created_at,updated_at=excluded.updated_at,file_path=excluded.file_path,fingerprint=excluded.fingerprint,tags=excluded.tags,status=excluded.status",
            params![entry.id, entry.category.as_str(), entry.title, entry.author.name, author_type(entry.author.kind), entry.source.kind.as_str(), entry.created_at.to_rfc3339(), entry.updated_at.to_rfc3339(), rel_path, hash, serde_json::to_string(&entry.tags).unwrap_or_default(), entry.status],
        )?;
        tx.execute("DELETE FROM entry_fts WHERE entry_id=?1", [&entry.id])?;
        tx.execute(
            "INSERT INTO entry_fts(entry_id,title,body,tags) VALUES(?1,?2,?3,?4)",
            params![entry.id, entry.title, entry.content, entry.tags.join(" ")],
        )?;
    }
    for id in existing_entries
        .keys()
        .filter(|id| !expected_ids.contains(id.as_str()))
    {
        tx.execute("DELETE FROM entry_fts WHERE entry_id=?1", [id])?;
        tx.execute("DELETE FROM entries WHERE id=?1", [id])?;
    }

    let existing_relationships = read_relationship_fingerprints(tx)?;
    let expected_relationship_ids: HashSet<_> = relationships
        .iter()
        .map(|relationship| relationship.id.as_str())
        .collect();
    for relationship in relationships {
        let record_json =
            serde_json::to_string(relationship).map_err(|error| Error::Index(error.to_string()))?;
        let hash = fingerprint(record_json.as_bytes());
        if !force && existing_relationships.get(&relationship.id) == Some(&hash) {
            continue;
        }
        tx.execute(
            "INSERT INTO relationships(id,source_id,target_id,kind,author_name,author_type,provenance,created_at,fingerprint,record_json)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(id) DO UPDATE SET source_id=excluded.source_id,target_id=excluded.target_id,kind=excluded.kind,author_name=excluded.author_name,author_type=excluded.author_type,provenance=excluded.provenance,created_at=excluded.created_at,fingerprint=excluded.fingerprint,record_json=excluded.record_json",
            params![relationship.id, relationship.source_id, relationship.target_id, relationship.kind.as_str(), relationship.author.name, author_type(relationship.author.kind), relationship.provenance.kind.as_str(), relationship.created_at.to_rfc3339(), hash, record_json],
        )?;
    }
    for id in existing_relationships
        .keys()
        .filter(|id| !expected_relationship_ids.contains(id.as_str()))
    {
        tx.execute("DELETE FROM relationships WHERE id=?1", [id])?;
    }
    Ok(())
}

fn read_entry_fingerprints(connection: &Connection) -> Result<HashMap<String, (String, String)>> {
    let mut statement = connection.prepare("SELECT id,fingerprint,file_path FROM entries")?;
    Ok(statement
        .query_map([], |row| Ok((row.get(0)?, (row.get(1)?, row.get(2)?))))?
        .collect::<std::result::Result<_, _>>()?)
}

fn read_relationship_fingerprints(connection: &Connection) -> Result<HashMap<String, String>> {
    let mut statement = connection.prepare("SELECT id,fingerprint FROM relationships")?;
    Ok(statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?)
}

fn read_fts_records(connection: &Connection) -> Result<HashMap<String, (String, String, String)>> {
    let mut statement = connection.prepare("SELECT entry_id,title,body,tags FROM entry_fts")?;
    Ok(statement
        .query_map([], |row| {
            Ok((row.get(0)?, (row.get(1)?, row.get(2)?, row.get(3)?)))
        })?
        .collect::<std::result::Result<_, _>>()?)
}

fn relative_path(journal: &Journal, path: &std::path::Path) -> String {
    path.strip_prefix(journal.root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn fingerprint(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn fts_expression(query: &str) -> String {
    query
        .split_whitespace()
        .map(|term| format!("\"{}\"*", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn author_type(author: crate::AuthorType) -> &'static str {
    match author {
        crate::AuthorType::Human => "human",
        crate::AuthorType::Ai => "ai",
        crate::AuthorType::System => "system",
    }
}

fn archive_corrupt(path: &std::path::Path) -> Result<()> {
    if path.exists() {
        let backup = path.with_file_name(format!("index.sqlite3.corrupt-{}", Uuid::new_v4()));
        fs::rename(path, backup)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Author, AuthorType, EntrySource, EntrySourceType, JournalConfig, RelationshipType,
    };

    fn author() -> Author {
        Author {
            kind: AuthorType::Human,
            name: "tester".into(),
        }
    }
    fn provenance() -> EntrySource {
        EntrySource {
            kind: EntrySourceType::Manual,
            detail: None,
        }
    }

    #[test]
    fn rebuild_indexes_markdown_entries_relationships_and_fts() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::init(dir.path(), JournalConfig::defaults("index-test")).unwrap();
        let note = Entry::new(
            "Authentication plan",
            Category::Note,
            "Use local tokens",
            author(),
        );
        let decision = Entry::new(
            "Token decision",
            Category::Decision,
            "Token scopes are limited",
            author(),
        );
        journal.create_entry(&note).unwrap();
        journal.create_entry(&decision).unwrap();
        let relationship = Relationship::new(
            note.id.clone(),
            decision.id.clone(),
            RelationshipType::Implements,
            author(),
            provenance(),
        );
        journal.create_relationship(&relationship).unwrap();

        let index = SearchIndex::rebuild(&journal).unwrap();
        assert_eq!(
            index.search_ids("authentication", None, 10).unwrap(),
            vec![note.id]
        );
        assert!(index.verify(&journal).unwrap().consistent);
        assert_eq!(index.graph().unwrap().stats().relationships, 1);
        assert_eq!(SearchIndex::status(&journal).unwrap().entries, 2);
    }

    #[test]
    fn incremental_sync_detects_manual_edits_and_deleted_entries() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::init(dir.path(), JournalConfig::defaults("incremental")).unwrap();
        let first = Entry::new("Before", Category::Note, "original body", author());
        let second = Entry::new("Remove me", Category::Note, "discard", author());
        let first_path = journal.create_entry(&first).unwrap();
        let second_path = journal.create_entry(&second).unwrap();
        let mut index = SearchIndex::rebuild(&journal).unwrap();
        let changed = fs::read_to_string(&first_path)
            .unwrap()
            .replace("original body", "manually edited phrase");
        fs::write(&first_path, changed).unwrap();
        fs::remove_file(second_path).unwrap();
        index.sync(&journal).unwrap();
        assert_eq!(
            index.search_ids("manually", None, 10).unwrap(),
            vec![first.id]
        );
        assert!(index.search_ids("original", None, 10).unwrap().is_empty());
        assert!(index.verify(&journal).unwrap().consistent);
    }

    #[test]
    fn verification_detects_fts_content_out_of_sync_with_markdown() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::init(dir.path(), JournalConfig::defaults("fts-verify")).unwrap();
        let entry = Entry::new("Indexed note", Category::Note, "searchable body", author());
        journal.create_entry(&entry).unwrap();
        let index = SearchIndex::rebuild(&journal).unwrap();
        index
            .connection
            .execute("DELETE FROM entry_fts WHERE entry_id = ?1", [&entry.id])
            .unwrap();

        let report = index.verify(&journal).unwrap();
        assert!(!report.consistent);
        assert_eq!(report.missing_fts_entries, vec![entry.id]);
    }

    #[test]
    fn deleting_the_disposable_database_and_rebuilding_recovers_it() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::init(dir.path(), JournalConfig::defaults("rebuild")).unwrap();
        let note = Entry::new("Recovery", Category::Note, "restore index", author());
        journal.create_entry(&note).unwrap();
        drop(SearchIndex::rebuild(&journal).unwrap());
        fs::remove_file(journal.index_path()).unwrap();
        assert!(!SearchIndex::status(&journal).unwrap().available);
        let rebuilt = SearchIndex::rebuild(&journal).unwrap();
        assert_eq!(
            rebuilt.search_ids("restore", None, 10).unwrap(),
            vec![note.id]
        );
        assert!(rebuilt.verify(&journal).unwrap().consistent);
    }

    #[test]
    fn corrupted_disposable_database_is_preserved_and_rebuilt() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::init(dir.path(), JournalConfig::defaults("corrupt")).unwrap();
        let entry = Entry::new(
            "Recover index",
            Category::Note,
            "markdown survives corruption",
            author(),
        );
        journal.create_entry(&entry).unwrap();
        fs::write(journal.index_path(), b"not a sqlite database").unwrap();
        let rebuilt = SearchIndex::rebuild(&journal).unwrap();
        assert!(rebuilt.verify(&journal).unwrap().consistent);
        assert!(
            rebuilt
                .search_ids("survives", None, 10)
                .unwrap()
                .contains(&entry.id)
        );
        assert!(fs::read_dir(journal.root()).unwrap().any(|item| {
            item.unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".corrupt-")
        }));
    }
}
