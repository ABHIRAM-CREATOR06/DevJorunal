# Roadmap

## Phase 1 — Core foundation (complete)

- [x] Rust workspace and core crate
- [x] Entry model, provenance, categories, and validation
- [x] Project initialization and default config
- [x] Markdown entry creation and retrieval
- [ ] Broader TOML parser and human-friendly update/revision workflow

## Phase 2 — CLI (in progress)

- [x] Clap CLI for initialization, notes, sessions, decisions, tasks, and status
- [ ] JSON output and direct task status/revision browsing

## Phase 3 — Indexing and search

- [ ] Rebuildable SQLite FTS5 index
- [ ] Metadata filters and index consistency checks

## Phase 4 — MCP server

- [ ] MCP tools, resources, validation, and integration guide

## Phase 5 — Agent continuity

- [ ] Context retrieval, session summaries, related knowledge, project overview

## Phase 6 — Zed integration

- [ ] Verify current official Zed extension and MCP support before implementation
- [ ] Implement only supported optional integration paths

## Phase 7 — Hardening and release

- [ ] Concurrency and corruption recovery, security review, packaging, examples, contribution docs
