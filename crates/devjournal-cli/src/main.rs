use std::{
    io::{self, BufRead, IsTerminal, Write},
    path::PathBuf,
    process::ExitCode,
};

use clap::{Args, Parser, Subcommand, ValueEnum, error::ErrorKind};
use devjournal_core::{
    Author, AuthorType, Category, ContextItem, ContextReason, Entry, EntrySource, EntrySourceType,
    Error, Journal, JournalConfig, KnowledgeGraph, Relationship, RelationshipType, SearchIndex,
    retrieve_context,
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
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Context {
        topic: Option<String>,
        #[arg(long = "topic")]
        topic_option: Option<String>,
        #[arg(long)]
        entry: Option<String>,
        #[arg(long, default_value_t = 1)]
        depth: usize,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Link {
        source: String,
        target: String,
        #[arg(long = "type", value_enum)]
        kind: RelationshipTypeArg,
        #[arg(long, default_value = "human")]
        author: String,
    },
    Unlink {
        relationship_id: String,
        #[arg(long, default_value = "human")]
        author: String,
    },
    Relationships {
        #[command(subcommand)]
        command: RelationshipsCommand,
    },
    Graph {
        #[command(subcommand)]
        command: GraphCommand,
    },
    Index {
        #[command(subcommand)]
        command: IndexCommand,
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
enum RelationshipsCommand {
    List {
        entry_id: String,
        #[arg(long)]
        incoming: bool,
        #[arg(long = "type", value_enum)]
        kind: Option<RelationshipTypeArg>,
    },
}
#[derive(Subcommand)]
enum GraphCommand {
    Neighbors {
        entry_id: String,
        #[arg(long, default_value_t = 1)]
        depth: usize,
        #[arg(long = "type", value_enum)]
        kind: Option<RelationshipTypeArg>,
    },
    Path {
        source_id: String,
        target_id: String,
    },
    Related {
        entry_id: String,
        #[arg(long, default_value_t = 1)]
        depth: usize,
    },
    Stats,
}
#[derive(Subcommand)]
enum IndexCommand {
    Status,
    Verify,
    Rebuild,
    Update,
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

#[derive(Clone, Copy, ValueEnum)]
enum RelationshipTypeArg {
    #[value(name = "relates_to")]
    RelatesTo,
    Implements,
    #[value(name = "depends_on")]
    DependsOn,
    Supersedes,
    #[value(name = "discovered_during")]
    DiscoveredDuring,
    #[value(name = "caused_by")]
    CausedBy,
    Resolves,
    References,
    #[value(name = "derived_from")]
    DerivedFrom,
    Blocks,
}
impl From<RelationshipTypeArg> for RelationshipType {
    fn from(value: RelationshipTypeArg) -> Self {
        match value {
            RelationshipTypeArg::RelatesTo => Self::RelatesTo,
            RelationshipTypeArg::Implements => Self::Implements,
            RelationshipTypeArg::DependsOn => Self::DependsOn,
            RelationshipTypeArg::Supersedes => Self::Supersedes,
            RelationshipTypeArg::DiscoveredDuring => Self::DiscoveredDuring,
            RelationshipTypeArg::CausedBy => Self::CausedBy,
            RelationshipTypeArg::Resolves => Self::Resolves,
            RelationshipTypeArg::References => Self::References,
            RelationshipTypeArg::DerivedFrom => Self::DerivedFrom,
            RelationshipTypeArg::Blocks => Self::Blocks,
        }
    }
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
        Command::Search {
            query,
            category,
            limit,
        } => {
            let journal = Journal::open(&project)?;
            let mut index = SearchIndex::open(&journal)?;
            index.sync(&journal)?;
            let entries = index
                .search_ids(&query, category.map(Into::into), limit.min(100))?
                .iter()
                .map(|id| journal.get_entry(id))
                .collect::<Result<Vec<_>, _>>()?;
            emit(as_json, json!(entries), format_entries(&entries));
        }
        Command::Context {
            topic,
            topic_option,
            entry,
            depth,
            limit,
        } => {
            if topic.is_some() && topic_option.is_some() {
                return Err(Box::new(Error::Invalid(
                    "provide the context topic once, either positionally or with --topic".into(),
                )));
            }
            let topic = topic.or(topic_option);
            let journal = Journal::open(&project)?;
            let results =
                retrieve_context(&journal, topic.as_deref(), entry.as_deref(), depth, limit)?;
            emit(
                as_json,
                json!({"topic": topic, "entry_id": entry, "depth": depth, "results": results}),
                format_context(&results),
            );
        }
        Command::Link {
            source,
            target,
            kind,
            author,
        } => {
            let journal = Journal::open(&project)?;
            let relationship = Relationship::new(
                source,
                target,
                kind.into(),
                author_record(&author),
                source_for(&author),
            );
            let path = journal.create_relationship(&relationship)?;
            emit(
                as_json,
                json!({"path": path, "relationship": relationship}),
                path.display().to_string(),
            );
        }
        Command::Unlink {
            relationship_id,
            author,
        } => {
            let journal = Journal::open(&project)?;
            let path = journal.unlink_relationship(
                &relationship_id,
                author_record(&author),
                source_for(&author),
            )?;
            emit(
                as_json,
                json!({"relationship_id": relationship_id, "tombstone": path}),
                format!("Unlinked {relationship_id}"),
            );
        }
        Command::Relationships {
            command:
                RelationshipsCommand::List {
                    entry_id,
                    incoming,
                    kind,
                },
        } => {
            let journal = Journal::open(&project)?;
            journal.get_entry(&entry_id)?;
            let kind = kind.map(Into::into);
            let relationships: Vec<_> = journal
                .relationships()?
                .into_iter()
                .filter(|relation| {
                    (if incoming {
                        relation.target_id == entry_id
                            || (relation.kind.is_symmetric() && relation.source_id == entry_id)
                    } else {
                        relation.source_id == entry_id
                            || (relation.kind.is_symmetric() && relation.target_id == entry_id)
                    }) && kind.is_none_or(|filter| relation.kind == filter)
                })
                .collect();
            emit(
                as_json,
                json!(relationships),
                format_relationships(&relationships),
            );
        }
        Command::Graph { command } => {
            let journal = Journal::open(&project)?;
            let graph = indexed_graph(&journal)?;
            match command {
                GraphCommand::Neighbors {
                    entry_id,
                    depth,
                    kind,
                } => {
                    let hits = graph.neighbors(&entry_id, depth, kind.map(Into::into))?;
                    let results = graph_hits_json(&journal, &hits)?;
                    emit(as_json, json!(results), format_graph_hits(&journal, &hits)?);
                }
                GraphCommand::Related { entry_id, depth } => {
                    let hits = graph.neighbors(&entry_id, depth, None)?;
                    let results = graph_hits_json(&journal, &hits)?;
                    emit(as_json, json!(results), format_graph_hits(&journal, &hits)?);
                }
                GraphCommand::Path {
                    source_id,
                    target_id,
                } => {
                    let path = graph.path(&source_id, &target_id)?;
                    let entries = path
                        .as_ref()
                        .map(|ids| {
                            ids.iter()
                                .map(|id| journal.get_entry(id))
                                .collect::<Result<Vec<_>, _>>()
                        })
                        .transpose()?;
                    emit(
                        as_json,
                        json!({"source_id":source_id,"target_id":target_id,"path":entries}),
                        path.map(|ids| ids.join(" -> "))
                            .unwrap_or_else(|| "No path found".into()),
                    );
                }
                GraphCommand::Stats => {
                    let stats = graph.stats();
                    emit(
                        as_json,
                        json!(stats),
                        format!(
                            "{} entries, {} relationships; directed cycle: {}",
                            stats.nodes, stats.relationships, stats.has_directed_cycle
                        ),
                    );
                }
            }
        }
        Command::Index { command } => {
            let journal = Journal::open(&project)?;
            match command {
                IndexCommand::Status => {
                    let status = SearchIndex::status(&journal)?;
                    emit(
                        as_json,
                        json!(status),
                        format!(
                            "Index {}: {} entries, {} relationships",
                            if status.available {
                                "available"
                            } else {
                                "not built"
                            },
                            status.entries,
                            status.relationships
                        ),
                    );
                }
                IndexCommand::Verify => {
                    let report = SearchIndex::verify_existing(&journal)?;
                    emit(
                        as_json,
                        json!(report),
                        if report.consistent {
                            "Index is consistent".into()
                        } else {
                            "Index needs rebuild or update".into()
                        },
                    );
                }
                IndexCommand::Rebuild => {
                    let index = SearchIndex::rebuild(&journal)?;
                    let report = index.verify(&journal)?;
                    emit(
                        as_json,
                        json!({"status":SearchIndex::status(&journal)?,"verification":report}),
                        "Index rebuilt from Markdown".into(),
                    );
                }
                IndexCommand::Update => {
                    let mut index = SearchIndex::open(&journal)?;
                    index.sync(&journal)?;
                    let status = SearchIndex::status(&journal)?;
                    emit(
                        as_json,
                        json!(status),
                        format!(
                            "Index updated: {} entries, {} relationships",
                            status.entries, status.relationships
                        ),
                    );
                }
            }
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
    if let Some(value) = value
        && !value.trim().is_empty()
    {
        return Ok(value);
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
    if let Some(value) = value
        && !value.trim().is_empty()
    {
        return Ok(value);
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

fn indexed_graph(journal: &Journal) -> Result<KnowledgeGraph, Box<dyn std::error::Error>> {
    let mut index = SearchIndex::open(journal)?;
    index.sync(journal)?;
    Ok(index.graph()?)
}

fn format_relationships(relationships: &[Relationship]) -> String {
    relationships
        .iter()
        .map(|relation| {
            format!(
                "{}\t{}\t{}\t{}\t{}",
                relation.id,
                relation.source_id,
                relation.kind.as_str(),
                relation.target_id,
                relation.author.name
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_graph_hits(
    journal: &Journal,
    hits: &[devjournal_core::TraversalHit],
) -> Result<String, Box<dyn std::error::Error>> {
    hits.iter()
        .map(|hit| {
            Ok(format!(
                "{}\tdepth={}\t{}\t{}",
                hit.entry_id,
                hit.depth,
                hit.relationship_type.as_str(),
                journal.get_entry(&hit.entry_id)?.title
            ))
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()
        .map(|lines| lines.join("\n"))
}

fn graph_hits_json(
    journal: &Journal,
    hits: &[devjournal_core::TraversalHit],
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    hits.iter().map(|hit| {
        let entry = journal.get_entry(&hit.entry_id)?;
        Ok(json!({"entry":entry,"depth":hit.depth,"from_entry_id":hit.from_id,"relationship_id":hit.relationship_id,"relationship_type":hit.relationship_type}))
    }).collect()
}

fn format_context(results: &[ContextItem]) -> String {
    results
        .iter()
        .map(|item| {
            let reason = match &item.reason {
                ContextReason::DirectTextMatch => "direct text match".to_owned(),
                ContextReason::RequestedEntry => "requested entry".to_owned(),
                ContextReason::RecentProjectActivity => "recent project activity".to_owned(),
                ContextReason::Relationship {
                    relationship_type,
                    from_entry_id,
                    ..
                } => format!("via {} from {}", relationship_type.as_str(), from_entry_id),
            };
            format!(
                "{}\tdepth={}\t{}\t[{:?}: {}]\n{}",
                item.entry.id,
                item.depth,
                reason,
                item.entry.author.kind,
                item.entry.author.name,
                item.entry.title
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
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
            Error::Index(_) | Error::Sqlite(_) => ("INDEX_ERROR", error.to_string()),
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
