# Development progress

## Phase 1 — Core foundation (complete)

Implemented the Rust workspace, core entry model, category and provenance types, validation, `.devjournal` initialization, default project configuration, and create-only Markdown storage with retrieval by ID. New records are stored in category folders where defined and under `entries/` for other categories. Existing records and project files are preserved during initialization.

Tests cover initialization and entry round-trip, config round-trip/validation, plus path traversal and overwrite refusal. The earlier create-only implementation used direct writes; the reliability behavior is now described below.

## Phase 2 — CLI (complete)

Implemented `init`, `status`, `note new`, `decision new`, `session start/list`, `task list/update`, `search`, `context`, and `entry show` (`show` remains an alias). Search scans Markdown directly. Task updates append a related follow-up record to preserve history. JSON is available with `--json`; errors use JSON on stderr, and results use JSON on stdout. Missing note/decision fields are prompted only in a terminal and never in JSON mode.

## Storage reliability milestone (complete)

Entry writes stage a uniquely named file in the final directory, flush and sync it, validate a read-back parse, and publish without replacing existing IDs. Windows uses same-volume `MoveFileW`; Unix uses atomic no-clobber hard-link publication and syncs the parent directory. Failed operations clean the temp. `recover` lists matching stale temps and `recover --clean` removes them explicitly without promotion. Tests cover injected validation failure, preservation, cleanup, duplicate IDs, and concurrent creation.

## Phase 3 — Knowledge graph, indexing & context (complete)

Added typed, provenance-bearing Markdown relationships with tombstones for unlinking, a directed graph API with symmetric `relates_to` edges, SQLite FTS5 indexing, incremental updates, verification and rebuild recovery. Search and context use the disposable index; context can include explained graph neighbors. Added `link`, `unlink`, `relationships`, `graph`, and `index` CLI commands with JSON output. Markdown remains the source of truth, and the generated SQLite database is ignored by Git. The workspace MSRV is Rust 1.89 to support the bundled SQLite dependency.

## Verification

`cargo test --workspace --offline` passed (30 tests); `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --offline -- -D warnings` passed. Integration coverage exercises relationships, graph queries, index recovery, context explanations, and JSON CLI workflows end to end. The workspace is on OneDrive, so Cargo may report incremental-cache hard-link fallback warnings; these do not affect the build or tests. MCP and Zed remain future phases.
