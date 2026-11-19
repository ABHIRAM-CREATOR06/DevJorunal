# Roadmap

## Phase 1 — Core foundation (complete)

- [x] Rust workspace and core crate
- [x] Entry model, provenance, categories, and validation
- [x] Project initialization and default config
- [x] Markdown entry creation and retrieval
- [ ] Broader TOML parser and human-friendly update/revision workflow

## Phase 2 — CLI (complete)

- [x] Clap CLI for initialization, notes, sessions, decisions, tasks, and status
- [x] JSON output for status, search, context, lists, entry display, and recovery
- [x] Terminal-aware prompts with cancellation and non-interactive failure behavior
- [ ] Task revision history grouping in list output

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
- [ ] Metadata filters beyond category and query

## Phase 4 — MCP server

- [ ] MCP tools, resources, validation, and integration guide

## Phase 5 — Agent continuity

- [x] Deterministic graph-aware context retrieval and related knowledge
- [ ] Session summaries and project overview generation

## Phase 6 — Zed integration

- [ ] Verify current official Zed extension and MCP support before implementation
- [ ] Implement only supported optional integration paths

## Phase 7 — Hardening and release

- [ ] Concurrency and corruption recovery, security review, packaging, examples, contribution docs
