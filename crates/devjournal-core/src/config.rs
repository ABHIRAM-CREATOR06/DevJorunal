use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

use crate::{Error, Result};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalConfig {
    pub project: ProjectConfig,
    pub local_only: bool,
    pub store_transcripts: bool,
    pub require_review: bool,
    pub allow_agent_writes: bool,
}

impl JournalConfig {
    pub fn defaults(name: impl Into<String>) -> Self {
        Self {
            project: ProjectConfig {
                name: name.into(),
                description: String::new(),
            },
            local_only: true,
            store_transcripts: false,
            require_review: true,
            allow_agent_writes: true,
        }
    }

    pub(crate) fn write(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(&ConfigFile::from(self))
            .map_err(|error| Error::Invalid(format!("could not encode config: {error}")))?;
        fs::write(path, text)?;
        Ok(())
    }

    pub(crate) fn read(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        let file: ConfigFile = toml::from_str(&text)
            .map_err(|error| Error::Invalid(format!("invalid TOML config: {error}")))?;
        let config = Self {
            project: file.project,
            local_only: file.privacy.local_only,
            store_transcripts: file.privacy.store_transcripts,
            require_review: file.sessions.require_review,
            allow_agent_writes: file.agents.allow_writes,
        };
        if config.project.name.trim().is_empty() {
            return Err(Error::Invalid("project.name is required".into()));
        }
        Ok(config)
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct ConfigFile {
    project: ProjectConfig,
    privacy: PrivacyConfig,
    sessions: SessionsConfig,
    agents: AgentsConfig,
}

impl From<&JournalConfig> for ConfigFile {
    fn from(config: &JournalConfig) -> Self {
        Self {
            project: config.project.clone(),
            privacy: PrivacyConfig {
                local_only: config.local_only,
                store_transcripts: config.store_transcripts,
            },
            sessions: SessionsConfig {
                require_review: config.require_review,
                ..SessionsConfig::default()
            },
            agents: AgentsConfig {
                allow_writes: config.allow_agent_writes,
                ..AgentsConfig::default()
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct PrivacyConfig {
    local_only: bool,
    store_transcripts: bool,
}
impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            local_only: true,
            store_transcripts: false,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct SessionsConfig {
    require_review: bool,
    auto_create: bool,
}
impl Default for SessionsConfig {
    fn default() -> Self {
        Self {
            require_review: true,
            auto_create: false,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct AgentsConfig {
    #[serde(rename = "allow_writes")]
    allow_writes: bool,
    require_confirmation_for_decisions: bool,
}
impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            allow_writes: true,
            require_confirmation_for_decisions: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trips_quoted_values_and_validates_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut config = JournalConfig::defaults("demo \"project\"");
        config.project.description = "two lines\nand a path \\".into();
        config.write(&path).unwrap();
        assert_eq!(JournalConfig::read(&path).unwrap(), config);
        fs::write(&path, "[project]\nname = \"\"\ndescription = \"\"\n").unwrap();
        assert!(JournalConfig::read(&path).is_err());
    }

    #[test]
    fn reads_standard_toml_syntax_comments_tables_and_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            r#"# Config produced by a TOML editor
project.name = 'sample project'
project.description = """A multiline
project description."""

[privacy]
local_only = false # sync is managed elsewhere
store_transcripts = true

[sessions]
require_review = false
auto_create = true

[agents]
allow_writes = false
require_confirmation_for_decisions = true
future_setting = ["preserved by TOML parser", 7]
"#,
        )
        .unwrap();

        let config = JournalConfig::read(&path).unwrap();
        assert_eq!(config.project.name, "sample project");
        assert_eq!(
            config.project.description,
            "A multiline\nproject description."
        );
        assert!(!config.local_only);
        assert!(config.store_transcripts);
        assert!(!config.require_review);
        assert!(!config.allow_agent_writes);
    }

    #[test]
    fn rejects_invalid_toml_and_wrong_field_types() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[project\nname = 'broken'").unwrap();
        assert!(JournalConfig::read(&path).is_err());
        fs::write(
            &path,
            "[project]\nname = 'demo'\n[privacy]\nlocal_only = 'yes'",
        )
        .unwrap();
        assert!(JournalConfig::read(&path).is_err());
    }
}
