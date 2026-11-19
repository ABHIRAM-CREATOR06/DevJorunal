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
