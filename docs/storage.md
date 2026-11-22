# Storage format

`.devjournal/config.toml` contains local project configuration. Initialization creates `JOURNAL.md`, `PROJECT.md`, `ARCHITECTURE.md`, and folders for sessions, decisions, notes, research, debugging, meetings, tasks, knowledge, and generic entries. Existing files are left intact.

Each entry is a standalone Markdown file named `<id>.md`. The opening `---` block contains one JSON object, which is valid YAML 1.2 front matter. The JSON encoding keeps quoting, Unicode, and multiline metadata unambiguous while remaining readable by ordinary tools. The rest of the file is Markdown content. The record contains stable ID, title, category, author and source provenance, UTC timestamps, tags, related IDs, and optional status.

Entry creation writes to a uniquely named temporary file in the entry's directory, flushes and synchronizes it, parses the completed Markdown back into an entry, and compares it with the requested record before publication. On Windows, a same-volume `MoveFileW` publishes the file and fails if the destination exists. On Unix, a hard link publishes it atomically without replacing an existing path, then the temporary name is removed. Unix also synchronizes the containing directory. Duplicate IDs therefore cannot overwrite records, including with concurrent writers. If the filesystem does not support hard links, publication fails safely and the temporary file is removed.

If a write or validation fails, DevJournal removes its temporary file. A process or machine interruption can still leave a temp behind. `devjournal recover` lists matching artifacts; `devjournal recover --clean` explicitly removes them. Recovery never promotes a temp, even if it parses as a valid entry. Only files matching DevJournal's exact temporary naming pattern are considered. These guarantees cover entry creation; initial config and template writes are not yet transactional. Hardware and filesystem caches can limit crash durability beyond the requested file and directory sync calls.

## Relationships and removals

Typed relationship edges are stored in `.devjournal/relationships/<djr-id>.md` using the same JSON-compatible YAML front matter convention. Records include source/target entry IDs, relationship type, author, provenance, timestamp, and optional string metadata. Entry bodies remain in their Markdown files.

Unlinking writes a tombstone to `.devjournal/relationship-removals/<djr-id>.md`; it never deletes the original edge record. Active graph reads ignore tombstoned edges and validate that tombstones point to stored relationship records. The legacy `related` entry metadata is still read as a directed `references` edge, so existing task follow-up links remain visible after upgrading. For revision history only, same-category, same-title legacy task references are recognized as prior task updates; other `references` edges retain their ordinary meaning.

## SQLite index

`.devjournal/index.sqlite3` is a derived SQLite database. It contains entry metadata and a rebuildable FTS5 index over titles, bodies, and tags, plus typed relationship rows. `.devjournal/.gitignore` excludes `index.sqlite3*` so only portable Markdown knowledge is committed. CLI search/context/graph operations incrementally synchronize the index with Markdown files; `devjournal index update` does the same explicitly. `index verify` compares indexed IDs and content fingerprints with source files. `index rebuild` reconstructs the database from Markdown. If the index is corrupt, rebuild preserves it under an `index.sqlite3.corrupt-*` name before building a fresh database. Removing the SQLite file never removes journal knowledge.

`entry revise` and `task update` write new records and connect them with a `supersedes` relationship. `entry history` reads the chain, and `task list` groups it with the latest record at the top. Older Markdown remains intact. If an active edge points to a missing entry, indexing fails visibly so the graph does not silently omit that relationship.

Journal initialization/open and source reads reject symlinked storage directories and files. The generated index path and `.gitignore` are also checked before use, so a repository cannot redirect DevJournal's normal journal/index operations through a static symlink.
