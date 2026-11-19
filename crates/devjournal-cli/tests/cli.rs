use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn run(project: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_devjournal"))
        .arg("--project")
        .arg(project.path())
        .args(args)
        .output()
        .unwrap()
}

fn init(project: &TempDir) {
    let output = run(project, &["init", "--name", "cli-test"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn json_status_and_empty_search_are_valid_without_stdout_diagnostics() {
    let project = TempDir::new().unwrap();
    init(&project);

    let status = run(&project, &["status", "--json"]);
    assert!(status.status.success());
    assert!(status.stderr.is_empty());
    let value: Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(value["project"]["name"], "cli-test");
    assert_eq!(value["entry_count"], 0);

    let search = run(&project, &["search", "missing", "--json"]);
    assert!(search.status.success());
    assert!(search.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&search.stdout).unwrap(),
        serde_json::json!([])
    );
}

#[test]
fn json_entry_create_show_and_missing_entry_use_contract() {
    let project = TempDir::new().unwrap();
    init(&project);
    let created = run(
        &project,
        &["note", "new", "from-cli", "--content", "portable", "--json"],
    );
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    assert!(created.stderr.is_empty());
    let created: Value = serde_json::from_slice(&created.stdout).unwrap();
    let id = created["entry"]["id"].as_str().unwrap();

    let shown = run(&project, &["entry", "show", id, "--json"]);
    assert!(shown.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&shown.stdout).unwrap()["content"],
        "portable"
    );

    let missing = run(&project, &["entry", "show", "dj-missing", "--json"]);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    let error: Value = serde_json::from_slice(&missing.stderr).unwrap();
    assert_eq!(error["error"]["code"], "ENTRY_NOT_FOUND");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("dj-missing")
    );
}

#[test]
fn json_invalid_arguments_and_missing_content_fail_without_prompting() {
    let project = TempDir::new().unwrap();
    init(&project);
    let invalid_args = run(&project, &["unknown", "--json"]);
    assert_eq!(invalid_args.status.code(), Some(2));
    assert!(invalid_args.stdout.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&invalid_args.stderr).unwrap()["error"]["code"],
        "INVALID_ARGUMENTS"
    );

    let missing = run(&project, &["note", "new", "--json"]);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&missing.stderr).unwrap()["error"]["code"],
        "INVALID_INPUT"
    );
}

#[test]
fn help_keeps_success_exit_code() {
    let output = Command::new(env!("CARGO_BIN_EXE_devjournal"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Local-first project development journal")
    );
}

#[test]
fn non_interactive_missing_fields_fail_cleanly() {
    let project = TempDir::new().unwrap();
    init(&project);
    let output = run(&project, &["note", "new"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("interactive prompts require a terminal")
    );
}

#[test]
fn json_lists_context_and_temp_recovery_are_structured() {
    let project = TempDir::new().unwrap();
    init(&project);
    let session = run(&project, &["session", "start", "review", "--json"]);
    assert!(session.status.success());
    let session: Value = serde_json::from_slice(&session.stdout).unwrap();
    assert_eq!(session["entry"]["category"], "session");

    for args in [
        vec!["session", "list", "--json"],
        vec!["task", "list", "--json"],
        vec!["context", "--json"],
    ] {
        let output = run(&project, &args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert!(serde_json::from_slice::<Value>(&output.stdout).is_ok());
    }

    let notes = project.path().join(".devjournal/notes");
    let temp = notes.join(".devjournal-entry-dj-stale.tmp-123e4567-e89b-12d3-a456-426614174000");
    std::fs::write(&temp, "incomplete").unwrap();
    let list = run(&project, &["recover", "--json"]);
    assert_eq!(
        serde_json::from_slice::<Value>(&list.stdout).unwrap()["artifacts"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(temp.exists());
    let clean = run(&project, &["recover", "--clean", "--json"]);
    assert_eq!(
        serde_json::from_slice::<Value>(&clean.stdout).unwrap()["artifacts"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(!temp.exists());
}

#[test]
fn graph_relationship_index_and_context_commands_work_end_to_end() {
    let project = TempDir::new().unwrap();
    init(&project);
    let note = run(
        &project,
        &[
            "note",
            "new",
            "Auth implementation",
            "--content",
            "authentication behavior",
            "--json",
        ],
    );
    let note: Value = serde_json::from_slice(&note.stdout).unwrap();
    let source = note["entry"]["id"].as_str().unwrap();
    let decision = run(
        &project,
        &[
            "decision",
            "new",
            "Token strategy",
            "--content",
            "narrowly scoped token claims",
            "--json",
        ],
    );
    let decision: Value = serde_json::from_slice(&decision.stdout).unwrap();
    let target = decision["entry"]["id"].as_str().unwrap();

    let link = run(
        &project,
        &["link", source, target, "--type", "implements", "--json"],
    );
    assert!(
        link.status.success(),
        "{}",
        String::from_utf8_lossy(&link.stderr)
    );
    let relationship_id =
        serde_json::from_slice::<Value>(&link.stdout).unwrap()["relationship"]["id"]
            .as_str()
            .unwrap()
            .to_owned();
    let outgoing = run(&project, &["relationships", "list", source, "--json"]);
    assert_eq!(
        serde_json::from_slice::<Value>(&outgoing.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let incoming = run(
        &project,
        &["relationships", "list", target, "--incoming", "--json"],
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&incoming.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let verify = run(&project, &["index", "verify", "--json"]);
    assert_eq!(
        serde_json::from_slice::<Value>(&verify.stdout).unwrap()["consistent"],
        false
    );
    let rebuild = run(&project, &["index", "rebuild", "--json"]);
    assert!(
        serde_json::from_slice::<Value>(&rebuild.stdout).unwrap()["verification"]["consistent"]
            .as_bool()
            .unwrap()
    );

    let neighbors = run(
        &project,
        &["graph", "neighbors", source, "--depth", "1", "--json"],
    );
    let neighbor_items = serde_json::from_slice::<Value>(&neighbors.stdout).unwrap();
    assert_eq!(neighbor_items[0]["entry"]["id"], target);
    let context = run(
        &project,
        &[
            "context",
            "--topic",
            "authentication",
            "--depth",
            "1",
            "--json",
        ],
    );
    assert!(
        context.status.success(),
        "{}",
        String::from_utf8_lossy(&context.stderr)
    );
    let context: Value = serde_json::from_slice(&context.stdout).unwrap();
    assert!(
        context["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["reason"]["kind"] == "relationship")
    );

    let path = run(&project, &["graph", "path", source, target, "--json"]);
    assert_eq!(
        serde_json::from_slice::<Value>(&path.stdout).unwrap()["path"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let db = project.path().join(".devjournal/index.sqlite3");
    std::fs::remove_file(db).unwrap();
    assert!(run(&project, &["index", "rebuild"]).status.success());
    let verified = run(&project, &["index", "verify", "--json"]);
    assert_eq!(
        serde_json::from_slice::<Value>(&verified.stdout).unwrap()["consistent"],
        true
    );

    let unlinked = run(&project, &["unlink", &relationship_id, "--json"]);
    assert!(
        unlinked.status.success(),
        "{}",
        String::from_utf8_lossy(&unlinked.stderr)
    );
    let now_empty = run(&project, &["relationships", "list", source, "--json"]);
    assert!(
        serde_json::from_slice::<Value>(&now_empty.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
}
