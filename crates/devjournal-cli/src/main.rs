use std::{path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand, ValueEnum};
use devjournal_core::{
    Author, AuthorType, Category, Entry, EntrySource, EntrySourceType, Journal, JournalConfig,
};

#[derive(Parser)]
#[command(
    name = "devjournal",
    version,
    about = "Local-first project development journal"
)]
struct Cli {
    #[arg(long, global = true, default_value = ".")]
    project: PathBuf,
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
        command: NewCommand,
    },
    Decision {
        #[command(subcommand)]
        command: NewCommand,
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
        topic: String,
    },
    Show {
        id: String,
    },
}

#[derive(Subcommand)]
enum NewCommand {
    New(NewArgs),
}
#[derive(Args)]
struct NewArgs {
    title: String,
    #[arg(long)]
    content: String,
    #[arg(long, value_delimiter = ',')]
    tags: Vec<String>,
    #[arg(long, default_value = "human")]
    author: String,
}
#[derive(Subcommand)]
enum SessionCommand {
    Start {
        title: String,
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
        status: String,
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
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("devjournal: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Command::Init { name, description } => {
            let name = name.unwrap_or_else(|| {
                cli.project
                    .file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or("project")
                    .to_owned()
            });
            let mut config = JournalConfig::defaults(name);
            config.project.description = description;
            let journal = Journal::init(&cli.project, config)?;
            println!("Initialized {}", journal.root().display());
        }
        Command::Status => {
            let journal = Journal::open(&cli.project)?;
            let config = journal.config()?;
            println!(
                "{} — {}",
                config.project.name,
                if config.local_only {
                    "local-only"
                } else {
                    "sync policy not configured"
                }
            );
            println!("{} entries", journal.entries()?.len());
        }
        Command::Note {
            command: NewCommand::New(args),
        } => create(&cli.project, Category::Note, args)?,
        Command::Decision {
            command: NewCommand::New(args),
        } => create(&cli.project, Category::Decision, args)?,
        Command::Session {
            command: SessionCommand::Start { title, author },
        } => {
            let journal = Journal::open(&cli.project)?;
            let mut entry = Entry::new(
                title,
                Category::Session,
                "Session started. Add a summary by editing this Markdown file.",
                author_record(&author),
            );
            entry.source = source_for(&author);
            println!("{}", journal.create_entry(&entry)?.display());
        }
        Command::Session {
            command: SessionCommand::List,
        } => list(&cli.project, Category::Session)?,
        Command::Task {
            command: TaskCommand::List,
        } => list(&cli.project, Category::Task)?,
        Command::Task {
            command: TaskCommand::Update { id, status, author },
        } => {
            let journal = Journal::open(&cli.project)?;
            let old = journal.get_entry(&id)?;
            if old.category != Category::Task {
                return Err(format!("{id} is not a task").into());
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
            println!(
                "Created task revision {}",
                journal.create_entry(&update)?.display()
            );
        }
        Command::Search { query, category } => {
            let journal = Journal::open(&cli.project)?;
            for entry in journal.search(&query, category.map(Into::into))? {
                print_entry(&entry);
            }
        }
        Command::Context { topic } => {
            let journal = Journal::open(&cli.project)?;
            let matches = journal.search(&topic, None)?;
            for entry in matches.into_iter().take(10) {
                print_entry(&entry);
            }
        }
        Command::Show { id } => {
            let journal = Journal::open(&cli.project)?;
            let entry = journal.get_entry(&id)?;
            println!("# {}\n\n{}", entry.title, entry.content);
            println!(
                "\nID: {} | category: {} | author: {:?} {} | created: {}",
                entry.id,
                entry.category.as_str(),
                entry.author.kind,
                entry.author.name,
                entry.created_at
            );
        }
    }
    Ok(())
}

fn create(
    project: &PathBuf,
    category: Category,
    args: NewArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let journal = Journal::open(project)?;
    let mut entry = Entry::new(
        args.title,
        category,
        args.content,
        author_record(&args.author),
    );
    entry.source = source_for(&args.author);
    entry.tags = args.tags;
    println!("{}", journal.create_entry(&entry)?.display());
    Ok(())
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
fn list(project: &PathBuf, category: Category) -> Result<(), Box<dyn std::error::Error>> {
    let journal = Journal::open(project)?;
    for entry in journal
        .entries()?
        .into_iter()
        .filter(|entry| entry.category == category)
    {
        print_entry(&entry);
    }
    Ok(())
}
fn print_entry(entry: &Entry) {
    println!(
        "{}\t{}\t{}\t{}",
        entry.id,
        entry.created_at.format("%Y-%m-%d"),
        entry.title,
        entry.status.as_deref().unwrap_or("")
    );
}
