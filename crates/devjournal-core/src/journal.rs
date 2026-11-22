use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

use crate::{
    Author, Category, Entry, EntrySource, Error, JournalConfig, Relationship, RelationshipRemoval,
    Result,
};

pub struct Journal {
    root: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecoveryReport {
    pub found: Vec<PathBuf>,
    pub removed: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ProjectOverview {
    pub project: crate::ProjectConfig,
    pub total_entries: usize,
    pub entries_by_category: std::collections::BTreeMap<String, usize>,
    pub recent_decisions: Vec<Entry>,
    pub open_tasks: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SessionSummary {
    pub session: Entry,
    pub linked_entries: Vec<Entry>,
    pub markdown: String,
}

impl Journal {
    pub fn init(project_dir: impl AsRef<Path>, config: JournalConfig) -> Result<Self> {
        let root = project_dir.as_ref().join(".devjournal");
        fs::create_dir_all(&root)?;
        ensure_real_directory(&root)?;
        for dir in [
            "sessions",
            "decisions",
            "notes",
            "research",
            "debugging",
            "meetings",
            "tasks",
            "knowledge",
            "entries",
            "relationships",
            "relationship-removals",
        ] {
            let path = root.join(dir);
            fs::create_dir_all(&path)?;
            ensure_real_directory(&path)?;
        }
        let config_path = root.join("config.toml");
        ensure_safe_file_target(&config_path)?;
        if !config_path.exists() {
            config.write(&config_path)?;
        }
        let ignore_path = root.join(".gitignore");
        ensure_safe_file_target(&ignore_path)?;
        if !ignore_path.exists() {
            fs::write(ignore_path, "/index.sqlite3*\n")?;
        }
        for (name, body) in [
            (
                "JOURNAL.md",
                "# Journal\n\nChronological development records are stored by entry in this directory.\n",
            ),
            (
                "PROJECT.md",
                "# Project\n\nAdd a human-reviewed project description here.\n",
            ),
            (
                "ARCHITECTURE.md",
                "# Architecture\n\nAdd current, human-reviewed architecture knowledge here.\n",
            ),
        ] {
            let path = root.join(name);
            ensure_safe_file_target(&path)?;
            if !path.exists() {
                fs::write(path, body)?;
            }
        }
        ensure_regular_file(&config_path)?;
        Ok(Self { root })
    }

    pub fn open(project_dir: impl AsRef<Path>) -> Result<Self> {
        let root = project_dir.as_ref().join(".devjournal");
        if !root.is_dir() {
            return Err(Error::NotFound(".devjournal directory".into()));
        }
        ensure_real_directory(&root)?;
        for dir in entry_folders()
            .into_iter()
            .chain(["relationships", "relationship-removals"])
        {
            ensure_real_directory(&root.join(dir))?;
        }
        ensure_regular_file(&root.join("config.toml"))?;
        JournalConfig::read(&root.join("config.toml"))?;
        Ok(Self { root })
    }

    pub fn config(&self) -> Result<JournalConfig> {
        JournalConfig::read(&self.root.join("config.toml"))
    }

    pub fn create_entry(&self, entry: &Entry) -> Result<PathBuf> {
        entry.validate()?;
        for related_id in &entry.related {
            if related_id == &entry.id {
                return Err(Error::Invalid("entry cannot relate to itself".into()));
            }
            self.get_entry(related_id)?;
        }
        let path = self.entry_path(&entry.id, entry.category)?;
        let parent = path.parent().expect("entry path has parent");
        fs::create_dir_all(parent)?;
        write_new(&path, entry.to_markdown()?.as_bytes(), |text| {
            let parsed = Entry::from_markdown(text)?;
            if parsed != *entry {
                return Err(Error::Invalid("serialized entry failed validation".into()));
            }
            Ok(())
        })
        .map_err(|error| match error {
            Error::AlreadyExists(_) => Error::AlreadyExists(entry.id.clone()),
            other => other,
        })?;
        Ok(path)
    }

    pub fn create_relationship(&self, relationship: &Relationship) -> Result<PathBuf> {
        relationship.validate()?;
        self.get_entry(&relationship.source_id)?;
        self.get_entry(&relationship.target_id)?;
        for existing in self.relationships()? {
            if same_edge(&existing, relationship) {
                return Err(Error::AlreadyExists(format!(
                    "{}:{}->{}",
                    relationship.kind.as_str(),
                    relationship.source_id,
                    relationship.target_id
                )));
            }
        }
        let path = self
            .root
            .join("relationships")
            .join(format!("{}.md", relationship.id));
        write_new(&path, relationship.to_markdown()?.as_bytes(), |text| {
            let parsed = Relationship::from_markdown(text)?;
            if parsed != *relationship {
                return Err(Error::Invalid(
                    "serialized relationship failed validation".into(),
                ));
            }
            Ok(())
        })?;
        Ok(path)
    }

    pub fn get_relationship(&self, id: &str) -> Result<Relationship> {
        crate::relationship::validate_relationship_id(id)?;
        let path = self.root.join("relationships").join(format!("{id}.md"));
        if !path.exists() {
            return Err(Error::NotFound(id.to_owned()));
        }
        ensure_regular_file(&path)?;
        Relationship::from_markdown(&fs::read_to_string(path)?)
    }

    pub fn unlink_relationship(
        &self,
        id: &str,
        author: Author,
        provenance: EntrySource,
    ) -> Result<PathBuf> {
        let relationship = self.get_relationship(id)?;
        if self
            .root
            .join("relationship-removals")
            .join(format!("{id}.md"))
            .exists()
        {
            return Err(Error::AlreadyExists(format!(
                "removal for relationship {id}"
            )));
        }
        let removal = RelationshipRemoval::new(relationship.id, author, provenance);
        let path = self
            .root
            .join("relationship-removals")
            .join(format!("{id}.md"));
        write_new(&path, removal.to_markdown()?.as_bytes(), |text| {
            let parsed = RelationshipRemoval::from_markdown(text)?;
            if parsed != removal {
                return Err(Error::Invalid(
                    "serialized relationship removal failed validation".into(),
                ));
            }
            Ok(())
        })?;
        Ok(path)
    }

    pub fn relationships(&self) -> Result<Vec<Relationship>> {
        let entries = self.entries()?;
        let ids: std::collections::HashSet<_> =
            entries.iter().map(|entry| entry.id.as_str()).collect();
        let mut removed = std::collections::HashSet::new();
        for item in fs::read_dir(self.root.join("relationship-removals"))? {
            let path = item?.path();
            if path.extension().is_some_and(|extension| extension == "md") {
                ensure_regular_file(&path)?;
                let removal = RelationshipRemoval::from_markdown(&fs::read_to_string(&path)?)?;
                let filename = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or_default();
                if filename != removal.relationship_id {
                    return Err(Error::Invalid(format!(
                        "relationship-removal filename does not match {}",
                        removal.relationship_id
                    )));
                }
                if !removed.insert(removal.relationship_id.clone()) {
                    return Err(Error::Invalid(format!(
                        "duplicate relationship removal for {}",
                        removal.relationship_id
                    )));
                }
            }
        }
        let mut known_relationship_ids = std::collections::HashSet::new();
        let mut explicit_relationships = Vec::new();
        for item in fs::read_dir(self.root.join("relationships"))? {
            let path = item?.path();
            if path.extension().is_some_and(|extension| extension == "md") {
                ensure_regular_file(&path)?;
                let relation = Relationship::from_markdown(&fs::read_to_string(&path)?)?;
                let filename = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or_default();
                if filename != relation.id {
                    return Err(Error::Invalid(format!(
                        "relationship filename does not match {}",
                        relation.id
                    )));
                }
                if !known_relationship_ids.insert(relation.id.clone()) {
                    return Err(Error::Invalid(format!(
                        "duplicate relationship id {}",
                        relation.id
                    )));
                }
                if !removed.contains(&relation.id) {
                    validate_endpoints(&relation, &ids)?;
                    explicit_relationships.push(relation);
                }
            }
        }
        if let Some(id) = removed
            .iter()
            .find(|id| !known_relationship_ids.contains(*id))
        {
            return Err(Error::Invalid(format!(
                "removal references missing relationship {id}"
            )));
        }
        let mut relationships = explicit_relationships.clone();
        let mut legacy_edges = Vec::new();
        for entry in &entries {
            for related_id in &entry.related {
                validate_id(related_id)?;
                if !ids.contains(related_id.as_str()) {
                    return Err(Error::Invalid(format!(
                        "entry {} references missing related entry {}",
                        entry.id, related_id
                    )));
                }
                let relation = crate::relationship::legacy_relationship(entry, related_id);
                if explicit_relationships
                    .iter()
                    .any(|existing| same_edge(existing, &relation))
                {
                    continue;
                }
                if legacy_edges
                    .iter()
                    .any(|existing| same_edge(existing, &relation))
                {
                    return Err(Error::Invalid(format!(
                        "entry {} contains duplicate related ids",
                        entry.id
                    )));
                }
                legacy_edges.push(relation.clone());
                relationships.push(relation);
            }
        }
        relationships.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(relationships)
    }

    pub fn entry_files(&self) -> Result<Vec<(Entry, PathBuf, Vec<u8>)>> {
        let mut records = Vec::new();
        for folder in entry_folders() {
            for item in fs::read_dir(self.root.join(folder))? {
                let path = item?.path();
                if path.extension().is_some_and(|extension| extension == "md") {
                    ensure_regular_file(&path)?;
                    let bytes = fs::read(&path)?;
                    let text = std::str::from_utf8(&bytes)
                        .map_err(|error| Error::Invalid(error.to_string()))?;
                    records.push((Entry::from_markdown(text)?, path, bytes));
                }
            }
        }
        Ok(records)
    }

    pub fn index_path(&self) -> PathBuf {
        self.root.join("index.sqlite3")
    }

    pub(crate) fn ensure_index_ignored(&self) -> Result<()> {
        let path = self.root.join(".gitignore");
        ensure_safe_file_target(&path)?;
        if path.exists() {
            ensure_regular_file(&path)?;
        }
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        if contents
            .lines()
            .any(|line| matches!(line.trim(), "/index.sqlite3*" | "index.sqlite3*"))
        {
            return Ok(());
        }
        let prefix = if contents.is_empty() || contents.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        use std::io::Write;
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?
            .write_all(format!("{prefix}/index.sqlite3*\n").as_bytes())?;
        Ok(())
    }

    pub fn temporary_artifacts(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        for folder in [
            "sessions",
            "decisions",
            "notes",
            "research",
            "debugging",
            "meetings",
            "tasks",
            "entries",
            "relationships",
            "relationship-removals",
        ] {
            for item in fs::read_dir(self.root.join(folder))? {
                let path = item?.path();
                if path.is_file() && is_temp_artifact(&path) {
                    files.push(path);
                }
            }
        }
        files.sort();
        Ok(files)
    }

    pub fn cleanup_temporary_artifacts(&self) -> Result<RecoveryReport> {
        let found = self.temporary_artifacts()?;
        let mut removed = Vec::new();
        for path in &found {
            fs::remove_file(path)?;
            removed.push(path.clone());
        }
        Ok(RecoveryReport { found, removed })
    }

    pub fn get_entry(&self, id: &str) -> Result<Entry> {
        validate_id(id)?;
        for folder in entry_folders() {
            let path = self.root.join(folder).join(format!("{id}.md"));
            if path.exists() {
                ensure_regular_file(&path)?;
                return Entry::from_markdown(&fs::read_to_string(path)?);
            }
        }
        Err(Error::NotFound(id.to_owned()))
    }

    pub fn entry_history(&self, id: &str) -> Result<Vec<Entry>> {
        let selected = self.get_entry(id)?;
        let entries = self.entries()?;
        let metadata = entries
            .iter()
            .map(|entry| (entry.id.as_str(), (entry.category, entry.title.as_str())))
            .collect::<std::collections::HashMap<_, _>>();
        let relationships = self.relationships()?;
        let mut connected = std::collections::HashSet::from([id.to_owned()]);
        loop {
            let previous_size = connected.len();
            for relation in relationships.iter().filter(|relation| {
                let Some((source_category, source_title)) =
                    metadata.get(relation.source_id.as_str())
                else {
                    return false;
                };
                let Some((target_category, target_title)) =
                    metadata.get(relation.target_id.as_str())
                else {
                    return false;
                };
                *source_category == selected.category
                    && *target_category == selected.category
                    && (relation.kind == crate::RelationshipType::Supersedes
                        || (relation.kind == crate::RelationshipType::References
                            && source_title == target_title))
            }) {
                if connected.contains(&relation.source_id)
                    || connected.contains(&relation.target_id)
                {
                    connected.insert(relation.source_id.clone());
                    connected.insert(relation.target_id.clone());
                }
            }
            if connected.len() == previous_size {
                break;
            }
        }
        let mut history = entries
            .into_iter()
            .filter(|entry| connected.contains(&entry.id))
            .collect::<Vec<_>>();
        history.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(history)
    }

    pub fn project_overview(&self) -> Result<ProjectOverview> {
        let config = self.config()?;
        let entries = self.entries()?;
        let task_ids = entries
            .iter()
            .filter(|entry| entry.category == Category::Task)
            .map(|entry| entry.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let superseded_tasks = self
            .relationships()?
            .into_iter()
            .filter(|relation| {
                relation.kind == crate::RelationshipType::Supersedes
                    && task_ids.contains(relation.source_id.as_str())
                    && task_ids.contains(relation.target_id.as_str())
            })
            .map(|relation| relation.target_id)
            .collect::<std::collections::HashSet<_>>();
        let mut entries_by_category = std::collections::BTreeMap::new();
        for entry in &entries {
            *entries_by_category
                .entry(entry.category.as_str().to_owned())
                .or_insert(0) += 1;
        }
        let mut recent_decisions = entries
            .iter()
            .filter(|entry| entry.category == Category::Decision)
            .cloned()
            .collect::<Vec<_>>();
        recent_decisions.reverse();
        recent_decisions.truncate(5);
        let mut open_tasks = entries
            .into_iter()
            .filter(|entry| {
                entry.category == Category::Task
                    && !superseded_tasks.contains(&entry.id)
                    && !entry.status.as_deref().is_some_and(|status| {
                        matches!(
                            status.to_ascii_lowercase().as_str(),
                            "done" | "complete" | "completed" | "closed" | "cancelled" | "canceled"
                        )
                    })
            })
            .collect::<Vec<_>>();
        open_tasks.reverse();
        open_tasks.truncate(20);
        Ok(ProjectOverview {
            project: config.project,
            total_entries: entries_by_category.values().sum(),
            entries_by_category,
            recent_decisions,
            open_tasks,
        })
    }

    pub fn session_summary(&self, id: &str) -> Result<SessionSummary> {
        let session = self.get_entry(id)?;
        if session.category != Category::Session {
            return Err(Error::Invalid(format!("{id} is not a session")));
        }
        let mut linked_ids = std::collections::HashSet::new();
        for relation in self.relationships()? {
            if relation.source_id == id && relation.target_id != id {
                linked_ids.insert(relation.target_id);
            } else if relation.target_id == id && relation.source_id != id {
                linked_ids.insert(relation.source_id);
            }
        }
        let mut linked_entries = linked_ids
            .into_iter()
            .map(|entry_id| self.get_entry(&entry_id))
            .collect::<Result<Vec<_>>>()?;
        linked_entries.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        let mut markdown = format!(
            "# Session summary: {}\n\nGenerated deterministically from the saved session and linked records.\n\n## Session record\n\n{}\n",
            session.title, session.content
        );
        if linked_entries.is_empty() {
            markdown.push_str("\n## Linked records\n\nNo linked records.\n");
        } else {
            markdown.push_str("\n## Linked records\n");
            for entry in &linked_entries {
                markdown.push_str(&format!(
                    "\n### {} ({})\n\n{}\n",
                    entry.title,
                    entry.category.as_str(),
                    entry.content
                ));
            }
        }
        Ok(SessionSummary {
            session,
            linked_entries,
            markdown,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn entries(&self) -> Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for folder in entry_folders() {
            let dir = self.root.join(folder);
            for item in fs::read_dir(dir)? {
                let path = item?.path();
                if path.extension().is_some_and(|extension| extension == "md") {
                    ensure_regular_file(&path)?;
                    entries.push(Entry::from_markdown(&fs::read_to_string(path)?)?);
                }
            }
        }
        entries.sort_by_key(|entry| entry.created_at);
        Ok(entries)
    }

    pub fn search(&self, query: &str, category: Option<Category>) -> Result<Vec<Entry>> {
        let query = query.to_lowercase();
        Ok(self
            .entries()?
            .into_iter()
            .filter(|entry| {
                category.is_none_or(|category| entry.category == category)
                    && (entry.title.to_lowercase().contains(&query)
                        || entry.content.to_lowercase().contains(&query)
                        || entry
                            .tags
                            .iter()
                            .any(|tag| tag.to_lowercase().contains(&query)))
            })
            .collect())
    }

    fn entry_path(&self, id: &str, category: Category) -> Result<PathBuf> {
        validate_id(id)?;
        let folder = match category {
            Category::Session => "sessions",
            Category::Decision => "decisions",
            Category::Note => "notes",
            Category::Research => "research",
            Category::Debugging => "debugging",
            Category::Meeting => "meetings",
            Category::Task => "tasks",
            _ => "entries",
        };
        Ok(self.root.join(folder).join(format!("{id}.md")))
    }
}

fn validate_id(id: &str) -> Result<()> {
    if id.len() < 4
        || id.len() > 80
        || !id.starts_with("dj-")
        || !id[3..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(Error::Invalid("unsafe entry id".into()));
    }
    Ok(())
}

fn ensure_real_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::Invalid(format!(
            "journal directory must not be a symlink: {}",
            path.display()
        )));
    }
    Ok(())
}

fn ensure_regular_file(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::Invalid(format!(
            "journal file must be a regular file, not a symlink: {}",
            path.display()
        )));
    }
    Ok(())
}

fn ensure_safe_file_target(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(Error::Invalid(format!(
                "journal file target must be a regular file: {}",
                path.display()
            )))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn entry_folders() -> [&'static str; 8] {
    [
        "sessions",
        "decisions",
        "notes",
        "research",
        "debugging",
        "meetings",
        "tasks",
        "entries",
    ]
}

fn is_temp_artifact(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    let Some(rest) = name.strip_prefix(".devjournal-entry-") else {
        return false;
    };
    let Some((id, nonce)) = rest.split_once(".tmp-") else {
        return false;
    };
    (validate_id(id).is_ok() || crate::relationship::validate_relationship_id(id).is_ok())
        && Uuid::parse_str(nonce).is_ok()
}

fn same_edge(left: &Relationship, right: &Relationship) -> bool {
    if left.kind != right.kind {
        return false;
    }
    if left.kind.is_symmetric() {
        (left.source_id == right.source_id && left.target_id == right.target_id)
            || (left.source_id == right.target_id && left.target_id == right.source_id)
    } else {
        left.source_id == right.source_id && left.target_id == right.target_id
    }
}

fn validate_endpoints(
    relation: &Relationship,
    ids: &std::collections::HashSet<&str>,
) -> Result<()> {
    if !ids.contains(relation.source_id.as_str()) || !ids.contains(relation.target_id.as_str()) {
        return Err(Error::Invalid(format!(
            "relationship {} references a missing entry",
            relation.id
        )));
    }
    Ok(())
}

fn write_new<F>(path: &Path, contents: &[u8], validate: F) -> Result<()>
where
    F: FnOnce(&str) -> Result<()>,
{
    let parent = path.parent().expect("entry path has parent");
    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::Invalid("entry filename is invalid".into()))?;
    let temp = parent.join(format!(".devjournal-entry-{id}.tmp-{}", Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(contents)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);

        let completed = fs::read_to_string(&temp)?;
        validate(&completed)?;

        commit_temp(&temp, path)?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(windows)]
fn commit_temp(from: &Path, to: &Path) -> Result<()> {
    move_file_no_replace(from, to).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            Error::AlreadyExists(
                to.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            )
        } else {
            Error::Io(error)
        }
    })
}

#[cfg(not(windows))]
fn commit_temp(from: &Path, to: &Path) -> Result<()> {
    fs::hard_link(from, to).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            Error::AlreadyExists(
                to.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            )
        } else {
            Error::Io(error)
        }
    })?;
    fs::remove_file(from)?;
    Ok(())
}

#[cfg(windows)]
fn move_file_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::MoveFileW;

    let from_display = from.display().to_string();
    let to_display = to.display().to_string();
    let from_name = extended_windows_path(from);
    let to_name = extended_windows_path(to);
    let from_exists = from.exists();
    let parent_exists = to.parent().is_some_and(Path::is_dir);
    let from: Vec<u16> = from_name.encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to_name.encode_wide().chain(Some(0)).collect();
    // MoveFileW on the same volume is atomic and fails when `to` already exists.
    let result = unsafe { MoveFileW(from.as_ptr(), to.as_ptr()) };
    if result == 0 {
        let error = std::io::Error::last_os_error();
        Err(std::io::Error::new(
            error.kind(),
            format!(
                "publishing {} (exists={from_exists}) to {} (parent exists={parent_exists}): {error}",
                from_display, to_display
            ),
        ))
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn extended_windows_path(path: &Path) -> std::ffi::OsString {
    let text = path.as_os_str().to_string_lossy();
    if text.starts_with(r"\\?\") || !path.is_absolute() {
        path.as_os_str().to_owned()
    } else if let Some(unc) = text.strip_prefix(r"\\") {
        std::ffi::OsString::from(format!(r"\\?\UNC\{unc}"))
    } else {
        std::ffi::OsString::from(format!(r"\\?\{text}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Author, AuthorType, Entry, EntrySource, EntrySourceType, Relationship, RelationshipType,
    };

    #[test]
    fn initializes_and_round_trips_human_entry() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::init(dir.path(), JournalConfig::defaults("demo")).unwrap();
        let mut entry = Entry::new(
            "First note",
            Category::Note,
            "A portable record.",
            Author {
                kind: AuthorType::Human,
                name: "Ada".into(),
            },
        );
        entry.tags.push("setup".into());
        journal.create_entry(&entry).unwrap();
        assert_eq!(journal.get_entry(&entry.id).unwrap(), entry);
        assert!(journal.root().join("config.toml").is_file());
    }

    #[test]
    fn refuses_path_traversal_and_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::init(dir.path(), JournalConfig::defaults("demo")).unwrap();
        let mut entry = Entry::new(
            "First",
            Category::Note,
            "body",
            Author {
                kind: AuthorType::Human,
                name: "Ada".into(),
            },
        );
        entry.id = "dj-..-outside".into();
        assert!(journal.create_entry(&entry).is_err());
        entry.id = "dj-fixed".into();
        journal.create_entry(&entry).unwrap();
        assert!(journal.create_entry(&entry).is_err());
    }

    #[test]
    fn rejects_invalid_entry_before_creating_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::init(dir.path(), JournalConfig::defaults("demo")).unwrap();
        let entry = Entry::new("  ", Category::Note, "body", author());
        assert!(journal.create_entry(&entry).is_err());
        assert!(journal.entries().unwrap().is_empty());
        assert!(journal.temporary_artifacts().unwrap().is_empty());
    }

    #[test]
    fn failed_validation_cleans_temp_and_preserves_existing_entry() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::init(dir.path(), JournalConfig::defaults("demo")).unwrap();
        let entry = Entry::new("First", Category::Note, "valid body", author());
        let path = journal.create_entry(&entry).unwrap();
        let original = fs::read(&path).unwrap();
        assert!(
            write_new(&path, b"broken", |_| Err(Error::Invalid(
                "simulated failure".into()
            )))
            .is_err()
        );
        assert_eq!(fs::read(path).unwrap(), original);
        assert!(journal.temporary_artifacts().unwrap().is_empty());
    }

    #[test]
    fn reports_and_cleans_only_named_stale_temp_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::init(dir.path(), JournalConfig::defaults("demo")).unwrap();
        let temp = journal
            .root
            .join("notes")
            .join(format!(".devjournal-entry-dj-stale.tmp-{}", Uuid::new_v4()));
        fs::write(&temp, "partial").unwrap();
        fs::write(journal.root.join("notes/.unrelated.tmp"), "keep").unwrap();
        let report = journal.cleanup_temporary_artifacts().unwrap();
        assert_eq!(report.found, vec![temp.clone()]);
        assert_eq!(report.removed, vec![temp.clone()]);
        assert!(!temp.exists());
        assert!(journal.root.join("notes/.unrelated.tmp").exists());
    }

    #[test]
    fn concurrent_duplicate_creation_has_one_winner() {
        let dir = tempfile::tempdir().unwrap();
        let journal = std::sync::Arc::new(
            Journal::init(dir.path(), JournalConfig::defaults("demo")).unwrap(),
        );
        let entry = std::sync::Arc::new(Entry::new("Concurrent", Category::Note, "one", author()));
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let journal = journal.clone();
                let entry = entry.clone();
                std::thread::spawn(move || journal.create_entry(&entry).is_ok())
            })
            .collect();
        let successes = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .filter(|success| *success)
            .count();
        assert_eq!(successes, 1);
        assert_eq!(journal.entries().unwrap().len(), 1);
    }

    fn author() -> Author {
        Author {
            kind: AuthorType::Human,
            name: "Ada".into(),
        }
    }

    #[test]
    fn persists_relationship_provenance_and_unlinks_with_a_tombstone() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::init(dir.path(), JournalConfig::defaults("relations")).unwrap();
        let first = Entry::new("Decision", Category::Decision, "chosen", author());
        let second = Entry::new(
            "Implementation",
            Category::Note,
            "uses the decision",
            author(),
        );
        journal.create_entry(&first).unwrap();
        journal.create_entry(&second).unwrap();
        let source = EntrySource {
            kind: EntrySourceType::Manual,
            detail: Some("reviewed in planning".into()),
        };
        let relation = Relationship::new(
            first.id.clone(),
            second.id.clone(),
            RelationshipType::RelatesTo,
            author(),
            source.clone(),
        );
        journal.create_relationship(&relation).unwrap();
        let loaded = journal.relationships().unwrap();
        assert_eq!(loaded, vec![relation.clone()]);
        assert_eq!(loaded[0].provenance, source);
        assert!(journal.root.join("relationship-removals").is_dir());
        assert!(
            journal
                .create_relationship(&Relationship::new(
                    second.id.clone(),
                    first.id.clone(),
                    RelationshipType::RelatesTo,
                    author(),
                    EntrySource {
                        kind: EntrySourceType::Manual,
                        detail: None
                    }
                ))
                .is_err()
        );

        journal
            .unlink_relationship(
                &relation.id,
                author(),
                EntrySource {
                    kind: EntrySourceType::Manual,
                    detail: None,
                },
            )
            .unwrap();
        assert!(journal.relationships().unwrap().is_empty());
        assert!(
            journal
                .root
                .join("relationships")
                .join(format!("{}.md", relation.id))
                .exists()
        );
        assert!(
            journal
                .root
                .join("relationship-removals")
                .join(format!("{}.md", relation.id))
                .exists()
        );
    }

    #[test]
    fn rejects_relationships_with_missing_endpoints() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::init(dir.path(), JournalConfig::defaults("relations")).unwrap();
        let first = Entry::new("Entry", Category::Note, "body", author());
        journal.create_entry(&first).unwrap();
        let relation = Relationship::new(
            first.id.clone(),
            "dj-missing",
            RelationshipType::References,
            author(),
            EntrySource {
                kind: EntrySourceType::Manual,
                detail: None,
            },
        );
        assert!(journal.create_relationship(&relation).is_err());
    }
}
