//! MCP tool saving a file description into memory.

use kameo::actor::ActorRef;
use rust_mcp_sdk::macros::{JsonSchema, mcp_tool};

use crate::actors::{UpsertMessage, project_info::ProjectInfoActor};

/// Category for per-file content descriptions (unique id = file path).
const FILE_CATEGORY: &str = "file";

/// Save a description of a file's content into project memory.
#[mcp_tool(
    name = "save_file_description",
    description = "Save description of a file's content"
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct SaveFileDescription {
    /// Unique record key: relative path to the file
    /// (for example, `src/auth.rs`).
    pub file_path: String,
    /// Short description of the file's content:
    /// purpose, key functions/types it defines, how
    /// it is used.
    pub description: String,
    /// Project scope; defaults to the server's
    /// PROJECT_NAME when omitted.
    pub project: Option<String>,
}

/// Executes the save-file tool by upserting the file
/// description under the `file` category.
pub async fn execute(actor: ActorRef<ProjectInfoActor>, args: SaveFileDescription) -> String {
    let msg = UpsertMessage {
        id: args.file_path,
        content: args.description,
        category: FILE_CATEGORY.to_string(),
        project: args.project.unwrap_or_default(),
    };
    match actor.ask(msg).await {
        Ok(text) => text,
        Err(e) => format!("❌ Failed to send message to Kameo actor: {e}"),
    }
}
