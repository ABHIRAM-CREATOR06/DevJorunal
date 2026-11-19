# Architecture

## Core and portable source

The workspace has one editor-independent crate, `devjournal-core`. It owns the entry model, project config, validation, and Markdown persistence. The `.devjournal` folder is the portable source of truth; all created entries are files, not database rows.

Each entry records category, author type/name, source, timestamps, tags, legacy related IDs, and optional status. An AI author remains explicitly marked `ai`; writing an entry does not promote it into verified human knowledge. Humans can review and edit the Markdown directly.

Entry writes stage a unique file in the destination directory, flush and sync it, parse it back and compare it with the requested entry, then publish it with a no-replace operation. Windows uses same-volume `MoveFileW`; Unix uses hard-link publication, then removes the temporary name and syncs the parent directory. Concurrent duplicate IDs cannot overwrite one another. Failed writes clean up their temp; interruption can leave a named temp for explicit `recover` inspection or cleanup.

## Planned boundaries

- CLI and MCP will call the core library and remain separate binaries.
- Markdown stays authoritative; SQLite FTS will be derived and rebuildable.
- Editor integrations are optional clients of the CLI/MCP surface.
- No network client or AI provider belongs in the core.

## Knowledge graph

Every entry ID is a graph node; the graph holds node IDs and edge metadata, never copies entry bodies. Typed relationships are separate JSON-compatible Markdown records under `.devjournal/relationships/`. Their source/target order expresses direction. `relates_to` is the sole symmetric edge and is treated as adjacent from either endpoint without writing inverse records. All other edges are directional: the source is the subject of the relation (for example, a task `depends_on` its target; a newer decision `supersedes` an older one; an issue `caused_by` its cause; a finding is `discovered_during` a session). Self-edges and dangling endpoints are rejected. Duplicate directed edges are rejected; reversed `relates_to` duplicates are also rejected.

`devjournal unlink` writes a relationship-removal tombstone. This keeps the historical edge auditable while removing it from the active graph. Existing entry `related` IDs are projected as legacy `references` edges so previous task-revision records continue to connect.

The graph engine is independent of SQL. It accepts a node-ID set and typed edges, and provides outgoing/incoming queries, incident neighbors, deterministic breadth-first traversal capped at depth 32, directed path search (symmetric edges can be traversed either way), and directed-cycle detection. Query helpers cover implementations, causes, superseding decisions, blocking tasks, and sessions associated with issues.

## SQLite indexing and context

Rusqlite with bundled SQLite provides `.devjournal/index.sqlite3`; FTS5 indexes entry title, body, and tags. Metadata and relationship tables store IDs and searchable fields, not entry bodies. The Markdown tree remains authoritative. Sync fingerprints Markdown files to skip unchanged entries, upserts changed entries/relationships, and removes stale index rows. CLI search, context, and graph operations sync before querying, which also notices manual edits and deletions. `index verify` compares source and index fingerprints/IDs and checks FTS integrity. `index rebuild` reindexes from Markdown; a corrupt database is retained as a `.corrupt-*` backup before rebuilding. `.devjournal/.gitignore` keeps derived SQLite files out of Git.

Context retrieval ranks direct FTS matches first, then adds unique graph neighbors breadth-first up to the requested depth/limit. Each item includes a deterministic reason (`direct_text_match`, `requested_entry`, `recent_project_activity`, or the edge type and parent entry ID). The full entry is retrieved from Markdown, retaining authorship and provenance. No embeddings or model calls are involved.

## Security

Entry and relationship IDs are constrained before they are joined to storage paths. Entries and relationships are treated as data and never executed. Atomic create-only writes preserve existing files. Config parsing validates supported values and required project name. The current parser handles the documented config subset; broad TOML compatibility and concurrent update/locking semantics remain future work.
