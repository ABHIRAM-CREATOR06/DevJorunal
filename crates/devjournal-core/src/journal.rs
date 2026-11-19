use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

use crate::{Category, Entry, Error, JournalConfig, Result};

pub struct Journal {
    root: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecoveryReport {
    pub found: Vec<PathBuf>,
    pub removed: Vec<PathBuf>,
}

impl Journal {
    pub fn init(project_dir: impl AsRef<Path>, config: JournalConfig) -> Result<Self> {
        let root = project_dir.as_ref().join(".devjournal");
        fs::create_dir_all(&root)?;
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
        ] {
            fs::create_dir_all(root.join(dir))?;
        }
        let config_path = root.join("config.toml");
        if !config_path.exists() {
            config.write(&config_path)?;
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
            if !path.exists() {
                fs::write(path, body)?;
            }
        }
        Ok(Self { root })
    }

    pub fn open(project_dir: impl AsRef<Path>) -> Result<Self> {
        let root = project_dir.as_ref().join(".devjournal");
        if !root.is_dir() {
            return Err(Error::NotFound(".devjournal directory".into()));
        }
        JournalConfig::read(&root.join("config.toml"))?;
        Ok(Self { root })
    }

    pub fn config(&self) -> Result<JournalConfig> {
        JournalConfig::read(&self.root.join("config.toml"))
    }

    pub fn create_entry(&self, entry: &Entry) -> Result<PathBuf> {
        entry.validate()?;
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

    pub fn temporary_artifacts(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        for folder in entry_folders() {
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
            if path.is_file() {
                return Entry::from_markdown(&fs::read_to_string(path)?);
            }
        }
        Err(Error::NotFound(id.to_owned()))
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
    validate_id(id).is_ok() && Uuid::parse_str(nonce).is_ok()
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

    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // MoveFileW on the same volume is atomic and fails when `to` already exists.
    let result = unsafe { MoveFileW(from.as_ptr(), to.as_ptr()) };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Author, AuthorType, Entry};

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
}
