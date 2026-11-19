use std::{fs, path::Path};

use crate::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
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
        let text = format!(
            "[project]\nname = {}\ndescription = {}\n\n[privacy]\nlocal_only = {}\nstore_transcripts = {}\n\n[sessions]\nrequire_review = {}\nauto_create = false\n\n[agents]\nallow_writes = {}\nrequire_confirmation_for_decisions = true\n",
            quote(&self.project.name),
            quote(&self.project.description),
            self.local_only,
            self.store_transcripts,
            self.require_review,
            self.allow_agent_writes
        );
        fs::write(path, text)?;
        Ok(())
    }

    pub(crate) fn read(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        let mut section = "";
        let mut config = Self::defaults("");
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = &line[1..line.len() - 1];
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| Error::Invalid(format!("invalid config line: {line}")))?;
            let key = key.trim();
            let value = value.trim();
            match (section, key) {
                ("project", "name") => config.project.name = unquote(value)?,
                ("project", "description") => config.project.description = unquote(value)?,
                ("privacy", "local_only") => config.local_only = parse_bool(value)?,
                ("privacy", "store_transcripts") => config.store_transcripts = parse_bool(value)?,
                ("sessions", "require_review") => config.require_review = parse_bool(value)?,
                ("agents", "allow_writes") => config.allow_agent_writes = parse_bool(value)?,
                _ => {}
            }
        }
        if config.project.name.trim().is_empty() {
            return Err(Error::Invalid("project.name is required".into()));
        }
        Ok(config)
    }
}

fn quote(s: &str) -> String {
    format!(
        "\"{}\"",
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}
fn unquote(s: &str) -> Result<String> {
    if !s.starts_with('"') || !s.ends_with('"') {
        return Err(Error::Invalid("expected quoted string in config".into()));
    }
    serde_json_unescape(&s[1..s.len() - 1])
}
fn serde_json_unescape(s: &str) -> Result<String> {
    // TOML basic strings and JSON strings share the escapes emitted by quote().
    let quoted = format!("\"{s}\"");
    serde_json::from_str(&quoted).map_err(|e| Error::Invalid(format!("invalid config string: {e}")))
}
fn parse_bool(s: &str) -> Result<bool> {
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(Error::Invalid(format!("expected boolean, got {s}"))),
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
}
