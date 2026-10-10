//! MCP tool deleting a project info record from memory.

use kameo::actor::ActorRef;
use rust_mcp_sdk::macros::{JsonSchema, mcp_tool};

/// Delete a single project info record by `(id, project)`.
#[mcp_tool(name = "delete_project_info", description = "Delete project info")]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct DeleteProjectInfo {
    /// Record ID, unique key (for example, path to file or task ID).
    pub info_id: String,
    /// Project scope; defaults to the server's
    /// PROJECT_NAME when omitted.
    pub project: Option<String>,
}

/// Executes the delete tool by forwarding the
/// arguments to the project info actor.
pub async fn execute(
    actor: ActorRef<crate::actors::project_info::ProjectInfoActor>,
    args: DeleteProjectInfo,
) -> String {
    let info_id = args.info_id.clone();
    let project = args.project.clone().unwrap_or_default();
    let msg = crate::actors::DeletePointMessage {
        id: args.info_id,
        project,
    };
    match actor.ask(msg).await {
        Ok(true) => format!(
            "✅ [Kameo] Data '{info_id}' deleted from project memory{}.",
            args.project
                .map(|p| format!(" (project '{p}')"))
                .unwrap_or_default()
        ),
        Ok(false) => format!("ℹ️ [Kameo] Data '{info_id}' not found, nothing deleted."),
        Err(e) => format!("❌ Delete failed: {e}"),
    }
}
