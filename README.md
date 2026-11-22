# DevJournal

### Your project's memory, in files you own.

DevJournal keeps the decisions, tasks, discoveries, and session notes behind a codebase close to the code itself. Write portable Markdown, connect related entries, and find the context again from the CLI, Rust library, or an MCP-compatible assistant.

**Local-first · Markdown-backed · Searchable · Built with Rust**

```text
Your project
└── .devjournal/
    ├── config.toml          project settings
    ├── decisions/           why the code is this way
    ├── notes/               discoveries and working knowledge
    ├── tasks/               work, status, and revision history
    ├── sessions/            what happened in a work session
    ├── relationships/       links between entries
    └── index.sqlite3        rebuildable search index
```

## Get started

From the repository checkout:

```powershell
cargo run -p devjournal -- init --name my-project
cargo run -p devjournal -- note new "Build setup" --content "The workspace uses Rust."
cargo run -p devjournal -- search Rust
cargo run -p devjournal -- overview
```

Use `--project PATH` to select a project directory. Add `--json` to supported commands for machine-readable output.

## A small, connected knowledge base

| Keep | Find and connect |
| --- | --- |
| Decisions, notes, tasks, and sessions as Markdown | Full-text search with category, author, tag, status, and date filters |
| Append-only revisions, so earlier thinking stays available | Relationships, graph traversal, and graph-aware context retrieval |
| Project overview and deterministic session summaries | JSON output for scripts and other tools |

The SQLite index is a disposable local cache. Rebuild it from the Markdown journal whenever needed; the records stay readable without DevJournal.

## Follow a decision from question to context

```powershell
# Capture the decision
cargo run -p devjournal -- decision new "Use SQLite for local search" --content "Markdown stays canonical; the index is rebuildable."

# Link related work, then find the thread later
cargo run -p devjournal -- link dj-task-id dj-decision-id --type implements
cargo run -p devjournal -- search SQLite --tag storage --json
cargo run -p devjournal -- context --topic "local search" --depth 2
```

Entry IDs are printed when records are created. Use those IDs in later commands. `cargo run -p devjournal -- --help` and `cargo run -p devjournal -- <command> --help` show the available options.

## Revise without erasing history

```powershell
cargo run -p devjournal -- entry revise dj-entry-id --content "Updated decision after measuring the workload."
cargo run -p devjournal -- entry history dj-entry-id
```

Revisions create a new entry connected with `supersedes`. Earlier text remains intact, and task lists group revisions under the current task.

## Connect an MCP client

The optional `devjournal-mcp` server runs locally over stdio. Its tools provide search, entry lookup, graph context, project overviews, and session summaries. It also exposes MCP resources for clients that support them.

```powershell
cargo install --locked --path crates/devjournal-cli
cargo install --locked --path crates/devjournal-mcp
```

Then configure your MCP client to launch `devjournal-mcp --project <absolute-project-path>`. See [MCP and Zed setup](docs/mcp.md) for configuration details and current Zed support.

## Rust library

```rust
use devjournal_core::{Author, AuthorType, Category, Entry, Journal, JournalConfig};

let journal = Journal::init(".", JournalConfig::defaults("my-project"))?;
let entry = Entry::new(
    "Why we chose SQLite",
    Category::Decision,
    "The index will be rebuildable from Markdown.",
    Author { kind: AuthorType::Human, name: "developer".into() },
);
journal.create_entry(&entry)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Design principles

- **Your records are yours.** Markdown is the source of truth; no hosted service or account is required.
- **History is additive.** Revisions preserve previous entries instead of silently replacing them.
- **Context has structure.** Typed relationships connect decisions, tasks, notes, and sessions.
- **Automation stays inspectable.** JSON and MCP make the same project knowledge available to tools.

Transcript storage is disabled by default. MCP is stdio-only and reads the configured project's journal; index synchronization may update the generated cache and its Git ignore rule.

## Build and contribute

```powershell
cargo test --workspace --offline
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --offline -- -D warnings
```

The workspace contains the core library, CLI, and MCP server. Start with [contributing](CONTRIBUTING.md), [architecture](ARCHITECTURE.md), [storage format](docs/storage.md), or the [roadmap](ROADMAP.md). Working examples are collected in [examples](examples/README.md).

## License

[MIT](LICENSE)
