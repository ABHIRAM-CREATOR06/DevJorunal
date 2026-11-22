use devjournal_core::{
    Author, AuthorType, Category, ContextReason, Entry, EntrySource, EntrySourceType, Journal,
    JournalConfig, Relationship, RelationshipType, SearchIndex, retrieve_context,
};

fn author() -> Author {
    Author {
        kind: AuthorType::Human,
        name: "test-author".into(),
    }
}

#[test]
fn rebuilt_index_restores_search_and_context_from_markdown_sources() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Journal::init(
        dir.path(),
        JournalConfig::defaults("index-context-contract"),
    )
    .unwrap();
    let decision = Entry::new(
        "Authentication design",
        Category::Decision,
        "Use rotating credentials for authentication",
        author(),
    );
    let session = Entry::new(
        "Credential review session",
        Category::Session,
        "We reviewed the token expiry implementation",
        author(),
    );
    journal.create_entry(&decision).unwrap();
    journal.create_entry(&session).unwrap();
    let relationship = Relationship::new(
        decision.id.clone(),
        session.id.clone(),
        RelationshipType::DiscoveredDuring,
        author(),
        EntrySource {
            kind: EntrySourceType::Manual,
            detail: Some("linked during review".into()),
        },
    );
    journal.create_relationship(&relationship).unwrap();

    let first_index = SearchIndex::rebuild(&journal).unwrap();
    assert!(first_index.verify(&journal).unwrap().consistent);
    let search_results = first_index.search_ids("authentication", None, 10).unwrap();
    assert_eq!(
        search_results.as_slice(),
        std::slice::from_ref(&decision.id)
    );
    drop(first_index);

    std::fs::remove_file(journal.index_path()).unwrap();
    assert!(!SearchIndex::status(&journal).unwrap().available);

    let context = retrieve_context(&journal, Some("authentication"), None, 1, 10).unwrap();
    assert_eq!(context.len(), 2);
    assert_eq!(context[0].entry.id, decision.id);
    assert_eq!(context[0].reason, ContextReason::DirectTextMatch);
    assert_eq!(context[1].entry.id, session.id);
    assert_eq!(context[1].depth, 1);
    assert!(matches!(
        &context[1].reason,
        ContextReason::Relationship {
            relationship_id,
            relationship_type: RelationshipType::DiscoveredDuring,
            ..
        } if relationship_id == &relationship.id
    ));

    let rebuilt = SearchIndex::rebuild(&journal).unwrap();
    assert!(rebuilt.verify(&journal).unwrap().consistent);
    assert_eq!(SearchIndex::status(&journal).unwrap().relationships, 1);
}

#[test]
fn context_limits_and_depth_are_validated() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Journal::init(dir.path(), JournalConfig::defaults("context-limits")).unwrap();
    assert!(retrieve_context(&journal, None, None, 9, 10).is_err());
    assert!(retrieve_context(&journal, None, None, 0, 101).is_err());
}
