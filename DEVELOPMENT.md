# Development progress

## Phase 1 — Core foundation (complete)

Implemented the Rust workspace, core entry model, category and provenance types, validation, `.devjournal` initialization, default project configuration, and create-only Markdown storage with retrieval by ID. New records are stored in category folders where defined and under `entries/` for other categories. Existing records and project files are preserved during initialization.

Tests cover initialization and entry round-trip, config round-trip/validation, plus path traversal and overwrite refusal. `cargo test --workspace --offline` passed (3 tests). `cargo fmt --all` and `cargo fmt --all -- --check` passed. A CLI smoke test found hard-link creation is denied in this OneDrive workspace, so entry writes use exclusive file creation and flushing instead; interrupted writes can leave a partial file.

## Phase 2 — CLI (in progress)

Implemented `init`, `status`, `note new`, `decision new`, `session start/list`, `task list/update`, `search`, `context`, and `show`. Search scans Markdown directly. Task updates append a related follow-up record to preserve history. A smoke run is in progress after adapting storage for this filesystem. Remaining: JSON output, task revision browsing, and interactive input.

Next: finish the CLI smoke check, then add output and help documentation before moving to indexing.
