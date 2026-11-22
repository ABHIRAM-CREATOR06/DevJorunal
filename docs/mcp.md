# MCP and Zed setup

DevJournal's optional `devjournal-mcp` binary exposes the journal to MCP clients over local stdio. It needs no account, network service, or AI provider.

## Build and run

From a checkout:

```powershell
cargo build --locked --release -p devjournal-mcp
target\release\devjournal-mcp.exe --project C:\path\to\your\project
```

The MCP process expects JSON-RPC on stdin and writes protocol responses to stdout. Keep logs and other diagnostics on stderr. For normal use, configure an MCP client to start the binary; do not run it as an interactive command.

## Tools and resources

The server provides these read tools:

- `search_entries`: full-text search with category, author, tag, status, and RFC3339 date filters.
- `get_entry`: fetch one entry by ID.
- `get_context`: retrieve direct and graph-connected context with inclusion reasons.
- `graph_neighbors`: traverse relationships from an entry.
- `project_overview`: report category counts, recent decisions, and current open tasks.
- `session_summary`: make a deterministic summary from a saved session and its linked entries.

Resources include `devjournal://project/overview` and templates for `devjournal://entries/{entry_id}` and `devjournal://sessions/{entry_id}/summary`.

Tool responses contain JSON text. Resource reads return JSON text for entries/overview and Markdown text for session summaries. MCP requests may create or update the disposable SQLite index while syncing, but they do not modify journal Markdown.

## Zed

Zed's current docs support custom local MCP servers from **Settings → AI → MCP Servers**. Add a local server with the installed binary and an absolute project path. In `settings.json`, the corresponding entry looks like:

```json
{
  "context_servers": {
    "devjournal": {
      "command": "devjournal-mcp",
      "args": ["--project", "C:\\path\\to\\your\\project"],
      "env": {}
    }
  }
}
```

Build/install the executable first, for example with `cargo install --locked --path crates/devjournal-mcp`. Zed currently supports MCP tools and prompts, so the DevJournal tools are available there. Resource templates remain useful to clients that support MCP resources. Zed's MCP server extension mechanism is being deprecated in favor of the MCP registry, so this project uses the supported custom local-server path instead of publishing an extension. See [Zed MCP docs](https://zed.dev/docs/ai/mcp) and [extension status](https://zed.dev/docs/extensions/mcp-extensions).

## Security and access

The server is local stdio-only. It exposes no HTTP listener and its tools do not create, revise, unlink, or delete journal records. The configured `--project` path defines the journal it can read. Entry resource IDs are validated by core storage before any file is opened; journal directories and source files are rejected when they are symlinks or non-regular files. Index synchronization may ensure the `.devjournal/.gitignore` rule and update the generated `.devjournal/index.sqlite3` cache.

The review for this milestone was source-based, with local regression tests for path validation and symlink rejection. It was not a full independent penetration test or race-condition audit. Keep the project directory and MCP configuration under the intended user's filesystem permissions.
