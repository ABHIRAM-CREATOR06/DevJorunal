use std::path::PathBuf;

use chrono::{DateTime, Utc};
use clap::Parser;
use devjournal_core::{Category, Journal, SearchFilters, SearchIndex, retrieve_context};
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters, model::*, schemars, service::RequestContext, tool,
    tool_handler, tool_router, transport::stdio,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Parser)]
#[command(name = "devjournal-mcp", about = "Local DevJournal MCP server")]
struct Cli {
    #[arg(long, default_value = ".")]
    project: PathBuf,
}

#[derive(Clone)]
struct DevJournalServer {
    project: PathBuf,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct SearchArgs {
    /// Full-text query.
    query: String,
    /// Optional entry category.
    category: Option<String>,
    /// Exact author name.
    author: Option<String>,
    /// Exact tag.
    tag: Option<String>,
    /// Exact status.
    status: Option<String>,
    /// Earliest creation time in RFC3339 format.
    created_after: Option<String>,
    /// Latest creation time in RFC3339 format.
    created_before: Option<String>,
    /// Maximum number of results (1–100).
    limit: Option<usize>,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct EntryArgs {
    /// DevJournal entry ID.
    entry_id: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct ContextArgs {
    /// Text topic to search for.
    topic: Option<String>,
    /// Start from a specific entry ID.
    entry_id: Option<String>,
    /// Relationship traversal depth (0–8).
    depth: Option<usize>,
    /// Maximum result count (0–100).
    limit: Option<usize>,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct NeighborsArgs {
    /// Starting DevJournal entry ID.
    entry_id: String,
    /// Relationship traversal depth (0–8).
    depth: Option<usize>,
}

#[tool_router]
impl DevJournalServer {
    fn journal(&self) -> Result<Journal, McpError> {
        Journal::open(&self.project).map_err(internal_error)
    }

    #[tool(
        description = "Search local DevJournal entries with optional metadata filters. Results are JSON text."
    )]
    fn search_entries(&self, Parameters(args): Parameters<SearchArgs>) -> Result<String, McpError> {
        let category = args.category.as_deref().map(parse_category).transpose()?;
        let parse_date = |value: Option<String>| -> Result<Option<DateTime<Utc>>, McpError> {
            value
                .map(|value| {
                    DateTime::parse_from_rfc3339(&value)
                        .map(|timestamp| timestamp.with_timezone(&Utc))
                        .map_err(|error| {
                            McpError::invalid_params(
                                format!("invalid RFC3339 timestamp: {error}"),
                                None,
                            )
                        })
                })
                .transpose()
        };
        let filters = SearchFilters {
            category,
            author: args.author,
            tag: args.tag,
            status: args.status,
            created_after: parse_date(args.created_after)?,
            created_before: parse_date(args.created_before)?,
        };
        if filters
            .created_after
            .zip(filters.created_before)
            .is_some_and(|(start, end)| start > end)
        {
            return Err(McpError::invalid_params(
                "created_after must not be later than created_before",
                None,
            ));
        }
        let journal = self.journal()?;
        let mut index = SearchIndex::open(&journal).map_err(internal_error)?;
        index.sync(&journal).map_err(internal_error)?;
        let ids = index
            .search_ids_filtered(&args.query, &filters, args.limit.unwrap_or(20).min(100))
            .map_err(internal_error)?;
        let entries = ids
            .iter()
            .map(|id| journal.get_entry(id))
            .collect::<devjournal_core::Result<Vec<_>>>()
            .map_err(internal_error)?;
        json_text(&entries)
    }

    #[tool(description = "Read one DevJournal entry by ID.")]
    fn get_entry(&self, Parameters(args): Parameters<EntryArgs>) -> Result<String, McpError> {
        let entry = self
            .journal()?
            .get_entry(&args.entry_id)
            .map_err(internal_error)?;
        json_text(&entry)
    }

    #[tool(
        description = "Retrieve matching DevJournal entries and their linked context, with reasons."
    )]
    fn get_context(&self, Parameters(args): Parameters<ContextArgs>) -> Result<String, McpError> {
        let depth = args.depth.unwrap_or(1);
        let limit = args.limit.unwrap_or(20);
        if depth > 8 || limit > 100 {
            return Err(McpError::invalid_params(
                "depth must be at most 8 and limit at most 100",
                None,
            ));
        }
        let journal = self.journal()?;
        let context = retrieve_context(
            &journal,
            args.topic.as_deref(),
            args.entry_id.as_deref(),
            depth,
            limit,
        )
        .map_err(internal_error)?;
        json_text(&context)
    }

    #[tool(
        description = "List graph neighbors for an entry, including edge type, traversal depth and source entry."
    )]
    fn graph_neighbors(
        &self,
        Parameters(args): Parameters<NeighborsArgs>,
    ) -> Result<String, McpError> {
        let depth = args.depth.unwrap_or(1);
        if depth > 8 {
            return Err(McpError::invalid_params("depth must be at most 8", None));
        }
        let journal = self.journal()?;
        let mut index = SearchIndex::open(&journal).map_err(internal_error)?;
        index.sync(&journal).map_err(internal_error)?;
        let hits = index
            .graph()
            .map_err(internal_error)?
            .neighbors(&args.entry_id, depth, None)
            .map_err(internal_error)?;
        let results = hits
            .into_iter()
            .map(|hit| {
                Ok(json!({
                    "entry": journal.get_entry(&hit.entry_id)?,
                    "depth": hit.depth,
                    "from_entry_id": hit.from_id,
                    "relationship_id": hit.relationship_id,
                    "relationship_type": hit.relationship_type,
                }))
            })
            .collect::<devjournal_core::Result<Vec<_>>>()
            .map_err(internal_error)?;
        json_text(&results)
    }

    #[tool(
        description = "Generate a deterministic overview from the project's saved journal records."
    )]
    fn project_overview(&self) -> Result<String, McpError> {
        let overview = self.journal()?.project_overview().map_err(internal_error)?;
        json_text(&overview)
    }

    #[tool(
        description = "Generate a deterministic summary of a saved session and its linked records."
    )]
    fn session_summary(&self, Parameters(args): Parameters<EntryArgs>) -> Result<String, McpError> {
        let summary = self
            .journal()?
            .session_summary(&args.entry_id)
            .map_err(internal_error)?;
        json_text(&summary)
    }

    fn read_resource_text(&self, uri: &str) -> Result<String, McpError> {
        let journal = self.journal()?;
        match uri {
            "devjournal://project/overview" => {
                let overview = journal.project_overview().map_err(internal_error)?;
                json_text(&overview)
            }
            uri if uri.starts_with("devjournal://entries/") => {
                let id = uri.trim_start_matches("devjournal://entries/");
                let entry = journal.get_entry(id).map_err(internal_error)?;
                json_text(&entry)
            }
            uri if uri.starts_with("devjournal://sessions/") && uri.ends_with("/summary") => {
                let id = uri
                    .trim_start_matches("devjournal://sessions/")
                    .trim_end_matches("/summary");
                let summary = journal.session_summary(id).map_err(internal_error)?;
                Ok(summary.markdown)
            }
            _ => Err(McpError::resource_not_found(
                "DevJournal resource not found",
                Some(json!({"uri": uri})),
            )),
        }
    }
}

#[tool_handler]
impl ServerHandler for DevJournalServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(vec![Resource::new(
            "devjournal://project/overview",
            "project-overview",
        )]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        let body = self.read_resource_text(&request.uri)?;
        Ok(ReadResourceResult::new(vec![ResourceContents::text(body, &request.uri)]).into())
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult::with_all_items(vec![
            ResourceTemplate::new("devjournal://entries/{entry_id}", "journal-entry"),
            ResourceTemplate::new(
                "devjournal://sessions/{entry_id}/summary",
                "session-summary",
            ),
        ]))
    }
}

fn parse_category(value: &str) -> Result<Category, McpError> {
    match value {
        "session" => Ok(Category::Session),
        "note" => Ok(Category::Note),
        "decision" => Ok(Category::Decision),
        "architecture" => Ok(Category::Architecture),
        "research" => Ok(Category::Research),
        "debugging" => Ok(Category::Debugging),
        "issue" => Ok(Category::Issue),
        "task" => Ok(Category::Task),
        "meeting" => Ok(Category::Meeting),
        "retrospective" => Ok(Category::Retrospective),
        "convention" => Ok(Category::Convention),
        "question" => Ok(Category::Question),
        _ => Err(McpError::invalid_params(
            format!("unknown category: {value}"),
            None,
        )),
    }
}

fn json_text(value: &impl serde::Serialize) -> Result<String, McpError> {
    serde_json::to_string(value).map_err(internal_error)
}

fn internal_error(error: impl std::fmt::Display) -> McpError {
    McpError::internal_error(error.to_string(), None)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    Journal::open(&cli.project)?;
    let server = DevJournalServer {
        project: cli.project,
    };
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use devjournal_core::{Author, AuthorType, Entry, JournalConfig};

    #[test]
    fn resource_uris_restrict_reads_to_journal_ids_and_known_resources() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::init(dir.path(), JournalConfig::defaults("mcp-test")).unwrap();
        let entry = Entry::new(
            "Stored note",
            Category::Note,
            "safe journal content",
            Author {
                kind: AuthorType::Human,
                name: "Ada".into(),
            },
        );
        journal.create_entry(&entry).unwrap();
        let server = DevJournalServer {
            project: dir.path().to_owned(),
        };
        assert!(
            server
                .read_resource_text(&format!("devjournal://entries/{}", entry.id))
                .unwrap()
                .contains("safe journal content")
        );
        assert!(
            server
                .read_resource_text("devjournal://entries/../../config.toml")
                .is_err()
        );
        assert!(server.read_resource_text("file:///etc/passwd").is_err());
    }
}
