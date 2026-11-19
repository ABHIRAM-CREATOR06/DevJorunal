use std::{
    io::{self, BufRead, IsTerminal, Write},
    path::PathBuf,
    process::ExitCode,
};

use clap::{Args, Parser, Subcommand, ValueEnum, error::ErrorKind};
use devjournal_core::{
    Author, AuthorType, Category, Entry, EntrySource, EntrySourceType, Error, Journal,
    JournalConfig,
};
use serde_json::{Value, json};

#[derive(Parser)]
#[command(
    name = "devjournal",
    version,
    about = "Local-first project development journal"
)]
struct Cli {
    #[arg(long, global = true, default_value = ".")]
    project: PathBuf,
    #[arg(long, global = true, help = "Emit machine-readable JSON")]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Init {
        #[arg(long)]
        name: Option<String>,
        #[arg(long, default_value = "")]
        description: String,
    },
    Status,
    Note {
        #[command(subcommand)]
        command: Option<NewCommand>,
    },
    Decision {
        #[command(subcommand)]
        command: Option<NewCommand>,
    },
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    Search {
        query: String,
        #[arg(long, value_enum)]
        category: Option<CategoryArg>,
    },
    Context {
        topic: Option<String>,
    },
    Entry {
        #[command(subcommand)]
        command: EntryCommand,
    },
    #[command(alias = "show")]
    LegacyShow {
        id: String,
    },
    Recover {
        #[arg(long, help = "Delete only recognized stale DevJournal temporary files")]
        clean: bool,
    },
}

#[derive(Subcommand)]
enum EntryCommand {
    Show { id: String },
}
#[derive(Subcommand)]
enum NewCommand {
    New(NewArgs),
}
#[derive(Args)]
struct NewArgs {
    title: Option<String>,
    #[arg(long)]
    content: Option<String>,
    #[arg(long, value_delimiter = ',')]
    tags: Vec<String>,
    #[arg(long, default_value = "human")]
    author: String,
}
#[derive(Subcommand)]
enum SessionCommand {
    Start {
        title: Option<String>,
        #[arg(long, default_value = "human")]
        author: String,
    },
    List,
}
#[derive(Subcommand)]
enum TaskCommand {
    List,
    Update {
        id: String,
        status: Option<String>,
        #[arg(long, default_value = "human")]
        author: String,
    },
}
#[derive(Clone, Copy, ValueEnum)]
enum CategoryArg {
    Session,
    Note,
    Decision,
    Architecture,
    Research,
    Debugging,
    Issue,
    Task,
    Meeting,
    Retrospective,
    Convention,
    Question,
}
impl From<CategoryArg> for Category {
    fn from(value: CategoryArg) -> Self {
        match value {
            CategoryArg::Session => Self::Session,
            CategoryArg::Note => Self::Note,
            CategoryArg::Decision => Self::Decision,
            CategoryArg::Architecture => Self::Architecture,
            CategoryArg::Research => Self::Research,
            CategoryArg::Debugging => Self::Debugging,
            CategoryArg::Issue => Self::Issue,
            CategoryArg::Task => Self::Task,
            CategoryArg::Meeting => Self::Meeting,
            CategoryArg::Retrospective => Self::Retrospective,
            CategoryArg::Convention => Self::Convention,
            CategoryArg::Question => Self::Question,
        }
    }
}

fn main() -> ExitCode {
    let json_requested = std::env::args_os().any(|arg| arg == "--json");
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                let _ = error.print();
                return ExitCode::SUCCESS;
            }
            if json_requested {
                emit_error("INVALID_ARGUMENTS", &error.to_string());
            } else {
                let _ = error.print();
            }
            return ExitCode::from(2);
        }
    };
    let json = cli.json;
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if json {
                let (code, message) = error_details(error.as_ref());
                emit_error(code, &message);
            } else {
                eprintln!("devjournal: {error}");
            }
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let Cli {
        project,
        json: as_json,
        command,
    } = cli;
    match command {
        Command::Init { name, description } => {
            let name = name.unwrap_or_else(|| {
                project
                    .file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or("project")
                    .to_owned()
            });
            let mut config = JournalConfig::defaults(name);
            config.project.description = description;
            let journal = Journal::init(&project, config)?;
            emit(
                as_json,
                json!({"journal": journal.root()}),
                format!("Initialized {}", journal.root().display()),
            );
        }
        Command::Status => {
            let journal = Journal::open(&project)?;
            let config = journal.config()?;
            let count = journal.entries()?.len();
            emit(
                as_json,
                json!({"project": config.project, "local_only": config.local_only, "entry_count": count}),
                format!(
                    "{} — {}\n{count} entries",
                    config.project.name,
                    if config.local_only {
                        "local-only"
                    } else {
                        "sync policy not configured"
                    }
                ),
            );
        }
        Command::Note { command } => {
            create_interactive(&project, Category::Note, command, as_json)?
        }
        Command::Decision { command } => {
            create_interactive(&project, Category::Decision, command, as_json)?
        }
        Command::Session {
            command: SessionCommand::Start { title, author },
        } => {
            let title = required_single(title, "Session title", as_json)?;
            let journal = Journal::open(&project)?;
            let mut entry = Entry::new(
                title,
                Category::Session,
                "Session started. Add a summary by editing this Markdown file.",
                author_record(&author),
            );
            entry.source = source_for(&author);
            let path = journal.create_entry(&entry)?;
            emit(
                as_json,
                json!({"path": path, "entry": entry}),
                path.display().to_string(),
            );
        }
        Command::Session {
            command: SessionCommand::List,
        } => list(&project, Category::Session, as_json)?,
        Command::Task {
            command: TaskCommand::List,
        } => list(&project, Category::Task, as_json)?,
        Command::Task {
            command: TaskCommand::Update { id, status, author },
        } => {
            let status = required_single(status, "Task status", as_json)?;
            let journal = Journal::open(&project)?;
            let old = journal.get_entry(&id)?;
            if old.category != Category::Task {
                return Err(Box::new(Error::Invalid(format!("{id} is not a task"))));
            }
            let mut update = Entry::new(
                old.title,
                Category::Task,
                old.content,
                author_record(&author),
            );
            update.source = source_for(&author);
            update.related.push(id);
            update.status = Some(status);
            let path = journal.create_entry(&update)?;
            emit(
                as_json,
                json!({"path": path, "entry": update}),
                format!("Created task revision {}", path.display()),
            );
        }
        Command::Search { query, category } => {
            let entries = Journal::open(&project)?.search(&query, category.map(Into::into))?;
            emit(as_json, json!(entries), format_entries(&entries));
        }
        Command::Context { topic } => {
            let journal = Journal::open(&project)?;
            let mut entries = if let Some(topic) = &topic {
                journal.search(topic, None)?
            } else {
                journal.entries()?
            };
            entries.reverse();
            entries.truncate(10);
            emit(
                as_json,
                json!({"topic": topic, "entries": entries}),
                format_entries(&entries),
            );
        }
        Command::Entry {
            command: EntryCommand::Show { id },
        }
        | Command::LegacyShow { id } => {
            let entry = Journal::open(&project)?.get_entry(&id)?;
            emit(
                as_json,
                json!(entry),
                format!(
                    "# {}\n\n{}\n\nID: {} | category: {} | author: {:?} {} | created: {}",
                    entry.title,
                    entry.content,
                    entry.id,
                    entry.category.as_str(),
                    entry.author.kind,
                    entry.author.name,
                    entry.created_at
                ),
            );
        }
        Command::Recover { clean } => {
            let journal = Journal::open(&project)?;
            let artifacts = if clean {
                journal.cleanup_temporary_artifacts()?.removed
            } else {
                journal.temporary_artifacts()?
            };
            let message = if artifacts.is_empty() {
                "No stale DevJournal temporary files found".to_owned()
            } else {
                format!(
                    "{} {} stale temporary file(s):\n{}",
                    if clean { "Removed" } else { "Found" },
                    artifacts.len(),
                    artifacts
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            };
            emit(
                as_json,
                json!({"cleaned": clean, "artifacts": artifacts}),
                message,
            );
        }
    }
    Ok(())
}

fn create_interactive(
    project: &PathBuf,
    category: Category,
    command: Option<NewCommand>,
    as_json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let args = match command {
        Some(NewCommand::New(args)) => args,
        None => NewArgs {
            title: None,
            content: None,
            tags: vec![],
            author: "human".into(),
        },
    };
    let title = required_single(args.title, "Title", as_json)?;
    let content = required_multiline(args.content, "Content", as_json)?;
    let journal = Journal::open(project)?;
    let mut entry = Entry::new(title, category, content, author_record(&args.author));
    entry.source = source_for(&args.author);
    entry.tags = args.tags;
    let path = journal.create_entry(&entry)?;
    emit(
        as_json,
        json!({"path": path, "entry": entry}),
        path.display().to_string(),
    );
    Ok(())
}

fn required_single(
    value: Option<String>,
    label: &str,
    as_json: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(value) = value {
        if !value.trim().is_empty() {
            return Ok(value);
        }
    }
    if as_json || !io::stdin().is_terminal() {
        return Err(Box::new(Error::Invalid(format!(
            "{label} is required; provide it as an argument (interactive prompts require a terminal)"
        ))));
    }
    eprint!("{label} (type :cancel to abort): ");
    io::stderr().flush()?;
    let mut value = String::new();
    io::stdin().lock().read_line(&mut value)?;
    let value = value.trim_end().to_owned();
    if value == ":cancel" {
        return Err(Box::new(Error::Invalid("cancelled".into())));
    }
    if value.trim().is_empty() {
        return Err(Box::new(Error::Invalid(format!("{label} cannot be empty"))));
    }
    Ok(value)
}

fn required_multiline(
    value: Option<String>,
    label: &str,
    as_json: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(value) = value {
        if !value.trim().is_empty() {
            return Ok(value);
        }
    }
    if as_json || !io::stdin().is_terminal() {
        return Err(Box::new(Error::Invalid(format!(
            "{label} is required; provide --content (interactive prompts require a terminal)"
        ))));
    }
    eprintln!("{label}; enter :cancel to abort or a single . on a line to finish:");
    match read_multiline(io::stdin().lock())? {
        Some(content) => Ok(content),
        None => Err(Box::new(Error::Invalid(format!(
            "{label} was empty or cancelled"
        )))),
    }
}

fn read_multiline<R: io::BufRead>(reader: R) -> io::Result<Option<String>> {
    let mut lines = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line == ":cancel" {
            return Ok(None);
        }
        if line == "." {
            break;
        }
        lines.push(line);
    }
    let value = lines.join("\n");
    Ok((!value.trim().is_empty()).then_some(value))
}

fn list(
    project: &PathBuf,
    category: Category,
    as_json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let entries: Vec<_> = Journal::open(project)?
        .entries()?
        .into_iter()
        .filter(|entry| entry.category == category)
        .collect();
    emit(as_json, json!(entries), format_entries(&entries));
    Ok(())
}

fn emit(as_json: bool, value: Value, human: String) {
    if as_json {
        println!("{value}");
    } else {
        println!("{human}");
    }
}
fn emit_error(code: &str, message: &str) {
    eprintln!("{}", json!({"error":{"code":code,"message":message}}));
}
fn error_details(error: &(dyn std::error::Error + 'static)) -> (&'static str, String) {
    if let Some(error) = error.downcast_ref::<Error>() {
        match error {
            Error::NotFound(item) => (
                if item.starts_with("dj-") {
                    "ENTRY_NOT_FOUND"
                } else {
                    "JOURNAL_NOT_FOUND"
                },
                error.to_string(),
            ),
            Error::AlreadyExists(_) => ("ENTRY_ALREADY_EXISTS", error.to_string()),
            Error::Invalid(_) => ("INVALID_INPUT", error.to_string()),
            Error::Io(_) => ("STORAGE_ERROR", error.to_string()),
        }
    } else {
        ("OPERATION_FAILED", error.to_string())
    }
}
fn format_entries(entries: &[Entry]) -> String {
    entries
        .iter()
        .map(|entry| {
            format!(
                "{}\t{}\t{}\t{}",
                entry.id,
                entry.created_at.format("%Y-%m-%d"),
                entry.title,
                entry.status.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn author_record(identity: &str) -> Author {
    let kind = if identity == "human" {
        AuthorType::Human
    } else if identity == "system" {
        AuthorType::System
    } else {
        AuthorType::Ai
    };
    Author {
        kind,
        name: identity.to_owned(),
    }
}
fn source_for(identity: &str) -> EntrySource {
    EntrySource {
        kind: if identity == "human" {
            EntrySourceType::Manual
        } else if identity == "system" {
            EntrySourceType::System
        } else {
            EntrySourceType::Agent
        },
        detail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn multiline_prompt_content_supports_unicode_and_blank_lines() {
        assert_eq!(
            read_multiline(Cursor::new("line one\n\nनमस्ते\n.\n"))
                .unwrap()
                .as_deref(),
            Some("line one\n\nनमस्ते")
        );
        assert_eq!(read_multiline(Cursor::new(":cancel\n")).unwrap(), None);
        assert_eq!(read_multiline(Cursor::new(".\n")).unwrap(), None);
    }
}
