use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{Category, Entry, Error, JournalConfig, Result};

pub struct Journal {
    root: PathBuf,
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
        write_new(&path, entry.to_markdown()?.as_bytes())?;
        Ok(path)
    }

    pub fn get_entry(&self, id: &str) -> Result<Entry> {
        validate_id(id)?;
        let paths = [
            "sessions",
            "decisions",
            "notes",
            "research",
            "debugging",
            "meetings",
            "tasks",
            "entries",
        ];
        for folder in paths {
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
        for folder in [
            "sessions",
            "decisions",
            "notes",
            "research",
            "debugging",
            "meetings",
            "tasks",
            "entries",
        ] {
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
    if id.len() > 80
        || !id.starts_with("dj-")
        || !id[3..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(Error::Invalid("unsafe entry id".into()));
    }
    Ok(())
}

fn write_new(path: &Path, contents: &[u8]) -> Result<()> {
    use std::io::Write;
    // Exclusive creation is portable across filesystems; an interrupted write can leave a partial file.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(contents)?;
    file.sync_all()?;
    Ok(())
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
}
