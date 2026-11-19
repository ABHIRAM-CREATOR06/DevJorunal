# DevJournal

DevJournal is a local-first, project-owned journal for development knowledge. It provides a Rust library and CLI for initializing `.devjournal`, writing portable Markdown entries, retrieving them by ID, and searching locally without an editor, agent, network connection, or external service.

## Current status

Phases 1 and 2 are implemented. The workspace includes validated entry metadata, human/AI/system provenance, project configuration, reliable create-only Markdown storage, a JSON-capable `devjournal` CLI, and basic local search. SQLite indexing, MCP, and Zed integration have not been implemented yet.

## Workspace

```text
crates/devjournal-core/  Core models, configuration, Markdown storage
ARCHITECTURE.md         Architecture and trust boundaries
ROADMAP.md              Phases and acceptance criteria
docs/storage.md         On-disk format
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

Commands include `init`, `status`, `note new`, `decision new`, `session start/list`, `task list/update`, `search`, `context`, and `show`. Task updates create a linked follow-up record so the prior task record remains intact. Use `--project PATH` to select a project directory.

Add `--json` to data commands for machine-readable output, for example `status --json`, `search Rust --json`, `entry show <id> --json`, `context --json`, and `session list --json`. JSON results go to stdout; JSON errors go to stderr with an `error.code` and `error.message`. Empty list/search results are JSON arrays. `context --json` without a topic returns up to ten most recent entries.

In a terminal, omit the note/decision title or content to be prompted. Multiline content ends with a line containing `.`; enter `:cancel` to abort. Prompts are disabled for piped input and JSON mode. Run `devjournal recover` to list recognized leftover write temps, or `devjournal recover --clean` to delete those temp files explicitly.

Run checks with `cargo test --workspace --offline` and `cargo fmt --all -- --check`.

## Data and privacy

Knowledge is stored as Markdown under `.devjournal/`; configuration defaults to local-only and transcript storage is disabled. SQLite is not yet used. See [storage format](docs/storage.md) and [architecture](ARCHITECTURE.md).

## License

MIT. See [LICENSE](LICENSE).
