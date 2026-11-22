# DevJournal

DevJournal is a local-first, project-owned journal for development knowledge. It provides a Rust library, CLI, and optional stdio MCP server for portable Markdown records, graph relationships, indexed search, and project context.

## Current status

The roadmap phases are implemented. The workspace includes standard TOML config parsing, append-only entry revisions, typed Markdown relationships, graph traversal, rebuildable SQLite FTS5 indexing, metadata-filtered search, graph-aware context, deterministic project/session summaries, a JSON CLI, and a local MCP server. See [ROADMAP.md](ROADMAP.md) for scope and remaining future work.

## Workspace

```text
crates/devjournal-core/  Core models, configuration, Markdown storage
crates/devjournal-cli/   Command-line interface
crates/devjournal-mcp/   Local stdio MCP server
ARCHITECTURE.md          Architecture and trust boundaries
ROADMAP.md               Phases and acceptance criteria
docs/storage.md          On-disk format
docs/mcp.md              MCP setup and Zed configuration
CONTRIBUTING.md          Build, test, and contribution workflow
```

## Library example

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
let saved = journal.get_entry(&entry.id)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## CLI quick start

```powershell
cargo run -p devjournal -- init --name my-project
cargo run -p devjournal -- note new "Build setup" --content "The workspace uses Rust."
cargo run -p devjournal -- search Rust
cargo run -p devjournal -- status
```

Commands include `init`, `status`, `overview`, `note new`, `decision new`, `session start/list/summary`, `task list/update`, `search`, `context`, `entry show/revise/history`, `link`, `unlink`, `relationships list`, `graph`, `index`, and `recover`. Revisions create new entries with `supersedes` relationships; earlier content remains intact. Use `--project PATH` to select a project directory.

```powershell
cargo run -p devjournal -- link dj-source dj-target --type implements
cargo run -p devjournal -- relationships list dj-source
cargo run -p devjournal -- graph neighbors dj-source --depth 2 --json
cargo run -p devjournal -- context --topic authentication --depth 2 --json
cargo run -p devjournal -- index rebuild
cargo run -p devjournal -- index verify --json
cargo run -p devjournal -- search auth --author Ada --tag security --status open --created-after 2026-01-01T00:00:00Z --json
cargo run -p devjournal -- entry revise dj-entry-id --content "Reviewed replacement text"
cargo run -p devjournal -- entry history dj-entry-id --json
cargo run -p devjournal -- overview --json
cargo run -p devjournal -- session summary dj-session-id
```

Add `--json` to data commands for machine-readable output, for example `status --json`, `search Rust --json`, `entry show <id> --json`, `context --json`, and `session list --json`. JSON results go to stdout; JSON errors go to stderr with an `error.code` and `error.message`. Empty list/search results are JSON arrays. `context --json` without a topic returns up to ten most recent entries.

In a terminal, omit the note/decision title or content to be prompted. Multiline content ends with a line containing `.`; enter `:cancel` to abort. Prompts are disabled for piped input and JSON mode. Run `devjournal recover` to list recognized leftover write temps, or `devjournal recover --clean` to delete those temp files explicitly.

See [MCP and Zed setup](docs/mcp.md) to build and connect the optional local server. Install both commands from a checkout with `cargo install --locked --path crates/devjournal-cli` and `cargo install --locked --path crates/devjournal-mcp`.

Run checks with `cargo test --workspace --offline`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets --offline -- -D warnings`.

## Data and privacy

Knowledge is stored as Markdown under `.devjournal/`; configuration defaults to local-only and transcript storage is disabled. SQLite is a disposable local index and is ignored by Git. MCP uses stdio and reads only the configured project's journal. See [storage format](docs/storage.md), [architecture](ARCHITECTURE.md), and [MCP security notes](docs/mcp.md#security-and-access).

## License

MIT. See [LICENSE](LICENSE).
