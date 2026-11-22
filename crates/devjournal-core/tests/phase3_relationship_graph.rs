use std::str::FromStr;

use devjournal_core::{
    Author, AuthorType, Category, Entry, EntrySource, EntrySourceType, Journal, JournalConfig,
    KnowledgeGraph, Relationship, RelationshipType,
};

fn author(kind: AuthorType, name: &str) -> Author {
    Author {
        kind,
        name: name.into(),
    }
}

fn source(kind: EntrySourceType, detail: &str) -> EntrySource {
    EntrySource {
        kind,
        detail: Some(detail.into()),
    }
}

fn edge(from: &str, to: &str, kind: RelationshipType) -> Relationship {
    Relationship::new(
        from,
        to,
        kind,
        author(AuthorType::Ai, "test-agent"),
        source(EntrySourceType::Agent, "integration test"),
    )
}

#[test]
fn relationship_records_round_trip_with_provenance_and_reject_invalid_links() {
    let dir = tempfile::tempdir().unwrap();
    let journal =
        Journal::init(dir.path(), JournalConfig::defaults("relationship-contract")).unwrap();
    let decision = Entry::new(
        "Token decision",
        Category::Decision,
        "Use short lived tokens",
        author(AuthorType::Human, "Ada"),
    );
    let implementation = Entry::new(
        "Token implementation",
        Category::Note,
        "The API validates token expiry",
        author(AuthorType::Human, "Ada"),
    );
    journal.create_entry(&decision).unwrap();
    journal.create_entry(&implementation).unwrap();

    let provenance = source(EntrySourceType::Agent, "derived from code review");
    let relationship = Relationship::new(
        implementation.id.clone(),
        decision.id.clone(),
        RelationshipType::Implements,
        author(AuthorType::Ai, "review-agent"),
        provenance.clone(),
    );
    let path = journal.create_relationship(&relationship).unwrap();
    assert!(path.is_file());
    assert_eq!(
        journal.get_relationship(&relationship.id).unwrap(),
        relationship
    );
    assert_eq!(journal.relationships().unwrap()[0].provenance, provenance);

    let duplicate = Relationship::new(
        implementation.id.clone(),
        decision.id.clone(),
        RelationshipType::Implements,
        author(AuthorType::Human, "Ada"),
        source(EntrySourceType::Manual, "duplicate"),
    );
    assert!(journal.create_relationship(&duplicate).is_err());

    let missing = edge(
        &implementation.id,
        "dj-missing",
        RelationshipType::References,
    );
    assert!(journal.create_relationship(&missing).is_err());
    let self_reference = edge(&decision.id, &decision.id, RelationshipType::RelatesTo);
    assert!(self_reference.validate().is_err());
    assert!(RelationshipType::from_str("invented_type").is_err());
}

#[test]
fn task_history_recognizes_legacy_same_title_follow_up_records() {
    let dir = tempfile::tempdir().unwrap();
    let journal =
        Journal::init(dir.path(), JournalConfig::defaults("legacy-task-history")).unwrap();
    let original = Entry::new(
        "Ship search",
        Category::Task,
        "Add metadata filters",
        author(AuthorType::Human, "Ada"),
    );
    journal.create_entry(&original).unwrap();
    let mut follow_up = Entry::new(
        original.title.clone(),
        Category::Task,
        original.content.clone(),
        author(AuthorType::Human, "Ada"),
    );
    follow_up.related.push(original.id.clone());
    follow_up.status = Some("in_progress".into());
    journal.create_entry(&follow_up).unwrap();

    let history = journal.entry_history(&follow_up.id).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].id, original.id);
    assert_eq!(history[1].id, follow_up.id);
}

#[test]
fn graph_queries_cover_direction_depth_paths_cycles_and_limits() {
    let nodes = [
        "dj-issue",
        "dj-component",
        "dj-session",
        "dj-task",
        "dj-decision",
    ]
    .into_iter()
    .map(str::to_owned);
    let relationships = vec![
        edge("dj-issue", "dj-component", RelationshipType::CausedBy),
        edge("dj-issue", "dj-session", RelationshipType::DiscoveredDuring),
        edge("dj-task", "dj-issue", RelationshipType::Blocks),
        edge("dj-component", "dj-decision", RelationshipType::Implements),
    ];
    let graph = KnowledgeGraph::new(nodes, relationships).unwrap();

    assert_eq!(
        graph.issues_caused_by("dj-component").unwrap(),
        ["dj-issue"]
    );
    assert_eq!(
        graph.sessions_for_issue("dj-issue").unwrap(),
        ["dj-session"]
    );
    assert_eq!(graph.blocking_tasks("dj-issue").unwrap(), ["dj-task"]);
    assert_eq!(
        graph.implementations_of("dj-decision").unwrap(),
        ["dj-component"]
    );

    let depth_one = graph.neighbors("dj-issue", 1, None).unwrap();
    assert_eq!(depth_one.len(), 3);
    assert!(depth_one.iter().all(|hit| hit.depth == 1));
    let depth_two = graph.neighbors("dj-issue", 2, None).unwrap();
    assert!(
        depth_two
            .iter()
            .any(|hit| hit.entry_id == "dj-decision" && hit.depth == 2)
    );
    assert_eq!(
        graph.path("dj-component", "dj-decision").unwrap(),
        Some(vec!["dj-component".into(), "dj-decision".into()])
    );
    assert!(graph.path("dj-decision", "dj-component").unwrap().is_none());
    assert!(graph.neighbors("dj-unknown", 1, None).is_err());
    assert!(graph.neighbors("dj-issue", 33, None).is_err());
    assert!(!graph.has_directed_cycle());

    let cycle = KnowledgeGraph::new(
        ["dj-a".to_owned(), "dj-b".to_owned(), "dj-c".to_owned()],
        vec![
            edge("dj-a", "dj-b", RelationshipType::DependsOn),
            edge("dj-b", "dj-c", RelationshipType::DependsOn),
            edge("dj-c", "dj-a", RelationshipType::DependsOn),
        ],
    )
    .unwrap();
    assert!(cycle.has_directed_cycle());
}

#[cfg(unix)]
#[test]
fn journal_rejects_symlinked_entry_files() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let journal = Journal::init(dir.path(), JournalConfig::defaults("symlink-guard")).unwrap();
    let entry = Entry::new(
        "Regular entry",
        Category::Note,
        "stored in the journal",
        author(AuthorType::Human, "Ada"),
    );
    let stored = journal.create_entry(&entry).unwrap();
    let external = dir.path().join("outside.md");
    std::fs::copy(stored, &external).unwrap();
    symlink(external, journal.root().join("notes/dj-symlinked.md")).unwrap();

    assert!(journal.get_entry("dj-symlinked").is_err());
    assert!(journal.entries().is_err());
}
