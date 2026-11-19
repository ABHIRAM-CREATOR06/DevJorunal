# Architecture

## Current Phase 1 design

The workspace has one editor-independent crate, `devjournal-core`. It owns the entry model, project config, validation, and Markdown persistence. The `.devjournal` folder is the portable source of truth; all created entries are files, not database rows.

Each entry records category, author type/name, source, timestamps, tags, relationships, and optional status. An AI author remains explicitly marked `ai`; writing an entry does not promote it into verified human knowledge. Humans can review and edit the Markdown directly.

Entry writes stage a unique file in the destination directory, flush and sync it, parse it back and compare it with the requested entry, then publish it with a no-replace operation. Windows uses same-volume `MoveFileW`; Unix uses hard-link publication, then removes the temporary name and syncs the parent directory. Concurrent duplicate IDs cannot overwrite one another. Failed writes clean up their temp; interruption can leave a named temp for explicit `recover` inspection or cleanup.

## Planned boundaries

- CLI and MCP will call the core library and remain separate binaries.
- Markdown stays authoritative; SQLite FTS will be derived and rebuildable.
- Editor integrations are optional clients of the CLI/MCP surface.
- No network client or AI provider belongs in the core.

## Security

Entry IDs are constrained before they are joined to storage paths. Entries are treated as data and never executed. Atomic create-only writes preserve existing files. Config parsing validates supported values and required project name. The current parser handles the documented config subset; broad TOML compatibility and concurrent update/locking semantics remain future work.
