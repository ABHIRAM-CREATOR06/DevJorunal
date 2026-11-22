# Contributing

DevJournal is a Rust workspace. The minimum supported Rust version is 1.89.

## Local checks

```powershell
cargo fmt --all -- --check
cargo test --workspace --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
```

SQLite is bundled with the core crate. Tests use temporary project directories and do not need a network service. If a new dependency is required, keep it small, update `Cargo.lock`, and confirm the offline workspace checks work after fetching it.

## Storage and compatibility

- Markdown under `.devjournal/` is authoritative. SQLite is disposable and rebuildable.
- Additive edits should preserve existing entry formats and history.
- Do not replace historical entries when adding revisions; create a new entry and a `supersedes` relationship.
- Keep MCP operations read-only with respect to Markdown unless the project adds a separately reviewed write workflow.
- Avoid logging MCP protocol data to stdout; stdio is the transport.

## Pull requests

Describe the user-visible behavior, storage changes, and validation performed. Include regression tests for new parsers, traversal, index, revision, or CLI behavior. Update `DEVELOPMENT.md`, `ROADMAP.md`, and relevant architecture/storage docs when behavior changes.
