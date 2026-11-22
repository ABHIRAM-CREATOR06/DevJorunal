# Development progress

## Foundation, configuration, and revisions

The Rust workspace stores portable Markdown entries under `.devjournal/`. Configuration uses serde-backed TOML parsing with validation of syntax, known field types, and the required project name. `entry revise` and `entry history` preserve prior records; new revisions link to the earlier record with a `supersedes` edge.

## CLI and storage reliability

The CLI supports JSON output, terminal-only prompts, task updates, and grouped task revision history. Entry writes stage a unique file in the destination directory, sync and validate it, then publish without replacing an existing ID. `recover` lists known stale temporary files and `recover --clean` removes them explicitly.

## Knowledge graph, indexing, and context

Typed relationships persist as Markdown, with removal tombstones. The graph engine supports traversal, paths, cycle detection, and relationship queries. Bundled SQLite FTS5 is a disposable search/index layer; filters include category, exact author, exact tag, status, and creation-time range. Context retrieval combines direct text results and linked records with deterministic reasons.

## MCP, Zed, and continuity

`devjournal-mcp` is a local stdio server built with the official Rust MCP SDK. Its tools search and read project knowledge, and its resources expose the overview, entries, and session summaries. `overview` and `session summary` create deterministic views from saved records. Zed connects through its custom local MCP server setting; the current Zed docs describe MCP server extensions as deprecated.

## Release preparation

`CONTRIBUTING.md` documents offline build/test/lint commands. `examples/README.md` shows common revision, search, overview, and Zed setup workflows. Cargo install commands are documented in `README.md`. Source review added guards against symlinked journal directories and files and confirmed MCP tools are read-only for Markdown. This was a focused source review, not a full independent penetration test or race-condition audit.

## Verification

`cargo test --workspace --offline` passes (41 tests); `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --offline -- -D warnings` pass. The MCP integration test starts the stdio server and exercises its JSON-RPC handshake, tools, and resources. Cargo may report incremental-cache hard-link fallback warnings on OneDrive; they do not affect build/test results.
