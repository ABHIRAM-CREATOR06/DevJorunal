# Roadmap

## Phase 1 — Core foundation (complete)

- [x] Rust workspace and core crate
- [x] Entry model, provenance, categories, and validation
- [x] Project initialization and default config
- [x] Markdown entry creation and retrieval
- [x] Standard TOML parser and append-only entry revision/history commands

## Phase 2 — CLI (complete)

- [x] Clap CLI for initialization, notes, sessions, decisions, tasks, and status
- [x] JSON output for status, search, context, lists, entry display, and recovery
- [x] Terminal-aware prompts with cancellation and non-interactive failure behavior
- [x] Task revision history grouped under the current revision

## Storage reliability milestone (complete)

- [x] Same-directory staging, sync, read-back validation, and no-replace publication
- [x] Stale temp discovery and explicit cleanup without promotion
- [x] Duplicate and concurrent create protection

## Phase 3 — Knowledge graph, indexing, and context (complete)

- [x] Markdown relationship records with provenance and removal tombstones
- [x] Typed relationship model and deterministic graph traversal/path/cycle operations
- [x] Rebuildable SQLite FTS5 index with incremental source synchronization
- [x] Index status, verification, corrupt-index preservation, and rebuild
- [x] Graph-aware context with depth, limits, deduplication, and relevance reasons
- [x] Search filters for author, tag, status, and creation date range

## Phase 4 — MCP server

- [x] Local stdio MCP server with read tools, resources, validation, and integration guide

## Phase 5 — Agent continuity

- [x] Deterministic graph-aware context retrieval and related knowledge
- [x] Deterministic session summaries and project overviews

## Phase 6 — Zed integration

- [x] Verify current official Zed MCP support and extension status
- [x] Document custom local-server integration supported by current Zed

## Phase 7 — Hardening and release

- [x] Storage/index corruption and concurrency checks; focused static security review
- [x] Cargo install packaging instructions, workflow examples, and contribution guide
