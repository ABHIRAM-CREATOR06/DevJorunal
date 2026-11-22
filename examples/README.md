# Examples

Initialize a journal, add a task, and create a revision without overwriting the original record:

```powershell
devjournal init --name sample-project
devjournal note new "Search rollout" --content "Add exact tag and author filters" --tags search
devjournal task update dj-existing-task in_progress
devjournal task list --json
devjournal entry history dj-existing-task --json
```

Search a subset of the index and gather connected context:

```powershell
devjournal search authentication --category decision --tag security --created-after 2026-01-01T00:00:00Z --json
devjournal context --topic authentication --depth 2 --limit 12 --json
devjournal overview --json
```

For Zed, install `devjournal-mcp` and add it under `context_servers` in the user's settings. Replace the command path and project path with local absolute paths:

```json
{
  "context_servers": {
    "devjournal": {
      "command": "devjournal-mcp",
      "args": ["--project", "C:\\work\\sample-project"],
      "env": {}
    }
  }
}
```

See [MCP and Zed setup](../docs/mcp.md) for tool and resource details.
