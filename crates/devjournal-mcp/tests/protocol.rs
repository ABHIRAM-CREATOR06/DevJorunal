use std::process::{Command, Stdio};

use devjournal_core::{Author, AuthorType, Category, Entry, Journal, JournalConfig};
use serde_json::{Value, json};

#[test]
fn stdio_server_advertises_tools_and_resources_and_reads_project_data() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Journal::init(dir.path(), JournalConfig::defaults("mcp-protocol-test")).unwrap();
    let entry = Entry::new(
        "MCP-visible note",
        Category::Note,
        "The journal entry is served through MCP.",
        Author {
            kind: AuthorType::Human,
            name: "tester".into(),
        },
    );
    journal.create_entry(&entry).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_devjournal-mcp"))
        .arg("--project")
        .arg(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().unwrap();
        for message in [
            json!({
                "jsonrpc":"2.0", "id":1, "method":"initialize",
                "params":{
                    "protocolVersion":"2025-11-25",
                    "capabilities":{},
                    "clientInfo":{"name":"devjournal-test","version":"0.1"}
                }
            }),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            json!({"jsonrpc":"2.0","id":3,"method":"resources/list","params":{}}),
            json!({"jsonrpc":"2.0","id":4,"method":"resources/templates/list","params":{}}),
            json!({
                "jsonrpc":"2.0", "id":5, "method":"tools/call",
                "params":{"name":"get_entry","arguments":{"entry_id":entry.id}}
            }),
            json!({
                "jsonrpc":"2.0", "id":6, "method":"resources/read",
                "params":{"uri":format!("devjournal://entries/{}", entry.id)}
            }),
        ] {
            writeln!(stdin, "{message}").unwrap();
        }
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let responses = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let response = |id| responses.iter().find(|item| item["id"] == id).unwrap();

    assert!(response(1)["result"]["capabilities"]["tools"].is_object());
    assert!(response(1)["result"]["capabilities"]["resources"].is_object());
    let tools = response(2)["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|tool| tool["name"] == "get_entry"));
    assert!(tools.iter().any(|tool| tool["name"] == "session_summary"));
    assert_eq!(
        response(3)["result"]["resources"][0]["uri"],
        "devjournal://project/overview"
    );
    assert_eq!(
        response(4)["result"]["resourceTemplates"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(
        response(5)["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("MCP-visible note")
    );
    assert!(
        response(6)["result"]["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("The journal entry is served through MCP.")
    );
}
