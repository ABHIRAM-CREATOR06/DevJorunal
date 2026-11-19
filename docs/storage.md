# Storage format

`.devjournal/config.toml` contains local project configuration. Initialization creates `JOURNAL.md`, `PROJECT.md`, `ARCHITECTURE.md`, and folders for sessions, decisions, notes, research, debugging, meetings, tasks, knowledge, and generic entries. Existing files are left intact.

Each new record is a standalone Markdown file named `<id>.md`. The opening `---` block contains one JSON object, which is valid YAML 1.2 front matter. The JSON encoding keeps quoting, Unicode, and multiline metadata unambiguous while remaining readable by ordinary tools. The rest of the file is Markdown content. The record contains stable ID, title, category, author and source provenance, UTC timestamps, tags, related IDs, and optional status.

Creation refuses to replace an existing file and flushes written data. A process interruption during creation can leave a partial file. Edit records with a Markdown editor only when you have separately preserved the history you need; a revision workflow is planned. SQLite is not part of the current implementation and must remain derived when introduced.
