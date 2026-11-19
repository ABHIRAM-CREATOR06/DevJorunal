# Architecture

## Current Phase 1 design

The workspace has one editor-independent crate, `devjournal-core`. It owns the entry model, project config, validation, and Markdown persistence. The `.devjournal` folder is the portable source of truth; all created entries are files, not database rows.

Each entry records category, author type/name, source, timestamps, tags, relationships, and optional status. An AI author remains explicitly marked `ai`; writing an entry does not promote it into verified human knowledge. Humans can review and edit the Markdown directly.

Writes create the final path exclusively and flush it, preventing simultaneous writers from silently replacing an existing record. Existing records are create-only in Phase 1. A process interruption during a write can leave a partial file; full atomic replacement remains an open implementation detail.

## Planned boundaries

- CLI and MCP will call the core library and remain separate binaries.
- Markdown stays authoritative; SQLite FTS will be derived and rebuildable.
- Editor integrations are optional clients of the CLI/MCP surface.
- No network client or AI provider belongs in the core.

## Security

Entry IDs are constrained before they are joined to storage paths. Entries are treated as data and never executed. Atomic create-only writes preserve existing files. Config parsing validates supported values and required project name. The current parser handles the documented config subset; broad TOML compatibility and concurrent update/locking semantics remain future work.
