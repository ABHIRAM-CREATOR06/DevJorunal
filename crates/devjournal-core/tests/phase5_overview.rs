use devjournal_core::{
    Author, AuthorType, Category, Entry, EntrySource, EntrySourceType, Journal, JournalConfig,
    Relationship, RelationshipType,
};

fn author() -> Author {
    Author {
        kind: AuthorType::Human,
        name: "tester".into(),
    }
}

#[test]
fn overview_and_session_summary_use_saved_project_records() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Journal::init(dir.path(), JournalConfig::defaults("summary-project")).unwrap();
    let mut session = Entry::new(
        "Authentication review",
        Category::Session,
        "Reviewed token expiry",
        author(),
    );
    let decision = Entry::new(
        "Token policy",
        Category::Decision,
        "Use short-lived tokens",
        author(),
    );
    let mut open_task = Entry::new(
        "Add expiry checks",
        Category::Task,
        "Implement validation",
        author(),
    );
    open_task.status = Some("in_progress".into());
    let mut closed_task = Entry::new("Document policy", Category::Task, "Write docs", author());
    closed_task.status = Some("done".into());
    session.tags.push("auth".into());
    journal.create_entry(&session).unwrap();
    journal.create_entry(&decision).unwrap();
    journal.create_entry(&open_task).unwrap();
    journal.create_entry(&closed_task).unwrap();
    let relation = Relationship::new(
        decision.id.clone(),
        session.id.clone(),
        RelationshipType::DiscoveredDuring,
        author(),
        EntrySource {
            kind: EntrySourceType::Manual,
            detail: Some("noted during the review".into()),
        },
    );
    journal.create_relationship(&relation).unwrap();

    let overview = journal.project_overview().unwrap();
    assert_eq!(overview.project.name, "summary-project");
    assert_eq!(overview.total_entries, 4);
    assert_eq!(overview.entries_by_category["task"], 2);
    assert_eq!(overview.recent_decisions[0].id, decision.id);
    assert_eq!(overview.open_tasks.len(), 1);
    assert_eq!(overview.open_tasks[0].id, open_task.id);

    let summary = journal.session_summary(&session.id).unwrap();
    assert_eq!(summary.session.id, session.id);
    assert_eq!(summary.linked_entries.len(), 1);
    assert_eq!(summary.linked_entries[0].id, decision.id);
    assert!(summary.markdown.contains("Use short-lived tokens"));
    assert!(journal.session_summary(&decision.id).is_err());
}
