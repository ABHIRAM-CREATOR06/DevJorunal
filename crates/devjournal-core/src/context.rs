use serde::Serialize;

use crate::{Entry, Error, Journal, RelationshipType, Result, SearchIndex};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContextReason {
    DirectTextMatch,
    RequestedEntry,
    RecentProjectActivity,
    Relationship {
        relationship_id: String,
        relationship_type: RelationshipType,
        from_entry_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ContextItem {
    pub entry: Entry,
    pub depth: usize,
    pub reason: ContextReason,
}

pub fn retrieve_context(
    journal: &Journal,
    topic: Option<&str>,
    entry_id: Option<&str>,
    depth: usize,
    limit: usize,
) -> Result<Vec<ContextItem>> {
    if depth > 8 {
        return Err(Error::Invalid("context depth cannot exceed 8".into()));
    }
    if limit > 100 {
        return Err(Error::Invalid("context limit cannot exceed 100".into()));
    }
    if let Some(id) = entry_id {
        let mut index = SearchIndex::open(journal)?;
        index.sync(journal)?;
        let graph = index.graph()?;
        let requested = journal.get_entry(id)?;
        let mut results = vec![ContextItem {
            entry: requested,
            depth: 0,
            reason: ContextReason::RequestedEntry,
        }];
        if depth == 0 {
            results.truncate(limit);
            return Ok(results);
        }
        for hit in graph.neighbors(id, depth, None)? {
            if results.len() >= limit {
                break;
            }
            results.push(ContextItem {
                entry: journal.get_entry(&hit.entry_id)?,
                depth: hit.depth,
                reason: ContextReason::Relationship {
                    relationship_id: hit.relationship_id,
                    relationship_type: hit.relationship_type,
                    from_entry_id: hit.from_id,
                },
            });
        }
        results.truncate(limit);
        return Ok(results);
    }
    if let Some(topic) = topic.filter(|topic| !topic.trim().is_empty()) {
        let mut index = SearchIndex::open(journal)?;
        index.sync(journal)?;
        let graph = index.graph()?;
        let direct_ids = index.search_ids(topic, None, limit.min(20))?;
        let mut seen = std::collections::HashSet::new();
        let mut results = Vec::new();
        for id in &direct_ids {
            if seen.insert(id.clone()) {
                results.push(ContextItem {
                    entry: journal.get_entry(id)?,
                    depth: 0,
                    reason: ContextReason::DirectTextMatch,
                });
            }
        }
        if depth > 0 {
            for id in direct_ids {
                for hit in graph.neighbors(&id, depth, None)? {
                    if results.len() >= limit {
                        break;
                    }
                    if seen.insert(hit.entry_id.clone()) {
                        results.push(ContextItem {
                            entry: journal.get_entry(&hit.entry_id)?,
                            depth: hit.depth,
                            reason: ContextReason::Relationship {
                                relationship_id: hit.relationship_id,
                                relationship_type: hit.relationship_type,
                                from_entry_id: hit.from_id,
                            },
                        });
                    }
                }
                if results.len() >= limit {
                    break;
                }
            }
        }
        results.truncate(limit);
        return Ok(results);
    }

    let mut entries = journal.entries()?;
    entries.reverse();
    entries.truncate(limit.min(10));
    Ok(entries
        .into_iter()
        .map(|entry| ContextItem {
            entry,
            depth: 0,
            reason: ContextReason::RecentProjectActivity,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Author, AuthorType, Category, EntrySource, EntrySourceType, JournalConfig, Relationship,
        RelationshipType,
    };

    #[test]
    fn direct_and_graph_context_are_ranked_deduplicated_and_explained() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::init(dir.path(), JournalConfig::defaults("context")).unwrap();
        let author = Author {
            kind: AuthorType::Ai,
            name: "assistant".into(),
        };
        let provenance = EntrySource {
            kind: EntrySourceType::Agent,
            detail: None,
        };
        let note = Entry::new(
            "Authentication changes",
            Category::Debugging,
            "token expiry issue",
            author.clone(),
        );
        let decision = Entry::new(
            "Token decision",
            Category::Decision,
            "token scopes expire",
            author.clone(),
        );
        let task = Entry::new("Rotate keys", Category::Task, "key rotation", author);
        journal.create_entry(&note).unwrap();
        journal.create_entry(&decision).unwrap();
        journal.create_entry(&task).unwrap();
        journal
            .create_relationship(&Relationship::new(
                note.id.clone(),
                decision.id.clone(),
                RelationshipType::RelatesTo,
                Author {
                    kind: AuthorType::Human,
                    name: "reviewer".into(),
                },
                provenance.clone(),
            ))
            .unwrap();
        journal
            .create_relationship(&Relationship::new(
                decision.id.clone(),
                task.id.clone(),
                RelationshipType::Implements,
                Author {
                    kind: AuthorType::Human,
                    name: "reviewer".into(),
                },
                provenance,
            ))
            .unwrap();

        let results = retrieve_context(&journal, Some("authentication"), None, 2, 10).unwrap();
        assert_eq!(results[0].reason, ContextReason::DirectTextMatch);
        assert_eq!(
            results
                .iter()
                .map(|item| &item.entry.id)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            results.len()
        );
        let task_context = results
            .iter()
            .find(|item| item.entry.id == task.id)
            .unwrap();
        assert_eq!(task_context.depth, 2);
        assert!(matches!(
            task_context.reason,
            ContextReason::Relationship {
                relationship_type: RelationshipType::Implements,
                ..
            }
        ));

        let requested = retrieve_context(&journal, None, Some(&task.id), 1, 10).unwrap();
        assert_eq!(
            requested.first().unwrap().reason,
            ContextReason::RequestedEntry
        );
        assert_eq!(requested.len(), 2);
    }
}
